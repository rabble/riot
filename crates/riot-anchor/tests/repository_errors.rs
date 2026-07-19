//! Error-path and edge-branch tests for the anchor [`AnchorRepository`]
//! service layer.
//!
//! Where `repository_foundation.rs` proves the happy-path durability guarantees,
//! this file drives the branches those tests leave uncovered: every
//! [`AnchorRepositoryError`] variant's `Display`, the lease renew/expiry/token
//! branches, the generation compare-and-swap outcomes, the removal-slot
//! reservation ceilings, the read-only accessors with no foundation caller, and
//! — crucially — the row decoders' rejection of malformed persisted rows. The
//! decoder-rejection tests corrupt a committed row through a raw connection with
//! CHECK enforcement disabled (a value the ordinary write path could never
//! produce), then assert the typed loader fails closed instead of silently
//! mis-decoding.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{params, Connection};

use riot_anchor::repository::{
    AccountingCeilings, AccountingClass, AnchorRepository, AnchorRepositoryError, CheckpointMember,
    CheckpointPlan, GenerationCas, IdempotencyClaimState, NewPreparedOperation, OperationKind,
    OperationStatus, RemovalSlotState, SlotReservation, StagedEntry,
};
use riot_anchor_protocol::authority::{AuthorityClass, ListingFloor};

