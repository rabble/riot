//! Storage for Willow entries.
//!
//! This module provides traits for abstracting over concrete storage backends for [Willow stores](https://willowprotocol.org/specs/data-model/index.html#store), and it provides two concrete store implementations: a non-persistent, in-memory store ([`MemoryStore`]), and a persistent store ([`PersistentStore`]).
//!
//! An overview of the different traits:
//!
//! - The [`Store`] trait provides the most basic functionality: inserting and querying entries.
//! - The [`PayloadPrefixStore`] can handle partial payloads: at all times, for every entry, it stores an arbitrary prefix of that entry's payload.

mod store;
pub use store::*;

mod payload_prefix_store;
pub use payload_prefix_store::*;

#[cfg(feature = "std")]
mod memory_store;
#[cfg(feature = "std")]
pub use memory_store::*;

#[cfg(feature = "std")]
mod persistent_store;
#[cfg(feature = "std")]
pub use persistent_store::*;
