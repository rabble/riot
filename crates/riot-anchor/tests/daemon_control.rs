//! WU-019 increment 1: the anchor daemon's `riot/anchor/1` control round-trip.
//!
//! These drive the single-writer control ACTOR and the anchor/1 [`Handler`] over
//! the router's IN-MEMORY [`RouterConnection`] test transport (an in-process
//! duplex, exactly the harness `alpn_router.rs` uses) — no real network. The key
//! case feeds a real, root-signed [`PrepareHostV1`] as a framed request through
//! the router and asserts a well-formed, signed [`ControlResponseV1`] comes back
//! AND that the durable repository reflects the prepared operation (proven by a
//! follow-up `GetOperation` returning `Prepared`). The negative case feeds a
//! garbage frame and asserts the handler closes without a response and without
//! panicking.

#![cfg(feature = "daemon")]

use std::sync::Arc;

use ed25519_dalek::{Signer, SigningKey};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use riot_anchor::control::{AnchorControlContext, AnchorControlService};
use riot_anchor::daemon::{
    control_handler, spawn_control_actor, ActorJob, ControlJob, ControlReply,
    Ed25519OperatorSigner, TicketRootAuthorityPolicy,
};
use riot_anchor::repository::AnchorRepository;
use riot_anchor::work::TokenSecretRing;

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::control::{
    ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1, ControlResponseV1,
    ControlSuccess, GetOperationState, GetOperationV1, PrepareHostV1, PrepareSuccessV1,
    TransportMode, MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::digest::anchor_id as compute_anchor_id;
use riot_anchor_protocol::records::{
    AnchorDescriptorBodyV1, AnchorLimitProfileV1, DescriptorEnvelopeV1, EnabledRole,
    OperatorVerificationKeyV1, PublicSiteTicketV2Core, RootSignedTicketCoreEnvelopeV2,
    TransportFloor,
};

use riot_transport::router::{AlpnRouter, BoxRead, BoxWrite, Deadlines, RouterConnection};
use riot_transport::{TransportError, ALPN_ANCHOR_V1};

// ---------------------------------------------------------------------------
// In-memory RouterConnection (mirrors crates/riot-transport/tests/alpn_router.rs)
// ---------------------------------------------------------------------------

struct FakeInner {
    alpn: Option<Vec<u8>>,
    halves: std::sync::Mutex<Option<(BoxWrite, BoxRead)>>,
}

#[derive(Clone)]
struct FakeConn {
    inner: Arc<FakeInner>,
}

impl FakeConn {
    fn new(alpn: Option<&[u8]>, halves: Option<(BoxWrite, BoxRead)>) -> Self {
        Self {
            inner: Arc::new(FakeInner {
                alpn: alpn.map(|a| a.to_vec()),
                halves: std::sync::Mutex::new(halves),
            }),
        }
    }
}

impl RouterConnection for FakeConn {
    fn negotiated_alpn(&self) -> Option<Vec<u8>> {
        self.inner.alpn.clone()
    }

    fn export_keying_material(
        &self,
        _label: &[u8],
        _context: &[u8],
        out_len: usize,
    ) -> Result<Vec<u8>, TransportError> {
        Ok(vec![0xAB; out_len])
    }

    async fn accept_bi(&self) -> Result<(BoxWrite, BoxRead), TransportError> {
        let taken = self.inner.halves.lock().unwrap().take();
        taken.ok_or(TransportError::StreamClosed)
    }

    async fn accept_extra(&self) {
        // A well-behaved peer opens exactly one stream: this never resolves.
        std::future::pending::<()>().await
    }

    fn close(&self, _reason: &[u8]) {}
}

/// Wire the server-side (router) halves plus the peer-side duplex streams the
/// test drives. Returns `(server_halves, peer_writer, peer_reader)`.
fn wire() -> ((BoxWrite, BoxRead), DuplexStream, DuplexStream) {
    let (peer_to_server, server_recv) = tokio::io::duplex(1 << 20);
    let (server_send, server_from_peer) = tokio::io::duplex(1 << 20);
    let halves: (BoxWrite, BoxRead) = (Box::pin(server_send), Box::pin(server_recv));
    (halves, peer_to_server, server_from_peer)
}

async fn write_frame(w: &mut DuplexStream, body: &[u8]) {
    let len = (body.len() as u32).to_be_bytes();
    w.write_all(&len).await.unwrap();
    w.write_all(body).await.unwrap();
    w.flush().await.unwrap();
}

/// Read one length-prefixed frame, or `None` on EOF (stream closed).
async fn read_frame(r: &mut DuplexStream) -> Option<Vec<u8>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await.ok()?;
    let n = u32::from_be_bytes(len) as usize;
    let mut body = vec![0u8; n];
    r.read_exact(&mut body).await.ok()?;
    Some(body)
}

