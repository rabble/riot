use std::convert::Infallible;

use crate::{
    authorisation::PossiblyAuthorisedEntry,
    prelude::*,
    storage::{
        AppendToPayloadPrefixError, GetPayloadSliceError, GetVerifiableStreamError,
        InternalOrNoSuchEntryError, PayloadPrefixStore, Store, StoreOrConsumerError,
        store::{CreateEntryError, NondestructiveInsert},
    },
};
use bab_rs::{
    CHUNK_SIZE,
    generic::storage::{
        backend_memory::{InMemoryBackend, KeyState},
        single_slice_store::SliceStreamResumptionInfo,
        storage_backend::WriteToConsumerError,
        units::*,
        verifiable_streaming::{
            EmitSliceStreamError, IngestSliceStreamError, SliceStreamingOptions,
        },
    },
    storage::SingleSliceStore,
};
use frugal_async::RwLock;
use std::{collections::BTreeMap, rc::Rc};
use ufotofu::prelude::*;

/// All the data in a key ([`Keylike`]), but in owned form.
///
/// Using this is kinda hacky/inefficient, but the Bab APIs require it and there wasn't really a sensible way for adjusting them.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub(crate) struct OwnedNamespacedKey {
    pub(crate) namespace_id: NamespaceId,
    pub(crate) subspace_id: SubspaceId,
    pub(crate) path: Path,
}

/// A non-persistent, in-memory Willow store implementation.
#[derive(Debug)]
pub struct MemoryStore {
    rc: Rc<RwLock<Store_>>,
}

impl MemoryStore {
    /// Creates a new, empty Willow store.
    pub fn new() -> Self {
        MemoryStore {
            rc: Rc::new(RwLock::new(Store_::new())),
        }
    }

