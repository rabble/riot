//! Capability fingerprints. The design pins the EXACT preimage
//! `SHA-256("riot/meadowcap-fingerprint/v1" || canonical_capability_bytes)`
//! with NO length prefix — deliberately different from
//! `willow::digest::entry_id`, which prepends a u32 length. This fingerprint is
//! the join key the governance ledger (Slice 2) uses; its bytes must match the
//! spec exactly. The canonical bytes already bind type, access mode, namespace,
//! receiver, area, and every delegation signature, so read and write caps of
//! the same shape produce different fingerprints.

use sha2::{Digest, Sha256};
use willow25::authorisation::{ReadCapability, WriteCapability};

use super::codec::encode_read_capability;

pub type CapabilityFingerprint = [u8; 32];

const FINGERPRINT_DOMAIN: &[u8] = b"riot/meadowcap-fingerprint/v1";

fn fingerprint_of_canonical_bytes(canonical: &[u8]) -> CapabilityFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_DOMAIN);
    hasher.update(canonical);
    hasher.finalize().into()
}

pub fn write_capability_fingerprint(cap: &WriteCapability) -> CapabilityFingerprint {
    fingerprint_of_canonical_bytes(&crate::willow::encode_capability(cap))
}

pub fn read_capability_fingerprint(cap: &ReadCapability) -> CapabilityFingerprint {
    fingerprint_of_canonical_bytes(&encode_read_capability(cap))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meadowcap::create::{new_communal_read, new_communal_write};
    use willow25::prelude::{NamespaceId, SubspaceSecret};

    #[test]
    fn fingerprint_is_deterministic_and_domain_separated() {
        let ns = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id();
        let cap = new_communal_write(ns.clone(), receiver.clone());

        let fp1 = write_capability_fingerprint(&cap);
        let fp2 = write_capability_fingerprint(&cap);
        assert_eq!(fp1, fp2, "fingerprint must be deterministic");

        // Domain separation: a raw SHA-256 of the same bytes (no domain) differs.
        let raw: [u8; 32] = Sha256::digest(crate::willow::encode_capability(&cap)).into();
        assert_ne!(fp1, raw, "domain prefix must change the digest");
    }

    #[test]
    fn read_and_write_caps_of_same_shape_have_different_fingerprints() {
        let ns = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id();
        let w = write_capability_fingerprint(&new_communal_write(ns.clone(), receiver.clone()));
        let r = read_capability_fingerprint(&new_communal_read(ns, receiver));
        assert_ne!(w, r, "access mode is bound in the canonical bytes");
    }
}
