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

/// The advertised, auto-installed catalog for a fresh generation-2 profile.
/// Seeded from the v1 bytes in WU-001; each Slice-4 work unit re-points one
/// entry to its generated v2 pair. Never resolve a held app from name/version —
/// only by exact app ID.
pub const CURRENT_STARTER_CATALOG: &[(&[u8], &[u8])] = &[
    (CHECKLIST_MANIFEST, CHECKLIST_BUNDLE),
    (SUPPLY_BOARD_MANIFEST, SUPPLY_BOARD_BUNDLE),
    (ROLL_CALL_MANIFEST, ROLL_CALL_BUNDLE),
    (QUICK_POLL_MANIFEST, QUICK_POLL_BUNDLE),
    (CHAT_MANIFEST, CHAT_BUNDLE),
    (DISPATCHES_MANIFEST, DISPATCHES_BUNDLE),
    (WIKI_MANIFEST, WIKI_BUNDLE),
    (PHOTO_WALL_MANIFEST, PHOTO_WALL_BUNDLE),
];

/// The frozen v1 built-ins. Never advertised as starters and never assigned a
/// synthetic directory timestamp; it exists only to resolve an already-held v1
/// ID for a generation-1/existing profile.
pub const LEGACY_BUILTIN_CATALOG: &[(&[u8], &[u8])] = &[
    (CHECKLIST_MANIFEST, CHECKLIST_BUNDLE),
    (SUPPLY_BOARD_MANIFEST, SUPPLY_BOARD_BUNDLE),
    (ROLL_CALL_MANIFEST, ROLL_CALL_BUNDLE),
    (QUICK_POLL_MANIFEST, QUICK_POLL_BUNDLE),
    (CHAT_MANIFEST, CHAT_BUNDLE),
    (DISPATCHES_MANIFEST, DISPATCHES_BUNDLE),
    (WIKI_MANIFEST, WIKI_BUNDLE),
    (PHOTO_WALL_MANIFEST, PHOTO_WALL_BUNDLE),
];

/// Plain back-compat alias: the advertised catalog. Every pre-split use
/// (`demo_fixture.rs`, the directory merge in `mobile_state.rs`, test fixtures)
/// references the advertised catalog, which is exactly `CURRENT_STARTER_CATALOG`,
/// so the alias keeps them correct AND compiling with zero migration. NOT
/// `#[deprecated]` — that attribute would fail `clippy -- -D warnings` on the
/// same-crate `demo_fixture.rs` uses.
pub const STARTER_CATALOG: &[(&[u8], &[u8])] = CURRENT_STARTER_CATALOG;

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
            let app_id = app_id_for(&manifest, &app_bundle_digest(bundle_bytes))
                .expect("a decoded manifest always re-encodes");
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