/// A temporary on-disk database path that cleans up its `-wal`/`-shm` siblings.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "riot-anchor-repo-err-{}-{}.db",
            std::process::id(),
            id
        ));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    fn path(&self) -> &Path {
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

/// Open a raw connection with CHECK constraints and foreign keys disabled, so a
/// test can write a deliberately malformed row that the repository's own write
/// path (which respects every CHECK) could never produce. Used only to prove the
/// row decoders reject corrupt persisted state.
fn with_unchecked_connection<R>(path: &Path, edit: impl FnOnce(&Connection) -> R) -> R {
    let connection = Connection::open(path).expect("open raw connection");
    connection
        .pragma_update(None, "ignore_check_constraints", true)
        .expect("disable check constraints");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys");
    edit(&connection)
}

/// A minimal, schema-valid staged entry for a namespace.
fn staged_entry(
    namespace_id: [u8; 32],
    entry_id: [u8; 32],
    payload_digest: [u8; 32],
    payload_length: u64,
) -> StagedEntry {
    StagedEntry {
        namespace_id,
        entry_id,
        subspace_id: key(0x5B),
        path_bytes: vec![0x2F, 0x61],
        timestamp_be: [0, 0, 0, 0, 0, 0, 0, 7],
        payload_digest,
        payload_length,
        entry_bytes: vec![0xE1, 0xE2],
        item_bytes: vec![0x10, 0x11, 0x12],
    }
}

fn new_operation(operation_id: [u8; 32], kind: OperationKind) -> NewPreparedOperation {
    NewPreparedOperation {
        operation_id,
        originating_kind: kind,
        token_secret_epoch: 3,
        base_generation: 0,
        created_at: 0,
        operation_expiry: 1_000,
        retention_deadline: 2_000,
        prepare_response_bytes: vec![0xA1, 0xA2, 0xA3],
    }
}

// --- Ceilings / getters -------------------------------------------------------

#[test]
fn default_ceilings_equal_the_mvp_defaults() {
    // `AccountingCeilings::default()` must delegate to the compiled MVP table, so
    // an operator who constructs ceilings by `Default` gets the same enforcement
    // as `open`.
    let default = AccountingCeilings::default();
    let mvp = AccountingCeilings::mvp_defaults();
    for class in AccountingClass::ALL {
        assert_eq!(
            default.ceiling(class),
            mvp.ceiling(class),
            "default ceiling for {class:?} must match the MVP default"
        );
    }
}

#[test]
fn repository_ceiling_getter_reports_the_configured_ceiling() {
    let mut ceilings = [0u64; 9];
    for (index, slot) in ceilings.iter_mut().enumerate() {
        *slot = 7 * (index as u64 + 1);
    }
    let ceilings = AccountingCeilings::from_array(ceilings);
    let repo = AnchorRepository::open_in_memory_with_ceilings(ceilings).expect("open repo");

    for class in AccountingClass::ALL {
        assert_eq!(
            repo.ceiling(class),
            ceilings.ceiling(class),
            "repository must expose the configured ceiling for {class:?}"
        );
        assert_eq!(repo.used(class), 0, "a fresh repository has zero usage");
    }
}

// --- Error Display ------------------------------------------------------------

#[test]
fn every_error_variant_formats_a_descriptive_message() {
    // ClassExceeded: charge past a zero ceiling.
    let mut repo =
        AnchorRepository::open_in_memory_with_ceilings(AccountingCeilings::from_array([0u64; 9]))
            .expect("open repo");
    let mut tx = repo.begin().expect("begin");
    let class_exceeded = tx
        .charge(AccountingClass::Logical, 1)
        .expect_err("charge past a zero ceiling must be refused");
    assert!(
        class_exceeded.to_string().contains("exceed ceiling"),
        "ClassExceeded message: {class_exceeded}"
    );
    drop(tx);

    // Sqlite: a foreign-key violation surfaces the raw error.
    let mut tx = repo.begin().expect("begin");
    let sqlite = tx
        .insert_directory_inclusion(&key(1), &key(2), 0, &[7u8; 16])
        .expect_err("inclusion for a missing community must be rejected");
    assert!(
        sqlite.to_string().contains("sqlite error"),
        "Sqlite message: {sqlite}"
    );
    drop(tx);

    // SnapshotUnavailable: an in-memory repository cannot open a shared snapshot.
    // (`ReadSnapshot` is not `Debug`, so inspect the error arm directly.)
    let snapshot_err = match repo.snapshot() {
        Ok(_) => panic!("in-memory repository must not open a shareable snapshot"),
        Err(error) => error,
    };
    assert!(
        snapshot_err.to_string().contains("file-backed"),
        "SnapshotUnavailable message: {snapshot_err}"
    );

    // Lease variants: held, token mismatch, lost, expired.
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open file repo");
    let lease = repo
        .acquire_deployment_lease(&key(0x11), &key(0x7), 0, 100)
        .expect("first holder acquires");
    let held = repo
        .acquire_deployment_lease(&key(0x22), &key(0x7), 10, 100)
        .expect_err("a live clone must be refused");
    assert!(
        held.to_string().contains("held by another holder"),
        "LeaseHeld message: {held}"
    );
    let mismatch = repo
        .acquire_deployment_lease(&key(0x22), &key(0x8), 200, 100)
        .expect_err("a different token is equivocation");
    assert!(
        mismatch.to_string().contains("token mismatch"),
        "LeaseTokenMismatch message: {mismatch}"
    );
    let expired = repo
        .verify_deployment_lease(&lease, 500)
        .expect_err("verifying past expiry must fail");
    assert!(
        expired.to_string().contains("expired"),
        "LeaseExpired message: {expired}"
    );

    // LeaseLost: a fresh holder steals the (now expired) lease, advancing the
    // epoch, so the first holder's verify fails as lost.
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open file repo");
    let first = repo
        .acquire_deployment_lease(&key(0x11), &key(0x7), 0, 100)
        .expect("first holder acquires");
    repo.acquire_deployment_lease(&key(0x22), &key(0x7), 200, 100)
        .expect("second holder steals after expiry");
    let lost = repo
        .verify_deployment_lease(&first, 250)
        .expect_err("the displaced holder's lease is lost");
    assert!(
        lost.to_string().contains("taken by another"),
        "LeaseLost message: {lost}"
    );

    // RemovalSlotsExhausted: with every preprovisioned slot removed, a claim has
    // nowhere to land.
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute("DELETE FROM removal_slots", [])
            .expect("clear removal slots");
    });
    let mut tx = repo.begin().expect("begin");
    let exhausted = tx
        .claim_removal_slot(&key(0x33), &key(0x34), &key(0x35))
        .expect_err("no free slot remains");
    assert!(
        exhausted.to_string().contains("no free removal slot"),
        "RemovalSlotsExhausted message: {exhausted}"
    );
}

#[test]
fn schema_error_surfaces_when_the_ledger_records_a_newer_version() {
    let temp = TempDb::new();
    {
        // Bring the schema to its current version.
        let _repo = AnchorRepository::open(temp.path()).expect("open repo");
    }
    // Record a version newer than this binary supports; reopening must fail
    // closed rather than migrate backward. This drives `From<SchemaError>` and
    // the schema arm of `Display`.
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute("INSERT INTO schema_migrations(version) VALUES (999)", [])
            .expect("record a future schema version");
    });
    let error = AnchorRepository::open(temp.path())
        .err()
        .expect("a future schema version must be refused");
    assert!(
        matches!(error, AnchorRepositoryError::Schema(_)),
        "expected Schema, got {error:?}"
    );
    assert!(
        error.to_string().contains("schema error"),
        "Schema message: {error}"
    );
}

