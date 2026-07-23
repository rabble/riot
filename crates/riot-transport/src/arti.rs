//! The Tor (Arti) dialer — outbound onion-service client transport.
//!
//! A `TorDialer` implements [`crate::Dialer`]: its `connect()` asks a
//! [`TorConnect`] implementation to reach a v3 onion service and returns the
//! resulting stream as the `(AsyncWrite, AsyncRead)` halves that
//! [`crate::run_dial`] / [`crate::pump`] consume.
//!
//! ## Why a trait, not a concrete `arti_client::TorClient`
//!
//! `arti_client::TorClient::connect` is async and returns an `DataStream` that
//! already implements tokio's `AsyncRead`/`AsyncWrite`. But pulling the real
//! Arti runtime into every test would require bootstrapping a Tor consensus
//! directory (seconds, network). The `TorConnect` trait keeps the dialer
//! exhaustively unit-testable with a fake that returns an in-memory duplex, and
//! lets the real `arti_client` impl live behind the `arti` Cargo feature without
//! the test path ever depending on it.
//!
//! ## Scope (this slice)
//!
//! Outbound/dial-side only: a follower dials a known onion address. Hosting an
//! onion service for inbound (a Tor-reachable seed) is a later slice — Arti's
//! `tor_hsservice` now supports it, but the responder-side router adaptation
//! (no ALPN / no QUIC exporter on a Tor stream) is separate work. See the plan
//! in the design conversation: "dial-side only, feature-gated, ticket-extended".

use crate::router::{BoxRead, BoxWrite};
use crate::{Dialer, TransportError};

/// Open an outbound Tor connection to a v3 onion service address.
///
/// The address is the attested `onion` field from a [`crate::ticket::Ticket`]
/// (signature-covered, unlike the iroh `node` hint): the 56-char service id,
/// optionally `<id>.onion` or `<id>.onion:<port>`. Implementations resolve it to
/// a connected byte stream.
///
/// The real implementation (behind the `arti` feature) delegates to
/// `arti_client::TorClient::connect`; tests inject a fake.
pub trait TorConnect: Send {
    /// Connect to `onion_addr` and return the stream halves.
    fn connect(
        &mut self,
        onion_addr: &str,
    ) -> impl std::future::Future<Output = Result<(BoxWrite, BoxRead), TransportError>> + Send;
}

/// A [`Dialer`] that reaches a peer over Tor. Holds a [`TorConnect`]
/// implementation and the onion address to dial; `connect` is called exactly
/// once by [`crate::run_dial`].
pub struct TorDialer<C: TorConnect> {
    client: C,
    onion_addr: String,
}

impl<C: TorConnect> TorDialer<C> {
    /// Build a dialer for `onion_addr` using `client` as the Tor transport.
    pub fn new(client: C, onion_addr: String) -> Self {
        Self { client, onion_addr }
    }
}

impl<C: TorConnect> Dialer for TorDialer<C> {
    async fn connect(&mut self) -> Result<(BoxWrite, BoxRead), TransportError> {
        self.client.connect(&self.onion_addr).await
    }
}
