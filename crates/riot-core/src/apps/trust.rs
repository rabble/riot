//! Per-space app trust evaluation. Trust authority is a fixed, known list
//! of organizer `SubspaceId`s for the space (see the design spec's
//! planning-time correction — this codebase has no capability-delegation
//! concept to reuse). A marker from any other subspace at the trust-list
//! path is ignored. Among markers from a *recognized* organizer for the
//! same app, the most recent timestamp wins — ordinary last-write-wins,
//! same as any other Willow path.

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

pub fn is_trusted(
    app_id: &[u8; 32],
    markers: &[TrustMarker],
    organizer_subspace_ids: &[[u8; 32]],
) -> bool {
    // At an exact timestamp tie, Revoke outranks Trust (fail closed) so the
    // outcome never depends on marker slice order.
    let latest = markers
        .iter()
        .filter(|m| &m.app_id == app_id)
        .filter(|m| organizer_subspace_ids.contains(&m.author_subspace_id))
        .max_by_key(|m| (m.timestamp_micros, m.kind == TrustMarkerKind::Revoke));

    matches!(latest, Some(m) if m.kind == TrustMarkerKind::Trust)
}