// --- Deployment lease branches ------------------------------------------------

#[test]
fn same_holder_renewing_a_live_lease_keeps_its_epoch() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let holder = key(0x11);
    let token = key(0x7);

    let first = repo
        .acquire_deployment_lease(&holder, &token, 0, 100)
        .expect("first acquisition");
    // The same holder re-acquiring while its term is still live is a renewal,
    // not a steal: the epoch must not advance (so no prior verify is invalidated)
    // while the expiry slides forward.
    let renewed = repo
        .acquire_deployment_lease(&holder, &token, 10, 100)
        .expect("same-holder renewal");
    assert_eq!(
        renewed.epoch, first.epoch,
        "a renewal by the same live holder must not advance the epoch"
    );
    assert_eq!(renewed.expires_at, 110, "the renewal slides the expiry");
    repo.verify_deployment_lease(&first, 50)
        .expect("the original lease handle still verifies after a same-holder renewal");
}

#[test]
fn verify_rejects_a_lease_bound_to_a_different_token() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let lease = repo
        .acquire_deployment_lease(&key(0x11), &key(0x7), 0, 100)
        .expect("acquire");
    // A lease handle whose token does not match the database's bound token is
    // equivocation and must be rejected before any holder/epoch comparison.
    let mut forged = lease;
    forged.token = key(0xEE);
    match repo.verify_deployment_lease(&forged, 10) {
        Err(AnchorRepositoryError::LeaseTokenMismatch) => {}
        other => panic!("expected LeaseTokenMismatch, got {other:?}"),
    }
}

#[test]
fn verify_reports_expiry_for_an_unstolen_but_stale_lease() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let lease = repo
        .acquire_deployment_lease(&key(0x11), &key(0x7), 0, 100)
        .expect("acquire");
    // Same holder, same epoch, correct token — but the term has elapsed.
    match repo.verify_deployment_lease(&lease, 200) {
        Err(AnchorRepositoryError::LeaseExpired) => {}
        other => panic!("expected LeaseExpired, got {other:?}"),
    }
}

// --- Generation compare-and-swap ----------------------------------------------

#[test]
fn generation_cas_first_host_requires_a_zero_base() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let community = key(0xC1);
    let mut tx = repo.begin().expect("begin");

    // No community row yet: a non-zero base cannot be a first host, so the swap
    // is stale against the implicit generation 0.
    match tx
        .commit_generation_cas(&community, 0, 5, 6)
        .expect("cas call")
    {
        GenerationCas::Stale { current_generation } => assert_eq!(current_generation, 0),
        other => panic!("expected Stale against generation 0, got {other:?}"),
    }

    // A zero base first-hosts the community at the committed generation.
    match tx
        .commit_generation_cas(&community, 0, 0, 1)
        .expect("cas call")
    {
        GenerationCas::Committed => {}
        other => panic!("expected Committed first host, got {other:?}"),
    }
    tx.commit().expect("commit");
    assert_eq!(
        repo.site_generation(&community).expect("site generation"),
        Some(1),
        "the first host set the committed generation"
    );
}

#[test]
fn generation_cas_swaps_on_matching_base_and_rejects_a_stale_base() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let community = key(0xC2);
    let mut tx = repo.begin().expect("begin");
    tx.commit_generation_cas(&community, 0, 0, 1)
        .expect("first host");

    // A base equal to the current generation advances it.
    match tx
        .commit_generation_cas(&community, 0, 1, 2)
        .expect("cas call")
    {
        GenerationCas::Committed => {}
        other => panic!("expected Committed swap, got {other:?}"),
    }

    // A base unequal to the current generation loses and reports the blocker.
    match tx
        .commit_generation_cas(&community, 0, 99, 100)
        .expect("cas call")
    {
        GenerationCas::Stale { current_generation } => assert_eq!(current_generation, 2),
        other => panic!("expected Stale against generation 2, got {other:?}"),
    }
    tx.commit().expect("commit");
    assert_eq!(repo.site_generation(&community).expect("gen"), Some(2));
}

// --- Removal-slot reservation ceilings ----------------------------------------

