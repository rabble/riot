//! App-index paths: where a distributable app lives as Willow entries.
//! `app-index/<app_id>/manifest`, `app-index/<app_id>/bundle`, and
//! `app-index/<app_id>/endorsements/<endorser-subspace>`, and
//! `app-index/<app_id>/trust/<organizer-subspace>`. Deliberately a
//! different top-level component from `apps/<app_id>/...` (runtime data,
//! `entry.rs`) so an app writing a data key named "manifest" can never
//! collide with its own distribution entries.

use std::collections::BTreeMap;

use willow25::entry::Entrylike;
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

/// A decoded manifest whose claimed path identity cannot yet be verified
/// because no matching bundle is live at the same Willow carrier coordinate.
/// Consumers may show this as "still arriving", but must not feed it into
/// trust, supersession, or launch decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingManifest {
    pub claimed_app_id: AppId,
    pub manifest: AppManifest,
    pub carrier_namespace_id: [u8; 32],
    pub carrier_subspace_id: [u8; 32],
    pub manifest_timestamp_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScannedIndex {
    pub apps: Vec<IndexedApp>,
    pub pending_manifests: Vec<PendingManifest>,
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
/// still-arriving bundle. Consequently, an error from the bundle commit may
/// be returned after the manifest commit has succeeded; callers must treat
/// publication as resumable partial arrival, not an atomic transaction.
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

#[derive(Clone)]
struct EndorsementCandidate {
    record: EndorsementRecord,
    timestamp_micros: u64,
    namespace_id: [u8; 32],
    payload_digest: [u8; 32],
    entry_id: [u8; 32],
}

impl EndorsementCandidate {
    /// Global reputation is keyed by endorser subspace, not namespace. A
    /// newer namespace copy wins; equal-time disagreement fails closed to a
    /// retraction. Fully tied values use immutable Willow identity bytes so
    /// input order can never select the result.
    fn wins_over(&self, other: &Self) -> bool {
        self.timestamp_micros > other.timestamp_micros
            || (self.timestamp_micros == other.timestamp_micros
                && (self.record.retracted && !other.record.retracted
                    || (self.record.retracted == other.record.retracted
                        && (self.namespace_id, self.payload_digest, self.entry_id)
                            < (other.namespace_id, other.payload_digest, other.entry_id))))
    }
}

/// Scans the complete live app-index view. Invalid records are item-local and
/// silently ignored. When several carriers publish one app, a complete pair
/// wins over an incomplete one, then the lexicographically smallest Willow
/// `(namespace_id, subspace_id)` coordinate wins. That coordinate is made of
/// Willow's native identity bytes and is independent of store iteration.
/// The endorsement ceiling bounds only returned materialized records; the
/// live store is still scanned so global `(app_id, endorser)` reconciliation
/// cannot be biased by early termination.
pub fn scan_app_index(store: &EvidenceStore) -> Result<ScannedIndex, AppsError> {
    let prefix = Path::from_slices(&[APP_INDEX_COMPONENT]).map_err(|_| AppsError::PathInvalid)?;
    let entries = store
        .entries_with_prefix(&prefix)
        .map_err(|_| AppsError::StoreRejected)?;
    type CarrierKey = (AppId, [u8; 32], [u8; 32]);
    let mut manifests: BTreeMap<CarrierKey, ManifestCandidate> = BTreeMap::new();
    let mut bundles: BTreeMap<CarrierKey, BundleCandidate> = BTreeMap::new();
    let mut endorsement_candidates = Vec::new();
    let mut trust_by_namespace: BTreeMap<[u8; 32], Vec<TrustMarker>> = BTreeMap::new();

    for (entry_id, entry, payload) in entries {
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
                let candidate = EndorsementCandidate {
                    record: EndorsementRecord {
                        app_id,
                        endorser_subspace_id,
                        retracted: marker.retracted,
                    },
                    timestamp_micros,
                    namespace_id,
                    payload_digest: *entry.payload_digest().as_bytes(),
                    entry_id,
                };
                endorsement_candidates.push(candidate);
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

    let mut candidates: Vec<([u8; 32], [u8; 32], IndexedApp)> = Vec::new();
    let mut pending_manifests = Vec::new();
    for ((app_id, namespace_id, subspace_id), candidate) in manifests {
        let bundle = bundles.get(&(app_id, namespace_id, subspace_id));
        let is_verified = bundle.is_some_and(|bundle| {
            candidate.manifest.entry_point == bundle.entry_point
                && app_id_for(&candidate.manifest, &app_bundle_digest(&bundle.bytes)).ok()
                    == Some(app_id)
        });
        if is_verified {
            candidates.push((
                namespace_id,
                subspace_id,
                IndexedApp {
                    app_id,
                    manifest: candidate.manifest,
                    bundle_present: true,
                    provenance: AppProvenance::Carried {
                        carrier_subspace_id: subspace_id,
                    },
                    manifest_timestamp_micros: candidate.timestamp_micros,
                },
            ));
        } else {
            pending_manifests.push(PendingManifest {
                claimed_app_id: app_id,
                manifest: candidate.manifest,
                carrier_namespace_id: namespace_id,
                carrier_subspace_id: subspace_id,
                manifest_timestamp_micros: candidate.timestamp_micros,
            });
        }
    }
    candidates.sort_by(|a, b| {
        a.2.app_id
            .cmp(&b.2.app_id)
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.1.cmp(&b.1))
    });
    let mut apps = Vec::new();
    for (_, _, app) in candidates {
        if apps
            .last()
            .is_none_or(|previous: &IndexedApp| previous.app_id != app.app_id)
        {
            apps.push(app);
        }
    }
    pending_manifests.sort_by_key(|pending| {
        (
            pending.claimed_app_id,
            pending.carrier_namespace_id,
            pending.carrier_subspace_id,
            pending.manifest_timestamp_micros,
        )
    });

    let endorsements = reconcile_endorsements(endorsement_candidates);

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
        pending_manifests,
        endorsements,
        spaces,
    })
}

