use core::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use crate::authorisation::PossiblyAuthorisedEntry;
use crate::prelude::*;
use crate::storage::AppendToPayloadPrefixError;
use crate::storage::CreateEntryError;
use crate::storage::GetPayloadSliceError;
use crate::storage::GetVerifiableStreamError;
use crate::storage::InternalOrNoSuchEntryError;
use crate::storage::NondestructiveInsert;
use crate::storage::PayloadPrefixStore;
use crate::storage::Store;
use crate::storage::StoreOrConsumerError;
use bab_rs::generic::storage::single_slice_store::SliceStreamResumptionInfo;
use bab_rs::generic::storage::storage_backend::OperationsError;
use bab_rs::generic::storage::units::ByteCount;
use bab_rs::generic::storage::units::ByteIndex;
use bab_rs::generic::storage::units::ChunkCount;
use bab_rs::generic::storage::units::ChunkIndex;
use bab_rs::generic::storage::units::string_length_to_chunk_count;
use bab_rs::{
    CHUNK_SIZE,
    generic::storage::{
        backend_filesystem::{FileBackend, KeyState},
        storage_backend::WriteToConsumerError,
        verifiable_streaming::{
            EmitSliceStreamError, IngestSliceStreamError, SliceStreamingOptions,
        },
    },
    storage::SingleSliceStore,
};
use base64::prelude::*;
use derive_more::{Display, Error};
use fjall::Guard;
use fjall::PersistMode;
use fjall::{Database, Error as FjallError, Keyspace, KeyspaceCreateOptions};
use frugal_async::RwLock;
use order_theory::GreatestElement;
use order_theory::LeastElement;
use order_theory::TrySuccessor;
use std::path;
use ufotofu::BulkProducer;
use ufotofu::IntoConsumer;
use ufotofu::codec::Decodable;
use ufotofu::codec::Encodable;
use ufotofu::codec::EncodableExt;
use ufotofu::prelude::*;
use ufotofu::producer::clone_from_slice;

/// A Willow data [store](https://willowprotocol.org/specs/data-model/index.html#store) that persists Willow data to disk.
pub struct PersistentStore {
    rc: Rc<RwLock<InnerPersistentStore_>>,
}

impl PersistentStore {
    /// Returns a new [`PersistentStore`] that stores data on the file system in the directory `path`.
    pub async fn new<P>(path: P) -> Result<Self, FjallError>
    where
        P: AsRef<path::Path>,
    {
        let mut db_path = path.as_ref().to_path_buf().clone();
        db_path.push(path::Path::new("db"));

        match Database::builder(db_path.as_path()).open() {
            Ok(database) => {
                let mut payload_path_buf = path.as_ref().to_path_buf().clone();
                payload_path_buf.push(path::Path::new("payloads"));

                Ok(Self {
                    rc: Rc::new(RwLock::new(InnerPersistentStore_::new(
                        database,
                        &payload_path_buf.as_path(),
                    ))),
                })
            }
            Err(err) => Err(err),
        }
    }
}

impl Clone for PersistentStore {
    fn clone(&self) -> Self {
        Self {
            rc: self.rc.clone(),
        }
    }
}

struct InnerPersistentStore_ {
    database: Database,
    bab_key_state: KeyState,
    payload_dir: PathBuf,
}

impl InnerPersistentStore_ {
    /// Returns a new [`PersistentStore`] using the given [`Database`].
    pub fn new<P>(database: Database, payload_dir: &P) -> Self
    where
        P: AsRef<path::Path>,
    {
        Self {
            database,
            bab_key_state: KeyState::new(),
            payload_dir: payload_dir.as_ref().to_path_buf(),
        }
    }

    fn namespace_keyspace(&self, namespace: &NamespaceId) -> Result<Keyspace, FjallError> {
        self.database.keyspace(
            &namespace_to_base64(namespace),
            KeyspaceCreateOptions::default,
        )
    }

    /// Returns the first fjall k/v pair prefixed with the given key, or `Ok(None)` if nothing was found.
    fn prefix_gt(
        &self,
        namespace_keyspace: &Keyspace,
        prefix: &[u8],
    ) -> Result<Option<Guard>, FjallError> {
        if let Some(guard) = namespace_keyspace.prefix(prefix).next() {
            Ok(Some(guard))
        } else {
            Ok(None)
        }
    }

    /// Returns ['true'] if the given entry will be pruned by entries persisted by the store.
    async fn will_be_pruned<E>(
        &self,
        namespace_keyspace: &Keyspace,
        entry: &E,
    ) -> Result<Option<AuthorisedEntry>, FjallError>
    where
        E: Entrylike,
    {
        let prefix = entry.subspace_id().as_bytes();

        for item in namespace_keyspace.prefix(prefix) {
            let (key, value) = item.into_inner()?;

            let (other_subspace, other_path, other_timestamp) = decode_spt_key(&key).await;

            let (other_payload_length, other_payload_digest, other_auth_token) =
                decode_entry_values(&value).await;

            // We unwrap here because we should only be persisting valid entries.
            let other_entry = Entry::builder()
                .namespace_id(entry.namespace_id().clone())
                .subspace_id(other_subspace)
                .path(other_path)
                .timestamp(other_timestamp)
                .payload_length(other_payload_length)
                .payload_digest(other_payload_digest)
                .build();

            if entry.path().is_prefixed_by(other_entry.path()) && other_entry.is_newer_than(entry) {
                // unwrap because we trust we only persist authorised entries
                let authed = PossiblyAuthorisedEntry::new(other_entry, other_auth_token)
                    .into_authorised_entry()
                    .unwrap();

                return Ok(Some(authed));
            }
        }

        Ok(None)
    }

    /// Returns a vec of pairs of Fjall keys and Pathbufs, representing entries which would be pruned by the given entry and their corresponding payload.
    async fn collect_keys_to_prune<E>(
        &self,
        namespace_keyspace: &Keyspace,
        entry: &E,
    ) -> Result<Vec<(fjall::Slice, PathBuf)>, FjallError>
    where
        E: Entrylike,
    {
        let same_subspace_path_prefix =
            encode_subspace_path_key(entry.subspace_id(), entry.path(), false).await;

        let mut keys_to_prune: Vec<(fjall::Slice, PathBuf)> = Vec::new();
        for item in namespace_keyspace.prefix(same_subspace_path_prefix) {
            let (key, value) = item.into_inner()?;

            let (other_subspace, other_path, other_timestamp) = decode_spt_key(&key).await;
            let (other_payload_length, other_payload_digest, _other_auth_token) =
                decode_entry_values(&value).await;

            // We unwrap here because we should only be persisting valid entries.
            let other_entry = Entry::builder()
                .namespace_id(entry.namespace_id().clone())
                .subspace_id(other_subspace.clone())
                .path(other_path.clone())
                .timestamp(other_timestamp)
                .payload_length(other_payload_length)
                .payload_digest(other_payload_digest)
                .build();

            if entry.is_newer_than(&other_entry) {
                let buf = payload_key(
                    self.payload_dir.clone(),
                    entry.namespace_id(),
                    &other_subspace,
                    &other_path,
                )
                .await;

                keys_to_prune.push((key, buf))
            }
        }

        Ok(keys_to_prune)
    }

