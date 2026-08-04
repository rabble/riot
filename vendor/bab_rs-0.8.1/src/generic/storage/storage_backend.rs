//! Abstractions for storing Bab data, most importantly provides the [`StorageBackend`] trait.

use core::cmp::min;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use derive_more::{Display, Error};

use ufotofu::prelude::*;

use crate::generic::{BabDigest, BabInstantiation, Label, storage::units::*};

/// Backing storage for a single bab-addressed string.
///
/// You only need to care about this trait if you want to implement a custom persistent Bab backend. If you are fine using our provided backends, you can safely ignore the internals of this trait.
///
/// An instance of this trait provides access to a (conceptual) array of bytes, storing a [baseline verifiable slice stream](https://bab-hash.org/spec#baseline_slice) for some given start chunk index and slice length (in chunks). This trait neither enforces validity of the bytes, nor does it even keep track which bytes have already been set to correct data and which bytes do not contain valid data yet. All such tracking and verification happens at a higher level of abstraction — this trait is the simplemost useful storage provider that can make such higher levels of abstractions work.
///
/// In addition to the byte array of Bab data, the trait also provides a simple interface for storing metadata - in the form of a fixed-sized (size specified at creation time) sequence of bytes. The intention for this metadata is that higher-level abstractions building on top of a StorageBackend can track which parts of a slice (or slices) have been stored yet, persist an expected root hash, etc.
///
/// Because this trait is intended for persistent storage, but persisting updates is expensive, the insertion methods are not guaranted to be made fully persistent immediately. To ensure that all prior updates are fully written to disk, call the [`flush`](StorageBackend::flush) method.
///
/// Errors returned by any method are fatal (i.e., the backend should be considered unusable after an error), unless the method documentation explicitly states otherwise.
///
/// The [`create`](StorageBackend::create), [`load`](StorageBackend::load), and [`delete`](StorageBackend::delete) methods allow associating keys to storage backends. This is particularly important with persistent stores, where you need a way of retrieving a particular store from disk. The system is based on two associated types: [`Key`](StorageBackend::Key) is the type of keys by which individual backends can be addressed, and [`KeyState`](StorageBackend::KeyState) can track state across multiple creation, loading, and deletion operations (for example to memoise data stored on disk).
///
/// The default methods on this trait offer access in terms of node contents and chunks. Internally, they perform a conversion to raw byte offsets, with a [`StringInfo`] argument providing the context required to do the conversions. The methods further take const generic parameters `WIDTH` and `CHUNK_SIZE` to convey the Bab [width](https://bab-hash.org/spec#width) and [chunk size](https://bab-hash.org/spec#chunk_size) parameters respectively.
///
/// Finally, note that cloning a `StorageBackend` should always be cheap (typically achieved by using reference-counting). Code written to work with arbitrary `StorageBackends` is allowed to assume that cloning is cheap.
pub trait StorageBackend: Sized + Clone {
    /// The type of errors the backends might yield on any operation.
    type InternalError;

    /// The type of values by which individual storage backends can be identified. Used by [`create`](StorageBackend::create), [`load`](StorageBackend::load), and [`delete`](StorageBackend::delete).
    type Key;

    /// The type of the shared, mutable state to keep across calls to [`create`](StorageBackend::create), [`load`](StorageBackend::load), and [`delete`](StorageBackend::delete).
    type KeyState;

    /// Creates a new storage backend, later retrievable via [`StorageBackend::load`] by supplying an equal key to the same `key_state`. If there already existed a StorageBackend for the given key, it is overwritten with a fresh backend.
    ///
    /// The `capacity` specifies how many bytes of Bab data the created backend can store. This value cannot be adjusted later.
    ///
    /// The `metadata_len` specifies the number of bytes of metadata the created storage backend can manage. This value cannot be adjusted later.
    async fn create(
        key_state: &mut Self::KeyState,
        key: Self::Key,
        capacity: ByteCount,
        metadata_len: ByteCount,
    ) -> Result<Self, Self::InternalError>;

    /// Creates a handle to a preexisting storage backend, identified by the given `key`, or `None` if no backend has been [created](StorageBackend::create) (and not already [deleted](StorageBackend::delete)) for this key yet (in the same `key_state`).
    async fn load(
        key_state: &mut Self::KeyState,
        key: &Self::Key,
    ) -> Result<Option<Self>, Self::InternalError>;

