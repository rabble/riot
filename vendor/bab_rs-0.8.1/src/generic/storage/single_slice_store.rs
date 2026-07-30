//! Storage for a (prefix of a) single contiguous subslice of a string.
//!
//! See the [`SingleSliceStore`] type for more precise documentation.

use core::cmp::min;
use core::fmt;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ufotofu::prelude::*;

use crate::generic::{
    BabDigest, BabInstantiation,
    storage::{
        storage_backend::{
            OperationsError, StorageBackend, StringInfo, WriteToConsumerError,
            approximate_length_of_verifiable_stream,
        },
        units::*,
        verifiable_streaming::{
            self, EmitSliceStreamError, IngestSliceStreamError, SliceStreamingOptions,
            produce_slice_stream,
        },
    },
};

// In order of layout in the underlying ByteStorage.
/// Information about the (subslice of a) string stored in a [`SingleSliceStore`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Metadata<const WIDTH: usize> {
    // Immutable.
    root_hash: [u8; WIDTH],
    // Immutable.
    string_length: ByteCount,
    // Immutable.
    slice_start: ChunkIndex,
    // Mutable, but only rarely, ideally.
    slice_length: ChunkCount,
    /// The first NodeNumber that is part of the slice but has not been verified yet. None if no such number exists (i.e., if the *full* slice was verified).
    // Mutable
    verified_progress: Option<NodeNumber>,
    // Immutable, and not even persisted.
    chunk_count: ChunkCount,
}

impl<const WIDTH: usize> Metadata<WIDTH> {
    /// Returns the digest of the full string whose slice is being stored.
    ///
    /// This value depends neither on the specific slice, nor on the prefix of slice data that is actually available.
    ///
    /// Runs in constant time.
    pub fn digest(&self) -> BabDigest<WIDTH> {
        self.root_hash.into()
    }

    /// Returns the length of the full string whose slice is being stored.
    ///
    /// This value depends neither on the specific slice, nor on the prefix of slice data that is actually available.
    ///
    /// Runs in constant time.
    pub fn string_length(&self) -> ByteCount {
        self.string_length
    }

    /// Returns the start chunk index of the slice to store.
    ///
    /// This value does not depend on the prefix of slice data that is actually available, it gives the *intent* of which data the storage was initialised to eventually store.
    ///
    /// Runs in constant time.
    pub fn slice_start(&self) -> ChunkIndex {
        self.slice_start
    }

    /// Returns the number of chunks in the slice to store.
    ///
    /// This value does not depend on the prefix of slice data that is actually available, it gives the *intent* of which data the storage was initialised to eventually store.
    ///
    /// Runs in constant time.
    pub fn slice_length(&self) -> ChunkCount {
        self.slice_length
    }

    /// Returns the number of chunks which have already been ingested.
    ///
    /// The ingested data always forms a prefix of the slice indicated by [`slice_start`](Metadata::slice_start) and [`slice_length`](Metadata::slice_length).
    pub fn ingested_chunks(&self) -> ChunkCount {
        match self.slice_stream_resumption_info() {
            None => self.slice_length(),
            Some(resumption_info) => resumption_info.start_chunk - self.slice_start(),
        }
    }

    /// Returns information for creating (or requesting) a suitable slice stream to fill up the slice, or `None` if the slice is already fully available.
    ///
    /// This is the only method that supplies information about how much data of the slice to store has actually been ingested already.
    ///
    /// Runs in constant time.
    pub fn slice_stream_resumption_info(&self) -> Option<SliceStreamResumptionInfo> {
        self.verified_progress.map(|verified_progress| {
            let start_chunk = core::cmp::max(
                node_number_to_chunk_index(verified_progress, self.chunk_count),
                self.slice_start,
            );
            let left_skip = node_number_to_left_skip(verified_progress, self.chunk_count);
            let right_skip = node_number_to_right_skip(
                verified_progress,
                self.slice_start + (self.slice_length - 1),
                self.chunk_count,
            );

            SliceStreamResumptionInfo {
                start_chunk,
                left_skip,
                right_skip,
            }
        })
    }

