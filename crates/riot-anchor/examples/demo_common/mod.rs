//! Shared wire helpers for the cross-city demo examples (`demo_anchor`,
//! `demo_host`, `demo_follow`): length-prefixed frame IO, a control-plane
//! round trip, a `riot/sync/2` initiator driver, and anchor address resolution
//! from the environment (`ANCHOR_ADDR` hint or `ANCHOR_NODE_ID`).
//!
//! This module mirrors the wire discipline of `tests/daemon_e2e.rs` (the same
//! `u32be` length prefix, the same protocol frame ceilings) but returns
//! `Result<_, String>` instead of panicking, so the demo binaries print a
//! readable failure instead of a backtrace.

#![allow(dead_code)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::timeout;

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::control::{ControlResponseV1, MAX_CONTROL_FRAME_BYTES};
use riot_anchor_protocol::sync2::{
    Sync2Action, Sync2Frame, Sync2Repository, Sync2Session, MAX_SYNC2_FRAME_BYTES,
};
use riot_transport::{ALPN_ANCHOR_V1, ALPN_SYNC_V2};

/// Per-network-step deadline: generous enough for a cross-city RTT, bounded
/// enough that a dead anchor fails the demo instead of hanging it.
pub const STEP: Duration = Duration::from_secs(30);

/// Print a demo stage marker.
pub fn stage(message: &str) {
    println!("==> {message}");
}

/// Current wall-clock unix seconds.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Lowercase hex of `bytes`.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Parse an even-length hex string.
pub fn from_hex(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err("hex string must have even length".to_string());
    }
    (0..hex.len() / 2)
        .map(|index| {
            u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                .map_err(|_| format!("invalid hex at byte {index}"))
        })
        .collect()
}

/// A fresh random 32-byte secret from the OS CSPRNG.
pub fn random_secret() -> Result<[u8; 32], String> {
    let mut secret = [0u8; 32];
    getrandom::getrandom(&mut secret).map_err(|error| format!("OS entropy failed: {error}"))?;
    Ok(secret)
}

/// A fresh random 16-byte idempotency key.
pub fn random_idempotency_key() -> Result<[u8; 16], String> {
    let secret = random_secret()?;
    let mut key = [0u8; 16];
    key.copy_from_slice(&secret[..16]);
    Ok(key)
}

/// Resolve the anchor's dialable address from the environment:
///
/// * `ANCHOR_ADDR` — a `<node_id_hex>[@<ip:port>,...]` hint (what
///   `demo_anchor` prints; the direct-address form works on any network the
///   addresses are reachable from), or
/// * `ANCHOR_NODE_ID` — 64 hex chars; discovery resolves the address (a public
///   anchor bound with the relay/discovery preset).
pub fn anchor_addr_from_env() -> Result<iroh::EndpointAddr, String> {
    if let Ok(hint) = std::env::var("ANCHOR_ADDR") {
        if !hint.trim().is_empty() {
            return riot_transport::iroh::addr_from_hint(hint.trim())
                .map_err(|error| format!("bad ANCHOR_ADDR: {error}"));
        }
    }
    if let Ok(id_hex) = std::env::var("ANCHOR_NODE_ID") {
        if !id_hex.trim().is_empty() {
            let bytes = from_hex(id_hex.trim())?;
            let id: [u8; 32] = bytes
                .try_into()
                .map_err(|_| "ANCHOR_NODE_ID must be 64 hex chars".to_string())?;
            return riot_transport::iroh::addr_from_node_id(id)
                .map_err(|error| format!("bad ANCHOR_NODE_ID: {error}"));
        }
    }
    Err(
        "set ANCHOR_ADDR (<node_id_hex>@<ip:port>, printed by demo_anchor) \
         or ANCHOR_NODE_ID (64 hex chars)"
            .to_string(),
    )
}

/// Bind the demo's CLIENT endpoint: an ephemeral identity on the public preset
/// (relay + discovery), so a remote anchor is reachable across NAT; the direct
/// addresses of an `ANCHOR_ADDR` hint are dialed directly either way.
pub async fn bind_client_endpoint() -> Result<iroh::Endpoint, String> {
    iroh::Endpoint::builder(iroh::endpoint::presets::N0)
        .bind()
        .await
        .map_err(|error| format!("failed to bind client endpoint: {error}"))
}