#[test]
fn reserve_visibility_slot_reserves_then_blocks_once_a_root_owns_two() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let root = key(0x40);
    let mut tx = repo.begin().expect("begin");
    for community in [key(0x41), key(0x42), key(0x43)] {
        tx.insert_community(&community, 0)
            .expect("insert community");
    }

    let first = tx
        .reserve_visibility_slot(&key(0x41), &root, &key(0xD1), 0)
        .expect("first reservation");
    let second = tx
        .reserve_visibility_slot(&key(0x42), &root, &key(0xD2), 0)
        .expect("second reservation");
    assert!(
        matches!(first, SlotReservation::Reserved(_)),
        "first reservation succeeds: {first:?}"
    );
    assert!(
        matches!(second, SlotReservation::Reserved(_)),
        "second reservation succeeds: {second:?}"
    );

    // A root may hold at most two slots; the third reservation is blocked. No
    // retained-Terminal slot exists, so there is no earliest retry time.
    match tx
        .reserve_visibility_slot(&key(0x43), &root, &key(0xD3), 0)
        .expect("third reservation")
    {
        SlotReservation::Blocked { earliest_retry_at } => assert_eq!(earliest_retry_at, None),
        other => panic!("expected Blocked at the two-slot ceiling, got {other:?}"),
    }
    assert_eq!(
        tx.count_owned_removal_slots(&root, 0).expect("owned count"),
        2,
        "the root owns exactly its two reserved slots"
    );
}

#[test]
fn reserve_visibility_slot_blocks_when_no_global_slot_is_free() {
    let temp = TempDb::new();
    {
        let _repo = AnchorRepository::open(temp.path()).expect("seed schema");
    }
    // Remove every preprovisioned slot so the global free pool is empty.
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute("DELETE FROM removal_slots", [])
            .expect("clear slots");
    });

    let mut repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let mut tx = repo.begin().expect("begin");
    match tx
        .reserve_visibility_slot(&key(0x50), &key(0x51), &key(0xD9), 0)
        .expect("reservation")
    {
        SlotReservation::Blocked { earliest_retry_at } => assert_eq!(earliest_retry_at, None),
        other => panic!("expected Blocked with no free slot, got {other:?}"),
    }
}

#[test]
fn removal_slot_lifecycle_reads_load_release_and_abandonment() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let community = key(0x60);
    let root = key(0x61);
    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community, 0)
        .expect("insert community");

    let slot = tx
        .claim_removal_slot(&community, &root, &key(0xC9))
        .expect("claim a free slot");

    // The claimed slot reads back as reserved for the community/root.
    let loaded = tx
        .load_removal_slot(slot)
        .expect("load")
        .expect("the claimed slot exists");
    assert_eq!(loaded.slot_index, slot);
    assert_eq!(loaded.state, RemovalSlotState::ReservedForListedRoot);
    assert_eq!(loaded.claimed_by_community, Some(community));
    assert_eq!(loaded.claimed_root_key, Some(root));
    assert_eq!(loaded.request_digest, Some(key(0xC9)));

    // The community's live reservation is discoverable by community id.
    let reserved = tx
        .reserved_slot_for_community(&community)
        .expect("reserved lookup")
        .expect("the community owns a reserved slot");
    assert_eq!(reserved.slot_index, slot);

    // With no listing row, the reservation is abandoned and startup cleanup can
    // find it.
    let abandoned = tx.abandoned_reserved_slots().expect("abandoned scan");
    assert!(
        abandoned.contains(&slot),
        "the reserved-but-unlisted slot is abandoned: {abandoned:?}"
    );

    // Releasing the slot clears every binding column back to Free.
    tx.release_removal_slot(slot).expect("release");
    let released = tx
        .load_removal_slot(slot)
        .expect("reload")
        .expect("slot still present");
    assert_eq!(released.state, RemovalSlotState::Free);
    assert_eq!(released.claimed_by_community, None);
    assert_eq!(released.claimed_root_key, None);
    assert_eq!(released.request_digest, None);
    assert!(
        tx.abandoned_reserved_slots().expect("rescan").is_empty(),
        "a released slot is no longer abandoned"
    );
}

#[test]
fn removal_slot_state_codes_match_the_stable_schema_encoding() {
    assert_eq!(RemovalSlotState::Free.to_code(), 0);
    assert_eq!(RemovalSlotState::ReservedForListedRoot.to_code(), 1);
    assert_eq!(RemovalSlotState::Committed.to_code(), 2);
    assert_eq!(RemovalSlotState::Terminal.to_code(), 3);
}

