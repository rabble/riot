//! WU-014 admission / idempotency / work / Prepare matrix.
//!
//! These pin the design's exhaustive `riot/anchor/1` admission ordering and the
//! recoverable Prepare → GetOperation lifecycle:
//!
//! * ordering spies — cheap checks (bound, decode, digest, idempotency lookup)
//!   precede every expensive check and any durable claim;
//! * collision / replay — same-key/same-body replays byte-identically, a changed
//!   body under a claimed key is `idempotency_conflict` without disclosure;
//! * pre-claim retry-through-success — a busy / quota / work refusal writes no
//!   row, so the same key retries and still succeeds;
//! * work — a changed / insufficient stamp is refused pre-claim; the pressure
//!   band decides whether work is required at all;
//! * namespace tokens — deterministic derivation and secret-epoch rotation;
//! * GetOperation — prepared / terminal / expiry / unknown, across a restart.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use ed25519_dalek::{Signer, SigningKey};

use riot_anchor::control::{
    AdmissionPolicy, AnchorControlContext, AnchorControlService, ControlHandling, PreparePlan,
    ProtocolFailure,
};
use riot_anchor::repository::AnchorRepository;
use riot_anchor::work::{derive_namespace_token, OperatorSigner, PressurePolicy, TokenSecretRing};

use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::{
    ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1, ControlResponseV1,
    ControlSuccess, GetOperationState, GetOperationV1, GetWorkChallengeV1, PrepareHostV1,
    PrepareKind, PrepareSuccessV1, TerminalOperationOutcome,
};
use riot_anchor_protocol::digest::{anchor_id as compute_anchor_id, digest_v1, label, work_proof};
use riot_anchor_protocol::records::{
    AnchorDescriptorBodyV1, AnchorLimitId, AnchorLimitProfileV1, ControlOperationKind,
    DescriptorEnvelopeV1, EnabledRole, OperatorVerificationKeyV1, PublicSiteTicketV2Core,
    RootSignedTicketCoreEnvelopeV2, TransportFloor, WorkChallengeV1, WorkStampV1,
};

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!("riot-anchor-ctrl-{}-{}.db", std::process::id(), id));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }
    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

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

/// A test operator signer producing real Ed25519 signatures.
struct TestSigner(SigningKey);
impl OperatorSigner for TestSigner {
    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        self.0.sign(preimage).to_bytes()
    }
}

/// A recording admission policy: it records the exact stage order and can be
/// told to refuse at authority or capacity, and to advertise a pressure band.
#[derive(Clone)]
struct SpyPolicy {
    plan: PreparePlan,
    authorize_refusal: Rc<RefCell<Option<ControlRefusal>>>,
    capacity_refusal: Rc<RefCell<Option<ControlRefusal>>>,
    pressure: Rc<RefCell<PressurePolicy>>,
    calls: Rc<RefCell<Vec<Stage>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Authorize,
    Capacity,
    Work,
}

impl SpyPolicy {
    fn new(community_root: [u8; 32]) -> Self {
        SpyPolicy {
            plan: PreparePlan {
                community_root,
                ordered_namespace_host_plan: [d32(10), d32(11), d32(12)],
                ordered_retained_snapshot_digests: [d32(20), d32(21), d32(22)],
                base_generation: 7,
            },
            authorize_refusal: Rc::new(RefCell::new(None)),
            capacity_refusal: Rc::new(RefCell::new(None)),
            pressure: Rc::new(RefCell::new(PressurePolicy {
                policy_epoch: 3,
                difficulty: 0,
            })),
            calls: Rc::new(RefCell::new(Vec::new())),
        }
    }
    fn calls(&self) -> Vec<Stage> {
        self.calls.borrow().clone()
    }
    fn reset_calls(&self) {
        self.calls.borrow_mut().clear();
    }
}

