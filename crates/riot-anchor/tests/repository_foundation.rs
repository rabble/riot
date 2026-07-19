//! Foundation tests for the anchor [`AnchorRepository`] service layer (WU-013B).
//!
//! These exercise the durable guarantees on top of WU-013A's migrated schema:
//! independent accounting ceilings, dedup that never discounts logical charge,
//! immutable read snapshots, the single-writer deployment lease
//! (clone/steal/token detection), deterministic eviction obeying signed
//! retention horizons, and crash recovery.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use riot_anchor::repository::{
    AccountingCeilings, AccountingClass, AnchorRepository, AnchorRepositoryError, EvictionTier,
    ACCOUNTING_CLASS_COUNT,
};

/// A temporary on-disk database path that cleans up its `-wal`/`-shm` siblings.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!("riot-anchor-repo-{}-{}.db", std::process::id(), id));
        // Start clean in case a prior crashed run left files behind.
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

fn key(byte: u8) -> [u8; 32] {
    [byte; 32]
}

// --- Transactions / durability ------------------------------------------------

#[test]
fn open_uses_wal_and_enforces_foreign_keys() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");

    // WAL is a persistent database property; a fresh independent connection
    // reports it.
    let probe = rusqlite::Connection::open(temp.path()).expect("probe connection");
    let mode: String = probe
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal_mode");
    assert_eq!(
        mode.to_lowercase(),
        "wal",
        "repository must open in WAL mode"
    );

    // Foreign keys are enforced on the repository connection: an inclusion for
    // a nonexistent community is rejected.
    let mut tx = repo.begin().expect("begin");
    let result = tx.insert_directory_inclusion(&key(1), &key(2), 0, &[7u8; 16]);
    assert!(
        matches!(result, Err(AnchorRepositoryError::Sqlite(_))),
        "foreign keys must be enforced on the repository connection"
    );
}

// --- Accounting classes -------------------------------------------------------

#[test]
fn each_accounting_class_rejects_at_its_own_ceiling_independently() {
    // Distinct small ceilings so a leak between classes would be obvious.
    let mut ceilings = [0u64; ACCOUNTING_CLASS_COUNT];
    for (index, slot) in ceilings.iter_mut().enumerate() {
        *slot = 100 * (index as u64 + 1);
    }
    let ceilings = AccountingCeilings::from_array(ceilings);
    let mut repo =
        AnchorRepository::open_in_memory_with_ceilings(ceilings).expect("open in-memory repo");

    let mut tx = repo.begin().expect("begin");
    // Charge every class to exactly its ceiling in ONE transaction. Because the
    // classes are independent, all nine can sit at their ceiling at once.
    for class in AccountingClass::ALL {
        let ceiling = ceilings.ceiling(class);
        tx.charge(class, ceiling)
            .unwrap_or_else(|error| panic!("charge {class:?} to ceiling {ceiling}: {error}"));
    }
    // With all classes at their ceiling, one more unit in ANY class is rejected,
    // and the error names exactly that class (no masking).
    for class in AccountingClass::ALL {
        match tx.charge(class, 1) {
            Err(AnchorRepositoryError::ClassExceeded {
                class: exceeded,
                ceiling,
                ..
            }) => {
                assert_eq!(exceeded, class, "the rejection must name the charged class");
                assert_eq!(ceiling, ceilings.ceiling(class));
            }
            other => panic!("expected ClassExceeded for {class:?}, got {other:?}"),
        }
    }
}

// --- Payload dedup ------------------------------------------------------------