// --- Listing floor round trip -------------------------------------------------

#[test]
fn listing_floor_round_trips_every_shown_class() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let mut tx = repo.begin().expect("begin");

    let cases = [
        (key(0x70), None),
        (key(0x71), Some(AuthorityClass::RootOwned)),
        (key(0x72), Some(AuthorityClass::Delegated)),
    ];
    for (root, shown_class) in cases {
        let floor = ListingFloor {
            root_id: root,
            epoch: 4,
            sealed: true,
            highest_revision: 9,
            shown_digest: Some(key(0xAB)),
            shown_class,
            equivocated: true,
        };
        tx.upsert_listing_floor(&floor).expect("upsert floor");
        let loaded = tx
            .load_listing_floor(&root)
            .expect("load floor")
            .expect("the floor exists");
        assert_eq!(loaded, floor, "floor with shown_class {shown_class:?}");
    }

    assert_eq!(
        tx.load_listing_floor(&key(0x7F)).expect("absent load"),
        None,
        "an unseen root has no floor"
    );
}

// --- Directory inclusion count ------------------------------------------------

#[test]
fn directory_inclusion_count_counts_a_communitys_signed_feed() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let community = key(0x80);
    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community, 0)
        .expect("insert community");
    tx.insert_directory_inclusion(&key(0x81), &community, 0, &[1u8; 8])
        .expect("first inclusion");
    tx.insert_directory_inclusion(&key(0x82), &community, 1, &[2u8; 8])
        .expect("second inclusion");

    assert_eq!(
        tx.directory_inclusion_count(&community)
            .expect("count present"),
        2
    );
    assert_eq!(
        tx.directory_inclusion_count(&key(0x8F))
            .expect("count absent"),
        0,
        "a community with no inclusions counts zero"
    );
}

// --- Emergency reserves -------------------------------------------------------

#[test]
fn emergency_reserve_value_reads_seeded_partitions() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let tx = repo.begin().expect("begin");
    assert_eq!(
        tx.emergency_reserve_value("owner_removal_verification_permits")
            .expect("seeded reserve"),
        Some(4),
        "the fixed owner-removal verification permit reserve is seeded at 4"
    );
    assert_eq!(
        tx.emergency_reserve_value("no_such_reserve")
            .expect("absent reserve"),
        None,
        "an unknown reserve name has no value"
    );
}

// --- Operations lifecycle -----------------------------------------------------

#[test]
fn operation_round_trips_replica_kind_and_every_terminal_status() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let operation_id = key(0x90);

    let mut tx = repo.begin().expect("begin");
    tx.insert_operation(&new_operation(operation_id, OperationKind::Replica))
        .expect("insert prepared operation");
    tx.commit().expect("commit");

    let stored = repo
        .load_operation(&operation_id)
        .expect("load")
        .expect("the operation exists");
    assert_eq!(stored.originating_kind, OperationKind::Replica);
    assert_eq!(
        stored.status,
        OperationStatus::Prepared,
        "a freshly inserted operation is Prepared"
    );

    // Persist each terminal status and read it back, exercising every code.
    for status in [
        OperationStatus::Committed,
        OperationStatus::Refused,
        OperationStatus::Prepared,
    ] {
        let mut tx = repo.begin().expect("begin");
        tx.set_operation_terminal(&operation_id, status, &[0x0F, 0x0E])
            .expect("set terminal status");
        tx.commit().expect("commit");
        let reloaded = repo
            .load_operation(&operation_id)
            .expect("reload")
            .expect("still present");
        assert_eq!(reloaded.status, status, "round-tripped status {status:?}");
        assert_eq!(
            reloaded.terminal_result_bytes.as_deref(),
            Some(&[0x0F, 0x0E][..])
        );
    }
}

// --- Idempotency claims -------------------------------------------------------

#[test]
fn claim_and_lookup_idempotency_round_trips_a_claimed_row() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let digest = key(0xA0);
    let idem_key = [0xA1u8; 16];
    let mut tx = repo.begin().expect("begin");
    tx.claim_idempotency(
        &digest,
        &idem_key,
        0,
        IdempotencyClaimState::Claimed,
        None,
        Some(30),
        0,
        100,
    )
    .expect("claim");

    let row = tx
        .lookup_idempotency(&idem_key)
        .expect("lookup")
        .expect("the key is claimed");
    assert_eq!(row.control_request_digest, digest);
    assert_eq!(row.result_class, 0);
    assert_eq!(row.claim_state, IdempotencyClaimState::Claimed);
    assert_eq!(row.operation_id, None);
    assert_eq!(row.lease_expires_at, Some(30));

    assert_eq!(
        tx.lookup_idempotency(&[0xFFu8; 16]).expect("absent lookup"),
        None,
        "an unclaimed key has no row"
    );
}

