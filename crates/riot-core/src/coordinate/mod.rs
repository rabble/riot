//! Coordinate ledger: signed ask / offer / task records for a community room.
//!
//! A sibling to [`crate::newswire`] with its own reserved path prefix
//! `coordinate/v1/…`. Where newswire carries a flat post wire, the Coordinate
//! ledger carries *object-kind* records with a mutable derived lifecycle
//! (`Open → Claimed → Done`) computed in projection from separate status
//! records — see `docs/superpowers/specs/2026-07-24-coordinate-ledger-design.md`.
//!
//! WU-1 (this slice) lands the module skeleton, the [`model::CoordinateItemV1`]
//! object-kind record, and its path family. Signing/admission (`entry`), the
//! store prefix scan (`store`), status transitions, projection, and the FFI
//! surface are later work units and are intentionally absent here.

mod entry;
mod model;
mod path;
mod store;

/// Stable Coordinate construction and inspection failures at the module
/// boundary. Dependency-specific codec errors never cross this boundary — they
/// are mapped into these closed variants, exactly like [`crate::newswire`]'s
/// `NewswireError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateError {
    /// A Willow path could not be built from the requested Coordinate family.
    PathInvalid,
    /// A Coordinate payload failed canonical model validation.
    ModelInvalid,
    /// The signed entry bytes were not canonical / decodable.
    CanonicalEntryInvalid,
    /// The signed capability bytes were not canonical / decodable.
    CanonicalCapabilityInvalid,
    /// The entry byte length exceeded the import ceiling.
    EntryBytesExceeded,
    /// The capability byte length exceeded the import ceiling.
    CapabilityBytesExceeded,
    /// The payload byte length exceeded the Coordinate ceiling.
    PayloadBytesExceeded,
    /// The capability is not a zero-delegation communal write capability
    /// scoped exactly to the entry's namespace + subspace (the closed gate).
    CapabilityInvalid,
    /// The Ed25519 signature did not verify against the entry.
    SignatureInvalid,
    /// The entry's declared payload length did not match the payload bytes.
    PayloadLengthMismatch,
    /// The entry's declared payload digest did not match the payload bytes.
    PayloadDigestMismatch,
    /// The path's embedded time did not match the entry timestamp.
    PathTimeMismatch,
    /// The path's embedded digest did not match the payload digest.
    PathDigestMismatch,
    /// A field duplicated on both the path and the payload disagreed.
    DuplicatedFieldMismatch,
    /// The signing author lacks authority for the requested record.
    AuthorityInvalid,
    /// A Coordinate record was authored under a non-communal namespace.
    NonCommunalNamespace,
    /// The system clock was unavailable at signing time.
    ClockUnavailable,
    /// The Willow signing operation failed.
    SigningFailed,
}

impl std::fmt::Display for CoordinateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CoordinateError {}

/// Whether a path belongs to the reserved Coordinate v1 family. This is a
/// prefix reservation, not a structural classifier: additional malformed
/// components remain Coordinate and must not fall through to another schema
/// (mirror of [`crate::newswire::is_newswire_prefix`]).
pub fn is_coordinate_prefix(path: &crate::willow::Path) -> bool {
    let mut components = path.components();
    components
        .next()
        .is_some_and(|component| component.as_ref() == b"coordinate")
        && components
            .next()
            .is_some_and(|component| component.as_ref() == b"v1")
}

pub(crate) use entry::inspect_verified_components;
pub use entry::{
    create_signed_coordinate_item, inspect_coordinate_record, CoordinatePayload,
    SignedCoordinateRecord, VerifiedCoordinateRecord,
};
pub use model::{
    decode_coordinate_item, encode_coordinate_item, CoordinateItemV1, CoordinateKind,
    CoordinateModelError, COORDINATE_ITEM_SCHEMA, MAX_COORDINATE_PAYLOAD_BYTES,
};
pub use path::{classify_coordinate_path, coordinate_path, CoordinatePathKind};
pub use store::{load_ledger_records, CoordinateStoreError};

#[cfg(feature = "conformance")]
pub use entry::create_signed_coordinate_item_with_clock;
