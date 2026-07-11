//! App-index paths: where a distributable app lives as Willow entries.
//! `app-index/<app_id>/manifest`, `app-index/<app_id>/bundle`, and
//! `app-index/<app_id>/endorsements/<endorser-subspace>`, and
//! `app-index/<app_id>/trust/<organizer-subspace>`. Deliberately a
//! different top-level component from `apps/<app_id>/...` (runtime data,
//! `entry.rs`) so an app writing a data key named "manifest" can never
//! collide with its own distribution entries.

use std::collections::BTreeMap;

use willow25::groupings::{Coordinatelike, Keylike, Namespaced};

use crate::session::{commit_at, EvidenceStore};
use crate::willow::identity::EvidenceAuthor;
use crate::willow::Path;

use super::bundle::decode_app_bundle;
use super::directory::{AppProvenance, EndorsementRecord, IndexedApp, SpaceTrust};
use super::endorse::decode_endorsement;
use super::entry::APP_ID_BYTES;
use super::manifest::{app_id_for, decode_manifest, AppId, AppManifest};
use super::trust::{decode_trust_marker, TrustMarker};
use super::AppsError;

pub const APP_INDEX_COMPONENT: &[u8] = b"app-index";

/// Local directory-materialization/DoS ceiling, not a protocol-validity
/// limit. Valid markers are sorted by stable Willow coordinates before this
/// per-app cap is applied, so store iteration order cannot choose survivors.
pub const MAX_SCANNED_ENDORSEMENTS_PER_APP: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScannedIndex {
    pub apps: Vec<IndexedApp>,
    pub endorsements: Vec<EndorsementRecord>,
    /// Namespace-grouped markers. Organizer recognition is deliberately left
    /// empty for the directory assembler's caller to supply as local policy.
    pub spaces: Vec<SpaceTrust>,
}

/// Reconciliation: this module briefly carried its own bundle digest under
/// the domain `riot/app-bundle-digest/v1`, duplicating
/// `bundle::app_bundle_digest` (domain `riot/app-bundle/v1`, the released
/// FFI install path). Two domains meant the same bundle bytes produced two
/// different app_ids depending on which code path computed them, breaking
/// dedup and trust once those paths met. There is exactly one canonical
/// digest domain now — `riot/app-bundle/v1` — and this re-export keeps
/// existing `index::app_bundle_digest` imports compiling against it.
pub use super::bundle::app_bundle_digest;

/// Validates a canonical manifest/bundle pair, derives its content identity,
/// then publishes the two Willow entries through normal admission. The two
/// commits are intentionally sequential: a live manifest may precede its
/// still-arriving bundle.
pub fn publish_app_index(
    store: &EvidenceStore,
    carrier: &EvidenceAuthor,
    manifest_bytes: &[u8],
    bundle_bytes: &[u8],
    willow_timestamp_micros: u64,
) -> Result<AppId, AppsError> {
    let manifest = decode_manifest(manifest_bytes)?;
    let bundle = decode_app_bundle(bundle_bytes)?;
    if manifest.entry_point != bundle.entry_point {
        return Err(AppsError::IndexEntryMismatch);
    }
    let app_id = app_id_for(&manifest, &app_bundle_digest(bundle_bytes))?;
    commit_at(
        store,
        carrier,
        &app_index_manifest_path(&app_id)?,
        manifest_bytes,
        willow_timestamp_micros,
    )?;
    commit_at(
        store,
        carrier,
        &app_index_bundle_path(&app_id)?,
        bundle_bytes,
        willow_timestamp_micros,
    )?;
    Ok(app_id)
}

#[derive(Clone)]
struct ManifestCandidate {
    manifest: AppManifest,
    timestamp_micros: u64,
}

#[derive(Clone)]
struct BundleCandidate {
    bytes: Vec<u8>,
    entry_point: String,
}