    /// Deletes all persistent storage (and possibly even in-memory data) for the storage backend of the given key. All future operations on that backend will yield an [`OperationsError::StorageDeleted`] error. This will also affect clones of the backend, or backends [loaded](StorageBackend::load) with the same `key`.
    ///
    /// If, after deletion, a new storage is created for the same `key`, handles to the old, deleted storage *should* still report errors instead of operating on the new data, but this is not required. Implementations in which handles to deleted storage can become non-erroneous again should explicitly document that this is the case. This behaviour should only be implemented if guaranteeing errors on old handles would be too expensive.
    ///
    /// Returns an `InternalError` instead of an `OperationsError`; if the storage was already deleted in the past, this is reported as a success.
    async fn delete(
        key_state: &mut Self::KeyState,
        key: &Self::Key,
    ) -> Result<(), Self::InternalError>;

    /// Changes the data associated with one key in the given key_state to being associated with a different key.
    ///
    /// Does nothing if there is no storage backend associated with the first key (irrespective of whether there never was one or whether it was deleted).
    async fn rename(
        key_state: &mut Self::KeyState,
        old_key: &Self::Key,
        new_key: Self::Key,
    ) -> Result<(), Self::InternalError>;

    /// Returns the number of bab bytes this can store.
    async fn get_capacity(&mut self) -> Result<ByteCount, OperationsError<Self::InternalError>>;

    /// Fills `buf` with stored bytes, starting from the given offset in the storage.
    ///
    /// Panics if `offset + buf.len() >= self.get_capacity()`.
    async fn get_bytes(
        &mut self,
        offset: ByteIndex,
        buf: &mut [u8],
    ) -> Result<(), OperationsError<Self::InternalError>>;

    /// Fills `buf` with the data of the chunk of index `chunk_index`, assuming the storage is for a string of `chunk_count` many chunks, storing chunks starting at chunk index `start_offset`.
    ///
    /// Careful: the final chunk might be shorter than `CHUNK_SIZE`. Calling this method with the `chunk_index` of the final chunk and a `buf` whose length exceeeds the length of the final chunk may exhibit unspecified behaviour (most likely, it will panic).
    async fn get_chunk<const WIDTH: usize, const CHUNK_SIZE: usize>(
        &mut self,
        chunk_index: ChunkIndex,
        string_info: StringInfo,
        buf: &mut [u8],
    ) -> Result<(), OperationsError<Self::InternalError>> {
        let start_byte_index = chunk_to_byte_index::<WIDTH, CHUNK_SIZE>(
            chunk_index,
            string_info.chunk_count,
            string_info.start_offset,
        );

        return self.get_bytes(start_byte_index, buf).await;
    }

    /// Fills `left` and `right` with the node content at the given `node_number`, assuming the storage is for a string of `chunk_count` many chunks, storing chunks starting at chunk index `start_offset`. Will fill `left` and `right` with unspecified data if the `node_number` does not correspond to an inner node.
    ///
    /// Surprisingly enough, the label of the left child is written into `left`, and the label of the right child is written into `right`.
    async fn get_node_contents<const WIDTH: usize, const CHUNK_SIZE: usize>(
        &mut self,
        node_number: NodeNumber,
        string_info: StringInfo,
        left: &mut Label<WIDTH>,
        right: &mut Label<WIDTH>,
    ) -> Result<(), OperationsError<Self::InternalError>> {
        let start_byte_index = node_number_to_byte_index::<WIDTH, CHUNK_SIZE>(
            node_number,
            string_info.chunk_count,
            string_info.start_offset,
        );

        // Wish this could be stack-allocated, but arrays of size `2 * WIDTH` are not possible on stable rust yet.
        let mut buf = vec![0; 2 * WIDTH];

        self.get_bytes(start_byte_index, &mut buf[..]).await?;

        left.copy_from_slice(&buf[..WIDTH]);
        right.copy_from_slice(&buf[WIDTH..]);

        return Ok(());
    }

