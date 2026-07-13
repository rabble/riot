//! Per-space app trust evaluation. Trust authority is a fixed, known list
//! of organizer `SubspaceId`s for the space (see the design spec's
//! planning-time correction — this codebase has no capability-delegation
//! concept to reuse). A marker from any other subspace at the trust-list
//! path is ignored. Among markers from a *recognized* organizer for the
//! same app, the most recent timestamp wins — ordinary last-write-wins,
//! same as any other Willow path.

use minicbor::{Decoder, Encoder};
use willow25::groupings::{Coordinatelike, Keylike, Namespaced};

use crate::session::{commit_at, EvidenceStore};
use crate::willow::identity::EvidenceAuthor;

use super::index::{
    app_index_prefix_for, app_index_trust_path, classify_app_index_path, AppIndexSlot,
};
use super::AppsError;

const FIELD_COUNT: u64 = 2;
const MAX_TRUST_MARKER_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustMarkerKind {
    Trust,
    Revoke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustMarker {
    pub app_id: [u8; 32],
    pub author_subspace_id: [u8; 32],
    pub kind: TrustMarkerKind,
    pub timestamp_micros: u64,
}

/// Encode the signed payload fields of a trust marker. Author and timestamp
/// are deliberately omitted: the Willow entry supplies both identities.
pub fn encode_trust_marker(marker: &TrustMarker) -> Result<Vec<u8>, AppsError> {
    let mut buffer = Vec::new();
    let mut encoder = Encoder::new(&mut buffer);
    // Encoding into Vec is infallible, and this fixed schema is 38 bytes:
    // one map header, two one-byte keys, a two-byte byte-string header,
    // 32 app-id bytes, and one kind byte.
    encoder.map(FIELD_COUNT).expect("Vec encoder is infallible");
    encoder.u8(0).expect("Vec encoder is infallible");
    encoder
        .bytes(&marker.app_id)
        .expect("Vec encoder is infallible");
    encoder.u8(1).expect("Vec encoder is infallible");
    encoder
        .u8(match marker.kind {
            TrustMarkerKind::Trust => 0,
            TrustMarkerKind::Revoke => 1,
        })
        .expect("Vec encoder is infallible");
    Ok(buffer)
}

/// Strict canonical payload decoder. The returned author and timestamp are
/// placeholders; callers reading a stored marker replace them from the entry.
pub fn decode_trust_marker(input: &[u8]) -> Result<TrustMarker, AppsError> {
    if input.len() > MAX_TRUST_MARKER_BYTES {
        return Err(AppsError::IndexFieldInvalid);
    }
    let mut decoder = Decoder::new(input);
    let pairs = decoder
        .map()
        .map_err(|_| AppsError::IndexFieldInvalid)?
        .ok_or(AppsError::IndexFieldInvalid)?;
    if pairs != FIELD_COUNT {
        return Err(AppsError::IndexFieldInvalid);
    }

    let key0 = decoder.u64().map_err(|_| AppsError::IndexFieldInvalid)?;
    if key0 != 0 {
        return Err(AppsError::IndexFieldInvalid);
    }
    let app_id = decoder
        .bytes()
        .map_err(|_| AppsError::IndexFieldInvalid)?
        .try_into()
        .map_err(|_| AppsError::IndexFieldInvalid)?;
    let key1 = decoder.u64().map_err(|_| AppsError::IndexFieldInvalid)?;
    if key1 != 1 {
        return Err(AppsError::IndexFieldInvalid);
    }
    let kind = match decoder.u8().map_err(|_| AppsError::IndexFieldInvalid)? {
        0 => TrustMarkerKind::Trust,
        1 => TrustMarkerKind::Revoke,
        _ => return Err(AppsError::IndexFieldInvalid),
    };
    if decoder.position() != input.len() {
        return Err(AppsError::IndexFieldInvalid);
    }

    let marker = TrustMarker {
        app_id,
        author_subspace_id: [0; 32],
        kind,
        timestamp_micros: 0,
    };
    if encode_trust_marker(&marker).expect("trust marker encoding is infallible") != input {
        return Err(AppsError::IndexFieldInvalid);
    }
    Ok(marker)
}

pub fn write_trust_marker(
    store: &EvidenceStore,
    organizer: &EvidenceAuthor,
    app_id: &[u8; 32],
    kind: TrustMarkerKind,
    willow_timestamp_micros: u64,
) -> Result<(), AppsError> {
    // Lower timestamps are stale. At an equal timestamp, do not layer a
    // semantic Trust/Revoke rule over this one Willow coordinate: the normal
    // payload-digest recency tie-break selects the live entry, and commit_at
    // reports Ok for that native winner or StaleWrite for the loser.
    if let Some(current) = trust_markers_for(store, organizer.namespace_id().as_bytes(), app_id)?
        .into_iter()
        .find(|marker| marker.author_subspace_id == *organizer.subspace_id().as_bytes())
    {
        if willow_timestamp_micros < current.timestamp_micros {
            return Err(AppsError::StaleWrite);
        }
    }
    let marker = TrustMarker {
        app_id: *app_id,
        author_subspace_id: *organizer.subspace_id().as_bytes(),
        kind,
        timestamp_micros: willow_timestamp_micros,
    };
    let payload = encode_trust_marker(&marker).expect("trust marker encoding is infallible");
    let path = app_index_trust_path(app_id, organizer.subspace_id().as_bytes())
        .expect("fixed-size trust path components always satisfy Willow limits");
    commit_at(store, organizer, &path, &payload, willow_timestamp_micros)
}

pub fn trust_markers_for(
    store: &EvidenceStore,
    namespace_id: &[u8; 32],
    app_id: &[u8; 32],
) -> Result<Vec<TrustMarker>, AppsError> {
    let prefix = app_index_prefix_for(app_id)
        .expect("fixed-size app-index prefix components always satisfy Willow limits");
    let entries = store
        .entries_with_prefix(&prefix)
        .map_err(|_| AppsError::StoreRejected)?;
    let mut markers = Vec::new();
    for (_, entry, payload) in entries {
        if entry.namespace_id().as_bytes() != namespace_id {
            continue;
        }
        let Some(AppIndexSlot::Trust {
            app_id: path_app_id,
            organizer_subspace_id,
        }) = classify_app_index_path(entry.path())
        else {
            continue;
        };
        // Import admission already binds the path app id, author subspace,
        // retained payload, and decoded payload app id. Re-checking those
        // impossible states here only duplicated the admission boundary.
        let payload = payload.expect("app-index admission retains payload bytes");
        let decoded = decode_trust_marker(&payload)
            .expect("app-index admission validates canonical trust markers");
        markers.push(TrustMarker {
            app_id: path_app_id,
            author_subspace_id: organizer_subspace_id,
            kind: decoded.kind,
            timestamp_micros: u64::from(entry.timestamp()),
        });
    }
    markers.sort_by_key(|marker| marker.author_subspace_id);
    Ok(markers)
}

pub fn is_trusted(
    app_id: &[u8; 32],
    markers: &[TrustMarker],
    organizer_subspace_ids: &[[u8; 32]],
) -> bool {
    // Input must contain at most one Willow-resolved live marker per
    // recognized organizer coordinate. Ignore other apps and unrecognized
    // authors first; duplicates among the remaining coordinates mean the
    // caller supplied unresolved input, so fail closed instead of inventing
    // a second semantic winner.
    let mut eligible = Vec::new();
    for marker in markers.iter().filter(|marker| {
        &marker.app_id == app_id && organizer_subspace_ids.contains(&marker.author_subspace_id)
    }) {
        if eligible
            .iter()
            .any(|existing: &&TrustMarker| existing.author_subspace_id == marker.author_subspace_id)
        {
            return false;
        }
        eligible.push(marker);
    }

    // This timestamp/Revoke tie-break is reader policy across different
    // recognized organizer coordinates, after Willow resolved each one.
    let latest = eligible
        .into_iter()
        .max_by_key(|m| (m.timestamp_micros, m.kind == TrustMarkerKind::Revoke));

    matches!(latest, Some(m) if m.kind == TrustMarkerKind::Trust)
}
