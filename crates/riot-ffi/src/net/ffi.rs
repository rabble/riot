//! Slice 3a — the `net`-gated UniFFI bridge for the phone anchor-pull client.
//!
//! Slice 2 landed the socket-owning drive loop (`NetRuntime::sync_with_anchor`,
//! `super::anchor`) but left it `pub(crate)` with no `uniffi` export — the native
//! wiring was deferred. This module is that wiring: it makes the phone anchor
//! pull callable from Swift/Kotlin behind the off-by-default `net` feature,
//! matching the crate's existing FFI shape (`MobileProfile`, `MobileSyncSession`).
//!
//! Shape (design "The FFI seam", committed): the FFI OWNS the iroh endpoint and
//! its internal tokio runtime. The native host constructs [`MobileNetRuntime`]
//! once (via [`bind_net_runtime`]) and calls [`MobileNetRuntime::sync_with_anchor`]
//! — a synchronous, trigger-and-observe entry point (the runtime `block_on`s the
//! async dial internally). The host never touches a socket or a frame.
//!
//! Type projection (why this layer exists rather than exporting the internal
//! types verbatim): UniFFI cannot carry `[u8; 32]`, `usize`, or the foreign
//! payloads (`AuthorityError`, `MobileError`) inside the internal
//! [`AnchorPullError`]. The outcome records (`AnchorSyncOutcome`,
//! `NamespacePullOutcome`) ARE the FFI records directly (ids projected to
//! lowercase hex, counts to `u32`, per the crate's id-as-hex convention); the
//! error is flattened into the FFI [`AnchorSyncError`] via `From`.
//!
//! Concurrency: the internal runtime is a single-threaded `current_thread` tokio
//! runtime; two concurrent `block_on`s on it are unsound. A `Mutex<NetRuntime>`
//! serialises calls, so the shared `Arc<MobileNetRuntime>` is safe to hold and
//! call from any thread — matching the "one runtime, drive one sync at a time"
//! design.

use std::sync::{Arc, Mutex};

use riot_transport::iroh::addr_from_hint;

use super::anchor::{AnchorPullError, AnchorSyncOutcome};
use super::NetRuntime;
use crate::mobile_api::MobileProfile;

/// Why a `MobileNetRuntime::sync_with_anchor` call failed before producing an
/// outcome. Flat `uniffi::Error` projection of the internal typed
/// [`AnchorPullError`] — every variant is fail-closed (nothing imported).
#[derive(Debug, uniffi::Error)]
pub enum AnchorSyncError {
    /// The ticket bytes did not decode as a `RootSignedTicketCoreEnvelopeV2`.
    TicketMalformed,
    /// The transport-floor gate REFUSED the dial (bad root signature, expired,
    /// epoch rollback, or a `require:arti` floor the phone cannot provide). NO
    /// connection was opened — the security-critical fail-closed refusal.
    DialRefused { reason: String },
    /// A network/transport fault while pulling.
    Transport { reason: String },
    /// The phone-store import failed (carries the `MobileError` code string).
    Import { reason: String },
    /// The anchor address hint did not parse to a dialable endpoint address.
    BadAnchorAddress { reason: String },
    /// No relay has been remembered by this durable profile yet.
    NoRelayConfigured,
    /// The FFI-owned iroh endpoint / tokio runtime could not be bound.
    Bind { reason: String },
}

impl std::fmt::Display for AnchorSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TicketMalformed => write!(f, "TICKET_MALFORMED"),
            Self::DialRefused { reason } => write!(f, "DIAL_REFUSED: {reason}"),
            Self::Transport { reason } => write!(f, "TRANSPORT_ERROR: {reason}"),
            Self::Import { reason } => write!(f, "IMPORT_ERROR: {reason}"),
            Self::BadAnchorAddress { reason } => write!(f, "BAD_ANCHOR_ADDRESS: {reason}"),
            Self::NoRelayConfigured => write!(f, "NO_RELAY_CONFIGURED"),
            Self::Bind { reason } => write!(f, "BIND_ERROR: {reason}"),
        }
    }
}

impl std::error::Error for AnchorSyncError {}

impl From<AnchorPullError> for AnchorSyncError {
    fn from(error: AnchorPullError) -> Self {
        match error {
            AnchorPullError::TicketMalformed => Self::TicketMalformed,
            AnchorPullError::DialRefused(authority) => Self::DialRefused {
                reason: authority.to_string(),
            },
            AnchorPullError::Transport(reason) => Self::Transport { reason },
            AnchorPullError::Import(mobile) => Self::Import {
                reason: mobile.to_string(),
            },
        }
    }
}

