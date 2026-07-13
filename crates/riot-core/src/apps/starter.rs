//! Built-in starter catalog. Built-ins are ordinary manifest+bundle pairs
//! run through the exact same decode/verify path as synced apps — "Built
//! into Riot" is a provenance label, not a trust shortcut, and a built-in
//! still needs an organizer's per-space trust decision to launch.
//!
//! Starter integrity is content-addressed, not signature-verified: there
//! are no committed private keys for built-in apps, only a fixed committed
//! public `AuthorIdentity` in each shipped manifest.

use super::bundle::decode_app_bundle;
use super::directory::{AppProvenance, IndexedApp};
use super::index::app_bundle_digest;
use super::manifest::{app_id_for, decode_manifest};

const CHECKLIST_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/checklist.manifest.cbor"
));
const CHECKLIST_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/checklist.bundle.cbor"
));
const SUPPLY_BOARD_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/supply-board.manifest.cbor"
));
const SUPPLY_BOARD_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/supply-board.bundle.cbor"
));
const ROLL_CALL_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/roll-call.manifest.cbor"
));
const ROLL_CALL_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/roll-call.bundle.cbor"
));
const QUICK_POLL_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/quick-poll.manifest.cbor"
));
const QUICK_POLL_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/quick-poll.bundle.cbor"
));
const CHAT_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/chat.manifest.cbor"
));
const CHAT_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/chat.bundle.cbor"
));
const DISPATCHES_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/dispatches.manifest.cbor"
));
const DISPATCHES_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/dispatches.bundle.cbor"
));
const WIKI_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/wiki.manifest.cbor"
));
const WIKI_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/wiki.bundle.cbor"
));
const PHOTO_WALL_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/photo-wall.manifest.cbor"
));
const PHOTO_WALL_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/photo-wall.bundle.cbor"
));

/// (manifest_bytes, bundle_bytes) pairs embedded at compile time.
pub const STARTER_CATALOG: &[(&[u8], &[u8])] = &[
    (CHECKLIST_MANIFEST, CHECKLIST_BUNDLE),
    (SUPPLY_BOARD_MANIFEST, SUPPLY_BOARD_BUNDLE),
    (ROLL_CALL_MANIFEST, ROLL_CALL_BUNDLE),
    (QUICK_POLL_MANIFEST, QUICK_POLL_BUNDLE),
    (CHAT_MANIFEST, CHAT_BUNDLE),
    (DISPATCHES_MANIFEST, DISPATCHES_BUNDLE),
    (WIKI_MANIFEST, WIKI_BUNDLE),
    (PHOTO_WALL_MANIFEST, PHOTO_WALL_BUNDLE),
];

/// Decodes and integrity-checks every pair; invalid pairs are silently
/// excluded, mirroring the import path's treatment of invalid items.
pub fn verify_starter_catalog(pairs: &[(&[u8], &[u8])]) -> Vec<IndexedApp> {
    pairs
        .iter()
        .filter_map(|(manifest_bytes, bundle_bytes)| {
            let manifest = decode_manifest(manifest_bytes).ok()?;
            let bundle = decode_app_bundle(bundle_bytes).ok()?;
            if manifest.entry_point != bundle.entry_point {
                return None;
            }
            let app_id = app_id_for(&manifest, &app_bundle_digest(bundle_bytes)).ok()?;
            Some(IndexedApp {
                app_id,
                manifest,
                bundle_present: true,
                provenance: AppProvenance::BuiltIn,
                manifest_timestamp_micros: 0,
            })
        })
        .collect()
}
