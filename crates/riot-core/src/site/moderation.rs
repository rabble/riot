//! Composite-site Unit 3 — owner-signed moderation record schema + strict
//! canonical CBOR codec for the records at `O:/mod/`.
//!
//! This module owns the *pure* schema + codec only, mirroring `site/manifest.rs`:
//! deterministic encode, byte-identical decode (`prove_canonical`), definite
//! lengths, strictly-ordered integer keys, exact-32 id fields, a closed failure
//! vocabulary, and a MAX byte ceiling. It never touches willow25, signing, the
//! store, or admission — those are Tasks 2/4 and sibling modules.
//!
//! Four owner-signed record kinds share one tagged envelope (`ModerationRecord`),
//! discriminated by a CLOSED `kind` code (an unknown kind is a hard reject, never
//! a silently-ignored record):
//!
//! - `Revoke { author_key, effective_ts }` — ban an author-key.
//! - `Tombstone { target_ns, target_entry }` — hide a specific entry.
//! - `ModEpoch { seq, ts, mod_set_digest }` — the freshness heartbeat.
//! - `Endorse { author_key }` — the re-endorse allow-list (see below).
//!
//! ## Re-endorse allow-list representation (plan Task 1, §8.1 case 8)
//!
//! The re-endorse allow-list is represented as its own **owner-signed record
//! type** (`Endorse`), NOT as a field on the manifest or on the `mod_epoch`. This
//! mirrors `Revoke` (a positive, individually-signed statement about one
//! author-key) and is chosen over a manifest field for three reasons: (1) the
//! manifest (`SiteManifestV1`) is a frozen v1 binding record — growing a
//! variable-length allow-list field per selective un-hide would churn the manifest
//! version and the durable version floor on every moderation decision; (2) an
//! endorse is authored at `O:/mod/` alongside the revoke it counteracts, so it
//! rides the exact same admission + freshness path (a field would live on a
//! different record with different freshness semantics); (3) selective un-hide
//! (keep pre-ban good work after banning an author, §4.3) is naturally a *positive
//! signed signal* the resolver overlays — the same shape as revoke/tombstone.
//! Semantics (endorse-overrides-revoke for the allow-listed author's pre-ban
//! entries) are applied in Task 5's freshness/exemption evaluation; this module
//! only defines and (de)serializes the record.

use minicbor::{Decoder, Encoder};

/// Frozen moderation-record schema tag (envelope top-level key 0).
pub const MODERATION_RECORD_SCHEMA: &str = "org.riot.site.moderation/1";

/// Largest accepted moderation-record encoding. These are tiny fixed-shape
/// binding records (a handful of 32-byte ids and uints), so the ceiling is small;
/// it keeps a hostile peer from spending unbounded decode work.
pub const MAX_MODERATION_RECORD_BYTES: usize = 512;

/// A revocation of an author-key. The guarantee rests on identity at render, not
/// on `effective_ts` (an attacker can backdate content) — the timestamp is
/// advisory ordering metadata, the author-key is the ban.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revoke {
    /// The author subspace/public key being revoked (32-byte willow key).
    pub author_key: [u8; 32],
    /// Advisory effective timestamp (ordering metadata, not the security lever).
    pub effective_ts: u64,
}

/// A tombstone hiding one specific entry by its `(namespace, entry-id)` identity.
/// `target_entry` is a willow entry-id (`crate::willow::EntryId`, a 32-byte value
/// identity — see `willow/digest.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tombstone {
    /// The namespace the tombstoned entry lives in (32-byte willow namespace key).
    pub target_ns: [u8; 32],
    /// The value identity of the entry to hide (willow `EntryId`, 32 bytes).
    pub target_entry: [u8; 32],
}

/// The moderation freshness heartbeat (`mod_epoch`). `moderation-current` is a
/// POSITIVE signed signal derived from these; Task 5 evaluates freshness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEpoch {
    /// Monotonic heartbeat sequence; a visible gap ⇒ `moderation-loading`.
    pub seq: u64,
    /// Heartbeat wall-clock; outside the freshness window ⇒ `moderation-loading`.
    pub ts: u64,
    /// Commitment to the revoke+tombstone record-ids `≤ seq`. Detection is
    /// recompute-over-held-and-compare (Task 5), never name-enumeration.
    pub mod_set_digest: [u8; 32],
}

