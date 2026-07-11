//! App-index paths: where a distributable app lives as Willow entries.
//! `app-index/<app_id>/manifest`, `app-index/<app_id>/bundle`, and
//! `app-index/<app_id>/endorsements/<endorser-subspace>`. Deliberately a
//! different top-level component from `apps/<app_id>/...` (runtime data,
//! `entry.rs`) so an app writing a data key named "manifest" can never
//! collide with its own distribution entries.

use sha2::{Digest, Sha256};

use crate::willow::Path;

use super::entry::APP_ID_BYTES;
use super::AppsError;

pub const APP_INDEX_COMPONENT: &[u8] = b"app-index";

const APP_BUNDLE_DIGEST_DOMAIN: &[u8] = b"riot/app-bundle-digest/v1";

/// Domain-separated digest of the encoded `AppBundle` bytes — the
/// `bundle_digest` input to `manifest::app_id_for`. Pinned here (not in
/// `willow/digest.rs`) because it is app-platform identity, not Willow
/// entry identity.
pub fn app_bundle_digest(bundle_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(APP_BUNDLE_DIGEST_DOMAIN);
    hasher.update((bundle_bytes.len() as u32).to_be_bytes());
    hasher.update(bundle_bytes);
    hasher.finalize().into()
}

pub fn app_index_prefix_for(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id]).map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_manifest_path(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id, b"manifest"])
        .map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_bundle_path(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id, b"bundle"])
        .map_err(|_| AppsError::PathInvalid)
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