/// Write one `u32be`-length-prefixed frame.
pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, body: &[u8]) -> Result<(), String> {
    let len = u32::try_from(body.len()).map_err(|_| "frame too large".to_string())?;
    writer
        .write_all(&len.to_be_bytes())
        .await
        .map_err(|error| format!("write frame length: {error}"))?;
    writer
        .write_all(body)
        .await
        .map_err(|error| format!("write frame body: {error}"))?;
    writer
        .flush()
        .await
        .map_err(|error| format!("flush frame: {error}"))
}

/// Read one `u32be`-length-prefixed frame.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, String> {
    let mut len = [0u8; 4];
    reader
        .read_exact(&mut len)
        .await
        .map_err(|error| format!("read frame length: {error}"))?;
    let n = u32::from_be_bytes(len) as usize;
    let mut body = vec![0u8; n];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|error| format!("read frame body: {error}"))?;
    Ok(body)
}

/// Dial the `riot/anchor/1` control ALPN, send one canonical request frame,
/// and return the decoded [`ControlResponseV1`].
pub async fn control_round_trip(
    client: &iroh::Endpoint,
    anchor_addr: iroh::EndpointAddr,
    frame: Vec<u8>,
) -> Result<ControlResponseV1, String> {
    let conn = timeout(STEP, client.connect(anchor_addr, ALPN_ANCHOR_V1))
        .await
        .map_err(|_| "control dial timed out".to_string())?
        .map_err(|error| format!("control dial failed: {error}"))?;
    let (send, recv) = conn
        .open_bi()
        .await
        .map_err(|error| format!("open control stream: {error}"))?;
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);

    write_frame(&mut send, &frame).await?;
    let bytes = timeout(STEP, read_frame(&mut recv))
        .await
        .map_err(|_| "control response timed out".to_string())??;
    let _ = send.shutdown().await;
    decode_canonical::<ControlResponseV1>(&bytes, MAX_CONTROL_FRAME_BYTES)
        .map_err(|error| format!("control response did not decode: {error:?}"))
}

/// Dial the `riot/sync/2` ALPN and drive `session` (a real initiator FSM) over
/// the connection: write its `start()` frames, then read → `on_frame` → write
/// until the session terminates. Returns the terminated session so the caller
/// can inspect completion or the refusal.
pub async fn drive_sync2<R: Sync2Repository>(
    client: &iroh::Endpoint,
    anchor_addr: iroh::EndpointAddr,
    mut session: Sync2Session<R>,
) -> Result<Sync2Session<R>, String> {
    let conn = timeout(STEP, client.connect(anchor_addr, ALPN_SYNC_V2))
        .await
        .map_err(|_| "sync dial timed out".to_string())?
        .map_err(|error| format!("sync dial failed: {error}"))?;
    let (send, recv) = conn
        .open_bi()
        .await
        .map_err(|error| format!("open sync stream: {error}"))?;
    let mut send = Box::pin(send);
    let mut recv = Box::pin(recv);

    fn encode_sends(actions: Vec<Sync2Action>) -> Result<Vec<Vec<u8>>, String> {
        actions
            .into_iter()
            .filter_map(|action| match action {
                Sync2Action::Send(frame) => Some(
                    frame
                        .encode_canonical()
                        .map_err(|error| format!("encode sync2 frame: {error:?}")),
                ),
                _ => None,
            })
            .collect()
    }

    for bytes in encode_sends(session.start())? {
        write_frame(&mut send, &bytes).await?;
    }
    while !session.is_terminated() {
        let bytes = timeout(STEP, read_frame(&mut recv))
            .await
            .map_err(|_| "sync frame timed out".to_string())??;
        let frame = decode_canonical::<Sync2Frame>(&bytes, MAX_SYNC2_FRAME_BYTES)
            .map_err(|error| format!("inbound sync2 frame did not decode: {error:?}"))?;
        for out in encode_sends(session.on_frame(frame))? {
            write_frame(&mut send, &out).await?;
        }
    }
    let _ = send.shutdown().await;
    Ok(session)
}
