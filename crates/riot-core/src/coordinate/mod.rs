//! Coordinate ledger: signed ask / offer / task records for a community room.
//!
//! A sibling to [`crate::newswire`] with its own reserved path prefix
//! `coordinate/v1/…`. Where newswire carries a flat post wire, the Coordinate
//! ledger carries *object-kind* records with a mutable derived lifecycle
//! (`Open → Claimed → Done`) computed in projection from separate status
//! records — see `docs/superpowers/specs/2026-07-24-coordinate-ledger-design.md`.
//!
//! WU-1 (this slice) lands the module skeleton, the [`model::CoordinateItemV1`]
//! object-kind record, and its path family. Signing/admission (`entry`), the
//! store prefix scan (`store`), status transitions, projection, and the FFI
//! surface are later work units and are intentionally absent here.

mod model;
mod path;

/// Stable Coordinate construction and inspection failures at the module
/// boundary. Dependency-specific codec errors never cross this boundary — they
/// are mapped into these closed variants, exactly like [`crate::newswire`]'s
/// `NewswireError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateError {
    /// A Willow path could not be built from the requested Coordinate family.
    PathInvalid,
    /// A Coordinate payload failed canonical model validation.
    ModelInvalid,
}

impl std::fmt::Display for CoordinateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CoordinateError {}

pub use model::{
    decode_coordinate_item, encode_coordinate_item, CoordinateItemV1, CoordinateKind,
    CoordinateModelError, COORDINATE_ITEM_SCHEMA, MAX_COORDINATE_PAYLOAD_BYTES,
};
pub use path::{classify_coordinate_path, coordinate_path, CoordinatePathKind};
