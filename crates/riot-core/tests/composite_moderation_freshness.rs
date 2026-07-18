//! Composite-site Unit 3 — Task 5: the `moderation-current` freshness keystone.
//!
//! `evaluate_freshness` is the anti-suppression + anti-brick core. It must:
//! - refuse a false "current" (no heartbeat / stale / seq-gap / digest mismatch
//!   ⇒ `Loading`), so a withholding provider is detectable, never silent;
//! - emit exemption-FILTERED sets so a rogue/seized moderator cannot brick the
//!   site (`revoke{root}` and `tombstone{manifest/owner}` are dropped);
//! - hide a revoked author regardless of a backdated timestamp (identity at
//!   render, not the clock).

use std::collections::BTreeSet;

use riot_core::site::{
    compute_mod_set_digest, evaluate_freshness, Endorse, HeldModerationRecord, ModEpoch,
    ModerationFreshness, ModerationLoading, ModerationRecord, Revoke, Tombstone,
    MODERATION_FRESHNESS_WINDOW_SECS,
};

const ROOT: [u8; 32] = [0xAA; 32];
const NOW: u64 = 1_000_000;

fn held(record: ModerationRecord, id: u8) -> HeldModerationRecord {
    HeldModerationRecord {
        record,
        record_id: [id; 32],
    }
}

fn revoke(author: u8) -> ModerationRecord {
    ModerationRecord::Revoke(Revoke {
        author_key: [author; 32],
        effective_ts: NOW,
    })
}

fn tombstone(entry: u8) -> ModerationRecord {
    ModerationRecord::Tombstone(Tombstone {
        target_ns: ROOT,
        target_entry: [entry; 32],
    })
}

/// A heartbeat whose digest correctly commits to the given held revoke/tombstone
/// record-ids (what an honest, fully-synced owner would emit).
fn heartbeat_committing(seq: u64, ts: u64, committed_ids: &[[u8; 32]]) -> ModerationRecord {
    let set: BTreeSet<[u8; 32]> = committed_ids.iter().copied().collect();
    ModerationRecord::ModEpoch(ModEpoch {
        seq,
        ts,
        mod_set_digest: compute_mod_set_digest(&set),
    })
}

fn no_protected() -> BTreeSet<[u8; 32]> {
    BTreeSet::new()
}

#[test]
fn a_fresh_complete_mod_set_is_current_with_the_expected_sets() {
    // Held: revoke(id 1), tombstone(id 2), endorse(id 3), heartbeat committing {1,2}.
    let records = vec![
        held(revoke(0x11), 1),
        held(tombstone(0x22), 2),
        held(
            ModerationRecord::Endorse(Endorse {
                author_key: [0x33; 32],
            }),
            3,
        ),
        held(heartbeat_committing(0, NOW, &[[1; 32], [2; 32]]), 9),
    ];
    match evaluate_freshness(&records, ROOT, &no_protected(), NOW) {
        ModerationFreshness::Current {
            revoked,
            tombstoned,
            endorsed,
        } => {
            assert!(revoked.contains(&[0x11; 32]));
            assert!(tombstoned.contains(&[0x22; 32]));
            assert!(endorsed.contains(&[0x33; 32]));
        }
        other => panic!("expected Current, got {other:?}"),
    }
}

#[test]
fn no_heartbeat_is_loading_never_current() {
    let records = vec![held(revoke(0x11), 1)];
    assert_eq!(
        evaluate_freshness(&records, ROOT, &no_protected(), NOW),
        ModerationFreshness::Loading(ModerationLoading::NoHeartbeat),
    );
}

#[test]
fn a_stale_heartbeat_is_loading() {
    let stale_ts = NOW - MODERATION_FRESHNESS_WINDOW_SECS - 1;
    let records = vec![held(heartbeat_committing(0, stale_ts, &[]), 9)];
    assert_eq!(
        evaluate_freshness(&records, ROOT, &no_protected(), NOW),
        ModerationFreshness::Loading(ModerationLoading::StaleHeartbeat),
    );
}

