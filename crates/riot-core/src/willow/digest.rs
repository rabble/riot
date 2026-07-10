//! Digest domains, exactly as specified. Canonical Willow value identity
//! (`entry_id`) is deliberately separate from authorization-proof identity
//! (`evidence_digest`): the same entry re-proved under a different
//! capability keeps its identity but not its evidence digest.

use sha2::{Digest, Sha256};

pub type EntryId = [u8; 32];
pub type EvidenceDigest = [u8; 32];
pub type ObjectDigest = [u8; 32];
pub type BundleDigest = [u8; 32];

/// Corrected WILLIAM3 of a payload — the Willow'25 payload digest.
pub fn william3_digest(payload: &[u8]) -> [u8; 32] {
    *willow25::entry::PayloadDigest::from_payload(payload).as_bytes()
}

/// Value identity of a canonical Willow entry.
/// `SHA256("riot/willow-entry-id/v1" || u32be(len) || entry_bytes)`.
pub fn entry_id(entry_bytes: &[u8]) -> EntryId {
    let mut hasher = Sha256::new();
    hasher.update(b"riot/willow-entry-id/v1");
    hasher.update((entry_bytes.len() as u32).to_be_bytes());
    hasher.update(entry_bytes);
    hasher.finalize().into()
}

/// Proof identity binding the entry to its capability and signature.
pub fn evidence_digest(
    entry_bytes: &[u8],
    capability_bytes: &[u8],
    signature: &[u8; 64],
) -> EvidenceDigest {
    let mut hasher = Sha256::new();
    hasher.update(b"riot/evidence-digest/v1");
    hasher.update((entry_bytes.len() as u32).to_be_bytes());
    hasher.update(entry_bytes);
    hasher.update((capability_bytes.len() as u32).to_be_bytes());
    hasher.update(capability_bytes);
    hasher.update(signature);
    hasher.finalize().into()
}

/// SHA-256 of the deterministic alert payload bytes (local artifact tooling).
pub fn object_digest(payload: &[u8]) -> ObjectDigest {
    Sha256::digest(payload).into()
}

/// SHA-256 of a complete `.riot-evidence` artifact.
pub fn bundle_digest(bundle_bytes: &[u8]) -> BundleDigest {
    Sha256::digest(bundle_bytes).into()
}