#[test]
fn cross_community_payload_dedup_charges_physical_once_but_logical_twice() {
    const N: u64 = 4096;
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let community_a = key(0xA1);
    let community_b = key(0xB2);
    let payload = key(0x50);

    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community_a, 1).expect("insert A");
    tx.insert_community(&community_b, 2).expect("insert B");

    let first = tx
        .add_payload(&community_a, &payload, N)
        .expect("A payload");
    assert!(
        first.logical_charged && first.physical_charged,
        "A pays both"
    );

    let second = tx
        .add_payload(&community_b, &payload, N)
        .expect("B payload");
    assert!(
        second.logical_charged,
        "B must pay full logical for the shared payload"
    );
    assert!(
        !second.physical_charged,
        "B must NOT pay physical again — dedup by digest"
    );
    tx.commit().expect("commit");

    // Physical deduped to one copy; logical charged fully to each community.
    assert_eq!(repo.used(AccountingClass::Physical), N, "physical deduped");
    assert_eq!(
        repo.used(AccountingClass::Logical),
        2 * N,
        "logical never discounted by dedup"
    );

    // Each community independently carries the full logical charge...
    let snapshot = repo.snapshot().expect("snapshot");
    assert_eq!(
        snapshot.community_logical_bytes(&community_a).unwrap(),
        Some(N)
    );
    assert_eq!(
        snapshot.community_logical_bytes(&community_b).unwrap(),
        Some(N)
    );

    // ...while there is exactly one physical payload row, shared (refcount 2).
    let probe = rusqlite::Connection::open(temp.path()).expect("probe");
    let (rows, refcount): (i64, i64) = probe
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(reference_count), 0) FROM payloads",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("payload row");
    assert_eq!(rows, 1, "one deduplicated physical payload row");
    assert_eq!(refcount, 2, "shared by two communities");
}

// --- Immutable read snapshots -------------------------------------------------

#[test]
fn immutable_snapshot_isolates_reader_from_concurrent_writer() {
    const N: u64 = 100;
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let community_a = key(1);
    let community_b = key(2);

    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community_a, 1).expect("insert A");
    tx.add_payload(&community_a, &key(9), N).expect("payload");
    tx.commit().expect("commit");

    // Take a point-in-time snapshot: A=N, exactly one community.
    let snapshot = repo.snapshot().expect("snapshot");
    assert_eq!(snapshot.community_count().unwrap(), 1);
    assert_eq!(
        snapshot.community_logical_bytes(&community_a).unwrap(),
        Some(N)
    );

    // The writer commits new state while the snapshot is held open.
    let mut tx = repo.begin().expect("begin 2");
    tx.insert_community(&community_b, 2).expect("insert B");
    tx.commit().expect("commit 2");

    // The snapshot still sees the original consistent view.
    assert_eq!(
        snapshot.community_count().unwrap(),
        1,
        "snapshot must not see the concurrent write"
    );
    assert_eq!(
        snapshot.community_logical_bytes(&community_b).unwrap(),
        None
    );

    // A fresh snapshot observes the committed write.
    drop(snapshot);
    let fresh = repo.snapshot().expect("fresh snapshot");
    assert_eq!(fresh.community_count().unwrap(), 2);
}

// --- Deployment lease ---------------------------------------------------------

#[test]
fn deployment_lease_detects_clone_before_expiry() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let token = key(0x7);
    let holder_one = key(0x11);
    let holder_two = key(0x22);

    let lease = repo
        .acquire_deployment_lease(&holder_one, &token, 0, 100)
        .expect("first holder acquires");

    // A restored clone (different holder, same token) tries to acquire while the
    // original lease is still valid: it must fail closed.
    let clone = repo.acquire_deployment_lease(&holder_two, &token, 10, 100);
    match clone {
        Err(AnchorRepositoryError::LeaseHeld { holder_id, .. }) => {
            assert_eq!(
                holder_id, holder_one,
                "clone rejected in favor of the live holder"
            );
        }
        other => panic!("expected LeaseHeld, got {other:?}"),
    }

    // The original holder still verifies successfully within its term.
    repo.verify_deployment_lease(&lease, 10)
        .expect("original holder still holds a valid lease");
}

