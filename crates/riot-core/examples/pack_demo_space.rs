//! Packs the seeded "Riverside Tenants Union" demo space into one committed,
//! signed RIOTE1 bundle.
//!
//! Run:  `cargo run -p riot-core --features conformance --example pack_demo_space`
//!
//! Reads `fixtures/demo/riverside/content.json` and writes
//! `fixtures/demo/riverside/demo-space.riot-evidence`. Run it twice: the bytes
//! must be identical the second time. `tests/demo_fixture_drift.rs` enforces
//! exactly that against the committed bytes, so a content edit without a repack
//! is a red test, not a surprise on stage.
//!
//! Why an example and not the `riot-app` CLI (the hard-won constraint recorded
//! in `pack_checklist.rs`, and it applies with more force here): the CLI signs
//! with a FRESH key every run, so its output is not reproducible and a drift
//! guard over it would be pure noise. This packer derives every signing key from
//! a fixed seed and every timestamp from a fixed constant — that determinism is
//! what makes the committed bytes checkable at all.
//!
//! It needs `--features conformance` for one reason: deriving an author from a
//! fixed seed is the raw-secret constructor that feature exists to keep out of
//! the release graph. Nothing about LOADING the bundle needs it — the phone
//! imports these bytes through the ordinary pipeline.

use riot_core::demo_fixture::{build_demo_bundle_from_source, demo_bundle_path};

fn main() {
    if let Err(err) = run() {
        eprintln!("pack_demo_space: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let bytes = build_demo_bundle_from_source()?;
    let path = demo_bundle_path();

    // Report whether this run actually changed anything. A packer that says
    // "unchanged" is the fastest possible confirmation that it is reproducible.
    let previous = std::fs::read(&path).ok();
    std::fs::write(&path, &bytes).map_err(|e| format!("write {}: {e}", path.display()))?;

    let state = match previous {
        Some(previous) if previous == bytes => "unchanged (reproducible)",
        Some(_) => "REWRITTEN — the bundle changed; commit the new bytes",
        None => "created",
    };
    println!("{}: {} bytes, {state}", path.display(), bytes.len());
    Ok(())
}
