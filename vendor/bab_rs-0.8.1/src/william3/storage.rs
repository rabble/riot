//! String storage, with efficient hash computation and [optimised](https://bab-hash.org/spec#optimizations) [verifiable streaming](https://bab-hash.org/spec#streaming_verification).
//!
//! This module provides the most complete (and thus complex) APIs for William3. Instead of simply hashing strings, the functionality in this module *stores* strings, together with the [Merkle tree used in William3](https://bab-hash.org/spec#tree). Such storage can be updated via [verifiablabe slice streams](https://bab-hash.org/spec#slice_verification), and it can emit verifiable slice streams for subslices of the stored strings.
//!
//! Before you can use this API, you need to make a couple of choices. The first choice is selecting a *storage backend*. All functionality in this module is generic over different kinds of backing storage. We provide an [in-memory backend](crate::generic::storage::backend_memory) (not persistent) and a [file-system backend](crate::generic::storage::backend_filesystem) (persistent) out of the box; you can write your own backends by implementing the [`StorageBackend`](crate::generic::storage::StorageBackend) trait. Notably, this trait is not aware of any William3-specific functionality, it merely provides access to a flat array of bytes.
//!
//! On top of the backend, you then select a suitable *William3 store*. The store introduces William3-specific functionality, such as ingesting or emitting [verifiable streams](https://bab-hash.org/spec#streaming_verification). We provide two kinds of stores: the [`SingleSliceStore`], which stores exactly one contiguous subslice of a string, and the [`MultiSliceStore`], which stores an arbitrary number of non-overlapping subslices of a string.
//!
//! Note that our implementation currently only suppoerts unkeyed William3 hashing.

mod single_slice_store;
pub use single_slice_store::SingleSliceStore;

mod multi_slice_store;
pub use multi_slice_store::MultiSliceStore;
