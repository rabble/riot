//! Per-kind governance record bodies. Authority-bearing kinds are fully typed
//! and evaluator-wired; deferred-semantics kinds freeze their envelope + path
//! and carry their later-slice payload as an opaque, length-bounded canonical
//! byte field validated by its owning slice.
//!
//! Canonical form: each body is a definite-length CBOR map with strictly
//! ascending integer keys starting at 0, one key per field in declaration
//! order (the newswire/site-manifest discipline). `decode_body` parses against
//! the envelope `kind`; a body whose shape mismatches its `kind` is
//! `Malformed`.

use minicbor::{Decoder, Encoder};

use super::{Fingerprint, GovernanceError, RecordKind, MAX_GOVERNANCE_RECORD_BYTES};

/// A length-bounded opaque canonical CBOR byte field for deferred-slice payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueBytes(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cutoff {
    pub actor_id: [u8; 32],
    pub receiver_id: [u8; 32],
    pub action_head: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    // ---- authority-bearing (evaluator-wired) ----
    Genesis,
    ActorBinding {
        bound_receiver: [u8; 32],
        encryption_key: [u8; 32],
    },
    /// A role instance and the capability fingerprints it grants. Concurrent
    /// role decisions for the same `role_instance_id` reduce by INTERSECTION of
    /// `granted_fingerprints` (most restrictive wins — evaluator Task 12).
    RoleDecision {
        role_instance_id: [u8; 32],
        covering_parent_fingerprint: Fingerprint,
        granted_fingerprints: Vec<Fingerprint>,
    },
    /// Embeds BOTH capability encodings so issuance is attenuation-verified, not
    /// fingerprint-trusted (Task 8). The fingerprints are the claimed values;
    /// Task 8 recomputes them from the embedded bytes and rejects a mismatch.
    CapabilityIssued {
        covering_parent_fingerprint: Fingerprint,
        child_fingerprint: Fingerprint,
        parent_capability_bytes: OpaqueBytes,
        child_capability_bytes: OpaqueBytes,
    },
    CapabilityRenewed {
        covering_parent_fingerprint: Fingerprint,
        child_fingerprint: Fingerprint,
        replaces_fingerprint: Fingerprint,
        parent_capability_bytes: OpaqueBytes,
        child_capability_bytes: OpaqueBytes,
    },
    CapabilityRevoked {
        target_fingerprint: Fingerprint,
        cutoffs: Vec<Cutoff>,
    },
    Checkpoint {
        checkpoint_id: [u8; 32],
        merged_frontier_hash: [u8; 32],
    },
    /// Canonical `ActionReceiptV1` bytes (Task 7 owns the inner codec).
    ActionReceipt {
        receipt: OpaqueBytes,
    },
    Proposal {
        proposal: OpaqueBytes,
    },
    // ---- deferred-semantics (envelope frozen; payload opaque) ----
    MemberDecision {
        member_actor: [u8; 32],
        decision: OpaqueBytes,
    },
    InviteManagerDecision {
        invite_id: [u8; 32],
        decision: OpaqueBytes,
    },
    InviteResponse {
        invite_id: [u8; 32],
        response: OpaqueBytes,
    },
    InviteActivation {
        invite_id: [u8; 32],
        activation: OpaqueBytes,
    },
    AppealSubmitted {
        action_id: [u8; 32],
        appeal: OpaqueBytes,
    },
    AppealResolved {
        action_id: [u8; 32],
        resolution: OpaqueBytes,
    },
    AppApproved {
        app_id: [u8; 32],
        manifest_digest: [u8; 32],
        granted_permissions_cbor: OpaqueBytes,
    },
    AppRevoked {
        app_id: [u8; 32],
        reason: OpaqueBytes,
    },
    AppProvisioned {
        app_id: [u8; 32],
        receiver: [u8; 32],
        provisioning: OpaqueBytes,
    },
    DirectoryWithdrawn {
        app_id: [u8; 32],
        withdrawal: OpaqueBytes,
    },
    RecoveryDeclared {
        recovery: OpaqueBytes,
    },
    MigrationDeclared {
        new_namespace: [u8; 32],
        migration: OpaqueBytes,
    },
    LensSuccessor {
        new_namespace: [u8; 32],
        successor: OpaqueBytes,
    },
}

