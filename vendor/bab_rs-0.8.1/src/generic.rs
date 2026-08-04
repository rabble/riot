//! The [Bab](https://bab-hash.org/) family of hash functions, generically: you determine details such as digest sizes and hash computation through explicit parameters.
//!
//! We provide three different styles of APIs:
//!
//! - [`batch_hash`] is a simple function that takes a slice of bytes and hashes it,
//! - [`BabHasher::new`] is the entrypoint for stateful hashing analogous to [`std::hash`], and
//! - the [`storage`] module provides the most sophisticated APIs, with support for [verified streaming](https://bab-hash.org/spec#streaming_verification).
//!
//! Most functionality in this module requires a [`BabInstantiation`] to make concrete the [generic parameters](https://bab-hash.org/spec#parameters) of Bab.

/// A convenient type alias for the labels of the Merkle tree used by Bab.
pub type Label<const WIDTH: usize> = [u8; WIDTH];

/// A convenient type alias for a Bab chunk of data.
pub type Chunk<const CHUNK_SIZE: usize> = [u8; CHUNK_SIZE];

/// An instantiation of the `hash_chunk` spec parameter, with immutable access to a value of type `HashChunkContext`.
/// The computed tree node label must be written into the final argument.
pub type HashChunk<const WIDTH: usize, HashChunkContext> =
    fn(&[u8], bool, &HashChunkContext, &mut [u8; WIDTH]);

/// An instantiation of the `hash_inner` spec parameter, with immutable access to a value of type `HashInnerContext`.
/// The computed tree node label must be written into the final argument.
pub type HashInner<const WIDTH: usize, HashInnerContext> =
    fn(&[u8; WIDTH], &[u8; WIDTH], u64, bool, &HashInnerContext, &mut [u8; WIDTH]);

/// A specific instantiation of all [Bab parameters](https://bab-hash.org/spec#parameters), together with specific contexts to supply when determining Merkle tree labels.
#[derive(Debug, Clone, Copy, Hash)]
pub struct BabInstantiation<
    const WIDTH: usize,
    const CHUNK_SIZE: usize,
    HashChunkContext,
    HashInnerContext,
> {
    /// The Bab [hash_chunk parameter](https://bab-hash.org/spec#hash_chunk).
    pub hash_chunk: HashChunk<WIDTH, HashChunkContext>,
    /// The Bab [hash_inner parameter](https://bab-hash.org/spec#hash_inner).
    pub hash_inner: HashInner<WIDTH, HashInnerContext>,
    /// The immutable context to pass to `hash_chunk`.
    ///
    /// For example, this could be a key used in keyed hashing.
    pub hash_chunk_context: HashChunkContext,
    /// The immutable context to pass to `hash_inner`.
    ///
    /// For example, this could be a key used in keyed hashing.
    pub hash_inner_context: HashInnerContext,
}

mod hasher;
pub use hasher::BabHasher;

mod batch;
pub use batch::batch_hash;

mod digest;
pub use digest::BabDigest;

#[cfg(feature = "storage")]
pub mod storage;
