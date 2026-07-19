//! WU-003B canonical authority records: tickets, listings, and delegate grants.
//!
//! Every type here is a positional-CBOR [`CanonicalRecord`] following the WU-002
//! conventions already implemented in [`crate::codec`]: definite arrays, minimal
//! integers, `snake_case` textual discriminants, `null`-or-value optionals,
//! sorted-canonical sets, and `_bytes` fields carrying separately-canonical byte
//! strings. Canonicality is enforced for free by
//! [`crate::codec::decode_canonical`] (bound → decode → reject trailing →
//! byte-identical re-encode).
//!
//! Layout follows `docs/research/2026-07-18-wu003b-record-layout-proposal.md`
//! with the three OWNER-LOCKED wire decisions applied:
//!
//! 1. [`PublicSiteTicketV2Core`] carries **no** leading schema/version field —
//!    version lives in the envelope tag `2` + signing domain
//!    `riot/public-site-ticket/v2`.
//! 2. [`CommunityListingV1`] embeds the ticket as the SIGNED-CORE ENVELOPE bytes
//!    (`ticket_core_bytes` = canonical [`RootSignedTicketCoreEnvelopeV2`] bytes),
//!    so a reader can independently re-verify the `O`-root signature from the
//!    listing alone.
//! 3. `topic_tags` and `languages` are SORTED CANONICAL SETS (dedup + ascending
//!    canonical-byte order); a reorder or duplicate is non-canonical and rejected.

use minicbor::{Decoder, Encoder};

use crate::codec::{
    assert_set_order, decode_canonical, definite_array, expect_array, peek_null, read_bytes_max,
    read_discriminant, read_fixed_bytes, read_null, read_text_max, read_version,
    sort_canonical_set, CanonicalRecord, CodecError,
};
use crate::digest::{digest_v1, label, work_proof};
use ed25519_dalek::{Signature, VerifyingKey};
use minicbor::data::Type;

// ---------------------------------------------------------------------------
// Bounds (design "codec maxima", lines 2405-2415)
// ---------------------------------------------------------------------------

/// `PublicSiteTicketV2Core` canonical bytes must not exceed this ceiling.
pub const MAX_TICKET_CORE_BYTES: usize = 768;
/// Listing entry plus capability and optional delegate grant.
pub const MAX_LISTING_ENVELOPE_BYTES: usize = 16_384;
/// Largest accepted `ListingDelegateGrantV1` canonical encoding.
pub const MAX_DELEGATE_GRANT_BYTES: usize = 512;

/// Frozen `CommunityListingV1` schema discriminant.
pub const COMMUNITY_LISTING_SCHEMA: &str = "riot/community-listing/1";

/// Listing presentation field caps.
const MAX_TITLE_BYTES: usize = 120;
const MAX_SUMMARY_BYTES: usize = 512;
const MAX_TOPIC_TAGS: usize = 8;
const MAX_TAG_BYTES: usize = 32;
const MAX_LANGUAGES: usize = 8;
const MAX_LANGUAGE_BYTES: usize = 35;
const MAX_REGION_BYTES: usize = 16;
const TRANSPORT_TOKEN_MAX: usize = 16;

/// Ticket-core signing domain (bare label prefix, NOT `digest_v1` framing):
/// `Sign("riot/public-site-ticket/v2" || canonical_cbor(core))`.
pub const PUBLIC_SITE_TICKET_SIGNING_DOMAIN: &[u8] = b"riot/public-site-ticket/v2";
/// Delegate-grant signing domain: `Sign("riot/listing-delegate-grant/v1" || core)`.
pub const LISTING_DELEGATE_GRANT_SIGNING_DOMAIN: &[u8] = b"riot/listing-delegate-grant/v1";
/// `digest_v1` label for the terminal Meadowcap capability digest bound into a
/// delegate grant. **INVENTED** here — the design specifies `digest_v1` over the
/// terminal capability's canonical bytes but does not fix the label; this is the
/// proposed constant and should be reviewed alongside the wire freeze.
pub const TERMINAL_CAPABILITY_DIGEST_LABEL: &[u8] = b"riot/listing-terminal-capability/v1";

// ---------------------------------------------------------------------------
// TransportFloor — closed, ORDERED enum (require_none < require_arti)
// ---------------------------------------------------------------------------

/// The mandatory transport floor token. CLOSED, ORDERED enum: `require_none` is
/// strictly less than `require_arti`. Derived `Ord` follows declaration order,
/// so `RequireNone < RequireArti` holds. An unknown token fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransportFloor {
    /// `require_none` — no confidential-transport requirement (MVP profile).
    RequireNone,
    /// `require_arti` — Arti/Tor required. Not admitted by the MVP.
    RequireArti,
}

impl TransportFloor {
    /// The exact `snake_case` wire token.
    pub fn token(self) -> &'static str {
        match self {
            TransportFloor::RequireNone => "require_none",
            TransportFloor::RequireArti => "require_arti",
        }
    }

    /// Parse a wire token, or `None` for any unrecognized string.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "require_none" => Some(TransportFloor::RequireNone),
            "require_arti" => Some(TransportFloor::RequireArti),
            _ => None,
        }
    }

    fn encode(self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), CodecError> {
        e.str(self.token()).map_err(|_| CodecError::Malformed)?;
        Ok(())
    }

    fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let token = read_discriminant(d, TRANSPORT_TOKEN_MAX)?;
        TransportFloor::from_token(&token).ok_or(CodecError::UnknownVariant)
    }
}

// ---------------------------------------------------------------------------
// PublicSiteTicketV2Core — OWNER-LOCKED decision 1: implicit schema/version
// ---------------------------------------------------------------------------

/// The site-wide, root-signed ticket core. 12 positional fields, NO leading
/// version field (version is carried by the envelope tag / signing domain).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicSiteTicketV2Core {
    /// Full `O` root key (32-byte Ed25519 public key; also the ticket signer).
    pub root_id: [u8; 32],
    /// `O` (owned masthead) namespace id.
    pub o_namespace_id: [u8; 32],
    /// `C` (communal comments) namespace id.
    pub c_namespace_id: [u8; 32],
    /// `W` (communal wire) namespace id.
    pub w_namespace_id: [u8; 32],
    /// Digest of the bound site manifest.
    pub manifest_digest: [u8; 32],
    /// Bound manifest version.
    pub manifest_version: u64,
    /// Minimum sync version; MUST be `2` to admit.
    pub min_sync_version: u64,
    /// Transport the manifest requires.
    pub manifest_required_transport: TransportFloor,
    /// Transport floor the ticket asserts (`>= manifest_required_transport`).
    pub transport_floor: TransportFloor,
    /// Monotonic per-root transport epoch; older epochs are rejected.
    pub transport_epoch: u32,
    /// Issue time (Unix seconds).
    pub issued_unix_seconds: u64,
    /// Expiry (Unix seconds); inclusive — `now >= expiry` is expired.
    pub expiry_unix_seconds: u64,
}

impl CanonicalRecord for PublicSiteTicketV2Core {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(12).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.root_id).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.o_namespace_id)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.c_namespace_id)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.w_namespace_id)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.manifest_digest)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.manifest_version)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.min_sync_version)
                .map_err(|_| CodecError::Malformed)?;
            self.manifest_required_transport.encode(&mut e)?;
            self.transport_floor.encode(&mut e)?;
            e.u32(self.transport_epoch)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.issued_unix_seconds)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.expiry_unix_seconds)
                .map_err(|_| CodecError::Malformed)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 12)?;
        let root_id = read_fixed_bytes::<32>(d)?;
        let o_namespace_id = read_fixed_bytes::<32>(d)?;
        let c_namespace_id = read_fixed_bytes::<32>(d)?;
        let w_namespace_id = read_fixed_bytes::<32>(d)?;
        let manifest_digest = read_fixed_bytes::<32>(d)?;
        let manifest_version = d.u64().map_err(|_| CodecError::Malformed)?;
        let min_sync_version = d.u64().map_err(|_| CodecError::Malformed)?;
        let manifest_required_transport = TransportFloor::decode(d)?;
        let transport_floor = TransportFloor::decode(d)?;
        let transport_epoch = d.u32().map_err(|_| CodecError::Malformed)?;
        let issued_unix_seconds = d.u64().map_err(|_| CodecError::Malformed)?;
        let expiry_unix_seconds = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(PublicSiteTicketV2Core {
            root_id,
            o_namespace_id,
            c_namespace_id,
            w_namespace_id,
            manifest_digest,
            manifest_version,
            min_sync_version,
            manifest_required_transport,
            transport_floor,
            transport_epoch,
            issued_unix_seconds,
            expiry_unix_seconds,
        })
    }
}

// ---------------------------------------------------------------------------
// RootSignedTicketCoreEnvelopeV2 = [2, PublicSiteTicketV2Core, bstr .size 64]
// ---------------------------------------------------------------------------

/// The `O`-root-signed ticket-core envelope. The core is embedded as a CBOR
/// value (nested array), not double-encoded bytes; the signature is a 64-byte
/// byte string over `SIGNING_DOMAIN || canonical_cbor(core)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSignedTicketCoreEnvelopeV2 {
    /// The signed ticket core.
    pub core: PublicSiteTicketV2Core,
    /// The 64-byte Ed25519 root signature.
    pub root_signature: [u8; 64],
}

impl RootSignedTicketCoreEnvelopeV2 {
    /// Bare-label signing domain for the root signature.
    pub const SIGNING_DOMAIN: &'static [u8] = PUBLIC_SITE_TICKET_SIGNING_DOMAIN;

    /// The exact preimage the `O` root signs: `SIGNING_DOMAIN || core_canonical`.
    pub fn signing_preimage(&self) -> Result<Vec<u8>, CodecError> {
        let mut preimage = Self::SIGNING_DOMAIN.to_vec();
        preimage.extend_from_slice(&self.core.encode_canonical()?);
        Ok(preimage)
    }

    /// `root_signed_ticket_core_digest = digest_v1(label, canonical(envelope))`.
    pub fn root_signed_ticket_core_digest(&self) -> Result<[u8; 32], CodecError> {
        Ok(digest_v1(
            label::PUBLIC_SITE_TICKET_SIGNED_CORE,
            &self.encode_canonical()?,
        ))
    }
}

impl CanonicalRecord for RootSignedTicketCoreEnvelopeV2 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(3).map_err(|_| CodecError::Malformed)?;
            e.u64(2).map_err(|_| CodecError::Malformed)?;
        }
        // Embed the core as a CBOR value (its own array), appended canonically.
        buf.extend_from_slice(&self.core.encode_canonical()?);
        {
            let mut e = Encoder::new(&mut buf);
            e.bytes(&self.root_signature)
                .map_err(|_| CodecError::Malformed)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 3)?;
        let tag = d.u64().map_err(|_| CodecError::Malformed)?;
        if tag != 2 {
            return Err(CodecError::UnknownVersion(tag));
        }
        let core = PublicSiteTicketV2Core::decode_fields(d)?;
        let root_signature = read_fixed_bytes::<64>(d)?;
        Ok(RootSignedTicketCoreEnvelopeV2 {
            core,
            root_signature,
        })
    }
}

// ---------------------------------------------------------------------------
// Sorted-canonical set helpers (byte strings and text strings)
// ---------------------------------------------------------------------------

fn encode_bytes_set(buf: &mut Vec<u8>, elements: &[Vec<u8>]) -> Result<(), CodecError> {
    let mut encoded: Vec<Vec<u8>> = Vec::with_capacity(elements.len());
    for el in elements {
        let mut item = Vec::new();
        Encoder::new(&mut item)
            .bytes(el)
            .map_err(|_| CodecError::Malformed)?;
        encoded.push(item);
    }
    let sorted = sort_canonical_set(encoded)?;
    {
        let mut e = Encoder::new(&mut *buf);
        e.array(sorted.len() as u64)
            .map_err(|_| CodecError::Malformed)?;
    }
    for item in &sorted {
        buf.extend_from_slice(item);
    }
    Ok(())
}

