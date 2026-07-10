//! Evidence bundle codec (`RiotEvidenceBundleV1`) and, in WU2, the
//! preview-first atomic import store.
//!
//! The bundle is a deliberately non-interoperable development codec: it is
//! not `.snk`, not Willow Drop Format, not a WTP stream. Visible magic
//! `RIOTE1`, then one deterministic CBOR document framing canonical Willow
//! bytes. Riot CBOR frames byte strings but never redefines Willow's own
//! field encodings.
//!
//! Decode order per the implementation audit: bounded outer CBOR →
//! canonical Entry → canonical capability → fixed 64-byte signature →
//! payload length/digest → Meadowcap authorisation. No untrusted item
//! reaches import staging before all prior checks pass.

use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};

use crate::willow::{
    decode_capability_canonic, decode_entry_canonic, encode_capability, encode_entry, verify_entry,
    william3_digest, AuthorisationToken, Entry,
};
use willow25::authorisation::AuthorisedEntry;
use willow25::entry::{Entrylike, SubspaceSignature};

pub const BUNDLE_MAGIC: &[u8; 6] = b"RIOTE1";
pub const BUNDLE_CODEC_ID: &str = "org.riot.evidence-bundle/1";

/// Ceilings from fixtures/manifest.json.
pub const MAX_BUNDLE_BYTES: usize = 8_388_608;
pub const MAX_BUNDLE_ENTRIES: usize = 64;
pub const MAX_ITEM_PAYLOAD_BYTES: usize = 1_048_576;
pub const MAX_AUTH_BYTES_PER_ENTRY: usize = 65_536;
pub const MAX_AUTH_BYTES_PER_BUNDLE: usize = 2_097_152;
const SIGNATURE_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    WrongMagic,
    UnsupportedCodec,
    TooManyEntries,
    BundleTooLarge,
    PayloadTooLarge,
    AuthorizationTooLarge,
    PayloadDigestMismatch,
    PayloadLengthMismatch,
    DoesNotAuthorise,
    TrailingBytes,
    Malformed,
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BundleError {}

/// One bundle item: canonical Willow entry bytes, canonical capability
/// bytes, the 64-byte subspace signature, and the exact payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleItem {
    entry_bytes: Vec<u8>,
    capability_bytes: Vec<u8>,
    signature_bytes: Vec<u8>,
    payload: Vec<u8>,
}

impl BundleItem {
    /// Builds an item from a verified authorised entry plus the payload
    /// whose WILLIAM3 digest and length the entry commits to.
    pub fn new(authorised: AuthorisedEntry, payload: Vec<u8>) -> Result<Self, BundleError> {
        let entry = authorised.entry();
        if entry.payload_length() != payload.len() as u64 {
            return Err(BundleError::PayloadLengthMismatch);
        }
        if *entry.payload_digest().as_bytes() != william3_digest(&payload) {
            return Err(BundleError::PayloadDigestMismatch);
        }
        let token = authorised.authorisation_token();
        let signature: ed25519_dalek::Signature = token.signature().clone().into();
        Ok(Self {
            entry_bytes: encode_entry(entry),
            capability_bytes: encode_capability(token.capability()),
            signature_bytes: signature.to_bytes().to_vec(),
            payload,
        })
    }

    /// Raw framing constructor for tests and hostile-fixture generation.
    /// Performs no validation — the decoder must catch everything.
    pub fn from_raw_parts(
        entry_bytes: Vec<u8>,
        capability_bytes: Vec<u8>,
        signature_bytes: Vec<u8>,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            entry_bytes,
            capability_bytes,
            signature_bytes,
            payload,
        }
    }

    pub fn entry_bytes(&self) -> &[u8] {
        &self.entry_bytes
    }

    pub fn capability_bytes(&self) -> &[u8] {
        &self.capability_bytes
    }

    pub fn signature_bytes(&self) -> &[u8] {
        &self.signature_bytes
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Decodes and fully re-verifies this item, returning the entry.
    fn verify(&self) -> Result<Entry, BundleError> {
        if self.payload.len() > MAX_ITEM_PAYLOAD_BYTES {
            return Err(BundleError::PayloadTooLarge);
        }
        if self.capability_bytes.len() + self.signature_bytes.len() > MAX_AUTH_BYTES_PER_ENTRY {
            return Err(BundleError::AuthorizationTooLarge);
        }
        let entry = decode_entry_canonic(&self.entry_bytes).map_err(|_| BundleError::Malformed)?;
        let capability = decode_capability_canonic(&self.capability_bytes)
            .map_err(|_| BundleError::Malformed)?;
        let sig_array: [u8; SIGNATURE_BYTES] = self
            .signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| BundleError::Malformed)?;
        let signature = SubspaceSignature::from(sig_array);

        if entry.payload_length() != self.payload.len() as u64 {
            return Err(BundleError::PayloadLengthMismatch);
        }
        if *entry.payload_digest().as_bytes() != william3_digest(&self.payload) {
            return Err(BundleError::PayloadDigestMismatch);
        }

        let token = AuthorisationToken::new(capability, signature);
        if !verify_entry(&entry, &token) {
            return Err(BundleError::DoesNotAuthorise);
        }
        Ok(entry)
    }
}

