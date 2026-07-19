//! WU-015 composite `CommitHost` crash-safety and recovery.
//!
//! Every durable mutation in the composite promotion (and in the terminal-cleanup
//! and reusable dispositions) is guarded by a named failpoint. Because the whole
//! disposition is ONE `RepoTransaction`, tripping any failpoint drops the
//! transaction before commit, so the store is WHOLLY ABSENT of the mutation — and
//! a subsequent clean retry is WHOLLY COMMITTED. A lost delivery after a
//! successful commit reconstructs the byte-identical receipt through
//! `GetOperation` (the WU-014 lifecycle) and through the Commit key's exact replay.

mod hosting_common;

use hosting_common::*;

use riot_anchor::hosting::{no_failpoint, CommitError, CommitHostService};
use riot_anchor::repository::{AnchorRepository, OperationStatus};

use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::{CommitHostV1, ControlOutcome, ControlRefusal, ControlSuccess};

const NOW: u64 = 1_500;
const EXPIRY: u64 = 4_600;
const DEADLINE: u64 = 4_600;

struct Fixture {
    repo: AnchorRepository,
    operation_id: [u8; 32],
    namespaces: [[u8; 32]; 3],
}

fn fixture() -> Fixture {
    let mut repo = repo();
    let o = make_item("o");
    let c = make_item("c");
    let w = make_item("w");
    let namespaces = [o.namespace_id, c.namespace_id, w.namespace_id];
    let operation_id = d32(200);
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

fn body(repo: &AnchorRepository, fx: &Fixture) -> CommitHostV1 {
    CommitHostV1 {
        operation_id: fx.operation_id,
        ordered_namespace_snapshot_digests: declared_digests(repo, fx.operation_id, fx.namespaces),
    }
}

/// The store must be wholly absent of any promotion: no committed entries,
/// generation unset, staging intact, operation still prepared.
fn assert_wholly_absent(fx: &Fixture) {
    for namespace in fx.namespaces {
        assert_eq!(fx.repo.committed_entry_count(&namespace).unwrap(), 0);
        assert_eq!(
            fx.repo
                .staged_entries(&fx.operation_id, &namespace)
                .unwrap()
                .len(),
            1,
            "staging must survive a rolled-back commit"
        );
    }
    assert_eq!(fx.repo.site_generation(&fx.namespaces[0]).unwrap(), None);
    let operation = fx.repo.load_operation(&fx.operation_id).unwrap().unwrap();
    assert_eq!(operation.status, OperationStatus::Prepared);
}

// ---------------------------------------------------------------------------
// Success-path failpoints: every durable mutation is all-or-nothing.
// ---------------------------------------------------------------------------

fn run_success_failpoint(label: &'static str) {
    let mut fx = fixture();
    let body = body(&fx.repo, &fx);
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);
    let mut fp = move |seen: &str| seen == label;
    let result = service.commit(
        &mut fx.repo,
        &d16(1),
        &body,
        &d32(1),
        NOW,
        &mut entropy,
        &mut fp,
    );
    match result {
        Err(CommitError::Failpoint(hit)) => assert_eq!(hit, label),
        other => panic!("failpoint {label} should abort, got {other:?}"),
    }
    // Wholly absent, and the Commit key was NOT claimed (a clean retry still works).
    assert_wholly_absent(&fx);

    let mut entropy2 = || d32(0xEE);
    let response = service
        .commit(
            &mut fx.repo,
            &d16(1),
            &body,
            &d32(1),
            NOW,
            &mut entropy2,
            &mut no_failpoint,
        )
        .expect("clean retry commits");
    assert!(
        matches!(
            response.outcome,
            ControlOutcome::Success(ControlSuccess::CommitHost(_))
        ),
        "retry after rollback is wholly committed"
    );
    for namespace in fx.namespaces {
        assert_eq!(fx.repo.committed_entry_count(&namespace).unwrap(), 1);
    }
    assert_eq!(fx.repo.site_generation(&fx.namespaces[0]).unwrap(), Some(1));
}

#[test]
fn failpoint_at_generation_cas_is_all_or_nothing() {
    run_success_failpoint("cas");
}

