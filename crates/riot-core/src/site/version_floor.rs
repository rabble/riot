//! Durable monotonic version floor for the site manifest (§5.2).
//!
//! Willow last-writer-wins only protects same-coordinate writes, so anti-rollback
//! is Riot-side: the client persists, per site root, the **highest manifest
//! `version` seen** and the **strictest `require` transport floor seen**, and:
//!
//! - refuses any manifest whose `version` is **below** the durable floor
//!   (rollback), and
//! - refuses a **higher**-version manifest that lowers `require` **below** the
//!   durable floor (require-monotonicity — a distinct attack: it passes the
//!   version check yet strips privacy), and
//! - raises an **equivocation alarm** on two *conflicting* owner signatures at
//!   the **same** version (a compromise signal), never a silent pick.
//!
//! The floor MUST survive an app restart — a memory-only floor re-opens rollback
//! on relaunch. The logic is written against the [`VersionFloorStore`] trait so
//! the pure decision is unit-testable without a database; the durable
//! implementation is the SQLite `local_state` KV (see the `RiotDatabase` impl).

use super::manifest::{encode_site_manifest, SiteManifestV1};
use crate::willow::william3_digest;

/// Durable key/value backing for the per-root version floor. The production
/// implementation is the SQLite `local_state` table; tests use an in-memory map.
pub trait VersionFloorStore {
    /// Store-specific error (e.g. `DatabaseError`).
    type Error;
    /// Read the persisted floor bytes for `key`, or `None` if unseen.
    fn floor_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error>;
    /// Persist `value` for `key`, overwriting any prior floor.
    fn set_floor_bytes(&self, key: &str, value: &[u8]) -> Result<(), Self::Error>;
}

/// The verdict of admitting a validated manifest against the durable floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionFloorOutcome {
    /// Accepted; the durable floor was advanced or (re-seeing the same manifest)
    /// left unchanged.
    Accepted,
    /// Refused: `version` is below the durable floor (rollback).
    RollbackRejected,
    /// Refused: a higher `version` that lowers `require` below the durable floor.
    RequireDowngradeRejected,
    /// Refused with a compromise alarm: a conflicting owner signature at the
    /// same `version` as the durable floor (equivocation).
    EquivocationAlarm,
}

/// Failure modes for floor admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionFloorError<E> {
    /// The underlying store failed.
    Store(E),
    /// The persisted floor bytes are corrupt (wrong length or tag).
    CorruptFloor,
    /// The (already-validated) manifest failed to re-encode for its identity.
    ManifestEncode,
}

/// Byte layout tag for the persisted floor record; bumped if the layout changes.
const FLOOR_TAG: u8 = 1;
/// tag(1) ‖ version(8 BE) ‖ require_strictness(1) ‖ identity(32).
const FLOOR_BYTES: usize = 1 + 8 + 1 + 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Floor {
    version: u64,
    require_strictness: u8,
    identity: [u8; 32],
}

impl Floor {
    fn encode(&self) -> [u8; FLOOR_BYTES] {
        let mut out = [0u8; FLOOR_BYTES];
        out[0] = FLOOR_TAG;
        out[1..9].copy_from_slice(&self.version.to_be_bytes());
        out[9] = self.require_strictness;
        out[10..].copy_from_slice(&self.identity);
        out
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != FLOOR_BYTES || bytes[0] != FLOOR_TAG {
            return None;
        }
        let version = u64::from_be_bytes(bytes[1..9].try_into().ok()?);
        let require_strictness = bytes[9];
        let identity = <[u8; 32]>::try_from(&bytes[10..]).ok()?;
        Some(Self {
            version,
            require_strictness,
            identity,
        })
    }
}

/// The durable-store key for a site root (hex of the 32-byte root, prefixed).
fn floor_key(site_root: &[u8; 32]) -> String {
    let mut key = String::with_capacity(12 + 64);
    key.push_str("site/vfloor/");
    for byte in site_root {
        key.push(char::from_digit((byte >> 4) as u32, 16).expect("nibble"));
        key.push(char::from_digit((byte & 0x0f) as u32, 16).expect("nibble"));
    }
    key
}

