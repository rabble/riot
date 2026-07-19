//! WU-M2 coverage — the ordinary `SubmitListing` resolve outcomes the happy-path
//! suite never reaches: Deduplicated (byte-identical under a new key),
//! Equivocation (same coordinates, different digest → show neither), Superseded
//! (a valid listing that loses to the shown one), the illegal-epoch-advance
//! authority mapping, and the verify-submission refusal arms
//! (listed=false, coordinate/namespace disagreement, grant-shape mismatches).

mod listing_common;

use listing_common::*;

use riot_anchor::listing::{
    no_failpoint, ListingError, RawListingSubmission, SubmitListingService,
};
use riot_anchor::repository::AnchorRepository;

use riot_anchor_protocol::codec::{CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{
    ControlOutcome, ControlRefusal, ControlResponseV1, ControlSuccess,
};
use riot_anchor_protocol::records::ControlOperationKind;

fn submit(
    service: &SubmitListingService<TestListingAuthority, TestSigner>,
    repo: &mut AnchorRepository,
    key: [u8; 16],
    submission: &RawListingSubmission,
    digest: [u8; 32],
    now: u64,
) -> ControlResponseV1 {
    service
        .submit(repo, &key, submission, &digest, now, &mut no_failpoint)
        .expect("submit produces a control response")
}

fn expect_refusal(response: &ControlResponseV1) -> ControlRefusal {
    assert_eq!(response.kind, ControlOperationKind::SubmitListing);
    match &response.outcome {
        ControlOutcome::Refused(refusal) => refusal.clone(),
        other => panic!("expected refusal, got {other:?}"),
    }
}

fn expect_success(response: &ControlResponseV1) {
    match &response.outcome {
        ControlOutcome::Success(ControlSuccess::SubmitListing(_)) => {}
        other => panic!("expected SubmitListing success, got {other:?}"),
    }
}

fn inclusion_count(repo: &mut AnchorRepository, community_id: [u8; 32]) -> u64 {
    let tx = repo.begin().expect("begin");
    let count = tx.directory_inclusion_count(&community_id).expect("count");
    drop(tx);
    count
}

fn projection_generation(repo: &mut AnchorRepository) -> u64 {
    let tx = repo.begin().expect("begin");
    let generation = tx.projection_generation().expect("gen");
    drop(tx);
    generation
}

fn is_listed(repo: &mut AnchorRepository, community_id: [u8; 32]) -> bool {
    let tx = repo.begin().expect("begin");
    let listed = tx
        .current_listing(&community_id)
        .expect("listing")
        .is_some();
    drop(tx);
    listed
}

// ---------------------------------------------------------------------------
// Deduplicated: a byte-identical listing under a NEW idempotency key
// ---------------------------------------------------------------------------

#[test]
fn byte_identical_listing_under_a_new_key_deduplicates_without_a_second_inclusion() {
    let now = 1_000_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x40, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    // First submission is shown.
    let first = submit(
        &service,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(1),
        now,
    );
    expect_success(&first);
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 1);
    let gen_after_first = projection_generation(&mut repo);

    // The SAME listing body under a DIFFERENT key (distinct control_request_digest)
    // is a novel idempotency key whose digest matches the shown listing.
    let dedup = submit(
        &service,
        &mut repo,
        d16(2),
        &listing.submission,
        d32(2),
        now,
    );
    expect_success(&dedup);

    // Deduplication re-issues a receipt at the current feed head: no second
    // inclusion, projection generation untouched, still listed.
    assert_eq!(
        inclusion_count(&mut repo, listing.community_id),
        1,
        "dedup appends no second inclusion"
    );
    assert_eq!(
        projection_generation(&mut repo),
        gen_after_first,
        "dedup does not invalidate the projection"
    );
    assert!(is_listed(&mut repo, listing.community_id));

    // And the dedup result is now stored terminally under its own key: an exact
    // same-key replay returns byte-identical bytes.
    let replay = submit(
        &service,
        &mut repo,
        d16(2),
        &listing.submission,
        d32(2),
        now + 3,
    );
    assert_eq!(dedup, replay, "dedup terminal replays identically");
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 1);
}

