//! Pure directory assembly: decoded index records in, sorted listings out.
//! No store dependency — Task 4's scan produces the inputs. The directory
//! is computed, never stored (see the design spec).

use std::collections::{BTreeMap, BTreeSet};

use crate::willow::identity::AuthorIdentity;

use super::manifest::{AppId, AppManifest};
use super::trust::{is_trusted, TrustMarker};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppProvenance {
    BuiltIn,
    Carried { carrier_subspace_id: [u8; 32] },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedApp {
    pub app_id: AppId,
    pub manifest: AppManifest,
    pub bundle_present: bool,
    pub provenance: AppProvenance,
    /// Willow timestamp of the manifest entry; 0 for built-ins.
    pub manifest_timestamp_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorsementRecord {
    pub app_id: AppId,
    pub endorser_subspace_id: [u8; 32],
    pub retracted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceTrust {
    pub space_namespace_id: [u8; 32],
    pub markers: Vec<TrustMarker>,
    pub organizer_subspace_ids: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryInputs {
    pub apps: Vec<IndexedApp>,
    pub endorsements: Vec<EndorsementRecord>,
    pub spaces: Vec<SpaceTrust>,
    /// Subspaces this phone has actually synced with — endorsers on this
    /// list are named groups; others only bump an anonymous count.
    pub met_subspace_ids: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EndorsementSummary {
    pub met_subspace_ids: Vec<[u8; 32]>,
    pub unmet_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppListing {
    pub app_id: AppId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: AuthorIdentity,
    pub permissions: Vec<String>,
    pub bundle_present: bool,
    pub provenance: AppProvenance,
    pub trusted_in_spaces: Vec<[u8; 32]>,
    pub endorsements: EndorsementSummary,
    /// Set when a manifest with the same (author signing key, name) and a
    /// newer manifest timestamp exists. Never set across different authors —
    /// impersonators don't get to "supersede" anyone.
    pub superseded_by: Option<AppId>,
}

pub fn assemble_directory(inputs: &DirectoryInputs) -> Vec<AppListing> {
    // Dedup by app_id; BuiltIn provenance wins, otherwise first seen wins.
    let mut by_id: BTreeMap<AppId, IndexedApp> = BTreeMap::new();
    for app in &inputs.apps {
        match by_id.get(&app.app_id) {
            Some(existing) if existing.provenance == AppProvenance::BuiltIn => {}
            Some(_) if app.provenance == AppProvenance::BuiltIn => {
                by_id.insert(app.app_id, app.clone());
            }
            Some(_) => {}
            None => {
                by_id.insert(app.app_id, app.clone());
            }
        }
    }

    // Supersession: within (author signing key, name), the newest manifest
    // timestamp wins; every other member points at it.
    let mut newest: BTreeMap<([u8; 32], String), (AppId, u64)> = BTreeMap::new();
    for app in by_id.values() {
        let key = (app.manifest.author.signing_key_id, app.manifest.name.clone());
        let candidate = (app.app_id, app.manifest_timestamp_micros);
        match newest.get(&key) {
            Some((_, ts)) if *ts >= candidate.1 => {}
            _ => {
                newest.insert(key, candidate);
            }
        }
    }

    let met: BTreeSet<[u8; 32]> = inputs.met_subspace_ids.iter().copied().collect();

    let mut listings: Vec<AppListing> = by_id
        .values()
        .map(|app| {
            let key = (app.manifest.author.signing_key_id, app.manifest.name.clone());
            let superseded_by = match newest.get(&key) {
                Some((winner, _)) if *winner != app.app_id => Some(*winner),
                _ => None,
            };

            let mut met_endorsers: BTreeSet<[u8; 32]> = BTreeSet::new();
            let mut unmet_endorsers: BTreeSet<[u8; 32]> = BTreeSet::new();
            for e in &inputs.endorsements {
                if e.app_id != app.app_id || e.retracted {
                    continue;
                }
                if met.contains(&e.endorser_subspace_id) {
                    met_endorsers.insert(e.endorser_subspace_id);
                } else {
                    unmet_endorsers.insert(e.endorser_subspace_id);
                }
            }

            let trusted_in_spaces: Vec<[u8; 32]> = inputs
                .spaces
                .iter()
                .filter(|s| is_trusted(&app.app_id, &s.markers, &s.organizer_subspace_ids))
                .map(|s| s.space_namespace_id)
                .collect();

            AppListing {
                app_id: app.app_id,
                name: app.manifest.name.clone(),
                description: app.manifest.description.clone(),
                version: app.manifest.version.clone(),
                author: app.manifest.author.clone(),
                permissions: app.manifest.permissions.clone(),
                bundle_present: app.bundle_present,
                provenance: app.provenance.clone(),
                trusted_in_spaces,
                endorsements: EndorsementSummary {
                    met_subspace_ids: met_endorsers.into_iter().collect(),
                    unmet_count: unmet_endorsers.len(),
                },
                superseded_by,
            }
        })
        .collect();

    listings.sort_by(|a, b| {
        b.endorsements
            .met_subspace_ids
            .len()
            .cmp(&a.endorsements.met_subspace_ids.len())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.app_id.cmp(&b.app_id))
    });
    listings
}
