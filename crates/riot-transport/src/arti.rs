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

// ---------------------------------------------------------------------------
// Real Arti (arti-client) implementation — behind the `arti` Cargo feature.
// ---------------------------------------------------------------------------

#[cfg(feature = "arti")]
pub mod arti_impl {
    //! The concrete `TorConnect` over a real `arti_client::TorClient`.
    //!
    //! `bootstrap_tor()` builds a bootstrapped client (fetches the consensus
    //! directory once, the heavy part); `ArtiTorClient` then connects to
    //! attested onion addresses. `DataStream` implements tokio's
    //! `AsyncRead`/`AsyncWrite` (arti-client's `tokio` feature is on), so it
    //! drops straight into the boxed halves `run_dial`/`pump` consume.

    use std::sync::Arc;

    use arti_client::{TorClient, TorClientConfig};

    use crate::arti::TorConnect;
    use crate::router::{BoxRead, BoxWrite};
    use crate::TransportError;

    /// A real Tor client wrapping a bootstrapped `arti_client::TorClient`.
    /// Cheap to clone (the inner client is `Arc`-shared); reuse one across dials.
    #[derive(Clone)]
    pub struct ArtiTorClient {
        inner: Arc<TorClient<tor_rtcompat::tokio::TokioNativeTlsRuntime>>,
    }

    impl ArtiTorClient {
        /// Wrap an existing, bootstrapped `TorClient`. `create_bootstrapped`
        /// returns the client already wrapped in an `Arc` (a cheap cloneable
        /// handle), so this takes the `Arc` as-is. For bootstrapping, use
        /// [`bootstrap_tor`] instead.
        pub fn from_client(
            client: Arc<TorClient<tor_rtcompat::tokio::TokioNativeTlsRuntime>>,
        ) -> Self {
            Self { inner: client }
        }
    }

    impl TorConnect for ArtiTorClient {
        async fn connect(
            &mut self,
            onion_addr: &str,
        ) -> Result<(BoxWrite, BoxRead), TransportError> {
            let (host, port) = parse_onion(onion_addr)?;
            // `(String, u16)` implements arti's IntoTorAddr; the connection is
            // anonymized through the Tor circuit. DataStream implements tokio
            // AsyncRead + AsyncWrite (arti-client `tokio` feature is enabled).
            let stream = self.inner.connect((host, port)).await.map_err(tor_error)?;
            // tokio::io::split returns (ReadHalf, WriteHalf) — note the order.
            let (read, write) = tokio::io::split(stream);
            Ok((Box::pin(write) as BoxWrite, Box::pin(read) as BoxRead))
        }
    }

    /// Bootstrap a Tor client with the default configuration.
    ///
    /// This is the heavy operation: it connects to the Tor network and downloads
    /// the consensus directory. Call it ONCE per process (a client is reusable
    /// across many dials via [`ArtiTorClient::clone`]). State and cache land in
    /// arti's default directories.
    ///
    /// Custom state-directory configuration (a future `--tor-state-dir` CLI flag)
    /// needs the `tor-config` builder API and is deferred until that's pinned;
    /// the default config is sufficient to prove the real-Tor dial path.
    pub async fn bootstrap_tor() -> Result<ArtiTorClient, TransportError> {
        let runtime = tor_rtcompat::tokio::TokioNativeTlsRuntime::current()
            .map_err(|e| TransportError::Io(std::io::Error::other(format!("tor runtime: {e}"))))?;
        let client = TorClient::with_runtime(runtime)
            .config(TorClientConfig::default())
            .create_bootstrapped()
            .await
            .map_err(tor_error)?;
        Ok(ArtiTorClient::from_client(client))
    }

    /// Parse a ticket `onion` field into `(host, port)`. Accepts:
    /// - bare 56-char id            → `<id>.onion`, default port 80
    /// - `<id>.onion`               → as-is, default port 80
    /// - `<id>.onion:<port>`        → explicit port
    /// - `<id>:<port>` (no .onion)  → `<id>.onion`, explicit port
    fn parse_onion(raw: &str) -> Result<(String, u16), TransportError> {
        let (id_or_host, port) = match raw.rsplit_once(':') {
            // A trailing `:port`. The port part must parse; otherwise the whole
            // thing is treated as an id with no port (defensive).
            Some((head, p)) => match p.parse::<u16>() {
                Ok(port) => (head, Some(port)),
                Err(_) => (raw, None),
            },
            None => (raw, None),
        };
        let host = if id_or_host.ends_with(".onion") {
            id_or_host.to_string()
        } else {
            format!("{id_or_host}.onion")
        };
        let port = port.unwrap_or(80);
        Ok((host, port))
    }

    fn tor_error<E: std::fmt::Display>(e: E) -> TransportError {
        TransportError::Io(std::io::Error::other(format!("tor: {e}")))
    }

    #[cfg(test)]
    mod tests {
        use super::parse_onion;

        const ID: &str = "abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijk";

        #[test]
        fn bare_id_gets_onion_suffix_and_default_port() {
            assert_eq!(parse_onion(ID).unwrap(), (format!("{ID}.onion"), 80));
        }

        #[test]
        fn explicit_onion_keeps_suffix() {
            let (host, port) = parse_onion("zzz.onion").unwrap();
            assert_eq!(host, "zzz.onion");
            assert_eq!(port, 80);
        }

        #[test]
        fn explicit_port_is_parsed() {
            let (host, port) = parse_onion(&format!("{ID}.onion:443")).unwrap();
            assert_eq!(host, format!("{ID}.onion"));
            assert_eq!(port, 443);
        }

        #[test]
        fn bare_id_with_port_adds_suffix() {
            let (host, port) = parse_onion(&format!("{ID}:9001")).unwrap();
            assert_eq!(host, format!("{ID}.onion"));
            assert_eq!(port, 9001);
        }

        #[test]
        fn non_numeric_suffix_is_treated_as_part_of_id() {
            // A colon that isn't followed by a number isn't a port separator.
            let (host, port) = parse_onion("has-colon-but-not-port").unwrap();
            assert_eq!(host, "has-colon-but-not-port.onion");
            assert_eq!(port, 80);
        }
    }
}