// ---------------------------------------------------------------------------
// Equivocation: same coordinates, different digest → show neither
// ---------------------------------------------------------------------------

#[test]
fn same_coordinates_different_digest_equivocates_and_censors_both() {
    let now = 2_000_000u64;
    let mut repo = repo();
    let first = genuine_listing(0x41, 0, 0, now);
    insert_hosted_community(&mut repo, first.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(first.root.root_id));

    let shown = submit(&service, &mut repo, d16(1), &first.submission, d32(1), now);
    expect_success(&shown);
    assert_eq!(inclusion_count(&mut repo, first.community_id), 1);

    // A DIFFERENT body at the same root/epoch/revision (distinct title → distinct
    // digest) collides: the directory shows neither.
    let collide = genuine_listing_for(&first.root, first.coords, 0, 0, now);
    let response = submit(&service, &mut repo, d16(2), &collide, d32(2), now);

    // The refusal is a closed equivocation (the poisoned floor now shows neither).
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::ListingEquivocation { .. }
    ));

    // No new inclusion, but the projection IS invalidated so the censoring
    // propagates consistently.
    assert_eq!(
        inclusion_count(&mut repo, first.community_id),
        1,
        "equivocation appends no inclusion"
    );
    assert_eq!(
        projection_generation(&mut repo),
        2,
        "equivocation invalidates the projection once"
    );
}

// ---------------------------------------------------------------------------
// Superseded: a valid listing that loses to the shown one
// ---------------------------------------------------------------------------

#[test]
fn a_lower_revision_listing_is_superseded_without_durable_state() {
    let now = 3_000_000u64;
    let mut repo = repo();
    let high = genuine_listing(0x42, 0, 1, now); // epoch 0, revision 1
    insert_hosted_community(&mut repo, high.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(high.root.root_id));

    expect_success(&submit(
        &service,
        &mut repo,
        d16(1),
        &high.submission,
        d32(1),
        now,
    ));
    assert_eq!(inclusion_count(&mut repo, high.community_id), 1);

    // A lower revision at the same epoch loses: closed refusal, no durable change.
    let low = genuine_listing_for(&high.root, high.coords, 0, 0, now);
    let response = submit(&service, &mut repo, d16(2), &low, d32(2), now);
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::InvalidListingAuthority
    ));
    assert_eq!(
        inclusion_count(&mut repo, high.community_id),
        1,
        "superseded candidate writes no inclusion"
    );
    assert_eq!(projection_generation(&mut repo), 1);
}

// ---------------------------------------------------------------------------
// Illegal epoch advance → the non-expiry authority-error mapping
// ---------------------------------------------------------------------------

#[test]
fn an_illegal_epoch_advance_maps_to_invalid_listing_authority() {
    let now = 4_000_000u64;
    let mut repo = repo();
    // Fresh floor is epoch 0; jumping straight to epoch 2 is an illegal advance.
    let listing = genuine_listing(0x43, 2, 0, now);
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
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::InvalidListingAuthority
    ));
    assert!(!is_listed(&mut repo, listing.community_id));
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 0);
}

// ---------------------------------------------------------------------------
// verify_submission refusal arms
// ---------------------------------------------------------------------------

