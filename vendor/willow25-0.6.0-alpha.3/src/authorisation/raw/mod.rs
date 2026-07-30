//! Lower-level details of Meadowcap.
//!
//! In particular, this module allows working with capabilities that are not guaranteed to be [valid](https://willowprotocol.org/specs/meadowcap/index.html#cap_valid).

use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use crate::prelude::*;

mod possibly_valid_write_capability;
pub use possibly_valid_write_capability::*;

mod possibly_valid_read_capability;
pub use possibly_valid_read_capability::*;

pub use meadowcap::raw::{AccessMode, InvalidCapability};

wrapper! {
    /// The information in a [communal capability](https://willowprotocol.org/specs/meadowcap/index.html#CommunalCapability) except for its [delegations](https://willowprotocol.org/specs/meadowcap/index.html#communal_cap_delegations).
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    CommunalGenesis; meadowcap::raw::CommunalGenesis<NamespaceId, SubspaceId>
}

impl fmt::Debug for CommunalGenesis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

wrapper! {
    /// The information in an [owned capability](https://willowprotocol.org/specs/meadowcap/index.html#OwnedCapability) except for its [delegations](https://willowprotocol.org/specs/meadowcap/index.html#owned_cap_delegations).
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    OwnedGenesis; meadowcap::raw::OwnedGenesis<NamespaceId, NamespaceSignature, SubspaceId>
}

impl fmt::Debug for OwnedGenesis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

wrapper! {
    /// The information in a capability except for its delegations.
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    Genesis; meadowcap::raw::Genesis<NamespaceId, NamespaceSignature, SubspaceId>
}

impl fmt::Debug for Genesis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Genesis {
    /// Returns the access mode of the genesis.
    pub fn access_mode(&self) -> AccessMode {
        self.0.access_mode()
    }

    /// Returns the namespace key of the genesis.
    pub fn namespace_key(&self) -> &NamespaceId {
        self.0.namespace_key()
    }

    /// Returns the user key of the genesis.
    pub fn user_key(&self) -> &SubspaceId {
        self.0.user_key()
    }

    /// Returns `true` if and only if this is the genesis of an owned capability.
    pub fn is_owned(&self) -> bool {
        self.0.is_owned()
    }

    /// Returns whether the given area is contained in the [granted area](PossiblyValidWriteCapability::granted_area_ref) of this genesis.
    pub fn includes_area(&self, area: &Area) -> bool {
        self.0.includes_area(area.into())
    }

    /// Returns whether this genesis is valid.
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }
}

wrapper! {
    /// The information describing a single delegation of a capability.
    ///
    /// Specification: [communal](https://willowprotocol.org/specs/meadowcap/index.html#communal_cap_delegations) and [owned](https://willowprotocol.org/specs/meadowcap/index.html#owned_cap_delegations) (both are identical data types).
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    Delegation; meadowcap::raw::Delegation<MCL, MCC, MPL, SubspaceId, SubspaceSignature>
}

impl fmt::Debug for Delegation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Delegation {
    /// Creates a new delegation, with a given area, user public key, and signature.
    pub fn new(area: Area, user: SubspaceId, signature: SubspaceSignature) -> Self {
        (meadowcap::raw::Delegation {
            area: area.into(),
            user,
            signature,
        })
        .into()
    }

    /// Returns a reference to the area of the delegation.
    pub fn area(&self) -> &Area {
        (&self.0.area).into()
    }

    /// Sets the area of the delegation.
    pub fn set_area(&mut self, area: Area) {
        self.0.area = area.into();
    }

    /// Returns a reference to the user public key of the delegation.
    pub fn user(&self) -> &SubspaceId {
        &self.0.user
    }

    /// Sets the user public key of the delegation.
    pub fn set_user(&mut self, user: SubspaceId) {
        self.0.user = user;
    }

    /// Returns a reference to the signature of the delegation.
    pub fn signature(&self) -> &SubspaceSignature {
        &self.0.signature
    }

    /// Sets the signature of the delegation.
    pub fn set_signature(&mut self, signature: SubspaceSignature) {
        self.0.signature = signature;
    }

    /// Takes ownership of the delegation and returns its area, user, and signature.
    pub fn into_parts(self) -> (Area, SubspaceId, SubspaceSignature) {
        (self.0.area.into(), self.0.user, self.0.signature)
    }
}
