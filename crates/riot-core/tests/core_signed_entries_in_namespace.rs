//! `EvidenceStore::signed_entries_in_namespace` — the durable read-back that
//! reconstructs a namespace's live entries in their FULL signed form
//! (entry + capability + signature + payload), byte-identical to what was
//! imported. This is the offer source for followed-site sync: the community
//! `sync_inventory` never holds owned-namespace entries, and the in-memory join
//! deliberately drops the capability/signature token, so the durable store is
//! the only honest source of the signed bytes. Requires `conformance` for
//! store construction, like the other core_import tests.

use riot_core::apps::entry::build_app_data_entry;
use riot_core::import::encode_bundle;
use riot_core::session::{
    CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, RiotSession,
};
use riot_core::store::{DatabaseConfig, RiotDatabase};
use riot_core::willow::{
    authorise_entry, decode_entry_canonic, encode_capability, encode_entry,
    generate_communal_author, EvidenceAuthor, SignedWillowEntry,
};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use willow25::groupings::Namespaced;

static NEXT_PATH: AtomicU64 = AtomicU64::new(1);

struct Scratch {
    directory: std::path::PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "riot-signed-entries-{tag}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        Self { directory }
    }

    fn db_path(&self) -> std::path::PathBuf {
        self.directory.join("riot.sqlite")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn durable_store(path: &std::path::Path) -> (RiotSession, EvidenceStore) {
    let database = RiotDatabase::open(path, DatabaseConfig::default()).unwrap();
    let session = RiotSession::open_sqlite(database).unwrap();
    let store = session.create_store().unwrap();
    (session, store)
}

/// Build, authorise and commit one app-data entry, returning the exact
/// `SignedWillowEntry` artifact that was imported (for byte-identity checks).
fn commit_app_entry(
    store: &EvidenceStore,
    author: &EvidenceAuthor,
    app_id: &[u8; 32],
    key: &str,
    payload: &[u8],
    timestamp: u64,
) -> SignedWillowEntry {
    let entry = build_app_data_entry(author, app_id, key, timestamp, payload).expect("entry");
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let signed = SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    };
    let bundle_bytes = encode_bundle(std::slice::from_ref(&signed)).expect("encode bundle");
    let preview = match store
        .inspect(&bundle_bytes, ImportContext::new("test-route"))
        .expect("inspect")
    {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("rejected: {r:?}"),
    };
    let plan = preview.plan_all().expect("plan_all");
    match plan.commit().expect("commit") {
        CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
    }
    signed
}

fn namespace_of(signed: &SignedWillowEntry) -> [u8; 32] {
    *decode_entry_canonic(&signed.entry_bytes)
        .expect("decode")
        .namespace_id()
        .as_bytes()
}

#[test]
fn returns_the_persisted_signed_form_byte_identical_to_the_imported_artifact() {
    let scratch = Scratch::new("byte-identity");
    let (_session, store) = durable_store(&scratch.db_path());
    let author = generate_communal_author().expect("author");

    let original = commit_app_entry(&store, &author, &[7u8; 32], "items/a", b"{\"v\":1}", 1);
    let namespace = namespace_of(&original);

    let read_back = store
        .signed_entries_in_namespace(&namespace)
        .expect("accessor")
        .expect("a durable store answers with Some");

    assert_eq!(read_back.len(), 1, "one live entry in the namespace");
    let got = &read_back[0];
    assert_eq!(
        got.entry_bytes, original.entry_bytes,
        "entry bytes must be byte-identical to the imported artifact"
    );
    assert_eq!(
        got.capability_bytes, original.capability_bytes,
        "capability bytes must be byte-identical (never re-encoded)"
    );
    assert_eq!(
        got.signature, original.signature,
        "signature must be the persisted verbatim signature"
    );
    assert_eq!(
        got.payload_bytes, original.payload_bytes,
        "payload bytes must be the exact imported payload"
    );
}

#[test]
fn excludes_entries_from_other_namespaces() {
    let scratch = Scratch::new("cross-ns");
    let (_session, store) = durable_store(&scratch.db_path());
    let wanted_author = generate_communal_author().expect("author");
    let other_author = generate_communal_author().expect("author");

    let wanted = commit_app_entry(&store, &wanted_author, &[1u8; 32], "a", b"1", 1);
    let other = commit_app_entry(&store, &other_author, &[1u8; 32], "b", b"2", 2);
    let wanted_ns = namespace_of(&wanted);
    let other_ns = namespace_of(&other);
    assert_ne!(wanted_ns, other_ns, "two authors yield two namespaces");

    let read_back = store
        .signed_entries_in_namespace(&wanted_ns)
        .expect("accessor")
        .expect("a durable store answers with Some");

    assert_eq!(read_back.len(), 1, "only the wanted namespace's entry");
    assert_eq!(read_back[0].entry_bytes, wanted.entry_bytes);
    assert!(
        read_back
            .iter()
            .all(|signed| namespace_of(signed) == wanted_ns),
        "no other namespace's entry may leak into the offer"
    );
}

#[test]
fn returns_only_live_entries_not_forgotten_ones() {
    let scratch = Scratch::new("live-only");
    let (_session, store) = durable_store(&scratch.db_path());
    let author = generate_communal_author().expect("author");

    let signed = commit_app_entry(&store, &author, &[3u8; 32], "gone", b"x", 1);
    let namespace = namespace_of(&signed);
    let id = store.live_entry_ids().expect("ids")[0];
    store.forget_entry(&id).expect("forget");

    let read_back = store
        .signed_entries_in_namespace(&namespace)
        .expect("accessor")
        .expect("a durable store answers with Some");

    assert!(
        read_back.is_empty(),
        "a forgotten entry is not live and must not be offered"
    );
}

#[test]
fn memory_backed_store_returns_none_not_an_empty_offer() {
    let session = RiotSession::open().expect("memory session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    let signed = commit_app_entry(&store, &author, &[5u8; 32], "m", b"y", 1);
    let namespace = namespace_of(&signed);
    // The entry IS live in memory, but the join retains no capability/signature
    // token. The accessor must say so EXPLICITLY with `None` — not an empty Vec,
    // which would be indistinguishable from "durable store, nothing to offer"
    // and would let a caller silently wire followed-site sync onto a memory
    // profile.
    assert_eq!(store.live_entry_ids().expect("ids").len(), 1);

    let read_back = store
        .signed_entries_in_namespace(&namespace)
        .expect("accessor");

    assert!(
        read_back.is_none(),
        "a memory-backed store cannot reconstruct the signed form: None, not empty"
    );
}