    async fn create_entry_impl<P>(
        &mut self,
        namespace_id: &NamespaceId,
        subspace_id: &SubspaceId,
        path: &Path,
        timestamp: Timestamp,
        payload_producer: &mut P,
        payload_length: u64,
        write_capability: &WriteCapability,
        secret: &SubspaceSecret,
        nondestructive: bool,
    ) -> Result<NondestructiveInsert, CreateEntryError<Infallible>>
    where
        P: BulkProducer<Item = u8>,
    {
        let mut store = self.rc.write().await;

        let owned_key = OwnedNamespacedKey {
            namespace_id: namespace_id.clone(),
            subspace_id: subspace_id.clone(),
            path: path.clone(),
        };

        let (payload_store, digest) = SingleSliceStore::create_and_initialise(
            &mut store.bab_key_state,
            owned_key,
            payload_length,
            payload_producer,
        )
        .await
        .unwrap(/* Infallible */);

        let entry: Entry = Entry::builder()
            .namespace_id(namespace_id.clone())
            .subspace_id(subspace_id.clone())
            .path(path.clone())
            .timestamp(timestamp)
            .payload_digest(PayloadDigest(digest.into()))
            .payload_length(payload_length)
            .build();

        let entry = entry
            .into_authorised_entry(write_capability, secret)
            .map_err(|_| CreateEntryError::AuthorisationTokenError)?;

        Ok(store.do_insert_entry(entry, nondestructive, payload_store))
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for MemoryStore {
    type InternalError = Infallible;

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
        P: ufotofu::BulkProducer<Item = u8>,
    {
        match self
            .create_entry_impl(
                namespace_id,
                subspace_id,
                path,
                timestamp,
                payload_producer,
                payload_length,
                write_capability,
                secret,
                false,
            )
            .await?
        {
            NondestructiveInsert::Success(yay) => Ok(Some(yay)),
            NondestructiveInsert::Outdated => Ok(None),
            NondestructiveInsert::Prevented => unreachable!(),
        }
    }

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
        P: ufotofu::BulkProducer<Item = u8>,
    {
        self.create_entry_impl(
            namespace_id,
            subspace_id,
            path,
            timestamp,
            payload_producer,
            payload_length,
            write_capability,
            secret,
            true,
        )
        .await
    }

    // async fn insert_entry_with_payload<P>(
    //     &mut self,
    //     entry: AuthorisedEntry,
    //     payload_producer: &mut P,
    //     payload_length: u64,
    // ) -> Result<bool, Self::InternalError>
    // where
    //     P: BulkProducer<Item = u8>,
    // {
    //     let mut store = self.rc.write().await;

    //     if store.will_be_pruned(&entry) {
    //         return Ok(false);
    //     }

    //     let owned_key = OwnedNamespacedKey {
    //         namespace_id: entry.namespace_id().clone(),
    //         subspace_id: entry.subspace_id().clone(),
    //         path: entry.path().clone(),
    //     };

    //     let (payload_store, digest) = SingleSliceStore::create_and_initialise(
    //         &mut store.bab_key_state,
    //         owned_key,
    //         payload_length,
    //         payload_producer,
    //     )
    //     .await?;

    //     if &PayloadDigest(digest.into()) != entry.payload_digest() {
    //         panic!(
    //             "payload producer must yield a payload that hashes to the claimed digest of the entry"
    //         );
    //     }

    //     match store.do_insert_entry(entry, false, payload_store) {
    //         NondestructiveInsert::Success(_) => Ok(true),
    //         NondestructiveInsert::Outdated => Ok(false),
    //         NondestructiveInsert::Prevented => unreachable!(),
    //     }
    // }

    async fn insert_entry(&mut self, entry: AuthorisedEntry) -> Result<bool, Self::InternalError> {
        // Check whether we are already storing this entry. If we are, we have no work to do!
        if let Some(existing_entry) = self
            .get_entry(
                entry.namespace_id(),
                &entry,
                Some(entry.payload_digest().clone()),
            )
            .await?
            && existing_entry == entry
        {
            return Ok(true);
        }

        let mut store = self.rc.write().await;

        let owned_key = OwnedNamespacedKey {
            namespace_id: entry.namespace_id().clone(),
            subspace_id: entry.subspace_id().clone(),
            path: entry.path().clone(),
        };

        let payload_store = SingleSliceStore::create(
            &mut store.bab_key_state,
            owned_key,
            entry.payload_digest().clone().into_bytes().into(),
            entry.payload_length(),
            0,
            string_length_to_chunk_count::<CHUNK_SIZE>(entry.payload_length()),
        )
        .await?;

        match store.do_insert_entry(entry, false, payload_store) {
            NondestructiveInsert::Outdated => Ok(false),
            NondestructiveInsert::Prevented => unreachable!(),
            NondestructiveInsert::Success(_yay) => Ok(true),
        }
    }

    async fn forget_entry<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
    ) -> Result<bool, Self::InternalError>
    where
        K: Keylike,
    {
        let mut store = self.rc.write().await;

        let namespace_store = store.get_or_create_namespace_store(namespace_id);

        let subspace_store = namespace_store.get_or_create_subspace_store(key.subspace_id());

        let found = subspace_store.entries.get(key.path());

        match found {
            None => Ok(false),
            Some(entry) => {
                if let Some(expected) = expected_digest
                    && entry.payload_digest != expected
                {
                    return Ok(false);
                }

                let owned_key = OwnedNamespacedKey {
                    namespace_id: namespace_id.clone(),
                    subspace_id: key.subspace_id().clone(),
                    path: key.path().clone(),
                };

                let removed_from_subspace_store =
                    subspace_store.entries.remove(key.path()).is_some();

                SingleSliceStore::<InMemoryBackend<_>>::delete(
                    &mut store.bab_key_state,
                    &owned_key,
                )
                .await
                .map_err(|_infallible| unreachable!())?;

                Ok(removed_from_subspace_store)
            }
        }
    }

    async fn forget_area(
        &mut self,
        namespace_id: &NamespaceId,
        area: &Area,
    ) -> Result<(), Self::InternalError> {
        let entries_to_forget = {
            let mut store = self.rc.write().await;

            let namespace_store = store.get_or_create_namespace_store(namespace_id);

            // Here we collect all entries we need to forget.
            let mut entries_to_forget = vec![];

            match area.subspace() {
                Some(subspace_id) => {
                    // Collect entries to forget for a single-subspace area.
                    let subspace_store = namespace_store.get_or_create_subspace_store(subspace_id);

                    for (path, entry) in subspace_store.entries.iter() {
                        if !area.times().includes_value(&entry.timestamp) {
                            continue;
                        }

                        if path.is_prefixed_by(area.path()) {
                            entries_to_forget.push((subspace_id.clone(), path.clone()));
                        }
                    }
                }
                None => {
                    // Collect entries to forget for an all-subspaces area.
                    for (subspace_id, subspace_store) in namespace_store.subspaces.iter() {
                        for (path, entry) in subspace_store.entries.iter() {
                            if !area.times().includes_value(&entry.timestamp) {
                                continue;
                            }

                            if path.is_prefixed_by(area.path()) {
                                entries_to_forget.push((subspace_id.clone(), path.clone()));
                            }
                        }
                    }
                }
            }

            entries_to_forget
        };

        // And finally, forget the collected entries.
        for forget_this in entries_to_forget {
            self.forget_entry(namespace_id, &forget_this, None)
                .await
                .expect("cannot fail when `expected_digest` is `None`");
        }

        Ok(())
    }

    async fn forget_namespace(
        &mut self,
        namespace_id: &NamespaceId,
    ) -> Result<(), Self::InternalError> {
        let mut store = self.rc.write().await;

        // A persistent store would also have to actively delete the Bab-based backing storage of all payloads.
        store.namespaces.remove(namespace_id);

        Ok(())
    }

    async fn get_entry<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
    ) -> Result<Option<AuthorisedEntry>, Self::InternalError>
    where
        K: Keylike,
    {
        let store = self.rc.read().await;

        let namespace_store = match store.get_namespace_store(namespace_id) {
            Some(namespace_store) => namespace_store,
            None => return Ok(None),
        };

        let subspace_store = match namespace_store.get_subspace_store(key.subspace_id()) {
            Some(subspace_store) => subspace_store,
            None => return Ok(None),
        };

        match subspace_store.entries.get(key.path()) {
            None => Ok(None),
            Some(found) => {
                if let Some(expected) = expected_digest
                    && found.payload_digest != expected
                {
                    return Ok(None);
                }

                let entry = Entry::builder()
                    .namespace_id(namespace_id.clone())
                    .subspace_id(key.subspace_id().clone())
                    .path(key.path().clone())
                    .timestamp(found.timestamp)
                    .payload_length(found.payload_length)
                    .payload_digest(found.payload_digest.clone())
                    .build();

                // SAFETY: we checked authorisation upon insertion.
                let authed = unsafe {
                    PossiblyAuthorisedEntry::new(entry, found.authorisation_token.clone())
                        .into_authorised_entry_unchecked()
                };

                Ok(Some(authed))
            }
        }
    }

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
        C: BulkConsumer<Item = u8>,
    {
        // For the most part, this is the same as `get_entry`.
        let mut store = self.rc.write().await;

        let namespace_store = store.get_or_create_namespace_store(namespace_id);

        let subspace_store = namespace_store.get_or_create_subspace_store(key.subspace_id());

        match subspace_store.entries.get_mut(key.path()) {
            None => Ok(None),
            Some(found) => {
                if let Some(expected) = expected_digest
                    && found.payload_digest != expected
                {
                    return Ok(None);
                }

                let entry = Entry::builder()
                    .namespace_id(namespace_id.clone())
                    .subspace_id(key.subspace_id().clone())
                    .path(key.path().clone())
                    .timestamp(found.timestamp)
                    .payload_length(found.payload_length)
                    .payload_digest(found.payload_digest.clone())
                    .build();

                // SAFETY: we checked authorisation upon insertion.
                let authed = unsafe {
                    PossiblyAuthorisedEntry::new(entry, found.authorisation_token.clone())
                        .into_authorised_entry_unchecked()
                };

                // Here we diverge from `get_entry`, to write the payload into `c`.
                let bytecount = found
                    .payload
                    .get_data(c, payload_slice_start, payload_slice_length)
                    .await
                    .map_err(|err| match err {
                        WriteToConsumerError::StorageError(_infallible) => unreachable!(),
                        WriteToConsumerError::ConsumerError(err) => {
                            StoreOrConsumerError::ConsumerError(err)
                        }
                    })?;
                // Guess that was a rather short divergence.

                Ok(Some((authed, bytecount)))
            }
        }
    }

