use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use meadowcap::raw::InvalidCapability;

use signature::{Keypair, Signer};
use ufotofu::codec_prelude::*;

use crate::{
    authorisation::raw::{Delegation, Genesis},
    prelude::*,
};

wrapper! {
    /// A [valid](https://willowprotocol.org/specs/meadowcap/index.html#cap_valid) write capability.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id,
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert_eq!(cap.receiver(), &subspace_id);
    ///
    /// let new_receiver = SubspaceId::from_bytes(&[18; 32]);
    ///
    /// cap.delegate(&secret, Area::new_subspace_area(subspace_id), new_receiver.clone());
    ///
    /// assert_eq!(cap.receiver(), &new_receiver);
    /// # }
    /// ```
    #[derive(PartialEq, Eq, Clone)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    WriteCapability; meadowcap::WriteCapability<MCL, MCC, MPL, NamespaceId, NamespaceSignature, SubspaceId, SubspaceSignature>
}

impl fmt::Debug for WriteCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl WriteCapability {
    /// Returns the [receiver](https://willowprotocol.org/specs/meadowcap/index.html#cap_receiver) of this capability.
    ///
    /// ```
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let cap = WriteCapability::new_communal(namespace_id, subspace_id.clone());
    ///
    /// assert_eq!(cap.receiver(), &subspace_id);
    /// # }
    /// ```
    pub fn receiver(&self) -> &SubspaceId {
        self.0.receiver()
    }

    /// Returns the [namespace id to which this grants access](https://willowprotocol.org/specs/meadowcap/index.html#cap_granted_namespace).
    ///
    /// ```
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let cap = WriteCapability::new_communal(namespace_id.clone(), subspace_id);
    ///
    /// assert_eq!(cap.granted_namespace(), &namespace_id);
    /// # }
    /// ```
    pub fn granted_namespace(&self) -> &NamespaceId {
        self.0.granted_namespace()
    }

    /// Returns a reference to the [area to which this grants access](https://willowprotocol.org/specs/meadowcap/index.html#cap_granted_area), or `None` if there is no delegation step which restricts the initial area.
    ///
    /// This method is slightly inconvenient to work with (you need special logic if there are no delegation steps), but it is efficient because it never explicitly creates a new area value. For the more convenient version, see [`granted_area`](WriteCapability::granted_area).
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id,
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert_eq!(cap.granted_area_ref(), None);
    ///
    /// let new_receiver = SubspaceId::from_bytes(&[18; 32]);
    ///
    /// cap.delegate(&secret, Area::new_subspace_area(subspace_id.clone()), new_receiver.clone());
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
    /// use willow25::{prelude::*, authorisation::*, authorisation::raw::AccessMode};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(namespace_id.clone(), subspace_id.clone());
    ///
    /// let genesis = cap.genesis();
    ///
    /// assert_eq!(genesis.access_mode(), AccessMode::Write);
    /// assert_eq!(genesis.namespace_key(), &namespace_id);
    /// assert_eq!(genesis.user_key(), &subspace_id);
    /// # }
    /// ```
    pub fn genesis(&self) -> &Genesis {
        self.0.genesis().into()
    }

    /// Returns `true` if and only if this is an owned capability.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (namespace_id, secret) = randomly_generate_namespace(&mut csprng);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut owncap = WriteCapability::new_owned(
    ///     &secret,
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert!(owncap.is_owned());
    ///
    /// let comcap = WriteCapability::new_communal(namespace_id.clone(), subspace_id);
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
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id,
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert!(cap.delegations().is_empty());
    ///
    /// let new_receiver = SubspaceId::from_bytes(&[18; 32]);
    /// cap.delegate(&secret, Area::new_subspace_area(subspace_id.clone()), new_receiver.clone());
    ///
    /// assert_eq!(cap.delegations().len(), 1);
    /// # }
    /// ```
    pub fn delegations(&self) -> &[Delegation] {
        let inner_delegations = self.0.delegations();
        let as_delegation25_ptr = inner_delegations.as_ptr() as *const Delegation;

        // SAFETY: all necessary invariants are already upheld by the original slice, and the layout is identical because `willow25::authorisation::raw::Delegation` is `repr(transparent)`
        unsafe { core::slice::from_raw_parts(as_delegation25_ptr, inner_delegations.len()) }
    }

