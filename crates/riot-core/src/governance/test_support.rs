//! Deterministic seeded builders for governance tests. Declared `#[doc(hidden)]
//! pub mod test_support` and gated behind `conformance` (Task 0). Every builder
//! is `pub fn` — NOT `pub(crate)` — because Tasks 14/15 are integration-test
//! CRATES (`tests/*.rs`) that link `riot-core` externally and can only see `pub`
//! items; a `pub(crate)` builder is E0603 from an integration test.

use crate::meadowcap::create::new_owned_write;
use crate::meadowcap::delegate::delegate_write;
use crate::meadowcap::fingerprint::write_capability_fingerprint;
use willow25::authorisation::WriteCapability;
use willow25::prelude::{Area, NamespaceSecret, Path, SubspaceSecret, TimeRange};

use super::body::{Body, OpaqueBytes};
use super::record::{record_id, GovernanceRecordV1};
use super::{Fingerprint, RecordId, RecordKind};

const NS_SEED: [u8; 32] = [3u8; 32];

fn owner() -> SubspaceSecret {
    SubspaceSecret::from_bytes(&[4u8; 32])
}
fn namespace_secret() -> NamespaceSecret {
    NamespaceSecret::from_bytes(&NS_SEED)
}
fn ssecret(seed: u8) -> SubspaceSecret {
    SubspaceSecret::from_bytes(&[seed; 32])
}
fn micros(from: u64, to: u64) -> TimeRange {
    use crate::willow::tai_j2000_micros_from_unix_seconds as tai;
    TimeRange::new(tai(from).unwrap().into(), Some(tai(to).unwrap().into()))
}
fn ob() -> OpaqueBytes {
    OpaqueBytes(vec![0x01, 0x02])
}

/// An owned genesis write cap and a one-hop child delegated to `seed`'s receiver.
pub fn parent_and_child(seed: u8) -> (WriteCapability, WriteCapability) {
    let parent = new_owned_write(&namespace_secret(), owner().corresponding_subspace_id());
    let child_id = SubspaceSecret::from_bytes(&[seed; 32]).corresponding_subspace_id();
    let area = Area::new(
        Some(child_id.clone()),
        Path::from_slices(&[b"content"]).unwrap(),
        micros(1_700_000_000, 1_800_000_000),
    );
    let child = delegate_write(&parent, &owner(), area, child_id).expect("attenuate");
    (parent, child)
}

pub fn genesis_record(namespace: [u8; 32]) -> GovernanceRecordV1 {
    let parent = new_owned_write(&namespace_secret(), owner().corresponding_subspace_id());
    GovernanceRecordV1 {
        kind: RecordKind::Genesis,
        namespace,
        parents: vec![],
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 0,
        prev_actor_record: None,
        authorizing_fingerprint: write_capability_fingerprint(&parent),
        body: Body::Genesis,
        created_display_micros: 1000,
    }
}

/// A valid `CapabilityIssued` record with GENUINE embedded parent+child bytes.
pub fn issued_record(namespace: [u8; 32], seed: u8) -> GovernanceRecordV1 {
    let (parent, child) = parent_and_child(seed);
    let parent_fp = write_capability_fingerprint(&parent);
    let child_fp = write_capability_fingerprint(&child);
    GovernanceRecordV1 {
        kind: RecordKind::CapabilityIssued,
        namespace,
        parents: vec![record_id(&genesis_record(namespace))],
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 1,
        prev_actor_record: Some(record_id(&genesis_record(namespace))),
        authorizing_fingerprint: parent_fp,
        body: Body::CapabilityIssued {
            covering_parent_fingerprint: parent_fp,
            child_fingerprint: child_fp,
            parent_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&parent)),
            child_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&child)),
        },
        created_display_micros: 2000,
    }
}

pub fn fingerprint_of_issued(r: &GovernanceRecordV1) -> Fingerprint {
    match &r.body {
        Body::CapabilityIssued {
            child_fingerprint, ..
        } => *child_fingerprint,
        _ => panic!("not an issuance record"),
    }
}

