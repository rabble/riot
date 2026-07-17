//! The iroh (QUIC) FrameChannel — v1 internet transport (spec §5.4).
//!
//! Thin: establish a connection, take its bidirectional stream, and hand it to
//! the transport-agnostic [`crate::pump`]. No reconcile logic lives here.

use iroh::endpoint::{presets, Connection};
use iroh::{Endpoint, EndpointAddr, TransportAddr, Watcher};
use tokio::time::{sleep, Duration};

use riot_core::sync::ByteSyncSession;

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

/// Bind an iroh endpoint that speaks the sync ALPN. `N0DisableRelay` = direct
/// connections, no relay — enough for the in-process test; the seed/native
/// build will choose a discovery preset later.
pub async fn bind() -> Result<Endpoint, TransportError> {
    Endpoint::builder(presets::N0DisableRelay)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(io_err)
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