impl AdmissionPolicy for SpyPolicy {
    fn authorize_prepare_host(
        &self,
        _request: &PrepareHostV1,
        _observed_at: u64,
    ) -> Result<PreparePlan, ControlRefusal> {
        self.calls.borrow_mut().push(Stage::Authorize);
        match self.authorize_refusal.borrow().clone() {
            Some(refusal) => Err(refusal),
            None => Ok(self.plan),
        }
    }
    fn capacity_for_prepare_host(
        &self,
        _plan: &PreparePlan,
        _observed_at: u64,
    ) -> Result<(), ControlRefusal> {
        self.calls.borrow_mut().push(Stage::Capacity);
        match self.capacity_refusal.borrow().clone() {
            Some(refusal) => Err(refusal),
            None => Ok(()),
        }
    }
    fn pressure_band(&self, _community_root: &[u8; 32], _observed_at: u64) -> PressurePolicy {
        self.calls.borrow_mut().push(Stage::Work);
        *self.pressure.borrow()
    }
}

const COMMUNITY_ROOT: [u8; 32] = [70u8; 32];

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
        limit_profile_digest: d32(50),
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

fn service(
    operator: &SigningKey,
    policy: SpyPolicy,
) -> AnchorControlService<SpyPolicy, TestSigner> {
    AnchorControlService::new(
        context(operator),
        policy,
        TestSigner(operator.clone()),
        TokenSecretRing::new(1, d32(200)),
    )
}

fn ticket_core(root: &SigningKey) -> RootSignedTicketCoreEnvelopeV2 {
    let core = PublicSiteTicketV2Core {
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
        expiry_unix_seconds: 2000,
    };
    let mut env = RootSignedTicketCoreEnvelopeV2 {
        core,
        root_signature: [0u8; 64],
    };
    let preimage = env.signing_preimage().unwrap();
    env.root_signature = root.sign(&preimage).to_bytes();
    env
}

fn prepare_body(snapshot_seed: u8, work_stamp: Option<WorkStampV1>) -> PrepareHostV1 {
    PrepareHostV1 {
        root_signed_ticket_core: ticket_core(&sk(9)),
        ordered_namespace_snapshot_digests: [
            d32(snapshot_seed),
            d32(snapshot_seed.wrapping_add(1)),
            d32(snapshot_seed.wrapping_add(2)),
        ],
        work_stamp,
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

fn work_target_of(body: &PrepareHostV1) -> [u8; 32] {
    let mut bare = body.clone();
    bare.work_stamp = None;
    ControlOperation::PrepareHost(Box::new(bare))
        .work_target_digest()
        .expect("work target digest")
}

fn leading_zero_bits(bytes: &[u8; 32]) -> u32 {
    let mut count = 0;
    for &byte in bytes {
        if byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}

/// Mine a valid work stamp against a signed challenge for the given difficulty.
fn mine_stamp(challenge: &WorkChallengeV1, difficulty: u64) -> WorkStampV1 {
    let bytes = challenge.encode_canonical().expect("encode challenge");
    let challenge_digest = digest_v1(label::WORK_CHALLENGE_ENVELOPE, &bytes);
    let mut counter = 0u64;
    loop {
        let proof = work_proof(&challenge_digest, counter);
        if u64::from(leading_zero_bits(&proof)) >= difficulty {
            return WorkStampV1 {
                challenge_envelope_bytes: bytes,
                counter,
                proof_bytes: proof,
            };
        }
        counter += 1;
    }
}

/// Ask the service to issue a signed work challenge for a prepare request body.
fn fetch_challenge(
    svc: &AnchorControlService<SpyPolicy, TestSigner>,
    repo: &mut AnchorRepository,
    key: [u8; 16],
    body: &PrepareHostV1,
    now: u64,
) -> WorkChallengeV1 {
    let request = GetWorkChallengeV1 {
        intended_operation_kind: ControlOperationKind::PrepareHost,
        intended_idempotency_key: key,
        community_root: COMMUNITY_ROOT,
        work_target_digest: work_target_of(body),
    };
    let frame = ControlRequestV1 {
        idempotency_key: key,
        operation: ControlOperation::GetWorkChallenge(request),
    }
    .encode_canonical()
    .unwrap();
    let mut entropy = entropy_from(0xC0);
    match svc.handle(repo, &frame, now, &mut entropy).unwrap() {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::GetWorkChallenge(challenge)),
            ..
        }) => *challenge,
        other => panic!("expected work challenge, got {other:?}"),
    }
}

