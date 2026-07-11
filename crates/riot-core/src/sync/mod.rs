//! Bounded, transport-independent conference reconciliation.
//!
//! This module exchanges public canonical identities and bundle bytes only.
//! The caller must pass received `Entries` bundles through the existing
//! preview/plan/commit admission boundary before retaining them.

mod ffi_bridge;
mod reconcile;
mod state;
mod wire;

pub use ffi_bridge::{ByteSyncOutcome, ByteSyncSession};
pub use reconcile::missing_entry_ids;
pub use state::{ReconcileSession, SyncAction};
pub use wire::{
    decode_frame, encode_frame, SyncError, SyncFrame, MAX_SYNC_FRAME_BYTES, MAX_SYNC_IDS,
};