#[test]
fn deployment_lease_detects_steal_after_expiry() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let token = key(0x7);
    let holder_one = key(0x11);
    let holder_two = key(0x22);

    let first = repo
        .acquire_deployment_lease(&holder_one, &token, 0, 100)
        .expect("first holder acquires");

    // After the term expires, a second holder legitimately takes over; the epoch
    // must advance.
    let second = repo
        .acquire_deployment_lease(&holder_two, &token, 200, 100)
        .expect("second holder acquires after expiry");
    assert!(
        second.epoch > first.epoch,
        "a steal advances the lease epoch"
    );

    // The first holder can now detect that its lease was taken.
    match repo.verify_deployment_lease(&first, 250) {
        Err(AnchorRepositoryError::LeaseLost) => {}
        other => panic!("expected LeaseLost for the displaced holder, got {other:?}"),
    }
    // The new holder verifies within its own term.
    repo.verify_deployment_lease(&second, 250)
        .expect("new holder holds the lease");
}

#[test]
fn deployment_lease_rejects_token_mismatch() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let holder_one = key(0x11);
    let holder_two = key(0x22);

    repo.acquire_deployment_lease(&holder_one, &key(0x7), 0, 100)
        .expect("bind the database to a token");

    // A deployment presenting a different instance token is equivocation.
    match repo.acquire_deployment_lease(&holder_two, &key(0x8), 200, 100) {
        Err(AnchorRepositoryError::LeaseTokenMismatch) => {}
        other => panic!("expected LeaseTokenMismatch, got {other:?}"),
    }
}

// --- Eviction / retention -----------------------------------------------------

#[test]
fn eviction_order_is_deterministic_across_tiers() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let now = 1_000u64;

    let listed_old = key(0x10);
    let listed_new = key(0x11);
    let projection_only = key(0x20);
    let unlisted_old = key(0x30);
    let unlisted_new = key(0x31);

    let mut tx = repo.begin().expect("begin");
    // Listed sites (with removal slots + listings, differing refresh times).
    tx.insert_community(&listed_old, 1).expect("listed_old");
    tx.insert_community(&listed_new, 2).expect("listed_new");
    let slot_old = tx
        .claim_removal_slot(&listed_old, &key(0xF0), &key(0xF1))
        .expect("slot old");
    let slot_new = tx
        .claim_removal_slot(&listed_new, &key(0xF2), &key(0xF3))
        .expect("slot new");
    tx.insert_listing(&listed_old, &key(0xF0), 0, 10, 100, slot_old)
        .expect("listing old");
    tx.insert_listing(&listed_new, &key(0xF2), 0, 10, 200, slot_new)
        .expect("listing new");

    // Projection-only community (has an inclusion, no listing).
    tx.insert_community(&projection_only, 3)
        .expect("projection");
    tx.insert_directory_inclusion(&key(0x21), &projection_only, 0, &[1u8; 16])
        .expect("inclusion");

    // Unlisted sites.
    tx.insert_community(&unlisted_old, 5).expect("unlisted_old");
    tx.insert_community(&unlisted_new, 6).expect("unlisted_new");

    // One expired staging (retention horizon passed) and one still-live.
    tx.stage_operation(&key(0x40), &[9u8; 8], 0, now - 1, 128)
        .expect("expired staging");
    tx.stage_operation(&key(0x41), &[9u8; 8], 0, now + 500, 128)
        .expect("live staging");
    tx.commit().expect("commit");

    let plan = repo.plan_eviction(now).expect("plan eviction");

    // Tiers appear in strict priority order.
    let rank = |tier: EvictionTier| match tier {
        EvictionTier::ExpiredProjection => 0,
        EvictionTier::AbandonedStaging => 1,
        EvictionTier::UnlistedSite => 2,
        EvictionTier::ListedSite => 3,
    };
    let ranks: Vec<u8> = plan.iter().map(|c| rank(c.tier)).collect();
    assert!(
        ranks.windows(2).all(|w| w[0] <= w[1]),
        "candidates must be grouped in tier priority order: {ranks:?}"
    );

    let by_tier = |tier: EvictionTier| -> Vec<Vec<u8>> {
        plan.iter()
            .filter(|c| c.tier == tier)
            .map(|c| c.key.clone())
            .collect()
    };

    assert_eq!(
        by_tier(EvictionTier::ExpiredProjection),
        vec![key(0x21).to_vec()],
        "tier 1: unlisted directory projection"
    );
    assert_eq!(
        by_tier(EvictionTier::AbandonedStaging),
        vec![key(0x40).to_vec()],
        "tier 2: only the expired staging op"
    );
    assert_eq!(
        by_tier(EvictionTier::UnlistedSite),
        vec![
            projection_only.to_vec(),
            unlisted_old.to_vec(),
            unlisted_new.to_vec(),
        ],
        "tier 3: unlisted sites by oldest created_at"
    );
    assert_eq!(
        by_tier(EvictionTier::ListedSite),
        vec![listed_old.to_vec(), listed_new.to_vec()],
        "tier 4: listed sites by oldest host refresh"
    );
}

