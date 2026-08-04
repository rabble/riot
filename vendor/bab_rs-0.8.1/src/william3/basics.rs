/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// All of the below is copied and slightly adapted from <https://github.com/BLAKE3-team/BLAKE3/blob/master/src/lib.rs> //
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// The number of bytes in a block, 64.
///
/// You don't usually need to think about this number. One case where it matters is calling
/// [`OutputReader::fill`] in a loop, where using a `buf` argument that's a multiple of `BLOCK_LEN`
/// avoids repeating work.
pub(crate) const BLOCK_LEN: usize = 64;

// While iterating the compression function within a chunk, the CV is
// represented as words, to avoid doing two extra endianness conversions for
// each compression in the portable implementation. But the hash_many interface
// needs to hash both input bytes and parent nodes, so its better for its
// output CVs to be represented as bytes.
pub(crate) type CVWords = [u32; 8];
pub(crate) type CVBytes = [u8; 32]; // little-endian

// These are different for WILLIAM3 than for BLAKE3!
pub(crate) const IV: &CVWords = &[
    0xc88f633b, 0x4168fbf2, 0x6ba32583, 0xb0ff1847, 0xac57e47d, 0xa8931330, 0x796a4645,
    0x6b28a3ee,
    // 0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19, // the BLAKE3 IVs
];

pub(crate) const MSG_SCHEDULE: [[usize; 16]; 7] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8],
    [3, 4, 10, 12, 13, 2, 7, 14, 6, 5, 9, 0, 11, 15, 8, 1],
    [10, 7, 12, 9, 14, 3, 13, 15, 4, 0, 11, 2, 5, 8, 1, 6],
    [12, 13, 9, 11, 15, 10, 14, 8, 7, 2, 5, 3, 0, 1, 6, 4],
    [9, 14, 11, 5, 8, 12, 15, 1, 13, 3, 0, 10, 2, 6, 4, 7],
    [11, 15, 5, 0, 1, 9, 8, 6, 14, 10, 2, 12, 3, 4, 7, 13],
];

// These are the internal flags that we use to domain separate root/non-root,
// chunk/parent, and chunk beginning/middle/end. These get set at the high end
// of the block flags word in the compression function, so their values start
// high and go down.
pub(crate) const CHUNK_START: u8 = 1 << 0;
pub(crate) const CHUNK_END: u8 = 1 << 1;
pub(crate) const PARENT: u8 = 1 << 2;
pub(crate) const ROOT: u8 = 1 << 3;
pub(crate) const KEYED_HASH: u8 = 1 << 4;

#[inline]
pub(crate) fn counter_low(counter: u64) -> u32 {
    counter as u32
}

#[inline]
pub(crate) fn counter_high(counter: u64) -> u32 {
    (counter >> 32) as u32
}
