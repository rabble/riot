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
#[cfg(feature = "net")]
pub mod net;
mod newswire_ffi;
mod profile_ffi;
mod site_ffi;

pub use apps_ffi::*;
pub use mobile_api::*;
pub use newswire_ffi::*;
pub use profile_ffi::*;
pub use site_ffi::*;

uniffi::setup_scaffolding!();
