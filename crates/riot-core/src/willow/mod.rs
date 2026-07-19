//! Willow entry construction, communal authority, and canonical encoding.
//!
//! Semantics per the implementation audit: a communal namespace confers no
//! root privilege (the ephemeral namespace secret is discarded at
//! generation); the subspace secret is the author signing secret; a
//! zero-delegation communal write capability is valid only for the named
//! author's own subspace; verification always goes through the checked
//! `PossiblyAuthorisedEntry` conversion.

pub mod clock;
pub mod digest;
pub mod entry;
pub mod identity;
mod masthead;
mod owned;
pub mod site_paths;

use ufotofu::codec_prelude::{DecodableCanonic, EncodableExt};
use ufotofu::producer::clone_from_slice;
use willow25::authorisation::{AuthorisedEntry, PossiblyAuthorisedEntry, WriteCapability};
use willow25::prelude::*;

pub use willow25::authorisation::AuthorisationToken;
pub use willow25::entry::{Entry, NamespaceId, SubspaceId};
pub use willow25::paths::Path;

pub use clock::{system_snapshot, tai_j2000_micros_from_unix_seconds, ClockSnapshot};
pub use digest::{
    bundle_digest, entry_id, evidence_digest, object_digest, william3_digest, BundleDigest,
    EntryId, EvidenceDigest, ObjectDigest,
};
pub use entry::{create_signed_alert, AlertDraft, SignedAlert, SignedWillowEntry};
pub use identity::{
    generate_communal_author, generate_communal_author_for_namespace,
    generate_space_organizer_author, AuthorIdentity, EvidenceAuthor, NamespaceKind,
    SEALED_IDENTITY_BYTES,
};
pub use masthead::OwnedMasthead;
pub use owned::OwnedRoot;
pub use site_paths::{
    is_owned_moderation_entry, is_under_articles, is_under_mod, ARTICLES_COMPONENT,
    MANIFEST_COMPONENT, MOD_COMPONENT,
};

// Conformance-only injection surface: absent from the release riot-ffi graph.
#[cfg(feature = "conformance")]
pub use clock::{snapshot_from_unix_seconds, ClockSource, SystemClock};
#[cfg(feature = "conformance")]
pub use entry::create_signed_alert_with;
#[cfg(feature = "conformance")]
pub use identity::{generate_communal_author_with, EntropySource, OsEntropy};

/// Evidence path layout: `objects / alert / object_id / revision_id`.
/// Binary 16-byte ID components keep immutable revisions unrelated by
/// prefix, so no revision accidentally prunes another.
pub const OBJECTS_COMPONENT: &[u8] = b"objects";
pub const ALERT_COMPONENT: &[u8] = b"alert";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WillowError {
    PathInvalid,
    DoesNotAuthorise,
    DecodeFailed,
    TrailingBytes,
    /// `ENTROPY_UNAVAILABLE`: OS randomness failed; no author or ID was constructed.
    EntropyUnavailable,
    /// `CLOCK_UNAVAILABLE`: system/pre-epoch/range/conversion failure; no partial entry exists.
    ClockUnavailable,
    /// The draft failed alert validation against the snapshot-derived times.
    InvalidAlert(crate::model::AlertError),
    /// A shared-author factory was given an owned rather than communal namespace.
    NamespaceNotCommunal,
    /// An authenticated sealed identity was malformed, corrupted, or opened
    /// with the wrong wrapping key.
    SealedIdentityInvalid,
    /// The AEAD could not seal an otherwise valid identity.
    IdentitySealFailed,
    /// A sealed owned-masthead envelope was malformed, corrupted, opened with
    /// the wrong wrapping key, or decoded to a non-owned namespace.
    SealedMastheadInvalid,
    /// A section delegation was requested for an area whose path escapes `/articles/`.
    DelegationAreaEscapesArticles,
    /// A moderation delegation was requested for an area whose path escapes `/mod/`.
    DelegationAreaEscapesMod,
}

impl std::fmt::Display for WillowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for WillowError {}

pub fn alert_path(object_id: &[u8; 16], revision_id: &[u8; 16]) -> Result<Path, WillowError> {
    Path::from_slices(&[OBJECTS_COMPONENT, ALERT_COMPONENT, object_id, revision_id])
        .map_err(|_| WillowError::PathInvalid)
}

