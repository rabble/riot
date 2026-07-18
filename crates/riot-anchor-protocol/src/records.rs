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
    read_discriminant, read_fixed_bytes, read_null, read_text_max, sort_canonical_set,
    CanonicalRecord, CodecError,
};
use crate::digest::{digest_v1, label};

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
