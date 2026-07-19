//! Server-only runtime for the Riot public community anchor network.
//!
//! `riot-anchor` is the anchor daemon: it hosts public communities, admits
//! signed listings and tickets, accumulates directory feed and snapshot state,
//! and serves the public HTTP/sync surfaces. It reuses the canonical wire layer
//! ([`riot_anchor_protocol`]) and the shared verify/project core
//! ([`riot_core`]), but owns its own durable store — a SQLite-backed
//! `AnchorRepository` — which is separate from the client-side
//! `EvidenceRepository`.
//!
//! # Server-only dependency boundary
//!
//! This crate is server-only. It must never be pulled into a native app or FFI
//! dependency graph. Nothing here declares an app, FFI, renderer, or platform
//! adapter dependency; the structural enforcement of that boundary lands with a
//! later work unit. Keep new dependencies inside the server envelope.
//!
//! # Forward-only schema
//!
//! [`schema`] owns the forward-only SQLite schema for the `AnchorRepository`.
//! Schema versioning is explicit and fails closed: a binary that does not
//! declare the version stamped in an existing database refuses to open it and
//! never migrates backward. See [`schema::migrate`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "daemon")]
pub mod admission;
pub mod checkpoint;
pub mod control;
#[cfg(feature = "daemon")]
pub mod daemon;
pub mod hosting;
pub mod idempotency;
pub mod listing;
pub mod removal;
pub mod repository;
pub mod schema;
pub mod sync_service;
pub mod work;
