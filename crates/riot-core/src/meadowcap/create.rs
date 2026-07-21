//! Communal and owned read/write capability creation over `willow25`.

use willow25::authorisation::{ReadCapability, WriteCapability};
use willow25::prelude::{NamespaceId, NamespaceSecret, SubspaceId};

/// A communal write capability for `user_key`'s own subspace in `namespace`.
pub fn new_communal_write(namespace: NamespaceId, user_key: SubspaceId) -> WriteCapability {
    WriteCapability::new_communal(namespace, user_key)
}

/// A communal read capability for `user_key`'s own subspace in `namespace`.
pub fn new_communal_read(namespace: NamespaceId, user_key: SubspaceId) -> ReadCapability {
    ReadCapability::new_communal(namespace, user_key)
}

/// An owned write capability granting `Area::full()` of the owned namespace,
/// received by `user_key`. `namespace_secret` is the owned-namespace root; it
/// stays inside `riot-core` and never crosses FFI.
pub fn new_owned_write(namespace_secret: &NamespaceSecret, user_key: SubspaceId) -> WriteCapability {
    WriteCapability::new_owned(namespace_secret, user_key)
}

/// An owned read capability granting `Area::full()` of the owned namespace.
pub fn new_owned_read(namespace_secret: &NamespaceSecret, user_key: SubspaceId) -> ReadCapability {
    ReadCapability::new_owned(namespace_secret, user_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use willow25::prelude::{Area, SubspaceSecret};

    fn owned_namespace_secret() -> NamespaceSecret {
        // Seeded: ed25519 keygen is deterministic, so tests are reproducible.
        NamespaceSecret::from_bytes(&[3u8; 32])
    }

    #[test]
    fn owned_write_cap_is_owned_full_area_zero_delegation() {
        let ns = owned_namespace_secret();
        let receiver = SubspaceSecret::from_bytes(&[4u8; 32]).corresponding_subspace_id();
        let cap = new_owned_write(&ns, receiver.clone());
        assert!(cap.is_owned());
        assert!(cap.delegations().is_empty());
        assert_eq!(cap.receiver(), &receiver);
        assert_eq!(cap.granted_namespace(), &ns.corresponding_namespace_id());
        assert_eq!(cap.granted_area(), Area::full());
    }

    #[test]
    fn communal_read_cap_is_not_owned_and_scopes_to_receiver_subspace() {
        let namespace = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[5u8; 32]).corresponding_subspace_id();
        let cap = new_communal_read(namespace.clone(), receiver.clone());
        assert!(!cap.is_owned());
        assert_eq!(cap.receiver(), &receiver);
        assert_eq!(cap.granted_namespace(), &namespace);
        assert_eq!(cap.granted_area(), Area::new_subspace_area(receiver));
    }
}
