use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ufotofu::codec_prelude::*;

use willow_data_model::prelude as wdm;

use crate::prelude::*;

wrapper! {
    /// The metadata associated with each Willow [Payload](https://willowprotocol.org/specs/data-model/index.html#Payload) string.
    ///
    /// [Entries](https://willowprotocol.org/specs/data-model/index.html#Entry) are the central concept in Willow. In order to make any bytestring of data accessible to Willow, you need to create an Entry describing its metadata. Specifically, an Entry consists of
    ///
    /// - a [namespace_id](https://willowprotocol.org/specs/data-model/index.html#entry_namespace_id) (roughly, this addresses a universe of Willow data, fully independent from all data (i.e., Entries) of different namespace ids) of type [`NamespaceId`],
    /// - a [subspace_id](https://willowprotocol.org/specs/data-model/index.html#entry_subspace_id) (roughly, a fully indendent part of a namespace, typically subspaces correspond to individual users) of type [`SubspaceId`],
    /// - a [path](https://willowprotocol.org/specs/data-model/index.html#entry_path) (roughly, a file-system-like way of arranging payloads hierarchically within a subspace) of type [`Path`],
    /// - a [timestamp](https://willowprotocol.org/specs/data-model/index.html#entry_timestamp) (newer Entries can overwrite certain older Entries) of type [`Timestamp`],
    /// - a [payload_length](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) (the length of the payload string), and
    /// - a [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest) (a secure hash of the payload string being inserted into Willow) of type [`PayloadDigest`].
    ///
    /// To access these six fields, use the methods of the [`Entrylike`] trait (which [`Entry`] implements). The [`EntrylikeExt`] trait provides additional helper methods, for example, methods to check which Entries can delete which other Entries.
    ///
    /// To create Entries, use the [`Entry::builder`] or [`Entry::prefilled_builder`] functions.
    ///
    /// # Example
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!("/vacation/plan"))
    ///     .timestamp(12345)
    ///     .payload_digest([77; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert_eq!(*entry.subspace_id(), [1; 32].into());
    ///
    /// let newer = Entry::prefilled_builder(&entry).timestamp(99999).build();
    /// assert!(newer.prunes(&entry));
    /// ```
    ///
    /// [Spec definition](https://willowprotocol.org/specs/data-model/index.html#Entry).
    #[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
    #[cfg_attr(feature = "dev", derive(Arbitrary))]
    Entry; wdm::Entry<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Entry {
    /// Creates a builder for [`Entry`].
    ///
    /// # Examples
    ///
    /// ```compile_fail
    /// use willow25::prelude::*;
    ///
    /// // Supplying incomplete data errors.
    /// Entry::builder()
    ///     .path(path!(""))
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .payload_digest([77; 32].into())
    ///     // fails to compile because timestamp and payload_length are missing!
    ///     .build();
    /// ```
    ///
    /// ```
    /// # use willow25::prelude::*;
    /// // Supplying all necessary data yields an entry.
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!(""))
    ///     .timestamp(12345)
    ///     .payload_digest([77; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert_eq!(*entry.subspace_id(), [1; 32].into());
    /// ```
    pub fn builder() -> EntryBuilder {
        wdm::Entry::builder().into()
    }

    /// Creates a builder which is prefilled with the data from some other entry.
    ///
    /// Use this function to create modified copies of entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// // Supplying all necessary data yields an entry.
    /// let first_entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!(""))
    ///     .timestamp(12345)
    ///     .payload_digest([77; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert_eq!(*first_entry.payload_digest(), [77; 32].into());
    ///
    /// let second_entry = Entry::prefilled_builder(&first_entry)
    ///     .timestamp(67890)
    ///     .payload_digest([78; 32].into())
    ///     .payload_length(4)
    ///     .build();
    ///
    /// assert_eq!(*second_entry.payload_digest(), [78; 32].into());
    /// ```
    pub fn prefilled_builder<E>(
        source: &E,
    ) -> EntryBuilder<willow_data_model::entry::BuilderComplete>
    where
        E: Entrylike + ?Sized,
    {
        wdm::Entry::prefilled_builder(source).into()
    }

    /// Creates an [`Entry`] with [equal data](EntrylikeExt::entry_eq) to that of the given [`Entrylike`].
    ///
    /// ```
    /// # #[cfg(feature = "dev")] {
    /// use willow_data_model::prelude::*;
    /// use willow_data_model::test_parameters::*;
    ///
    /// let entry1 = Entry::builder()
    ///     .namespace_id(Family)
    ///     .subspace_id(Alfie)
    ///     .path(Path::<4, 4, 4>::new())
    ///     .timestamp(12345)
    ///     .payload_digest(Spades)
    ///     .payload_length(2)
    ///     .build();
    ///
    /// let entry2 = Entry::from_entrylike(&entry1);
    /// assert_eq!(entry1, entry2);
    /// # }
    /// ```
    pub fn from_entrylike<E>(entrylike_to_clone: &E) -> Self
    where
        E: Entrylike + ?Sized,
    {
        Self::prefilled_builder(entrylike_to_clone).build()
    }

    /// Turns `self` into an [`AuthorisedEntry`] by creating an authorisation token for it.
    ///
    /// ```
    /// use rand::rngs::OsRng;
    /// use willow25::prelude::*;
    /// use willow25::authorisation::PossiblyAuthorisedEntry;
    ///
    /// # #[cfg(feature = "dev")] {
    /// let mut csprng = OsRng;
    /// let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
    /// let namespace_id = NamespaceId::from_bytes(&[17; 32]);
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id(namespace_id.clone())
    ///     .subspace_id(subspace_id.clone())
    ///     .path(path!("/ideas"))
    ///     .timestamp(12345)
    ///     .payload(b"chocolate with mustard")
    ///     .build();
    ///
    /// let mut cap = WriteCapability::new_communal(
    ///     namespace_id.clone(),
    ///     subspace_id.clone(),
    /// );
    ///
    /// let authed = entry.into_authorised_entry(&cap, &secret);
    /// assert!(authed.is_ok());
    /// # }
    /// ```
    pub fn into_authorised_entry(
        self,
        write_capability: &WriteCapability,
        secret: &SubspaceSecret,
    ) -> Result<AuthorisedEntry, DoesNotAuthorise> {
        let authorisation_token =
            AuthorisationToken::new_for_entry(&self, write_capability, secret)?;

        Ok(crate::authorisation::PossiblyAuthorisedEntry::new(
                self,
                authorisation_token,
        )
            .into_authorised_entry().expect("`AuthorisationToken::new_for_entry` must produce an authorisation token that authorises the entry"))
    }
}