#[test]
fn reserved_idempotency_claim_lands_in_the_reserved_partition() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let digest = key(0xA5);
    let idem_key = [0xA6u8; 16];
    let mut tx = repo.begin().expect("begin");
    tx.claim_idempotency_reserved(&digest, &idem_key, IdempotencyClaimState::Prepared, 0, 100)
        .expect("reserved claim");

    let row = tx
        .lookup_idempotency(&idem_key)
        .expect("lookup")
        .expect("the reserved key is claimed");
    assert_eq!(
        row.result_class, 1,
        "a reserved claim uses the removal partition (result_class 1)"
    );
    assert_eq!(row.claim_state, IdempotencyClaimState::Prepared);
}

// --- Repository read accessors ------------------------------------------------

#[test]
fn repository_reads_committed_entries_receipts_and_staging() {
    let temp = TempDb::new();
    let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
    let community = key(0xB0);
    let namespace = key(0xB1);
    let operation = key(0xB2);

    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community, 0)
        .expect("insert community");
    let entry = staged_entry(namespace, key(0xB3), key(0xB4), 128);
    tx.insert_committed_entry(&community, 0, &entry)
        .expect("promote committed entry");
    tx.insert_hosting_receipt(&key(0xB5), &community, 0, &[0x7A, 0x7B])
        .expect("insert receipt");
    tx.ensure_staged_operation(&operation, &[9u8; 8], 0, 100)
        .expect("ensure staging op");
    tx.stage_entry(
        &operation,
        &staged_entry(namespace, key(0xB6), key(0xB7), 64),
    )
    .expect("stage entry");
    tx.commit().expect("commit");

    // Committed entries and their count.
    let committed = repo
        .committed_entries(&namespace)
        .expect("committed entries");
    assert_eq!(committed.len(), 1, "exactly one committed entry");
    assert_eq!(
        committed[0].0,
        key(0xB3).to_vec(),
        "the entry id is its sort key"
    );
    assert_eq!(
        repo.committed_entry_count(&namespace)
            .expect("committed count"),
        1
    );
    assert_eq!(
        repo.committed_entry_count(&key(0xBF))
            .expect("absent namespace count"),
        0,
        "an unseen namespace counts zero committed entries"
    );

    // Hosting receipt bytes, present and absent.
    assert_eq!(
        repo.hosting_receipt(&key(0xB5)).expect("receipt present"),
        Some(vec![0x7A, 0x7B])
    );
    assert_eq!(
        repo.hosting_receipt(&key(0xBE)).expect("receipt absent"),
        None
    );

    // Direction-private staged entries for the operation's namespace.
    let staged = repo
        .staged_entries(&operation, &namespace)
        .expect("staged entries");
    assert_eq!(staged.len(), 1, "one staged entry for the operation");
    assert_eq!(staged[0].entry_id, key(0xB6));
}

// --- Checkpoint work ----------------------------------------------------------

#[test]
fn insert_checkpoint_work_persists_members_and_covered_removals() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let work_id = key(0xE0);
    let plan = CheckpointPlan {
        work_id,
        created_at: 5,
        frozen_state_generation: 2,
        covered_head_sequence: 3,
        covered_head_inclusion_digest: key(0xE1),
        previous_checkpoint_digest: Some(key(0xE2)),
        snapshot_generation_id: 7,
        canonical_checkpoint_body: vec![0xC0, 0xDE],
        ordered_members: vec![
            CheckpointMember {
                community_id: key(0xE3),
                frozen_head_digest: key(0xE4),
                snapshot_record_bytes: vec![0x01],
            },
            CheckpointMember {
                community_id: key(0xE5),
                frozen_head_digest: key(0xE6),
                snapshot_record_bytes: vec![0x02],
            },
        ],
        // Slot 0 is a preprovisioned free slot (FK target).
        covered_removal_slots: vec![0],
    };

    let mut tx = repo.begin().expect("begin");
    tx.insert_checkpoint_work(&plan).expect("insert plan");

    let members = tx.checkpoint_work_members(&work_id).expect("read members");
    assert_eq!(
        members, plan.ordered_members,
        "members round-trip in frozen order"
    );
    assert_eq!(
        tx.checkpoint_covered_removals(&work_id)
            .expect("read covered removals"),
        vec![0],
        "the covered removal slot is recorded"
    );
    let row = tx
        .load_checkpoint_work(&work_id)
        .expect("load work")
        .expect("the work exists");
    assert_eq!(row.work_id, work_id);
    assert_eq!(row.frozen_state_generation, 2);
    assert_eq!(row.covered_head_inclusion_digest, Some(key(0xE1)));
}