#[test]
fn a_seq_gap_is_loading() {
    // Held heartbeats seq {0, 1, 3} — seq 2 is missing.
    let records = vec![
        held(heartbeat_committing(0, NOW, &[]), 7),
        held(heartbeat_committing(1, NOW, &[]), 8),
        held(heartbeat_committing(3, NOW, &[]), 9),
    ];
    assert_eq!(
        evaluate_freshness(&records, ROOT, &no_protected(), NOW),
        ModerationFreshness::Loading(ModerationLoading::SeqGap),
    );
}

#[test]
fn tail_suppression_is_detected_as_digest_mismatch() {
    // The heartbeat commits to {1, 2}, but the client holds only revoke id 1 —
    // the latest revoke (id 2) was withheld. No seq gap (single heartbeat), yet
    // the recomputed digest over {1} != the committed digest over {1,2}.
    let records = vec![
        held(revoke(0x11), 1),
        held(heartbeat_committing(0, NOW, &[[1; 32], [2; 32]]), 9),
    ];
    assert_eq!(
        evaluate_freshness(&records, ROOT, &no_protected(), NOW),
        ModerationFreshness::Loading(ModerationLoading::DigestMismatch),
    );
}

#[test]
fn a_revoke_of_the_root_is_exempt() {
    // A rogue moderator tries to revoke the owner. It must NOT appear in revoked.
    let root_revoke = ModerationRecord::Revoke(Revoke {
        author_key: ROOT,
        effective_ts: NOW,
    });
    let records = vec![
        held(root_revoke, 1),
        held(heartbeat_committing(0, NOW, &[[1; 32]]), 9),
    ];
    match evaluate_freshness(&records, ROOT, &no_protected(), NOW) {
        ModerationFreshness::Current { revoked, .. } => {
            assert!(
                !revoked.contains(&ROOT),
                "root revoke must be hard-ignored — a moderator cannot revoke the owner"
            );
        }
        other => panic!("expected Current, got {other:?}"),
    }
}

#[test]
fn a_tombstone_of_a_protected_manifest_or_owner_entry_is_exempt() {
    // A rogue/seized moderator tombstones the manifest entry-id. It must NOT
    // appear in the tombstoned set (the brick-the-site attack, plan case 5b).
    let protected_manifest_id = [0x99; 32];
    let mut protected = BTreeSet::new();
    protected.insert(protected_manifest_id);

    let records = vec![
        held(tombstone(0x99), 1), // targets the protected manifest entry
        held(tombstone(0x44), 2), // targets an ordinary article — allowed
        held(heartbeat_committing(0, NOW, &[[1; 32], [2; 32]]), 9),
    ];
    match evaluate_freshness(&records, ROOT, &protected, NOW) {
        ModerationFreshness::Current { tombstoned, .. } => {
            assert!(
                !tombstoned.contains(&protected_manifest_id),
                "a tombstone targeting /manifest (or an owner entry) must be hard-ignored"
            );
            assert!(
                tombstoned.contains(&[0x44; 32]),
                "an ordinary tombstone is still applied"
            );
        }
        other => panic!("expected Current, got {other:?}"),
    }
}

#[test]
fn a_backdated_revoke_still_hides_the_author() {
    // effective_ts far in the past — the ban rests on identity, not the clock.
    let backdated = ModerationRecord::Revoke(Revoke {
        author_key: [0x55; 32],
        effective_ts: 1, // ancient
    });
    let records = vec![
        held(backdated, 1),
        held(heartbeat_committing(0, NOW, &[[1; 32]]), 9),
    ];
    match evaluate_freshness(&records, ROOT, &no_protected(), NOW) {
        ModerationFreshness::Current { revoked, .. } => {
            assert!(
                revoked.contains(&[0x55; 32]),
                "a backdated revoke still hides — identity at render, not the clock"
            );
        }
        other => panic!("expected Current, got {other:?}"),
    }
}
