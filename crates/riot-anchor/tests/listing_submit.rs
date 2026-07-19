//! WU-015B — ordinary `SubmitListing` lifecycle: verification-before-admit, the
//! one-transaction inclusion/receipt/projection accept, refresh, replay/conflict
//! recovery, and the three SECURITY refusals that protect the directory trust root.

mod listing_common;

use listing_common::*;

use riot_anchor::listing::no_failpoint;
use riot_anchor::repository::AnchorRepository;

use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::{
    ControlOutcome, ControlRefusal, ControlResponseV1, ControlSuccess,
};
use riot_anchor_protocol::records::{ControlOperationKind, ListingReceiptV1};

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------

fn submit(
    service: &riot_anchor::listing::SubmitListingService<TestListingAuthority, TestSigner>,
    repo: &mut AnchorRepository,
    key: [u8; 16],
    submission: &riot_anchor::listing::RawListingSubmission,
    digest: [u8; 32],
    now: u64,
) -> ControlResponseV1 {
    service
        .submit(repo, &key, submission, &digest, now, &mut no_failpoint)
        .expect("submit produces a control response")
}

fn expect_receipt(response: &ControlResponseV1) -> ListingReceiptV1 {
    assert_eq!(response.kind, ControlOperationKind::SubmitListing);
    match &response.outcome {
        ControlOutcome::Success(ControlSuccess::SubmitListing(receipt)) => (**receipt).clone(),
        other => panic!("expected SubmitListing success, got {other:?}"),
    }
}

fn expect_refusal(response: &ControlResponseV1) -> ControlRefusal {
    assert_eq!(response.kind, ControlOperationKind::SubmitListing);
    match &response.outcome {
        ControlOutcome::Refused(refusal) => refusal.clone(),
        other => panic!("expected refusal, got {other:?}"),
    }
}

fn inclusion_count(repo: &mut AnchorRepository, community_id: [u8; 32]) -> u64 {
    let tx = repo.begin().expect("begin");
    let count = tx
        .directory_inclusion_count(&community_id)
        .expect("inclusion count");
    drop(tx);
    count
}

fn projection_generation(repo: &mut AnchorRepository) -> u64 {
    let tx = repo.begin().expect("begin");
    let generation = tx.projection_generation().expect("projection generation");
    drop(tx);
    generation
}

fn is_listed(repo: &mut AnchorRepository, community_id: [u8; 32]) -> bool {
    let tx = repo.begin().expect("begin");
    let listed = tx
        .current_listing(&community_id)
        .expect("current listing")
        .is_some();
    drop(tx);
    listed
}

// ---------------------------------------------------------------------------
// ordinary submit
// ---------------------------------------------------------------------------

#[test]
fn ordinary_submit_lists_appends_one_inclusion_and_invalidates_projection() {
    let now = 1_000_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x10, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    let response = submit(
        &service,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(0x80),
        now,
    );

    let receipt = expect_receipt(&response);
    assert_eq!(receipt.body.full_site_root, listing.root.root_id);
    assert_eq!(receipt.body.accepted_listing_epoch, 0);
    assert_eq!(receipt.body.accepted_listing_revision, 0);
    assert_eq!(
        receipt.body.feed_coordinate, 1,
        "first inclusion is sequence 1"
    );
    assert_eq!(receipt.body.request_idempotency_key, d16(1));
    assert_eq!(receipt.body.expires_at, listing.expiry);

    assert!(is_listed(&mut repo, listing.community_id));
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 1);
    assert_eq!(projection_generation(&mut repo), 1);
}

#[test]
fn refresh_replaces_current_state_and_retains_feed_history() {
    let now = 2_000_000u64;
    let mut repo = repo();
    let first = genuine_listing(0x11, 0, 0, now);
    insert_hosted_community(&mut repo, first.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(first.root.root_id));

    let r1 = submit(&service, &mut repo, d16(1), &first.submission, d32(1), now);
    expect_receipt(&r1);

    // A refresh: same root/epoch, higher revision (distinct digest).
    let refresh = genuine_listing_for(&first.root, first.coords, 0, 1, now + 10);
    let r2 = submit(&service, &mut repo, d16(2), &refresh, d32(2), now + 10);
    let receipt = expect_receipt(&r2);

    assert_eq!(receipt.body.accepted_listing_revision, 1);
    assert_eq!(
        receipt.body.feed_coordinate, 2,
        "second inclusion is sequence 2"
    );

    // Current state replaced (one listing row), signed feed history retained (two).
    assert!(is_listed(&mut repo, first.community_id));
    assert_eq!(
        inclusion_count(&mut repo, first.community_id),
        2,
        "signed feed history is retained across a refresh"
    );
    assert_eq!(projection_generation(&mut repo), 2);
}