#[test]
fn latest_checkpoint_generation_tracks_the_highest_published() {
    let mut repo = AnchorRepository::open_in_memory().expect("open repo");
    let mut tx = repo.begin().expect("begin");
    assert_eq!(
        tx.latest_checkpoint_generation()
            .expect("initial generation"),
        0,
        "no published checkpoint yet"
    );
    tx.insert_directory_checkpoint(5, &[0xAA], 0)
        .expect("publish generation 5");
    tx.insert_directory_checkpoint(3, &[0xBB], 1)
        .expect("publish generation 3");
    assert_eq!(
        tx.latest_checkpoint_generation()
            .expect("latest generation"),
        5,
        "the latest generation is the maximum published"
    );
}

// --- Row decoders reject malformed persisted rows -----------------------------

#[test]
fn load_operation_rejects_a_corrupt_originating_kind() {
    let temp = TempDb::new();
    let operation_id = key(0x90);
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.insert_operation(&new_operation(operation_id, OperationKind::Host))
            .expect("insert");
        tx.commit().expect("commit");
    }
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute("UPDATE operations SET originating_kind = 9", [])
            .expect("corrupt kind");
    });
    let repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let error = repo
        .load_operation(&operation_id)
        .expect_err("a corrupt originating_kind must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}

#[test]
fn load_operation_rejects_a_corrupt_operation_status() {
    let temp = TempDb::new();
    let operation_id = key(0x91);
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.insert_operation(&new_operation(operation_id, OperationKind::Host))
            .expect("insert");
        tx.commit().expect("commit");
    }
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute("UPDATE operations SET operation_status = 9", [])
            .expect("corrupt status");
    });
    let repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let error = repo
        .load_operation(&operation_id)
        .expect_err("a corrupt operation_status must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}

#[test]
fn lookup_idempotency_rejects_a_corrupt_claim_state() {
    let temp = TempDb::new();
    let idem_key = [0xC1u8; 16];
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.claim_idempotency(
            &key(0xC0),
            &idem_key,
            0,
            IdempotencyClaimState::Claimed,
            None,
            None,
            0,
            100,
        )
        .expect("claim");
        tx.commit().expect("commit");
    }
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute("UPDATE idempotency_key_index SET claim_state = 9", [])
            .expect("corrupt claim_state");
    });
    let mut repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let tx = repo.begin().expect("begin");
    let error = tx
        .lookup_idempotency(&idem_key)
        .expect_err("a corrupt claim_state must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}

#[test]
fn load_removal_slot_rejects_a_corrupt_removal_state() {
    let temp = TempDb::new();
    let slot;
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.insert_community(&key(0xC5), 0)
            .expect("insert community");
        slot = tx
            .claim_removal_slot(&key(0xC5), &key(0xC6), &key(0xC7))
            .expect("claim");
        tx.commit().expect("commit");
    }
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute(
                "UPDATE removal_slots SET removal_state = 9 WHERE slot_index = ?1",
                params![slot],
            )
            .expect("corrupt removal_state");
    });
    let mut repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let tx = repo.begin().expect("begin");
    let error = tx
        .load_removal_slot(slot)
        .expect_err("a corrupt removal_state must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}

#[test]
fn load_removal_slot_rejects_a_wrong_length_root_key_blob() {
    let temp = TempDb::new();
    let slot;
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.insert_community(&key(0xC5), 0)
            .expect("insert community");
        slot = tx
            .claim_removal_slot(&key(0xC5), &key(0xC6), &key(0xC7))
            .expect("claim");
        tx.commit().expect("commit");
    }
    // A five-byte root key can never be produced by the write path (the column
    // CHECKs `length = 32`); the 32-byte blob decoder must reject it.
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute(
                "UPDATE removal_slots SET claimed_root_key = ?1 WHERE slot_index = ?2",
                params![[0u8; 5].as_slice(), slot],
            )
            .expect("corrupt root key length");
    });
    let mut repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let tx = repo.begin().expect("begin");
    let error = tx
        .load_removal_slot(slot)
        .expect_err("a wrong-length 32-byte blob must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}

#[test]
fn load_removal_slot_rejects_a_wrong_length_idempotency_key_blob() {
    let temp = TempDb::new();
    let slot;
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.insert_community(&key(0xC5), 0)
            .expect("insert community");
        slot = tx
            .claim_removal_slot(&key(0xC5), &key(0xC6), &key(0xC7))
            .expect("claim");
        tx.commit().expect("commit");
    }
    // A five-byte idempotency key can never be produced by the write path (the
    // column CHECKs `length = 16`); the 16-byte blob decoder must reject it.
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute(
                "UPDATE removal_slots SET removal_idempotency_key = ?1 WHERE slot_index = ?2",
                params![[0u8; 5].as_slice(), slot],
            )
            .expect("corrupt idempotency key length");
    });
    let mut repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let tx = repo.begin().expect("begin");
    let error = tx
        .load_removal_slot(slot)
        .expect_err("a wrong-length 16-byte blob must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}

#[test]
fn load_listing_floor_rejects_a_corrupt_shown_class() {
    let temp = TempDb::new();
    let root = key(0x71);
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.upsert_listing_floor(&ListingFloor {
            root_id: root,
            epoch: 1,
            sealed: false,
            highest_revision: 0,
            shown_digest: Some(key(0xAB)),
            shown_class: Some(AuthorityClass::RootOwned),
            equivocated: false,
        })
        .expect("upsert");
        tx.commit().expect("commit");
    }
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute("UPDATE listing_floors SET shown_class = 9", [])
            .expect("corrupt shown_class");
    });
    let mut repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let tx = repo.begin().expect("begin");
    let error = tx
        .load_listing_floor(&root)
        .expect_err("a corrupt shown_class must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}

#[test]
fn load_checkpoint_work_rejects_a_corrupt_publication_phase() {
    let temp = TempDb::new();
    let work_id = key(0xE0);
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.insert_checkpoint_work(&CheckpointPlan {
            work_id,
            created_at: 0,
            frozen_state_generation: 0,
            covered_head_sequence: 0,
            covered_head_inclusion_digest: key(0xE1),
            previous_checkpoint_digest: None,
            snapshot_generation_id: 0,
            canonical_checkpoint_body: vec![0xC0],
            ordered_members: Vec::new(),
            covered_removal_slots: Vec::new(),
        })
        .expect("insert plan");
        tx.commit().expect("commit");
    }
    // Phase 5 satisfies the column CHECK (`BETWEEN 0 AND 6`) but is not a phase
    // the decoder maps, so the loader must reject it.
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute("UPDATE checkpoint_work SET publication_phase = 5", [])
            .expect("corrupt phase");
    });
    let mut repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let tx = repo.begin().expect("begin");
    let error = tx
        .load_checkpoint_work(&work_id)
        .expect_err("an unmapped publication_phase must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}

#[test]
fn staged_entries_reject_a_wrong_length_timestamp() {
    let temp = TempDb::new();
    let namespace = key(0xF1);
    let operation = key(0xF2);
    {
        let mut repo = AnchorRepository::open(temp.path()).expect("open repo");
        let mut tx = repo.begin().expect("begin");
        tx.ensure_staged_operation(&operation, &[9u8; 8], 0, 100)
            .expect("ensure staging op");
        tx.stage_entry(
            &operation,
            &staged_entry(namespace, key(0xF3), key(0xF4), 32),
        )
        .expect("stage entry");
        tx.commit().expect("commit");
    }
    // A three-byte timestamp can never be produced by the write path (the column
    // CHECKs `length = 8`); the fixed-width decoder must reject it.
    with_unchecked_connection(temp.path(), |connection| {
        connection
            .execute(
                "UPDATE staged_entries SET timestamp_be = ?1",
                params![[0u8; 3].as_slice()],
            )
            .expect("corrupt timestamp length");
    });
    let repo = AnchorRepository::open(temp.path()).expect("reopen repo");
    let error = repo
        .staged_entries(&operation, &namespace)
        .expect_err("a wrong-length timestamp must be rejected");
    assert!(matches!(error, AnchorRepositoryError::Sqlite(_)));
}
