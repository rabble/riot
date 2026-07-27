use crate::{authorisation::AuthorisedEntry, entry::Entrylike};
use bab_rs::{
    CHUNK_SIZE, WIDTH,
    generic::storage::{
        single_slice_store::SliceStreamResumptionInfo, verifiable_streaming::SliceStreamingOptions,
    },
};
use core::cmp::Ordering;
use ufotofu::codec::Blame;

/// The ways in which [`DropSliceMetadata`] can be malformed.
#[derive(Debug, Clone)]
pub enum SliceMetadataError {
    /// The specified `chunk_count` exceeds the number of chunks in the slice from the specified `first_chunk` to the end of the `entry`.
    TooManyChunks,
    /// The specified first chunk lies after the last chunk of the `entry`.
    OutOfBounds,
    /// Encountered some other error.
    OtherError(Blame),
}

// TODO:
// Okay... *maybe* it would be really nice if `DropSliceMetadata` implemented `RelativeDecodable` / `RelativeEncodable`.
// I'll get the logic working first, then see about the refactor.

/// The metadata associated with one payload slice decoded from data in the [Willow drop format](https://willowprotocol.org/specs/drop-format/index.html#drop).
///
/// This metadata consists of the [`AuthorisedEntry`] of which the slice is a part, the chunk index
/// at which the slice starts, the number of chunks in the slice, and the [`left_skip`](https://worm-blossom.github.io/bab/#left_skip) minimising
/// the verification data required for this slice in the context of the drop.
///
/// The drop format APIs usually work with producers of payload slices, paired with a [`DropSliceMetadata`]. The entries that make up a drop are typically communicated only through the `entry` field of the `DropSliceMetadata`.
#[derive(Debug, Clone)]
pub struct DropSliceMetadata {
    /// The [entry](AuthorisedEntry) to which this payload slice contributes.
    entry: AuthorisedEntry,
    /// The chunk index of the payload at which this slice starts.
    first_chunk: u64,
    /// The number of payload chunks in this slice.
    chunk_count: u64,
    /// The number of left labels skipped due to redundancy with previous slices of the drop.
    left_skip: u8,
    /// `true` if `self` may describe one of multiple slices of the same payload within the drop.
    is_multislice_encoded: bool,
}

impl DropSliceMetadata {
    /// Creates a new `DropSliceMetadata` describing the [64-grouped](https://bab-hash.org/spec#kgrouped_baseline) [baseline verifiable slice stream](https://bab-hash.org/spec#baseline_slice) of the slice of `entry` from `first_chunk` to `first_chunk + chunk_count`.
    pub fn new(
        entry: AuthorisedEntry,
        first_chunk: u64,
        chunk_count: u64,
        left_skip: u8,
        is_multislice_encoded: bool,
    ) -> Result<Self, SliceMetadataError> {
        let max_chunks = entry.payload_length().div_ceil(
            CHUNK_SIZE
                .try_into()
                .map_err(|_| SliceMetadataError::OtherError(Blame::OurFault))?,
        );

        if first_chunk > max_chunks {
            return Err(SliceMetadataError::OutOfBounds);
        }

        if first_chunk + chunk_count > max_chunks {
            return Err(SliceMetadataError::TooManyChunks);
        }

        Ok(DropSliceMetadata {
            entry,
            first_chunk,
            chunk_count,
            left_skip,
            is_multislice_encoded,
        })
    }

    /// Returns the [`AuthorisedEntry`] to which the payload slice described by `self` contributes.
    pub fn entry(&self) -> &AuthorisedEntry {
        &self.entry
    }

    /// Returns in the index in the total payload of the first chunk of the payload slice described by `self`.
    pub fn first_chunk(&self) -> u64 {
        self.first_chunk
    }

    /// Returns the number of chunks in the payload slice described by `self`.
    pub fn chunk_count(&self) -> u64 {
        self.chunk_count
    }

    /// Returns the [`left_skip`](https://worm-blossom.github.io/bab/#left_skip) used to encode the payload slice described by `self`.
    pub fn left_skip(&self) -> u8 {
        self.left_skip
    }

    /// Returns the [`SliceStreamResumptionInfo`] indicating where the payload slice described by `self` can be appended to existing payload data.
    pub fn resumption_info(&self) -> SliceStreamResumptionInfo {
        SliceStreamResumptionInfo {
            start_chunk: self.first_chunk,
            left_skip: self.left_skip,
            right_skip: 0,
        }
    }

    /// Returns the [`SliceStreamingOptions`] used to encode the payload slice described by `self`.
    pub fn streaming_options(&self) -> SliceStreamingOptions {
        let mut options = SliceStreamingOptions::default();
        options.k = 64;
        options.left_skip = self.left_skip;
        options
    }

    /// Returns the number of bytes which are expected to occur in the [64-grouped baseline verifiable stream]() of the payload slice described by `self`.
    pub fn expected_bytes(&self) -> u64 {
        let total_chunks = self.entry.payload_length().div_ceil(CHUNK_SIZE as u64);
        let label_bytes = WIDTH as u64
            * labels_in_drop_slice(
                self.first_chunk,
                self.chunk_count,
                total_chunks,
                self.left_skip,
                0,
            );
        (self.chunk_count * CHUNK_SIZE as u64)
            .min(self.entry.payload_length() - (self.first_chunk * CHUNK_SIZE as u64))
            + label_bytes
    }

    /// Returns `true` if the payload slice described by `self` covers the complete payload of the [entry](AuthorisedEntry), else false.
    pub fn is_complete(&self) -> bool {
        self.chunk_count == self.entry().payload_length().div_ceil(CHUNK_SIZE as u64)
    }