// ---------------------------------------------------------------------------
// listing-before-hosting / stale generation / manifest mismatch
// ---------------------------------------------------------------------------

#[test]
fn listing_before_hosting_rejected_without_durable_state() {
    let now = 3_000_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x12, 0, 0, now);
    // NOTE: community is NOT hosted; authority returns not_hosted.
    let service = service_with(TestListingAuthority::refusing(ControlRefusal::NotHosted));

    let response = submit(
        &service,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(1),
        now,
    );
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::NotHosted
    ));

    // No durable state whatsoever.
    assert!(!is_listed(&mut repo, listing.community_id));
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 0);
    assert_eq!(projection_generation(&mut repo), 0);

    // And the key was never claimed: a later hosted retry with the SAME key can
    // still succeed.
    let service_ok = service_with(TestListingAuthority::hosted_root(listing.root.root_id));
    insert_hosted_community(&mut repo, listing.community_id, now);
    let retry = submit(
        &service_ok,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(1),
        now,
    );
    expect_receipt(&retry);
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 1);
}

#[test]
fn stale_generation_refused_without_durable_state() {
    let now = 3_500_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x13, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::refusing(ControlRefusal::StaleBase {
        current_generation: 7,
        ordered_namespace_snapshot_digests: [d32(1), d32(2), d32(3)],
    }));

    let response = submit(
        &service,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(1),
        now,
    );
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::StaleBase { .. }
    ));
    assert!(!is_listed(&mut repo, listing.community_id));
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 0);
    assert_eq!(projection_generation(&mut repo), 0);
}

// ---------------------------------------------------------------------------
// idempotency: replay, conflict, recovery
// ---------------------------------------------------------------------------

#[test]
fn same_key_retry_returns_terminal_bytes_without_second_inclusion() {
    let now = 4_000_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x14, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    let first = submit(
        &service,
        &mut repo,
        d16(9),
        &listing.submission,
        d32(1),
        now,
    );
    let second = submit(
        &service,
        &mut repo,
        d16(9),
        &listing.submission,
        d32(1),
        now + 5,
    );

    // Byte-identical terminal replay (lost-delivery recovery), no second inclusion.
    assert_eq!(first, second);
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 1);
    assert_eq!(projection_generation(&mut repo), 1);
}

#[test]
fn same_key_changed_body_conflicts_without_disclosure() {
    let now = 4_500_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x15, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    let first = submit(
        &service,
        &mut repo,
        d16(9),
        &listing.submission,
        d32(1),
        now,
    );
    expect_receipt(&first);

    // Same key, DIFFERENT control_request_digest (changed body) → conflict.
    let other = genuine_listing_for(&listing.root, listing.coords, 0, 1, now);
    let response = submit(&service, &mut repo, d16(9), &other, d32(2), now);
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::IdempotencyConflict
    ));
    // No second inclusion; the stored result is not disclosed.
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 1);
}

// ---------------------------------------------------------------------------
// delegated listing (happy path)
// ---------------------------------------------------------------------------

#[test]
fn genuine_delegated_listing_is_listed() {
    let now = 5_000_000u64;
    let mut repo = repo();
    let listing = delegated_listing(0x16, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    let response = submit(
        &service,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(1),
        now,
    );
    expect_receipt(&response);
    assert!(is_listed(&mut repo, listing.community_id));
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 1);
}

// ---------------------------------------------------------------------------
// SECURITY refusals (the key deliverable)
// ---------------------------------------------------------------------------

#[test]
fn security_forged_listing_entry_is_refused_and_never_listed() {
    let now = 6_000_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x20, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    // Corrupt the entry signature so the REAL verify_entry fails.
    let forged = riot_anchor::listing::RawListingSubmission {
        listing_item_bytes: forge_item_signature(&listing.submission.listing_item_bytes),
        delegate_grant: None,
    };
    let response = submit(&service, &mut repo, d16(1), &forged, d32(1), now);
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::InvalidListingAuthority
    ));
    assert!(!is_listed(&mut repo, listing.community_id));
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 0);
    assert_eq!(projection_generation(&mut repo), 0);
}