fn decode_bytes_set(
    d: &mut Decoder<'_>,
    max_len: usize,
    max_elem_bytes: usize,
) -> Result<Vec<Vec<u8>>, CodecError> {
    let count = definite_array(d)?;
    if count as usize > max_len {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(count as usize);
    let mut previous: Option<Vec<u8>> = None;
    for _ in 0..count {
        let value = read_bytes_max(d, max_elem_bytes)?;
        let mut canonical = Vec::new();
        Encoder::new(&mut canonical)
            .bytes(&value)
            .map_err(|_| CodecError::Malformed)?;
        assert_set_order(previous.as_deref(), &canonical)?;
        previous = Some(canonical);
        out.push(value);
    }
    Ok(out)
}

fn encode_text_set(buf: &mut Vec<u8>, elements: &[String]) -> Result<(), CodecError> {
    let mut encoded: Vec<Vec<u8>> = Vec::with_capacity(elements.len());
    for el in elements {
        let mut item = Vec::new();
        Encoder::new(&mut item)
            .str(el)
            .map_err(|_| CodecError::Malformed)?;
        encoded.push(item);
    }
    let sorted = sort_canonical_set(encoded)?;
    {
        let mut e = Encoder::new(&mut *buf);
        e.array(sorted.len() as u64)
            .map_err(|_| CodecError::Malformed)?;
    }
    for item in &sorted {
        buf.extend_from_slice(item);
    }
    Ok(())
}

fn decode_text_set(
    d: &mut Decoder<'_>,
    max_len: usize,
    max_elem_bytes: usize,
) -> Result<Vec<String>, CodecError> {
    let count = definite_array(d)?;
    if count as usize > max_len {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut out: Vec<String> = Vec::with_capacity(count as usize);
    let mut previous: Option<Vec<u8>> = None;
    for _ in 0..count {
        let value = read_text_max(d, 0, max_elem_bytes)?;
        let mut canonical = Vec::new();
        Encoder::new(&mut canonical)
            .str(&value)
            .map_err(|_| CodecError::Malformed)?;
        assert_set_order(previous.as_deref(), &canonical)?;
        previous = Some(canonical);
        out.push(value);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// CommunityListingV1 — explicit schema tstr; embedded ticket-core envelope bytes
// ---------------------------------------------------------------------------

/// The directory listing payload written at `O:/directory/listing`. 18 positional
/// fields led by an explicit schema string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityListingV1 {
    /// Full `O` root key.
    pub root_id: [u8; 32],
    /// `O` namespace id.
    pub o_namespace_id: [u8; 32],
    /// `C` namespace id.
    pub c_namespace_id: [u8; 32],
    /// `W` namespace id.
    pub w_namespace_id: [u8; 32],
    /// Bound manifest digest.
    pub manifest_digest: [u8; 32],
    /// Bound manifest version.
    pub manifest_version: u64,
    /// Canonical [`RootSignedTicketCoreEnvelopeV2`] bytes (OWNER-LOCKED decision
    /// 2): carries a verifiable `O`-root signature independent of the entry.
    pub ticket_core_bytes: Vec<u8>,
    /// Listing epoch.
    pub listing_epoch: u32,
    /// Listing revision.
    pub listing_revision: u32,
    /// `false` is an explicit unlisting tombstone.
    pub listed: bool,
    /// Title (<= 120 UTF-8 bytes).
    pub title: String,
    /// Summary (<= 512 UTF-8 bytes).
    pub summary: String,
    /// Topic tags — SORTED CANONICAL SET, <= 8 entries, each <= 32 bytes.
    pub topic_tags: Vec<Vec<u8>>,
    /// Languages — SORTED CANONICAL SET, <= 8 BCP-47 tags, each <= 35 bytes.
    pub languages: Vec<String>,
    /// Optional coarse region (<= 16 bytes).
    pub region: Option<Vec<u8>>,
    /// Issue time (Unix seconds).
    pub issued_unix_seconds: u64,
    /// Expiry (Unix seconds); inclusive.
    pub expiry_unix_seconds: u64,
}

impl CommunityListingV1 {
    /// The frozen schema discriminant this record carries.
    pub fn schema(&self) -> &'static str {
        COMMUNITY_LISTING_SCHEMA
    }
}

impl CanonicalRecord for CommunityListingV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        if self.title.len() > MAX_TITLE_BYTES
            || self.summary.len() > MAX_SUMMARY_BYTES
            || self.topic_tags.len() > MAX_TOPIC_TAGS
            || self.languages.len() > MAX_LANGUAGES
            || self
                .region
                .as_ref()
                .is_some_and(|r| r.len() > MAX_REGION_BYTES)
        {
            return Err(CodecError::LengthOutOfRange);
        }
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(18).map_err(|_| CodecError::Malformed)?;
            e.str(COMMUNITY_LISTING_SCHEMA)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.root_id).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.o_namespace_id)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.c_namespace_id)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.w_namespace_id)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.manifest_digest)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.manifest_version)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.ticket_core_bytes)
                .map_err(|_| CodecError::Malformed)?;
            e.u32(self.listing_epoch)
                .map_err(|_| CodecError::Malformed)?;
            e.u32(self.listing_revision)
                .map_err(|_| CodecError::Malformed)?;
            e.bool(self.listed).map_err(|_| CodecError::Malformed)?;
            e.str(&self.title).map_err(|_| CodecError::Malformed)?;
            e.str(&self.summary).map_err(|_| CodecError::Malformed)?;
        }
        encode_bytes_set(&mut buf, &self.topic_tags)?;
        encode_text_set(&mut buf, &self.languages)?;
        {
            let mut e = Encoder::new(&mut buf);
            match &self.region {
                Some(region) => {
                    e.bytes(region).map_err(|_| CodecError::Malformed)?;
                }
                None => {
                    e.null().map_err(|_| CodecError::Malformed)?;
                }
            }
            e.u64(self.issued_unix_seconds)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.expiry_unix_seconds)
                .map_err(|_| CodecError::Malformed)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 18)?;
        let schema = read_text_max(d, 1, 64)?;
        if schema != COMMUNITY_LISTING_SCHEMA {
            return Err(CodecError::UnknownVariant);
        }
        let root_id = read_fixed_bytes::<32>(d)?;
        let o_namespace_id = read_fixed_bytes::<32>(d)?;
        let c_namespace_id = read_fixed_bytes::<32>(d)?;
        let w_namespace_id = read_fixed_bytes::<32>(d)?;
        let manifest_digest = read_fixed_bytes::<32>(d)?;
        let manifest_version = d.u64().map_err(|_| CodecError::Malformed)?;
        let ticket_core_bytes = read_bytes_max(d, MAX_TICKET_CORE_BYTES + 128)?;
        let listing_epoch = d.u32().map_err(|_| CodecError::Malformed)?;
        let listing_revision = d.u32().map_err(|_| CodecError::Malformed)?;
        let listed = d.bool().map_err(|_| CodecError::Malformed)?;
        let title = read_text_max(d, 0, MAX_TITLE_BYTES)?;
        let summary = read_text_max(d, 0, MAX_SUMMARY_BYTES)?;
        let topic_tags = decode_bytes_set(d, MAX_TOPIC_TAGS, MAX_TAG_BYTES)?;
        let languages = decode_text_set(d, MAX_LANGUAGES, MAX_LANGUAGE_BYTES)?;
        let region = if peek_null(d)? {
            read_null(d)?;
            None
        } else {
            Some(read_bytes_max(d, MAX_REGION_BYTES)?)
        };
        let issued_unix_seconds = d.u64().map_err(|_| CodecError::Malformed)?;
        let expiry_unix_seconds = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(CommunityListingV1 {
            root_id,
            o_namespace_id,
            c_namespace_id,
            w_namespace_id,
            manifest_digest,
            manifest_version,
            ticket_core_bytes,
            listing_epoch,
            listing_revision,
            listed,
            title,
            summary,
            topic_tags,
            languages,
            region,
            issued_unix_seconds,
            expiry_unix_seconds,
        })
    }
}

// ---------------------------------------------------------------------------
// AdmittedListingEnvelopeV1 = [1, entry_bytes, capability_chain_bytes, null/grant]
// ---------------------------------------------------------------------------

/// The admitted-listing envelope. All three payloads are opaque byte strings at
/// this layer; `signed_listing_entry_bytes` carries the canonical listing entry
/// whose payload is a [`CommunityListingV1`], `capability_chain_bytes` the
/// Meadowcap chain, and the optional grant a canonical [`ListingDelegateGrantV1`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedListingEnvelopeV1 {
    /// Canonical signed listing entry bytes (payload = [`CommunityListingV1`]).
    pub signed_listing_entry_bytes: Vec<u8>,
    /// Canonical Meadowcap capability chain bytes.
    pub capability_chain_bytes: Vec<u8>,
    /// `None` = root-owned (zero delegation); `Some` = delegated, carrying the
    /// canonical [`ListingDelegateGrantV1`] bytes.
    pub delegate_grant_bytes: Option<Vec<u8>>,
}

impl AdmittedListingEnvelopeV1 {
    /// `listing_digest = digest_v1(label, canonical(envelope))`.
    pub fn listing_digest(&self) -> Result<[u8; 32], CodecError> {
        Ok(digest_v1(
            label::ADMITTED_LISTING_ENVELOPE,
            &self.encode_canonical()?,
        ))
    }
}

impl CanonicalRecord for AdmittedListingEnvelopeV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(4).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.signed_listing_entry_bytes)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.capability_chain_bytes)
                .map_err(|_| CodecError::Malformed)?;
            match &self.delegate_grant_bytes {
                Some(grant) => {
                    e.bytes(grant).map_err(|_| CodecError::Malformed)?;
                }
                None => {
                    e.null().map_err(|_| CodecError::Malformed)?;
                }
            }
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 4)?;
        let tag = d.u64().map_err(|_| CodecError::Malformed)?;
        if tag != 1 {
            return Err(CodecError::UnknownVersion(tag));
        }
        let signed_listing_entry_bytes = read_bytes_max(d, MAX_LISTING_ENVELOPE_BYTES)?;
        let capability_chain_bytes = read_bytes_max(d, MAX_LISTING_ENVELOPE_BYTES)?;
        let delegate_grant_bytes = if peek_null(d)? {
            read_null(d)?;
            None
        } else {
            Some(read_bytes_max(d, MAX_DELEGATE_GRANT_BYTES)?)
        };
        Ok(AdmittedListingEnvelopeV1 {
            signed_listing_entry_bytes,
            capability_chain_bytes,
            delegate_grant_bytes,
        })
    }
}

// ---------------------------------------------------------------------------
// ListingDelegateGrantV1 — root-signed, implicit version (signing domain carries it)
// ---------------------------------------------------------------------------

/// A root-signed grant binding a delegate key to exactly one listing epoch. The
/// wire body is the 6 positional fields below; the signature travels separately
/// and is verified via [`signing_preimage`](ListingDelegateGrantV1::signing_preimage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingDelegateGrantV1 {
    /// The `O` root key (grant signer).
    pub root_id: [u8; 32],
    /// The delegated listing-key subspace.
    pub delegate_key: [u8; 32],
    /// `digest_v1(TERMINAL_CAPABILITY_DIGEST_LABEL, terminal_capability_canonical)`.
    pub terminal_capability_digest: [u8; 32],
    /// The single epoch this grant authorizes.
    pub listing_epoch: u32,
    /// Issue time (Unix seconds).
    pub issued_unix_seconds: u64,
    /// Expiry (Unix seconds); cannot outlive the Meadowcap time range.
    pub expiry_unix_seconds: u64,
}

impl ListingDelegateGrantV1 {
    /// The exact preimage the `O` root signs:
    /// `LISTING_DELEGATE_GRANT_SIGNING_DOMAIN || canonical_cbor(body)`.
    pub fn signing_preimage(&self) -> Result<Vec<u8>, CodecError> {
        let mut preimage = LISTING_DELEGATE_GRANT_SIGNING_DOMAIN.to_vec();
        preimage.extend_from_slice(&self.encode_canonical()?);
        Ok(preimage)
    }
}

impl CanonicalRecord for ListingDelegateGrantV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(6).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.root_id).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.delegate_key)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.terminal_capability_digest)
                .map_err(|_| CodecError::Malformed)?;
            e.u32(self.listing_epoch)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.issued_unix_seconds)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.expiry_unix_seconds)
                .map_err(|_| CodecError::Malformed)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 6)?;
        let root_id = read_fixed_bytes::<32>(d)?;
        let delegate_key = read_fixed_bytes::<32>(d)?;
        let terminal_capability_digest = read_fixed_bytes::<32>(d)?;
        let listing_epoch = d.u32().map_err(|_| CodecError::Malformed)?;
        let issued_unix_seconds = d.u64().map_err(|_| CodecError::Malformed)?;
        let expiry_unix_seconds = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(ListingDelegateGrantV1 {
            root_id,
            delegate_key,
            terminal_capability_digest,
            listing_epoch,
            issued_unix_seconds,
            expiry_unix_seconds,
        })
    }
}

/// `terminal_capability_digest_bytes = digest_v1(TERMINAL_CAPABILITY_DIGEST_LABEL,
/// terminal_capability_canonical)`. The preimage is the terminal Meadowcap
/// capability's own canonical byte encoding; the label is documented as INVENTED
/// on [`TERMINAL_CAPABILITY_DIGEST_LABEL`].
pub fn terminal_capability_digest(terminal_capability_canonical: &[u8]) -> [u8; 32] {
    digest_v1(
        TERMINAL_CAPABILITY_DIGEST_LABEL,
        terminal_capability_canonical,
    )
}

