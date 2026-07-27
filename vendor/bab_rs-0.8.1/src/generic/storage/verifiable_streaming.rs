//! Types used in (optimised) [verifiable slice streaming](https://bab-hash.org/spec#slice_streaming).
//!
//! This module exposes the [`SliceStreamingOptions`] struct for expressing the optimisations to apply when creating or ingesting verifiable slice streams. The module further provides some helpful type aliases that clarify the roles in which [`u64`]s appear in the Bab spec: as a byte index or chunk index of a string, or a s a quantity of bytes or chunks, etc. And finally, the module exposes some common error types around slice streaming.
//!
//! In short, here you find those types that can be reused across the APIs of different Bab stores.

use core::num::NonZeroU8;

#[cfg(feature = "dev")]
use arbitrary::Arbitrary;

use ufotofu::{ConsumeAtLeastError, ProduceAtLeastError, prelude::*};

use crate::generic::{
    BabInstantiation,
    storage::{
        storage_backend::{OperationsError, StorageBackend, StringInfo, node_number_to_length},
        units::*,
    },
};

/// Optimisation options around slice streaming. Specifically, this combines
///
/// - which level of [k-grouping](https://bab-hash.org/spec#kgrouped) to apply,
/// - how to filter by [layer](https://bab-hash.org/spec#layer),
/// - the [left_skip](https://bab-hash.org/spec#left_skip), and
/// - the [right_skip](https://bab-hash.org/spec#right_skip).
///
/// Typically you would use [`Default::default`] to obtain the options that do not optimise away anything, and adjust from there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub struct SliceStreamingOptions {
    /// The level of [k-grouping](https://bab-hash.org/spec#kgrouped).
    pub k: RootedPathLength,
    /// The `layer_filter_target` and `layer_filter_factor` fields determine which left labels to skip: a label of a left child node is included iff its [layer](https://bab-hash.org/spec#layer) modulo `layer_filter_factor` is equal to `layer_filter_target`.
    ///
    /// To produce a [baseline stream](https://bab-hash.org/spec#baseline_slice), set the `layer_filter_target` to `0` and the `layer_filter_factor` to `1`. The [`Default`] has this setting.
    ///
    /// To produce a [light stream](https://bab-hash.org/spec#light), use the [`SliceStreamingOptions::set_light`] method (which takes into account the level of [k-grouping](https://bab-hash.org/spec#kgrouped), configuring things for the [k-grouped light stream](https://bab-hash.org/spec#kgrouped) automatically).
    pub layer_filter_target: RootedPathLength,
    /// See the documentation for [`SliceStreamingOptions::layer_filter_target`] for how this field works.
    pub layer_filter_factor: NonZeroU8,
    /// The [left_skip](https://bab-hash.org/spec#left_skip).
    pub left_skip: RootedPathLength,
    /// The [right_skip](https://bab-hash.org/spec#right_skip).
    pub right_skip: RootedPathLength,
}

impl SliceStreamingOptions {
    /// Sets `self.layer_filter` to the appropriate value for the [k-grouped light stream](https://bab-hash.org/spec#kgrouped) for a bab instantiation with a [width](https://bab-hash.org/spec#width) and [chunk size](https://bab-hash.org/spec#chunk_size) of `WIDTH` and `CHUNK_SIZE` respectively.
    ///
    /// Note that this method is based on the value of `self.k` at the moment of calling the method. If you change `self.k` later, the resulting configuration will not represent the [k-grouped light stream](https://bab-hash.org/spec#kgrouped) anymore.
    pub fn set_light<const WIDTH: usize, const CHUNK_SIZE: usize>(&mut self) {
        // Skip everything. This is a special case because the bitshift in the regular computation would overflow.
        if self.k < 64 {
            match (1u64 << self.k).checked_mul(CHUNK_SIZE.div_ceil(WIDTH) as u64) {
                None => {
                    // Everything will be skipped anyway, irrespective of the layer_filter.
                    // We do not need to do anything here.
                }
                Some(factor) => {
                    self.layer_filter_factor = (factor as u8).try_into().unwrap();
                    self.layer_filter_target = (self.k + 1) % self.layer_filter_factor;
                }
            }
        } else {
            // For k >= 64, everything will be skipped anyway, irrespective of the layer_filter.
            // We do not need to do anything here, this special case only exists to prevent a bitshift overflow in the regular computation.
        }
    }
}

/// The default [`SliceStreamingOptions`] are those describing a [baseline stream](https://bab-hash.org/spec#baseline_slice) without [k-grouping](https://bab-hash.org/spec#kgrouped) (i.e., `k == 0`) and a [left_skip](https://bab-hash.org/spec#left_skip) and [right_skip](https://bab-hash.org/spec#right_skip) of zero.
impl Default for SliceStreamingOptions {
    fn default() -> Self {
        Self {
            layer_filter_target: 0,
            layer_filter_factor: 1.try_into().unwrap(),
            k: 0,
            left_skip: 0,
            right_skip: 0,
        }
    }
}

/// Writes a verifiable slice stream into the given bulk consumer of bytes `c`, for the slice of `len` many chunks, starting at chunk index `from`.
///
/// `len` must not be zero, and `from + len` must not exceed 2^64 - 1.
///
/// All data is read from the given `byte_storage`, which is assumed to store data for a string on `chunk_code` many chunks, beginning at chunk offset `start_offset`. See the documentation of the [`ByteStorage`] trait for more details on these two parameters.
///
/// The optimisations to apply (i.e., which data to omit from the [baseline verifiable slice stream](https://bab-hash.org/spec#baseline_slice)) are specified by the given [SliceStreamingOptions].
///
/// Returns how many bytes were written.
pub(crate) async fn produce_slice_stream<const WIDTH: usize, const CHUNK_SIZE: usize, Storage, C>(
    from: ChunkIndex,
    len: ChunkCount,
    byte_storage: &mut Storage,
    string_info: StringInfo,
    string_len: ByteCount,
    options: SliceStreamingOptions,
    c: &mut C,
) -> Result<ByteCount, EmitSliceStreamError<C::Error, Storage::InternalError>>
where
    C: BulkConsumer<Item = u8>,
    Storage: StorageBackend,
{
    let mut written_bytes = 0;

    // This iterator tells us exactly which data to query for and to write into the consumer, taking all requested optimisations into account.
    let slice_progress_iterator = SliceProgressIterator::new(from, len, string_info, options);

    for progress in slice_progress_iterator {
        match progress {
            SliceProgress::Inner {
                node_number,
                layer: _,
                skip,
                omit_left,
                omit_right,
                is_left_child_part_of_stream: _,
                is_right_child_part_of_stream: _,
            } => {
                if !skip {
                    let (mut left, mut right) = ([0; WIDTH], [0; WIDTH]);

                    byte_storage
                        .get_node_contents::<WIDTH, CHUNK_SIZE>(
                            node_number,
                            string_info,
                            &mut left,
                            &mut right,
                        )
                        .await?;

                    if !omit_left {
                        c.consume_full_slice(&left).await?;
                        written_bytes += WIDTH as ByteCount;
                    }

                    if !omit_right {
                        c.consume_full_slice(&right).await?;
                        written_bytes += WIDTH as ByteCount;
                    }
                }
            }

            SliceProgress::Chunk {
                chunk_index,
                node_number: _,
            } => {
                let mut buf = [0; CHUNK_SIZE];
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

                byte_storage
                    .get_chunk::<WIDTH, CHUNK_SIZE>(chunk_index, string_info, &mut buf[..chunk_len])
                    .await?;

                c.consume_full_slice(&buf[..chunk_len]).await?;
                written_bytes += chunk_len as ByteCount;
            }
        }
    }

    Ok(written_bytes)
}

/// Verifies and then writes data from a verifiable slice stream from the given bulk producer of bytes `p` into the given `byte_storage` — for the slice of `len` many chunks, starting at chunk index `from`.
///
/// `len` must not be zero, and `from + len` must not exceed 2^64 - 1.
///
/// All data is written to the given `byte_storage`, which is assumed to store data for a string on `chunk_code` many chunks, beginning at chunk offset `start_offset`. See the documentation of the [`ByteStorage`] trait for more details on these two parameters.
///
/// The optimisations to apply (i.e., which data are omitted from the [baseline verifiable slice stream](https://bab-hash.org/spec#baseline_slice)) are specified by the given [SliceStreamingOptions].
///
/// Returns the least node number that is part of the stream but has not been verified still — both when returning an `Ok` or an error (or `None` if the whole stream was successfully verified).
pub(crate) async fn consume_slice_stream<
    const WIDTH: usize,
    const CHUNK_SIZE: usize,
    Storage,
    P,
    HashChunkContext,
    HashInnerContext,
