//! The iroh (QUIC) FrameChannel — v1 internet transport (spec §5.4).
//!
//! Thin: establish a connection, take its bidirectional stream, and hand it to
//! the transport-agnostic [`crate::pump`]. No reconcile logic lives here.

use iroh::endpoint::{presets, Connection};
use iroh::{Endpoint, EndpointAddr, SecretKey, TransportAddr, Watcher};
use tokio::time::{sleep, Duration};

use riot_core::session::EvidenceStore;
use riot_core::site::admit_followed_site_frame;
use riot_core::sync::ByteSyncSession;
use riot_core::willow::SignedWillowEntry;

use crate::ticket::{admit_dial, Capabilities, Ticket};
use crate::{pump, TransportError, ALPN};

/// The route recorded for transport-delivered followed-site imports.
const FOLLOWED_SITE_ROUTE: &str = "site-follow-transport";

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

/// A ticket `node` hint carrying the NodeId AND direct socket addresses:
/// `<id_hex>@<ip:port>,<ip:port>`. Uses the endpoint's REAL bound UDP port (not
/// the STUN-observed external port, which is invalid for a LAN/tailnet peer) —
/// so a peer on the same LAN or tailnet dials directly and connects. For a
/// public seed behind NAT, discovery-by-id (id-only hint) is the reachable path.
/// Untrusted — outside the ticket signature.
pub fn endpoint_addr_hint(endpoint: &Endpoint) -> String {
    let idhex: String = endpoint
        .id()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    // The actual bound local ports (e.g. 0.0.0.0:54426) — what a same-network
    // peer must dial, unlike the STUN-mapped port in watch_addr().
    let bound_v4 = endpoint
        .bound_sockets()
        .into_iter()
        .find(|s| s.is_ipv4())
        .map(|s| s.port());

    // The endpoint's interface IPs, re-homed onto the real bound port.
    let mut seen = std::collections::BTreeSet::new();
    for a in endpoint.addr().addrs {
        if let TransportAddr::Ip(s) = a {
            let port = bound_v4.unwrap_or_else(|| s.port());
            if s.is_ipv4() {
                seen.insert(format!("{}:{}", s.ip(), port));
            }
        }
    }
    let addrs: Vec<String> = seen.into_iter().collect();
    if addrs.is_empty() {
        idhex
    } else {
        format!("{idhex}@{}", addrs.join(","))
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

/// Owner side (WU3): passively SERVE an owned site's offer to a follower over an
/// accepted connection. Read-mostly v1 — the owner reseeds `offer` (what
/// `build_followed_site_offer(root)` returns on a device) and does not ingest
/// follower publishes (`|_| true` acknowledges without importing). A separate
/// session keyed on the site `root`, distinct from any community sync.
pub async fn serve_followed_site(
    endpoint: &Endpoint,
    root: [u8; 32],
    offer: Vec<SignedWillowEntry>,
) -> Result<ByteSyncSession, TransportError> {
    let session = ByteSyncSession::new(root, offer).map_err(TransportError::Sync)?;
    sync_accept(endpoint, session, |_| true).await
}

/// Follower side (WU3): dial a followed site through the FAIL-CLOSED ticket gate
/// and admit every delivered bundle through the SINGLE canonical core gate
/// (`admit_followed_site_frame`), committing owner /mod + /articles into `store`.
///
/// The ticket is verified BEFORE any packet: a ticket that fails its transport
/// floor refuses without opening a connection. Every received bundle is admitted
/// under `followed_root = root` and family-gated — the exact gate the manual
/// (Option B) and sync-session (WU2) paths use, so nothing drifts.
#[allow(clippy::too_many_arguments)]
pub async fn connect_followed_site(
    endpoint: &Endpoint,
    peer: EndpointAddr,
    ticket: &Ticket,
    caps: &Capabilities,
    now_unix: u64,
    durable_epoch_floor: u64,
    store: &EvidenceStore,
    root: [u8; 32],
    offer: Vec<SignedWillowEntry>,
) -> Result<ByteSyncSession, TransportError> {
    let session = ByteSyncSession::new(root, offer).map_err(TransportError::Sync)?;
    dial_with_ticket(
        endpoint,
        ticket,
        caps,
        now_unix,
        durable_epoch_floor,
        peer,
        session,
        |bundle| admit_followed_site_frame(store, root, bundle, FOLLOWED_SITE_ROUTE).is_ok(),
    )
    .await
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
