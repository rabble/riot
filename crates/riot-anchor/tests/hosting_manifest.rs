//! WU-BC production manifest resolution: the ticket-envelope operation column,
//! the committed-manifest tables, and the `TicketManifestAuthority` — the
//! canonical-gate path that resolves the digest-matched, owner-signed `/manifest`
//! from staged ∪ committed `O` state and refuses every trap in the design
//! (delegated signer, payload swap, missing ticket, expired ticket, version
//! rollback), plus the crash-safety failpoint before the manifest rows.

mod hosting_common;

use hosting_common::*;

use riot_anchor::hosting::{no_failpoint, CommitHostService, TicketManifestAuthority};
use riot_anchor::repository::{OperationStatus, StagedEntry};

use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::{CommitHostV1, ControlOutcome, ControlRefusal, ControlSuccess};

const NOW: u64 = 1_500;
const EXPIRY: u64 = 4_600;
const DEADLINE: u64 = 4_600;

// ---------------------------------------------------------------------------
// Repository: the ticket column and the manifest tables.
// ---------------------------------------------------------------------------

#[test]
fn prepared_operation_round_trips_ticket_envelope_bytes() {
    let mut repo = repo();
    let with_ticket = d32(1);
    let without_ticket = d32(2);
    let ticket_bytes = vec![0xAB; 200];

    insert_prepared_operation_with_ticket(
        &mut repo,
        with_ticket,
        [d32(10), d32(11), d32(12)],
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
        Some(ticket_bytes.clone()),
    );
    insert_prepared_operation(
        &mut repo,
        without_ticket,
        [d32(10), d32(11), d32(12)],
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
    );

    let stored = repo.load_operation(&with_ticket).unwrap().unwrap();
    assert_eq!(
        stored.ticket_envelope_bytes,
        Some(ticket_bytes),
        "the persisted ticket envelope must round-trip byte-identically"
    );
    let bare = repo.load_operation(&without_ticket).unwrap().unwrap();
    assert_eq!(
        bare.ticket_envelope_bytes, None,
        "an operation without a stored ticket reads back NULL"
    );
}

#[test]
fn manifest_floor_is_monotonic() {
    let mut repo = repo();
    let community = d32(7);
    let mut tx = repo.begin().unwrap();
    tx.insert_community(&community, 1_000).unwrap();

    assert_eq!(tx.manifest_floor(&community).unwrap(), None);
    tx.advance_manifest_floor(&community, 3, &d32(0x30))
        .unwrap();
    assert_eq!(tx.manifest_floor(&community).unwrap(), Some((3, d32(0x30))));
    // A lower generation never rolls the floor (or its digest) backward.
    tx.advance_manifest_floor(&community, 2, &d32(0x20))
        .unwrap();
    assert_eq!(tx.manifest_floor(&community).unwrap(), Some((3, d32(0x30))));
    // A higher generation advances both.
    tx.advance_manifest_floor(&community, 5, &d32(0x50))
        .unwrap();
    assert_eq!(tx.manifest_floor(&community).unwrap(), Some((5, d32(0x50))));
}

#[test]
fn committed_manifest_returns_the_highest_generation() {
    let mut repo = repo();
    let community = d32(8);
    let mut tx = repo.begin().unwrap();
    tx.insert_community(&community, 1_000).unwrap();

    assert_eq!(tx.committed_manifest(&community).unwrap(), None);
    tx.upsert_manifest(&community, 1, &d32(0x41), b"manifest-v1")
        .unwrap();
    tx.upsert_manifest(&community, 2, &d32(0x42), b"manifest-v2")
        .unwrap();
    let (generation, digest, bytes) = tx.committed_manifest(&community).unwrap().unwrap();
    assert_eq!(generation, 2);
    assert_eq!(digest, d32(0x42));
    assert_eq!(bytes, b"manifest-v2");
    // Re-upserting the same generation replaces in place, never duplicates.
    tx.upsert_manifest(&community, 2, &d32(0x43), b"manifest-v2b")
        .unwrap();
    let (generation, digest, bytes) = tx.committed_manifest(&community).unwrap().unwrap();
    assert_eq!(generation, 2);
    assert_eq!(digest, d32(0x43));
    assert_eq!(bytes, b"manifest-v2b");
}

