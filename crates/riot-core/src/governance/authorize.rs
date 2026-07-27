//! Governance authorization checks layered on Slice-1 capability validity.

use super::body::Body;
use super::frontier::Frontier;
use super::record::{record_id, GovernanceRecordV1};
use super::{GovernanceError, RecordKind};
use crate::meadowcap::codec::decode_write_capability_bounded;
use crate::meadowcap::fingerprint::write_capability_fingerprint;

/// Verify a `CapabilityIssued`/`CapabilityRenewed` body: both embedded caps
/// decode (cryptographic chain-validity via Slice 1), their recomputed
/// fingerprints match the body's claims, and the child is an attenuation-
/// descendant of the parent (same genesis, child chain extends parent chain,
/// child area ⊆ parent area).
pub fn verify_capability_issuance(body: &Body) -> Result<(), GovernanceError> {
    let (parent_fp, child_fp, parent_bytes, child_bytes) = match body {
        Body::CapabilityIssued {
            covering_parent_fingerprint,
            child_fingerprint,
            parent_capability_bytes,
            child_capability_bytes,
        }
        | Body::CapabilityRenewed {
            covering_parent_fingerprint,
            child_fingerprint,
            parent_capability_bytes,
            child_capability_bytes,
            ..
        } => (
            covering_parent_fingerprint,
            child_fingerprint,
            &parent_capability_bytes.0,
            &child_capability_bytes.0,
        ),
        _ => return Err(GovernanceError::Malformed),
    };

    // (1) Both decode → willow25 cryptographically verified every chain signature.
    let parent =
        decode_write_capability_bounded(parent_bytes).map_err(GovernanceError::Capability)?;
    let child =
        decode_write_capability_bounded(child_bytes).map_err(GovernanceError::Capability)?;

    // (2) Claimed fingerprints are recomputed from the bytes (forgery guard).
    if &write_capability_fingerprint(&parent) != parent_fp
        || &write_capability_fingerprint(&child) != child_fp
    {
        return Err(GovernanceError::IssuanceNotAttenuated);
    }

    // (3) Ancestry — STRUCTURAL comparison (no chain-truncation constructor
    // exists in willow25). The `meadowcap` `Delegation` type derives
    // `PartialEq, Eq` and exposes public `area`/`user`/`signature` fields
    // (meadowcap-0.5.0/src/raw/mod.rs:190 derive, :197-204 fields), so
    // `&[Delegation]` slice equality is a direct `==`; the per-delegation
    // signature bytes are what make the comparison forgery-proof.
    let pd = parent.delegations();
    let cd = child.delegations();
    // (3a) Same genesis: namespace key, user key, access mode, and communal-vs-
    // owned all match (meadowcap-0.5.0/src/raw/mod.rs:78/86/94 genesis
    // accessors; write_capability.rs:248 `is_owned`).
    let same_genesis = parent.granted_namespace() == child.granted_namespace()
        && parent.genesis().namespace_key() == child.genesis().namespace_key()
        && parent.genesis().user_key() == child.genesis().user_key()
        && parent.genesis().access_mode() == child.genesis().access_mode()
        && parent.is_owned() == child.is_owned();
    // (3b) Child chain extends the parent's by exactly one hop AND the child's
    // leading delegations equal the parent's delegations element-wise (derived
    // `Eq` on `Delegation`). A same-genesis SIBLING (a differently-signed chain
    // of the same length/depth) fails here because its delegation prefix — the
    // signatures included — differs from the parent's.
    let extends = cd.len() == pd.len() + 1 && cd[..pd.len()] == *pd;
    // (4) Attenuation: child area ⊆ parent area (willow25 also enforced this at
    // delegation time; re-assert defensively).
    let attenuated = parent.includes_area(&child.granted_area());

    if same_genesis && extends && attenuated {
        Ok(())
    } else {
        Err(GovernanceError::IssuanceNotAttenuated)
    }
}

/// A governance record's authority must come from an already-accepted ancestor
/// frontier — never from itself. Rejects a record whose authorizing_fingerprint
/// resolves only to its own issued child fingerprint.
pub fn authorize_record(
    record: &GovernanceRecordV1,
    _frontier: &Frontier,
) -> Result<(), GovernanceError> {
    if let Body::CapabilityIssued {
        child_fingerprint, ..
    } = &record.body
    {
        if &record.authorizing_fingerprint == child_fingerprint {
            return Err(GovernanceError::SelfAuthorization);
        }
    }
    // (frontier-ancestor membership is checked by the evaluator; this guard is
    // the self-authorization base case — record_id can never equal a parent.)
    let _ = record_id(record);
    Ok(())
}