fn entropy_from(seed: u8) -> impl FnMut() -> [u8; 32] {
    let mut n = seed;
    move || {
        let value = [n; 32];
        n = n.wrapping_add(1);
        value
    }
}

fn expect_prepare_success(handling: &ControlHandling) -> PrepareSuccessV1 {
    match handling {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::PrepareHost(payload)),
            ..
        }) => *payload.clone(),
        other => panic!("expected prepare success, got {other:?}"),
    }
}

fn expect_refusal(handling: &ControlHandling) -> ControlRefusal {
    match handling {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Refused(refusal),
            ..
        }) => refusal.clone(),
        other => panic!("expected refusal, got {other:?}"),
    }
}

fn get_operation(
    svc: &AnchorControlService<SpyPolicy, TestSigner>,
    repo: &mut AnchorRepository,
    operation_id: [u8; 32],
    now: u64,
) -> ControlHandling {
    let frame = ControlRequestV1 {
        idempotency_key: d16(0xEE),
        operation: ControlOperation::GetOperation(GetOperationV1 { operation_id }),
    }
    .encode_canonical()
    .unwrap();
    let mut entropy = entropy_from(0);
    svc.handle(repo, &frame, now, &mut entropy).unwrap()
}

// ---------------------------------------------------------------------------
// Describe / protocol bounds.
// ---------------------------------------------------------------------------

#[test]
fn describe_returns_descriptor_and_limit_profile() {
    let op = sk(1);
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let frame = ControlRequestV1 {
        idempotency_key: d16(1),
        operation: ControlOperation::Describe(riot_anchor_protocol::control::DescribeV1),
    }
    .encode_canonical()
    .unwrap();
    let mut entropy = entropy_from(0);
    match svc.handle(&mut repo, &frame, 1000, &mut entropy).unwrap() {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::Describe(payload)),
            ..
        }) => {
            assert_eq!(payload.limit_profile, AnchorLimitProfileV1::mvp_defaults(0));
        }
        other => panic!("expected describe success, got {other:?}"),
    }
}

#[test]
fn oversize_frame_is_protocol_failure_before_any_work() {
    let op = sk(1);
    let policy = SpyPolicy::new(COMMUNITY_ROOT);
    let svc = service(&op, policy.clone());
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let frame = vec![0u8; 64 * 1024 + 1];
    let mut entropy = entropy_from(0);
    assert_eq!(
        svc.handle(&mut repo, &frame, 1000, &mut entropy).unwrap(),
        ControlHandling::ProtocolFailure(ProtocolFailure::FrameTooLarge)
    );
    assert!(policy.calls().is_empty(), "no admission stage ran");
}

#[test]
fn malformed_frame_is_protocol_failure_not_refusal() {
    let op = sk(1);
    let policy = SpyPolicy::new(COMMUNITY_ROOT);
    let svc = service(&op, policy.clone());
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let frame = vec![0xff, 0x00, 0x13, 0x37];
    let mut entropy = entropy_from(0);
    assert_eq!(
        svc.handle(&mut repo, &frame, 1000, &mut entropy).unwrap(),
        ControlHandling::ProtocolFailure(ProtocolFailure::Malformed)
    );
    assert!(policy.calls().is_empty(), "decode precedes every check");
}

// ---------------------------------------------------------------------------
// Admission ordering.
// ---------------------------------------------------------------------------

#[test]
fn ordering_runs_cheap_checks_before_durable_claim() {
    let op = sk(1);
    let policy = SpyPolicy::new(COMMUNITY_ROOT);
    let svc = service(&op, policy.clone());
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let frame = prepare_frame(d16(1), prepare_body(30, None));
    let mut entropy = entropy_from(0x50);
    let handling = svc.handle(&mut repo, &frame, 1000, &mut entropy).unwrap();
    let success = expect_prepare_success(&handling);
    // The authority → capacity → work order ran, in that order.
    assert_eq!(
        policy.calls(),
        vec![Stage::Authorize, Stage::Capacity, Stage::Work]
    );
    // The durable claim happened: the operation is retrievable by id.
    let getop = get_operation(&svc, &mut repo, success.operation_id, 1010);
    assert!(matches!(
        getop,
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::GetOperation(_)),
            ..
        })
    ));
}

