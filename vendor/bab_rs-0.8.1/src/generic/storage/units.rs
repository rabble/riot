//! Type aliases for some units/quantities, and conversion between them.
//!
//! The type aliases add no actual type safety, they are merely meant to make our APIs more explicit.

/// Some whole number of chunks.
pub type ChunkCount = u64;

/// The zero-based index of a chunk of the stored string.
pub type ChunkIndex = ChunkCount;

/// The numbering of a Merkle tree node in depth-first order (numbering the root at zero).
pub type NodeNumber = u64;

/// The length of a path in the Merkle tree, starting in the root. The path consisting only of the root itself has length zero.
pub type RootedPathLength = u8;

/// A [layer](https://bab-hash.org/spec#layer) in the Merkle tree.
pub type Layer = u8;

/// Some whole number of bytes.
pub type ByteCount = u64;

/// A byte-based offset into a the data stored by a [`StorageBackend`](crate::generic::storage::storage_backend::StorageBackend).
pub type ByteIndex = ByteCount;

/// Converts a [`ChunkIndex`] into a [`NodeNumber`], given the total number of chunks in the tree.
///
/// Panics if `chunk_index >= chunk_count`.
pub fn chunk_index_to_node_number(chunk_index: ChunkIndex, chunk_count: ChunkCount) -> NodeNumber {
    assert!(chunk_index < chunk_count);

    if chunk_count == 1 {
        // trivial tree, trivial solution
        return 0;
    } else {
        let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

        if chunk_index < left_subtree_leaf_count {
            // chunk is in left subtree of the root (which is a perfect subtree)
            return 1 + chunk_index_to_node_number(chunk_index, left_subtree_leaf_count);
        } else {
            // chunk is in right subtree of the root
            return left_subtree_leaf_count * 2
                + chunk_index_to_node_number(
                    chunk_index - left_subtree_leaf_count,
                    chunk_count - left_subtree_leaf_count,
                );
        }
    }
}

#[test]
fn test_chunk_index_to_node_number() {
    let expected = [
        // The i-th slice lists the NodeNumber for the leaves of the trees on i + 1 leaves.
        &[0u64][..],
        &[1, 2][..],
        &[2, 3, 4][..],
        &[2, 3, 5, 6][..],
        &[3, 4, 6, 7, 8][..],
        &[3, 4, 6, 7, 9, 10][..],
        &[3, 4, 6, 7, 10, 11, 12][..],
        &[3, 4, 6, 7, 10, 11, 13, 14][..],
        &[4, 5, 7, 8, 11, 12, 14, 15, 16][..],
    ];

    for (i, testdata) in expected.iter().enumerate() {
        for (chunk_index, node_number) in testdata.iter().enumerate() {
            assert_eq!(
                chunk_index_to_node_number(chunk_index as ChunkIndex, (i as u64) + 1),
                *node_number,
            );
        }
    }
}

/// Converts a [`NodeNumber`] into a [`ChunkIndex`], given the total number of chunks in the tree.
///
/// More specifically, this returns the chunk index of the leftmost leaf of the subtree rooted at the given node number.
///
/// Panics if the node number is too great for the given chunk_count.
pub fn node_number_to_chunk_index(node_number: ChunkIndex, chunk_count: ChunkCount) -> NodeNumber {
    if node_number > chunk_count.saturating_mul(2) {
        panic!("node_number greater than possible for the given chunk_count");
    }

    node_number_to_chunk_index_(node_number, chunk_count)
}

fn node_number_to_chunk_index_(node_number: ChunkIndex, chunk_count: ChunkCount) -> NodeNumber {
    if node_number == 0 {
        // Trivial case.
        return 0;
    } else {
        let left_subtree_node_count = chunk_count.next_power_of_two() - 1;
        let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

        if node_number <= left_subtree_node_count {
            // node_number is in the left subtree.
            return node_number_to_chunk_index(node_number - 1, left_subtree_leaf_count);
        } else {
            return left_subtree_leaf_count
                + node_number_to_chunk_index(
                    node_number - (left_subtree_node_count + 1),
                    chunk_count - left_subtree_leaf_count,
                );
        }
    }
}

#[test]
fn test_node_number_to_chunk_index() {
    let expected = [
        &[0u64][..],
        &[0, 0, 1][..],
        &[0, 0, 0, 1, 2][..],
        &[0, 0, 0, 1, 2, 2, 3][..],
        &[0, 0, 0, 0, 1, 2, 2, 3, 4][..],
        &[0, 0, 0, 0, 1, 2, 2, 3, 4, 4, 5][..],
        &[0, 0, 0, 0, 1, 2, 2, 3, 4, 4, 4, 5, 6][..],
        &[0, 0, 0, 0, 1, 2, 2, 3, 4, 4, 4, 5, 6, 6, 7][..],
        &[0, 0, 0, 0, 0, 1, 2, 2, 3, 4, 4, 4, 5, 6, 6, 7, 8][..],
    ];

    for (i, testdata) in expected.iter().enumerate() {
        for (node_number, expected_chunk_index) in testdata.iter().enumerate() {
            // println!("node_number {node_number}, chunk_count {}", i + 1);

            assert_eq!(
                node_number_to_chunk_index(node_number as NodeNumber, (i + 1) as ChunkCount),
                *expected_chunk_index,
            );
        }
    }
}

