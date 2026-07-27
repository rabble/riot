//! Hashing complete bytestrings in one go. The least flexible (but easiest to implement) form of Bab hashing.

#![allow(clippy::type_complexity)]

use crate::generic::{BabInstantiation, digest::BabDigest};

/// Computes a Bab digest of the given input bytes, and writes it into `out`.
///
/// This is the simplemost hashing API; it requires the full string to be available at once. See the [`BabHasher`](super::BabHasher) API for incremental hashing.
pub fn batch_hash<
    const WIDTH: usize,
    const CHUNK_SIZE: usize,
    HashChunkContext,
    HashInnerContext,
>(
    bab_instantiation: &BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    bytes: &[u8],
    out: &mut BabDigest<WIDTH>,
) {
    // Call a recursive helper function, instruct it to set the is_root flag to true for the digest it produces.
    do_batch_hash::<WIDTH, CHUNK_SIZE, _, _>(bab_instantiation, bytes, &mut out.0, true);
}

fn do_batch_hash<
    const WIDTH: usize,
    const CHUNK_SIZE: usize,
    HashChunkContext,
    HashInnerContext,
>(
    bab_instantiation: &BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
    bytes: &[u8],
    out: &mut [u8; WIDTH],
    is_root: bool,
) {
    if bytes.len() <= CHUNK_SIZE {
        // If the input fits in a single chunk, the digest is obtained by calling `hash_chunk`.
        (bab_instantiation.hash_chunk)(bytes, is_root, &bab_instantiation.hash_chunk_context, out);
    } else {
        // The input is larger than a single chunk. Time to build a Merkle tree, except we don't — we use a recursive
        // implementation with an implicit tree of function calls.
        // The idea is quite simple: we call `hash_inner`, and their first two arguments are computed by recursive calls.
        // The recursive calls set `is_root` to false.
        let mut left_child_label = [0; WIDTH];
        let mut right_child_label = [0; WIDTH];

        // For the recursive call, we split the input slice into two slices: left child covers everything up until the
        // greatest power of two strictly less than `bytes.len()`, and the right child covers everything else.
        // The greatest power of two strictly less than some `n` happens to be 2 to the power of the floored base-two
        // logarithm of `n`, unless `n` itself is a power of two, in which case we can simply use `n / 2`.
        let left_len = if bytes.len().is_power_of_two() {
            bytes.len() / 2
        } else {
            1 << bytes.len().ilog2()
        };

        // And that was the only difficult thing about all of this. We can do the recursive calls (setting is_root to false).
        do_batch_hash::<WIDTH, CHUNK_SIZE, _, _>(
            bab_instantiation,
            &bytes[..left_len],
            &mut left_child_label,
            false,
        );
        do_batch_hash::<WIDTH, CHUNK_SIZE, _, _>(
            bab_instantiation,
            &bytes[left_len..],
            &mut right_child_label,
            false,
        );

        // We have computed the labels of the two children, now we can compute the root label and are done.
        (bab_instantiation.hash_inner)(
            &left_child_label,
            &right_child_label,
            bytes.len() as u64,
            is_root,
            &bab_instantiation.hash_inner_context,
            out,
        );
    }
}
