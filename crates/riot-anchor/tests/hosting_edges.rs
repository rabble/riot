//! WU-015 composite `CommitHost`: the error surface and the terminalised-operation
//! replay edges that the promotion / refusal / crash-safety suites
//! (`hosting_commit.rs`, `hosting_failpoints.rs`) leave untouched.
//!
//! These pin:
//!
//! * `CommitError` `Display` (every arm) + `From` conversions — the fault surface a
//!   caller sees when a store, codec, malformed-plan, or failpoint fault escapes;
//! * a NOVEL Commit key against an already-terminalised operation replaying the
//!   operation's committed receipt / refused outcome without a fresh mutation
//!   (`committed_response_from_operation` / `refused_response_from_operation`);
//! * a stored prepared response that does not project to a host plan surfacing as
//!   `MalformedPlan`.

mod hosting_common;

use hosting_common::*;

use riot_anchor::hosting::{no_failpoint, CommitError, CommitHostService};
use riot_anchor::repository::{
    AnchorRepository, AnchorRepositoryError, NewPreparedOperation, OperationKind, OperationStatus,
};

use riot_anchor_protocol::codec::{CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{
    CommitHostV1, ControlOutcome, ControlRefusal, ControlResponseV1, ControlSuccess,
};
use riot_anchor_protocol::records::ControlOperationKind;

const NOW: u64 = 1_500;
const EXPIRY: u64 = 4_600;
const DEADLINE: u64 = 4_600;

struct Fixture {
    repo: AnchorRepository,
    operation_id: [u8; 32],
    namespaces: [[u8; 32]; 3],
}

fn base_fixture(operation_id: [u8; 32]) -> Fixture {
    let mut repo = repo();
    let o = make_item("o-entry");
    let c = make_item("c-entry");
    let w = make_item("w-entry");
    let namespaces = [o.namespace_id, c.namespace_id, w.namespace_id];
    insert_prepared_operation(
        &mut repo,
        operation_id,
        namespaces,
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
    );
    stage_entries(&mut repo, operation_id, vec![o.staged.clone()], DEADLINE);
    stage_entries(&mut repo, operation_id, vec![c.staged.clone()], DEADLINE);
    stage_entries(&mut repo, operation_id, vec![w.staged.clone()], DEADLINE);
    Fixture {
        repo,
        operation_id,
        namespaces,
    }
}

fn commit_body(repo: &AnchorRepository, fx: &Fixture) -> CommitHostV1 {
    CommitHostV1 {
        operation_id: fx.operation_id,
        ordered_namespace_snapshot_digests: declared_digests(repo, fx.operation_id, fx.namespaces),
    }
}

// ---------------------------------------------------------------------------
// CommitError surface: Display (store + codec arms) + From conversions.
// ---------------------------------------------------------------------------

#[test]
fn commit_error_from_and_display_for_store_and_codec() {
    // From<AnchorRepositoryError> -> Repository variant; Display names the store.
    let repo_error: CommitError = AnchorRepositoryError::RemovalSlotsExhausted.into();
    let shown = format!("{repo_error}");
    assert!(
        shown.contains("repository"),
        "repository error Display should mention the repository, got {shown:?}"
    );
    let _as_error: &dyn std::error::Error = &repo_error;

    // From<CodecError> -> Codec variant; Display names the codec.
    let codec_error: CommitError = CodecError::NonCanonical.into();
    let shown = format!("{codec_error}");
    assert!(
        shown.contains("codec"),
        "codec error Display should mention the codec, got {shown:?}"
    );
}

// ---------------------------------------------------------------------------
// Novel Commit key against an already-terminalised operation.
// ---------------------------------------------------------------------------

#[test]
fn novel_commit_key_against_committed_operation_replays_receipt() {
    let mut fx = base_fixture(d32(200));
    let body = commit_body(&fx.repo, &fx);
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);

    // First Commit under key A promotes the site and terminalises the operation.
    let first = service
        .commit(
            &mut fx.repo,
            &d16(1),
            &body,
            &d32(1),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("first commit");
    let first_receipt = match &first.outcome {
        ControlOutcome::Success(ControlSuccess::CommitHost(receipt)) => receipt.clone(),
        other => panic!("expected receipt, got {other:?}"),
    };

    // A NOVEL Commit key B for the same, now-committed operation replays the
    // operation's terminal receipt without re-promoting anything.
    let replay = service
        .commit(
            &mut fx.repo,
            &d16(2),
            &body,
            &d32(2),
            NOW + 5,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("novel key against committed operation resolves");
    let replay_receipt = match &replay.outcome {
        ControlOutcome::Success(ControlSuccess::CommitHost(receipt)) => receipt.clone(),
        other => panic!("expected replayed receipt, got {other:?}"),
    };
    assert_eq!(
        replay_receipt.encode_canonical().unwrap(),
        first_receipt.encode_canonical().unwrap(),
        "novel key replays the byte-identical committed receipt"
    );
    // No fresh mutation: generation stayed 1, operation still Committed.
    assert_eq!(fx.repo.site_generation(&fx.namespaces[0]).unwrap(), Some(1));
    assert_eq!(
        fx.repo
            .load_operation(&fx.operation_id)
            .unwrap()
            .unwrap()
            .status,
        OperationStatus::Committed
    );
}

#[test]
fn novel_commit_key_against_refused_operation_replays_refusal() {
    let mut fx = base_fixture(d32(201));
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);

    // Force a terminal-cleanup refusal under key A (declare a wrong O digest).
    let mut digests = declared_digests(&fx.repo, fx.operation_id, fx.namespaces);
    digests[0] = d32(0xBB);
    let bad_body = CommitHostV1 {
        operation_id: fx.operation_id,
        ordered_namespace_snapshot_digests: digests,
    };
    let refused = service
        .commit(
            &mut fx.repo,
            &d16(3),
            &bad_body,
            &d32(3),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("first commit refuses");
    let first_refusal = match &refused.outcome {
        ControlOutcome::Refused(refusal) => refusal.clone(),
        other => panic!("expected refusal, got {other:?}"),
    };
    assert!(matches!(
        first_refusal,
        ControlRefusal::SnapshotMismatch { .. }
    ));

    // A NOVEL Commit key B for the same, now-refused operation replays the
    // operation's terminal refusal (its body is irrelevant; the operation is
    // already terminal). No fresh mutation.
    let replay = service
        .commit(
            &mut fx.repo,
            &d16(4),
            &bad_body,
            &d32(4),
            NOW + 5,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("novel key against refused operation resolves");
    match &replay.outcome {
        ControlOutcome::Refused(refusal) => assert_eq!(
            refusal.encode_canonical().unwrap(),
            first_refusal.encode_canonical().unwrap(),
            "novel key replays the byte-identical refused outcome"
        ),
        other => panic!("expected replayed refusal, got {other:?}"),
    }
    assert_eq!(
        fx.repo
            .load_operation(&fx.operation_id)
            .unwrap()
            .unwrap()
            .status,
        OperationStatus::Refused
    );
}

// ---------------------------------------------------------------------------
// A stored prepared response that does not project to a host plan.
// ---------------------------------------------------------------------------

#[test]
fn stored_prepared_response_without_a_host_plan_is_malformed_plan() {
    let mut repo = repo();
    let operation_id = d32(210);
    // Store a canonical control RESPONSE that is not a Prepare success (a refusal),
    // so the operation's captured plan cannot be projected.
    let not_a_plan = ControlResponseV1 {
        kind: ControlOperationKind::PrepareHost,
        outcome: ControlOutcome::Refused(ControlRefusal::IdempotencyConflict),
    }
    .encode_canonical()
    .unwrap();
    let mut tx = repo.begin().unwrap();
    tx.insert_operation(&NewPreparedOperation {
        operation_id,
        originating_kind: OperationKind::Host,
        token_secret_epoch: 0,
        base_generation: 0,
        created_at: 1_000,
        operation_expiry: EXPIRY,
        retention_deadline: EXPIRY + 24 * 60 * 60,
        prepare_response_bytes: not_a_plan,
    })
    .unwrap();
    tx.commit().unwrap();

    let body = CommitHostV1 {
        operation_id,
        ordered_namespace_snapshot_digests: [d32(0); 3],
    };
    let service =
        CommitHostService::new(commit_context(), TestAuthority::new([d32(0); 3]), signer());
    let mut entropy = || d32(0xEE);
    let result = service.commit(
        &mut repo,
        &d16(5),
        &body,
        &d32(5),
        NOW,
        &mut entropy,
        &mut no_failpoint,
    );
    match result {
        Err(err @ CommitError::MalformedPlan) => {
            let shown = format!("{err}");
            assert!(
                shown.contains("not a host plan"),
                "MalformedPlan Display should explain the plan is unusable, got {shown:?}"
            );
        }
        other => panic!("expected MalformedPlan, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// CommitError::Failpoint Display.
// ---------------------------------------------------------------------------

#[test]
fn commit_failpoint_error_displays_its_label() {
    let mut fx = base_fixture(d32(220));
    let body = commit_body(&fx.repo, &fx);
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);
    let mut fp = |seen: &str| seen == "cas";
    let result = service.commit(
        &mut fx.repo,
        &d16(6),
        &body,
        &d32(6),
        NOW,
        &mut entropy,
        &mut fp,
    );
    match result {
        Err(err @ CommitError::Failpoint("cas")) => {
            let shown = format!("{err}");
            assert!(
                shown.contains("failpoint") && shown.contains("cas"),
                "Failpoint Display should name the tripped label, got {shown:?}"
            );
        }
        other => panic!("expected a cas failpoint abort, got {other:?}"),
    }
}
