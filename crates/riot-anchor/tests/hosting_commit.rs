//! WU-015 composite `CommitHost`: the happy path, every Commit refusal row with
//! its exact reusable-vs-terminal cleanup disposition, the same-base race (one
//! winner via generation CAS), and the security proof that a forged/unauthorised
//! staged entry is refused — the anchor never promotes bytes it has not itself
//! verified with riot-core's real Meadowcap check.

mod hosting_common;

use hosting_common::*;

use riot_anchor::hosting::{no_failpoint, CommitHostService};
use riot_anchor::repository::OperationStatus;
use riot_anchor::sync_service::AnchorSyncRepository;
use riot_anchor::work::TokenSecretRing;

use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::{
    CommitHostV1, ControlOutcome, ControlRefusal, ControlSuccess, RetryScope,
};
use riot_anchor_protocol::sync2::{
    OpenNamespace, PhaseParty, Sync2DirectionStage, Sync2Mode, Sync2Repository,
};

use std::cell::RefCell;
use std::rc::Rc;

const NOW: u64 = 1_500;
const EXPIRY: u64 = 4_600;
const DEADLINE: u64 = 4_600;

fn key(seed: u8) -> [u8; 16] {
    d16(seed)
}

fn digest(seed: u8) -> [u8; 32] {
    d32(seed)
}

/// Stage a genuine O/C/W site under one prepared operation and return the plan.
struct Fixture {
    repo: riot_anchor::repository::AnchorRepository,
    operation_id: [u8; 32],
    namespaces: [[u8; 32]; 3],
}

fn base_fixture(base_generation: u64) -> Fixture {
    let mut repo = repo();
    let o = make_item("o-entry");
    let c = make_item("c-entry");
    let w = make_item("w-entry");
    let namespaces = [o.namespace_id, c.namespace_id, w.namespace_id];
    let operation_id = d32(200);
    insert_prepared_operation(
        &mut repo,
        operation_id,
        namespaces,
        [d32(0); 3],
        base_generation,
        1_000,
        EXPIRY,
        0,
    );
    stage_entries(&mut repo, operation_id, vec![o.staged.clone()], DEADLINE);
    stage_entries(&mut repo, operation_id, vec![c.staged.clone()], DEADLINE);
    stage_entries(&mut repo, operation_id, vec![w.staged.clone()], DEADLINE);
    let _ = (o, c, w);
    Fixture {
        repo,
        operation_id,
        namespaces,
    }
}

fn commit_body(repo: &riot_anchor::repository::AnchorRepository, fx: &Fixture) -> CommitHostV1 {
    CommitHostV1 {
        operation_id: fx.operation_id,
        ordered_namespace_snapshot_digests: declared_digests(repo, fx.operation_id, fx.namespaces),
    }
}

// ---------------------------------------------------------------------------
// Happy path: the whole composite promotion is one atomic transaction.
// ---------------------------------------------------------------------------

#[test]
fn commit_promotes_composite_site_atomically() {
    let mut fx = base_fixture(0);
    let body = commit_body(&fx.repo, &fx);
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);

    let response = service
        .commit(
            &mut fx.repo,
            &key(1),
            &body,
            &digest(1),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit succeeds");

    // A signed hosting receipt binding base/committed generations.
    let receipt = match &response.outcome {
        ControlOutcome::Success(ControlSuccess::CommitHost(receipt)) => receipt.clone(),
        other => panic!("expected CommitHost success, got {other:?}"),
    };
    assert_eq!(receipt.body.base_site_generation, 0);
    assert_eq!(receipt.body.committed_site_generation, 1);
    assert_eq!(receipt.body.ordered_namespace_results.len(), 3);

    // State promoted: one committed entry per namespace, generation advanced.
    for namespace in fx.namespaces {
        assert_eq!(fx.repo.committed_entry_count(&namespace).unwrap(), 1);
    }
    assert_eq!(fx.repo.site_generation(&fx.namespaces[0]).unwrap(), Some(1));

    // Staging fully drained (nothing left private).
    for namespace in fx.namespaces {
        assert!(fx
            .repo
            .staged_entries(&fx.operation_id, &namespace)
            .unwrap()
            .is_empty());
    }

    // Operation is terminally committed with the byte-identical receipt.
    let operation = fx
        .repo
        .load_operation(&fx.operation_id)
        .unwrap()
        .expect("operation present");
    assert_eq!(operation.status, OperationStatus::Committed);
    assert_eq!(
        operation.terminal_result_bytes.unwrap(),
        receipt.encode_canonical().unwrap()
    );
}