/// Scans the complete live app-index view. Invalid records are item-local and
/// silently ignored. When several carriers publish one app, a complete pair
/// wins over an incomplete one, then the lexicographically smallest Willow
/// `(namespace_id, subspace_id)` coordinate wins. That coordinate is made of
/// Willow's native identity bytes and is independent of store iteration.
pub fn scan_app_index(store: &EvidenceStore) -> Result<ScannedIndex, AppsError> {
    let prefix = Path::from_slices(&[APP_INDEX_COMPONENT]).map_err(|_| AppsError::PathInvalid)?;
    let entries = store
        .entries_with_prefix(&prefix)
        .map_err(|_| AppsError::StoreRejected)?;
    type CarrierKey = (AppId, [u8; 32], [u8; 32]);
    let mut manifests: BTreeMap<CarrierKey, ManifestCandidate> = BTreeMap::new();
    let mut bundles: BTreeMap<CarrierKey, BundleCandidate> = BTreeMap::new();
    let mut endorsements = Vec::new();
    let mut trust_by_namespace: BTreeMap<[u8; 32], Vec<TrustMarker>> = BTreeMap::new();

    for (_, entry, payload) in entries {
        let Some(payload) = payload else { continue };
        let namespace_id = *entry.namespace_id().as_bytes();
        let subspace_id = *entry.subspace_id().as_bytes();
        let timestamp_micros = u64::from(entry.timestamp());
        match classify_app_index_path(entry.path()) {
            Some(AppIndexSlot::Manifest { app_id }) => {
                if let Ok(manifest) = decode_manifest(&payload) {
                    manifests.insert(
                        (app_id, namespace_id, subspace_id),
                        ManifestCandidate {
                            manifest,
                            timestamp_micros,
                        },
                    );
                }
            }
            Some(AppIndexSlot::Bundle { app_id }) => {
                if let Ok(bundle) = decode_app_bundle(&payload) {
                    bundles.insert(
                        (app_id, namespace_id, subspace_id),
                        BundleCandidate {
                            bytes: payload,
                            entry_point: bundle.entry_point,
                        },
                    );
                }
            }
            Some(AppIndexSlot::Endorsement {
                app_id,
                endorser_subspace_id,
            }) => {
                if subspace_id != endorser_subspace_id {
                    continue;
                }
                let Ok(marker) = decode_endorsement(&payload) else {
                    continue;
                };
                if marker.app_id != app_id {
                    continue;
                }
                endorsements.push((
                    app_id,
                    endorser_subspace_id,
                    namespace_id,
                    EndorsementRecord {
                        app_id,
                        endorser_subspace_id,
                        retracted: marker.retracted,
                    },
                ));
            }
            Some(AppIndexSlot::Trust {
                app_id,
                organizer_subspace_id,
            }) => {
                if subspace_id != organizer_subspace_id {
                    continue;
                }
                let Ok(marker) = decode_trust_marker(&payload) else {
                    continue;
                };
                if marker.app_id != app_id {
                    continue;
                }
                trust_by_namespace
                    .entry(namespace_id)
                    .or_default()
                    .push(TrustMarker {
                        app_id,
                        author_subspace_id: organizer_subspace_id,
                        kind: marker.kind,
                        timestamp_micros,
                    });
            }
            None => {}
        }
    }

    let mut candidates: Vec<(bool, [u8; 32], [u8; 32], IndexedApp)> = Vec::new();
    for ((app_id, namespace_id, subspace_id), candidate) in manifests {
        let bundle = bundles.get(&(app_id, namespace_id, subspace_id));
        let bundle_present = if let Some(bundle) = bundle {
            if candidate.manifest.entry_point != bundle.entry_point
                || app_id_for(&candidate.manifest, &app_bundle_digest(&bundle.bytes)).ok()
                    != Some(app_id)
            {
                continue;
            }
            true
        } else {
            false
        };
        candidates.push((
            bundle_present,
            namespace_id,
            subspace_id,
            IndexedApp {
                app_id,
                manifest: candidate.manifest,
                bundle_present,
                provenance: AppProvenance::Carried {
                    carrier_subspace_id: subspace_id,
                },
                manifest_timestamp_micros: candidate.timestamp_micros,
            },
        ));
    }
    candidates.sort_by(|a, b| {
        a.3.app_id
            .cmp(&b.3.app_id)
            .then_with(|| b.0.cmp(&a.0))
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    let mut apps = Vec::new();
    for (_, _, _, app) in candidates {
        if apps
            .last()
            .is_none_or(|previous: &IndexedApp| previous.app_id != app.app_id)
        {
            apps.push(app);
        }
    }

    endorsements.sort_by_key(|(app_id, endorser, namespace, _)| (*app_id, *endorser, *namespace));
    let mut endorsement_counts: BTreeMap<AppId, usize> = BTreeMap::new();
    let endorsements = endorsements
        .into_iter()
        .filter_map(|(app_id, _, _, record)| {
            let count = endorsement_counts.entry(app_id).or_default();
            if *count == MAX_SCANNED_ENDORSEMENTS_PER_APP {
                return None;
            }
            *count += 1;
            Some(record)
        })
        .collect();

    let spaces = trust_by_namespace
        .into_iter()
        .map(|(space_namespace_id, mut markers)| {
            markers.sort_by_key(|marker| {
                (
                    marker.app_id,
                    marker.author_subspace_id,
                    marker.timestamp_micros,
                )
            });
            SpaceTrust {
                space_namespace_id,
                markers,
                organizer_subspace_ids: Vec::new(),
            }
        })
        .collect();
    Ok(ScannedIndex {
        apps,
        endorsements,
        spaces,
    })
}

pub fn app_index_prefix_for(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id]).map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_manifest_path(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id, b"manifest"])
        .map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_bundle_path(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id, b"bundle"]).map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_endorsement_path(
    app_id: &[u8; APP_ID_BYTES],
    endorser_subspace_id: &[u8; 32],
) -> Result<Path, AppsError> {
    Path::from_slices(&[
        APP_INDEX_COMPONENT,
        app_id,
        b"endorsements",
        endorser_subspace_id,
    ])
    .map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_trust_path(
    app_id: &[u8; APP_ID_BYTES],
    organizer_subspace_id: &[u8; 32],
) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id, b"trust", organizer_subspace_id])
        .map_err(|_| AppsError::PathInvalid)
}

