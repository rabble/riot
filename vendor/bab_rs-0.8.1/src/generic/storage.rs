//! String storage, with efficient hash computation and [optimised](https://bab-hash.org/spec#optimizations) [verifiable streaming](https://bab-hash.org/spec#streaming_verification).
//!
//! This module provides the most complete (and thus complex) APIs for Bab. Instead of simply hashing strings, the functionality in this module *stores* strings, together with the [Merkle tree used in Bab](https://bab-hash.org/spec#tree). Such storage can be updated via [verifiablabe slice streams](https://bab-hash.org/spec#slice_verification), and it can emit verifiable slice streams for subslices of the stored strings.
//!
//! Before you can use this API, you need to make a couple of choices. The first choice is selecting a *storage backend*. All functionality in this module is generic over different kinds of backing storage. We provide an [in-memory backend](backend_memory) (not persistent) and a [file-system backend](backend_filesystem) (persistent) out of the box; you can write your own backends by implementing the [`StorageBackend`] trait. Notably, this trait is not aware of any Bab-specific functionality, it merely provides access to a flat array of bytes.
//!
//! On top of the backend, you then select a suitable *Bab store*. The store introduces Bab-specific functionality, such as ingesting or emitting [verifiable streams](https://bab-hash.org/spec#streaming_verification). We provide two kinds of stores: the [`SingleSliceStore`], which stores exactly one contiguous subslice of a string, and the [`MultiSliceStore`], which stores an arbitrary number of non-overlapping subslices of a string.
//!
//! The types for describing slices and optimisation parameters for verifiable streaming are provided in the [`verifiable_streaming`] module.

pub mod storage_backend;
pub use storage_backend::StorageBackend;

#[cfg(feature = "storage-fs")]
pub mod backend_filesystem;

pub mod backend_memory;

pub mod single_slice_store;
pub use single_slice_store::SingleSliceStore;

pub mod multi_slice_store;
pub use multi_slice_store::MultiSliceStore;

pub mod verifiable_streaming;

pub mod units;