>(
    from: ChunkIndex,
    len: ChunkCount,
    byte_storage: &mut Storage,
    string_info: StringInfo,
    string_len_in_bytes: ByteCount,
    root_label: [u8; WIDTH],
    options: SliceStreamingOptions,
    p: &mut P,
    bab_instantiation: &BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
) -> Result<Option<NodeNumber>, IngestSliceStreamError<P::Error, Storage::InternalError>>
where
    P: BulkProducer<Item = u8>,
    Storage: StorageBackend,
{
    // See https://worm-blossom.org/notes/bab_verification for an explanation of how this works.
    // Note this implementation does not store the *currently reconstructed label* in a variable that is being persisted across SliceProgress steps (i.e., it is not defined outside the outer for loop), instead it is always kept on the top of the *verification stack* between iterations of the outer for loop.

    // How many bytes of string data remain to still be read.
    let initially_skipped_chunk_bytes = from.saturating_mul(CHUNK_SIZE as u64);
    let mut remaining_chunk_bytes =
        string_len_in_bytes.saturating_sub(initially_skipped_chunk_bytes);

    // Here we store all data we have received but could not verify yet.
    // Flushed into the byte_storage whenever we are able to verify a label.
    let mut unverified_data = vec![];

    let mut pending_verifications_stack = vec![LabelToVerify::Verbatim(root_label)];

    let mut reconstruction_stack: Vec<ReconstructionStep<WIDTH>> = vec![];

    let mut slice_progress_iterator =
        SliceProgressIterator::new(from, len, string_info, options).peekable();

    let mut next_node = Some(0);

    while let Some(progress) = slice_progress_iterator.next() {
        // println!(
        //     "\nprogress {progress:?}\npending_verifications_stack {pending_verifications_stack:?}"
        // );
        debug_assert!(
            !pending_verifications_stack.is_empty(),
            "Verification stack must only be empty after processing the final chunk. {pending_verifications_stack:?}"
        );

        // Buffers for holding the data we might read from the stream.
        let mut left_label = [0u8; WIDTH];
        let mut right_label = [0u8; WIDTH];
        let mut chunk_data = [0u8; CHUNK_SIZE];

        // Save the number of remaining_chunk_bytes *before* subtracting from it in read_data, because we later need the value from before the subtraction.
        let current_remaining_chunk_bytes = remaining_chunk_bytes;

        // Fill the buffers, as dictated by the current SliceProgress.
        progress
            .read_data(
                &mut left_label,
                &mut right_label,
                &mut chunk_data,
                &mut remaining_chunk_bytes,
                &mut unverified_data,
                p,
            )
            .await
            .map_err(
                |err: ProduceAtLeastError<P::Final, P::Error>| match err.reason {
                    Ok(_fin) => IngestSliceStreamError::UnexpectedEndOfStream {
                        next_node: next_node.expect("Never None in case of an error."),
                    },
                    Err(producer_err) => IngestSliceStreamError::ProducerError {
                        next_node: next_node.expect("Never None in case of an error."),
                        producer_err,
                    },
                },
            )?;
        // println!("left_label {left_label:?}, right_label {right_label:?}");

        // This variable corresponds to the *currently reconstructed label* of the [writeup](https://worm-blossom.org/notes/bab_verification).
        // If we cannot fully verify it with the information available in this iteration of the loop, we end by pushing it back onto the stack, so that the next loop iteration will have it available again.
        let next_verification_step = pending_verifications_stack.pop().unwrap();
        // (We can unwrap because the pending_verifications_stack is empty only after processing the very last SliceProgress.)

        // Irrespective of what else happens, we need to push non-omitted child labels onto the pending_verifications_stack (unless the node content of that child node is not part of the stream at all).

        // Push the label of the right child onto the pending verifications stack, then same for the label of the left child; skipping any that are not part of the stream at all. Does nothing if `progress` corresponds to a chunk.
        progress.push_labels::<WIDTH, CHUNK_SIZE>(
            &mut pending_verifications_stack,
            &left_label,
            &right_label,
        );
        // println!("after progress.push_labels: {pending_verifications_stack:?}");

        if progress.is_skipped() {
            // When the current SliceProgress is skipped, all we need to do is update the `next_node`.
            next_node = slice_progress_iterator
                .peek()
                .map(|next| next.node_number());
        } else {
            // Next we reconstruct labels until we have to stop (either because we cleared the reconstruction_stack, or because we realise we are unable to clear it).

            // The first label reconstruction, based on the data we just read from `p`, works differently than later steps, so we do that now, outside the loop that follows.

            let mut reconstructed = [0u8; WIDTH];

            if progress.reconstruct_label::<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>(
                &left_label,
                &right_label,
                &chunk_data[..core::cmp::min(CHUNK_SIZE, current_remaining_chunk_bytes as usize)],
                &mut reconstructed,
                bab_instantiation,
                string_len_in_bytes,
            ) {
                // It worked, we reconstructed a label. We may have to reconstruct further labels, so we go into a loop.
                // (There are two possibilities for breaking from the loop: either we empty the reconstruction_stack and can perform actual verification, or we realise we need more data before we can empty the reconstruction_stack).

                loop {
                    // Let's figure out what to do with the most recently reconstructed label.
                    match reconstruction_stack.pop() {
                        None => {
                            // No further reconstructions necessary. We can compare against the `next_verification_step`.

                            // Retrieve the expected label from the byte storage, if necessary.
                            let expected = next_verification_step
                                .to_label::<CHUNK_SIZE, _>(byte_storage, string_info)
                                .await
                                .map_err(|storage_error| {
                                    IngestSliceStreamError::StorageBackendError {
                                        next_node: next_node
                                            .expect("Never None in case of an error."),
                                        storage_error,
                                    }
                                })?;

                            if reconstructed == expected {
                                // Passed verification. Yay!

                                // We can write all unverified data we have buffered into the byte storage.
                                for buffered_data in unverified_data.drain(..) {
                                    match buffered_data {
                                        DataToFlushOnceVerified::InnerNodeContent {
                                            left,
                                            right,
                                            node_number,
                                        } => {
                                            byte_storage
                                                .set_inner_node_contents::<WIDTH, CHUNK_SIZE>(
                                                    node_number,
                                                    string_info,
                                                    &left,
                                                    &right,
                                                )
                                                .await
                                                .map_err(|storage_error| {
                                                    IngestSliceStreamError::StorageBackendError {
                                                        next_node: next_node.expect(
                                                            "Never None in case of an error.",
                                                        ),
                                                        storage_error,
                                                    }
                                                })?;
                                        }
                                        DataToFlushOnceVerified::Chunk {
                                            data,
                                            data_len,
                                            chunk_index,
                                        } => {
                                            byte_storage
                                                .set_chunk::<WIDTH, CHUNK_SIZE>(
                                                    chunk_index,
                                                    string_info,
                                                    &data[..data_len],
                                                )
                                                .await
                                                .map_err(|storage_error| {
                                                    IngestSliceStreamError::StorageBackendError {
                                                        next_node: next_node.expect(
                                                            "Never None in case of an error.",
                                                        ),
                                                        storage_error,
                                                    }
                                                })?;
                                        }
                                    }
                                }

                                // We verified the current SliceProgress.
                                next_node = slice_progress_iterator
                                    .peek()
                                    .map(|next| next.node_number());

                                // And that is it. Moving on to the next SliceProgress.
                                break; // Breaks out of the loop for clearing the reconstruction_stack. In the outer for loop, no further work remains, so we then proceed to the next SliceProgress.
                            } else {
                                // We were given invalid data. Boo!
                                return Err(IngestSliceStreamError::VerificationError {
                                    next_node: next_node.expect("Never None in case of an error."),
                                });
                            }
                        }
                        Some(mut current_reconstruction_step) => {
                            // We reconstructed a label, but the reconstruction_stack is not empty yet.

                            // We nevertheless update the `unverified_data` with the reconstructed label.
                            match unverified_data
                                .get_mut(current_reconstruction_step.unverified_data_position)
                                .unwrap()
                            {
                                DataToFlushOnceVerified::InnerNodeContent {
                                    left,
                                    right,
                                    node_number: _,
                                } => {
                                    if current_reconstruction_step.left.is_none() {
                                        *left = reconstructed;
                                    } else {
                                        *right = reconstructed;
                                    }
                                }
                                DataToFlushOnceVerified::Chunk { .. } => unreachable!(
                                    "If we set the unverified_data_position correctly, it refers to an inner node content, not a chunk."
                                ),
                            }

                            match (
                                current_reconstruction_step.left,
                                current_reconstruction_step.right,
                            ) {
                                (None, None) => {
                                    // Both labels of the current reconstruction step were omitted.
                                    // We cannot progress reconstruction beyond reconstructing the left label
                                    // and pushing the incomplete step back onto the reconstruction_stack.
                                    current_reconstruction_step.left = Some(reconstructed);
                                    reconstruction_stack.push(current_reconstruction_step);

                                    // Since we failed to verify the `next_verification_step`, we push it back onto the verification_stack.
                                    pending_verifications_stack.push(next_verification_step);

                                    // Continue to the next SliceProgress.
                                    break;
                                }
                                (Some(_), Some(_)) => unreachable!(
                                    "We never push an already-verifiable reconstruction step onto the reconstruction_stack.",
                                ),
                                (Some(new_left), None) => {
                                    left_label = new_left;
                                    right_label = reconstructed;
                                }
                                (None, Some(new_right)) => {
                                    left_label = reconstructed;
                                    right_label = new_right;
                                }
                            }

                            // We have overwritten left_label and right_label.

                            // Now, we can perform the next label reconstruction, based on the updated left_label and right_label.
                            (bab_instantiation.hash_inner)(
                                &left_label,
                                &right_label,
                                current_reconstruction_step.summarised_length,
                                current_reconstruction_step.is_root,
                                &bab_instantiation.hash_inner_context,
                                &mut reconstructed,
                            );

                            // That's it, we now go to the next iteration of the loop, to determine what to do with the just-reconstructed label.
                        }
                    }
                }
            } else {
                // We could not reconstruct a label with the current data (i.e., the current SliceProgress is for an inner vertex and and least one child is omitted).

                // The next iteration of the outer for loop should continue verifying the same label as the current iteration.
                pending_verifications_stack.push(next_verification_step);

                // Push the work we could not perform at this point onto the reconstruction_stack.
                if let SliceProgress::Inner {
                    node_number,
                    layer: _,
                    skip: _,
                    omit_left,
                    omit_right,
                    is_left_child_part_of_stream: _,
                    is_right_child_part_of_stream: _,
                } = progress
                {
                    debug_assert!(
                        omit_left || omit_right,
                        "If neither child was omitted, label reconstruction would have succeeded in the first place."
                    );

                    reconstruction_stack.push(ReconstructionStep {
                        left: if omit_left { None } else { Some(left_label) },
                        right: if omit_right { None } else { Some(right_label) },
                        summarised_length: node_number_to_length::<CHUNK_SIZE>(
                            node_number,
                            string_len_in_bytes,
                        ),
                        is_root: node_number == 0,
                        unverified_data_position: unverified_data.len() - 1,
                    });
                } else {
                    unreachable!(
                        "Label reconstruction never fails for chunks, only for inner nodes."
                    );
                }

                // We failed to reconstruct a label, and we recorded all state to correctly deal with this failure once more data becomes available. We simply continue with the next iteration of the for loop.
            }
        }
    }

    // If we leave the `for progress in slice_progress_iterator`, then we have successfully verified the full stream.
    return Ok(next_node);
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum LabelToVerify<const WIDTH: usize> {
    Verbatim([u8; WIDTH]),
    Lookup {
        /// The inner node whose node content to look up.
        node: NodeNumber,
        /// Whether to use the label of the left child or not (in which case to use the label of the right child).
        use_left_child_label: bool,
    },
}

impl<const WIDTH: usize> LabelToVerify<WIDTH> {
    async fn to_label<const CHUNK_SIZE: usize, Storage>(
        &self,
        byte_storage: &mut Storage,
        string_info: StringInfo,
    ) -> Result<[u8; WIDTH], OperationsError<Storage::InternalError>>
    where
        Storage: StorageBackend,
    {
        match self {
            LabelToVerify::Verbatim(label) => Ok(*label),
            LabelToVerify::Lookup {
                node,
                use_left_child_label,
            } => {
                let mut left = [0u8; WIDTH];
                let mut right = [0u8; WIDTH];

                byte_storage
                    .get_node_contents::<WIDTH, CHUNK_SIZE>(
                        *node,
                        string_info,
                        &mut left,
                        &mut right,
                    )
                    .await?;

                if *use_left_child_label {
                    Ok(left)
                } else {
                    Ok(right)
                }
            }
        }
    }
}

/// A record of a verification step that could not be completed because of omitted data. When one of these is pushed onto the `reconstruction-step`, either or even both of `left` and `right` is `None`.
// #[derive(Clone, Copy)]
struct ReconstructionStep<const WIDTH: usize> {
    left: Option<[u8; WIDTH]>,
    right: Option<[u8; WIDTH]>,
    /// The sum of the lengths in bytes of all chunks in this subtree (note that the final chunk of the string can be non-full, which is why we track this explicitly).
    summarised_length: ByteCount,
    is_root: bool,
    /// The offset in the `unverified_data` vec at which the label is stored. We use this offset to overwrite the corresponding location in the `unverified_data` once we reconstruct the label.
    unverified_data_position: usize,
}

/// Data we could not verify yet but will write to a ByteStorage once we could verify it.
pub(crate) enum DataToFlushOnceVerified<const WIDTH: usize, const CHUNK_SIZE: usize> {
    InnerNodeContent {
        left: [u8; WIDTH],
        right: [u8; WIDTH],
        node_number: NodeNumber,
    },
    Chunk {
        data: [u8; CHUNK_SIZE],
        /// At most CHUNK_SIZE, possibly less for the final chunk.
        data_len: usize,
        chunk_index: ChunkIndex,
    },
}

/// Initialises a ByteStorage from the raw bytes of a string (supplied as a producer of bytes; the total length must be known in advance).
/// Returns the root label, i.e., the Bab digest, of the string.
pub(crate) async fn initialise_store<
    const WIDTH: usize,
    const CHUNK_SIZE: usize,
    Storage,
    P,
    HashChunkContext,
    HashInnerContext,
>(
    storage: &mut Storage,
    length_in_bytes: ByteCount,
    start_offset: ChunkIndex,
    string_bytes: &mut P,
    bab_instantiation: &BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
) -> Result<[u8; WIDTH], OperationsError<Storage::InternalError>>
where
    P: BulkProducer<Item = u8>,
    Storage: StorageBackend,
{
    let chunk_count = string_length_to_chunk_count::<CHUNK_SIZE>(length_in_bytes);

    let string_info = StringInfo {
        chunk_count,
        start_offset,
    };

    let slice_progress_iterator = SliceProgressIterator::new(
        0,
        chunk_count,
        string_info,
        SliceStreamingOptions::default(),
    );

    let mut bytes_so_far = 0;
    let mut stack: Vec<InitialiseStoreStackframe<WIDTH>> = vec![];

    for progress in slice_progress_iterator {
        match progress {
            SliceProgress::Inner {
                node_number, layer, ..
            } => {
                stack.push(InitialiseStoreStackframe {
                    node_number,
                    layer,
                    left_label: None,
                });
            }
            SliceProgress::Chunk {
                chunk_index,
                node_number,
            } => {
                let chunk_len =
                    core::cmp::min(CHUNK_SIZE, (length_in_bytes - bytes_so_far) as usize);
                let mut chunk_data = [0u8; CHUNK_SIZE];

                if let Err(_) = string_bytes
                    .bulk_overwrite_full_slice(&mut chunk_data[..chunk_len])
                    .await
                {
                    panic!("producer must emit the expected number of bytes");
                }

                bytes_so_far += chunk_len as u64;

                // println!(
                //     "chunk_index {chunk_index}, string_info {string_info:?}, chunk_data {chunk_data:?}, chunk_len {chunk_len:?}"
                // );

                storage
                    .set_chunk::<WIDTH, CHUNK_SIZE>(
                        chunk_index,
                        string_info,
                        &chunk_data[..chunk_len],
                    )
                    .await?;

                let mut digest = [0u8; WIDTH];
                (bab_instantiation.hash_chunk)(
                    &chunk_data[..chunk_len],
                    node_number == 0,
                    &bab_instantiation.hash_chunk_context,
                    &mut digest,
                );

                loop {
                    match stack.pop() {
                        None => return Ok(digest),
                        Some(mut stackframe) => {
                            let node_number = stackframe.node_number;

                            match stackframe.left_label {
                                None => {
                                    // We have not buffered a left label yet, so the current `digest` cannot be used in computing further labels.
                                    // Instead, we re-push the stackgrame, with the current `digest` becoming the left label.
                                    stackframe.left_label = Some(digest);
                                    stack.push(stackframe);

                                    // And now we go to the next iteration of the outer for loop.
                                    break;
                                }
                                Some(left_label) => {
                                    // We can reconstruct an inner label, by combining the current `digest` (as a right label) with the `left_label`.
                                    let right_label = digest.clone();

                                    (bab_instantiation.hash_inner)(
                                        &left_label,
                                        &right_label,
                                        node_number_to_length::<CHUNK_SIZE>(
                                            node_number,
                                            length_in_bytes,
                                        ),
                                        node_number == 0,
                                        &bab_instantiation.hash_inner_context,
                                        &mut digest,
                                    );

                                    // Write the reconstructed labels to storage.
                                    storage
                                        .set_inner_node_contents::<WIDTH, CHUNK_SIZE>(
                                            node_number,
                                            string_info,
                                            &left_label,
                                            &right_label,
                                        )
                                        .await?;

                                    // And go to the next iteration of the inner loop, i.e., pop the next piece of data from the stack to check whether we can reconstruct even more labels.
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // The slice_progress_iterator always emits at least one `Some` value, so we always enter the for loop. Within it, we `return` after trying to pop from an empty stack, which happens exactly in the for-loop iteration corresponding to the final `Some` value of the slice_progress_iterator.
    unreachable!();
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
struct InitialiseStoreStackframe<const WIDTH: usize> {
    node_number: NodeNumber,
    layer: Layer,
    left_label: Option<[u8; WIDTH]>,
}

/// Everything that can go wrong when emitting a verifiable slice stream from a Bab storage.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Debug)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub enum EmitSliceStreamError<ConsumerError, ByteStorageInternalError> {
    /// The [`BulkConsumer`] receiving the bytes of the verifiable slice stream emitted an error.
    ConsumerError(ConsumerError),
    /// The underlying [`StorageBackend`] yielded an [`OperationsError`] upon attempting to retrieve some bytes.
    StorageBackendError(OperationsError<ByteStorageInternalError>),
}

impl<ConsumerError, ByteStorageInternalError> From<ConsumeAtLeastError<ConsumerError>>
    for EmitSliceStreamError<ConsumerError, ByteStorageInternalError>
{
    fn from(value: ConsumeAtLeastError<ConsumerError>) -> Self {
        EmitSliceStreamError::ConsumerError(value.into_reason())
    }
}

impl<ConsumerError, ByteStorageInternalError> From<OperationsError<ByteStorageInternalError>>
    for EmitSliceStreamError<ConsumerError, ByteStorageInternalError>
{
    fn from(value: OperationsError<ByteStorageInternalError>) -> Self {
        EmitSliceStreamError::StorageBackendError(value)
    }
}

/// Everything that can go wrong when ingesting a verifiable slice stream into a Bab storage.
///
/// All variants have a `next_node` field indicating how much data was successfully verified and ingested (though not yet necessarily [flushed](StorageBackend::flush)) before the error happened — use this value to determine the next stream to request in order to resume ingestion where it left off. The `next_node` always indicates the *least* node number whose content is part of the requested stream but that has not been ingested yet. (Note that it is impossible to indicate that *everything* was ingested successfully, but that is by design: in those cases, there is no error to report in the first place.)
///
/// To access the `next_node` field irrespective of the variant, use the [`IngestSliceStreamError::next_node`] method.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Debug)]
#[cfg_attr(feature = "dev", derive(Arbitrary))]
pub enum IngestSliceStreamError<ProducerError, ByteStorageInternalError> {
    /// The [`BulkProducer`] supplying the bytes of the verifiable slice stream emitted an error.
    ProducerError {
        /// The least node number whose content is part of the requested stream but that has not been ingested yet.
        next_node: NodeNumber,
        /// The error emitted by the [`BulkProducer`].
        producer_err: ProducerError,
    },
    /// The [`BulkProducer`] supplying the bytes of the verifiable slice stream ended unexpectedly (for example by ending in the middle of a transmitting chunk).
    UnexpectedEndOfStream {
        /// The least node number whose content is part of the requested stream but that has not been ingested yet.
        next_node: NodeNumber,
    },
    /// The underlying [`StorageBackend`] yielded an [`OperationsError`] upon attempting to ingest some bytes.
    StorageBackendError {
        /// The least node number whose content is part of the requested stream but that has not been ingested yet.
        next_node: NodeNumber,
        /// The error emitted by the underlying [`StorageBackend`].
        storage_error: OperationsError<ByteStorageInternalError>,
    },
    /// The bytes we received failed verification, they did not constitue a valid slice stream. Shame on the entity that supplied these bytes!
    VerificationError {
        /// The least node number whose content is part of the requested stream but that has not been ingested yet.
        next_node: NodeNumber,
    },
}

impl<ProducerError, ByteStorageInternalError>
    IngestSliceStreamError<ProducerError, ByteStorageInternalError>
{
    /// Returns the least node number whose content is part of the requested stream but that has not been ingested yet.
    pub fn next_node(&self) -> NodeNumber {
        match self {
            IngestSliceStreamError::ProducerError {
                next_node,
                producer_err: _,
            } => *next_node,
            IngestSliceStreamError::UnexpectedEndOfStream { next_node } => *next_node,
            IngestSliceStreamError::StorageBackendError {
                next_node,
                storage_error: _,
            } => *next_node,
            IngestSliceStreamError::VerificationError { next_node } => *next_node,
        }
    }
}

// /// Computes the length of the path from the root to the chunk at index `i`, in a tree on `chunk_count` many vertices.
// fn chunk_depth(i: ChunkIndex, chunk_count: ChunkCount) -> RootedPathLength {
//     if chunk_count.next_power_of_two() == chunk_count {
//         // Perfect trees are trivial.
//         return chunk_count.ilog2() as RootedPathLength;
//     } else {
//         let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

//         if i < left_subtree_leaf_count {
//             // We are in a left subtree, which is again perfect, and hence trivial.
//             return (chunk_count.next_power_of_two()).ilog2() as RootedPathLength;
//         } else {
//             // In right subtree, take the chunk in the subtree and add one (for our current root).
//             return 1 + chunk_depth(
//                 i - left_subtree_leaf_count,
//                 chunk_count - left_subtree_leaf_count,
//             );
//         }
//     }
// }

// #[test]
// fn test_chunk_depth() {
//     let expected = [
//         // The i-th slice lists the depths for the leaves of the trees on i + 1 leaves.
//         &[0u64][..],
//         &[1, 1][..],
//         &[2, 2, 1][..],
//         &[2, 2, 2, 2][..],
//         &[3, 3, 3, 3, 1][..],
//         &[3, 3, 3, 3, 2, 2][..],
//         &[3, 3, 3, 3, 3, 3, 2][..],
//         &[3, 3, 3, 3, 3, 3, 3, 3][..],
//         &[4, 4, 4, 4, 4, 4, 4, 4, 1][..],
//     ];

//     for (i, testdata) in expected.iter().enumerate() {
//         for (j, depth) in testdata.iter().enumerate() {
//             assert_eq!(
//                 chunk_depth(j as u64, (i as u64) + 1),
//                 *depth as RootedPathLength
//             );
//         }
//     }
// }

/// Computes the layer of the node at node-number `node_number`, in a tree on `chunk_count` many vertices.
fn node_layer(node_number: NodeNumber, chunk_count: ChunkCount) -> RootedPathLength {
    if chunk_count == 1 {
        return 0;
    } else {
        let left_subtree_node_count = chunk_count.next_power_of_two() - 1;
        let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

        let recursive = if node_number <= left_subtree_node_count {
            // node_number is in the left subtree.
            node_layer(node_number.saturating_sub(1), left_subtree_leaf_count)
        } else {
            node_layer(
                node_number - (left_subtree_node_count + 1),
                chunk_count - left_subtree_leaf_count,
            )
        };

        if node_number == 0 {
            return 1 + recursive;
        } else {
            return recursive;
        }
    }
}

#[test]
fn test_node_node_layer() {
    let expected = [
        // The i-th slice lists the depths for the node numbers of the trees on i + 1 leaves.
        &[0u64][..],
        &[1, 0, 0][..],
        &[2, 1, 0, 0, 0][..],
        &[2, 1, 0, 0, 1, 0, 0][..],
        &[3, 2, 1, 0, 0, 1, 0, 0, 0][..],
        &[3, 2, 1, 0, 0, 1, 0, 0, 1, 0, 0][..],
        &[3, 2, 1, 0, 0, 1, 0, 0, 2, 1, 0, 0, 0][..],
        &[3, 2, 1, 0, 0, 1, 0, 0, 2, 1, 0, 0, 1, 0, 0][..],
        &[4, 3, 2, 1, 0, 0, 1, 0, 0, 2, 1, 0, 0, 1, 0, 0, 0][..],
    ];

    for (i, testdata) in expected.iter().enumerate() {
        for (node_number, depth) in testdata.iter().enumerate() {
            // println!("total_chunk_count {} node_number {node_number}\n", i + 1,);

            assert_eq!(
                node_layer(node_number as u64, (i as u64) + 1),
                *depth as RootedPathLength
            );
        }
    }
}

/// An iterator emitting the node numbers and layers along the path from the root to some chunk, excluding the root itself (which is known to always have node number zero).
#[derive(Clone, Debug)]
struct PathToLeaf {
    // `chunk_index` and `chunk_count` start out as the original values specified in `Self::new`, but they shrink as we do recursive stuff.
    chunk_index: ChunkIndex,
    chunk_count: ChunkCount,
    // The current NodeCount (initialised to zero, i.e., we start at the root).
    node_number: NodeNumber,
}

impl PathToLeaf {
    /// `chunk_index`: the index of the chunk/leaf to go to.
    /// `chunk_count`: the total number of chunks/leaves in the tree.
    fn new(chunk_index: ChunkIndex, chunk_count: ChunkCount) -> Self {
        Self {
            chunk_index,
            chunk_count,
            node_number: 0,
        }
    }
}

impl Iterator for PathToLeaf {
    type Item = NodeNumber;

    fn next(&mut self) -> Option<Self::Item> {
        if self.chunk_count == 1 {
            return None;
        } else {
            let left_subtree_leaf_count = self.chunk_count.next_power_of_two() / 2;

            if self.chunk_index < left_subtree_leaf_count {
                // Chunk is in left subtree of the root.
                self.node_number += 1; // left child of current node_number
                self.chunk_count = left_subtree_leaf_count;

                return Some(self.node_number);
            } else {
                // chunk is in right subtree of the root
                self.node_number += self.chunk_count.next_power_of_two(); // number of nodes in left subtree + 1
                self.chunk_index -= left_subtree_leaf_count;
                self.chunk_count -= left_subtree_leaf_count;

                return Some(self.node_number);
            }
        }
    }
}

#[test]
fn test_path_to_leaf() {
    let expected = [
        // The i-th slice lists the node numbers along the path from the root to the i-th chunk (excluding the root itself), for a tree on seven vertices.
        &[1u64, 2, 3][..],
        &[1, 2, 4][..],
        &[1, 5, 6][..],
        &[1, 5, 7][..],
        &[8, 9, 10][..],
        &[8, 9, 11][..],
        &[8, 12][..],
    ];

    for (i, testdata) in expected.iter().enumerate() {
        let path_to_leaf = PathToLeaf::new(i as u64, 7);
        assert!(path_to_leaf.eq(testdata.iter().map(Clone::clone)));
    }
}

/// Information about which data to process next in optimised slice streaming, and to which degree to process it.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub(crate) enum SliceProgress {
    Chunk {
        chunk_index: ChunkIndex,
        node_number: NodeNumber,
    },
    Inner {
        node_number: NodeNumber,
        layer: RootedPathLength,
        /// `true` iff this node must be skipped completely because it falls within the `left_skip` or `right_skip` option.
        skip: bool,
        /// `true` iff light streaming or k-grouping warrants to not process the left child label in this node's node contents.
        omit_left: bool,
        /// `true` iff k-grouping warrants to not process the right child label in this node's node contents.
        omit_right: bool,
        /// `true` iff the left child of this node is part of the slice stream (i.e., on a path from the root to a chunk in the slice).
        is_left_child_part_of_stream: bool,
        /// `true` iff the right child of this node is part of the slice stream (i.e., on a path from the root to a chunk in the slice).
        is_right_child_part_of_stream: bool,
    },
}

impl SliceProgress {
    pub fn node_number(&self) -> NodeNumber {
        match self {
            SliceProgress::Chunk { node_number, .. } => *node_number,
            SliceProgress::Inner { node_number, .. } => *node_number,
        }
    }

    pub fn is_skipped(&self) -> bool {
        match self {
            Self::Chunk { .. } => false,
            SliceProgress::Inner { skip, .. } => *skip,
        }
    }

    pub async fn read_data<const WIDTH: usize, const CHUNK_SIZE: usize, P>(
        &self,
        left_label: &mut [u8; WIDTH],
        right_label: &mut [u8; WIDTH],
        chunk_data: &mut [u8; CHUNK_SIZE],
        remaining_chunk_bytes: &mut ByteCount,
        unverified_data: &mut Vec<DataToFlushOnceVerified<WIDTH, CHUNK_SIZE>>,
        p: &mut P,
    ) -> Result<(), ProduceAtLeastError<P::Final, P::Error>>
    where
        P: BulkProducer<Item = u8>,
    {
        match self {
            Self::Inner {
                node_number,
                layer: _,
                skip,
                omit_left,
                omit_right,
                is_left_child_part_of_stream: _,
                is_right_child_part_of_stream: _,
            } => {
                if !skip && !omit_left {
                    p.bulk_overwrite_full_slice(left_label).await?;
                }

                if !skip && !omit_right {
                    p.bulk_overwrite_full_slice(right_label).await?;
                }

                if !skip {
                    unverified_data.push(DataToFlushOnceVerified::InnerNodeContent {
                        left: *left_label,
                        right: *right_label,
                        node_number: *node_number,
                    });
                }

                Ok(())
            }

            Self::Chunk {
                chunk_index,
                node_number: _,
            } => {
                let len = core::cmp::min(CHUNK_SIZE, *remaining_chunk_bytes as usize);
                p.bulk_overwrite_full_slice(&mut chunk_data[..len]).await?;
                *remaining_chunk_bytes -= len as u64;

                unverified_data.push(DataToFlushOnceVerified::Chunk {
                    data: *chunk_data,
                    data_len: len,
                    chunk_index: *chunk_index,
                });

                Ok(())
            }
        }
    }

    pub fn push_labels<const WIDTH: usize, const CHUNK_SIZE: usize>(
        &self,
        pending_verifications_stack: &mut Vec<LabelToVerify<WIDTH>>,
        left_label: &[u8; WIDTH],
        right_label: &[u8; WIDTH],
    ) {
        match self {
            Self::Chunk { .. } => { /* no-op */ }
            SliceProgress::Inner {
                node_number,
                layer: _,
                skip,
                omit_left,
                omit_right,
                is_left_child_part_of_stream,
                is_right_child_part_of_stream,
            } => {
                if *is_right_child_part_of_stream {
                    if *skip {
                        pending_verifications_stack.push(LabelToVerify::Lookup {
                            node: *node_number,
                            use_left_child_label: false,
                        });
                    } else if !omit_right {
                        pending_verifications_stack.push(LabelToVerify::Verbatim(*right_label));
                    }
                }

                // And then the same for the left child. Pushing the left child second means it will be verified first.

                if *is_left_child_part_of_stream {
                    if *skip {
                        pending_verifications_stack.push(LabelToVerify::Lookup {
                            node: *node_number,
                            use_left_child_label: true,
                        });
                    } else if !omit_left {
                        pending_verifications_stack.push(LabelToVerify::Verbatim(*left_label));
                    }
                }
            }
        }
    }

    /// Reconstructs a label, writes into the `reconstructed_label`. Returns whether this worked, or not (because of omitted data).
    pub fn reconstruct_label<
        const WIDTH: usize,
        const CHUNK_SIZE: usize,
        HashChunkContext,
        HashInnerContext,
    >(
        &self,
        left_label: &[u8; WIDTH],
        right_label: &[u8; WIDTH],
        chunk_data: &[u8],
        reconstructed_label: &mut [u8; WIDTH],
        bab_instantiation: &BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
        string_length_in_bytes: ByteCount,
    ) -> bool {
        match self {
            SliceProgress::Chunk {
                chunk_index: _,
                node_number,
            } => {
                (bab_instantiation.hash_chunk)(
                    chunk_data,
                    *node_number == 0,
                    &bab_instantiation.hash_chunk_context,
                    reconstructed_label,
                );
                true
            }
            SliceProgress::Inner {
                node_number,
                layer: _,
                skip: _,
                omit_left,
                omit_right,
                is_left_child_part_of_stream: _,
                is_right_child_part_of_stream: _,
            } => {
                if !(*omit_left || *omit_right) {
                    (bab_instantiation.hash_inner)(
                        left_label,
                        right_label,
                        node_number_to_length::<CHUNK_SIZE>(*node_number, string_length_in_bytes),
                        *node_number == 0,
                        &bab_instantiation.hash_inner_context,
                        reconstructed_label,
                    );
                    true
                } else {
                    false
                }
            }
        }
    }
}

/// An iterator that provides [`SliceProgress`] for all node contents and chunks that make up an optimised slice stream.
#[derive(Debug)]
struct SliceProgressIterator {
    /// Start chunk of the slice.
    from: ChunkIndex,
    /// Lenght of the slice, in chunks.
    len: ChunkCount,
    /// Total number of chunks in the ByteStorage.
    chunk_count: ChunkCount,
    /// OptimisationOptions for the slice.
    options: SliceStreamingOptions,
    /// Mutable state during the iteration.
    state: SliceProgressState,
}

/// The mutable state for iterating through the slice in a `SliceProgressIterator`.
#[derive(Debug)]
enum SliceProgressState {
    /// In the first phase, we trace the path from the root to the first chunk.
    RootToFirstChunk {
        current_node_number: NodeNumber,
        path_to_first_chunk: PathToLeaf,
        path_to_final_chunk: PathToLeaf,
        // The node number obtained by the previous call to self.path_to_fial_chunk.iter(); or initially zero.
        prev_on_path_to_final_chunk: NodeNumber,
    },
    /// Starting from the first chunk, we can now simply increment node-counts until we reach the final chunk of the slice.
    SliceDataProper {
        current_node_number: NodeNumber,
        path_to_final_chunk: PathToLeaf,
        // The node number obtained by the previous call to self.path_to_fial_chunk.iter(); or initially zero.
        prev_on_path_to_final_chunk: NodeNumber,
        // For how many chunks did we already emit a SliceProgress?
        processed_chunks: ChunkCount,
    },
    /// We have emitted the final SliceProgress, all that is left is emitting `None`.
    Done,
}

impl SliceProgressIterator {
    fn new(
        from: ChunkIndex,
        len: ChunkCount,
        string_info: StringInfo,
        options: SliceStreamingOptions,
    ) -> Self {
        Self {
            from,
            len,
            chunk_count: string_info.chunk_count,
            options,
            state: SliceProgressState::RootToFirstChunk {
                current_node_number: 0,
                path_to_first_chunk: PathToLeaf::new(from, string_info.chunk_count),
                path_to_final_chunk: PathToLeaf::new(from + (len - 1), string_info.chunk_count),
                prev_on_path_to_final_chunk: 0,
            },
        }
    }
}

impl Iterator for SliceProgressIterator {
    type Item = SliceProgress;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            SliceProgressState::RootToFirstChunk {
                current_node_number,
                path_to_first_chunk,
                path_to_final_chunk,
                prev_on_path_to_final_chunk,
            } => {
                // Compute the layer of the current node.
                let current_layer = node_layer(*current_node_number, self.chunk_count);

                ////////////////////////////////////////
                // Consider left_skip and right_skip. //
                ////////////////////////////////////////

                // This will be set to `true` iff one (or both) of left_skip or right_skip apply.
                let mut skip = false;

                // This is set to true simply if the current_node_number is on the path to the final chunk, irrespective of whether that implies skipping or not.
                let mut is_on_path_to_final = false;

                // This is set to true simply if the current_node_number is on the path to the final chunk, irrespective of whether that implies skipping or not.
                let mut must_not_omit_right = false;

                // Check whether right_skip applies.
                if current_node_number == prev_on_path_to_final_chunk {
                    is_on_path_to_final = true;

                    // Apply right skip.
                    if self.options.right_skip > 0 {
                        skip = true;

                        self.options.right_skip -= 1;
                    }

                    // Update prev_on_path_to_final, and check whether right label is unskippable.
                    if current_layer > 0 {
                        let prev_prev_on_path_to_final_chunk = *prev_on_path_to_final_chunk;

                        *prev_on_path_to_final_chunk = path_to_final_chunk.next().unwrap(); // Only returns None if current_node_number is a leaf, which implies current_layer == 0.

                        if prev_prev_on_path_to_final_chunk + 1 == *prev_on_path_to_final_chunk {
                            // The next vertex on the path to the final chunk is a left child, hence we must not skip the right child label.
                            must_not_omit_right = true;
                        }
                    }
                }

                // Check whether left_skip applies (we have to do this even if right_skip already applied).
                // Since in the RootToFirstChunk case we are tracing the path to the first chunk, left_skip *always* applies if it is nonzero.
                if self.options.left_skip > 0 {
                    skip = true;
                    self.options.left_skip -= 1;
                    debug_assert!(
                        current_layer > 0 || self.chunk_count == 1,
                        "A left_skip slice streaming optimisation option must not instruct to skip the first chunk."
                    );
                }

                /////////////////////////////
                // Node content omissions. //
                /////////////////////////////

                let mut omit_left = false;
                let mut omit_right = false;

                // Implement k-grouping: parent nodes in the lowest k layers are skipped.
                if current_layer <= self.options.k {
                    omit_left = true;
                    omit_right = true;
                }

                // Implement light streaming: omit the left labels of vertices whose layer modulo `options.layer_filter_factor` is not equal to `options.layer_filter_target`.
                if current_layer % self.options.layer_filter_factor
                    != self.options.layer_filter_target
                {
                    omit_left = true;
                }

                //////////////////////////
                // Progress and return. //
                //////////////////////////

                match path_to_first_chunk.next() {
                    Some(next_node_number) => {
                        // current_node_number is on the path to the first chunk.
                        // If the next vertex on that path is a right child, the left child is not part of the stream.
                        let is_left_child_part_of_stream =
                            next_node_number - 1 == *current_node_number;

                        // If the left child is not part of the stream, then we must not skip the left label.
                        if !is_left_child_part_of_stream {
                            omit_left = false;
                        }

                        // If current_node_number is on the path to the final chunk and the next vertex on that path is a left child, the right child is not part of the stream.
                        let is_right_child_part_of_stream =
                            !(is_on_path_to_final && must_not_omit_right);

                        if !is_right_child_part_of_stream {
                            // current_node_number is on the path to the final chunk and the next vertex on that path is a left child - we must not skip the right label.
                            omit_right = false;
                        }

                        let layer_of_next_node = node_layer(next_node_number, self.chunk_count);

                        if layer_of_next_node == 0 {
                            // We processed the parent of the first chunk. Time to switch into sequential mode.

                            // Assert that no left skips are left (otherwise, options.left_skip started larger than valid, indicating a bug, so panicking in that case is a feature).
                            assert_eq!(self.options.left_skip, 0);

                            let new_current_node_number = *current_node_number;

                            self.state = SliceProgressState::SliceDataProper {
                                current_node_number: next_node_number,
                                path_to_final_chunk: path_to_final_chunk.clone(),
                                prev_on_path_to_final_chunk: prev_on_path_to_final_chunk.clone(),
                                processed_chunks: 0,
                            };

                            return Some(SliceProgress::Inner {
                                node_number: new_current_node_number,
                                layer: current_layer,
                                skip,
                                omit_left,
                                omit_right,
                                is_left_child_part_of_stream,
                                is_right_child_part_of_stream,
                            });
                        } else {
                            debug_assert!(current_layer > 1);

                            let ret = Some(SliceProgress::Inner {
                                node_number: *current_node_number,
                                layer: current_layer,
                                skip,
                                omit_left,
                                omit_right,
                                is_left_child_part_of_stream,
                                is_right_child_part_of_stream,
                            });

                            // We have not hit the first chunk yet, so we stay in the RootToFirstChunk case.
                            *current_node_number = next_node_number;

                            return ret;
                        }
                    }
                    None => {
                        // This should only happen if the whole tree is a single leaf.
                        debug_assert_eq!(self.chunk_count, 1);
                        debug_assert_eq!(self.from, 0);
                        debug_assert_eq!(self.len, 1);

                        self.state = SliceProgressState::Done;

                        return Some(SliceProgress::Chunk {
                            chunk_index: 0,
                            node_number: 0,
                        });
                    }
                }
            }

            // This case duplicates a bunch of code from the previous case, but I prefer the explicit and sequential readability here over keeping things DRY.
            SliceProgressState::SliceDataProper {
                current_node_number,
                path_to_final_chunk,
                prev_on_path_to_final_chunk,
                processed_chunks,
            } => {
                let is_left_child_part_of_stream = true;

                // Compute the layer of the current node.
                let current_layer = node_layer(*current_node_number, self.chunk_count);

                //////////////////////////
                // Consider right_skip. //
                //////////////////////////

                // This will be set to `true` iff right_skip applies.
                let mut skip = false;

                // This is set to true simply if the current_node_number is on the path to the final chunk, irrespective of whether that implies skipping or not.
                let mut is_on_path_to_final = false;

                let mut must_not_omit_right = false;
                let mut is_right_child_part_of_stream = true;

                // Check whether right_skip applies.
                if current_node_number == prev_on_path_to_final_chunk {
                    is_on_path_to_final = true;

                    // Apply right skip.
                    if self.options.right_skip > 0 {
                        skip = true;

                        self.options.right_skip -= 1;
                    }

                    // Update prev_on_path_to_final, and check whether right label is unskippable.
                    if current_layer > 0 {
                        let prev_prev_on_path_to_final_chunk = *prev_on_path_to_final_chunk;

                        *prev_on_path_to_final_chunk = path_to_final_chunk.next().unwrap(); // Only returns None if current_node_number is a leaf, which implies current_layer == 0.

                        if prev_prev_on_path_to_final_chunk + 1 == *prev_on_path_to_final_chunk {
                            // The next vertex on the path to the final chunk is a left child, hence we must not skip the right child label.
                            must_not_omit_right = true;
                            is_right_child_part_of_stream = false;
                        }
                    }
                }

                // Left-skip only happens in the RootToFirstChunk case.

                /////////////////////////////
                // Node content omissions. //
                /////////////////////////////

                let mut omit_left = false;
                let mut omit_right = false;

                // Implement k-grouping: parent nodes in the lowest k layers are skipped.
                // (we'll simply ignore omit_left and omit_right if the current node is a chunk)
                if current_layer <= self.options.k {
                    omit_left = true;
                    omit_right = true;
                }

                // Implement light streaming: omit the left labels of vertices whose layer modulo `options.layer_filter_factor` is not equal to `options.layer_filter_target`.
                if current_layer % self.options.layer_filter_factor
                    != self.options.layer_filter_target
                {
                    omit_left = true;
                }

                //////////////////////////
                // Progress and return. //
                //////////////////////////

                if current_layer == 0 {
                    *processed_chunks += 1;
                }

                if *processed_chunks == self.len {
                    let new_current_node_number = *current_node_number;
                    let new_chunk_index = self.from + (*processed_chunks - 1);

                    self.state = SliceProgressState::Done;

                    return Some(SliceProgress::Chunk {
                        chunk_index: new_chunk_index,
                        node_number: new_current_node_number,
                    });
                } else {
                    // current_node_number not on the path to the first chunk (that only happens in the RootToFirstChunk case)
                    // current_node_number might be on the path to the final chunk though!

                    if is_on_path_to_final && must_not_omit_right {
                        // current_node_number is on the path to the final chunk and the next vertex on that path is a left child - we must not skip the right label.
                        omit_right = false;
                    }

                    *current_node_number += 1;
                }

                if current_layer == 0 {
                    return Some(SliceProgress::Chunk {
                        chunk_index: self.from + (*processed_chunks - 1),
                        node_number: *current_node_number - 1,
                    });
                } else {
                    return Some(SliceProgress::Inner {
                        node_number: *current_node_number - 1,
                        layer: current_layer,
                        skip,
                        omit_left,
                        omit_right,
                        is_left_child_part_of_stream,
                        is_right_child_part_of_stream,
                    });
                }
            }

            SliceProgressState::Done => {
                // Assert that no right skips are left (otherwise, options.right_skip started larger than valid, indicating a bug in the calling code, so panicking in that case is a feature).
                debug_assert_eq!(
                    self.options.right_skip, 0,
                    "Tried to iterate through the nodes of a verifiable slice stream, but the supplied `options.right_skip` was greater than the length of the path from the root to the final leaf. This means you called a method with incorrect right_skip."
                );
                return None;
            }
        }
    }
}

/// Tests for the SliceProgressIterator.
#[cfg(all(test, feature = "std"))]
mod tests {
    use ufotofu::producer::clone_from_slice;

    use crate::generic::storage::SingleSliceStore;
    use crate::generic::storage::backend_memory::{self, InMemoryBackend};
    use crate::generic::storage::single_slice_store::SliceStreamResumptionInfo;

    use super::*;

    ///////////////////////////////////////////////
    // baseline streams, starting at offset zero //
    ///////////////////////////////////////////////

    #[test]
    fn slice_progress_000() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            0,
            6,
            StringInfo {
                chunk_count: 6,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 3,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_001() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            0,
            6,
            StringInfo {
                chunk_count: 8,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 3,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_002() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            0,
            7,
            StringInfo {
                chunk_count: 9,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 3,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 5,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    //////////////////////////////////////////////////
    // baseline streams, starting at nonzero offset //
    //////////////////////////////////////////////////

    #[test]
    fn slice_progress_003() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            2,
            4,
            StringInfo {
                chunk_count: 6,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_004() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            3,
            3,
            StringInfo {
                chunk_count: 8,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_005() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            2,
            5,
            StringInfo {
                chunk_count: 9,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    /////////////////////////////////////////////////////////////////////////////////////
    // baseline streams, starting at nonzero offset, with nonzero storage start offset //
    /////////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn slice_progress_006() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            2,
            4,
            StringInfo {
                chunk_count: 6,
                start_offset: 1,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_007() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            3,
            3,
            StringInfo {
                chunk_count: 8,
                start_offset: 3,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_008() {
        let opts = SliceStreamingOptions::default();

        let mut iter = SliceProgressIterator::new(
            2,
            5,
            StringInfo {
                chunk_count: 9,
                start_offset: 2,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    //////////////////////////////////////
    // baseline streams, with left_skip //
    //////////////////////////////////////

    #[test]
    fn slice_progress_009() {
        let mut opts = SliceStreamingOptions::default();
        opts.left_skip = 1;

        let mut iter = SliceProgressIterator::new(
            2,
            4,
            StringInfo {
                chunk_count: 6,
                start_offset: 1,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_010() {
        let mut opts = SliceStreamingOptions::default();
        opts.left_skip = 2;

        let mut iter = SliceProgressIterator::new(
            3,
            3,
            StringInfo {
                chunk_count: 8,
                start_offset: 3,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_011() {
        let mut opts = SliceStreamingOptions::default();
        opts.left_skip = 4;

        let mut iter = SliceProgressIterator::new(
            2,
            5,
            StringInfo {
                chunk_count: 9,
                start_offset: 2,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    ///////////////////////////////////////
    // baseline streams, with right_skip //
    ///////////////////////////////////////

    #[test]
    fn slice_progress_012() {
        let mut opts = SliceStreamingOptions::default();
        opts.right_skip = 1;

        let mut iter = SliceProgressIterator::new(
            2,
            4,
            StringInfo {
                chunk_count: 6,
                start_offset: 1,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_013() {
        let mut opts = SliceStreamingOptions::default();
        opts.right_skip = 2;

        let mut iter = SliceProgressIterator::new(
            3,
            3,
            StringInfo {
                chunk_count: 8,
                start_offset: 3,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_014() {
        let mut opts = SliceStreamingOptions::default();
        opts.right_skip = 4;

        let mut iter = SliceProgressIterator::new(
            2,
            5,
            StringInfo {
                chunk_count: 9,
                start_offset: 2,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    /////////////////////////////////////////////////////
    // baseline streams, with left_skip and right_skip //
    /////////////////////////////////////////////////////

    #[test]
    fn slice_progress_015() {
        let mut opts = SliceStreamingOptions::default();
        opts.left_skip = 2;
        opts.right_skip = 1;

        let mut iter = SliceProgressIterator::new(
            2,
            4,
            StringInfo {
                chunk_count: 6,
                start_offset: 1,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_016() {
        let mut opts = SliceStreamingOptions::default();
        opts.left_skip = 2;
        opts.right_skip = 2;

        let mut iter = SliceProgressIterator::new(
            3,
            3,
            StringInfo {
                chunk_count: 8,
                start_offset: 3,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_017() {
        let mut opts = SliceStreamingOptions::default();
        opts.left_skip = 3;
        opts.right_skip = 4;

        let mut iter = SliceProgressIterator::new(
            2,
            5,
            StringInfo {
                chunk_count: 9,
                start_offset: 2,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: true,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    ////////////////////////////////////////////
    // light streams, starting at offset zero //
    ////////////////////////////////////////////

    #[test]
    fn slice_progress_018() {
        let mut opts = SliceStreamingOptions::default();
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            0,
            6,
            StringInfo {
                chunk_count: 6,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 3,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_019() {
        let mut opts = SliceStreamingOptions::default();
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            0,
            6,
            StringInfo {
                chunk_count: 8,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 3,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_020() {
        let mut opts = SliceStreamingOptions::default();
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            0,
            7,
            StringInfo {
                chunk_count: 9,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 3,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 5,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    //////////////////////////////////////////////////////////////////////////////////
    // light streams, starting at nonzero offset, with nonzero storage start offset //
    //////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn slice_progress_021() {
        let mut opts = SliceStreamingOptions::default();
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            2,
            4,
            StringInfo {
                chunk_count: 6,
                start_offset: 1,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_022() {
        let mut opts = SliceStreamingOptions::default();
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            3,
            3,
            StringInfo {
                chunk_count: 8,
                start_offset: 3,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_023() {
        let mut opts = SliceStreamingOptions::default();
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            2,
            5,
            StringInfo {
                chunk_count: 9,
                start_offset: 2,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    //////////////////////////////////////////////////////
    // 1-grouped light streams, starting at offset zero //
    //////////////////////////////////////////////////////

    #[test]
    fn slice_progress_024() {
        let mut opts = SliceStreamingOptions::default();
        opts.k = 1;
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            0,
            6,
            StringInfo {
                chunk_count: 6,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 3,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_025() {
        let mut opts = SliceStreamingOptions::default();
        opts.k = 1;
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            0,
            6,
            StringInfo {
                chunk_count: 8,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 3,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_026() {
        let mut opts = SliceStreamingOptions::default();
        opts.k = 1;
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            0,
            7,
            StringInfo {
                chunk_count: 9,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 3,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 5,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    ///////////////////////
    // Larger k-grouping //
    ///////////////////////

    #[test]
    fn slice_progress_027() {
        let mut opts = SliceStreamingOptions::default();
        opts.k = 2;
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            0,
            7,
            StringInfo {
                chunk_count: 9,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 3,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 5,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_028() {
        let mut opts = SliceStreamingOptions::default();
        opts.k = 64;
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            0,
            7,
            StringInfo {
                chunk_count: 9,
                start_offset: 0,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 3,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 0,
                node_number: 4,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 1,
                node_number: 5,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    ////////////////////////////////////////////////////////////////////////////////////////////
    // 1-grouped light streams, starting at nonzero offset, with nonzero storage start offset //
    ////////////////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn slice_progress_029() {
        let mut opts = SliceStreamingOptions::default();
        opts.k = 1;
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            2,
            4,
            StringInfo {
                chunk_count: 6,
                start_offset: 1,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 6,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 9,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 10,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_030() {
        let mut opts = SliceStreamingOptions::default();
        opts.k = 1;
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            3,
            3,
            StringInfo {
                chunk_count: 8,
                start_offset: 3,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 5,
                layer: 1,
                skip: false,
                omit_left: false,
                omit_right: true,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 8,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 10,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 11,
            }
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn slice_progress_031() {
        let mut opts = SliceStreamingOptions::default();
        opts.k = 1;
        opts.set_light::<32, 1024>();

        let mut iter = SliceProgressIterator::new(
            2,
            5,
            StringInfo {
                chunk_count: 9,
                start_offset: 2,
            },
            opts,
        );

        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 0,
                layer: 4,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 1,
                layer: 3,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 2,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: false,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 6,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 2,
                node_number: 7,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 3,
                node_number: 8,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 9,
                layer: 2,
                skip: false,
                omit_left: false,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 10,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: true,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: true,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 4,
                node_number: 11,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 5,
                node_number: 12,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Inner {
                node_number: 13,
                layer: 1,
                skip: false,
                omit_left: true,
                omit_right: false,
                is_left_child_part_of_stream: true,
                is_right_child_part_of_stream: false,
            }
        );
        assert_eq!(
            iter.next().unwrap(),
            SliceProgress::Chunk {
                chunk_index: 6,
                node_number: 14,
            }
        );
        assert_eq!(iter.next(), None);
    }

    //////////////////////////////////
    // Testing consume_slice_stream //
    //////////////////////////////////

    fn testing_hash_chunk(chunk: &[u8], is_root: bool, _: &(), output: &mut [u8; 8]) {
        let mut digest: u64 = if is_root { 123477 } else { 11 };
        for (i, byte) in chunk.iter().enumerate() {
            digest = digest
                .wrapping_mul((*byte as u64) + 7 * (i as u64 + 1))
                .wrapping_add(3u64);
        }

        output.copy_from_slice(&digest.to_be_bytes()[..]);
    }

    fn testing_hash_inner(
        left: &[u8; 8],
        right: &[u8; 8],
        covered: u64,
        is_root: bool,
        _: &(),
        output: &mut [u8; 8],
    ) {
        let mut digest: u64 =
            covered
                .wrapping_mul(1211051u64)
                .wrapping_add(if is_root { 7u64 } else { 1020247u64 });
        digest = digest
            .wrapping_add(u64::from_be_bytes(*left))
            .wrapping_mul(1256303u64)
            .wrapping_add(u64::from_be_bytes(*right))
            .wrapping_mul(1020407u64);

        output.copy_from_slice(&digest.to_be_bytes()[..]);
    }

    fn testing_instantiation() -> BabInstantiation<8, 16, (), ()> {
        BabInstantiation {
            hash_chunk: testing_hash_chunk,
            hash_inner: testing_hash_inner,
            hash_chunk_context: (),
            hash_inner_context: (),
        }
    }

    // Returns the labels and the chunks for a tree based on a test parameterisation of Bab.
    // The `i`-th chunk contains 16 `i` bytes, except that its label is computed only over its first three bytes.
    fn create_test_data() -> ([[u8; 8]; 15], [[u8; 16]; 8], ByteCount) {
        let test_data_len = 115; // 16 * 7 + 3, i.e., 7 complete chunks and the three bytes of the final chunk.

        let mut chunks = [[0u8; 16]; 8];
        let mut labels = [[0u8; 8]; 15];

        for i in 0..8 {
            for j in 0..16 {
                (chunks[i])[j] = i as u8;
            }
        }

        testing_hash_chunk(&chunks[0], false, &(), &mut labels[3]);
        testing_hash_chunk(&chunks[1], false, &(), &mut labels[4]);
        testing_hash_chunk(&chunks[2], false, &(), &mut labels[6]);
        testing_hash_chunk(&chunks[3], false, &(), &mut labels[7]);
        testing_hash_chunk(&chunks[4], false, &(), &mut labels[10]);
        testing_hash_chunk(&chunks[5], false, &(), &mut labels[11]);
        testing_hash_chunk(&chunks[6], false, &(), &mut labels[13]);
        testing_hash_chunk(&chunks[7][0..3], false, &(), &mut labels[14]);

        testing_hash_inner(
            &labels[3].clone(),
            &labels[4].clone(),
            32,
            false,
            &(),
            &mut labels[2],
        );
        testing_hash_inner(
            &labels[6].clone(),
            &labels[7].clone(),
            32,
            false,
            &(),
            &mut labels[5],
        );
        testing_hash_inner(
            &labels[10].clone(),
            &labels[11].clone(),
            32,
            false,
            &(),
            &mut labels[9],
        );
        testing_hash_inner(
            &labels[13].clone(),
            &labels[14].clone(),
            19,
            false,
            &(),
            &mut labels[12],
        );

        testing_hash_inner(
            &labels[2].clone(),
            &labels[5].clone(),
            64,
            false,
            &(),
            &mut labels[1],
        );
        testing_hash_inner(
            &labels[9].clone(),
            &labels[12].clone(),
            51,
            false,
            &(),
            &mut labels[8],
        );

        testing_hash_inner(
            &labels[1].clone(),
            &labels[8].clone(),
            test_data_len,
            true,
            &(),
            &mut labels[0],
        );

        (labels, chunks, test_data_len)
    }

    /// Returns the verifiable stream, the total length of the hashed string, and the root label.
    fn full_stream_as_vec() -> (Vec<u8>, ByteCount, [u8; 8]) {
        let (labels, chunks, total_len) = create_test_data();
        // println!("labels: {:?}", labels);
        // println!("chunks: {:?}", chunks);

        let mut stream_data = vec![];

        stream_data.extend_from_slice(&labels[1]);
        stream_data.extend_from_slice(&labels[8]);
        stream_data.extend_from_slice(&labels[2]);
        stream_data.extend_from_slice(&labels[5]);
        stream_data.extend_from_slice(&labels[3]);
        stream_data.extend_from_slice(&labels[4]);
        stream_data.extend_from_slice(&chunks[0]);
        stream_data.extend_from_slice(&chunks[1]);
        stream_data.extend_from_slice(&labels[6]);
        stream_data.extend_from_slice(&labels[7]);
        stream_data.extend_from_slice(&chunks[2]);
        stream_data.extend_from_slice(&chunks[3]);
        stream_data.extend_from_slice(&labels[9]);
        stream_data.extend_from_slice(&labels[12]);
        stream_data.extend_from_slice(&labels[10]);
        stream_data.extend_from_slice(&labels[11]);
        stream_data.extend_from_slice(&chunks[4]);
        stream_data.extend_from_slice(&chunks[5]);
        stream_data.extend_from_slice(&labels[13]);
        stream_data.extend_from_slice(&labels[14]);
        stream_data.extend_from_slice(&chunks[6]);
        stream_data.extend_from_slice(&chunks[7][..3]);

        return (stream_data, total_len, labels[0]);
    }

    // Full stream (on 8 chunks).
    #[test]
    fn test_consume_slice_stream_00() {
        pollster::block_on(async {
            let (raw_stream, string_len, root_label) = full_stream_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage = InMemoryBackend::create(&mut key_state, (), string_len + 15 * 8, 0)
                .await
                .unwrap();

            let mut p = clone_from_slice(&raw_stream[..]);
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                consume_slice_stream(
                    0,
                    8,
                    &mut storage,
                    StringInfo {
                        chunk_count: 8,
                        start_offset: 0
                    },
                    string_len,
                    root_label,
                    opts,
                    &mut p,
                    &testing_instantiation(),
                )
                .await,
                Ok(None)
            );

            // And now for asserting that incorrect input streams are rejected.
            for (byte_index_in_stream_to_change, expected_first_unverified_node_number) in [
                (0, 0),
                (8, 0),
                (16, 1),
                (24, 1),
                (32, 2),
                (40, 2),
                (6 * 8 + 0 * 16, 3),
                (6 * 8 + 1 * 16, 4),
                (6 * 8 + 2 * 16, 5),
                (7 * 8 + 2 * 16, 5),
                (8 * 8 + 2 * 16, 6),
                (8 * 8 + 3 * 16, 7),
                (8 * 8 + 4 * 16, 8),
                (9 * 8 + 4 * 16, 8),
                (10 * 8 + 4 * 16, 9),
                (11 * 8 + 4 * 16, 9),
                (12 * 8 + 4 * 16, 10),
                (12 * 8 + 5 * 16, 11),
                (12 * 8 + 6 * 16, 12),
                (13 * 8 + 6 * 16, 12),
                (14 * 8 + 6 * 16, 13),
                (14 * 8 + 7 * 16, 14),
            ] {
                // println!("\n\n\n");
                // println!(
                //     "byte_index_in_stream_to_change {byte_index_in_stream_to_change}, expected_first_unverified_node_number {expected_first_unverified_node_number}"
                // );

                let mut raw_altered_stream = raw_stream.clone();
                raw_altered_stream[byte_index_in_stream_to_change] =
                    raw_altered_stream[byte_index_in_stream_to_change] ^ 42;

                let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
                let mut storage =
                    InMemoryBackend::create(&mut key_state, (), string_len + 15 * 8, 0)
                        .await
                        .unwrap();

                let mut p = clone_from_slice(&raw_altered_stream[..]);
                let opts = SliceStreamingOptions::default();

                assert_eq!(
                    consume_slice_stream(
                        0,
                        8,
                        &mut storage,
                        StringInfo {
                            chunk_count: 8,
                            start_offset: 0
                        },
                        string_len,
                        root_label,
                        opts,
                        &mut p,
                        &testing_instantiation(),
                    )
                    .await,
                    Err(IngestSliceStreamError::VerificationError {
                        next_node: expected_first_unverified_node_number
                    }),
                );
            }
        });
    }

    /// Returns the verifiable stream, the total length of the hashed string, and the root label.
    fn stream_start1_len5_as_vec() -> (Vec<u8>, ByteCount, [u8; 8]) {
        let (labels, chunks, total_len) = create_test_data();
        // println!("labels: {:?}", labels);
        // println!("chunks: {:?}", chunks);

        let mut stream_data = vec![];

        stream_data.extend_from_slice(&labels[1]);
        stream_data.extend_from_slice(&labels[8]);
        stream_data.extend_from_slice(&labels[2]);
        stream_data.extend_from_slice(&labels[5]);
        stream_data.extend_from_slice(&labels[3]);
        stream_data.extend_from_slice(&labels[4]);
        stream_data.extend_from_slice(&chunks[1]);
        stream_data.extend_from_slice(&labels[6]);
        stream_data.extend_from_slice(&labels[7]);
        stream_data.extend_from_slice(&chunks[2]);
        stream_data.extend_from_slice(&chunks[3]);
        stream_data.extend_from_slice(&labels[9]);
        stream_data.extend_from_slice(&labels[12]);
        stream_data.extend_from_slice(&labels[10]);
        stream_data.extend_from_slice(&labels[11]);
        stream_data.extend_from_slice(&chunks[4]);
        stream_data.extend_from_slice(&chunks[5]);

        return (stream_data, total_len, labels[0]);
    }

    #[test]
    fn test_consume_slice_stream_01() {
        pollster::block_on(async {
            let (raw_stream, string_len, root_label) = stream_start1_len5_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage = InMemoryBackend::create(&mut key_state, (), 16 * 5 + 12 * 8, 0)
                .await
                .unwrap();

            let mut p = clone_from_slice(&raw_stream[..]);
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                consume_slice_stream(
                    1,
                    5,
                    &mut storage,
                    StringInfo {
                        chunk_count: 8,
                        start_offset: 1
                    },
                    string_len,
                    root_label,
                    opts,
                    &mut p,
                    &testing_instantiation(),
                )
                .await,
                Ok(None)
            );

            // And now for asserting that incorrect input streams are rejected.
            for (byte_index_in_stream_to_change, expected_first_unverified_node_number) in [
                (0, 0),
                (8, 0),
                (16, 1),
                (24, 1),
                (32, 2),
                (40, 2),
                (6 * 8 + 0 * 16, 4),
                (6 * 8 + 1 * 16, 5),
                (7 * 8 + 1 * 16, 5),
                (8 * 8 + 1 * 16, 6),
                (8 * 8 + 2 * 16, 7),
                (8 * 8 + 3 * 16, 8),
                (9 * 8 + 3 * 16, 8),
                (10 * 8 + 3 * 16, 9),
                (11 * 8 + 3 * 16, 9),
                (12 * 8 + 3 * 16, 10),
                (12 * 8 + 4 * 16, 11),
            ] {
                // println!("\n\n\n");
                // println!(
                //     "byte_index_in_stream_to_change {byte_index_in_stream_to_change}, expected_first_unverified_node_number {expected_first_unverified_node_number}"
                // );

                let mut raw_altered_stream = raw_stream.clone();
                raw_altered_stream[byte_index_in_stream_to_change] =
                    raw_altered_stream[byte_index_in_stream_to_change] ^ 42;

                let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
                let mut storage =
                    InMemoryBackend::create(&mut key_state, (), string_len + 15 * 8, 0)
                        .await
                        .unwrap();

                let mut p = clone_from_slice(&raw_altered_stream[..]);
                let opts = SliceStreamingOptions::default();

                assert_eq!(
                    consume_slice_stream(
                        1,
                        5,
                        &mut storage,
                        StringInfo {
                            chunk_count: 8,
                            start_offset: 1
                        },
                        string_len,
                        root_label,
                        opts,
                        &mut p,
                        &testing_instantiation(),
                    )
                    .await,
                    Err(IngestSliceStreamError::VerificationError {
                        next_node: expected_first_unverified_node_number
                    }),
                );
            }
        });
    }

    /// Returns the verifiable stream, the total length of the hashed string, and the root label.
    fn stream_start1_len5_skip3_and2_as_vec() -> (Vec<u8>, ByteCount, [u8; 8]) {
        let (labels, chunks, total_len) = create_test_data();
        // println!("labels: {:?}", labels);
        // println!("chunks: {:?}", chunks);

        let mut stream_data = vec![];

        // stream_data.extend_from_slice(&labels[1]);
        // stream_data.extend_from_slice(&labels[8]);
        // stream_data.extend_from_slice(&labels[2]);
        // stream_data.extend_from_slice(&labels[5]);
        // stream_data.extend_from_slice(&labels[3]);
        // stream_data.extend_from_slice(&labels[4]);
        stream_data.extend_from_slice(&chunks[1]);
        stream_data.extend_from_slice(&labels[6]);
        stream_data.extend_from_slice(&labels[7]);
        stream_data.extend_from_slice(&chunks[2]);
        stream_data.extend_from_slice(&chunks[3]);
        // stream_data.extend_from_slice(&labels[9]);
        // stream_data.extend_from_slice(&labels[12]);
        stream_data.extend_from_slice(&labels[10]);
        stream_data.extend_from_slice(&labels[11]);
        stream_data.extend_from_slice(&chunks[4]);
        stream_data.extend_from_slice(&chunks[5]);

        return (stream_data, total_len, labels[0]);
    }

    #[test]
    fn test_consume_slice_stream_02() {
        pollster::block_on(async {
            let (raw_stream, string_len, root_label) = stream_start1_len5_skip3_and2_as_vec();
            let (very_raw_stream, _, _) = full_stream_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage = InMemoryBackend::create(&mut key_state, (), 16 * 5 + 12 * 8, 0)
                .await
                .unwrap();
            storage
                .set_bytes(0, &very_raw_stream[..6 * 8])
                .await
                .unwrap();
            storage
                .set_bytes(
                    8 * 8 + 3 * 16,
                    &very_raw_stream[8 * 8 + 4 * 16..8 * 8 + 4 * 16 + 2 * 8],
                )
                .await
                .unwrap();

            let mut p = clone_from_slice(&raw_stream[..]);
            let mut opts = SliceStreamingOptions::default();
            opts.left_skip = 3;
            opts.right_skip = 2;

            assert_eq!(
                consume_slice_stream(
                    1,
                    5,
                    &mut storage,
                    StringInfo {
                        chunk_count: 8,
                        start_offset: 1
                    },
                    string_len,
                    root_label,
                    opts,
                    &mut p,
                    &testing_instantiation(),
                )
                .await,
                Ok(None)
            );

            // And now for asserting that incorrect input streams are rejected.
            for (byte_index_in_stream_to_change, expected_first_unverified_node_number) in [
                (0 * 8 + 0 * 16, 4),
                (0 * 8 + 1 * 16, 5),
                (1 * 8 + 1 * 16, 5),
                (2 * 8 + 1 * 16, 6),
                (2 * 8 + 2 * 16, 7),
                (2 * 8 + 3 * 16, 9),
                (3 * 8 + 3 * 16, 9),
                (4 * 8 + 3 * 16, 10),
                (4 * 8 + 4 * 16, 11),
            ] {
                // println!("\n\n\n");
                // println!(
                //     "byte_index_in_stream_to_change {byte_index_in_stream_to_change}, expected_first_unverified_node_number {expected_first_unverified_node_number}"
                // );

                let mut raw_altered_stream = raw_stream.clone();
                raw_altered_stream[byte_index_in_stream_to_change] =
                    raw_altered_stream[byte_index_in_stream_to_change] ^ 42;

                let (very_raw_stream, _, _) = full_stream_as_vec();
                // println!("raw stream: {:?}", raw_stream);

                let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
                let mut storage = InMemoryBackend::create(&mut key_state, (), 16 * 5 + 12 * 8, 0)
                    .await
                    .unwrap();
                storage
                    .set_bytes(0, &very_raw_stream[..6 * 8])
                    .await
                    .unwrap();
                storage
                    .set_bytes(
                        8 * 8 + 3 * 16,
                        &very_raw_stream[8 * 8 + 4 * 16..8 * 8 + 4 * 16 + 2 * 8],
                    )
                    .await
                    .unwrap();

                let mut p = clone_from_slice(&raw_altered_stream[..]);
                let mut opts = SliceStreamingOptions::default();
                opts.left_skip = 3;
                opts.right_skip = 2;

                assert_eq!(
                    consume_slice_stream(
                        1,
                        5,
                        &mut storage,
                        StringInfo {
                            chunk_count: 8,
                            start_offset: 1
                        },
                        string_len,
                        root_label,
                        opts,
                        &mut p,
                        &testing_instantiation(),
                    )
                    .await,
                    Err(IngestSliceStreamError::VerificationError {
                        next_node: expected_first_unverified_node_number
                    }),
                );
            }
        });
    }

    // A string consisting of only three bytes. Returns the one three-byte chunk, and the root label.
    fn create_tiny_test_data() -> ([u8; 3], [u8; 8]) {
        let chunk = [1, 2, 3];
        let mut root_label = [0; 8];
        testing_hash_chunk(&chunk[..], true, &(), &mut root_label);

        (chunk, root_label)
    }

    // Single-chunk string.
    #[test]
    fn test_consume_slice_stream_03() {
        pollster::block_on(async {
            let (stream, root_label) = create_tiny_test_data();

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage = InMemoryBackend::create(&mut key_state, (), 3, 0)
                .await
                .unwrap();

            let mut p = clone_from_slice(&stream[..]);
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                consume_slice_stream(
                    0,
                    1,
                    &mut storage,
                    StringInfo {
                        chunk_count: 1,
                        start_offset: 0
                    },
                    3,
                    root_label,
                    opts,
                    &mut p,
                    &testing_instantiation(),
                )
                .await,
                Ok(None)
            );

            // And test the failure case.
            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage = InMemoryBackend::create(&mut key_state, (), 3, 0)
                .await
                .unwrap();

            let mut p = clone_from_slice(&[42, 42, 17][..]);
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                consume_slice_stream(
                    0,
                    1,
                    &mut storage,
                    StringInfo {
                        chunk_count: 1,
                        start_offset: 0
                    },
                    3,
                    root_label,
                    opts,
                    &mut p,
                    &testing_instantiation(),
                )
                .await,
                Err(IngestSliceStreamError::VerificationError { next_node: 0 }),
            );
        });
    }

    // Full stream (on 8 chunks).
    #[test]
    fn test_produce_slice_stream_00() {
        pollster::block_on(async {
            let (raw_stream, string_len, _root_label) = full_stream_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage =
                InMemoryBackend::create(&mut key_state, (), raw_stream.len() as u64, 0)
                    .await
                    .unwrap();
            storage.set_bytes(0, &raw_stream[..]).await.unwrap();

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                produce_slice_stream::<8, 16, _, _>(
                    0,
                    8,
                    &mut storage,
                    StringInfo {
                        chunk_count: 8,
                        start_offset: 0
                    },
                    string_len,
                    opts,
                    &mut c,
                )
                .await,
                Ok(raw_stream.len() as u64)
            );

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, raw_stream);
        });
    }

    #[test]
    fn test_produce_slice_stream_01() {
        pollster::block_on(async {
            let (raw_stream, string_len, _root_label) = stream_start1_len5_as_vec();
            let (full_stream, _, _) = full_stream_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage =
                InMemoryBackend::create(&mut key_state, (), full_stream.len() as u64, 0)
                    .await
                    .unwrap();
            storage.set_bytes(0, &full_stream[..]).await.unwrap();

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                produce_slice_stream::<8, 16, _, _>(
                    1,
                    5,
                    &mut storage,
                    StringInfo {
                        chunk_count: 8,
                        start_offset: 0
                    },
                    string_len,
                    opts,
                    &mut c,
                )
                .await,
                Ok(raw_stream.len() as u64)
            );

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, raw_stream);
        });
    }

    #[test]
    fn test_produce_slice_stream_02() {
        pollster::block_on(async {
            let (raw_stream, string_len, _root_label) = stream_start1_len5_skip3_and2_as_vec();
            let (full_stream, _, _) = full_stream_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage =
                InMemoryBackend::create(&mut key_state, (), full_stream.len() as u64, 0)
                    .await
                    .unwrap();
            storage.set_bytes(0, &full_stream[..]).await.unwrap();

            let mut c = vec![].into_consumer();
            let mut opts = SliceStreamingOptions::default();
            opts.left_skip = 3;
            opts.right_skip = 2;

            assert_eq!(
                produce_slice_stream::<8, 16, _, _>(
                    1,
                    5,
                    &mut storage,
                    StringInfo {
                        chunk_count: 8,
                        start_offset: 0
                    },
                    string_len,
                    opts,
                    &mut c,
                )
                .await,
                Ok(raw_stream.len() as u64)
            );

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, raw_stream);
        });
    }

    #[test]
    fn test_produce_slice_stream_03() {
        pollster::block_on(async {
            let (raw_stream, _root_label) = create_tiny_test_data();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage =
                InMemoryBackend::create(&mut key_state, (), raw_stream.len() as u64, 0)
                    .await
                    .unwrap();
            storage.set_bytes(0, &raw_stream[..]).await.unwrap();

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                produce_slice_stream::<8, 16, _, _>(
                    0,
                    1,
                    &mut storage,
                    StringInfo {
                        chunk_count: 1,
                        start_offset: 0
                    },
                    3,
                    opts,
                    &mut c,
                )
                .await,
                Ok(raw_stream.len() as u64)
            );

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, raw_stream);
        });
    }

    fn raw_chunks() -> Vec<u8> {
        let mut v = vec![];
        for i in 0..7 {
            v.extend_from_slice(&[i; 16]);
        }
        v.extend_from_slice(&[7; 3]);
        v
    }

    #[test]
    fn test_get_slice_00() {
        pollster::block_on(async {
            let (raw_stream, string_len, _root_label) = full_stream_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage =
                InMemoryBackend::create(&mut key_state, (), raw_stream.len() as u64, 0)
                    .await
                    .unwrap();
            storage.set_bytes(0, &raw_stream[..]).await.unwrap();

            // Full slice.

            let mut c = vec![].into_consumer();

            assert_eq!(
                storage
                    .get_slice::<8, 16, _>(
                        &mut c,
                        0,
                        7 * 16 + 3,
                        StringInfo {
                            chunk_count: 8,
                            start_offset: 0
                        },
                        string_len,
                    )
                    .await,
                Ok(7 * 16 + 3)
            );

            let produced_stream: Vec<u8> = c.into();
            let chunks = raw_chunks();
            assert_eq!(produced_stream, &chunks[..]);

            // Partial slice, in chunk boundaries.

            let mut c = vec![].into_consumer();

            assert_eq!(
                storage
                    .get_slice::<8, 16, _>(
                        &mut c,
                        10,
                        29,
                        StringInfo {
                            chunk_count: 8,
                            start_offset: 0
                        },
                        string_len,
                    )
                    .await,
                Ok(29)
            );

            let produced_stream: Vec<u8> = c.into();
            let chunks = raw_chunks();
            assert_eq!(produced_stream, &chunks[10..10 + 29]);

            // Partial slice, within single chunk.

            let mut c = vec![].into_consumer();

            assert_eq!(
                storage
                    .get_slice::<8, 16, _>(
                        &mut c,
                        10,
                        3,
                        StringInfo {
                            chunk_count: 8,
                            start_offset: 0
                        },
                        string_len,
                    )
                    .await,
                Ok(3)
            );

            let produced_stream: Vec<u8> = c.into();
            let chunks = raw_chunks();
            assert_eq!(produced_stream, &chunks[10..10 + 3]);

            // Partial slice, within the final, incomplete chunk.

            let mut c = vec![].into_consumer();

            assert_eq!(
                storage
                    .get_slice::<8, 16, _>(
                        &mut c,
                        7 * 16 + 1,
                        1,
                        StringInfo {
                            chunk_count: 8,
                            start_offset: 0
                        },
                        string_len,
                    )
                    .await,
                Ok(1)
            );

            let produced_stream: Vec<u8> = c.into();
            let chunks = raw_chunks();
            assert_eq!(produced_stream, &chunks[7 * 16 + 1..7 * 16 + 2]);
        });
    }

    #[test]
    fn test_get_slice_01() {
        pollster::block_on(async {
            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage = InMemoryBackend::create(&mut key_state, (), 3, 0)
                .await
                .unwrap();
            storage.set_bytes(0, &[1, 2, 3][..]).await.unwrap();

            // Full slice.

            let mut c = vec![].into_consumer();

            assert_eq!(
                storage
                    .get_slice::<8, 16, _>(
                        &mut c,
                        0,
                        3,
                        StringInfo {
                            chunk_count: 1,
                            start_offset: 0
                        },
                        3,
                    )
                    .await,
                Ok(3)
            );

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, &[1, 2, 3][..]);

            // Non-full slice.

            let mut c = vec![].into_consumer();

            assert_eq!(
                storage
                    .get_slice::<8, 16, _>(
                        &mut c,
                        1,
                        1,
                        StringInfo {
                            chunk_count: 1,
                            start_offset: 0
                        },
                        3,
                    )
                    .await,
                Ok(1)
            );

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, &[2][..]);
        });
    }

    #[test]
    fn test_initialise_backend_00() {
        pollster::block_on(async {
            let chunks = raw_chunks();
            let (raw_stream, string_len, root_label) = full_stream_as_vec();

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage =
                InMemoryBackend::create(&mut key_state, (), raw_stream.len() as u64, 0)
                    .await
                    .unwrap();
            // println!("raw_stream_len and thus capacity: {}", raw_stream.len());

            // Full slice.

            let mut p = clone_from_slice(&chunks[..]);

            assert_eq!(
                storage
                    .initialise_backend::<8, 16, _, _, _>(
                        string_len,
                        0,
                        &mut p,
                        &testing_instantiation()
                    )
                    .await,
                Ok(root_label.into())
            );

            // And check that the storage is initialised with the correct data.

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                produce_slice_stream::<8, 16, _, _>(
                    0,
                    8,
                    &mut storage,
                    StringInfo {
                        chunk_count: 8,
                        start_offset: 0
                    },
                    string_len,
                    opts,
                    &mut c,
                )
                .await,
                Ok(raw_stream.len() as u64)
            );

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, raw_stream);
        });
    }

    #[test]
    fn test_initialise_backend_01() {
        pollster::block_on(async {
            let chunks = vec![1, 2, 3];
            let (raw_stream, root_label) = create_tiny_test_data();
            let string_len = 3;

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut storage =
                InMemoryBackend::create(&mut key_state, (), raw_stream.len() as u64, 0)
                    .await
                    .unwrap();
            // println!("raw_stream_len and thus capacity: {}", raw_stream.len());

            // Full slice.

            let mut p = clone_from_slice(&chunks[..]);

            assert_eq!(
                storage
                    .initialise_backend::<8, 16, _, _, _>(
                        string_len,
                        0,
                        &mut p,
                        &testing_instantiation()
                    )
                    .await,
                Ok(root_label.into())
            );

            // And check that the storage is initialised with the correct data.

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            assert_eq!(
                produce_slice_stream::<8, 16, _, _>(
                    0,
                    1,
                    &mut storage,
                    StringInfo {
                        chunk_count: 1,
                        start_offset: 0
                    },
                    string_len,
                    opts,
                    &mut c,
                )
                .await,
                Ok(raw_stream.len() as u64)
            );

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, raw_stream);
        });
    }

    // sss = SingleSliceStore.
    #[test]
    fn test_sss_create_and_initialise_00() {
        pollster::block_on(async {
            let chunks = raw_chunks();
            let (raw_stream, string_len, root_label) = full_stream_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();

            let mut p = clone_from_slice(&chunks[..]);

            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    string_len,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            assert_eq!(&computed_digest.as_bytes()[..], root_label);

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            sss.get_verifiable_stream(&mut c, 0, 8, opts).await.unwrap();

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, raw_stream);
        });
    }

    #[test]
    fn test_sss_create_and_initialise_01() {
        pollster::block_on(async {
            let chunks = vec![1, 2, 3];
            let (raw_stream, root_label) = create_tiny_test_data();
            let string_len = 3;

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();

            let mut p = clone_from_slice(&chunks[..]);

            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    string_len,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            assert_eq!(&computed_digest.as_bytes()[..], root_label);

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            sss.get_verifiable_stream(&mut c, 0, 1, opts).await.unwrap();

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(produced_stream, raw_stream);
        });
    }

    #[test]
    fn test_sss_append_data_00() {
        pollster::block_on(async {
            let chunks = raw_chunks();
            let (raw_stream, string_len, root_label) = full_stream_as_vec();
            // println!("raw stream: {:?}", raw_stream);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();

            let mut sss = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state,
                (),
                root_label.into(),
                string_len,
                0,
                8,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&raw_stream[..4 * 8]);
            let opts1 = SliceStreamingOptions::default();
            assert_eq!(
                sss.append_data(&mut p1, opts1).await,
                Err(IngestSliceStreamError::UnexpectedEndOfStream { next_node: 2 })
            );

            let mut c = vec![].into_consumer();
            assert_eq!(sss.get_data(&mut c, 0, 9999999).await.unwrap(), 0);

            let mut p2 = clone_from_slice(&raw_stream[4 * 8..4 * 8 + 3]);
            let resumption_info = sss.metadata().slice_stream_resumption_info().unwrap();
            let mut opts2 = SliceStreamingOptions::default();
            opts2.left_skip = resumption_info.left_skip;
            assert_eq!(
                sss.append_data(&mut p2, opts2).await,
                Err(IngestSliceStreamError::UnexpectedEndOfStream { next_node: 2 })
            );

            let mut p3 = clone_from_slice(&raw_stream[4 * 8..]);
            let resumption_info = sss.metadata().slice_stream_resumption_info().unwrap();
            let mut opts3 = SliceStreamingOptions::default();
            opts3.left_skip = resumption_info.left_skip;
            assert_eq!(sss.append_data(&mut p3, opts3).await, Ok(None),);

            let mut c = vec![].into_consumer();
            assert_eq!(
                sss.get_data(&mut c, 0, 9999999).await.unwrap(),
                chunks.len() as u64
            );
            let produced_slice: Vec<u8> = c.into();
            assert_eq!(produced_slice, &chunks[..]);
        });
    }

    #[test]
    fn test_sss_append_data_01() {
        pollster::block_on(async {
            let chunks = [1, 2, 3];
            let (raw_stream, root_label) = create_tiny_test_data();
            let string_len = 3;

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();

            let mut sss = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state,
                (),
                root_label.into(),
                string_len,
                0,
                1,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p = clone_from_slice(&raw_stream[..]);
            let opts = SliceStreamingOptions::default();
            assert_eq!(sss.append_data(&mut p, opts).await, Ok(None),);

            let mut c = vec![].into_consumer();
            assert_eq!(
                sss.get_data(&mut c, 0, 9999999).await.unwrap(),
                chunks.len() as u64
            );
            let produced_slice: Vec<u8> = c.into();
            assert_eq!(produced_slice, &chunks[..]);
        });
    }

    #[test]
    fn test_sss_regression_000() {
        pollster::block_on(async {
            let chunks = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17];

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            sss.get_verifiable_stream(&mut c, 1, 1, opts).await.unwrap();

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(
                produced_stream.len(),
                2 * 8 + 2,
                "produced_stream {produced_stream:?}"
            );

            let mut key_state2: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut sss2 = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state2,
                (),
                computed_digest.clone(),
                18,
                1,
                1,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&produced_stream[..]);
            let append_result = sss2
                .append_data(&mut p1, SliceStreamingOptions::default())
                .await;
            assert_eq!(append_result, Ok(None));

            let resumption1 = sss2.metadata().slice_stream_resumption_info();
            assert_eq!(resumption1, None);
        });
    }

    #[test]
    fn test_sss_regression_001() {
        pollster::block_on(async {
            let chunks = vec![
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31,
            ];

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            sss.get_verifiable_stream(&mut c, 0, 2, opts).await.unwrap();

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(
                produced_stream.len(),
                2 * 8 + 2 * 16,
                "produced_stream {produced_stream:?}"
            );

            let mut key_state2: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut sss2 = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state2,
                (),
                computed_digest.clone(),
                32,
                0,
                2,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&produced_stream[..]);
            let append_result = sss2
                .append_data(&mut p1, SliceStreamingOptions::default())
                .await;
            assert_eq!(append_result, Ok(None));

            let resumption1 = sss2.metadata().slice_stream_resumption_info();
            assert_eq!(resumption1, None);
        });
    }

    #[test]
    fn test_sss_regression_002() {
        pollster::block_on(async {
            let chunks = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (_sss, _computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();
        });
    }

    #[test]
    fn test_sss_regression_003() {
        pollster::block_on(async {
            let mut chunks = vec![];
            for i in 0..260 {
                chunks.push((i % 256) as u8);
            }

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, _computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut c = vec![].into_consumer();
            let opts = SliceStreamingOptions::default();

            sss.get_verifiable_stream(&mut c, 16, 1, opts)
                .await
                .unwrap();

            let produced_stream: Vec<u8> = c.into();
            assert_eq!(
                produced_stream.len(),
                2 * 8 + 4,
                "produced_stream {produced_stream:?}"
            );
        });
    }

    #[test]
    fn test_sss_regression_004() {
        pollster::block_on(async {
            let mut chunks = vec![];
            for i in 0..513 {
                chunks.push((i % 256) as u8);
            }

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut buf1 = [17u8; 51];
            let mut c = (&mut buf1[..]).into_consumer();
            let opts = SliceStreamingOptions::default();

            let _ = sss.get_verifiable_stream(&mut c, 16, 1, opts).await;

            let produced_stream: Vec<u8> = buf1.into();
            assert_eq!(
                produced_stream.len(),
                51,
                "produced_stream {produced_stream:?}"
            );

            let mut key_state2: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut sss2 = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state2,
                (),
                computed_digest.clone(),
                513,
                16,
                1,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&produced_stream[..]);
            let append_result = sss2
                .append_data(&mut p1, SliceStreamingOptions::default())
                .await;
            assert!(append_result.is_err());

            let resumption1 = sss2.metadata().slice_stream_resumption_info().unwrap();
            assert_eq!(
                resumption1,
                SliceStreamResumptionInfo {
                    start_chunk: 16,
                    left_skip: 3,
                    right_skip: 3
                }
            );

            let mut c2 = vec![].into_consumer();
            let mut opts2 = SliceStreamingOptions::default();
            opts2.left_skip = resumption1.left_skip;
            opts2.right_skip = resumption1.right_skip;

            sss.get_verifiable_stream(&mut c2, resumption1.start_chunk, 1, opts2)
                .await
                .unwrap();
        });
    }

    #[test]
    fn test_sss_regression_005() {
        pollster::block_on(async {
            let mut chunks = vec![];
            for i in 0..131 {
                chunks.push((i % 256) as u8);
            }

            let slice_start = 0;
            let slice_len = 6;

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut buf1 = [17u8; 178];
            let mut c = (&mut buf1[..]).into_consumer();
            let opts = SliceStreamingOptions::default();

            let _ = sss
                .get_verifiable_stream(&mut c, slice_start, slice_len, opts)
                .await;

            let produced_stream: Vec<u8> = buf1.into();
            assert_eq!(
                produced_stream.len(),
                178,
                "produced_stream {produced_stream:?}"
            );

            let mut key_state2: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut sss2 = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state2,
                (),
                computed_digest.clone(),
                chunks.len() as u64,
                slice_start,
                slice_len,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&produced_stream[..]);
            let append_result = sss2
                .append_data(&mut p1, SliceStreamingOptions::default())
                .await;
            assert!(append_result.is_err());

            let resumption1 = sss2.metadata().slice_stream_resumption_info().unwrap();
            assert_eq!(
                resumption1,
                SliceStreamResumptionInfo {
                    start_chunk: 4,
                    left_skip: 4,
                    right_skip: 4
                }
            );

            let mut c2 = vec![].into_consumer();
            let mut opts2 = SliceStreamingOptions::default();
            opts2.left_skip = resumption1.left_skip;
            opts2.right_skip = resumption1.right_skip;

            sss.get_verifiable_stream(
                &mut c2,
                resumption1.start_chunk,
                slice_len - (resumption1.start_chunk - slice_start),
                opts2,
            )
            .await
            .unwrap();
        });
    }

    #[test]
    fn test_sss_regression_006() {
        pollster::block_on(async {
            let mut chunks = vec![];
            for i in 0..16 {
                chunks.push((i % 256) as u8);
            }

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, _computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut buf = [17; 16];
            let mut final_c = (&mut buf).into_consumer();
            assert_eq!(16, sss.get_data(&mut final_c, 0, u64::MAX).await.unwrap());
            assert_eq!(&chunks[..], &buf[..]);
        });
    }

    #[test]
    fn test_sss_regression_007() {
        pollster::block_on(async {
            let mut chunks = vec![];
            for i in 0..17 {
                chunks.push((i % 256) as u8);
            }

            let slice_start = 0;
            let slice_len = 1;

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut buf1 = [17u8; 32];
            let mut c = (&mut buf1[..]).into_consumer();
            let opts = SliceStreamingOptions::default();

            let _ = sss
                .get_verifiable_stream(&mut c, slice_start, slice_len, opts)
                .await;

            let produced_stream: Vec<u8> = buf1.into();
            assert_eq!(
                produced_stream.len(),
                32,
                "produced_stream {produced_stream:?}"
            );

            let mut key_state2: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut sss2 = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state2,
                (),
                computed_digest.clone(),
                chunks.len() as u64,
                slice_start,
                slice_len,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&produced_stream[..]);
            let append_result = sss2
                .append_data(&mut p1, SliceStreamingOptions::default())
                .await;
            assert!(append_result.is_ok());

            let resumption1 = sss2.metadata().slice_stream_resumption_info();
            assert_eq!(resumption1, None);
            assert_eq!(sss2.metadata().slice_length(), 1);
            assert_eq!(sss2.metadata().ingested_chunks(), 1);

            let mut buf = [17; 99];
            let mut final_c = (&mut buf).into_consumer();
            assert_eq!(16, sss2.get_data(&mut final_c, 0, u64::MAX).await.unwrap());
            assert_eq!(&chunks[..16], &buf[..16]);
        });
    }

    #[test]
    fn test_sss_regression_008() {
        pollster::block_on(async {
            let mut chunks = vec![];
            for i in 0..33 {
                chunks.push((i % 256) as u8);
            }

            let slice_start = 0;
            let slice_len = 2;

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut buf1 = [17u8; 58];
            let mut c = (&mut buf1[..]).into_consumer();
            let opts1 = SliceStreamingOptions::default();

            let _ = sss
                .get_verifiable_stream(&mut c, slice_start, slice_len, opts1)
                .await;

            let produced_stream: Vec<u8> = buf1.into();
            assert_eq!(
                produced_stream.len(),
                58,
                "produced_stream {produced_stream:?}"
            );
            println!("produced_stream {produced_stream:?}");

            let mut key_state2: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut sss2 = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state2,
                (),
                computed_digest.clone(),
                chunks.len() as u64,
                slice_start,
                slice_len,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&produced_stream[..]);
            let append_result = sss2
                .append_data(&mut p1, SliceStreamingOptions::default())
                .await;
            assert_eq!(
                append_result,
                Err(IngestSliceStreamError::UnexpectedEndOfStream { next_node: 3 })
            );
            println!("append_result {append_result:?}");
        });
    }

    #[test]
    fn test_sss_regression_009() {
        pollster::block_on(async {
            let mut chunks = vec![];
            for i in 0..33 {
                chunks.push((i % 256) as u8);
            }

            let slice_start = 0;
            let slice_len = 2;

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut buf1 = [17u8; 42];
            let mut c = (&mut buf1[..]).into_consumer();
            let mut opts1 = SliceStreamingOptions::default();
            opts1.layer_filter_target = 19;
            opts1.layer_filter_factor = 19.try_into().unwrap();

            let _ = sss
                .get_verifiable_stream(&mut c, slice_start, slice_len, opts1)
                .await;

            let produced_stream: Vec<u8> = buf1.into();
            assert_eq!(
                produced_stream.len(),
                42,
                "produced_stream {produced_stream:?}"
            );

            let mut key_state2: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut sss2 = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state2,
                (),
                computed_digest.clone(),
                chunks.len() as u64,
                slice_start,
                slice_len,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&produced_stream[..]);
            let append_result = sss2.append_data(&mut p1, opts1).await;
            assert_eq!(
                append_result,
                Err(IngestSliceStreamError::UnexpectedEndOfStream { next_node: 3 })
            );

            let resumption1 = sss2.metadata().slice_stream_resumption_info().unwrap();
            assert_eq!(
                resumption1,
                SliceStreamResumptionInfo {
                    start_chunk: 1,
                    left_skip: 2,
                    right_skip: 2
                }
            );
            println!("\nresumption1 {resumption1:?}");

            let mut c2 = vec![].into_consumer();
            let mut opts2 = opts1;
            // let mut opts2 = SliceStreamingOptions::default();
            opts2.left_skip = resumption1.left_skip;
            opts2.right_skip = resumption1.right_skip;

            sss.get_verifiable_stream(
                &mut c2,
                resumption1.start_chunk,
                slice_len - (resumption1.start_chunk - slice_start),
                opts2,
            )
            .await
            .unwrap();

            let produced_stream2: Vec<u8> = Vec::<u8>::from(c2);
            assert_eq!(
                produced_stream2.len(),
                16,
                "produced_stream {produced_stream2:?}"
            );
            println!("produced_stream2 {produced_stream2:?}");

            println!("\n\n\nappend nr 2\n======\n");

            let mut p2 = clone_from_slice(&produced_stream2[..]);
            let append_result2 = sss2.append_data(&mut p2, opts2).await;
            assert_eq!(append_result2, Ok(None));

            let resumption2 = sss2.metadata().slice_stream_resumption_info();
            assert_eq!(resumption2, None);
        });
    }

    #[test]
    fn test_sss_regression_010() {
        pollster::block_on(async {
            let mut chunks = vec![];
            for i in 0..50 {
                chunks.push((i % 256) as u8);
            }

            let slice_start = 0;
            let slice_len = 4;

            let mut p = clone_from_slice(&chunks[..]);

            let mut key_state: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let (mut sss, computed_digest) =
                SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create_and_initialise(
                    &mut key_state,
                    (),
                    chunks.len() as u64,
                    &mut p,
                    testing_instantiation(),
                )
                .await
                .unwrap();

            let mut buf1 = [17u8; 74];
            let mut c = (&mut buf1[..]).into_consumer();
            let mut opts1 = SliceStreamingOptions::default();
            opts1.layer_filter_target = 19;
            opts1.layer_filter_factor = 19.try_into().unwrap();

            let _ = sss
                .get_verifiable_stream(&mut c, slice_start, slice_len, opts1)
                .await;

            let produced_stream: Vec<u8> = buf1.into();
            assert_eq!(
                produced_stream.len(),
                74,
                "produced_stream {produced_stream:?}"
            );
            println!("produced_stream {produced_stream:?}");

            let mut key_state2: backend_memory::KeyState<()> = backend_memory::KeyState::new();
            let mut sss2 = SingleSliceStore::<8, 16, InMemoryBackend<()>, _, _>::create(
                &mut key_state2,
                (),
                computed_digest.clone(),
                chunks.len() as u64,
                slice_start,
                slice_len,
                testing_instantiation(),
            )
            .await
            .unwrap();

            let mut p1 = clone_from_slice(&produced_stream[..]);
            let append_result = sss2.append_data(&mut p1, opts1).await;
            assert_eq!(append_result, Ok(None));

            let resumption1 = sss2.metadata().slice_stream_resumption_info();
            assert_eq!(resumption1, None);
        });
    }
}