#[test]
fn failpoint_at_promotion_is_all_or_nothing() {
    run_success_failpoint("promote");
}

#[test]
fn failpoint_at_receipt_is_all_or_nothing() {
    run_success_failpoint("receipt");
}

#[test]
fn failpoint_at_terminal_is_all_or_nothing() {
    run_success_failpoint("terminal");
}

#[test]
fn failpoint_before_commit_is_all_or_nothing() {
    run_success_failpoint("commit");
}

// ---------------------------------------------------------------------------
// Terminal-cleanup failpoints: a rolled-back cleanup leaves the operation intact.
// ---------------------------------------------------------------------------

fn run_cleanup_failpoint(label: &'static str) {
    let mut fx = fixture();
    // Force a terminal refusal path (snapshot mismatch on O).
    let mut digests = declared_digests(&fx.repo, fx.operation_id, fx.namespaces);
    digests[0] = d32(0xBB);
    let body = CommitHostV1 {
        operation_id: fx.operation_id,
        ordered_namespace_snapshot_digests: digests,
    };
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);
    let mut fp = move |seen: &str| seen == label;
    let result = service.commit(
        &mut fx.repo,
        &d16(2),
        &body,
        &d32(2),
        NOW,
        &mut entropy,
        &mut fp,
    );
    match result {
        Err(CommitError::Failpoint(hit)) => assert_eq!(hit, label),
        other => panic!("cleanup failpoint {label} should abort, got {other:?}"),
    }
    // The refusal never persisted: operation still prepared, staging intact.
    assert_wholly_absent(&fx);
}

#[test]
fn failpoint_at_cleanup_operation_is_all_or_nothing() {
    run_cleanup_failpoint("cleanup.operation");
}

#[test]
fn failpoint_at_cleanup_staging_is_all_or_nothing() {
    run_cleanup_failpoint("cleanup.staging");
}

#[test]
fn failpoint_at_cleanup_commit_is_all_or_nothing() {
    run_cleanup_failpoint("cleanup.commit");
}

// ---------------------------------------------------------------------------
// Reusable-disposition failpoints.
// ---------------------------------------------------------------------------

fn run_reusable_failpoint(label: &'static str) {
    let mut fx = fixture();
    let body = body(&fx.repo, &fx);
    let refusal = ControlRefusal::CommitBusy {
        limit_id: riot_anchor_protocol::records::AnchorLimitId::from_id(8).unwrap(),
        retry_after_seconds: 30,
    };
    let authority = TestAuthority::new(fx.namespaces).refuse_capacity(refusal);
    let service = CommitHostService::new(commit_context(), authority, signer());
    let mut entropy = || d32(0xEE);
    let mut fp = move |seen: &str| seen == label;
    let result = service.commit(
        &mut fx.repo,
        &d16(3),
        &body,
        &d32(3),
        NOW,
        &mut entropy,
        &mut fp,
    );
    match result {
        Err(CommitError::Failpoint(hit)) => assert_eq!(hit, label),
        other => panic!("reusable failpoint {label} should abort, got {other:?}"),
    }
    assert_wholly_absent(&fx);
}

#[test]
fn failpoint_at_reusable_write_is_all_or_nothing() {
    run_reusable_failpoint("reusable.write");
}

#[test]
fn failpoint_at_reusable_commit_is_all_or_nothing() {
    run_reusable_failpoint("reusable.commit");
}

// ---------------------------------------------------------------------------
// Authority-signalled terminal-cleanup rows (manifest transport / stale source).
// ---------------------------------------------------------------------------