// ---------------------------------------------------------------------------
// TicketManifestAuthority through the composite Commit.
// ---------------------------------------------------------------------------

/// A prepared + fully staged site commit fixture over the REAL owned-root site:
/// the persisted ticket, the staged owner-signed `/manifest` in `O`, and one
/// communal entry each in `C` and `W`.
struct SiteCommit {
    repo: riot_anchor::repository::AnchorRepository,
    site: SiteFixture,
    operation_id: [u8; 32],
}

fn site_commit(seed: u8, operation_id: [u8; 32]) -> SiteCommit {
    let site = make_site_fixture(seed, 3, 1_000, 1_000 + 24 * 60 * 60);
    let mut repo = repo();
    insert_prepared_operation_with_ticket(
        &mut repo,
        operation_id,
        site.namespaces,
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
        Some(site.ticket_envelope_bytes.clone()),
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.manifest_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.c_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.w_staged.clone()],
        DEADLINE,
    );
    SiteCommit {
        repo,
        site,
        operation_id,
    }
}

fn service() -> CommitHostService<TicketManifestAuthority, TestSigner> {
    CommitHostService::new(commit_context(), TicketManifestAuthority, signer())
}

fn commit_body(fx: &SiteCommit) -> CommitHostV1 {
    CommitHostV1 {
        operation_id: fx.operation_id,
        ordered_namespace_snapshot_digests: declared_digests(
            &fx.repo,
            fx.operation_id,
            fx.site.namespaces,
        ),
    }
}

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
fn commit_resolves_manifest_from_staged_o_entry_and_promotes() {
    let mut fx = site_commit(0x11, d32(0xA0));
    let body = commit_body(&fx);
    let service = service();
    let mut entropy = || d32(0xEE);

    let response = service
        .commit(
            &mut fx.repo,
            &d16(1),
            &body,
            &d32(1),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit succeeds");

    let receipt = match &response.outcome {
        ControlOutcome::Success(ControlSuccess::CommitHost(receipt)) => receipt.clone(),
        other => panic!("expected CommitHost success, got {other:?}"),
    };
    // The receipt binds the REAL resolved manifest coordinates, not operator say-so.
    assert_eq!(receipt.body.full_site_root, fx.site.root_id);
    assert_eq!(receipt.body.manifest_digest, fx.site.manifest_digest);
    assert_eq!(receipt.body.manifest_version, 3);
    assert_eq!(receipt.body.committed_site_generation, 1);

    // Promotion happened for all three namespaces.
    for namespace in fx.site.namespaces {
        assert_eq!(fx.repo.committed_entry_count(&namespace).unwrap(), 1);
    }

    // The manifests + manifest_floors rows landed in the SAME commit transaction.
    let tx = fx.repo.begin().unwrap();
    let (generation, digest, bytes) = tx
        .committed_manifest(&fx.site.root_id)
        .unwrap()
        .expect("committed manifest row");
    assert_eq!(generation, 3);
    assert_eq!(digest, fx.site.manifest_digest);
    assert_eq!(
        bytes, fx.site.manifest_payload_bytes,
        "the stored manifest bytes are the validated canonical payload"
    );
    assert_eq!(
        tx.manifest_floor(&fx.site.root_id).unwrap(),
        Some((3, fx.site.manifest_digest))
    );
    drop(tx);
}

#[test]
fn commit_refuses_when_no_manifest_entry_is_staged_or_committed() {
    let site = make_site_fixture(0x12, 3, 1_000, 1_000 + 24 * 60 * 60);
    let mut repo = repo();
    let operation_id = d32(0xA1);
    insert_prepared_operation_with_ticket(
        &mut repo,
        operation_id,
        site.namespaces,
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
        Some(site.ticket_envelope_bytes.clone()),
    );
    // C and W stage, but NO /manifest entry anywhere in O.
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.c_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.w_staged.clone()],
        DEADLINE,
    );

    let body = CommitHostV1 {
        operation_id,
        ordered_namespace_snapshot_digests: declared_digests(&repo, operation_id, site.namespaces),
    };
    let mut entropy = || d32(0xEE);
    let response = service()
        .commit(
            &mut repo,
            &d16(2),
            &body,
            &d32(2),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    assert!(
        matches!(
            response.outcome,
            ControlOutcome::Refused(ControlRefusal::CommitManifestMismatch { .. })
        ),
        "a site with no manifest must refuse commit_manifest_mismatch, got {:?}",
        response.outcome
    );
    assert_terminal_cleanup(&repo, operation_id, site.namespaces);
}