    /// Writes the bytes of a (sub-) slice of the stored string into the given bulk consumer of bytes. Returns how many bytes were written.
    ///
    /// This method only writes original string data, but no Merkle tree node labels.
    async fn get_slice<const WIDTH: usize, const CHUNK_SIZE: usize, C>(
        &mut self,
        c: &mut C,
        slice_start: ByteIndex,
        slice_length: ByteCount,
        string_info: StringInfo,
        string_len: ByteCount,
    ) -> Result<ByteCount, WriteToConsumerError<Self::InternalError, C::Error>>
    where
        C: BulkConsumer<Item = u8>,
    {
        let mut gotten = 0;

        while gotten < slice_length {
            // Process one chunk per iteration of the loop.
            let current_position_in_string = slice_start + gotten;

            let chunk_index = current_position_in_string / (CHUNK_SIZE as u64);
            let mut chunk = [0u8; CHUNK_SIZE];
            let chunk_len = if chunk_index == string_info.chunk_count - 1 {
                let remainder = (string_len as usize) % CHUNK_SIZE;
                if remainder == 0 {
                    CHUNK_SIZE
                } else {
                    remainder
                }
            } else {
                CHUNK_SIZE
            };

            self.get_chunk::<WIDTH, CHUNK_SIZE>(chunk_index, string_info, &mut chunk[..chunk_len])
                .await
                .map_err(|err| WriteToConsumerError::StorageError(err))?;

            let start_in_chunk = current_position_in_string % (CHUNK_SIZE as u64);
            let end_in_chunk =
                start_in_chunk + min(slice_length - gotten, CHUNK_SIZE as u64 - start_in_chunk);

            debug_assert!(start_in_chunk < end_in_chunk);
            debug_assert!(end_in_chunk <= CHUNK_SIZE as u64);

            c.bulk_consume_full_slice(&chunk[start_in_chunk as usize..end_in_chunk as usize])
                .await
                .map_err(|err| WriteToConsumerError::ConsumerError(err.into_reason()))?;

            gotten += end_in_chunk - start_in_chunk;
        }

        Ok(gotten)
    }

    // /// Reserves more capacity at the end of the stored array.
    // ///
    // /// The argument specifies how much capacity to add to the existing capacity (i.e., *not* the new total).
    // ///
    // /// If this returns `Err(None)`, then the error is not fatal: allocating more capacity was not possible, so the capacity remained unchanged, and the storage remains operational.
    // async fn reserve_capacity(
    //     &mut self,
    //     additional_capacity: ByteCount,
    // ) -> Result<(), Option<OperationsError<Self::InternalError>>>;

    /// Sets bytes in the storage backend to the given slice, starting at the given offset.
    ///
    /// Panics if `offset + new_data.len() >= self.get_capacity()`.
    async fn set_bytes(
        &mut self,
        offset: ByteIndex,
        new_data: &[u8],
    ) -> Result<(), OperationsError<Self::InternalError>>;

    /// Writes into the storage backend the node contents of the inner node `node_number`, assuming the backend is for a string of `chunk_count` many chunks, storing chunks starting at chunk index `start_offset`.
    ///
    /// Panics if `node_number` does not correspond to an inner node, or the node number does not fit into the allocated storage backend.
    async fn set_inner_node_contents<const WIDTH: usize, const CHUNK_SIZE: usize>(
        &mut self,
        node_number: NodeNumber,
        string_info: StringInfo,
        left: &Label<WIDTH>,
        right: &Label<WIDTH>,
    ) -> Result<(), OperationsError<Self::InternalError>> {
        let start_byte_index = node_number_to_byte_index::<WIDTH, CHUNK_SIZE>(
            node_number,
            string_info.chunk_count,
            string_info.start_offset,
        );

        self.set_bytes(start_byte_index, left).await?;
        self.set_bytes(start_byte_index + (WIDTH as u64), right)
            .await?;

        Ok(())
    }

    /// Writes into the storage backend the chunk data of a chunk at the given `chunk_index`, assuming the backend is for a string of `chunk_count` many chunks, storing chunks starting at chunk index `start_offset`.
    ///
    /// Panics if the chunk index does not fit into the allocated storage backend.
    async fn set_chunk<const WIDTH: usize, const CHUNK_SIZE: usize>(
        &mut self,
        chunk_index: ChunkIndex,
        string_info: StringInfo,
        chunk_bytes: &[u8],
    ) -> Result<(), OperationsError<Self::InternalError>> {
        debug_assert!(chunk_bytes.len() <= CHUNK_SIZE);

        let start_byte_index = chunk_to_byte_index::<WIDTH, CHUNK_SIZE>(
            chunk_index,
            string_info.chunk_count,
            string_info.start_offset,
        );

        self.set_bytes(start_byte_index, chunk_bytes).await?;

        Ok(())
    }

