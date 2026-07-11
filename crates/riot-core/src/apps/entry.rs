//! App-data path construction and entry building. A key like `"items/<id>"`
//! maps to Willow path segments `apps / <app_id> / items / <id>` — the same
//! communal namespace/subspace an alert entry would use, just under a
//! different top-level component so app data never collides with evidence.

use crate::import::bundle::{MAX_PATH_COMPONENTS, MAX_PATH_COMPONENT_BYTES, MAX_PATH_TOTAL_BYTES};
use crate::willow::identity::EvidenceAuthor;
use crate::willow::{Entry, Path};

use super::AppsError;

pub const APPS_COMPONENT: &[u8] = b"apps";
pub const APP_ID_BYTES: usize = 32;

// An app ID must always fit within a single path component.
const _: () = assert!(APP_ID_BYTES <= MAX_PATH_COMPONENT_BYTES);

/// A key segment is a non-empty sequence of lowercase ASCII letters,
/// digits, or hyphens — the same safe-path-segment rule already used for
/// conference-fixture routes. `crypto.randomUUID()` output (lowercase hex
/// and hyphens) satisfies this directly.
fn is_valid_key_segment(segment: &[u8]) -> bool {
    !segment.is_empty()
        && segment
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn validate_segment(segment: &str) -> Result<(), AppsError> {
    if is_valid_key_segment(segment.as_bytes()) {
        Ok(())
    } else {
        Err(AppsError::KeySegmentInvalid)
    }
}

/// Admission-boundary shape check: `apps / <32-byte app_id> / <one or more
/// valid key segments>`. The single source of truth shared by
/// `app_data_path` (local writes) and the import pipeline's `verify_frame`
/// (remote entries), so locally constructible paths and remotely admissible
/// ones can never drift apart. Size ceilings are not re-checked here — the
/// import pipeline already enforces `MAX_PATH_*` on every path before any
/// schema decision.
pub fn is_app_data_path(path: &Path) -> bool {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return false;
    };
    if first.as_ref() != APPS_COMPONENT {
        return false;
    }
    let Some(app_id) = components.next() else {
        return false;
    };
    if app_id.len() != APP_ID_BYTES {
        return false;
    }
    let mut saw_key_segment = false;
    for component in components {
        if !is_valid_key_segment(component.as_ref()) {
            return false;
        }
        saw_key_segment = true;
    }
    saw_key_segment
}

pub fn app_data_path(app_id: &[u8; APP_ID_BYTES], key: &str) -> Result<Path, AppsError> {
    if key.is_empty() {
        return Err(AppsError::KeyEmpty);
    }
    let segments: Vec<&str> = key.split('/').collect();
    for segment in &segments {
        validate_segment(segment)?;
    }

    let component_count = 2 + segments.len();
    if component_count > MAX_PATH_COMPONENTS {
        return Err(AppsError::TooManyPathComponents);
    }
    let mut total_bytes = APPS_COMPONENT.len() + app_id.len();
    for segment in &segments {
        if segment.len() > MAX_PATH_COMPONENT_BYTES {
            return Err(AppsError::PathComponentTooLong);
        }
        total_bytes += segment.len();
    }
    if total_bytes > MAX_PATH_TOTAL_BYTES {
        return Err(AppsError::PathTooLong);
    }

    let mut raw_segments: Vec<&[u8]> = Vec::with_capacity(component_count);
    raw_segments.push(APPS_COMPONENT);
    raw_segments.push(app_id);
    for segment in &segments {
        raw_segments.push(segment.as_bytes());
    }
    Path::from_slices(&raw_segments).map_err(|_| AppsError::PathInvalid)
}

pub fn build_app_data_entry(
    author: &EvidenceAuthor,
    app_id: &[u8; APP_ID_BYTES],
    key: &str,
    willow_timestamp_micros: u64,
    payload: &[u8],
) -> Result<Entry, AppsError> {
    let path = app_data_path(app_id, key)?;
    Ok(Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(willow_timestamp_micros)
        .payload(payload)
        .build())
}