/// A re-endorse of an author-key: selective un-hide of that author's pre-ban
/// entries (the allow-list; see the module doc for why this is a record type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endorse {
    /// The author subspace/public key being re-endorsed (32-byte willow key).
    pub author_key: [u8; 32],
}

/// The tagged moderation-record envelope. One codec, four kinds, discriminated by
/// a CLOSED `kind` code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationRecord {
    Revoke(Revoke),
    Tombstone(Tombstone),
    ModEpoch(ModEpoch),
    Endorse(Endorse),
}

/// Internal closed `kind` discriminant (envelope key 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModKind {
    Revoke,
    Tombstone,
    ModEpoch,
    Endorse,
}

impl ModKind {
    fn to_code(self) -> u64 {
        match self {
            ModKind::Revoke => 0,
            ModKind::Tombstone => 1,
            ModKind::ModEpoch => 2,
            ModKind::Endorse => 3,
        }
    }
    fn from_code(code: u64) -> Result<Self, ModerationRecordError> {
        match code {
            0 => Ok(ModKind::Revoke),
            1 => Ok(ModKind::Tombstone),
            2 => Ok(ModKind::ModEpoch),
            3 => Ok(ModKind::Endorse),
            _ => Err(ModerationRecordError::InvalidEnum("kind")),
        }
    }
}

impl ModerationRecord {
    fn kind(&self) -> ModKind {
        match self {
            ModerationRecord::Revoke(_) => ModKind::Revoke,
            ModerationRecord::Tombstone(_) => ModKind::Tombstone,
            ModerationRecord::ModEpoch(_) => ModKind::ModEpoch,
            ModerationRecord::Endorse(_) => ModKind::Endorse,
        }
    }
}

/// Stable, closed failure vocabulary for moderation-record encode/decode.
/// Mirrors `SiteManifestError` (`site/manifest.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationRecordError {
    InputTooLarge,
    UnknownKey(u64),
    DuplicateOrMisorderedKey(u64),
    MissingKey(u64),
    WrongSchema,
    InvalidEnum(&'static str),
    NonCanonical,
    TrailingBytes,
    Malformed,
}

impl std::fmt::Display for ModerationRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ModerationRecordError {}

// ---------- encode ----------

/// Encode a moderation record to canonical CBOR. Envelope is a definite map(3):
/// `0 => schema`, `1 => kind`, `2 => body` (a kind-specific definite map with
/// strictly-ordered integer keys).
pub fn encode_moderation_record(
    record: &ModerationRecord,
) -> Result<Vec<u8>, ModerationRecordError> {
    encode_bounded(|e| {
        e.map(3)?;
        e.u8(0)?.str(MODERATION_RECORD_SCHEMA)?;
        e.u8(1)?.u64(record.kind().to_code())?;
        e.u8(2)?;
        match record {
            ModerationRecord::Revoke(revoke) => {
                e.map(2)?;
                e.u8(0)?.bytes(&revoke.author_key)?;
                e.u8(1)?.u64(revoke.effective_ts)?;
            }
            ModerationRecord::Tombstone(tombstone) => {
                e.map(2)?;
                e.u8(0)?.bytes(&tombstone.target_ns)?;
                e.u8(1)?.bytes(&tombstone.target_entry)?;
            }
            ModerationRecord::ModEpoch(epoch) => {
                e.map(3)?;
                e.u8(0)?.u64(epoch.seq)?;
                e.u8(1)?.u64(epoch.ts)?;
                e.u8(2)?.bytes(&epoch.mod_set_digest)?;
            }
            ModerationRecord::Endorse(endorse) => {
                e.map(1)?;
                e.u8(0)?.bytes(&endorse.author_key)?;
            }
        }
        Ok(())
    })
}