    /// Writes to a StorageBackend the raw bytes of a string (supplied as a producer of bytes; the total length must be known in advance) and automatically fills in the correct Merkle tree labels as well. I.e., this method takes a raw string, but stores a verifiable stream.
    /// Returns the root label, i.e., the Bab digest, of the string.
    ///
    /// Panics if the producer does not produce at least `string_length` many bytes. The storage associated with `key` is unspecified in this case.
    ///
    /// The `start_offset` parameter has the same meaning as the corresponding field of the [`StringInfo`] struct. Internally, this method constructs a [`StringInfo`] by converting the `string_length_in_bytes` into a chunk count and reusing the supplied `start_offset`.
    async fn initialise_backend<
        const WIDTH: usize,
        const CHUNK_SIZE: usize,
        P,
        HashChunkContext,
        HashInnerContext,
    >(
        &mut self,
        string_length_in_bytes: ByteCount,
        start_offset: ChunkIndex,
        string_bytes: &mut P,
        bab_instantiation: &BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    ) -> Result<BabDigest<WIDTH>, OperationsError<Self::InternalError>>
    where
        P: BulkProducer<Item = u8>,
    {
        super::verifiable_streaming::initialise_store::<WIDTH, CHUNK_SIZE, _, _, _, _>(
            self,
            string_length_in_bytes,
            start_offset,
            string_bytes,
            bab_instantiation,
        )
        .await
        .map(From::from)
    }

    /// Returns how many bytes of metadata can be stored in `self`.
    async fn get_len_of_metadata(
        &mut self,
    ) -> Result<ByteCount, OperationsError<Self::InternalError>>;

    /// Overwrites `buf` with metadata bytes, starting from the given offset.
    ///
    /// Panics if `offset + buf.len() >= self.get_metadata_len()`.
    async fn get_metadata(
        &mut self,
        offset: ByteIndex,
        buf: &mut [u8],
    ) -> Result<(), OperationsError<Self::InternalError>>;

    /// Sets a slice of metadata, starting at the given offset.
    ///
    /// Panics if `offset + new_data.len() >= self.get_metadata_len()`.
    async fn set_metadata(
        &mut self,
        offset: ByteIndex,
        new_data: &[u8],
    ) -> Result<(), OperationsError<Self::InternalError>>;

    /// Flushes all prior operations. If the storage backend is persistent, a successful flush guarantees persistence of all mutations up to that point.
    async fn flush(&mut self) -> Result<(), OperationsError<Self::InternalError>>;
}

/// The error type for most operations involving a [`StorageBackend`].
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Debug, Display, Error)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub enum OperationsError<InternalError> {
    /// The storage backend has been [deleted](StorageBackend::delete).
    ///
    /// All further operations on it will error as well.
    #[display("storage was deleted")]
    StorageDeleted,
    /// The storage backend encountered an error internally. The `is_fatal` flag indicates whether the backend is in a consistent state and can be used going forward (`false`) or whether the error was irrecoverable (`true`).
    #[display("internal storage error")]
    Internal {
        #[error(source)]
        /// The cause of the error.
        err: InternalError,
        /// Whether the error is recoverable (`false`) or irrecoverable (`true`).
        is_fatal: bool,
    },
}

/// Everything that can go wrong when writing data from a [`StorageBackend`] into a [`BulkConsumer`]: the backend might encounter an [`OperationsError`], or the consumer might emit an error.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Debug, Display, Error)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub enum WriteToConsumerError<InternalError, ConsumerError> {
    #[display("storage error")]
    /// Failed to retrieve slice data from the [`StorageBackend`].
    StorageError(#[error(source)] OperationsError<InternalError>),
    #[display("consumer error")]
    /// The consumer emitted an error before all slice data could be written into it.
    ConsumerError(#[error(source)] ConsumerError),
}

/// Information about the string stored in a [`StorageBackend`], required by the methods that interact with the backend in terms of Bab concepts.
///
/// Each [`StorageBackend`] is expected to store Bab data in a certain form: as the a [baselin verifiable slice stream](https://bab-hash.org/spec#baseline_slice) for a string of some known total number of chunks, with the slice starting at some known chunk index. This slice may or may not extend to the very end of the stream, this is determined implicitly through the total size of allocated byte storage in the backend. In other words, a backend is not required to allocate storage for a full string, but only to allocate storage for some subslice of a string.
///
/// The methods for interacting with a backend in terms of Bab concepts (retrieving a chunk, setting a Merkle tree label, etc) need to know the number of chunks of the underlying string, and the start offset of the slice that is actually being stored. This type supplies that information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StringInfo {
    /// How many chunks does the underlying string consist of in total?
    pub chunk_count: ChunkCount,
    /// Beginning at which chunk index does the backend actually store string data?
    pub start_offset: ChunkIndex,
}

////////////////
// Index Math //
////////////////

fn byte_size_of_node_content<const WIDTH: usize>() -> ByteCount {
    (WIDTH as u64) * 2
}

