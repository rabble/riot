//! Minimal profiles: a person's self-claimed display name, stored as an
//! ordinary signed Willow entry in their own subspace. Deliberately tiny —
//! one name field, no avatars, no persona linking (see
//! `docs/research/2026-07-11-user-profiles-willow-research.md` for the
//! larger identity design this leaves alone).
//!
//! The name is SELF-CLAIMED and unverified. Rendering rule (Earthstar's,
//! adopted): never show a claimed name without its key-derived suffix —
//! `resolver::render_display_name` is the only sanctioned way to display one.

pub mod card;
pub mod path;
// pub mod resolver;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    FieldInvalid,
    PathInvalid,
    Willow(crate::willow::WillowError),
    StoreRejected,
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ProfileError {}

impl From<crate::willow::WillowError> for ProfileError {
    fn from(e: crate::willow::WillowError) -> Self {
        ProfileError::Willow(e)
    }
}