/// A content identity for the signed manifest at its version — the digest of the
/// canonical encoding. willow25's ed25519 signatures are deterministic, so a
/// *different* owner signature at the same version implies different content and
/// thus a different digest; this faithfully detects equivocation.
fn manifest_identity<E>(manifest: &SiteManifestV1) -> Result<[u8; 32], VersionFloorError<E>> {
    let bytes = encode_site_manifest(manifest).map_err(|_| VersionFloorError::ManifestEncode)?;
    Ok(william3_digest(&bytes))
}

/// Admit a **validated** manifest against the durable per-root floor, persisting
/// an advance on acceptance. Rejections and the equivocation alarm never mutate
/// the floor, so a rollback stays refused across restarts.
pub fn admit_manifest_version<S: VersionFloorStore>(
    store: &S,
    site_root: &[u8; 32],
    manifest: &SiteManifestV1,
) -> Result<VersionFloorOutcome, VersionFloorError<S::Error>> {
    let key = floor_key(site_root);
    let incoming = Floor {
        version: manifest.version,
        require_strictness: manifest.transport_policy.require.strictness(),
        identity: manifest_identity(manifest)?,
    };

    let existing = match store.floor_bytes(&key).map_err(VersionFloorError::Store)? {
        Some(bytes) => Some(Floor::decode(&bytes).ok_or(VersionFloorError::CorruptFloor)?),
        None => None,
    };

    let Some(floor) = existing else {
        // First sight of this site: seed the floor.
        persist(store, &key, &incoming)?;
        return Ok(VersionFloorOutcome::Accepted);
    };

    if incoming.version < floor.version {
        return Ok(VersionFloorOutcome::RollbackRejected);
    }

    if incoming.version == floor.version {
        if incoming.identity != floor.identity {
            // Two conflicting owner signatures at the same version.
            return Ok(VersionFloorOutcome::EquivocationAlarm);
        }
        // Re-seeing the same manifest is idempotent; the floor already holds it.
        return Ok(VersionFloorOutcome::Accepted);
    }

    // incoming.version > floor.version: require may never drop below the floor.
    if incoming.require_strictness < floor.require_strictness {
        return Ok(VersionFloorOutcome::RequireDowngradeRejected);
    }

    persist(store, &key, &incoming)?;
    Ok(VersionFloorOutcome::Accepted)
}

fn persist<S: VersionFloorStore>(
    store: &S,
    key: &str,
    floor: &Floor,
) -> Result<(), VersionFloorError<S::Error>> {
    store
        .set_floor_bytes(key, &floor.encode())
        .map_err(VersionFloorError::Store)
}

#[cfg(feature = "sqlite")]
impl VersionFloorStore for crate::store::RiotDatabase {
    type Error = crate::store::DatabaseError;