/// Converts a [`ChunkIndex`] into the [`ByteIndex`] at which a [`StorageBackend`] stores the raw bytes of the chunk.
///
/// The `start_offset` gives the first chunk of the stored slice.
///
/// Panics if `chunk_index >= chunk_count`, or if `chunk_index < start_offset`.
pub(crate) fn chunk_to_byte_index<const WIDTH: usize, const CHUNK_SIZE: usize>(
    chunk_index: ChunkIndex,
    chunk_count: ChunkCount,
    start_offset: ChunkIndex,
) -> ByteIndex {
    assert!(chunk_index < chunk_count);
    assert!(chunk_index >= start_offset,);

    return chunk_to_byte_index_actual::<WIDTH, CHUNK_SIZE>(chunk_index, chunk_count, start_offset);

    // Actual, recursion-friendly implementation of `chunk_to_byte`.
    fn chunk_to_byte_index_actual<const WIDTH: usize, const CHUNK_SIZE: usize>(
        chunk_index: ChunkIndex,
        chunk_count: ChunkCount,
        start_offset: ChunkIndex,
    ) -> ByteIndex {
        if chunk_index < start_offset {
            return 0;
        } else {
            if chunk_count == 1 {
                // trivial tree, trivial solution
                return 0;
            } else {
                let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

                if chunk_index < left_subtree_leaf_count {
                    // chunk is in left subtree of the root (which is a perfect subtree)
                    return byte_size_of_node_content::<WIDTH>()
                        + chunk_to_byte_index_actual::<WIDTH, CHUNK_SIZE>(
                            chunk_index,
                            left_subtree_leaf_count,
                            start_offset,
                        );
                } else {
                    // chunk is in right subtree of the root
                    return size_of_perfect_tree::<WIDTH, CHUNK_SIZE>(
                        left_subtree_leaf_count,
                        start_offset,
                    ) + byte_size_of_node_content::<WIDTH>() // root content
                     + chunk_to_byte_index_actual::<WIDTH, CHUNK_SIZE>(
                        chunk_index - left_subtree_leaf_count,
                        chunk_count - left_subtree_leaf_count,
                        start_offset.saturating_sub(left_subtree_leaf_count),
                    );
                }
            }
        }
    }
}

fn size_of_perfect_tree<const WIDTH: usize, const CHUNK_SIZE: usize>(
    leaf_count: ChunkCount,
    start_offset: ChunkIndex,
) -> ByteCount {
    // Only called with perfect trees, i.e. leaf count is a power of two.
    debug_assert!(
        leaf_count.next_power_of_two() == leaf_count,
        "leaf count: {leaf_count}"
    );

    if start_offset >= leaf_count {
        return 0;
    } else if start_offset == 0 {
        leaf_count * (CHUNK_SIZE as u64) + (leaf_count - 1) * byte_size_of_node_content::<WIDTH>()
    } else {
        size_of_perfect_tree::<WIDTH, CHUNK_SIZE>(leaf_count / 2, start_offset)
            + byte_size_of_node_content::<WIDTH>() // root content
            + size_of_perfect_tree::<WIDTH, CHUNK_SIZE>(leaf_count / 2, start_offset.saturating_sub(leaf_count / 2))
    }
}