#[test]
fn commit_refuses_a_delegated_signer_manifest() {
    // TRAP 1: a delegated owned cap covering /manifest passes ordinary admission
    // (and genuinely authorises the entry), but must never sign the manifest.
    let site = make_site_fixture(0x13, 3, 1_000, 1_000 + 24 * 60 * 60);
    let delegated = make_delegated_manifest_item(&site);
    let mut repo = repo();
    let operation_id = d32(0xA2);
    insert_prepared_operation_with_ticket(
        &mut repo,
        operation_id,
        site.namespaces,
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
        Some(site.ticket_envelope_bytes.clone()),
    );
    stage_entries(&mut repo, operation_id, vec![delegated], DEADLINE);
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.c_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.w_staged.clone()],
        DEADLINE,
    );

    let body = CommitHostV1 {
        operation_id,
        ordered_namespace_snapshot_digests: declared_digests(&repo, operation_id, site.namespaces),
    };
    let mut entropy = || d32(0xEE);
    let response = service()
        .commit(
            &mut repo,
            &d16(3),
            &body,
            &d32(3),
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
        "a delegated manifest signer must refuse invalid_operation_authority, got {:?}",
        response.outcome
    );
    assert_terminal_cleanup(&repo, operation_id, site.namespaces);
}

#[test]
fn commit_refuses_a_payload_swapped_manifest_item() {
    // TRAP 2: the entry + signature are genuine, but the carried payload bytes
    // were swapped for a different manifest encoding. `validate_site_manifest`
    // alone would accept it (it never binds payload to the entry digest); only
    // `verify_anchor_item_parts` refuses.
    let site = make_site_fixture(0x14, 3, 1_000, 1_000 + 24 * 60 * 60);
    let swapped = make_payload_swapped_manifest_item(&site);
    let mut repo = repo();
    let operation_id = d32(0xA3);
    insert_prepared_operation_with_ticket(
        &mut repo,
        operation_id,
        site.namespaces,
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
        Some(swapped.ticket_envelope_bytes),
    );
    stage_entries(&mut repo, operation_id, vec![swapped.staged], DEADLINE);
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.c_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.w_staged.clone()],
        DEADLINE,
    );

    let body = CommitHostV1 {
        operation_id,
        ordered_namespace_snapshot_digests: declared_digests(&repo, operation_id, site.namespaces),
    };
    let mut entropy = || d32(0xEE);
    let response = service()
        .commit(
            &mut repo,
            &d16(4),
            &body,
            &d32(4),
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
        "a payload-swapped manifest item must refuse invalid_operation_authority, got {:?}",
        response.outcome
    );
    assert_terminal_cleanup(&repo, operation_id, site.namespaces);
}

#[test]
fn commit_refuses_an_expired_ticket_at_commit_time() {
    // TRAP 5: the gate call at commit re-enforces expiry with the commit-time
    // clock, even though the ticket was fresh at PrepareHost.
    let site = make_site_fixture(0x15, 3, 1_000, 2_000);
    let mut repo = repo();
    let operation_id = d32(0xA4);
    insert_prepared_operation_with_ticket(
        &mut repo,
        operation_id,
        site.namespaces,
        [d32(0); 3],
        0,
        1_000,
        // The OPERATION is still alive well past the ticket's expiry.
        10_000,
        0,
        Some(site.ticket_envelope_bytes.clone()),
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.manifest_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.c_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.w_staged.clone()],
        DEADLINE,
    );

    let body = CommitHostV1 {
        operation_id,
        ordered_namespace_snapshot_digests: declared_digests(&repo, operation_id, site.namespaces),
    };
    let mut entropy = || d32(0xEE);
    let response = service()
        .commit(
            &mut repo,
            &d16(5),
            &body,
            &d32(5),
            // Past the ticket expiry (2_000), before the operation expiry.
            3_000,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    assert!(
        matches!(
            response.outcome,
            ControlOutcome::Refused(ControlRefusal::TicketExpired {
                expires_at: 2_000,
                ..
            })
        ),
        "an expired ticket at commit time must refuse ticket_expired, got {:?}",
        response.outcome
    );
    assert_terminal_cleanup(&repo, operation_id, site.namespaces);
}