    fn floor_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        self.local_state(key)
    }

    fn set_floor_bytes(&self, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        self.set_local_state(key, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::manifest::{
        RequireTransport, SiteDisplay, SiteLayout, SiteMemberV1, SiteRole, SiteRule,
        TransportPolicyV1,
    };
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::convert::Infallible;

    /// An in-memory floor store for the pure decision logic (no database).
    #[derive(Default)]
    struct MemoryStore {
        map: RefCell<HashMap<String, Vec<u8>>>,
    }

    impl VersionFloorStore for MemoryStore {
        type Error = Infallible;
        fn floor_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
            Ok(self.map.borrow().get(key).cloned())
        }
        fn set_floor_bytes(&self, key: &str, value: &[u8]) -> Result<(), Self::Error> {
            self.map
                .borrow_mut()
                .insert(key.to_string(), value.to_vec());
            Ok(())
        }
    }

    const ROOT: [u8; 32] = [0x42; 32];

    fn manifest(version: u64, require: RequireTransport) -> SiteManifestV1 {
        SiteManifestV1 {
            root: ROOT,
            members: vec![SiteMemberV1 {
                ns: ROOT,
                role: SiteRole::Masthead,
                rule: SiteRule::OwnedWrite,
                display: SiteDisplay::FrontArticles,
            }],
            moderation_path: vec![b"mod".to_vec()],
            transport_policy: TransportPolicyV1 {
                allow: vec![],
                require,
            },
            version,
            layout: SiteLayout::SiteDefault,
            sections: vec![],
        }
    }

    #[test]
    fn first_sight_seeds_the_floor() {
        let store = MemoryStore::default();
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &manifest(5, RequireTransport::None)),
            Ok(VersionFloorOutcome::Accepted)
        );
    }

    #[test]
    fn a_lower_version_is_rejected_as_rollback() {
        let store = MemoryStore::default();
        admit_manifest_version(&store, &ROOT, &manifest(5, RequireTransport::None)).unwrap();
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &manifest(4, RequireTransport::None)),
            Ok(VersionFloorOutcome::RollbackRejected)
        );
    }

    #[test]
    fn a_higher_version_with_equal_or_stricter_require_advances_the_floor() {
        let store = MemoryStore::default();
        admit_manifest_version(&store, &ROOT, &manifest(5, RequireTransport::None)).unwrap();
        // higher version, stricter require -> accept and advance.
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &manifest(6, RequireTransport::Arti)),
            Ok(VersionFloorOutcome::Accepted)
        );
        // now version 6 is the floor; version 5 is a rollback.
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &manifest(5, RequireTransport::Arti)),
            Ok(VersionFloorOutcome::RollbackRejected)
        );
    }

    #[test]
    fn a_higher_version_that_lowers_require_is_rejected() {
        let store = MemoryStore::default();
        // durable floor at v5 with require=arti (strictness 1).
        admit_manifest_version(&store, &ROOT, &manifest(5, RequireTransport::Arti)).unwrap();
        // v6 passes the version check but lowers require arti -> none: refuse.
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &manifest(6, RequireTransport::None)),
            Ok(VersionFloorOutcome::RequireDowngradeRejected)
        );
        // and the floor was NOT advanced by the rejected downgrade.
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &manifest(6, RequireTransport::Arti)),
            Ok(VersionFloorOutcome::Accepted)
        );
    }

    #[test]
    fn same_version_conflicting_signature_raises_equivocation_alarm() {
        let store = MemoryStore::default();
        // Two DIFFERENT manifests at the same version (require differs -> content
        // differs -> a different owner signature): equivocation.
        admit_manifest_version(&store, &ROOT, &manifest(7, RequireTransport::None)).unwrap();
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &manifest(7, RequireTransport::Arti)),
            Ok(VersionFloorOutcome::EquivocationAlarm)
        );
    }

    #[test]
    fn re_seeing_the_same_manifest_is_idempotent() {
        let store = MemoryStore::default();
        let m = manifest(7, RequireTransport::None);
        admit_manifest_version(&store, &ROOT, &m).unwrap();
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &m),
            Ok(VersionFloorOutcome::Accepted)
        );
    }

    #[test]
    fn corrupt_floor_bytes_are_reported() {
        let store = MemoryStore::default();
        store
            .set_floor_bytes(&floor_key(&ROOT), b"too-short")
            .unwrap();
        assert_eq!(
            admit_manifest_version(&store, &ROOT, &manifest(1, RequireTransport::None)),
            Err(VersionFloorError::CorruptFloor)
        );
    }

    #[test]
    fn floor_record_round_trips() {
        let floor = Floor {
            version: 0x0102030405060708,
            require_strictness: 1,
            identity: [0x9a; 32],
        };
        assert_eq!(Floor::decode(&floor.encode()), Some(floor));
        assert_eq!(Floor::decode(&[]), None);
        let mut bad_tag = floor.encode();
        bad_tag[0] = 9;
        assert_eq!(Floor::decode(&bad_tag), None);
    }

    #[test]
    fn distinct_roots_have_distinct_floors() {
        let store = MemoryStore::default();
        let other = [0x99; 32];
        admit_manifest_version(&store, &ROOT, &manifest(5, RequireTransport::None)).unwrap();
        // A different root at version 1 is a first sight, not a rollback.
        let mut m = manifest(1, RequireTransport::None);
        m.root = other;
        assert_eq!(
            admit_manifest_version(&store, &other, &m),
            Ok(VersionFloorOutcome::Accepted)
        );
    }
}