/// Decode a canonical [`CommunityListingV1`] from an admitted entry's payload
/// bytes, bounded by [`MAX_LISTING_ENVELOPE_BYTES`].
pub(crate) fn decode_listing_payload(bytes: &[u8]) -> Result<CommunityListingV1, CodecError> {
    decode_canonical::<CommunityListingV1>(bytes, MAX_LISTING_ENVELOPE_BYTES)
}

/// Decode a canonical [`ListingDelegateGrantV1`], bounded by
/// [`MAX_DELEGATE_GRANT_BYTES`].
pub(crate) fn decode_delegate_grant(bytes: &[u8]) -> Result<ListingDelegateGrantV1, CodecError> {
    decode_canonical::<ListingDelegateGrantV1>(bytes, MAX_DELEGATE_GRANT_BYTES)
}

// ===========================================================================
// WU-004: the 82-limit registry (design "Encoded control-record profile" +
// the MVP resource table). limit_id is the numeric stable ID 1..=82 (the design
// says "strictly ascending order" over the numeric registry, and the closed
// textual-discriminant rule enumerates control ops / frames / states / reasons /
// modes but NOT limits, so limits use their numeric ID on the wire).
// ===========================================================================

/// `AnchorLimitProfileV1` canonical bytes must not exceed 8 KiB (design "Encoded
/// control-record profile", "Limit profile | 8 KiB").
pub const MAX_LIMIT_PROFILE_BYTES: usize = 8 * 1024;

macro_rules! anchor_limits {
    ($($variant:ident = $id:literal => $name:literal),+ $(,)?) => {
        /// The closed registry of the 82 anchor resource limits. Declaration order
        /// is the canonical ascending ID order `1..=82`; the derived [`Ord`] therefore
        /// matches wire ordering.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum AnchorLimitId {
            $(
                #[doc = $name]
                $variant,
            )+
        }

        impl AnchorLimitId {
            /// The stable numeric limit ID (`1..=82`).
            pub fn id(self) -> u64 {
                match self { $(AnchorLimitId::$variant => $id,)+ }
            }
            /// The exact `snake_case` registry name (diagnostic; not the wire form).
            pub fn name(self) -> &'static str {
                match self { $(AnchorLimitId::$variant => $name,)+ }
            }
            /// Parse a numeric ID, or `None` for anything outside `1..=82`.
            pub fn from_id(id: u64) -> Option<Self> {
                match id { $($id => Some(AnchorLimitId::$variant),)+ _ => None }
            }
        }

        /// Every limit ID in canonical ascending order.
        pub const ALL_LIMIT_IDS: &[AnchorLimitId] = &[ $(AnchorLimitId::$variant,)+ ];
    };
}

anchor_limits! {
    LogicalRetainedBytesWholeAnchor = 1 => "logical_retained_bytes_whole_anchor",
    PhysicalRetainedBytes = 2 => "physical_retained_bytes",
    OrdinarySqliteDatabaseIncludingWal = 3 => "ordinary_sqlite_database_including_wal",
    NonPayloadMetadataBytes = 4 => "non_payload_metadata_bytes",
    SqliteWalBytes = 5 => "sqlite_wal_bytes",
    EmergencyRemovalMetadataReserve = 6 => "emergency_removal_metadata_reserve",
    EmergencyRemovalWalFsyncReserve = 7 => "emergency_removal_wal_fsync_reserve",
    StagedBytes = 8 => "staged_bytes",
    LiveStagedOperations = 9 => "live_staged_operations",
    IdempotencyRows = 10 => "idempotency_rows",
    IdempotencyRowsPerSourcePer24h = 11 => "idempotency_rows_per_source_per_24h",
    ReservedRemovalIdempotencyResultRows = 12 => "reserved_removal_idempotency_result_rows",
    IncidentConflictRecords = 13 => "incident_conflict_records",
    ConflictProofsPerSiteSubject = 14 => "conflict_proofs_per_site_subject",
    HostedSites = 15 => "hosted_sites",
    LogicalBytesPerSite = 16 => "logical_bytes_per_site",
    LiveEntriesPerNamespace = 17 => "live_entries_per_namespace",
    ItemPayload = 18 => "item_payload",
    Bundle = 19 => "bundle",
    ConcurrentSyncControlSessions = 20 => "concurrent_sync_control_sessions",
    SessionsPerSource = 21 => "sessions_per_source",
    SessionsPerSite = 22 => "sessions_per_site",
    TcpListenBacklog = 23 => "tcp_listen_backlog",
    AcceptedPublicHttpsSockets = 24 => "accepted_public_https_sockets",
    PendingTlsHandshakes = 25 => "pending_tls_handshakes",
    TlsHandshakesPerSourcePerMinute = 26 => "tls_handshakes_per_source_per_minute",
    TlsHandshakesGloballyPerSecond = 27 => "tls_handshakes_globally_per_second",
    TlsClienthelloTotalHandshakeBytes = 28 => "tls_clienthello_total_handshake_bytes",
    TlsHandshakeCpuWallTime = 29 => "tls_handshake_cpu_wall_time",
    ActivePublicHttpsConnections = 30 => "active_public_https_connections",
    HttpRequestsPerKeepAliveConnection = 31 => "http_requests_per_keep_alive_connection",
    HttpIdleAbsoluteConnectionLifetime = 32 => "http_idle_absolute_connection_lifetime",
    HttpDecodedHeaderFieldsOneFieldLine = 33 => "http_decoded_header_fields_one_field_line",
    ConcurrentPublicHttpsHandlers = 34 => "concurrent_public_https_handlers",
    QueuedPublicHttpsHandlers = 35 => "queued_public_https_handlers",
    PublicHttpsRequestsPerSourcePerMinute = 36 => "public_https_requests_per_source_per_minute",
    PublicHttpsRequestsGloballyPerSecond = 37 => "public_https_requests_globally_per_second",
    ConcurrentPublicHttpDatabaseSnapshots = 38 => "concurrent_public_http_database_snapshots",
    PublicHttpDatabaseSnapshotsPerSource = 39 => "public_http_database_snapshots_per_source",
    PublicHttpQueryCpuWallTime = 40 => "public_http_query_cpu_wall_time",
    PublicApiResponseBytes = 41 => "public_api_response_bytes",
    OneStaticWebResponse = 42 => "one_static_web_response",
    SearchResultsPerPage = 43 => "search_results_per_page",
    SearchQueryUtf8Bytes = 44 => "search_query_utf8_bytes",
    DirectoryListings = 45 => "directory_listings",
    DirectoryFeedRecords = 46 => "directory_feed_records",
    VerificationQueueJobs = 47 => "verification_queue_jobs",
    VerificationCpuPerRequest = 48 => "verification_cpu_per_request",
    AggregateOutstandingVerificationCpuBudget = 49 => "aggregate_outstanding_verification_cpu_budget",
    ReservedOwnerRemovalVerificationPermits = 50 => "reserved_owner_removal_verification_permits",
    QueuedReservedRemovalJobs = 51 => "queued_reserved_removal_jobs",
    QueuedReservedRemovalCanonicalBytes = 52 => "queued_reserved_removal_canonical_bytes",
    ReservedValidRemovalDatabaseWriterPermits = 53 => "reserved_valid_removal_database_writer_permits",
    EmergencyCheckpointWorker = 54 => "emergency_checkpoint_worker",
    OwnerRemovalAttemptsPerSourcePerMinute = 55 => "owner_removal_attempts_per_source_per_minute",
    OwnerRemovalAttemptsGloballyPerSecond = 56 => "owner_removal_attempts_globally_per_second",
    WorkChallengeSignaturesPerSecond = 57 => "work_challenge_signatures_per_second",
    WorkChallengesPerSourcePerMinute = 58 => "work_challenges_per_source_per_minute",
    StaticProjectionBytes = 59 => "static_projection_bytes",
    RendererTemporaryFilesystem = 60 => "renderer_temporary_filesystem",
    RendererTemporaryFilesInodes = 61 => "renderer_temporary_files_inodes",
    ConcurrentRendererJobs = 62 => "concurrent_renderer_jobs",
    RendererCpuWallTimePerGeneration = 63 => "renderer_cpu_wall_time_per_generation",
    PublishedGenerationsPerSite = 64 => "published_generations_per_site",
    LocalOperationalLogBytesAllClasses = 65 => "local_operational_log_bytes_all_classes",
    DiagnosticLogBytes = 66 => "diagnostic_log_bytes",
    RotatedLocalLogFiles = 67 => "rotated_local_log_files",
    ConcurrentGossipSessionsPerPeer = 68 => "concurrent_gossip_sessions_per_peer",
    GossipTransferPerPeerPerHour = 69 => "gossip_transfer_per_peer_per_hour",
    PendingPublicIrohQuicHandshakes = 70 => "pending_public_iroh_quic_handshakes",
    PublicIrohQuicHandshakeWallTime = 71 => "public_iroh_quic_handshake_wall_time",
    ControlSyncFirstFrameWallTime = 72 => "control_sync_first_frame_wall_time",
    ControlFrameReadWriteWallTime = 73 => "control_frame_read_write_wall_time",
    SyncFrameReadWriteWallTime = 74 => "sync_frame_read_write_wall_time",
    ControlSyncProgressInterval = 75 => "control_sync_progress_interval",
    ControlSyncIdleAbsoluteSessionLifetime = 76 => "control_sync_idle_absolute_session_lifetime",
    SnapshotCursorLifetime = 77 => "snapshot_cursor_lifetime",
    PublicIrohHandshakesPerSourcePerMinute = 78 => "public_iroh_handshakes_per_source_per_minute",
    PublicIrohHandshakesGloballyPerSecond = 79 => "public_iroh_handshakes_globally_per_second",
    DirectRootPrefilterQueueJobs = 80 => "direct_root_prefilter_queue_jobs",
    DirectRootPrefilterQueueCanonicalBytes = 81 => "direct_root_prefilter_queue_canonical_bytes",
    DirectRootSignatureCpuWallTime = 82 => "direct_root_signature_cpu_wall_time",
}

/// A limit value: a scalar `u64` or a slash-compound `[first_u64, second_u64]`
/// (design: "A scalar value is `u64`; a slash-compound value is
/// `[first_u64, second_u64]`").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitValue {
    /// A single scalar quantity.
    Scalar(u64),
    /// A slash-compound `[first, second]` quantity.
    Compound(u64, u64),
}

impl LimitValue {
    pub(crate) fn encode(self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), CodecError> {
        match self {
            LimitValue::Scalar(v) => {
                e.u64(v).map_err(|_| CodecError::Malformed)?;
            }
            LimitValue::Compound(a, b) => {
                e.array(2).map_err(|_| CodecError::Malformed)?;
                e.u64(a).map_err(|_| CodecError::Malformed)?;
                e.u64(b).map_err(|_| CodecError::Malformed)?;
            }
        }
        Ok(())
    }

    pub(crate) fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match d.datatype().map_err(|_| CodecError::Malformed)? {
            Type::Array | Type::ArrayIndef => {
                expect_array(d, 2)?;
                let a = d.u64().map_err(|_| CodecError::Malformed)?;
                let b = d.u64().map_err(|_| CodecError::Malformed)?;
                Ok(LimitValue::Compound(a, b))
            }
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => Ok(LimitValue::Scalar(
                d.u64().map_err(|_| CodecError::Malformed)?,
            )),
            _ => Err(CodecError::UnexpectedType),
        }
    }
}

/// One profile row: `[limit_id, effective_value, absolute_value]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorLimitEntry {
    /// The limit this row constrains.
    pub id: AnchorLimitId,
    /// The operator-effective value (`<= absolute`; only lowerable).
    pub effective: LimitValue,
    /// The compiled absolute ceiling.
    pub absolute: LimitValue,
}

/// `AnchorLimitProfileV1 = [1, profile_epoch, [[limit_id, effective, absolute]...]]`
/// with all 82 IDs exactly once in strictly ascending order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorLimitProfileV1 {
    /// Incremented whenever any effective value changes.
    pub profile_epoch: u64,
    /// The 82 rows in ascending ID order.
    pub entries: Vec<AnchorLimitEntry>,
}

impl AnchorLimitProfileV1 {
    /// `limit_profile_digest = digest_v1("riot/anchor-limit-profile/v1", body)`.
    pub fn limit_profile_digest(&self) -> Result<[u8; 32], CodecError> {
        Ok(digest_v1(label::LIMIT_PROFILE, &self.encode_canonical()?))
    }