#[test]
fn commit_refuses_manifest_version_rollback_via_the_floor() {
    // TRAP 6: replaying an older root-signed manifest+ticket pair must not roll
    // the site's manifest backward once the floor has advanced.
    let mut fx = site_commit(0x16, d32(0xA5));
    {
        let mut tx = fx.repo.begin().unwrap();
        tx.insert_community(&fx.site.root_id, 900).unwrap();
        tx.advance_manifest_floor(&fx.site.root_id, 5, &d32(0x99))
            .unwrap();
        tx.commit().unwrap();
    }

    let body = CommitHostV1 {
        operation_id: fx.operation_id,
        // The community row now exists with generation 0, so base 0 still CASes;
        // the floor refusal fires before any CAS.
        ordered_namespace_snapshot_digests: declared_digests(
            &fx.repo,
            fx.operation_id,
            fx.site.namespaces,
        ),
    };
    let mut entropy = || d32(0xEE);
    let response = service()
        .commit(
            &mut fx.repo,
            &d16(6),
            &body,
            &d32(6),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("commit resolves");
    assert!(
        matches!(
            response.outcome,
            ControlOutcome::Refused(ControlRefusal::ManifestEquivocation { .. })
        ),
        "a manifest version below the floor must refuse manifest_equivocation, got {:?}",
        response.outcome
    );
    assert_terminal_cleanup(&fx.repo, fx.operation_id, fx.site.namespaces);
    // The floor itself never rolled back.
    let tx = fx.repo.begin().unwrap();
    assert_eq!(
        tx.manifest_floor(&fx.site.root_id).unwrap(),
        Some((5, d32(0x99)))
    );
    drop(tx);
}

#[test]
fn commit_without_a_persisted_ticket_fails_closed() {
    // TRAP 3: the operation row is the ONLY ticket source. A prepared operation
    // without one (pre-migration row) fails closed.
    let site = make_site_fixture(0x17, 3, 1_000, 1_000 + 24 * 60 * 60);
    let mut repo = repo();
    let operation_id = d32(0xA6);
    insert_prepared_operation(
        &mut repo,
        operation_id,
        site.namespaces,
        [d32(0); 3],
        0,
        1_000,
        EXPIRY,
        0,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.manifest_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.c_staged.clone()],
        DEADLINE,
    );
    stage_entries(
        &mut repo,
        operation_id,
        vec![site.w_staged.clone()],
        DEADLINE,
    );

    let body = CommitHostV1 {
        operation_id,
        ordered_namespace_snapshot_digests: declared_digests(&repo, operation_id, site.namespaces),
    };
    let mut entropy = || d32(0xEE);
    let response = service()
        .commit(
            &mut repo,
            &d16(7),
            &body,
            &d32(7),
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
        "a missing persisted ticket must refuse invalid_operation_authority, got {:?}",
        response.outcome
    );
    assert_terminal_cleanup(&repo, operation_id, site.namespaces);
}

