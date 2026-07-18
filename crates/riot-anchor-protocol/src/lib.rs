//! Canonical wire layer for the Riot public community anchor network.
//!
//! `riot-anchor-protocol` is the dependency-neutral crate that owns the shared
//! canonical frames, records, state enums, digests, and codecs of the anchor
//! network. It is consumed by the client internet runtime (`riot-client-net`),
//! the anchor daemon (`riot-anchor`), and — through conformance vectors — by the
//! native shells. To keep the verify/project core reusable everywhere (including
//! `wasm32`), this crate never depends on SQLite, HTTP, iroh, Tokio, a renderer,
//! FFI, or any platform adapter; it depends on [`riot_core`] with default
//! features disabled. `tests/dependency_boundary.rs` fails the build if that
//! contract is ever broken.
//!
//! Later work units add the codec, digest, authority, control, sync, directory,
//! and peer modules; each declares its own `pub mod` here in the same commit
//! that creates the module, so the crate stays buildable at every commit.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
