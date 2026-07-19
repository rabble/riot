//! The iroh (QUIC) FrameChannel — v1 internet transport (spec §5.4).
//!
//! Thin: establish a connection, take its bidirectional stream, and hand it to
//! the transport-agnostic [`crate::pump`]. No reconcile logic lives here.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use iroh::endpoint::{presets, Connection, VarInt};
use iroh::{Endpoint, EndpointAddr, SecretKey, TransportAddr, Watcher};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{sleep, Duration};

use riot_core::sync::ByteSyncSession;

use crate::router::{
    AlpnRouter, BoundedStream, BoxRead, BoxWrite, Deadlines, Exporter, RouterConnection,
};
use crate::ticket::{admit_dial, Capabilities, Ticket};
use crate::{pump, TransportError, ALPN};

fn io_err<E: std::fmt::Display>(e: E) -> TransportError {
    TransportError::Io(std::io::Error::other(e.to_string()))
}

/// The endpoint's own dialable address, once at least one direct (IP) address
/// has been discovered. With no relay in this preset, a fresh endpoint needs a
/// moment of netcheck before its address is reachable; poll the addr watcher.
pub async fn dialable_addr(endpoint: &Endpoint) -> EndpointAddr {
    let mut watch = endpoint.watch_addr();
    for _ in 0..200 {
        let addr = watch.get();
        if addr.addrs.iter().any(|a| matches!(a, TransportAddr::Ip(_))) {
            return addr;
        }
        sleep(Duration::from_millis(25)).await;
    }
    endpoint.addr()
}

/// Bind a FOLLOWER endpoint: a fresh random secret key each call means an
/// EPHEMERAL NodeId (§5.4), reducing cross-session linkability — a follower is
/// not a stable point in the follow-graph.
pub async fn bind() -> Result<Endpoint, TransportError> {
    Endpoint::builder(presets::N0DisableRelay)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(io_err)
}

/// Bind a SEED endpoint with a STABLE identity from the durable profile. Only
/// seeds need a stable NodeId (so followers can find them); followers stay
/// ephemeral via [`bind`]. `N0DisableRelay` — direct only, for local/LAN.
pub async fn bind_seed(secret: [u8; 32]) -> Result<Endpoint, TransportError> {
    Endpoint::builder(presets::N0DisableRelay)
        .secret_key(SecretKey::from_bytes(&secret))
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(io_err)
}

/// Bind a PUBLIC endpoint (N0 preset: relay + pkarr/DNS discovery) reachable
/// from anywhere across NAT — for an always-on internet seed. `secret` gives it
/// a stable NodeId so followers can find it by id via discovery.
pub async fn bind_public(secret: [u8; 32]) -> Result<Endpoint, TransportError> {
    Endpoint::builder(presets::N0)
        .secret_key(SecretKey::from_bytes(&secret))
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(io_err)
}

/// This endpoint's NodeId (public key) — a follower dials it by this id, and
/// discovery resolves the address. Stable across restarts when bound from a
/// persisted secret.
pub fn node_id(endpoint: &Endpoint) -> [u8; 32] {
    *endpoint.id().as_bytes()
}

/// Dial a peer known only by NodeId (discovery resolves the address).
pub fn addr_from_node_id(node_id: [u8; 32]) -> Result<EndpointAddr, TransportError> {
    let key = iroh::PublicKey::from_bytes(&node_id).map_err(io_err)?;
    Ok(EndpointAddr::from(key))
}

