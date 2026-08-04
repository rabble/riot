use core::cmp::Ordering;

use ufotofu::codec_prelude::*;

use willow_data_model::prelude as wdm;

use crate::prelude::*;

/// A trait describing the metadata associated with each Willow [Payload](https://willowprotocol.org/specs/data-model/index.html#Payload) string.
///
/// [Entries](https://willowprotocol.org/specs/data-model/index.html#Entry) are the central concept in Willow. In order to make any bytestring of data accessible to Willow, you need to create an Entry describing its metadata. Specifically, an Entry consists of
///
/// - a [namespace_id](https://willowprotocol.org/specs/data-model/index.html#entry_namespace_id) (roughly, this addresses a universe of Willow data, fully independent from all data (i.e., Entries) of different namespace ids) of type `N`,
/// - a [subspace_id](https://willowprotocol.org/specs/data-model/index.html#entry_subspace_id) (roughly, a fully indendent part of a namespace, typically subspaces correspond to individual users) of type `S`,
/// - a [path](https://willowprotocol.org/specs/data-model/index.html#entry_path) (roughly, a file-system-like way of arranging payloads hierarchically within a subspace) of type [`Path`],
/// - a [timestamp](https://willowprotocol.org/specs/data-model/index.html#entry_timestamp) (newer Entries can overwrite certain older Entries) of type [`Timestamp`],
/// - a [payload_length](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) (the length of the payload string), and
/// - a [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest) (a secure hash of the payload string being inserted into Willow).
///
/// We use this trait in order to be able to abstract over specific implementations of entries. If you want a concrete type for representing entries, use the [`Entry`] struct.
pub trait Entrylike:
    Coordinatelike + Namespaced + wdm::Entrylike<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>
{
    /// Returns the [payload_length](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) of `self`.
    fn payload_length(&self) -> u64;

    /// Returns the [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest) of `self`.
    fn payload_digest(&self) -> &PayloadDigest;
}

impl<T> Entrylike for T
where
    T: Coordinatelike
        + Namespaced
        + wdm::Entrylike<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>
        + ?Sized,
{
    fn payload_length(&self) -> u64 {
        self.wdm_payload_length()
    }

    fn payload_digest(&self) -> &PayloadDigest {
        self.wdm_payload_digest()
    }
}

