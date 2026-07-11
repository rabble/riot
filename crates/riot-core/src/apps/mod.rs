//! Signed JS apps: manifest/bundle format, per-space trust list, and the
//! namespace-scoped data bridge apps use to read/write their own Willow
//! entries. Kept separate from `import/` (evidence-only).

pub mod bridge;
pub mod bundle;
pub mod directory;
pub mod endorse;
pub mod entry;
pub mod index;
pub mod manifest;
pub mod trust;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppsError {
    KeyEmpty,
    KeySegmentInvalid,
    TooManyPathComponents,
    PathComponentTooLong,
    PathTooLong,
    PathInvalid,
    Willow(crate::willow::WillowError),
    ManifestFieldInvalid,
    BundleFieldInvalid,
    BundleTooLarge,
    /// The store refused the write (session/budget/admission failure).
    StoreRejected,
    /// A local app-index write would replace an active import review.
    StoreBusy,
    /// The requested local write is older than, or conflicts at the same
    /// timestamp with, the live value at its exact Willow coordinate.
    StaleWrite,
    IndexFieldInvalid,
    EndorsementFieldInvalid,
    IndexEntryMismatch,
}

impl std::fmt::Display for AppsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AppsError {}

impl From<crate::willow::WillowError> for AppsError {
    fn from(e: crate::willow::WillowError) -> Self {
        AppsError::Willow(e)
    }
}