#[test]
fn authority_refusal_is_pre_claim_and_key_retries() {
    let op = sk(1);
    let policy = SpyPolicy::new(COMMUNITY_ROOT);
    *policy.authorize_refusal.borrow_mut() = Some(ControlRefusal::InvalidTicketAuthority);
    let svc = service(&op, policy.clone());
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let frame = prepare_frame(d16(1), prepare_body(30, None));

    let mut entropy = entropy_from(0x50);
    let refused = svc.handle(&mut repo, &frame, 1000, &mut entropy).unwrap();
    assert_eq!(
        expect_refusal(&refused),
        ControlRefusal::InvalidTicketAuthority
    );
    // Only authority ran; capacity and work never did (authority precedes them).
    assert_eq!(policy.calls(), vec![Stage::Authorize]);

    // No row was claimed: clearing the refusal and retrying the SAME key succeeds.
    *policy.authorize_refusal.borrow_mut() = None;
    policy.reset_calls();
    let mut entropy = entropy_from(0x60);
    let ok = svc.handle(&mut repo, &frame, 1000, &mut entropy).unwrap();
    let _ = expect_prepare_success(&ok);
}

#[test]
fn capacity_busy_refusal_is_pre_claim_and_key_retries() {
    let op = sk(1);
    let policy = SpyPolicy::new(COMMUNITY_ROOT);
    *policy.capacity_refusal.borrow_mut() = Some(ControlRefusal::AdmissionBusy {
        limit_id: AnchorLimitId::from_id(1).unwrap(),
        retry_after_seconds: 5,
    });
    let svc = service(&op, policy.clone());
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let frame = prepare_frame(d16(2), prepare_body(40, None));

    let mut entropy = entropy_from(0x50);
    let refused = svc.handle(&mut repo, &frame, 2000, &mut entropy).unwrap();
    assert!(matches!(
        expect_refusal(&refused),
        ControlRefusal::AdmissionBusy { .. }
    ));
    // Authority then capacity ran; work did NOT (capacity precedes work here).
    assert_eq!(policy.calls(), vec![Stage::Authorize, Stage::Capacity]);

    // Busy leaves no row: same key/body retries after headroom returns.
    *policy.capacity_refusal.borrow_mut() = None;
    policy.reset_calls();
    let mut entropy = entropy_from(0x70);
    let ok = svc.handle(&mut repo, &frame, 2000, &mut entropy).unwrap();
    let _ = expect_prepare_success(&ok);
}

// ---------------------------------------------------------------------------
// Idempotency collision / replay.
// ---------------------------------------------------------------------------

#[test]
fn same_key_same_body_replays_byte_identical() {
    let op = sk(1);
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let frame = prepare_frame(d16(3), prepare_body(50, None));

    let mut entropy = entropy_from(0x50);
    let first = svc.handle(&mut repo, &frame, 3000, &mut entropy).unwrap();
    // A replay must NOT consume fresh entropy or create a new operation id.
    let mut entropy = entropy_from(0x99);
    let replay = svc.handle(&mut repo, &frame, 3005, &mut entropy).unwrap();
    assert_eq!(
        first, replay,
        "same key + same body replays byte-identically"
    );
}

#[test]
fn same_key_changed_body_is_conflict_without_disclosure() {
    let op = sk(1);
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let key = d16(4);

    let mut entropy = entropy_from(0x50);
    let first = svc
        .handle(
            &mut repo,
            &prepare_frame(key, prepare_body(60, None)),
            4000,
            &mut entropy,
        )
        .unwrap();
    let first_success = expect_prepare_success(&first);

    // Same key, different body (different snapshot digests) → conflict.
    let mut entropy = entropy_from(0x80);
    let conflict = svc
        .handle(
            &mut repo,
            &prepare_frame(key, prepare_body(61, None)),
            4001,
            &mut entropy,
        )
        .unwrap();
    assert_eq!(
        expect_refusal(&conflict),
        ControlRefusal::IdempotencyConflict
    );

    // The stored state is unchanged and undisclosed: the ORIGINAL body still
    // replays its exact original operation.
    let mut entropy = entropy_from(0x81);
    let replay = svc
        .handle(
            &mut repo,
            &prepare_frame(key, prepare_body(60, None)),
            4002,
            &mut entropy,
        )
        .unwrap();
    assert_eq!(
        expect_prepare_success(&replay).operation_id,
        first_success.operation_id
    );
}

