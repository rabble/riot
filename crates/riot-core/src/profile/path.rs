//! Where a profile lives: `profile/<subspace_id>/card`. One slot per person,
//! last-write-wins, in that person's OWN subspace — the entry's subspace must
//! equal the path's subspace component, which `session.rs`'s inspect gate
//! enforces so nobody can write a name into someone else's slot.
//!
//! `classify_profile_path` is the single source of truth for this shape,
//! shared by local writes and the import pipeline's two admission gates —
//! the same discipline as `apps::entry::is_app_data_path` and
//! `apps::index::classify_app_index_path`.

use crate::willow::Path;

use super::ProfileError;

pub const PROFILE_COMPONENT: &[u8] = b"profile";
pub const SUBSPACE_ID_BYTES: usize = 32;
/// `profile` + `<subspace>` — the components before the slot name.
pub const PROFILE_PREFIX_COMPONENT_COUNT: usize = 2;

pub fn profile_card_path(subspace_id: &[u8; SUBSPACE_ID_BYTES]) -> Result<Path, ProfileError> {
    Path::from_slices(&[PROFILE_COMPONENT, subspace_id, b"card"])
        .map_err(|_| ProfileError::PathInvalid)
}

/// The whole `profile/` subtree — used by the resolver's prefix scan.
pub fn profile_prefix() -> Result<Path, ProfileError> {
    Path::from_slices(&[PROFILE_COMPONENT]).map_err(|_| ProfileError::PathInvalid)
}

/// Returns the subspace that owns this profile slot, or `None` for any path
/// that is not exactly `profile/<32-byte subspace>/card`.
pub fn classify_profile_path(path: &Path) -> Option<[u8; SUBSPACE_ID_BYTES]> {
    let mut components = path.components();
    if components.next()?.as_ref() != PROFILE_COMPONENT {
        return None;
    }
    let subspace_id: [u8; SUBSPACE_ID_BYTES] = components.next()?.as_ref().try_into().ok()?;
    if components.next()?.as_ref() != b"card" {
        return None;
    }
    components.next().is_none().then_some(subspace_id)
}

/// True for any path under the reserved `profile/` prefix, well-formed or
/// not. The import gate needs this to *reserve* the prefix: a malformed
/// profile path must be rejected outright, never fall through to the alert
/// schema.
pub fn is_profile_prefixed(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|component| component.as_ref() == PROFILE_COMPONENT)
}
