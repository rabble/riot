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

use crate::willow::site_paths::is_under_mod;
use crate::willow::{william3_digest, Path};
use minicbor::{Decoder, Encoder};
use std::collections::BTreeSet;

/// How stale a heartbeat may be before `/mod/` is no longer "current". A client
/// whose latest `mod_epoch` is older than this holds the open namespaces as
/// `moderation-loading` rather than falsely rendering unmoderated content.
pub const MODERATION_FRESHNESS_WINDOW_SECS: u64 = 24 * 60 * 60;

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
    /// The entry carrying a moderation payload is not under `O:/mod/`. A
    /// moderation record is only meaningful at a `/mod/` path; a record body at
    /// `/articles/` or `/manifest` is refused (read-side path guard, Task 2).
    NotUnderMod,
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

/// Read a moderation record from an owner-signed entry's `(path, payload)`.
///
/// The signature over the entry is willow25's concern (verified upstream at
/// admission via `verify_entry` / `authorise_owner_entry`); this is the Riot-side
/// **path guard**: a moderation record is only meaningful at `O:/mod/`, so a
/// record body carried at `/articles/` or `/manifest` is refused
/// (`NotUnderMod`) BEFORE the payload is trusted as moderation. Owner signing
/// itself uses `OwnedMasthead::authorise_owner_entry` with an entry built at a
/// `/mod/` path (Task 2 tests exercise the full sign → verify → read round-trip).
pub fn read_moderation_record(
    path: &Path,
    payload: &[u8],
) -> Result<ModerationRecord, ModerationRecordError> {
    if !is_under_mod(path) {
        return Err(ModerationRecordError::NotUnderMod);
    }
    decode_moderation_record(payload)
}

// ---------- Task 5: freshness evaluation + exemption filtering ----------

/// A held moderation record paired with its willow value-identity (record-id).
/// The id is what `mod_set_digest` commits to; the caller supplies it from the
/// synced entry (`entry_id`), so this module stays free of willow entry types.
#[derive(Debug, Clone)]
pub struct HeldModerationRecord {
    pub record: ModerationRecord,
    pub record_id: [u8; 32],
}

/// Why `/mod/` is not yet current. Every variant means the resolver holds the
/// open namespaces as `moderation-loading` — NEVER a false "current".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationLoading {
    /// No `mod_epoch` heartbeat held at all — freshness is a positive signal, so
    /// its absence is loading, not "current-and-empty".
    NoHeartbeat,
    /// The latest heartbeat's `ts` is older than the freshness window.
    StaleHeartbeat,
    /// A gap in the held heartbeat `seq` sequence — a heartbeat is missing.
    SeqGap,
    /// The recomputed digest over the held revoke+tombstone record-ids does not
    /// match the heartbeat's `mod_set_digest` — the client is missing (or has
    /// extra) records the owner committed to. This is what detects tail
    /// suppression (a withheld latest revoke shows no `seq` gap).
    DigestMismatch,
}

/// The resolved `/mod/` freshness verdict. `Current` carries the exemption-FILTERED
/// sets the resolver (Unit 4) overlays directly: `revoke{root}` and
/// tombstones of protected (manifest/root/owner) entries are already removed, so
/// a rogue/seized moderator cannot brick the site through the overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationFreshness {
    Current {
        revoked: BTreeSet<[u8; 32]>,
        tombstoned: BTreeSet<[u8; 32]>,
        endorsed: BTreeSet<[u8; 32]>,
    },
    Loading(ModerationLoading),
}

/// Recompute `mod_set_digest` over a set of revoke+tombstone record-ids, the
/// recompute-and-compare form (a one-way hash cannot enumerate missing names;
/// it can only reveal the held set differs). `BTreeSet` gives the canonical
/// sorted order, so owner and client agree byte-for-byte.
pub fn compute_mod_set_digest(record_ids: &BTreeSet<[u8; 32]>) -> [u8; 32] {
    let mut buf = Vec::with_capacity(record_ids.len() * 32);
    for id in record_ids {
        buf.extend_from_slice(id);
    }
    william3_digest(&buf)
}

/// Evaluate whether `/mod/` is current, and if so emit the exemption-filtered
/// revoked/tombstoned/endorsed sets (§4.3 + plan §2 invariants 1, 3, 4).
///
/// `manifest_root` exempts `revoke{author_key == root}` (a moderator cannot
/// revoke the owner). `protected_entry_ids` (the manifest entry-id + owner
/// entry-ids, supplied by the resolver which knows entry identities) exempts
/// `tombstone{target ∈ protected}` (a moderator cannot tombstone `/manifest`
/// or owner records). Both exemptions are applied HERE so the overlay never sees
/// a brick-the-site record.
pub fn evaluate_freshness(
    held: &[HeldModerationRecord],
    manifest_root: [u8; 32],
    protected_entry_ids: &BTreeSet<[u8; 32]>,
    now: u64,
) -> ModerationFreshness {
    // 1. Latest heartbeat. Absence ⇒ loading (freshness is a positive signal).
    let mut epochs: Vec<&ModEpoch> = held
        .iter()
        .filter_map(|h| match &h.record {
            ModerationRecord::ModEpoch(e) => Some(e),
            _ => None,
        })
        .collect();
    epochs.sort_by_key(|e| e.seq);
    let Some(latest) = epochs.last().copied() else {
        return ModerationFreshness::Loading(ModerationLoading::NoHeartbeat);
    };

    // 2. Freshness window.
    if now.saturating_sub(latest.ts) > MODERATION_FRESHNESS_WINDOW_SECS {
        return ModerationFreshness::Loading(ModerationLoading::StaleHeartbeat);
    }

    // 3. Seq contiguity — a hole in the held heartbeat sequence is a missing
    //    heartbeat. (Missing revoke/tombstone RECORDS are caught by the digest;
    //    this catches missing HEARTBEATS.)
    for pair in epochs.windows(2) {
        if pair[1].seq != pair[0].seq + 1 {
            return ModerationFreshness::Loading(ModerationLoading::SeqGap);
        }
    }

    // 4. Digest recompute-and-compare over held revoke+tombstone ids.
    let held_mod_ids: BTreeSet<[u8; 32]> = held
        .iter()
        .filter(|h| {
            matches!(
                h.record,
                ModerationRecord::Revoke(_) | ModerationRecord::Tombstone(_)
            )
        })
        .map(|h| h.record_id)
        .collect();
    if compute_mod_set_digest(&held_mod_ids) != latest.mod_set_digest {
        return ModerationFreshness::Loading(ModerationLoading::DigestMismatch);
    }

    // 5. Current — build exemption-filtered sets. The guarantee rests on
    //    identity at render, not the clock, so a backdated revoke still hides.
    let mut revoked = BTreeSet::new();
    let mut tombstoned = BTreeSet::new();
    let mut endorsed = BTreeSet::new();
    for h in held {
        match &h.record {
            // Root is exempt: a moderator can never revoke the owner.
            ModerationRecord::Revoke(r) if r.author_key != manifest_root => {
                revoked.insert(r.author_key);
            }
            ModerationRecord::Revoke(_) => {}
            // Protected entries (manifest / owner) are exempt from tombstone.
            ModerationRecord::Tombstone(t) if !protected_entry_ids.contains(&t.target_entry) => {
                tombstoned.insert(t.target_entry);
            }
            ModerationRecord::Tombstone(_) => {}
            ModerationRecord::Endorse(e) => {
                endorsed.insert(e.author_key);
            }
            ModerationRecord::ModEpoch(_) => {}
        }
    }
    ModerationFreshness::Current {
        revoked,
        tombstoned,
        endorsed,
    }
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
