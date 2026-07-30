use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use meadowcap::raw::InvalidCapability;

use ufotofu::codec_prelude::*;

use crate::{
    authorisation::{
        ReadCapability,
        raw::{Delegation, Genesis},
    },
    prelude::*,
};

wrapper! {
    /// A read capability that has not been checked for [validity](https://willowprotocol.org/specs/meadowcap/index.html#cap_valid).
    ///
    /// This is a low-level type. If possible, you should prefer using [`ReadCapability`](super::ReadCapability).
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(namespace_id, subspace_id.clone());
    ///
    /// assert_eq!(cap.receiver(), &subspace_id);
    ///
    /// let delegated_user = SubspaceId::from_bytes(&[18; 32]);
    ///
    /// let delegation = Delegation::new(
    ///     Area::new_subspace_area(subspace_id),
    ///     delegated_user.clone(),
    ///     SubspaceSignature::from([19; 64]),
    /// );
    ///
    /// cap.append_delegation(delegation.clone());
    ///
    /// assert_eq!(cap.receiver(), &delegated_user);
    /// # }
    /// ```
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    PossiblyValidReadCapability; meadowcap::raw::PossiblyValidReadCapability<MCL, MCC, MPL, NamespaceId, NamespaceSignature, SubspaceId, SubspaceSignature>
}

impl fmt::Debug for PossiblyValidReadCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PossiblyValidReadCapability {
    /// Returns the [receiver](https://willowprotocol.org/specs/meadowcap/index.html#cap_receiver) of this capability (and simply assumes the capability is valid).
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let cap = PossiblyValidReadCapability::new_communal(namespace_id, subspace_id.clone());
    ///
    /// assert_eq!(cap.receiver(), &subspace_id);
    /// # }
    /// ```
    pub fn receiver(&self) -> &SubspaceId {
        self.0.receiver()
    }

    /// Returns the [namespace id to which this grants access](https://willowprotocol.org/specs/meadowcap/index.html#cap_granted_namespace) (and simply assumes the capability is valid).
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let cap = PossiblyValidReadCapability::new_communal(namespace_id.clone(), subspace_id);
    ///
    /// assert_eq!(cap.granted_namespace(), &namespace_id);
    /// # }
    /// ```
    pub fn granted_namespace(&self) -> &NamespaceId {
        self.0.granted_namespace()
    }

    /// Returns a reference to the [area to which this grants access](https://willowprotocol.org/specs/meadowcap/index.html#cap_granted_area) (and simply assumes the capability is valid), or `None` if there is no delegation step which restricts the initial area.
    ///
    /// This method is slightly inconvenient to work with (you need special logic if there are no delegation steps), but it is efficient because it never explicitly creates a new area value. For the more convenient version, see [`granted_area`](PossiblyValidReadCapability::granted_area).
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(namespace_id, subspace_id.clone());
    ///
    /// assert_eq!(cap.granted_area_ref(), None);
    ///
    /// let delegation = Delegation::new(
    ///     Area::new_subspace_area(subspace_id.clone()),
    ///     SubspaceId::from_bytes(&[18; 32]),
    ///     SubspaceSignature::from([19; 64]),
    /// );
    ///
    /// cap.append_delegation(delegation.clone());
    ///
    /// assert_eq!(cap.granted_area_ref(), Some(&Area::new_subspace_area(subspace_id)));
    /// # }
    /// ```
    pub fn granted_area_ref(&self) -> Option<&Area> {
        self.0.granted_area_ref().map(Into::into)
    }

    /// Returns the [`Genesis`] of this capability.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(namespace_id.clone(), subspace_id.clone());
    ///
    /// let genesis = cap.genesis();
    ///
    /// assert_eq!(genesis.access_mode(), AccessMode::Read);
    /// assert_eq!(genesis.namespace_key(), &namespace_id);
    /// assert_eq!(genesis.user_key(), &subspace_id);
    /// # }
    /// ```
    pub fn genesis(&self) -> &Genesis {
        self.0.genesis().into()
    }