#[test]
fn test_chunk_to_byte_index() {
    let expected = [
        &[
            // The i-th slice lists the ByteIndex for the chunks of the trees on i + 1 leaves,
            // for a start_offset of zero.
            &[0u64][..],
            &[64, 64 + 1024][..],
            &[64 * 2, 64 * 2 + 1024, 64 * 2 + 1024 * 2][..],
            &[64 * 2, 64 * 2 + 1024, 64 * 3 + 1024 * 2, 64 * 3 + 1024 * 3][..],
            &[
                64 * 3,
                64 * 3 + 1024,
                64 * 4 + 1024 * 2,
                64 * 4 + 1024 * 3,
                64 * 4 + 1024 * 4,
            ][..],
        ][..],
        &[
            // The i-th slice lists the ByteIndex for the chunks of the trees on i + 2 leaves,
            // for a start_offset of one.
            &[64][..],
            &[64 * 2, 64 * 2 + 1024][..],
            &[64 * 2, 64 * 3 + 1024, 64 * 3 + 1024 * 2][..],
            &[64 * 3, 64 * 4 + 1024, 64 * 4 + 1024 * 2, 64 * 4 + 1024 * 3][..],
        ][..],
        &[
            // The i-th slice lists the ByteIndex for the chunks of the trees on i + 3 leaves,
            // for a start_offset of two.
            &[64][..],
            &[64 * 2, 64 * 2 + 1024][..],
            &[64 * 3, 64 * 3 + 1024, 64 * 3 + 1024 * 2][..],
        ][..],
        &[
            // The i-th slice lists the ByteIndex for the chunks of the trees on i + 4 leaves,
            // for a start_offset of three.
            &[64 * 2][..],
            &[64 * 3, 64 * 3 + 1024][..],
        ][..],
        &[
            // The i-th slice lists the ByteIndex for the chunks of the trees on i + 5 leaves,
            // for a start_offset of four.
            &[64][..],
        ][..],
    ];

    for (start_offset, expected) in expected.iter().enumerate() {
        for (i, testdata) in expected.iter().enumerate() {
            let total_chunk_count = (i + 1 + start_offset) as u64;
            for (chunk_index, byte_offset) in testdata.iter().enumerate() {
                let chunk_index = chunk_index + start_offset;

                assert_eq!(
                    chunk_to_byte_index::<32, 1024>(
                        chunk_index as ChunkIndex,
                        total_chunk_count,
                        start_offset as u64,
                    ),
                    *byte_offset,
                    "start_offset: {start_offset}, total_chunk_count: {total_chunk_count}, chunk_index: {chunk_index}",
                );
            }
        }
    }
}

/// Converts a [`NodeNumber`] into the [`ByteIndex`] at which a [`StorageBackend`] stores the raw bytes for the node (a node content for inner nodes, or a chunk for a leaf node).
///
/// The `start_offset` gives the first chunk of the stored slice.
///
/// Unspecified behaviour if the `node_number` refers to a node outside the range of valid nodes defined by the `start_offset` and the `chunk_count`.
pub(crate) fn node_number_to_byte_index<const WIDTH: usize, const CHUNK_SIZE: usize>(
    node_number: NodeNumber,
    chunk_count: ChunkCount,
    start_offset: ChunkIndex,
) -> ByteIndex {
    if start_offset >= chunk_count {
        return 0;
    }

    if node_number == 0 {
        // Trivial case: the root is always stored at offset zero.
        return 0;
    } else {
        let left_subtree_node_count = chunk_count.next_power_of_two() - 1;
        let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

        if node_number <= left_subtree_node_count {
            // node_number is in the left subtree.
            return byte_size_of_node_content::<WIDTH>() // root content
                    + node_number_to_byte_index::<WIDTH, CHUNK_SIZE>(
                        node_number - 1,
                        left_subtree_leaf_count,
                        start_offset,
                    );
        } else {
            return size_of_perfect_tree::<WIDTH, CHUNK_SIZE>(
                        left_subtree_leaf_count,
                        start_offset,
                    ) + byte_size_of_node_content::<WIDTH>() // root content
                     + node_number_to_byte_index::<WIDTH, CHUNK_SIZE>(
                        node_number - (left_subtree_node_count + 1),
                        chunk_count - left_subtree_leaf_count,
                        start_offset.saturating_sub(left_subtree_leaf_count),
                    );
        }
    }
}

#[test]
fn test_node_number_to_byte_index() {
    let expected = [
        &[
            // The i-th slice lists the byte sizes for the nodes of the trees on i + 1 leaves,
            // for a start_offset of zero.
            &[1024][..],
            &[64, 1024, 1024][..],
            &[64, 64, 1024, 1024, 1024][..],
            &[64, 64, 1024, 1024, 64, 1024, 1024][..],
            &[64, 64, 64, 1024, 1024, 64, 1024, 1024, 1024][..],
        ][..],
        &[
            // The i-th slice lists the byte sizes for the nodes of the trees on i + 2 leaves,
            // for a start_offset of one.
            &[64, 0, 1024][..],
            &[64, 64, 0, 1024, 1024][..],
            &[64, 64, 0, 1024, 64, 1024, 1024][..],
            &[64, 64, 64, 0, 1024, 64, 1024, 1024, 1024][..],
        ][..],
        &[
            // The i-th slice lists the byte sizes for the nodes of the trees on i + 3 leaves,
            // for a start_offset of two.
            &[64, 0, 0, 0, 1024][..],
            &[64, 0, 0, 0, 64, 1024, 1024][..],
            &[64, 64, 0, 0, 0, 64, 1024, 1024, 1024][..],
        ][..],
        &[
            // The i-th slice lists the byte sizes for the nodes of the trees on i + 4 leaves,
            // for a start_offset of three.
            &[64, 0, 0, 0, 64, 0, 1024][..],
            &[64, 64, 0, 0, 0, 64, 0, 1024, 1024][..],
        ][..],
        &[
            // The i-th slice lists the byte sizes for the nodes of the trees on i + 5 leaves,
            // for a start_offset of four.
            &[64, 0, 0, 0, 0, 0, 0, 0, 1024][..],
        ][..],
    ];

    for (start_offset, expected) in expected.iter().enumerate() {
        for (i, testdata) in expected.iter().enumerate() {
            let total_chunk_count = (i + 1 + start_offset) as u64;

            let mut total_offset = 0;
            for (node_number, node_encoding_size) in testdata.iter().enumerate() {
                // println!(
                //     "start_offset {start_offset} total_chunk_count {total_chunk_count} node_number {node_number}\n"
                // );

                assert_eq!(
                    node_number_to_byte_index::<32, 1024>(
                        node_number as NodeNumber,
                        total_chunk_count,
                        start_offset as u64,
                    ),
                    total_offset,
                    "start_offset: {start_offset}, total_chunk_count: {total_chunk_count}, node_number: {node_number}",
                );

                total_offset += *node_encoding_size;
            }
        }
    }
}

