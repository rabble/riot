# Riot vendor patch: bab_rs 0.8.1

This directory is the verbatim crates.io source of `bab_rs 0.8.1` plus the
minimal feature-gate patch described below.

## Provenance

- Crate: `bab_rs` `=0.8.1` (workspace-pinned in root `Cargo.toml`)
- Source: `registry+https://github.com/rust-lang/crates.io-index`
- crates.io sha256 (verified against `Cargo.lock` before vendoring): `b80f2b89b5f64db0f0fadd7c3144ab78fd2082925ca438f961c7c346c9bcc29f`
- Vendored: 2026-07-27
- License: upstream license files in this directory, retained unmodified

## Patch

The `storage` feature becomes `["std"]`, and a new `storage-fs` feature
(`["storage", "dep:async-fs", "dep:futures-lite"]`) takes over the filesystem
backend. (`async-fs` and `futures-lite` are already optional upstream; this is
feature rewiring.) The `backend_filesystem` module in
`src/generic/storage.rs` (the sole user of both crates) is gated on
`feature = "storage-fs"`.

Justification: upstream's `storage` feature unconditionally pulls `async-fs`,
and willow25's unconditional storage traits require `bab_rs::generic::storage`,
so `async-fs` otherwise enters every willow25 consumer — including Wasm builds
that can never touch a filesystem. Riot's browser client requires the Wasm
graph to exclude it. The `storage-fs` feature keeps the filesystem backend
available for consumers that opt in — willow25 does so via its
`persistent-storage` feature (its `persistent_store.rs` builds on
`FileBackend`); no other Riot code uses it.

Files modified from upstream (exact list — nothing else may differ):

- `Cargo.toml` — `storage` becomes `["std"]`; `storage-fs =
  ["storage", "dep:async-fs", "dep:futures-lite"]` added
- `src/generic/storage.rs` — `pub mod backend_filesystem;` gated on
  `#[cfg(feature = "storage-fs")]`
- Removed: `.cargo-ok`, `.cargo_vcs_info.json` (cargo registry bookkeeping,
  not upstream source; house convention per vendor/tor-dirmgr-0.44.0)

Do not make any other source changes here.

## Verification

```bash
cd vendor/bab_rs-0.8.1 && shasum -a 256 -c RIOT_INTEGRITY.sha256
cargo test --manifest-path vendor/bab_rs-0.8.1/Cargo.toml --all-features
node scripts/web/check-wasm-graph.mjs
```

## Removal condition

Remove this patch when upstream bab_rs publishes a release that gates the
filesystem storage backend behind an opt-in cargo feature, and Riot moves its
pin to that release.

## Integrity manifest

Same convention as `vendor/willow25-0.6.0-alpha.3/RIOT_PATCH.md`. Regenerate
after any deliberate change:

```bash
cd vendor/bab_rs-0.8.1
find . -type f ! -name RIOT_INTEGRITY.sha256 ! -name RIOT_PATCH.md ! -path './target/*' -print0 | sort -z | xargs -0 shasum -a 256 > RIOT_INTEGRITY.sha256
```
