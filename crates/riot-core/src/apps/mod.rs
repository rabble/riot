//! Signed JS apps: manifest/bundle format, per-space trust list, and the
//! namespace-scoped data bridge apps use to read/write their own Willow
//! entries. Kept separate from `import/` (evidence-only).

pub mod entry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppsError {
    KeyEmpty,
    KeySegmentInvalid,
    TooManyPathComponents,
    PathComponentTooLong,
    PathTooLong,
    PathInvalid,
    Willow(crate::willow::WillowError),
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