// ---------------------------------------------------------------------------
// Admission work.
// ---------------------------------------------------------------------------

#[test]
fn work_required_is_pre_claim_and_retry_with_stamp_succeeds() {
    let op = sk(1);
    let policy = SpyPolicy::new(COMMUNITY_ROOT);
    *policy.pressure.borrow_mut() = PressurePolicy {
        policy_epoch: 3,
        difficulty: 8,
    };
    let svc = service(&op, policy.clone());
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let key = d16(5);

    // No stamp under a work-requiring band → `work_required`, pre-claim.
    let body = prepare_body(70, None);
    let mut entropy = entropy_from(0x50);
    let refused = svc
        .handle(
            &mut repo,
            &prepare_frame(key, body.clone()),
            5000,
            &mut entropy,
        )
        .unwrap();
    assert!(matches!(
        expect_refusal(&refused),
        ControlRefusal::WorkRequired {
            policy_epoch: 3,
            difficulty: 8
        }
    ));

    // Same key retries with a freshly mined stamp and succeeds (no prior digest).
    let challenge = fetch_challenge(&svc, &mut repo, key, &body, 5001);
    let stamp = mine_stamp(&challenge, 8);
    let stamped = prepare_body(70, Some(stamp));
    let mut entropy = entropy_from(0x60);
    let ok = svc
        .handle(&mut repo, &prepare_frame(key, stamped), 5002, &mut entropy)
        .unwrap();
    let _ = expect_prepare_success(&ok);
}

#[test]
fn changed_work_stamp_bound_to_other_request_is_refused_pre_claim() {
    let op = sk(1);
    let policy = SpyPolicy::new(COMMUNITY_ROOT);
    *policy.pressure.borrow_mut() = PressurePolicy {
        policy_epoch: 3,
        difficulty: 8,
    };
    let svc = service(&op, policy);
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let key = d16(6);

    // Mine a stamp for a DIFFERENT body (different work_target_digest)...
    let other_body = prepare_body(80, None);
    let challenge = fetch_challenge(&svc, &mut repo, key, &other_body, 6000);
    let mismatched = mine_stamp(&challenge, 8);

    // ...then attach it to the real request. Its bound target no longer matches.
    let body = prepare_body(90, Some(mismatched));
    let mut entropy = entropy_from(0x50);
    let refused = svc
        .handle(&mut repo, &prepare_frame(key, body), 6001, &mut entropy)
        .unwrap();
    assert!(matches!(
        expect_refusal(&refused),
        ControlRefusal::WorkRequired { .. }
    ));
}

#[test]
fn low_pressure_band_needs_no_work_high_band_does() {
    let op = sk(1);
    // Low band: difficulty 0, no stamp, succeeds.
    let low = SpyPolicy::new(COMMUNITY_ROOT);
    let svc_low = service(&op, low);
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let mut entropy = entropy_from(0x50);
    let ok = svc_low
        .handle(
            &mut repo,
            &prepare_frame(d16(7), prepare_body(100, None)),
            7000,
            &mut entropy,
        )
        .unwrap();
    let _ = expect_prepare_success(&ok);

    // High band: difficulty > 0, no stamp, refused.
    let high = SpyPolicy::new(COMMUNITY_ROOT);
    *high.pressure.borrow_mut() = PressurePolicy {
        policy_epoch: 9,
        difficulty: 6,
    };
    let svc_high = service(&op, high);
    let mut repo2 = AnchorRepository::open_in_memory().unwrap();
    let mut entropy = entropy_from(0x50);
    let refused = svc_high
        .handle(
            &mut repo2,
            &prepare_frame(d16(8), prepare_body(110, None)),
            7000,
            &mut entropy,
        )
        .unwrap();
    assert!(matches!(
        expect_refusal(&refused),
        ControlRefusal::WorkRequired {
            policy_epoch: 9,
            difficulty: 6
        }
    ));
}