impl wdm::Keylike<MCL, MCC, MPL, SubspaceId> for Entry {
    fn wdm_subspace_id(&self) -> &SubspaceId {
        wdm::Keylike::wdm_subspace_id(&self.0)
    }

    fn wdm_path(&self) -> &wdm::Path<MCL, MCC, MPL> {
        wdm::Keylike::wdm_path(&self.0)
    }
}

impl wdm::Coordinatelike<MCL, MCC, MPL, SubspaceId> for Entry {
    fn wdm_timestamp(&self) -> Timestamp {
        wdm::Coordinatelike::wdm_timestamp(&self.0)
    }
}

impl wdm::Namespaced<NamespaceId> for Entry {
    fn wdm_namespace_id(&self) -> &NamespaceId {
        self.0.wdm_namespace_id()
    }
}

impl wdm::Entrylike<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest> for Entry {
    fn wdm_payload_length(&self) -> u64 {
        self.0.wdm_payload_length()
    }

    fn wdm_payload_digest(&self) -> &PayloadDigest {
        self.0.wdm_payload_digest()
    }
}

/// Implements [encode_entry](https://willowprotocol.org/specs/encodings/index.html#encode_entry).
impl Encodable for Entry {
    async fn encode<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        self.0.encode(consumer).await
    }
}

/// Implements [encode_entry](https://willowprotocol.org/specs/encodings/index.html#encode_entry).
impl EncodableKnownLength for Entry {
    fn len_of_encoding(&self) -> usize {
        self.0.len_of_encoding()
    }
}

/// Implements [EncodeEntry](https://willowprotocol.org/specs/encodings/index.html#EncodeEntry).
impl Decodable for Entry {
    type ErrorReason = Blame;

    async fn decode<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorReason>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        wdm::Entry::<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>::decode(producer)
            .await
            .map(Into::into)
    }
}

/// Implements [encode_entry](https://willowprotocol.org/specs/encodings/index.html#encode_entry).
impl DecodableCanonic for Entry {
    type ErrorCanonic = Blame;

    async fn decode_canonic<P>(
        producer: &mut P,
    ) -> Result<Self, DecodeError<P::Final, P::Error, Self::ErrorCanonic>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
        Self: Sized,
    {
        wdm::Entry::<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>::decode_canonic(
            producer,
        )
        .await
        .map(Into::into)
    }
}