    /// The compiled MVP default profile: every ID's `(effective, absolute)` pair
    /// from the design resource table. Bytes are bytes, durations milliseconds,
    /// counts/rates their printed unit; `2 * L` (row 12) resolves against the
    /// hosted-sites limit (effective `2*10_000`, absolute `2*50_000`).
    pub fn mvp_defaults(profile_epoch: u64) -> Self {
        const G: u64 = 1024 * 1024 * 1024;
        const M: u64 = 1024 * 1024;
        const K: u64 = 1024;
        let s = |e: u64, a: u64| (LimitValue::Scalar(e), LimitValue::Scalar(a));
        let c = |e: (u64, u64), a: (u64, u64)| {
            (
                LimitValue::Compound(e.0, e.1),
                LimitValue::Compound(a.0, a.1),
            )
        };
        let rows: [(AnchorLimitId, (LimitValue, LimitValue)); 82] = [
            (
                AnchorLimitId::LogicalRetainedBytesWholeAnchor,
                s(20 * G, 100 * G),
            ),
            (AnchorLimitId::PhysicalRetainedBytes, s(20 * G, 100 * G)),
            (
                AnchorLimitId::OrdinarySqliteDatabaseIncludingWal,
                s(24 * G, 110 * G),
            ),
            (AnchorLimitId::NonPayloadMetadataBytes, s(2 * G, 8 * G)),
            (AnchorLimitId::SqliteWalBytes, s(256 * M, G)),
            (
                AnchorLimitId::EmergencyRemovalMetadataReserve,
                s(768 * M, 3 * G),
            ),
            (
                AnchorLimitId::EmergencyRemovalWalFsyncReserve,
                s(768 * M, 3 * G),
            ),
            (AnchorLimitId::StagedBytes, s(256 * M, G)),
            (AnchorLimitId::LiveStagedOperations, s(10_000, 50_000)),
            (AnchorLimitId::IdempotencyRows, s(100_000, 500_000)),
            (
                AnchorLimitId::IdempotencyRowsPerSourcePer24h,
                s(2_000, 10_000),
            ),
            (
                AnchorLimitId::ReservedRemovalIdempotencyResultRows,
                s(20_000, 100_000),
            ),
            (AnchorLimitId::IncidentConflictRecords, s(10_000, 50_000)),
            (AnchorLimitId::ConflictProofsPerSiteSubject, s(2, 4)),
            (AnchorLimitId::HostedSites, s(10_000, 50_000)),
            (AnchorLimitId::LogicalBytesPerSite, s(64 * M, 256 * M)),
            (AnchorLimitId::LiveEntriesPerNamespace, s(4_096, 16_384)),
            (AnchorLimitId::ItemPayload, s(M, M)),
            (AnchorLimitId::Bundle, c((8 * M, 64), (8 * M, 64))),
            (AnchorLimitId::ConcurrentSyncControlSessions, s(128, 512)),
            (AnchorLimitId::SessionsPerSource, s(4, 16)),
            (AnchorLimitId::SessionsPerSite, s(8, 32)),
            (AnchorLimitId::TcpListenBacklog, s(256, 1_024)),
            (AnchorLimitId::AcceptedPublicHttpsSockets, s(512, 2_048)),
            (AnchorLimitId::PendingTlsHandshakes, s(64, 256)),
            (AnchorLimitId::TlsHandshakesPerSourcePerMinute, s(30, 120)),
            (AnchorLimitId::TlsHandshakesGloballyPerSecond, s(200, 800)),
            (
                AnchorLimitId::TlsClienthelloTotalHandshakeBytes,
                c((16 * K, 64 * K), (16 * K, 64 * K)),
            ),
            (
                AnchorLimitId::TlsHandshakeCpuWallTime,
                c((100, 5_000), (500, 10_000)),
            ),
            (AnchorLimitId::ActivePublicHttpsConnections, s(256, 1_024)),
            (
                AnchorLimitId::HttpRequestsPerKeepAliveConnection,
                s(100, 1_000),
            ),
            (
                AnchorLimitId::HttpIdleAbsoluteConnectionLifetime,
                c((15_000, 300_000), (60_000, 1_800_000)),
            ),
            (
                AnchorLimitId::HttpDecodedHeaderFieldsOneFieldLine,
                c((64, 8 * K), (64, 8 * K)),
            ),
            (AnchorLimitId::ConcurrentPublicHttpsHandlers, s(128, 512)),
            (AnchorLimitId::QueuedPublicHttpsHandlers, s(128, 512)),
            (
                AnchorLimitId::PublicHttpsRequestsPerSourcePerMinute,
                s(120, 600),
            ),
            (
                AnchorLimitId::PublicHttpsRequestsGloballyPerSecond,
                s(500, 2_000),
            ),
            (
                AnchorLimitId::ConcurrentPublicHttpDatabaseSnapshots,
                s(32, 128),
            ),
            (AnchorLimitId::PublicHttpDatabaseSnapshotsPerSource, s(2, 8)),
            (
                AnchorLimitId::PublicHttpQueryCpuWallTime,
                c((250, 2_000), (1_000, 5_000)),
            ),
            (AnchorLimitId::PublicApiResponseBytes, s(M, 4 * M)),
            (AnchorLimitId::OneStaticWebResponse, s(2 * M, 8 * M)),
            (AnchorLimitId::SearchResultsPerPage, s(50, 100)),
            (AnchorLimitId::SearchQueryUtf8Bytes, s(128, 256)),
            (AnchorLimitId::DirectoryListings, s(10_000, 50_000)),
            (AnchorLimitId::DirectoryFeedRecords, s(100_000, 500_000)),
            (AnchorLimitId::VerificationQueueJobs, s(512, 2_048)),
            (AnchorLimitId::VerificationCpuPerRequest, s(500, 2_000)),
            (
                AnchorLimitId::AggregateOutstandingVerificationCpuBudget,
                s(16_000, 64_000),
            ),
            (
                AnchorLimitId::ReservedOwnerRemovalVerificationPermits,
                s(4, 4),
            ),
            (AnchorLimitId::QueuedReservedRemovalJobs, s(256, 1_024)),
            (
                AnchorLimitId::QueuedReservedRemovalCanonicalBytes,
                s(4 * M, 16 * M),
            ),
            (
                AnchorLimitId::ReservedValidRemovalDatabaseWriterPermits,
                s(2, 2),
            ),
            (AnchorLimitId::EmergencyCheckpointWorker, s(1, 1)),
            (
                AnchorLimitId::OwnerRemovalAttemptsPerSourcePerMinute,
                s(10, 40),
            ),
            (
                AnchorLimitId::OwnerRemovalAttemptsGloballyPerSecond,
                s(100, 400),
            ),
            (AnchorLimitId::WorkChallengeSignaturesPerSecond, s(100, 500)),
            (AnchorLimitId::WorkChallengesPerSourcePerMinute, s(30, 120)),
            (AnchorLimitId::StaticProjectionBytes, s(5 * G, 20 * G)),
            (AnchorLimitId::RendererTemporaryFilesystem, s(G, 4 * G)),
            (
                AnchorLimitId::RendererTemporaryFilesInodes,
                s(10_000, 50_000),
            ),
            (AnchorLimitId::ConcurrentRendererJobs, s(4, 16)),
            (
                AnchorLimitId::RendererCpuWallTimePerGeneration,
                s(30_000, 120_000),
            ),
            (AnchorLimitId::PublishedGenerationsPerSite, s(2, 2)),
            (
                AnchorLimitId::LocalOperationalLogBytesAllClasses,
                s(512 * M, 2 * G),
            ),
            (AnchorLimitId::DiagnosticLogBytes, s(128 * M, 512 * M)),
            (AnchorLimitId::RotatedLocalLogFiles, s(128, 512)),
            (AnchorLimitId::ConcurrentGossipSessionsPerPeer, s(2, 4)),
            (AnchorLimitId::GossipTransferPerPeerPerHour, s(256 * M, G)),
            (AnchorLimitId::PendingPublicIrohQuicHandshakes, s(64, 256)),
            (
                AnchorLimitId::PublicIrohQuicHandshakeWallTime,
                s(5_000, 10_000),
            ),
            (
                AnchorLimitId::ControlSyncFirstFrameWallTime,
                s(5_000, 10_000),
            ),
            (
                AnchorLimitId::ControlFrameReadWriteWallTime,
                s(10_000, 30_000),
            ),
            (
                AnchorLimitId::SyncFrameReadWriteWallTime,
                s(30_000, 120_000),
            ),
            (AnchorLimitId::ControlSyncProgressInterval, s(5_000, 15_000)),
            (
                AnchorLimitId::ControlSyncIdleAbsoluteSessionLifetime,
                c((30_000, 900_000), (60_000, 3_600_000)),
            ),
            (AnchorLimitId::SnapshotCursorLifetime, s(900_000, 3_600_000)),
            (
                AnchorLimitId::PublicIrohHandshakesPerSourcePerMinute,
                s(30, 120),
            ),
            (
                AnchorLimitId::PublicIrohHandshakesGloballyPerSecond,
                s(200, 800),
            ),
            (AnchorLimitId::DirectRootPrefilterQueueJobs, s(128, 512)),
            (
                AnchorLimitId::DirectRootPrefilterQueueCanonicalBytes,
                s(4 * M, 16 * M),
            ),
            (
                AnchorLimitId::DirectRootSignatureCpuWallTime,
                c((2, 50), (10, 100)),
            ),
        ];
        let entries = rows
            .into_iter()
            .map(|(id, (effective, absolute))| AnchorLimitEntry {
                id,
                effective,
                absolute,
            })
            .collect();
        AnchorLimitProfileV1 {
            profile_epoch,
            entries,
        }
    }
}

impl CanonicalRecord for AnchorLimitProfileV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        if self.entries.len() != ALL_LIMIT_IDS.len() {
            return Err(CodecError::WrongArrayLength {
                expected: ALL_LIMIT_IDS.len() as u64,
                actual: self.entries.len() as u64,
            });
        }
        // Enforce all-present-once, strictly ascending.
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.id.id() != (index as u64) + 1 {
                return Err(CodecError::NonCanonical);
            }
        }
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(3).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            e.u64(self.profile_epoch)
                .map_err(|_| CodecError::Malformed)?;
            e.array(self.entries.len() as u64)
                .map_err(|_| CodecError::Malformed)?;
            for entry in &self.entries {
                e.array(3).map_err(|_| CodecError::Malformed)?;
                e.u64(entry.id.id()).map_err(|_| CodecError::Malformed)?;
                entry.effective.encode(&mut e)?;
                entry.absolute.encode(&mut e)?;
            }
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 3)?;
        read_version(d, 1)?;
        let profile_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
        let count = definite_array(d)?;
        if count != ALL_LIMIT_IDS.len() as u64 {
            return Err(CodecError::WrongArrayLength {
                expected: ALL_LIMIT_IDS.len() as u64,
                actual: count,
            });
        }
        let mut entries = Vec::with_capacity(count as usize);
        for index in 0..count {
            expect_array(d, 3)?;
            let raw_id = d.u64().map_err(|_| CodecError::Malformed)?;
            let id = AnchorLimitId::from_id(raw_id).ok_or(CodecError::UnknownVariant)?;
            if id.id() != index + 1 {
                // Out of ascending order or a repeat.
                return Err(CodecError::NonCanonical);
            }
            let effective = LimitValue::decode(d)?;
            let absolute = LimitValue::decode(d)?;
            entries.push(AnchorLimitEntry {
                id,
                effective,
                absolute,
            });
        }
        Ok(AnchorLimitProfileV1 {
            profile_epoch,
            entries,
        })
    }
}

// ===========================================================================
// WU-004: operator keys, the generic operator-signed envelope, and Ed25519
// verification shared by descriptors, receipts, work challenges, and
// attestations.
// ===========================================================================

/// Encoded-record ceilings (design "Encoded control-record profile").
/// `DescriptorEnvelopeV1` maximum.
pub const MAX_DESCRIPTOR_ENVELOPE_BYTES: usize = 8 * 1024;
/// Well-known descriptor response maximum.
pub const MAX_WELL_KNOWN_DESCRIPTOR_BYTES: usize = 16 * 1024;
/// One descriptor-chain page maximum (also `MAX_DESCRIPTOR_CHAIN_PAGE_ENVELOPES`).
pub const MAX_DESCRIPTOR_CHAIN_PAGE_BYTES: usize = 60 * 1024;
/// At most 16 descriptor envelopes per chain page.
pub const MAX_DESCRIPTOR_CHAIN_PAGE_ENVELOPES: usize = 16;
/// A full descriptor-chain traversal is capped at 32 hops.
pub const MAX_DESCRIPTOR_CHAIN_HOPS: usize = 32;
/// A full descriptor-chain traversal is capped at 256 KiB of canonical bytes.
pub const MAX_DESCRIPTOR_CHAIN_BYTES: usize = 256 * 1024;
/// `HostingReceiptV1`, `ListingReceiptV1`, or `ReplicaSourceAttestationV1` maximum.
pub const MAX_RECEIPT_BYTES: usize = 4 * 1024;
/// `WorkChallengeV1` / `WorkStampV1` / `ReplicaPrepareChallengeV1` maximum.
pub const MAX_WORK_FRAME_BYTES: usize = 4 * 1024;