    /// Returns `true` if and only if this is a capability for an owned namespace.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut owncap = PossiblyValidReadCapability::new_owned(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    ///     NamespaceSignature::from([18; 64]),
    /// );
    ///
    /// assert!(owncap.is_owned());
    ///
    /// let mut comcap = PossiblyValidReadCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert!(!comcap.is_owned());
    /// # }
    /// ```
    pub fn is_owned(&self) -> bool {
        self.0.is_owned()
    }

    /// Returns the [`Delegations`](Delegation) of this capability as a slice.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(namespace_id, subspace_id.clone());
    ///
    /// assert!(cap.delegations().is_empty());
    ///
    /// let delegation = Delegation::new(
    ///     Area::new_subspace_area(subspace_id.clone()),
    ///     SubspaceId::from_bytes(&[18; 32]),
    ///     SubspaceSignature::from([19; 64]),
    /// );
    ///
    /// cap.append_delegation(delegation.clone());
    ///
    /// assert_eq!(cap.delegations()[0], delegation);
    /// # }
    /// ```
    pub fn delegations(&self) -> &[Delegation] {
        let inner_delegations = self.0.delegations();
        let as_delegation25_ptr = inner_delegations.as_ptr() as *const Delegation;

        // SAFETY: all necessary invariants are already upheld by the original slice, and the layout is identical because `willow25::authorisation::raw::Delegation` is `repr(transparent)`
        unsafe { core::slice::from_raw_parts(as_delegation25_ptr, inner_delegations.len()) }
    }

    /// Creates a new [communal](https://willowprotocol.org/specs/meadowcap/index.html#communal_capabilities) read capability with no delegations.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert!(!cap.is_owned());
    /// assert_eq!(cap.receiver(), &subspace_id);
    /// assert_eq!(cap.granted_namespace(), &namespace_id);
    /// assert_eq!(cap.granted_area(), Area::new_subspace_area(subspace_id));
    /// assert!(cap.delegations().is_empty());
    /// # }
    /// ```
    pub fn new_communal(namespace_key: NamespaceId, user_key: SubspaceId) -> Self {
        Self(meadowcap::raw::PossiblyValidReadCapability::new_communal(
            namespace_key,
            user_key,
        ))
    }

    /// Creates a new [owned](https://willowprotocol.org/specs/meadowcap/index.html#owned_capabilities) read capability with no delegations.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_owned(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    ///     NamespaceSignature::from([18; 64]),
    /// );
    ///
    /// assert!(cap.is_owned());
    /// assert_eq!(cap.receiver(), &subspace_id);
    /// assert_eq!(cap.granted_namespace(), &namespace_id);
    /// assert_eq!(cap.granted_area(), Area::full());
    /// assert!(cap.delegations().is_empty());
    /// # }
    /// ```
    pub fn new_owned(
        namespace_key: NamespaceId,
        user_key: SubspaceId,
        initial_authorisation: NamespaceSignature,
    ) -> Self {
        Self(meadowcap::raw::PossiblyValidReadCapability::new_owned(
            namespace_key,
            user_key,
            initial_authorisation,
        ))
    }

    /// Converts `self` into a [`ReadCapability`] **without enforcing [validity](https://willowprotocol.org/specs/meadowcap/index.html#cap_valid)**.
    ///
    /// See [`into_read_capability`](PossiblyValidReadCapability::into_read_capability) for the safe (but less performant) version of this.
    ///
    /// #### Safety
    ///
    /// Undefined behaviour may occur if `self` is not [valid](https://willowprotocol.org/specs/meadowcap/index.html#cap_valid).
    pub unsafe fn into_read_capability_unchecked(self) -> ReadCapability {
        // SAFETY: the contract for safe calls to this function is identical to that of the function call in the unsafe block.
        unsafe { self.0.into_read_capability_unchecked().into() }
    }

    /// Returns whether the given area is contained in the [granted area](PossiblyValidReadCapability::granted_area_ref) of this capability.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert!(cap.includes_area(&Area::new_subspace_area(subspace_id)));
    /// assert!(!cap.includes_area(&Area::full()));
    /// # }
    /// ```
    pub fn includes_area(&self, area: &Area) -> bool {
        self.0.includes_area(area.into())
    }

