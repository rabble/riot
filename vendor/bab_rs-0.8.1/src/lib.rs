#![cfg_attr(not(feature = "std"), no_std)]

//! An implementation of the [Bab](https://bab-hash.org/) family of hash functions.
//!
//! The crate root exposes the [WILLIAM3](https://bab-hash.org/spec#instantiations_william) instantiation of Bab, which is a concrete hash function you can use immediately. The [`generic`] module provides parmaterisable implementations of Bab, which you can use to define your own hash functions.
//!
//! ```
//! # #[cfg(feature = "william3")] {
//! use bab_rs::{batch_hash, William3Hasher, William3Digest, WIDTH, Hasher, HasherWrite};
//! let mut hasher = William3Hasher::new();
//! hasher.write(&[0, 1, 2]);
//! hasher.write(&[3, 4]);
//! let incrementally_computed_hash = hasher.finish();
//!
//! let mut batch_digest = William3Digest::default();
//! batch_hash(&[0, 1, 2, 3, 4], &mut batch_digest);
//!
//! assert_eq!(
//!     hasher.finish(),
//!     batch_digest,
//! );
//! # }
//! ```
//!
//! ## Features
//!
//! The `william3` feature controls whether the implementations of WILLIAM3 are included. This feature is enabled by default.
//!
//! The `dev` feature adds implementations of the [`Arbitrary`](https://docs.rs/arbitrary/latest/arbitrary/trait.Arbitrary.html) trait to various types. This feature is not enabled by default.
//!
//! ## WILLIAM3 APIs
//!
//! We provide three different styles of APIs to use the [WILLIAM3](https://bab-hash.org/spec#instantiations_william) hash function:
//!
//! - [`batch_hash`] and [`batch_hash_keyed`] are simple functions that takes a slice of bytes and hashes it,
//! - [`William3Hasher::new`] and [`William3Hasher::new_keyed`] are the entrypoints for stateful hashing analogous to [`std::hash`], and
//! - the [`storage`] module provides the most sophisticated APIs, with support for [verified streaming](https://bab-hash.org/spec#streaming_verification).

pub mod generic;

pub use anyhash::{Hasher, HasherWrite};

#[cfg(feature = "william3")]
mod william3;
#[cfg(feature = "william3")]
pub use william3::*;
