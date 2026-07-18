//! Owner-signed composite-site moderation records — the WRITE side of Unit 3.
//!
//! `site/moderation.rs` owns the pure schema + codec (and the read-side freshness
//! evaluation); this module signs a moderation record as the site owner at
//! `O:/mod/` and hands back the wire triple for import. It mirrors
//! `newswire/entry.rs::build_signed`: build the canonical payload, place it at a
//! collision-free `/mod/` path (time + digest, exactly like `newswire_path`), sign
//! it under the owner's `OwnedMasthead` capability, and expose the entry id.
//!
//! Only the site OWNER can produce these: signing requires the masthead secret
//! (`authorise_owner_entry`). A record authored under any non-owner capability is
//! dropped at admission (`admissible_capability` requires an owned cap rooted at
//! the followed site) — enforced and tested on the read/import side.

use crate::willow::site_paths::MOD_COMPONENT;
use crate::willow::{
    encode_capability, encode_entry, entry_id, william3_digest, ClockSnapshot, Entry, EntryId,
    OwnedMasthead, Path, SignedWillowEntry,
};

use super::moderation::{encode_moderation_record, ModerationRecord, ModerationRecordError};

/// Stable failure vocabulary for signing a moderation record. Dependency-specific
/// codec / crypto errors never cross this boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationSignError {
    /// The record failed canonical encoding or exceeded its byte ceiling.
    ModelInvalid,
    /// The `/mod/` path could not be formed (fixed-shape, so effectively unreachable).
    PathInvalid,
    /// The owner capability did not authorise the entry (never expected for an
    /// owner-signed `/mod/` entry, since `Area::full()` includes `/mod/`).
    SigningFailed,
}

impl std::fmt::Display for ModerationSignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ModerationSignError {}

/// A signed moderation record ready for import: the wire triple plus its willow
/// value identity (the record-id `mod_set_digest` commits to).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedModerationRecord {
    pub signed: SignedWillowEntry,
    pub entry_id: EntryId,
}

/// The `O:/mod/` path for a moderation record: `[mod, be64(tai_micros), digest]`.
/// Mirrors `newswire_path` (time + payload digest) — unique, time-sortable, and
/// compatible with the read side, which only requires the first component to be
/// `mod` (`is_under_mod`) and scans the `[mod]` prefix.
fn mod_record_path(
    tai_j2000_micros: u64,
    payload_digest: &[u8; 32],
) -> Result<Path, ModerationSignError> {
    let time = tai_j2000_micros.to_be_bytes();
    Path::from_slices(&[MOD_COMPONENT, &time, payload_digest])
        .map_err(|_| ModerationSignError::PathInvalid)
}

/// Sign `record` as the site owner at `O:/mod/`, timestamped by `snapshot`. The
/// Willow entry timestamp is `tai_j2000_micros`; a `ModEpoch`'s own `ts` field is
/// unix seconds (set by the caller in the record itself, for the freshness window).
pub fn create_signed_moderation_record(
    masthead: &OwnedMasthead,
    record: &ModerationRecord,
    snapshot: ClockSnapshot,
) -> Result<SignedModerationRecord, ModerationSignError> {
    let payload = encode_moderation_record(record).map_err(sign_model_error)?;
    let digest = william3_digest(&payload);
    let path = mod_record_path(snapshot.tai_j2000_micros, &digest)?;
    let entry = Entry::builder()
        .namespace_id(masthead.namespace_id().clone())
        .subspace_id(masthead.owner_subspace_id())
        .path(path)
        .timestamp(snapshot.tai_j2000_micros)
        .payload(&payload)
        .build();
    let authorised = masthead
        .authorise_owner_entry(entry)
        .map_err(|_| ModerationSignError::SigningFailed)?;
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let entry_bytes = encode_entry(authorised.entry());
    let id = entry_id(&entry_bytes);
    Ok(SignedModerationRecord {
        signed: SignedWillowEntry {
            entry_bytes,
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes: payload,
        },
        entry_id: id,
    })
}