/// Classify competing migrations: returns `None` (a fork requiring human
/// selection) whenever more than one distinct `MigrationDeclared` new-namespace
/// is present; `Some(ns)` only when exactly one candidate exists.
pub fn selected_migration(records: &[GovernanceRecordV1]) -> Option<[u8; 32]> {
    let mut candidates = std::collections::BTreeSet::new();
    for r in records {
        if r.kind == RecordKind::MigrationDeclared {
            if let Body::MigrationDeclared { new_namespace, .. } = &r.body {
                candidates.insert(*new_namespace);
            }
        }
    }
    if candidates.len() == 1 {
        candidates.into_iter().next()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::body::{Body, OpaqueBytes};
    use crate::governance::test_support::{issued_record, parent_and_child};
    use crate::willow::encode_capability;

    #[test]
    fn a_genuine_issuance_verifies() {
        assert_eq!(
            verify_capability_issuance(&issued_record([9u8; 32], 8).body),
            Ok(())
        );
    }

    #[test]
    fn a_forged_fingerprint_is_rejected() {
        let mut record = issued_record([9u8; 32], 8);
        if let Body::CapabilityIssued {
            child_fingerprint, ..
        } = &mut record.body
        {
            *child_fingerprint = [0xFF; 32]; // claim a fingerprint the bytes don't hash to
        }
        assert_eq!(
            verify_capability_issuance(&record.body),
            Err(GovernanceError::IssuanceNotAttenuated)
        );
    }

    #[test]
    fn a_non_descendant_child_is_rejected() {
        // Embed a child delegated from a DIFFERENT genesis than the parent.
        let (parent, _) = parent_and_child(8);
        let (_other_parent, foreign_child) = {
            // a second, unrelated owned namespace → different genesis
            use crate::meadowcap::create::new_owned_write;
            use crate::meadowcap::delegate::delegate_write;
            use willow25::prelude::{Area, NamespaceSecret, Path, SubspaceSecret, TimeRange};
            let ns = NamespaceSecret::from_bytes(&[99u8; 32]);
            let owner = SubspaceSecret::from_bytes(&[98u8; 32]);
            let root = new_owned_write(&ns, owner.corresponding_subspace_id());
            let leaf = SubspaceSecret::from_bytes(&[97u8; 32]).corresponding_subspace_id();
            let area = Area::new(
                Some(leaf.clone()),
                Path::from_slices(&[b"content"]).unwrap(),
                TimeRange::new(0u64.into(), Some(u64::MAX.into())),
            );
            (
                root.clone(),
                delegate_write(&root, &owner, area, leaf).unwrap(),
            )
        };
        let body = Body::CapabilityIssued {
            covering_parent_fingerprint: write_capability_fingerprint(&parent),
            child_fingerprint: write_capability_fingerprint(&foreign_child),
            parent_capability_bytes: OpaqueBytes(encode_capability(&parent)),
            child_capability_bytes: OpaqueBytes(encode_capability(&foreign_child)),
        };
        assert_eq!(
            verify_capability_issuance(&body),
            Err(GovernanceError::IssuanceNotAttenuated)
        );
    }

    #[test]
    fn a_same_genesis_sibling_presented_as_a_child_is_rejected_by_extends() {
        // THE ISOLATING ATTACK, constructed so `extends` is the ONLY failing
        // condition (deleting `extends` would turn this Err into Ok):
        //   - parent_b: root → (subspace None, path /a, full time)  [depth 1]
        //   - sibling_c: root → (subspace Some(c_id), path /a/b, full)  [depth 1]
        // both delegated from the SAME owned genesis `root`.
        // • same_genesis → TRUE  (identical genesis).
        // • fingerprint checks → PASS (genuine caps, genuine fingerprints).
        // • attenuated = parent_b.includes_area(sibling_c.granted_area()) → TRUE:
        //   None-subspace ⊇ Some(c_id); /a is a prefix of /a/b; full ⊇ full.
        // • extends: cd.len()==1 is NOT pd.len()+1==2 → FALSE. Only `extends`
        //   rejects the sibling.
        use crate::meadowcap::create::new_owned_write;
        use crate::meadowcap::delegate::delegate_write;
        use willow25::prelude::{Area, NamespaceSecret, Path, SubspaceSecret, TimeRange};
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let root = new_owned_write(&ns, owner.corresponding_subspace_id());
        let b_id = SubspaceSecret::from_bytes(&[8u8; 32]).corresponding_subspace_id();
        let c_id = SubspaceSecret::from_bytes(&[9u8; 32]).corresponding_subspace_id();
        let full = || TimeRange::new(0u64.into(), Some(u64::MAX.into()));
        // Broad parent: all subspaces (None), path /a — contains the sibling area.
        let parent_area = Area::new(None, Path::from_slices(&[b"a"]).unwrap(), full());
        // Narrower sibling: subspace c_id, path /a/b — inside the parent area.
        let sibling_area = Area::new(
            Some(c_id.clone()),
            Path::from_slices(&[b"a", b"b"]).unwrap(),
            full(),
        );
        let parent_b = delegate_write(&root, &owner, parent_area, b_id).unwrap();
        let sibling_c = delegate_write(&root, &owner, sibling_area, c_id).unwrap();

        // Precondition the isolation depends on: the parent's area really does
        // contain the sibling's area (so `attenuated` is TRUE and cannot be the
        // rejecting condition).
        assert!(
            parent_b.includes_area(&sibling_c.granted_area()),
            "test setup: parent must cover the sibling area so ONLY extends can reject"
        );

        let body = Body::CapabilityIssued {
            covering_parent_fingerprint: write_capability_fingerprint(&parent_b),
            child_fingerprint: write_capability_fingerprint(&sibling_c), // genuine fp of the sibling
            parent_capability_bytes: OpaqueBytes(encode_capability(&parent_b)),
            child_capability_bytes: OpaqueBytes(encode_capability(&sibling_c)),
        };
        assert_eq!(
            verify_capability_issuance(&body),
            Err(GovernanceError::IssuanceNotAttenuated),
            "a same-genesis sibling is not an attenuation-descendant — rejected by extends"
        );
    }

    #[test]
    fn an_undecodable_embedded_capability_yields_capability_error() {
        // Producer for GovernanceError::Capability: garbage embedded bytes fail
        // Slice-1's decode_write_capability_bounded → mapped to Capability(..).
        let (parent, _) = parent_and_child(8);
        let body = Body::CapabilityIssued {
            covering_parent_fingerprint: write_capability_fingerprint(&parent),
            child_fingerprint: [2u8; 32],
            parent_capability_bytes: OpaqueBytes(encode_capability(&parent)),
            child_capability_bytes: OpaqueBytes(vec![0xFF; 10]), // not a capability
        };
        assert!(matches!(
            verify_capability_issuance(&body),
            Err(GovernanceError::Capability(_))
        ));
    }
}
