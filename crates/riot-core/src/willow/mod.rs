//! Willow entry construction, communal authority, and canonical encoding.
//!
//! Semantics per the implementation audit: a communal namespace confers no
//! root privilege (the ephemeral namespace secret is discarded at
//! generation); the subspace secret is the author signing secret; a
//! zero-delegation communal write capability is valid only for the named
//! author's own subspace; verification always goes through the checked
//! `PossiblyAuthorisedEntry` conversion.

use ufotofu::codec_prelude::{DecodableCanonic, EncodableExt};
use ufotofu::producer::clone_from_slice;
use willow25::authorisation::{AuthorisedEntry, PossiblyAuthorisedEntry, WriteCapability};
use willow25::prelude::*;

pub use willow25::authorisation::AuthorisationToken;
pub use willow25::entry::{Entry, NamespaceId, SubspaceId};
pub use willow25::paths::Path;

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
}

impl std::fmt::Display for WillowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for WillowError {}

/// An ephemeral communal evidence author: a communal namespace public key
/// plus the author's subspace signing secret. The namespace secret is
/// discarded at generation — in a communal namespace it grants nothing.
pub struct EvidenceAuthor {
    namespace_id: NamespaceId,
    subspace_secret: SubspaceSecret,
}

impl EvidenceAuthor {
    pub fn namespace_id(&self) -> &NamespaceId {
        &self.namespace_id
    }

    pub fn subspace_id(&self) -> SubspaceId {
        self.subspace_secret.corresponding_subspace_id()
    }

    /// Zero-delegation communal write capability for the author's own subspace.
    pub fn write_capability(&self) -> WriteCapability {
        WriteCapability::new_communal(self.namespace_id.clone(), self.subspace_id())
    }
}

/// Generates a fresh communal namespace and author subspace from OS
/// randomness. The private key stays inside this struct until drop.
pub fn generate_communal_author() -> EvidenceAuthor {
    let mut rng = rand_core::OsRng;
    let (namespace_id, _discarded_namespace_secret) =
        randomly_generate_communal_namespace(&mut rng);
    debug_assert!(namespace_id.is_communal());
    let (_subspace_id, subspace_secret) = randomly_generate_subspace(&mut rng);
    EvidenceAuthor {
        namespace_id,
        subspace_secret,
    }
}

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
    let path = alert_path(object_id, revision_id)?;
    Ok(Entry::builder()
        .namespace_id(author.namespace_id.clone())
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
        .into_authorised_entry(&capability, &author.subspace_secret)
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

/// Corrected WILLIAM3 digest of a payload (the Willow'25 payload digest).
pub fn william3_digest(payload: &[u8]) -> [u8; 32] {
    *PayloadDigest::from_payload(payload).as_bytes()
}

pub fn encode_entry(entry: &Entry) -> Vec<u8> {
    pollster::block_on(entry.new_vec_storing_encoding())
}

pub fn decode_entry_canonic(bytes: &[u8]) -> Result<Entry, WillowError> {
    decode_canonic_exact::<Entry>(bytes)
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
