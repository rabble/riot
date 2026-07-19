//! WU-016 — reserved owner-removal: the fair two-lane admission scheduler
//! (protected direct quarter, aggregate caps, reserved verifier/writer permits,
//! two-round guarantee, pre-claim overload), the exact per-root two-slot / relist
//! / expiry rules, the global idempotency index spanning the ordinary and
//! reserved classes without disclosure, `already_unlisted`, ack-before-compaction,
//! invalid-candidate saturation, and the maximum-record removal that completes via
//! the reserved partition even when ordinary idempotency capacity is exhausted.

mod removal_common;

use removal_common::*;

use riot_anchor::removal::{no_failpoint, RemovalLane, RemovalLaneLimits, RemovalScheduler};
use riot_anchor::repository::{
    AccountingCeilings, AnchorRepository, IdempotencyClaimState, RemovalSlotState, SlotReservation,
    ACCOUNTING_CLASS_COUNT,
};

use riot_anchor_protocol::control::{ControlOutcome, ControlRefusal, ControlResponseV1};
use riot_anchor_protocol::records::AnchorLimitId;

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------

fn is_success(response: &ControlResponseV1) -> bool {
    matches!(response.outcome, ControlOutcome::Success(_))
}

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

// ===========================================================================
// Part 1: the fair, two-lane scheduler
// ===========================================================================

#[test]
fn pre_claim_overload_returns_removal_busy_with_no_durable_trace() {
    // A pure-in-memory refusal leaves nothing durable by construction. The three
    // delegated verification permits are a hard pre-claim gate.
    let mut scheduler = RemovalScheduler::new(small_limits());
    for i in 0..3 {
        scheduler
            .try_admit(RemovalLane::Delegated, root(i), src(i), 10)
            .expect("delegated verification permit");
    }
    let overload = scheduler
        .try_admit(RemovalLane::Delegated, root(9), src(9), 10)
        .expect_err("delegated verification permits exhausted → overload");
    assert_eq!(
        overload.limit_id,
        AnchorLimitId::ReservedOwnerRemovalVerificationPermits
    );
    // The refused candidate consumed nothing.
    assert_eq!(scheduler.in_flight_jobs(), 3);
}

#[test]
fn protected_quarter_is_reserved_for_direct_root() {
    // Enough verification permits that the JOB quarter is the binding constraint.
    let mut scheduler = RemovalScheduler::new(RemovalLaneLimits {
        delegated_verification_permits: 16,
        ..small_limits()
    });
    // Delegates are confined to the un-reserved three quarters (6 of 8).
    for i in 0..6 {
        scheduler
            .try_admit(RemovalLane::Delegated, root(i), src(i), 10)
            .expect("delegated within 3/4 ceiling");
    }
    // A seventh delegate is refused even though the aggregate has 2 free slots:
    // those two are the protected direct quarter delegates can never consume.
    let overload = scheduler
        .try_admit(RemovalLane::Delegated, root(50), src(50), 10)
        .expect_err("delegate cannot consume the protected quarter");
    assert_eq!(overload.limit_id, AnchorLimitId::QueuedReservedRemovalJobs);
    // But a direct-root candidate draws freely on the reserved quarter.
    scheduler
        .try_admit(RemovalLane::DirectRoot, root(60), src(60), 10)
        .expect("direct draws the reserved quarter");
    assert_eq!(scheduler.in_flight_jobs(), 7);
}

#[test]
fn aggregate_byte_cap_bounds_both_lanes() {
    let mut scheduler = RemovalScheduler::new(RemovalLaneLimits {
        aggregate_canonical_bytes: 1_000,
        ..small_limits()
    });
    // Delegated byte ceiling = 1000 - quarter(250) = 750.
    scheduler
        .try_admit(RemovalLane::Delegated, root(1), src(1), 400)
        .expect("first delegated 400 bytes");
    let overload = scheduler
        .try_admit(RemovalLane::Delegated, root(2), src(2), 400)
        .expect_err("800 > 750 delegated byte ceiling");
    assert_eq!(
        overload.limit_id,
        AnchorLimitId::QueuedReservedRemovalCanonicalBytes
    );
    // A direct candidate may draw on the reserved byte quarter.
    scheduler
        .try_admit(RemovalLane::DirectRoot, root(3), src(3), 300)
        .expect("direct draws its reserved byte quarter");
}

