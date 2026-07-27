//! Storage for many non-overlapping subslices of a string.
//!
//! See the [`MultiSliceStore`] type for precise information.

use ufotofu::prelude::*;

use core::cmp::min;
use core::fmt;

use crate::generic::{
    BabDigest, BabInstantiation,
    storage::{
        single_slice_store::SliceStreamResumptionInfo,
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
/// Information about the string stored in a [`MultiSliceStore`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StringMetadata<const WIDTH: usize> {
    // Immutable.
    root_hash: [u8; WIDTH],
    // Immutable.
    string_length: ByteCount,
    // Immutable, and not even persisted.
    chunk_count: ChunkCount,
}

impl<const WIDTH: usize> StringMetadata<WIDTH> {
    /// Returns the digest of the full string whose slices are being stored.
    ///
    /// This value depends neither on the specific slices, nor on the prefixes of slice data that are actually available.
    ///
    /// Runs in constant time.
    pub fn digest(&self) -> BabDigest<WIDTH> {
        self.root_hash.into()
    }

    /// Returns the length of the full string whose slices are being stored.
    ///
    /// This value depends neither on the specific slices, nor on the prefixes of slice data that are actually available.
    ///
    /// Runs in constant time.
    pub fn string_length(&self) -> ByteCount {
        self.string_length
    }
}

// In order of layout in the underlying ByteStorage.
/// Information about a single subslice of the string stored in a [`MultiSliceStore`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubsliceMetadata {
    // Immutable.
    slice_start: ChunkIndex,
    // Mutable, but only rarely, ideally.
    slice_length: ChunkCount,
    /// The first NodeNumber that is part of the slice but has not been verified yet. None if no such number exists (i.e., if the *full* slice was verified).
    // Mutable
    verified_progress: Option<NodeNumber>,
}

impl SubsliceMetadata {
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

    /// Returns the number of chunks which have already been ingested for this slice.
    ///
    /// The `total_string_length` must be the length of the complete Bab string in bytes whose slices are being stored.
    ///
    /// The ingested data always forms a prefix of the slice.
    pub fn ingested_chunks<const CHUNK_SIZE: usize>(
        &self,
        total_string_length: ByteCount,
    ) -> ChunkCount {
        match self.slice_stream_resumption_info::<CHUNK_SIZE>(total_string_length) {
            None => self.slice_length(),
            Some(resumption_info) => resumption_info.start_chunk - self.slice_start(),
        }
    }

