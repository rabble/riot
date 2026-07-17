//! The transport-agnostic pump, driven over an in-memory duplex — no network.
//! Proves the reconcile loop + framing carry a real entries bundle end to end.

mod common;

use std::sync::{Arc, Mutex};

use riot_core::sync::ByteSyncSession;
use riot_core::willow::generate_communal_author;
use riot_transport::pump;
use tokio::io::split;

use common::signed;

#[tokio::test]
async fn reconcile_over_a_duplex_delivers_the_entries_bundle() {
    let author = generate_communal_author().unwrap();
    let namespace = author.identity().namespace_id;

    // Sender holds two entries; receiver holds none and must pull them.
    let sender =
        ByteSyncSession::new(namespace, vec![signed(&author, 1), signed(&author, 2)]).unwrap();
    let receiver = ByteSyncSession::new(namespace, vec![]).unwrap();

    let (a, b) = tokio::io::duplex(1 << 16);
    let (mut a_read, mut a_write) = split(a);
    let (mut b_read, mut b_write) = split(b);

    let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = received.clone();

    let send_side = pump(sender, &mut a_write, &mut a_read, true, |_bundle| true);
    let recv_side = pump(receiver, &mut b_write, &mut b_read, false, move |bundle| {
        sink.lock().unwrap().push(bundle.to_vec());
        true
    });

    let (sent, got) = tokio::join!(send_side, recv_side);
    let sender = sent.expect("sender pump");
    let receiver = got.expect("receiver pump");

    assert!(
        sender.is_terminal() && receiver.is_terminal(),
        "both reach Complete"
    );
    let bundles = received.lock().unwrap();
    assert_eq!(bundles.len(), 1, "receiver got exactly one entries bundle");
    assert!(!bundles[0].is_empty(), "the bundle carries the entries");
}