#[test]
fn security_delegated_listing_with_forged_grant_signature_is_refused() {
    let now = 6_500_000u64;
    let mut repo = repo();
    let mut listing = delegated_listing(0x21, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    // Corrupt the (separately supplied) grant signature.
    if let Some(grant) = listing.submission.delegate_grant.as_mut() {
        grant.signature[0] ^= 0x01;
    }
    let response = submit(
        &service,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(1),
        now,
    );
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::InvalidListingAuthority
    ));
    assert!(!is_listed(&mut repo, listing.community_id));
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 0);
}

#[test]
fn security_delegated_cap_presented_as_root_owned_is_refused() {
    // A delegate (delegated cap, non-empty delegations) submits with NO grant,
    // trying to be treated as root-owned so it seals the epoch. Rejected.
    let now = 6_700_000u64;
    let mut repo = repo();
    let mut listing = delegated_listing(0x22, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    listing.submission.delegate_grant = None; // strip the grant
    let response = submit(
        &service,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(1),
        now,
    );
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::InvalidListingAuthority
    ));
    assert!(!is_listed(&mut repo, listing.community_id));
}

#[test]
fn security_listing_coordinates_disagreeing_with_ticket_is_refused() {
    let now = 7_000_000u64;
    let mut repo = repo();
    let root = owned_root(0x23);
    // The listing's coordinates disagree with the embedded signed ticket: the
    // ticket is signed over `honest` coords, but the listing claims a different C.
    let honest = Coords {
        root_id: root.root_id,
        o: root.root_id,
        c: d32(0x31),
        w: d32(0x32),
        manifest_digest: d32(0x33),
        manifest_version: 3,
    };
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(&root, &honest, now - 1, expiry);
    let mut lying = honest;
    lying.c = d32(0x99); // disagree with the signed ticket's C
    let payload = listing_payload(&lying, ticket, 0, 0, true, "Liar", now - 1, expiry);
    let payload_bytes = payload.encode_canonical().expect("encode");
    let owner = willow25::entry::SubspaceSecret::from_bytes(&[0x40; 32]);
    let item = root_owned_item(&root, &owner, &payload_bytes);
    let submission = riot_anchor::listing::RawListingSubmission {
        listing_item_bytes: item,
        delegate_grant: None,
    };

    insert_hosted_community(&mut repo, root.root_id, now);
    let service = service_with(TestListingAuthority::hosted_root(root.root_id));
    let response = submit(&service, &mut repo, d16(1), &submission, d32(1), now);
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::InvalidListingAuthority
    ));
    assert!(!is_listed(&mut repo, root.root_id));
    assert_eq!(inclusion_count(&mut repo, root.root_id), 0);
}

#[test]
fn expired_listing_is_refused() {
    // The listing is expired but its embedded ticket is still valid — so crypto
    // verification passes and `resolve_listing` refuses with `listing_expired`.
    let now = 8_000_000u64;
    let mut repo = repo();
    let root = owned_root(0x24);
    let coords = Coords {
        root_id: root.root_id,
        o: root.root_id,
        c: d32(0x41),
        w: d32(0x42),
        manifest_digest: d32(0x43),
        manifest_version: 3,
    };
    let ticket = root_signed_ticket(&root, &coords, now - 100, now + 24 * 60 * 60);
    let payload = listing_payload(&coords, ticket, 0, 0, true, "Expired", now - 100, now);
    let payload_bytes = payload.encode_canonical().expect("encode");
    let owner = willow25::entry::SubspaceSecret::from_bytes(&[0x44; 32]);
    let submission = riot_anchor::listing::RawListingSubmission {
        listing_item_bytes: root_owned_item(&root, &owner, &payload_bytes),
        delegate_grant: None,
    };
    insert_hosted_community(&mut repo, root.root_id, now);
    let service = service_with(TestListingAuthority::hosted_root(root.root_id));

    // Observe at `now`, which equals the listing expiry (inclusive) → expired.
    let response = submit(&service, &mut repo, d16(1), &submission, d32(1), now);
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::ListingExpired { .. }
    ));
    assert!(!is_listed(&mut repo, root.root_id));
}