// ---------- decode ----------

/// Decode canonical CBOR to a moderation record. Rejects oversized input, any
/// non-canonical (indefinite) container, misordered/duplicate/unknown keys, a
/// wrong schema tag, an unknown `kind`, wrong-length id fields, and trailing
/// bytes; re-encodes the result and requires it to be byte-identical.
pub fn decode_moderation_record(input: &[u8]) -> Result<ModerationRecord, ModerationRecordError> {
    check_input_size(input)?;
    let mut d = Decoder::new(input);
    let pairs = definite_map(&mut d)?;
    if pairs > 3 {
        return Err(ModerationRecordError::Malformed);
    }

    let mut schema = None;
    let mut kind = None;
    let mut record = None;
    let mut last_key = None;

    for _ in 0..pairs {
        let key = decode_ordered_key(&mut d, &mut last_key)?;
        match key {
            0 => schema = Some(decode_schema(&mut d)?),
            1 => kind = Some(ModKind::from_code(decode_u64(&mut d)?)?),
            2 => {
                let resolved = kind.ok_or(ModerationRecordError::MissingKey(1))?;
                record = Some(decode_body(&mut d, resolved)?);
            }
            other => return Err(ModerationRecordError::UnknownKey(other)),
        }
    }
    finish_input(&d, input)?;
    if schema.as_deref() != Some(MODERATION_RECORD_SCHEMA) {
        return Err(ModerationRecordError::WrongSchema);
    }
    let record = record.ok_or(ModerationRecordError::MissingKey(2))?;
    prove_canonical(input, encode_moderation_record(&record)?)?;
    Ok(record)
}

fn decode_body(
    d: &mut Decoder<'_>,
    kind: ModKind,
) -> Result<ModerationRecord, ModerationRecordError> {
    match kind {
        ModKind::Revoke => {
            if definite_map(d)? != 2 {
                return Err(ModerationRecordError::Malformed);
            }
            let mut author_key = None;
            let mut effective_ts = None;
            let mut last_key = None;
            for _ in 0..2 {
                match decode_ordered_key(d, &mut last_key)? {
                    0 => author_key = Some(decode_id32(d)?),
                    1 => effective_ts = Some(decode_u64(d)?),
                    other => return Err(ModerationRecordError::UnknownKey(other)),
                }
            }
            Ok(ModerationRecord::Revoke(Revoke {
                author_key: author_key.ok_or(ModerationRecordError::MissingKey(0))?,
                effective_ts: effective_ts.ok_or(ModerationRecordError::MissingKey(1))?,
            }))
        }
        ModKind::Tombstone => {
            if definite_map(d)? != 2 {
                return Err(ModerationRecordError::Malformed);
            }
            let mut target_ns = None;
            let mut target_entry = None;
            let mut last_key = None;
            for _ in 0..2 {
                match decode_ordered_key(d, &mut last_key)? {
                    0 => target_ns = Some(decode_id32(d)?),
                    1 => target_entry = Some(decode_id32(d)?),
                    other => return Err(ModerationRecordError::UnknownKey(other)),
                }
            }
            Ok(ModerationRecord::Tombstone(Tombstone {
                target_ns: target_ns.ok_or(ModerationRecordError::MissingKey(0))?,
                target_entry: target_entry.ok_or(ModerationRecordError::MissingKey(1))?,
            }))
        }
        ModKind::ModEpoch => {
            if definite_map(d)? != 3 {
                return Err(ModerationRecordError::Malformed);
            }
            let mut seq = None;
            let mut ts = None;
            let mut mod_set_digest = None;
            let mut last_key = None;
            for _ in 0..3 {
                match decode_ordered_key(d, &mut last_key)? {
                    0 => seq = Some(decode_u64(d)?),
                    1 => ts = Some(decode_u64(d)?),
                    2 => mod_set_digest = Some(decode_id32(d)?),
                    other => return Err(ModerationRecordError::UnknownKey(other)),
                }
            }
            Ok(ModerationRecord::ModEpoch(ModEpoch {
                seq: seq.ok_or(ModerationRecordError::MissingKey(0))?,
                ts: ts.ok_or(ModerationRecordError::MissingKey(1))?,
                mod_set_digest: mod_set_digest.ok_or(ModerationRecordError::MissingKey(2))?,
            }))
        }
        ModKind::Endorse => {
            if definite_map(d)? != 1 {
                return Err(ModerationRecordError::Malformed);
            }
            let mut author_key = None;
            let mut last_key = None;
            for _ in 0..1 {
                match decode_ordered_key(d, &mut last_key)? {
                    0 => author_key = Some(decode_id32(d)?),
                    other => return Err(ModerationRecordError::UnknownKey(other)),
                }
            }
            Ok(ModerationRecord::Endorse(Endorse {
                author_key: author_key.ok_or(ModerationRecordError::MissingKey(0))?,
            }))
        }
    }
}