/// Converts a [`NodeNumber`] into the `left_skip` for requesting a verifiable slice stream starting at the given node number.
///
/// This is the same as the height of the tree minus the layer of the given node_number.
pub(crate) fn node_number_to_left_skip(node_number: NodeNumber, chunk_count: ChunkCount) -> u8 {
    if node_number == 0 {
        // Trivial case.
        return 0;
    } else {
        let left_subtree_node_count = chunk_count.next_power_of_two() - 1;
        let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

        if node_number <= left_subtree_node_count {
            // node_number is in the left subtree.
            return 1 + node_number_to_left_skip(node_number - 1, left_subtree_leaf_count);
        } else {
            return 1 + node_number_to_left_skip(
                node_number - (left_subtree_node_count + 1),
                chunk_count - left_subtree_leaf_count,
            );
        }
    }
}

#[test]
fn test_node_number_to_left_skip() {
    let expected = [
        &[0u8][..],
        &[0, 1, 1][..],
        &[0, 1, 2, 2, 1][..],
        &[0, 1, 2, 2, 1, 2, 2][..],
        &[0, 1, 2, 3, 3, 2, 3, 3, 1][..],
        &[0, 1, 2, 3, 3, 2, 3, 3, 1, 2, 2][..],
        &[0, 1, 2, 3, 3, 2, 3, 3, 1, 2, 3, 3, 2][..],
        &[0, 1, 2, 3, 3, 2, 3, 3, 1, 2, 3, 3, 2, 3, 3][..],
        &[0, 1, 2, 3, 4, 4, 3, 4, 4, 2, 3, 4, 4, 3, 4, 4, 1][..],
    ];

    for (i, testdata) in expected.iter().enumerate() {
        for (node_number, expected_left_skip) in testdata.iter().enumerate() {
            // println!("node_number {node_number}, chunk_count {}", i + 1);

            assert_eq!(
                node_number_to_left_skip(node_number as NodeNumber, (i + 1) as ChunkCount),
                *expected_left_skip,
            );
        }
    }
}

/// Converts a [`NodeNumber`] into the `right_skip` for requesting a verifiable slice stream starting at the given node number and ending at the given final_chunk_of_slice (inclusive). The given NodeNumber is the greatest node number for which *no* data is available yet.
pub(crate) fn node_number_to_right_skip(
    node_number: NodeNumber,
    final_chunk_of_slice: ChunkIndex,
    chunk_count: ChunkCount,
) -> u8 {
    let final_node_number = chunk_index_to_node_number(final_chunk_of_slice, chunk_count);
    node_number_to_right_skip_(node_number, final_node_number, chunk_count)
}

fn node_number_to_right_skip_(
    node_number: NodeNumber,
    final_node_number: NodeNumber,
    chunk_count: ChunkCount,
) -> u8 {
    if node_number == 0 {
        // Trivial case.
        return 0;
    } else {
        let left_subtree_node_count = chunk_count.next_power_of_two() - 1;
        let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;

        if node_number <= left_subtree_node_count {
            // node_number is in the left subtree.

            if final_node_number > left_subtree_node_count {
                // The final chunk is in the right subtree.
                // Since node_number is not zero, the root can be skipped. But since the final chunk is in the right subtree whereas the given node_number is in the left, nothing else is shared. Hence, return exactly 1.
                return 1;
            } else {
                return 1 + node_number_to_right_skip_(
                    node_number - 1,
                    final_node_number - 1,
                    left_subtree_leaf_count,
                );
            }
        } else {
            // node_number is in the right subtree, and so must be the final chunk.
            return 1 + node_number_to_right_skip_(
                node_number - (left_subtree_node_count + 1),
                final_node_number - (left_subtree_node_count + 1),
                chunk_count - left_subtree_leaf_count,
            );
        }
    }
}

