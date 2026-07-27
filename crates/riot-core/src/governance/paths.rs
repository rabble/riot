//! Exact `governance/v1/...` path templates per kind + path↔body target binding.
//! Reproduces the design record-path table (lines ~456–480) verbatim. Ids are
//! raw 32-byte components; `sequence_be` is an 8-byte big-endian component.

use super::body::Body;
use super::record::{record_id, GovernanceRecordV1};
use super::{GovernanceError, RecordKind};
use crate::willow::Path;

pub fn path_for(r: &GovernanceRecordV1) -> Result<Path, GovernanceError> {
    let rid = record_id(r);
    let seq_be = r.sequence.to_be_bytes();
    let p = |parts: &[&[u8]]| {
        Path::from_slices(parts).map_err(|_| GovernanceError::PathBindingMismatch)
    };
    match (&r.kind, &r.body) {
        (RecordKind::Genesis, Body::Genesis) => p(&[b"governance", b"v1", b"genesis"]),
        (RecordKind::ActorBinding, Body::ActorBinding { bound_receiver, .. }) => p(&[
            b"governance",
            b"v1",
            b"actors",
            &r.actor_id,
            b"bindings",
            bound_receiver,
            &rid,
        ]),
        (RecordKind::MemberDecision, Body::MemberDecision { .. }) => {
            p(&[b"governance", b"v1", b"members", &r.actor_id, &rid])
        }
        (RecordKind::InviteManagerDecision, Body::InviteManagerDecision { invite_id, .. }) => p(&[
            b"governance",
            b"v1",
            b"invitations",
            invite_id,
            b"manager",
            &rid,
        ]),
        (RecordKind::InviteResponse, Body::InviteResponse { invite_id, .. }) => p(&[
            b"governance",
            b"v1",
            b"invitations",
            invite_id,
            b"responses",
            &r.receiver,
            &rid,
        ]),
        (RecordKind::InviteActivation, Body::InviteActivation { invite_id, .. }) => {
            p(&[b"governance", b"v1", b"activations", invite_id, &rid])
        }
        (
            RecordKind::RoleDecision,
            Body::RoleDecision {
                role_instance_id, ..
            },
        ) => p(&[
            b"governance",
            b"v1",
            b"roles",
            &r.actor_id,
            role_instance_id,
            &rid,
        ]),
        (
            RecordKind::CapabilityIssued,
            Body::CapabilityIssued {
                child_fingerprint, ..
            },
        ) => p(&[
            b"governance",
            b"v1",
            b"capabilities",
            b"issued",
            child_fingerprint,
            &rid,
        ]),
        (
            RecordKind::CapabilityRenewed,
            Body::CapabilityRenewed {
                child_fingerprint, ..
            },
        ) => p(&[
            b"governance",
            b"v1",
            b"capabilities",
            b"renewed",
            child_fingerprint,
            &rid,
        ]),
        (
            RecordKind::CapabilityRevoked,
            Body::CapabilityRevoked {
                target_fingerprint, ..
            },
        ) => p(&[
            b"governance",
            b"v1",
            b"revocations",
            target_fingerprint,
            &rid,
        ]),
        (RecordKind::Checkpoint, Body::Checkpoint { checkpoint_id, .. }) => {
            p(&[b"governance", b"v1", b"checkpoints", checkpoint_id])
        }
        (RecordKind::ActionReceipt, Body::ActionReceipt { .. }) => p(&[
            b"governance",
            b"v1",
            b"actions",
            &r.actor_id,
            &r.receiver,
            &seq_be,
        ]),
        (RecordKind::Proposal, Body::Proposal { .. }) => {
            p(&[b"governance", b"v1", b"proposals", &r.actor_id, &rid])
        }
        (RecordKind::AppealSubmitted, Body::AppealSubmitted { action_id, .. }) => p(&[
            b"governance",
            b"v1",
            b"appeals",
            b"submissions",
            &r.actor_id,
            action_id,
            &rid,
        ]),
        (RecordKind::AppealResolved, Body::AppealResolved { action_id, .. }) => p(&[
            b"governance",
            b"v1",
            b"appeals",
            b"resolutions",
            action_id,
            &rid,
        ]),
        (RecordKind::AppApproved, Body::AppApproved { app_id, .. }) => {
            p(&[b"governance", b"v1", b"apps", b"approvals", app_id, &rid])
        }
        (RecordKind::AppRevoked, Body::AppRevoked { app_id, .. }) => {
            p(&[b"governance", b"v1", b"apps", b"revocations", app_id, &rid])
        }
        (
            RecordKind::AppProvisioned,
            Body::AppProvisioned {
                app_id, receiver, ..
            },
        ) => p(&[
            b"governance",
            b"v1",
            b"apps",
            b"provisioning",
            app_id,
            receiver,
            &rid,
        ]),
        (RecordKind::DirectoryWithdrawn, Body::DirectoryWithdrawn { app_id, .. }) => p(&[
            b"governance",
            b"v1",
            b"directory",
            b"withdrawals",
            app_id,
            &rid,
        ]),
        (RecordKind::RecoveryDeclared, Body::RecoveryDeclared { .. }) => {
            p(&[b"governance", b"v1", b"recovery", &rid])
        }
        (RecordKind::MigrationDeclared, Body::MigrationDeclared { new_namespace, .. }) => {
            p(&[b"governance", b"v1", b"migrations", new_namespace, &rid])
        }
        (RecordKind::LensSuccessor, Body::LensSuccessor { new_namespace, .. }) => p(&[
            b"governance",
            b"v1",
            b"lenses",
            b"successors",
            new_namespace,
            &rid,
        ]),
        // Envelope kind and body shape disagree: ineligible.
        _ => Err(GovernanceError::PathBindingMismatch),
    }
}