// ---------- shared codec primitives (mirror site/manifest.rs) ----------

fn encode_bounded<F>(encode: F) -> Result<Vec<u8>, ModerationRecordError>
where
    F: FnOnce(
        &mut Encoder<&mut Vec<u8>>,
    ) -> Result<(), minicbor::encode::Error<core::convert::Infallible>>,
{
    let mut buffer = Vec::new();
    let mut encoder = Encoder::new(&mut buffer);
    encode(&mut encoder).map_err(|_| ModerationRecordError::Malformed)?;
    if buffer.len() > MAX_MODERATION_RECORD_BYTES {
        return Err(ModerationRecordError::InputTooLarge);
    }
    Ok(buffer)
}

fn check_input_size(input: &[u8]) -> Result<(), ModerationRecordError> {
    if input.len() > MAX_MODERATION_RECORD_BYTES {
        Err(ModerationRecordError::InputTooLarge)
    } else {
        Ok(())
    }
}

fn definite_map(d: &mut Decoder<'_>) -> Result<u64, ModerationRecordError> {
    d.map()
        .map_err(|_| ModerationRecordError::Malformed)?
        .ok_or(ModerationRecordError::NonCanonical)
}

fn decode_ordered_key(
    d: &mut Decoder<'_>,
    last_key: &mut Option<u64>,
) -> Result<u64, ModerationRecordError> {
    let key = d.u64().map_err(|_| ModerationRecordError::Malformed)?;
    if last_key.is_some_and(|previous| key <= previous) {
        return Err(ModerationRecordError::DuplicateOrMisorderedKey(key));
    }
    *last_key = Some(key);
    Ok(key)
}

fn decode_schema(d: &mut Decoder<'_>) -> Result<String, ModerationRecordError> {
    if d.datatype().map_err(|_| ModerationRecordError::Malformed)? != minicbor::data::Type::String {
        return Err(ModerationRecordError::Malformed);
    }
    let value = d.str().map_err(|_| ModerationRecordError::Malformed)?;
    if value.len() > 64 {
        return Err(ModerationRecordError::Malformed);
    }
    Ok(value.to_string())
}

fn decode_id32(d: &mut Decoder<'_>) -> Result<[u8; 32], ModerationRecordError> {
    let bytes = d.bytes().map_err(|_| ModerationRecordError::Malformed)?;
    <[u8; 32]>::try_from(bytes).map_err(|_| ModerationRecordError::Malformed)
}

fn decode_u64(d: &mut Decoder<'_>) -> Result<u64, ModerationRecordError> {
    d.u64().map_err(|_| ModerationRecordError::Malformed)
}

fn finish_input(d: &Decoder<'_>, input: &[u8]) -> Result<(), ModerationRecordError> {
    if d.position() == input.len() {
        Ok(())
    } else {
        Err(ModerationRecordError::TrailingBytes)
    }
}

fn prove_canonical(input: &[u8], encoded: Vec<u8>) -> Result<(), ModerationRecordError> {
    if encoded == input {
        Ok(())
    } else {
        Err(ModerationRecordError::NonCanonical)
    }
}
