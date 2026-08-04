//! Storage for many non-overlapping subslices of a string.
//!
//! See the [`MultiSliceStore`] type for precise information.

use ufotofu::prelude::*;

use core::fmt;

use crate::{
    CHUNK_SIZE, HashChunkContext, HashInnerContext, WIDTH,
    generic::{
        BabDigest,
        storage::{
            self as gen_storage,
            multi_slice_store::{StringMetadata, SubsliceMetadata},
            storage_backend::{OperationsError, StorageBackend, WriteToConsumerError},
            units::*,
            verifiable_streaming::{
                EmitSliceStreamError, IngestSliceStreamError, SliceStreamingOptions,
            },
        },
    },
    william3_instantiation,
};

/// Storage for a fixed number of non-empty subslices of a William3 string, in some given [`StorageBackend`].
///
/// Do not confuse the three different levels of metadata when dealing with a `SingleSliceStore`:
///
/// - information about the string whose subslices are being stored (supplied at creation time),
/// - information about the subslices to store (supplied at creation time), and
/// - those parts of the subslices that have actually been ingested yet (updated as more data is stored).
///
/// A `MultiSliceStore` is [created](MultiSliceStore::create) to store (prefixes of) a particular set of subslices of a string of known length and digest. Initially, the stored prefixes is empty. The [`append_data`](MultiSliceStore::append_data) method accepts a verifiable slice stream, verifies it, and then uses it to append to some specified prefix. Alternatively, if the full string is already known (i.e., you are not receiving data from a peer, but want to store a string you yourself created), you can use the [`create_and_initialise`](MultiSliceStore::create_and_initialise) method to store the full string and return the William3 digest of that string.
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
pub struct MultiSliceStore<ByteStorage>(
    gen_storage::MultiSliceStore<
        WIDTH,
        CHUNK_SIZE,
        ByteStorage,
        HashChunkContext,
        HashInnerContext,
    >,
);

impl<ByteStorage> fmt::Debug for MultiSliceStore<ByteStorage>
where
    ByteStorage: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<ByteStorage> MultiSliceStore<ByteStorage>
where
    ByteStorage: StorageBackend,
    ByteStorage::Key: Clone,
{
    /// Creates a new [`MultiSliceStore`].
    ///
    /// You must specify the expected root hash (i.e., William3 digest) and length of the string in advance, all ingested data is verified against these two expected values.
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
    ) -> Result<Self, ByteStorage::InternalError> {
        gen_storage::MultiSliceStore::create(
            key_state,
            key,
            expected_root_hash.into(),
            expected_string_length,
            slices,
            william3_instantiation(),
        )
        .await
        .map(MultiSliceStore)
    }

    /// Loads a [`MultiSliceStore`].
    ///
    /// Use the [`MultiSliceStore::string_metadata`] and [`MultiSliceStore::subslice_metadata`] methods to retrieve the initial parameters originally supplied to [`MultiSliceStore::create`] (as well as to query how much of the desired slice has already been ingested).
    pub async fn load(
        key_state: &mut ByteStorage::KeyState,
        key: &ByteStorage::Key,
    ) -> Result<Option<Self>, ByteStorage::InternalError> {
        gen_storage::MultiSliceStore::load(key_state, key, william3_instantiation())
            .await
            .map(|yay| yay.map(MultiSliceStore))
    }

    /// Deletes a [`MultiSliceStore`], analogous (and in fact directly delegating to) [`StorageBackend::delete`].
    pub async fn delete(
        key_state: &mut ByteStorage::KeyState,
        key: &ByteStorage::Key,
    ) -> Result<(), ByteStorage::InternalError> {
        gen_storage::MultiSliceStore::<
            WIDTH,
            CHUNK_SIZE,
            ByteStorage,
            HashChunkContext,
            HashInnerContext,
        >::delete(key_state, key)
        .await
    }

    /// Changes the [`MultiSliceStore`] associated with one key in the given `key_state` to being associated with a different key.
    ///
    /// Does nothing if there is no [`MultiSliceStore`] associated with the first key (irrespective of whether there never was one or whether it was deleted).
    pub async fn rename(
        key_state: &mut ByteStorage::KeyState,
        old_key: &ByteStorage::Key,
        new_key: ByteStorage::Key,
    ) -> Result<(), ByteStorage::InternalError> {
        gen_storage::MultiSliceStore::<
            WIDTH,
            CHUNK_SIZE,
            ByteStorage,
            HashChunkContext,
            HashInnerContext,
        >::rename(key_state, old_key, new_key)
        .await
    }

    /// Creates a new store similar to [`MultiSliceStore::create`], but with the difference that the full string must be supplied immediately, and the resulting digest is returned along with the created store. The whole string is stored as a single contiguous slice, and the metadata is set accordingly.
    ///
    /// Panics if the producer does not produce at least `string_length` many bytes. The storage associated with `key` is unspecified in this case.
    pub async fn create_and_initialise<P>(
        key_state: &mut ByteStorage::KeyState,
        key: ByteStorage::Key,
        string_length: ByteCount,
        string_bytes: &mut P,
    ) -> Result<(Self, BabDigest<WIDTH>), ByteStorage::InternalError>
    where
        P: BulkProducer<Item = u8>,
    {
        gen_storage::MultiSliceStore::create_and_initialise(
            key_state,
            key,
            string_length,
            string_bytes,
            william3_instantiation(),
        )
        .await
        .map(|(store, digest)| (MultiSliceStore(store), digest.into()))
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
        &self.0.string_metadata()
    }

    /// Retrieves metadata about the stored subslices.
    pub fn subslice_metadata(&self) -> &Vec<SubsliceMetadata> {
        &self.0.subslice_metadata()
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
        self.0.append_data(p, stream_options, i).await
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
        self.0.get_data(c, start, length).await
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
        self.0
            .get_verifiable_stream(c, start, length, stream_options)
            .await
    }

    /// Call [`StorageBackend::flush`] on the wrapped storage backend.
    ///
    /// Without calling this method, there are no guarantees about persistence of any ingested data.
    pub async fn flush(&mut self) -> Result<(), OperationsError<ByteStorage::InternalError>> {
        self.0.flush().await
    }
}