fn reconcile_endorsements(candidates: Vec<EndorsementCandidate>) -> Vec<EndorsementRecord> {
    let mut by_endorser: BTreeMap<(AppId, [u8; 32]), EndorsementCandidate> = BTreeMap::new();
    for candidate in candidates {
        let key = (
            candidate.record.app_id,
            candidate.record.endorser_subspace_id,
        );
        match by_endorser.get(&key) {
            Some(current) if !candidate.wins_over(current) => {}
            _ => {
                by_endorser.insert(key, candidate);
            }
        }
    }
    let mut counts: BTreeMap<AppId, usize> = BTreeMap::new();
    by_endorser
        .into_iter()
        .filter_map(|((app_id, _), candidate)| {
            let count = counts.entry(app_id).or_default();
            if *count == MAX_SCANNED_ENDORSEMENTS_PER_APP {
                return None;
            }
            *count += 1;
            Some(candidate.record)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        reconcile_endorsements, EndorsementCandidate, EndorsementRecord,
        MAX_SCANNED_ENDORSEMENTS_PER_APP,
    };

    fn candidate(
        app_id: [u8; 32],
        endorser: [u8; 32],
        namespace: [u8; 32],
    ) -> EndorsementCandidate {
        EndorsementCandidate {
            record: EndorsementRecord {
                app_id,
                endorser_subspace_id: endorser,
                retracted: false,
            },
            timestamp_micros: 100,
            namespace_id: namespace,
            payload_digest: [3; 32],
            entry_id: namespace,
        }
    }

    #[test]
    fn two_hundred_fifty_six_namespace_copies_consume_one_output_slot() {
        let app_id = [7; 32];
        let duplicate = [1; 32];
        let mut candidates = Vec::new();
        for i in 0..MAX_SCANNED_ENDORSEMENTS_PER_APP {
            let mut namespace = [0u8; 32];
            namespace[..8].copy_from_slice(&(i as u64).to_be_bytes());
            candidates.push(candidate(app_id, duplicate, namespace));
        }
        for i in 0..(MAX_SCANNED_ENDORSEMENTS_PER_APP - 1) {
            let mut endorser = [2u8; 32];
            endorser[..8].copy_from_slice(&(i as u64).to_be_bytes());
            candidates.push(candidate(app_id, endorser, [9; 32]));
        }

        let records = reconcile_endorsements(candidates);
        assert_eq!(records.len(), MAX_SCANNED_ENDORSEMENTS_PER_APP);
        assert_eq!(
            records
                .iter()
                .filter(|record| record.endorser_subspace_id == duplicate)
                .count(),
            1
        );
    }
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