    async fn get_area<C>(
        &mut self,
        namespace_id: &NamespaceId,
        area: &Area,
        c: &mut C,
    ) -> Result<(), StoreOrConsumerError<Self::InternalError, C::Error>>
    where
        C: Consumer<Item = AuthorisedEntry>,
    {
        let store = self.rc.read().await;

        let namespace_store = match store.get_namespace_store(namespace_id) {
            Some(namespace_store) => namespace_store,
            None => return Ok(()),
        };

        match area.subspace() {
            Some(subspace_id) => {
                // Write entries for a single-subspace area.
                let subspace_store = match namespace_store.get_subspace_store(subspace_id) {
                    Some(subspace_store) => subspace_store,
                    None => return Ok(()),
                };

                for (path, entry) in subspace_store.entries.iter() {
                    if !area.times().includes_value(&entry.timestamp) {
                        continue;
                    }

                    if path.is_prefixed_by(area.path()) {
                        let new_entry = Entry::builder()
                            .namespace_id(namespace_id.clone())
                            .subspace_id(subspace_id.clone())
                            .path(path.clone())
                            .timestamp(entry.timestamp)
                            .payload_length(entry.payload_length)
                            .payload_digest(entry.payload_digest.clone())
                            .build();

                        // SAFETY: we checked authorisation upon insertion.
                        let authed = unsafe {
                            PossiblyAuthorisedEntry::new(
                                new_entry,
                                entry.authorisation_token.clone(),
                            )
                            .into_authorised_entry_unchecked()
                        };

                        c.consume_item(authed)
                            .await
                            .map_err(StoreOrConsumerError::ConsumerError)?;
                    }
                }
            }
            None => {
                // Write entries for an all-subspaces area.
                for (subspace_id, subspace_store) in namespace_store.subspaces.iter() {
                    for (path, entry) in subspace_store.entries.iter() {
                        if !area.times().includes_value(&entry.timestamp) {
                            continue;
                        }

                        if path.is_prefixed_by(area.path()) {
                            let new_entry = Entry::builder()
                                .namespace_id(namespace_id.clone())
                                .subspace_id(subspace_id.clone())
                                .path(path.clone())
                                .timestamp(entry.timestamp)
                                .payload_length(entry.payload_length)
                                .payload_digest(entry.payload_digest.clone())
                                .build();

                            // SAFETY: we checked authorisation upon insertion.
                            let authed = unsafe {
                                PossiblyAuthorisedEntry::new(
                                    new_entry,
                                    entry.authorisation_token.clone(),
                                )
                                .into_authorised_entry_unchecked()
                            };

                            c.consume_item(authed)
                                .await
                                .map_err(StoreOrConsumerError::ConsumerError)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn flush(&mut self) -> Result<(), Self::InternalError> {
        // Note that this in-memory store has no need to ever flush payloads (flushing the in-memory backend of Bab is a no-op), but a persistent implementation *does* need to ensure that data is persisted. This could be accomplished either by maintaining a set of all payloads that require flushing and then draining that set here, or by eagerly flushing payload storage in every method that mutates a payload.
        Ok(())
    }

    async fn get_payload_slice<K, C>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
        start: ByteIndex,
        length: ByteCount,
        c: &mut C,
    ) -> Result<u64, GetPayloadSliceError<Self::InternalError, C::Error>>
    where
        K: Keylike,
        C: BulkConsumer<Item = u8>,
    {
        let mut store = self.rc.write().await;

        let namespace_store = store.get_or_create_namespace_store(namespace_id);

        let subspace_store = namespace_store.get_or_create_subspace_store(key.subspace_id());

        match subspace_store.entries.get_mut(key.path()) {
            None => Err(GetPayloadSliceError::NoSuchEntry),
            Some(found) => {
                if let Some(expected) = expected_digest
                    && found.payload_digest != expected
                {
                    return Err(GetPayloadSliceError::NoSuchEntry);
                }

                let amount_written =
                    found
                        .payload
                        .get_data(c, start, length)
                        .await
                        .map_err(|err| match err {
                            WriteToConsumerError::ConsumerError(err) => {
                                GetPayloadSliceError::ConsumerError(err)
                            }
                            WriteToConsumerError::StorageError(_infallible) => unreachable!(),
                        })?;

                Ok(amount_written)
            }
        }
    }
}

impl PayloadPrefixStore for MemoryStore {
    async fn append_to_payload_prefix<K, P>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        p: &mut P,
        stream_options: SliceStreamingOptions,
    ) -> Result<(), AppendToPayloadPrefixError<P::Error, Self::InternalError>>
    where
        K: Keylike,
        P: BulkProducer<Item = u8>,
    {
        let mut store = self.rc.write().await;

        let namespace_store = store.get_or_create_namespace_store(namespace_id);

        let subspace_store = namespace_store.get_or_create_subspace_store(key.subspace_id());

        match subspace_store.entries.get_mut(key.path()) {
            None => Err(AppendToPayloadPrefixError::NoSuchEntry),
            Some(found) => {
                found
                    .payload
                    .append_data(p, stream_options)
                    .await
                    .map_err(|err| match err {
                        IngestSliceStreamError::ProducerError { producer_err, .. } => {
                            AppendToPayloadPrefixError::ProducerError(producer_err)
                        }
                        IngestSliceStreamError::UnexpectedEndOfStream { .. } => {
                            AppendToPayloadPrefixError::UnexpectedEndOfStream
                        }
                        IngestSliceStreamError::StorageBackendError {
                            storage_error: _infallible,
                            ..
                        } => {
                            unreachable!()
                        }
                        IngestSliceStreamError::VerificationError { .. } => {
                            AppendToPayloadPrefixError::VerificationError
                        }
                    })?;

                Ok(())
            }
        }
    }

    async fn length_of_payload_prefix<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
    ) -> Result<ByteCount, InternalOrNoSuchEntryError<Self::InternalError>>
    where
        K: Keylike,
    {
        let mut store = self.rc.write().await;

        let namespace_store = store.get_or_create_namespace_store(namespace_id);

        let subspace_store = namespace_store.get_or_create_subspace_store(key.subspace_id());

        match subspace_store.entries.get_mut(key.path()) {
            None => Err(InternalOrNoSuchEntryError::NoSuchEntry),
            Some(found) => {
                if let Some(expected) = expected_digest
                    && found.payload_digest != expected
                {
                    return Err(InternalOrNoSuchEntryError::NoSuchEntry);
                }

                match found.payload.metadata().slice_stream_resumption_info() {
                    None => Ok(found.payload_length),
                    Some(info) => Ok(info.start_chunk * CHUNK_SIZE as u64),
                }
            }
        }
    }

    async fn prefix_stream_resumption_info<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
    ) -> Result<Option<SliceStreamResumptionInfo>, InternalOrNoSuchEntryError<Self::InternalError>>
    where
        K: Keylike,
    {
        let mut store = self.rc.write().await;

        let namespace_store = store.get_or_create_namespace_store(namespace_id);

        let subspace_store = namespace_store.get_or_create_subspace_store(key.subspace_id());

        match subspace_store.entries.get_mut(key.path()) {
            None => Err(InternalOrNoSuchEntryError::NoSuchEntry),
            Some(found) => {
                if let Some(expected) = expected_digest
                    && found.payload_digest != expected
                {
                    return Err(InternalOrNoSuchEntryError::NoSuchEntry);
                }

                Ok(found.payload.metadata().slice_stream_resumption_info())
            }
        }
    }

    async fn get_verifiable_stream<K, C>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
        expected_digest: Option<PayloadDigest>,
        start: ChunkIndex,
        length: ChunkCount,
        stream_options: SliceStreamingOptions,
        c: &mut C,
    ) -> Result<ByteCount, GetVerifiableStreamError<C::Error, Self::InternalError>>
    where
        K: Keylike,
        C: BulkConsumer<Item = u8>,
    {
        let mut store = self.rc.write().await;

        let namespace_store = store.get_or_create_namespace_store(namespace_id);

        let subspace_store = namespace_store.get_or_create_subspace_store(key.subspace_id());

        match subspace_store.entries.get_mut(key.path()) {
            None => Err(GetVerifiableStreamError::NoSuchEntry),
            Some(found) => {
                if let Some(expected) = expected_digest
                    && found.payload_digest != expected
                {
                    return Err(GetVerifiableStreamError::NoSuchEntry);
                }

                let bytecount = found
                    .payload
                    .get_verifiable_stream(c, start, length, stream_options)
                    .await
                    .map_err(|err| match err {
                        EmitSliceStreamError::ConsumerError(err) => {
                            GetVerifiableStreamError::ConsumerError(err)
                        }
                        EmitSliceStreamError::StorageBackendError(_infallible) => unreachable!(),
                    })?;

                Ok(bytecount)
            }
        }
    }
}

#[derive(Debug)]
struct Store_ {
    namespaces: BTreeMap<NamespaceId, NamespaceStore>,
    bab_key_state: KeyState<OwnedNamespacedKey>,
}

impl Store_ {
    fn new() -> Self {
        Store_ {
            namespaces: BTreeMap::new(),
            bab_key_state: KeyState::new(),
        }
    }

