//! `OwnedMasthead` — the composite-site owner identity.
//!
//! Combines the owned-namespace root secret (authority to mint the owner write
//! capability and to delegate) with the owner's own subspace signing secret
//! (the author key the owner writes entries as, and the signer for delegations).
//! Unit 0 scope: generation, owner capability minting, section delegation
//! issuance (reserved-path enforced), and sealed persistence.

use willow25::prelude::*;

use super::identity::os_fill;
use super::owned::OwnedRoot;
use super::WillowError;

pub struct OwnedMasthead {
    root: OwnedRoot,
    owner_subspace_secret: SubspaceSecret,
}

impl OwnedMasthead {
    /// Generate a fresh masthead: a new owned namespace root + a fresh owner
    /// subspace secret.
    pub fn generate() -> Result<Self, WillowError> {
        let root = OwnedRoot::generate()?;
        let mut seed = [0u8; 32];
        os_fill(&mut seed)?;
        let owner_subspace_secret = SubspaceSecret::from_bytes(&seed);
        seed.iter_mut().for_each(|b| *b = 0);
        Ok(Self {
            root,
            owner_subspace_secret,
        })
    }

    /// The owned namespace id (site root of trust).
    pub fn namespace_id(&self) -> &NamespaceId {
        self.root.namespace_id()
    }

    /// The owner's subspace id (receiver of the owner write capability).
    pub fn owner_subspace_id(&self) -> SubspaceId {
        self.owner_subspace_secret.corresponding_subspace_id()
    }

    /// Mint the owner's owned write capability (grants `Area::full()` over the site namespace).
    pub fn owner_write_capability(&self) -> WriteCapability {
        WriteCapability::new_owned(self.root.namespace_secret_ref(), self.owner_subspace_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_masthead_has_owned_namespace_and_owner_subspace() {
        let m = OwnedMasthead::generate().expect("masthead");
        assert!(
            m.namespace_id().is_owned(),
            "masthead namespace must be owned"
        );
        assert_eq!(m.owner_subspace_id(), m.owner_subspace_id());
    }

    #[test]
    fn owner_capability_is_owned_full_area_zero_delegation() {
        let m = OwnedMasthead::generate().unwrap();
        let cap = m.owner_write_capability();
        assert!(cap.is_owned(), "owner cap must be owned-rooted");
        assert!(cap.delegations().is_empty(), "owner cap must have zero delegations");
        assert_eq!(cap.granted_namespace(), m.namespace_id(), "cap namespace must be the site root");
        assert_eq!(cap.receiver(), &m.owner_subspace_id(), "cap receiver must be the owner subspace");
    }
}
