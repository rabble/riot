//! Access control for Willow data.
//!
//! This module provides an implementation of [Meadowcap](https://willowprotocol.org/specs/meadowcap/), the recommended mechanism for scoping read and write access to entries. Meadowcap is a capability-based system with two seperate kinds of access control: *write capabilities* are required to author new Willow entries, and *read capabilities* can be used by transport protocols to determine whether a peer should grant access to its data to another peer.
//!
//! The [Meadowcap specification](https://willowprotocol.org/specs/meadowcap/) has all the details, but here is a rough overview for how writing entries works:
//!
//! ##### 1. Obtaining a secret key
//!
//! Access control is rooted in cryptography, more specifically, in a [digital signature scheme](https://en.wikipedia.org/wiki/Digital_signature). You prove your identity by posessing a secret key (of type [`SubspaceSecret`](crate::prelude::SubspaceSecret)). You can generate a new random secret with the [`entry::randomly_generate_subspace`](crate::entry::randomly_generate_subspace) function.
//!
//! ```
//! use rand::rngs::OsRng;
//! use willow25::prelude::*;
//!
//! let mut csprng = OsRng; // cryptographically secure pseudo-random number generator
//!
//! // Generate a new keypair, which roughly corresponds to a new user identity.
//! let (_subspace_id, _secret) = randomly_generate_subspace(&mut csprng);
//! ```
//!
//! ##### 2. Obtaining a write capability
//!
//! Whereas a [`SubspaceSecret`](crate::prelude::SubspaceSecret) lets you prove your identity, a [`WriteCapability`] proves that some identity is allowed to write new entries in a certain part of the three-dimensional Willow space. Every write capability has a *receiver* (the public key corresponding to a [`SubspaceSecret`](crate::prelude::SubspaceSecret), of type [`SubspaceId`](crate::prelude::SubspaceId), and an [`Area`](crate::prelude::Area) for which it grants access.
//!
//! You can create fresh write-capabilities with the [`WriteCapability::new_communal`] and [`WriteCapability::new_owned`] methods, or you can delegate an existing write capability via [`WriteCapability::delegate`]. When delegating, you can change the *receiver*, allowing you to give other people access to your data. You can also restrict the granted area when delegating.
//!
//! ```
//! use rand::rngs::OsRng;
//! use willow25::{prelude::*, authorisation::*};
//!
//! # #[cfg(feature = "dev")] {
//! let mut csprng = OsRng;
//! let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
//! let namespace_id = NamespaceId::from_bytes(&[17; 32]);
//! let new_receiver = SubspaceId::from_bytes(&[18; 32]);
//!
//! // Create a new capability from scratch. To use it, you need to know the `secret`.
//! let mut cap = WriteCapability::new_communal(
//!     namespace_id,
//!     subspace_id.clone(),
//! );
//!
//! // Using the `secret`, we can delegate it to someone else.
//! // To use the delegated capability for authoring, you would need to know
//! // the secret corresponding to the `new_receiver`.
//! cap.delegate(
//!     &secret,
//!     Area::new_subspace_area(subspace_id.clone()),
//!     new_receiver.clone(),
//! );
//! # }
//! ```
//!
//! ##### 3. Creating authorised entries
//!
//! Once you have a write capability and the secret key corresponding to its receiver, you can easily create [`AuthorisedEntries`](AuthorisedEntry) via [`Entry::into_authorised_entry`](crate::prelude::Entry::into_authorised_entry). Unlike regular [`Entries`](crate::prelude::Entry), [`AuthorisedEntries`](AuthorisedEntry) contain cryptographic proof of their authenticity — they are the only entries that will be accepted by other peers.
//!
//! ```
//! use rand::rngs::OsRng;
//! use willow25::prelude::*;
//! use willow25::authorisation::PossiblyAuthorisedEntry;
//!
//! # #[cfg(feature = "dev")] {
//! let mut csprng = OsRng;
//! let (subspace_id, secret) = randomly_generate_subspace(&mut csprng);
//! let namespace_id = NamespaceId::from_bytes(&[17; 32]);
//!
//! let mut cap = WriteCapability::new_communal(
//!     namespace_id.clone(),
//!     subspace_id.clone(),
//! );
//!
//! // We have a capability and the secret for its receiver. Time to write an authenticated entry.
//!
//! let entry = Entry::builder()
//!     .namespace_id(namespace_id.clone())
//!     .subspace_id(subspace_id.clone())
//!     .path(path!("/ideas"))
//!     .timestamp(12345)
//!     .payload(b"chocolate with mustard")
//!     .build();
//!
//! let authed = entry.into_authorised_entry(&cap, &secret);
//! assert!(authed.is_ok());
//! # }
//! ```

pub mod raw;

mod write_capability;
pub use write_capability::*;

mod read_capability;
pub use read_capability::*;

mod authorisation_token;
pub use authorisation_token::*;

mod authorised_entry;
pub use authorised_entry::*;
