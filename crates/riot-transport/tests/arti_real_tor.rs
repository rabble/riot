//! REAL-Tor integration test — #[ignore] by default.
//!
//! Run locally with:
//!   cargo test -p riot-transport --features arti --test arti_real_tor -- --ignored
//!
//! This bootstraps a real arti_client::TorClient (fetches the consensus over the
//! network — tens of seconds, requires internet + reachability to the Tor
//! network) and connects to a known-public onion service. It proves the real
//! ArtiTorClient → DataStream → boxed-halves path works end to end. It is NOT a
//! Riot reconcile against a Riot seed over Tor: the seed still runs iroh, and
//! hosting a Riot onion service (the responder side) is a later slice. So this
//! validates the transport connection, not the protocol exchange over it.
//!
//! A full Riot-over-Tor reconcile test waits on onion-service hosting.

#![cfg(feature = "arti")]

use std::time::Duration;

use riot_transport::arti::arti_impl::bootstrap_tor;
use riot_transport::arti::TorDialer;
use riot_transport::Dialer;

/// DuckDuckGo's v3 onion — a widely-used, reliably-connectable plain onion
/// service (no client-auth, no PoW gate), common as a Tor connectivity smoke
/// target. Port 443. Used here to prove Riot's Tor plumbing (bootstrap →
/// circuit → onion stream) reaches a real onion service end to end.
const DUCKDUCKGO_ONION: &str = "duckduckgogg42xjoc72x3sjasowoarfbgcmvfimaftt6twagswzczad.onion:443";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires network + Tor reachability; run with --ignored"]
async fn real_tor_bootstraps_and_connects_to_a_known_onion() {
    // Bootstrap is the heavy part: connects to the Tor network and fetches the
    // consensus directory. Give it generous room.
    let tor = tokio::time::timeout(Duration::from_secs(180), bootstrap_tor())
        .await
        .expect("bootstrap timed out (180s)")
        .expect("bootstrap succeeded");

    // Connect to the known onion. A successful connect returns a DataStream we
    // can split into the boxed halves run_dial/pump expect.
    let mut dialer = TorDialer::new(tor, DUCKDUCKGO_ONION.to_string());
    let streams = tokio::time::timeout(Duration::from_secs(120), Dialer::connect(&mut dialer))
        .await
        .expect("connect timed out (120s)")
        .expect("connect succeeded");

    // The halves are non-trivial type-erased streams; reaching this point proves
    // the whole arti path (bootstrap → circuit → DataStream → boxed halves) works.
    let (_write, _read) = streams;
    // (A real reconcile would hand these to run_dial; that needs a Riot onion
    // service to dial, which is the deferred responder-side slice.)
}
