use crate::prelude::*;
use bab_rs::generic::storage::units::{ByteCount, ByteIndex};
use derive_more::{Display, Error};
use ufotofu::prelude::*;

/// The most basic trait describing storage for [`Entries`](Entry).
///
/// A `Store` is a collection of entries such that none of them [prefix-prune](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning) each other.
///
/// This trait provides only a very basic set of operations: creation of new entries, querying individual entries and all entries within an [`Area`], local deletion, and accessing (subslices of) payloads. More specialised operations are provided by subtraits.
///
/// Stores must implement [`Clone`], and the expectation is that cloning is cheap and that changes on one clone also affect all other clones. You perform concurrent operations on a store by cloning it and then performing the operations on different clones.
///
/// Important: persistent stores are not required to persist mutations immediately. Call the [`Store::flush`] method to ensure that an operation is persisted.
///
/// Implementations of this trait ands its subtraits must guarantee not to panic unless explicitly speicfied otherwise, even under adversarial inputs. It must be save to call those methods of a store based on input from an untrusted peer over the network.
pub trait Store {
    /// The type of errors the store implementation might yield on any operation.
    type InternalError;

    /// Creates an [`AuthorisedEntry`] with the given data and payload, and atomically inserts the payload and entry into the store.
    ///
    /// If the new entry would be prefix-pruned immediately by a newer entry already in the store, this method returns `Ok(None)`. If the new entry prefix-prunes older entries in the store, those older entries are automatically removed.
    ///
    /// If the `write_capability` and the `secret` do not yield a valid authorisation token for the entry that would be created, this method returns an error, and does not modify the store at all.
    ///
    /// The payload must be available completely via the given producer: if the producer does not emit at least `payload_length` many bytes, this method panics. It is unspecified whether an incomplete entry is created, and whether any prefix pruning takes place.
    async fn create_entry<P>(
        &mut self,
        namespace_id: &NamespaceId,
        subspace_id: &SubspaceId,
        path: &Path,
        timestamp: Timestamp,
        payload_producer: &mut P,
        payload_length: u64,
        write_capability: &WriteCapability,
        secret: &SubspaceSecret,
    ) -> Result<Option<AuthorisedEntry>, CreateEntryError<Self::InternalError>>
    where
        P: BulkProducer<Item = u8>;

    /// Creates an [`Entry`] with the given data and payload, and atomically inserts the payload and entry into the store, unless it would prune one or more older entries.
    ///
    /// If the new entry would be prefix-pruned immediately by a newer entry already in the store, this method returns `Ok(None)`. If the new entry prefix-prunes older entries in the store, the store remains unchanged.
    ///
    /// If the `write_capability` and the `secret` do not yield a valid authorisation token for the entry that would be created, this method returns an error, and does not modify the store at all.
    ///
    /// The payload must be available completely via the given producer: if the producer does not emit at least `payload_length` many bytes, this method panics. It is unspecified whether an incomplete entry is created, and whether any prefix pruning takes place.
    async fn create_entry_nondestructive<P>(
        &mut self,
        namespace_id: &NamespaceId,
        subspace_id: &SubspaceId,
        path: &Path,
        timestamp: Timestamp,
        payload_producer: &mut P,
        payload_length: u64,
        write_capability: &WriteCapability,
        secret: &SubspaceSecret,
    ) -> Result<NondestructiveInsert, CreateEntryError<Self::InternalError>>
    where
        P: BulkProducer<Item = u8>;

    // /// Inserts a given [`AuthorisedEntry`] together with its payload into the store.
    // ///
    // /// The payload must be available completely via the given producer: if the producer does not emit at least `payload_length` many bytes, this method panics. It is unspecified whether an incomplete entry is created, and whether any prefix pruning takes place.
    // ///
    // /// If the new entry would be prefix-pruned immediately by a newer entry already in the store, this method returns `Ok(false)`, otherwise `Ok(true)`. If the new entry prefix-prunes older entries in the store, those older entries are automatically removed.
    // async fn insert_entry_with_payload<P>(
    //     &mut self,
    //     entry: AuthorisedEntry,
    //     payload_producer: &mut P,
    //     payload_length: u64,
    // ) -> Result<bool, Self::InternalError>
    // where
    //     P: BulkProducer<Item = u8>;

    /// Inserts a given [`AuthorisedEntry`] into the store.
    ///
    /// If the new entry would be prefix-pruned immediately by a newer entry already in the store, this method returns `Ok(false)`, otherwise `Ok(true)`. If the new entry prefix-prunes older entries in the store, those older entries are automatically removed.
    async fn insert_entry(&mut self, entry: AuthorisedEntry) -> Result<bool, Self::InternalError>;

