//! Digest-bound share/join references for community Newswire spaces.
//!
//! A share reference is the small, encodable coordinate a community publishes so
//! a peer can find and *verify* its descriptor. It binds three 32-byte values:
//! the communal namespace, the descriptor's Willow entry id, and the WILLIAM3
//! digest of the descriptor's canonical payload. The digest is the
//! anti-substitution guard: a relay or gateway that swaps in a different
//! community name or editorial roster produces different canonical bytes, so the
//! recomputed digest no longer matches and [`verify_descriptor_matches`] returns
//! `false`. The reference carries no secret and grants no capability — it only
//! lets a receiver prove a descriptor is the one that was shared.

use crate::willow::william3_digest;

use super::entry::{NewswirePayload, VerifiedNewswireRecord};
use super::model::encode_space_descriptor;

/// The versioned, self-describing prefix of an encoded share reference. The
/// three hex coordinates follow it, `/`-separated.
pub const SHARE_REFERENCE_PREFIX: &str = "riot://newswire/join/v1/";

/// A digest-bound reference to a community Newswire space. All three fields are
/// public coordinates; none is a secret. Two references are equal iff they name
/// the same namespace, descriptor entry, and content digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewswireShareReferenceV1 {
    /// The communal namespace id the space lives in.
    pub namespace_id: [u8; 32],
    /// The Willow entry id of the space descriptor record.
    pub descriptor_entry_id: [u8; 32],
    /// WILLIAM3 digest over the descriptor's canonical CBOR payload. Binding
    /// this is what makes substitution of a different descriptor detectable.
    pub content_digest: [u8; 32],
}

/// Why a share reference could not be built or decoded. A closed, stable
/// vocabulary — dependency-specific errors never cross this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareReferenceError {
    /// The verified record handed to [`build_share_reference`] is not a space
    /// descriptor, so it cannot anchor a community reference.
    NotADescriptor,
    /// The descriptor payload could not be canonically re-encoded to derive its
    /// content digest.
    EncodingFailed,
    /// The string is not a well-formed `riot://newswire/join/v1/...` reference
    /// with exactly three 32-byte lowercase-hex coordinates.
    Malformed,
}

impl std::fmt::Display for ShareReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ShareReferenceError {}

/// Builds the digest-bound share reference for a verified space descriptor. The
/// content digest is recomputed from the descriptor's canonical payload, so the
/// reference always reflects the descriptor's real content — it cannot be minted
/// with a mismatched digest.
pub fn build_share_reference(
    descriptor: &VerifiedNewswireRecord,
) -> Result<NewswireShareReferenceV1, ShareReferenceError> {
    let NewswirePayload::SpaceDescriptor(payload) = descriptor.payload() else {
        return Err(ShareReferenceError::NotADescriptor);
    };
    let canonical =
        encode_space_descriptor(payload).map_err(|_| ShareReferenceError::EncodingFailed)?;
    Ok(NewswireShareReferenceV1 {
        namespace_id: descriptor.namespace_id(),
        descriptor_entry_id: descriptor.entry_id(),
        content_digest: william3_digest(&canonical),
    })
}

/// Whether `descriptor` is the exact record `reference` names: same namespace,
/// same entry id, and a content digest that matches the descriptor's own
/// canonical payload. A substituted descriptor — even one that reuses the
/// namespace and entry id but carries different content — fails the digest
/// comparison and returns `false`.
pub fn verify_descriptor_matches(
    reference: &NewswireShareReferenceV1,
    descriptor: &VerifiedNewswireRecord,
) -> bool {
    match build_share_reference(descriptor) {
        Ok(rebuilt) => &rebuilt == reference,
        Err(_) => false,
    }
}

/// Encodes a share reference to its canonical `riot://newswire/join/v1/...`
/// string, suitable for a link or a QR payload. Deterministic: the three
/// coordinates are lowercase hex, `/`-separated, in a fixed order.
pub fn encode_share_reference(reference: &NewswireShareReferenceV1) -> String {
    format!(
        "{SHARE_REFERENCE_PREFIX}{}/{}/{}",
        hex_encode(&reference.namespace_id),
        hex_encode(&reference.descriptor_entry_id),
        hex_encode(&reference.content_digest),
    )
}

/// Parses a canonical share-reference string back into its coordinates. Rejects
/// any string that is not the exact scheme with three 32-byte lowercase-hex
/// components — no partial or lenient decoding.
pub fn decode_share_reference(
    encoded: &str,
) -> Result<NewswireShareReferenceV1, ShareReferenceError> {
    let body = encoded
        .strip_prefix(SHARE_REFERENCE_PREFIX)
        .ok_or(ShareReferenceError::Malformed)?;
    let mut parts = body.split('/');
    let namespace_id = decode_hex32(parts.next().ok_or(ShareReferenceError::Malformed)?)?;
    let descriptor_entry_id = decode_hex32(parts.next().ok_or(ShareReferenceError::Malformed)?)?;
    let content_digest = decode_hex32(parts.next().ok_or(ShareReferenceError::Malformed)?)?;
    if parts.next().is_some() {
        return Err(ShareReferenceError::Malformed);
    }
    Ok(NewswireShareReferenceV1 {
        namespace_id,
        descriptor_entry_id,
        content_digest,
    })
}

fn hex_encode(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex32(text: &str) -> Result<[u8; 32], ShareReferenceError> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(ShareReferenceError::Malformed);
    }
    let mut out = [0u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        let start = index * 2;
        *slot = u8::from_str_radix(&text[start..start + 2], 16)
            .map_err(|_| ShareReferenceError::Malformed)?;
    }
    Ok(out)
}
