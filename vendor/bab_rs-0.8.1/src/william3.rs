mod basics;
mod portable;

mod hasher;
pub use hasher::William3Hasher;

mod batch;
pub use batch::{batch_hash, batch_hash_keyed};

mod digest;
pub use digest::William3Digest;

#[cfg(feature = "storage")]
pub mod storage;

#[cfg(all(feature = "william3", feature = "storage"))]
use crate::generic::BabInstantiation;
use crate::william3::{
    basics::{BLOCK_LEN, CHUNK_END, CHUNK_START, IV, KEYED_HASH, PARENT, ROOT},
    portable::hash1,
};

/// The [number of bytes](https://bab-hash.org/spec#width) in a WILLIAM3 digest.
pub const WIDTH: usize = 32;

/// The [number of bytes](https://bab-hash.org/spec#chunk_size) in a WILLIAM3.
pub const CHUNK_SIZE: usize = 1024;

/// A convenient type alias for the labels of the Merkle tree used by Bab.
pub type Label<const WIDTH: usize> = [u8; WIDTH];

/// A convenient type alias for a Bab chunk of data.
pub type Chunk<const CHUNK_SIZE: usize> = [u8; CHUNK_SIZE];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HashChunkContext {
    key: Option<[u32; 8]>,
}

impl HashChunkContext {
    pub fn new() -> Self {
        Self { key: None }
    }

    pub fn new_keyed(key: [u32; 8]) -> Self {
        Self { key: Some(key) }
    }
}

pub(crate) fn hash_chunk(
    chunk: &[u8],
    is_root: bool,
    state: &HashChunkContext,
    output: &mut [u8; WIDTH],
) {
    let mut flags = 0;
    if state.key.is_some() {
        flags |= KEYED_HASH
    }

    let flags_start = CHUNK_START;

    let mut flags_end = CHUNK_END;
    if is_root {
        flags_end |= ROOT;
    }

    hash1(
        chunk,
        state.key.as_ref().unwrap_or(IV),
        0,
        flags,
        flags_start,
        flags_end,
        output,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HashInnerContext {
    key: Option<[u32; 8]>,
}

impl HashInnerContext {
    pub fn new() -> Self {
        Self { key: None }
    }

    pub fn new_keyed(key: [u32; 8]) -> Self {
        Self { key: Some(key) }
    }
}

pub(crate) fn hash_inner(
    left_label: &[u8; WIDTH],
    right_label: &[u8; WIDTH],
    length_of_subtree: u64,
    is_root: bool,
    state: &HashInnerContext,
    output: &mut [u8; WIDTH],
) {
    let mut flags = PARENT;
    if is_root {
        flags |= ROOT
    }

    let flags_start = 0;
    let flags_end = 0;

    let mut message_words = [0; BLOCK_LEN];
    message_words[..WIDTH].copy_from_slice(left_label);
    message_words[WIDTH..].copy_from_slice(right_label);

    hash1(
        &message_words[..],
        state.key.as_ref().unwrap_or(IV),
        length_of_subtree,
        flags,
        flags_start,
        flags_end,
        output,
    );
}

#[cfg(all(feature = "william3", feature = "storage"))]
pub(crate) fn william3_instantiation()
-> BabInstantiation<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext> {
    BabInstantiation {
        hash_chunk,
        hash_inner,
        hash_chunk_context: HashChunkContext { key: None },
        hash_inner_context: HashInnerContext { key: None },
    }
}