/// The `RecordKind` a body variant belongs to (used by `decode_record`).
pub fn kind_of(body: &Body) -> RecordKind {
    match body {
        Body::Genesis => RecordKind::Genesis,
        Body::ActorBinding { .. } => RecordKind::ActorBinding,
        Body::MemberDecision { .. } => RecordKind::MemberDecision,
        Body::InviteManagerDecision { .. } => RecordKind::InviteManagerDecision,
        Body::InviteResponse { .. } => RecordKind::InviteResponse,
        Body::InviteActivation { .. } => RecordKind::InviteActivation,
        Body::RoleDecision { .. } => RecordKind::RoleDecision,
        Body::CapabilityIssued { .. } => RecordKind::CapabilityIssued,
        Body::CapabilityRenewed { .. } => RecordKind::CapabilityRenewed,
        Body::CapabilityRevoked { .. } => RecordKind::CapabilityRevoked,
        Body::Checkpoint { .. } => RecordKind::Checkpoint,
        Body::ActionReceipt { .. } => RecordKind::ActionReceipt,
        Body::Proposal { .. } => RecordKind::Proposal,
        Body::AppealSubmitted { .. } => RecordKind::AppealSubmitted,
        Body::AppealResolved { .. } => RecordKind::AppealResolved,
        Body::AppApproved { .. } => RecordKind::AppApproved,
        Body::AppRevoked { .. } => RecordKind::AppRevoked,
        Body::AppProvisioned { .. } => RecordKind::AppProvisioned,
        Body::DirectoryWithdrawn { .. } => RecordKind::DirectoryWithdrawn,
        Body::RecoveryDeclared { .. } => RecordKind::RecoveryDeclared,
        Body::MigrationDeclared { .. } => RecordKind::MigrationDeclared,
        Body::LensSuccessor { .. } => RecordKind::LensSuccessor,
    }
}

/// The primary target id for the `governance_by_target` index (Task 13):
/// issued/renewed → child_fingerprint; revoked → target_fingerprint; actor
/// binding → bound_receiver; role → role_instance_id; app kinds → app_id;
/// invites → invite_id; appeals → action_id; migration/lens → new_namespace;
/// member decision → member_actor; checkpoint → checkpoint_id;
/// genesis/proposal/recovery/action-receipt → the caller-supplied fallback id
/// (the record's actor for the former three, the envelope receiver for
/// receipts — `record.rs` passes the right one).
pub fn target_id_of(kind: RecordKind, body: &Body, fallback_id: &[u8; 32]) -> [u8; 32] {
    let _ = kind; // today every variant's target derives from the body alone
    match body {
        Body::Genesis
        | Body::Proposal { .. }
        | Body::RecoveryDeclared { .. }
        | Body::ActionReceipt { .. } => *fallback_id,
        Body::ActorBinding { bound_receiver, .. } => *bound_receiver,
        Body::MemberDecision { member_actor, .. } => *member_actor,
        Body::InviteManagerDecision { invite_id, .. }
        | Body::InviteResponse { invite_id, .. }
        | Body::InviteActivation { invite_id, .. } => *invite_id,
        Body::RoleDecision {
            role_instance_id, ..
        } => *role_instance_id,
        Body::CapabilityIssued {
            child_fingerprint, ..
        }
        | Body::CapabilityRenewed {
            child_fingerprint, ..
        } => *child_fingerprint,
        Body::CapabilityRevoked {
            target_fingerprint, ..
        } => *target_fingerprint,
        Body::Checkpoint { checkpoint_id, .. } => *checkpoint_id,
        Body::AppealSubmitted { action_id, .. } | Body::AppealResolved { action_id, .. } => {
            *action_id
        }
        Body::AppApproved { app_id, .. }
        | Body::AppRevoked { app_id, .. }
        | Body::AppProvisioned { app_id, .. }
        | Body::DirectoryWithdrawn { app_id, .. } => *app_id,
        Body::MigrationDeclared { new_namespace, .. }
        | Body::LensSuccessor { new_namespace, .. } => *new_namespace,
    }
}