// ---------------------------------------------------------------------------
// Prepare atomic store + namespace tokens.
// ---------------------------------------------------------------------------

#[test]
fn prepare_stores_base_generation_tokens_expiry_and_kind() {
    let op = sk(1);
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let mut entropy = entropy_from(0xA0);
    let handling = svc
        .handle(
            &mut repo,
            &prepare_frame(d16(9), prepare_body(120, None)),
            8000,
            &mut entropy,
        )
        .unwrap();
    let success = expect_prepare_success(&handling);

    assert_eq!(success.base_site_generation, 7);
    assert_eq!(success.operation_expiry, 8000 + 3600);
    assert_eq!(success.sync_version, 2);
    assert_eq!(
        success.ordered_namespace_host_plan,
        [d32(10), d32(11), d32(12)]
    );
    assert_eq!(
        success.ordered_retained_snapshot_digests,
        [d32(20), d32(21), d32(22)]
    );

    // The tokens are exactly the deterministic derivation under the current
    // secret/epoch — never zero, and one per namespace.
    for (index, namespace_id) in success.ordered_namespace_host_plan.iter().enumerate() {
        let expected = derive_namespace_token(
            &d32(200),
            &success.operation_id,
            namespace_id,
            success.operation_expiry,
            1,
        );
        assert_eq!(success.ordered_namespace_tokens[index], expected);
    }

    // The stored operation carries the originating kind.
    let stored = repo.load_operation(&success.operation_id).unwrap().unwrap();
    assert_eq!(
        stored.originating_kind,
        riot_anchor::repository::OperationKind::Host
    );
    assert_eq!(stored.token_secret_epoch, 1);
}

#[test]
fn namespace_tokens_are_deterministic_and_rotate_with_secret_epoch() {
    let secret_a = d32(200);
    let secret_b = d32(201);
    let op_id = d32(1);
    let ns = d32(10);
    let expiry = 4600;

    // Deterministic: same inputs → same token.
    let a1 = derive_namespace_token(&secret_a, &op_id, &ns, expiry, 1);
    let a2 = derive_namespace_token(&secret_a, &op_id, &ns, expiry, 1);
    assert_eq!(a1, a2);
    assert_ne!(a1, [0u8; 32]);

    // Different operation / namespace / expiry / epoch → different token.
    assert_ne!(
        a1,
        derive_namespace_token(&secret_a, &d32(2), &ns, expiry, 1)
    );
    assert_ne!(
        a1,
        derive_namespace_token(&secret_a, &op_id, &d32(11), expiry, 1)
    );
    assert_ne!(
        a1,
        derive_namespace_token(&secret_a, &op_id, &ns, expiry + 1, 1)
    );
    assert_ne!(
        a1,
        derive_namespace_token(&secret_a, &op_id, &ns, expiry, 2)
    );

    // The ring retains prior secrets across a rotation so old operations still
    // derive their original token, while the new epoch mints a different one.
    let mut ring = TokenSecretRing::new(1, secret_a);
    ring.rotate(2, secret_b);
    assert_eq!(ring.current_epoch(), 2);
    assert_eq!(ring.derive(1, &op_id, &ns, expiry), Some(a1));
    assert_ne!(ring.derive(2, &op_id, &ns, expiry), Some(a1));
    // After every epoch-1 operation expires, its secret can be retired.
    ring.retire_below(2);
    assert_eq!(ring.derive(1, &op_id, &ns, expiry), None);
}

// ---------------------------------------------------------------------------
// GetOperation lifecycle.
// ---------------------------------------------------------------------------

#[test]
fn get_operation_prepared_matches_prepare_payload() {
    let op = sk(1);
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let mut entropy = entropy_from(0xB0);
    let handling = svc
        .handle(
            &mut repo,
            &prepare_frame(d16(10), prepare_body(130, None)),
            9000,
            &mut entropy,
        )
        .unwrap();
    let success = expect_prepare_success(&handling);

    match get_operation(&svc, &mut repo, success.operation_id, 9100) {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::GetOperation(payload)),
            ..
        }) => {
            assert_eq!(payload.operation_id, success.operation_id);
            assert_eq!(payload.originating_prepare_kind, PrepareKind::PrepareHost);
            match payload.state {
                GetOperationState::Prepared {
                    operation_expiry,
                    prepare_success,
                } => {
                    assert_eq!(operation_expiry, success.operation_expiry);
                    assert_eq!(*prepare_success, success);
                }
                other => panic!("expected prepared, got {other:?}"),
            }
        }
        other => panic!("expected get_operation success, got {other:?}"),
    }
}

