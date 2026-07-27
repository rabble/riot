//! Functionality around Willow [Entries](https://willowprotocol.org/specs/data-model/index.html#Entry).
//!
//! Entries are the metadata by which payload strings are identified in Willow — see the [`Entry`] struct for more information.
//!
//! Applications can construct concrete entries via [`EntryBuilders`](EntryBuilder). Code which merely needs to inspect entries should make use of the [`Entrylike`] trait, however. This trait describes exactly which information an entry provides, and lets code abstract over any *specific* representation (such as the [`Entry`] struct) of entries.
//!
//! This module further provides some concrete types for the different fields of entries: [`NamespaceId`] for the [namespace_id](https://willowprotocol.org/specs/data-model/index.html#entry_namespace_id), [`SubspaceId`] for the [subspace_id](https://willowprotocol.org/specs/data-model/index.html#entry_subspace_id), [`Timestamp`] for the [timestamp](https://willowprotocol.org/specs/data-model/index.html#entry_timestamp), and [`PayloadDigest`] for the [payload_digest](https://willowprotocol.org/specs/data-model/index.html#entry_payload_digest).
//!
//! ```
//! use willow25::prelude::*;
//! # #[cfg(feature = "std")] {
//!
//! // Create an entry for a specific payload, timestamped with the current time.
//! let entry = Entry::builder()
//!     .namespace_id([0; 32].into())
//!     .subspace_id([1; 32].into())
//!     .path(path!("/vacation/plan"))
//!     .now().expect("`std` should be able to give the current time")
//!     .payload(b"See many sights!")
//!     .build();
//!
//! // Create a new entry based on the previous one, with greater timestamp and another payload.
//! let updated = Entry::prefilled_builder(&entry)
//!     .timestamp(entry.timestamp() + 5.minutes())
//!     .payload(b"See *all* the sights!")
//!     .build();
//!
//! // Assert that the new entry would overwrite the old entry if both were inserted in a store.
//! assert!(updated.prunes(&entry));
//! # }
//! ```
use core::fmt;

pub use willow_data_model::Timestamp;

#[allow(clippy::module_inception)]
mod entry;
pub use entry::*;

mod entrylike;
pub use entrylike::*;

mod builder;
pub use builder::*;

mod namespace_id;
pub use namespace_id::*;

mod subspace_id;
pub use subspace_id::*;

mod payload_digest;
pub use payload_digest::*;

mod ed25519;

struct HexFormatter<const N: usize>([u8; N]);

impl<const N: usize> fmt::Debug for HexFormatter<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = const_hex::Buffer::<N>::new();
        let printed = buffer.format(&self.0);
        f.write_str(printed)
    }
}