#[test]
fn a_tombstone_on_the_ordinary_path_is_refused() {
    // listed == false is the reserved removal operation, never the ordinary path.
    let now = 5_000_000u64;
    let mut repo = repo();
    let root = owned_root(0x44);
    let coords = Coords {
        root_id: root.root_id,
        o: root.root_id,
        c: d32(0x51),
        w: d32(0x52),
        manifest_digest: d32(0x53),
        manifest_version: 3,
    };
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(&root, &coords, now - 1, expiry);
    let payload = listing_payload(&coords, ticket, 0, 0, false, "tomb", now - 1, expiry);
    let payload_bytes = payload.encode_canonical().expect("encode");
    let owner = willow25::entry::SubspaceSecret::from_bytes(&[0x60; 32]);
    let submission = RawListingSubmission {
        listing_item_bytes: root_owned_item(&root, &owner, &payload_bytes),
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
}

#[test]
fn a_listing_whose_root_disagrees_with_the_signing_namespace_is_refused() {
    // The entry is signed in the real root's namespace, but the listing claims a
    // different root_id/o_namespace_id — the "signed by root_id" binding fails.
    let now = 6_000_000u64;
    let mut repo = repo();
    let root = owned_root(0x45);
    let foreign = d32(0xC0);
    let coords = Coords {
        root_id: foreign, // NOT the signing namespace
        o: foreign,
        c: d32(0x61),
        w: d32(0x62),
        manifest_digest: d32(0x63),
        manifest_version: 3,
    };
    // No valid ticket needed: the namespace-binding check fires before the ticket
    // self-check.
    let payload = listing_payload(&coords, vec![], 0, 0, true, "liar", now - 1, now + 3600);
    let payload_bytes = payload.encode_canonical().expect("encode");
    let owner = willow25::entry::SubspaceSecret::from_bytes(&[0x64; 32]);
    let submission = RawListingSubmission {
        listing_item_bytes: root_owned_item(&root, &owner, &payload_bytes),
        delegate_grant: None,
    };
    insert_hosted_community(&mut repo, foreign, now);
    let service = service_with(TestListingAuthority::hosted_root(foreign));

    let response = submit(&service, &mut repo, d16(1), &submission, d32(1), now);
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::InvalidListingAuthority
    ));
    assert!(!is_listed(&mut repo, foreign));
}

#[test]
fn a_root_owned_cap_carrying_a_grant_is_refused() {
    // A zero-delegation (root-owned) capability must NOT carry a delegate grant:
    // the authority class must match the capability shape.
    let now = 7_000_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x46, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    let expiry = now + 24 * 60 * 60;
    let mut submission = listing.submission.clone();
    submission.delegate_grant = Some(root_signed_grant(&listing.root, d32(0), 0, expiry));

    let response = submit(&service, &mut repo, d16(1), &submission, d32(1), now);
    assert!(matches!(
        expect_refusal(&response),
        ControlRefusal::InvalidListingAuthority
    ));
    assert!(!is_listed(&mut repo, listing.community_id));
}

#[test]
fn a_delegated_listing_whose_grant_binds_the_wrong_key_is_refused() {
    // The grant signature verifies (correctly root-signed) but binds a delegate key
    // that is not the entry's author — the exact-binding check refuses it.
    let now = 8_000_000u64;
    let mut repo = repo();
    let mut listing = delegated_listing(0x47, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    let expiry = now + 24 * 60 * 60;
    // A validly-signed grant that binds a DIFFERENT delegate key.
    listing.submission.delegate_grant =
        Some(root_signed_grant(&listing.root, d32(0xAA), 0, expiry));

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

// ---------------------------------------------------------------------------
// ListingError Display / From wiring
// ---------------------------------------------------------------------------

#[test]
fn listing_error_display_and_conversions_are_wired() {
    // Failpoint variant.
    let failpoint = ListingError::Failpoint("inclusion");
    assert!(format!("{failpoint}").contains("inclusion"));

    // Codec variant, via the From<CodecError> conversion.
    let codec: ListingError = CodecError::Malformed.into();
    assert!(matches!(codec, ListingError::Codec(_)));
    assert!(format!("{codec}").contains("codec"));

    // std::error::Error is implemented.
    let as_error: &dyn std::error::Error = &failpoint;
    assert!(as_error.source().is_none());
}
