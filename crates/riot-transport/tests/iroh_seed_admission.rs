//! The full p2p pipeline: a follower syncs a namespace from a seed over real
//! iroh QUIC AND admits the received entries through riot-core's real
//! preview→commit boundary into a durable store. Proves transport + admission
//! end to end — the entries actually land, verified, in the follower's store.
//!
//! The admission chain (inspect→expect_preview→plan_all→commit) is synchronous,
//! so it runs inside pump's on_bundle hook between awaits — never holding an
//! arbiter across an await (spec §5.4).

mod common;

use riot_core::session::{ImportContext, RiotSession};
use riot_core::sync::ByteSyncSession;
use riot_core::willow::generate_communal_author;
use riot_transport::iroh::{bind, bind_seed, dialable_addr, sync_accept, sync_connect};

use common::signed;

#[tokio::test(flavor = "multi_thread")]
async fn follower_syncs_from_a_seed_and_admits_entries_into_its_store() {
    let author = generate_communal_author().unwrap();
    let namespace = author.identity().namespace_id;

    // Seed holds two signed entries and accepts on a STABLE identity.
    let seed_session =
        ByteSyncSession::new(namespace, vec![signed(&author, 1), signed(&author, 2)]).unwrap();
    let seed = bind_seed([0x55; 32]).await.expect("bind seed");
    let seed_addr = dialable_addr(&seed).await;
    let seed_task = tokio::spawn(async move { sync_accept(&seed, seed_session, |_| true).await });

    // Follower: empty session + a real durable store to admit into.
    let follower = bind().await.expect("bind follower");
    let follower_session = ByteSyncSession::new(namespace, vec![]).unwrap();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();

    let follower_session = sync_connect(&follower, seed_addr, follower_session, |bytes| {
        // The admission hook: run the REAL preview→commit boundary. Communal
        // entries with valid signatures admit; anything else fails closed.
        store
            .inspect(bytes, ImportContext::new("iroh-sync"))
            .expect("inspect")
            .expect_preview()
            .plan_all()
            .expect("plan")
            .commit()
            .expect("commit");
        true
    })
    .await
    .expect("follower sync");

    let seed_session = seed_task.await.expect("seed task").expect("seed sync");

    assert!(seed_session.is_terminal() && follower_session.is_terminal());
    assert_eq!(
        store.live_count().unwrap(),
        2,
        "the two seeded entries are now live in the follower's store, admitted over iroh"
    );
}
