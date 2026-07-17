//! The real iroh (QUIC) FrameChannel: two in-process endpoints reconcile a
//! namespace over an actual encrypted connection. Same frames as the duplex
//! test — this proves iroh carries them (spec §8: loopback + iroh identical).

mod common;

use std::sync::{Arc, Mutex};

use riot_core::sync::ByteSyncSession;
use riot_core::willow::generate_communal_author;
use riot_transport::iroh::{bind, dial_with_ticket, dialable_addr, sync_accept, sync_connect};
use riot_transport::ticket::{mint, Capabilities, TransportBlocked};
use riot_transport::TransportError;

use common::signed;

#[tokio::test(flavor = "multi_thread")]
async fn require_arti_ticket_fails_closed_before_any_dial() {
    // A require:arti site, a client with only iroh: dial_with_ticket must REFUSE
    // before opening a connection — the peer here is reachable, yet no sync runs.
    let key = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
    let ticket = mint(&key, [0x11; 32], "arti", 1, 10_000, [0x22; 32], None);

    let follower = bind().await.unwrap();
    let seed = bind().await.unwrap();
    let seed_addr = dialable_addr(&seed).await;
    let session = ByteSyncSession::new([0x11; 32], vec![]).unwrap();

    let result = dial_with_ticket(
        &follower,
        &ticket,
        &Capabilities {
            iroh: true,
            arti: false,
        },
        1_000,
        0,
        seed_addr,
        session,
        |_| true,
    )
    .await;

    assert!(matches!(
        result,
        Err(TransportError::Blocked(
            TransportBlocked::RequiresUnavailableTransport(_)
        ))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn reconcile_over_real_iroh_quic() {
    let author = generate_communal_author().unwrap();
    let namespace = author.identity().namespace_id;

    // The seed holds the entries and accepts; the follower connects empty.
    let seed_session =
        ByteSyncSession::new(namespace, vec![signed(&author, 1), signed(&author, 2)]).unwrap();
    let follower_session = ByteSyncSession::new(namespace, vec![]).unwrap();

    let seed = bind().await.expect("bind seed");
    let follower = bind().await.expect("bind follower");
    let seed_addr = riot_transport::iroh::dialable_addr(&seed).await;

    let seed_task = tokio::spawn(async move { sync_accept(&seed, seed_session, |_| true).await });

    let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = received.clone();
    let follower_session = sync_connect(&follower, seed_addr, follower_session, move |bundle| {
        sink.lock().unwrap().push(bundle.to_vec());
        true
    })
    .await
    .expect("follower connect+sync");

    let seed_session = seed_task
        .await
        .expect("seed task")
        .expect("seed accept+sync");

    assert!(
        seed_session.is_terminal() && follower_session.is_terminal(),
        "both reach Complete over iroh"
    );
    let bundles = received.lock().unwrap();
    assert_eq!(
        bundles.len(),
        1,
        "follower pulled the entries bundle over QUIC"
    );
    assert!(!bundles[0].is_empty());
}
