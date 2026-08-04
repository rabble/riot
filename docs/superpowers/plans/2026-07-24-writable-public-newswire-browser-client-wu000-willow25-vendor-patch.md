# WU-000 — willow25 + bab_rs Vendor Patch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make willow25's filesystem-backed store chain optional so the Wasm dependency graph excludes `fjall`/`lsm-tree`/`async-fs`, with zero behavior change for native builds.

**Architecture:** Vendor the pinned crates.io `willow25 0.6.0-alpha.3` and `bab_rs 0.8.1` verbatim, apply minimal feature-gate patches (willow25: `persistent-storage`; bab_rs: `storage-fs`), reference both via `[patch.crates-io]`, and have `riot-core` pass `persistent-storage` through its default feature set so native builds keep today's behavior while `default-features = false` (Wasm-shaped) consumers drop the filesystem chain.

**Why two crates (gate-discovered):** `async-fs` enters the graph twice — directly from willow25, and transitively via `bab_rs`'s default `storage` feature (`storage = ["std", "dep:async-fs", "dep:futures-lite"]`). willow25's unconditional modules (`storage/store.rs:2`, `storage/payload_prefix_store.rs:4-8`, `storage/memory_store.rs:14`) all use `bab_rs::generic::storage`, which bab_rs gates behind its `storage` feature (`bab_rs/src/generic.rs:58-59`), so willow25 cannot compile without it. Patching only willow25 leaves `async-fs` in the Wasm graph forever. Inside bab_rs, `async_fs` and `futures_lite` are used ONLY by `src/generic/storage/backend_filesystem.rs` (verified by grep). willow25's `persistent_store.rs:25-31` builds its payload-slice store on `backend_filesystem::{FileBackend, KeyState}` — so willow25's `persistent-storage` feature must also enable `bab_rs/storage-fs`, keeping the full chain in native builds while the Wasm shape (no `persistent-storage`) drops it. Nothing in Riot's `crates/` uses `backend_filesystem` directly (verified by grep).

**Tech Stack:** Rust 1.95.0, willow25 `=0.6.0-alpha.3` / bab_rs `=0.8.1` (crates.io, checksum-pinned in `Cargo.lock`), Node 26.4.0 `node --test` + c8 for the graph-contract script.

**Master plan:** `docs/superpowers/plans/2026-07-24-writable-public-newswire-browser-client-master-plan.md` (WU-000 section, verified-baseline finding 2, file ownership map).

**Execution worktree:** `/Users/rabble/.config/superpowers/worktrees/riot/feat-browser-client-wu000-willow25-vendor-patch`, branch `feat/browser-client-wu000-willow25-vendor-patch`, base `91b16afc`. Baseline verified green: `cargo test --workspace --all-features` 1980 passed / 0 failed; fmt + clippy clean.

---

## Load-bearing project constraints

