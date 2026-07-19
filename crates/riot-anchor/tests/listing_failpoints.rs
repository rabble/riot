//! WU-015B — crash safety. A failpoint injected at every durable mutation of an
//! accepted `SubmitListing` transaction must leave state WHOLLY ABSENT (never
//! partial); a clean run then commits exactly one inclusion, one projection
//! invalidation, and a byte-identical, replayable receipt/terminal result.

mod listing_common;

use listing_common::*;

use riot_anchor::listing::{ListingError, RawListingSubmission, SubmitListingService};
use riot_anchor::repository::AnchorRepository;

use riot_anchor_protocol::control::{ControlOutcome, ControlResponseV1, ControlSuccess};
use riot_anchor_protocol::records::ControlOperationKind;

/// Every durable mutation the accept path performs, in order. A failpoint at any
/// one must roll back the entire transaction.
const DURABLE_FAILPOINTS: &[&str] = &[
    "listing_state",
    "inclusion",
    "projection",
    "receipt",
    "terminal",
    "commit",
];

fn submit_fp(
    service: &SubmitListingService<TestListingAuthority, TestSigner>,
    repo: &mut AnchorRepository,
    key: [u8; 16],
    submission: &RawListingSubmission,
    digest: [u8; 32],
    now: u64,
    target: &str,
) -> Result<ControlResponseV1, ListingError> {
    let mut fp = |label: &str| label == target;
    service.submit(repo, &key, submission, &digest, now, &mut fp)
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

fn idempotency_claimed(repo: &mut AnchorRepository, key: [u8; 16]) -> bool {
    let tx = repo.begin().expect("begin");
    let claimed = tx.lookup_idempotency(&key).expect("lookup").is_some();
    drop(tx);
    claimed
}

#[test]
fn every_durable_failpoint_leaves_state_wholly_absent() {
    let now = 1_000_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x30, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    for target in DURABLE_FAILPOINTS {
        let result = submit_fp(
            &service,
            &mut repo,
            d16(1),
            &listing.submission,
            d32(1),
            now,
            target,
        );
        match result {
            Err(ListingError::Failpoint(label)) => assert_eq!(&label, target),
            other => panic!("failpoint {target} should abort, got {other:?}"),
        }

        // Wholly absent: no listing, no inclusion, projection untouched, key unclaimed.
        assert!(
            !is_listed(&mut repo, listing.community_id),
            "no listing @ {target}"
        );
        assert_eq!(
            inclusion_count(&mut repo, listing.community_id),
            0,
            "no inclusion @ {target}"
        );
        assert_eq!(
            projection_generation(&mut repo),
            0,
            "projection intact @ {target}"
        );
        assert!(
            !idempotency_claimed(&mut repo, d16(1)),
            "key unclaimed @ {target}"
        );
    }
}

#[test]
fn clean_run_after_failpoints_commits_exactly_once_and_replays_identically() {
    let now = 2_000_000u64;
    let mut repo = repo();
    let listing = genuine_listing(0x31, 0, 0, now);
    insert_hosted_community(&mut repo, listing.community_id, now);
    let service = service_with(TestListingAuthority::hosted_root(listing.root.root_id));

    // Trip the commit failpoint first — still wholly absent.
    let aborted = submit_fp(
        &service,
        &mut repo,
        d16(1),
        &listing.submission,
        d32(1),
        now,
        "commit",
    );
    assert!(matches!(aborted, Err(ListingError::Failpoint("commit"))));
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 0);

    // Clean run: exactly one inclusion, one projection invalidation.
    let mut clean = |_: &str| false;
    let response = service
        .submit(
            &mut repo,
            &d16(1),
            &listing.submission,
            &d32(1),
            now,
            &mut clean,
        )
        .expect("clean submit commits");
    let receipt = expect_receipt(&response);
    assert_eq!(receipt.body.feed_coordinate, 1);
    assert_eq!(inclusion_count(&mut repo, listing.community_id), 1);
    assert_eq!(projection_generation(&mut repo), 1);

    // Byte-identical receipt/terminal replay via the same key.
    let replay = service
        .submit(
            &mut repo,
            &d16(1),
            &listing.submission,
            &d32(1),
            now + 9,
            &mut clean,
        )
        .expect("replay");
    assert_eq!(response, replay, "terminal replay is byte-identical");
    assert_eq!(
        inclusion_count(&mut repo, listing.community_id),
        1,
        "replay appends no second inclusion"
    );
    assert_eq!(projection_generation(&mut repo), 1);
}

fn expect_receipt(response: &ControlResponseV1) -> riot_anchor_protocol::records::ListingReceiptV1 {
    assert_eq!(response.kind, ControlOperationKind::SubmitListing);
    match &response.outcome {
        ControlOutcome::Success(ControlSuccess::SubmitListing(receipt)) => (**receipt).clone(),
        other => panic!("expected success, got {other:?}"),
    }
}