/// A ticket `node` hint carrying the NodeId AND any direct socket addresses:
/// `<id_hex>@<ip:port>,<ip:port>`. The addresses let a follower dial directly
/// (LAN, or a public seed) without waiting on DHT discovery; with no addresses
/// it degrades to id-only (discovery). Untrusted — outside the ticket signature.
pub fn endpoint_addr_hint(addr: &EndpointAddr) -> String {
    let id = addr
        .addrs
        .iter()
        .filter_map(|a| match a {
            TransportAddr::Ip(s) => Some(s.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let idhex: String = addr
        .id
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if id.is_empty() {
        idhex
    } else {
        format!("{idhex}@{}", id.join(","))
    }
}

/// Parse a `node` hint back to a dialable [`EndpointAddr`].
pub fn addr_from_hint(hint: &str) -> Result<EndpointAddr, TransportError> {
    let (id_hex, addrs) = match hint.split_once('@') {
        Some((id, list)) => (id, Some(list)),
        None => (hint, None),
    };
    let mut id = [0u8; 32];
    if id_hex.len() != 64 {
        return Err(io_err("node id must be 64 hex chars"));
    }
    for (i, b) in id.iter_mut().enumerate() {
        *b = u8::from_str_radix(&id_hex[i * 2..i * 2 + 2], 16).map_err(io_err)?;
    }
    let key = iroh::PublicKey::from_bytes(&id).map_err(io_err)?;
    match addrs {
        None => Ok(EndpointAddr::from(key)),
        Some(list) => {
            let socks: Vec<TransportAddr> = list
                .split(',')
                .filter_map(|s| s.parse().ok())
                .map(TransportAddr::Ip)
                .collect();
            Ok(EndpointAddr::from_parts(key, socks))
        }
    }
}

/// The fail-closed dial (§5.1-5.3): run the pre-connection gate on a root-signed
/// ticket FIRST — verifying the signature and the transport floor — and dial
/// only if it is safe. A refusal returns `TransportError::Blocked` WITHOUT ever
/// opening a connection, so a `require:arti` site never leaks an IP over iroh.
#[allow(clippy::too_many_arguments)]
pub async fn dial_with_ticket<F: FnMut(&[u8]) -> bool>(
    endpoint: &Endpoint,
    ticket: &Ticket,
    caps: &Capabilities,
    now_unix: u64,
    durable_epoch_floor: u64,
    peer: EndpointAddr,
    session: ByteSyncSession,
    on_bundle: F,
) -> Result<ByteSyncSession, TransportError> {
    // Verify-before-dial: the gate decides refuse-or-connect BEFORE any packet.
    admit_dial(ticket, caps, now_unix, durable_epoch_floor)?;
    sync_connect(endpoint, peer, session, on_bundle).await
}

/// Initiator: dial `peer`, open a bi-stream, reconcile the namespace.
pub async fn sync_connect<F: FnMut(&[u8]) -> bool>(
    endpoint: &Endpoint,
    peer: EndpointAddr,
    session: ByteSyncSession,
    on_bundle: F,
) -> Result<ByteSyncSession, TransportError> {
    let conn: Connection = endpoint.connect(peer, ALPN).await.map_err(io_err)?;
    let (mut send, mut recv) = conn.open_bi().await.map_err(io_err)?;
    let session = pump(session, &mut send, &mut recv, true, on_bundle).await?;
    graceful_close(&mut send, &mut recv).await;
    Ok(session)
}

/// Responder: accept the next inbound connection and reconcile.
///
/// This is the `riot/sync/1` COMPATIBILITY WRAPPER over the general
/// [`AlpnRouter`]: it builds a router that routes ONLY `riot/sync/1` and
/// dispatches the accepted connection through it. The reconcile therefore now
/// runs inside the router's bounded envelope — one session permit, exactly one
/// bidirectional stream (a forbidden extra stream resets the connection), the
/// handshake deadline, and the absolute session lifetime — while the legacy
/// [`pump`] framing on the raw stream is preserved byte-for-byte.
pub async fn sync_accept<F: FnMut(&[u8]) -> bool + Send + 'static>(
    endpoint: &Endpoint,
    session: ByteSyncSession,
    on_bundle: F,
) -> Result<ByteSyncSession, TransportError> {
    let incoming = endpoint
        .accept()
        .await
        .ok_or(TransportError::StreamClosed)?;
    let conn: Connection = incoming.await.map_err(io_err)?;

    // The pumped session and its admission hook are handed to the handler, which
    // runs exactly once; a oneshot carries the terminal session back out. `Fn`
    // (not `FnOnce`) storage forces the take-once cell.
    let cell = Arc::new(Mutex::new(Some((session, on_bundle))));
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let handler: crate::router::Handler = Arc::new(move |stream: BoundedStream, _ex: Exporter| {
        let cell = Arc::clone(&cell);
        let tx = Arc::clone(&tx);
        Box::pin(async move {
            let (session, on_bundle) = cell
                .lock()
                .expect("sync/1 handler cell")
                .take()
                .expect("sync/1 handler runs at most once");
            let (mut send, mut recv) = stream.into_halves();
            let session = pump(session, &mut send, &mut recv, false, on_bundle).await?;
            graceful_close(&mut send, &mut recv).await;
            if let Some(tx) = tx.lock().expect("sync/1 result tx").take() {
                let _ = tx.send(session);
            }
            Ok(())
        }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
    });

    let mut router = AlpnRouter::new(1);
    router.register(ALPN, Deadlines::sync(), handler);
    router.dispatch(IrohConnection::new(conn)).await?;

    rx.await.map_err(|_| TransportError::StreamClosed)
}

/// Accept ONE inbound connection on a public endpoint and dispatch it through a
/// multi-ALPN [`AlpnRouter`]. This is the general accept primitive a public
/// anchor runs in a loop: the endpoint advertises `router.alpns()`, and each
/// accepted connection is routed to the handler for its negotiated ALPN under
/// the router's full bounded envelope. Unknown ALPNs, permit exhaustion, and
/// stream violations return the corresponding [`TransportError`] without leaking
/// a session.
pub async fn accept_with_router(
    endpoint: &Endpoint,
    router: &AlpnRouter,
) -> Result<(), TransportError> {
    let incoming = endpoint
        .accept()
        .await
        .ok_or(TransportError::StreamClosed)?;
    let conn: Connection = incoming.await.map_err(io_err)?;
    router.dispatch(IrohConnection::new(conn)).await
}

/// Both sides finish writing, then drain the peer to EOF, so neither tears the
/// QUIC connection down while the other is still flushing its final frame.
/// `shutdown` on an iroh send stream performs a graceful QUIC `finish`.
async fn graceful_close<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(send: &mut S, recv: &mut R) {
    let _ = send.shutdown().await;
    let mut sink = Vec::new();
    let _ = recv.read_to_end(&mut sink).await;
}

/// Adapts a live iroh [`Connection`] to the transport-agnostic
/// [`RouterConnection`] the [`AlpnRouter`] drives. A `Connection` is a cheap
/// `Arc` handle, so `Clone` is free and lets the exporter capability outlive the
/// dispatch call.
#[derive(Clone)]
pub struct IrohConnection(Connection);

impl IrohConnection {
    /// Wrap an already-accepted, handshake-completed connection.
    pub fn new(conn: Connection) -> Self {
        Self(conn)
    }
}

impl RouterConnection for IrohConnection {
    fn negotiated_alpn(&self) -> Option<Vec<u8>> {
        Some(self.0.alpn().to_vec())
    }

    fn export_keying_material(
        &self,
        label: &[u8],
        context: &[u8],
        out_len: usize,
    ) -> Result<Vec<u8>, TransportError> {
        let mut out = vec![0u8; out_len];
        self.0
            .export_keying_material(&mut out, label, context)
            .map_err(|e| TransportError::Io(std::io::Error::other(format!("{e:?}"))))?;
        Ok(out)
    }

    fn accept_bi(
        &self,
    ) -> impl Future<Output = Result<(BoxWrite, BoxRead), TransportError>> + Send {
        let conn = self.0.clone();
        async move {
            let (send, recv) = conn.accept_bi().await.map_err(io_err)?;
            Ok((Box::pin(send) as BoxWrite, Box::pin(recv) as BoxRead))
        }
    }

    fn accept_extra(&self) -> impl Future<Output = ()> + Send {
        let conn = self.0.clone();
        async move {
            // A forbidden second bidirectional stream or ANY unidirectional
            // stream is a violation. A connection close (Err) is NOT a violation:
            // in that case pend forever so the running handler's own outcome wins.
            let is_extra = tokio::select! {
                r = conn.accept_bi() => r.is_ok(),
                r = conn.accept_uni() => r.is_ok(),
            };
            if !is_extra {
                std::future::pending::<()>().await;
            }
        }
    }

    fn close(&self, reason: &[u8]) {
        self.0.close(VarInt::from_u32(0), reason);
    }
}
