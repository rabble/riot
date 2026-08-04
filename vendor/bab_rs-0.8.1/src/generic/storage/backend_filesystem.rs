//! A file-system-based persistent [`StorageBackend`].
//!
//! See the [`storage`](super) module docs for context.

// Stores bytes in files, using their paths as the keys. Each file consists of eight big-endian bytes encoding the length of the metadata, followed by the metadata, followed by the actual payload data.

use core::fmt;

use std::collections::HashMap;
use std::io::ErrorKind::NotFound;
use std::io::{self, ErrorKind, SeekFrom};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use async_fs::{File, OpenOptions};
use futures_lite::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use frugal_async::RwLock;

use crate::generic::{
    storage::storage_backend::{OperationsError, StorageBackend},
    storage::units::*,
};

/// A [`StorageBackend`] that stores data on the file system.
#[derive(Clone)]
pub struct FileBackend {
    // `None` after the store has been deleted.
    rc: Rc<RwLock<Option<BackendState>>>,
}

impl fmt::Debug for FileBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FileBackend")
            .field(&self.rc.deref())
            .finish()
    }
}

impl StorageBackend for FileBackend {
    type InternalError = io::Error;

    type Key = PathBuf;

    type KeyState = KeyState;

    async fn create(
        key_state: &mut Self::KeyState,
        key: Self::Key,
        capacity: ByteCount,
        metadata_len: ByteCount,
    ) -> Result<Self, Self::InternalError> {
        let storage = FileBackend {
            rc: Rc::new(RwLock::new(Some(
                BackendState::new(&key, metadata_len, capacity).await?,
            ))),
        };

        key_state.stores.insert(key, Rc::downgrade(&storage.rc));

        Ok(storage)
    }