    /// Creates a new [communal](https://willowprotocol.org/specs/meadowcap/index.html#communal_capabilities) write capability with no delegations.
    ///
    /// ```
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let cap = WriteCapability::new_communal(namespace_id.clone(), subspace_id.clone());
    ///
    /// assert!(!cap.is_owned());
    /// assert_eq!(cap.receiver(), &subspace_id);
    /// assert_eq!(cap.granted_namespace(), &namespace_id);
    /// assert_eq!(cap.granted_area(), Area::new_subspace_area(subspace_id));
    /// assert!(cap.delegations().is_empty());
    /// # }
    /// ```
    pub fn new_communal(namespace_key: NamespaceId, user_key: SubspaceId) -> Self {
        Self(meadowcap::WriteCapability::new_communal(
            namespace_key,
            user_key,
        ))
    }

    /// Creates a new [owned](https://willowprotocol.org/specs/meadowcap/index.html#owned_capabilities) write capability with no delegations.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (namespace_id, secret) = randomly_generate_namespace(&mut csprng);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let mut cap = WriteCapability::new_owned(
    ///     &secret,
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert!(cap.is_owned());
    /// assert_eq!(cap.receiver(), &subspace_id);
    /// assert_eq!(cap.granted_namespace(), &namespace_id);
    /// assert_eq!(cap.granted_area(), Area::full());
    /// assert!(cap.delegations().is_empty());
    /// # }
    /// ```
    pub fn new_owned<NamespaceKeypair>(keypair: &NamespaceKeypair, user_key: SubspaceId) -> Self
    where
        NamespaceKeypair: Signer<NamespaceSignature> + Keypair<VerifyingKey = NamespaceId>,
    {
        Self(meadowcap::WriteCapability::new_owned(keypair, user_key))
    }

    /// Returns whether the given area is contained in the [granted area](WriteCapability::granted_area_ref) of this capability.
    ///
    /// ```
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let namespace_id = NamespaceId::from_bytes(&[16; 32]);
    /// let subspace_id = SubspaceId::from_bytes(&[17; 32]);
    ///
    /// let cap = WriteCapability::new_communal(namespace_id.clone(), subspace_id.clone());
    ///
    /// assert!(cap.includes_area(&Area::new_subspace_area(subspace_id)));
    /// assert!(!cap.includes_area(&Area::full()));
    /// # }
    /// ```
    pub fn includes_area(&self, area: &Area) -> bool {
        self.0.includes_area(area.into())
    }

    /// Returns by value the [area to which this grants access](https://willowprotocol.org/specs/meadowcap/index.html#cap_granted_area).
    ///
    /// Prefer using [`includes`](WriteCapability::includes), [`includes_area`](WriteCapability::includes_area) or [`granted_area_ref`](WriteCapability::granted_area_ref) whenever applicable, as these are more efficient.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (namespace_id, namespace_secret) = randomly_generate_namespace(&mut csprng);
    /// let (subspace_id, subspace_secret) = randomly_generate_subspace(&mut csprng);
    ///
    /// let mut cap = WriteCapability::new_owned(
    ///     &namespace_secret,
    ///     subspace_id.clone(),
    /// );
    ///
    /// assert_eq!(cap.granted_area(), Area::full());
    ///
    /// let new_receiver = SubspaceId::from_bytes(&[18; 32]);
    /// cap.delegate(&subspace_secret, Area::new_subspace_area(subspace_id.clone()), new_receiver.clone());
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
    /// let mut cap = WriteCapability::new_communal(namespace_id.clone(), subspace_id.clone());
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

