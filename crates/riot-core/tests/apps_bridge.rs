//! `AppDataBridge` contract: namespace-scoped put/get/list over the same
//! inspect/plan/commit pipeline every other entry uses. Requires
//! `conformance` for `RiotSession`/store construction.

use riot_core::apps::bridge::AppDataBridge;
use riot_core::session::RiotSession;
use riot_core::willow::generate_communal_author;

#[test]
fn put_then_get_round_trips() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");
    let app_id = [7u8; 32];

    AppDataBridge::put(&store, &author, &app_id, "items/a", 1, b"{\"done\":false}").expect("put");

    let value = AppDataBridge::get(&store, &app_id, "items/a").expect("get");
    assert_eq!(value, Some(b"{\"done\":false}".to_vec()));
}

#[test]
fn get_on_missing_key_returns_none() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let app_id = [7u8; 32];

    let value = AppDataBridge::get(&store, &app_id, "items/missing").expect("get");
    assert_eq!(value, None);
}

#[test]
fn list_only_returns_entries_for_the_requesting_app() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    AppDataBridge::put(&store, &author, &[7u8; 32], "items/a", 1, b"1").expect("put");
    AppDataBridge::put(&store, &author, &[9u8; 32], "items/b", 2, b"2").expect("put");

    let listed = AppDataBridge::list(&store, &[7u8; 32], "items").expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, "items/a");
    assert_eq!(listed[0].1, b"1");
}

#[test]
fn put_rejects_traversal_like_key_before_touching_willow() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    let result = AppDataBridge::put(&store, &author, &[7u8; 32], "../escape", 1, b"x");
    assert!(result.is_err());

    let listed = AppDataBridge::list(&store, &[7u8; 32], "items").unwrap_or_default();
    assert!(listed.is_empty());
}

#[test]
fn independent_puts_for_different_apps_do_not_interfere() {
    // The plan's original version of this test manufactured a pending
    // ImportPreview from an empty bundle to prove put's inspect/plan/commit
    // call is self-contained; `InspectOutcome::Rejected` isn't `Default` in
    // the real API, so per the plan's fallback this asserts the same
    // property directly: consecutive puts for different app_ids succeed
    // with no intervening state reset.
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    AppDataBridge::put(&store, &author, &[1u8; 32], "unrelated", 1, b"x").expect("first put");
    AppDataBridge::put(&store, &author, &[7u8; 32], "items/a", 2, b"y").expect("second put");

    assert_eq!(
        AppDataBridge::get(&store, &[1u8; 32], "unrelated").expect("get"),
        Some(b"x".to_vec())
    );
    assert_eq!(
        AppDataBridge::get(&store, &[7u8; 32], "items/a").expect("get"),
        Some(b"y".to_vec())
    );
}

#[test]
fn newer_put_to_the_same_key_wins() {
    // Last-write-wins is Willow's ordinary prune semantics; the bridge
    // must surface the newer payload, not both.
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");
    let app_id = [7u8; 32];

    AppDataBridge::put(&store, &author, &app_id, "items/a", 1, b"old").expect("put old");
    AppDataBridge::put(&store, &author, &app_id, "items/a", 2, b"new").expect("put new");

    let value = AppDataBridge::get(&store, &app_id, "items/a").expect("get");
    assert_eq!(value, Some(b"new".to_vec()));

    let listed = AppDataBridge::list(&store, &app_id, "items").expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1, b"new");
}