    /// Returns information for creating (or requesting) a suitable slice stream to fill up the slice, or `None` if the slice is already fully available.
    ///
    /// The `total_string_length` must be the length of the complete Bab string in bytes whose slices are being stored.
    ///
    /// This is the only method that supplies information about how much data of the slice to store has actually been ingested already.
    ///
    /// Runs in constant time.
    pub fn slice_stream_resumption_info<const CHUNK_SIZE: usize>(
        &self,
        total_string_length: ByteCount,
    ) -> Option<SliceStreamResumptionInfo> {
        let total_chunk_count = string_length_to_chunk_count::<CHUNK_SIZE>(total_string_length);
        self.verified_progress.map(|verified_progress| {
            let start_chunk = core::cmp::max(
                node_number_to_chunk_index(verified_progress, total_chunk_count),
                self.slice_start,
            );
            let left_skip = node_number_to_left_skip(verified_progress, total_chunk_count);
            let right_skip = node_number_to_right_skip(
                verified_progress,
                self.slice_start + (self.slice_length - 1),
                total_chunk_count,
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

/// Storage for a fixed number of non-empty subslices of a Bab string, in some given [`StorageBackend`].
///
/// Do not confuse the three different levels of metadata when dealing with a `MultiSliceStore`:
///
/// - information about the string whose subslices are being stored (supplied at creation time),
/// - information about the subslices to store (supplied at creation time), and
/// - those parts of the subslices that have actually been ingested yet (updated as more data is stored).
///
/// A `MultiSliceStore` is [created](MultiSliceStore::create) to store (prefixes of) a particular set of subslices of a string of known length and digest. Initially, the stored prefixes is empty. The [`append_data`](MultiSliceStore::append_data) method accepts a verifiable slice stream, verifies it, and then uses it to append to some specified prefix. Alternatively, if the full string is already known (i.e., you are not receiving data from a peer, but want to store a string you yourself created), you can use the [`create_and_initialise`](MultiSliceStore::create_and_initialise) method to store the full string and return the Bab digest of that string.
///
/// Note that [`append_data`](MultiSliceStore::append_data) and [`create_and_initialise`](MultiSliceStore::create_and_initialise) do not [flush](StorageBackend::flush) the storage backend, you need to do so manually via [`MultiSliceStore::flush`].
///
/// [Creating](MultiSliceStore::create), [loading](MultiSliceStore::load), and [deleting](MultiSliceStore::delete) a `MultiSliceStore` works analagously to [`StorageBackend::create`], [`StorageBackend::load`], and [`StorageBackend::delete`] respectively.
///
/// To access stored data, you can either use [`get_data`](MultiSliceStore::get_data) to retrieve (consecutive parts of) a stored string data verbatim (i.e., without interleaved verification data), or use [`get_verifiable_stream`](MultiSliceStore::get_verifiable_stream) to obtain a slice stream suitable for ingestion by untrusted peers.
///
/// Finally, the [`MultiSliceStore::string_metadata`] method lets you query information about the string of which the storage stores subslices (its digest, its length), and the [`MultiSliceStore::subslice_metadata`]  method lets you query information about the slices the storage intends to store (their starts, their lengths), and about the actual prefixes of those slices that has already been ingested.
///
/// The methods on this type are guaranteed not to panic under adversarial inputs. Unless stated otherwise, you can safely call the methods with data supplied from an untrusted peer over a network.
#[derive(Clone)]
pub struct MultiSliceStore<
    const WIDTH: usize,
    const CHUNK_SIZE: usize,
    ByteStorage,
    HashChunkContext,
    HashInnerContext,
> {
    byte_storage: ByteStorage,
    bab_instantiation: BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    string_metadata: StringMetadata<WIDTH>,
    subslice_metadata: Vec<SubsliceMetadata>,
}

impl<const WIDTH: usize, const CHUNK_SIZE: usize, ByteStorage, HashChunkContext, HashInnerContext>
    fmt::Debug
    for MultiSliceStore<WIDTH, CHUNK_SIZE, ByteStorage, HashChunkContext, HashInnerContext>
where
    ByteStorage: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleSliceStore")
            .field("byte_storage", &self.byte_storage)
            //.field("bab_instantiation", &self.bab_instantiation)
            .field("string_metadata", &self.string_metadata)
            .field("subslice_metadata", &self.subslice_metadata)
            .finish()
    }
}

impl<const WIDTH: usize, const CHUNK_SIZE: usize, ByteStorage, HashChunkContext, HashInnerContext>
    MultiSliceStore<WIDTH, CHUNK_SIZE, ByteStorage, HashChunkContext, HashInnerContext>
where
    ByteStorage: StorageBackend,
    ByteStorage::Key: Clone,
{
    /// Creates a new [`MultiSliceStore`].
    ///
    /// You must specify the expected root hash (i.e., Bab digest) and length of the string in advance, all ingested data is verified against these two expected values.
    ///
    /// Further, you specify the start and length (both counted in chunks, supplied as pairs of `(start, length)`) of the subslices of the expected string that you actually wish to store. The slices must not overlap, and they must be provided in ascending order.
    ///
    /// Panics if any slice is non-empty, any two slices overlap, there are zero slices total, the slices are not sorted ascendingly, any slice starts at a greater index than the length of the full string, or if any slice extends beyond the length of the full string.
    pub async fn create(
        key_state: &mut ByteStorage::KeyState,
        key: ByteStorage::Key,
        expected_root_hash: BabDigest<WIDTH>,
        expected_string_length: ByteCount,
        slices: Vec<(ChunkIndex, ChunkCount)>,
        bab_instantiation: BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    ) -> Result<Self, ByteStorage::InternalError> {
        let chunk_count = string_length_to_chunk_count::<CHUNK_SIZE>(expected_string_length);
        let slices_len = slices.len();

        assert!(slices_len != 0);

        let (mut prev_start, mut prev_length) = slices[0];
        assert!(prev_length != 0);
        assert!(prev_start < chunk_count);
        assert!(prev_start.checked_add(prev_length).unwrap() <= chunk_count);

        for (start, length) in &slices[1..] {
            let start = *start;
            let length = *length;

            assert!(length != 0);
            assert!(start < chunk_count);
            assert!(start.checked_add(length).unwrap() <= chunk_count);
            assert!(prev_start + prev_length < start);

            prev_start = start;
            prev_length = length;
        }

        let metadata_len = Self::meta_offset_verified_progress_end(slices_len);

        let capacity = approximate_length_of_verifiable_stream::<WIDTH, CHUNK_SIZE>(
            slices[0].0,
            (slices[slices_len - 1].0 + slices[slices_len - 1].1) - slices[0].0,
            expected_string_length,
        );

        let string_metadata = StringMetadata {
            root_hash: *expected_root_hash.as_bytes(),
            string_length: expected_string_length,
            chunk_count,
        };

        let mut subslice_metadata = vec![];

        for slice in slices {
            subslice_metadata.push(SubsliceMetadata {
                slice_start: slice.0,
                slice_length: slice.1,
                verified_progress: Some(0),
            });
        }

        let mut byte_storage = ByteStorage::create(key_state, key, capacity, metadata_len).await?;

        Self::set_metadata_during_initialisation(
            &mut byte_storage,
            Self::meta_offset_roothash_start(),
            &string_metadata.root_hash,
        )
        .await?;

        Self::set_metadata_during_initialisation(
            &mut byte_storage,
            Self::meta_offset_string_length_start(),
            &string_metadata.string_length.to_be_bytes().as_ref(),
        )
        .await?;

        Self::set_metadata_during_initialisation(
            &mut byte_storage,
            Self::meta_offset_slice_count_start(),
            &(subslice_metadata.len() as u64).to_be_bytes().as_ref(),
        )
        .await?;

        for (i, slice_metadata) in subslice_metadata.iter().enumerate() {
            Self::set_metadata_during_initialisation(
                &mut byte_storage,
                Self::meta_offset_slice_start_start(i),
                slice_metadata.slice_start.to_be_bytes().as_ref(),
            )
            .await?;

            Self::set_metadata_during_initialisation(
                &mut byte_storage,
                Self::meta_offset_slice_length_start(i),
                slice_metadata.slice_length.to_be_bytes().as_ref(),
            )
            .await?;

            Self::set_metadata_during_initialisation(
                &mut byte_storage,
                Self::meta_offset_verified_progress_start(i),
                slice_metadata.verified_progress_to_bytes().as_ref(),
            )
            .await?;
        }

        Ok(Self {
            byte_storage,
            string_metadata,
            subslice_metadata,
            bab_instantiation,
        })
    }

    /// Loads a [`MultiSliceStore`].
    ///
    /// Use the [`MultiSliceStore::string_metadata`] and [`MultiSliceStore::subslice_metadata`] methods to retrieve the initial parameters originally supplied to [`MultiSliceStore::create`] (as well as to query how much of the desired slice has already been ingested).
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

                let string_length = u64::from_be_bytes(buf_string_length);
                let chunk_count = string_length_to_chunk_count::<CHUNK_SIZE>(string_length);

                let string_metadata = StringMetadata {
                    root_hash: buf_root_hash,
                    string_length,
                    chunk_count,
                };

                let mut buf_slice_count = [0u8; 8];
                Self::get_metadata_during_initialisation(
                    &mut byte_store,
                    Self::meta_offset_slice_count_start(),
                    &mut buf_slice_count,
                )
                .await?;
                let slice_count = u64::from_be_bytes(buf_slice_count);

                let mut subslice_metadata = vec![];
                for i in 0..(slice_count as usize) {
                    let mut buf_slice_start = [0u8; 8];
                    Self::get_metadata_during_initialisation(
                        &mut byte_store,
                        Self::meta_offset_slice_start_start(i),
                        &mut buf_slice_start,
                    )
                    .await?;

                    let mut buf_slice_length = [0u8; 8];
                    Self::get_metadata_during_initialisation(
                        &mut byte_store,
                        Self::meta_offset_slice_length_start(i),
                        &mut buf_slice_length,
                    )
                    .await?;

                    let mut buf_verified_progress = [0u8; 9];
                    Self::get_metadata_during_initialisation(
                        &mut byte_store,
                        Self::meta_offset_verified_progress_start(i),
                        &mut buf_verified_progress,
                    )
                    .await?;

                    subslice_metadata.push(SubsliceMetadata {
                        slice_start: u64::from_be_bytes(buf_slice_start),
                        slice_length: u64::from_be_bytes(buf_slice_length),
                        verified_progress: if buf_verified_progress[0] == 0 {
                            None
                        } else {
                            Some(u64::from_be_bytes(
                                *(&buf_verified_progress[1..].try_into().unwrap()),
                            ))
                        },
                    });
                }

                Ok(Some(MultiSliceStore {
                    byte_storage: byte_store,
                    string_metadata,
                    subslice_metadata,
                    bab_instantiation,
                }))
            }
        }
    }

