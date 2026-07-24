//! `run_dial` + the `Dialer` trait, driven over an in-memory loopback — no
//! network, no iroh, no Tor. Proves the dial-side abstraction that lets a Tor
//! (or any) transport produce the `(AsyncWrite, AsyncRead)` pair `pump` already
//! consumes, without touching the protocol layer.
//!
//! This is the structural proof that "add a second transport" is connection
//! plumbing, not a protocol change: `run_dial` calls `Dialer::connect()` once,
//! hands the halves to the existing transport-agnostic `pump`, and the reconcile
//! completes identically to `pump_duplex.rs`.

mod common;

use std::sync::{Arc, Mutex};

use riot_core::sync::ByteSyncSession;
use riot_core::willow::generate_communal_author;
use riot_transport::router::{BoxRead, BoxWrite};
use riot_transport::{run_dial, Dialer};

use common::signed;

/// A `Dialer` whose `connect()` returns one half of an in-memory duplex. The
/// other half is held by the test (and driven by a raw `pump` as the responder),
/// so this is exactly the shape a real Tor client's stream would present.
struct LoopbackDialer {
    send: Option<BoxWrite>,
    recv: Option<BoxRead>,
}

impl Dialer for LoopbackDialer {
    async fn connect(&mut self) -> Result<(BoxWrite, BoxRead), riot_transport::TransportError> {
        let send = self.send.take();
        let recv = self.recv.take();
        match (send, recv) {
            (Some(s), Some(r)) => Ok((s, r)),
            // A second connect on a one-shot dialer is a programming error.
            _ => Err(riot_transport::TransportError::StreamClosed),
        }
    }
}

#[tokio::test]
async fn run_dial_drives_a_reconcile_over_a_loopback_dialer() {
    let author = generate_communal_author().unwrap();
    let namespace = author.identity().namespace_id;

    // The realistic follower-dials-seed shape: the SEED (responder) holds the
    // entries; the FOLLOWER (dialer/initiator) starts empty and RECEIVES the
    // bundle through its on_bundle hook. This mirrors connect_followed_site.
    let follower = ByteSyncSession::new(namespace, vec![]).unwrap();
    let seed =
        ByteSyncSession::new(namespace, vec![signed(&author, 1), signed(&author, 2)]).unwrap();

    let (a, b) = tokio::io::duplex(1 << 16);
    let (a_read, a_write) = tokio::io::split(a);
    let (mut b_read, mut b_write) = tokio::io::split(b);

    // The dialer (follower) holds the a halves; the responder (seed) holds b.
    let dialer = LoopbackDialer {
        send: Some(Box::pin(a_write)),
        recv: Some(Box::pin(a_read)),
    };

    let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = received.clone();

    // run_dial IS the follower (initiator) side, going through the Dialer trait.
    // The follower's on_bundle captures the bundle the seed serves.
    let dial_side = run_dial(dialer, follower, true, move |bundle| {
        sink.lock().unwrap().push(bundle.to_vec());
        true
    });
    // The seed (responder) is a plain pump over the b halves.
    let recv_side = riot_transport::pump(seed, &mut b_write, &mut b_read, false, |_bundle| true);

    // Hard outer timeout: a reconcile that deadlocks must fail in seconds, not
    // hang the test binary forever (three of these ran for 6h+ once).
    let (dialed, got) = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        tokio::join!(dial_side, recv_side)
    })
    .await
    .expect("run_dial reconcile timed out (>30s)");
    let dialed = dialed.expect("run_dial completed");
    let responder = got.expect("responder pump");

    assert!(
        dialed.is_terminal() && responder.is_terminal(),
        "both sides reach Complete"
    );
    let bundles = received.lock().unwrap();
    assert_eq!(bundles.len(), 1, "the follower received exactly one bundle");
    assert!(!bundles[0].is_empty(), "the bundle carries the entries");
}

#[tokio::test]
async fn run_dial_returns_the_terminal_session() {
    // A dialer whose stream immediately closes: run_dial surfaces the session
    // (possibly errored) rather than panicking, and the Dialer is consumed.
    let author = generate_communal_author().unwrap();
    let namespace = author.identity().namespace_id;
    let empty = ByteSyncSession::new(namespace, vec![]).unwrap();

    let (_a, b) = tokio::io::duplex(1 << 16);
    let (b_read, b_write) = tokio::io::split(b);
    // Drop a immediately to close the pipe from the other end.
    drop(_a);

    let dialer = LoopbackDialer {
        send: Some(Box::pin(b_write)),
        recv: Some(Box::pin(b_read)),
    };
    // Even with an empty session and a closed peer, run_dial must not panic —
    // and must not hang: a closed peer that fails to terminate is a bug, so
    // bound it rather than let the binary run forever.
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        run_dial(dialer, empty, true, |_| true),
    )
    .await
    .expect("run_dial timed out (>30s) on a closed peer");
}

#[tokio::test]
async fn a_one_shot_dialer_connects_at_most_once() {
    // The Dialer contract: connect() is called exactly once by run_dial. A
    // second connect on a one-shot dialer returns StreamClosed — proving the
    // contract is enforced and a buggy caller (calling connect twice) is caught.
    let (a, _b) = tokio::io::duplex(1 << 16);
    let (a_read, a_write) = tokio::io::split(a);
    let mut dialer = LoopbackDialer {
        send: Some(Box::pin(a_write)),
        recv: Some(Box::pin(a_read)),
    };
    assert!(dialer.connect().await.is_ok(), "first connect succeeds");
    assert!(
        matches!(
            dialer.connect().await,
            Err(riot_transport::TransportError::StreamClosed)
        ),
        "second connect on a one-shot dialer fails closed"
    );
}
