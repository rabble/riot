//! Owned personal-space primitives. Kept out of `identity.rs` so the communal
//! sealed-identity invariants there are never accidentally loosened.
//!
//! Slice 1 Task 1 scope: owned-namespace generation only. The owned write
//! capability, the space author, and the sealed owned-root envelope arrive in
//! later tasks (see `docs/superpowers/plans/2026-07-12-personal-spaces-slice1.md`).

use willow25::prelude::*;
use zeroize::Zeroize;

use super::identity::os_fill;
use super::WillowError;

/// Custodian of a personal space. Holds the owned namespace root secret; the
/// namespace ID is the root public key. Neither `Clone` nor `Debug` — the root
/// secret must never be duplicated or printed. Unlike a communal namespace,
/// whose secret confers no privilege and is discarded at generation, this
/// secret IS the space's root authority and is retained.
pub struct OwnedRoot {
    namespace_id: NamespaceId,
    // Retained root authority; consumed by owned write-capability minting in
    // Slice 1 Task 2.
    namespace_secret: NamespaceSecret,
}

impl OwnedRoot {
    /// Draws namespace candidates until the public key is owned, mirroring the
    /// rejection-sampling pattern of communal generation in `identity.rs`. Each
    /// rejected draw is zeroized. The retained secret is the space's root.
    pub fn generate() -> Result<Self, WillowError> {
        for _ in 0..128 {
            let mut secret_bytes = [0u8; 32];
            let result = os_fill(&mut secret_bytes);
            let secret = result.map(|()| NamespaceSecret::from_bytes(&secret_bytes));
            secret_bytes.zeroize();
            let secret = secret?;
            let namespace_id = secret.corresponding_namespace_id();
            if namespace_id.is_owned() {
                return Ok(Self {
                    namespace_id,
                    namespace_secret: secret,
                });
            }
        }
        Err(WillowError::EntropyUnavailable)
    }

    /// The owned namespace's public identity — the root public key.
    pub fn namespace_id(&self) -> &NamespaceId {
        &self.namespace_id
    }

    /// Borrow the retained owned-namespace root secret for capability minting.
    /// `pub(crate)` — the secret never leaves the crate and never crosses FFI.
    pub(crate) fn namespace_secret_ref(&self) -> &NamespaceSecret {
        &self.namespace_secret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_owned_namespace_reports_owned() {
        let root = OwnedRoot::generate().expect("entropy");
        assert!(root.namespace_id().is_owned());
        assert!(!root.namespace_id().is_communal());
    }

    #[test]
    fn owned_root_exposes_secret_ref_for_minting() {
        let root = OwnedRoot::generate().expect("owned root");
        let secret = root.namespace_secret_ref();
        assert_eq!(secret.corresponding_namespace_id(), *root.namespace_id());
    }

    #[test]
    fn communal_and_owned_are_disjoint() {
        // A communal author's namespace is never owned, and vice versa. The
        // kinds are distinguished by the namespace ID itself.
        let communal = crate::willow::generate_space_organizer_author().expect("entropy");
        assert!(communal.namespace_id().is_communal());
        let owned = OwnedRoot::generate().expect("entropy");
        assert!(owned.namespace_id().is_owned());
    }
}