    async fn load(
        key_state: &mut Self::KeyState,
        key: &Self::Key,
    ) -> Result<Option<Self>, Self::InternalError> {
        if let Some(store) = key_state.stores.get(key) {
            if let Some(rc) = store.upgrade() {
                return Ok(Some(Self { rc }));
            }
        }

        // The following is executed either if we stored no value for this key, or if we stored an empty Weak.
        match OpenOptions::new().read(true).write(true).open(key).await {
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(None),
                _ => Err(err),
            },
            Ok(mut file) => {
                file.seek(SeekFrom::Start(0)).await?;

                let mut big_endian_metadata_len = [0; 8];

                file.read_exact(&mut big_endian_metadata_len).await?;

                let metadata_len = u64::from_be_bytes(big_endian_metadata_len);

                let total_len = file.metadata().await?.len();

                let storage = FileBackend {
                    rc: Rc::new(RwLock::new(Some(BackendState {
                        file,
                        metadata_len,
                        data_len: total_len
                            .checked_sub(metadata_len)
                            .unwrap()
                            .checked_sub(8)
                            .unwrap(),
                    }))),
                };

                key_state
                    .stores
                    .insert(key.to_owned(), Rc::downgrade(&storage.rc));

                Ok(Some(storage))
            }
        }
    }

    async fn delete(
        key_state: &mut Self::KeyState,
        key: &Self::Key,
    ) -> Result<(), Self::InternalError> {
        if let Some(store) = key_state.stores.get_mut(key) {
            if let Some(rc) = store.upgrade() {
                // Set the Option to None, signalling to other references to the storage that it has been deleted.
                rc.write().await.take();
            }
        }

        // And delete the file on disk in every case.
        // It's okay if removing the file fails because the file is already gone!
        if let Err(err) = async_fs::remove_file(key).await
            && err.kind() != NotFound
        {
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn rename(
        key_state: &mut Self::KeyState,
        old_key: &Self::Key,
        new_key: Self::Key,
    ) -> Result<(), Self::InternalError> {
        match key_state.stores.remove(old_key) {
            None => Ok(()),
            Some(store) => match store.upgrade() {
                None => Ok(()),
                Some(rc) => {
                    if rc.read().await.is_none() {
                        Ok(())
                    } else {
                        // Create parent dirs for the destination if necessary
                        match new_key.parent() {
                            None => { /* no need to create anything */ }
                            Some(parent_path) => async_fs::create_dir_all(parent_path).await?,
                        }

                        async_fs::rename(old_key, &new_key).await?;
                        key_state.stores.insert(new_key, store);
                        Ok(())
                    }
                }
            },
        }
    }

    async fn get_capacity(&mut self) -> Result<ByteCount, OperationsError<Self::InternalError>> {
        match self.rc.read().await.deref() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => Ok(state.data_len),
        }
    }

    async fn get_bytes(
        &mut self,
        offset: ByteIndex,
        buf: &mut [u8],
    ) -> Result<(), OperationsError<Self::InternalError>> {
        match self.rc.write().await.deref_mut() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => {
                state
                    .file
                    .seek(SeekFrom::Start(8 + state.metadata_len + offset))
                    .await
                    .map_err(|err| OperationsError::Internal {
                        err,
                        is_fatal: true,
                    })?;

                state
                    .file
                    .read_exact(buf)
                    .await
                    .map_err(|err| OperationsError::Internal {
                        err,
                        is_fatal: true,
                    })?;

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
                state
                    .file
                    .seek(SeekFrom::Start(8 + state.metadata_len + offset))
                    .await
                    .map_err(|err| OperationsError::Internal {
                        err,
                        is_fatal: true,
                    })?;

                state
                    .file
                    .write_all(new_data)
                    .await
                    .map_err(|err| OperationsError::Internal {
                        err,
                        is_fatal: true,
                    })?;

                Ok(())
            }
        }
    }

    async fn get_len_of_metadata(
        &mut self,
    ) -> Result<ByteCount, OperationsError<Self::InternalError>> {
        match self.rc.read().await.deref() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => Ok(state.metadata_len),
        }
    }

    async fn get_metadata(
        &mut self,
        offset: ByteIndex,
        buf: &mut [u8],
    ) -> Result<(), OperationsError<Self::InternalError>> {
        match self.rc.write().await.deref_mut() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => {
                state
                    .file
                    .seek(SeekFrom::Start(8 + offset))
                    .await
                    .map_err(|err| OperationsError::Internal {
                        err,
                        is_fatal: true,
                    })?;

                state
                    .file
                    .read_exact(buf)
                    .await
                    .map_err(|err| OperationsError::Internal {
                        err,
                        is_fatal: true,
                    })?;

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
                state
                    .file
                    .seek(SeekFrom::Start(8 + offset))
                    .await
                    .map_err(|err| OperationsError::Internal {
                        err,
                        is_fatal: true,
                    })?;

                state
                    .file
                    .write_all(new_data)
                    .await
                    .map_err(|err| OperationsError::Internal {
                        err,
                        is_fatal: true,
                    })?;

                Ok(())
            }
        }
    }

    async fn flush(&mut self) -> Result<(), OperationsError<Self::InternalError>> {
        match self.rc.write().await.deref_mut() {
            None => Err(OperationsError::StorageDeleted),
            Some(state) => state
                .file
                .sync_all()
                .await
                .map_err(|err| OperationsError::Internal {
                    err,
                    is_fatal: true,
                }),
        }
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
    file: File,
    metadata_len: ByteCount,
    data_len: ByteCount,
}

