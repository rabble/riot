use riot_core::apps::entry::{app_data_path, build_app_data_entry};
use riot_core::import::encode_bundle;
use riot_core::session::{CommitOutcome, ImportContext, RiotSession, SessionError};
use riot_core::store::{DatabaseConfig, RiotDatabase};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, EvidenceAuthor, SignedWillowEntry,
};
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-sqlite-evidence-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("riot.sqlite")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn author(seed: [u8; 32]) -> EvidenceAuthor {
    let _ = seed;
    riot_core::willow::generate_communal_author().expect("production author")
}

fn signed_app(
    author: &EvidenceAuthor,
    app_id: [u8; 32],
    key: &str,
    timestamp: u64,
    payload: &[u8],
) -> SignedWillowEntry {
    let entry = build_app_data_entry(author, &app_id, key, timestamp, payload).expect("entry");
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

fn commit(store: &riot_core::session::EvidenceStore, entry: &SignedWillowEntry) -> CommitOutcome {
    store
        .inspect(
            &encode_bundle(std::slice::from_ref(entry)).expect("bundle"),
            ImportContext::new("sqlite-test"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("commit")
}

#[test]
fn accepted_live_receipts_payload_and_generation_survive_restart() {
    let directory = TestDir::new("restart");
    let path = directory.database();
    let app_id = [9; 32];
    let author = author(*b"sqlite-evidence-author-seed-0001");
    let signed = signed_app(&author, app_id, "polls/one", 10, br#"{"title":"Lunch?"}"#);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("sqlite session");
    let store = session.create_store().expect("store");
    let receipt = match commit(&store, &signed) {
        CommitOutcome::Committed(receipt) => receipt,
        CommitOutcome::NoChanges(_) => panic!("new entry was a duplicate"),
    };
    let entry_id = receipt.dispositions[0].entry_id;
    drop(store);
    drop(session);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("reopen database");
    let session = RiotSession::open_sqlite(database).expect("reopen session");
    let store = session.create_store().expect("reopen store");
    assert_eq!(store.generation().expect("generation"), 1);
    assert_eq!(store.receipt_count().expect("receipts"), 1);
    assert_eq!(store.live_entry_ids().expect("live ids"), vec![entry_id]);
    let prefix = app_data_path(&app_id, "polls").expect("prefix");
    let entries = store.entries_with_prefix(&prefix).expect("prefix entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].2.as_deref(),
        Some(br#"{"title":"Lunch?"}"#.as_slice())
    );
    assert_eq!(
        store
            .provenance(&entry_id)
            .expect("provenance")
            .first_receipt_id,
        1
    );
    drop(store);
    drop(session);
    let raw = Connection::open(&path).expect("inspect schema records");
    let shape: (i64, i64, i64, i64, i64, i64, i64) = raw
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM accepted_entries),
                (SELECT COUNT(*) FROM live_entries),
                (SELECT COUNT(*) FROM import_receipts),
                (SELECT COUNT(*) FROM import_dispositions),
                (SELECT COUNT(*) FROM entry_path_prefixes),
                (SELECT MIN(length(entry_bytes) + length(capability_bytes)) FROM accepted_entries),
                (SELECT MIN(length(signature_bytes)) FROM accepted_entries)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .expect("record shape");
    assert_eq!(shape.0, 1);
    assert_eq!(shape.1, 1);
    assert_eq!(shape.2, 1);
    assert_eq!(shape.3, 1);
    assert!(shape.4 > 1, "materialized path ancestors exist");
    assert!(shape.5 > 0, "canonical entry and capability bytes persist");
    assert_eq!(shape.6, 64);
}

#[test]
fn rollback_and_namespace_isolation_are_durable() {
    let directory = TestDir::new("rollback-isolation");
    let path = directory.database();
    let app_id = [4; 32];
    let alice = author(*b"sqlite-evidence-alice-seed-00001");
    let bob = author(*b"sqlite-evidence-bob-seed-0000003");
    let alice_entry = signed_app(&alice, app_id, "votes/a", 1, b"alice");
    let bob_entry = signed_app(&bob, app_id, "votes/a", 1, b"bob");

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    let failed = store
        .inspect(
            &encode_bundle(std::slice::from_ref(&alice_entry)).expect("bundle"),
            ImportContext::new("failure"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit_with_injected_failure_for_tests();
    assert_eq!(failed, Err(SessionError::Injected));
    assert_eq!(store.generation().expect("generation"), 0);
    let mixed = encode_bundle(&[alice_entry, bob_entry]).expect("mixed namespace bundle");
    let receipt = store
        .inspect(&mixed, ImportContext::new("mixed"))
        .expect("inspect mixed")
        .expect_preview()
        .plan_all()
        .expect("plan mixed")
        .commit()
        .expect("commit mixed");
    assert!(matches!(receipt, CommitOutcome::Committed(ref r) if r.dispositions.len() == 2));
    assert_eq!(store.live_count().expect("live"), 2);
    let prefix = app_data_path(&app_id, "votes").expect("prefix");
    let alice_namespace = alice.identity().namespace_id;
    let alice_entries = store
        .entries_with_prefix_in_namespace(&alice_namespace, &prefix)
        .expect("Alice namespace query");
    assert_eq!(alice_entries.len(), 1);
    assert_eq!(alice_entries[0].2.as_deref(), Some(b"alice".as_slice()));
    drop(store);
    drop(session);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("reopen");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    assert_eq!(store.generation().expect("generation"), 1);
    assert_eq!(store.live_count().expect("live"), 2);
    assert_eq!(store.receipt_count().expect("receipts"), 1);
}

#[test]
fn forgetting_and_duplicate_reimport_restore_payload_across_restart() {
    let directory = TestDir::new("forget-restore");
    let path = directory.database();
    let app_id = [7; 32];
    let author = author(*b"sqlite-evidence-forget-seed-0001");
    let signed = signed_app(&author, app_id, "docs/a", 3, b"payload");

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    let entry_id = match commit(&store, &signed) {
        CommitOutcome::Committed(receipt) => receipt.dispositions[0].entry_id,
        CommitOutcome::NoChanges(_) => panic!("duplicate"),
    };
    store.forget_entry(&entry_id).expect("forget entry");
    let prefix = app_data_path(&app_id, "docs").expect("prefix");
    assert_eq!(store.live_count().expect("live after forget"), 0);
    assert!(store
        .entries_with_prefix(&prefix)
        .expect("entries")
        .is_empty());
    drop(store);
    drop(session);
    let raw = Connection::open(&path).expect("inspect forgotten marker");
    assert_eq!(
        raw.query_row("SELECT COUNT(*) FROM forgotten_entries", [], |row| row
            .get::<_, i64>(0))
            .expect("forgotten count"),
        1
    );
    drop(raw);
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("reopen forgotten");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    assert!(matches!(
        commit(&store, &signed),
        CommitOutcome::Committed(_)
    ));
    assert_eq!(
        store.entries_with_prefix(&prefix).expect("entries")[0]
            .2
            .as_deref(),
        Some(b"payload".as_slice())
    );
    drop(store);
    drop(session);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("reopen");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    assert_eq!(
        store.entries_with_prefix(&prefix).expect("entries")[0]
            .2
            .as_deref(),
        Some(b"payload".as_slice())
    );
}

#[test]
fn stale_database_generation_cannot_overwrite_newer_durable_state() {
    let directory = TestDir::new("generation-cas");
    let path = directory.database();
    let author = author(*b"sqlite-evidence-cas-seed-0000001");
    let first = signed_app(&author, [1; 32], "items/a", 1, b"first");
    let second = signed_app(&author, [1; 32], "items/b", 2, b"second");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    commit(&store, &first);

    let raw = Connection::open(&path).expect("external generation writer");
    raw.execute(
        "UPDATE evidence_meta SET generation = 99 WHERE singleton = 1",
        [],
    )
    .expect("advance generation");
    drop(raw);
    let plan = store
        .inspect(
            &encode_bundle(std::slice::from_ref(&second)).expect("bundle"),
            ImportContext::new("stale"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan");
    assert_eq!(plan.commit(), Err(SessionError::StalePreview));
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("inspect rollback");
    let counts: (i64, i64, i64) = raw
        .query_row(
            "SELECT (SELECT COUNT(*) FROM accepted_entries),
                    (SELECT COUNT(*) FROM live_entries),
                    (SELECT COUNT(*) FROM import_receipts)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("counts");
    assert_eq!(counts, (1, 1, 1));
}

#[test]
fn sqlite_failure_rolls_back_accepted_live_receipt_and_generation_together() {
    let directory = TestDir::new("sqlite-rollback");
    let path = directory.database();
    let author = author(*b"sqlite-evidence-sqlfail-seed-001");
    let signed = signed_app(&author, [2; 32], "items/a", 1, b"payload");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    let raw = Connection::open(&path).expect("failure injector");
    raw.execute_batch(
        "CREATE TRIGGER fail_receipt BEFORE INSERT ON import_receipts
         BEGIN SELECT RAISE(ABORT, 'injected'); END;",
    )
    .expect("create failure trigger");
    drop(raw);
    let result = store
        .inspect(
            &encode_bundle(std::slice::from_ref(&signed)).expect("bundle"),
            ImportContext::new("failure"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit();
    assert_eq!(result, Err(SessionError::Internal));
    assert_eq!(store.generation().expect("cached generation"), 0);
    drop(store);
    drop(session);
    let raw = Connection::open(&path).expect("inspect transaction");
    let state: (i64, i64, i64, i64) = raw
        .query_row(
            "SELECT (SELECT COUNT(*) FROM accepted_entries),
                    (SELECT COUNT(*) FROM live_entries),
                    (SELECT COUNT(*) FROM import_receipts),
                    (SELECT generation FROM evidence_meta WHERE singleton = 1)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("transaction state");
    assert_eq!(state, (0, 0, 0, 0));
}

#[test]
fn reopen_fails_closed_when_coordinate_columns_disagree_with_canonical_entry() {
    let directory = TestDir::new("coordinate-corruption");
    let path = directory.database();
    let author = author(*b"sqlite-evidence-corrupt-seed-001");
    let signed = signed_app(&author, [5; 32], "items/a", 1, b"payload");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    commit(&store, &signed);
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("tamper coordinate");
    raw.execute("UPDATE accepted_entries SET subspace_id = zeroblob(32)", [])
        .expect("tamper");
    drop(raw);
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("physical open");
    let session = RiotSession::open_sqlite(database).expect("session handle");
    assert!(matches!(
        session.create_store(),
        Err(SessionError::Internal)
    ));
}

fn make_one_entry_database(label: &str) -> (TestDir, PathBuf) {
    let directory = TestDir::new(label);
    let path = directory.database();
    let author = author(*b"sqlite-evidence-adversary-seed-1");
    let signed = signed_app(&author, [6; 32], "items/a", 1, b"payload");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    commit(&store, &signed);
    drop(store);
    drop(session);
    (directory, path)
}

fn assert_evidence_reopen_fails(path: &PathBuf) {
    let database = RiotDatabase::open(path, DatabaseConfig::default()).expect("physical open");
    let session = RiotSession::open_sqlite(database).expect("session handle");
    assert!(matches!(
        session.create_store(),
        Err(SessionError::Internal)
    ));
}

#[test]
fn reopen_rejects_corrupt_live_projection_and_every_prefix_set_mismatch() {
    let mutations = [
        "UPDATE live_entries SET subspace_id = zeroblob(32)",
        "UPDATE live_entries SET payload = X'00'",
        "DELETE FROM entry_path_prefixes WHERE depth = (SELECT MAX(depth) FROM entry_path_prefixes)",
        "UPDATE entry_path_prefixes SET prefix_bytes = X'FF' WHERE depth = 1",
        "INSERT INTO entry_path_prefixes(namespace_id, entry_id, depth, prefix_bytes)
         SELECT namespace_id, entry_id, 99, X'00' FROM live_entries",
    ];
    for (index, mutation) in mutations.iter().enumerate() {
        let (_directory, path) = make_one_entry_database(&format!("projection-{index}"));
        let raw = Connection::open(&path).expect("tamper projection");
        raw.execute_batch(mutation).expect("tamper");
        drop(raw);
        assert_evidence_reopen_fails(&path);
    }
}

#[test]
fn reopen_rejects_receipt_relationship_and_metadata_corruption() {
    let mutations = [
        "UPDATE import_dispositions SET namespace_id = zeroblob(32)",
        "DELETE FROM import_dispositions",
        "UPDATE accepted_entries SET first_receipt_id = 99",
        "UPDATE evidence_meta SET generation = 99",
        "UPDATE evidence_meta SET next_receipt_id = 99",
        "UPDATE evidence_meta SET retained_receipt_charge_bytes = retained_receipt_charge_bytes + 1",
        "UPDATE evidence_meta SET retained_receipt_charge_bytes = 0",
        "UPDATE import_receipts SET before_generation = 7, after_generation = 8",
        "WITH RECURSIVE ids(value) AS (
             SELECT 2 UNION ALL SELECT value + 1 FROM ids WHERE value < 257
         )
         INSERT INTO import_receipts(receipt_id, route, before_generation, after_generation)
             SELECT value, '', value - 1, value FROM ids;
         UPDATE evidence_meta SET generation = 257, next_receipt_id = 258",
    ];
    for (index, mutation) in mutations.iter().enumerate() {
        let (_directory, path) = make_one_entry_database(&format!("relations-{index}"));
        let raw = Connection::open(&path).expect("tamper relations");
        raw.execute_batch("PRAGMA foreign_keys = OFF")
            .expect("disable test-only relation enforcement");
        raw.execute_batch(mutation).expect("tamper");
        drop(raw);
        assert_evidence_reopen_fails(&path);
    }
}

fn make_reference_database(label: &str) -> (TestDir, PathBuf) {
    let directory = TestDir::new(label);
    let path = directory.database();
    let author = author(*b"sqlite-evidence-reference-seed-1");
    let older = signed_app(&author, [3; 32], "items/a", 1, b"older");
    let newer = signed_app(&author, [3; 32], "items/a", 2, b"newer");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    commit(&store, &older);
    let mixed = encode_bundle(&[older, newer]).expect("duplicate plus pruner");
    store
        .inspect(&mixed, ImportContext::new("relations"))
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("commit");
    drop(store);
    drop(session);
    (directory, path)
}

#[test]
fn reopen_rejects_reference_and_already_present_relation_corruption() {
    let mutations = [
        "UPDATE import_references SET namespace_id = zeroblob(32)",
        "UPDATE import_references SET entry_id = zeroblob(32)",
        "DELETE FROM import_references",
        "UPDATE import_dispositions SET insertion_receipt_id = 99 WHERE kind = 2",
        "UPDATE import_dispositions SET position = 9 WHERE receipt_id = 2 AND position = 1",
        "UPDATE import_receipts SET receipt_id = 99 WHERE receipt_id = 2",
    ];
    for (index, mutation) in mutations.iter().enumerate() {
        let (_directory, path) = make_reference_database(&format!("reference-{index}"));
        let raw = Connection::open(&path).expect("tamper reference");
        raw.execute_batch("PRAGMA foreign_keys = OFF")
            .expect("disable test-only relation enforcement");
        raw.execute_batch(mutation).expect("tamper");
        drop(raw);
        assert_evidence_reopen_fails(&path);
    }
}

fn make_same_batch_dominated_database(label: &str) -> (TestDir, PathBuf) {
    let directory = TestDir::new(label);
    let path = directory.database();
    let author = author(*b"sqlite-evidence-cas-seed-0000001");
    let newer = signed_app(&author, [4; 32], "items/a", 2, b"newer");
    let older = signed_app(&author, [4; 32], "items/a", 1, b"older");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    let batch = encode_bundle(&[newer, older]).expect("same-batch winner and dominated");
    store
        .inspect(&batch, ImportContext::new("same-batch"))
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("commit");
    drop(store);
    drop(session);
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("reopen database");
    let session = RiotSession::open_sqlite(database).expect("reopen session");
    let store = session.create_store().expect("reopen same-batch evidence");
    drop(store);
    drop(session);
    (directory, path)
}

#[test]
fn reopen_rejects_semantically_forged_prune_references_and_nonnull_insertion_fields() {
    let mutations = [
        "UPDATE import_references
         SET entry_id = (
             SELECT entry_id FROM import_dispositions
             WHERE receipt_id = 1 AND kind = 1
         )",
        "UPDATE import_dispositions SET insertion_receipt_id = 1 WHERE kind = 0",
        "UPDATE import_dispositions SET insertion_receipt_id = 1 WHERE kind = 1",
    ];
    for (index, mutation) in mutations.iter().enumerate() {
        let (_directory, path) =
            make_same_batch_dominated_database(&format!("semantic-forgery-{index}"));
        let raw = Connection::open(&path).expect("tamper semantic relation");
        raw.execute_batch("PRAGMA foreign_keys = OFF")
            .expect("disable test-only relation enforcement");
        raw.execute_batch(mutation).expect("tamper");
        drop(raw);
        assert_evidence_reopen_fails(&path);
    }
}

#[test]
fn reopen_rejects_generation_gaps_without_matching_forget_evidence() {
    let directory = TestDir::new("unexplained-generation-gaps");
    let path = directory.database();
    let author = author(*b"sqlite-evidence-gap-seed-0000001");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    for (key, payload) in [
        ("items/a", b"a".as_slice()),
        ("items/b", b"b".as_slice()),
        ("items/c", b"c".as_slice()),
    ] {
        commit(&store, &signed_app(&author, [5; 32], key, 1, payload));
    }
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("tamper generations");
    raw.execute_batch(
        "UPDATE import_receipts SET before_generation = 2, after_generation = 3
             WHERE receipt_id = 2;
         UPDATE import_receipts SET before_generation = 5, after_generation = 6
             WHERE receipt_id = 3;
         UPDATE evidence_meta SET generation = 6",
    )
    .expect("forge unexplained gaps");
    drop(raw);
    assert_evidence_reopen_fails(&path);
}

#[test]
fn reopen_rejects_forget_markers_assigned_to_impossible_generation_identities() {
    let directory = TestDir::new("impossible-forget-identities");
    let path = directory.database();
    let author = author(*b"sqlite-evidence-gap-seed-0000001");
    let entries = [
        signed_app(&author, [6; 32], "items/a", 1, b"a"),
        signed_app(&author, [6; 32], "items/b", 1, b"b"),
        signed_app(&author, [6; 32], "items/c", 1, b"c"),
        signed_app(&author, [6; 32], "items/d", 1, b"d"),
    ];
    let impossible_ids = [
        riot_core::willow::entry_id(&entries[1].entry_bytes),
        riot_core::willow::entry_id(&entries[2].entry_bytes),
    ];
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    for entry in &entries {
        commit(&store, entry);
    }
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("tamper forget identities");
    raw.execute_batch(
        "UPDATE import_receipts SET before_generation = 2, after_generation = 3
             WHERE receipt_id = 2;
         UPDATE import_receipts SET before_generation = 4, after_generation = 5
             WHERE receipt_id = 3;
         UPDATE import_receipts SET before_generation = 5, after_generation = 6
             WHERE receipt_id = 4;
         UPDATE evidence_meta SET generation = 6",
    )
    .expect("forge generation ledger");
    for (entry_id, forged_generation) in impossible_ids.into_iter().zip([2_i64, 4]) {
        raw.execute(
            "INSERT INTO forget_events(
                namespace_id, entry_id, forgotten_generation, restored_generation
             ) SELECT namespace_id, entry_id, ?2, NULL
               FROM accepted_entries WHERE entry_id = ?1",
            params![entry_id, forged_generation],
        )
        .expect("forge impossible immutable event");
        raw.execute(
            "INSERT INTO forgotten_entries(namespace_id, entry_id, forgotten_generation)
             SELECT namespace_id, entry_id, ?2 FROM accepted_entries WHERE entry_id = ?1",
            params![entry_id, forged_generation],
        )
        .expect("forge active marker");
        raw.execute("DELETE FROM live_entries WHERE entry_id = ?1", [entry_id])
            .expect("rewrite projection");
    }
    drop(raw);
    assert_evidence_reopen_fails(&path);
}

#[test]
fn immutable_forget_events_survive_partial_restore_and_restart() {
    let directory = TestDir::new("partial-restore-ledger");
    let path = directory.database();
    let author = author(*b"sqlite-evidence-gap-seed-0000001");
    let first = signed_app(&author, [7; 32], "items/a", 1, b"a");
    let second = signed_app(&author, [7; 32], "items/b", 1, b"b");
    let first_id = riot_core::willow::entry_id(&first.entry_bytes);
    let second_id = riot_core::willow::entry_id(&second.entry_bytes);
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    commit(&store, &first);
    commit(&store, &second);
    store.forget_entry(&first_id).expect("forget first");
    store.forget_entry(&second_id).expect("forget second");
    assert!(matches!(
        commit(&store, &first),
        CommitOutcome::Committed(_)
    ));
    assert_eq!(store.live_entry_ids().expect("live"), vec![first_id]);
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("inspect ledger");
    let first_event: (i64, Option<i64>) = raw
        .query_row(
            "SELECT forgotten_generation, restored_generation
             FROM forget_events WHERE entry_id = ?1",
            [first_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("first event");
    let second_event: (i64, Option<i64>) = raw
        .query_row(
            "SELECT forgotten_generation, restored_generation
             FROM forget_events WHERE entry_id = ?1",
            [second_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("second event");
    let active_generation: i64 = raw
        .query_row(
            "SELECT forgotten_generation FROM forgotten_entries WHERE entry_id = ?1",
            [second_id],
            |row| row.get(0),
        )
        .expect("active marker");
    assert_eq!(first_event, (3, Some(5)));
    assert_eq!(second_event, (4, None));
    assert_eq!(active_generation, 4);
    drop(raw);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("reopen database");
    let session = RiotSession::open_sqlite(database).expect("reopen session");
    let store = session.create_store().expect("reopen evidence");
    assert_eq!(
        store.live_entry_ids().expect("reopened live"),
        vec![first_id]
    );
}

#[test]
fn reopen_rejects_active_marker_and_open_event_mismatch() {
    let (directory, path) = make_one_entry_database("marker-event-mismatch");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    let entry_id = store.live_entry_ids().expect("live")[0];
    store.forget_entry(&entry_id).expect("forget");
    drop(store);
    drop(session);
    let raw = Connection::open(&path).expect("tamper marker");
    raw.execute("DELETE FROM forgotten_entries", [])
        .expect("remove active projection only");
    drop(raw);
    assert_evidence_reopen_fails(&path);
    drop(directory);
}

#[test]
fn reopen_rejects_restoration_generation_attached_to_the_wrong_receipt() {
    let directory = TestDir::new("wrong-restoration-generation");
    let path = directory.database();
    let author = author(*b"sqlite-evidence-gap-seed-0000001");
    let restored = signed_app(&author, [8; 32], "items/a", 1, b"a");
    let unrelated = signed_app(&author, [8; 32], "items/b", 1, b"b");
    let restored_id = riot_core::willow::entry_id(&restored.entry_bytes);
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    commit(&store, &restored);
    store.forget_entry(&restored_id).expect("forget");
    commit(&store, &restored);
    commit(&store, &unrelated);
    drop(store);
    drop(session);

    let raw = Connection::open(&path).expect("tamper restore generation");
    raw.execute(
        "UPDATE forget_events SET restored_generation = 4 WHERE entry_id = ?1",
        [restored_id],
    )
    .expect("move restoration to unrelated receipt");
    drop(raw);
    assert_evidence_reopen_fails(&path);
}

#[test]
fn pinned_reader_many_live_to_one_pruner_is_rejected_before_mutation_at_the_hard_wal_bound() {
    // The initial 32-entry commit fits. Replacing its live and prefix rows
    // does not: the conservative estimate must include the rows being deleted,
    // not only the single winner that remains.
    const HARD_PAGES: u32 = 465;
    let directory = TestDir::new("wal-pruning-bound");
    let path = directory.database();
    let author = author(*b"sqlite-evidence-wal-prune-seed-1");
    let app_id = [8; 32];
    let initial: Vec<_> = (0..32)
        .map(|index| signed_app(&author, app_id, &format!("items/item-{index}"), 1, b"value"))
        .collect();
    let database = RiotDatabase::open(
        &path,
        DatabaseConfig::default().with_checkpoint_pages(128, HARD_PAGES),
    )
    .expect("database");
    let session = RiotSession::open_sqlite(database).expect("session");
    let store = session.create_store().expect("store");
    let initial_bundle = encode_bundle(&initial).expect("initial bundle");
    store
        .inspect(&initial_bundle, ImportContext::new("initial"))
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("initial commit");
    assert_eq!(store.live_count().expect("initial live"), 32);

    let reader = Connection::open(&path).expect("pinned reader");
    reader.execute_batch("BEGIN").expect("begin reader");
    let _: i64 = reader
        .query_row("SELECT COUNT(*) FROM live_entries", [], |row| row.get(0))
        .expect("pin snapshot");
    let page_size = reader
        .query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
        .map(u64::try_from)
        .expect("page size query")
        .expect("positive page size");
    let pruner = signed_app(&author, app_id, "items", 2, b"winner");
    let outcome = store
        .inspect(
            &encode_bundle(std::slice::from_ref(&pruner)).expect("pruner bundle"),
            ImportContext::new("pruner"),
        )
        .expect("inspect pruner")
        .expect_preview()
        .plan_all()
        .expect("plan pruner")
        .commit();
    assert_eq!(outcome, Err(SessionError::StalePreview));
    assert_eq!(store.live_count().expect("unchanged live"), 32);
    let wal_path = PathBuf::from(format!("{}-wal", path.display()));
    let wal_bytes = fs::metadata(wal_path).map_or(0, |metadata| metadata.len());
    let absolute_bound = 32 + u64::from(HARD_PAGES) * (24 + page_size);
    assert!(
        wal_bytes <= absolute_bound,
        "WAL {wal_bytes} exceeded absolute bound {absolute_bound}"
    );
    reader.execute_batch("ROLLBACK").expect("release reader");
}