/// The FFI-owned non-local transport runtime: the iroh endpoint + internal tokio
/// runtime, wrapped for Swift/Kotlin. Construct once with [`bind_net_runtime`];
/// the shared handle drives anchor pulls one at a time.
#[derive(uniffi::Object)]
pub struct MobileNetRuntime {
    /// The owned runtime. `Mutex` serialises `block_on`s on the single-threaded
    /// tokio runtime (see the module note on concurrency).
    inner: Mutex<NetRuntime>,
}

/// Bind the FFI-owned iroh endpoint + internal tokio runtime (an ephemeral,
/// unlinkable follower identity per the anonymity design) and hand back a shared
/// [`MobileNetRuntime`] handle. Synchronous from the caller's view.
#[uniffi::export]
pub fn bind_net_runtime() -> Result<Arc<MobileNetRuntime>, AnchorSyncError> {
    let runtime = NetRuntime::bind_follower().map_err(|error| AnchorSyncError::Bind {
        reason: error.to_string(),
    })?;
    Ok(Arc::new(MobileNetRuntime {
        inner: Mutex::new(runtime),
    }))
}

#[uniffi::export]
impl MobileNetRuntime {
    /// Pull using the durable profile's next remembered relay. The profile owns
    /// the relay's NodeId and admission ticket; the host never has to reconstruct
    /// either value from a compiled-in constant.
    pub fn sync_with_next_relay(
        &self,
        profile: Arc<MobileProfile>,
        now_unix: u64,
    ) -> Result<AnchorSyncOutcome, AnchorSyncError> {
        let relay = profile
            .relay_for_next_pull()
            .map_err(|error| AnchorSyncError::Import {
                reason: error.to_string(),
            })?
            .ok_or(AnchorSyncError::NoRelayConfigured)?;
        let outcome = self.sync_with_anchor(
            Arc::clone(&profile),
            relay.node_id.clone(),
            relay.ticket_bytes,
            now_unix,
        )?;
        crate::mobile_state::mark_relay_answered(&profile.inner, &relay.node_id, now_unix)
            .map_err(|error| AnchorSyncError::Import {
                reason: error.to_string(),
            })?;
        Ok(outcome)
    }

    /// Pull a community's committed O/C/W data from the anchor identified by
    /// `anchor_hint` (a `<id_hex>` or `<id_hex>@<ip:port>,…` node hint), verify
    /// every entry through the canonical gate, and import the store-admissible
    /// entries into `profile`'s willow store through the canonical
    /// preview→plan→commit boundary.
    ///
    /// Synchronous (the design's `block_on` seam): the gated dial + ReadCommitted
    /// FSM drive runs to completion on the FFI-owned runtime before returning.
    /// `now_unix` is the wall-clock second used for ticket freshness.
    ///
    /// Fail-closed order: the transport-floor gate runs BEFORE any packet; a
    /// `require:arti` ticket the phone cannot satisfy returns `DialRefused` with
    /// no connection opened. Raw ungated `sync_connect` is never exposed.
    pub fn sync_with_anchor(
        &self,
        profile: Arc<MobileProfile>,
        anchor_hint: String,
        ticket_bytes: Vec<u8>,
        now_unix: u64,
    ) -> Result<AnchorSyncOutcome, AnchorSyncError> {
        let anchor_addr =
            addr_from_hint(&anchor_hint).map_err(|error| AnchorSyncError::BadAnchorAddress {
                reason: error.to_string(),
            })?;
        let runtime = self.inner.lock().map_err(|_| AnchorSyncError::Transport {
            reason: "net runtime lock poisoned".to_string(),
        })?;
        runtime
            .sync_with_anchor(&profile.inner, anchor_addr, &ticket_bytes, now_unix)
            .map_err(AnchorSyncError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile_api::RelayRecord;
    use crate::mobile_state::open_local_profile;

    #[test]
    fn registered_pull_reads_the_profile_selected_relay() {
        let profile = open_local_profile().expect("profile opens");
        profile
            .add_relay(RelayRecord {
                node_id: "11".repeat(32),
                ticket_bytes: vec![0xa1, 0xb2, 0xc3],
                last_answered_unix_seconds: None,
            })
            .expect("remember relay");
        let runtime = bind_net_runtime().expect("net runtime binds");

        let error = runtime
            .sync_with_next_relay(profile, 1)
            .expect_err("the selected test ticket is malformed");
        assert!(matches!(error, AnchorSyncError::TicketMalformed));
    }
}
