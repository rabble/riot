//! WU-019 increment 1: the anchor daemon end-to-end over REAL local iroh.
//!
//! Two in-process endpoints, no relay (`bind_local_anchor_endpoint` uses the
//! `N0DisableRelay` preset, mirroring `crates/riot-transport/tests/followed_site.rs`
//! and `alpn_router.rs`). The daemon runs [`serve`] over its local endpoint; a
//! client dials it with the `riot/anchor/1` ALPN, opens the one bidirectional
//! stream, and speaks length-prefixed control frames. This drives the accept
//! loop + handler + single-writer actor + repository over a genuine QUIC
//! connection.
//!
//! Two cases: a valid root-signed PrepareHost is admitted (signed PrepareSuccess
//! back), and a `require_arti` ticket is refused (no PrepareSuccess). Everything
//! is bounded by timeouts so it cannot hang or flake in CI.

#![cfg(feature = "daemon")]

mod hosting_common;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio::time::timeout;

use riot_anchor::admission::IngressLimits;
use riot_anchor::config::{assemble_service, derive, resolve_config, Config};
use riot_anchor::daemon::{bind_local_anchor_endpoint, serve, DaemonError, EntropyFn};
use riot_anchor::repository::{AnchorRepository, AnchorRepositoryError};
use riot_anchor::work::TokenSecretRing;

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::control::{
    CommitHostV1, ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1,
    ControlResponseV1, ControlSuccess, DescribeV1, GetWorkChallengeV1, PrepareHostV1,
    PrepareSuccessV1, MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::records::{
    ControlOperationKind, PublicSiteTicketV2Core, RootSignedTicketCoreEnvelopeV2, TransportFloor,
    MAX_TICKET_CORE_BYTES,
};
use riot_anchor_protocol::sync2::{
    Sync2Action, Sync2Frame, Sync2Refusal, Sync2Repository, Sync2Session, MAX_SYNC2_FRAME_BYTES,
};

use riot_core::sync::MAX_SYNC_FRAME_BYTES;
use riot_transport::iroh::dialable_addr;
use riot_transport::{ALPN_ANCHOR_V1, ALPN_SYNC_V2};

use hosting_common::{
    client_snapshot_digest, insert_prepared_operation, make_item, make_site_fixture,
    pull_initiator, push_initiator, SiteFixture, SyncItem,
};

const STEP: Duration = Duration::from_secs(15);

fn unique_db() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!("riot-anchor-e2e-{}-{}.db", std::process::id(), id));
    let _ = std::fs::remove_file(&path);
    path
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// A root-signed ticket whose validity window brackets the daemon's real clock.
fn signed_ticket(
    root: &SigningKey,
    mutate: impl FnOnce(&mut PublicSiteTicketV2Core),
) -> RootSignedTicketCoreEnvelopeV2 {
    let now = now_secs();
    let mut core = PublicSiteTicketV2Core {
        root_id: root.verifying_key().to_bytes(),
        o_namespace_id: [10u8; 32],
        c_namespace_id: [11u8; 32],
        w_namespace_id: [12u8; 32],
        manifest_digest: [13u8; 32],
        manifest_version: 3,
        min_sync_version: 2,
        manifest_required_transport: TransportFloor::RequireNone,
        transport_floor: TransportFloor::RequireNone,
        transport_epoch: 1,
        issued_unix_seconds: now.saturating_sub(100),
        expiry_unix_seconds: now + 3600,
    };
    mutate(&mut core);
    let mut env = RootSignedTicketCoreEnvelopeV2 {
        core,
        root_signature: [0u8; 64],
    };
    let preimage = env.signing_preimage().unwrap();
    env.root_signature = root.sign(&preimage).to_bytes();
    env
}

fn prepare_frame(ticket: RootSignedTicketCoreEnvelopeV2) -> Vec<u8> {
    prepare_frame_with_key(ticket, [1u8; 16])
}

fn prepare_frame_with_key(
    ticket: RootSignedTicketCoreEnvelopeV2,
    idempotency_key: [u8; 16],
) -> Vec<u8> {
    let body = PrepareHostV1 {
        root_signed_ticket_core: ticket,
        ordered_namespace_snapshot_digests: [[30u8; 32], [31u8; 32], [32u8; 32]],
        work_stamp: None,
    };
    ControlRequestV1 {
        idempotency_key,
        operation: ControlOperation::PrepareHost(Box::new(body)),
    }
    .encode_canonical()
    .expect("encode prepare request")
}

async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, body: &[u8]) {
    w.write_all(&(body.len() as u32).to_be_bytes())
        .await
        .unwrap();
    w.write_all(body).await.unwrap();
    w.flush().await.unwrap();
}

async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Vec<u8> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await.unwrap();
    let n = u32::from_be_bytes(len) as usize;
    let mut body = vec![0u8; n];
    r.read_exact(&mut body).await.unwrap();
    body
}

/// Dial the daemon, send one PrepareHost frame, and return the decoded response.
async fn round_trip(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    ticket: RootSignedTicketCoreEnvelopeV2,
) -> ControlResponseV1 {
    control_round_trip(client, daemon_addr, prepare_frame(ticket)).await
}

async fn control_round_trip(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    frame: Vec<u8>,
) -> ControlResponseV1 {
    let conn = timeout(STEP, client.connect(daemon_addr, ALPN_ANCHOR_V1))
        .await
        .expect("dial did not time out")
        .expect("dial connects");
    let (send, recv) = conn.open_bi().await.expect("open bi-stream");
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);

    write_frame(&mut send, &frame).await;
    let bytes = timeout(STEP, read_frame(&mut recv))
        .await
        .expect("response arrived before timeout");
    // Close cleanly so the server session ends promptly.
    let _ = send.shutdown().await;
    decode_canonical::<ControlResponseV1>(&bytes, MAX_CONTROL_FRAME_BYTES).unwrap()
}

fn daemon_config() -> (Config, PathBuf) {
    let db_path = unique_db();
    let config = daemon_config_for(&db_path, "E2E Anchor");
    (config, db_path)
}

fn daemon_config_for(db_path: &std::path::Path, display_label: &str) -> Config {
    daemon_config_with_sessions(
        db_path,
        display_label,
        IngressLimits::DEFAULT_MAX_CONTROL_SESSIONS,
    )
}