/// Validates and encodes a bundle. Every item is re-verified before encoding
/// so Riot never exports bytes it would itself reject.
pub fn encode_bundle(items: &[BundleItem]) -> Result<Vec<u8>, BundleError> {
    if items.len() > MAX_BUNDLE_ENTRIES {
        return Err(BundleError::TooManyEntries);
    }
    let auth_total: usize = items
        .iter()
        .map(|i| i.capability_bytes.len() + i.signature_bytes.len())
        .sum();
    if auth_total > MAX_AUTH_BYTES_PER_BUNDLE {
        return Err(BundleError::AuthorizationTooLarge);
    }
    for item in items {
        item.verify()?;
    }
    let bytes = encode_bundle_raw(items);
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(BundleError::BundleTooLarge);
    }
    Ok(bytes)
}

/// Raw deterministic framing without validation. Public so hostile fixtures
/// can produce structurally valid but cryptographically invalid bundles.
pub fn encode_bundle_raw(items: &[BundleItem]) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    buffer.extend_from_slice(BUNDLE_MAGIC);
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(2)?;
        e.u8(0)?.str(BUNDLE_CODEC_ID)?;
        e.u8(1)?.array(items.len() as u64)?;
        for item in items {
            e.map(4)?;
            e.u8(0)?.bytes(&item.entry_bytes)?;
            e.u8(1)?.bytes(&item.capability_bytes)?;
            e.u8(2)?.bytes(&item.signature_bytes)?;
            e.u8(3)?.bytes(&item.payload)?;
        }
        Ok(())
    })();
    debug_assert!(r.is_ok());
    buffer
}

/// Strict bounded decode with full cryptographic re-verification.
pub fn decode_bundle(input: &[u8]) -> Result<Vec<BundleItem>, BundleError> {
    if input.len() > MAX_BUNDLE_BYTES {
        return Err(BundleError::BundleTooLarge);
    }
    if input.len() < BUNDLE_MAGIC.len() || &input[..BUNDLE_MAGIC.len()] != BUNDLE_MAGIC {
        return Err(BundleError::WrongMagic);
    }
    let body = &input[BUNDLE_MAGIC.len()..];
    let mut d = Decoder::new(body);

    let pairs = d
        .map()
        .map_err(|_| BundleError::Malformed)?
        .ok_or(BundleError::Malformed)?;
    if pairs != 2 {
        return Err(BundleError::Malformed);
    }
    if d.u8().map_err(|_| BundleError::Malformed)? != 0 {
        return Err(BundleError::Malformed);
    }
    let codec = d.str().map_err(|_| BundleError::Malformed)?;
    if codec != BUNDLE_CODEC_ID {
        return Err(BundleError::UnsupportedCodec);
    }
    if d.u8().map_err(|_| BundleError::Malformed)? != 1 {
        return Err(BundleError::Malformed);
    }
    let count = d
        .array()
        .map_err(|_| BundleError::Malformed)?
        .ok_or(BundleError::Malformed)?;
    if count as usize > MAX_BUNDLE_ENTRIES {
        return Err(BundleError::TooManyEntries);
    }

    let mut auth_total: usize = 0;
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let inner = d
            .map()
            .map_err(|_| BundleError::Malformed)?
            .ok_or(BundleError::Malformed)?;
        if inner != 4 {
            return Err(BundleError::Malformed);
        }
        let item = BundleItem {
            entry_bytes: decode_field_bytes(&mut d, 0, MAX_AUTH_BYTES_PER_ENTRY)?,
            capability_bytes: decode_field_bytes(&mut d, 1, MAX_AUTH_BYTES_PER_ENTRY)?,
            signature_bytes: decode_field_bytes(&mut d, 2, SIGNATURE_BYTES)?,
            payload: decode_field_bytes(&mut d, 3, MAX_ITEM_PAYLOAD_BYTES)?,
        };
        if item.signature_bytes.len() != SIGNATURE_BYTES {
            return Err(BundleError::Malformed);
        }
        auth_total += item.capability_bytes.len() + item.signature_bytes.len();
        if auth_total > MAX_AUTH_BYTES_PER_BUNDLE {
            return Err(BundleError::AuthorizationTooLarge);
        }
        items.push(item);
    }

    if d.position() != body.len() {
        return Err(BundleError::TrailingBytes);
    }

    for item in &items {
        item.verify()?;
    }
    Ok(items)
}

fn decode_field_bytes(
    d: &mut Decoder<'_>,
    expected_key: u8,
    max: usize,
) -> Result<Vec<u8>, BundleError> {
    if d.u8().map_err(|_| BundleError::Malformed)? != expected_key {
        return Err(BundleError::Malformed);
    }
    let bytes = d.bytes().map_err(|_| BundleError::Malformed)?;
    if bytes.len() > max {
        return Err(BundleError::Malformed);
    }
    Ok(bytes.to_vec())
}

/// SHA-256 of the complete artifact bytes.
pub fn bundle_digest(bundle_bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bundle_bytes).into()
}

/// Domain-separated SHA-256 binding entry, capability, and signature bytes
/// so concatenation is unambiguous.
pub fn entry_digest(
    entry_bytes: &[u8],
    capability_bytes: &[u8],
    signature_bytes: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"riot/entry-digest/v1");
    hasher.update((entry_bytes.len() as u32).to_be_bytes());
    hasher.update(entry_bytes);
    hasher.update((capability_bytes.len() as u32).to_be_bytes());
    hasher.update(capability_bytes);
    hasher.update(signature_bytes);
    hasher.finalize().into()
}

/// SHA-256 of the deterministic alert payload bytes (local artifact tooling).
pub fn object_digest(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).into()
}
