//! UniFFI boundary for the Riot public-data kernel.
//!
//! The conference API presents typed mobile records and opaque handles. Core
//! signer, session, store, preview, and plan values remain private to this
//! crate.

mod apps_ffi;
mod community_registry;
mod demo_ffi;
mod mobile_api;
mod mobile_state;
mod newswire_ffi;
mod profile_ffi;

pub use apps_ffi::*;
pub use mobile_api::*;
pub use newswire_ffi::*;
pub use profile_ffi::*;

uniffi::setup_scaffolding!();