1. **TDD mandatory.** RED first, watch fail, implement, GREEN, commit per task.
2. **Explicit `git add <paths>`** — never `git add -A`/`git add .`. Never `--no-verify`.
3. **Vendor convention** (from `vendor/tor-dirmgr-0.44.0/RIOT_PATCH.md`): verbatim upstream source, provenance + license retention documented, minimal justified patch, documented verification command, removal condition.
4. **Native behavior unchanged.** With default features, the graph must still contain `fjall`/`lsm-tree`/`async-fs` exactly as today.
5. **jsTooling coverage floor is 100%** (c8 `--100 --all` over `scripts/web/**/*.mjs` via `npm run test:web:coverage`). Any new script MUST ship with a `node --test` file reaching 100%.
6. **Coverage engines exclude `vendor/*`** (tarpaulin `--exclude-files 'vendor/*'`, llvm-cov `--ignore-filename-regex '(^|/)vendor/'`).
7. **`xtask validate-contracts` pins dependency metadata** (`crates/xtask/src/main.rs:635-639` asserts willow25's version; `cargo_lock_sha256` at `:666-673` is sha256 of `Cargo.lock` bytes) — the lockfile changes, so the fixture hash MUST be refreshed. Check whether bab_rs is also pinned and keep every pin intact.
8. **Workspace `exclude`** currently `["vendor/tor-dirmgr-0.44.0"]` (root `Cargo.toml:13`) — vendored crates are not workspace members.

## Verified code facts (checked on base `91b16afc`)

- Root `Cargo.toml:66`: `[patch.crates-io]` with `tor-dirmgr`; `:13` `exclude`; `:20`-region `willow25 = { version = "=0.6.0-alpha.3", default-features = false, features = ["std"] }`; `:21` `bab_rs = { version = "=0.8.1", default-features = false, features = ["william3"] }`.
- `Cargo.lock` near `:8096`: `willow25 0.6.0-alpha.3` + checksum `6477a05f…`; near `:483`: `bab_rs 0.8.1` + checksum `b80f2b89b5f64db0f0fadd7c3144ab78fd2082925ca438f961c7c346c9bcc29f`.
- `crates/riot-core/Cargo.toml`: `default = ["sqlite"]`, `sqlite = ["dep:rusqlite"]`, `conformance = ["dep:serde_json"]`; willow25 and bab_rs consumed via `workspace = true`.
- willow25 upstream (`~/.cargo/registry/src/index.crates.io-*/willow25-0.6.0-alpha.3/`):
  - `Cargo.toml`: `[dependencies.async-fs] version = "2.2.0"` (non-optional), `[dependencies.fjall] version = "3.0.3"` (non-optional), `[dependencies.bab_rs] version = "0.8.0"` (default features ON → pulls bab_rs `william3` + `storage`). Features: `default = ["std", "drop_format"]`, plus `std`, `dev`, `drop_format`; `dev` references `bab_rs/dev`.
  - `src/storage/mod.rs`: `mod store;` / `mod payload_prefix_store;` unconditional; `memory_store` and `persistent_store` gated only on `#[cfg(feature = "std")]` with matching `pub use` re-exports.
  - `src/storage/persistent_store.rs` imports `fjall::{...}`, `async_fs::remove_dir_all`, and `bab_rs::generic::storage::{...}` including `backend_filesystem::{FileBackend, KeyState}` (`:25-31`) — `FileBackend` is its payload-slice backend (`SingleSliceStore::<FileBackend>` at `:227,:244,:329,:367,:403`).
- bab_rs upstream (`~/.cargo/registry/src/index.crates.io-*/bab_rs-0.8.1/`):
  - `Cargo.toml` features: `default = ["william3", "storage"]`, `storage = ["std", "dep:async-fs", "dep:futures-lite"]`, `william3 = ["dep:arrayref"]`, `dev`, `std`.
  - `src/generic/storage.rs:14`: `pub mod backend_filesystem;` — the ONLY module using `async_fs` (`use async_fs::{File, OpenOptions}`) and `futures_lite`.
  - `src/generic.rs:58-59`: `#[cfg(feature = "storage")] pub mod storage;`.
- Nothing in `crates/` references `backend_filesystem` or `bab_rs::generic::storage` (verified by grep).
- `cargo tree -p riot-core --no-default-features --target wasm32-unknown-unknown -e normal` runs clean on this machine (wasm32 target installed via rust-toolchain/bootstrap).

## File Structure

| Path | Responsibility |
|---|---|
| `vendor/willow25-0.6.0-alpha.3/` (create) | Verbatim vendored source + `persistent-storage` patch |
| `vendor/willow25-0.6.0-alpha.3/RIOT_PATCH.md` (create) | Provenance, checksum, patch description, verification, removal condition |
| `vendor/willow25-0.6.0-alpha.3/RIOT_INTEGRITY.sha256` (create) | Per-file post-patch integrity manifest |
| `vendor/bab_rs-0.8.1/` (create) | Verbatim vendored source + `storage-fs` patch |
| `vendor/bab_rs-0.8.1/RIOT_PATCH.md` (create) | Same convention |
| `vendor/bab_rs-0.8.1/RIOT_INTEGRITY.sha256` (create) | Same convention |
| `scripts/web/check-wasm-graph.mjs` (create) | Wasm dependency-graph contract (extended in WU-003) |
| `scripts/web/test/check-wasm-graph.test.mjs` (create) | 100% c8 coverage for the script |
| Root `Cargo.toml` (modify) | `[patch.crates-io]` add willow25 + bab_rs; `exclude` add both vendor dirs |
| `crates/riot-core/Cargo.toml` (modify) | `persistent-storage` feature passthrough; added to `default` |
| `Cargo.lock` (regenerate) | willow25 + bab_rs become path deps |
| `fixtures/manifest.json` (modify) | refresh `cargo_lock_sha256` via xtask's own formula |
| `crates/xtask/src/main.rs` (modify, conditional) | ONLY if validate-contracts rejects path-patched sources |
| `package.json` (verify only) | globs already cover the new test file |

---

### Task 1: Vendor both crates verbatim + provenance docs

**Files:**
- Create: `vendor/willow25-0.6.0-alpha.3/**`, `vendor/bab_rs-0.8.1/**` (verbatim copies)
- Create: both `RIOT_PATCH.md` files
- Modify: root `Cargo.toml` (`exclude` only — patch wiring lands in Task 3)

- [ ] **Step 1: Verify upstream archives against Cargo.lock checksums**

```bash
grep -A3 'name = "willow25"' Cargo.lock | grep checksum
grep -A3 'name = "bab_rs"' Cargo.lock | grep checksum
shasum -a 256 ~/.cargo/registry/cache/index.crates.io-*/willow25-0.6.0-alpha.3.crate
shasum -a 256 ~/.cargo/registry/cache/index.crates.io-*/bab_rs-0.8.1.crate
```
Expected: each computed archive hash EQUALS the lockfile checksum (willow25 `6477a05f…`, bab_rs `b80f2b89…`). Mismatch = STOP, do not vendor unverified bytes.

- [ ] **Step 2: Copy the registry sources verbatim**

```bash
mkdir -p vendor/willow25-0.6.0-alpha.3 vendor/bab_rs-0.8.1
cp -R ~/.cargo/registry/src/index.crates.io-*/willow25-0.6.0-alpha.3/. vendor/willow25-0.6.0-alpha.3/
cp -R ~/.cargo/registry/src/index.crates.io-*/bab_rs-0.8.1/. vendor/bab_rs-0.8.1/
rm -f vendor/willow25-0.6.0-alpha.3/.cargo-checksum.json vendor/bab_rs-0.8.1/.cargo-checksum.json
```
(`.cargo-checksum.json` is cargo bookkeeping, not upstream source; the lockfile-verified archive hash from Step 1 is the provenance record.)

- [ ] **Step 3: Write both RIOT_PATCH.md files**

`vendor/willow25-0.6.0-alpha.3/RIOT_PATCH.md`:

```markdown
# Riot vendor patch: willow25 0.6.0-alpha.3

This directory is the verbatim crates.io source of `willow25 0.6.0-alpha.3`
plus the minimal feature-gate patch described below.

## Provenance

- Crate: `willow25` `=0.6.0-alpha.3` (workspace-pinned in root `Cargo.toml`)
- Source: `registry+https://github.com/rust-lang/crates.io-index`
- crates.io sha256 (verified against `Cargo.lock` before vendoring): `6477a05f…<full value from lockfile>`
- Vendored: 2026-07-27
- License: upstream license files in this directory, retained unmodified

## Patch

1. `async-fs` and `fjall` are made optional behind a new `persistent-storage`
   feature. The `persistent_store` module and the `PersistentStore` re-export
   in `src/storage/mod.rs` are gated on
   `all(feature = "std", feature = "persistent-storage")`.
2. The `bab_rs` dependency is pinned `default-features = false` with
   `features = ["william3", "storage"]` (upstream left default features on).
   `persistent-storage` additionally enables `bab_rs/storage-fs`, because
   `persistent_store.rs` builds its payload-slice store on bab_rs's
   `backend_filesystem::{FileBackend, KeyState}`.

Justification: upstream compiles the filesystem-backed store unconditionally
under `std`, pulling `fjall`/`lsm-tree`/`async-fs` into every consumer —
including Wasm builds that can never use a filesystem store. Riot's browser
client (spec
`docs/superpowers/specs/2026-07-24-writable-public-newswire-browser-client-design.md`)
requires the Wasm graph to exclude those crates. Native Riot enables
`persistent-storage` via `riot-core`'s default features, so native behavior is
unchanged.

Files modified from upstream (exact list — nothing else may differ):

- `Cargo.toml` — `async-fs`/`fjall` optional; `persistent-storage =
  ["dep:async-fs", "dep:fjall", "bab_rs/storage-fs"]` feature added; `bab_rs`
  dep pinned `default-features = false, features = ["william3", "storage"]`;
  `[patch.crates-io] bab_rs = { path = "../bab_rs-0.8.1" }` appended so
  standalone cargo invocations against this vendored crate resolve Riot's
  patched bab_rs (inert under workspace consumption)
- `src/storage/mod.rs` — `persistent_store` module declaration and
  `PersistentStore` re-export gated on
  `all(feature = "std", feature = "persistent-storage")`

Do not make any other source changes here.

## Verification

```bash
cd vendor/willow25-0.6.0-alpha.3 && shasum -a 256 -c RIOT_INTEGRITY.sha256
cargo test --manifest-path vendor/willow25-0.6.0-alpha.3/Cargo.toml --all-features
node scripts/web/check-wasm-graph.mjs
```

## Removal condition

Remove this patch when upstream willow25 publishes a release that gates the
`fjall`/`async-fs` persistent store behind an opt-in cargo feature (and works
against a bab_rs whose filesystem backend is likewise opt-in), and Riot moves
its pin to that release.

## Integrity manifest

`RIOT_INTEGRITY.sha256` records the post-patch SHA-256 of every vendored file
(excluding itself and this document). Regenerate after any deliberate change:

```bash
cd vendor/willow25-0.6.0-alpha.3
find . -type f ! -name RIOT_INTEGRITY.sha256 ! -name RIOT_PATCH.md -print0 | sort -z | xargs -0 shasum -a 256 > RIOT_INTEGRITY.sha256
```
```

`vendor/bab_rs-0.8.1/RIOT_PATCH.md`:

```markdown
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

Same convention as `vendor/willow25-0.6.0-alpha.3/RIOT_PATCH.md`.
```

- [ ] **Step 4: Add both vendor dirs to workspace `exclude`**

Root `Cargo.toml:13`:
```toml
exclude = ["vendor/tor-dirmgr-0.44.0", "vendor/willow25-0.6.0-alpha.3", "vendor/bab_rs-0.8.1"]
```

- [ ] **Step 5: Verify the workspace still resolves**

Run: `cargo metadata --format-version 1 --no-deps > /dev/null`
Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add vendor/willow25-0.6.0-alpha.3 vendor/bab_rs-0.8.1 Cargo.toml
git commit -m "chore(vendor): import willow25 0.6.0-alpha.3 and bab_rs 0.8.1 verbatim"
```

### Task 2: RED — Wasm dependency-graph contract

**Files:**
- Create: `scripts/web/check-wasm-graph.mjs`
- Create: `scripts/web/test/check-wasm-graph.test.mjs`

- [ ] **Step 1: Write the script**

`scripts/web/check-wasm-graph.mjs`:

```js
#!/usr/bin/env node
// Wasm dependency-graph contract for the Riot browser client.
//
// The Wasm-shaped graph (riot-core consumed with default features disabled,
// resolved for wasm32-unknown-unknown) must not contain willow25's
// filesystem-backed store chain. WU-003 extends FORBIDDEN_WASM with the full
// transport/FFI list from the browser-client spec.
//
// Usage: node scripts/web/check-wasm-graph.mjs
// Exit 0 = contract holds. Exit 1 = forbidden crate present (or cargo tree failed).

import { execFileSync } from 'node:child_process';

export const FORBIDDEN_WASM = ['fjall', 'lsm-tree', 'async-fs'];

// cargo tree prefixes dependency lines with drawing glyphs (│ ├── └──), so
// match "name v<digit>" anywhere on a line. Crate names are distinctive
// enough that substring matching cannot false-positive within this graph.
export function findForbidden(treeOutput, forbidden = FORBIDDEN_WASM) {
  return forbidden.filter((name) =>
    treeOutput.split('\n').some((line) => line.includes(`${name} v`)),
  );
}

export function cargoTree(args) {
  return execFileSync('cargo', ['tree', ...args], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
}

export function wasmShapeArgs() {
  // riot-core with default features off (no sqlite, no persistent-storage),
  // resolved for the browser target. This is the riot-web consumption shape.
  return [
    '-p',
    'riot-core',
    '--no-default-features',
    '--target',
    'wasm32-unknown-unknown',
    '-e',
    'normal',
  ];
}

export function nativeDefaultArgs() {
  // Native default-feature graph: persistent-storage must remain enabled so
  // the filesystem store chain stays exactly as before the vendor patch.
  return ['-p', 'riot-core', '-e', 'normal'];
}

export function check(cargoTreeFn = cargoTree) {
  const wasmGraph = cargoTreeFn(wasmShapeArgs());
  const leaks = findForbidden(wasmGraph);
  const nativeGraph = cargoTreeFn(nativeDefaultArgs());
  const nativeMissing = FORBIDDEN_WASM.filter(
    (name) => !findForbidden(nativeGraph, [name]).includes(name),
  );
  return { leaks, nativeMissing };
}

export function main(argv = process.argv.slice(2), cargoTreeFn = cargoTree) {
  void argv;
  let result;
  try {
    result = check(cargoTreeFn);
  } catch (error) {
    console.error(`wasm graph contract: cargo tree failed: ${error.message}`);
    return 1;
  }
  if (result.leaks.length > 0) {
    console.error(
      `wasm graph contract FAILED: forbidden crates in wasm-shaped graph: ${result.leaks.join(', ')}`,
    );
    return 1;
  }
  if (result.nativeMissing.length > 0) {
    console.error(
      `wasm graph contract FAILED: native default graph lost expected crates: ${result.nativeMissing.join(', ')}`,
    );
    return 1;
  }
  console.log('wasm graph contract OK: no fjall/lsm-tree/async-fs in wasm-shaped graph; native graph unchanged');
  return 0;
}

const isDirectRun =
  process.argv[1] != null && import.meta.url === new URL(`file://${process.argv[1]}`).href;
if (isDirectRun) {
  process.exit(main());
}
```

- [ ] **Step 2: Write the node test (100% coverage target)**

`scripts/web/test/check-wasm-graph.test.mjs`:

```js
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  FORBIDDEN_WASM,
  findForbidden,
  cargoTree,
  wasmShapeArgs,
  nativeDefaultArgs,
  check,
  main,
} from '../check-wasm-graph.mjs';