impl BackendState {
    pub async fn new<P: AsRef<Path>>(
        path: P,
        metadata_len: ByteCount,
        data_len: ByteCount,
    ) -> Result<Self, std::io::Error> {
        match path.as_ref().parent() {
            None => { /* no need to create anything */ }
            Some(parent_path) => async_fs::create_dir_all(parent_path).await?,
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .await?;

        file.set_len(
            metadata_len
                .checked_add(data_len)
                .expect("total size of metadata and data must not overflow a u64")
                .checked_add(8)
                .expect("total size of metadata and data plus 8 must not overflow a u64"),
        )
        .await?;

        file.write_all(metadata_len.to_be_bytes().as_ref()).await?;

        Ok(Self {
            file,
            metadata_len,
            data_len,
        })
    }
}

/// The persistent state that allows for [loading](StorageBackend::load) [`FileBackends`](FileBackend) by key.
///
/// Loading also looks things up on the file system, but the `KeyState` allows us to speed things up and to share allocations between multiple [`FileBackends`](FileBackend) referring to the same data.
#[derive(Debug)]
pub struct KeyState {
    stores: HashMap<PathBuf, Weak<RwLock<Option<BackendState>>>>,
}

impl KeyState {
    /// Creates a new [`KeyState`], initially not knowing about any backends.
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
        }
    }
}

#[cfg(all(test, feature = "storage", feature = "william3"))]
mod test {
    use tempfile::NamedTempFile;

    use ufotofu::producer::clone_from_slice;

    use crate::storage::SingleSliceStore;

    use super::*;

    #[test]
    fn test00() {
        pollster::block_on(async {
            let tmp_file = NamedTempFile::new().unwrap();

            {
                let mut state = KeyState::new();

                let mut backend1 =
                    FileBackend::create(&mut state, tmp_file.path().to_owned(), 32, 16)
                        .await
                        .unwrap();

                backend1.set_metadata(5, &[42, 13, 42]).await.unwrap();
                backend1.flush().await.unwrap();
                let mut buf1 = [0; 3];
                backend1.get_metadata(5, &mut buf1[..]).await.unwrap();
                assert_eq!(buf1, [42, 13, 42]);

                backend1.set_bytes(5, &[17, 18, 17]).await.unwrap();
                backend1.flush().await.unwrap();
                let mut buf2 = [0; 3];
                backend1.get_bytes(5, &mut buf2[..]).await.unwrap();
                assert_eq!(buf2, [17, 18, 17]);

                // Now we create a new backend from the same state and check that the same data is retrieved.
                // This will use the same file handle as `backend1` internally, but it *will* read from that actual file, not any buffered data.
                let mut backend2 = FileBackend::load(&mut state, &tmp_file.path().to_owned())
                    .await
                    .unwrap()
                    .unwrap();

                let mut buf1 = [0; 3];
                backend2.get_metadata(5, &mut buf1[..]).await.unwrap();
                assert_eq!(buf1, [42, 13, 42]);

                let mut buf2 = [0; 3];
                backend2.get_bytes(5, &mut buf2[..]).await.unwrap();
                assert_eq!(buf2, [17, 18, 17]);

                // state is dropped here, so when we create a new state later, it will truly read data from the file system
            }

            {
                let mut state = KeyState::new();

                let mut backend3 = FileBackend::load(&mut state, &tmp_file.path().to_owned())
                    .await
                    .unwrap()
                    .unwrap();

                let mut buf1 = [0; 3];
                backend3.get_metadata(5, &mut buf1[..]).await.unwrap();
                assert_eq!(buf1, [42, 13, 42]);

                let mut buf2 = [0; 3];
                backend3.get_bytes(5, &mut buf2[..]).await.unwrap();
                assert_eq!(buf2, [17, 18, 17]);
            }
        });
    }

    #[test]
    fn test01() {
        pollster::block_on(async {
            let tmp_file = NamedTempFile::new().unwrap();
            let key = tmp_file.path().to_owned();

            {
                let mut state = KeyState::new();
                let data = &[];

                let mut p = clone_from_slice(data);

                {
                    let _ = SingleSliceStore::<FileBackend>::create_and_initialise(
                        &mut state,
                        key.clone(),
                        data.len() as u64,
                        &mut p,
                    )
                    .await
                    .unwrap();
                }

                let store2 = SingleSliceStore::<FileBackend>::load(&mut state, &key).await;

                assert!(store2.is_ok());
            }
        });
    }
}