pub fn revoke_record(target: Fingerprint) -> GovernanceRecordV1 {
    GovernanceRecordV1 {
        kind: RecordKind::CapabilityRevoked,
        namespace: [9u8; 32],
        parents: vec![],
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 0,
        prev_actor_record: None,
        authorizing_fingerprint: [7u8; 32],
        body: Body::CapabilityRevoked {
            target_fingerprint: target,
            cutoffs: vec![],
        },
        created_display_micros: 3000,
    }
}

pub fn actor_record(actor: [u8; 32], seq: u64, prev: Option<RecordId>) -> GovernanceRecordV1 {
    GovernanceRecordV1 {
        kind: RecordKind::Proposal,
        namespace: [9u8; 32],
        parents: vec![],
        actor_id: actor,
        receiver: [2u8; 32],
        sequence: seq,
        prev_actor_record: prev,
        authorizing_fingerprint: [7u8; 32],
        body: Body::Proposal { proposal: ob() },
        created_display_micros: 1000,
    }
}

pub fn binding_record(actor: [u8; 32], receiver: [u8; 32]) -> GovernanceRecordV1 {
    GovernanceRecordV1 {
        kind: RecordKind::ActorBinding,
        namespace: [9u8; 32],
        parents: vec![],
        actor_id: actor,
        receiver,
        sequence: 0,
        prev_actor_record: None,
        authorizing_fingerprint: [7u8; 32],
        body: Body::ActorBinding {
            bound_receiver: receiver,
            encryption_key: [4u8; 32],
        },
        created_display_micros: 1000,
    }
}

pub fn child_record(parents: &[RecordId], seed: u8) -> GovernanceRecordV1 {
    GovernanceRecordV1 {
        kind: RecordKind::Proposal,
        namespace: [9u8; 32],
        parents: parents.to_vec(),
        actor_id: [seed; 32],
        receiver: [2u8; 32],
        sequence: 0,
        prev_actor_record: None,
        authorizing_fingerprint: [7u8; 32],
        body: Body::Proposal {
            proposal: OpaqueBytes(vec![seed]),
        },
        created_display_micros: 1000,
    }
}

/// One seeded `Body` per record kind. Issuance/renewal/role bodies carry GENUINE
/// capability bytes and fingerprints (real `delegate_write` chains) so the
/// attenuation-proof tasks can consume them; the deferred-semantics kinds carry
/// a small opaque placeholder.
pub fn sample_body_for(kind: RecordKind) -> Body {
    match kind {
        RecordKind::Genesis => Body::Genesis,
        RecordKind::ActorBinding => Body::ActorBinding {
            bound_receiver: [3u8; 32],
            encryption_key: [4u8; 32],
        },
        RecordKind::MemberDecision => Body::MemberDecision {
            member_actor: [5u8; 32],
            decision: ob(),
        },
        RecordKind::InviteManagerDecision => Body::InviteManagerDecision {
            invite_id: [6u8; 32],
            decision: ob(),
        },
        RecordKind::InviteResponse => Body::InviteResponse {
            invite_id: [6u8; 32],
            response: ob(),
        },
        RecordKind::InviteActivation => Body::InviteActivation {
            invite_id: [6u8; 32],
            activation: ob(),
        },
        RecordKind::RoleDecision => {
            let root = new_owned_write(&namespace_secret(), owner().corresponding_subspace_id());
            let (_p, x) = parent_and_child(20);
            Body::RoleDecision {
                role_instance_id: [7u8; 32],
                covering_parent_fingerprint: write_capability_fingerprint(&root),
                granted_fingerprints: vec![write_capability_fingerprint(&x)],
            }
        }
        RecordKind::CapabilityIssued => {
            let (parent, child) = parent_and_child(30);
            Body::CapabilityIssued {
                covering_parent_fingerprint: write_capability_fingerprint(&parent),
                child_fingerprint: write_capability_fingerprint(&child),
                parent_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&parent)),
                child_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&child)),
            }
        }
        RecordKind::CapabilityRenewed => {
            let (parent, child) = parent_and_child(31);
            Body::CapabilityRenewed {
                covering_parent_fingerprint: write_capability_fingerprint(&parent),
                child_fingerprint: write_capability_fingerprint(&child),
                replaces_fingerprint: [33u8; 32],
                parent_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&parent)),
                child_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&child)),
            }
        }
        RecordKind::CapabilityRevoked => Body::CapabilityRevoked {
            target_fingerprint: [2u8; 32],
            cutoffs: vec![],
        },
        RecordKind::Checkpoint => Body::Checkpoint {
            checkpoint_id: [10u8; 32],
            merged_frontier_hash: [11u8; 32],
        },
        RecordKind::ActionReceipt => Body::ActionReceipt { receipt: ob() },
        RecordKind::Proposal => Body::Proposal { proposal: ob() },
        RecordKind::AppealSubmitted => Body::AppealSubmitted {
            action_id: [12u8; 32],
            appeal: ob(),
        },
        RecordKind::AppealResolved => Body::AppealResolved {
            action_id: [12u8; 32],
            resolution: ob(),
        },
        RecordKind::AppApproved => Body::AppApproved {
            app_id: [13u8; 32],
            manifest_digest: [14u8; 32],
            granted_permissions_cbor: ob(),
        },
        RecordKind::AppRevoked => Body::AppRevoked {
            app_id: [13u8; 32],
            reason: ob(),
        },
        RecordKind::AppProvisioned => Body::AppProvisioned {
            app_id: [13u8; 32],
            receiver: [15u8; 32],
            provisioning: ob(),
        },
        RecordKind::DirectoryWithdrawn => Body::DirectoryWithdrawn {
            app_id: [13u8; 32],
            withdrawal: ob(),
        },
        RecordKind::RecoveryDeclared => Body::RecoveryDeclared { recovery: ob() },
        RecordKind::MigrationDeclared => Body::MigrationDeclared {
            new_namespace: [16u8; 32],
            migration: ob(),
        },
        RecordKind::LensSuccessor => Body::LensSuccessor {
            new_namespace: [16u8; 32],
            successor: ob(),
        },
    }
}