// ---------- codec ----------

type EncErr = minicbor::encode::Error<core::convert::Infallible>;

fn put_bytes32(e: &mut Encoder<&mut Vec<u8>>, key: u64, v: &[u8; 32]) -> Result<(), EncErr> {
    e.u64(key)?.bytes(v)?;
    Ok(())
}

fn put_opaque(e: &mut Encoder<&mut Vec<u8>>, key: u64, v: &OpaqueBytes) -> Result<(), EncErr> {
    e.u64(key)?.bytes(&v.0)?;
    Ok(())
}

fn map(e: &mut Encoder<&mut Vec<u8>>, len: u64) -> Result<(), EncErr> {
    e.map(len)?;
    Ok(())
}

pub fn encode_body(body: &Body, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), GovernanceError> {
    let r: Result<(), EncErr> = (|| {
        match body {
            Body::Genesis => map(e, 0)?,
            Body::ActorBinding {
                bound_receiver,
                encryption_key,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, bound_receiver)?;
                put_bytes32(e, 1, encryption_key)?;
            }
            Body::RoleDecision {
                role_instance_id,
                covering_parent_fingerprint,
                granted_fingerprints,
            } => {
                map(e, 3)?;
                put_bytes32(e, 0, role_instance_id)?;
                put_bytes32(e, 1, covering_parent_fingerprint)?;
                e.u64(2)?.array(granted_fingerprints.len() as u64)?;
                for f in granted_fingerprints {
                    e.bytes(f)?;
                }
            }
            Body::CapabilityIssued {
                covering_parent_fingerprint,
                child_fingerprint,
                parent_capability_bytes,
                child_capability_bytes,
            } => {
                map(e, 4)?;
                put_bytes32(e, 0, covering_parent_fingerprint)?;
                put_bytes32(e, 1, child_fingerprint)?;
                put_opaque(e, 2, parent_capability_bytes)?;
                put_opaque(e, 3, child_capability_bytes)?;
            }
            Body::CapabilityRenewed {
                covering_parent_fingerprint,
                child_fingerprint,
                replaces_fingerprint,
                parent_capability_bytes,
                child_capability_bytes,
            } => {
                map(e, 5)?;
                put_bytes32(e, 0, covering_parent_fingerprint)?;
                put_bytes32(e, 1, child_fingerprint)?;
                put_bytes32(e, 2, replaces_fingerprint)?;
                put_opaque(e, 3, parent_capability_bytes)?;
                put_opaque(e, 4, child_capability_bytes)?;
            }
            Body::CapabilityRevoked {
                target_fingerprint,
                cutoffs,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, target_fingerprint)?;
                e.u64(1)?.array(cutoffs.len() as u64)?;
                for c in cutoffs {
                    e.array(3)?;
                    e.bytes(&c.actor_id)?;
                    e.bytes(&c.receiver_id)?;
                    e.bytes(&c.action_head)?;
                }
            }
            Body::Checkpoint {
                checkpoint_id,
                merged_frontier_hash,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, checkpoint_id)?;
                put_bytes32(e, 1, merged_frontier_hash)?;
            }
            Body::ActionReceipt { receipt } => {
                map(e, 1)?;
                put_opaque(e, 0, receipt)?;
            }
            Body::Proposal { proposal } => {
                map(e, 1)?;
                put_opaque(e, 0, proposal)?;
            }
            Body::MemberDecision {
                member_actor,
                decision,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, member_actor)?;
                put_opaque(e, 1, decision)?;
            }
            Body::InviteManagerDecision {
                invite_id,
                decision,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, invite_id)?;
                put_opaque(e, 1, decision)?;
            }
            Body::InviteResponse {
                invite_id,
                response,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, invite_id)?;
                put_opaque(e, 1, response)?;
            }
            Body::InviteActivation {
                invite_id,
                activation,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, invite_id)?;
                put_opaque(e, 1, activation)?;
            }
            Body::AppealSubmitted { action_id, appeal } => {
                map(e, 2)?;
                put_bytes32(e, 0, action_id)?;
                put_opaque(e, 1, appeal)?;
            }
            Body::AppealResolved {
                action_id,
                resolution,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, action_id)?;
                put_opaque(e, 1, resolution)?;
            }
            Body::AppApproved {
                app_id,
                manifest_digest,
                granted_permissions_cbor,
            } => {
                map(e, 3)?;
                put_bytes32(e, 0, app_id)?;
                put_bytes32(e, 1, manifest_digest)?;
                put_opaque(e, 2, granted_permissions_cbor)?;
            }
            Body::AppRevoked { app_id, reason } => {
                map(e, 2)?;
                put_bytes32(e, 0, app_id)?;
                put_opaque(e, 1, reason)?;
            }
            Body::AppProvisioned {
                app_id,
                receiver,
                provisioning,
            } => {
                map(e, 3)?;
                put_bytes32(e, 0, app_id)?;
                put_bytes32(e, 1, receiver)?;
                put_opaque(e, 2, provisioning)?;
            }
            Body::DirectoryWithdrawn { app_id, withdrawal } => {
                map(e, 2)?;
                put_bytes32(e, 0, app_id)?;
                put_opaque(e, 1, withdrawal)?;
            }
            Body::RecoveryDeclared { recovery } => {
                map(e, 1)?;
                put_opaque(e, 0, recovery)?;
            }
            Body::MigrationDeclared {
                new_namespace,
                migration,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, new_namespace)?;
                put_opaque(e, 1, migration)?;
            }
            Body::LensSuccessor {
                new_namespace,
                successor,
            } => {
                map(e, 2)?;
                put_bytes32(e, 0, new_namespace)?;
                put_opaque(e, 1, successor)?;
            }
        }
        Ok(())
    })();
    r.map_err(|_| GovernanceError::Malformed)
}