#[test]
fn manifest_transport_mismatch_is_terminal_cleanup() {
    let mut fx = fixture();
    let body = body(&fx.repo, &fx);
    let authority = TestAuthority::new(fx.namespaces).refuse_manifest(
        ControlRefusal::ManifestTransportMismatch {
            expected_digest: d32(0x40),
            observed_digest: d32(0x41),
        },
    );
    let service = CommitHostService::new(commit_context(), authority, signer());
    let mut entropy = || d32(0xEE);
    let response = service
        .commit(
            &mut fx.repo,
            &d16(4),
            &body,
            &d32(4),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("resolves");
    assert!(matches!(
        response.outcome,
        ControlOutcome::Refused(ControlRefusal::ManifestTransportMismatch { .. })
    ));
    let operation = fx.repo.load_operation(&fx.operation_id).unwrap().unwrap();
    assert_eq!(operation.status, OperationStatus::Refused);
    for namespace in fx.namespaces {
        assert!(fx
            .repo
            .staged_entries(&fx.operation_id, &namespace)
            .unwrap()
            .is_empty());
    }
}

#[test]
fn stale_source_from_authority_is_terminal_cleanup() {
    let mut fx = fixture();
    let body = body(&fx.repo, &fx);
    let authority =
        TestAuthority::new(fx.namespaces).refuse_manifest(ControlRefusal::StaleSource {
            attested_generation: 5,
            observed_generation: 6,
            ordered_observed_namespace_snapshot_digests: [d32(1), d32(2), d32(3)],
        });
    let service = CommitHostService::new(commit_context(), authority, signer());
    let mut entropy = || d32(0xEE);
    let response = service
        .commit(
            &mut fx.repo,
            &d16(5),
            &body,
            &d32(5),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("resolves");
    assert!(matches!(
        response.outcome,
        ControlOutcome::Refused(ControlRefusal::StaleSource { .. })
    ));
    let operation = fx.repo.load_operation(&fx.operation_id).unwrap().unwrap();
    assert_eq!(operation.status, OperationStatus::Refused);
}

// ---------------------------------------------------------------------------
// Lost-delivery reconstruction: byte-identical receipt through GetOperation and
// through the Commit key's exact replay, across a real restart.
// ---------------------------------------------------------------------------

#[test]
fn lost_receipt_reconstructs_byte_identically_across_restart() {
    let temp = TempDb::new();
    let namespaces;
    let operation_id = d32(200);
    let committed_receipt_bytes;
    let commit_response_bytes;
    {
        let mut repo = temp.open();
        let o = make_item("o");
        let c = make_item("c");
        let w = make_item("w");
        namespaces = [o.namespace_id, c.namespace_id, w.namespace_id];
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
        let body = CommitHostV1 {
            operation_id,
            ordered_namespace_snapshot_digests: declared_digests(&repo, operation_id, namespaces),
        };
        let service =
            CommitHostService::new(commit_context(), TestAuthority::new(namespaces), signer());
        let mut entropy = || d32(0xEE);
        // The Commit "delivery" is lost: capture the receipt bytes but pretend the
        // client never received the response.
        let response = service
            .commit(
                &mut repo,
                &d16(9),
                &body,
                &d32(9),
                NOW,
                &mut entropy,
                &mut no_failpoint,
            )
            .expect("commit succeeds");
        commit_response_bytes = response.encode_canonical().unwrap();
        committed_receipt_bytes = match response.outcome {
            ControlOutcome::Success(ControlSuccess::CommitHost(receipt)) => {
                receipt.encode_canonical().unwrap()
            }
            other => panic!("expected receipt, got {other:?}"),
        };
    }

    // Restart: reopen the durable database.
    let mut repo = temp.open();

    // (1) GetOperation reconstructs the byte-identical committed receipt.
    let control = control_service();
    let recovered = get_operation_receipt(&control, &mut repo, operation_id, NOW + 100);
    assert_eq!(
        recovered.encode_canonical().unwrap(),
        committed_receipt_bytes,
        "GetOperation reconstructs the byte-identical receipt after restart"
    );

    // (2) The exact Commit key replays the byte-identical Commit response.
    let body = CommitHostV1 {
        operation_id,
        ordered_namespace_snapshot_digests: declared_digests(&repo, operation_id, namespaces),
    };
    let service =
        CommitHostService::new(commit_context(), TestAuthority::new(namespaces), signer());
    let mut entropy = || d32(0xEE);
    let replay = service
        .commit(
            &mut repo,
            &d16(9),
            &body,
            &d32(9),
            NOW + 100,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("replay resolves");
    assert_eq!(
        replay.encode_canonical().unwrap(),
        commit_response_bytes,
        "same Commit key replays the byte-identical response after restart"
    );
}