test('forbidden list pins the willow25 filesystem chain', () => {
  assert.deepEqual(FORBIDDEN_WASM, ['fjall', 'lsm-tree', 'async-fs']);
});

test('findForbidden flags crates behind cargo tree glyphs', () => {
  const tree = [
    'riot-core v0.1.0',
    '├── willow25 v0.6.0-alpha.3',
    '│   ├── fjall v3.1.6',
    '│   └── async-fs v2.2.0',
    '└── lsm-tree v3.1.6',
  ].join('\n');
  assert.deepEqual(findForbidden(tree), ['fjall', 'lsm-tree', 'async-fs']);
  assert.deepEqual(findForbidden('riot-core v0.1.0\n└── willow25 v0.6.0-alpha.3'), []);
});

test('cargo tree arg shapes encode the wasm and native contracts', () => {
  assert.deepEqual(wasmShapeArgs(), [
    '-p', 'riot-core', '--no-default-features', '--target',
    'wasm32-unknown-unknown', '-e', 'normal',
  ]);
  assert.deepEqual(nativeDefaultArgs(), ['-p', 'riot-core', '-e', 'normal']);
});

test('check reports wasm leaks and native regressions separately', () => {
  const fake = (args) =>
    args.includes('--no-default-features')
      ? 'riot-core v0.1.0\n└── fjall v3.1.6'
      : 'riot-core v0.1.0';
  const result = check(fake);
  assert.deepEqual(result.leaks, ['fjall']);
  assert.deepEqual(result.nativeMissing, ['fjall', 'lsm-tree', 'async-fs']);
});