/// Descriptor HTTPS origin maximum (UTF-8 bytes).
pub const MAX_HTTPS_ORIGIN_BYTES: usize = 255;
/// Operator / failure-domain label maximum (UTF-8 bytes).
pub const MAX_OPERATOR_LABEL_BYTES: usize = 64;
/// Supported control/sync version arrays hold at most 16 distinct entries.
pub const MAX_VERSION_ENTRIES: usize = 16;
/// A random 128-bit idempotency key is exactly 16 bytes.
pub const IDEMPOTENCY_KEY_BYTES: usize = 16;

const OPERATOR_KEY_ALGORITHM: &str = "ed25519";
const MAX_ALGORITHM_TOKEN: usize = 16;
const MAX_ROLE_TOKEN: usize = 16;

/// Verify a strict Ed25519 signature; `false` for a bad key point or signature.
pub(crate) fn verify_ed25519_strict(
    pubkey: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> bool {
    match VerifyingKey::from_bytes(pubkey) {
        Ok(key) => key
            .verify_strict(message, &Signature::from_bytes(signature))
            .is_ok(),
        Err(_) => false,
    }
}

/// `OperatorVerificationKeyV1 { algorithm: ed25519, public_key: 32 }`.
///
/// LAYOUT DECISION (design gives a field list, not a byte layout): the `V1`
/// suffix selects the `NameVn -> [n, ...]` convention, so the canonical form is
/// `[1, "ed25519", public_key]`. This is load-bearing — `operator_key_id`
/// hashes exactly these bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorVerificationKeyV1 {
    /// The 32-byte Ed25519 public key (the only supported algorithm).
    pub public_key: [u8; 32],
}

impl OperatorVerificationKeyV1 {
    /// `operator_key_id = BLAKE3(label || canonical_cbor(verification_key))`.
    pub fn operator_key_id(&self) -> Result<[u8; 32], CodecError> {
        Ok(crate::digest::operator_key_id(&self.encode_canonical()?))
    }
}

impl CanonicalRecord for OperatorVerificationKeyV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(3).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            e.str(OPERATOR_KEY_ALGORITHM)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.public_key)
                .map_err(|_| CodecError::Malformed)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 3)?;
        read_version(d, 1)?;
        let algorithm = read_discriminant(d, MAX_ALGORITHM_TOKEN)?;
        if algorithm != OPERATOR_KEY_ALGORITHM {
            return Err(CodecError::UnknownVariant);
        }
        let public_key = read_fixed_bytes::<32>(d)?;
        Ok(OperatorVerificationKeyV1 { public_key })
    }
}

/// A body carried inside an [`OperatorSignedEnvelopeV1`], with its exact bare
/// signing-domain prefix.
pub trait AnchorSignedBody: CanonicalRecord {
    /// `Sign(SIGNING_DOMAIN || canonical_cbor(body))`.
    const SIGNING_DOMAIN: &'static [u8];
}

/// `OperatorSignedEnvelopeV1<T> = canonical_cbor([1, T, exactly_64_signature_bytes])`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorSignedEnvelopeV1<B: AnchorSignedBody> {
    /// The signed body.
    pub body: B,
    /// The 64-byte Ed25519 operator signature over `B::SIGNING_DOMAIN || body`.
    pub operator_signature: [u8; 64],
}

impl<B: AnchorSignedBody> OperatorSignedEnvelopeV1<B> {
    /// The exact preimage the operator signs.
    pub fn signing_preimage(&self) -> Result<Vec<u8>, CodecError> {
        let mut preimage = B::SIGNING_DOMAIN.to_vec();
        preimage.extend_from_slice(&self.body.encode_canonical()?);
        Ok(preimage)
    }

    /// Verify the envelope signature under `operator_public_key`.
    pub fn verify(&self, operator_public_key: &[u8; 32]) -> Result<(), CodecError> {
        let preimage = self.signing_preimage()?;
        if verify_ed25519_strict(operator_public_key, &preimage, &self.operator_signature) {
            Ok(())
        } else {
            Err(CodecError::NonCanonical)
        }
    }

    /// `digest_v1(label, canonical(envelope))` for the envelope digests named in
    /// the design's identity-digest table.
    pub fn envelope_digest(&self, digest_label: &[u8]) -> Result<[u8; 32], CodecError> {
        Ok(digest_v1(digest_label, &self.encode_canonical()?))
    }
}

impl<B: AnchorSignedBody> CanonicalRecord for OperatorSignedEnvelopeV1<B> {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(3).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
        }
        buf.extend_from_slice(&self.body.encode_canonical()?);
        {
            let mut e = Encoder::new(&mut buf);
            e.bytes(&self.operator_signature)
                .map_err(|_| CodecError::Malformed)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 3)?;
        read_version(d, 1)?;
        let body = B::decode_fields(d)?;
        let operator_signature = read_fixed_bytes::<64>(d)?;
        Ok(OperatorSignedEnvelopeV1 {
            body,
            operator_signature,
        })
    }
}

// ---------------------------------------------------------------------------
// EnabledRole — closed set of descriptor roles.
// ---------------------------------------------------------------------------

/// A descriptor-advertised anchor role. CLOSED enum, wire = `snake_case` token.
///
/// AMBIGUITY (flagged for pre-WU-006 confirmation): the design states "roles have
/// at most the four defined values" and confirms `host` and `mirror` (line 3131)
/// but never enumerates all four in one place. `directory` and `gossip` are
/// inferred from the architecture (directory feeds + gossip amplification). If
/// WU-006 freezes different tokens these must change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EnabledRole {
    /// `directory` — publishes the directory feed/snapshot.
    Directory,
    /// `gossip` — participates in gossip amplification.
    Gossip,
    /// `host` — hosts committed public site state (CONFIRMED by design).
    Host,
    /// `mirror` — serves the static web projection/search (CONFIRMED by design).
    Mirror,
}

impl EnabledRole {
    /// The exact `snake_case` wire token.
    pub fn token(self) -> &'static str {
        match self {
            EnabledRole::Directory => "directory",
            EnabledRole::Gossip => "gossip",
            EnabledRole::Host => "host",
            EnabledRole::Mirror => "mirror",
        }
    }

    /// Parse a wire token, or `None` for anything unrecognized.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "directory" => Some(EnabledRole::Directory),
            "gossip" => Some(EnabledRole::Gossip),
            "host" => Some(EnabledRole::Host),
            "mirror" => Some(EnabledRole::Mirror),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Sorted-canonical set helpers for u64 and EnabledRole.
// ---------------------------------------------------------------------------

fn encode_u64_set(buf: &mut Vec<u8>, elements: &[u64], max_len: usize) -> Result<(), CodecError> {
    if elements.len() > max_len {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut encoded: Vec<Vec<u8>> = Vec::with_capacity(elements.len());
    for el in elements {
        let mut item = Vec::new();
        Encoder::new(&mut item)
            .u64(*el)
            .map_err(|_| CodecError::Malformed)?;
        encoded.push(item);
    }
    let sorted = sort_canonical_set(encoded)?;
    {
        let mut e = Encoder::new(&mut *buf);
        e.array(sorted.len() as u64)
            .map_err(|_| CodecError::Malformed)?;
    }
    for item in &sorted {
        buf.extend_from_slice(item);
    }
    Ok(())
}

fn decode_u64_set(d: &mut Decoder<'_>, max_len: usize) -> Result<Vec<u64>, CodecError> {
    let count = definite_array(d)?;
    if count as usize > max_len {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut out = Vec::with_capacity(count as usize);
    let mut previous: Option<Vec<u8>> = None;
    for _ in 0..count {
        let value = d.u64().map_err(|_| CodecError::Malformed)?;
        let mut canonical = Vec::new();
        Encoder::new(&mut canonical)
            .u64(value)
            .map_err(|_| CodecError::Malformed)?;
        assert_set_order(previous.as_deref(), &canonical)?;
        previous = Some(canonical);
        out.push(value);
    }
    Ok(out)
}

fn encode_role_set(buf: &mut Vec<u8>, roles: &[EnabledRole]) -> Result<(), CodecError> {
    if roles.len() > 4 {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut encoded: Vec<Vec<u8>> = Vec::with_capacity(roles.len());
    for role in roles {
        let mut item = Vec::new();
        Encoder::new(&mut item)
            .str(role.token())
            .map_err(|_| CodecError::Malformed)?;
        encoded.push(item);
    }
    let sorted = sort_canonical_set(encoded)?;
    {
        let mut e = Encoder::new(&mut *buf);
        e.array(sorted.len() as u64)
            .map_err(|_| CodecError::Malformed)?;
    }
    for item in &sorted {
        buf.extend_from_slice(item);
    }
    Ok(())
}

fn decode_role_set(d: &mut Decoder<'_>) -> Result<Vec<EnabledRole>, CodecError> {
    let count = definite_array(d)?;
    if count as usize > 4 {
        return Err(CodecError::LengthOutOfRange);
    }
    let mut out = Vec::with_capacity(count as usize);
    let mut previous: Option<Vec<u8>> = None;
    for _ in 0..count {
        let token = read_discriminant(d, MAX_ROLE_TOKEN)?;
        let mut canonical = Vec::new();
        Encoder::new(&mut canonical)
            .str(&token)
            .map_err(|_| CodecError::Malformed)?;
        assert_set_order(previous.as_deref(), &canonical)?;
        previous = Some(canonical);
        out.push(EnabledRole::from_token(&token).ok_or(CodecError::UnknownVariant)?);
    }
    Ok(out)
}

fn read_idempotency_key(d: &mut Decoder<'_>) -> Result<[u8; IDEMPOTENCY_KEY_BYTES], CodecError> {
    read_fixed_bytes::<IDEMPOTENCY_KEY_BYTES>(d)
}

// ---------------------------------------------------------------------------
// ControlOperationKind — the 9 control operation-kind tokens.
// ---------------------------------------------------------------------------

/// The closed set of control operation kinds. CLOSED enum, wire = `snake_case`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlOperationKind {
    /// `describe`
    Describe,
    /// `get_work_challenge`
    GetWorkChallenge,
    /// `prepare_host`
    PrepareHost,
    /// `commit_host`
    CommitHost,
    /// `submit_listing`
    SubmitListing,
    /// `prepare_replica`
    PrepareReplica,
    /// `pull_directory_feed`
    PullDirectoryFeed,
    /// `pull_directory_snapshot`
    PullDirectorySnapshot,
    /// `get_operation`
    GetOperation,
}

impl ControlOperationKind {
    /// The exact `snake_case` wire token.
    pub fn token(self) -> &'static str {
        match self {
            ControlOperationKind::Describe => "describe",
            ControlOperationKind::GetWorkChallenge => "get_work_challenge",
            ControlOperationKind::PrepareHost => "prepare_host",
            ControlOperationKind::CommitHost => "commit_host",
            ControlOperationKind::SubmitListing => "submit_listing",
            ControlOperationKind::PrepareReplica => "prepare_replica",
            ControlOperationKind::PullDirectoryFeed => "pull_directory_feed",
            ControlOperationKind::PullDirectorySnapshot => "pull_directory_snapshot",
            ControlOperationKind::GetOperation => "get_operation",
        }
    }

    /// Parse a wire token, or `None` for anything unrecognized.
    pub fn from_token(token: &str) -> Option<Self> {
        Some(match token {
            "describe" => ControlOperationKind::Describe,
            "get_work_challenge" => ControlOperationKind::GetWorkChallenge,
            "prepare_host" => ControlOperationKind::PrepareHost,
            "commit_host" => ControlOperationKind::CommitHost,
            "submit_listing" => ControlOperationKind::SubmitListing,
            "prepare_replica" => ControlOperationKind::PrepareReplica,
            "pull_directory_feed" => ControlOperationKind::PullDirectoryFeed,
            "pull_directory_snapshot" => ControlOperationKind::PullDirectorySnapshot,
            "get_operation" => ControlOperationKind::GetOperation,
            _ => return None,
        })
    }

    pub(crate) fn encode(self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), CodecError> {
        e.str(self.token()).map_err(|_| CodecError::Malformed)?;
        Ok(())
    }

    pub(crate) fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let token = read_discriminant(d, 32)?;
        ControlOperationKind::from_token(&token).ok_or(CodecError::UnknownVariant)
    }
}

// ===========================================================================
// AnchorDescriptorBodyV1 / DescriptorEnvelopeV1 / DescriptorFloor
// ===========================================================================

