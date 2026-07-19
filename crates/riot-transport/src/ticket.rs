//! The site ticket + fail-closed transport gate now live in riot-core
//! (`riot_core::site::ticket`) — pure ed25519 protocol logic, so the FFI can
//! parse+verify a ticket without depending on this transport crate. Re-exported
//! here so every existing `riot_transport::ticket::{...}` path keeps working.

pub use riot_core::site::ticket::*;