/// One seeded, valid, canonically round-tripping record per kind. The envelope
/// `kind` matches `sample_body_for(kind)`'s body kind, so `decode_record`'s
/// kind↔body invariant holds for every tag.
pub fn seeded_record_for(kind: RecordKind) -> GovernanceRecordV1 {
    GovernanceRecordV1 {
        kind,
        namespace: [9u8; 32],
        parents: vec![],
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 0,
        prev_actor_record: None,
        authorizing_fingerprint: [7u8; 32],
        body: sample_body_for(kind),
        created_display_micros: 1000,
    }
}

/// A valid star DAG: a genesis, a binding, and `n` issuances all parented on it.
pub fn seeded_journal(n: usize) -> Vec<GovernanceRecordV1> {
    let ns = [9u8; 32];
    let genesis = genesis_record(ns);
    let gid = record_id(&genesis);
    let mut out = vec![genesis];
    out.push({
        let mut b = binding_record([1u8; 32], [2u8; 32]);
        b.parents = vec![gid];
        b
    });
    for i in 0..n {
        let seed = 80u8.wrapping_add(i as u8);
        out.push(issued_record_for_cap(ns, seed, &[gid]));
    }
    out
}

/// A `CapabilityIssued` record whose embedded child is `parent_and_child(seed).1`
/// (so its fingerprint matches the corresponding candidate cap), with `parents`.
pub fn issued_record_for_cap(
    namespace: [u8; 32],
    seed: u8,
    parents: &[RecordId],
) -> GovernanceRecordV1 {
    let (parent, child) = parent_and_child(seed);
    let parent_fp = write_capability_fingerprint(&parent);
    let child_fp = write_capability_fingerprint(&child);
    GovernanceRecordV1 {
        kind: RecordKind::CapabilityIssued,
        namespace,
        parents: parents.to_vec(),
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 1,
        prev_actor_record: None,
        authorizing_fingerprint: parent_fp,
        body: Body::CapabilityIssued {
            covering_parent_fingerprint: parent_fp,
            child_fingerprint: child_fp,
            parent_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&parent)),
            child_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&child)),
        },
        created_display_micros: 2000,
    }
}