    async fn remove_payload(
        &mut self,
        namespace_id: &NamespaceId,
        subspace_id: &SubspaceId,
        path: &Path,
    ) -> Result<(), PersistentStoreError> {
        let payload_path_buf =
            payload_key(self.payload_dir.clone(), namespace_id, subspace_id, path).await;

        SingleSliceStore::<FileBackend>::delete(&mut self.bab_key_state, &payload_path_buf)
            .await
            .map_err(|err| {
                PersistentStoreError::Bab(OperationsError::Internal {
                    err,
                    is_fatal: false,
                })
            })?;

        Ok(())
    }

    /// Try and load a [`SingleSliceStore`] for a given namespace and key.
    async fn load_payload<K>(
        &mut self,
        namespace_id: &NamespaceId,
        key: &K,
    ) -> Result<Option<SingleSliceStore<FileBackend>>, PersistentStoreError>
    where
        K: Keylike,
    {
        let payload_key = payload_key(
            self.payload_dir.clone(),
            namespace_id,
            key.subspace_id(),
            key.path(),
        )
        .await;

        SingleSliceStore::<FileBackend>::load(&mut self.bab_key_state, &payload_key)
            .await
            .map_err(|err| {
                PersistentStoreError::Bab(OperationsError::Internal {
                    err,
                    is_fatal: false,
                })
            })
    }
}

impl From<FjallError> for CreateEntryError<PersistentStoreError> {
    fn from(value: FjallError) -> Self {
        CreateEntryError::StoreError(PersistentStoreError::Fjall(value))
    }
}

impl From<OperationsError<std::io::Error>> for CreateEntryError<PersistentStoreError> {
    fn from(value: OperationsError<std::io::Error>) -> Self {
        CreateEntryError::StoreError(PersistentStoreError::Bab(value))
    }
}

impl From<std::io::Error> for CreateEntryError<PersistentStoreError> {
    fn from(value: std::io::Error) -> Self {
        CreateEntryError::StoreError(PersistentStoreError::Bab(OperationsError::Internal {
            err: value,
            is_fatal: true,
        }))
    }
}

impl From<FjallError> for PersistentStoreError {
    fn from(value: FjallError) -> Self {
        PersistentStoreError::Fjall(value)
    }
}

#[derive(Debug, Display, Error)]
/// The kinds of error a [`PersistentStore`] can return.
pub enum PersistentStoreError {
    /// An error caused by the underlying [`Database`].
    #[display("fjall error")]
    Fjall(FjallError),
    /// An error caused by the underlying Bab storage.
    #[display("bab error")]
    Bab(OperationsError<std::io::Error>),
}

impl PersistentStore {
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
    ) -> Result<Option<AuthorisedEntry>, CreateEntryError<<PersistentStore as Store>::InternalError>>
    where
        P: BulkProducer<Item = u8>,
    {
        let mut store = self.rc.write().await;

        let namespace_keyspace = store.namespace_keyspace(namespace_id)?;

        let key = payload_key(store.payload_dir.clone(), namespace_id, subspace_id, path).await;
        let mut tmp_root = store.payload_dir.clone();
        tmp_root.push(".tmp");
        let tmp_key = payload_key(tmp_root.clone(), namespace_id, subspace_id, path).await;

        let (mut payload_store, digest) = SingleSliceStore::<FileBackend>::create_and_initialise(
            &mut store.bab_key_state,
            tmp_key.clone(),
            payload_length,
            payload_producer,
        )
        .await?;

        // We can unwrap here because we've got all the details.
        let entry = Entry::builder()
            .namespace_id(namespace_id.clone())
            .subspace_id(subspace_id.clone())
            .path(path.clone())
            .timestamp(timestamp)
            .payload_digest(PayloadDigest(digest.into()))
            .payload_length(payload_length)
            .build();

        // Check if this entry would be pruned immediately.
        match store.will_be_pruned(&namespace_keyspace, &entry).await {
            Ok(Some(_newer)) => return Ok(None),
            Err(err) => return Err(CreateEntryError::from(err)),
            Ok(None) => {
                // It's fine, continue!
            }
        }

        // Check if this entry will prune any other entries.
        let keys_to_prune = store
            .collect_keys_to_prune(&namespace_keyspace, &entry)
            .await
            .map_err(CreateEntryError::from)?;

        let mut batch = store.database.batch();

        for (fjall_key, payload_path_buf) in keys_to_prune {
            batch.remove(&namespace_keyspace, fjall_key);

            SingleSliceStore::<FileBackend>::delete(&mut store.bab_key_state, &payload_path_buf)
                .await
                .map_err(|err| {
                    CreateEntryError::StoreError(PersistentStoreError::Bab(
                        OperationsError::Internal {
                            err,
                            is_fatal: false,
                        },
                    ))
                })?;
        }

        let auth_token = AuthorisationToken::new_for_entry(&entry, write_capability, secret)
            .map_err(|_| CreateEntryError::AuthorisationTokenError)?;

        // We can unwrap here as we verified authenticity when constructing the auth_token just above.
        let authed_entry = PossiblyAuthorisedEntry::new(entry, auth_token)
            .into_authorised_entry()
            .unwrap();

        let entry_key = encode_spt_key(
            authed_entry.subspace_id(),
            authed_entry.path(),
            authed_entry.timestamp(),
        )
        .await;
        let entry_value = encode_entry_values(
            authed_entry.payload_length(),
            authed_entry.payload_digest(),
            authed_entry.authorisation_token(),
        )
        .await;

        batch.insert(&namespace_keyspace, entry_key, entry_value);
        batch.commit()?;

        SingleSliceStore::<FileBackend>::rename(&mut store.bab_key_state, &tmp_key, key).await?;

        payload_store.flush().await?;

        let _ = async_fs::remove_dir_all(&tmp_root).await;

        Ok(Some(authed_entry))
    }

    async fn create_entry_nondestructive_impl<P>(
        &mut self,
        namespace_id: &NamespaceId,
        subspace_id: &SubspaceId,
        path: &Path,
        timestamp: Timestamp,
        payload_producer: &mut P,
        payload_length: u64,
        write_capability: &WriteCapability,
        secret: &SubspaceSecret,
    ) -> Result<NondestructiveInsert, CreateEntryError<<PersistentStore as Store>::InternalError>>
    where
        P: BulkProducer<Item = u8>,
    {
        // Produce a payload digest.

        let mut store = self.rc.write().await;

        let namespace_keyspace = store.namespace_keyspace(namespace_id)?;

        let key = payload_key(store.payload_dir.clone(), namespace_id, subspace_id, path).await;
        let mut tmp_root = store.payload_dir.clone();
        tmp_root.push(".tmp");
        let tmp_key = payload_key(tmp_root.clone(), namespace_id, subspace_id, path).await;

        let (mut payload_store, digest) = SingleSliceStore::<FileBackend>::create_and_initialise(
            &mut store.bab_key_state,
            tmp_key.clone(),
            payload_length,
            payload_producer,
        )
        .await?;

        // We can unwrap here because we've got all the details.
        let entry = Entry::builder()
            .namespace_id(namespace_id.clone())
            .subspace_id(subspace_id.clone())
            .path(path.clone())
            .timestamp(timestamp)
            .payload_digest(PayloadDigest(digest.into()))
            .payload_length(payload_length)
            .build();

        // Check if this entry would be pruned immediately.
        match store.will_be_pruned(&namespace_keyspace, &entry).await {
            Ok(Some(_newer)) => return Ok(NondestructiveInsert::Outdated),
            Err(err) => return Err(CreateEntryError::from(err)),
            Ok(None) => {
                // It's fine, continue!
            }
        }

        // Check if this entry will prune any other entries.
        let keys_to_prune = store
            .collect_keys_to_prune(&namespace_keyspace, &entry)
            .await
            .map_err(CreateEntryError::from)?;

        if !keys_to_prune.is_empty() {
            return Ok(NondestructiveInsert::Prevented);
        }

        let mut batch = store.database.batch();

        let auth_token = AuthorisationToken::new_for_entry(&entry, write_capability, secret)
            .map_err(|_| CreateEntryError::AuthorisationTokenError)?;

        // We can unwrap here as we verified authenticity when constructing the auth_token just above.
        let authed_entry = PossiblyAuthorisedEntry::new(entry, auth_token)
            .into_authorised_entry()
            .unwrap();

        let entry_key = encode_spt_key(
            authed_entry.subspace_id(),
            authed_entry.path(),
            authed_entry.timestamp(),
        )
        .await;
        let entry_value = encode_entry_values(
            authed_entry.payload_length(),
            authed_entry.payload_digest(),
            authed_entry.authorisation_token(),
        )
        .await;

        batch.insert(&namespace_keyspace, entry_key, entry_value);
        batch.commit()?;

        SingleSliceStore::<FileBackend>::rename(&mut store.bab_key_state, &tmp_key, key).await?;

        payload_store.flush().await?;

        let _ = async_fs::remove_dir_all(&tmp_root).await;

        Ok(NondestructiveInsert::Success(authed_entry))
    }

    /// Writes all [`Subspace`]s within a given [`Namespace`] with at least one known entry from within the store into the given consumer.
    pub async fn subspaces<C>(
        &self,
        namespace_id: &NamespaceId,
        consumer: &mut C,
    ) -> Result<(), StoreOrConsumerError<PersistentStoreError, C::Error>>
    where
        C: Consumer<Item = SubspaceId, Final = ()>,
    {
        let store = self.rc.read().await;
        let namespace_keyspace = store
            .namespace_keyspace(namespace_id)
            .map_err(|err| StoreOrConsumerError::StoreError(PersistentStoreError::Fjall(err)))?;

        let mut current_subspace = Some(SubspaceId::least());

        while let Some(subspace) = current_subspace {
            let mut iter = namespace_keyspace.range(subspace.to_bytes()..);

            match iter.next() {
                Some(guard) => {
                    let key = guard.key().map_err(|err| {
                        StoreOrConsumerError::StoreError(PersistentStoreError::Fjall(err))
                    })?;

                    let (subspace, _path, _timestamp) = decode_spt_key(&key).await;

                    current_subspace = subspace.try_successor();

                    consumer
                        .consume_item(subspace)
                        .await
                        .map_err(|err| StoreOrConsumerError::ConsumerError(err))?;
                }
                None => current_subspace = None,
            }
        }

        Ok(())
    }
}

