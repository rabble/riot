//! The durable multi-community registry: the list of communities a person
//! holds, which one is active, and — per community — the sealed author that
//! lets them post there.
//!
//! # Why a per-community sealed author
//!
//! Both `generate_space_organizer_author` and
//! `generate_communal_author_for_namespace` mint FRESH RANDOM keypairs; neither
//! is re-derivable. Reusing one subspace key across communities would make the
//! same pseudonym linkable across every community a person joins — a privacy
//! regression for an activist tool. So each community keeps its OWN author, and
//! because those authors are random-and-non-re-derivable, each must be
//! persisted. The author is sealed with the profile wrapping key through the
//! EXISTING `EvidenceAuthor::seal_identity` mechanism (XChaCha20Poly1305, 64-byte
//! namespace+subspace plaintext) — this module invents no crypto of its own.
//!
//! A sealed author is genuinely un-loadable without both the wrapping key and a
//! deliberate switch: listing a community never unseals it. That is the isolation
//! property the registry exists to guarantee.
//!
//! # Corruption is quarantined, never dropped
//!
//! Two independent corruption cases, both preserved for recovery:
//!   * a single community's `sealed_author` fails to open (tampered / wrong key)
//!     → that record's `quarantined` flag is set, its bytes are RETAINED, and it
//!     is excluded from selection;
//!   * the whole registry blob fails to decode (a bad migration) → the raw bytes
//!     are copied to a quarantine key before anything overwrites them, so the
//!     data is never discarded.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

/// `local_state` key holding the CBOR-encoded registry.
pub(crate) const REGISTRY_KEY: &str = "community_registry/v1";
/// `local_state` key preserving a registry blob that failed to decode, so a bad
/// migration is quarantined for recovery rather than silently discarded.
pub(crate) const REGISTRY_QUARANTINE_KEY: &str = "community_registry_quarantine/v1";

const REGISTRY_VERSION: u8 = 1;
const RECORD_FIELDS: u64 = 9;

/// The person's relationship to a community, in plain product terms. Derived
/// from the sealed author, never caller-asserted: `Organizer` iff the author's
/// subspace equals the namespace; `Member` when they hold a distinct author in
/// the namespace; `PublicReader` when they carry the community but no author.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Relationship {
    Organizer,
    Member,
    PublicReader,
    /// A composite indymedia site the user follows (author-less; surfaced via
    /// `list_followed_sites`, filtered out of `list_communities`).
    Following,
    /// The user's own distinguished personal home space (author-bearing; rides
    /// `CommunityRow`/`list_communities`).
    Personal,
}

impl Relationship {
    fn to_wire(self) -> u8 {
        match self {
            Relationship::Organizer => 0,
            Relationship::Member => 1,
            Relationship::PublicReader => 2,
            Relationship::Following => 3,
            Relationship::Personal => 4,
        }
    }

    fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Relationship::Organizer),
            1 => Some(Relationship::Member),
            2 => Some(Relationship::PublicReader),
            3 => Some(Relationship::Following),
            4 => Some(Relationship::Personal),
            _ => None,
        }
    }
}

/// One held community. Metadata is cheap and persisted eagerly at create/join;
/// `sealed_author` is filled once the wrapping key is available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommunityRecord {
    pub namespace_id: [u8; 32],
    pub title: String,
    pub relationship: Relationship,
    /// Sealed per-community author, or `None` for a public-reader (no author) or
    /// a not-yet-persisted author still held only in the session's in-memory map.
    pub sealed_author: Option<Vec<u8>>,
    /// Pinned `SpaceDescriptorV1` EntryId — the handle a loaded/joined community's
    /// Home uses to reproject its newswire from the store (closes Risk 11).
    pub descriptor_entry_id: Option<[u8; 32]>,
    pub archived: bool,
    /// A record whose `sealed_author` failed to open is quarantined: retained for
    /// recovery, excluded from selection, never dropped.
    pub quarantined: bool,
    pub last_activity_unix_seconds: Option<u64>,
    pub last_sync_unix_seconds: Option<u64>,
}

/// The whole registry: every held community plus which one is active.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommunityRegistry {
    pub communities: Vec<CommunityRecord>,
    pub active: Option<[u8; 32]>,
}

/// A registry blob that could not be decoded. Carried out of `decode` so the
/// caller can quarantine the raw bytes before overwriting anything.
#[derive(Debug)]
pub(crate) struct RegistryCorrupt;