    fn verified_progress_to_bytes(&self) -> [u8; 9] {
        match self.verified_progress {
            None => [0; 9],
            Some(node_number) => {
                let mut ret = [255; 9];
                (&mut ret[1..]).copy_from_slice(node_number.to_be_bytes().as_ref());
                ret
            }
        }
    }
}

/// Information about the kind of verifiable stream you need in order to append data to the stored slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub struct SliceStreamResumptionInfo {
    /// The index of the first chunk to ask for.
    pub start_chunk: ChunkIndex,
    /// The largest [`left_skip`](https://bab-hash.org/spec#left_skip) that you can request while still getting all required data.
    pub left_skip: u8,
    /// The largest [`right_skip`](https://bab-hash.org/spec#right_skip) that you can request while still getting all required data.
    pub right_skip: u8,
}

/// Storage for a single non-empty subslice of a Bab string, in some given [`StorageBackend`].
///
/// Do not confuse the three different levels of metadata when dealing with a `SingleSliceStore`:
///
/// - information about the string of which a subslice is being stored (supplied at creation time),
/// - information about that subslice to store (supplied at creation time), and
/// - those parts of the subslice that have actually been ingested yet (updated as more data is stored; this is always a contiguous prefix of the total subslice).
///
/// A `SingleSliceStore` is [created](SingleSliceStore::create) to store a (prefix of) a particular slice of a string of known length and digest. Initially, the stored prefix is empty. The [`append_data`](SingleSliceStore::append_data) method accepts a verifiable slice stream, verifies it, and then uses it to append to the stored prefix. Alternatively, if the full string is already known (i.e., you are not receiving data from a peer, but want to store a string you yourself created), you can use the [`create_and_initialise`](SingleSliceStore::create_and_initialise) method to store the full string and return the Bab digest of that string.
///
/// Note that [`append_data`](SingleSliceStore::append_data) and [`create_and_initialise`](SingleSliceStore::create_and_initialise) do not [flush](StorageBackend::flush) the storage backend, you need to do so manually via [`SingleSliceStore::flush`].
///
/// [Creating](SingleSliceStore::create), [loading](SingleSliceStore::load), and [deleting](SingleSliceStore::delete) a `SingleSliceStore` works analagously to [`StorageBackend::create`], [`StorageBackend::load`], and [`StorageBackend::delete`] respectively.
///
/// To access stored data, you can either use [`get_data`](SingleSliceStore::get_data) to retrieve (parts of) the stored prefix verbatim (i.e., without interleaved verification data), or use [`get_verifiable_stream`](SingleSliceStore::get_verifiable_stream) to obtain a slice stream suitable for ingestion by untrusted peers.
///
/// Finally, the [`SingleSliceStore::metadata`] method lets you query three kinds of information: information about the string of which the storage stores a slice (its digest, its length), information about the slice the storage intends to store (its start, its length), and about the actual prefix of that slice that has already been ingested.
///
/// The methods on this type are guaranteed not to panic under adversarial inputs. Unless stated otherwise, you can safely call the methods with data supplied from an untrusted peer over a network.
#[derive(Clone)]
pub struct SingleSliceStore<
    const WIDTH: usize,
    const CHUNK_SIZE: usize,
    ByteStorage,
    HashChunkContext,
    HashInnerContext,
> {
    byte_storage: ByteStorage,
    bab_instantiation: BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    /// Further information, which is also persisted.
    metadata: Metadata<WIDTH>,
}

impl<const WIDTH: usize, const CHUNK_SIZE: usize, ByteStorage, HashChunkContext, HashInnerContext>
    fmt::Debug
    for SingleSliceStore<WIDTH, CHUNK_SIZE, ByteStorage, HashChunkContext, HashInnerContext>
where
    ByteStorage: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleSliceStore")
            .field("byte_storage", &self.byte_storage)
            //.field("bab_instantiation", &self.bab_instantiation)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<const WIDTH: usize, const CHUNK_SIZE: usize, ByteStorage, HashChunkContext, HashInnerContext>
    SingleSliceStore<WIDTH, CHUNK_SIZE, ByteStorage, HashChunkContext, HashInnerContext>
