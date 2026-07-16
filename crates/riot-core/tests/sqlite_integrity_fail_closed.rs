//! Integrity-gate coverage for the durable SQLite store.
//!
//! These tests exercise the `quick_check` / `integrity_check` fail-closed paths
//! and one deep evidence-replay invariant that the other fail-closed suites do
//! not reach:
//!
//!  * `quick_check` reporting a non-"ok" string during open (a CHECK-constraint
//!    violation SQLite surfaces structurally) must abort the open.
//!  * `integrity_check` must report `false` when the reader observes page-level
//!    corruption introduced after a clean open.
//!  * a receipt that claims an entry was *applied and live* while the batch
//!    replay proves it was dominated must fail closed at load.
//!
//! Every corruption is physically real: a raw `rusqlite::Connection` writing a
//! constraint-violating row, or raw bytes overwriting content pages on disk.

use riot_core::apps::entry::build_app_data_entry;
use riot_core::import::encode_bundle;
use riot_core::session::{ImportContext, RiotSession, SessionError};
use riot_core::store::{CheckpointMode, DatabaseConfig, DatabaseError, RiotDatabase};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, EvidenceAuthor, SignedWillowEntry,
};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "riot-integrity-{label}-{}-{sequence}",
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

fn author() -> EvidenceAuthor {
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

fn commit(store: &riot_core::session::EvidenceStore, entry: &SignedWillowEntry) {
    store
        .inspect(
            &encode_bundle(std::slice::from_ref(entry)).expect("bundle"),
            ImportContext::new("integrity-test"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("commit");
}

fn commit_batch(store: &riot_core::session::EvidenceStore, entries: &[SignedWillowEntry]) {
    store
        .inspect(
            &encode_bundle(entries).expect("bundle"),
            ImportContext::new("integrity-batch"),
        )
        .expect("inspect")
        .expect_preview()
        .plan_all()
        .expect("plan")
        .commit()
        .expect("commit");
}

// ---------------------------------------------------------------------------
// database.rs: quick_check / integrity_check fail closed
// ---------------------------------------------------------------------------

#[test]
fn quick_check_reports_a_check_violation_string_and_open_fails_closed() {
    // A CHECK-constraint violation cannot be written through the managed API, so
    // it is forged with `PRAGMA ignore_check_constraints`. On the next open the
    // integrity gate runs `PRAGMA quick_check`, which returns a non-"ok" string
    // ("CHECK constraint failed ...") that the store maps to `CorruptDatabase`
    // rather than trusting the row.
    let directory = TestDir::new("quick-check-string");
    let path = directory.database();
    let author = author();
    let signed = signed_app(&author, [30; 32], "items/a", 1, b"payload");
    {
        let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
        let session = RiotSession::open_sqlite(database).expect("session");
        let store = session.create_store().expect("store");
        commit(&store, &signed);
    }

    let raw = Connection::open(&path).expect("forge check violation");
    raw.execute_batch("PRAGMA ignore_check_constraints = ON;")
        .expect("suspend check enforcement for the tamper only");
    let changed = raw
        .execute("UPDATE import_dispositions SET kind = 3 WHERE kind = 0", [])
        .expect("write an out-of-range disposition kind");
    assert_eq!(changed, 1, "the fixture has one applied disposition");
    drop(raw);

    let error = RiotDatabase::open(&path, DatabaseConfig::default())
        .expect_err("a CHECK violation must fail the integrity gate on open");
    assert!(matches!(error, DatabaseError::CorruptDatabase));
}

#[test]
fn integrity_check_reports_false_for_page_corruption_seen_after_open() {
    // `open` gates on the same `quick_check`, so the file must be clean when the
    // handle is created and corrupted only afterwards. A read-only handle then
    // observes the damaged pages and reports an unhealthy database instead of
    // silently serving corrupt content.
    let directory = TestDir::new("integrity-false");
    let path = directory.database();
    {
        let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
        database
            .set_local_state("k", &vec![7_u8; 48 * 1024])
            .expect("seed overflow pages");
        database
            .checkpoint(CheckpointMode::Truncate)
            .expect("fold WAL into the main file");
        drop(database);
    }

    let read_only = RiotDatabase::open_read_only(&path, DatabaseConfig::default())
        .expect("clean read-only open");
    corrupt_content_pages(&path);
    let healthy = read_only
        .integrity_check()
        .expect("integrity_check completes");
    assert!(
        !healthy,
        "page corruption must report an unhealthy database"
    );
}

fn corrupt_content_pages(path: &Path) {
    let mut bytes = fs::read(path).expect("read database bytes");
    assert!(
        bytes.len() > 4096,
        "the seeded database must have content pages to damage"
    );
    // Leave the first page (the file header + schema root) intact so the file
    // remains openable; scramble every content page after it.
    for byte in bytes.iter_mut().skip(2048) {
        *byte ^= 0x5a;
    }
    fs::write(path, &bytes).expect("write corrupted pages");
}

// ---------------------------------------------------------------------------
// evidence.rs: an "applied and live" claim contradicted by the replay
// ---------------------------------------------------------------------------

#[test]
fn reopen_rejects_an_applied_disposition_for_an_entry_the_replay_proves_dominated() {
    // Commit two entries on one path in a single batch: the newer wins (kind 0,
    // live) and the older is dominated on arrival (kind 1, not live). Rewrite the
    // older row to masquerade as an ordinary applied-and-live insert — both the
    // disposition kind and the dominated flag are flipped to values that satisfy
    // every CHECK. The batch replay still proves the older entry is dominated, so
    // its "applied and live" claim must fail closed at load.
    let directory = TestDir::new("applied-but-dominated");
    let path = directory.database();
    let author = author();
    let newer = signed_app(&author, [31; 32], "items/a", 2, b"newer");
    let older = signed_app(&author, [31; 32], "items/a", 1, b"older");
    let older_id = riot_core::willow::entry_id(&older.entry_bytes);
    {
        let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
        let session = RiotSession::open_sqlite(database).expect("session");
        let store = session.create_store().expect("store");
        commit_batch(&store, &[newer, older]);
    }

    let raw = Connection::open(&path).expect("tamper");
    raw.execute_batch("PRAGMA foreign_keys = OFF")
        .expect("disable test-only relation enforcement");
    // kind 1 -> 0 (DominatedAtCommit reinterpreted as AppliedAtCommit) and clear
    // the dominated flag so the applied-arm preconditions all pass; only the
    // final replay-vs-live cross-check can catch the lie.
    let changed = raw
        .execute(
            "UPDATE import_dispositions SET kind = 0 WHERE entry_id = ?1",
            [older_id],
        )
        .expect("reinterpret the dominated disposition as applied");
    assert_eq!(changed, 1);
    raw.execute(
        "UPDATE accepted_entries SET dominated_on_arrival = 0 WHERE entry_id = ?1",
        [older_id],
    )
    .expect("clear the dominated flag");
    drop(raw);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).expect("physical open");
    let session = RiotSession::open_sqlite(database).expect("session handle");
    assert!(matches!(
        session.create_store(),
        Err(SessionError::Internal)
    ));
}