#[test]
fn commit_replay_returns_byte_identical_receipt_without_new_manifest_rows() {
    let mut fx = site_commit(0x18, d32(0xA7));
    let body = commit_body(&fx);
    let service = service();
    let mut entropy = || d32(0xEE);

    let first = service
        .commit(
            &mut fx.repo,
            &d16(8),
            &body,
            &d32(8),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("first commit");
    let (first_manifest, first_floor) = {
        let tx = fx.repo.begin().unwrap();
        (
            tx.committed_manifest(&fx.site.root_id).unwrap(),
            tx.manifest_floor(&fx.site.root_id).unwrap(),
        )
    };

    let replay = service
        .commit(
            &mut fx.repo,
            &d16(8),
            &body,
            &d32(8),
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
    // The replay resolved nothing and wrote nothing: manifest state unchanged.
    let tx = fx.repo.begin().unwrap();
    assert_eq!(
        tx.committed_manifest(&fx.site.root_id).unwrap(),
        first_manifest
    );
    assert_eq!(tx.manifest_floor(&fx.site.root_id).unwrap(), first_floor);
    drop(tx);
}

#[test]
fn commit_failpoint_before_manifest_row_leaves_no_partial_state() {
    let mut fx = site_commit(0x19, d32(0xA8));
    let body = commit_body(&fx);
    let service = service();
    let mut entropy = || d32(0xEE);
    let mut fp = |seen: &str| seen == "manifest";

    let result = service.commit(
        &mut fx.repo,
        &d16(9),
        &body,
        &d32(9),
        NOW,
        &mut entropy,
        &mut fp,
    );
    assert!(
        matches!(
            result,
            Err(riot_anchor::hosting::CommitError::Failpoint("manifest"))
        ),
        "the manifest failpoint must abort the transaction"
    );

    // The WHOLE transaction rolled back: nothing promoted, no generation, no
    // manifest rows, staging intact, operation still prepared.
    for namespace in fx.site.namespaces {
        assert_eq!(fx.repo.committed_entry_count(&namespace).unwrap(), 0);
        assert_eq!(
            fx.repo
                .staged_entries(&fx.operation_id, &namespace)
                .unwrap()
                .len(),
            1
        );
    }
    assert_eq!(fx.repo.site_generation(&fx.site.root_id).unwrap(), None);
    let tx = fx.repo.begin().unwrap();
    assert_eq!(tx.committed_manifest(&fx.site.root_id).unwrap(), None);
    assert_eq!(tx.manifest_floor(&fx.site.root_id).unwrap(), None);
    drop(tx);
    assert_eq!(
        fx.repo
            .load_operation(&fx.operation_id)
            .unwrap()
            .unwrap()
            .status,
        OperationStatus::Prepared
    );
}

// ---------------------------------------------------------------------------
// A refresh commit resolves the manifest from COMMITTED O state.
// ---------------------------------------------------------------------------

#[test]
fn refresh_commit_resolves_manifest_from_committed_o_state() {
    // First host: manifest staged in O, promoted.
    let mut fx = site_commit(0x1A, d32(0xA9));
    let body = commit_body(&fx);
    let service = service();
    // Fresh entropy per call so the two commits mint distinct receipt ids.
    let mut counter = 0u8;
    let mut entropy = move || {
        counter = counter.wrapping_add(1);
        d32(counter)
    };
    service
        .commit(
            &mut fx.repo,
            &d16(10),
            &body,
            &d32(10),
            NOW,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("first commit");

    // Refresh at base 1: a second operation stages a NEW C entry only; the
    // manifest is already committed in O and must resolve from there.
    let refresh_op = d32(0xAA);
    insert_prepared_operation_with_ticket(
        &mut fx.repo,
        refresh_op,
        fx.site.namespaces,
        [d32(0); 3],
        1,
        2_000,
        EXPIRY,
        0,
        Some(fx.site.ticket_envelope_bytes.clone()),
    );
    let extra = make_item("refresh-c-entry");
    let mut staged_extra: StagedEntry = extra.staged.clone();
    staged_extra.namespace_id = fx.site.namespaces[1];
    stage_entries(&mut fx.repo, refresh_op, vec![staged_extra], DEADLINE);

    let body = CommitHostV1 {
        operation_id: refresh_op,
        ordered_namespace_snapshot_digests: declared_digests(
            &fx.repo,
            refresh_op,
            fx.site.namespaces,
        ),
    };
    let response = service
        .commit(
            &mut fx.repo,
            &d16(11),
            &body,
            &d32(11),
            NOW + 100,
            &mut entropy,
            &mut no_failpoint,
        )
        .expect("refresh commit");
    let receipt = match &response.outcome {
        ControlOutcome::Success(ControlSuccess::CommitHost(receipt)) => receipt.clone(),
        other => panic!("expected refresh receipt, got {other:?}"),
    };
    assert_eq!(receipt.body.base_site_generation, 1);
    assert_eq!(receipt.body.committed_site_generation, 2);
    assert_eq!(receipt.body.manifest_digest, fx.site.manifest_digest);
}