/// A `RoleDecision` record granting `granted`, parented on `parents`. The
/// covering parent is the owned genesis cap fingerprint.
pub fn role_decision_record(
    namespace: [u8; 32],
    role: [u8; 32],
    parents: &[RecordId],
    granted: Vec<Fingerprint>,
) -> GovernanceRecordV1 {
    let root = new_owned_write(&namespace_secret(), owner().corresponding_subspace_id());
    GovernanceRecordV1 {
        kind: RecordKind::RoleDecision,
        namespace,
        parents: parents.to_vec(),
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 1,
        prev_actor_record: None,
        authorizing_fingerprint: write_capability_fingerprint(&root),
        body: Body::RoleDecision {
            role_instance_id: role,
            covering_parent_fingerprint: write_capability_fingerprint(&root),
            granted_fingerprints: granted,
        },
        created_display_micros: 2500,
    }
}

// The role's two candidate capabilities X and Y are real one-hop delegations
// from a fixed genesis (seeds 20 and 21), so their fingerprints are genuine and
// reproducible. `is_role_fp` recomputes both and tests membership.
fn role_candidate_caps() -> (WriteCapability, WriteCapability) {
    let (_p, x) = parent_and_child(20);
    let (_p2, y) = parent_and_child(21);
    (x, y)
}
pub fn is_role_fp(fp: &Fingerprint) -> bool {
    let (x, y) = role_candidate_caps();
    *fp == write_capability_fingerprint(&x) || *fp == write_capability_fingerprint(&y)
}

/// A journal in which capabilities X and Y are BOTH genuinely issued (active
/// after the grant fold), plus TWO concurrent RoleDecision records for the same
/// role instance R — both parented on genesis, neither an ancestor of the other:
/// decision_1 grants {X, Y}, decision_2 grants {X}. The intersection is {X}, so
/// after `apply_role_restrictions` only X survives among the role's fps.
/// Returns `(records, fingerprint_of_X)`.
pub fn two_concurrent_role_restrictions() -> (Vec<GovernanceRecordV1>, Fingerprint) {
    let ns = [9u8; 32];
    let (x, y) = role_candidate_caps();
    let (fp_x, fp_y) = (
        write_capability_fingerprint(&x),
        write_capability_fingerprint(&y),
    );
    let genesis = genesis_record(ns);
    let gid = record_id(&genesis);

    // Real issuances so the grant fold activates both X and Y.
    let issue_x = issued_record_for_cap(ns, 20, &[gid]);
    let issue_y = issued_record_for_cap(ns, 21, &[gid]);

    let role = [77u8; 32];
    let decision_1 = role_decision_record(ns, role, &[gid], vec![fp_x, fp_y]); // grants {X, Y}
    let decision_2 = role_decision_record(ns, role, &[gid], vec![fp_x]); // grants {X}, concurrent
    (
        vec![genesis, issue_x, issue_y, decision_1, decision_2],
        fp_x,
    )
}

/// The five real caps of a `root -> A -> B -> C` lineage plus sibling `D` off A.
/// Every hop re-grants `Area::full()` (reflexive inclusion) so only depth grows.
fn lineage_caps() -> (
    WriteCapability,
    WriteCapability,
    WriteCapability,
    WriteCapability,
    WriteCapability,
) {
    let root = new_owned_write(&namespace_secret(), owner().corresponding_subspace_id());
    let cap_a = delegate_write(
        &root,
        &owner(),
        Area::full(),
        ssecret(41).corresponding_subspace_id(),
    )
    .expect("A");
    let cap_b = delegate_write(
        &cap_a,
        &ssecret(41),
        Area::full(),
        ssecret(42).corresponding_subspace_id(),
    )
    .expect("B");
    let cap_c = delegate_write(
        &cap_b,
        &ssecret(42),
        Area::full(),
        ssecret(43).corresponding_subspace_id(),
    )
    .expect("C");
    let cap_d = delegate_write(
        &cap_a,
        &ssecret(41),
        Area::full(),
        ssecret(44).corresponding_subspace_id(),
    )
    .expect("D");
    (root, cap_a, cap_b, cap_c, cap_d)
}