    /// Returns `true` if the payload slice described by `self` might be one of multiple slices from the same [entry](AuthorisedEntry) in the drop, else `false`.
    pub fn is_multislice(&self) -> bool {
        self.is_multislice_encoded
    }
}

/// Counts the number of verification labels required to verify a payload slice of `chunk_count` chunks,
/// starting at `first_chunk`, taken from a total payload of length `total_chunks`, when the given
/// `left_skip` and `right_skip` are set.
fn labels_in_drop_slice(
    first_chunk: u64,
    chunk_count: u64,
    total_chunks: u64,
    left_skip: u8,
    right_skip: u8,
) -> u64 {
    // TODO: This function probably belongs in bab_rs.
    // It should be extended to count labels for different SliceStreamingOptions, then
    // it would satisfy the requirements of (Bab's issue #13)[https://codeberg.org/worm-blossom/bab_rs/issues/13]
    debug_assert!(chunk_count <= total_chunks);

    // No data? No labels!
    if chunk_count == 0 {
        return 0;
    }

    // All the data? No labels!
    if chunk_count == total_chunks {
        return 0;
    }

    // left side is first_chunk.count_ones().saturating_sub(left_skip);
    // right side is... ? (Local depth - last_chunk.count_ones()).saturating_sub(right_skip);
    // What is local depth? We can calculate it iteratively if we have to, but it feels there should be a neater way.

    let left_labels = first_chunk.count_ones().saturating_sub(left_skip as u32);

    let mut local_depth_at_right_edge = 0;

    let mut left_leaves = total_chunks.next_power_of_two() / 2;
    let mut right_leaves = total_chunks - left_leaves;
    let mut split = left_leaves;

    let last_chunk = first_chunk + chunk_count - 1;
    while left_leaves > 0 {
        match last_chunk.cmp(&split) {
            Ordering::Less => {
                // go left, left subrtees are complete and thus split evenly in half
                left_leaves /= 2;
                right_leaves = left_leaves;
                split -= left_leaves;
            }
            _ => {
                // go right, potentially divide the remaining leaves unevenly
                left_leaves = right_leaves.next_power_of_two() / 2;
                right_leaves -= left_leaves;
                split += left_leaves;
            }
        }
        local_depth_at_right_edge += 1;
    }

    let right_labels =
        (local_depth_at_right_edge - last_chunk.count_ones()).saturating_sub(right_skip as u32);

    (left_labels + right_labels) as u64
}

#[test]
fn test_label_count() {
    let cases = [
        (0, 11, 11, 0),
        (1, 10, 11, 1),
        (2, 9, 11, 1),
        (3, 8, 11, 2),
        (4, 7, 11, 1),
        (5, 6, 11, 2),
        (6, 5, 11, 2),
        (7, 4, 11, 3),
        (8, 3, 11, 1),
        (9, 2, 11, 2),
        (10, 1, 11, 2),
        (0, 10, 11, 1),
        (1, 9, 11, 2),
        (2, 8, 11, 2),
        (3, 7, 11, 3),
        (4, 6, 11, 2),
        (5, 5, 11, 3),
        (6, 4, 11, 3),
        (7, 3, 11, 4),
        (8, 2, 11, 2),
        (9, 1, 11, 3),
        (0, 9, 11, 2),
        (1, 8, 11, 3),
        (2, 7, 11, 3),
        (3, 6, 11, 4),
        (4, 5, 11, 3),
        (5, 4, 11, 4),
        (6, 3, 11, 4),
        (7, 2, 11, 5),
        (8, 1, 11, 3),
        (0, 8, 11, 1),
        (1, 7, 11, 2),
        (2, 6, 11, 2),
        (3, 5, 11, 3),
        (4, 4, 11, 2),
        (5, 3, 11, 3),
        (6, 2, 11, 3),
        (7, 1, 11, 4),
        (0, 7, 11, 2),
        (1, 6, 11, 3),
        (2, 5, 11, 3),
        (3, 4, 11, 4),
        (4, 3, 11, 3),
        (5, 2, 11, 4),
        (6, 1, 11, 4),
        (0, 6, 11, 2),
        (1, 5, 11, 3),
        (2, 4, 11, 3),
        (3, 3, 11, 4),
        (4, 2, 11, 3),
        (5, 1, 11, 4),
        (0, 5, 11, 3),
        (1, 4, 11, 4),
        (2, 3, 11, 4),
        (3, 2, 11, 5),
        (4, 1, 11, 4),
        (0, 4, 11, 2),
        (1, 3, 11, 3),
        (2, 2, 11, 3),
        (3, 1, 11, 4),
        (0, 3, 11, 3),
        (1, 2, 11, 4),
        (2, 1, 11, 4),
        (0, 2, 11, 3),
        (1, 1, 11, 4),
        (0, 1, 11, 4),
        (0, 0, 11, 0),
        (0, 6, 6, 0),
        (1, 5, 6, 1),
        (2, 4, 6, 1),
        (3, 3, 6, 2),
        (4, 2, 6, 1),
        (5, 1, 6, 2),
        (6, 0, 6, 0),
        (0, 5, 6, 1),
        (1, 4, 6, 2),
        (2, 3, 6, 2),
        (3, 2, 6, 3),
        (4, 1, 6, 2),
        (0, 3, 6, 2),
        (1, 2, 6, 3),
        (2, 1, 6, 3),
        (3, 0, 6, 0),
        (0, 2, 6, 2),
        (1, 1, 6, 3),
        (2, 0, 6, 0),
        (0, 1, 6, 3),
        (1, 0, 6, 0),
    ];

    for (first, width, total, expected) in cases {
        assert_eq!(
            labels_in_drop_slice(first, width, total, 0, 0),
            expected,
            "{first}, {width}, {total}: {expected}"
        );
    }
}