/// `AnchorDescriptorBodyV1` — the 18-field descriptor floor body.
///
/// LAYOUT DECISION: `NameVn -> [1, ...fields]` (design gives a field list only).
/// `current_iroh_endpoint_id` is modelled as a 32-byte iroh node id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorDescriptorBodyV1 {
    /// `AnchorId` recomputed and checked by verifiers.
    pub anchor_id: [u8; 32],
    /// Genesis operator public key (an `AnchorId` input).
    pub genesis_operator_public_key: [u8; 32],
    /// Genesis 256-bit random (an `AnchorId` input).
    pub genesis_random_256_bits: [u8; 32],
    /// The current operator verification key.
    pub current_operator_verification_key: OperatorVerificationKeyV1,
    /// `operator_key_id` of the current key (recomputed and checked).
    pub current_operator_key_id: [u8; 32],
    /// Monotonic descriptor epoch (0 = genesis).
    pub descriptor_epoch: u64,
    /// Digest of the previous descriptor envelope (`null` only at epoch 0).
    pub previous_descriptor_digest: Option<[u8; 32]>,
    /// Current iroh endpoint (node) id.
    pub current_iroh_endpoint_id: [u8; 32],
    /// HTTPS origin (<= 255 UTF-8 bytes).
    pub https_origin: String,
    /// Operator display label (<= 64 UTF-8 bytes).
    pub operator_display_label: String,
    /// Self-reported failure-domain label (<= 64 UTF-8 bytes).
    pub self_reported_failure_domain_label: String,
    /// Supported control versions (sorted set, <= 16).
    pub supported_control_versions: Vec<u64>,
    /// Supported sync versions (sorted set, <= 16).
    pub supported_sync_versions: Vec<u64>,
    /// Enabled roles (sorted set, <= 4).
    pub enabled_roles: Vec<EnabledRole>,
    /// `limit_profile_digest` of the advertised profile.
    pub limit_profile_digest: [u8; 32],
    /// Predecessor operator verification key (`null` only at epoch 0).
    pub predecessor_operator_verification_key: Option<OperatorVerificationKeyV1>,
    /// Issue time (Unix seconds).
    pub issued_at: u64,
    /// Expiry (Unix seconds).
    pub expires_at: u64,
}

impl CanonicalRecord for AnchorDescriptorBodyV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        if self.https_origin.len() > MAX_HTTPS_ORIGIN_BYTES
            || self.operator_display_label.len() > MAX_OPERATOR_LABEL_BYTES
            || self.self_reported_failure_domain_label.len() > MAX_OPERATOR_LABEL_BYTES
        {
            return Err(CodecError::LengthOutOfRange);
        }
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(19).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.anchor_id)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.genesis_operator_public_key)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.genesis_random_256_bits)
                .map_err(|_| CodecError::Malformed)?;
        }
        buf.extend_from_slice(&self.current_operator_verification_key.encode_canonical()?);
        {
            let mut e = Encoder::new(&mut buf);
            e.bytes(&self.current_operator_key_id)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.descriptor_epoch)
                .map_err(|_| CodecError::Malformed)?;
            match &self.previous_descriptor_digest {
                Some(d) => {
                    e.bytes(d).map_err(|_| CodecError::Malformed)?;
                }
                None => {
                    e.null().map_err(|_| CodecError::Malformed)?;
                }
            }
            e.bytes(&self.current_iroh_endpoint_id)
                .map_err(|_| CodecError::Malformed)?;
            e.str(&self.https_origin)
                .map_err(|_| CodecError::Malformed)?;
            e.str(&self.operator_display_label)
                .map_err(|_| CodecError::Malformed)?;
            e.str(&self.self_reported_failure_domain_label)
                .map_err(|_| CodecError::Malformed)?;
        }
        encode_u64_set(
            &mut buf,
            &self.supported_control_versions,
            MAX_VERSION_ENTRIES,
        )?;
        encode_u64_set(&mut buf, &self.supported_sync_versions, MAX_VERSION_ENTRIES)?;
        encode_role_set(&mut buf, &self.enabled_roles)?;
        {
            let mut e = Encoder::new(&mut buf);
            e.bytes(&self.limit_profile_digest)
                .map_err(|_| CodecError::Malformed)?;
        }
        match &self.predecessor_operator_verification_key {
            Some(key) => buf.extend_from_slice(&key.encode_canonical()?),
            None => {
                let mut e = Encoder::new(&mut buf);
                e.null().map_err(|_| CodecError::Malformed)?;
            }
        }
        {
            let mut e = Encoder::new(&mut buf);
            e.u64(self.issued_at).map_err(|_| CodecError::Malformed)?;
            e.u64(self.expires_at).map_err(|_| CodecError::Malformed)?;
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 19)?;
        read_version(d, 1)?;
        let anchor_id = read_fixed_bytes::<32>(d)?;
        let genesis_operator_public_key = read_fixed_bytes::<32>(d)?;
        let genesis_random_256_bits = read_fixed_bytes::<32>(d)?;
        let current_operator_verification_key = OperatorVerificationKeyV1::decode_fields(d)?;
        let current_operator_key_id = read_fixed_bytes::<32>(d)?;
        let descriptor_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
        let previous_descriptor_digest = if peek_null(d)? {
            read_null(d)?;
            None
        } else {
            Some(read_fixed_bytes::<32>(d)?)
        };
        let current_iroh_endpoint_id = read_fixed_bytes::<32>(d)?;
        let https_origin = read_text_max(d, 0, MAX_HTTPS_ORIGIN_BYTES)?;
        let operator_display_label = read_text_max(d, 0, MAX_OPERATOR_LABEL_BYTES)?;
        let self_reported_failure_domain_label = read_text_max(d, 0, MAX_OPERATOR_LABEL_BYTES)?;
        let supported_control_versions = decode_u64_set(d, MAX_VERSION_ENTRIES)?;
        let supported_sync_versions = decode_u64_set(d, MAX_VERSION_ENTRIES)?;
        let enabled_roles = decode_role_set(d)?;
        let limit_profile_digest = read_fixed_bytes::<32>(d)?;
        let predecessor_operator_verification_key = if peek_null(d)? {
            read_null(d)?;
            None
        } else {
            Some(OperatorVerificationKeyV1::decode_fields(d)?)
        };
        let issued_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let expires_at = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(AnchorDescriptorBodyV1 {
            anchor_id,
            genesis_operator_public_key,
            genesis_random_256_bits,
            current_operator_verification_key,
            current_operator_key_id,
            descriptor_epoch,
            previous_descriptor_digest,
            current_iroh_endpoint_id,
            https_origin,
            operator_display_label,
            self_reported_failure_domain_label,
            supported_control_versions,
            supported_sync_versions,
            enabled_roles,
            limit_profile_digest,
            predecessor_operator_verification_key,
            issued_at,
            expires_at,
        })
    }
}

impl AnchorDescriptorBodyV1 {
    /// The `AnchorId` recomputed from this body's genesis inputs.
    pub fn recomputed_anchor_id(&self) -> [u8; 32] {
        crate::digest::anchor_id(
            &self.genesis_operator_public_key,
            &self.genesis_random_256_bits,
        )
    }
}

/// `DescriptorEnvelopeV1 { body, current_signature, predecessor_signature? }`.
///
/// LAYOUT DECISION: `[1, body, current_signature, null|predecessor_signature]`.
/// `current_signature = Sign(current_key, "riot/anchor-descriptor/v1" ||
/// canonical_cbor(body))`; `predecessor_signature = Sign(predecessor_key,
/// "riot/anchor-descriptor-transition/v1" || BLAKE3(canonical_cbor(body)))`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorEnvelopeV1 {
    /// The descriptor body.
    pub body: AnchorDescriptorBodyV1,
    /// The current-operator signature.
    pub current_signature: [u8; 64],
    /// The predecessor-operator transition signature (`null` only at epoch 0).
    pub predecessor_signature: Option<[u8; 64]>,
}

impl DescriptorEnvelopeV1 {
    /// Preimage the current operator signs.
    pub fn current_signing_preimage(&self) -> Result<Vec<u8>, CodecError> {
        let mut preimage = label::DESCRIPTOR_SIG.to_vec();
        preimage.extend_from_slice(&self.body.encode_canonical()?);
        Ok(preimage)
    }

    /// Preimage the predecessor operator signs (a domain-prefixed body BLAKE3).
    pub fn predecessor_signing_preimage(&self) -> Result<Vec<u8>, CodecError> {
        let body_hash = blake3::hash(&self.body.encode_canonical()?);
        let mut preimage = label::DESCRIPTOR_TRANSITION_SIG.to_vec();
        preimage.extend_from_slice(body_hash.as_bytes());
        Ok(preimage)
    }

    /// `descriptor_digest = digest_v1("riot/anchor-descriptor-envelope/v1", envelope)`.
    pub fn descriptor_digest(&self) -> Result<[u8; 32], CodecError> {
        Ok(digest_v1(
            label::DESCRIPTOR_ENVELOPE,
            &self.encode_canonical()?,
        ))
    }

    /// Verify the current-operator signature under the body's current key.
    pub fn verify_current(&self) -> Result<(), CodecError> {
        let preimage = self.current_signing_preimage()?;
        let key = self.body.current_operator_verification_key.public_key;
        if verify_ed25519_strict(&key, &preimage, &self.current_signature) {
            Ok(())
        } else {
            Err(CodecError::NonCanonical)
        }
    }
}

impl CanonicalRecord for DescriptorEnvelopeV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(4).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
        }
        buf.extend_from_slice(&self.body.encode_canonical()?);
        {
            let mut e = Encoder::new(&mut buf);
            e.bytes(&self.current_signature)
                .map_err(|_| CodecError::Malformed)?;
            match &self.predecessor_signature {
                Some(sig) => {
                    e.bytes(sig).map_err(|_| CodecError::Malformed)?;
                }
                None => {
                    e.null().map_err(|_| CodecError::Malformed)?;
                }
            }
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 4)?;
        read_version(d, 1)?;
        let body = AnchorDescriptorBodyV1::decode_fields(d)?;
        let current_signature = read_fixed_bytes::<64>(d)?;
        let predecessor_signature = if peek_null(d)? {
            read_null(d)?;
            None
        } else {
            Some(read_fixed_bytes::<64>(d)?)
        };
        Ok(DescriptorEnvelopeV1 {
            body,
            current_signature,
            predecessor_signature,
        })
    }
}

/// The authenticated descriptor floor a client pins:
/// `(AnchorId, epoch, descriptor_digest, OperatorVerificationKeyV1)`.
///
/// LAYOUT DECISION: version-scoped tuple `[anchor_id, descriptor_epoch,
/// descriptor_digest, operator_verification_key]` (no leading version int).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorFloor {
    /// The stable anchor id.
    pub anchor_id: [u8; 32],
    /// The pinned descriptor epoch.
    pub descriptor_epoch: u64,
    /// The pinned descriptor digest.
    pub descriptor_digest: [u8; 32],
    /// The pinned operator verification key.
    pub operator_verification_key: OperatorVerificationKeyV1,
}

impl CanonicalRecord for DescriptorFloor {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(4).map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.anchor_id)
                .map_err(|_| CodecError::Malformed)?;
            e.u64(self.descriptor_epoch)
                .map_err(|_| CodecError::Malformed)?;
            e.bytes(&self.descriptor_digest)
                .map_err(|_| CodecError::Malformed)?;
        }
        buf.extend_from_slice(&self.operator_verification_key.encode_canonical()?);
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 4)?;
        let anchor_id = read_fixed_bytes::<32>(d)?;
        let descriptor_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
        let descriptor_digest = read_fixed_bytes::<32>(d)?;
        let operator_verification_key = OperatorVerificationKeyV1::decode_fields(d)?;
        Ok(DescriptorFloor {
            anchor_id,
            descriptor_epoch,
            descriptor_digest,
            operator_verification_key,
        })
    }
}

// ===========================================================================
// Receipts: HostingReceiptBodyV1, ListingReceiptBodyV1.
// ===========================================================================

/// A per-namespace hosting result `[namespace_id, snapshot_digest, entry_count]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamespaceResult {
    /// The namespace id.
    pub namespace_id: [u8; 32],
    /// The committed snapshot digest.
    pub snapshot_digest: [u8; 32],
    /// The committed live entry count.
    pub entry_count: u64,
}

impl NamespaceResult {
    fn encode(&self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), CodecError> {
        e.array(3).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.namespace_id)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.snapshot_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.entry_count).map_err(|_| CodecError::Malformed)?;
        Ok(())
    }

    fn decode(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 3)?;
        let namespace_id = read_fixed_bytes::<32>(d)?;
        let snapshot_digest = read_fixed_bytes::<32>(d)?;
        let entry_count = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(NamespaceResult {
            namespace_id,
            snapshot_digest,
            entry_count,
        })
    }
}

/// The hosting-receipt status token.
///
/// AMBIGUITY (flagged for pre-WU-006 confirmation): the design lists a `status`
/// field but never enumerates its vocabulary. `committed` is the only value a
/// signed hosting receipt is created with (design lifecycle: a Commit success
/// stores `terminal ["committed", hosting_receipt]`). If WU-006 introduces more
/// status tokens (e.g. partial hosting) this closed enum must grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostingStatus {
    /// `committed` — the site generation was committed and admitted.
    Committed,
}

impl HostingStatus {
    fn token(self) -> &'static str {
        match self {
            HostingStatus::Committed => "committed",
        }
    }
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "committed" => Some(HostingStatus::Committed),
            _ => None,
        }
    }
}