// ---------------------------------------------------------------------------
// Security: a forged staged entry is refused, never promoted.
// ---------------------------------------------------------------------------

#[test]
fn commit_refuses_forged_staged_entry_with_invalid_operation_authority() {
    let mut repo = repo();
    let o = make_item("o-entry");
    let c = make_item("c-entry");
    let w = make_item("w-entry");
    let namespaces = [o.namespace_id, c.namespace_id, w.namespace_id];
    let operation_id = d32(201);
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
    // O carries a FORGED item (genuine metadata, corrupted signature) staged raw,
    // as if a compromised or buggy stage admitted it.
    stage_entries(&mut repo, operation_id, vec![forged_staged(&o)], DEADLINE);
    stage_entries(&mut repo, operation_id, vec![c.staged.clone()], DEADLINE);
    stage_entries(&mut repo, operation_id, vec![w.staged.clone()], DEADLINE);

    let mut ordered = [[0u8; 32]; 3];
    ordered.copy_from_slice(&declared_digests(&repo, operation_id, namespaces));
    let body = CommitHostV1 {
        operation_id,
        ordered_namespace_snapshot_digests: ordered,
    };
    let service =
        CommitHostService::new(commit_context(), TestAuthority::new(namespaces), signer());
    let mut entropy = || d32(0xEE);

    let response = service
        .commit(
            &mut repo,
            &key(2),
            &body,
            &digest(2),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");

    assert!(
        matches!(
            response.outcome,
            ControlOutcome::Refused(ControlRefusal::InvalidOperationAuthority)
        ),
        "forged entry must refuse invalid_operation_authority, got {:?}",
        response.outcome
    );
    // Terminal cleanup: nothing promoted, staging deleted, operation refused.
    for namespace in namespaces {
        assert_eq!(repo.committed_entry_count(&namespace).unwrap(), 0);
        assert!(repo
            .staged_entries(&operation_id, &namespace)
            .unwrap()
            .is_empty());
    }
    assert_eq!(repo.site_generation(&namespaces[0]).unwrap(), None);
    let operation = repo.load_operation(&operation_id).unwrap().unwrap();
    assert_eq!(operation.status, OperationStatus::Refused);
}

// ---------------------------------------------------------------------------
// Terminal-cleanup rows.
// ---------------------------------------------------------------------------

fn assert_terminal_cleanup(
    repo: &riot_anchor::repository::AnchorRepository,
    operation_id: [u8; 32],
    namespaces: [[u8; 32]; 3],
) {
    for namespace in namespaces {
        assert_eq!(repo.committed_entry_count(&namespace).unwrap(), 0);
        assert!(repo
            .staged_entries(&operation_id, &namespace)
            .unwrap()
            .is_empty());
    }
    let operation = repo.load_operation(&operation_id).unwrap().unwrap();
    assert_eq!(operation.status, OperationStatus::Refused);
}

#[test]
fn commit_snapshot_mismatch_is_terminal_cleanup() {
    let mut fx = base_fixture(0);
    // Declare a wrong O digest.
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
    let response = service
        .commit(
            &mut fx.repo,
            &key(3),
            &body,
            &digest(3),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    assert!(matches!(
        response.outcome,
        ControlOutcome::Refused(ControlRefusal::SnapshotMismatch { .. })
    ));
    assert_terminal_cleanup(&fx.repo, fx.operation_id, fx.namespaces);
}

#[test]
fn commit_manifest_mismatch_on_wrong_routing_is_terminal_cleanup() {
    let mut fx = base_fixture(0);
    let body = commit_body(&fx.repo, &fx);
    // The manifest authorises a DIFFERENT C/W routing than the captured plan.
    let wrong_routing = [fx.namespaces[0], d32(0x71), d32(0x72)];
    let authority = TestAuthority::new(fx.namespaces).override_routing(wrong_routing);
    let service = CommitHostService::new(commit_context(), authority, signer());
    let mut entropy = || d32(0xEE);
    let response = service
        .commit(
            &mut fx.repo,
            &key(4),
            &body,
            &digest(4),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    assert!(matches!(
        response.outcome,
        ControlOutcome::Refused(ControlRefusal::CommitManifestMismatch { .. })
    ));
    assert_terminal_cleanup(&fx.repo, fx.operation_id, fx.namespaces);
}

#[test]
fn commit_manifest_equivocation_from_authority_is_terminal_cleanup() {
    let mut fx = base_fixture(0);
    let body = commit_body(&fx.repo, &fx);
    let authority =
        TestAuthority::new(fx.namespaces).refuse_manifest(ControlRefusal::ManifestEquivocation {
            first_digest: d32(0x30),
            second_digest: d32(0x31),
        });
    let service = CommitHostService::new(commit_context(), authority, signer());
    let mut entropy = || d32(0xEE);
    let response = service
        .commit(
            &mut fx.repo,
            &key(5),
            &body,
            &digest(5),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    assert!(matches!(
        response.outcome,
        ControlOutcome::Refused(ControlRefusal::ManifestEquivocation { .. })
    ));
    assert_terminal_cleanup(&fx.repo, fx.operation_id, fx.namespaces);
}

#[test]
fn commit_operation_expired_is_terminal_cleanup() {
    let mut fx = base_fixture(0);
    let body = commit_body(&fx.repo, &fx);
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);
    // Commit past the operation expiry.
    let response = service
        .commit(
            &mut fx.repo,
            &key(6),
            &body,
            &digest(6),
            EXPIRY + 1,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    assert!(matches!(
        response.outcome,
        ControlOutcome::Refused(ControlRefusal::OperationExpired { .. })
    ));
    assert_terminal_cleanup(&fx.repo, fx.operation_id, fx.namespaces);
}

// ---------------------------------------------------------------------------
// Reusable rows: commit_busy leaves the operation prepared with valid staging.
// ---------------------------------------------------------------------------

#[test]
fn commit_busy_is_reusable_and_leaves_operation_prepared() {
    let mut fx = base_fixture(0);
    let body = commit_body(&fx.repo, &fx);
    let refusal = ControlRefusal::CommitBusy {
        limit_id: riot_anchor_protocol::records::AnchorLimitId::from_id(8).unwrap(),
        retry_after_seconds: 30,
    };
    let authority = TestAuthority::new(fx.namespaces).refuse_capacity(refusal.clone());
    let service = CommitHostService::new(commit_context(), authority, signer());
    let mut entropy = || d32(0xEE);

    let response = service
        .commit(
            &mut fx.repo,
            &key(7),
            &body,
            &digest(7),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    match &response.outcome {
        ControlOutcome::Refused(r @ ControlRefusal::CommitBusy { .. }) => {
            assert_eq!(r.retry_scope(), RetryScope::SameOperationNewCommitKey);
        }
        other => panic!("expected commit_busy, got {other:?}"),
    }
    // Reusable disposition: operation still PREPARED, staging still present.
    let operation = fx.repo.load_operation(&fx.operation_id).unwrap().unwrap();
    assert_eq!(operation.status, OperationStatus::Prepared);
    for namespace in fx.namespaces {
        assert_eq!(
            fx.repo
                .staged_entries(&fx.operation_id, &namespace)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(fx.repo.committed_entry_count(&namespace).unwrap(), 0);
    }
}

// ---------------------------------------------------------------------------
// Pre-claim rows: operation_not_found and idempotency_conflict write nothing.
// ---------------------------------------------------------------------------

#[test]
fn commit_operation_not_found_refuses_without_mutation() {
    let mut repo = repo();
    let body = CommitHostV1 {
        operation_id: d32(0xAA),
        ordered_namespace_snapshot_digests: [d32(0); 3],
    };
    let service =
        CommitHostService::new(commit_context(), TestAuthority::new([d32(0); 3]), signer());
    let mut entropy = || d32(0xEE);
    let response = service
        .commit(
            &mut repo,
            &key(8),
            &body,
            &digest(8),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    assert!(matches!(
        response.outcome,
        ControlOutcome::Refused(ControlRefusal::OperationNotFound { .. })
    ));
    // No idempotency row was claimed (a later novel commit for a real op still works).
    assert!(repo.load_operation(&d32(0xAA)).unwrap().is_none());
}

#[test]
fn commit_changed_body_under_claimed_key_is_idempotency_conflict() {
    let mut fx = base_fixture(0);
    let body = commit_body(&fx.repo, &fx);
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);
    // First commit terminalises key(9)/digest(9).
    service
        .commit(
            &mut fx.repo,
            &key(9),
            &body,
            &digest(9),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("first commit");
    // Same key, DIFFERENT request digest -> conflict, no disclosure.
    let response = service
        .commit(
            &mut fx.repo,
            &key(9),
            &body,
            &digest(0x99),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("second resolves");
    assert!(matches!(
        response.outcome,
        ControlOutcome::Refused(ControlRefusal::IdempotencyConflict)
    ));
}

#[test]
fn commit_same_key_same_body_replays_byte_identically() {
    let mut fx = base_fixture(0);
    let body = commit_body(&fx.repo, &fx);
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);
    let first = service
        .commit(
            &mut fx.repo,
            &key(10),
            &body,
            &digest(10),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("first commit");
    let replay = service
        .commit(
            &mut fx.repo,
            &key(10),
            &body,
            &digest(10),
            NOW + 50,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("replay");
    assert_eq!(
        first.encode_canonical().unwrap(),
        replay.encode_canonical().unwrap(),
        "same-key/same-body replay is byte-identical"
    );
}

// ---------------------------------------------------------------------------
// Generation CAS: two same-base commits have exactly one winner.
// ---------------------------------------------------------------------------

#[test]
fn two_same_base_commits_have_exactly_one_winner() {
    // op1 hosts the site at base 0 and wins.
    let mut fx = base_fixture(0);
    let body1 = commit_body(&fx.repo, &fx);
    let service = CommitHostService::new(
        commit_context(),
        TestAuthority::new(fx.namespaces),
        signer(),
    );
    let mut entropy = || d32(0xEE);
    let first = service
        .commit(
            &mut fx.repo,
            &key(11),
            &body1,
            &digest(11),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("first commit");
    assert!(matches!(
        first.outcome,
        ControlOutcome::Success(ControlSuccess::CommitHost(_))
    ));
    assert_eq!(fx.repo.site_generation(&fx.namespaces[0]).unwrap(), Some(1));

    // op2 is prepared from the SAME base 0 for the same site, stages a new O entry.
    let op2 = d32(0x77);
    let extra = make_item("o-entry-2");
    // op2 must route the same site: reuse the same O/C/W namespaces. Its new entry
    // is in a fresh namespace, but the plan pins the committed O/C/W; stage it under
    // the O namespace so the digest check has content to match, then CAS loses.
    let op2_namespaces = fx.namespaces;
    insert_prepared_operation(
        &mut fx.repo,
        op2,
        op2_namespaces,
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
    );
    // Stage a duplicate-safe new entry: use extra's staged projection but re-key it
    // into the committed O namespace so it is a genuinely new entry id there.
    let mut staged_extra = extra.staged.clone();
    staged_extra.namespace_id = op2_namespaces[0];
    stage_entries(&mut fx.repo, op2, vec![staged_extra], DEADLINE);
    let body2 = CommitHostV1 {
        operation_id: op2,
        ordered_namespace_snapshot_digests: declared_digests(&fx.repo, op2, op2_namespaces),
    };
    let second = service
        .commit(
            &mut fx.repo,
            &key(12),
            &body2,
            &digest(12),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("second commit resolves");
    assert!(
        matches!(
            second.outcome,
            ControlOutcome::Refused(ControlRefusal::StaleBase { .. })
        ),
        "second same-base commit must lose with stale_base, got {:?}",
        second.outcome
    );
    // The winner's state is untouched; generation stayed 1.
    assert_eq!(
        fx.repo.site_generation(&op2_namespaces[0]).unwrap(),
        Some(1)
    );
    let operation2 = fx.repo.load_operation(&op2).unwrap().unwrap();
    assert_eq!(operation2.status, OperationStatus::Refused);
}

// ---------------------------------------------------------------------------
// Sync adapter ingress: the AnchorStage verifies at admission (real Meadowcap).
// ---------------------------------------------------------------------------

fn open_host_stage(
    shared: &Rc<RefCell<riot_anchor::repository::AnchorRepository>>,
    ring: &TokenSecretRing,
    operation_id: [u8; 32],
    namespace_id: [u8; 32],
) -> riot_anchor::sync_service::AnchorStage {
    let token = ring
        .derive(0, &operation_id, &namespace_id, EXPIRY)
        .unwrap();
    let adapter = AnchorSyncRepository::new(Rc::clone(shared), ring.clone(), NOW);
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1, 2, 3],
        ticket_core_bytes: vec![9u8; 40],
        namespace_id,
        mode: Sync2Mode::HostReconcileStaged {
            operation_id,
            namespace_token: token,
        },
    };
    let opened = adapter.open_namespace(&open).expect("routes");
    for (_, party) in opened.parties {
        if let PhaseParty::Receiver(stage) = party {
            return stage;
        }
    }
    panic!("no receiver stage");
}

#[test]
fn sync_stage_admits_genuine_entry_and_refuses_forged() {
    let mut base = repo();
    let item = make_item("ingress");
    let namespace_id = item.namespace_id;
    let operation_id = d32(0x50);
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let token = ring
        .derive(0, &operation_id, &namespace_id, EXPIRY)
        .unwrap();
    insert_prepared_operation(
        &mut base,
        operation_id,
        [namespace_id, d32(0x61), d32(0x62)],
        [token, d32(0), d32(0)],
        0,
        1_000,
        EXPIRY,
        0,
    );
    let shared = Rc::new(RefCell::new(base));

    // Genuine item admits and lands in staging.
    {
        let mut stage = open_host_stage(&shared, &ring, operation_id, namespace_id);
        let ids = vec![item.entry_id.to_vec()];
        let items = vec![item.item_bytes.clone()];
        stage.admit(&ids, &items).expect("genuine item admits");
    }
    assert_eq!(
        shared
            .borrow()
            .staged_entries(&operation_id, &namespace_id)
            .unwrap()
            .len(),
        1
    );

    // Forged item is refused at ingress and never staged.
    {
        let mut stage = open_host_stage(&shared, &ring, operation_id, namespace_id);
        let ids = vec![item.entry_id.to_vec()];
        let items = vec![item.forged_item_bytes.clone()];
        let result = stage.admit(&ids, &items);
        assert!(result.is_err(), "forged item must be refused at admission");
    }
    // Still exactly the one genuine staged entry.
    assert_eq!(
        shared
            .borrow()
            .staged_entries(&operation_id, &namespace_id)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn sync_open_namespace_rejects_bad_token() {
    let mut base = repo();
    let namespace_id = d32(0x40);
    let operation_id = d32(0x51);
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let token = ring
        .derive(0, &operation_id, &namespace_id, EXPIRY)
        .unwrap();
    insert_prepared_operation(
        &mut base,
        operation_id,
        [namespace_id, d32(0x61), d32(0x62)],
        [token, d32(0), d32(0)],
        0,
        1_000,
        EXPIRY,
        0,
    );
    let shared = Rc::new(RefCell::new(base));
    let adapter = AnchorSyncRepository::new(Rc::clone(&shared), ring.clone(), NOW);
    let mut bad_token = token;
    bad_token[0] ^= 0x01;
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1],
        ticket_core_bytes: vec![9u8; 40],
        namespace_id,
        mode: Sync2Mode::HostReconcileStaged {
            operation_id,
            namespace_token: bad_token,
        },
    };
    let refusal = adapter
        .open_namespace(&open)
        .err()
        .expect("bad token refused");
    assert!(matches!(
        refusal,
        riot_anchor_protocol::sync2::Sync2Refusal::InvalidNamespaceToken { .. }
    ));
}