    /// Delegates this capability, returning an error if the resulting granted area would not be included in the previous granted area, or if the public key of the keypair was not the receiver of the capability pre-delegation.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let (_subspace_id2, secret2) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    /// let new_receiver = SubspaceId::from_bytes(&[18; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id,
    ///     subspace_id.clone(),
    /// );
    ///
    /// // Trying to delegate with an invalid secret fails.
    /// assert!(
    ///     cap.try_delegate(
    ///         &secret2,
    ///         Area::new_subspace_area(subspace_id.clone()),
    ///         new_receiver.clone(),
    ///     ).is_err()
    /// );
    ///
    /// // Trying to delegate to a greater area fails.
    /// assert!(
    ///     cap.try_delegate(
    ///         &secret,
    ///         Area::full(),
    ///         new_receiver.clone(),
    ///     ).is_err()
    /// );
    ///
    /// // Delegating with the correct secret and to a contained area succeeds.
    /// assert!(
    ///     cap.try_delegate(
    ///         &secret,
    ///         Area::new_subspace_area(subspace_id.clone()),
    ///         new_receiver.clone(),
    ///     ).is_ok()
    /// );
    /// # }
    /// ```
    pub fn try_delegate<UserKeypair>(
        &mut self,
        keypair: &UserKeypair,
        new_area: Area,
        new_receiver: SubspaceId,
    ) -> Result<(), InvalidCapability>
    where
        UserKeypair: Signer<SubspaceSignature> + Keypair<VerifyingKey = SubspaceId>,
    {
        self.0.try_delegate(keypair, new_area.into(), new_receiver)
    }

    /// Delegates this capability, panicking if the resulting granted area would not be included in the previous granted area, or if the public key of the keypair was not the receiver of the capability pre-delegation.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    /// let new_receiver = SubspaceId::from_bytes(&[18; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id,
    ///     subspace_id.clone(),
    /// );
    ///
    /// // This works =)
    /// cap.delegate(
    ///     &secret,
    ///     Area::new_subspace_area(subspace_id.clone()),
    ///     new_receiver.clone(),
    /// );
    /// # }
    /// ```
    ///
    /// ```should_panic
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(not(feature = "dev"))] {panic!()}
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let (_subspace_id2, secret2) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    /// let new_receiver = SubspaceId::from_bytes(&[18; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id,
    ///     subspace_id.clone(),
    /// );
    ///
    /// // Delegating with an invalid secret panics.
    /// cap.delegate(
    ///     &secret2,
    ///     Area::new_subspace_area(subspace_id.clone()),
    ///     new_receiver.clone(),
    /// );
    /// # }
    /// ```
    ///
    /// ```should_panic
    /// use rand::rngs::OsRng;
    /// use willow25::{prelude::*, authorisation::*};
    ///
    /// # #[cfg(not(feature = "dev"))] {panic!()}
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    /// let new_receiver = SubspaceId::from_bytes(&[18; 32]);
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id,
    ///     subspace_id.clone(),
    /// );
    ///
    /// // Delegating to a greater area panics.
    /// cap.delegate(
    ///     &secret,
    ///     Area::full(),
    ///     new_receiver.clone(),
    /// );
    /// # }
    /// ```
    pub fn delegate<UserKeypair>(
        &mut self,
        keypair: &UserKeypair,
        new_area: Area,
        new_receiver: SubspaceId,
    ) where
        UserKeypair: Signer<SubspaceSignature> + Keypair<VerifyingKey = SubspaceId>,
    {
        self.0.delegate(keypair, new_area.into(), new_receiver)
    }
}

/// Implements encoding according to the [encode_mc_capability](https://willowprotocol.org/specs/encodings/index.html#encode_mc_capability) encoding function.
impl Encodable for WriteCapability {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        self.0.encode(consumer).await
    }
}

impl EncodableKnownLength for WriteCapability {
    fn len_of_encoding(&self) -> usize {
        self.0.len_of_encoding()
    }
}

/// Implements decoding according to the [EncodeMcCapability](https://willowprotocol.org/specs/encodings/index.html#EncodeMcCapability) encoding relation, and further errors if the decoded capability is a write capability.
impl Decodable for WriteCapability {
    type ErrorReason = Blame;

    async fn decode<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        meadowcap::WriteCapability::<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
        >::decode(producer)
        .await
        .map(Into::into)
    }
}