#[test]
fn reserved_verifier_and_writer_permits_are_available_to_direct_root() {
    let mut scheduler = RemovalScheduler::new(small_limits());
    // Exhaust the three delegated verification permits.
    for i in 0..3 {
        scheduler
            .try_admit(RemovalLane::Delegated, root(i), src(i), 10)
            .expect("delegated verify permit");
    }
    let overload = scheduler
        .try_admit(RemovalLane::Delegated, root(9), src(9), 10)
        .expect_err("delegated verification permits exhausted");
    assert_eq!(
        overload.limit_id,
        AnchorLimitId::ReservedOwnerRemovalVerificationPermits
    );
    // The direct-root lane still has its own exclusive verification permit.
    let direct = scheduler
        .try_admit(RemovalLane::DirectRoot, root(20), src(20), 10)
        .expect("direct verify permit is exclusive");
    // And its own exclusive writer: run_round grants the direct writer even while
    // the delegated writer is busy.
    let granted = scheduler.run_round();
    assert!(
        granted.iter().any(|t| t.lane() == RemovalLane::DirectRoot),
        "direct-root reaches its exclusive writer"
    );
    scheduler.release(&direct);
}

#[test]
fn admitted_direct_root_reaches_verification_within_two_rounds() {
    let mut scheduler = RemovalScheduler::new(small_limits());
    // Continuous valid delegated candidates flood the delegated queue (churn).
    for i in 0..3 {
        scheduler
            .try_admit(RemovalLane::Delegated, root(30 + i), src(30 + i), 10)
            .expect("delegated churn");
    }
    // A single admitted direct-root candidate.
    let direct = scheduler
        .try_admit(RemovalLane::DirectRoot, root(1), src(1), 10)
        .expect("direct admitted");
    // Regardless of the delegated churn, it reaches its exclusive writer within
    // two scheduler rounds.
    let mut granted_round = None;
    for round in 1..=2 {
        if scheduler
            .run_round()
            .iter()
            .any(|t| t.lane() == RemovalLane::DirectRoot)
        {
            granted_round = Some(round);
            break;
        }
    }
    assert!(
        granted_round.is_some(),
        "admitted direct-root candidate reaches verification within two rounds"
    );
    scheduler.release(&direct);
}

#[test]
fn delegated_lane_saturation_never_blocks_direct_root() {
    let mut scheduler = RemovalScheduler::new(small_limits());
    // Saturate the delegated lane entirely (jobs + verification permits).
    for i in 0..6 {
        let _ = scheduler.try_admit(RemovalLane::Delegated, root(i), src(i), 10);
    }
    // A valid direct-root candidate still admits and reaches a writer.
    let direct = scheduler
        .try_admit(RemovalLane::DirectRoot, root(70), src(70), 10)
        .expect("direct unaffected by delegated saturation");
    let granted = scheduler.run_round();
    assert!(granted.iter().any(|t| t.lane() == RemovalLane::DirectRoot));
    scheduler.release(&direct);
}

// ===========================================================================
// Part 2: durable reserved-removal service
// ===========================================================================

