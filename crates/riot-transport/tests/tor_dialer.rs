//! The Tor dialer plumbing against a FAKE connect — no real Tor runtime.
//!
//! `TorDialer` is generic over a `TorConnect` trait so it is exhaustively
//! testable without bootstrapping a Tor client. The fake returns an in-memory
//! duplex pair; the real `arti_client::TorClient` impl lands behind the `arti`
//! feature in `src/arti.rs`. These tests prove the dialer:
//!
//! - calls `connect` exactly once with the onion address it was constructed with;
//! - yields the resulting stream to `run_dial`, which reconciles over it; and
//! - fails closed (no dial) when the underlying connect fails.

mod common;

use std::sync::{Arc, Mutex};

use riot_core::sync::ByteSyncSession;
use riot_core::willow::generate_communal_author;
use riot_transport::arti::{TorConnect, TorDialer};
use riot_transport::router::{BoxRead, BoxWrite};
use riot_transport::{run_dial, Dialer, TransportError};
use tokio::io::{split, ReadHalf, WriteHalf};
use tokio::io::{AsyncRead, AsyncWrite};

use common::signed;

const ONION: &str = "abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijk";

// ---- A fake TorConnect for tests (no real Tor) ----------------------------

/// A fake `TorConnect` that either yields a pre-made duplex half (success) or
/// fails immediately. Records every address it was asked to connect to.
struct FakeTorConnect {
    /// The stream half to hand back from `connect` (None on the failing variant).
    stream: Option<(BoxWrite, BoxRead)>,
    /// Addresses this fake was asked to connect to, in order.
    observed: Arc<Mutex<Vec<String>>>,
}

impl FakeTorConnect {
    /// Build a succeeding fake PLUS the matching other duplex half the test
    /// drives as the peer. The two halves are a connected pair.
    fn with_duplex() -> (Self, DuplexOther) {
        let (a, b) = tokio::io::duplex(1 << 16);
        let (a_read, a_write) = split(a);
        let (b_read, b_write) = split(b);
        let observed = Arc::new(Mutex::new(Vec::new()));
        let fake = FakeTorConnect {
            stream: Some((Box::pin(a_write), Box::pin(a_read))),
            observed: observed.clone(),
        };
        (
            fake,
            DuplexOther {
                read: b_read,
                write: b_write,
            },
        )
    }

    /// A fake whose connect always fails (onion unreachable).
    fn failing() -> Self {
        FakeTorConnect {
            stream: None,
            observed: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn observed_addrs(&self) -> Arc<Mutex<Vec<String>>> {
        self.observed.clone()
    }
}

/// The peer-side half of the duplex a `FakeTorConnect::with_duplex` produced.
struct DuplexOther {
    read: ReadHalf<tokio::io::DuplexStream>,
    write: WriteHalf<tokio::io::DuplexStream>,
}

impl TorConnect for FakeTorConnect {
    async fn connect(&mut self, onion_addr: &str) -> Result<(BoxWrite, BoxRead), TransportError> {
        self.observed.lock().unwrap().push(onion_addr.to_string());
        match self.stream.take() {
            Some(halves) => Ok(halves),
            None => Err(TransportError::StreamClosed),
        }
    }
}

// ---- tests -----------------------------------------------------------------

#[tokio::test]
async fn tor_dialer_connects_once_with_its_onion_address() {
    let (fake, _other) = FakeTorConnect::with_duplex();
    let seen = fake.observed_addrs();
    let mut dialer = TorDialer::new(fake, ONION.to_string());

    let _streams: (BoxWrite, BoxRead) = Dialer::connect(&mut dialer)
        .await
        .expect("fake connect yields a stream");

    let observed = seen.lock().unwrap().clone();
    assert_eq!(
        observed,
        vec![ONION.to_string()],
        "connect called once with the onion"
    );
}

#[tokio::test]
async fn run_dial_over_a_tor_dialer_delivers_the_bundle() {
    // End-to-end through the TorDialer (fake transport): a seed serves, a
    // follower dials over "Tor" and receives the entries bundle. This is the
    // proof that the whole pump-over-Tor path works once a real TorConnect is
    // plugged in — only the bytes-on-the-wire are faked.
    let author = generate_communal_author().unwrap();
    let namespace = author.identity().namespace_id;
    let follower = ByteSyncSession::new(namespace, vec![]).unwrap();
    let seed =
        ByteSyncSession::new(namespace, vec![signed(&author, 1), signed(&author, 2)]).unwrap();

    let (fake, other) = FakeTorConnect::with_duplex();
    let dialer = TorDialer::new(fake, ONION.to_string());

    let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = received.clone();

    let dial_side = run_dial(dialer, follower, true, move |bundle| {
        sink.lock().unwrap().push(bundle.to_vec());
        true
    });
    let DuplexOther {
        read: mut seed_read,
        write: mut seed_write,
    } = other;
    let recv_side = riot_transport::pump(seed, &mut seed_write, &mut seed_read, false, |_| true);

    let (dialed, got) = tokio::join!(dial_side, recv_side);
    assert!(dialed.expect("dial").is_terminal());
    assert!(got.expect("seed").is_terminal());

    let bundles = received.lock().unwrap();
    assert_eq!(
        bundles.len(),
        1,
        "follower received the bundle over the Tor dialer"
    );
}

#[tokio::test]
async fn a_failed_tor_connect_surfaces_as_a_transport_error() {
    // If the underlying Tor connect fails (e.g. onion unreachable), the dialer
    // must fail closed — run_dial must NOT proceed to pump on a missing stream.
    let fake = FakeTorConnect::failing();
    let mut dialer = TorDialer::new(fake, ONION.to_string());
    let result = Dialer::connect(&mut dialer).await;
    assert!(
        result.is_err(),
        "a failed Tor connect fails closed, never yields a stream"
    );
}

// Suppress unused-import noise: AsyncRead/AsyncWrite bound the DuplexOther use
// site indirectly through pump's generics.
#[allow(dead_code)]
fn _assert_io<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(_: R, _: W) {}