/// Implements decoding according to the [encode_mc_capability](https://willowprotocol.org/specs/encodings/index.html#encode_mc_capability) encoding function, and further errors if the decoded capability is a write capability.
impl DecodableCanonic for WriteCapability {
    type ErrorCanonic = Blame;

    async fn decode_canonic<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorCanonic>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        meadowcap::WriteCapability::<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
        >::decode_canonic(producer)
        .await
        .map(Into::into)
    }
}

wrapper! {
    /// A pair of a prior [`WriteCapability`] and [`Entry`], used to encoded and decode [`WriteCapability`] values privately.
    #[derive(Clone, PartialEq, Debug)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    PriorCapEntryPair; meadowcap::PriorCapEntryPair<MCL, MCC, MPL, NamespaceId, NamespaceSignature, SubspaceId, SubspaceSignature, PayloadDigest>
}

impl PriorCapEntryPair {
    /// Returns a new [`PriorCapEntryPair`] with the given `prior_cap` and `entry`.
    pub fn new(prior_cap: WriteCapability, entry: Entry) -> Self {
        let inner_prior_cap: meadowcap::WriteCapability<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
        > = prior_cap.into();
        let inner_entry: willow_data_model::entry::Entry<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            SubspaceId,
            PayloadDigest,
        > = entry.into();

        Self(meadowcap::PriorCapEntryPair::new(
            inner_prior_cap,
            inner_entry,
        ))
    }

    /// Returns a [`WriteCapability`] which was previously encoded against..
    pub fn prior_cap(&self) -> &WriteCapability {
        <&WriteCapability>::from(self.0.prior_cap())
    }

    /// Returns the entry included by the [`WriteCapability`] being privately encoded against.
    pub fn entry(&self) -> &Entry {
        <&Entry>::from(self.0.entry())
    }
}

/// Implements relative encoding according to the [EncodeCommunalCapabilityRelative](https://willowprotocol.org/specs/encodings/index.html#encsec_EncodeCommunalCapabilityRelative)  and [EncodeOwnedCapabalityRelative](https://willowprotocol.org/specs/encodings/index.html#encsec_EncodeOwnedCapabilityRelative) encoding functions.
impl RelativeEncodable<PriorCapEntryPair> for WriteCapability {
    async fn relative_encode<C>(
        &self,
        rel: &PriorCapEntryPair,
        consumer: &mut C,
    ) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        let inner = <&meadowcap::PriorCapEntryPair<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
            PayloadDigest,
        >>::from(rel);

        self.0.relative_encode(inner, consumer).await
    }

    /// Returns false if the relative entry's namespace is not `self`'s [granted area](https://willowprotocol.org/specs/meadowcap/index.html#communal_cap_granted_namespace), or if the relative entry is not included by `self`'s [granted area](https://willowprotocol.org/specs/meadowcap/index.html#communal_cap_granted_area).
    fn can_be_encoded_relative_to(&self, rel: &PriorCapEntryPair) -> bool {
        let inner = <&meadowcap::PriorCapEntryPair<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
            PayloadDigest,
        >>::from(rel);

        self.0.can_be_encoded_relative_to(inner)
    }
}

/// Implements relative decoding according to the [EncodeCommunalCapabilityRelative](https://willowprotocol.org/specs/encodings/index.html#encsec_EncodeCommunalCapabilityRelative)  and [EncodeOwnedCapabalityRelative](https://willowprotocol.org/specs/encodings/index.html#encsec_EncodeOwnedCapabilityRelative) encoding functions.
impl RelativeDecodable<PriorCapEntryPair> for WriteCapability {
    type ErrorReason = Blame;

    async fn relative_decode<P>(
        rel: &PriorCapEntryPair,
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        let inner = meadowcap::WriteCapability::<
            MCL,
            MCC,
            MPL,
            NamespaceId,
            NamespaceSignature,
            SubspaceId,
            SubspaceSignature,
        >::relative_decode(
            <&meadowcap::PriorCapEntryPair<
                MCL,
                MCC,
                MPL,
                NamespaceId,
                NamespaceSignature,
                SubspaceId,
                SubspaceSignature,
                PayloadDigest,
            >>::from(rel),
            producer,
        )
        .await?;

        Ok(Self(inner))
    }
}
