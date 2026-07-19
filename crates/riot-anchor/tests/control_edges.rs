//! WU-014 control admission: the refusal / error / lifecycle edges the happy-path
//! matrix (`control_prepare.rs`) leaves untouched.
//!
//! These pin the seldom-driven arms of `control.rs`:
//!
//! * `ControlError` `Display` + `From` conversions — the error surface a caller
//!   sees when a durable store or codec fault escapes;
//! * `token_ring_mut` — secret-epoch rotation between operations, observed through
//!   the epoch a subsequent prepare stamps into its stored operation;
//! * the `Unsupported` protocol failure for an operation this build does not serve;
//! * `replay_prepare`'s terminal arm — a terminalised prepare replays its terminal
//!   refusal rather than a fresh prepare;
//! * `GetOperation` on a replica-originated operation and on a terminally-committed
//!   operation;
//! * `stored_prepare_success`'s closed refusal when the stored prepared bytes are
//!   not a prepare success.

use ed25519_dalek::{Signer, SigningKey};

use riot_anchor::control::{
    AdmissionPolicy, AnchorControlContext, AnchorControlService, ControlError, ControlHandling,
    PreparePlan, ProtocolFailure,
};
use riot_anchor::repository::{AnchorRepository, NewPreparedOperation, OperationKind};
use riot_anchor::work::{OperatorSigner, PressurePolicy, TokenSecretRing};

use riot_anchor_protocol::codec::{CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{
    CommitHostV1, ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1,
    ControlResponseV1, ControlSuccess, EffectiveOperationLimits, GetOperationState, GetOperationV1,
    PrepareHostV1, PrepareKind, PrepareSuccessV1, TerminalOperationOutcome,
};
use riot_anchor_protocol::digest::anchor_id as compute_anchor_id;
use riot_anchor_protocol::records::{
    AnchorDescriptorBodyV1, AnchorLimitProfileV1, ControlOperationKind, DescriptorEnvelopeV1,
    EnabledRole, HostingReceiptBodyV1, HostingReceiptV1, HostingStatus, NamespaceResult,
    OperatorSignedEnvelopeV1, OperatorVerificationKeyV1, PublicSiteTicketV2Core,
    RootSignedTicketCoreEnvelopeV2, TransportFloor,
};

use riot_anchor::repository::AnchorRepositoryError;

// ---------------------------------------------------------------------------
// Fixtures (a self-contained subset of control_prepare.rs).
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

struct TestSigner(SigningKey);
impl OperatorSigner for TestSigner {
    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        self.0.sign(preimage).to_bytes()
    }
}