#[test]
fn get_operation_unknown_id_returns_not_found() {
    let op = sk(1);
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let refused = get_operation(&svc, &mut repo, d32(0xAB), 1000);
    assert!(matches!(
        expect_refusal(&refused),
        ControlRefusal::OperationNotFound { .. }
    ));
}

#[test]
fn get_operation_expired_returns_operation_expired_then_not_found() {
    let op = sk(1);
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let mut entropy = entropy_from(0xC0);
    let success = expect_prepare_success(
        &svc.handle(
            &mut repo,
            &prepare_frame(d16(11), prepare_body(140, None)),
            10_000,
            &mut entropy,
        )
        .unwrap(),
    );
    let expiry = success.operation_expiry; // 13_600

    // Past expiry but inside the retention window: operation_expired.
    let refused = get_operation(&svc, &mut repo, success.operation_id, expiry + 10);
    assert!(matches!(
        expect_refusal(&refused),
        ControlRefusal::OperationExpired { .. }
    ));

    // Past the 24h retention horizon: indistinguishable from unknown.
    let gone = get_operation(
        &svc,
        &mut repo,
        success.operation_id,
        expiry + 24 * 3600 + 1,
    );
    assert!(matches!(
        expect_refusal(&gone),
        ControlRefusal::OperationNotFound { .. }
    ));
}

#[test]
fn get_operation_terminal_returns_terminal_outcome() {
    let op = sk(1);
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let mut entropy = entropy_from(0xD0);
    let success = expect_prepare_success(
        &svc.handle(
            &mut repo,
            &prepare_frame(d16(12), prepare_body(150, None)),
            11_000,
            &mut entropy,
        )
        .unwrap(),
    );

    let terminal = TerminalOperationOutcome::Refused(ControlRefusal::PeerContextChanged {
        side: riot_anchor_protocol::control::PeerSide::Destination,
        prior_descriptor_digest: d32(1),
        latest_descriptor_digest: None,
        reason: riot_anchor_protocol::control::PeerContextReason::ProcessRestart,
    });
    svc.terminalize_operation(&mut repo, &success.operation_id, &terminal)
        .unwrap();

    match get_operation(&svc, &mut repo, success.operation_id, 11_100) {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::GetOperation(payload)),
            ..
        }) => match payload.state {
            GetOperationState::Terminal { outcome } => assert_eq!(outcome, terminal),
            other => panic!("expected terminal, got {other:?}"),
        },
        other => panic!("expected get_operation success, got {other:?}"),
    }
}

#[test]
fn prepared_operation_survives_restart() {
    let op = sk(1);
    let db = TempDb::new();
    let success;
    {
        let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
        let mut repo = AnchorRepository::open(db.path()).unwrap();
        let mut entropy = entropy_from(0xE0);
        success = expect_prepare_success(
            &svc.handle(
                &mut repo,
                &prepare_frame(d16(13), prepare_body(160, None)),
                12_000,
                &mut entropy,
            )
            .unwrap(),
        );
    }
    // Reopen from disk with a fresh service: GetOperation still sees Prepared and
    // the byte-identical prepared payload.
    let svc = service(&op, SpyPolicy::new(COMMUNITY_ROOT));
    let mut repo = AnchorRepository::open(db.path()).unwrap();
    match get_operation(&svc, &mut repo, success.operation_id, 12_010) {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::GetOperation(payload)),
            ..
        }) => match payload.state {
            GetOperationState::Prepared {
                prepare_success, ..
            } => {
                assert_eq!(*prepare_success, success);
            }
            other => panic!("expected prepared after restart, got {other:?}"),
        },
        other => panic!("expected get_operation success, got {other:?}"),
    }
}
