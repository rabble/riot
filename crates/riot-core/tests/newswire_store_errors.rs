//! The `NewswireStoreError` diagnostic surface: its stable string codes and the
//! projection-error conversion. Both are public, deterministic, and were
//! previously exercised only for the two variants that store queries return
//! directly (`DescriptorNotFound`, `StoreQueryFailed`).

use riot_core::newswire::{NewswireProjectionError, NewswireStoreError};

#[test]
fn every_store_error_renders_its_stable_screaming_snake_code() {
    for (error, code) in [
        (
            NewswireStoreError::DescriptorNotFound,
            "DESCRIPTOR_NOT_FOUND",
        ),
        (
            NewswireStoreError::DuplicateDescriptor,
            "DUPLICATE_DESCRIPTOR",
        ),
        (
            NewswireStoreError::MissingRetainedPayload,
            "MISSING_RETAINED_PAYLOAD",
        ),
        (
            NewswireStoreError::MalformedRetainedRecord,
            "MALFORMED_RETAINED_RECORD",
        ),
        (NewswireStoreError::EntryIdMismatch, "ENTRY_ID_MISMATCH"),
        (
            NewswireStoreError::DescriptorMismatch,
            "DESCRIPTOR_MISMATCH",
        ),
        (NewswireStoreError::NamespaceMismatch, "NAMESPACE_MISMATCH"),
        (
            NewswireStoreError::ConflictingDuplicate,
            "CONFLICTING_DUPLICATE",
        ),
        (
            NewswireStoreError::ProjectionLimitExceeded,
            "PROJECTION_LIMIT_EXCEEDED",
        ),
        (NewswireStoreError::StoreQueryFailed, "STORE_QUERY_FAILED"),
        (NewswireStoreError::DescriptorInvalid, "DESCRIPTOR_INVALID"),
        (NewswireStoreError::ClockUnavailable, "CLOCK_UNAVAILABLE"),
        (NewswireStoreError::ClockOutOfRange, "CLOCK_OUT_OF_RANGE"),
    ] {
        assert_eq!(error.to_string(), code);
        // The error carries no wrapped source.
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn projection_errors_map_onto_their_store_level_equivalents() {
    for (projection, store) in [
        (
            NewswireProjectionError::DescriptorInvalid,
            NewswireStoreError::DescriptorInvalid,
        ),
        (
            NewswireProjectionError::ConflictingDuplicate,
            NewswireStoreError::ConflictingDuplicate,
        ),
        (
            NewswireProjectionError::ProjectionLimitExceeded,
            NewswireStoreError::ProjectionLimitExceeded,
        ),
        (
            NewswireProjectionError::ClockUnavailable,
            NewswireStoreError::ClockUnavailable,
        ),
        (
            NewswireProjectionError::ClockOutOfRange,
            NewswireStoreError::ClockOutOfRange,
        ),
    ] {
        assert_eq!(NewswireStoreError::from(projection), store);
    }
}