    /// Removes an entry and its payload from the store. Return `Ok(true)` if data was actually removed, and `Ok(false)` if there was no matching entry in the first place.
    ///
    /// If the `expected_digest` is `Some(pd)`, then the addressed entry is only forgotten if its payload digest is `pd`. If the payload digest does not match, the method returns `Ok(false)`.
    ///
    /// Forgetting is not the same as [pruning](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning)! Subsequent [joins](https://willowprotocol.org/specs/data-model/index.html#store_join) with other [`Store`]s may bring the forgotten entry back.
    async fn forget_entry<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
    ) -> Result<bool, Self::InternalError>
    where
        K: Keylike;

    /// Removes all entries and their payloads in some [`Area`] from the store.
    ///
    /// Forgetting is not the same as [pruning](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning)! Subsequent [joins](https://willowprotocol.org/specs/data-model/index.html#store_join) with other [`Store`]s may bring forgotten entries back.
    async fn forget_area(
        &mut self,
        namespace_id: &NamespaceId,
        area: &Area,
    ) -> Result<(), Self::InternalError>;

    /// Removes all entries and their payloads in some namespace from the store.
    ///
    /// Forgetting is not the same as [pruning](https://willowprotocol.org/specs/data-model/index.html#prefix_pruning)! Subsequent [joins](https://willowprotocol.org/specs/data-model/index.html#store_join) with other [`Store`]s may bring forgotten entries back.
    async fn forget_namespace(
        &mut self,
        namespace_id: &NamespaceId,
    ) -> Result<(), Self::InternalError>;

    /// Gets an entry (or `None` if there is none).
    ///
    /// If the `expected_digest` is `Some(pd)`, then the addressed entry is only returned if its payload digest is `pd`. If the payload digest does not match, the method returns `Ok(None)`.
    async fn get_entry<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
    ) -> Result<Option<AuthorisedEntry>, Self::InternalError>
    where
        K: Keylike;

    /// Gets an entry (or `None` if there is none), and writes a slice of its payload into the given consumer.
    ///
    /// If the `expected_digest` is `Some(pd)`, then the addressed entry is only returned if its payload digest is `pd`. If the payload digest does not match, the method returns `Ok(None)`.
    ///
    /// Writes as many bytes as available into `c`. Returns the entry, and how many bytes were written into `c`.
    ///
    /// If the start of the indicated payload slice is strictly greater than its end, or if its start is strictly greater than the length of the stored payload, the behaviour of this method is unspecified.
    async fn get_entry_and_payload_slice<K, C>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
        payload_slice_start: ByteIndex,
        payload_slice_length: ByteCount,
        c: &mut C,
    ) -> Result<
        Option<(AuthorisedEntry, ByteCount)>,
        StoreOrConsumerError<Self::InternalError, C::Error>,
    >
    where
        K: Keylike,
        C: BulkConsumer<Item = u8>;

    /// Writes a slice of the payload stored for some entry into the given [`BulkConsumer`].
    ///
    /// The requested slice starts at offset `start` (zero-indexed) and spans `length` many bytes. The method returns how many payload bytes were successfully written into `c`.
    ///
    /// If the `expected_digest` is `Some(pd)` but the entry for the given namespace_id and key does not have `pd` as its payload digest, the method acts as if no matching entry was found.
    async fn get_payload_slice<K, C>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
        start: ByteIndex,
        length: ByteCount,
        c: &mut C,
    ) -> Result<ByteCount, GetPayloadSliceError<Self::InternalError, C::Error>>
    where
        K: Keylike,
        C: BulkConsumer<Item = u8>;

    /// Writes all entries in some [`Area`] from the store into the given consumer.
    async fn get_area<C>(
        &mut self,
        namespace_id: &NamespaceId,
        area: &Area,
        c: &mut C,
    ) -> Result<(), StoreOrConsumerError<Self::InternalError, C::Error>>
    where
        C: Consumer<Item = AuthorisedEntry>;

    /// Flushes all prior operations. If the backing storage is persistent, all mutations up to that point will have been successfully persisted after a successful flush.
    async fn flush(&mut self) -> Result<(), Self::InternalError>;
}

/// The possible successful outcomes of non-destructively inserting an [`AuthorisedEntry`] into a [`Store`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NondestructiveInsert {
    /// Inserted the entry, and no other data was pruned.
    Success(AuthorisedEntry),
    /// Inserting an entry would have pruned old entries, so instead the store was not modified at all.
    Prevented,
    /// The inserted entry would be pruned by a newer entry in the store, so it was not inserted in the first place.
    Outdated,
}

/// The errors that can be encountered when creating an entry (from a producer emitting the payload and the ingredients for authorising it).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Display, Error)]
pub enum CreateEntryError<StoreError> {
    /// The store implementation encountered an internal error.
    #[display("store error")]
    StoreError(StoreError),
    /// Could not create an authorisation token for the created entry.
    #[display("does not authorise")]
    AuthorisationTokenError,
}

/// The errors that can be encountered when getting a slice of a payload (via [`Store::get_payload_slice`]).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Display, Error)]
pub enum GetPayloadSliceError<StoreError, ConsumerError> {
    /// The store implementation encountered an internal error.
    #[display("store error")]
    StoreError(StoreError),
    /// The payload consumer emitted an error.
    #[display("consumer error")]
    ConsumerError(ConsumerError),
    /// There was no entry in the store for the given namespace and key.
    #[display("no such entry")]
    NoSuchEntry,
}

/// The errors that can be encountered by methods that interact with a store and write data into a consumer.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Display, Error)]
pub enum StoreOrConsumerError<StoreError, ConsumerError> {
    /// The store implementation encountered an internal error.
    #[display("store error")]
    StoreError(StoreError),
    /// The payload consumer emitted an error.
    #[display("consumer error")]
    ConsumerError(ConsumerError),
}
