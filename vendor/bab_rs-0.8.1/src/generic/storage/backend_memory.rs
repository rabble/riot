//! An in-memory [`StorageBackend`].
//!
//! See the [`storage`](super) module docs for context.

use core::fmt;

use std::{
    collections::HashMap,
    convert::Infallible,
    hash::Hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use frugal_async::RwLock;

use crate::generic::{
    storage::storage_backend::{OperationsError, StorageBackend},
    storage::units::*,
};

/// A [`StorageBackend`] that stores data in memory, without persisting it to disk.
///
/// Supports [keys](StorageBackend::Key) of an arbitrary type `K`, as long as they implement [`Hash`] and [`Eq`].
pub struct InMemoryBackend<K> {
    // `None` after the store has been deleted.
    rc: Rc<RwLock<Option<BackendState>>>,
    phantom: PhantomData<K>,
}

impl<K> fmt::Debug for InMemoryBackend<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("InMemoryBackend")
            .field(&self.rc.deref())
            .finish()
    }
}

impl<K> Clone for InMemoryBackend<K> {
    fn clone(&self) -> Self {
        Self {
            rc: self.rc.clone(),
            phantom: self.phantom.clone(),
        }
    }
}

impl<K> StorageBackend for InMemoryBackend<K>
where
    K: Hash + Eq,
{
    type InternalError = Infallible;

    type Key = K;

    type KeyState = KeyState<K>;

    async fn create(
        key_state: &mut Self::KeyState,
        key: Self::Key,
        capacity: ByteCount,
        metadata_len: ByteCount,
    ) -> Result<Self, Self::InternalError> {
        let storage = InMemoryBackend {
            rc: Rc::new(RwLock::new(Some(BackendState::new(capacity, metadata_len)))),
            phantom: PhantomData,
        };

        key_state.stores.insert(key, storage.clone());

        Ok(storage)
    }

    async fn load(
        key_state: &mut Self::KeyState,
        key: &Self::Key,
    ) -> Result<Option<Self>, Self::InternalError> {
        match key_state.stores.get(key) {
            Some(store) => Ok(Some(store.clone())),
            None => Ok(None),
        }
    }

    async fn delete(
        key_state: &mut Self::KeyState,
        key: &Self::Key,
    ) -> Result<(), Self::InternalError> {
        match key_state.stores.get_mut(key) {
            Some(store) => {
                let _ = store.rc.write().await.take();
                Ok(())
            }
            None => Ok(()),
        }
    }

    async fn rename(
        key_state: &mut Self::KeyState,
        old_key: &Self::Key,
        new_key: Self::Key,
    ) -> Result<(), Self::InternalError> {
        match key_state.stores.remove(old_key) {
            None => Ok(()),
            Some(store) => {
                if store.rc.read().await.is_none() {
                    Ok(())
                } else {
                    key_state.stores.insert(new_key, store);
                    Ok(())
                }
            }
        }
    }

    async fn get_capacity(&mut self) -> Result<ByteCount, OperationsError<Self::InternalError>> {
        match self.rc.read().await.deref() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => Ok(state.data.len() as ByteCount),
        }
    }

    async fn get_bytes(
        &mut self,
        offset: ByteIndex,
        buf: &mut [u8],
    ) -> Result<(), OperationsError<Self::InternalError>> {
        match self.rc.read().await.deref() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => {
                let len = buf.len();
                let offset = offset as usize;

                buf.copy_from_slice(&state.data[offset..offset + len]);

                Ok(())
            }
        }
    }

    async fn set_bytes(
        &mut self,
        offset: ByteIndex,
        new_data: &[u8],
    ) -> Result<(), OperationsError<Self::InternalError>> {
        match self.rc.write().await.deref_mut() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => {
                let len = new_data.len();
                let offset = offset as usize;

                (&mut state.data[offset..offset + len]).copy_from_slice(new_data);

                Ok(())
            }
        }
    }

    async fn get_len_of_metadata(
        &mut self,
    ) -> Result<ByteCount, OperationsError<Self::InternalError>> {
        match self.rc.read().await.deref() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => Ok(state.metadata.len() as ByteCount),
        }
    }

    async fn get_metadata(
        &mut self,
        offset: ByteIndex,
        buf: &mut [u8],
    ) -> Result<(), OperationsError<Self::InternalError>> {
        match self.rc.read().await.deref() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => {
                let len = buf.len();
                let offset = offset as usize;

                buf.copy_from_slice(&state.metadata[offset..offset + len]);

                Ok(())
            }
        }
    }

    async fn set_metadata(
        &mut self,
        offset: ByteIndex,
        new_data: &[u8],
    ) -> Result<(), OperationsError<Self::InternalError>> {
        match self.rc.write().await.deref_mut() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => {
                let len = new_data.len();
                let offset = offset as usize;

                (&mut state.metadata[offset..offset + len]).copy_from_slice(new_data);

                Ok(())
            }
        }
    }

    async fn flush(&mut self) -> Result<(), OperationsError<Self::InternalError>> {
        // no-op
        Ok(())
    }

    // async fn reserve_capacity(
    //     &mut self,
    //     additional_capacity: ByteCount,
    // ) -> Result<(), Option<OperationsError<Self::InternalError>>> {
    //     let proposed_capacity = self.capacity.checked_add(additional_capacity).unwrap();

    //     self.file
    //         .set_len(proposed_capacity)
    //         .await
    //         .map_err(|err| OperationsError::Internal {
    //             err,
    //             is_fatal: false,
    //         })?;

    //     self.capacity = proposed_capacity;
    //     Ok(())
    // }
}

#[derive(Debug)]
struct BackendState {
    metadata: Vec<u8>,
    data: Vec<u8>,
}

impl BackendState {
    fn new(capacity: ByteCount, metadata_len: ByteCount) -> Self {
        Self {
            metadata: vec![0; metadata_len as usize],
            data: vec![0; capacity as usize],
        }
    }
}

/// The persistent state that allows for [loading](StorageBackend::load) [`InMemoryBackends`](InMemoryBackend) by key.
#[derive(Debug)]
pub struct KeyState<K> {
    stores: HashMap<K, InMemoryBackend<K>>,
}

impl<K> KeyState<K> {
    /// Creates a new [`KeyState`], initially not knowing about any backends.
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
        }
    }
}