    /// Appends a new [`Delegation`] to this capability, returning an error if the resulting granted area would not be included in the previous granted area. **Does not verify any signatures.**
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(namespace_id, subspace_id.clone());
    ///
    /// let contained_delegation = Delegation::new(
    ///     Area::new_subspace_area(subspace_id.clone()),
    ///     SubspaceId::from_bytes(&[18; 32]),
    ///     SubspaceSignature::from([19; 64]), // Invalid signature, but this is not checked.
    /// );
    ///
    /// assert!(cap.try_append_delegation(contained_delegation).is_ok());
    ///
    /// let invalid_delegation = Delegation::new(
    ///     Area::full(), // Greater than the granted area of `cap`.
    ///     SubspaceId::from_bytes(&[18; 32]),
    ///     SubspaceSignature::from([19; 64]), // Invalid signature, but this is not checked.
    /// );
    ///
    /// assert!(cap.try_append_delegation(invalid_delegation).is_err());
    /// # }
    /// ```
    pub fn try_append_delegation(
        &mut self,
        delegation: Delegation,
    ) -> Result<(), InvalidCapability> {
        self.0.try_append_delegation(delegation.into())
    }

    /// Appends a new [`Delegation`] to this capability, panicking if the resulting granted area would not be included in the previous granted area. **Does not verify any signatures.**
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(namespace_id, subspace_id.clone());
    ///
    /// let contained_delegation = Delegation::new(
    ///     Area::new_subspace_area(subspace_id.clone()),
    ///     SubspaceId::from_bytes(&[18; 32]),
    ///     SubspaceSignature::from([19; 64]), // Invalid signature, but this is not checked.
    /// );
    ///
    /// // Does not panic.
    /// cap.append_delegation(contained_delegation);
    /// # }
    /// ```
    ///
    /// ```should_panic
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(namespace_id, subspace_id.clone());
    ///
    /// let invalid_delegation = Delegation::new(
    ///     Area::full(), // Greater than the granted area of `cap`.
    ///     SubspaceId::from_bytes(&[18; 32]),
    ///     SubspaceSignature::from([19; 64]), // Invalid signature, but this is not checked.
    /// );
    ///
    /// // Panics.
    /// cap.append_delegation(invalid_delegation)
    /// # }
    /// # #[cfg(not(feature = "dev"))] {panic!()}
    /// ```
    pub fn append_delegation(&mut self, delegation: Delegation) {
        self.0.append_delegation(delegation.into())
    }

    /// Appends a new [`Delegation`] to this capability, returning an error if the resulting granted area would not be included in the previous granted area or if the signature is invalid.
    pub fn try_append_delegation_checking_validity(
        &mut self,
        delegation: Delegation,
    ) -> Result<(), InvalidCapability> {
        self.0
            .try_append_delegation_checking_validity(delegation.into())
    }

    /// Appends a new [`Delegation`] to this capability, panicking if the resulting granted area would not be included in the previous granted area or if the signature is invalid.
    pub fn append_delegation_checking_validity(&mut self, delegation: Delegation) {
        self.0
            .append_delegation_checking_validity(delegation.into());
    }

    /// Returns by value the [area to which this grants access](https://willowprotocol.org/specs/meadowcap/index.html#cap_granted_area) (and simply assumes the capability is valid).
    ///
    /// Prefer using [`includes`](PossiblyValidReadCapability::includes), [`includes_area`](PossiblyValidReadCapability::includes_area) or [`granted_area_ref`](PossiblyValidReadCapability::granted_area_ref) whenever applicable, as these are more efficient.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_owned(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    ///     NamespaceSignature::from([18; 64]),
    /// );
    ///
    /// assert_eq!(cap.granted_area(), Area::full());
    ///
    /// let delegation = Delegation::new(
    ///     Area::new_subspace_area(subspace_id.clone()),
    ///     SubspaceId::from_bytes(&[18; 32]),
    ///     SubspaceSignature::from([19; 64]),
    /// );
    ///
    /// cap.append_delegation(delegation.clone());
    ///
    /// assert_eq!(cap.granted_area(), Area::new_subspace_area(subspace_id));
    /// # }
    /// ```
    pub fn granted_area(&self) -> Area {
        self.0.granted_area().into()
    }