fn sign_model_error(_error: ModerationRecordError) -> ModerationSignError {
    ModerationSignError::ModelInvalid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::moderation::{read_moderation_record, ModEpoch, Revoke, Tombstone};
    use crate::willow::{decode_capability_canonic, decode_entry_canonic};
    use willow25::prelude::*;

    fn snapshot(tai: u64) -> ClockSnapshot {
        ClockSnapshot {
            unix_seconds: 1_800_000_000,
            tai_j2000_micros: tai,
            uncertainty_seconds: 0,
        }
    }

    /// A signed moderation record is authored at `O:/mod/`, under an OWNED,
    /// zero-delegation capability rooted at the masthead namespace, and its payload
    /// reads back as the same record — the exact shape admission trusts.
    #[test]
    fn signs_under_the_owner_cap_at_a_mod_path() {
        let masthead = OwnedMasthead::generate().unwrap();
        let record = ModerationRecord::Revoke(Revoke {
            author_key: [0x42; 32],
            effective_ts: 1_800_000_000,
        });
        let signed = create_signed_moderation_record(&masthead, &record, snapshot(500)).unwrap();

        let entry = decode_entry_canonic(&signed.signed.entry_bytes).unwrap();
        // Owned namespace == the masthead root; author == the owner subspace.
        assert!(entry.namespace_id().is_owned());
        assert_eq!(
            entry.namespace_id().as_bytes(),
            masthead.namespace_id().as_bytes()
        );
        assert_eq!(
            entry.subspace_id().as_bytes(),
            masthead.owner_subspace_id().as_bytes()
        );
        // Path is under /mod/ and the Willow timestamp is the tai snapshot.
        assert_eq!(
            entry.path().components().next().unwrap().as_ref(),
            MOD_COMPONENT
        );
        assert_eq!(u64::from(entry.timestamp()), 500);

        // The capability is an owned, zero-delegation cap rooted at the masthead ns.
        let capability = decode_capability_canonic(&signed.signed.capability_bytes).unwrap();
        assert!(capability.is_owned());
        assert!(capability.delegations().is_empty());
        assert_eq!(
            capability.granted_namespace().as_bytes(),
            masthead.namespace_id().as_bytes()
        );

        // The payload round-trips through the read-side path guard as the same record.
        let read = read_moderation_record(entry.path(), &signed.signed.payload_bytes).unwrap();
        assert_eq!(read, record);
    }

    /// Each record kind signs and reads back, and the entry id is the willow value
    /// identity of the canonical entry bytes (what `mod_set_digest` commits to).
    #[test]
    fn every_record_kind_signs_and_the_entry_id_is_the_value_identity() {
        let masthead = OwnedMasthead::generate().unwrap();
        let records = [
            ModerationRecord::Revoke(Revoke {
                author_key: [1; 32],
                effective_ts: 10,
            }),
            ModerationRecord::Tombstone(Tombstone {
                target_ns: [2; 32],
                target_entry: [3; 32],
            }),
            ModerationRecord::ModEpoch(ModEpoch {
                seq: 1,
                ts: 1_800_000_000,
                mod_set_digest: [4; 32],
            }),
        ];
        for (index, record) in records.iter().enumerate() {
            let signed =
                create_signed_moderation_record(&masthead, record, snapshot(index as u64)).unwrap();
            assert_eq!(signed.entry_id, entry_id(&signed.signed.entry_bytes));
            let entry = decode_entry_canonic(&signed.signed.entry_bytes).unwrap();
            assert_eq!(
                read_moderation_record(entry.path(), &signed.signed.payload_bytes).unwrap(),
                *record
            );
        }
    }

    /// Two different masthead owners sign into DIFFERENT owned namespaces — a
    /// record is bound to the site whose masthead signed it, never another.
    #[test]
    fn a_record_is_bound_to_the_signing_masthead_namespace() {
        let a = OwnedMasthead::generate().unwrap();
        let b = OwnedMasthead::generate().unwrap();
        let record = ModerationRecord::Revoke(Revoke {
            author_key: [9; 32],
            effective_ts: 1,
        });
        let sa = create_signed_moderation_record(&a, &record, snapshot(1)).unwrap();
        let sb = create_signed_moderation_record(&b, &record, snapshot(1)).unwrap();
        let ea = decode_entry_canonic(&sa.signed.entry_bytes).unwrap();
        let eb = decode_entry_canonic(&sb.signed.entry_bytes).unwrap();
        assert_ne!(ea.namespace_id().as_bytes(), eb.namespace_id().as_bytes());
        assert_eq!(ea.namespace_id().as_bytes(), a.namespace_id().as_bytes());
        assert_eq!(eb.namespace_id().as_bytes(), b.namespace_id().as_bytes());
    }
}
