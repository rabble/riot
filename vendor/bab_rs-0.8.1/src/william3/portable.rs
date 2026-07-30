// Provides a portable implementation of the compression function of WILLIAM3.
// Code adapted from https://github.com/BLAKE3-team/BLAKE3/blob/master/src/portable.rs

use core::cmp::min;

use crate::william3::basics::{
    BLOCK_LEN, CVBytes, CVWords, IV, MSG_SCHEDULE, counter_high, counter_low,
};
use arrayref::{array_mut_ref, array_ref};

#[inline(always)]
fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(x);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(y);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

#[inline(always)]
fn round(state: &mut [u32; 16], msg: &[u32; 16], round: usize) {
    // Select the message schedule based on the round.
    let schedule = MSG_SCHEDULE[round];

    // Mix the columns.
    g(state, 0, 4, 8, 12, msg[schedule[0]], msg[schedule[1]]);
    g(state, 1, 5, 9, 13, msg[schedule[2]], msg[schedule[3]]);
    g(state, 2, 6, 10, 14, msg[schedule[4]], msg[schedule[5]]);
    g(state, 3, 7, 11, 15, msg[schedule[6]], msg[schedule[7]]);

    // Mix the diagonals.
    g(state, 0, 5, 10, 15, msg[schedule[8]], msg[schedule[9]]);
    g(state, 1, 6, 11, 12, msg[schedule[10]], msg[schedule[11]]);
    g(state, 2, 7, 8, 13, msg[schedule[12]], msg[schedule[13]]);
    g(state, 3, 4, 9, 14, msg[schedule[14]], msg[schedule[15]]);
}

#[inline(always)]
fn compress_pre(
    cv: &CVWords,
    block: &[u8; BLOCK_LEN],
    block_len: u8,
    counter: u64,
    flags: u8,
) -> [u32; 16] {
    let block_words = words_from_le_bytes_64(block);

    let mut state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter_low(counter),
        counter_high(counter),
        block_len as u32,
        flags as u32,
    ];

    round(&mut state, &block_words, 0);
    round(&mut state, &block_words, 1);
    round(&mut state, &block_words, 2);
    round(&mut state, &block_words, 3);
    round(&mut state, &block_words, 4);
    round(&mut state, &block_words, 5);
    round(&mut state, &block_words, 6);

    state
}

pub(crate) fn compress_in_place(
    cv: &mut CVWords,
    block: &[u8; BLOCK_LEN],
    block_len: u8,
    counter: u64,
    flags: u8,
) {
    let state = compress_pre(cv, block, block_len, counter, flags);

    cv[0] = state[0] ^ state[8];
    cv[1] = state[1] ^ state[9];
    cv[2] = state[2] ^ state[10];
    cv[3] = state[3] ^ state[11];
    cv[4] = state[4] ^ state[12];
    cv[5] = state[5] ^ state[13];
    cv[6] = state[6] ^ state[14];
    cv[7] = state[7] ^ state[15];
}

/// Compress a single chunk.
// Compared to the BLAKE3 implementation this is based on, we added block padding for the final block in here.
pub fn hash1(
    input: &[u8],
    key: &CVWords,
    counter: u64,
    flags: u8,
    flags_start: u8,
    flags_end: u8,
    out: &mut CVBytes,
) {
    let mut cv = *key;
    let mut block_flags = flags | flags_start;
    let mut slice = input;

    if slice.len() == 0 {
        block_flags |= flags_end;

        let only_block = [0; BLOCK_LEN];
        compress_in_place(
            &mut cv,
            &only_block,
            0,
            counter,
            block_flags,
        );
    } else {
        while !slice.is_empty() {
            if slice.len() <= BLOCK_LEN {
                block_flags |= flags_end;

                // The slice len was not divisible by the BLOCK_LEN, and we reached the final block.
                // We need to pad it with zeroes.
                let mut final_block = [0; BLOCK_LEN];
                final_block[..slice.len()].copy_from_slice(slice);

                compress_in_place(
                    &mut cv,
                    &final_block,
                    slice.len() as u8,
                    counter,
                    block_flags,
                );
            } else {
                compress_in_place(
                    &mut cv,
                    array_ref!(slice, 0, BLOCK_LEN),
                    BLOCK_LEN as u8,
                    counter,
                    block_flags,
                );
            }

            block_flags = flags;
            slice = &slice[min(slice.len(), BLOCK_LEN)..];
        }
    }

    *out = le_bytes_from_words_32(&cv);
}

#[inline(always)]
pub fn words_from_le_bytes_64(bytes: &[u8; 64]) -> [u32; 16] {
    let mut out = [0; 16];
    out[0] = u32::from_le_bytes(*array_ref!(bytes, 0, 4));
    out[1] = u32::from_le_bytes(*array_ref!(bytes, 4, 4));
    out[2] = u32::from_le_bytes(*array_ref!(bytes, 2 * 4, 4));
    out[3] = u32::from_le_bytes(*array_ref!(bytes, 3 * 4, 4));
    out[4] = u32::from_le_bytes(*array_ref!(bytes, 4 * 4, 4));
    out[5] = u32::from_le_bytes(*array_ref!(bytes, 5 * 4, 4));
    out[6] = u32::from_le_bytes(*array_ref!(bytes, 6 * 4, 4));
    out[7] = u32::from_le_bytes(*array_ref!(bytes, 7 * 4, 4));
    out[8] = u32::from_le_bytes(*array_ref!(bytes, 8 * 4, 4));
    out[9] = u32::from_le_bytes(*array_ref!(bytes, 9 * 4, 4));
    out[10] = u32::from_le_bytes(*array_ref!(bytes, 10 * 4, 4));
    out[11] = u32::from_le_bytes(*array_ref!(bytes, 11 * 4, 4));
    out[12] = u32::from_le_bytes(*array_ref!(bytes, 12 * 4, 4));
    out[13] = u32::from_le_bytes(*array_ref!(bytes, 13 * 4, 4));
    out[14] = u32::from_le_bytes(*array_ref!(bytes, 14 * 4, 4));
    out[15] = u32::from_le_bytes(*array_ref!(bytes, 15 * 4, 4));
    out
}

#[inline(always)]
pub fn le_bytes_from_words_32(words: &[u32; 8]) -> [u8; 32] {
    let mut out = [0; 32];
    *array_mut_ref!(out, 0, 4) = words[0].to_le_bytes();
    *array_mut_ref!(out, 4, 4) = words[1].to_le_bytes();
    *array_mut_ref!(out, 2 * 4, 4) = words[2].to_le_bytes();
    *array_mut_ref!(out, 3 * 4, 4) = words[3].to_le_bytes();
    *array_mut_ref!(out, 4 * 4, 4) = words[4].to_le_bytes();
    *array_mut_ref!(out, 5 * 4, 4) = words[5].to_le_bytes();
    *array_mut_ref!(out, 6 * 4, 4) = words[6].to_le_bytes();
    *array_mut_ref!(out, 7 * 4, 4) = words[7].to_le_bytes();
    out
}