fn daemon_config_with_sessions(
    db_path: &std::path::Path,
    display_label: &str,
    max_control_sessions: usize,
) -> Config {
    let args = vec!["--db".to_string(), db_path.to_string_lossy().into_owned()];
    let env = vec![
        ("RIOT_ANCHOR_OPERATOR_KEY_HEX".to_string(), "07".repeat(32)),
        ("RIOT_ANCHOR_ENDPOINT_KEY_HEX".to_string(), "08".repeat(32)),
        (
            "RIOT_ANCHOR_HTTPS_ORIGIN".to_string(),
            "https://anchor.test".to_string(),
        ),
        (
            "RIOT_ANCHOR_DISPLAY_LABEL".to_string(),
            display_label.to_string(),
        ),
        ("RIOT_ANCHOR_FAILURE_DOMAIN".to_string(), "test".to_string()),
        (
            "RIOT_ANCHOR_MAX_CONTROL_SESSIONS".to_string(),
            max_control_sessions.to_string(),
        ),
    ];
    resolve_config(&args, &env).expect("test daemon config resolves")
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_serves_prepare_host_over_real_iroh_and_refuses_require_arti() {
    // The daemon endpoint (local, no relay) and its dialable address.
    let daemon_endpoint = bind_local_anchor_endpoint([100u8; 32])
        .await
        .expect("daemon endpoint binds");
    let daemon_addr = dialable_addr(&daemon_endpoint).await;

    // Run the control plane over that endpoint until shutdown.
    let (config, db_path) = daemon_config();
    let (daemon_config, service) = assemble_service(config);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let mut entropy_byte = 0x50u8;
    let entropy = Box::new(move || {
        let value = [entropy_byte; 32];
        entropy_byte = entropy_byte.wrapping_add(1);
        value
    });
    let serve_task = tokio::spawn(async move {
        serve(
            daemon_endpoint,
            daemon_config,
            service,
            entropy,
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
    });

    let client = bind_local_anchor_endpoint([200u8; 32])
        .await
        .expect("client endpoint binds");

    // 1) A valid, root-signed PrepareHost is admitted with a signed PrepareSuccess.
    let root = SigningKey::from_bytes(&[9u8; 32]);
    let response = round_trip(&client, daemon_addr.clone(), signed_ticket(&root, |_| {})).await;
    match response.outcome {
        ControlOutcome::Success(ControlSuccess::PrepareHost(success)) => {
            assert_ne!(
                success.operation_id, [0u8; 32],
                "an operation id was minted"
            );
            assert_eq!(
                success.ordered_namespace_host_plan,
                [[10u8; 32], [11u8; 32], [12u8; 32]],
                "the host plan is the ticket's O/C/W namespaces",
            );
        }
        other => panic!("expected PrepareSuccess over the wire, got {other:?}"),
    }

    // 2) A require_arti ticket is REFUSED — never a PrepareSuccess.
    let arti = signed_ticket(&root, |core| {
        core.transport_floor = TransportFloor::RequireArti;
    });
    let refused = round_trip(&client, daemon_addr, arti).await;
    assert!(
        matches!(refused.outcome, ControlOutcome::Refused(_)),
        "require_arti must be refused over the wire, got {:?}",
        refused.outcome
    );

    // Clean shutdown.
    let _ = shutdown_tx.send(());
    let served = timeout(STEP, serve_task)
        .await
        .expect("serve stops promptly")
        .expect("serve task joined");
    assert!(served.is_ok(), "serve returned Ok: {served:?}");

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_accepts_a_second_control_session_while_the_first_is_stalled() {
    let daemon_endpoint = bind_local_anchor_endpoint([101u8; 32])
        .await
        .expect("daemon endpoint binds");
    let daemon_addr = dialable_addr(&daemon_endpoint).await;

    let (config, db_path) = daemon_config();
    let (daemon_config, service) = assemble_service(config);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let serve_task = tokio::spawn(async move {
        serve(
            daemon_endpoint,
            daemon_config,
            service,
            Box::new(|| [0x51u8; 32]),
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
    });

    let client = bind_local_anchor_endpoint([201u8; 32])
        .await
        .expect("client endpoint binds");

    // Occupy one routed session with a partial length prefix. The progress
    // deadline is five seconds, so a serialized accept loop cannot service the
    // second connection within the two-second assertion window.
    let stalled = timeout(STEP, client.connect(daemon_addr.clone(), ALPN_ANCHOR_V1))
        .await
        .expect("first dial did not time out")
        .expect("first dial connects");
    let (mut stalled_send, _stalled_recv) = stalled.open_bi().await.expect("first bi-stream");
    stalled_send.write_all(&[0]).await.unwrap();
    stalled_send.flush().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let root = SigningKey::from_bytes(&[10u8; 32]);
    let second = timeout(
        Duration::from_secs(2),
        round_trip(&client, daemon_addr, signed_ticket(&root, |_| {})),
    )
    .await;

    drop(stalled_send);
    let _ = shutdown_tx.send(());
    let served = timeout(STEP, serve_task)
        .await
        .expect("serve stops promptly")
        .expect("serve task joined");
    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));

    assert!(served.is_ok(), "serve returned Ok: {served:?}");
    let response = second.expect("second control session was starved by the first");
    assert!(
        matches!(
            response.outcome,
            ControlOutcome::Success(ControlSuccess::PrepareHost(_))
        ),
        "second session should complete normally: {:?}",
        response.outcome,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_restart_reuses_the_persisted_descriptor_at_the_same_epoch() {
    let db_path = unique_db();
    let client = bind_local_anchor_endpoint([202u8; 32])
        .await
        .expect("client endpoint binds");
    let describe = ControlRequestV1 {
        idempotency_key: [0x77; 16],
        operation: ControlOperation::Describe(DescribeV1),
    }
    .encode_canonical()
    .unwrap();

    let first_endpoint = bind_local_anchor_endpoint([8u8; 32])
        .await
        .expect("first daemon endpoint binds");
    let first_addr = dialable_addr(&first_endpoint).await;
    let (first_config, first_service) =
        assemble_service(daemon_config_for(&db_path, "First label"));
    let (first_shutdown_tx, first_shutdown_rx) = oneshot::channel::<()>();
    let first_task = tokio::spawn(async move {
        serve(
            first_endpoint,
            first_config,
            first_service,
            Box::new(|| [0x61; 32]),
            async move {
                let _ = first_shutdown_rx.await;
            },
        )
        .await
    });
    let first = control_round_trip(&client, first_addr.clone(), describe.clone()).await;
    let root = SigningKey::from_bytes(&[11u8; 32]);
    let higher = signed_ticket(&root, |core| {
        core.transport_epoch = 5;
    });
    let first_prepare = control_round_trip(
        &client,
        first_addr,
        prepare_frame_with_key(higher, [0x81; 16]),
    )
    .await;
    assert!(matches!(
        first_prepare.outcome,
        ControlOutcome::Success(ControlSuccess::PrepareHost(_))
    ));
    let _ = first_shutdown_tx.send(());
    assert!(timeout(STEP, first_task).await.unwrap().unwrap().is_ok());

    // A changed label makes the freshly assembled epoch-0 descriptor differ.
    // Restart reconciliation must return the already-persisted descriptor,
    // never publish a second digest for the same epoch.
    let second_endpoint = bind_local_anchor_endpoint([8u8; 32])
        .await
        .expect("second daemon endpoint binds");
    let second_addr = dialable_addr(&second_endpoint).await;
    let (second_config, second_service) =
        assemble_service(daemon_config_for(&db_path, "Changed label"));
    let (second_shutdown_tx, second_shutdown_rx) = oneshot::channel::<()>();
    let second_task = tokio::spawn(async move {
        serve(
            second_endpoint,
            second_config,
            second_service,
            Box::new(|| [0x62; 32]),
            async move {
                let _ = second_shutdown_rx.await;
            },
        )
        .await
    });
    let second = control_round_trip(&client, second_addr.clone(), describe).await;
    let lower = signed_ticket(&root, |core| {
        core.transport_epoch = 4;
    });
    let second_prepare = control_round_trip(
        &client,
        second_addr,
        prepare_frame_with_key(lower, [0x82; 16]),
    )
    .await;
    assert!(
        matches!(
            second_prepare.outcome,
            ControlOutcome::Refused(ControlRefusal::InvalidTicketAuthority)
        ),
        "the durable ticket epoch floor must survive daemon restart",
    );
    let _ = second_shutdown_tx.send(());
    assert!(timeout(STEP, second_task).await.unwrap().unwrap().is_ok());

    let descriptor = |response: ControlResponseV1| match response.outcome {
        ControlOutcome::Success(ControlSuccess::Describe(success)) => success.descriptor,
        other => panic!("expected Describe success, got {other:?}"),
    };
    assert_eq!(
        descriptor(first).encode_canonical().unwrap(),
        descriptor(second).encode_canonical().unwrap(),
        "restart must not equivocate by publishing a different digest at epoch 0",
    );

    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

/// The accept-loop ingress ceiling (`max_concurrent_control_sessions`) REFUSES
/// excess connections at accept — a connection-level error before any stream or
/// frame exchange, not queueing — and a released permit restores service.
#[tokio::test(flavor = "multi_thread")]
async fn daemon_refuses_connections_beyond_the_ingress_ceiling() {
    let daemon_endpoint = bind_local_anchor_endpoint([102u8; 32])
        .await
        .expect("daemon endpoint binds");
    let daemon_addr = dialable_addr(&daemon_endpoint).await;

    // A ceiling of exactly ONE control session.
    let db_path = unique_db();
    let config = daemon_config_with_sessions(&db_path, "Ceiling anchor", 1);
    let (daemon_config, service) = assemble_service(config);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let serve_task = tokio::spawn(async move {
        serve(
            daemon_endpoint,
            daemon_config,
            service,
            Box::new(|| [0x52u8; 32]),
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
    });

    let client = bind_local_anchor_endpoint([203u8; 32])
        .await
        .expect("client endpoint binds");

    // Connection 1 occupies the single ingress permit: its handshake completes
    // and the session stalls mid-frame on a partial length prefix, holding the
    // permit (the progress deadline is five seconds, far beyond this test's
    // assertion windows).
    let first = timeout(STEP, client.connect(daemon_addr.clone(), ALPN_ANCHOR_V1))
        .await
        .expect("first dial did not time out")
        .expect("first dial connects");
    let (mut first_send, first_recv) = first.open_bi().await.expect("first bi-stream");
    first_send.write_all(&[0]).await.unwrap();
    first_send.flush().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connection 2 must be REFUSED at accept: the dial itself fails before any
    // frame exchange. (Were the refusal only the router's dispatch-time `busy`,
    // the handshake — and thus this connect — would still succeed.)
    let refused = timeout(STEP, client.connect(daemon_addr.clone(), ALPN_ANCHOR_V1))
        .await
        .expect("second dial did not time out");
    assert!(
        refused.is_err(),
        "a connection beyond the ingress ceiling must be refused at accept, not served",
    );

    // Release the permit by closing connection 1 entirely.
    drop(first_send);
    drop(first_recv);
    drop(first);

    // A fresh connection must now be admitted and served end-to-end — the
    // refusal above was capacity, not breakage. Retry briefly while the daemon
    // reaps the first session and its permit.
    let root = SigningKey::from_bytes(&[12u8; 32]);
    let frame = prepare_frame(signed_ticket(&root, |_| {}));
    let give_up = tokio::time::Instant::now() + STEP;
    let response = loop {
        match client.connect(daemon_addr.clone(), ALPN_ANCHOR_V1).await {
            Ok(conn) => {
                let (send, recv) = conn.open_bi().await.expect("third bi-stream");
                let mut send = Box::pin(send);
                let mut recv = Box::pin(recv);
                write_frame(&mut send, &frame).await;
                let bytes = timeout(STEP, read_frame(&mut recv))
                    .await
                    .expect("third response arrived before timeout");
                let _ = send.shutdown().await;
                break decode_canonical::<ControlResponseV1>(&bytes, MAX_CONTROL_FRAME_BYTES)
                    .unwrap();
            }
            Err(error) => {
                assert!(
                    tokio::time::Instant::now() < give_up,
                    "a fresh connection was never admitted after the permit was released: {error}",
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    };
    assert!(
        matches!(
            response.outcome,
            ControlOutcome::Success(ControlSuccess::PrepareHost(_))
        ),
        "the post-release connection should be served normally: {:?}",
        response.outcome,
    );

    let _ = shutdown_tx.send(());
    let served = timeout(STEP, serve_task)
        .await
        .expect("serve stops promptly")
        .expect("serve task joined");
    assert!(served.is_ok(), "serve returned Ok: {served:?}");
    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

/// The `riot/anchor/1` registration carries the CONTROL frame ceiling
/// ([`MAX_CONTROL_FRAME_BYTES`]), not the router's default sync ceiling: a
/// length prefix declaring one byte over the control cap (far under the sync
/// cap) must terminate the session AT the prefix, with no response frame.
#[tokio::test(flavor = "multi_thread")]
async fn control_plane_rejects_frames_over_the_control_cap_but_under_the_sync_cap() {
    // The declared length sits strictly between the two ceilings, so it can
    // only be rejected by the per-protocol control cap.
    let declared = MAX_CONTROL_FRAME_BYTES + 1;
    assert!(
        declared < MAX_SYNC_FRAME_BYTES,
        "the probe length must sit between the control and sync caps",
    );

    let daemon_endpoint = bind_local_anchor_endpoint([103u8; 32])
        .await
        .expect("daemon endpoint binds");
    let daemon_addr = dialable_addr(&daemon_endpoint).await;

    let (config, db_path) = daemon_config();
    let (daemon_config, service) = assemble_service(config);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let serve_task = tokio::spawn(async move {
        serve(
            daemon_endpoint,
            daemon_config,
            service,
            Box::new(|| [0x53u8; 32]),
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
    });

    let client = bind_local_anchor_endpoint([204u8; 32])
        .await
        .expect("client endpoint binds");
    let conn = timeout(STEP, client.connect(daemon_addr, ALPN_ANCHOR_V1))
        .await
        .expect("dial did not time out")
        .expect("dial connects");
    let (send, recv) = conn.open_bi().await.expect("bi-stream opens");
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);

    // A well-formed `u32be` length prefix declaring the oversized frame. The
    // body is deliberately withheld: the control cap is enforced AT the prefix
    // (before allocating or reading a body), so the daemon must cut the session
    // off immediately. If this ALPN inherited the sync ceiling instead, the
    // daemon would accept the prefix and sit in its five-second progress
    // deadline awaiting body bytes — caught by the prompt window below.
    send.write_all(&(declared as u32).to_be_bytes())
        .await
        .unwrap();
    send.flush().await.unwrap();

    // The session terminates promptly and with NO response frame: the next read
    // observes stream/connection termination, never response bytes.
    let mut probe = [0u8; 1];
    let observed = timeout(Duration::from_secs(2), recv.read(&mut probe))
        .await
        .expect("the session must end at the oversized prefix, not wait out the progress deadline");
    match observed {
        Ok(0) | Err(_) => {}
        Ok(n) => panic!("expected no response frame, but read {n} byte(s)"),
    }

    let _ = shutdown_tx.send(());
    let served = timeout(STEP, serve_task)
        .await
        .expect("serve stops promptly")
        .expect("serve task joined");
    assert!(served.is_ok(), "serve returned Ok: {served:?}");
    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

// ---------------------------------------------------------------------------
// `riot/sync/2` end-to-end (Part C): push → commit → pull over real iroh.
// ---------------------------------------------------------------------------

/// The `(entry_id, item_bytes)` sync items of a site fixture, ordered O/C/W.
fn site_items(site: &SiteFixture) -> [SyncItem; 3] {
    [
        (
            site.manifest_staged.entry_id.to_vec(),
            site.manifest_staged.item_bytes.clone(),
        ),
        (
            site.c_staged.entry_id.to_vec(),
            site.c_staged.item_bytes.clone(),
        ),
        (
            site.w_staged.entry_id.to_vec(),
            site.w_staged.item_bytes.clone(),
        ),
    ]
}

/// A canonical `PrepareHost` frame carrying the site fixture's root-signed
/// ticket and the client's declared current snapshot digests.
fn site_prepare_frame(
    site: &SiteFixture,
    declared: [[u8; 32]; 3],
    idempotency_key: [u8; 16],
) -> Vec<u8> {
    let ticket = decode_canonical::<RootSignedTicketCoreEnvelopeV2>(
        &site.ticket_envelope_bytes,
        MAX_TICKET_CORE_BYTES + 128,
    )
    .expect("site ticket decodes");
    let body = PrepareHostV1 {
        root_signed_ticket_core: ticket,
        ordered_namespace_snapshot_digests: declared,
        work_stamp: None,
    };
    ControlRequestV1 {
        idempotency_key,
        operation: ControlOperation::PrepareHost(Box::new(body)),
    }
    .encode_canonical()
    .expect("encode site prepare request")
}

/// A canonical `GetWorkChallenge` frame for an intended site `PrepareHost`.
fn site_work_challenge_frame(
    site: &SiteFixture,
    intended_idempotency_key: [u8; 16],
    idempotency_key: [u8; 16],
) -> Vec<u8> {
    let intended_bytes = site_prepare_frame(site, [[0u8; 32]; 3], intended_idempotency_key);
    let intended = decode_canonical::<ControlRequestV1>(&intended_bytes, MAX_CONTROL_FRAME_BYTES)
        .expect("intended prepare decodes");
    let body = GetWorkChallengeV1 {
        intended_operation_kind: ControlOperationKind::PrepareHost,
        intended_idempotency_key,
        community_root: site.root_id,
        work_target_digest: intended
            .operation
            .work_target_digest()
            .expect("work target digest"),
    };
    ControlRequestV1 {
        idempotency_key,
        operation: ControlOperation::GetWorkChallenge(body),
    }
    .encode_canonical()
    .expect("encode work challenge request")
}

/// A canonical `CommitHost` frame.
fn commit_frame(
    operation_id: [u8; 32],
    ordered_namespace_snapshot_digests: [[u8; 32]; 3],
    idempotency_key: [u8; 16],
) -> Vec<u8> {
    ControlRequestV1 {
        idempotency_key,
        operation: ControlOperation::CommitHost(CommitHostV1 {
            operation_id,
            ordered_namespace_snapshot_digests,
        }),
    }
    .encode_canonical()
    .expect("encode commit request")
}

/// Run PrepareHost over the wire and unwrap the PrepareSuccess.
async fn prepare_site(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    site: &SiteFixture,
    idempotency_key: [u8; 16],
) -> PrepareSuccessV1 {
    prepare_site_declaring(client, daemon_addr, site, [[0u8; 32]; 3], idempotency_key).await
}

/// [`prepare_site`] with the client's declared current snapshot digests (a
/// client that already holds the site's data declares those digests, which
/// also distinguishes the request body from an earlier zero-declared prepare).
async fn prepare_site_declaring(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    site: &SiteFixture,
    declared: [[u8; 32]; 3],
    idempotency_key: [u8; 16],
) -> PrepareSuccessV1 {
    let response = control_round_trip(
        client,
        daemon_addr,
        site_prepare_frame(site, declared, idempotency_key),
    )
    .await;
    match response.outcome {
        ControlOutcome::Success(ControlSuccess::PrepareHost(success)) => *success,
        other => panic!("expected PrepareSuccess for the site, got {other:?}"),
    }
}

/// Dial the `riot/sync/2` ALPN and drive `session` (a real initiator FSM) over
/// the connection: write its `start()` frames, then read → `on_frame` → write
/// until the session terminates. Returns the terminated session for
/// completion/refusal assertions.
async fn drive_sync2<R: Sync2Repository>(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    mut session: Sync2Session<R>,
) -> Sync2Session<R> {
    let conn = timeout(STEP, client.connect(daemon_addr, ALPN_SYNC_V2))
        .await
        .expect("sync dial did not time out")
        .expect("sync dial connects");
    let (send, recv) = conn.open_bi().await.expect("open sync bi-stream");
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);

    let write_actions = |actions: Vec<Sync2Action>| -> Vec<Vec<u8>> {
        actions
            .into_iter()
            .filter_map(|action| match action {
                Sync2Action::Send(frame) => {
                    Some(frame.encode_canonical().expect("encode sync2 frame"))
                }
                _ => None,
            })
            .collect()
    };

    for bytes in write_actions(session.start()) {
        write_frame(&mut send, &bytes).await;
    }
    while !session.is_terminated() {
        let bytes = timeout(STEP, read_frame(&mut recv))
            .await
            .expect("sync frame arrived before timeout");
        let frame = decode_canonical::<Sync2Frame>(&bytes, MAX_SYNC2_FRAME_BYTES)
            .expect("inbound sync2 frame decodes");
        for out in write_actions(session.on_frame(frame)) {
            write_frame(&mut send, &out).await;
        }
    }
    let _ = send.shutdown().await;
    session
}

/// Push one namespace's items through a full `HostReconcileStaged` session and
/// assert `NamespaceComplete`.
async fn push_namespace(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    namespace_id: [u8; 32],
    operation_id: [u8; 32],
    namespace_token: [u8; 32],
    items: Vec<SyncItem>,
) {
    let session = push_initiator(namespace_id, operation_id, namespace_token, items);
    let session = drive_sync2(client, daemon_addr, session).await;
    assert!(
        session.is_complete(),
        "push must reach NamespaceComplete, refusal: {:?}",
        session.refusal(),
    );
}

/// Pull one namespace via `ReadCommitted` and return `(complete, refusal,
/// admitted items)`.
async fn pull_namespace(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    namespace_id: [u8; 32],
    ticket_core_bytes: Vec<u8>,
) -> (bool, Option<Sync2Refusal>, Vec<SyncItem>) {
    let (session, admitted) = pull_initiator(namespace_id, ticket_core_bytes);
    let session = drive_sync2(client, daemon_addr, session).await;
    let items = admitted.borrow().clone();
    (session.is_complete(), session.refusal().cloned(), items)
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_serves_sync2_push_then_commit_then_pull_over_real_iroh() {
    let daemon_endpoint = bind_local_anchor_endpoint([104u8; 32])
        .await
        .expect("daemon endpoint binds");
    let daemon_addr = dialable_addr(&daemon_endpoint).await;
    let db_path = unique_db();
    let now = now_secs();

    // Fail-closed leg (b) seed: a prepared operation whose tokens are VALID
    // (derived under the daemon's deterministic token ring) but whose operation
    // expiry is already past, inserted before the daemon opens the database.
    let expired_operation_id = [0xE1u8; 32];
    let expired_item = make_item("expired-operation entry");
    let expired_namespaces = [expired_item.namespace_id, [0xE2u8; 32], [0xE3u8; 32]];
    let expired_expiry = now.saturating_sub(10);
    let daemon_ring =
        TokenSecretRing::new(1, derive(b"riot/anchor/token-secret/v1", &[0x07u8; 32]));
    let mut expired_tokens = [[0u8; 32]; 3];
    for (slot, namespace_id) in expired_tokens.iter_mut().zip(expired_namespaces.iter()) {
        *slot = daemon_ring
            .derive(1, &expired_operation_id, namespace_id, expired_expiry)
            .expect("daemon ring derives");
    }
    {
        let mut store = AnchorRepository::open(&db_path).expect("open db before the daemon");
        insert_prepared_operation(
            &mut store,
            expired_operation_id,
            expired_namespaces,
            expired_tokens,
            0,
            now.saturating_sub(100),
            expired_expiry,
            1,
        );
    }

    let config = daemon_config_for(&db_path, "Sync2 anchor");
    let (daemon_config, service) = assemble_service(config);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let mut entropy_byte = 0x70u8;
    let entropy = Box::new(move || {
        let value = [entropy_byte; 32];
        entropy_byte = entropy_byte.wrapping_add(1);
        value
    });
    let serve_task = tokio::spawn(async move {
        serve(
            daemon_endpoint,
            daemon_config,
            service,
            entropy,
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
    });

    let client = bind_local_anchor_endpoint([205u8; 32])
        .await
        .expect("client endpoint binds");

    // ------------------------------------------------------------------
    // 1. Control plane: GetWorkChallenge, then PrepareHost with the site's
    //    root-signed ticket.
    // ------------------------------------------------------------------
    let site = make_site_fixture(0x33, 7, now.saturating_sub(100), now + 3600);
    let challenge = control_round_trip(
        &client,
        daemon_addr.clone(),
        site_work_challenge_frame(&site, [0x91u8; 16], [0x90u8; 16]),
    )
    .await;
    assert!(
        matches!(
            challenge.outcome,
            ControlOutcome::Success(ControlSuccess::GetWorkChallenge(_))
        ),
        "work challenge must be served: {:?}",
        challenge.outcome,
    );
    let prepared = prepare_site(&client, daemon_addr.clone(), &site, [0x91u8; 16]).await;
    assert_eq!(
        prepared.ordered_namespace_host_plan, site.namespaces,
        "the host plan is the ticket's O/C/W namespaces",
    );

    // ------------------------------------------------------------------
    // 2. sync/2 push: all three namespaces INCLUDING the O `/manifest` entry
    //    (CommitHost's manifest authority resolves from staged O).
    // ------------------------------------------------------------------
    let items = site_items(&site);
    for (index, item) in items.iter().enumerate() {
        push_namespace(
            &client,
            daemon_addr.clone(),
            site.namespaces[index],
            prepared.operation_id,
            prepared.ordered_namespace_tokens[index],
            vec![item.clone()],
        )
        .await;
    }

    // ------------------------------------------------------------------
    // Fail-closed leg (a), while the operation is still Prepared: a forged
    // namespace_token is refused; nothing staged, nothing served. (After
    // CommitHost the operation is terminal and its tokens are dead — the
    // refusal would collapse into operation_not_found, hiding the token gate.)
    // ------------------------------------------------------------------
    let mut forged = prepared.ordered_namespace_tokens[1];
    forged[0] ^= 0x01;
    let session = push_initiator(
        site.namespaces[1],
        prepared.operation_id,
        forged,
        vec![items[1].clone()],
    );
    let session = drive_sync2(&client, daemon_addr.clone(), session).await;
    assert!(!session.is_complete());
    assert!(
        matches!(
            session.refusal(),
            Some(Sync2Refusal::InvalidNamespaceToken { .. })
        ),
        "a forged token must be refused as invalid_namespace_token: {:?}",
        session.refusal(),
    );

    // ------------------------------------------------------------------
    // 3. CommitHost over the wire: a signed hosting receipt comes back.
    // ------------------------------------------------------------------
    let declared = [
        client_snapshot_digest(&site.namespaces[0], &[items[0].clone()]),
        client_snapshot_digest(&site.namespaces[1], &[items[1].clone()]),
        client_snapshot_digest(&site.namespaces[2], &[items[2].clone()]),
    ];
    let committed = control_round_trip(
        &client,
        daemon_addr.clone(),
        commit_frame(prepared.operation_id, declared, [0x92u8; 16]),
    )
    .await;
    match committed.outcome {
        ControlOutcome::Success(ControlSuccess::CommitHost(receipt)) => {
            assert_eq!(receipt.body.full_site_root, site.root_id);
            assert_eq!(receipt.body.manifest_digest, site.manifest_digest);
            assert_eq!(receipt.body.hosting_operation_id, prepared.operation_id);
        }
        other => panic!("expected a hosting receipt over the wire, got {other:?}"),
    }

    // ------------------------------------------------------------------
    // 4. NEW connections: ReadCommitted pull of each namespace, byte-verbatim.
    // ------------------------------------------------------------------
    for (index, item) in items.iter().enumerate() {
        let (complete, refusal, pulled) = pull_namespace(
            &client,
            daemon_addr.clone(),
            site.namespaces[index],
            site.ticket_envelope_bytes.clone(),
        )
        .await;
        assert!(
            complete,
            "pull of namespace {index} must complete: {refusal:?}"
        );
        assert_eq!(
            pulled,
            vec![item.clone()],
            "pulled item bytes must byte-match what was pushed (namespace {index})",
        );
    }

    // ------------------------------------------------------------------
    // 5. Remaining fail-closed legs over the wire, one connection each.
    // ------------------------------------------------------------------
    // (b) An EXPIRED operation is refused even with a validly-derived token.
    let session = push_initiator(
        expired_namespaces[0],
        expired_operation_id,
        expired_tokens[0],
        vec![(
            expired_item.entry_id.to_vec(),
            expired_item.item_bytes.clone(),
        )],
    );
    let session = drive_sync2(&client, daemon_addr.clone(), session).await;
    assert!(!session.is_complete());
    assert!(
        matches!(
            session.refusal(),
            Some(Sync2Refusal::OperationExpired { .. })
        ),
        "an expired operation must be refused: {:?}",
        session.refusal(),
    );

    // (c) A token for an operation the anchor never prepared is unknown.
    let session = push_initiator(
        site.namespaces[1],
        [0xEEu8; 32],
        prepared.ordered_namespace_tokens[1],
        vec![items[1].clone()],
    );
    let session = drive_sync2(&client, daemon_addr.clone(), session).await;
    assert!(!session.is_complete());
    assert!(
        matches!(
            session.refusal(),
            Some(Sync2Refusal::OperationNotFound { .. })
        ),
        "an unknown operation must be refused: {:?}",
        session.refusal(),
    );

    // (d) A push WITHOUT the `/manifest` entry: CommitHost refuses
    //     commit_manifest_mismatch over the wire and ReadCommitted serves
    //     nothing for that community.
    let site2 = make_site_fixture(0x47, 4, now.saturating_sub(100), now + 3600);
    let prepared2 = prepare_site(&client, daemon_addr.clone(), &site2, [0x93u8; 16]).await;
    let items2 = site_items(&site2);
    for index in [1usize, 2] {
        push_namespace(
            &client,
            daemon_addr.clone(),
            site2.namespaces[index],
            prepared2.operation_id,
            prepared2.ordered_namespace_tokens[index],
            vec![items2[index].clone()],
        )
        .await;
    }
    let declared2 = [
        client_snapshot_digest(&site2.namespaces[0], &[]),
        client_snapshot_digest(&site2.namespaces[1], &[items2[1].clone()]),
        client_snapshot_digest(&site2.namespaces[2], &[items2[2].clone()]),
    ];
    let refused_commit = control_round_trip(
        &client,
        daemon_addr.clone(),
        commit_frame(prepared2.operation_id, declared2, [0x94u8; 16]),
    )
    .await;
    assert!(
        matches!(
            refused_commit.outcome,
            ControlOutcome::Refused(ControlRefusal::CommitManifestMismatch { .. })
        ),
        "a manifest-less commit must refuse commit_manifest_mismatch: {:?}",
        refused_commit.outcome,
    );
    let (complete, refusal, pulled) = pull_namespace(
        &client,
        daemon_addr.clone(),
        site2.namespaces[1],
        site2.ticket_envelope_bytes.clone(),
    )
    .await;
    assert!(!complete, "an uncommitted community must serve nothing");
    assert!(pulled.is_empty(), "no items may leak: {pulled:?}");
    assert!(
        matches!(refusal, Some(Sync2Refusal::ManifestMismatch { .. })),
        "the pull refusal is the committed-manifest gate: {refusal:?}",
    );

    // Clean shutdown.
    let _ = shutdown_tx.send(());
    let served = timeout(STEP, serve_task)
        .await
        .expect("serve stops promptly")
        .expect("serve task joined");
    assert!(served.is_ok(), "serve returned Ok: {served:?}");
    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

// ---------------------------------------------------------------------------
// Sync-slot reclamation: a session killed mid-exchange (router-level
// cancellation drops the handler future) must still free its
// `SyncSessionTable` slot via the RAII close guard.
// ---------------------------------------------------------------------------

/// Encode the `Send` frames of a batch of initiator actions.
fn encode_sends(actions: Vec<Sync2Action>) -> Vec<Vec<u8>> {
    actions
        .into_iter()
        .filter_map(|action| match action {
            Sync2Action::Send(frame) => Some(frame.encode_canonical().expect("encode sync2 frame")),
            _ => None,
        })
        .collect()
}

/// [`read_frame`] that reports failure instead of panicking (a dead or SILENT
/// peer — the capacity-refusal signature transmits nothing — returns `None`).
async fn try_read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Option<Vec<u8>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await.ok()?;
    let n = u32::from_be_bytes(len) as usize;
    let mut body = vec![0u8; n];
    r.read_exact(&mut body).await.ok()?;
    Some(body)
}

/// Attempt a full push session. Unlike [`push_namespace`] this never panics on
/// a silent daemon: a connect failure, a read timeout (a table-capacity refusal
/// writes NO frames back), or a refusal all return `false`. `true` means the
/// responder admitted the open and drove the session to `NamespaceComplete`.
async fn try_push_namespace(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    namespace_id: [u8; 32],
    operation_id: [u8; 32],
    namespace_token: [u8; 32],
    items: Vec<SyncItem>,
) -> bool {
    let attempt = Duration::from_secs(2);
    let Ok(Ok(conn)) = timeout(attempt, client.connect(daemon_addr, ALPN_SYNC_V2)).await else {
        return false;
    };
    let Ok((send, recv)) = conn.open_bi().await else {
        return false;
    };
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);
    let mut session = push_initiator(namespace_id, operation_id, namespace_token, items);
    for bytes in encode_sends(session.start()) {
        write_frame(&mut send, &bytes).await;
    }
    while !session.is_terminated() {
        let Ok(Some(bytes)) = timeout(attempt, try_read_frame(&mut recv)).await else {
            return false;
        };
        let Ok(frame) = decode_canonical::<Sync2Frame>(&bytes, MAX_SYNC2_FRAME_BYTES) else {
            return false;
        };
        for out in encode_sends(session.on_frame(frame)) {
            write_frame(&mut send, &out).await;
        }
    }
    let _ = send.shutdown().await;
    session.is_complete()
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_reclaims_the_sync_slot_when_a_session_is_killed_mid_exchange() {
    let daemon_endpoint = bind_local_anchor_endpoint([109u8; 32])
        .await
        .expect("daemon endpoint binds");
    let daemon_addr = dialable_addr(&daemon_endpoint).await;
    let db_path = unique_db();
    let now = now_secs();

    // Capacity 1: the sync table holds at most ONE live session, so a single
    // stranded slot would brick the data path permanently.
    let config = daemon_config_with_sessions(&db_path, "Leak anchor", 1);
    let (daemon_config, service) = assemble_service(config);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let mut entropy_byte = 0x61u8;
    let entropy = Box::new(move || {
        let value = [entropy_byte; 32];
        entropy_byte = entropy_byte.wrapping_add(1);
        value
    });
    let serve_task = tokio::spawn(async move {
        serve(
            daemon_endpoint,
            daemon_config,
            service,
            entropy,
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
    });

    let client = bind_local_anchor_endpoint([209u8; 32])
        .await
        .expect("client endpoint binds");

    let site = make_site_fixture(0x71, 11, now.saturating_sub(100), now + 3600);
    let prepared = prepare_site(&client, daemon_addr.clone(), &site, [0xD1u8; 16]).await;
    let items = site_items(&site);

    // Victim session: dial (with retry — the capacity-1 ingress permit of the
    // control session above may release a beat after its response) and get the
    // open ADMITTED, proven by the responder's first reply frame (a capacity
    // refusal would transmit nothing).
    let mut victim = None;
    for _ in 0..40 {
        if let Ok(Ok(conn)) = timeout(
            Duration::from_secs(2),
            client.connect(daemon_addr.clone(), ALPN_SYNC_V2),
        )
        .await
        {
            victim = Some(conn);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let victim = victim.expect("victim sync dial connects");
    let (send, recv) = victim.open_bi().await.expect("open victim bi-stream");
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);
    let mut victim_session = push_initiator(
        site.namespaces[0],
        prepared.operation_id,
        prepared.ordered_namespace_tokens[0],
        vec![items[0].clone()],
    );
    for bytes in encode_sends(victim_session.start()) {
        write_frame(&mut send, &bytes).await;
    }
    let _admitted_proof = timeout(STEP, read_frame(&mut recv))
        .await
        .expect("the admitted victim session answers its open");

    // Kill the handler THROUGH THE ROUTER, mid-session: any unidirectional
    // stream is a stream violation, and the router's `accept_extra` select arm
    // CANCELS (drops) the handler future — code after the handler's read loop
    // never runs. Only a close-on-drop mechanism can free the table slot.
    let mut extra = victim.open_uni().await.expect("open violating uni stream");
    let _ = extra.write_all(&[0u8]).await;
    let _ = extra.finish();

    // Wait until the daemon has actually torn the victim connection down (the
    // handler future — and with it the close guard — is dropped before the
    // router returns and the connection dies).
    let died = timeout(STEP, async {
        let mut probe = [0u8; 1];
        loop {
            match recv.read(&mut probe).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    })
    .await;
    assert!(died.is_ok(), "daemon must reset the violated connection");

    // The slot must come back: a NEW session on the same prepared operation is
    // admitted and drives to NamespaceComplete. Retries absorb the daemon-side
    // teardown races (ingress permit release, Close delivery); under a leak
    // the slot NEVER returns and every attempt times out in silence.
    let mut admitted = false;
    for _ in 0..6 {
        if try_push_namespace(
            &client,
            daemon_addr.clone(),
            site.namespaces[1],
            prepared.operation_id,
            prepared.ordered_namespace_tokens[1],
            vec![items[1].clone()],
        )
        .await
        {
            admitted = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        admitted,
        "a session killed mid-exchange stranded its SyncSessionTable slot: \
         a new sync session must be admitted once the guard's Close propagates",
    );

    // Clean shutdown.
    let _ = shutdown_tx.send(());
    let served = timeout(STEP, serve_task)
        .await
        .expect("serve stops promptly")
        .expect("serve task joined");
    assert!(served.is_ok(), "serve returned Ok: {served:?}");
    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

// ---------------------------------------------------------------------------
// Task C3 — restart survivability: clean restart after push+commit, and a
// kill mid-lifecycle with a staged-but-uncommitted operation.
// ---------------------------------------------------------------------------

/// Spawn a daemon over a fresh local endpoint bound to `endpoint_secret`,
/// serving `db_path`. Returns the dialable address, the shutdown sender, and
/// the serve task handle.
async fn start_daemon(
    db_path: &std::path::Path,
    endpoint_secret: [u8; 32],
    entropy_seed: u8,
) -> (
    iroh::EndpointAddr,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), riot_anchor::daemon::DaemonError>>,
) {
    let endpoint = bind_local_anchor_endpoint(endpoint_secret)
        .await
        .expect("daemon endpoint binds");
    let addr = dialable_addr(&endpoint).await;
    let (daemon_config, service) = assemble_service(daemon_config_for(db_path, "Restart anchor"));
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let mut entropy_byte = entropy_seed;
    let entropy = Box::new(move || {
        let value = [entropy_byte; 32];
        entropy_byte = entropy_byte.wrapping_add(1);
        value
    });
    let task = tokio::spawn(async move {
        serve(endpoint, daemon_config, service, entropy, async move {
            let _ = shutdown_rx.await;
        })
        .await
    });
    (addr, shutdown_tx, task)
}

/// Stop a daemon started with [`start_daemon`] and assert a clean exit.
async fn stop_daemon(
    shutdown_tx: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<Result<(), riot_anchor::daemon::DaemonError>>,
) {
    let _ = shutdown_tx.send(());
    let served = timeout(STEP, task)
        .await
        .expect("serve stops promptly")
        .expect("serve task joined");
    assert!(served.is_ok(), "serve returned Ok: {served:?}");
}

/// `Describe` over the wire, returning the descriptor's canonical bytes.
async fn describe_bytes(client: &iroh::Endpoint, daemon_addr: iroh::EndpointAddr) -> Vec<u8> {
    let frame = ControlRequestV1 {
        idempotency_key: [0x7Au8; 16],
        operation: ControlOperation::Describe(DescribeV1),
    }
    .encode_canonical()
    .unwrap();
    let response = control_round_trip(client, daemon_addr, frame).await;
    match response.outcome {
        ControlOutcome::Success(ControlSuccess::Describe(success)) => success
            .descriptor
            .encode_canonical()
            .expect("descriptor re-encodes"),
        other => panic!("expected Describe success, got {other:?}"),
    }
}

/// Run the full prepare → push(O,C,W) → commit cycle for `site` and assert the
/// hosting receipt. `key_seed` disambiguates idempotency keys.
async fn full_host_cycle(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    site: &SiteFixture,
    key_seed: u8,
) {
    let items = site_items(site);
    // The pushing client declares its CURRENT snapshot digests (it holds the
    // site's items) — as a fresh re-prepare after a crash would.
    let declared = [
        client_snapshot_digest(&site.namespaces[0], &[items[0].clone()]),
        client_snapshot_digest(&site.namespaces[1], &[items[1].clone()]),
        client_snapshot_digest(&site.namespaces[2], &[items[2].clone()]),
    ];
    let prepared =
        prepare_site_declaring(client, daemon_addr.clone(), site, declared, [key_seed; 16]).await;
    for (index, item) in items.iter().enumerate() {
        push_namespace(
            client,
            daemon_addr.clone(),
            site.namespaces[index],
            prepared.operation_id,
            prepared.ordered_namespace_tokens[index],
            vec![item.clone()],
        )
        .await;
    }
    let committed = control_round_trip(
        client,
        daemon_addr.clone(),
        commit_frame(prepared.operation_id, declared, [key_seed + 1; 16]),
    )
    .await;
    assert!(
        matches!(
            committed.outcome,
            ControlOutcome::Success(ControlSuccess::CommitHost(_))
        ),
        "commit must yield a hosting receipt: {:?}",
        committed.outcome,
    );
}

/// Pull every namespace of `site` and assert the byte-verbatim committed items.
async fn assert_pull_serves_site(
    client: &iroh::Endpoint,
    daemon_addr: iroh::EndpointAddr,
    site: &SiteFixture,
) {
    let items = site_items(site);
    for (index, item) in items.iter().enumerate() {
        let (complete, refusal, pulled) = pull_namespace(
            client,
            daemon_addr.clone(),
            site.namespaces[index],
            site.ticket_envelope_bytes.clone(),
        )
        .await;
        assert!(complete, "pull {index} must complete: {refusal:?}");
        assert_eq!(
            pulled,
            vec![item.clone()],
            "pulled bytes must match the committed entry verbatim (namespace {index})",
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_clean_restart_serves_identical_descriptor_and_committed_data() {
    let db_path = unique_db();
    let now = now_secs();
    let client = bind_local_anchor_endpoint([206u8; 32])
        .await
        .expect("client endpoint binds");
    let site = make_site_fixture(0x51, 6, now.saturating_sub(100), now + 3600);

    // Run 1: full push + commit, capture the served descriptor, stop cleanly.
    let (addr1, shutdown1, task1) = start_daemon(&db_path, [105u8; 32], 0x30).await;
    let descriptor_before = describe_bytes(&client, addr1.clone()).await;
    full_host_cycle(&client, addr1.clone(), &site, 0xA0).await;
    assert_pull_serves_site(&client, addr1.clone(), &site).await;
    stop_daemon(shutdown1, task1).await;

    // Run 2: same DB, same endpoint secret. The descriptor must be
    // byte-identical (no equivocation) and the committed data still served.
    let (addr2, shutdown2, task2) = start_daemon(&db_path, [105u8; 32], 0x40).await;
    let descriptor_after = describe_bytes(&client, addr2.clone()).await;
    assert_eq!(
        descriptor_before, descriptor_after,
        "a clean restart must serve the byte-identical persisted descriptor",
    );
    assert_pull_serves_site(&client, addr2.clone(), &site).await;
    stop_daemon(shutdown2, task2).await;

    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_restart_with_staged_uncommitted_operation_is_unwedged_and_leaks_nothing() {
    let db_path = unique_db();
    let now = now_secs();
    let client = bind_local_anchor_endpoint([207u8; 32])
        .await
        .expect("client endpoint binds");
    let site = make_site_fixture(0x63, 9, now.saturating_sub(100), now + 3600);
    let items = site_items(&site);

    // Run 1: prepare + push all three namespaces to NamespaceComplete, but do
    // NOT CommitHost — then stop with the operation still staged.
    let (addr1, shutdown1, task1) = start_daemon(&db_path, [106u8; 32], 0x30).await;
    let descriptor_before = describe_bytes(&client, addr1.clone()).await;
    let prepared = prepare_site(&client, addr1.clone(), &site, [0xB0u8; 16]).await;
    for (index, item) in items.iter().enumerate() {
        push_namespace(
            &client,
            addr1.clone(),
            site.namespaces[index],
            prepared.operation_id,
            prepared.ordered_namespace_tokens[index],
            vec![item.clone()],
        )
        .await;
    }
    stop_daemon(shutdown1, task1).await;

    // Run 2: the daemon must start against the staged-uncommitted database
    // (no wedge, no corruption)...
    let (addr2, shutdown2, task2) = start_daemon(&db_path, [106u8; 32], 0x40).await;
    // ...serve the byte-identical descriptor...
    let descriptor_after = describe_bytes(&client, addr2.clone()).await;
    assert_eq!(
        descriptor_before, descriptor_after,
        "a mid-lifecycle kill must not equivocate the descriptor",
    );
    // ...and serve NOTHING for that community: staged data must never leak
    // into the committed view across a restart.
    for index in 0..3 {
        let (complete, refusal, pulled) = pull_namespace(
            &client,
            addr2.clone(),
            site.namespaces[index],
            site.ticket_envelope_bytes.clone(),
        )
        .await;
        assert!(
            !complete,
            "an uncommitted community must not serve a snapshot (namespace {index})",
        );
        assert!(
            pulled.is_empty(),
            "staged entries must not leak as committed (namespace {index}): {pulled:?}",
        );
        assert!(
            matches!(refusal, Some(Sync2Refusal::ManifestMismatch { .. })),
            "the pull must refuse at the committed-manifest gate: {refusal:?}",
        );
    }
    // A fresh full cycle on the SAME community then succeeds end-to-end.
    full_host_cycle(&client, addr2.clone(), &site, 0xC0).await;
    assert_pull_serves_site(&client, addr2.clone(), &site).await;
    stop_daemon(shutdown2, task2).await;

    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

/// Task D2 (lease renewal): the running daemon RENEWS its deployment lease, so
/// a second deployment instance on the same database fails to start long after
/// the raw TTL has elapsed — and can take the lease only once the first stops
/// AND the lease term lapses.
///
/// PRODUCTION lease-identity derivation for BOTH daemons — no hand-overridden
/// holder id. The two daemons run from BYTE-IDENTICAL configs (same operator
/// secret, same database; only their per-process entropy differs): the
/// realistic accidental double-start the single-writer lease exists to
/// prevent. The lease holder id is a per-process random draw ([`serve`]'s
/// first entropy use), NOT an operator-secret derivation, so the second
/// daemon presents the SAME deployment token with a DIFFERENT holder and is
/// refused `LeaseHeld` — were the holder operator-derived, it would present
/// identical holder+token, renew the first daemon's lease in place, and start
/// fine: two live writers forking one database.
#[tokio::test(flavor = "multi_thread")]
async fn daemon_renews_the_lease_so_a_second_deployment_cannot_start_until_it_stops() {
    const SHORT_TTL_SECS: u64 = 2;
    let db_path = unique_db();

    let first_endpoint = bind_local_anchor_endpoint([103u8; 32])
        .await
        .expect("first daemon endpoint binds");
    let (mut first_config, first_service) =
        assemble_service(daemon_config_for(&db_path, "Lease deployment"));
    first_config.lease_ttl_secs = SHORT_TTL_SECS;
    let first_token = first_config.deployment_token;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let first_task = tokio::spawn(async move {
        serve(
            first_endpoint,
            first_config,
            first_service,
            Box::new(|| [0x53u8; 32]),
            async move {
                let _ = shutdown_rx.await;
            },
        )
        .await
    });

    // Long past the raw TTL: without renewal the startup lease would now be
    // expired and free for the taking.
    tokio::time::sleep(Duration::from_secs(SHORT_TTL_SECS + 2)).await;

    // The accidental double-start: a second daemon assembled from the
    // BYTE-IDENTICAL config — same operator secret, hence the SAME
    // database-bound deployment token — must fail closed while the first is
    // alive and renewing. Only its per-process entropy (its startup holder
    // draw) differs.
    let contender_endpoint = bind_local_anchor_endpoint([104u8; 32])
        .await
        .expect("contender endpoint binds");
    let (mut contender_config, contender_service) =
        assemble_service(daemon_config_for(&db_path, "Lease deployment"));
    contender_config.lease_ttl_secs = SHORT_TTL_SECS;
    assert_eq!(
        contender_config.deployment_token, first_token,
        "byte-identical configs derive the same deployment token",
    );
    let contended = timeout(
        STEP,
        serve(
            contender_endpoint,
            contender_config,
            contender_service,
            Box::new(|| [0x54u8; 32]),
            std::future::ready(()),
        ),
    )
    .await
    .expect("contended startup fails promptly");
    match contended {
        Err(DaemonError::Repository(AnchorRepositoryError::LeaseHeld { .. })) => {}
        other => panic!(
            "a same-config second deployment must fail with LeaseHeld while the first \
             renews (same token, different per-process holder), got {other:?}"
        ),
    }

    // Stop the first daemon and let its (short) final lease term lapse.
    let _ = shutdown_tx.send(());
    let first_result = timeout(STEP, first_task)
        .await
        .expect("first daemon stops promptly")
        .expect("first daemon task joined");
    assert!(
        first_result.is_ok(),
        "first daemon exits cleanly: {first_result:?}"
    );
    tokio::time::sleep(Duration::from_secs(SHORT_TTL_SECS + 1)).await;

    // A fresh start of the same-config deployment now takes the lease and
    // starts serving.
    let retry_endpoint = bind_local_anchor_endpoint([105u8; 32])
        .await
        .expect("retry endpoint binds");
    let (mut retry_config, retry_service) =
        assemble_service(daemon_config_for(&db_path, "Lease deployment"));
    retry_config.lease_ttl_secs = SHORT_TTL_SECS;
    let (retry_shutdown_tx, retry_shutdown_rx) = oneshot::channel::<()>();
    let retry_task = tokio::spawn(async move {
        serve(
            retry_endpoint,
            retry_config,
            retry_service,
            Box::new(|| [0x55u8; 32]),
            async move {
                let _ = retry_shutdown_rx.await;
            },
        )
        .await
    });
    // A moment past startup (lease acquire + descriptor init), then a clean
    // shutdown proves the contender was serving, not failing.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = retry_shutdown_tx.send(());
    let retried = timeout(STEP, retry_task)
        .await
        .expect("retry stops promptly")
        .expect("retry task joined");
    assert!(
        retried.is_ok(),
        "after the first stops and the TTL elapses, the contender starts: {retried:?}"
    );

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

/// The deployment token stays OPERATOR-derived (the per-process holder id does
/// not weaken clone detection): a deployment assembled from a DIFFERENT
/// operator secret derives a different database-bound token and is refused
/// `LeaseTokenMismatch` at the repository gate — regardless of holder identity
/// and even against an already-expired lease.
#[test]
fn a_different_operator_deployment_token_is_refused_as_lease_token_mismatch() {
    let db_path = unique_db();
    let (first_config, _first_service) =
        assemble_service(daemon_config_for(&db_path, "Operator A"));

    // Same database, DIFFERENT operator secret → a different deployment token.
    let args = vec!["--db".to_string(), db_path.to_string_lossy().into_owned()];
    let env = vec![
        ("RIOT_ANCHOR_OPERATOR_KEY_HEX".to_string(), "09".repeat(32)),
        ("RIOT_ANCHOR_ENDPOINT_KEY_HEX".to_string(), "08".repeat(32)),
    ];
    let other = resolve_config(&args, &env).expect("other-operator config resolves");
    let (other_config, _other_service) = assemble_service(other);
    assert_ne!(
        other_config.deployment_token, first_config.deployment_token,
        "a different operator secret derives a different deployment token",
    );

    let mut repo = AnchorRepository::open(&db_path).expect("repository opens");
    let now = now_secs();
    repo.acquire_deployment_lease(&[0xA1u8; 32], &first_config.deployment_token, now, 300)
        .expect("the first operator's deployment binds the database token");
    // Long past expiry — the lease itself is free for the taking — the foreign
    // token is still refused: token mismatch, not holder contention.
    match repo.acquire_deployment_lease(
        &[0xB2u8; 32],
        &other_config.deployment_token,
        now + 1_000,
        300,
    ) {
        Err(AnchorRepositoryError::LeaseTokenMismatch) => {}
        other => {
            panic!("expected LeaseTokenMismatch for a different operator's token, got {other:?}")
        }
    }

    drop(repo);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

/// Task D2 (actor watchdog): the lease-renew interval doubles as the WATCHDOG
/// for the single-writer thread. When the actor dies (poisoned entropy panics
/// on its first post-startup use, killed by one `GetWorkChallenge`), the
/// daemon must stop with the fatal actor-death configuration error within ~2
/// lease intervals — never keep accepting connections against a dead writer.
#[tokio::test(flavor = "multi_thread")]
async fn daemon_stops_fatally_when_the_single_writer_actor_dies() {
    const SHORT_TTL_SECS: u64 = 3; // renew (watchdog) interval: 1 second
    let daemon_endpoint = bind_local_anchor_endpoint([106u8; 32])
        .await
        .expect("daemon endpoint binds");
    let daemon_addr = dialable_addr(&daemon_endpoint).await;

    let db_path = unique_db();
    let (mut daemon_config, service) =
        assemble_service(daemon_config_for(&db_path, "Watchdog anchor"));
    daemon_config.lease_ttl_secs = SHORT_TTL_SECS;
    // Entropy that PANICS on its first POST-STARTUP use: `serve`'s startup
    // draw (the per-process lease holder id, call 1) passes through; the
    // first entropy-minting control request (call 2, inside the actor thread)
    // then kills the single-writer thread.
    let mut entropy_calls = 0u32;
    let entropy: EntropyFn = Box::new(move || -> [u8; 32] {
        entropy_calls += 1;
        if entropy_calls == 1 {
            return [0x66u8; 32]; // the startup holder draw
        }
        panic!("test entropy poisoned")
    });
    let serve_task = tokio::spawn(async move {
        serve(
            daemon_endpoint,
            daemon_config,
            service,
            entropy,
            std::future::pending(),
        )
        .await
    });

    // One GetWorkChallenge forces an entropy call (the challenge nonce) inside
    // the actor thread.
    let root = SigningKey::from_bytes(&[12u8; 32]);
    let intended_bytes = prepare_frame(signed_ticket(&root, |_| {}));
    let intended = decode_canonical::<ControlRequestV1>(&intended_bytes, MAX_CONTROL_FRAME_BYTES)
        .expect("intended prepare decodes");
    let challenge_frame = ControlRequestV1 {
        idempotency_key: [0x91; 16],
        operation: ControlOperation::GetWorkChallenge(GetWorkChallengeV1 {
            intended_operation_kind: ControlOperationKind::PrepareHost,
            intended_idempotency_key: [1u8; 16],
            community_root: root.verifying_key().to_bytes(),
            work_target_digest: intended
                .operation
                .work_target_digest()
                .expect("work target digest"),
        }),
    }
    .encode_canonical()
    .expect("encode work challenge request");

    let client = bind_local_anchor_endpoint([204u8; 32])
        .await
        .expect("client endpoint binds");
    let conn = timeout(STEP, client.connect(daemon_addr, ALPN_ANCHOR_V1))
        .await
        .expect("dial did not time out")
        .expect("dial connects");
    let (send, recv) = conn.open_bi().await.expect("open bi-stream");
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);
    write_frame(&mut send, &challenge_frame).await;
    // The actor dies before replying; the stream ends with an error, not a
    // frame. Only delivery matters here — tolerate any read outcome.
    let mut length_prefix = [0u8; 4];
    let _ = timeout(Duration::from_secs(5), recv.read_exact(&mut length_prefix)).await;

    // Within ~2 lease intervals the watchdog's RenewLease send/reply hits the
    // closed channel and run() exits with the fatal actor-death error.
    let died = timeout(Duration::from_secs(2 * SHORT_TTL_SECS), serve_task)
        .await
        .expect("the watchdog stops the daemon within ~2 lease intervals")
        .expect("serve task joined");
    match died {
        Err(DaemonError::Configuration(message)) => assert!(
            message.contains("single-writer actor died"),
            "the fatal error names the dead actor: {message}"
        ),
        other => panic!("expected the fatal actor-death configuration error, got {other:?}"),
    }

    client.close().await;
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}