impl CommunityRegistry {
    pub(crate) fn find(&self, namespace_id: &[u8; 32]) -> Option<&CommunityRecord> {
        self.communities
            .iter()
            .find(|record| &record.namespace_id == namespace_id)
    }

    pub(crate) fn find_mut(&mut self, namespace_id: &[u8; 32]) -> Option<&mut CommunityRecord> {
        self.communities
            .iter_mut()
            .find(|record| &record.namespace_id == namespace_id)
    }

    /// Inserts a community or updates the mutable metadata of an existing one.
    /// Never clears a stored `sealed_author` or `descriptor_entry_id`: a later
    /// registration that lacks them (e.g. a metadata-only pass before the key is
    /// available) must not erase what an earlier pass already persisted.
    pub(crate) fn upsert(&mut self, incoming: CommunityRecord) {
        if let Some(existing) = self.find_mut(&incoming.namespace_id) {
            existing.title = incoming.title;
            existing.relationship = incoming.relationship;
            if incoming.sealed_author.is_some() {
                existing.sealed_author = incoming.sealed_author;
            }
            if incoming.descriptor_entry_id.is_some() {
                existing.descriptor_entry_id = incoming.descriptor_entry_id;
            }
            if incoming.last_activity_unix_seconds.is_some() {
                existing.last_activity_unix_seconds = incoming.last_activity_unix_seconds;
            }
            if incoming.last_sync_unix_seconds.is_some() {
                existing.last_sync_unix_seconds = incoming.last_sync_unix_seconds;
            }
        } else {
            self.communities.push(incoming);
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut e = Encoder::new(&mut buffer);
        // Encoding into a Vec is infallible; match the codebase's `expect` style.
        e.array(3).expect("vec encoder infallible");
        e.u8(REGISTRY_VERSION).expect("vec encoder infallible");
        encode_opt_bytes(&mut e, self.active.as_ref().map(|id| id.as_slice()));
        e.array(self.communities.len() as u64)
            .expect("vec encoder infallible");
        for record in &self.communities {
            encode_record(&mut e, record);
        }
        buffer
    }

    pub(crate) fn decode(input: &[u8]) -> Result<Self, RegistryCorrupt> {
        let mut d = Decoder::new(input);
        if d.array().map_err(|_| RegistryCorrupt)? != Some(3) {
            return Err(RegistryCorrupt);
        }
        if d.u8().map_err(|_| RegistryCorrupt)? != REGISTRY_VERSION {
            return Err(RegistryCorrupt);
        }
        let active = decode_opt_array32(&mut d)?;
        let count = d
            .array()
            .map_err(|_| RegistryCorrupt)?
            .ok_or(RegistryCorrupt)?;
        let mut communities = Vec::with_capacity(count as usize);
        for _ in 0..count {
            communities.push(decode_record(&mut d)?);
        }
        if d.position() != input.len() {
            return Err(RegistryCorrupt);
        }
        // An active pointer must name a held community; otherwise the blob is
        // internally inconsistent and treated as corrupt.
        if let Some(active) = active.as_ref() {
            if !communities.iter().any(|c| &c.namespace_id == active) {
                return Err(RegistryCorrupt);
            }
        }
        Ok(Self {
            communities,
            active,
        })
    }
}

fn encode_record(e: &mut Encoder<&mut Vec<u8>>, record: &CommunityRecord) {
    e.array(RECORD_FIELDS).expect("vec encoder infallible");
    e.bytes(&record.namespace_id)
        .expect("vec encoder infallible");
    e.str(&record.title).expect("vec encoder infallible");
    e.u8(record.relationship.to_wire())
        .expect("vec encoder infallible");
    encode_opt_bytes(e, record.sealed_author.as_deref());
    encode_opt_bytes(
        e,
        record.descriptor_entry_id.as_ref().map(|id| id.as_slice()),
    );
    e.bool(record.archived).expect("vec encoder infallible");
    e.bool(record.quarantined).expect("vec encoder infallible");
    encode_opt_u64(e, record.last_activity_unix_seconds);
    encode_opt_u64(e, record.last_sync_unix_seconds);
}

fn decode_record(d: &mut Decoder) -> Result<CommunityRecord, RegistryCorrupt> {
    if d.array().map_err(|_| RegistryCorrupt)? != Some(RECORD_FIELDS) {
        return Err(RegistryCorrupt);
    }
    let namespace_id = decode_array32(d)?;
    let title = d.str().map_err(|_| RegistryCorrupt)?.to_string();
    let relationship =
        Relationship::from_wire(d.u8().map_err(|_| RegistryCorrupt)?).ok_or(RegistryCorrupt)?;
    let sealed_author = decode_opt_bytes(d)?;
    let descriptor_entry_id = decode_opt_array32(d)?;
    let archived = d.bool().map_err(|_| RegistryCorrupt)?;
    let quarantined = d.bool().map_err(|_| RegistryCorrupt)?;
    let last_activity_unix_seconds = decode_opt_u64(d)?;
    let last_sync_unix_seconds = decode_opt_u64(d)?;
    Ok(CommunityRecord {
        namespace_id,
        title,
        relationship,
        sealed_author,
        descriptor_entry_id,
        archived,
        quarantined,
        last_activity_unix_seconds,
        last_sync_unix_seconds,
    })
}

fn encode_opt_bytes(e: &mut Encoder<&mut Vec<u8>>, value: Option<&[u8]>) {
    match value {
        Some(bytes) => {
            e.bytes(bytes).expect("vec encoder infallible");
        }
        None => {
            e.null().expect("vec encoder infallible");
        }
    }
}

fn encode_opt_u64(e: &mut Encoder<&mut Vec<u8>>, value: Option<u64>) {
    match value {
        Some(v) => {
            e.u64(v).expect("vec encoder infallible");
        }
        None => {
            e.null().expect("vec encoder infallible");
        }
    }
}

fn decode_opt_bytes(d: &mut Decoder) -> Result<Option<Vec<u8>>, RegistryCorrupt> {
    if d.datatype().map_err(|_| RegistryCorrupt)? == Type::Null {
        d.null().map_err(|_| RegistryCorrupt)?;
        return Ok(None);
    }
    Ok(Some(d.bytes().map_err(|_| RegistryCorrupt)?.to_vec()))
}

fn decode_array32(d: &mut Decoder) -> Result<[u8; 32], RegistryCorrupt> {
    d.bytes()
        .map_err(|_| RegistryCorrupt)?
        .try_into()
        .map_err(|_| RegistryCorrupt)
}

fn decode_opt_array32(d: &mut Decoder) -> Result<Option<[u8; 32]>, RegistryCorrupt> {
    if d.datatype().map_err(|_| RegistryCorrupt)? == Type::Null {
        d.null().map_err(|_| RegistryCorrupt)?;
        return Ok(None);
    }
    Ok(Some(decode_array32(d)?))
}

fn decode_opt_u64(d: &mut Decoder) -> Result<Option<u64>, RegistryCorrupt> {
    if d.datatype().map_err(|_| RegistryCorrupt)? == Type::Null {
        d.null().map_err(|_| RegistryCorrupt)?;
        return Ok(None);
    }
    Ok(Some(d.u64().map_err(|_| RegistryCorrupt)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(seed: u8) -> CommunityRecord {
        CommunityRecord {
            namespace_id: [seed; 32],
            title: format!("Community {seed}"),
            relationship: Relationship::Organizer,
            sealed_author: Some(vec![seed; 16]),
            descriptor_entry_id: Some([seed.wrapping_add(1); 32]),
            archived: false,
            quarantined: false,
            last_activity_unix_seconds: Some(1_000 + seed as u64),
            last_sync_unix_seconds: None,
        }
    }

    #[test]
    fn following_and_personal_round_trip_through_wire_without_a_version_bump() {
        for r in [
            Relationship::Organizer,
            Relationship::Member,
            Relationship::PublicReader,
            Relationship::Following,
            Relationship::Personal,
        ] {
            assert_eq!(Relationship::from_wire(r.to_wire()), Some(r));
        }
        assert_eq!(Relationship::from_wire(5), None);
        assert_eq!(REGISTRY_VERSION, 1);
        assert_eq!(RECORD_FIELDS, 9);
    }

    #[test]
    fn round_trips_a_populated_registry() {
        let registry = CommunityRegistry {
            communities: vec![record(1), {
                let mut r = record(2);
                r.relationship = Relationship::Member;
                r.sealed_author = None;
                r.descriptor_entry_id = None;
                r.archived = true;
                r.quarantined = true;
                r.last_sync_unix_seconds = Some(42);
                r
            }],
            active: Some([1; 32]),
        };
        let decoded = CommunityRegistry::decode(&registry.encode()).expect("round trip");
        assert_eq!(decoded, registry);
    }

    #[test]
    fn round_trips_an_empty_registry() {
        let registry = CommunityRegistry::default();
        let decoded = CommunityRegistry::decode(&registry.encode()).expect("round trip");
        assert_eq!(decoded, registry);
    }

    #[test]
    fn a_truncated_blob_is_corrupt_not_a_panic() {
        let bytes = CommunityRegistry {
            communities: vec![record(7)],
            active: Some([7; 32]),
        }
        .encode();
        for len in 0..bytes.len() {
            assert!(CommunityRegistry::decode(&bytes[..len]).is_err());
        }
    }

    #[test]
    fn trailing_garbage_is_corrupt() {
        let mut bytes = CommunityRegistry::default().encode();
        bytes.push(0xff);
        assert!(CommunityRegistry::decode(&bytes).is_err());
    }

    #[test]
    fn an_active_pointer_to_an_absent_community_is_corrupt() {
        let registry = CommunityRegistry {
            communities: vec![record(1)],
            active: Some([9; 32]),
        };
        // Encoding is structural, so this produces a syntactically valid blob
        // whose active pointer names no held community — decode must reject it.
        assert!(CommunityRegistry::decode(&registry.encode()).is_err());
    }

    #[test]
    fn upsert_updates_metadata_without_erasing_author_or_descriptor() {
        let mut registry = CommunityRegistry::default();
        registry.upsert(record(3));
        let mut metadata_only = record(3);
        metadata_only.title = "Renamed".into();
        metadata_only.sealed_author = None;
        metadata_only.descriptor_entry_id = None;
        registry.upsert(metadata_only);
        let stored = registry.find(&[3; 32]).expect("present");
        assert_eq!(stored.title, "Renamed");
        assert_eq!(stored.sealed_author, Some(vec![3; 16]));
        assert_eq!(stored.descriptor_entry_id, Some([4; 32]));
        assert_eq!(registry.communities.len(), 1);
    }

    #[test]
    fn a_structurally_valid_blob_with_a_wrong_header_is_corrupt() {
        // Outer arity other than three: syntactically fine CBOR, semantically not
        // a registry.
        let mut wrong_arity = Vec::new();
        let mut e = Encoder::new(&mut wrong_arity);
        e.array(2)
            .unwrap()
            .u8(REGISTRY_VERSION)
            .unwrap()
            .null()
            .unwrap();
        assert!(CommunityRegistry::decode(&wrong_arity).is_err());

        // Correct shape, unsupported version byte.
        let mut wrong_version = CommunityRegistry::default().encode();
        wrong_version[1] = REGISTRY_VERSION + 1;
        assert!(CommunityRegistry::decode(&wrong_version).is_err());
    }

    #[test]
    fn upsert_overwrites_optional_fields_when_the_incoming_record_supplies_them() {
        let mut registry = CommunityRegistry::default();
        let mut initial = record(3);
        initial.sealed_author = Some(vec![3; 8]);
        initial.descriptor_entry_id = Some([30; 32]);
        initial.last_activity_unix_seconds = Some(100);
        initial.last_sync_unix_seconds = Some(200);
        registry.upsert(initial);

        let mut refreshed = record(3);
        refreshed.title = "Refreshed".into();
        refreshed.relationship = Relationship::Member;
        refreshed.sealed_author = Some(vec![9; 12]);
        refreshed.descriptor_entry_id = Some([31; 32]);
        refreshed.last_activity_unix_seconds = Some(300);
        refreshed.last_sync_unix_seconds = Some(400);
        registry.upsert(refreshed);

        let stored = registry.find(&[3; 32]).expect("present");
        assert_eq!(stored.title, "Refreshed");
        assert_eq!(stored.relationship, Relationship::Member);
        assert_eq!(stored.sealed_author, Some(vec![9; 12]));
        assert_eq!(stored.descriptor_entry_id, Some([31; 32]));
        assert_eq!(stored.last_activity_unix_seconds, Some(300));
        assert_eq!(stored.last_sync_unix_seconds, Some(400));
        assert_eq!(registry.communities.len(), 1);
    }
}