// ---------- decode helpers (mirror site/manifest.rs discipline) ----------

fn dmap(d: &mut Decoder<'_>, want: u64) -> Result<(), GovernanceError> {
    match d.map().map_err(|_| GovernanceError::Malformed)? {
        Some(n) if n == want => Ok(()),
        _ => Err(GovernanceError::Malformed),
    }
}

fn dkey(d: &mut Decoder<'_>, want: u64) -> Result<(), GovernanceError> {
    if d.u64().map_err(|_| GovernanceError::Malformed)? != want {
        return Err(GovernanceError::Malformed);
    }
    Ok(())
}

fn d32(d: &mut Decoder<'_>) -> Result<[u8; 32], GovernanceError> {
    let b = d.bytes().map_err(|_| GovernanceError::Malformed)?;
    <[u8; 32]>::try_from(b).map_err(|_| GovernanceError::Malformed)
}

fn dkey32(d: &mut Decoder<'_>, key: u64) -> Result<[u8; 32], GovernanceError> {
    dkey(d, key)?;
    d32(d)
}

fn dopaque(d: &mut Decoder<'_>, key: u64) -> Result<OpaqueBytes, GovernanceError> {
    dkey(d, key)?;
    let b = d.bytes().map_err(|_| GovernanceError::Malformed)?;
    if b.len() > MAX_GOVERNANCE_RECORD_BYTES {
        return Err(GovernanceError::RecordTooLarge {
            bytes: b.len(),
            max: MAX_GOVERNANCE_RECORD_BYTES,
        });
    }
    Ok(OpaqueBytes(b.to_vec()))
}

