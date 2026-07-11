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

/// A key segment is a non-empty sequence of lowercase ASCII letters,
/// digits, or hyphens — the same safe-path-segment rule already used for
/// conference-fixture routes. `crypto.randomUUID()` output (lowercase hex
/// and hyphens) satisfies this directly.
fn validate_segment(segment: &str) -> Result<(), AppsError> {
    if segment.is_empty() {
        return Err(AppsError::KeySegmentInvalid);
    }
    if !segment
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(AppsError::KeySegmentInvalid);
    }
    Ok(())
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
    if app_id.len() > MAX_PATH_COMPONENT_BYTES {
        return Err(AppsError::PathComponentTooLong);
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