test('check passes when wasm graph is clean and native keeps the chain', () => {
  const fake = (args) =>
    args.includes('--no-default-features')
      ? 'riot-core v0.1.0\n└── willow25 v0.6.0-alpha.3'
      : 'riot-core v0.1.0\n├── fjall v3.1.6\n├── lsm-tree v3.1.6\n└── async-fs v2.2.0';
  assert.deepEqual(check(fake), { leaks: [], nativeMissing: [] });
});

test('main returns 1 on leak, 1 on native regression, 1 on cargo failure, 0 when clean', () => {
  const leaky = () => 'fjall v3.1.6';
  assert.equal(main([], leaky), 1);
  const nativeRegressed = (args) =>
    args.includes('--no-default-features') ? 'willow25 v0.6.0-alpha.3' : 'riot-core v0.1.0';
  assert.equal(main([], nativeRegressed), 1);
  const boom = () => {
    throw new Error('cargo exploded');
  };
  assert.equal(main([], boom), 1);
  const clean = (args) =>
    args.includes('--no-default-features')
      ? 'willow25 v0.6.0-alpha.3'
      : 'fjall v3.1.6\nlsm-tree v3.1.6\nasync-fs v2.2.0';
  assert.equal(main([], clean), 0);
});

test('cargoTree invokes real cargo (smoke: --version works)', () => {
  const out = cargoTree(['--version']);
  assert.match(out, /cargo/);
});
```

- [ ] **Step 3: Run the unit tests — they must PASS**

Run: `node --test scripts/web/test/check-wasm-graph.test.mjs`
Expected: PASS.

- [ ] **Step 4: Run the contract — watch it FAIL (RED)**

Run: `node scripts/web/check-wasm-graph.mjs`
Expected: exit 1 naming forbidden crates in the wasm-shaped graph (`fjall`, `lsm-tree`, `async-fs` — all present pre-patch; `async-fs` arrives via BOTH willow25's direct dep and bab_rs's `storage` feature).

- [ ] **Step 5: Verify c8 still at 100% for the new files**

Run: `npm run test:web:coverage 2>&1 | tail -20`
Expected: 100% lines/functions/branches for `check-wasm-graph.mjs`. If a line is uncovered, extend the test — do not lower the floor.

- [ ] **Step 6: Commit**

```bash
git add scripts/web/check-wasm-graph.mjs scripts/web/test/check-wasm-graph.test.mjs
git commit -m "test(web): add wasm dependency graph contract (red)"
```

### Task 3: Apply the vendor patches + wire features (GREEN)

**Files:**
- Modify: `vendor/bab_rs-0.8.1/Cargo.toml`, `vendor/bab_rs-0.8.1/src/generic/storage.rs`
- Modify: `vendor/willow25-0.6.0-alpha.3/Cargo.toml`, `vendor/willow25-0.6.0-alpha.3/src/storage/mod.rs`
- Create: both `RIOT_INTEGRITY.sha256` manifests
- Modify: root `Cargo.toml` (`[patch.crates-io]`)
- Modify: `crates/riot-core/Cargo.toml` (feature passthrough)
- Modify: `Cargo.lock` (regenerated by cargo)

- [ ] **Step 1: Patch vendored bab_rs `Cargo.toml`**

(`async-fs` and `futures-lite` are already `optional = true` upstream — the patch is feature rewiring, not new optionality.)
- Features: `storage = ["std"]` (remove `dep:async-fs`, `dep:futures-lite`); add `storage-fs = ["storage", "dep:async-fs", "dep:futures-lite"]`
- Leave `default = ["william3", "storage"]`, `william3`, `dev`, `std`, and both dependency declarations unchanged.

- [ ] **Step 2: Gate bab_rs `backend_filesystem`**

`vendor/bab_rs-0.8.1/src/generic/storage.rs:14`:
```rust
pub mod backend_filesystem;
```
becomes:
```rust
#[cfg(feature = "storage-fs")]
pub mod backend_filesystem;
```
(Grep the file for any other `backend_filesystem` reference — e.g. doc links are fine; code references must be gated identically.)

- [ ] **Step 3: Patch vendored willow25 `Cargo.toml`**

- `[dependencies.async-fs]`: add `optional = true`
- `[dependencies.fjall]`: add `optional = true`
- `[dependencies.bab_rs]`: becomes
  ```toml
  [dependencies.bab_rs]
  version = "0.8.0"
  default-features = false
  features = ["william3", "storage"]
  ```
- Add to `[features]`: `persistent-storage = ["dep:async-fs", "dep:fjall", "bab_rs/storage-fs"]`
  (`bab_rs/storage-fs` is required: `persistent_store.rs` builds its payload-slice store on `backend_filesystem::{FileBackend, KeyState}`.)
- Append a `[patch.crates-io]` section so STANDALONE cargo commands against the vendored crate (Step 10, the RIOT_PATCH.md verification command) resolve the patched sibling bab_rs instead of registry bab_rs 0.8.1 (which lacks `storage-fs`; cargo validates `dep/feature` references at resolve time and rejects the manifest otherwise — gate-iteration-3 finding, fix empirically validated):
  ```toml
  [patch.crates-io]
  bab_rs = { path = "../bab_rs-0.8.1" }
  ```
  (Inert when willow25 is itself consumed as a path/patch dependency from Riot's workspace — the workspace's own `[patch.crates-io]` governs there. This only affects standalone `--manifest-path` invocations.)
- Leave `default = ["std", "drop_format"]` and `std`/`dev`/`drop_format` unchanged (`dev` keeps `bab_rs/dev`).

- [ ] **Step 4: Gate willow25 `persistent_store`**

`vendor/willow25-0.6.0-alpha.3/src/storage/mod.rs` — change ONLY the persistent_store gates:
```rust
#[cfg(feature = "std")]
mod memory_store;
#[cfg(feature = "std")]
pub use memory_store::*;