    fn get_namespace_store(&self, namespace_id: &NamespaceId) -> Option<&NamespaceStore> {
        self.namespaces.get(namespace_id)
    }

    fn get_or_create_namespace_store(&mut self, namespace_id: &NamespaceId) -> &mut NamespaceStore {
        if !self.namespaces.contains_key(namespace_id) {
            let _ = self
                .namespaces
                .insert(namespace_id.clone(), NamespaceStore::new());
        }

        self.namespaces.get_mut(namespace_id).unwrap()
    }

    // fn will_be_pruned(&mut self, authorised_entry: &AuthorisedEntry) -> bool {
    //     let namespace_store = self.get_or_create_namespace_store(authorised_entry.namespace_id());

    //     let subspace_store =
    //         namespace_store.get_or_create_subspace_store(authorised_entry.subspace_id());

    //     // Is the inserted entry redundant? If so, return early.
    //     for (path, entry) in subspace_store.entries.iter() {
    //         if path.is_prefix_of(authorised_entry.path())
    //             && entry.is_newer_than(authorised_entry.entry())
    //         {
    //             return true;
    //         }
    //     }

    //     false
    // }

    fn do_insert_entry(
        &mut self,
        authorised_entry: AuthorisedEntry,
        prevent_pruning: bool,
        payload: SingleSliceStore<InMemoryBackend<OwnedNamespacedKey>>,
    ) -> NondestructiveInsert {
        let namespace_store = self.get_or_create_namespace_store(authorised_entry.namespace_id());

        let subspace_store =
            namespace_store.get_or_create_subspace_store(authorised_entry.subspace_id());

        // Is the inserted entry redundant? If so, return early.
        for (path, entry) in subspace_store.entries.iter() {
            if path.is_prefix_of(authorised_entry.path())
                && entry.is_newer_than(authorised_entry.entry())
            {
                return NondestructiveInsert::Outdated;
            }
        }

        if subspace_store.handle_insertion(&authorised_entry, prevent_pruning, true, payload) {
            NondestructiveInsert::Prevented
        } else {
            NondestructiveInsert::Success(authorised_entry)
        }
    }
}

#[derive(Debug)]
struct NamespaceStore {
    subspaces: BTreeMap<SubspaceId, SubspaceStore>,
}

impl NamespaceStore {
    fn new() -> Self {
        NamespaceStore {
            subspaces: BTreeMap::new(),
        }
    }