fn issue_between(
    ns: [u8; 32],
    parent: &WriteCapability,
    child: &WriteCapability,
    parents: Vec<RecordId>,
    seq: u64,
) -> GovernanceRecordV1 {
    GovernanceRecordV1 {
        kind: RecordKind::CapabilityIssued,
        namespace: ns,
        parents,
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: seq,
        prev_actor_record: None,
        authorizing_fingerprint: write_capability_fingerprint(parent),
        body: Body::CapabilityIssued {
            covering_parent_fingerprint: write_capability_fingerprint(parent),
            child_fingerprint: write_capability_fingerprint(child),
            parent_capability_bytes: OpaqueBytes(crate::willow::encode_capability(parent)),
            child_capability_bytes: OpaqueBytes(crate::willow::encode_capability(child)),
        },
        created_display_micros: 2000 + seq,
    }
}

fn lineage_fps(
    a: &WriteCapability,
    b: &WriteCapability,
    c: &WriteCapability,
    d: &WriteCapability,
) -> Fps {
    Fps {
        a: write_capability_fingerprint(a),
        b: write_capability_fingerprint(b),
        c: write_capability_fingerprint(c),
        d_sibling: write_capability_fingerprint(d),
    }
}

/// `G -> A -> B -> C` linear lineage (sibling D's cap exists for its fingerprint
/// but is NOT yet issued as a record).
pub fn three_hop_lineage_records() -> (Vec<GovernanceRecordV1>, Fps) {
    let ns = [9u8; 32];
    let (root, cap_a, cap_b, cap_c, cap_d) = lineage_caps();
    let genesis = genesis_record(ns);
    let gid = record_id(&genesis);
    let issue_a = issue_between(ns, &root, &cap_a, vec![gid], 1);
    let ia = record_id(&issue_a);
    let issue_b = issue_between(ns, &cap_a, &cap_b, vec![ia], 2);
    let ib = record_id(&issue_b);
    let issue_c = issue_between(ns, &cap_b, &cap_c, vec![ib], 3);
    (
        vec![genesis, issue_a, issue_b, issue_c],
        lineage_fps(&cap_a, &cap_b, &cap_c, &cap_d),
    )
}

/// `G -> A -> B -> C` plus an issuance for sibling D (all four caps active).
pub fn three_hop_lineage_records_with_sibling_active() -> (Vec<GovernanceRecordV1>, Fps) {
    let ns = [9u8; 32];
    let (root, cap_a, cap_b, cap_c, cap_d) = lineage_caps();
    let genesis = genesis_record(ns);
    let gid = record_id(&genesis);
    let issue_a = issue_between(ns, &root, &cap_a, vec![gid], 1);
    let ia = record_id(&issue_a);
    let issue_b = issue_between(ns, &cap_a, &cap_b, vec![ia], 2);
    let ib = record_id(&issue_b);
    let issue_c = issue_between(ns, &cap_b, &cap_c, vec![ib], 3);
    let issue_d = issue_between(ns, &cap_a, &cap_d, vec![ia], 2);
    (
        vec![genesis, issue_a, issue_b, issue_c, issue_d],
        lineage_fps(&cap_a, &cap_b, &cap_c, &cap_d),
    )
}

/// Issue → revoke → renew: returns `(records, old_fp, new_fp)` where the renewal
/// replaces the revoked capability with one bearing a fresh fingerprint.
pub fn renewal_after_revoke() -> (Vec<GovernanceRecordV1>, Fingerprint, Fingerprint) {
    let ns = [9u8; 32];
    let genesis = genesis_record(ns);
    let gid = record_id(&genesis);
    let issue = issued_record_for_cap(ns, 50, &[gid]);
    let old_fp = fingerprint_of_issued(&issue);
    let iid = record_id(&issue);
    let mut revoke = revoke_record(old_fp);
    revoke.parents = vec![iid];
    let rid = record_id(&revoke);
    let (parent, child) = parent_and_child(51);
    let new_fp = write_capability_fingerprint(&child);
    let renew = GovernanceRecordV1 {
        kind: RecordKind::CapabilityRenewed,
        namespace: ns,
        parents: vec![rid],
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 2,
        prev_actor_record: None,
        authorizing_fingerprint: write_capability_fingerprint(&parent),
        body: Body::CapabilityRenewed {
            covering_parent_fingerprint: write_capability_fingerprint(&parent),
            child_fingerprint: new_fp,
            replaces_fingerprint: old_fp,
            parent_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&parent)),
            child_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&child)),
        },
        created_display_micros: 4000,
    };
    (vec![genesis, issue, revoke, renew], old_fp, new_fp)
}

