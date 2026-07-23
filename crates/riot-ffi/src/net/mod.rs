//! FFI-owned non-local transport runtime (Slice 1 — scaffold, no behavior).
//!
//! This module is compiled ONLY under the off-by-default `net` feature. It
//! proves the load-bearing risk of the mobile-iroh direction: that iroh + an
//! internal tokio runtime build and link cleanly into the `riot-ffi`
//! staticlib/cdylib. There is NO dial logic, NO uniffi surface, and NO peer
//! behavior yet — see
//! `docs/decisions/2026-07-23-mobile-iroh-transport-design.md`, "The FFI seam".
//!
//! The committed seam (design "Core architectural decision"): `riot-ffi` OWNS
//! the iroh endpoint and an internal single-threaded tokio runtime, mirroring
//! `riot-client-net`'s `TokioTaskSpawner`/`IrohEndpointFactory` shape but
//! depending on `riot-transport` directly (`riot_transport::iroh::bind` /
//! `bind_public`). The native host never touches the socket — later slices add
//! synchronous `block_on` trigger/observe entry points. This skeleton only
//! constructs the runtime + endpoint and tears them down cleanly.

use tokio::runtime::{Builder, Runtime};

use riot_transport::iroh::{bind_public, node_id};
use riot_transport::seed::rand32;
use riot_transport::TransportError;

/// Slice 2 — the phone-side anchor pull client (`sync_with_anchor`).
mod anchor;

/// Slice 3a — the `net`-gated UniFFI bridge (`MobileNetRuntime`).
mod ffi;

pub use anchor::{AnchorPullError, AnchorSyncOutcome, NamespacePullOutcome, PulledItemReject};
pub use ffi::{bind_net_runtime, AnchorSyncError, MobileNetRuntime};

/// The Slice-2 Checkpoint-B-in-Rust e2e: an in-process anchor over loopback iroh,
/// the real `sync_with_anchor` pull, verify, and import. Test-only.
#[cfg(test)]
mod anchor_e2e;

/// Owns the FFI-internal tokio runtime and the iroh endpoint for one non-local
/// transport session. Constructed on the FFI side (never on the native host),
/// it drives async iroh operations to completion via `block_on` on its own
/// single-threaded runtime.
///
/// It binds an EPHEMERAL follower endpoint with a FRESH random secret each bind
/// (`bind_public(rand32())`, unlinkable NodeId per the anonymity design) under
/// the `N0` preset — relay + pkarr/DNS discovery ON. That is what lets the phone
/// reach the deployed anchor by its bare NodeId (discovery resolves the address)
/// and NAT-traverse via the relay: a pure dialer, so the advertised `sync/1`
/// ALPN is irrelevant (dials happen with `ALPN_SYNC_V2`).
pub struct NetRuntime {
    /// The internal single-threaded runtime. All iroh futures are driven here.
    /// Ordered before `endpoint` so it outlives the endpoint's teardown.
    runtime: Runtime,
    /// The owned iroh endpoint (ephemeral follower identity).
    endpoint: iroh::Endpoint,
}

impl NetRuntime {
    /// Build the internal runtime and bind an ephemeral follower endpoint.
    ///
    /// Synchronous from the caller's view (the design's block_on seam): the
    /// async bind is driven to completion on the freshly built runtime. Binds
    /// under the `N0` preset (relay + pkarr/DNS discovery) with a fresh random
    /// ephemeral secret so the phone can dial the deployed anchor by bare NodeId
    /// and NAT-traverse — the root-cause fix for "the client can't reach a relay".
    pub fn bind_follower() -> Result<Self, TransportError> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| TransportError::Io(std::io::Error::other(e.to_string())))?;
        let endpoint = runtime.block_on(bind_public(rand32()))?;
        Ok(Self { runtime, endpoint })
    }

    /// This endpoint's NodeId (public key). Ephemeral in Slice 1 — a fresh
    /// random identity each bind, unlinkable across sessions.
    pub fn node_id(&self) -> [u8; 32] {
        node_id(&self.endpoint)
    }
}

impl Drop for NetRuntime {
    fn drop(&mut self) {
        // Close the endpoint cleanly on the owned runtime before the runtime is
        // dropped. `close()` is bounded; a best-effort teardown is sufficient
        // for the scaffold.
        self.runtime.block_on(async {
            self.endpoint.close().await;
        });
    }
}

#[cfg(all(test, feature = "net"))]
mod tests {
    use super::*;

    /// Proves iroh + tokio link and run inside the `riot-ffi` crate: build the
    /// runtime, bind an endpoint, read a non-zero NodeId, and drop it (endpoint
    /// closes) without panicking. This is the Slice 1 acceptance test.
    #[test]
    fn net_runtime_binds_endpoint_and_reports_node_id() {
        let net = NetRuntime::bind_follower().expect("bind ephemeral follower endpoint");
        let id = net.node_id();
        // An Ed25519 public key is never all-zero.
        assert_ne!(id, [0u8; 32], "endpoint must report a real NodeId");
        // A second bind yields a DIFFERENT ephemeral identity (unlinkability).
        let other = NetRuntime::bind_follower().expect("bind second ephemeral endpoint");
        assert_ne!(
            id,
            other.node_id(),
            "each bind must produce a fresh ephemeral NodeId"
        );
        // Both drop here, exercising the runtime + endpoint teardown path.
    }
}