// ---------------------------------------------------------------------------
// Fixtures (mirror crates/riot-anchor/tests/control_prepare.rs constructions)
// ---------------------------------------------------------------------------

fn sk(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}
fn pk(k: &SigningKey) -> [u8; 32] {
    k.verifying_key().to_bytes()
}
fn vkey(k: &SigningKey) -> OperatorVerificationKeyV1 {
    OperatorVerificationKeyV1 { public_key: pk(k) }
}
fn d32(seed: u8) -> [u8; 32] {
    [seed; 32]
}
fn d16(seed: u8) -> [u8; 16] {
    [seed; 16]
}

fn descriptor(operator: &SigningKey) -> DescriptorEnvelopeV1 {
    let genesis_random = d32(99);
    let anchor = compute_anchor_id(&pk(operator), &genesis_random);
    let current_key = vkey(operator);
    let body = AnchorDescriptorBodyV1 {
        anchor_id: anchor,
        genesis_operator_public_key: pk(operator),
        genesis_random_256_bits: genesis_random,
        current_operator_verification_key: current_key,
        current_operator_key_id: current_key.operator_key_id().unwrap(),
        descriptor_epoch: 0,
        previous_descriptor_digest: None,
        current_iroh_endpoint_id: d32(40),
        https_origin: "https://anchor.example".to_string(),
        operator_display_label: "Example Anchor".to_string(),
        self_reported_failure_domain_label: "eu-west".to_string(),
        supported_control_versions: vec![1],
        supported_sync_versions: vec![1, 2],
        enabled_roles: vec![EnabledRole::Host, EnabledRole::Mirror],
        limit_profile_digest: AnchorLimitProfileV1::mvp_defaults(0)
            .limit_profile_digest()
            .unwrap(),
        predecessor_operator_verification_key: None,
        issued_at: 1000,
        expires_at: 5000,
    };
    let mut env = DescriptorEnvelopeV1 {
        body,
        current_signature: [0u8; 64],
        predecessor_signature: None,
    };
    let preimage = env.current_signing_preimage().unwrap();
    env.current_signature = operator.sign(&preimage).to_bytes();
    env
}

fn context(operator: &SigningKey) -> AnchorControlContext {
    let desc = descriptor(operator);
    let current_key = vkey(operator);
    AnchorControlContext {
        anchor_id: desc.body.anchor_id,
        operator_key_id: current_key.operator_key_id().unwrap(),
        operator_public_key: pk(operator),
        descriptor_epoch: 0,
        descriptor_digest: desc.descriptor_digest().unwrap(),
        descriptor: desc,
        limit_profile: AnchorLimitProfileV1::mvp_defaults(0),
        sync_version: 2,
        operation_lifetime_secs: 3600,
    }
}