#[test]
fn removal_terminalizes_slot_deletes_visibility_and_returns_receipt() {
    let now = 1_000;
    let tomb = genuine_tombstone(7, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0xA1);
    let slot = list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");

    let service = service();
    let mut scheduler = scheduler_defaults();
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
        .expect("removal");
    assert!(is_success(&response), "owner removal succeeds");

    // The reserved slot is Terminal; visibility is gone; the result is retained.
    let tx = repo.begin().expect("begin");
    let loaded = tx.load_removal_slot(slot).expect("load").expect("slot");
    assert_eq!(loaded.state, RemovalSlotState::Terminal);
    assert!(loaded.terminal_expires_at.is_some());
    assert!(tx
        .reserved_slot_for_community(&tomb.community_id)
        .expect("lookup")
        .is_none());
    assert!(tx
        .current_listing(&tomb.community_id)
        .expect("listing")
        .is_none());
    assert!(tx
        .reserved_result(&digest)
        .expect("reserved result")
        .is_some());
    // The scheduler released its permit after completion.
    assert_eq!(scheduler.in_flight_jobs(), 0);
}

#[test]
fn same_key_replay_returns_byte_identical_result() {
    let now = 1_000;
    let tomb = genuine_tombstone(8, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0xB2);
    list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();

    let first = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(2),
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("first removal");
    let second = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(2),
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("replay");
    assert!(is_success(&first) && is_success(&second));
    // Byte-identical replay (no second inclusion).
    use riot_anchor_protocol::codec::CanonicalRecord;
    assert_eq!(
        first.encode_canonical().unwrap(),
        second.encode_canonical().unwrap()
    );
}

#[test]
fn same_key_changed_body_is_idempotency_conflict_without_disclosure() {
    let now = 1_000;
    let tomb = genuine_tombstone(9, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0xC3);
    list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();

    service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(3),
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("first removal");
    // Same key, different request digest → conflict, no stored bytes revealed.
    let conflict = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(3),
            &tomb.submission,
            &d32(0xC4),
            now,
            &mut no_failpoint,
        )
        .expect("conflict");
    assert!(matches!(
        refusal(&conflict),
        Some(ControlRefusal::IdempotencyConflict)
    ));
}

#[test]
fn second_distinct_key_observes_already_unlisted() {
    let now = 1_000;
    let tomb = genuine_tombstone(10, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0xD5);
    list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();

    service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(5),
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("first removal");
    // A DIFFERENT key for the same, now-unlisted community: bounded already_unlisted.
    let loser = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(6),
            &tomb.submission,
            &d32(0xD6),
            now,
            &mut no_failpoint,
        )
        .expect("second key");
    assert!(matches!(
        refusal(&loser),
        Some(ControlRefusal::AlreadyUnlisted)
    ));
    // No slot was consumed and no permit retained.
    assert_eq!(scheduler.in_flight_jobs(), 0);
}

#[test]
fn invalid_tombstone_is_refused_and_never_occupies_the_reserved_queue() {
    let now = 1_000;
    let mut tomb = genuine_tombstone(11, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0xE7);
    list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    // Forge the entry signature: verification fails, the candidate never admits.
    tomb.submission.tombstone_item_bytes =
        forge_item_signature(&tomb.submission.tombstone_item_bytes);
    let service = service();
    let mut scheduler = scheduler_defaults();

    let refused = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(7),
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("refused");
    assert!(matches!(
        refusal(&refused),
        Some(ControlRefusal::InvalidListingAuthority)
    ));
    // Invalid candidates release only prefilter memory: no permit, no slot change.
    assert_eq!(scheduler.in_flight_jobs(), 0);
    let tx = repo.begin().expect("begin");
    assert!(tx
        .reserved_slot_for_community(&tomb.community_id)
        .expect("lookup")
        .is_some());
}