pub fn decode_body(kind: RecordKind, d: &mut Decoder<'_>) -> Result<Body, GovernanceError> {
    Ok(match kind {
        RecordKind::Genesis => {
            dmap(d, 0)?;
            Body::Genesis
        }
        RecordKind::ActorBinding => {
            dmap(d, 2)?;
            Body::ActorBinding {
                bound_receiver: dkey32(d, 0)?,
                encryption_key: dkey32(d, 1)?,
            }
        }
        RecordKind::RoleDecision => {
            dmap(d, 3)?;
            let role_instance_id = dkey32(d, 0)?;
            let covering_parent_fingerprint = dkey32(d, 1)?;
            dkey(d, 2)?;
            let n = d
                .array()
                .map_err(|_| GovernanceError::Malformed)?
                .ok_or(GovernanceError::Malformed)?;
            let mut granted_fingerprints = Vec::with_capacity(n as usize);
            for _ in 0..n {
                granted_fingerprints.push(d32(d)?);
            }
            Body::RoleDecision {
                role_instance_id,
                covering_parent_fingerprint,
                granted_fingerprints,
            }
        }
        RecordKind::CapabilityIssued => {
            dmap(d, 4)?;
            Body::CapabilityIssued {
                covering_parent_fingerprint: dkey32(d, 0)?,
                child_fingerprint: dkey32(d, 1)?,
                parent_capability_bytes: dopaque(d, 2)?,
                child_capability_bytes: dopaque(d, 3)?,
            }
        }
        RecordKind::CapabilityRenewed => {
            dmap(d, 5)?;
            Body::CapabilityRenewed {
                covering_parent_fingerprint: dkey32(d, 0)?,
                child_fingerprint: dkey32(d, 1)?,
                replaces_fingerprint: dkey32(d, 2)?,
                parent_capability_bytes: dopaque(d, 3)?,
                child_capability_bytes: dopaque(d, 4)?,
            }
        }
        RecordKind::CapabilityRevoked => {
            dmap(d, 2)?;
            let target_fingerprint = dkey32(d, 0)?;
            dkey(d, 1)?;
            let n = d
                .array()
                .map_err(|_| GovernanceError::Malformed)?
                .ok_or(GovernanceError::Malformed)?;
            let mut cutoffs = Vec::with_capacity(n as usize);
            for _ in 0..n {
                match d.array().map_err(|_| GovernanceError::Malformed)? {
                    Some(3) => {}
                    _ => return Err(GovernanceError::Malformed),
                }
                cutoffs.push(Cutoff {
                    actor_id: d32(d)?,
                    receiver_id: d32(d)?,
                    action_head: d32(d)?,
                });
            }
            Body::CapabilityRevoked {
                target_fingerprint,
                cutoffs,
            }
        }
        RecordKind::Checkpoint => {
            dmap(d, 2)?;
            Body::Checkpoint {
                checkpoint_id: dkey32(d, 0)?,
                merged_frontier_hash: dkey32(d, 1)?,
            }
        }
        RecordKind::ActionReceipt => {
            dmap(d, 1)?;
            Body::ActionReceipt {
                receipt: dopaque(d, 0)?,
            }
        }
        RecordKind::Proposal => {
            dmap(d, 1)?;
            Body::Proposal {
                proposal: dopaque(d, 0)?,
            }
        }
        RecordKind::MemberDecision => {
            dmap(d, 2)?;
            Body::MemberDecision {
                member_actor: dkey32(d, 0)?,
                decision: dopaque(d, 1)?,
            }
        }
        RecordKind::InviteManagerDecision => {
            dmap(d, 2)?;
            Body::InviteManagerDecision {
                invite_id: dkey32(d, 0)?,
                decision: dopaque(d, 1)?,
            }
        }
        RecordKind::InviteResponse => {
            dmap(d, 2)?;
            Body::InviteResponse {
                invite_id: dkey32(d, 0)?,
                response: dopaque(d, 1)?,
            }
        }
        RecordKind::InviteActivation => {
            dmap(d, 2)?;
            Body::InviteActivation {
                invite_id: dkey32(d, 0)?,
                activation: dopaque(d, 1)?,
            }
        }
        RecordKind::AppealSubmitted => {
            dmap(d, 2)?;
            Body::AppealSubmitted {
                action_id: dkey32(d, 0)?,
                appeal: dopaque(d, 1)?,
            }
        }
        RecordKind::AppealResolved => {
            dmap(d, 2)?;
            Body::AppealResolved {
                action_id: dkey32(d, 0)?,
                resolution: dopaque(d, 1)?,
            }
        }
        RecordKind::AppApproved => {
            dmap(d, 3)?;
            Body::AppApproved {
                app_id: dkey32(d, 0)?,
                manifest_digest: dkey32(d, 1)?,
                granted_permissions_cbor: dopaque(d, 2)?,
            }
        }
        RecordKind::AppRevoked => {
            dmap(d, 2)?;
            Body::AppRevoked {
                app_id: dkey32(d, 0)?,
                reason: dopaque(d, 1)?,
            }
        }
        RecordKind::AppProvisioned => {
            dmap(d, 3)?;
            Body::AppProvisioned {
                app_id: dkey32(d, 0)?,
                receiver: dkey32(d, 1)?,
                provisioning: dopaque(d, 2)?,
            }
        }
        RecordKind::DirectoryWithdrawn => {
            dmap(d, 2)?;
            Body::DirectoryWithdrawn {
                app_id: dkey32(d, 0)?,
                withdrawal: dopaque(d, 1)?,
            }
        }
        RecordKind::RecoveryDeclared => {
            dmap(d, 1)?;
            Body::RecoveryDeclared {
                recovery: dopaque(d, 0)?,
            }
        }
        RecordKind::MigrationDeclared => {
            dmap(d, 2)?;
            Body::MigrationDeclared {
                new_namespace: dkey32(d, 0)?,
                migration: dopaque(d, 1)?,
            }
        }
        RecordKind::LensSuccessor => {
            dmap(d, 2)?;
            Body::LensSuccessor {
                new_namespace: dkey32(d, 0)?,
                successor: dopaque(d, 1)?,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issued_body() -> Body {
        // Inline literals — no test_support yet (that arrives in Task 3). The
        // embedded bytes are placeholder here; Task 8 supplies genuine caps.
        Body::CapabilityIssued {
            covering_parent_fingerprint: [1u8; 32],
            child_fingerprint: [2u8; 32],
            parent_capability_bytes: OpaqueBytes(vec![0xAA; 200]),
            child_capability_bytes: OpaqueBytes(vec![0xBB; 260]),
        }
    }

    #[test]
    fn every_record_kind_has_a_body_variant_and_maps_back() {
        // One Body per kind; kind_of round-trips through the full tag space,
        // proving the 22-kind schema is complete. Each sample also round-trips
        // the codec and exercises target_id_of.
        let ob = || OpaqueBytes(vec![0x01, 0x02]);
        let samples = [
            Body::Genesis,
            Body::ActorBinding {
                bound_receiver: [3u8; 32],
                encryption_key: [4u8; 32],
            },
            Body::MemberDecision {
                member_actor: [5u8; 32],
                decision: ob(),
            },
            Body::InviteManagerDecision {
                invite_id: [6u8; 32],
                decision: ob(),
            },
            Body::InviteResponse {
                invite_id: [6u8; 32],
                response: ob(),
            },
            Body::InviteActivation {
                invite_id: [6u8; 32],
                activation: ob(),
            },
            Body::RoleDecision {
                role_instance_id: [7u8; 32],
                covering_parent_fingerprint: [8u8; 32],
                granted_fingerprints: vec![[9u8; 32]],
            },
            issued_body(),
            Body::CapabilityRenewed {
                covering_parent_fingerprint: [1u8; 32],
                child_fingerprint: [2u8; 32],
                replaces_fingerprint: [3u8; 32],
                parent_capability_bytes: ob(),
                child_capability_bytes: ob(),
            },
            Body::CapabilityRevoked {
                target_fingerprint: [2u8; 32],
                cutoffs: vec![Cutoff {
                    actor_id: [1u8; 32],
                    receiver_id: [2u8; 32],
                    action_head: [3u8; 32],
                }],
            },
            Body::Checkpoint {
                checkpoint_id: [10u8; 32],
                merged_frontier_hash: [11u8; 32],
            },
            Body::ActionReceipt { receipt: ob() },
            Body::Proposal { proposal: ob() },
            Body::AppealSubmitted {
                action_id: [12u8; 32],
                appeal: ob(),
            },
            Body::AppealResolved {
                action_id: [12u8; 32],
                resolution: ob(),
            },
            Body::AppApproved {
                app_id: [13u8; 32],
                manifest_digest: [14u8; 32],
                granted_permissions_cbor: ob(),
            },
            Body::AppRevoked {
                app_id: [13u8; 32],
                reason: ob(),
            },
            Body::AppProvisioned {
                app_id: [13u8; 32],
                receiver: [15u8; 32],
                provisioning: ob(),
            },
            Body::DirectoryWithdrawn {
                app_id: [13u8; 32],
                withdrawal: ob(),
            },
            Body::RecoveryDeclared { recovery: ob() },
            Body::MigrationDeclared {
                new_namespace: [16u8; 32],
                migration: ob(),
            },
            Body::LensSuccessor {
                new_namespace: [16u8; 32],
                successor: ob(),
            },
        ];
        assert_eq!(samples.len(), 22, "one sample per record kind");
        let mut seen = std::collections::BTreeSet::new();
        for body in &samples {
            let kind = kind_of(body);
            assert!((0..=21).contains(&kind.tag()));
            assert!(seen.insert(kind.tag()), "duplicate kind {kind:?}");
            let mut buf = Vec::new();
            encode_body(body, &mut Encoder::new(&mut buf)).unwrap();
            assert_eq!(&decode_body(kind, &mut Decoder::new(&buf)).unwrap(), body);
            let _ = target_id_of(kind, body, &[42u8; 32]);
        }
    }

    #[test]
    fn body_round_trips_and_wrong_kind_shape_is_malformed() {
        let body = issued_body();
        let mut buf = Vec::new();
        encode_body(&body, &mut Encoder::new(&mut buf)).unwrap();
        assert_eq!(
            decode_body(RecordKind::CapabilityIssued, &mut Decoder::new(&buf)).unwrap(),
            body
        );
        // A Genesis body decoded as CapabilityIssued must be Malformed.
        let mut g = Vec::new();
        encode_body(&Body::Genesis, &mut Encoder::new(&mut g)).unwrap();
        assert_eq!(
            decode_body(RecordKind::CapabilityIssued, &mut Decoder::new(&g)),
            Err(GovernanceError::Malformed)
        );
    }

    #[test]
    fn a_realistic_depth_16_issuance_body_fits_under_the_record_ceiling() {
        // Two ~3 KiB caps (depth-16 worst case) must stay under 16 KiB so the
        // embedded-capability design does not collide with the record ceiling.
        let body = Body::CapabilityIssued {
            covering_parent_fingerprint: [1u8; 32],
            child_fingerprint: [2u8; 32],
            parent_capability_bytes: OpaqueBytes(vec![0u8; 3 * 1024]),
            child_capability_bytes: OpaqueBytes(vec![0u8; 3 * 1024]),
        };
        let mut buf = Vec::new();
        encode_body(&body, &mut Encoder::new(&mut buf)).unwrap();
        assert!(
            buf.len() < MAX_GOVERNANCE_RECORD_BYTES,
            "issuance body must fit the record ceiling"
        );
    }
}
