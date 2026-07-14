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

/// The canonical pair invariant, in one place: decode both codecs, require
/// entry-point equality, and re-derive the content identity from the exact
/// bytes. Every path that accepts a manifest/bundle pair (install, publish,
/// scan, share) must go through this — the dual-digest-domain reconciliation
/// above records what happens when this invariant family forks.
pub fn verify_app_pair(manifest_bytes: &[u8], bundle_bytes: &[u8]) -> Result<AppId, AppsError> {
    let manifest = decode_manifest(manifest_bytes)?;
    let bundle = decode_app_bundle(bundle_bytes)?;
    if manifest.entry_point != bundle.entry_point {
        return Err(AppsError::IndexEntryMismatch);
    }
    app_id_for(&manifest, &app_bundle_digest(bundle_bytes))
}

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
    let app_id = verify_app_pair(manifest_bytes, bundle_bytes)?;
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

/// The exact stored payload bytes behind one app-index listing. Named rather
/// than a `(Vec<u8>, Vec<u8>)` pair because the install path takes the two
/// side by side and they are trivially swappable at a call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPairBytes {
    pub manifest_bytes: Vec<u8>,
    pub bundle_bytes: Vec<u8>,
}

/// Reads back the exact stored manifest and bundle payload bytes for `app_id`
/// — the bytes behind a directory listing, ready to feed straight into the
/// install path. This is what makes an app that *arrived* over sync openable
/// rather than merely visible.
///
/// Carriers are considered in the order `scan_app_index` ranks them (ascending
/// Willow `(namespace_id, subspace_id)`), so these are the bytes behind the
/// listing the caller was shown. A carrier whose pair does not re-derive
/// `app_id` is skipped, not fatal: any peer may write an
/// `app-index/<app_id>/bundle` entry, so one hostile carrier must not be able
/// to block installing an app an honest carrier also holds. `Ok(None)`
/// therefore means exactly what `bundle_present: false` means — no carrier
/// holds a complete verified pair (unknown app, still-arriving bundle, or
/// nothing but unverifiable copies).
///
/// The pair leaves through `verify_app_pair`, the same invariant publish and
/// scan enforce, so bytes that do not derive the requested `app_id` are never
/// returned.
pub fn app_pair_bytes(
    store: &EvidenceStore,
    app_id: &AppId,
) -> Result<Option<AppPairBytes>, AppsError> {
    let prefix = app_index_prefix_for(app_id)?;
    let entries = store
        .entries_with_prefix(&prefix)
        .map_err(|_| AppsError::StoreRejected)?;
    type Carrier = ([u8; 32], [u8; 32]);
    let mut manifests: BTreeMap<Carrier, Vec<u8>> = BTreeMap::new();
    let mut bundles: BTreeMap<Carrier, Vec<u8>> = BTreeMap::new();

    for (_, entry, payload) in entries {
        let Some(payload) = payload else { continue };
        let carrier = (
            *entry.namespace_id().as_bytes(),
            *entry.subspace_id().as_bytes(),
        );
        match classify_app_index_path(entry.path()) {
            Some(AppIndexSlot::Manifest { app_id: slot }) if slot == *app_id => {
                manifests.insert(carrier, payload);
            }
            Some(AppIndexSlot::Bundle { app_id: slot }) if slot == *app_id => {
                bundles.insert(carrier, payload);
            }
            _ => {}
        }
    }

    for (carrier, manifest_bytes) in manifests {
        let Some(bundle_bytes) = bundles.get(&carrier) else {
            continue;
        };
        if verify_app_pair(&manifest_bytes, bundle_bytes).ok() == Some(*app_id) {
            return Ok(Some(AppPairBytes {
                manifest_bytes,
                bundle_bytes: bundle_bytes.clone(),
            }));
        }
    }
    Ok(None)
}

