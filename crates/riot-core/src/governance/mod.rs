//! Governance ledger (Slice 2). Versioned `GovernanceRecordV1` schema, actor/
//! device/action hash chains, a durable authority repository, a deterministic
//! policy evaluator, and transitive capability revocation. Governance answers
//! the product question; Meadowcap answers protocol validity. No admission,
//! sync, vault, app-broker, FFI, or UI concepts live here.

pub mod action;
pub mod actor;
pub mod authorize;
pub mod body;
pub mod evaluator;
pub mod frontier;
pub mod lineage;
pub mod paths;
pub mod record;
pub mod repository;
pub mod revoke;

#[cfg(any(test, feature = "conformance"))]
#[doc(hidden)]
pub mod test_support;

use crate::meadowcap::MeadowcapError;

/// A governance record id (domain-separated SHA-256 of the canonical record).
pub type RecordId = [u8; 32];
/// A Slice-1 capability fingerprint, reused verbatim as the governance join key.
pub type Fingerprint = [u8; 32];

/// Largest accepted governance record encoding (design resource ceilings).
pub const MAX_GOVERNANCE_RECORD_BYTES: usize = 16 * 1024;
/// Maximum accepted parent frontier references per record.
pub const MAX_PARENTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum RecordKind {
    Genesis = 0,
    ActorBinding = 1,
    MemberDecision = 2,
    InviteManagerDecision = 3,
    InviteResponse = 4,
    InviteActivation = 5,
    RoleDecision = 6,
    CapabilityIssued = 7,
    CapabilityRenewed = 8,
    CapabilityRevoked = 9,
    Checkpoint = 10,
    ActionReceipt = 11,
    Proposal = 12,
    AppealSubmitted = 13,
    AppealResolved = 14,
    AppApproved = 15,
    AppRevoked = 16,
    AppProvisioned = 17,
    DirectoryWithdrawn = 18,
    RecoveryDeclared = 19,
    MigrationDeclared = 20,
    LensSuccessor = 21,
}

impl RecordKind {
    pub fn from_tag(tag: u64) -> Result<Self, GovernanceError> {
        use RecordKind::*;
        Ok(match tag {
            0 => Genesis,
            1 => ActorBinding,
            2 => MemberDecision,
            3 => InviteManagerDecision,
            4 => InviteResponse,
            5 => InviteActivation,
            6 => RoleDecision,
            7 => CapabilityIssued,
            8 => CapabilityRenewed,
            9 => CapabilityRevoked,
            10 => Checkpoint,
            11 => ActionReceipt,
            12 => Proposal,
            13 => AppealSubmitted,
            14 => AppealResolved,
            15 => AppApproved,
            16 => AppRevoked,
            17 => AppProvisioned,
            18 => DirectoryWithdrawn,
            19 => RecoveryDeclared,
            20 => MigrationDeclared,
            21 => LensSuccessor,
            _ => return Err(GovernanceError::UnknownKind { tag }),
        })
    }
    pub fn tag(self) -> u64 {
        self as u8 as u64
    }
}

/// Stable, non-secret governance rejection codes. Every variant has a producer
/// and a test (no unreachable codes — the Slice-1 `NonCanonical` lesson).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernanceError {
    /// Bytes did not decode as a canonical `GovernanceRecordV1` / body-shape
    /// mismatch with the envelope `kind`. (Task 1/2)
    Malformed,
    /// Canonical value did not consume all input bytes. (Task 2)
    TrailingBytes,
    /// Unknown record-kind tag (fails closed). (Task 0)
    UnknownKind { tag: u64 },
    /// Encoded record larger than `MAX_GOVERNANCE_RECORD_BYTES`. (Task 2)
    RecordTooLarge { bytes: usize, max: usize },
    /// More than `MAX_PARENTS`, or parents unsorted/duplicated. (Task 2)
    ParentsInvalid,
    /// Entry path does not match the record kind's canonical target. (Task 4)
    PathBindingMismatch,
    /// Per-actor sequence gap, fork, or wrong `prev_actor_record`. (Task 5)
    ActorChainBroken,
    /// A receipt referenced a receipt/itself, a missing action, or a
    /// privileged action had no paired receipt. (Task 7)
    ActionChainInvalid,
    /// A record purported to authorize itself. (Task 8)
    SelfAuthorization,
    /// Issuance body's embedded child is not a valid attenuation-descendant of
    /// its named parent (fingerprint forgery or non-descendant). (Task 8)
    IssuanceNotAttenuated,
    /// Underlying capability decode/validity failure from Slice 1. (Task 8)
    Capability(MeadowcapError),
    /// Durable-store failure surfaced non-gated so wasm (Memory-only) compiles.
    /// (Task 13)
    Storage,
}

impl std::fmt::Display for GovernanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for GovernanceError {}
impl From<MeadowcapError> for GovernanceError {
    fn from(e: MeadowcapError) -> Self {
        GovernanceError::Capability(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceilings_match_design_resource_limits() {
        assert_eq!(MAX_GOVERNANCE_RECORD_BYTES, 16 * 1024);
        assert_eq!(MAX_PARENTS, 16);
    }

    #[test]
    fn every_kind_round_trips_its_tag_and_unknown_fails_closed() {
        for tag in 0u64..=21 {
            assert_eq!(RecordKind::from_tag(tag).unwrap().tag(), tag);
        }
        assert_eq!(
            RecordKind::from_tag(22),
            Err(GovernanceError::UnknownKind { tag: 22 })
        );
    }
}