#[test]
fn invalid_candidate_saturation_does_not_starve_a_valid_removal() {
    let now = 1_000;
    let mut repo = repo();
    let service = service();
    let mut scheduler = scheduler_defaults();
    // A flood of invalid tombstones for a listed community.
    let mut tomb = genuine_tombstone(12, 0, 1, now);
    let digest = d32(0xF0);
    list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    let forged = forge_item_signature(&tomb.submission.tombstone_item_bytes);
    for i in 0..16u8 {
        let mut bad = tomb.submission.clone();
        bad.tombstone_item_bytes = forged.clone();
        let r = service
            .submit(
                &mut repo,
                &mut scheduler,
                &d16(100 + i),
                &bad,
                &d32(0x10 + i),
                now,
                &mut no_failpoint,
            )
            .expect("refused");
        assert!(matches!(
            refusal(&r),
            Some(ControlRefusal::InvalidListingAuthority)
        ));
    }
    assert_eq!(scheduler.in_flight_jobs(), 0);
    // The genuine removal still succeeds.
    tomb.submission.tombstone_item_bytes = genuine_tombstone(12, 0, 1, now)
        .submission
        .tombstone_item_bytes;
    let ok = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(200),
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("valid removal");
    assert!(is_success(&ok));
}

#[test]
fn delegated_owner_removal_succeeds() {
    let now = 1_000;
    let tomb = delegated_tombstone(13, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0x21);
    list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();
    let response = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(21),
            &tomb.submission,
            &digest,
            now,
            &mut no_failpoint,
        )
        .expect("delegated removal");
    assert!(is_success(&response));
}

// ---------------------------------------------------------------------------
// per-root two-slot / relist window / expiry release
// ---------------------------------------------------------------------------

#[test]
fn two_relist_cycles_then_third_is_blocked_with_relist_window() {
    let now = 1_000;
    let seed = 30u8;
    let mut repo = repo();
    let service = service();
    let mut scheduler = scheduler_defaults();

    // Cycle 1: list → remove (slot A → Terminal, expires now + 24h).
    let tomb1 = genuine_tombstone(seed, 0, 1, now);
    let coords = tomb1.coords;
    let slot_a = list_and_reserve(&mut repo, &coords, &d32(1), now).expect("reserve A");
    service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(1),
            &tomb1.submission,
            &d32(1),
            now,
            &mut no_failpoint,
        )
        .expect("remove 1");

    // Cycle 2: relist (owns 1 → reserve B) → remove.
    let slot_b = list_and_reserve(&mut repo, &coords, &d32(2), now).expect("reserve B");
    assert_ne!(slot_a, slot_b, "relisting acquires a fresh free slot");
    let tomb2 = genuine_tombstone(seed, 1, 1, now);
    service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(2),
            &tomb2.submission,
            &d32(2),
            now,
            &mut no_failpoint,
        )
        .expect("remove 2");

    // Cycle 3: relisting — not a subsequent removal — is BLOCKED (root owns two
    // retained Terminal slots) with the earliest blocking expiry.
    let mut tx = repo.begin().expect("begin");
    let reservation = tx
        .reserve_visibility_slot(&coords.o, &coords.root_id, &d32(3), now)
        .expect("reserve");
    match reservation {
        SlotReservation::Blocked { earliest_retry_at } => {
            assert_eq!(earliest_retry_at, Some(now + 24 * 60 * 60));
        }
        SlotReservation::Reserved(_) => panic!("third cycle must be blocked"),
    }
    drop(tx);

    // Expiry release: once both Terminals expire, a relist can reserve again.
    let later = now + 24 * 60 * 60 + 1;
    let mut tx = repo.begin().expect("begin");
    let reservation = tx
        .reserve_visibility_slot(&coords.o, &coords.root_id, &d32(4), later)
        .expect("reserve after expiry");
    assert!(matches!(reservation, SlotReservation::Reserved(_)));
}

#[test]
fn reservation_race_gives_each_listed_root_its_own_slot() {
    // Two different roots reserving concurrently each get a distinct slot; no root
    // can consume another root's reserved capacity.
    let now = 1_000;
    let mut repo = repo();
    let a = genuine_tombstone(40, 0, 1, now);
    let b = genuine_tombstone(41, 0, 1, now);
    let slot_a = list_and_reserve(&mut repo, &a.coords, &d32(1), now).expect("A");
    let slot_b = list_and_reserve(&mut repo, &b.coords, &d32(2), now).expect("B");
    assert_ne!(slot_a, slot_b);
}

