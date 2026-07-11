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
}

/// Returns which app-index slot `path` addresses, or `None` when the path
/// is not exactly one of the three recognized shapes (wrong prefix, wrong
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
        _ => None,
    }
}
