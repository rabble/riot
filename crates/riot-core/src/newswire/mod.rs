//! Closed, canonical payloads for community Newswire spaces.

mod entry;
mod model;
mod path;

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

#[cfg(feature = "conformance")]
pub use entry::{
    create_signed_editorial_action_with_clock, create_signed_news_post_with_clock,
    create_signed_space_descriptor_with_clock,
};