// ---------------------------------------------------------------------------
// one global idempotency index spans ordinary + reserved without disclosure
// ---------------------------------------------------------------------------

#[test]
fn ordinary_and_reserved_keys_coexist_without_collision_or_disclosure() {
    let now = 1_000;
    let mut repo = repo();

    // An ordinary key with its own stored bytes.
    {
        let mut tx = repo.begin().expect("begin");
        tx.claim_idempotency(
            &d32(0x01),
            &d16(1),
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        )
        .expect("ordinary claim");
        tx.store_ordinary_result(&d32(0x01), b"ordinary-bytes")
            .expect("ordinary result");
        tx.commit().expect("commit");
    }

    // A reserved removal under a DIFFERENT key.
    let tomb = genuine_tombstone(42, 0, 1, now);
    list_and_reserve(&mut repo, &tomb.coords, &d32(0x02), now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();
    service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(2),
            &tomb.submission,
            &d32(0x02),
            now,
            &mut no_failpoint,
        )
        .expect("reserved removal");

    // Each key replays only its own bytes; neither partition can read the other.
    let tx = repo.begin().expect("begin");
    assert_eq!(
        tx.ordinary_result(&d32(0x01)).expect("ord").as_deref(),
        Some(&b"ordinary-bytes"[..])
    );
    assert!(tx.reserved_result(&d32(0x01)).expect("res").is_none());
    assert!(tx.reserved_result(&d32(0x02)).expect("res").is_some());
    assert!(tx.ordinary_result(&d32(0x02)).expect("ord").is_none());
}

#[test]
fn reserved_removal_under_key_first_used_by_ordinary_op_is_conflict() {
    // Lookup precedence applies across classes: a key already used by an ordinary
    // op, replayed with a different body, is `idempotency_conflict` — no oracle.
    let now = 1_000;
    let mut repo = repo();
    {
        let mut tx = repo.begin().expect("begin");
        tx.claim_idempotency(
            &d32(0x30),
            &d16(9),
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        )
        .expect("ordinary claim");
        tx.store_ordinary_result(&d32(0x30), b"secret")
            .expect("ord");
        tx.commit().expect("commit");
    }
    let tomb = genuine_tombstone(43, 0, 1, now);
    list_and_reserve(&mut repo, &tomb.coords, &d32(0x31), now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();
    // Same key (d16(9)) as the ordinary op, but a removal body → digest differs.
    let response = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(9),
            &tomb.submission,
            &d32(0x31),
            now,
            &mut no_failpoint,
        )
        .expect("removal");
    assert!(matches!(
        refusal(&response),
        Some(ControlRefusal::IdempotencyConflict)
    ));
}

// ---------------------------------------------------------------------------
// maximum-record removal survives ordinary exhaustion; ack ≠ compaction
// ---------------------------------------------------------------------------

fn ceilings_with_tiny_idempotency() -> AccountingCeilings {
    let mut values = [u64::MAX / 4; ACCOUNTING_CLASS_COUNT];
    // AccountingClass::Idempotency is index 5 (row-count ceiling of one).
    values[5] = 1;
    AccountingCeilings::from_array(values)
}