pub(crate) fn chunk_count_to_tree_height(chunk_count: ChunkCount) -> RootedPathLength {
    1 + chunk_count.next_power_of_two().ilog2() as u8
}

#[test]
fn test_tree_height_computation() {
    for (i, height) in [1, 2, 3, 3, 4].into_iter().enumerate() {
        let chunk_count = i + 1;
        assert_eq!(chunk_count_to_tree_height(chunk_count as u64), height);
    }
}

// /// Given a total string length, returns how many bytes long the chunk at the given index is.
// ///
// /// Returns `0` only if `string_length_in_bytes` is zero, otherwise returns a value between one and `CHUNK_SIZE`, both inclusive.
// pub(crate) fn chunk_to_length<const CHUNK_SIZE: usize>(
//     chunk_index: ChunkIndex,
//     string_length_in_bytes: ByteCount,
// ) -> ByteCount {
//     if string_length_in_bytes == 0 {
//         0
//     } else {
//         let total_number_of_chunks = string_length_in_bytes.div_ceil(CHUNK_SIZE as u64);
//         if chunk_index < total_number_of_chunks - 1 {
//             CHUNK_SIZE as u64
//         } else {
//             let tmp = string_length_in_bytes % (CHUNK_SIZE as u64);
//             if tmp == 0 { CHUNK_SIZE as u64 } else { tmp }
//         }
//     }
// }

// #[test]
// fn test_chunk_to_length() {
//     assert_eq!(chunk_to_length::<4>(0, 0), 0);
//     assert_eq!(chunk_to_length::<4>(0, 1), 1);
//     assert_eq!(chunk_to_length::<4>(0, 2), 2);
//     assert_eq!(chunk_to_length::<4>(0, 3), 3);
//     assert_eq!(chunk_to_length::<4>(0, 4), 4);

//     assert_eq!(chunk_to_length::<4>(0, 5), 4);
//     assert_eq!(chunk_to_length::<4>(1, 5), 1);
//     assert_eq!(chunk_to_length::<4>(0, 6), 4);
//     assert_eq!(chunk_to_length::<4>(1, 6), 2);
//     assert_eq!(chunk_to_length::<4>(0, 7), 4);
//     assert_eq!(chunk_to_length::<4>(1, 7), 3);
//     assert_eq!(chunk_to_length::<4>(0, 8), 4);
//     assert_eq!(chunk_to_length::<4>(1, 8), 4);

//     assert_eq!(chunk_to_length::<4>(0, 9), 4);
//     assert_eq!(chunk_to_length::<4>(1, 9), 4);
//     assert_eq!(chunk_to_length::<4>(2, 9), 1);
//     assert_eq!(chunk_to_length::<4>(0, 10), 4);
//     assert_eq!(chunk_to_length::<4>(1, 10), 4);
//     assert_eq!(chunk_to_length::<4>(2, 10), 2);
//     assert_eq!(chunk_to_length::<4>(0, 11), 4);
//     assert_eq!(chunk_to_length::<4>(1, 11), 4);
//     assert_eq!(chunk_to_length::<4>(2, 11), 3);
//     assert_eq!(chunk_to_length::<4>(0, 12), 4);
//     assert_eq!(chunk_to_length::<4>(1, 12), 4);
//     assert_eq!(chunk_to_length::<4>(2, 12), 4);
// }