/// Admission-boundary classification of an app-index path. Single source
/// of truth shared by local writes and the import pipeline's two gates,
/// same discipline as `entry::is_app_data_path`: locally constructible
/// paths (the builders above) and remotely admissible ones can never drift
/// apart. Size ceilings are not re-checked here — the import pipeline
/// already enforces `MAX_PATH_*` on every path before any schema decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIndexSlot {
    Manifest {
        app_id: [u8; APP_ID_BYTES],
    },
    Bundle {
        app_id: [u8; APP_ID_BYTES],
    },
    Endorsement {
        app_id: [u8; APP_ID_BYTES],
        endorser_subspace_id: [u8; 32],
    },
    Trust {
        app_id: [u8; APP_ID_BYTES],
        organizer_subspace_id: [u8; 32],
    },
}

/// Returns which app-index slot `path` addresses, or `None` when the path
/// is not exactly one of the four recognized shapes (wrong prefix, wrong
/// id length, unknown slot name, missing or extra trailing components).
pub fn classify_app_index_path(path: &Path) -> Option<AppIndexSlot> {
    let mut components = path.components();
    if components.next()?.as_ref() != APP_INDEX_COMPONENT {
        return None;
    }
    let app_id: [u8; APP_ID_BYTES] = components.next()?.as_ref().try_into().ok()?;
    let slot = components.next()?;
    match slot.as_ref() {
        b"manifest" => components
            .next()
            .is_none()
            .then_some(AppIndexSlot::Manifest { app_id }),
        b"bundle" => components
            .next()
            .is_none()
            .then_some(AppIndexSlot::Bundle { app_id }),
        b"endorsements" => {
            let endorser_subspace_id: [u8; 32] = components.next()?.as_ref().try_into().ok()?;
            components
                .next()
                .is_none()
                .then_some(AppIndexSlot::Endorsement {
                    app_id,
                    endorser_subspace_id,
                })
        }
        b"trust" => {
            let organizer_subspace_id: [u8; 32] = components.next()?.as_ref().try_into().ok()?;
            components.next().is_none().then_some(AppIndexSlot::Trust {
                app_id,
                organizer_subspace_id,
            })
        }
        _ => None,
    }
}