#[test]
fn maximum_record_removal_completes_when_ordinary_idempotency_is_exhausted() {
    let now = 1_000;
    let mut repo = AnchorRepository::open_in_memory_with_ceilings(ceilings_with_tiny_idempotency())
        .expect("open");
    // Fill the ordinary idempotency ceiling (1 row).
    {
        let mut tx = repo.begin().expect("begin");
        tx.claim_idempotency(
            &d32(0x40),
            &d16(1),
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        )
        .expect("ordinary claim fills the ceiling");
        tx.commit().expect("commit");
    }
    // A second ordinary claim would now exceed the ceiling.
    {
        let mut tx = repo.begin().expect("begin");
        let over = tx.claim_idempotency(
            &d32(0x41),
            &d16(2),
            0,
            IdempotencyClaimState::Terminal,
            None,
            None,
            now,
            now + 100,
        );
        assert!(over.is_err(), "ordinary idempotency is exhausted");
        drop(tx);
    }
    // But an owner removal still terminalizes: it claims the RESERVED partition,
    // which does not charge the ordinary row ceiling.
    let tomb = genuine_tombstone(44, 0, 1, now);
    let slot = list_and_reserve(&mut repo, &tomb.coords, &d32(0x42), now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();
    let response = service
        .submit(
            &mut repo,
            &mut scheduler,
            &d16(3),
            &tomb.submission,
            &d32(0x42),
            now,
            &mut no_failpoint,
        )
        .expect("removal");
    assert!(
        is_success(&response),
        "max removal survives ordinary exhaustion"
    );
    let tx = repo.begin().expect("begin");
    let loaded = tx.load_removal_slot(slot).expect("load").expect("slot");
    assert_eq!(loaded.state, RemovalSlotState::Terminal);
}

#[test]
fn acknowledgement_is_durable_logical_and_does_not_wait_for_compaction() {
    let now = 1_000;
    let tomb = genuine_tombstone(45, 0, 1, now);
    let mut repo = repo();
    let digest = d32(0x50);
    let slot = list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
    let service = service();
    let mut scheduler = scheduler_defaults();
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
        .expect("removal");
    assert!(is_success(&response));
    // The ack is durable-logical: the slot is Terminal and the result is retained,
    // yet NO checkpoint / physical compaction has run.
    let tx = repo.begin().expect("begin");
    assert_eq!(
        tx.load_removal_slot(slot)
            .expect("load")
            .expect("slot")
            .state,
        RemovalSlotState::Terminal
    );
    assert!(tx.reserved_result(&digest).expect("result").is_some());
    assert_eq!(
        tx.latest_checkpoint_generation().expect("generation"),
        0,
        "no compaction is on the acknowledgement path"
    );
}

// ---------------------------------------------------------------------------
// crash safety: the atomic terminalization is wholly-absent-or-committed
// ---------------------------------------------------------------------------

#[test]
fn every_removal_failpoint_leaves_no_partial_state() {
    for failpoint in [
        "visibility",
        "inclusion",
        "projection",
        "receipt",
        "terminal",
        "commit",
    ] {
        let now = 1_000;
        let tomb = genuine_tombstone(46, 0, 1, now);
        let mut repo = repo();
        let digest = d32(0x60);
        let slot = list_and_reserve(&mut repo, &tomb.coords, &digest, now).expect("reserved");
        let service = service();
        let mut scheduler = scheduler_defaults();
        let mut fp = |label: &str| label == failpoint;
        let result = service.submit(
            &mut repo,
            &mut scheduler,
            &d16(1),
            &tomb.submission,
            &digest,
            now,
            &mut fp,
        );
        assert!(
            result.is_err(),
            "failpoint {failpoint} aborts before commit"
        );
        // Nothing durable changed: the slot is still Reserved, the listing intact,
        // and no reserved result exists. A retry can still succeed.
        let tx = repo.begin().expect("begin");
        assert_eq!(
            tx.load_removal_slot(slot)
                .expect("load")
                .expect("slot")
                .state,
            RemovalSlotState::ReservedForListedRoot,
            "failpoint {failpoint}: slot stays reserved"
        );
        assert!(
            tx.current_listing(&tomb.community_id)
                .expect("listing")
                .is_some(),
            "failpoint {failpoint}: visibility intact"
        );
        assert!(tx.reserved_result(&digest).expect("result").is_none());
        drop(tx);
        // The scheduler permit was released so a retry can proceed.
        assert_eq!(scheduler.in_flight_jobs(), 0);
    }
}
