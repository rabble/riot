//! Meadowcap capability core (Slice 1). Wraps the pinned `willow25` Meadowcap
//! implementation and owns all protocol-level capability operations: creation,
//! delegation, canonical codec, inspection, verification, and fingerprints.
//!
//! This module contains no admission, sync, governance, or FFI concepts. It
//! returns typed facts and the stable rejection codes in `MeadowcapError`.

pub mod codec;
pub mod create;
pub mod delegate;
pub mod fingerprint;
pub mod inspect;
pub mod verify;

pub use willow25::authorisation::{ReadCapability, WriteCapability};

/// Maximum delegation-chain depth admitted before recursive verification.
/// Pinned by the design's resource ceilings. Lowering requires measured
/// fixtures; raising requires a security review and updated exhaustion tests.
pub const MAX_DELEGATION_DEPTH: usize = 16;

/// Maximum encoded capability size (bytes) admitted before allocation/verify.
pub const MAX_CAPABILITY_ENCODED_BYTES: usize = 64 * 1024;

/// Meadowcap access mode as a typed fact (never string-parsed by consumers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
}

/// Whether a capability is rooted in a communal or an owned namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    Communal,
    Owned,
}

/// Stable, non-secret capability rejection codes for Slice 1 surfaces
/// (creation, delegation, codec, inspection, verification). The broader
/// admission taxonomy (stale-policy, revoked, missing-parent, …) is Slice 3.
///
/// NOTE on folded conditions (both proven by crate source, tested in Task 9):
/// - **Non-canonical encodings** are rejected by willow25's canonical decode,
///   which uses `produce_decoded_canonic`
///   (`meadowcap-0.5.0/src/raw/possibly_valid_write_capability.rs:1048` — this is
///   the underlying meadowcap crate's file, 1157 lines, NOT the 587-line
///   willow25 wrapper file of the same name); the `DecodableCanonic` contract
///   rejects non-minimal compact-width forms as decode errors, so they surface
///   here as `Malformed`. There is deliberately no `NonCanonical` variant —
///   nothing in Slice 1 can construct one after decode, so declaring it would
///   ship an unreachable code. Slice 3's management taxonomy may reintroduce a
///   distinct code if a producer emerges.
/// - **Wrong-access-mode bytes** (read-capability bytes fed to the write decoder
///   or vice versa) are rejected because the decoder returns `Err` unless the
///   decoded access mode matches
///   (`meadowcap-0.5.0/src/raw/possibly_valid_write_capability.rs:997` in the
///   non-canonic path, `:1050` in the canonic path; the read decoder has the
///   symmetric Read check), so they too surface as `Malformed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeadowcapError {
    /// Bytes did not decode as a canonical, valid capability of the requested
    /// access mode. Covers structural garbage, invalid chain signatures,
    /// non-canonical encodings, and wrong-access-mode bytes (see the note above).
    Malformed,
    /// Canonical value did not consume all input bytes.
    TrailingBytes,
    /// Delegation chain deeper than `MAX_DELEGATION_DEPTH`.
    ChainTooDeep { depth: usize, max: usize },
    /// Encoded capability larger than `MAX_CAPABILITY_ENCODED_BYTES`.
    CapabilityTooLarge { bytes: usize, max: usize },
    /// Delegation would widen authority (new area not contained in prior area).
    AuthorityExpanding,
    /// Delegation signer's public key is not the capability's current receiver.
    ReceiverMismatch,
}

impl std::fmt::Display for MeadowcapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for MeadowcapError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceilings_match_design_resource_limits() {
        assert_eq!(MAX_DELEGATION_DEPTH, 16);
        assert_eq!(MAX_CAPABILITY_ENCODED_BYTES, 64 * 1024);
    }

    #[test]
    fn access_mode_and_kind_are_distinct_facts() {
        assert_ne!(AccessMode::Read, AccessMode::Write);
        assert_ne!(CapabilityKind::Communal, CapabilityKind::Owned);
    }
}