    /// Returns whether the given [namespaced](Namespaced) [coordinate](Coordinatelike) is covered by this capability.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use willow25::authorisation::raw::*;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = PossiblyValidReadCapability::new_communal(namespace_id.clone(), subspace_id.clone());
    ///
    /// let included_entry = Entry::builder()
    ///     .namespace_id(namespace_id.clone())
    ///     .subspace_id(subspace_id.clone())
    ///     .path(path!(""))
    ///     .timestamp(12345)
    ///     .payload(b"hi")
    ///     .build();
    ///
    /// let outer_entry = Entry::prefilled_builder(&included_entry)
    ///     .subspace_id(SubspaceId::from_bytes(&[18; 32]))
    ///     .build();
    ///
    /// assert!(cap.includes(&included_entry));
    /// assert!(!cap.includes(&outer_entry));
    /// # }
    /// ```
    pub fn includes<T>(&self, t: &T) -> bool
    where
        T: Namespaced + Coordinatelike + ?Sized,
    {
        self.0.includes(t)
    }

    /// Returns whether this capability is [valid](https://willowprotocol.org/specs/meadowcap/index.html#cap_valid).
    pub fn is_valid(&self) -> bool {
        self.0.is_valid()
    }

    /// Converts `self` into a [`ReadCapability`], or returning `Err(self)` if `self` is not [valid](https://willowprotocol.org/specs/meadowcap/index.html#cap_valid).
    ///
    /// See [`into_read_capability_unchecked`](PossiblyValidReadCapability::into_read_capability_unchecked) for the unsafe (but more efficient) version of this.
    #[allow(clippy::result_large_err)]
    pub fn into_read_capability(self) -> Result<ReadCapability, Self> {
        match self.0.into_read_capability() {
            Ok(yay) => Ok(yay.into()),
            Err(nay) => Err(nay.into()),
        }
    }
}

/// Implements encoding according to the [encode_mc_capability](https://willowprotocol.org/specs/encodings/index.html#encode_mc_capability) encoding function.
impl Encodable for PossiblyValidReadCapability {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        self.0.encode(consumer).await
    }
}

impl EncodableKnownLength for PossiblyValidReadCapability {
    fn len_of_encoding(&self) -> usize {
        self.0.len_of_encoding()
    }
}

/// Implements decoding according to the [EncodeMcCapability](https://willowprotocol.org/specs/encodings/index.html#EncodeMcCapability) encoding relation, and further errors if the decoded capability is a read capability. **Does not check the validity** of the decoded capability.
impl Decodable for PossiblyValidReadCapability {
    type ErrorReason = Blame;

    async fn decode<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let decoded: meadowcap::raw::PossiblyValidReadCapability<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
        > = producer.produce_decoded().await?;

        Ok(decoded.into())
    }
}

/// Implements decoding according to the [encode_mc_capability](https://willowprotocol.org/specs/encodings/index.html#encode_mc_capability) encoding function, and further errors if the decoded capability is a read capability. **Does not check the validity** of the decoded capability.
impl DecodableCanonic for PossiblyValidReadCapability {
    type ErrorCanonic = Blame;

    async fn decode_canonic<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorCanonic>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let decoded: meadowcap::raw::PossiblyValidReadCapability<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
        > = producer.produce_decoded_canonic().await?;

        Ok(decoded.into())
    }
}

impl From<ReadCapability> for PossiblyValidReadCapability {
    fn from(value: ReadCapability) -> Self {
        meadowcap::raw::PossiblyValidReadCapability::<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
        >::from(meadowcap::ReadCapability::<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
        >::from(value))
        .into()
    }
}