/// Methods for working with [`Entrylikes`](Entrylike).
///
/// This trait is automatically implemented by all types implementing [`Entrylike`].
pub trait EntrylikeExt:
    wdm::EntrylikeExt<MCL, MCC, MPL, NamespaceId, SubspaceId, PayloadDigest>
{
    /// Returns whether `self` and `other` describe equal entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!(""))
    ///     .timestamp(12345)
    ///     .payload_digest([1; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert!(entry.entry_eq(&entry));
    ///
    /// let changed = Entry::prefilled_builder(&entry).timestamp(999999).build();
    /// assert!(!entry.entry_eq(&changed));
    ///
    /// ```
    fn entry_eq<OtherEntry>(&self, other: &OtherEntry) -> bool
    where
        OtherEntry: Entrylike,
    {
        self.wdm_entry_eq(other)
    }

    /// Returns whether `self` and `other` describe non-equal entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!(""))
    ///     .timestamp(12345)
    ///     .payload_digest([1; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert!(!entry.entry_ne(&entry));
    ///
    /// let changed = Entry::prefilled_builder(&entry).timestamp(999999).build();
    /// assert!(entry.entry_ne(&changed));
    ///
    /// ```
    fn entry_ne<OtherEntry>(&self, other: &OtherEntry) -> bool
    where
        OtherEntry: Entrylike,
    {
        self.wdm_entry_ne(other)
    }

    /// Compares `self` to another entry by [timestamp](https://willowprotocol.org/specs/data-model/index.html#entry_timestamp), [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest) (in case of a tie), and [payload_length](https://willowprotocol.org/specs/data-model/index.html#entry_payload_length) third (in case of yet another tie). See also [`EntrylikeExt::is_newer_than`] and [`EntrylikeExt::is_older_than`].
    ///
    /// Comparing recency is primarily important to determine [which entries overwrite each other](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning); the [`EntrylikeExt::prunes`] method checks for that directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::cmp::Ordering;
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!(""))
    ///     .timestamp(12345)
    ///     .payload_digest([1; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert_eq!(entry.cmp_recency(&entry), Ordering::Equal);
    ///
    /// let lesser_timestamp = Entry::prefilled_builder(&entry).timestamp(5).build();
    /// assert_eq!(entry.cmp_recency(&lesser_timestamp), Ordering::Greater);
    ///
    /// let lesser_digest = Entry::prefilled_builder(&entry).payload_digest([0; 32].into()).build();
    /// assert_eq!(entry.cmp_recency(&lesser_digest), Ordering::Greater);
    ///
    /// let lesser_length = Entry::prefilled_builder(&entry).payload_length(0).build();
    /// assert_eq!(entry.cmp_recency(&lesser_length), Ordering::Greater);
    ///
    /// let greater_timestamp = Entry::prefilled_builder(&entry).timestamp(999999).build();
    /// assert_eq!(entry.cmp_recency(&greater_timestamp), Ordering::Less);
    ///
    /// let greater_digest = Entry::prefilled_builder(&entry).payload_digest([2; 32].into()).build();
    /// assert_eq!(entry.cmp_recency(&greater_digest), Ordering::Less);
    ///
    /// let greater_length = Entry::prefilled_builder(&entry).payload_length(99).build();
    /// assert_eq!(entry.cmp_recency(&greater_length), Ordering::Less);
    /// ```
    ///
    /// [Spec definition](https://willowprotocol.org/specs/data-model/index.html#entry_newer).
    fn cmp_recency<OtherEntry>(&self, other: &OtherEntry) -> Ordering
    where
        OtherEntry: Entrylike,
    {
        self.wdm_cmp_recency(other)
    }

    /// Returns whether this entry is strictly [newer](https://willowprotocol.org/specs/data-model/index.html#entry_newer) than another entry. See also [`EntrylikeExt::cmp_recency`] and [`EntrylikeExt::is_older_than`].
    ///
    /// Comparing recency is primarily important to determine [which entries overwrite each other](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning); the [`EntrylikeExt::prunes`] method checks for that directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!(""))
    ///     .timestamp(12345)
    ///     .payload_digest([1; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert!(!entry.is_newer_than(&entry));
    ///
    /// let lesser_timestamp = Entry::prefilled_builder(&entry).timestamp(5).build();
    /// assert!(entry.is_newer_than(&lesser_timestamp));
    ///
    /// let lesser_digest = Entry::prefilled_builder(&entry).payload_digest([0; 32].into()).build();
    /// assert!(entry.is_newer_than(&lesser_digest));
    ///
    /// let lesser_length = Entry::prefilled_builder(&entry).payload_length(0).build();
    /// assert!(entry.is_newer_than(&lesser_length));
    ///
    /// let greater_timestamp = Entry::prefilled_builder(&entry).timestamp(999999).build();
    /// assert!(!entry.is_newer_than(&greater_timestamp));
    ///
    /// let greater_digest = Entry::prefilled_builder(&entry).payload_digest([2; 32].into()).build();
    /// assert!(!entry.is_newer_than(&greater_digest));
    ///
    /// let greater_length = Entry::prefilled_builder(&entry).payload_length(99).build();
    /// assert!(!entry.is_newer_than(&greater_length));
    /// ```
    fn is_newer_than<OtherEntry>(&self, other: &OtherEntry) -> bool
    where
        OtherEntry: Entrylike,
    {
        self.wdm_is_newer_than(other)
    }

    /// Returns whether this entry is strictly [older](https://willowprotocol.org/specs/data-model/index.html#entry_newer) than another entry. See also [`EntrylikeExt::cmp_recency`] and [`EntrylikeExt::is_newer_than`].
    ///
    /// Comparing recency is primarily important to determine [which entries overwrite each other](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning); the [`EntrylikeExt::prunes`] method checks for that directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!(""))
    ///     .timestamp(12345)
    ///     .payload_digest([1; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// assert!(!entry.is_older_than(&entry));
    ///
    /// let lesser_timestamp = Entry::prefilled_builder(&entry).timestamp(5).build();
    /// assert!(!entry.is_older_than(&lesser_timestamp));
    ///
    /// let lesser_digest = Entry::prefilled_builder(&entry).payload_digest([0; 32].into()).build();
    /// assert!(!entry.is_older_than(&lesser_digest));
    ///
    /// let lesser_length = Entry::prefilled_builder(&entry).payload_length(0).build();
    /// assert!(!entry.is_older_than(&lesser_length));
    ///
    /// let greater_timestamp = Entry::prefilled_builder(&entry).timestamp(999999).build();
    /// assert!(entry.is_older_than(&greater_timestamp));
    ///
    /// let greater_digest = Entry::prefilled_builder(&entry).payload_digest([2; 32].into()).build();
    /// assert!(entry.is_older_than(&greater_digest));
    ///
    /// let greater_length = Entry::prefilled_builder(&entry).payload_length(99).build();
    /// assert!(entry.is_older_than(&greater_length));
    /// ```
    fn is_older_than<OtherEntry>(&self, other: &OtherEntry) -> bool
    where
        OtherEntry: Entrylike,
    {
        self.wdm_is_older_than(other)
    }

    /// Returns whether this entry would [prefix prune](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning) another entry.
    ///
    /// Prefix pruning powers deletion in Willow: whenever a data store would contain two entries, one of which pruens the other, the other is removed from the data store (or never inserted in the first place). Informally speaking, newer entries remove older entries, but only if they are in the same namespace and subspace, and only if the path of the newer entry is a prefix of the path of the older entry.
    ///
    /// More precisely: an entry `e1` prunes an entry `e2` if and only if
    ///
    /// - `e1.namespace_id() == e2.namespace_id()`,
    /// - `e1.subspace_id() == e2.subspace_id()`,
    /// - `e1.path().is_prefix_of(e2.path())`, and
    /// - `e1.is_newer_than(&e2)`.
    ///
    /// This method is the reciprocal of [`EntrylikeExt::is_pruned_by`].
    ///
    /// # Examples
    ///
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!("/a/b"))
    ///     .timestamp(12345)
    ///     .payload_digest([1; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// let newer = Entry::prefilled_builder(&entry).timestamp(99999).build();
    /// assert!(!entry.prunes(&newer));
    /// assert!(newer.prunes(&entry));
    ///
    /// let newer_and_prefix = Entry::prefilled_builder(&newer)
    ///     .path(path!("/a")).build();
    /// assert!(!entry.prunes(&newer_and_prefix));
    /// assert!(newer_and_prefix.prunes(&entry));
    ///
    /// let newer_and_extension = Entry::prefilled_builder(&newer)
    ///     .path(path!("/a/b/c")).build();
    /// assert!(!entry.prunes(&newer_and_extension));
    /// assert!(!newer_and_extension.prunes(&entry));
    ///
    /// let newer_but_unrelated_namespace = Entry::prefilled_builder(&newer)
    ///     .namespace_id([17; 32].into()).build();
    /// assert!(!entry.prunes(&newer_but_unrelated_namespace));
    /// assert!(!newer_but_unrelated_namespace.prunes(&entry));
    ///
    /// let newer_but_unrelated_subspace = Entry::prefilled_builder(&newer)
    ///     .subspace_id([3; 32].into()).build();
    /// assert!(!entry.prunes(&newer_but_unrelated_subspace));
    /// assert!(!newer_but_unrelated_subspace.prunes(&entry));
    /// # Ok::<(), PathError>(())
    /// ```
    ///
    fn prunes<OtherEntry>(&self, other: &OtherEntry) -> bool
    where
        OtherEntry: Entrylike,
    {
        self.wdm_prunes(other)
    }

    /// Returns whether this entry would be [prefix pruned](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning) by another entry.
    ///
    /// Prefix pruning powers deletion in Willow: whenever a data store would contain two entries, one of which pruens the other, the other is removed from the data store (or never inserted in the first place). Informally speaking, newer entries remove older entries, but only if they are in the same namespace and subspace, and only if the path of the newer entry is a prefix of the path of the older entry.
    ///
    /// More precisely: an entry `e1` prunes an entry `e2` if and only if
    ///
    /// - `e1.namespace_id() == e2.namespace_id()`,
    /// - `e1.subspace_id() == e2.subspace_id()`,
    /// - `e1.path().is_prefix_of(e2.path())`, and
    /// - `e1.is_newer_than(&e2)`.
    ///
    /// This method is the reciprocal of [`EntrylikeExt::prunes`].
    ///
    /// # Examples
    ///
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let entry = Entry::builder()
    ///     .namespace_id([0; 32].into())
    ///     .subspace_id([1; 32].into())
    ///     .path(path!("/a/b"))
    ///     .timestamp(12345)
    ///     .payload_digest([1; 32].into())
    ///     .payload_length(17)
    ///     .build();
    ///
    /// let newer = Entry::prefilled_builder(&entry).timestamp(99999).build();
    /// assert!(entry.is_pruned_by(&newer));
    /// assert!(!newer.is_pruned_by(&entry));
    ///
    /// let newer_and_prefix = Entry::prefilled_builder(&newer)
    ///     .path(path!("/a")).build();
    /// assert!(entry.is_pruned_by(&newer_and_prefix));
    /// assert!(!newer_and_prefix.is_pruned_by(&entry));
    ///
    /// let newer_and_extension = Entry::prefilled_builder(&newer)
    ///     .path(path!("/a/b/c")).build();
    /// assert!(!entry.is_pruned_by(&newer_and_extension));
    /// assert!(!newer_and_extension.is_pruned_by(&entry));
    ///
    /// let newer_but_unrelated_namespace = Entry::prefilled_builder(&newer)
    ///     .namespace_id([17; 32].into()).build();
    /// assert!(!entry.is_pruned_by(&newer_but_unrelated_namespace));
    /// assert!(!newer_but_unrelated_namespace.is_pruned_by(&entry));
    ///
    /// let newer_but_unrelated_subspace = Entry::prefilled_builder(&newer)
    ///     .subspace_id([3; 32].into()).build();
    /// assert!(!entry.is_pruned_by(&newer_but_unrelated_subspace));
    /// assert!(!newer_but_unrelated_subspace.is_pruned_by(&entry));
    /// # Ok::<(), PathError>(())
    /// ```
    ///
    fn is_pruned_by<OtherEntry>(&self, other: &OtherEntry) -> bool
    where
        OtherEntry: Entrylike,
        Self: Sized,
    {
        self.wdm_is_pruned_by(other)
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
    /// let authed = entry.authorise(&cap, &secret).unwrap();
    ///
    /// assert_eq!(authed.entry(), &entry);
    /// # }
    /// ```
    fn authorise(
        &self,
        write_capability: &WriteCapability,
        secret: &SubspaceSecret,
    ) -> Result<AuthorisedEntry, DoesNotAuthorise> {
        let authorisation_token =
            AuthorisationToken::new_for_entry(self, write_capability, secret)?;

        Ok(crate::authorisation::PossiblyAuthorisedEntry::new(
                Entry::from_entrylike(self),
                authorisation_token,
        )
            .into_authorised_entry().expect("`AuthorisationToken::new_for_entry` must produce an authorisation token that authorises the entry"))
    }

    /// Encodes `self` according to the [encode_entry](https://willowprotocol.org/specs/encodings/index.html#encode_entry) encoding function.
    async fn encode_entry<C>(&self, consumer: &mut C) -> Result<(), C::Error>
    where
        C: BulkConsumer<Item = u8> + ?Sized,
    {
        self.wdm_encode_entry(consumer).await
    }

    /// Computes the length of the encoding of `self` according to the [encode_entry](https://willowprotocol.org/specs/encodings/index.html#encode_entry) encoding function.
    fn length_of_entry_encoding(&self) -> usize {
        self.wdm_length_of_entry_encoding()
    }
}

impl<E> EntrylikeExt for E where E: Entrylike + ?Sized {}