#[test]
fn eviction_respects_signed_staging_retention_horizon() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let now = 1_000u64;

    let mut tx = repo.begin().expect("begin");
    tx.stage_operation(&key(1), &[9u8; 8], 0, now - 10, 64)
        .expect("expired");
    tx.stage_operation(&key(2), &[9u8; 8], 0, now + 10, 64)
        .expect("within horizon");
    tx.commit().expect("commit");

    let plan = repo.plan_eviction(now).expect("plan");
    let staged: Vec<Vec<u8>> = plan
        .iter()
        .filter(|c| c.tier == EvictionTier::AbandonedStaging)
        .map(|c| c.key.clone())
        .collect();
    assert_eq!(
        staged,
        vec![key(1).to_vec()],
        "staging still inside its signed retention horizon is not evicted"
    );
}

// --- Crash recovery -----------------------------------------------------------

#[test]
fn crash_before_commit_leaves_no_partial_state() {
    const N: u64 = 50;
    let temp = TempDb::new();
    let community = key(0xC1);

    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.insert_community(&community, 1).expect("insert");
        tx.add_payload(&community, &key(9), N).expect("payload");
        // Simulate a crash before commit: drop the transaction (rollback) and
        // the repository without committing.
        drop(tx);
    }

    // Reopen: no partial state, and accounting rehydrates to zero.
    let repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    assert_eq!(repo.used(AccountingClass::Logical), 0);
    assert_eq!(repo.used(AccountingClass::Physical), 0);
    let snapshot = repo.snapshot().expect("snapshot");
    assert_eq!(
        snapshot.community_count().unwrap(),
        0,
        "rollback left nothing"
    );
    assert_eq!(snapshot.community_logical_bytes(&community).unwrap(), None);
}

#[test]
fn committed_state_and_accounting_survive_reopen() {
    const N: u64 = 50;
    let temp = TempDb::new();
    let community = key(0xC2);

    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.insert_community(&community, 1).expect("insert");
        tx.add_payload(&community, &key(9), N).expect("payload");
        tx.commit().expect("commit");
    }

    // Reopen: committed rows are present and accounting rehydrates from them.
    let repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    assert_eq!(repo.used(AccountingClass::Logical), N, "logical rehydrated");
    assert_eq!(
        repo.used(AccountingClass::Physical),
        N,
        "physical rehydrated"
    );
    let snapshot = repo.snapshot().expect("snapshot");
    assert_eq!(
        snapshot.community_logical_bytes(&community).unwrap(),
        Some(N)
    );
}

#[test]
fn recover_readiness_clears_only_expired_staging() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let now = 1_000u64;

    let mut tx = repo.begin().expect("begin");
    tx.stage_operation(&key(1), &[9u8; 8], 0, now - 5, 200)
        .expect("expired");
    tx.stage_operation(&key(2), &[9u8; 8], 0, now + 5, 300)
        .expect("live");
    tx.commit().expect("commit");
    assert_eq!(repo.used(AccountingClass::Staging), 500);

    let report = repo.recover_readiness(now).expect("recover");
    assert_eq!(report.cleared_staging_operations, 1);
    assert_eq!(report.reclaimed_staging_bytes, 200);
    assert_eq!(
        repo.used(AccountingClass::Staging),
        300,
        "only the live staging charge remains"
    );
}
