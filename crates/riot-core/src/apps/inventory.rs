//! Generated current/legacy catalog inventory. Authoritative machine-checkable
//! record used by the repack `--check` audit and the existing-user presentation
//! descriptors. Runtime selection/authorization always uses the full app ID,
//! never name or semantic version — this report is documentation + audit only.

use sha2::{Digest, Sha256};

use super::bundle::decode_app_bundle;
use super::index::app_bundle_digest;
use super::manifest::{app_id_for, decode_manifest, AppId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CatalogMembership {
    pub current: bool,
    pub legacy: bool,
}

#[derive(Debug, Clone)]
pub struct InventoryEntry {
    pub app_id: AppId,
    pub name: String,
    pub version: String,
    pub manifest_sha256: [u8; 32],
    pub bundle_sha256: [u8; 32],
    pub manifest_bytes_len: usize,
    pub bundle_bytes_len: usize,
    pub resource_count: usize,
    pub membership: CatalogMembership,
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

/// Build the inventory for the two catalogs. Invalid pairs are skipped, mirroring
/// `verify_starter_catalog`. Membership is merged by app ID so a pair present in
/// both catalogs (the pre-v2 state) reports `current && legacy`.
pub fn catalog_inventory(
    current: &[(&[u8], &[u8])],
    legacy: &[(&[u8], &[u8])],
) -> Vec<InventoryEntry> {
    let mut out: Vec<InventoryEntry> = Vec::new();
    for (in_current, catalog) in [(true, current), (false, legacy)] {
        for (manifest_bytes, bundle_bytes) in catalog {
            let Ok(manifest) = decode_manifest(manifest_bytes) else {
                continue;
            };
            let Ok(bundle) = decode_app_bundle(bundle_bytes) else {
                continue;
            };
            if manifest.entry_point != bundle.entry_point {
                continue;
            }
            let Ok(app_id) = app_id_for(&manifest, &app_bundle_digest(bundle_bytes)) else {
                continue;
            };
            if let Some(existing) = out.iter_mut().find(|e| e.app_id == app_id) {
                existing.membership.current |= in_current;
                existing.membership.legacy |= !in_current;
                continue;
            }
            out.push(InventoryEntry {
                app_id,
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                manifest_sha256: sha256(manifest_bytes),
                bundle_sha256: sha256(bundle_bytes),
                manifest_bytes_len: manifest_bytes.len(),
                bundle_bytes_len: bundle_bytes.len(),
                resource_count: bundle.resources.len(),
                membership: CatalogMembership {
                    current: in_current,
                    legacy: !in_current,
                },
            });
        }
    }
    out
}