/// Builds the Willow entry for a signed alert payload. The payload digest
/// (corrected WILLIAM3) and length are computed from the exact bytes.
pub fn build_alert_entry(
    author: &EvidenceAuthor,
    object_id: &[u8; 16],
    revision_id: &[u8; 16],
    willow_timestamp_micros: u64,
    payload: &[u8],
) -> Result<Entry, WillowError> {
    let path = alert_path(object_id, revision_id)
        .expect("fixed-width alert identifiers always form a valid path");
    Ok(Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(willow_timestamp_micros)
        .payload(payload)
        .build())
}

/// Mints the authorisation token by signing the canonical entry encoding
/// with the author's subspace secret, provided the capability includes the
/// entry (same namespace, author's own subspace area).
pub fn authorise_entry(
    author: &EvidenceAuthor,
    entry: Entry,
) -> Result<AuthorisedEntry, WillowError> {
    let capability = author.write_capability();
    entry
        .into_authorised_entry(&capability, author.subspace_secret())
        .map_err(|_| WillowError::DoesNotAuthorise)
}

/// Checked verification of an untrusted (entry, token) pair: the capability
/// must grant the write and the receiver's signature over the canonical
/// entry bytes must verify. Never uses an unchecked conversion.
pub fn verify_entry(entry: &Entry, token: &AuthorisationToken) -> bool {
    PossiblyAuthorisedEntry::new(entry.clone(), token.clone())
        .into_authorised_entry()
        .is_ok()
}

pub fn encode_entry(entry: &Entry) -> Vec<u8> {
    pollster::block_on(entry.new_vec_storing_encoding())
}

pub fn decode_entry_canonic(bytes: &[u8]) -> Result<Entry, WillowError> {
    decode_canonic_exact::<Entry>(bytes)
}

/// The Willow timestamp (microseconds) of a canonical entry encoding, as a
/// plain integer. Lets callers outside this crate (riot-ffi) advance a
/// monotonic write floor past replayed entries without pulling in willow25's
/// entry traits — the willow value type never crosses the boundary.
pub fn entry_timestamp_micros(entry_bytes: &[u8]) -> Result<u64, WillowError> {
    Ok(u64::from(decode_entry_canonic(entry_bytes)?.timestamp()))
}

/// Whether canonical entry bytes use the exact alert path bound to the
/// decoded payload's object and revision IDs. Callers receive only a boolean,
/// never the generic Willow entry or path value.
pub fn alert_entry_path_matches_payload(
    entry_bytes: &[u8],
    object_id: &[u8; 16],
    revision_id: &[u8; 16],
) -> Result<bool, WillowError> {
    use willow25::groupings::Keylike;

    let entry = decode_entry_canonic(entry_bytes)?;
    let expected = alert_path(object_id, revision_id)
        .expect("fixed-width alert identifiers always form a valid path");
    Ok(entry.path() == &expected)
}

pub fn encode_capability(capability: &WriteCapability) -> Vec<u8> {
    pollster::block_on(capability.new_vec_storing_encoding())
}

pub fn decode_capability_canonic(bytes: &[u8]) -> Result<WriteCapability, WillowError> {
    decode_canonic_exact::<WriteCapability>(bytes)
}

/// Canonical decode that also rejects trailing bytes: the value must
/// consume the producer exactly.
fn decode_canonic_exact<T>(bytes: &[u8]) -> Result<T, WillowError>
where
    T: DecodableCanonic,
{
    let mut producer = clone_from_slice(bytes);
    let value = pollster::block_on(T::decode_canonic(&mut producer))
        .map_err(|_| WillowError::DecodeFailed)?;
    if !producer.remaining().is_empty() {
        return Err(WillowError::TrailingBytes);
    }
    Ok(value)
}

/// Convenience accessors used by tests and the import layer.
pub trait EntryFacts {
    fn payload_digest_bytes(&self) -> [u8; 32];
}

impl EntryFacts for Entry {
    fn payload_digest_bytes(&self) -> [u8; 32] {
        *self.payload_digest().as_bytes()
    }
}