#[cfg(all(feature = "std", feature = "persistent-storage"))]
mod persistent_store;
#[cfg(all(feature = "std", feature = "persistent-storage"))]
pub use persistent_store::*;
```
(Match the file's actual declaration order/text; apply the same minimal gate to whatever lines name `persistent_store`.)

- [ ] **Step 5: Wire patches into the workspace**

Root `Cargo.toml` `[patch.crates-io]` — add under the tor-dirmgr entry:
```toml
willow25 = { path = "vendor/willow25-0.6.0-alpha.3" }
bab_rs = { path = "vendor/bab_rs-0.8.1" }
```

- [ ] **Step 6: riot-core feature passthrough**

`crates/riot-core/Cargo.toml`:
```toml
conformance = ["dep:serde_json"]
default = ["sqlite", "persistent-storage"]
persistent-storage = ["willow25/persistent-storage"]
sqlite = ["dep:rusqlite"]
```

- [ ] **Step 7: Regenerate the lockfile**

Run: `cargo check -p riot-core --all-features`
Expected: clean; `Cargo.lock` now lists willow25 and bab_rs without registry checksums (path-patched). If cargo errors on a vendored manifest, fix the vendored edit — do not work around it in Riot's manifests.

- [ ] **Step 8: Run the graph contract — GREEN**

Run: `node scripts/web/check-wasm-graph.mjs`
Expected: exit 0, `wasm graph contract OK`.

- [ ] **Step 9: Native behavior check**

Run: `cargo tree -p riot-core -e normal | grep -E '(fjall|lsm-tree|async-fs) v'`
Expected: all three present (default features keep the chain).

- [ ] **Step 10: Vendored crates' own tests + feature-shape checks**

```bash
cargo test --manifest-path vendor/bab_rs-0.8.1/Cargo.toml --all-features
cargo check --manifest-path vendor/bab_rs-0.8.1/Cargo.toml --no-default-features --features "william3,storage"
cargo test --manifest-path vendor/willow25-0.6.0-alpha.3/Cargo.toml --all-features
cargo check --manifest-path vendor/willow25-0.6.0-alpha.3/Cargo.toml --no-default-features --features std
```
Expected: upstream suites pass with all features on; both minimal-feature checks compile clean (these are the Wasm shapes). If willow25's `--features std` check fails on an un gated `persistent_store` reference elsewhere in its source, the gate in Step 4 missed a spot — find and gate it identically.

- [ ] **Step 11: Generate post-patch integrity manifests**

```bash
cd vendor/willow25-0.6.0-alpha.3 && find . -type f ! -name RIOT_INTEGRITY.sha256 ! -name RIOT_PATCH.md -print0 | sort -z | xargs -0 shasum -a 256 > RIOT_INTEGRITY.sha256 && shasum -a 256 -c RIOT_INTEGRITY.sha256
cd ../bab_rs-0.8.1 && find . -type f ! -name RIOT_INTEGRITY.sha256 ! -name RIOT_PATCH.md -print0 | sort -z | xargs -0 shasum -a 256 > RIOT_INTEGRITY.sha256 && shasum -a 256 -c RIOT_INTEGRITY.sha256
```
Expected: all OK.

- [ ] **Step 12: Commit**

```bash
git add vendor/willow25-0.6.0-alpha.3 vendor/bab_rs-0.8.1 Cargo.toml crates/riot-core/Cargo.toml Cargo.lock
git commit -m "feat(core): gate willow25 persistent storage chain behind opt-in features"
```

### Task 4: Contract pins + full verification

**Files:**
- Modify: `fixtures/manifest.json` (cargo_lock_sha256 refresh)
- Modify: `crates/xtask/src/main.rs` (ONLY if validate-contracts rejects the path-patched sources)

- [ ] **Step 1: Run the contracts gate**

Run: `cargo run -p xtask -- validate-contracts`
Expected: may FAIL on `cargo_lock_sha256` mismatch (lockfile changed) and/or on a willow25/bab_rs provenance assertion that assumed registry sources.

- [ ] **Step 2: Fix only what the gate reports**

- If `cargo_lock_sha256` mismatches: read `crates/xtask/src/main.rs` (hash computed at `:666-673` as sha256 of `Cargo.lock` bytes), recompute identically, update `fixtures/manifest.json`.
- If the validator asserts a registry source/checksum for willow25 or bab_rs (`:635-639` region): amend it to accept the vendored path sources while still pinning the exact versions and requiring both `RIOT_PATCH.md` files to exist. Keep every other pin intact. Add/adjust xtask tests RED-first.
- Do not relax any other contract.

- [ ] **Step 3: Re-run the gate — GREEN**

Run: `cargo run -p xtask -- validate-contracts`
Expected: exit 0.

- [ ] **Step 4: Full workspace verification**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
npm run test:web:unit
npm run test:web:coverage
node scripts/web/check-wasm-graph.mjs
```
Expected: all clean; cargo test 1980+ passed / 0 failed; coverage floors from `.coverage-thresholds.json` hold; no floor lowered.