/// A valid issuance record whose single parent id was never ingested, so the
/// evaluator must leave it pending (never accepted).
pub fn issued_record_with_missing_parent(namespace: [u8; 32], seed: u8) -> GovernanceRecordV1 {
    let mut r = issued_record(namespace, seed);
    r.parents = vec![[0xEEu8; 32]];
    r
}

/// An issuance record purporting to authorize itself: `authorizing_fingerprint`
/// equals its own `child_fingerprint`.
pub fn self_authorizing_record() -> GovernanceRecordV1 {
    let ns = [9u8; 32];
    let mut r = issued_record(ns, 60);
    let child_fp = fingerprint_of_issued(&r);
    r.authorizing_fingerprint = child_fp;
    r
}

/// Issue → revoke → appeal-submitted → appeal-resolved (favorable). Returns
/// `(records, revoked_fp)`.
pub fn revoke_then_favorable_appeal() -> (Vec<GovernanceRecordV1>, Fingerprint) {
    let ns = [9u8; 32];
    let genesis = genesis_record(ns);
    let gid = record_id(&genesis);
    let issue = issued_record_for_cap(ns, 70, &[gid]);
    let fp = fingerprint_of_issued(&issue);
    let iid = record_id(&issue);
    let mut revoke = revoke_record(fp);
    revoke.parents = vec![iid];
    let rid = record_id(&revoke);
    let submitted = GovernanceRecordV1 {
        kind: RecordKind::AppealSubmitted,
        namespace: ns,
        parents: vec![rid],
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 3,
        prev_actor_record: None,
        authorizing_fingerprint: [7u8; 32],
        body: Body::AppealSubmitted {
            action_id: rid,
            appeal: OpaqueBytes(vec![0x01]),
        },
        created_display_micros: 5000,
    };
    let sid = record_id(&submitted);
    let resolved = GovernanceRecordV1 {
        kind: RecordKind::AppealResolved,
        namespace: ns,
        parents: vec![sid],
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 4,
        prev_actor_record: None,
        authorizing_fingerprint: [7u8; 32],
        body: Body::AppealResolved {
            action_id: rid,
            resolution: OpaqueBytes(vec![0x01]),
        },
        created_display_micros: 6000,
    };
    (vec![genesis, issue, revoke, submitted, resolved], fp)
}

/// Two `MigrationDeclared` records with distinct `new_namespace` targets, both
/// parented on the same genesis (a concurrent migration conflict).
pub fn two_competing_migrations() -> Vec<GovernanceRecordV1> {
    let ns = [9u8; 32];
    let genesis = genesis_record(ns);
    let gid = record_id(&genesis);
    let mk = |new_ns: [u8; 32], tag: u8| GovernanceRecordV1 {
        kind: RecordKind::MigrationDeclared,
        namespace: ns,
        parents: vec![gid],
        actor_id: [1u8; 32],
        receiver: [2u8; 32],
        sequence: 1,
        prev_actor_record: None,
        authorizing_fingerprint: [7u8; 32],
        body: Body::MigrationDeclared {
            new_namespace: new_ns,
            migration: OpaqueBytes(vec![tag]),
        },
        created_display_micros: 7000 + tag as u64,
    };
    vec![genesis, mk([21u8; 32], 1), mk([22u8; 32], 2)]
}

pub struct Fps {
    pub a: Fingerprint,
    pub b: Fingerprint,
    pub c: Fingerprint,
    pub d_sibling: Fingerprint,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::record::{decode_record, encode_record};

    #[test]
    fn every_seeded_record_decodes_canonically() {
        for tag in 0u64..=21 {
            let kind = RecordKind::from_tag(tag).unwrap();
            let r = seeded_record_for(kind);
            assert_eq!(
                decode_record(&encode_record(&r)).unwrap(),
                r,
                "{kind:?} seeded record must round-trip"
            );
        }
    }
}
