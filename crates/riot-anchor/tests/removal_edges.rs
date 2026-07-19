//! WU-016 edge coverage for reserved owner-removal: the scheduler's cap / permit
//! refusal arms, the writer-release accounting, the `RemovalError` `Display` / `From`
//! surface, the idempotency-replay fallbacks (reserved-absent → ordinary, undecodable
//! ordinary bytes, and the wholly-missing result), the pre-claim `removal_busy`, every
//! closed `verify_removal` refusal branch, and the free-function helpers
//! (`load_lane_limits`, `reserve_visibility_slot`, `release_abandoned_reservations`,
//! `slot_state_code`). These exercise the branches `removal_reserve.rs` leaves cold.

mod removal_common;

use removal_common::*;

use riot_anchor::removal::{
    load_lane_limits, no_failpoint, release_abandoned_reservations, reserve_visibility_slot,
    slot_state_code, RawRemovalSubmission, RemovalError, RemovalLane, RemovalLaneLimits,
    RemovalScheduler,
};
use riot_anchor::repository::{
    AccountingCeilings, AnchorRepository, IdempotencyClaimState, RemovalSlotState, SlotReservation,
    ACCOUNTING_CLASS_COUNT,
};

use riot_anchor_protocol::codec::{CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{ControlOutcome, ControlRefusal, ControlResponseV1};
use riot_anchor_protocol::records::{AnchorLimitId, CommunityListingV1, ControlOperationKind};

use riot_anchor::sync_service::encode_item;
use riot_core::willow::{encode_capability, encode_entry, Entry, Path, DIRECTORY_COMPONENT};
use willow25::authorisation::WriteCapability;
use willow25::entry::SubspaceSecret;

// ---------------------------------------------------------------------------
// small helpers (mirroring removal_reserve.rs)
// ---------------------------------------------------------------------------

fn refusal(response: &ControlResponseV1) -> Option<&ControlRefusal> {
    match &response.outcome {
        ControlOutcome::Refused(refusal) => Some(refusal),
        ControlOutcome::Success(_) => None,
    }
}

fn scheduler_defaults() -> RemovalScheduler {
    RemovalScheduler::new(RemovalLaneLimits::defaults())
}

/// A small, explicit limit profile: aggregate 8 jobs / 8000 bytes, quarter = 2.
fn small_limits() -> RemovalLaneLimits {
    RemovalLaneLimits {
        aggregate_jobs: 8,
        aggregate_canonical_bytes: 8_000,
        delegated_jobs_per_root: 8,
        direct_verification_permits: 1,
        delegated_verification_permits: 3,
        direct_writer_permits: 1,
        delegated_writer_permits: 1,
    }
}

fn root(seed: u8) -> [u8; 32] {
    d32(seed)
}
fn src(seed: u8) -> [u8; 32] {
    d32(seed.wrapping_add(100))
}

fn owner_secret(seed: u8) -> SubspaceSecret {
    SubspaceSecret::from_bytes(&[seed.wrapping_add(50); 32])
}

/// A tiny ordinary-idempotency ceiling (one row) — as `removal_reserve.rs` uses to
/// exhaust the ordinary partition.
fn ceilings_with_tiny_idempotency() -> AccountingCeilings {
    let mut values = [u64::MAX / 4; ACCOUNTING_CLASS_COUNT];
    // AccountingClass::Idempotency is index 5 (row-count ceiling of one).
    values[5] = 1;
    AccountingCeilings::from_array(values)
}

/// Build a genuine root-owned tombstone, then mutate the decoded listing before it
/// is re-signed — a targeted way to hit a single `verify_removal` cross-check.
fn mutated_tombstone(
    seed: u8,
    now: u64,
    edit: impl FnOnce(&mut CommunityListingV1),
) -> RawRemovalSubmission {
    let root = owned_root(seed);
    let coords = coords_for(&root, seed);
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(&root, &coords, now.saturating_sub(1), expiry);
    let mut payload = listing_payload(
        &coords,
        ticket,
        0,
        1,
        false,
        "Mutated tombstone",
        now.saturating_sub(1),
        expiry,
    );
    edit(&mut payload);
    let payload_bytes = payload
        .encode_canonical()
        .expect("encode tombstone payload");
    let owner = owner_secret(seed);
    let item = root_owned_item(&root, &owner, &payload_bytes);
    RawRemovalSubmission {
        tombstone_item_bytes: item,
        delegate_grant: None,
    }
}

/// Encode a signed root-owned item over an arbitrary path (owner subspace signs a
/// zero-delegation owned cap). Used to place a valid tombstone at a NON-directory
/// path so `is_directory_listing` rejects it.
fn item_with_path(root: &OwnedRoot, owner: &SubspaceSecret, path: Path, payload: &[u8]) -> Vec<u8> {
    let owner_id = owner.corresponding_subspace_id();
    let cap = WriteCapability::new_owned(&root.namespace_secret, owner_id.clone());
    let entry = Entry::builder()
        .namespace_id(root.namespace_secret.corresponding_namespace_id())
        .subspace_id(owner_id)
        .path(path)
        .timestamp(1_000u64)
        .payload(payload)
        .build();
    let authorised = entry
        .into_authorised_entry(&cap, owner)
        .expect("owner authorises the entry");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    encode_item(
        &encode_entry(authorised.entry()),
        &encode_capability(token.capability()),
        &signature.to_bytes(),
        payload,
    )
}

/// Submit `submission` against a fresh repository and assert the single closed
/// `invalid_listing_authority` refusal, with nothing left in the scheduler.
fn expect_invalid_authority(submission: &RawRemovalSubmission, key: [u8; 16], digest: [u8; 32]) {
    let now = 1_000;
    let mut repo = repo();
    let service = service();
    let mut scheduler = scheduler_defaults();
    let response = service
        .submit(
            &mut repo,
            &mut scheduler,
            &key,
            submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("refused, not errored");
    assert!(
        matches!(
            refusal(&response),
            Some(ControlRefusal::InvalidListingAuthority)
        ),
        "expected invalid_listing_authority"
    );
    assert_eq!(scheduler.in_flight_jobs(), 0);
}

// ===========================================================================
// Scheduler getters
// ===========================================================================

#[test]
fn scheduler_limits_getter_returns_configured_limits() {
    let limits = small_limits();
    let scheduler = RemovalScheduler::new(limits);
    assert_eq!(scheduler.limits(), limits);
}

#[test]
fn admit_ticket_writer_granted_flag_tracks_the_scheduler_round() {
    let mut scheduler = scheduler_defaults();
    let ticket = scheduler
        .try_admit(RemovalLane::DirectRoot, root(1), src(1), 10)
        .expect("admit");
    // Before any round the candidate holds a verification permit but no writer.
    assert!(!ticket.writer_granted());
    let granted = scheduler.run_round();
    assert!(
        granted.iter().all(|granted| granted.writer_granted()),
        "run_round only returns candidates that reached a writer"
    );
    assert!(granted
        .iter()
        .any(|granted| granted.lane() == RemovalLane::DirectRoot));
}

// ===========================================================================
// Scheduler cap / permit refusal arms
// ===========================================================================

#[test]
fn second_direct_candidate_for_the_same_root_serializes_to_busy() {
    let mut scheduler = scheduler_defaults();
    scheduler
        .try_admit(RemovalLane::DirectRoot, root(1), src(1), 10)
        .expect("first direct admits");
    let overload = scheduler
        .try_admit(RemovalLane::DirectRoot, root(1), src(2), 10)
        .expect_err("a second direct for the same root is refused");
    assert_eq!(overload.limit_id, AnchorLimitId::QueuedReservedRemovalJobs);
    assert_eq!(scheduler.in_flight_jobs(), 1);
}

#[test]
fn direct_candidate_refused_when_aggregate_job_cap_is_full() {
    // aggregate 3, quarter = 0, so delegates alone can fill the whole aggregate.
    let mut scheduler = RemovalScheduler::new(RemovalLaneLimits {
        aggregate_jobs: 3,
        aggregate_canonical_bytes: 100_000,
        delegated_verification_permits: 3,
        ..small_limits()
    });
    for i in 0..3 {
        scheduler
            .try_admit(RemovalLane::Delegated, root(i), src(i), 10)
            .expect("delegated fills the aggregate");
    }
    assert_eq!(scheduler.in_flight_jobs(), 3);
    let overload = scheduler
        .try_admit(RemovalLane::DirectRoot, root(9), src(9), 10)
        .expect_err("aggregate is full");
    assert_eq!(overload.limit_id, AnchorLimitId::QueuedReservedRemovalJobs);
}

#[test]
fn direct_candidate_refused_when_aggregate_byte_cap_is_full() {
    // aggregate bytes 1000, quarter = 250, delegated byte ceiling = 750.
    let mut scheduler = RemovalScheduler::new(RemovalLaneLimits {
        aggregate_canonical_bytes: 1_000,
        ..small_limits()
    });
    scheduler
        .try_admit(RemovalLane::Delegated, root(1), src(1), 700)
        .expect("delegated 700 bytes within its ceiling");
    let overload = scheduler
        .try_admit(RemovalLane::DirectRoot, root(9), src(9), 400)
        .expect_err("700 + 400 exceeds the aggregate byte cap");
    assert_eq!(
        overload.limit_id,
        AnchorLimitId::QueuedReservedRemovalCanonicalBytes
    );
}

#[test]
fn direct_candidate_refused_when_its_verification_permit_is_in_use() {
    let mut scheduler = RemovalScheduler::new(small_limits());
    scheduler
        .try_admit(RemovalLane::DirectRoot, root(1), src(1), 10)
        .expect("first direct takes the exclusive verification permit");
    // A distinct root: passes the one-per-root, job, and byte gates, but the sole
    // direct verification permit is held.
    let overload = scheduler
        .try_admit(RemovalLane::DirectRoot, root(2), src(2), 10)
        .expect_err("direct verification permit exhausted");
    assert_eq!(
        overload.limit_id,
        AnchorLimitId::ReservedOwnerRemovalVerificationPermits
    );
}

#[test]
fn duplicate_delegated_source_for_a_root_is_busy() {
    let mut scheduler = scheduler_defaults();
    scheduler
        .try_admit(RemovalLane::Delegated, root(1), src(1), 10)
        .expect("first delegated admits");
    let overload = scheduler
        .try_admit(RemovalLane::Delegated, root(1), src(1), 10)
        .expect_err("same (root, source) is refused");
    assert_eq!(overload.limit_id, AnchorLimitId::QueuedReservedRemovalJobs);
    assert_eq!(scheduler.in_flight_jobs(), 1);
}

#[test]
fn ninth_delegated_source_for_a_root_hits_the_per_root_cap() {
    // Enough verification permits and aggregate headroom that the eight-per-root
    // structural cap is the binding constraint.
    let mut scheduler = RemovalScheduler::new(RemovalLaneLimits {
        delegated_verification_permits: 16,
        ..RemovalLaneLimits::defaults()
    });
    for i in 0..8 {
        scheduler
            .try_admit(RemovalLane::Delegated, root(1), src(i), 10)
            .expect("eight delegated per root");
    }
    let overload = scheduler
        .try_admit(RemovalLane::Delegated, root(1), src(8), 10)
        .expect_err("a ninth delegate for the root is refused");
    assert_eq!(overload.limit_id, AnchorLimitId::QueuedReservedRemovalJobs);
    assert_eq!(scheduler.in_flight_jobs(), 8);
}

// ===========================================================================
// Writer-release accounting
// ===========================================================================

#[test]
fn releasing_a_granted_direct_candidate_frees_its_writer_permit() {
    let mut scheduler = scheduler_defaults();
    scheduler
        .try_admit(RemovalLane::DirectRoot, root(1), src(1), 10)
        .expect("direct admits");
    let granted = scheduler.run_round();
    let ticket = *granted
        .iter()
        .find(|ticket| ticket.lane() == RemovalLane::DirectRoot)
        .expect("direct reaches its writer");
    assert!(ticket.writer_granted());
    scheduler.release(&ticket);
    assert_eq!(scheduler.in_flight_jobs(), 0);
    // The direct writer permit was freed: a fresh direct candidate reaches it again.
    scheduler
        .try_admit(RemovalLane::DirectRoot, root(2), src(2), 10)
        .expect("re-admit after release");
    assert!(scheduler
        .run_round()
        .iter()
        .any(|ticket| ticket.lane() == RemovalLane::DirectRoot && ticket.writer_granted()));
}

#[test]
fn releasing_a_granted_delegated_candidate_frees_its_writer_and_prunes_the_root() {
    let mut scheduler = scheduler_defaults();
    scheduler
        .try_admit(RemovalLane::Delegated, root(1), src(1), 10)
        .expect("delegated admits");
    let granted = scheduler.run_round();
    let ticket = *granted
        .iter()
        .find(|ticket| ticket.lane() == RemovalLane::Delegated)
        .expect("delegated reaches its writer");
    assert!(ticket.writer_granted());
    scheduler.release(&ticket);
    // Releasing the root's only source empties and prunes the per-root entry.
    assert_eq!(scheduler.in_flight_jobs(), 0);
    // The delegated writer permit is available again.
    scheduler
        .try_admit(RemovalLane::Delegated, root(2), src(2), 10)
        .expect("re-admit after release");
    assert!(scheduler
        .run_round()
        .iter()
        .any(|ticket| ticket.lane() == RemovalLane::Delegated && ticket.writer_granted()));
}

#[test]
fn run_round_on_an_empty_scheduler_grants_nothing() {
    let mut scheduler = scheduler_defaults();
    // Both lane loops break immediately on their empty queues.
    assert!(scheduler.run_round().is_empty());
}

// ===========================================================================
// RemovalError Display + From
// ===========================================================================

#[test]
fn removal_error_from_codec_and_displays_it() {
    let error = RemovalError::from(CodecError::Malformed);
    let rendered = error.to_string();
    assert!(rendered.contains("codec"), "{rendered}");
}

#[test]
fn removal_error_from_repository_and_displays_it() {
    let now = 1_000;
    let mut repo = AnchorRepository::open_in_memory_with_ceilings(ceilings_with_tiny_idempotency())
        .expect("open");
    {
        let mut tx = repo.begin().expect("begin");
        tx.claim_idempotency(
            &d32(1),
            &d16(1),
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        )
        .expect("first claim fills the ceiling");
        tx.commit().expect("commit");
    }
    let mut tx = repo.begin().expect("begin");
    let repo_error = tx
        .claim_idempotency(
            &d32(2),
            &d16(2),
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        )
        .expect_err("ordinary idempotency is exhausted");
    drop(tx);
    let error = RemovalError::from(repo_error);
    let rendered = error.to_string();
    assert!(rendered.contains("repository"), "{rendered}");
}

#[test]
fn removal_error_failpoint_displays_its_label() {
    let now = 1_000;
    let tomb = genuine_tombstone(80, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0x80);
    list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();
    let mut fp = |label: &str| label == "commit";
    let error = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(1),
            &tomb.submission,
            &digest,
            now,
            &mut fp,
        )
        .expect_err("failpoint aborts before commit");
    let rendered = error.to_string();
    assert!(rendered.contains("failpoint"), "{rendered}");
}

// ===========================================================================
// Idempotency-replay fallbacks
// ===========================================================================

#[test]
fn replay_returns_ordinary_stored_result_when_no_reserved_result_exists() {
    let now = 1_000;
    let mut repo = repo();
    let key = d16(0x11);
    let digest = d32(0x11);
    // An ordinary op previously stored a valid control response under this key.
    let stored = ControlResponseV1 {
        kind: ControlOperationKind::SubmitListing,
        outcome: ControlOutcome::Refused(ControlRefusal::AlreadyUnlisted),
    };
    let stored_bytes = stored.encode_canonical().expect("encode ordinary response");
    {
        let mut tx = repo.begin().expect("begin");
        tx.claim_idempotency(
            &digest,
            &key,
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        )
        .expect("ordinary claim");
        tx.store_ordinary_result(&digest, &stored_bytes)
            .expect("ordinary result");
        tx.commit().expect("commit");
    }
    // A removal under the SAME key + digest replays the ordinary bytes verbatim.
    let tomb = genuine_tombstone(50, 0, 1, now);
    let service = service();
    let mut scheduler = scheduler_defaults();
    let replayed = service
        .submit(
            &mut repo,
            &mut scheduler,
            &key,
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("replay");
    assert!(matches!(
        refusal(&replayed),
        Some(ControlRefusal::AlreadyUnlisted)
    ));
    assert_eq!(replayed.encode_canonical().unwrap(), stored_bytes);
}

#[test]
fn replay_with_undecodable_ordinary_result_is_a_codec_error() {
    let now = 1_000;
    let mut repo = repo();
    let key = d16(0x12);
    let digest = d32(0x12);
    {
        let mut tx = repo.begin().expect("begin");
        tx.claim_idempotency(
            &digest,
            &key,
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        )
        .expect("ordinary claim");
        tx.store_ordinary_result(&digest, b"not-a-canonical-response")
            .expect("ordinary result");
        tx.commit().expect("commit");
    }
    let tomb = genuine_tombstone(51, 0, 1, now);
    let service = service();
    let mut scheduler = scheduler_defaults();
    let error = service
        .submit(
            &mut repo,
            &mut scheduler,
            &key,
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect_err("undecodable stored bytes surface a codec error");
    assert!(matches!(error, RemovalError::Codec(_)), "{error:?}");
}

#[test]
fn replay_with_no_stored_result_in_either_partition_is_malformed() {
    let now = 1_000;
    let mut repo = repo();
    let key = d16(0x13);
    let digest = d32(0x13);
    {
        let mut tx = repo.begin().expect("begin");
        // A claimed key with NO stored result in either the reserved or ordinary
        // partition — the replay branch has nothing to return.
        tx.claim_idempotency(
            &digest,
            &key,
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        )
        .expect("claim with no result");
        tx.commit().expect("commit");
    }
    let tomb = genuine_tombstone(52, 0, 1, now);
    let service = service();
    let mut scheduler = scheduler_defaults();
    let error = service
        .submit(
            &mut repo,
            &mut scheduler,
            &key,
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect_err("a claimed-but-resultless replay is malformed");
    assert!(
        matches!(error, RemovalError::Codec(CodecError::Malformed)),
        "{error:?}"
    );
}

// ===========================================================================
// Pre-claim removal_busy
// ===========================================================================

#[test]
fn scheduler_overload_during_submit_yields_removal_busy_with_no_durable_trace() {
    let now = 1_000;
    let tomb = genuine_tombstone(67, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0x67);
    list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();
    // Pre-occupy the direct lane for this exact root so admission serializes.
    scheduler
        .try_admit(RemovalLane::DirectRoot, tomb.community_id, d32(0xAA), 10)
        .expect("pre-occupy the root's direct lane");
    let response = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(1),
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("busy refusal");
    match refusal(&response) {
        Some(ControlRefusal::RemovalBusy {
            limit_id,
            retry_after_seconds,
        }) => {
            assert_eq!(*limit_id, AnchorLimitId::QueuedReservedRemovalJobs);
            assert_eq!(*retry_after_seconds, 60);
        }
        other => panic!("expected removal_busy, got {other:?}"),
    }
    // No durable trace: no reserved result, the pre-occupation is the only job.
    let tx = repo.begin().expect("begin");
    assert!(tx.reserved_result(&digest).expect("reserved").is_none());
    drop(tx);
    assert_eq!(scheduler.in_flight_jobs(), 1);
}

// ===========================================================================
// verify_removal closed refusal branches
// ===========================================================================

#[test]
fn a_listed_true_record_is_not_a_tombstone() {
    let submission = mutated_tombstone(60, 1_000, |listing| listing.listed = true);
    expect_invalid_authority(&submission, d16(1), d32(0x60));
}

#[test]
fn entry_namespace_that_disagrees_with_the_listing_is_refused() {
    let submission = mutated_tombstone(61, 1_000, |listing| listing.root_id = d32(0xEE));
    expect_invalid_authority(&submission, d16(1), d32(0x61));
}

#[test]
fn embedded_ticket_coordinate_mismatch_is_refused() {
    let submission = mutated_tombstone(62, 1_000, |listing| listing.manifest_version = 99);
    expect_invalid_authority(&submission, d16(1), d32(0x62));
}

#[test]
fn a_tombstone_off_the_directory_listing_path_is_refused() {
    let now = 1_000;
    let seed = 63u8;
    let root = owned_root(seed);
    let coords = coords_for(&root, seed);
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(&root, &coords, now - 1, expiry);
    let payload = listing_payload(&coords, ticket, 0, 1, false, "off-path", now - 1, expiry);
    let payload_bytes = payload.encode_canonical().expect("encode payload");
    let owner = owner_secret(seed);
    let path = Path::from_slices(&[DIRECTORY_COMPONENT]).expect("directory-only path");
    let item = item_with_path(&root, &owner, path, &payload_bytes);
    let submission = RawRemovalSubmission {
        tombstone_item_bytes: item,
        delegate_grant: None,
    };
    expect_invalid_authority(&submission, d16(1), d32(0x63));
}

#[test]
fn a_delegated_capability_without_a_grant_is_refused() {
    let mut tomb = delegated_tombstone(64, 0, 1, 1_000);
    // Drop the grant: a delegated (non-zero-delegation) cap on the direct-root lane.
    tomb.submission.delegate_grant = None;
    expect_invalid_authority(&tomb.submission, d16(1), d32(0x64));
}

#[test]
fn a_grant_over_a_root_owned_capability_is_refused() {
    let now = 1_000;
    let mut tomb = genuine_tombstone(65, 0, 1, now);
    let expiry = now + 24 * 60 * 60;
    // Attach a grant to a zero-delegation (root-owned) cap: the delegated lane
    // requires a delegated cap.
    tomb.submission.delegate_grant = Some(root_signed_grant(&tomb.root, d32(0x01), 0, expiry));
    expect_invalid_authority(&tomb.submission, d16(1), d32(0x65));
}

#[test]
fn a_validly_signed_grant_with_mismatched_fields_is_refused() {
    let now = 1_000;
    let seed = 66u8;
    let mut tomb = delegated_tombstone(seed, 0, 1, now);
    let expiry = now + 24 * 60 * 60;
    let delegate_id =
        SubspaceSecret::from_bytes(&[seed.wrapping_add(70); 32]).corresponding_subspace_id();
    // A correctly root-signed grant that verifies, but binds the WRONG listing
    // epoch (1 ≠ the tombstone's epoch 0): the field cross-check rejects it.
    tomb.submission.delegate_grant = Some(root_signed_grant(
        &tomb.root,
        *delegate_id.as_bytes(),
        1,
        expiry,
    ));
    expect_invalid_authority(&tomb.submission, d16(1), d32(0x66));
}

// ===========================================================================
// Free-function helpers
// ===========================================================================

#[test]
fn load_lane_limits_splits_the_seeded_reserve_permits() {
    let mut repo = repo();
    let limits = load_lane_limits(&mut repo, 128, 2_048).expect("load lane limits");
    assert_eq!(limits.aggregate_jobs, 128);
    assert_eq!(limits.aggregate_canonical_bytes, 2_048);
    assert_eq!(limits.delegated_jobs_per_root, 8);
    // Default seeds: 4 verification permits (1 direct + 3 delegated), 2 writers (1 + 1).
    assert_eq!(limits.direct_verification_permits, 1);
    assert_eq!(limits.delegated_verification_permits, 3);
    assert_eq!(limits.direct_writer_permits, 1);
    assert_eq!(limits.delegated_writer_permits, 1);
}

#[test]
fn reserve_visibility_slot_wrapper_reserves_a_free_slot() {
    let now = 1_000;
    let mut repo = repo();
    let community = d32(0x71);
    let root_key = d32(0x71);
    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community, now)
        .expect("insert community");
    let reservation =
        reserve_visibility_slot(&mut tx, &community, &root_key, &d32(1), now).expect("reserve");
    assert!(matches!(reservation, SlotReservation::Reserved(_)));
    tx.commit().expect("commit");
}

#[test]
fn release_abandoned_reservations_frees_slots_without_a_live_listing() {
    let now = 1_000;
    let mut repo = repo();
    let community = d32(0x72);
    let root_key = d32(0x72);
    {
        let mut tx = repo.begin().expect("begin");
        tx.insert_community(&community, now)
            .expect("insert community");
        // Reserve a slot but NEVER insert a listing — an abandoned reservation.
        let reservation =
            reserve_visibility_slot(&mut tx, &community, &root_key, &d32(1), now).expect("reserve");
        assert!(matches!(reservation, SlotReservation::Reserved(_)));
        tx.commit().expect("commit");
    }
    let released = release_abandoned_reservations(&mut repo).expect("release");
    assert_eq!(released, 1);
    // Idempotent: a second startup pass finds nothing to release.
    assert_eq!(
        release_abandoned_reservations(&mut repo).expect("second release"),
        0
    );
}

#[test]
fn slot_state_code_matches_the_state_discriminant() {
    for state in [
        RemovalSlotState::Free,
        RemovalSlotState::ReservedForListedRoot,
        RemovalSlotState::Committed,
        RemovalSlotState::Terminal,
    ] {
        assert_eq!(slot_state_code(state), state.to_code());
    }
}
