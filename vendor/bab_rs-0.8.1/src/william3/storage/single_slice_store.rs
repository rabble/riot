//! Storage for a (prefix of a) single contiguous subslice of a string.
//!
//! See the [`SingleSliceStore`] type for precise information.

use core::fmt;

use ufotofu::prelude::*;

use crate::{
    CHUNK_SIZE, HashChunkContext, HashInnerContext, WIDTH, William3Digest,
    generic::{
        BabDigest,
        storage::{
            self as gen_storage,
            single_slice_store::Metadata,
            storage_backend::{OperationsError, StorageBackend, WriteToConsumerError},
            units::*,
            verifiable_streaming::{
                EmitSliceStreamError, IngestSliceStreamError, SliceStreamingOptions,
            },
        },
    },
    william3_instantiation,
};

/// Storage for a single non-empty subslice of a William3 string, in some given [`StorageBackend`].
///
/// Do not confuse the three different levels of metadata when dealing with a `SingleSliceStore`:
///
/// - information about the string of which a subslice is being stored (supplied at creation time),
/// - information about that subslice to store (supplied at creation time), and
/// - those parts of the subslice that have actually been ingested yet (updated as more data is stored; this is always a contiguous prefix of the total subslice).
///
/// A `SingleSliceStore` is [created](SingleSliceStore::create) to store a (prefix of) a particular slice of a string of known length and digest. Initially, the stored prefix is empty. The [`append_data`](SingleSliceStore::append_data) method accepts a verifiable slice stream, verifies it, and then uses it to append to the stored prefix. Alternatively, if the full string is already known (i.e., you are not receiving data from a peer, but want to store a string you yourself created), you can use the [`create_and_initialise`](SingleSliceStore::create_and_initialise) method to store the full string and return the William3 digest of that string.
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
pub struct SingleSliceStore<ByteStorage>(
    gen_storage::SingleSliceStore<
        WIDTH,
        CHUNK_SIZE,
        ByteStorage,
        HashChunkContext,
        HashInnerContext,
    >,
);

impl<ByteStorage> fmt::Debug for SingleSliceStore<ByteStorage>
where
    ByteStorage: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<ByteStorage> SingleSliceStore<ByteStorage>
where
    ByteStorage: StorageBackend,
    ByteStorage::Key: Clone,
{
    /// Creates a new [`SingleSliceStore`], analogous (and in fact directly delegating to) [`StorageBackend::create`].
    ///
    /// You must specify the expected root hash (i.e., William3 digest) and length of the string in advance, all ingested data is verified against these two expected values.
    ///
    /// Further, you specify the start and length of the subslice of the expected string that you actually wish to store. If you set the start to zero and the length to the total number of chunks for a string of the expected length, then you get a backend for storing the full string.
    ///
    /// Panics if the `slice_start` is greater than the number of chunks for a string of `expected_string_length` many bytes. Also panics if `slice_start + slice_length` is greater than the chunk count.
    ///
    /// Panics if `slice_length` is zero.
    pub async fn create(
        key_state: &mut ByteStorage::KeyState,
        key: ByteStorage::Key,
        expected_root_hash: William3Digest,
        expected_string_length: ByteCount,
        slice_start: ChunkIndex,
        slice_length: ChunkCount,
    ) -> Result<Self, ByteStorage::InternalError> {
        gen_storage::SingleSliceStore::create(
            key_state,
            key,
            expected_root_hash.into(),
            expected_string_length,
            slice_start,
            slice_length,
            william3_instantiation(),
        )
        .await
        .map(SingleSliceStore)
    }

    /// Loads a [`SingleSliceStore`], analogous (and in fact directly delegating to) [`StorageBackend::load`].
    ///
    /// Use the [`SingleSliceStore::metadata`] method to retrieve the initial parameters originally supplied to [`SingleSliceStore::create`] (as well as to query how much of the desired slice has already been ingested).
    pub async fn load(
        key_state: &mut ByteStorage::KeyState,
        key: &ByteStorage::Key,
    ) -> Result<Option<Self>, ByteStorage::InternalError> {
        gen_storage::SingleSliceStore::load(key_state, key, william3_instantiation())
            .await
            .map(|yay| yay.map(SingleSliceStore))
    }

    /// Deletes a [`SingleSliceStore`], analogous (and in fact directly delegating to) [`StorageBackend::delete`].
    pub async fn delete(
        key_state: &mut ByteStorage::KeyState,
        key: &ByteStorage::Key,
    ) -> Result<(), ByteStorage::InternalError> {
        gen_storage::SingleSliceStore::<
            WIDTH,
            CHUNK_SIZE,
            ByteStorage,
            HashChunkContext,
            HashInnerContext,
        >::delete(key_state, key)
        .await
    }

    /// Changes the [`SingleSliceStore`] associated with one key in the given `key_state` to being associated with a different key.
    ///
    /// Does nothing if there is no [`SingleSliceStore`] associated with the first key (irrespective of whether there never was one or whether it was deleted).
    pub async fn rename(
        key_state: &mut ByteStorage::KeyState,
        old_key: &ByteStorage::Key,
        new_key: ByteStorage::Key,
    ) -> Result<(), ByteStorage::InternalError> {
        gen_storage::SingleSliceStore::<
            WIDTH,
            CHUNK_SIZE,
            ByteStorage,
            HashChunkContext,
            HashInnerContext,
        >::rename(key_state, old_key, new_key)
        .await
    }

    /// Creates a new store similar to [`SingleSliceStore::create`], but with the difference that the full string must be supplied immediately, and the resulting digest is returned along with created store.
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
        gen_storage::SingleSliceStore::create_and_initialise(
            key_state,
            key,
            string_length,
            string_bytes,
            william3_instantiation(),
        )
        .await
        .map(|(store, digest)| (SingleSliceStore(store), digest.into()))
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
        self.0.metadata()
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
        self.0.append_data(p, stream_options).await
    }

    /// Writes stored string data into the given [`BulkConsumer`], and returns how many bytes were written.
    ///
    /// The `start` index (in bytes) is relative to the start of the stored slice.
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
