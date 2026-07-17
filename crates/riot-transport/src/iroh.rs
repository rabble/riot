//! The iroh (QUIC) FrameChannel — v1 internet transport (spec §5.4).
//!
//! Thin: establish a connection, take its bidirectional stream, and hand it to
//! the transport-agnostic [`crate::pump`]. No reconcile logic lives here.

use iroh::endpoint::{presets, Connection};
use iroh::{Endpoint, EndpointAddr, SecretKey, TransportAddr, Watcher};
use tokio::time::{sleep, Duration};

use riot_core::sync::ByteSyncSession;

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
/// ephemeral via [`bind`].
pub async fn bind_seed(secret: [u8; 32]) -> Result<Endpoint, TransportError> {
    Endpoint::builder(presets::N0DisableRelay)
        .secret_key(SecretKey::from_bytes(&secret))
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(io_err)
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
pub async fn sync_accept<F: FnMut(&[u8]) -> bool>(
    endpoint: &Endpoint,
    session: ByteSyncSession,
    on_bundle: F,
) -> Result<ByteSyncSession, TransportError> {
    let incoming = endpoint
        .accept()
        .await
        .ok_or(TransportError::StreamClosed)?;
    let conn: Connection = incoming.await.map_err(io_err)?;
    let (mut send, mut recv) = conn.accept_bi().await.map_err(io_err)?;
    let session = pump(session, &mut send, &mut recv, false, on_bundle).await?;
    graceful_close(&mut send, &mut recv).await;
    Ok(session)
}

/// Both sides finish writing, then drain the peer to EOF, so neither tears the
/// QUIC connection down while the other is still flushing its final frame.
async fn graceful_close(
    send: &mut iroh::endpoint::SendStream,
    recv: &mut iroh::endpoint::RecvStream,
) {
    let _ = send.finish();
    let mut sink = Vec::new();
    let _ = tokio::io::AsyncReadExt::read_to_end(recv, &mut sink).await;
}
