//! The namespace-scoped read/write API apps use. `key`/`prefix` are always
//! relative to `apps/<app_id>/` — callers (the native WebView bridge) never
//! see or set a full Willow path. Writes are signed with the *calling
//! person's* own identity, exactly like a self-authored alert
//! (`riot-ffi::mobile_state::sign_draft`), and land through the identical
//! inspect/plan/commit pipeline — no separate trusted-write path. Reads are
//! served from the payload bytes the store retains for live app-data
//! entries (`import::join::Stored::payload`).

use ed25519_dalek::Signature;
use willow25::entry::EntrylikeExt;
use willow25::groupings::Keylike;

use crate::import::bundle::encode_bundle;
use crate::session::{CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, SessionError};
use crate::willow::entry::SignedWillowEntry;
use crate::willow::identity::EvidenceAuthor;
use crate::willow::{authorise_entry, encode_capability, encode_entry, Entry};

use super::entry::{app_data_path, build_app_data_entry, APP_ID_BYTES};
use super::AppsError;

pub struct AppDataBridge;

impl AppDataBridge {
    pub fn put(
        store: &EvidenceStore,
        author: &EvidenceAuthor,
        app_id: &[u8; APP_ID_BYTES],
        key: &str,
        willow_timestamp_micros: u64,
        value: &[u8],
    ) -> Result<(), AppsError> {
        Self::put_returning_bundle(store, author, app_id, key, willow_timestamp_micros, value)
            .map(|_| ())
    }

    /// Exactly `put`, but hands back the canonical signed bundle bytes it
    /// committed. Hosts that persist app data across relaunch (the native
    /// runtime saves the bytes and replays them into a fresh profile) need
    /// the committed bundle; `put` delegates here so both paths sign, encode,
    /// and admit through one implementation.
    pub fn put_returning_bundle(
        store: &EvidenceStore,
        author: &EvidenceAuthor,
        app_id: &[u8; APP_ID_BYTES],
        key: &str,
        willow_timestamp_micros: u64,
        value: &[u8],
    ) -> Result<Vec<u8>, AppsError> {
        let entry = build_app_data_entry(author, app_id, key, willow_timestamp_micros, value)?;
        let authorised = authorise_entry(author, entry)?;
        let token = authorised.authorisation_token();
        let signature: Signature = token.signature().clone().into();
        let signed = SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes: value.to_vec(),
        };
        let bundle_bytes =
            encode_bundle(std::slice::from_ref(&signed)).map_err(|_| AppsError::StoreRejected)?;

        let preview = match store
            .inspect(&bundle_bytes, ImportContext::new("app-write"))
            .map_err(session_err)?
        {
            InspectOutcome::Preview(p) => p,
            InspectOutcome::Rejected(_) => return Err(AppsError::StoreRejected),
        };
        let plan = preview.plan_all().map_err(session_err)?;
        match plan.commit().map_err(session_err)? {
            CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => Ok(bundle_bytes),
        }
    }

    pub fn get(
        store: &EvidenceStore,
        app_id: &[u8; APP_ID_BYTES],
        key: &str,
    ) -> Result<Option<Vec<u8>>, AppsError> {
        let path = app_data_path(app_id, key)?;
        let matches = store.entries_with_prefix(&path).map_err(session_err)?;
        // Same-key entries from different subspaces never prune each other
        // (Willow pruning is per-subspace), so several may be live at once;
        // surface exactly one winner by Willow's own recency order.
        Ok(matches
            .into_iter()
            .filter(|(_, entry, _)| entry.path() == &path)
            .max_by(|(_, a, _), (_, b, _)| a.cmp_recency(b))
            .and_then(|(_, _, payload)| payload))
    }

    /// Every live `(relative_key, value)` pair under `apps/<app_id>/<prefix>`,
    /// sorted by key for deterministic output.
    pub fn list(
        store: &EvidenceStore,
        app_id: &[u8; APP_ID_BYTES],
        prefix: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, AppsError> {
        let path = app_data_path(app_id, prefix)?;
        let matches = store.entries_with_prefix(&path).map_err(session_err)?;
        // One winner per key across subspaces, same recency order as `get`.
        let mut winners: Vec<(String, Entry, Vec<u8>)> = Vec::new();
        for (_, entry, payload) in matches {
            // Entries under an app prefix always retain their payload; a
            // `None` here would mean a non-app entry somehow matched, which
            // the admission shape check makes impossible — skip defensively
            // rather than panic.
            let Some(payload) = payload else { continue };
            let key = relative_key(&entry)?;
            match winners.iter_mut().find(|(existing, _, _)| *existing == key) {
                Some((_, best, best_payload)) => {
                    if entry.cmp_recency(best) == std::cmp::Ordering::Greater {
                        *best = entry;
                        *best_payload = payload;
                    }
                }
                None => winners.push((key, entry, payload)),
            }
        }
        let mut items: Vec<(String, Vec<u8>)> = winners
            .into_iter()
            .map(|(key, _, payload)| (key, payload))
            .collect();
        items.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        Ok(items)
    }
}

/// The `items/abc` part of `apps/<app_id>/items/abc`. Admission guarantees
/// app-data key segments are ASCII, so UTF-8 conversion cannot fail on a
/// well-formed entry; a malformed one maps to `PathInvalid`.
fn relative_key(entry: &Entry) -> Result<String, AppsError> {
    let segments: Result<Vec<&str>, AppsError> = entry
        .path()
        .components()
        .skip(2)
        .map(|component| {
            std::str::from_utf8(component.as_ref()).map_err(|_| AppsError::PathInvalid)
        })
        .collect();
    Ok(segments?.join("/"))
}

fn session_err(_: SessionError) -> AppsError {
    AppsError::StoreRejected
}