/// Build a root-signed ticket from the default valid core, applying `mutate`
/// BEFORE signing so the signature stays valid over the (possibly hostile)
/// coordinates. The default is a ticket the canonical authority gate accepts.
fn signed_ticket(
    root: &SigningKey,
    mutate: impl FnOnce(&mut PublicSiteTicketV2Core),
) -> RootSignedTicketCoreEnvelopeV2 {
    let mut core = PublicSiteTicketV2Core {
        root_id: pk(root),
        o_namespace_id: d32(10),
        c_namespace_id: d32(11),
        w_namespace_id: d32(12),
        manifest_digest: d32(13),
        manifest_version: 3,
        min_sync_version: 2,
        manifest_required_transport: TransportFloor::RequireNone,
        transport_floor: TransportFloor::RequireNone,
        transport_epoch: 1,
        issued_unix_seconds: 1000,
        expiry_unix_seconds: 100_000,
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

/// A ticket the anchor's real authority check accepts: signed by `root`, whose
/// public key IS the ticket `root_id`.
fn ticket_core(root: &SigningKey) -> RootSignedTicketCoreEnvelopeV2 {
    signed_ticket(root, |_| {})
}

fn prepare_body(snapshot_seed: u8) -> PrepareHostV1 {
    prepare_body_with_ticket(snapshot_seed, ticket_core(&sk(9)))
}

fn prepare_body_with_ticket(
    snapshot_seed: u8,
    ticket: RootSignedTicketCoreEnvelopeV2,
) -> PrepareHostV1 {
    PrepareHostV1 {
        root_signed_ticket_core: ticket,
        ordered_namespace_snapshot_digests: [
            d32(snapshot_seed),
            d32(snapshot_seed.wrapping_add(1)),
            d32(snapshot_seed.wrapping_add(2)),
        ],
        work_stamp: None,
    }
}

fn prepare_frame(key: [u8; 16], body: PrepareHostV1) -> Vec<u8> {
    ControlRequestV1 {
        idempotency_key: key,
        operation: ControlOperation::PrepareHost(Box::new(body)),
    }
    .encode_canonical()
    .expect("encode prepare request")
}

fn entropy_from(seed: u8) -> impl FnMut() -> [u8; 32] {
    let mut n = seed;
    move || {
        let value = [n; 32];
        n = n.wrapping_add(1);
        value
    }
}

/// The control service the actor tests drive (operator seed 1, ring epoch 1).
fn control_service() -> AnchorControlService<TicketRootAuthorityPolicy, Ed25519OperatorSigner> {
    let op = sk(1);
    let ctx = context(&op);
    let policy = TicketRootAuthorityPolicy::new(ctx.sync_version);
    let signer = Ed25519OperatorSigner::from_secret_bytes(op.to_bytes());
    AnchorControlService::new(ctx, policy, signer, TokenSecretRing::new(1, d32(200)))
}

/// Build a running actor + registered anchor/1 handler, returning the router.
fn built_router() -> AlpnRouter {
    let service = control_service();
    let repo = AnchorRepository::open_in_memory().unwrap();
    let (tx, _actor) = spawn_control_actor(repo, service, Box::new(entropy_from(0x50)), 8);
    let now_fn: Arc<dyn Fn() -> u64 + Send + Sync> = Arc::new(|| 1_500u64);
    let handler = control_handler(tx, now_fn);

    let mut router = AlpnRouter::new(8);
    router.register(ALPN_ANCHOR_V1, Deadlines::control(), handler);
    router
}

fn expect_prepare_success(bytes: &[u8]) -> PrepareSuccessV1 {
    let response = decode_canonical::<ControlResponseV1>(bytes, MAX_CONTROL_FRAME_BYTES).unwrap();
    match response.outcome {
        ControlOutcome::Success(ControlSuccess::PrepareHost(payload)) => *payload,
        other => panic!("expected prepare success, got {other:?}"),
    }
}

/// Drive one PrepareHost request through a fresh router/actor and return the
/// decoded control response (success OR refusal).
async fn single_prepare(body: PrepareHostV1) -> ControlResponseV1 {
    let router = built_router();
    let (halves, mut peer_w, mut peer_r) = wire();
    let conn = FakeConn::new(Some(ALPN_ANCHOR_V1), Some(halves));
    let dispatch = tokio::spawn(async move { router.dispatch(conn).await });

    write_frame(&mut peer_w, &prepare_frame(d16(1), body)).await;
    let bytes = read_frame(&mut peer_r).await.expect("response frame");
    drop(peer_w);
    let _ = dispatch.await.unwrap();
    decode_canonical::<ControlResponseV1>(&bytes, MAX_CONTROL_FRAME_BYTES).unwrap()
}

/// Assert the response is a refusal (NOT a prepared/stored success) and return it.
fn expect_refusal(response: &ControlResponseV1) -> ControlRefusal {
    match &response.outcome {
        ControlOutcome::Refused(refusal) => refusal.clone(),
        ControlOutcome::Success(ControlSuccess::PrepareHost(_)) => {
            panic!("ticket was ADMITTED and prepared; expected a refusal")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn prepare_host_round_trip_over_router_stores_operation() {
    let router = built_router();

    let (halves, mut peer_w, mut peer_r) = wire();
    let conn = FakeConn::new(Some(ALPN_ANCHOR_V1), Some(halves));
    let dispatch = tokio::spawn(async move { router.dispatch(conn).await });

    // 1) A real, root-signed PrepareHost frame in → a signed PrepareSuccess out.
    write_frame(&mut peer_w, &prepare_frame(d16(1), prepare_body(30))).await;
    let response_bytes = read_frame(&mut peer_r)
        .await
        .expect("prepare response frame");
    let success = expect_prepare_success(&response_bytes);
    assert_ne!(
        success.operation_id, [0u8; 32],
        "an operation id was minted"
    );
    assert_eq!(
        success.ordered_namespace_host_plan,
        [d32(10), d32(11), d32(12)],
        "the host plan is the ticket's O/C/W namespaces",
    );
    assert_eq!(success.operation_expiry, 1_500 + 3600);
    // Tokens are the deterministic derivation, never zero.
    for token in success.ordered_namespace_tokens {
        assert_ne!(token, [0u8; 32]);
    }

    // 2) The durable store reflects the prepared operation: GetOperation returns
    //    Prepared for the same id.
    let getop = ControlRequestV1 {
        idempotency_key: d16(0xEE),
        operation: ControlOperation::GetOperation(GetOperationV1 {
            operation_id: success.operation_id,
        }),
    }
    .encode_canonical()
    .unwrap();
    write_frame(&mut peer_w, &getop).await;
    let getop_bytes = read_frame(&mut peer_r).await.expect("getop response frame");
    let getop_response =
        decode_canonical::<ControlResponseV1>(&getop_bytes, MAX_CONTROL_FRAME_BYTES).unwrap();
    match getop_response.outcome {
        ControlOutcome::Success(ControlSuccess::GetOperation(payload)) => {
            assert_eq!(payload.operation_id, success.operation_id);
            assert!(
                matches!(payload.state, GetOperationState::Prepared { .. }),
                "repository reflects the prepared operation",
            );
        }
        other => panic!("expected getop success, got {other:?}"),
    }

    // Close the peer's write half: the handler's next read hits EOF and the
    // session ends cleanly.
    drop(peer_w);
    let out = dispatch.await.unwrap();
    assert!(out.is_ok(), "session completes cleanly: {out:?}");
}

#[tokio::test]
async fn garbage_frame_closes_without_response_or_panic() {
    let router = built_router();

    let (halves, mut peer_w, mut peer_r) = wire();
    let conn = FakeConn::new(Some(ALPN_ANCHOR_V1), Some(halves));
    let dispatch = tokio::spawn(async move { router.dispatch(conn).await });

    // A frame that is not a canonical ControlRequestV1: a bounded protocol
    // failure. The handler must close the stream with NO response frame.
    write_frame(&mut peer_w, &[0xff, 0x00, 0x13, 0x37]).await;
    drop(peer_w);

    assert!(
        read_frame(&mut peer_r).await.is_none(),
        "a protocol failure produces no response frame",
    );
    let out = dispatch.await.unwrap();
    assert!(out.is_ok(), "handler closes gracefully, no panic: {out:?}");
}

// ---------------------------------------------------------------------------
// Canonical authority gates the daemon MUST enforce (delegated to
// `admit_public_site_ticket`, not hand-rolled). Each flips ONE root-signed field
// of an otherwise-valid ticket and asserts a refusal — the ticket is never
// admitted/prepared.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn require_arti_ticket_is_refused_unsupported_transport() {
    // A root DEMANDING require_arti must NOT be hosted on the clearnet MVP anchor
    // (Product Decision 11 / design spec:172). The hand-rolled subset dropped this.
    let ticket = signed_ticket(&sk(9), |core| {
        core.transport_floor = TransportFloor::RequireArti;
    });
    let response = single_prepare(prepare_body_with_ticket(30, ticket)).await;
    assert!(
        matches!(
            expect_refusal(&response),
            ControlRefusal::UnsupportedTransport {
                required_mode: TransportMode::RequireArti,
                observed_mode: TransportMode::RequireNone,
            }
        ),
        "require_arti must be refused unsupported_transport, got {:?}",
        response.outcome
    );
}

#[tokio::test]
async fn ticket_exceeding_max_lifetime_is_refused() {
    // expiry > issued + 90 days exceeds MAX_TICKET_LIFETIME_SECONDS. `now` (1500)
    // is still < expiry, so the hand-rolled `now >= expiry` check admitted it.
    let ticket = signed_ticket(&sk(9), |core| {
        core.issued_unix_seconds = 1_000;
        core.expiry_unix_seconds = 1_000 + 91 * 24 * 60 * 60; // 91 days > 90-day cap
    });
    assert!(
        matches!(
            expect_refusal(&single_prepare(prepare_body_with_ticket(40, ticket)).await),
            ControlRefusal::InvalidTicketAuthority
        ),
        "a ticket lifetime beyond the 90-day cap must be refused",
    );
}

#[tokio::test]
async fn ticket_with_wrong_min_sync_version_is_refused() {
    // Canonical requires min_sync_version == 2 exactly; the hand-rolled `>` check
    // admitted min_sync_version 1.
    let ticket = signed_ticket(&sk(9), |core| {
        core.min_sync_version = 1;
    });
    assert!(
        matches!(
            expect_refusal(&single_prepare(prepare_body_with_ticket(50, ticket)).await),
            ControlRefusal::UnsupportedVersion { .. }
        ),
        "min_sync_version != 2 must be refused unsupported_version",
    );
}

#[tokio::test]
async fn lower_ticket_epoch_is_refused_after_a_higher_epoch_was_admitted() {
    let router = built_router();
    let (halves, mut peer_w, mut peer_r) = wire();
    let conn = FakeConn::new(Some(ALPN_ANCHOR_V1), Some(halves));
    let dispatch = tokio::spawn(async move { router.dispatch(conn).await });
    let root = sk(9);

    let higher = signed_ticket(&root, |core| {
        core.transport_epoch = 5;
    });
    write_frame(
        &mut peer_w,
        &prepare_frame(d16(0x51), prepare_body_with_ticket(60, higher)),
    )
    .await;
    let admitted = read_frame(&mut peer_r)
        .await
        .expect("higher-epoch response");
    assert!(
        matches!(
            decode_canonical::<ControlResponseV1>(&admitted, MAX_CONTROL_FRAME_BYTES)
                .unwrap()
                .outcome,
            ControlOutcome::Success(ControlSuccess::PrepareHost(_))
        ),
        "higher epoch must establish the durable floor",
    );

    let lower = signed_ticket(&root, |core| {
        core.transport_epoch = 4;
    });
    write_frame(
        &mut peer_w,
        &prepare_frame(d16(0x52), prepare_body_with_ticket(70, lower)),
    )
    .await;
    let refused = read_frame(&mut peer_r).await.expect("lower-epoch response");
    assert!(
        matches!(
            decode_canonical::<ControlResponseV1>(&refused, MAX_CONTROL_FRAME_BYTES)
                .unwrap()
                .outcome,
            ControlOutcome::Refused(ControlRefusal::InvalidTicketAuthority)
        ),
        "a lower signed transport epoch must fail closed",
    );

    drop(peer_w);
    assert!(dispatch.await.unwrap().is_ok());
}

// ---------------------------------------------------------------------------
// The single-writer actor THREAD (WU-B): a control job still round-trips after
// the tokio::spawn -> std::thread move, and the thread stops when every sender
// is dropped (no leaked OS thread, no hang on shutdown).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn actor_thread_processes_jobs_and_stops_when_senders_drop() {
    let repo = AnchorRepository::open_in_memory().expect("open");
    let service = control_service();
    let (tx, join_handle) = spawn_control_actor(repo, service, Box::new(entropy_from(0x50)), 4);

    // A garbage frame must yield Close (decode failure), not a hang or panic.
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    tx.send(ActorJob::Control(ControlJob {
        request: vec![0xFF; 8],
        now: 1_700_000_000,
        reply: reply_tx,
    }))
    .expect("actor alive");
    assert_eq!(reply_rx.await.expect("reply"), ControlReply::Close);

    // Dropping the last sender must let the thread exit: join must complete.
    drop(tx);
    tokio::task::spawn_blocking(move || join_handle.join().expect("actor thread exits cleanly"))
        .await
        .expect("join");
}

// ---------------------------------------------------------------------------
// Cleanup-delivery losslessness (WU-C fix): the actor channel is UNBOUNDED, so
// enqueueing never fails or blocks under a busy queue. The old bounded(64)
// channel made the SyncCloseGuard's `try_send` silently drop session Closes
// whenever 64+ jobs were queued — routine backpressure with 256 concurrent
// handlers — permanently stranding SyncSessionTable slots. This test floods
// the queue far past the old capacity from the sending side WITHOUT draining a
// single reply, then verifies every job (the stand-in for a guard Close at the
// back of a busy queue) is delivered and processed.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn busy_actor_queue_never_drops_jobs() {
    let repo = AnchorRepository::open_in_memory().expect("open");
    let service = control_service();
    let (tx, join_handle) = spawn_control_actor(repo, service, Box::new(entropy_from(0x50)), 4);

    // Enqueue 200 jobs synchronously — over 3x the old bounded capacity —
    // before the actor can possibly have drained them. Every send must
    // succeed immediately (send is sync and infallible while the actor
    // lives); with the old bounded channel this loop would have blocked or,
    // via try_send, dropped jobs.
    let mut replies = Vec::new();
    for _ in 0..200 {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        tx.send(ActorJob::Control(ControlJob {
            request: vec![0xFF; 8],
            now: 1_700_000_000,
            reply: reply_tx,
        }))
        .expect("unbounded send never fails while the actor lives");
        replies.push(reply_rx);
    }

    // Every queued job — including the last one, sent into the busiest
    // possible queue — is processed: no reply channel is ever dropped
    // unanswered.
    for reply_rx in replies {
        assert_eq!(reply_rx.await.expect("reply"), ControlReply::Close);
    }

    drop(tx);
    tokio::task::spawn_blocking(move || join_handle.join().expect("actor thread exits cleanly"))
        .await
        .expect("join");
}
