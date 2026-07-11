//! Path-prefix queries over the live view. Requires `conformance` for
//! `RiotSession`/store construction, same as the other core_import tests.

use riot_core::apps::entry::{app_data_path, build_app_data_entry};
use riot_core::import::encode_bundle;
use riot_core::session::{
    CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, RiotSession,
};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, EvidenceAuthor,
    SignedWillowEntry,
};

fn commit_app_entry(
    store: &EvidenceStore,
    author: &EvidenceAuthor,
    app_id: &[u8; 32],
    key: &str,
    payload: &[u8],
    timestamp: u64,
) {
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
}

#[test]
fn entries_with_prefix_returns_only_matching_live_entries() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    commit_app_entry(&store, &author, &[7u8; 32], "items/a", b"{}", 1);
    commit_app_entry(&store, &author, &[7u8; 32], "items/b", b"{}", 2);
    commit_app_entry(&store, &author, &[9u8; 32], "items/c", b"{}", 3);

    let prefix = app_data_path(&[7u8; 32], "items").expect("prefix");
    let matches = store.entries_with_prefix(&prefix).expect("query");

    assert_eq!(matches.len(), 2);
}

#[test]
fn entries_with_prefix_is_empty_when_nothing_matches() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let prefix = app_data_path(&[1u8; 32], "items").expect("prefix");

    let matches = store.entries_with_prefix(&prefix).expect("query");

    assert!(matches.is_empty());
}