where
    ByteStorage: StorageBackend,
    ByteStorage::Key: Clone,
{
    /// Creates a new [`SingleSliceStore`], analogous (and in fact directly delegating to) [`StorageBackend::create`].
    ///
    /// You must specify the expected root hash (i.e., Bab digest) and length of the string in advance, all ingested data is verified against these two expected values.
    ///
    /// Further, you specify the start and length of the subslice of the expected string that you actually wish to store. If you set the start to zero and the length to the total number of chunks for a string of the expected length, then you get a backend for storing the full string.
    ///
    /// Panics if the `slice_start` is greater than the number of chunks for a string of `expected_string_length` many bytes. Also panics if `slice_start + slice_length` is greater than the chunk count.
    ///
    /// Panics if `slice_length` is zero.
    pub async fn create(
        key_state: &mut ByteStorage::KeyState,
        key: ByteStorage::Key,
        expected_root_hash: BabDigest<WIDTH>,
        expected_string_length: ByteCount,
        slice_start: ChunkIndex,
        slice_length: ChunkCount,
        bab_instantiation: BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    ) -> Result<Self, ByteStorage::InternalError> {
        assert!(slice_length != 0);

        let metadata_len = Self::meta_offset_verified_progress_end();

        let chunk_count = string_length_to_chunk_count::<CHUNK_SIZE>(expected_string_length);

        assert!(
            slice_start <= chunk_count,
            "Must not `create` a SingleSliceStore with a `slice_start` greater than the chunk_count of the stored string."
        );
        assert!(
            slice_start
                .checked_add(slice_length)
                .expect("slice_start plus slice_length must not overflow")
                <= chunk_count,
            "Must not `create` a SingleSliceStore with a `slice_start + slice_length` greater than the chunk_count of the stored string."
        );

        let capacity = approximate_length_of_verifiable_stream::<WIDTH, CHUNK_SIZE>(
            slice_start,
            slice_length,
            expected_string_length,
        );

        let metadata = Metadata {
            root_hash: *expected_root_hash.as_bytes(),
            string_length: expected_string_length,
            slice_start,
            slice_length,
            verified_progress: Some(0),
            chunk_count,
        };

        let mut byte_storage = ByteStorage::create(key_state, key, capacity, metadata_len).await?;

        Self::set_metadata_during_initialisation(
            &mut byte_storage,
            Self::meta_offset_roothash_start(),
            &metadata.root_hash,
        )
        .await?;

        Self::set_metadata_during_initialisation(
            &mut byte_storage,
            Self::meta_offset_string_length_start(),
            &metadata.string_length.to_be_bytes().as_ref(),
        )
        .await?;

        Self::set_metadata_during_initialisation(
            &mut byte_storage,
            Self::meta_offset_slice_start_start(),
            metadata.slice_start.to_be_bytes().as_ref(),
        )
        .await?;

        Self::set_metadata_during_initialisation(
            &mut byte_storage,
            Self::meta_offset_slice_length_start(),
            metadata.slice_length.to_be_bytes().as_ref(),
        )
        .await?;

        Self::set_metadata_during_initialisation(
            &mut byte_storage,
            Self::meta_offset_verified_progress_start(),
            metadata.verified_progress_to_bytes().as_ref(),
        )
        .await?;

        Ok(Self {
            byte_storage,
            metadata,
            bab_instantiation,
        })
    }

    /// Loads a [`SingleSliceStore`], analogous (and in fact directly delegating to) [`StorageBackend::load`].
    ///
    /// Use the [`SingleSliceStore::metadata`] method to retrieve the initial parameters originally supplied to [`SingleSliceStore::create`] (as well as to query how much of the desired slice has already been ingested).
    pub async fn load(
        key_state: &mut ByteStorage::KeyState,
        key: &ByteStorage::Key,
        bab_instantiation: BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    ) -> Result<Option<Self>, ByteStorage::InternalError> {
        match ByteStorage::load(key_state, key).await? {
            None => Ok(None),
            Some(mut byte_store) => {
                let mut buf_root_hash = [0u8; WIDTH];
                Self::get_metadata_during_initialisation(
                    &mut byte_store,
                    Self::meta_offset_roothash_start(),
                    &mut buf_root_hash,
                )
                .await?;

                let mut buf_string_length = [0u8; 8];
                Self::get_metadata_during_initialisation(
                    &mut byte_store,
                    Self::meta_offset_string_length_start(),
                    &mut buf_string_length,
                )
                .await?;

                let mut buf_slice_start = [0u8; 8];
                Self::get_metadata_during_initialisation(
                    &mut byte_store,
                    Self::meta_offset_slice_start_start(),
                    &mut buf_slice_start,
                )
                .await?;

                let mut buf_slice_length = [0u8; 8];
                Self::get_metadata_during_initialisation(
                    &mut byte_store,
                    Self::meta_offset_slice_length_start(),
                    &mut buf_slice_length,
                )
                .await?;

                let mut buf_verified_progress = [0u8; 9];
                Self::get_metadata_during_initialisation(
                    &mut byte_store,
                    Self::meta_offset_verified_progress_start(),
                    &mut buf_verified_progress,
                )
                .await?;

                let string_length = u64::from_be_bytes(buf_string_length);
                let chunk_count = string_length_to_chunk_count::<CHUNK_SIZE>(string_length);

                let metadata = Metadata {
                    root_hash: buf_root_hash,
                    string_length,
                    slice_start: u64::from_be_bytes(buf_slice_start),
                    slice_length: u64::from_be_bytes(buf_slice_length),
                    verified_progress: if buf_verified_progress[0] == 0 {
                        None
                    } else {
                        Some(u64::from_be_bytes(
                            *(&buf_verified_progress[1..].try_into().unwrap()),
                        ))
                    },
                    chunk_count,
                };

                Ok(Some(SingleSliceStore {
                    byte_storage: byte_store,
                    metadata,
                    bab_instantiation,
                }))
            }
        }
    }

    /// Deletes a [`SingleSliceStore`], analogous (and in fact directly delegating to) [`StorageBackend::delete`].
    pub async fn delete(
        key_state: &mut ByteStorage::KeyState,
        key: &ByteStorage::Key,
    ) -> Result<(), ByteStorage::InternalError> {
        ByteStorage::delete(key_state, key).await
    }

    /// Changes the [`SingleSliceStore`] associated with one key in the given `key_state` to being associated with a different key.
    ///
    /// Does nothing if there is no [`SingleSliceStore`] associated with the first key (irrespective of whether there never was one or whether it was deleted).
    pub async fn rename(
        key_state: &mut ByteStorage::KeyState,
        old_key: &ByteStorage::Key,
        new_key: ByteStorage::Key,
    ) -> Result<(), ByteStorage::InternalError> {
        ByteStorage::rename(key_state, old_key, new_key).await
    }

    /// Creates a new store similar to [`SingleSliceStore::create`], but with the difference that the full string must be supplied immediately, and the resulting digest is returned along with the created store.
    ///
    /// Panics if the producer does not produce at least `string_length` many bytes. The storage associated with `key` is unspecified in this case.
    pub async fn create_and_initialise<P>(
        key_state: &mut ByteStorage::KeyState,
        key: ByteStorage::Key,
        string_length: ByteCount,
        string_bytes: &mut P,
        bab_instantiation: BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    ) -> Result<(Self, BabDigest<WIDTH>), ByteStorage::InternalError>
    where
        P: BulkProducer<Item = u8>,
    {
        let metadata_len = Self::meta_offset_verified_progress_end();

        let chunk_count = string_length_to_chunk_count::<CHUNK_SIZE>(string_length);

        let capacity = approximate_length_of_verifiable_stream::<WIDTH, CHUNK_SIZE>(
            0,
            chunk_count,
            string_length,
        );

        let mut byte_storage =
            ByteStorage::create(key_state, key.clone(), capacity, metadata_len).await?;

        match byte_storage
            .initialise_backend::<WIDTH, CHUNK_SIZE, _, _, _>(
                string_length,
                0,
                string_bytes,
                &bab_instantiation,
            )
            .await
        {
            Err(OperationsError::StorageDeleted) => unreachable!(),
            Err(OperationsError::Internal { err, .. }) => return Err(err),
            Ok(digest) => {
                let metadata = Metadata {
                    root_hash: *digest.as_bytes(),
                    string_length: string_length,
                    slice_start: 0,
                    slice_length: chunk_count,
                    verified_progress: None,
                    chunk_count,
                };

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_roothash_start(),
                    &metadata.root_hash,
                )
                .await?;

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_string_length_start(),
                    &metadata.string_length.to_be_bytes().as_ref(),
                )
                .await?;

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_slice_start_start(),
                    metadata.slice_start.to_be_bytes().as_ref(),
                )
                .await?;

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_slice_length_start(),
                    metadata.slice_length.to_be_bytes().as_ref(),
                )
                .await?;

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_verified_progress_start(),
                    metadata.verified_progress_to_bytes().as_ref(),
                )
                .await?;

                return Ok((
                    Self {
                        byte_storage,
                        metadata,
                        bab_instantiation,
                    },
                    digest,
                ));
            }
        }
    }

    // // Err(None) if allocation failed but things are still in a usable state.
    // pub async fn increase_slice_length(
    //     &mut self,
    //     additional_slice_length: ChunkCount,
    // ) -> Result<(), Option<OperationsError<ByteStorage::InternalError>>> {
    //     todo!()
    // }

    /// Retrieves [`Metadata`] about the stored slice: its boundaries, the expected digest and total length of the string it belongs to, and how much of that slice has already been ingested.
    pub fn metadata(&self) -> &Metadata<WIDTH> {
        &self.metadata
    }

    /// Verifies an incoming [verifiable slice stream](https://bab-hash.org/spec#slice_verification) (passed as a [`BulkProducer`] of bytes), and appends its chunk data to the available prefix of the stored slice.
    ///
    /// Use `self.metadata().slice_stream_resumption_info()` to know what kind of stream to request from a peer, and then supply the exact [`SliceStreamingOptions`] you requested also to this method.
    pub async fn append_data<P>(
        &mut self,
        p: &mut P,
        stream_options: SliceStreamingOptions,
    ) -> Result<Option<NodeNumber>, IngestSliceStreamError<P::Error, ByteStorage::InternalError>>
    where
        P: BulkProducer<Item = u8>,
    {
        let stream_info = self.metadata().slice_stream_resumption_info().expect("A Bab slice was already fully verified and stored, so do not try to append more data to it.");

        let first_chunk_of_the_stream = stream_info.start_chunk;
        let number_of_chunks_in_the_stream = self.metadata().slice_length()
            - (first_chunk_of_the_stream - self.metadata().slice_start());
        let start_offset = self.start_offset();

        match verifiable_streaming::consume_slice_stream::<
            WIDTH,
            CHUNK_SIZE,
            ByteStorage,
            P,
            HashChunkContext,
            HashInnerContext,
        >(
            first_chunk_of_the_stream,
            number_of_chunks_in_the_stream,
            &mut self.byte_storage,
            StringInfo {
                chunk_count: self.metadata.chunk_count,
                start_offset,
            },
            self.metadata.string_length,
            self.metadata.root_hash,
            stream_options,
            p,
            &self.bab_instantiation,
        )
        .await
        {
            Err(err) => {
                self.metadata.verified_progress = Some(err.next_node());
                return Err(err);
            }
            Ok(next_missing_node_number) => {
                self.metadata.verified_progress = next_missing_node_number;
                return Ok(next_missing_node_number);
            }
        }
    }

    /// Writes stored string data into the given [`BulkConsumer`], returns how many bytes were written.
    ///
    /// The `start` index (in bytes) is relative to the start of the stored slice.
    ///
    /// The `length` is given in bytes (not in chunks).
    pub async fn get_data<C>(
        &mut self,
        c: &mut C,
        start: ByteIndex,
        length: ByteCount,
    ) -> Result<ByteCount, WriteToConsumerError<ByteStorage::InternalError, C::Error>>
    where
        C: BulkConsumer<Item = u8>,
    {
        let start_offset = self.start_offset();

        let first_byte_of_the_stored_slice = self.metadata.slice_start * (CHUNK_SIZE as ByteCount);

        // Offset into the original string.
        let first_byte_of_the_requested_slice =
            match first_byte_of_the_stored_slice.checked_add(start) {
                // The requested start index would be greater than u64::max, so report being done immediately.
                None => {
                    return Ok(0);
                }
                Some(first_byte_of_the_requested_slice) => first_byte_of_the_requested_slice,
            };

        // How many bytes of data does this store currently store?
        let stored_data_len = match self.metadata().slice_stream_resumption_info() {
            None => {
                if self.metadata.slice_start + self.metadata.slice_length
                    == self.metadata.chunk_count
                {
                    self.metadata.string_length
                        - self.metadata().slice_start() * (CHUNK_SIZE as ByteCount)
                } else {
                    self.metadata.slice_length * (CHUNK_SIZE as ByteCount)
                }
            }
            Some(resumption_info) => {
                (resumption_info.start_chunk - start_offset) * (CHUNK_SIZE as ByteCount)
            }
        };

        // How many bytes do we have available, from `start` to the end of the stored prefix?
        let num_bytes_to_write_at_max = match stored_data_len.checked_sub(start) {
            // The requested start index is greater than how many bytes we have in the first place, so report being done immediately.
            None => {
                return Ok(0);
            }
            Some(num_bytes_to_write) => num_bytes_to_write,
        };

        // How many bytes will we actually write into `c` (barring `c` or the storage erroring)?
        let num_bytes_to_write = min(num_bytes_to_write_at_max, length);

        let amount = self
            .byte_storage
            .get_slice::<WIDTH, CHUNK_SIZE, C>(
                c,
                first_byte_of_the_requested_slice,
                num_bytes_to_write,
                StringInfo {
                    chunk_count: self.metadata.chunk_count,
                    start_offset,
                },
                self.metadata.string_length,
            )
            .await?;

        Ok(amount)
    }

    /// Writes a verifiable slice stream into the given [`BulkConsumer`], returns how many bytes were written.
    ///
    /// The `start` index (in chunks) is relative to the start of the stored slice.
    ///
    /// The `stream_options` determine which optimisations are performed to obtain the stream. The `length` is given in chunks, not bytes.
    pub async fn get_verifiable_stream<C>(
        &mut self,
        c: &mut C,
        start: ChunkIndex,
        length: ChunkCount,
        stream_options: SliceStreamingOptions,
    ) -> Result<ByteCount, EmitSliceStreamError<C::Error, ByteStorage::InternalError>>
    where
        C: BulkConsumer<Item = u8>,
    {
        if length == 0 {
            return Ok(0);
        }

        let start_offset = self.start_offset();

        let first_chunk_of_the_stored_slice = self.metadata.slice_start;

        // Offset into the original string.
        let first_chunk_of_the_requested_slice =
            match first_chunk_of_the_stored_slice.checked_add(start) {
                // The requested start index would be greater than u64::max, so report being done immediately.
                None => {
                    return Ok(0);
                }
                Some(first_chunk_of_the_requested_slice) => {
                    if first_chunk_of_the_requested_slice >= u64::MAX / (CHUNK_SIZE as u64) {
                        // There cannot be this many chunks, so this request is impossible to satisfy.
                        return Ok(0);
                    } else {
                        first_chunk_of_the_requested_slice
                    }
                }
            };

        // How many chunks of data does this store currently store?
        let stored_data_len: ChunkCount = match self.metadata().slice_stream_resumption_info() {
            None => self.metadata().slice_length(),
            Some(resumption_info) => resumption_info.start_chunk,
        };

        // How many chunks do we have available, from `start` to the end of the stored prefix?
        let num_chunks_to_write_at_max = match stored_data_len.checked_sub(start) {
            // The requested start index is greater than how many bytes we have in the first place, so report being done immediately.
            None => {
                return Ok(0);
            }
            Some(num_chunks_to_write) => num_chunks_to_write,
        };

        // How many bytes will we actually write into `c` (barring `c` or the storage erroring)?
        let num_chunks_to_write = min(num_chunks_to_write_at_max, length);

        let amount = produce_slice_stream::<WIDTH, CHUNK_SIZE, ByteStorage, C>(
            first_chunk_of_the_requested_slice,
            num_chunks_to_write,
            &mut self.byte_storage,
            StringInfo {
                chunk_count: self.metadata.chunk_count,
                start_offset,
            },
            self.metadata.string_length,
            stream_options,
            c,
        )
        .await?;

        Ok(amount)
    }

    /// Call [`StorageBackend::flush`] on the wrapped storage backend.
    ///
    /// Without calling this method, there are no guarantees about persistence of any ingested data.
    pub async fn flush(&mut self) -> Result<(), OperationsError<ByteStorage::InternalError>> {
        self.byte_storage
            .set_metadata(
                Self::meta_offset_verified_progress_start(),
                self.metadata.verified_progress_to_bytes().as_ref(),
            )
            .await?;
        self.byte_storage.flush().await
    }

    fn meta_offset_roothash_start() -> ByteCount {
        0
    }

    // fn meta_offset_roothash_end() -> ByteCount {
    //     Self::meta_offset_slice_start_start()
    // }

    fn meta_offset_string_length_start() -> ByteCount {
        Self::meta_offset_roothash_start() + size_of::<BabDigest<WIDTH>>() as ByteCount
    }

    // fn meta_offset_string_length_end() -> ByteCount {
    //     Self::meta_offset_slice_start_start()
    // }

    fn meta_offset_slice_start_start() -> ByteCount {
        Self::meta_offset_string_length_start() + size_of::<ByteCount>() as ByteCount
    }

    // fn meta_offset_slice_start_end() -> ByteCount {
    //     Self::meta_offset_slice_length_start()
    // }

    fn meta_offset_slice_length_start() -> ByteCount {
        Self::meta_offset_slice_start_start() + (size_of::<ChunkIndex>() as ByteCount)
    }

    // fn meta_offset_slice_length_end() -> ByteCount {
    //     Self::meta_offset_verified_progress_start()
    // }

    fn meta_offset_verified_progress_start() -> ByteCount {
        Self::meta_offset_slice_length_start() + (size_of::<ChunkCount>() as ByteCount)
    }

    fn meta_offset_verified_progress_end() -> ByteCount {
        Self::meta_offset_verified_progress_start() + (size_of::<NodeNumber>() as ByteCount) + 1
    }

    // Panics if the inner byte storage has been deleted, so only call this during initialisation.
    async fn set_metadata_during_initialisation(
        byte_storage: &mut ByteStorage,
        offset: ByteIndex,
        metadata: &[u8],
    ) -> Result<(), ByteStorage::InternalError> {
        byte_storage
            .set_metadata(offset, metadata)
            .await
            .map_err(|ops_err| match ops_err {
                OperationsError::StorageDeleted => unreachable!(),
                OperationsError::Internal {
                    err,
                    is_fatal: _is_fatal,
                } => err,
            })?;

        Ok(())
    }

    // Panics if the inner byte storage has been deleted, so only call this during initialisation.
    async fn get_metadata_during_initialisation(
        byte_storage: &mut ByteStorage,
        offset: ByteIndex,
        metadata: &mut [u8],
    ) -> Result<(), ByteStorage::InternalError> {
        byte_storage
            .get_metadata(offset, metadata)
            .await
            .map_err(|ops_err| match ops_err {
                OperationsError::StorageDeleted => unreachable!(),
                OperationsError::Internal {
                    err,
                    is_fatal: _is_fatal,
                } => err,
            })?;

        Ok(())
    }

    fn start_offset(&self) -> ChunkIndex {
        self.metadata.slice_start
    }
}
