//! Closed, canonical payloads for community Newswire spaces.

mod contributors;
mod entry;
mod model;
mod path;
mod projection;
mod share;
mod store;

/// Stable Newswire construction and inspection failures. Dependency-specific
/// codec and cryptography errors never cross this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewswireError {
    PathInvalid,
    ModelInvalid,
    CanonicalEntryInvalid,
    CanonicalCapabilityInvalid,
    EntryBytesExceeded,
    CapabilityBytesExceeded,
    PayloadBytesExceeded,
    CapabilityInvalid,
    SignatureInvalid,
    PayloadLengthMismatch,
    PayloadDigestMismatch,
    PathTimeMismatch,
    PathDigestMismatch,
    DuplicatedFieldMismatch,
    AuthorityInvalid,
    NonCommunalNamespace,
    ClockUnavailable,
    SigningFailed,
}

impl std::fmt::Display for NewswireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for NewswireError {}

/// Whether a path belongs to the reserved Newswire v1 family. This is a
/// prefix reservation, not a structural classifier: additional malformed
/// components remain Newswire and must not fall through to another schema.
pub fn is_newswire_prefix(path: &crate::willow::Path) -> bool {
    let mut components = path.components();
    components
        .next()
        .is_some_and(|component| component.as_ref() == b"newswire")
        && components
            .next()
            .is_some_and(|component| component.as_ref() == b"v1")
}

pub use contributors::{contributors, ContributorRowV1};
pub(crate) use entry::inspect_verified_components;
pub use entry::{
    create_signed_editorial_action, create_signed_news_post, create_signed_space_descriptor,
    inspect_news_record, NewswirePayload, SignedNewswireRecord, VerifiedNewswireRecord,
};
pub use model::{
    decode_editorial_action, decode_news_post, decode_space_descriptor, encode_editorial_action,
    encode_news_post, encode_space_descriptor, AlertProfileV1, EditorialActionKind,
    EditorialActionV1, NewsPostV1, NewswireModelError, OperationalProfileV1, RequestKind,
    RequestProfileV1, SpaceDescriptorV1, ACTION_SCHEMA, MAX_NEWSWIRE_PAYLOAD_BYTES, POST_SCHEMA,
    SPACE_SCHEMA,
};
pub use path::{classify_newswire_path, newswire_path, NewswirePathKind};
pub use projection::{
    project, NewswireProjection, NewswireProjectionError, PostTreatment, ProjectedEditorialAction,
    ProjectedPost, ProjectionClockV1, MAX_FUTURE_SKEW_MICROS, MAX_PROJECTED_RECORDS,
};
pub use share::{
    build_share_reference, decode_share_reference, encode_share_reference,
    verify_descriptor_matches, NewswireShareReferenceV1, ShareReferenceError,
    SHARE_REFERENCE_PREFIX,
};
pub use store::{
    contributors_for_space, load_space_descriptor, load_space_records, project_space,
    NewswireStoreError,
};

#[cfg(feature = "conformance")]
pub use entry::{
    create_signed_editorial_action_with_clock, create_signed_news_post_with_clock,
    create_signed_space_descriptor_with_clock,
};
