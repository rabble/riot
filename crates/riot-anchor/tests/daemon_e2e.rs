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

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio::time::timeout;

use riot_anchor::admission::IngressLimits;
use riot_anchor::config::{assemble_service, resolve_config, Config};
use riot_anchor::daemon::{bind_local_anchor_endpoint, serve};

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::control::{
    ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1, ControlResponseV1,
    ControlSuccess, DescribeV1, PrepareHostV1, MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::records::{
    PublicSiteTicketV2Core, RootSignedTicketCoreEnvelopeV2, TransportFloor,
};

use riot_transport::iroh::dialable_addr;
use riot_transport::ALPN_ANCHOR_V1;

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
            IngressLimits::DEFAULT_MAX_CONTROL_SESSIONS.to_string(),
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