/// Given a total string length, returns how many bytes the node of the given node number covers.
pub(crate) fn node_number_to_length<const CHUNK_SIZE: usize>(
    node_number: NodeNumber,
    string_length_in_bytes: ByteCount,
) -> ByteCount {
    if node_number == 0 {
        // Trivial case: the root covers the full string.
        return string_length_in_bytes;
    } else if string_length_in_bytes <= CHUNK_SIZE as u64 {
        // Another trivial case: if all bytes fit into a single chunk, the chunk node covers all bytes.
        return string_length_in_bytes;
    } else {
        let chunk_count = string_length_in_bytes.div_ceil(CHUNK_SIZE as u64);

        let left_subtree_node_count = chunk_count.next_power_of_two() - 1;
        let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

        if node_number <= left_subtree_node_count {
            // node_number is in the left subtree.
            return node_number_to_length::<CHUNK_SIZE>(
                node_number - 1,
                left_subtree_leaf_count * (CHUNK_SIZE as u64),
            );
        } else {
            return node_number_to_length::<CHUNK_SIZE>(
                node_number - (left_subtree_node_count + 1),
                string_length_in_bytes - (left_subtree_leaf_count * (CHUNK_SIZE as u64)),
            );
        }
    }
}

#[test]
fn test_node_number_to_length() {
    assert_eq!(node_number_to_length::<4>(0, 0), 0);
    assert_eq!(node_number_to_length::<4>(0, 1), 1);
    assert_eq!(node_number_to_length::<4>(0, 2), 2);
    assert_eq!(node_number_to_length::<4>(0, 3), 3);
    assert_eq!(node_number_to_length::<4>(0, 4), 4);

    assert_eq!(node_number_to_length::<4>(0, 11), 11);
    assert_eq!(node_number_to_length::<4>(1, 11), 8);
    assert_eq!(node_number_to_length::<4>(2, 11), 4);
    assert_eq!(node_number_to_length::<4>(3, 11), 4);
    assert_eq!(node_number_to_length::<4>(4, 11), 3);

    assert_eq!(node_number_to_length::<4>(0, 16), 16);
    assert_eq!(node_number_to_length::<4>(1, 16), 8);
    assert_eq!(node_number_to_length::<4>(2, 16), 4);
    assert_eq!(node_number_to_length::<4>(3, 16), 4);
    assert_eq!(node_number_to_length::<4>(4, 16), 8);
    assert_eq!(node_number_to_length::<4>(5, 16), 4);
    assert_eq!(node_number_to_length::<4>(6, 16), 4);
}

/// Returns at least as many bytes as the length of the [baseline verifiable slice stream](https://bab-hash.org/spec#baseline_slice) for the given slice.
pub(crate) fn approximate_length_of_verifiable_stream<
    const WIDTH: usize,
    const CHUNK_SIZE: usize,
>(
    slice_start: ChunkIndex,
    slice_length: ChunkCount,
    string_length: ByteCount,
) -> ByteCount {
    let chunk_count = string_length_to_chunk_count::<CHUNK_SIZE>(string_length);

    let length_of_the_chunks = if slice_start + slice_length == chunk_count {
        let len_of_final_chunk = if string_length % (CHUNK_SIZE as u64) == 0 {
            CHUNK_SIZE as u64
        } else {
            string_length % (CHUNK_SIZE as u64)
        };
        (CHUNK_SIZE as ByteCount) * (slice_length - 1) + len_of_final_chunk
    } else {
        (CHUNK_SIZE as ByteCount) * slice_length
    };

    if chunk_count == 1 {
        return length_of_the_chunks;
    } else {
        let num_vertices_in_tree_on_slice_length_many_leaves = slice_length * 2 - 1;

        let height_of_tree_on_slice_length_many_leaves =
            chunk_count_to_tree_height(slice_length) as u64;
        let total_height = chunk_count_to_tree_height(chunk_count) as u64;

        return
            // The chunks themselves.
            length_of_the_chunks
            // The subtree for the slice. (this might actually be multiple disjoint trees, but the counting stays the same).
            + num_vertices_in_tree_on_slice_length_many_leaves * (WIDTH as ByteCount) * 2
            // Paths from the root to the subtree(s) for the slice. If the two paths share the same vertices, this part overcounts.
            + (total_height - height_of_tree_on_slice_length_many_leaves) * (WIDTH as ByteCount) * 2 * 2
            // The root label itself is not part of the verifiable stream.
            - (WIDTH as ByteCount);
    }
}
