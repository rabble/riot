//! WU-019 increment 1 smoke test: a real anchor daemon (ephemeral root secret,
//! in-memory repository, direct-only/loopback iroh binding — never the public
//! internet) and a bare iroh client do a genuine `riot/anchor/1` control
//! round-trip through the actual `AnchorControlService`. Proves:
//!
//! * a client can dial the daemon's control ALPN and get back a real
//!   `Describe` response built by the production service (not a test double);
//! * repository access is serialized through the single-writer actor — two
//!   concurrent control connections each get back exactly their own answer,
//!   with no cross-talk, corruption, or panic;
//! * graceful shutdown returns cleanly and promptly;
//! * diagnostics never carry key material or a secret path (the config-level
//!   redaction is covered by unit tests in `daemon.rs`; this file only proves
//!   the runtime round-trip).

use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

use riot_anchor::daemon::{Daemon, DaemonConfig, EndpointBinding, RepoLocation, SecretKeySource};

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::control::{
    ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1, ControlResponseV1,
    ControlSuccess, DescribeV1, GetOperationV1, MAX_CONTROL_FRAME_BYTES,
};

use riot_transport::iroh::bind;
use riot_transport::ALPN_ANCHOR_V1;

fn daemon_config(secret: [u8; 32]) -> DaemonConfig {
    DaemonConfig {
        repo: RepoLocation::InMemory,
        secret_key: SecretKeySource::Ephemeral(secret),
        endpoint_binding: EndpointBinding::LocalOnly,
        lease_ttl_secs: 300,
        max_concurrent_sessions: 8,
    }
}

fn d16(seed: u8) -> [u8; 16] {
    [seed; 16]
}

fn d32(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// Dial `peer` on the anchor control ALPN with a fresh ephemeral client
/// endpoint, send one canonical control request frame, and decode the one
/// canonical control response frame that comes back — the exact
/// `u32be(len) || body` framing `BoundedStream` speaks.
async fn send_request(
    peer: &iroh::EndpointAddr,
    request: &ControlRequestV1,
) -> ControlResponseV1 {
    let client = bind().await.expect("bind client endpoint");
    let conn = client
        .connect(peer.clone(), ALPN_ANCHOR_V1)
        .await
        .expect("connect to the anchor control ALPN");
    let (mut send, mut recv) = conn.open_bi().await.expect("open bi stream");

    let frame = request.encode_canonical().expect("encode control request");
    let len = u32::try_from(frame.len()).expect("frame fits in u32");
    send.write_all(&len.to_be_bytes())
        .await
        .expect("write length prefix");
    send.write_all(&frame).await.expect("write frame body");
    send.shutdown().await.expect("finish send side");

    let mut len_bytes = [0u8; 4];
    recv.read_exact(&mut len_bytes)
        .await
        .expect("read response length prefix");
    let response_len = u32::from_be_bytes(len_bytes) as usize;
    let mut body = vec![0u8; response_len];
    recv.read_exact(&mut body)
        .await
        .expect("read response body");

    decode_canonical::<ControlResponseV1>(&body, MAX_CONTROL_FRAME_BYTES)
        .expect("decode a canonical control response")
}

fn describe_request() -> ControlRequestV1 {
    ControlRequestV1 {
        idempotency_key: d16(0xAB),
        operation: ControlOperation::Describe(DescribeV1),
    }
}

fn get_operation_request(operation_id: [u8; 32], key_seed: u8) -> ControlRequestV1 {
    ControlRequestV1 {
        idempotency_key: d16(key_seed),
        operation: ControlOperation::GetOperation(GetOperationV1 { operation_id }),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn control_round_trip_over_a_real_iroh_connection() {
    let (daemon, mut readiness) = Daemon::start(daemon_config([7u8; 32]))
        .await
        .expect("start anchor daemon");

    let anchor_id = daemon.anchor_id();
    let peer_addr = daemon.dialable_addr().await;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let run_handle = tokio::spawn(daemon.run(shutdown_rx));

    readiness
        .changed()
        .await
        .expect("readiness channel updates before the sender is dropped");
    assert!(*readiness.borrow(), "daemon reports ready once it serves");

    // --- A real Describe round trip through the production service. ---
    let response = send_request(&peer_addr, &describe_request()).await;
    match response.outcome {
        ControlOutcome::Success(ControlSuccess::Describe(describe)) => {
            assert_eq!(
                describe.descriptor.body.anchor_id, anchor_id,
                "the served descriptor is this daemon's own signed descriptor"
            );
        }
        other => panic!("expected a Describe success, got {other:?}"),
    }

    // --- Two concurrent connections: proves single-writer serialization ---
    // (no data race / cross-talk) rather than merely sequential correctness.
    let op_a = d32(0x11);
    let op_b = d32(0x22);
    let request_a = get_operation_request(op_a, 0x01);
    let request_b = get_operation_request(op_b, 0x02);
    let (response_a, response_b) = tokio::join!(
        send_request(&peer_addr, &request_a),
        send_request(&peer_addr, &request_b),
    );
    assert_operation_not_found(&response_a, op_a);
    assert_operation_not_found(&response_b, op_b);

    // --- Graceful shutdown returns cleanly and promptly. ---
    shutdown_tx.send(true).expect("signal shutdown");
    let run_result = tokio::time::timeout(Duration::from_secs(5), run_handle)
        .await
        .expect("daemon run() returns promptly once shutdown is signalled")
        .expect("the daemon task did not panic");
    assert!(
        run_result.is_ok(),
        "graceful shutdown returns Ok: {run_result:?}"
    );
}

fn assert_operation_not_found(response: &ControlResponseV1, expected_operation_id: [u8; 32]) {
    match &response.outcome {
        ControlOutcome::Refused(ControlRefusal::OperationNotFound { operation_id }) => {
            assert_eq!(
                *operation_id, expected_operation_id,
                "each concurrent connection gets back its OWN operation id, not a peer's"
            );
        }
        other => panic!("expected operation_not_found, got {other:?}"),
    }
}