#[test]
fn test_node_number_to_right_skip() {
    // Expected right_skips for a tree on six leaves.
    // outer array members are progressive node numbers from 0 to 10,
    // inner slice items are the right_skips for all valid final_chunk_of_slice values.
    let expected = [
        &[0, 0, 0, 0, 0, 0][..],
        &[1, 1, 1, 1, 1, 1][..],
        &[2, 2, 2, 2, 1, 1][..],
        &[3, 3, 2, 2, 1, 1][..],
        &[3, 2, 2, 1, 1][..],
        &[2, 2, 1, 1][..],
        &[3, 3, 1, 1][..],
        &[3, 1, 1][..],
        &[1, 1][..],
        &[2, 2][..],
        &[2][..],
    ];

    for (i, testdata) in expected.iter().enumerate() {
        for (j, expected_right_skip) in testdata.iter().enumerate() {
            let final_chunk_of_slice = (j + (6 - testdata.len())) as ChunkIndex;
            // println!(
            //     "node_number {}, final_chunk_of_slice {final_chunk_of_slice}, chunk_count 6",
            //     i,
            // );

            assert_eq!(
                node_number_to_right_skip(i as NodeNumber, final_chunk_of_slice, 6),
                *expected_right_skip,
            );
        }
    }
}

/// Returns the chunk count for a string of the given length, assuming a chunk size of `CHUNK_SIZE` bytes.
pub fn string_length_to_chunk_count<const CHUNK_SIZE: usize>(len: ByteCount) -> ChunkCount {
    if len == 0 {
        1
    } else {
        len.div_ceil(CHUNK_SIZE as u64)
    }
}

/// Computes the greatest possible `left_skip` you can set when requesting a slice starting at chunk index `next_start` assuming you already have all bab metadata for a slice ending at chunk index `prior_end`, in a string of `chunk_count` many chunks. Contract: expects that `prior_end < next_start`.
pub fn optimal_left_skip(
    mut chunk_count: ChunkCount,
    mut prior_end: ChunkIndex,
    mut next_start: ChunkIndex,
) -> RootedPathLength {
    debug_assert!(prior_end < next_start);

    let mut left_skip = 1;

    loop {
        let left_subtree_leaf_count = chunk_count.next_power_of_two() / 2;
        let right_subtree_leaf_count = chunk_count - left_subtree_leaf_count;

        if next_start < left_subtree_leaf_count {
            debug_assert!(prior_end < left_subtree_leaf_count);

            chunk_count -= right_subtree_leaf_count;

            left_skip += 1;
        } else if prior_end >= left_subtree_leaf_count {
            debug_assert!(next_start >= left_subtree_leaf_count);

            chunk_count -= left_subtree_leaf_count;
            prior_end -= left_subtree_leaf_count;
            next_start -= left_subtree_leaf_count;

            left_skip += 1;
        } else {
            debug_assert!(
                prior_end < left_subtree_leaf_count && next_start >= left_subtree_leaf_count
            );
            return left_skip;
        }
    }
}

#[test]
fn test_optimal_left_skip() {
    // (chunk_count, prior_end, next_start, expected_optimal_left_skip)
    let expected = [
        (2, 0, 1, 1),
        (6, 0, 1, 3),
        (6, 0, 2, 2),
        (6, 0, 3, 2),
        (6, 0, 4, 1),
        (6, 0, 5, 1),
        (6, 1, 2, 2),
        (6, 1, 3, 2),
        (6, 1, 4, 1),
        (6, 1, 5, 1),
        (6, 2, 3, 3),
        (6, 2, 4, 1),
        (6, 2, 5, 1),
        (6, 3, 4, 1),
        (6, 3, 5, 1),
        (6, 4, 5, 2),
        (8, 0, 1, 3),
        (8, 0, 2, 2),
        (8, 0, 3, 2),
        (8, 0, 4, 1),
        (8, 0, 5, 1),
        (8, 0, 6, 1),
        (8, 0, 7, 1),
        (8, 1, 2, 2),
        (8, 1, 3, 2),
        (8, 1, 4, 1),
        (8, 1, 5, 1),
        (8, 1, 6, 1),
        (8, 1, 7, 1),
        (8, 2, 3, 3),
        (8, 2, 4, 1),
        (8, 2, 5, 1),
        (8, 2, 6, 1),
        (8, 2, 7, 1),
        (8, 3, 4, 1),
        (8, 3, 5, 1),
        (8, 3, 6, 1),
        (8, 3, 7, 1),
        (8, 4, 5, 3),
        (8, 4, 6, 2),
        (8, 4, 7, 2),
        (8, 5, 6, 2),
        (8, 5, 7, 2),
        (8, 6, 7, 3),
    ];

    for (chunk_count, prior_end, next_start, expected_optimal_left_skip) in expected.into_iter() {
        assert_eq!(
            optimal_left_skip(chunk_count, prior_end, next_start),
            expected_optimal_left_skip,
            "chunk_count {chunk_count}, prior_end {prior_end}, next_start {next_start}, expected_optimal_left_skip {expected_optimal_left_skip}"
        );
    }
}