    /// Deletes a [`MultiSliceStore`], analogous (and in fact directly delegating to) [`StorageBackend::delete`].
    pub async fn delete(
        key_state: &mut ByteStorage::KeyState,
        key: &ByteStorage::Key,
    ) -> Result<(), ByteStorage::InternalError> {
        ByteStorage::delete(key_state, key).await
    }

    /// Changes the [`MultiSliceStore`] associated with one key in the given `key_state` to being associated with a different key.
    ///
    /// Does nothing if there is no [`MultiSliceStore`] associated with the first key (irrespective of whether there never was one or whether it was deleted).
    pub async fn rename(
        key_state: &mut ByteStorage::KeyState,
        old_key: &ByteStorage::Key,
        new_key: ByteStorage::Key,
    ) -> Result<(), ByteStorage::InternalError> {
        ByteStorage::rename(key_state, old_key, new_key).await
    }

    /// Creates a new store similar to [`MultiSliceStore::create`], but with the difference that the full string must be supplied immediately, and the resulting digest is returned along with the created store. The whole string is stored as a single contiguous slice, and the metadata is set accordingly.
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
        let metadata_len = Self::meta_offset_verified_progress_end(0);

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
                let string_metadata = StringMetadata {
                    root_hash: *digest.as_bytes(),
                    string_length: string_length,
                    chunk_count,
                };

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_roothash_start(),
                    &string_metadata.root_hash,
                )
                .await?;

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_string_length_start(),
                    &string_metadata.string_length.to_be_bytes().as_ref(),
                )
                .await?;

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_slice_count_start(),
                    1u64.to_be_bytes().as_ref(),
                )
                .await?;

                let subslice_metadata = vec![SubsliceMetadata {
                    slice_start: 0,
                    slice_length: chunk_count,
                    verified_progress: None,
                }];

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_slice_start_start(0),
                    subslice_metadata[0].slice_start.to_be_bytes().as_ref(),
                )
                .await?;

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_slice_length_start(0),
                    subslice_metadata[0].slice_length.to_be_bytes().as_ref(),
                )
                .await?;

                Self::set_metadata_during_initialisation(
                    &mut byte_storage,
                    Self::meta_offset_verified_progress_start(0),
                    subslice_metadata[0].verified_progress_to_bytes().as_ref(),
                )
                .await?;

                return Ok((
                    Self {
                        byte_storage,
                        string_metadata,
                        subslice_metadata,
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

    /// Retrieves metadata about the stored string.
    pub fn string_metadata(&self) -> &StringMetadata<WIDTH> {
        &self.string_metadata
    }

    /// Retrieves metadata about the stored subslices.
    pub fn subslice_metadata(&self) -> &Vec<SubsliceMetadata> {
        &self.subslice_metadata
    }

    /// Verifies an incoming [verifiable slice stream](https://bab-hash.org/spec#slice_verification) (passed as a [`BulkProducer`] of bytes), and appends its chunk data to the available prefix of the `i`-th stored subslice.
    ///
    /// Use `self.subslice_metadata()[i].slice_stream_resumption_info(total_string_length)` to know what kind of stream to request from a peer, and then supply the exact [`SliceStreamingOptions`] you requested also to this method.
    pub async fn append_data<P>(
        &mut self,
        p: &mut P,
        stream_options: SliceStreamingOptions,
        i: usize,
    ) -> Result<Option<NodeNumber>, IngestSliceStreamError<P::Error, ByteStorage::InternalError>>
    where
        P: BulkProducer<Item = u8>,
    {
        let metadata = &self.subslice_metadata()[i];
        let stream_info = metadata.slice_stream_resumption_info::<CHUNK_SIZE>(self.string_metadata.string_length).expect("A Bab slice was already fully verified and stored, so do not try to append more data to it.");

        let first_chunk_of_the_stream = stream_info.start_chunk;
        let number_of_chunks_in_the_stream =
            metadata.slice_length() - (first_chunk_of_the_stream - metadata.slice_start());
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
                chunk_count: self.string_metadata.chunk_count,
                start_offset,
            },
            self.string_metadata.string_length,
            self.string_metadata.root_hash,
            stream_options,
            p,
            &self.bab_instantiation,
        )
        .await
        {
            Err(err) => {
                self.subslice_metadata[0].verified_progress = Some(err.next_node());
                return Err(err);
            }
            Ok(next_missing_node_number) => {
                self.subslice_metadata[0].verified_progress = next_missing_node_number;
                return Ok(next_missing_node_number);
            }
        }
    }

    /// Writes stored string data into the given [`BulkConsumer`], returns how many bytes were written.
    ///
    /// The `start` index (in bytes) is relative to the start of the full string.
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

        let mut available_bytes = 0;

        let mut first_match = true;
        for (i, subslice_metadata) in self.subslice_metadata.iter().enumerate() {
            if (subslice_metadata.slice_start + subslice_metadata.slice_length)
                * (CHUNK_SIZE as u64)
                < start
            {
                continue;
            }

            if subslice_metadata.slice_start * (CHUNK_SIZE as u64) >= start + length {
                break;
            }

            // We didn't leave the loop; this subslice possibly has data in the requested interval.

            let offset_in_slice = if first_match {
                first_match = false;
                start - (subslice_metadata.slice_start * (CHUNK_SIZE as u64))
            } else {
                0
            };

            match subslice_metadata
                .slice_stream_resumption_info::<CHUNK_SIZE>(self.string_metadata.string_length)
            {
                None => {
                    // The full slice is available.
                    available_bytes +=
                        subslice_metadata.slice_length * (CHUNK_SIZE as u64) - offset_in_slice;

                    // If the next slice starts exactly where the current slice ends, go to the next iteration of the loop, otherwise we reached the end of available bytes.
                    match self.subslice_metadata.get(i + 1) {
                        None => break,
                        Some(next_slice_metadata) => {
                            if next_slice_metadata.slice_start
                                == subslice_metadata.slice_start + subslice_metadata.slice_length
                            {
                                continue;
                            } else {
                                break;
                            }
                        }
                    }
                }
                Some(resumption_info) => {
                    // Only part of this slice is available. We break from the loop after adding the available bytes.

                    available_bytes += (resumption_info.start_chunk
                        - subslice_metadata.slice_start)
                        * (CHUNK_SIZE as u64)
                        - offset_in_slice;

                    break;
                }
            }
        }

        // How many bytes will we actually write into `c` (barring `c` or the storage erroring)?
        let num_bytes_to_write = min(available_bytes, length);

        if num_bytes_to_write == 0 {
            return Ok(0);
        } else {
            let amount = self
                .byte_storage
                .get_slice::<WIDTH, CHUNK_SIZE, C>(
                    c,
                    start,
                    num_bytes_to_write,
                    StringInfo {
                        chunk_count: self.string_metadata.chunk_count,
                        start_offset,
                    },
                    self.string_metadata.string_length,
                )
                .await?;

            Ok(amount)
        }
    }

    /// Writes a verifiable slice stream into the given [`BulkConsumer`], returns how many bytes were written.
    ///
    /// The `start` index (in chunks) is relative to the start of the full string.
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

        let mut available_chunks = 0;

        let mut first_match = true;
        for (i, subslice_metadata) in self.subslice_metadata.iter().enumerate() {
            if subslice_metadata.slice_start + subslice_metadata.slice_length < start {
                continue;
            }

            if subslice_metadata.slice_start >= start + length {
                break;
            }

            // We didn't leave the loop; this subslice possibly has chunks in the requested interval.

            let offset_in_slice = if first_match {
                first_match = false;
                start - subslice_metadata.slice_start
            } else {
                0
            };

            match subslice_metadata
                .slice_stream_resumption_info::<CHUNK_SIZE>(self.string_metadata.string_length)
            {
                None => {
                    // The full slice is available.
                    available_chunks += subslice_metadata.slice_length - offset_in_slice;

                    // If the next slice starts exactly where the current slice ends, go to the next iteration of the loop, otherwise we reached the end of available bytes.
                    match self.subslice_metadata.get(i + 1) {
                        None => break,
                        Some(next_slice_metadata) => {
                            if next_slice_metadata.slice_start
                                == subslice_metadata.slice_start + subslice_metadata.slice_length
                            {
                                continue;
                            } else {
                                break;
                            }
                        }
                    }
                }
                Some(resumption_info) => {
                    // Only part of this slice is available. We break from the loop after adding the available chunks.

                    available_chunks += (resumption_info.start_chunk
                        - subslice_metadata.slice_start)
                        - offset_in_slice;

                    break;
                }
            }
        }

        let start_offset = self.start_offset();

        // How many bytes will we actually write into `c` (barring `c` or the storage erroring)?
        let num_chunks_to_write = min(available_chunks, length);

        let amount = produce_slice_stream::<WIDTH, CHUNK_SIZE, ByteStorage, C>(
            start,
            num_chunks_to_write,
            &mut self.byte_storage,
            StringInfo {
                chunk_count: self.string_metadata.chunk_count,
                start_offset,
            },
            self.string_metadata.string_length,
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
        for (i, subslice_metadata) in self.subslice_metadata.iter().enumerate() {
            self.byte_storage
                .set_metadata(
                    Self::meta_offset_verified_progress_start(i),
                    subslice_metadata.verified_progress_to_bytes().as_ref(),
                )
                .await?;
        }
        self.byte_storage.flush().await
    }

    // Metadata layout in storage:
    //
    // - root hash
    // - string length in bytes
    // - number of slices to store
    // - for each slice to store:
    //     - start in chunks
    //     - length in chunks
    //     - verified progress (u64 plus an extra byte, to encode `None`)

    fn meta_offset_roothash_start() -> ByteCount {
        0
    }

    fn meta_offset_roothash_end() -> ByteCount {
        Self::meta_offset_roothash_start() + size_of::<BabDigest<WIDTH>>() as ByteCount
    }

    fn meta_offset_string_length_start() -> ByteCount {
        Self::meta_offset_roothash_end()
    }

    fn meta_offset_string_length_end() -> ByteCount {
        Self::meta_offset_string_length_start() + size_of::<ByteCount>() as ByteCount
    }

    fn meta_offset_slice_count_start() -> ByteCount {
        Self::meta_offset_string_length_end()
    }

    fn meta_offset_slice_count_end() -> ByteCount {
        Self::meta_offset_slice_count_start() + size_of::<u64>() as ByteCount
    }

    fn meta_offset_slice_start_start(i: usize) -> ByteCount {
        Self::meta_offset_slice_count_end() + (i as u64) * (3 * (size_of::<u64>() as ByteCount) + 1)
    }

    fn meta_offset_slice_start_end(i: usize) -> ByteCount {
        Self::meta_offset_slice_start_start(i) + (size_of::<u64>() as ByteCount)
    }

    fn meta_offset_slice_length_start(i: usize) -> ByteCount {
        Self::meta_offset_slice_start_end(i)
    }

    fn meta_offset_slice_length_end(i: usize) -> ByteCount {
        Self::meta_offset_slice_length_start(i) + (size_of::<u64>() as ByteCount)
    }

    fn meta_offset_verified_progress_start(i: usize) -> ByteCount {
        Self::meta_offset_slice_length_end(i)
    }

    fn meta_offset_verified_progress_end(i: usize) -> ByteCount {
        Self::meta_offset_verified_progress_start(i) + (size_of::<u64>() as ByteCount) + 1
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
        self.subslice_metadata[0].slice_start
    }
}
