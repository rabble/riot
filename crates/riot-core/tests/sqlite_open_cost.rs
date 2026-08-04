//! How long does opening a profile take, as a function of how much is in it?
//!
//! `EvidenceStore::load` selects EVERY accepted entry and runs
//! `validate_stored_entry` on each one: a canonical re-decode, a capability
//! decode, an ed25519 signature verification, and a payload re-hash. That is a
//! full cryptographic re-verification of the whole store on every launch, and
//! it is the reason `MAX_ACCEPTED_ENTRIES`/`MAX_LIVE_ENTRIES` are 1024 — the
//! constant is a startup-time budget wearing a corruption check's clothes, which
//! is why crossing it returns `CorruptDatabase` and bricks the profile rather
//! than reporting that the store is full.
//!
//! These are measurements, not assertions — they print a table and are
//! `#[ignore]`d so they never slow the default suite (a real risk: tarpaulin
//! runs with `--timeout 300`). Run them deliberately:
//!
//! ```text
//! cargo test -p riot-core --test sqlite_open_cost -- --ignored --nocapture
//! ```

use riot_core::apps::entry::build_app_data_entry;
use riot_core::import::encode_bundle;
use riot_core::session::{EvidenceStore, ImportContext, RiotSession};
use riot_core::store::{DatabaseConfig, RiotDatabase};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, EvidenceAuthor, SignedWillowEntry,
};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static NEXT_PATH: AtomicU64 = AtomicU64::new(1);

/// Entries per bundle. Bounded by `MAX_BUNDLE_ENTRIES` (64) — a fourth capacity
/// ceiling, after MAX_SYNC_IDS (64), MAX_ACCEPTED_ENTRIES (1024) and
/// MAX_BUNDLE_BYTES (8 MB). Batching also matters in the other direction:
/// `MAX_IMPORT_RECEIPTS` is 256 and every commit writes a receipt, so committing
/// one entry at a time would trip THAT ceiling first and measure the wrong wall.
const BATCH: usize = 64;

fn author() -> EvidenceAuthor {
    riot_core::willow::generate_communal_author().expect("production author")
}

fn signed(author: &EvidenceAuthor, index: usize, payload: &[u8]) -> SignedWillowEntry {
    // A DISTINCT path per entry: same-path entries prune each other, and a store
    // that prunes everything it is given measures nothing.
    let entry = build_app_data_entry(
        author,
        &[3; 32],
        &format!("items/{index:08}"),
        index as u64 + 1,
        payload,
    )
    .unwrap();
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

fn commit_batch(store: &EvidenceStore, entries: &[SignedWillowEntry]) {
    store
        .inspect(
            &encode_bundle(entries).unwrap(),
            ImportContext::new("open-cost"),
        )
        .unwrap()
        .expect_preview()
        .plan_all()
        .unwrap()
        .commit()
        .unwrap();
}

/// Builds a durable store holding `count` live entries, then times how long a
/// FRESH open of that same database takes. Returns (build, open) in millis.
fn measure_batched(count: usize, payload_bytes: usize, batch_size: usize) -> (u128, u128) {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    let directory =
        std::env::temp_dir().join(format!("riot-open-cost-{}-{sequence}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("riot.sqlite");
    let author = author();
    let payload = vec![7u8; payload_bytes];

    let build_started = Instant::now();
    {
        let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
        let session = RiotSession::open_sqlite(database).unwrap();
        let store = session.create_store().unwrap();
        let mut batch = Vec::with_capacity(batch_size);
        for index in 0..count {
            batch.push(signed(&author, index, &payload));
            if batch.len() == batch_size {
                commit_batch(&store, &batch);
                batch.clear();
            }
        }
        if !batch.is_empty() {
            commit_batch(&store, &batch);
        }
    }
    let build = build_started.elapsed().as_millis();

    // The measurement: a cold open of a store that already holds `count` entries.
    let open_started = Instant::now();
    let database = RiotDatabase::open(&path, DatabaseConfig::default()).unwrap();
    let session = RiotSession::open_sqlite(database).unwrap();
    let store = session.create_store().unwrap();
    let live = store.live_entry_ids().unwrap().len();
    let open = open_started.elapsed().as_millis();

    assert_eq!(live, count, "every entry should be live at a distinct path");
    let _ = fs::remove_dir_all(&directory);
    (build, open)
}

fn measure(count: usize, payload_bytes: usize) -> (u128, u128) {
    measure_batched(count, payload_bytes, BATCH)
}

/// DOES open cost scale with the number of RECEIPTS, independently of entry
/// count? `validate_relational_state` calls `replay_final_live` once per receipt
/// (`evidence.rs:1376`, inside the `for receipt` loop at `:1257`), and that
/// function is an all-pairs dominance scan over the whole live set. If that is
/// the dominant cost, holding entries fixed and shrinking the batch — which
/// makes MORE receipts for the SAME data — must make opening dramatically
/// slower. If open time barely moves, the per-receipt theory is wrong.
#[test]
#[ignore = "measurement, not an assertion; run with --ignored --nocapture"]
fn profile_open_cost_by_receipt_count() {
    println!();
    println!("  entries | batch | receipts |     OPEN");
    println!("  --------+-------+----------+----------");
    for batch_size in [64usize, 32, 16, 8] {
        let entries = 512;
        let (_, open) = measure_batched(entries, 64, batch_size);
        println!(
            "  {entries:7} | {batch_size:5} | {:8} | {open:6} ms",
            entries.div_ceil(batch_size)
        );
    }
    println!();
    println!("  Same 512 entries every row. Only the receipt count changes.");
    println!();
}

#[test]
#[ignore = "measurement, not an assertion; run with --ignored --nocapture"]
fn profile_open_cost_by_store_size() {
    println!();
    println!("  entries | payload |    build |     OPEN | open per entry");
    println!("  --------+---------+----------+----------+---------------");
    for count in [100usize, 250, 500, 1000] {
        let (build, open) = measure(count, 64);
        println!(
            "  {count:7} |    64 B | {build:6} ms | {open:6} ms | {:.3} ms",
            open as f64 / count as f64
        );
    }
    println!();
    println!("  1024 is MAX_ACCEPTED_ENTRIES/MAX_LIVE_ENTRIES: past it, load()");
    println!("  returns CorruptDatabase and the profile will not open at all.");
    println!();
}

/// Payload size is charged separately from entry count — `validate_stored_entry`
/// re-hashes every payload — so a store of photos and a store of one-line posts
/// do not open at the same speed even at the same entry count.
#[test]
#[ignore = "measurement, not an assertion; run with --ignored --nocapture"]
fn profile_open_cost_by_payload_size() {
    println!();
    println!("  entries | payload |     OPEN");
    println!("  --------+---------+----------");
    for payload in [64usize, 1_024, 16_384] {
        let (_, open) = measure(500, payload);
        println!("  {:7} | {payload:6} B | {open:6} ms", 500);
    }
    println!();
}