/// The pair for `app_id` if the embedded built-in catalog holds it.
///
/// A built-in app's bytes are compiled into the binary; they are never written
/// to the store and never arrive over sync. Any resolver that reads only the
/// store therefore cannot open a built-in — which is exactly how the directory
/// came to list built-ins (it merges `verify_starter_catalog` with
/// `scan_app_index`) while installing one always failed. The catalog is passed
/// in rather than read from `starter`, so this stays a pure function of the
/// bytes handed to it.
///
/// Every candidate leaves through `verify_app_pair`, the same canonical
/// invariant publish, scan, and install enforce: an embedded pair that fails to
/// re-derive the requested id is skipped, so a corrupt built-in is no more
/// installable than a corrupt carried one, and the caller still sees the honest
/// "not resolvable here" outcome.
pub fn starter_pair_bytes(catalog: &[(&[u8], &[u8])], app_id: &AppId) -> Option<AppPairBytes> {
    catalog
        .iter()
        .find(|(manifest_bytes, bundle_bytes)| {
            verify_app_pair(manifest_bytes, bundle_bytes).ok().as_ref() == Some(app_id)
        })
        .map(|(manifest_bytes, bundle_bytes)| AppPairBytes {
            manifest_bytes: manifest_bytes.to_vec(),
            bundle_bytes: bundle_bytes.to_vec(),
        })
}

#[derive(Clone)]
struct ManifestCandidate {
    manifest: AppManifest,
    /// The exact payload bytes, retained so the completeness check can run
    /// through `verify_app_pair` rather than a re-implementation of it.
    bytes: Vec<u8>,
    timestamp_micros: u64,
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
    let mut bundles: BTreeMap<CarrierKey, Vec<u8>> = BTreeMap::new();
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
                            bytes: payload,
                            timestamp_micros,
                        },
                    );
                }
            }
            Some(AppIndexSlot::Bundle { app_id }) if decode_app_bundle(&payload).is_ok() => {
                bundles.insert((app_id, namespace_id, subspace_id), payload);
            }
            Some(AppIndexSlot::Bundle { .. }) => {}
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
        match bundles.get(&(app_id, namespace_id, subspace_id)) {
            None => pending_manifests.push(PendingManifest {
                claimed_app_id: app_id,
                manifest: candidate.manifest,
                carrier_namespace_id: namespace_id,
                carrier_subspace_id: subspace_id,
                manifest_timestamp_micros: candidate.timestamp_micros,
            }),
            Some(bundle_bytes)
                if verify_app_pair(&candidate.bytes, bundle_bytes).ok() == Some(app_id) =>
            {
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
            }
            Some(_) => {}
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
        reconcile_endorsements, verify_app_pair, EndorsementCandidate, EndorsementRecord,
        MAX_SCANNED_ENDORSEMENTS_PER_APP,
    };
    use crate::apps::manifest::{decode_manifest, encode_manifest};
    use crate::apps::starter::{verify_starter_catalog, STARTER_CATALOG};
    use crate::apps::AppsError;

    #[test]
    fn verify_app_pair_enforces_the_canonical_triple() {
        let (manifest_bytes, bundle_bytes) = STARTER_CATALOG[0];

        // Happy path: the derived id matches the starter catalog's own
        // verification of the same pair.
        let expected = verify_starter_catalog(STARTER_CATALOG)[0].app_id;
        assert_eq!(verify_app_pair(manifest_bytes, bundle_bytes), Ok(expected));

        // Entry-point mismatch between an otherwise-valid pair.
        let mut manifest = decode_manifest(manifest_bytes).expect("starter manifest");
        manifest.entry_point = "elsewhere.html".to_string();
        let mismatched = encode_manifest(&manifest).expect("re-encode");
        assert_eq!(
            verify_app_pair(&mismatched, bundle_bytes),
            Err(AppsError::IndexEntryMismatch)
        );

        // Garbage on either side is a decode failure, never a panic.
        assert!(verify_app_pair(&[0xff; 8], bundle_bytes).is_err());
        assert!(verify_app_pair(manifest_bytes, &[0xff; 8]).is_err());
    }

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
