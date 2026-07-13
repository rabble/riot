use riot_core::apps::entry::build_app_data_entry;
use riot_core::import::encode_bundle;
use riot_core::session::{CommitOutcome, EvidenceStore, ImportContext, RiotSession};
use riot_core::store::{DatabaseConfig, RiotDatabase};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, EvidenceAuthor, SignedWillowEntry,
};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PATH: AtomicU64 = AtomicU64::new(1);

fn author() -> EvidenceAuthor {
    riot_core::willow::generate_communal_author().expect("production author")
}

#[test]
fn forgotten_pruned_entry_only_restores_after_the_newer_winner_is_forgotten() {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "riot-sqlite-forget-pruned-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("riot.sqlite");
    let memory_session = RiotSession::open().unwrap();
    let memory = memory_session.create_store().unwrap();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    let author = author();
    let older = signed(&author, 1, b"old");
    let newer = signed(&author, 2, b"new");

    commit(&memory, &older);
    commit(&sqlite, &older);
    let older_id = memory.live_entry_ids().unwrap()[0];
    memory.forget_entry(&older_id).unwrap();
    sqlite.forget_entry(&older_id).unwrap();
    commit(&memory, &newer);
    commit(&sqlite, &newer);
    let newer_id = memory.live_entry_ids().unwrap()[0];
    assert!(matches!(
        commit(&memory, &older),
        CommitOutcome::NoChanges(_)
    ));
    assert!(matches!(
        commit(&sqlite, &older),
        CommitOutcome::NoChanges(_)
    ));
    assert_same(&memory, &sqlite);
    assert_eq!(sqlite.live_entry_ids().unwrap(), vec![newer_id]);

    memory.forget_entry(&newer_id).unwrap();
    sqlite.forget_entry(&newer_id).unwrap();
    assert!(matches!(
        commit(&memory, &older),
        CommitOutcome::Committed(_)
    ));
    assert!(matches!(
        commit(&sqlite, &older),
        CommitOutcome::Committed(_)
    ));
    assert_same(&memory, &sqlite);
    assert_eq!(sqlite.live_entry_ids().unwrap(), vec![older_id]);

    drop(sqlite);
    drop(sqlite_session);
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    assert_same(&memory, &sqlite);
    drop(sqlite);
    drop(sqlite_session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn restoring_a_forgotten_winner_records_and_prunes_the_intervening_live_victim() {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "riot-sqlite-restore-prunes-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("riot.sqlite");
    let memory_session = RiotSession::open().unwrap();
    let memory = memory_session.create_store().unwrap();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    let author = author();
    let winner = signed(&author, 2, b"winner");
    let victim = signed(&author, 1, b"victim");

    let winner_receipt = commit(&memory, &winner);
    commit(&sqlite, &winner);
    let winner_id = match winner_receipt {
        CommitOutcome::Committed(receipt) => receipt.dispositions[0].entry_id,
        CommitOutcome::NoChanges(_) => panic!("winner duplicate"),
    };
    memory.forget_entry(&winner_id).unwrap();
    sqlite.forget_entry(&winner_id).unwrap();
    let victim_receipt = commit(&memory, &victim);
    commit(&sqlite, &victim);
    let victim_id = match victim_receipt {
        CommitOutcome::Committed(receipt) => receipt.dispositions[0].entry_id,
        CommitOutcome::NoChanges(_) => panic!("victim duplicate"),
    };
    let restored_memory = commit(&memory, &winner);
    let restored_sqlite = commit(&sqlite, &winner);
    assert_eq!(restored_memory, restored_sqlite);
    let receipt = match restored_sqlite {
        CommitOutcome::Committed(receipt) => receipt,
        CommitOutcome::NoChanges(_) => panic!("forgotten winner was not restored"),
    };
    assert!(matches!(
        receipt.dispositions[0].disposition,
        riot_core::session::EntryDisposition::AppliedAtCommit { ref pruned_entry_ids }
            if pruned_entry_ids == &vec![victim_id]
    ));
    assert_eq!(sqlite.live_entry_ids().unwrap(), vec![winner_id]);
    drop(sqlite);
    drop(sqlite_session);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    assert_same(&memory, &sqlite);
    assert_eq!(sqlite.live_entry_ids().unwrap(), vec![winner_id]);
    drop(sqlite);
    drop(sqlite_session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_forget_gap_remains_attributable_after_an_unrelated_receipt() {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "riot-sqlite-delayed-restore-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("riot.sqlite");
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    let author = author();
    let forgotten = signed_at(&author, "items/a", 2, b"forgotten");
    let unrelated = signed_at(&author, "items/b", 1, b"unrelated");

    let first = commit(&sqlite, &forgotten);
    let forgotten_id = match first {
        CommitOutcome::Committed(receipt) => receipt.dispositions[0].entry_id,
        CommitOutcome::NoChanges(_) => panic!("first entry was a duplicate"),
    };
    sqlite.forget_entry(&forgotten_id).unwrap();
    commit(&sqlite, &unrelated);
    commit(&sqlite, &forgotten);
    drop(sqlite);
    drop(sqlite_session);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    assert!(sqlite.live_entry_ids().unwrap().contains(&forgotten_id));
    drop(sqlite);
    drop(sqlite_session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_dominated_reimport_does_not_clear_the_marker_needed_for_later_restoration() {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "riot-sqlite-dominated-reimport-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("riot.sqlite");
    let memory_session = RiotSession::open().unwrap();
    let memory = memory_session.create_store().unwrap();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    let author = author();
    let forgotten = signed_at(&author, "items/a", 1, b"forgotten");
    let dominator = signed_at(&author, "items/a", 2, b"dominator");
    let unrelated = signed_at(&author, "items/b", 1, b"unrelated");

    let first = commit(&memory, &forgotten);
    commit(&sqlite, &forgotten);
    let forgotten_id = match first {
        CommitOutcome::Committed(receipt) => receipt.dispositions[0].entry_id,
        CommitOutcome::NoChanges(_) => panic!("first entry was a duplicate"),
    };
    memory.forget_entry(&forgotten_id).unwrap();
    sqlite.forget_entry(&forgotten_id).unwrap();
    let dominant = commit(&memory, &dominator);
    commit(&sqlite, &dominator);
    let dominator_id = match dominant {
        CommitOutcome::Committed(receipt) => receipt.dispositions[0].entry_id,
        CommitOutcome::NoChanges(_) => panic!("dominator was a duplicate"),
    };

    commit_batch(&memory, &[forgotten.clone(), unrelated.clone()]);
    commit_batch(&sqlite, &[forgotten.clone(), unrelated]);
    assert!(!memory.live_entry_ids().unwrap().contains(&forgotten_id));
    assert!(!sqlite.live_entry_ids().unwrap().contains(&forgotten_id));

    memory.forget_entry(&dominator_id).unwrap();
    sqlite.forget_entry(&dominator_id).unwrap();
    assert!(matches!(
        commit(&memory, &forgotten),
        CommitOutcome::Committed(_)
    ));
    assert!(matches!(
        commit(&sqlite, &forgotten),
        CommitOutcome::Committed(_)
    ));
    assert_same(&memory, &sqlite);
    assert!(sqlite.live_entry_ids().unwrap().contains(&forgotten_id));
    drop(sqlite);
    drop(sqlite_session);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    assert_same(&memory, &sqlite);
    assert!(sqlite.live_entry_ids().unwrap().contains(&forgotten_id));
    drop(sqlite);
    drop(sqlite_session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_historical_nonforgotten_duplicate_never_resurrects_in_a_mixed_commit() {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "riot-sqlite-historical-duplicate-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("riot.sqlite");
    let memory_session = RiotSession::open().unwrap();
    let memory = memory_session.create_store().unwrap();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    let author = author();
    let historical = signed_at(&author, "items/a", 1, b"historical");
    let newer = signed_at(&author, "items/a", 2, b"newer");
    let unrelated = signed_at(&author, "items/b", 1, b"unrelated");
    let newer_id = riot_core::willow::entry_id(&newer.entry_bytes);
    let unrelated_id = riot_core::willow::entry_id(&unrelated.entry_bytes);

    commit(&memory, &historical);
    commit(&sqlite, &historical);
    commit(&memory, &newer);
    commit(&sqlite, &newer);
    memory.forget_entry(&newer_id).unwrap();
    sqlite.forget_entry(&newer_id).unwrap();
    commit_batch(&memory, &[historical.clone(), unrelated.clone()]);
    commit_batch(&sqlite, &[historical, unrelated]);
    assert_same(&memory, &sqlite);
    assert_eq!(sqlite.live_entry_ids().unwrap(), vec![unrelated_id]);
    drop(sqlite);
    drop(sqlite_session);

    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    assert_eq!(sqlite.live_entry_ids().unwrap(), vec![unrelated_id]);
    assert!(matches!(
        commit(&sqlite, &newer),
        CommitOutcome::Committed(_)
    ));
    assert!(sqlite.live_entry_ids().unwrap().contains(&newer_id));
    drop(sqlite);
    drop(sqlite_session);
    fs::remove_dir_all(directory).unwrap();
}

fn signed(author: &EvidenceAuthor, timestamp: u64, payload: &[u8]) -> SignedWillowEntry {
    signed_at(author, "items/a", timestamp, payload)
}

fn signed_at(
    author: &EvidenceAuthor,
    key: &str,
    timestamp: u64,
    payload: &[u8],
) -> SignedWillowEntry {
    let entry = build_app_data_entry(author, &[3; 32], key, timestamp, payload).unwrap();
    let authorised = authorise_entry(author, entry).unwrap();
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

fn commit(store: &EvidenceStore, entry: &SignedWillowEntry) -> CommitOutcome {
    commit_batch(store, std::slice::from_ref(entry))
}

fn commit_batch(store: &EvidenceStore, entries: &[SignedWillowEntry]) -> CommitOutcome {
    store
        .inspect(
            &encode_bundle(entries).unwrap(),
            ImportContext::new("differential"),
        )
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap()
}

fn assert_same(memory: &EvidenceStore, sqlite: &EvidenceStore) {
    assert_eq!(memory.generation().unwrap(), sqlite.generation().unwrap());
    assert_eq!(
        memory.receipt_count().unwrap(),
        sqlite.receipt_count().unwrap()
    );
    let mut memory_ids = memory.live_entry_ids().unwrap();
    let mut sqlite_ids = sqlite.live_entry_ids().unwrap();
    memory_ids.sort();
    sqlite_ids.sort();
    assert_eq!(memory_ids, sqlite_ids);
}

#[test]
fn memory_oracle_and_sqlite_match_for_recency_pruning_duplicates_and_forgetting() {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "riot-sqlite-differential-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("riot.sqlite");

    let memory_session = RiotSession::open().unwrap();
    let memory = memory_session.create_store().unwrap();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    let author = author();
    let older = signed(&author, 1, b"old");
    let newer = signed(&author, 2, b"new");

    assert_eq!(commit(&memory, &older), commit(&sqlite, &older));
    assert_same(&memory, &sqlite);
    assert_eq!(commit(&memory, &newer), commit(&sqlite, &newer));
    assert_same(&memory, &sqlite);
    assert_eq!(commit(&memory, &newer), commit(&sqlite, &newer));
    assert_same(&memory, &sqlite);

    let live_id = memory.live_entry_ids().unwrap()[0];
    memory.forget_entry(&live_id).unwrap();
    sqlite.forget_entry(&live_id).unwrap();
    assert_same(&memory, &sqlite);
    assert_eq!(commit(&memory, &newer), commit(&sqlite, &newer));
    assert_same(&memory, &sqlite);

    drop(sqlite);
    drop(sqlite_session);
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let sqlite_session = RiotSession::open_sqlite(database).unwrap();
    let sqlite = sqlite_session.create_store().unwrap();
    assert_same(&memory, &sqlite);
    fs::remove_dir_all(directory).unwrap();
}