- [ ] **Step 5: Commit**

```bash
git add fixtures/manifest.json
# plus crates/xtask/src/main.rs and its tests only if Step 2 required them
git commit -m "chore(contracts): accept vendored willow25/bab_rs sources in contract pins"
```

## Verification

Final matrix for this unit (from the worktree root):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p xtask -- validate-contracts
npm run test:web:unit && npm run test:web:coverage
node scripts/web/check-wasm-graph.mjs
cd vendor/willow25-0.6.0-alpha.3 && shasum -a 256 -c RIOT_INTEGRITY.sha256
cd ../bab_rs-0.8.1 && shasum -a 256 -c RIOT_INTEGRITY.sha256
```

## Self-review checklist (requirement → task)

| Requirement | Task |
|---|---|
| willow25: fjall/async-fs optional behind `persistent-storage` (which also enables `bab_rs/storage-fs`); mod.rs + re-export gate on `all(std, persistent-storage)` | Task 3 Steps 3-4 |
| bab_rs: async-fs/futures-lite optional behind `storage-fs`; `backend_filesystem` gated (gate-discovered necessity) | Task 3 Steps 1-2 |
| willow25 bab_rs dep: `default-features = false, features = ["william3", "storage"]` | Task 3 Step 3 |
| Native enables chain by default; Wasm graph excludes it | Task 3 Step 6 + contract both directions (Tasks 2-3) |
| Archive checksums verified against Cargo.lock BEFORE vendoring | Task 1 Step 1 |
| Provenance, licenses, per-file integrity manifests | Task 1 Step 3, Task 3 Step 11 |
| Native riot-core/riot-ffi behavior unchanged | Task 3 Steps 7-10, Task 4 Step 4 |
| Graph contract RED → GREEN | Task 2 Step 4, Task 3 Step 8 |
| xtask validate-contracts + fixture hash refresh | Task 4 |
| jsTooling 100% c8 floor maintained | Task 2 Steps 3-5 |