    fn get_subspace_store(&self, subspace_id: &SubspaceId) -> Option<&SubspaceStore> {
        self.subspaces.get(subspace_id)
    }

    fn get_or_create_subspace_store(&mut self, subspace_id: &SubspaceId) -> &mut SubspaceStore {
        if !self.subspaces.contains_key(subspace_id) {
            let _ = self
                .subspaces
                .insert(subspace_id.clone(), SubspaceStore::new());
        }

        self.subspaces.get_mut(subspace_id).unwrap()
    }
}

#[derive(Debug)]
struct SubspaceStore {
    entries: BTreeMap<Path, ControlEntry>,
}

impl SubspaceStore {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    // Returns `true` if this would have pruned anything but the prevent_pruning flag was set.
    // Inserts the entry if `do_insert_if_necessary` is true, and insertion is allowed (i.e., pruning does not need to be prevented).
    fn handle_insertion(
        &mut self,
        new_entry: &AuthorisedEntry,
        prevent_pruning: bool,
        do_insert_if_necessary: bool,
        payload: SingleSliceStore<InMemoryBackend<OwnedNamespacedKey>>,
    ) -> bool {
        // Does the new entry replace others?
        let prune_these: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(path, entry)| {
                if new_entry.path().is_prefix_of(path) && !entry.is_newer_than(new_entry.entry()) {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect();

        if prevent_pruning && !prune_these.is_empty() {
            true
        } else {
            for path_to_prune in prune_these {
                // A persistent store would also have to actively delete the Bab-based backing storage of the payload.
                self.entries.remove(&path_to_prune);
            }

            if do_insert_if_necessary {
                self.entries.insert(
                    new_entry.path().clone(),
                    ControlEntry {
                        authorisation_token: new_entry.authorisation_token().clone(),
                        payload,
                        payload_digest: new_entry.payload_digest().clone(),
                        payload_length: new_entry.payload_length(),
                        timestamp: new_entry.timestamp(),
                    },
                );
            }

            false
        }
    }
}

impl Clone for MemoryStore {
    fn clone(&self) -> Self {
        Self {
            rc: self.rc.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct ControlEntry {
    timestamp: Timestamp,
    payload_length: u64,
    payload_digest: PayloadDigest,
    authorisation_token: AuthorisationToken,
    payload: SingleSliceStore<InMemoryBackend<OwnedNamespacedKey>>,
}

impl ControlEntry {
    /// [newer than relation](https://willowprotocol.org/specs/data-model/index.html#entry_newer)
    fn is_newer_than(&self, entry: &Entry) -> bool {
        entry.timestamp() < self.timestamp
            || (entry.timestamp() == self.timestamp
                && *entry.payload_digest() < self.payload_digest)
            || (entry.timestamp() == self.timestamp
                && *entry.payload_digest() == self.payload_digest
                && entry.payload_length() < self.payload_length)
    }
}

#[cfg(test)]
mod test {
    use ed25519_dalek::SigningKey;

    use ufotofu::producer::clone_from_slice;

    use crate::defaults::{
        DEFAULT_PAYLOAD_LENGTH, default_authorisation_token, default_authorised_entry,
        default_entry, default_namespace_id, default_payload_digest, default_subspace_id,
        default_subspace_secret,
    };

    use super::*;

    pub static FAMILY_SK: [u8; 32] = [10; 32];
    pub static FAMILY_PK: [u8; NAMESPACE_ID_WIDTH] = [
        67, 167, 46, 113, 68, 1, 118, 45, 246, 107, 104, 194, 109, 251, 223, 38, 130, 170, 236,
        159, 36, 116, 236, 164, 97, 62, 66, 74, 15, 186, 253, 60,
    ];

    pub static ALFIE_SK: [u8; 32] = [1; 32];
    pub static ALFIE_PK: [u8; SUBSPACE_ID_WIDTH] = [
        138, 136, 227, 221, 116, 9, 241, 149, 253, 82, 219, 45, 60, 186, 93, 114, 202, 103, 9, 191,
        29, 148, 18, 27, 243, 116, 136, 1, 180, 15, 111, 92,
    ];

    fn write_cap(
        namespace: &NamespaceId,
        namespace_secret: &NamespaceSecret,
        subspace_id: &SubspaceId,
    ) -> WriteCapability {
        if namespace.is_communal() {
            WriteCapability::new_communal(namespace.clone(), subspace_id.clone())
        } else {
            WriteCapability::new_owned(namespace_secret, subspace_id.clone())
        }
    }

    #[test]
    fn regression_test_001() {
        // Tests whether basic prefix pruning via a non-empty prefix works.
        pollster::block_on(async {
            let mut store = MemoryStore::new();

            let namespace_id = NamespaceId::from_bytes(&FAMILY_PK);
            let namespace_secret = NamespaceSecret::from(SigningKey::from_bytes(&FAMILY_SK));

            let subspace_id = SubspaceId::from_bytes(&ALFIE_PK);
            let subspace_secret = SubspaceSecret::from(SigningKey::from_bytes(&ALFIE_SK));

            let write_capability = write_cap(&namespace_id, &namespace_secret, &subspace_id);

            let path1 = path!("/a/b");
            let timestamp = 0;

            let payload = &[];
            let mut p = clone_from_slice(payload);

            // Add an entry, to be overwritten later.
            store
                .create_entry(
                    &namespace_id,
                    &subspace_id,
                    &path1,
                    timestamp.into(),
                    &mut p,
                    payload.len() as u64,
                    &write_capability,
                    &subspace_secret,
                )
                .await
                .unwrap();

            // The first entry is in the store.
            let len_of_payload_prefix = store
                .length_of_payload_prefix(
                    &namespace_id,
                    &(subspace_id.clone(), path1.clone()),
                    None,
                )
                .await;
            assert!(len_of_payload_prefix.is_ok());

            // Add a newer entry whose non-empty path prefixes the first path.
            let path2 = path!("/a");
            store
                .create_entry(
                    &namespace_id,
                    &subspace_id,
                    &path2,
                    (timestamp + 1).into(),
                    &mut p,
                    payload.len() as u64,
                    &write_capability,
                    &subspace_secret,
                )
                .await
                .unwrap();

            // The first entry is no longerin the store.
            let len_of_payload_prefix2 = store
                .length_of_payload_prefix(
                    &namespace_id,
                    &(subspace_id.clone(), path1.clone()),
                    None,
                )
                .await;
            assert!(matches!(
                len_of_payload_prefix2,
                Err(InternalOrNoSuchEntryError::NoSuchEntry)
            ));
        });
    }

    #[test]
    fn regression_test_002() {
        // Write to empty path at high timestamp, then again with a lower timestamp. Ensure that the first payload stays unaffected.
        pollster::block_on(async {
            let mut store = MemoryStore::new();
            let default_authed = default_authorised_entry();

            // Add an entry at the empty path, to be overwritten later.
            let later_authed = store
                .create_entry(
                    &default_namespace_id(),
                    &default_subspace_id(),
                    &Path::new(),
                    (1).into(),
                    &mut [].into_producer(),
                    DEFAULT_PAYLOAD_LENGTH,
                    &default_authorisation_token().capability(),
                    &default_subspace_secret(),
                )
                .await
                .unwrap()
                .unwrap();

            store
                .forget_area(&default_namespace_id(), &Area::full())
                .await
                .unwrap();

            // Inserting entries with equal keys should return Ok(true) when inserted in timestamp-ascending order
            assert!(store.insert_entry(default_authed.clone()).await.unwrap());
            assert!(store.insert_entry(later_authed.clone()).await.unwrap());

            // Inserting entries with equal keys should return Ok(false) when inserted in timestamp-descending order
            assert!(!store.insert_entry(default_authed).await.unwrap());

            // Retrieving an entry should give the most recently inserted entry with that key.
            assert_eq!(
                store
                    .get_entry(
                        &default_namespace_id(),
                        &default_entry(),
                        Some(default_payload_digest())
                    )
                    .await
                    .unwrap(),
                Some(later_authed)
            );
        });
    }
}