/// An admission policy that always authorises with a fixed plan and never requires
/// work — the edges under test are downstream of admission.
struct PassPolicy {
    plan: PreparePlan,
}
impl PassPolicy {
    fn new() -> Self {
        PassPolicy {
            plan: PreparePlan {
                community_root: d32(70),
                ordered_namespace_host_plan: [d32(10), d32(11), d32(12)],
                ordered_retained_snapshot_digests: [d32(20), d32(21), d32(22)],
                base_generation: 7,
            },
        }
    }
}
impl AdmissionPolicy for PassPolicy {
    fn authorize_prepare_host(
        &self,
        _request: &PrepareHostV1,
        _observed_at: u64,
    ) -> Result<PreparePlan, ControlRefusal> {
        Ok(self.plan)
    }
    fn capacity_for_prepare_host(
        &self,
        _plan: &PreparePlan,
        _observed_at: u64,
    ) -> Result<(), ControlRefusal> {
        Ok(())
    }
    fn pressure_band(&self, _community_root: &[u8; 32], _observed_at: u64) -> PressurePolicy {
        PressurePolicy {
            policy_epoch: 0,
            difficulty: 0,
        }
    }
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

fn service(operator: &SigningKey) -> AnchorControlService<PassPolicy, TestSigner> {
    AnchorControlService::new(
        context(operator),
        PassPolicy::new(),
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

fn prepare_body(snapshot_seed: u8) -> PrepareHostV1 {
    PrepareHostV1 {
        root_signed_ticket_core: ticket_core(&sk(9)),
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
    svc: &AnchorControlService<PassPolicy, TestSigner>,
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

/// Insert an operation row directly, so tests can craft `originating_kind` and the
/// stored prepared bytes the service would never itself produce.
fn insert_operation(
    repo: &mut AnchorRepository,
    operation_id: [u8; 32],
    kind: OperationKind,
    prepare_response_bytes: Vec<u8>,
    operation_expiry: u64,
) {
    let mut tx = repo.begin().expect("begin");
    tx.insert_operation(&NewPreparedOperation {
        operation_id,
        originating_kind: kind,
        token_secret_epoch: 1,
        base_generation: 0,
        created_at: 1000,
        operation_expiry,
        retention_deadline: operation_expiry + 24 * 60 * 60,
        prepare_response_bytes,
    })
    .expect("insert operation");
    tx.commit().expect("commit operation");
}

/// A canonical PrepareHost success response, as `insert_operation` fodder.
fn prepare_success_bytes(operation_id: [u8; 32], operation_expiry: u64) -> Vec<u8> {
    let profile = AnchorLimitProfileV1::mvp_defaults(0);
    let success = PrepareSuccessV1 {
        operation_id,
        base_site_generation: 0,
        ordered_namespace_host_plan: [d32(10), d32(11), d32(12)],
        ordered_namespace_tokens: [d32(0); 3],
        ordered_retained_snapshot_digests: [d32(20), d32(21), d32(22)],
        sync_version: 2,
        effective_operation_limits: EffectiveOperationLimits::from_profile(&profile),
        operation_expiry,
    };
    ControlResponseV1 {
        kind: ControlOperationKind::PrepareHost,
        outcome: ControlOutcome::Success(ControlSuccess::PrepareHost(Box::new(success))),
    }
    .encode_canonical()
    .expect("encode prepare success")
}

fn dummy_receipt(operation_id: [u8; 32]) -> HostingReceiptV1 {
    OperatorSignedEnvelopeV1 {
        body: HostingReceiptBodyV1 {
            anchor_id: d32(1),
            operator_key_id: d32(2),
            descriptor_epoch: 0,
            descriptor_digest: d32(3),
            hosting_operation_id: operation_id,
            full_site_root: d32(4),
            manifest_digest: d32(5),
            manifest_version: 1,
            base_site_generation: 7,
            committed_site_generation: 8,
            ordered_namespace_results: vec![
                NamespaceResult {
                    namespace_id: d32(10),
                    snapshot_digest: d32(20),
                    entry_count: 1,
                },
                NamespaceResult {
                    namespace_id: d32(11),
                    snapshot_digest: d32(21),
                    entry_count: 1,
                },
                NamespaceResult {
                    namespace_id: d32(12),
                    snapshot_digest: d32(22),
                    entry_count: 1,
                },
            ],
            status: HostingStatus::Committed,
            accepted_at: 100,
            reported_retention_through: 200,
            limit_profile_digest: d32(50),
        },
        operator_signature: [0u8; 64],
    }
}

// ---------------------------------------------------------------------------
// ControlError surface: Display + From conversions.
// ---------------------------------------------------------------------------

#[test]
fn control_error_display_and_from_conversions() {
    // From<AnchorRepositoryError> yields a Repository variant; its Display names the
    // repository fault. (A durable store error escaping the admission service.)
    let repo_error: ControlError = AnchorRepositoryError::RemovalSlotsExhausted.into();
    let shown = format!("{repo_error}");
    assert!(
        shown.contains("repository"),
        "repository error Display should mention the repository, got {shown:?}"
    );
    // It is a real std::error::Error.
    let _as_error: &dyn std::error::Error = &repo_error;

    // From<CodecError> yields a Codec variant; its Display names the codec fault.
    let codec_error: ControlError = CodecError::NonCanonical.into();
    let shown = format!("{codec_error}");
    assert!(
        shown.contains("codec"),
        "codec error Display should mention the codec, got {shown:?}"
    );
}

// ---------------------------------------------------------------------------
// token_ring_mut: secret-epoch rotation between operations.
// ---------------------------------------------------------------------------

#[test]
fn token_ring_mut_rotates_epoch_used_by_next_prepare() {
    let op = sk(1);
    let mut svc = service(&op);
    let mut repo = AnchorRepository::open_in_memory().unwrap();

    // A prepare before rotation stamps the initial epoch (1) onto its operation.
    let mut entropy = entropy_from(0x40);
    let before = expect_prepare_success(
        &svc.handle(
            &mut repo,
            &prepare_frame(d16(1), prepare_body(30)),
            1000,
            &mut entropy,
        )
        .unwrap(),
    );
    assert_eq!(
        repo.load_operation(&before.operation_id)
            .unwrap()
            .unwrap()
            .token_secret_epoch,
        1
    );

    // Rotate the ring through the mutable accessor; the next prepare must adopt it.
    svc.token_ring_mut().rotate(2, d32(201));
    assert_eq!(svc.token_ring_mut().current_epoch(), 2);

    let mut entropy = entropy_from(0x60);
    let after = expect_prepare_success(
        &svc.handle(
            &mut repo,
            &prepare_frame(d16(2), prepare_body(40)),
            1001,
            &mut entropy,
        )
        .unwrap(),
    );
    assert_eq!(
        repo.load_operation(&after.operation_id)
            .unwrap()
            .unwrap()
            .token_secret_epoch,
        2,
        "the prepare after rotation stamps the rotated secret epoch"
    );
}

// ---------------------------------------------------------------------------
// Unsupported operation: a bounded protocol failure, no admission, no claim.
// ---------------------------------------------------------------------------

#[test]
fn unsupported_operation_is_protocol_failure() {
    let op = sk(1);
    let svc = service(&op);
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    // CommitHost is a valid, canonical control operation this control service does
    // not itself serve (it is handled by the Commit host service).
    let frame = ControlRequestV1 {
        idempotency_key: d16(3),
        operation: ControlOperation::CommitHost(CommitHostV1 {
            operation_id: d32(0x55),
            ordered_namespace_snapshot_digests: [d32(1), d32(2), d32(3)],
        }),
    }
    .encode_canonical()
    .unwrap();
    let mut entropy = entropy_from(0);
    assert_eq!(
        svc.handle(&mut repo, &frame, 1000, &mut entropy).unwrap(),
        ControlHandling::ProtocolFailure(ProtocolFailure::Unsupported)
    );
}

// ---------------------------------------------------------------------------
// replay_prepare terminal arm: a terminalised prepare replays its terminal refusal.
// ---------------------------------------------------------------------------

#[test]
fn terminalized_prepare_replays_terminal_refusal_not_a_fresh_prepare() {
    let op = sk(1);
    let svc = service(&op);
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let key = d16(4);
    let frame = prepare_frame(key, prepare_body(50));

    // Prepare succeeds and claims the key as Prepared.
    let mut entropy = entropy_from(0x50);
    let success =
        expect_prepare_success(&svc.handle(&mut repo, &frame, 1000, &mut entropy).unwrap());

    // Terminalise the operation with a refusal (session close / security exception).
    // This flips the operation and its idempotency mapping to Terminal.
    let terminal = TerminalOperationOutcome::Refused(ControlRefusal::InvalidTicketAuthority);
    svc.terminalize_operation(&mut repo, &success.operation_id, &terminal)
        .unwrap();

    // Replaying the SAME key + body now replays the terminal refusal (not a new
    // prepare, no fresh operation id).
    let mut entropy = entropy_from(0x99);
    let replay = svc.handle(&mut repo, &frame, 1010, &mut entropy).unwrap();
    assert_eq!(
        expect_refusal(&replay),
        ControlRefusal::InvalidTicketAuthority,
        "a terminalised prepare replays its exact terminal refusal"
    );
}

// ---------------------------------------------------------------------------
// GetOperation: replica-originated kind, and a committed terminal outcome.
// ---------------------------------------------------------------------------

#[test]
fn get_operation_reports_replica_originating_kind() {
    let op = sk(1);
    let svc = service(&op);
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let operation_id = d32(0x71);
    let expiry = 5000;
    // A replica-originated operation (the control service only mints Host, so insert
    // it directly) is reported with a PrepareReplica originating kind.
    insert_operation(
        &mut repo,
        operation_id,
        OperationKind::Replica,
        prepare_success_bytes(operation_id, expiry),
        expiry,
    );

    match get_operation(&svc, &mut repo, operation_id, 1000) {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::GetOperation(payload)),
            ..
        }) => {
            assert_eq!(
                payload.originating_prepare_kind,
                PrepareKind::PrepareReplica
            );
            assert!(matches!(payload.state, GetOperationState::Prepared { .. }));
        }
        other => panic!("expected get_operation success, got {other:?}"),
    }
}

#[test]
fn terminalize_committed_is_reconstructed_by_get_operation() {
    let op = sk(1);
    let svc = service(&op);
    let mut repo = AnchorRepository::open_in_memory().unwrap();

    // Prepare, then terminalise the operation with a COMMITTED receipt outcome.
    let mut entropy = entropy_from(0xA0);
    let success = expect_prepare_success(
        &svc.handle(
            &mut repo,
            &prepare_frame(d16(5), prepare_body(60)),
            2000,
            &mut entropy,
        )
        .unwrap(),
    );
    let receipt = dummy_receipt(success.operation_id);
    let terminal = TerminalOperationOutcome::Committed(Box::new(receipt.clone()));
    svc.terminalize_operation(&mut repo, &success.operation_id, &terminal)
        .unwrap();

    // GetOperation returns the committed terminal outcome with the byte-identical
    // receipt.
    match get_operation(&svc, &mut repo, success.operation_id, 2100) {
        ControlHandling::Responded(ControlResponseV1 {
            outcome: ControlOutcome::Success(ControlSuccess::GetOperation(payload)),
            ..
        }) => match payload.state {
            GetOperationState::Terminal {
                outcome: TerminalOperationOutcome::Committed(got),
            } => assert_eq!(
                got.encode_canonical().unwrap(),
                receipt.encode_canonical().unwrap()
            ),
            other => panic!("expected committed terminal, got {other:?}"),
        },
        other => panic!("expected get_operation success, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// stored_prepare_success: closed refusal when stored bytes are not a prepare
// success (defense in depth against a corrupt operation row).
// ---------------------------------------------------------------------------

#[test]
fn get_operation_on_non_prepare_stored_bytes_is_control_error() {
    let op = sk(1);
    let svc = service(&op);
    let mut repo = AnchorRepository::open_in_memory().unwrap();
    let operation_id = d32(0x72);
    let expiry = 5000;
    // Store a canonical control RESPONSE that is not a prepare success (a refusal),
    // so the Prepared-state projection cannot extract a PrepareSuccessV1.
    let not_a_prepare = ControlResponseV1 {
        kind: ControlOperationKind::PrepareHost,
        outcome: ControlOutcome::Refused(ControlRefusal::IdempotencyConflict),
    }
    .encode_canonical()
    .unwrap();
    insert_operation(
        &mut repo,
        operation_id,
        OperationKind::Host,
        not_a_prepare,
        expiry,
    );

    let frame = ControlRequestV1 {
        idempotency_key: d16(0xEE),
        operation: ControlOperation::GetOperation(GetOperationV1 { operation_id }),
    }
    .encode_canonical()
    .unwrap();
    let mut entropy = entropy_from(0);
    let result = svc.handle(&mut repo, &frame, 1000, &mut entropy);
    // The corrupt row surfaces as a ControlError, never a panic or a wrong success.
    match result {
        Err(err) => {
            let shown = format!("{err}");
            assert!(
                shown.contains("codec"),
                "expected codec error, got {shown:?}"
            );
        }
        Ok(other) => panic!("expected a control error, got {other:?}"),
    }
}