pub fn verify_path_binding(
    entry_path: &Path,
    r: &GovernanceRecordV1,
) -> Result<(), GovernanceError> {
    if *entry_path == path_for(r)? {
        Ok(())
    } else {
        Err(GovernanceError::PathBindingMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::seeded_record_for;

    #[test]
    fn genesis_path_is_exact_and_a_mangled_path_is_rejected() {
        let r = seeded_record_for(RecordKind::Genesis);
        assert_eq!(
            path_for(&r).unwrap(),
            Path::from_slices(&[b"governance", b"v1", b"genesis"]).unwrap()
        );
        let mangled = Path::from_slices(&[b"governance", b"v1", b"genesis", b"x"]).unwrap();
        assert_eq!(
            verify_path_binding(&mangled, &r),
            Err(GovernanceError::PathBindingMismatch)
        );
    }

    #[test]
    fn capability_issued_path_binds_the_fingerprint_component() {
        let r = seeded_record_for(RecordKind::CapabilityIssued);
        assert_eq!(verify_path_binding(&path_for(&r).unwrap(), &r), Ok(()));
        let wrong = Path::from_slices(&[
            b"governance",
            b"v1",
            b"capabilities",
            b"issued",
            &[0xAAu8; 32],
            &record_id(&r),
        ])
        .unwrap();
        assert_eq!(
            verify_path_binding(&wrong, &r),
            Err(GovernanceError::PathBindingMismatch)
        );
    }

    #[test]
    fn every_kind_has_an_exact_path_and_an_extra_component_rejects() {
        for tag in 0u64..=21 {
            let kind = RecordKind::from_tag(tag).unwrap();
            let r = seeded_record_for(kind);
            let good = path_for(&r).expect("path_for");
            assert_eq!(
                verify_path_binding(&good, &r),
                Ok(()),
                "{kind:?} self-binds"
            );
            // willow25 `Path::components()` yields `&Component`, not `&[u8]`;
            // `.as_bytes()` gives the component's raw bytes.
            let mut parts: Vec<&[u8]> = good.components().map(|c| c.as_bytes()).collect();
            parts.push(b"extra");
            let extra = Path::from_slices(&parts).unwrap();
            assert_eq!(
                verify_path_binding(&extra, &r),
                Err(GovernanceError::PathBindingMismatch),
                "{kind:?} extra rejected"
            );
        }
    }
}