impl Store for PersistentStore {
    type InternalError = PersistentStoreError;

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
        P: BulkProducer<Item = u8>,
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
            )
            .await
        {
            Ok(yay) => Ok(yay),
            Err(err) => {
                let store = self.rc.write().await;

                let mut tmp_root = store.payload_dir.clone();
                tmp_root.push(".tmp");

                let _ = async_fs::remove_dir_all(&tmp_root).await;
                Err(err)
            }
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
        P: BulkProducer<Item = u8>,
    {
        match self
            .create_entry_nondestructive_impl(
                namespace_id,
                subspace_id,
                path,
                timestamp,
                payload_producer,
                payload_length,
                write_capability,
                secret,
            )
            .await
        {
            Ok(yay) => Ok(yay),
            Err(err) => {
                let store = self.rc.write().await;

                let mut tmp_root = store.payload_dir.clone();
                tmp_root.push(".tmp");

                let _ = async_fs::remove_dir_all(&tmp_root).await;
                Err(err)
            }
        }
    }

    async fn insert_entry(&mut self, entry: AuthorisedEntry) -> Result<bool, Self::InternalError> {
        // Check whether we are already storing this entry. If we are, we have no work to do!
        if let Some(existing_entry) = self
            .get_entry(
                entry.namespace_id(),
                &entry,
                Some(entry.payload_digest().clone()),
            )
            .await?
            && entry == existing_entry
        {
            return Ok(true);
        }

        let mut store = self.rc.write().await;

        let namespace_keyspace = store.namespace_keyspace(entry.namespace_id())?;

        // Check if this entry would be pruned immediately.
        match store.will_be_pruned(&namespace_keyspace, &entry).await {
            Ok(Some(_newer)) => return Ok(false),
            Err(err) => return Err(err.into()),
            Ok(None) => {
                // It's fine, continue!
            }
        }

        let key = payload_key(
            store.payload_dir.clone(),
            entry.namespace_id(),
            entry.subspace_id(),
            entry.path(),
        )
        .await;

        // Check if this entry will prune any other entries.
        let keys_to_prune = store
            .collect_keys_to_prune(&namespace_keyspace, &entry)
            .await?;

        let mut batch = store.database.batch();

        for (fjall_key, payload_path_buf) in keys_to_prune {
            batch.remove(&namespace_keyspace, fjall_key);

            SingleSliceStore::<FileBackend>::delete(&mut store.bab_key_state, &payload_path_buf)
                .await
                .map_err(|err| {
                    PersistentStoreError::Bab(OperationsError::Internal {
                        err,
                        is_fatal: false,
                    })
                })?;
        }

        let entry_key = encode_spt_key(entry.subspace_id(), entry.path(), entry.timestamp()).await;

        let entry_value = encode_entry_values(
            entry.payload_length(),
            entry.payload_digest(),
            entry.authorisation_token(),
        )
        .await;

        batch.insert(&namespace_keyspace, entry_key, entry_value);

        let mut store = SingleSliceStore::<FileBackend>::create(
            &mut store.bab_key_state,
            key,
            entry.payload_digest().clone().into_bytes().into(),
            entry.payload_length(),
            0,
            string_length_to_chunk_count::<CHUNK_SIZE>(entry.payload_length()),
        )
        .await
        .map_err(|err| {
            PersistentStoreError::Bab(OperationsError::Internal {
                err,
                is_fatal: false,
            })
        })?;

        store.flush().await.map_err(PersistentStoreError::Bab)?;

        batch.commit()?;

        Ok(true)
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

    //     let namespace_keyspace = store
    //         .namespace_keyspace(entry.namespace_id())
    //         .map_err(|err| PersistentStoreError::Fjall(err))?;

    //     // Check if this entry would be pruned immediately.
    //     match store.will_be_pruned(&namespace_keyspace, &entry).await {
    //         Ok(Some(_newer)) => return Ok(false),
    //         Err(err) => {
    //             return Err(PersistentStoreError::Fjall(err));
    //         }
    //         Ok(None) => {
    //             // It's fine, continue with producing bytes!
    //         }
    //     }

    //     // Check if this entry will prune any other entries.
    //     let keys_to_prune = store
    //         .collect_keys_to_prune(&namespace_keyspace, &entry)
    //         .await
    //         .map_err(|err| PersistentStoreError::Fjall(err))?;

    //     let mut batch = store.database.batch();

    //     for (fjall_key, payload_path_buf) in keys_to_prune {
    //         batch.remove(&namespace_keyspace, fjall_key);

    //         SingleSliceStore::<FileBackend>::delete(&mut store.bab_key_state, &payload_path_buf)
    //             .await
    //             .map_err(|err| {
    //                 PersistentStoreError::Bab(OperationsError::Internal {
    //                     err,
    //                     is_fatal: false,
    //                 })
    //             })?;
    //     }

    //     let entry_key = encode_spt_key(entry.subspace_id(), entry.path(), entry.timestamp()).await;

    //     let entry_value = encode_entry_values(
    //         entry.payload_length(),
    //         entry.payload_digest(),
    //         entry.authorisation_token(),
    //     )
    //     .await;

    //     batch.insert(&namespace_keyspace, entry_key, entry_value);

    //     let key = payload_key(
    //         store.payload_dir.clone(),
    //         entry.namespace_id(),
    //         entry.subspace_id(),
    //         entry.path(),
    //     )
    //     .await;

    //     let (mut payload_store, digest) = SingleSliceStore::<FileBackend>::create_and_initialise(
    //         &mut store.bab_key_state,
    //         key,
    //         payload_length,
    //         payload_producer,
    //     )
    //     .await
    //     .map_err(|err| {
    //         PersistentStoreError::Bab(OperationsError::Internal {
    //             err,
    //             is_fatal: true,
    //         })
    //     })?;

    //     if &PayloadDigest(digest.into()) != entry.payload_digest() {
    //         panic!(
    //             "payload producer must yield a payload that hashes to the claimed digest of the entry"
    //         );
    //     }

    //     payload_store
    //         .flush()
    //         .await
    //         .map_err(|err| PersistentStoreError::Bab(err))?;

    //     batch
    //         .commit()
    //         .map_err(|err| PersistentStoreError::Fjall(err))?;

    //     Ok(true)
    // }

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

        let namespace_keyspace = store.namespace_keyspace(namespace_id)?;

        // Does an entry prefixed with this subspace + path exist?
        let subspace_path_prefix =
            encode_subspace_path_key(key.subspace_id(), key.path(), true).await;

        // Then remove it.
        if let Some(guard) = store.prefix_gt(&namespace_keyspace, &subspace_path_prefix)? {
            let (entry_key, entry_value) = guard.into_inner()?;

            if let Some(expected) = expected_digest {
                let (_payload_length, payload_digest, _token) =
                    decode_entry_values(&entry_value).await;

                if expected != payload_digest {
                    return Ok(false);
                }
            }

            namespace_keyspace.remove(entry_key)?;

            store
                .remove_payload(namespace_id, key.subspace_id(), key.path())
                .await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn forget_area(
        &mut self,
        namespace_id: &NamespaceId,
        area: &Area,
    ) -> Result<(), Self::InternalError> {
        let mut store = self.rc.write().await;

        let namespace_keyspace = store.namespace_keyspace(namespace_id)?;

        let entry_iterator = match area.subspace() {
            Some(subspace) => {
                // One particular subspace.
                let subspace_prefix = encode_subspace_path_key(subspace, area.path(), false).await;

                namespace_keyspace.prefix(subspace_prefix)
            }
            None => {
                // Any subspace.
                namespace_keyspace.iter()
            }
        };

        let mut batch = store.database.batch();

        for guard in entry_iterator {
            let (key, _value) = guard.into_inner()?;

            let (subspace_id, path, timestamp) = decode_spt_key(&key).await;

            let prefix_matches = match area.subspace() {
                Some(area_subspace) => {
                    area_subspace == &subspace_id && path.is_prefixed_by(area.path())
                }
                None => true,
            };

            if !prefix_matches {
                continue;
            }

            let timestamp_included = area.times().includes_value(&timestamp);

            if !timestamp_included {
                continue;
            }

            batch.remove(&namespace_keyspace, key);
            store
                .remove_payload(namespace_id, &subspace_id, &path)
                .await?;
        }

        batch.commit()?;

        Ok(())
    }

    async fn forget_namespace(
        &mut self,
        namespace_id: &NamespaceId,
    ) -> Result<(), Self::InternalError> {
        let mut store = self.rc.write().await;

        let namespace_keyspace = store.namespace_keyspace(namespace_id)?;

        // First remove all payloads in this namespace.
        for guard in namespace_keyspace.iter() {
            let key = guard.key()?;
            let (subspace, path, _timestamp) = decode_spt_key(&key).await;
            store.remove_payload(namespace_id, &subspace, &path).await?;
        }

        store.database.delete_keyspace(namespace_keyspace)?;

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

        let namespace_keyspace = store.namespace_keyspace(namespace_id)?;

        // Does an entry prefixed with this subspace + path exist?
        let subspace_path_prefix =
            encode_subspace_path_key(key.subspace_id(), key.path(), true).await;

        // Then get it.
        if let Some(guard) = store.prefix_gt(&namespace_keyspace, &subspace_path_prefix)? {
            let (entry_key, entry_value) = guard.into_inner()?;

            let (payload_length, payload_digest, token) = decode_entry_values(&entry_value).await;

            if let Some(expected) = expected_digest
                && expected != payload_digest
            {
                return Ok(None);
            }

            let (subspace_id, path, timestamp) = decode_spt_key(&entry_key).await;

            // We can unwrap here because we know we have all the details.
            let entry = Entry::builder()
                .namespace_id(namespace_id.clone())
                .subspace_id(subspace_id)
                .path(path)
                .timestamp(timestamp)
                .payload_length(payload_length)
                .payload_digest(payload_digest)
                .build();

            // We can unwrap the entry here because we are only storing valid tokens.
            let authed_entry = PossiblyAuthorisedEntry::new(entry, token)
                .into_authorised_entry()
                .unwrap();

            Ok(Some(authed_entry))
        } else {
            Ok(None)
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
        let mut store = self.rc.write().await;

        let namespace_keyspace = store
            .namespace_keyspace(namespace_id)
            .map_err(|err| StoreOrConsumerError::StoreError(PersistentStoreError::Fjall(err)))?;

        // Does entry prefixed with these key exist?
        let subspace_path_prefix =
            encode_subspace_path_key(key.subspace_id(), key.path(), true).await;

        // Then get it.
        if let Some(guard) = store
            .prefix_gt(&namespace_keyspace, &subspace_path_prefix)
            .map_err(|err| StoreOrConsumerError::StoreError(PersistentStoreError::Fjall(err)))?
        {
            let (entry_key, entry_value) = guard.into_inner().map_err(|err| {
                StoreOrConsumerError::StoreError(PersistentStoreError::Fjall(err))
            })?;

            let (payload_length, payload_digest, token) = decode_entry_values(&entry_value).await;

            if let Some(expected) = expected_digest
                && expected != payload_digest
            {
                return Ok(None);
            }

            let (subspace_id, path, timestamp) = decode_spt_key(&entry_key).await;

            // We can unwrap here because we know we have all the details.
            let entry = Entry::builder()
                .namespace_id(namespace_id.clone())
                .subspace_id(subspace_id.clone())
                .path(path.clone())
                .timestamp(timestamp)
                .payload_length(payload_length)
                .payload_digest(payload_digest)
                .build();

            // We can unwrap here because we are only storing valid tokens.
            let authed_entry = PossiblyAuthorisedEntry::new(entry, token)
                .into_authorised_entry()
                .unwrap();

            let payload_location_buf =
                payload_key(store.payload_dir.clone(), namespace_id, &subspace_id, &path).await;

            let maybe_slice_store = SingleSliceStore::<FileBackend>::load(
                &mut store.bab_key_state,
                &payload_location_buf,
            )
            .await
            .map_err(|err| {
                StoreOrConsumerError::StoreError(PersistentStoreError::Bab(
                    OperationsError::Internal {
                        err,
                        is_fatal: false,
                    },
                ))
            })?;

            let byte_count = if let Some(mut store) = maybe_slice_store {
                store
                    .get_data(c, payload_slice_start, payload_slice_length)
                    .await
                    .map_err(|err| match err {
                        WriteToConsumerError::StorageError(operations_error) => {
                            StoreOrConsumerError::StoreError(PersistentStoreError::Bab(
                                operations_error,
                            ))
                        }
                        WriteToConsumerError::ConsumerError(err) => {
                            StoreOrConsumerError::ConsumerError(err)
                        }
                    })?
            } else {
                0
            };

            Ok(Some((authed_entry, byte_count)))
        } else {
            Ok(None)
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
        let range: Range3d = area.clone().into();

        let range_start_key = encode_spt_key(
            range.subspaces().start(),
            range.paths().start(),
            *range.times().start(),
        )
        .await;

        let open_subspace_end = SubspaceId::greatest();
        let open_path_end = Path::greatest();

        let subspace_end = range.subspaces().end().unwrap_or(&open_subspace_end);
        let path_end = range.paths().end().unwrap_or(&open_path_end);

        let greatest_timestamp = Timestamp::from(u64::MAX);
        let time_end = range.times().end().unwrap_or(&greatest_timestamp);
        let range_end_key = encode_spt_key(subspace_end, path_end, *time_end).await;

        let store = self.rc.read().await;

        let namespace_keyspace = store
            .namespace_keyspace(namespace_id)
            .map_err(|err| StoreOrConsumerError::StoreError(PersistentStoreError::Fjall(err)))?;

        let new_iter = namespace_keyspace.range(range_start_key..=range_end_key);

        for guard in new_iter {
            let (key, value) = guard.into_inner().map_err(|err| {
                StoreOrConsumerError::StoreError(PersistentStoreError::Fjall(err))
            })?;

            let (subspace, path, timestamp) = decode_spt_key(&key).await;
            let (length, digest, token) = decode_entry_values(&value).await;

            // We can unwrap here because we know we filled in all the details.
            let entry = Entry::builder()
                .namespace_id(namespace_id.clone())
                .subspace_id(subspace)
                .path(path)
                .timestamp(timestamp)
                .payload_length(length)
                .payload_digest(digest)
                .build();

            if !range.includes(&entry) {
                continue;
            }

            // We can uwrap this entry because we don't store invalid entries.
            let authed_entry = PossiblyAuthorisedEntry::new(entry, token)
                .into_authorised_entry()
                .unwrap();

            c.consume_item(authed_entry)
                .await
                .map_err(StoreOrConsumerError::ConsumerError)?;
        }

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

        let payload_buf = payload_key(
            store.payload_dir.clone(),
            namespace_id,
            key.subspace_id(),
            key.path(),
        )
        .await;

        let maybe_slice_store =
            SingleSliceStore::<FileBackend>::load(&mut store.bab_key_state, &payload_buf)
                .await
                .map_err(|err| {
                    GetPayloadSliceError::StoreError(PersistentStoreError::Bab(
                        OperationsError::Internal {
                            err,
                            is_fatal: false,
                        },
                    ))
                })?;

        match maybe_slice_store {
            Some(mut store) => {
                if let Some(expected) = expected_digest
                    && expected != PayloadDigest(store.metadata().digest().into())
                {
                    return Err(GetPayloadSliceError::NoSuchEntry);
                }

                store
                    .get_data(c, start, length)
                    .await
                    .map_err(|err| match err {
                        WriteToConsumerError::StorageError(operations_error) => {
                            GetPayloadSliceError::StoreError(PersistentStoreError::Bab(
                                operations_error,
                            ))
                        }
                        WriteToConsumerError::ConsumerError(err) => {
                            GetPayloadSliceError::ConsumerError(err)
                        }
                    })?;

                Ok(length)
            }
            None => Err(GetPayloadSliceError::NoSuchEntry),
        }
    }

    async fn flush(&mut self) -> Result<(), Self::InternalError> {
        let store = self.rc.write().await;

        store.database.persist(PersistMode::SyncAll)?;

        Ok(())
    }
}

impl PayloadPrefixStore for PersistentStore {
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
        let mut rc_store = self.rc.write().await;

        let maybe_slice_store = rc_store
            .load_payload(namespace_id, key)
            .await
            .map_err(AppendToPayloadPrefixError::StoreError)?;

        if let Some(mut store) = maybe_slice_store {
            store
                .append_data(p, stream_options)
                .await
                .map_err(|err| match err {
                    IngestSliceStreamError::ProducerError { producer_err, .. } => {
                        AppendToPayloadPrefixError::ProducerError(producer_err)
                    }
                    IngestSliceStreamError::UnexpectedEndOfStream { .. } => {
                        AppendToPayloadPrefixError::UnexpectedEndOfStream
                    }
                    IngestSliceStreamError::StorageBackendError { storage_error, .. } => {
                        AppendToPayloadPrefixError::StoreError(PersistentStoreError::Bab(
                            storage_error,
                        ))
                    }
                    IngestSliceStreamError::VerificationError { .. } => {
                        AppendToPayloadPrefixError::VerificationError
                    }
                })?;

            store.flush().await.map_err(|err| {
                AppendToPayloadPrefixError::StoreError(PersistentStoreError::Bab(err))
            })?;

            Ok(())
        } else {
            Err(AppendToPayloadPrefixError::NoSuchEntry)
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
        let mut rc_store = self.rc.write().await;

        let maybe_slice_store = rc_store.load_payload(namespace_id, key).await?;

        match maybe_slice_store {
            Some(store) => {
                if let Some(expected) = expected_digest
                    && PayloadDigest(store.metadata().digest().into()) != expected
                {
                    return Err(InternalOrNoSuchEntryError::NoSuchEntry);
                }

                match store.metadata().slice_stream_resumption_info() {
                    None => Ok(store.metadata().string_length()),
                    Some(info) => Ok(info.start_chunk * CHUNK_SIZE as u64),
                }
            }
            None => Err(InternalOrNoSuchEntryError::NoSuchEntry),
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
        let mut rc_store = self.rc.write().await;

        let maybe_slice_store = rc_store.load_payload(namespace_id, key).await?;

        match maybe_slice_store {
            Some(store) => {
                if let Some(expected) = expected_digest
                    && PayloadDigest(store.metadata().digest().into()) != expected
                {
                    return Err(InternalOrNoSuchEntryError::NoSuchEntry);
                }

                Ok(store.metadata().slice_stream_resumption_info())
            }
            None => Err(InternalOrNoSuchEntryError::NoSuchEntry),
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
        let mut rc_store = self.rc.write().await;

        let maybe_slice_store = rc_store
            .load_payload(namespace_id, key)
            .await
            .map_err(GetVerifiableStreamError::StoreError)?;

        match maybe_slice_store {
            Some(mut store) => {
                if let Some(expected) = expected_digest
                    && PayloadDigest(store.metadata().digest().into()) != expected
                {
                    return Err(GetVerifiableStreamError::NoSuchEntry);
                }

                store
                    .get_verifiable_stream(c, start, length, stream_options)
                    .await
                    .map_err(|err| match err {
                        EmitSliceStreamError::ConsumerError(err) => {
                            GetVerifiableStreamError::ConsumerError(err)
                        }
                        EmitSliceStreamError::StorageBackendError(err) => {
                            GetVerifiableStreamError::StoreError(PersistentStoreError::Bab(err))
                        }
                    })
            }
            None => Err(GetVerifiableStreamError::NoSuchEntry),
        }
    }
}

fn namespace_to_base64(namespace: &NamespaceId) -> String {
    BASE64_STANDARD.encode(namespace.as_bytes())
}

async fn payload_key(
    mut root: PathBuf,
    namespace: &NamespaceId,
    subspace: &SubspaceId,
    path: &Path,
) -> PathBuf {
    let namespace_component = base32::encode(
        base32::Alphabet::Rfc4648 { padding: false },
        &namespace.to_bytes(),
    );

    root.push(namespace_component);

    let subspace_component = base32::encode(
        base32::Alphabet::Rfc4648 { padding: false },
        &subspace.to_bytes(),
    );

    root.push(subspace_component);

    let encoded_path_bytes = path.new_vec_storing_encoding().await;

    let path_digest = PayloadDigest::from_payload(encoded_path_bytes);

    let path_component = base32::encode(
        base32::Alphabet::Rfc4648 { padding: false },
        path_digest.as_bytes(),
    );

    root.push(path_component);

    root
}

/// Encodes a key of subspace + path + timestamp (in that order)
async fn encode_spt_key(subspace: &SubspaceId, path: &Path, timestamp: Timestamp) -> Vec<u8> {
    // Encode subspace and path.
    let mut encoding = encode_subspace_path_key(subspace, path, true).await;

    {
        // Append the timestamp.
        let mut consumer = (&mut encoding).into_consumer();
        consumer.encode_u64_be(u64::from(timestamp)).await;
    }

    encoding
}

/// Decodes a key of subspace + path + timestamp (in that order)
async fn decode_spt_key(key: &fjall::Slice) -> (SubspaceId, Path, Timestamp) {
    let mut producer = clone_from_slice(&key[..]);

    let (subspace_id, path) = decode_subspace_path_key(&mut producer).await;
    let timestamp = producer.decode_u64_be().await.unwrap();

    (subspace_id, path, timestamp.into())
}

async fn encode_entry_values(
    payload_length: u64,
    payload_digest: &PayloadDigest,
    auth_token: &AuthorisationToken,
) -> Vec<u8> {
    let mut encoded_bytes_vec = Vec::new();
    {
        let mut consumer = (&mut encoded_bytes_vec).into_consumer();

        compact_u64::cu64_encode_standalone(payload_length, &mut consumer)
            .await
            .unwrap();
        payload_digest.encode(&mut consumer).await.unwrap();
        auth_token.signature().encode(&mut consumer).await.unwrap();
        auth_token.capability().encode(&mut consumer).await.unwrap();
    }

    encoded_bytes_vec
}

async fn decode_entry_values(key: &fjall::Slice) -> (u64, PayloadDigest, AuthorisationToken) {
    let mut producer = clone_from_slice(key);

    let payload_length = compact_u64::cu64_decode_standalone(&mut producer)
        .await
        .unwrap();
    let payload_digest = PayloadDigest::decode(&mut producer).await.unwrap();
    let signature = SubspaceSignature::decode(&mut producer).await.unwrap();
    let write_cap = WriteCapability::decode(&mut producer).await.unwrap();

    let auth_token = AuthorisationToken::new(write_cap, signature);

    (payload_length, payload_digest, auth_token)
}

async fn encode_subspace_path_key(
    subspace_id: &SubspaceId,
    path: &Path,
    with_path_end: bool,
) -> Vec<u8> {
    let mut path_bytes_vec = Vec::<u8>::new();
    {
        let mut path_bytes_consumer = (&mut path_bytes_vec).into_consumer();

        for component in path.components() {
            for byte in component.as_ref() {
                // Encode each component byte, but escape zero bytes as `[0, 2]`
                if *byte == 0 {
                    // Unwrap because IntoVec should not fail.
                    path_bytes_consumer
                        .bulk_consume_full_slice(&[0, 2])
                        .await
                        .unwrap();
                } else {
                    // Unwrap because IntoVec should not fail.
                    path_bytes_consumer
                        .consume(Either::Left(*byte))
                        .await
                        .unwrap();
                }
            }

            // Terminate every component with `[0, 1]`.
            // Unwrap because IntoVec should not fail.
            path_bytes_consumer
                .bulk_consume_full_slice(&[0, 1])
                .await
                .unwrap();
        }

        // Unwrap because IntoVec should not fail.
        if with_path_end {
            // Terminate the path with `[0, 0]`.
            path_bytes_consumer
                .bulk_consume_full_slice(&[0, 0])
                .await
                .unwrap();
        }
    }

    // No timestamp here!

    let mut vec = Vec::from(subspace_id.to_bytes());
    vec.append(&mut path_bytes_vec);
    vec
}

async fn decode_subspace_path_key<P>(producer: &mut P) -> (SubspaceId, Path)
where
    P: BulkProducer<Item = u8>,
    P::Error: fmt::Debug,
    P::Final: fmt::Debug,
{
    let subspace = SubspaceId::decode(producer).await.unwrap();

    let mut components_vecs: Vec<Vec<u8>> = Vec::new();

    while let Some(bytes) = component_bytes(producer).await {
        components_vecs.push(bytes);
    }

    let mut components = components_vecs
        .iter()
        .map(|bytes| Component::new(bytes).expect("Component was unexpectedly longer than MCL."));

    let total_len = components.clone().fold(0, |acc, comp| acc + comp.len());

    let path: Path = Path::from_components_iter(total_len, &mut components).unwrap();

    (subspace, path)
}

async fn component_bytes<P: Producer<Item = u8>>(producer: &mut P) -> Option<Vec<u8>>
where
    P::Error: core::fmt::Debug,
    P::Final: core::fmt::Debug,
{
    let mut vec: Vec<u8> = Vec::new();
    let mut previous_was_zero = false;

    loop {
        match producer.produce().await {
            Ok(Either::Left(byte)) => {
                if !previous_was_zero && byte == 0 {
                    previous_was_zero = true
                } else if previous_was_zero && byte == 2 {
                    // Append a zero.

                    vec.push(0);
                    previous_was_zero = false;
                } else if previous_was_zero && byte == 1 {
                    // That's the end of this component.
                    return Some(vec);
                } else if previous_was_zero && byte == 0 {
                    // That's the end of the path.
                    return None;
                } else {
                    // Append to the component.
                    vec.push(byte);
                    previous_was_zero = false;
                }
            }
            Ok(Either::Right(_)) => {
                if previous_was_zero {
                    panic!("Unterminated escaped key!")
                }

                return None;
            }
            Err(err) => panic!("Unexpected error: {err:?}"),
        }
    }
}

#[cfg(test)]
mod test {
    use std::fs::remove_dir_all;

    use ed25519_dalek::SigningKey;

    use ufotofu::producer::clone_from_slice;

    use crate::defaults::{
        DEFAULT_PAYLOAD_LENGTH, default_authorisation_token, default_authorised_entry,
        default_entry, default_namespace_id, default_payload_digest, default_subspace_id,
        default_subspace_secret,
    };

    use super::*;

    use rand::rngs::OsRng;

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

    async fn write_entry(
        store: &mut PersistentStore,
        cap: &WriteCapability,
        secret: &SubspaceSecret,
    ) {
        let mut payload = String::from("Hello").into_bytes().into_producer();
        store
            .create_entry(
                cap.granted_namespace(),
                cap.receiver(),
                &Path::new(),
                Timestamp::from(0),
                &mut payload,
                5,
                cap,
                secret,
            )
            .await
            .unwrap();
    }

    #[test]
    fn persistent_store_subspaces() {
        pollster::block_on(async {
            let mut csprng = OsRng;

            let fspath = "./testsubspaces_store/";
            let _ = remove_dir_all(fspath);

            let mut store = PersistentStore::new(fspath).await.unwrap();

            let namespace_id = NamespaceId::from_bytes(&FAMILY_PK);
            let namespace_secret = NamespaceSecret::from(SigningKey::from_bytes(&FAMILY_SK));

            let (alfie_id, alfie_secret) = randomly_generate_subspace(&mut csprng);
            let (betty_id, betty_secret) = randomly_generate_subspace(&mut csprng);
            let (gemma_id, gemma_secret) = randomly_generate_subspace(&mut csprng);
            let (dalton_id, dalton_secret) = randomly_generate_subspace(&mut csprng);

            let (other_namespace_id, other_namespace_secret) =
                randomly_generate_namespace(&mut csprng);

            let alfie_cap = write_cap(&namespace_id, &namespace_secret, &alfie_id);
            let betty_cap = write_cap(&namespace_id, &namespace_secret, &betty_id);
            let gemma_cap = write_cap(&other_namespace_id, &other_namespace_secret, &gemma_id);
            let dalton_cap = write_cap(&namespace_id, &namespace_secret, &dalton_id);

            write_entry(&mut store, &alfie_cap, &alfie_secret).await;
            write_entry(&mut store, &betty_cap, &betty_secret).await;
            write_entry(&mut store, &gemma_cap, &gemma_secret).await;
            write_entry(&mut store, &dalton_cap, &dalton_secret).await;

            let mut subspace_consumer = vec![].into_consumer();

            store
                .subspaces(&namespace_id, &mut subspace_consumer)
                .await
                .unwrap();

            let subspaces: Vec<SubspaceId> = subspace_consumer.into();

            assert_eq!(subspaces.len(), 3);

            let has_alfie = subspaces
                .iter()
                .find(|subspace| *subspace == &alfie_id)
                .is_some();
            assert!(has_alfie);

            let has_betty = subspaces
                .iter()
                .find(|subspace| *subspace == &betty_id)
                .is_some();
            assert!(has_betty);

            let has_gemma = subspaces
                .iter()
                .find(|subspace| *subspace == &gemma_id)
                .is_some();
            assert!(!has_gemma);

            let has_dalton = subspaces
                .iter()
                .find(|subspace| *subspace == &dalton_id)
                .is_some();
            assert!(has_dalton);

            let _ = remove_dir_all(fspath);
        });
    }

    #[test]
    fn regression_test_000() {
        pollster::block_on(async {
            let fspath = "./test000_store/";
            let _ = remove_dir_all(fspath);

            let mut store = PersistentStore::new(fspath).await.unwrap();

            let namespace_id = NamespaceId::from_bytes(&FAMILY_PK);
            let namespace_secret = NamespaceSecret::from(SigningKey::from_bytes(&FAMILY_SK));

            let subspace_id = SubspaceId::from_bytes(&ALFIE_PK);
            let subspace_secret = SubspaceSecret::from(SigningKey::from_bytes(&ALFIE_SK));

            let write_capability = write_cap(&namespace_id, &namespace_secret, &subspace_id);

            let path = path!("");
            let timestamp = 0;

            let payload = &[];
            let mut p = clone_from_slice(payload);

            let _nd_insert = store
                .create_entry_nondestructive(
                    &namespace_id,
                    &subspace_id,
                    &path,
                    timestamp.into(),
                    &mut p,
                    payload.len() as u64,
                    &write_capability,
                    &subspace_secret,
                )
                .await
                .unwrap();

            let len_of_payload_prefix = store
                .length_of_payload_prefix(&namespace_id, &(subspace_id.clone(), path.clone()), None)
                .await;
            assert!(len_of_payload_prefix.is_ok());

            let _ = remove_dir_all(fspath);
        });
    }

    #[test]
    fn regression_test_001() {
        // Tests whether basic prefix pruning via the empty path works.
        pollster::block_on(async {
            let fspath = "./test001_store/";
            let _ = remove_dir_all(fspath);

            let mut store = PersistentStore::new(fspath).await.unwrap();

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

            // Add a newer entry whose path is the empty path.
            let path2 = Path::new();
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

            // The first entry is no longer in the store.
            let len_of_payload_prefix2 = store
                .length_of_payload_prefix(
                    &namespace_id,
                    &(subspace_id.clone(), path1.clone()),
                    None,
                )
                .await;
            assert!(
                matches!(
                    len_of_payload_prefix2,
                    Err(InternalOrNoSuchEntryError::NoSuchEntry),
                ),
                "got the following value: {len_of_payload_prefix2:?}",
            );

            let _ = remove_dir_all(fspath);
        });
    }

    #[test]
    fn regression_test_002() {
        let fspath = "./test002_store/";
        // Tests whether basic prefix pruning via a non-empty prefix works.
        pollster::block_on(async {
            let _ = remove_dir_all(fspath);

            let mut store = PersistentStore::new(fspath).await.unwrap();

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

            // The first entry is no longer in the store.
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

            let _ = remove_dir_all(fspath);
        });
    }

    #[test]
    fn regression_test_003() {
        let fspath = "./test003_store/";
        //Write to empty path, then again with a higher timestamp.
        pollster::block_on(async {
            let _ = remove_dir_all(fspath);

            let mut store = PersistentStore::new(fspath).await.unwrap();

            let namespace_id = NamespaceId::from_bytes(&FAMILY_PK);
            let namespace_secret = NamespaceSecret::from(SigningKey::from_bytes(&FAMILY_SK));

            let subspace_id = SubspaceId::from_bytes(&ALFIE_PK);
            let subspace_secret = SubspaceSecret::from(SigningKey::from_bytes(&ALFIE_SK));

            let write_capability = write_cap(&namespace_id, &namespace_secret, &subspace_id);

            let path1 = Path::new();
            let timestamp = 0;

            let payload = &[];
            let mut p = clone_from_slice(payload);

            // Add an entry at the empty path, to be overwritten later.
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

            // Add a newer entry, again at the empty path.
            let path2 = Path::new();
            let payload2 = &[0, 1, 2];
            let mut p2 = clone_from_slice(payload2);
            store
                .create_entry(
                    &namespace_id,
                    &subspace_id,
                    &path2,
                    (timestamp + 1).into(),
                    &mut p2,
                    payload2.len() as u64,
                    &write_capability,
                    &subspace_secret,
                )
                .await
                .unwrap();

            // The payload got updated.
            let len_of_payload_prefix2 = store
                .length_of_payload_prefix(
                    &namespace_id,
                    &(subspace_id.clone(), path1.clone()),
                    None,
                )
                .await;
            assert_eq!(len_of_payload_prefix2.unwrap(), 3);

            let _ = remove_dir_all(fspath);
        });
    }

    #[test]
    fn regression_test_004() {
        let fspath = "./test004_store/";
        //Write to non-empty path, then write to the same path at higher timestamp, finally write to the empty path at even higher timestamp.
        pollster::block_on(async {
            let _ = remove_dir_all(fspath);

            let mut store = PersistentStore::new(fspath).await.unwrap();

            let namespace_id = NamespaceId::from_bytes(&FAMILY_PK);
            let namespace_secret = NamespaceSecret::from(SigningKey::from_bytes(&FAMILY_SK));

            let subspace_id = SubspaceId::from_bytes(&ALFIE_PK);
            let subspace_secret = SubspaceSecret::from(SigningKey::from_bytes(&ALFIE_SK));

            let write_capability = write_cap(&namespace_id, &namespace_secret, &subspace_id);

            let path1 = path!("/a");
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

            // Add a newer entry at the same path.
            store
                .create_entry(
                    &namespace_id,
                    &subspace_id,
                    &path1,
                    (timestamp + 1).into(),
                    &mut p,
                    payload.len() as u64,
                    &write_capability,
                    &subspace_secret,
                )
                .await
                .unwrap();

            // The second entry is now in the store.
            let len_of_payload_prefix2 = store
                .length_of_payload_prefix(
                    &namespace_id,
                    &(subspace_id.clone(), path1.clone()),
                    None,
                )
                .await;
            assert!(len_of_payload_prefix2.is_ok());

            // Add an even newer entry at the empty path.
            let path3 = Path::new();
            store
                .create_entry(
                    &namespace_id,
                    &subspace_id,
                    &path3,
                    (timestamp + 2).into(),
                    &mut p,
                    payload.len() as u64,
                    &write_capability,
                    &subspace_secret,
                )
                .await
                .unwrap();

            // The second entry is no longer in the store.
            let len_of_payload_prefix3 = store
                .length_of_payload_prefix(
                    &namespace_id,
                    &(subspace_id.clone(), path1.clone()),
                    None,
                )
                .await;
            assert!(matches!(
                len_of_payload_prefix3,
                Err(InternalOrNoSuchEntryError::NoSuchEntry)
            ));

            let _ = remove_dir_all(fspath);
        });
    }

    #[test]
    fn regression_test_005() {
        let fspath = "./test005_store/";
        // Write to empty path at high timestamp, then again with a lower timestamp. Ensure that the first payload stays unaffected.
        pollster::block_on(async {
            let _ = remove_dir_all(fspath);

            let mut store = PersistentStore::new(fspath).await.unwrap();

            let namespace_id = NamespaceId::from_bytes(&FAMILY_PK);
            let namespace_secret = NamespaceSecret::from(SigningKey::from_bytes(&FAMILY_SK));

            let subspace_id = SubspaceId::from_bytes(&ALFIE_PK);
            let subspace_secret = SubspaceSecret::from(SigningKey::from_bytes(&ALFIE_SK));

            let write_capability = write_cap(&namespace_id, &namespace_secret, &subspace_id);

            let path1 = Path::new();
            let timestamp = 0;

            let payload = &[0, 1, 2];
            let mut p = clone_from_slice(payload);

            // Add an entry at the empty path, to be overwritten later.
            store
                .create_entry(
                    &namespace_id,
                    &subspace_id,
                    &path1,
                    (timestamp + 1).into(),
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
            assert_eq!(len_of_payload_prefix.unwrap(), 3);

            // Add an older entry, again at the empty path.
            let path2 = Path::new();
            let payload2 = &[0];
            let mut p2 = clone_from_slice(payload);
            store
                .create_entry(
                    &namespace_id,
                    &subspace_id,
                    &path2,
                    timestamp.into(),
                    &mut p2,
                    payload2.len() as u64,
                    &write_capability,
                    &subspace_secret,
                )
                .await
                .unwrap();

            // The first entry is in the store, and its payload still has length three.
            let len_of_payload_prefix = store
                .length_of_payload_prefix(
                    &namespace_id,
                    &(subspace_id.clone(), path1.clone()),
                    None,
                )
                .await;
            assert_eq!(len_of_payload_prefix.unwrap(), 3);

            let _ = remove_dir_all(fspath);
        });
    }

    #[test]
    fn regression_test_006() {
        let fspath = "./test006_store/";
        // Write to empty path at high timestamp, then again with a lower timestamp. Ensure that the first payload stays unaffected.
        pollster::block_on(async {
            let _ = remove_dir_all(fspath);

            let mut store = PersistentStore::new(fspath).await.unwrap();
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

            let _ = remove_dir_all(fspath);
        });
    }
}
