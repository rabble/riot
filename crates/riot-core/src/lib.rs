//! Riot public-data kernel: deterministic alert codec, Willow authority,
//! and preview-first atomic import. Phase 0A evidence scope only.

pub mod apps;
/// The seeded demo space, built from committed source content. Conformance-only:
/// it derives signing keys from fixed seeds (the raw-secret constructor that
/// feature exists to keep out of the release graph), and nothing in the release
/// build needs it — the phone only ever *loads* the committed bytes, through the
/// ordinary import pipeline.
#[cfg(feature = "conformance")]
pub mod demo_fixture;
pub mod import;
pub mod meadowcap;
pub mod model;
pub mod newswire;
pub mod profile;
pub mod session;
pub mod site;
pub mod store;
pub mod sync;
pub mod willow;