/// `HostingReceiptBodyV1` — 15 positional fields; `NameVn -> [1, ...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostingReceiptBodyV1 {
    /// Stable anchor id.
    pub anchor_id: [u8; 32],
    /// Signing operator key id.
    pub operator_key_id: [u8; 32],
    /// Signing descriptor epoch.
    pub descriptor_epoch: u64,
    /// Signing descriptor digest.
    pub descriptor_digest: [u8; 32],
    /// The hosting operation id.
    pub hosting_operation_id: [u8; 32],
    /// The full site root (`O` root).
    pub full_site_root: [u8; 32],
    /// Committed manifest digest.
    pub manifest_digest: [u8; 32],
    /// Committed manifest version.
    pub manifest_version: u64,
    /// Base site generation captured at Prepare.
    pub base_site_generation: u64,
    /// Committed site generation.
    pub committed_site_generation: u64,
    /// Ordered `O`, `C`, `W` namespace results (exactly 3).
    pub ordered_namespace_results: Vec<NamespaceResult>,
    /// Hosting status.
    pub status: HostingStatus,
    /// Accept time (Unix seconds).
    pub accepted_at: u64,
    /// Operator-claimed retention horizon (Unix seconds).
    pub reported_retention_through: u64,
    /// Advertised limit-profile digest.
    pub limit_profile_digest: [u8; 32],
}

impl AnchorSignedBody for HostingReceiptBodyV1 {
    const SIGNING_DOMAIN: &'static [u8] = label::HOSTING_RECEIPT;
}

impl CanonicalRecord for HostingReceiptBodyV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        if self.ordered_namespace_results.len() != 3 {
            return Err(CodecError::WrongArrayLength {
                expected: 3,
                actual: self.ordered_namespace_results.len() as u64,
            });
        }
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(16).map_err(|_| CodecError::Malformed)?;
        e.u64(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.anchor_id)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.operator_key_id)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.descriptor_epoch)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.descriptor_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.hosting_operation_id)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.full_site_root)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.manifest_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.manifest_version)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.base_site_generation)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.committed_site_generation)
            .map_err(|_| CodecError::Malformed)?;
        e.array(3).map_err(|_| CodecError::Malformed)?;
        for result in &self.ordered_namespace_results {
            result.encode(&mut e)?;
        }
        e.str(self.status.token())
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.accepted_at).map_err(|_| CodecError::Malformed)?;
        e.u64(self.reported_retention_through)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.limit_profile_digest)
            .map_err(|_| CodecError::Malformed)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 16)?;
        read_version(d, 1)?;
        let anchor_id = read_fixed_bytes::<32>(d)?;
        let operator_key_id = read_fixed_bytes::<32>(d)?;
        let descriptor_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
        let descriptor_digest = read_fixed_bytes::<32>(d)?;
        let hosting_operation_id = read_fixed_bytes::<32>(d)?;
        let full_site_root = read_fixed_bytes::<32>(d)?;
        let manifest_digest = read_fixed_bytes::<32>(d)?;
        let manifest_version = d.u64().map_err(|_| CodecError::Malformed)?;
        let base_site_generation = d.u64().map_err(|_| CodecError::Malformed)?;
        let committed_site_generation = d.u64().map_err(|_| CodecError::Malformed)?;
        expect_array(d, 3)?;
        let mut ordered_namespace_results = Vec::with_capacity(3);
        for _ in 0..3 {
            ordered_namespace_results.push(NamespaceResult::decode(d)?);
        }
        let status_token = read_discriminant(d, 32)?;
        let status = HostingStatus::from_token(&status_token).ok_or(CodecError::UnknownVariant)?;
        let accepted_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let reported_retention_through = d.u64().map_err(|_| CodecError::Malformed)?;
        let limit_profile_digest = read_fixed_bytes::<32>(d)?;
        Ok(HostingReceiptBodyV1 {
            anchor_id,
            operator_key_id,
            descriptor_epoch,
            descriptor_digest,
            hosting_operation_id,
            full_site_root,
            manifest_digest,
            manifest_version,
            base_site_generation,
            committed_site_generation,
            ordered_namespace_results,
            status,
            accepted_at,
            reported_retention_through,
            limit_profile_digest,
        })
    }
}

/// `HostingReceiptV1 = OperatorSignedEnvelopeV1<HostingReceiptBodyV1>`.
pub type HostingReceiptV1 = OperatorSignedEnvelopeV1<HostingReceiptBodyV1>;

/// `ListingReceiptBodyV1` — 12 positional fields; `NameVn -> [1, ...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingReceiptBodyV1 {
    /// Stable anchor id.
    pub anchor_id: [u8; 32],
    /// Signing operator key id.
    pub operator_key_id: [u8; 32],
    /// Signing descriptor epoch.
    pub descriptor_epoch: u64,
    /// Signing descriptor digest.
    pub descriptor_digest: [u8; 32],
    /// The admitted listing digest.
    pub listing_digest: [u8; 32],
    /// The full site root.
    pub full_site_root: [u8; 32],
    /// Accepted listing epoch.
    pub accepted_listing_epoch: u32,
    /// Accepted listing revision.
    pub accepted_listing_revision: u32,
    /// The directory feed coordinate (monotonic sequence).
    pub feed_coordinate: u64,
    /// Accept time (Unix seconds).
    pub accepted_at: u64,
    /// Expiry (Unix seconds).
    pub expires_at: u64,
    /// The request's 128-bit idempotency key.
    pub request_idempotency_key: [u8; IDEMPOTENCY_KEY_BYTES],
}

impl AnchorSignedBody for ListingReceiptBodyV1 {
    const SIGNING_DOMAIN: &'static [u8] = label::LISTING_RECEIPT;
}

impl CanonicalRecord for ListingReceiptBodyV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(13).map_err(|_| CodecError::Malformed)?;
        e.u64(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.anchor_id)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.operator_key_id)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.descriptor_epoch)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.descriptor_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.listing_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.full_site_root)
            .map_err(|_| CodecError::Malformed)?;
        e.u32(self.accepted_listing_epoch)
            .map_err(|_| CodecError::Malformed)?;
        e.u32(self.accepted_listing_revision)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.feed_coordinate)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.accepted_at).map_err(|_| CodecError::Malformed)?;
        e.u64(self.expires_at).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.request_idempotency_key)
            .map_err(|_| CodecError::Malformed)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 13)?;
        read_version(d, 1)?;
        let anchor_id = read_fixed_bytes::<32>(d)?;
        let operator_key_id = read_fixed_bytes::<32>(d)?;
        let descriptor_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
        let descriptor_digest = read_fixed_bytes::<32>(d)?;
        let listing_digest = read_fixed_bytes::<32>(d)?;
        let full_site_root = read_fixed_bytes::<32>(d)?;
        let accepted_listing_epoch = d.u32().map_err(|_| CodecError::Malformed)?;
        let accepted_listing_revision = d.u32().map_err(|_| CodecError::Malformed)?;
        let feed_coordinate = d.u64().map_err(|_| CodecError::Malformed)?;
        let accepted_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let expires_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let request_idempotency_key = read_idempotency_key(d)?;
        Ok(ListingReceiptBodyV1 {
            anchor_id,
            operator_key_id,
            descriptor_epoch,
            descriptor_digest,
            listing_digest,
            full_site_root,
            accepted_listing_epoch,
            accepted_listing_revision,
            feed_coordinate,
            accepted_at,
            expires_at,
            request_idempotency_key,
        })
    }
}

/// `ListingReceiptV1 = OperatorSignedEnvelopeV1<ListingReceiptBodyV1>`.
pub type ListingReceiptV1 = OperatorSignedEnvelopeV1<ListingReceiptBodyV1>;

// ===========================================================================
// Admission work challenge + stamp.
// ===========================================================================

/// Maximum admission-work difficulty (design: "Difficulty is `0..24`").
pub const MAX_WORK_DIFFICULTY: u64 = 24;

/// `WorkChallengeBodyV1` — 13 positional fields; `NameVn -> [1, ...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkChallengeBodyV1 {
    /// Stable anchor id.
    pub anchor_id: [u8; 32],
    /// Signing operator key id.
    pub operator_key_id: [u8; 32],
    /// Signing descriptor epoch.
    pub descriptor_epoch: u64,
    /// Signing descriptor digest.
    pub descriptor_digest: [u8; 32],
    /// The intended control operation kind.
    pub operation_kind: ControlOperationKind,
    /// The bound 128-bit idempotency key.
    pub idempotency_key: [u8; IDEMPOTENCY_KEY_BYTES],
    /// The bound `work_target_digest` (request with `work_stamp` slot `null`).
    pub work_target_digest: [u8; 32],
    /// The bound community root.
    pub community_root: [u8; 32],
    /// The anchor's 256-bit random challenge.
    pub random_challenge: [u8; 32],
    /// The pressure-policy epoch.
    pub policy_epoch: u64,
    /// The required leading-zero-bit difficulty (`0..=24`).
    pub difficulty: u64,
    /// Issue time (Unix seconds).
    pub issued_at: u64,
    /// Expiry (Unix seconds; at most 5 minutes after issuance).
    pub expires_at: u64,
}

impl AnchorSignedBody for WorkChallengeBodyV1 {
    const SIGNING_DOMAIN: &'static [u8] = label::WORK_CHALLENGE_SIG;
}

impl CanonicalRecord for WorkChallengeBodyV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        if self.difficulty > MAX_WORK_DIFFICULTY {
            return Err(CodecError::LengthOutOfRange);
        }
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(14).map_err(|_| CodecError::Malformed)?;
        e.u64(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.anchor_id)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.operator_key_id)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.descriptor_epoch)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.descriptor_digest)
            .map_err(|_| CodecError::Malformed)?;
        self.operation_kind.encode(&mut e)?;
        e.bytes(&self.idempotency_key)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.work_target_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.community_root)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.random_challenge)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.policy_epoch)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.difficulty).map_err(|_| CodecError::Malformed)?;
        e.u64(self.issued_at).map_err(|_| CodecError::Malformed)?;
        e.u64(self.expires_at).map_err(|_| CodecError::Malformed)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 14)?;
        read_version(d, 1)?;
        let anchor_id = read_fixed_bytes::<32>(d)?;
        let operator_key_id = read_fixed_bytes::<32>(d)?;
        let descriptor_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
        let descriptor_digest = read_fixed_bytes::<32>(d)?;
        let operation_kind = ControlOperationKind::decode(d)?;
        let idempotency_key = read_idempotency_key(d)?;
        let work_target_digest = read_fixed_bytes::<32>(d)?;
        let community_root = read_fixed_bytes::<32>(d)?;
        let random_challenge = read_fixed_bytes::<32>(d)?;
        let policy_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
        let difficulty = d.u64().map_err(|_| CodecError::Malformed)?;
        if difficulty > MAX_WORK_DIFFICULTY {
            return Err(CodecError::LengthOutOfRange);
        }
        let issued_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let expires_at = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(WorkChallengeBodyV1 {
            anchor_id,
            operator_key_id,
            descriptor_epoch,
            descriptor_digest,
            operation_kind,
            idempotency_key,
            work_target_digest,
            community_root,
            random_challenge,
            policy_epoch,
            difficulty,
            issued_at,
            expires_at,
        })
    }
}

/// `WorkChallengeV1 = OperatorSignedEnvelopeV1<WorkChallengeBodyV1>`.
pub type WorkChallengeV1 = OperatorSignedEnvelopeV1<WorkChallengeBodyV1>;

/// Why a [`WorkStampV1`] failed verification. Closed by design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkStampError {
    /// The stamp, nested challenge envelope, or a field failed canonical decode.
    Malformed(CodecError),
    /// The nested challenge's operator signature did not verify.
    BadChallengeSignature,
    /// `proof_bytes != BLAKE3(domain || work_challenge_digest || counter)`.
    BadProof,
    /// The proof has fewer leading zero bits than the challenge difficulty.
    InsufficientWork,
}

impl From<CodecError> for WorkStampError {
    fn from(err: CodecError) -> Self {
        WorkStampError::Malformed(err)
    }
}

fn leading_zero_bits(bytes: &[u8; 32]) -> u32 {
    let mut count = 0;
    for &byte in bytes {
        if byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}

/// `WorkStampV1 = [1, challenge_envelope_bytes, counter, proof_bytes]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkStampV1 {
    /// The complete canonical `OperatorSignedEnvelopeV1<WorkChallengeBodyV1>` bytes.
    pub challenge_envelope_bytes: Vec<u8>,
    /// The proof counter.
    pub counter: u64,
    /// The 32-byte BLAKE3 proof output.
    pub proof_bytes: [u8; 32],
}

impl WorkStampV1 {
    /// Verify the stamp against the anchor's `expected_operator_key`: decode the
    /// nested challenge, recompute `work_challenge_digest`, verify the challenge
    /// signature and proof, and require the proof's leading-zero bits to meet the
    /// challenge difficulty. Returns the verified challenge body on success.
    pub fn verify(
        &self,
        expected_operator_key: &[u8; 32],
    ) -> Result<WorkChallengeBodyV1, WorkStampError> {
        let challenge: WorkChallengeV1 =
            decode_canonical(&self.challenge_envelope_bytes, MAX_WORK_FRAME_BYTES)?;
        if challenge.verify(expected_operator_key).is_err() {
            return Err(WorkStampError::BadChallengeSignature);
        }
        let challenge_digest = digest_v1(
            label::WORK_CHALLENGE_ENVELOPE,
            &self.challenge_envelope_bytes,
        );
        let expected_proof = work_proof(&challenge_digest, self.counter);
        if expected_proof != self.proof_bytes {
            return Err(WorkStampError::BadProof);
        }
        if u64::from(leading_zero_bits(&self.proof_bytes)) < challenge.body.difficulty {
            return Err(WorkStampError::InsufficientWork);
        }
        Ok(challenge.body)
    }
}

impl CanonicalRecord for WorkStampV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(4).map_err(|_| CodecError::Malformed)?;
        e.u64(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.challenge_envelope_bytes)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.counter).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.proof_bytes)
            .map_err(|_| CodecError::Malformed)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 4)?;
        read_version(d, 1)?;
        let challenge_envelope_bytes = read_bytes_max(d, MAX_WORK_FRAME_BYTES)?;
        let counter = d.u64().map_err(|_| CodecError::Malformed)?;
        let proof_bytes = read_fixed_bytes::<32>(d)?;
        Ok(WorkStampV1 {
            challenge_envelope_bytes,
            counter,
            proof_bytes,
        })
    }
}

// ===========================================================================
// Replica prepare challenge + source attestation.
// ===========================================================================

/// `ReplicaPrepareChallengeV1` — 6 positional fields; `NameVn -> [1, ...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicaPrepareChallengeV1 {
    /// The destination anchor id.
    pub destination_anchor_id: [u8; 32],
    /// A 256-bit destination nonce.
    pub random_256_bit_nonce: [u8; 32],
    /// The 128-bit prepare idempotency key.
    pub prepare_idempotency_key: [u8; IDEMPOTENCY_KEY_BYTES],
    /// The full site root being replicated.
    pub full_site_root: [u8; 32],
    /// Issue time (Unix seconds).
    pub issued_at: u64,
    /// Expiry (Unix seconds; at most one minute after issuance).
    pub expires_at: u64,
}

impl CanonicalRecord for ReplicaPrepareChallengeV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(7).map_err(|_| CodecError::Malformed)?;
        e.u64(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.destination_anchor_id)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.random_256_bit_nonce)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.prepare_idempotency_key)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.full_site_root)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.issued_at).map_err(|_| CodecError::Malformed)?;
        e.u64(self.expires_at).map_err(|_| CodecError::Malformed)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 7)?;
        read_version(d, 1)?;
        let destination_anchor_id = read_fixed_bytes::<32>(d)?;
        let random_256_bit_nonce = read_fixed_bytes::<32>(d)?;
        let prepare_idempotency_key = read_idempotency_key(d)?;
        let full_site_root = read_fixed_bytes::<32>(d)?;
        let issued_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let expires_at = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(ReplicaPrepareChallengeV1 {
            destination_anchor_id,
            random_256_bit_nonce,
            prepare_idempotency_key,
            full_site_root,
            issued_at,
            expires_at,
        })
    }
}

/// `ReplicaSourceAttestationBodyV1` — 15 positional fields; `NameVn -> [1, ...]`.
///
/// LAYOUT DECISION: `manifest_digest_and_version` is encoded as the nested value
/// `[manifest_digest, manifest_version]`; `ordered_namespace_snapshot_digests` is
/// the ordered `O`, `C`, `W` triple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicaSourceAttestationBodyV1 {
    /// The source anchor id.
    pub source_anchor_id: [u8; 32],
    /// The source's current operator key id.
    pub source_current_operator_key_id: [u8; 32],
    /// The source's current descriptor epoch.
    pub source_current_descriptor_epoch: u64,
    /// The source's current descriptor digest.
    pub source_current_descriptor_digest: [u8; 32],
    /// The destination anchor id.
    pub destination_anchor_id: [u8; 32],
    /// The bound peer transcript digest.
    pub peer_transcript_digest: [u8; 32],
    /// The destination-issued prepare nonce.
    pub destination_prepare_nonce: [u8; 32],
    /// The 128-bit prepare idempotency key.
    pub prepare_idempotency_key: [u8; IDEMPOTENCY_KEY_BYTES],
    /// The full site root.
    pub full_site_root: [u8; 32],
    /// The bound manifest digest.
    pub manifest_digest: [u8; 32],
    /// The bound manifest version.
    pub manifest_version: u64,
    /// The `root_signed_ticket_core_digest`.
    pub root_signed_ticket_core_digest: [u8; 32],
    /// The source site generation of the immutable snapshot.
    pub source_site_generation: u64,
    /// The ordered `O`, `C`, `W` snapshot digests.
    pub ordered_namespace_snapshot_digests: [[u8; 32]; 3],
    /// Issue time (Unix seconds).
    pub issued_at: u64,
    /// Expiry (Unix seconds; at most 5 minutes after issuance).
    pub expires_at: u64,
}

impl AnchorSignedBody for ReplicaSourceAttestationBodyV1 {
    const SIGNING_DOMAIN: &'static [u8] = label::REPLICA_SOURCE_ATTESTATION_SIG;
}

impl CanonicalRecord for ReplicaSourceAttestationBodyV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(16).map_err(|_| CodecError::Malformed)?;
        e.u64(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.source_anchor_id)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.source_current_operator_key_id)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.source_current_descriptor_epoch)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.source_current_descriptor_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.destination_anchor_id)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.peer_transcript_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.destination_prepare_nonce)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.prepare_idempotency_key)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.full_site_root)
            .map_err(|_| CodecError::Malformed)?;
        e.array(2).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.manifest_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.manifest_version)
            .map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.root_signed_ticket_core_digest)
            .map_err(|_| CodecError::Malformed)?;
        e.u64(self.source_site_generation)
            .map_err(|_| CodecError::Malformed)?;
        e.array(3).map_err(|_| CodecError::Malformed)?;
        for digest in &self.ordered_namespace_snapshot_digests {
            e.bytes(digest).map_err(|_| CodecError::Malformed)?;
        }
        e.u64(self.issued_at).map_err(|_| CodecError::Malformed)?;
        e.u64(self.expires_at).map_err(|_| CodecError::Malformed)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 16)?;
        read_version(d, 1)?;
        let source_anchor_id = read_fixed_bytes::<32>(d)?;
        let source_current_operator_key_id = read_fixed_bytes::<32>(d)?;
        let source_current_descriptor_epoch = d.u64().map_err(|_| CodecError::Malformed)?;
        let source_current_descriptor_digest = read_fixed_bytes::<32>(d)?;
        let destination_anchor_id = read_fixed_bytes::<32>(d)?;
        let peer_transcript_digest = read_fixed_bytes::<32>(d)?;
        let destination_prepare_nonce = read_fixed_bytes::<32>(d)?;
        let prepare_idempotency_key = read_idempotency_key(d)?;
        let full_site_root = read_fixed_bytes::<32>(d)?;
        expect_array(d, 2)?;
        let manifest_digest = read_fixed_bytes::<32>(d)?;
        let manifest_version = d.u64().map_err(|_| CodecError::Malformed)?;
        let root_signed_ticket_core_digest = read_fixed_bytes::<32>(d)?;
        let source_site_generation = d.u64().map_err(|_| CodecError::Malformed)?;
        expect_array(d, 3)?;
        let mut ordered_namespace_snapshot_digests = [[0u8; 32]; 3];
        for slot in ordered_namespace_snapshot_digests.iter_mut() {
            *slot = read_fixed_bytes::<32>(d)?;
        }
        let issued_at = d.u64().map_err(|_| CodecError::Malformed)?;
        let expires_at = d.u64().map_err(|_| CodecError::Malformed)?;
        Ok(ReplicaSourceAttestationBodyV1 {
            source_anchor_id,
            source_current_operator_key_id,
            source_current_descriptor_epoch,
            source_current_descriptor_digest,
            destination_anchor_id,
            peer_transcript_digest,
            destination_prepare_nonce,
            prepare_idempotency_key,
            full_site_root,
            manifest_digest,
            manifest_version,
            root_signed_ticket_core_digest,
            source_site_generation,
            ordered_namespace_snapshot_digests,
            issued_at,
            expires_at,
        })
    }
}

/// `ReplicaSourceAttestationV1 = OperatorSignedEnvelopeV1<ReplicaSourceAttestationBodyV1>`.
pub type ReplicaSourceAttestationV1 = OperatorSignedEnvelopeV1<ReplicaSourceAttestationBodyV1>;

// ===========================================================================
// AnchorBootstrapV1 — app-embedded pinned descriptor floors.
// ===========================================================================

/// The minimum number of enabled descriptors an `AnchorBootstrapV1` must pin.
pub const MIN_BOOTSTRAP_DESCRIPTORS: usize = 3;
/// The minimum number of distinct operators the bootstrap must span.
pub const MIN_BOOTSTRAP_OPERATORS: usize = 2;

/// One pinned bootstrap descriptor: floor + HTTPS origin + roles.
///
/// LAYOUT DECISION: `NameVn -> [1, floor, https_origin, roles_set]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapDescriptorV1 {
    /// The complete pinned descriptor floor.
    pub floor: DescriptorFloor,
    /// The pinned HTTPS origin (<= 255 UTF-8 bytes).
    pub https_origin: String,
    /// The pinned roles (sorted set, <= 4).
    pub roles: Vec<EnabledRole>,
}

impl CanonicalRecord for BootstrapDescriptorV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        if self.https_origin.len() > MAX_HTTPS_ORIGIN_BYTES {
            return Err(CodecError::LengthOutOfRange);
        }
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(4).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
        }
        buf.extend_from_slice(&self.floor.encode_canonical()?);
        {
            let mut e = Encoder::new(&mut buf);
            e.str(&self.https_origin)
                .map_err(|_| CodecError::Malformed)?;
        }
        encode_role_set(&mut buf, &self.roles)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 4)?;
        read_version(d, 1)?;
        let floor = DescriptorFloor::decode_fields(d)?;
        let https_origin = read_text_max(d, 0, MAX_HTTPS_ORIGIN_BYTES)?;
        let roles = decode_role_set(d)?;
        Ok(BootstrapDescriptorV1 {
            floor,
            https_origin,
            roles,
        })
    }
}

/// `AnchorBootstrapV1` — the app-embedded fallback set: `[1, [BootstrapDescriptorV1...]]`.
///
/// At least [`MIN_BOOTSTRAP_DESCRIPTORS`] across [`MIN_BOOTSTRAP_OPERATORS`]
/// operators/failure domains; encoded list capped at
/// [`MAX_DESCRIPTOR_CHAIN_PAGE_ENVELOPES`] entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorBootstrapV1 {
    /// The pinned descriptors (ordered as embedded).
    pub descriptors: Vec<BootstrapDescriptorV1>,
}

impl AnchorBootstrapV1 {
    /// `true` iff at least 3 descriptors span at least 2 distinct operators
    /// (distinguished by genesis-independent current verification keys).
    pub fn meets_diversity_floor(&self) -> bool {
        if self.descriptors.len() < MIN_BOOTSTRAP_DESCRIPTORS {
            return false;
        }
        let mut keys: Vec<[u8; 32]> = self
            .descriptors
            .iter()
            .map(|d| d.floor.operator_verification_key.public_key)
            .collect();
        keys.sort();
        keys.dedup();
        keys.len() >= MIN_BOOTSTRAP_OPERATORS
    }
}

impl CanonicalRecord for AnchorBootstrapV1 {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        if self.descriptors.len() > MAX_DESCRIPTOR_CHAIN_PAGE_ENVELOPES {
            return Err(CodecError::LengthOutOfRange);
        }
        let mut buf = Vec::new();
        {
            let mut e = Encoder::new(&mut buf);
            e.array(2).map_err(|_| CodecError::Malformed)?;
            e.u64(1).map_err(|_| CodecError::Malformed)?;
            e.array(self.descriptors.len() as u64)
                .map_err(|_| CodecError::Malformed)?;
        }
        for descriptor in &self.descriptors {
            buf.extend_from_slice(&descriptor.encode_canonical()?);
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        expect_array(d, 2)?;
        read_version(d, 1)?;
        let count = definite_array(d)?;
        if count as usize > MAX_DESCRIPTOR_CHAIN_PAGE_ENVELOPES {
            return Err(CodecError::LengthOutOfRange);
        }
        let mut descriptors = Vec::with_capacity(count as usize);
        for _ in 0..count {
            descriptors.push(BootstrapDescriptorV1::decode_fields(d)?);
        }
        Ok(AnchorBootstrapV1 { descriptors })
    }
}
