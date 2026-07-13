# Riot Local-First PWA Implementation Plan

Plan review gate: **APPROVED.** Fresh feasibility, completeness, and
scope/alignment reviewers passed this exact text after the censorship-resistance
revision.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a framework-free, installable Riot web app whose browser is the
primary Riot node: it owns one public community replica and local publisher,
can verify/project/read, create/review/sign/commit offline, persists accepted
records durably, and exchanges bounded signed records by file without a
server-side account, canonical gateway, or server-held key.

**Architecture:** A static PWA calls a thin `wasm-bindgen` adapter over
`riot-client` and `riot-core`. `riot-client` separates signer-free
`CommunityReplica` verification/replay/projection from optional
`PublisherAuthority`, and owns transport-neutral exchange semantics while the
JS host owns asynchronous I/O ports. Publishing is review → sign → core commit →
IndexedDB persist → exact-hash acknowledgement; only then may normal projection,
export, or later synchronization observe the record. IndexedDB retains the
encrypted publisher identity and append-only accepted log. File is the only MVP
transport. Nearby, direct community nodes, HTTPS gateways, and Nym remain
unimplemented registry IDs behind the same boundary; gateways and renderers are
disposable, never authoritative. The service worker caches only coherent,
content-addressed releases. The async signer boundary can later admit an
OAuth-authorized recovery authority without changing reviewed/signed bytes.

**Tech Stack:** Rust 2021/1.95, `riot-core`, Willow/Meadowcap, `wasm-bindgen
0.2.126`, browser WebAssembly, framework-free ES modules, IndexedDB, WebCrypto,
Web Locks, service workers, Node test runner with c8, and Playwright Chromium +
WebKit.

**Approved design:**
`docs/superpowers/specs/2026-07-13-riot-local-first-pwa-design.md` Revision 12.
That document controls product semantics, DTO fields, error codes, copy,
security headers, storage state machines, and acceptance criteria when this plan
uses shorthand.

---

## Release slices and hard gates

This is one vertical-slice plan, but it is implemented through four demonstrable
checkpoints. A checkpoint may be shown locally; none may be called complete
while its gate is red.

1. **Directly openable shell:** immediately after the prerequisite coverage
   harness/baseline is green, a local static URL opens the responsive
   first-run/Home visual shell with
   honest `Prototype — storage and signing not connected yet` copy. It includes
   final layout and manifest metadata but does not claim installation, offline
   durability, or a working signer.
2. **Browser core:** the full release WASM graph builds and replaces the shell's
   prototype boundary with the real shared controller adapter.
3. **Durable local post:** create community, prepare exact review, post, reload
   offline, and see the same signer/update/draft state.
4. **Carry authenticated data:** signer-free replay/projection, preview-first
   import/join, selective existing-community import, deterministic export, the
   file transport port, and honest recovery states. Reserved duplex transports
   remain unavailable and invisible in product UI.
5. **Release proof:** coherent service-worker updates, Chromium plus real Safari
   offline/service-worker proof, cross-engine storage/crypto E2E,
   accessibility/security contracts, and all repository coverage gates at 100%.

The current repository is already below its configured 100% coverage threshold:
the 2026-07-13 baseline is 88.921% LLVM lines, 86.484% functions, 87.234%
regions/statements, and 74.645% branches; the retained Tarpaulin artifact is
about 83.37% lines. Task 1 closes that debt without changing product behavior.
It is not legal to lower `.coverage-thresholds.json`, exclude authored code, or
claim the PWA complete while the composite gate is red.

## Declared file scope

### Workspace, dependency, build, and coverage contract

- Modify `Cargo.toml`, `Cargo.lock`, `.gitignore`, `.coverage-thresholds.json`,
  and `COLLABORATION.md`.
- Create `.cargo/config.toml`.
- Create `scripts/web/bootstrap.sh`, `scripts/web/build.sh`,
  `scripts/web/coverage.sh`, `scripts/web/validate-llvm-coverage.mjs`, and
  `scripts/web/verify-static-contracts.mjs`, plus their tests under
  `scripts/web/test/**` and the real-Safari runner
  `scripts/web/test-safari-service-worker.mjs`.
- Modify `crates/xtask/src/main.rs` and `crates/xtask/Cargo.toml`.
- Modify `crates/riot-app-cli/src/lib.rs`, `crates/riot-app-cli/src/main.rs`,
  and tests under `crates/riot-app-cli/tests/**` only for behavior-preserving
  coverage closure.
- Modify any currently reported uncovered production module under
  `crates/riot-core/src/**` and its tests only for behavior-preserving coverage
  closure. The exact finite worklist appears in Task 1.
- Modify `crates/riot-ffi/src/mobile_state.rs`,
  `crates/riot-ffi/src/mobile_api.rs`, and focused FFI tests for the same
  coverage closure before their later controller extraction.
- Create `vendor/willow25-browser/**` only from the verified upstream archive,
  plus its checksum, provenance, file-hash manifest, license files, and patch.

### Rust protocol/controller/browser adapter

- Modify `crates/riot-core/src/willow/entry.rs`,
  `crates/riot-core/src/willow/mod.rs`, and
  focused existing core tests.
- Create `crates/riot-core/tests/prepared_alert.rs`.
- Create `crates/riot-client/Cargo.toml`, `crates/riot-client/src/lib.rs`,
  `crates/riot-client/src/replica.rs`, `crates/riot-client/src/publisher.rs`,
  `crates/riot-client/src/controller.rs`, `crates/riot-client/src/projection.rs`,
  `crates/riot-client/src/exchange.rs`, `crates/riot-client/src/dto.rs`,
  `crates/riot-client/src/error.rs`, `crates/riot-client/src/review.rs`,
  `crates/riot-client/tests/replica_contract.rs`,
  `crates/riot-client/tests/controller_contract.rs`,
  `crates/riot-client/tests/exchange_contract.rs`, and projection fixtures under
  `crates/riot-client/tests/fixtures/**`.
- Modify `crates/riot-ffi/Cargo.toml`, `crates/riot-ffi/src/mobile_state.rs`,
  `crates/riot-ffi/src/mobile_api.rs`, and focused `crates/riot-ffi/tests/**`.
- Create `crates/riot-web/Cargo.toml`, `crates/riot-web/src/lib.rs`,
  `crates/riot-web/src/dto.rs`, `crates/riot-web/src/error.rs`, and
  `crates/riot-web/tests/adapter_contract.rs`.

### Browser host

- Create `apps/web/index.html`, `apps/web/styles.css`,
  `apps/web/manifest.webmanifest`, `apps/web/icon.svg`, and `apps/web/sw.js`.
- Create `apps/web/src/app.js`, `apps/web/src/controller.js`,
  `apps/web/src/view.js`, `apps/web/src/validation.js`,
  `apps/web/src/storage.js`, `apps/web/src/vault.js`,
  `apps/web/src/bundle-log.js`, `apps/web/src/writer-lock.js`,
  `apps/web/src/download.js`, `apps/web/src/transport-contract.js`,
  `apps/web/src/transport-registry.js`, `apps/web/src/transports/file.js`, and
  `apps/web/src/release-client.js`.
- Create matching unit tests under `apps/web/test/unit/` and Playwright tests
  under `apps/web/test/e2e/`.
- Create `package.json`, `package-lock.json`, and `playwright.config.js`.
- Create `docs/product/riot-web-usability-results.md` as the product-readiness
  protocol/results sheet; human sessions are a release-readiness activity, not
  an autonomous code-completion claim.

Do not modify the currently dirty iOS/macOS project or transport files. If
execution requires another production path, stop, amend this plan, rerun the
plan gate, and claim the new path before editing it.

`.coverage-thresholds.json` is currently untracked but the user-supplied project
instructions explicitly designate it as the source of truth. Approval of this
plan adopts its existing four 100% thresholds as a project artifact and permits
staging the entire file after changing only its enforcement command/description.
`COLLABORATION.md` is tracked with unrelated user edits: the temporary claim is
never staged or committed, and its exact appended block is removed with
`apply_patch` at handoff.

## Task 0: Claim scope and capture a reproducible preflight

**Files:**
- Modify coordination state: `COLLABORATION.md`
- Read only: all dirty paths from `git status --short`

- [ ] **Step 1: Confirm no active claim overlaps the declared scope**

Run:

```bash
git status --short
tail -n 160 COLLABORATION.md
```

Expected: the existing Apple project/transport changes remain untouched and no
active claim in this checkout owns the Rust/web/build paths above. The recorded
SQLite `Cargo.toml`/`Cargo.lock`/`riot-core` work lives in the isolated
`codex/sqlite-foundation` worktree; nevertheless, do not edit an overlapping
path until its owner records release/integration. Non-overlapping harness work
may proceed. Re-check claims immediately before Tasks 1, 2, 3, and 4.

- [ ] **Step 2: Claim the exact paths**

Use `apply_patch` to append one active PWA claim to `COLLABORATION.md`. Re-read
the appended block and `git status --short`; never stage this dirty coordination
file. Remove only the exact PWA block at handoff.

- [ ] **Step 3: Reproduce the baseline before changing it**

Run:

```bash
cargo test --workspace --all-features
cargo +nightly-2026-07-01 llvm-cov --workspace --all-features --branch \
  --json --output-path target/llvm-cov/riot-before-pwa.json
cargo +nightly-2026-07-01 llvm-cov report --branch --show-missing-lines \
  > target/llvm-cov/riot-before-pwa.txt
cargo tarpaulin --workspace --all-features --out Json \
  --output-dir target/tarpaulin
```

Expected: tests pass; coverage is red near the recorded baseline. Preserve both
machine-readable reports under ignored `target/` for task-by-task comparison.

## Task 0A: Pin the non-production browser test and serving harness

**Files:**
- Create `package.json`, `package-lock.json`
- Create `scripts/web/serve.mjs`
- Create `scripts/web/test/serve.test.mjs`

- [ ] **Step 1: Pin the browser toolchain before any npm command**

Pin Node 26.4.0, npm 11.17.0, `@playwright/test 1.61.1`, `c8 11.0.0`, and
`selenium-webdriver 4.46.0` exactly. Initially define `test:web:unit` as
`node --test scripts/web/test/*.test.mjs` and `test:web:coverage` as pinned c8
with `--100 --all --include='scripts/web/**/*.mjs'` over that exact unit glob.
Playwright E2E is always separate and never enters Node's unit glob. Later tasks
extend explicit production includes/unit globs for `apps/web` and `sw.js`
without floating resolution. Run `npm install --package-lock-only` then
`npm ci`.

- [ ] **Step 2: Write the RED local-server contract**

Against a temporary static fixture root, assert path traversal and missing files
return fixed safe errors, MIME types are deterministic, the printed origin is
capturable, and every success/error response sends the design's exact CSP,
Referrer-Policy, nosniff, and Permissions-Policy headers. Assert no CORS wildcard
and no directory listing.

Run `node --test scripts/web/test/serve.test.mjs`.

Expected: FAIL because `serve.mjs` does not exist.

- [ ] **Step 3: Implement the built-in-Node server and keep tooling covered**

Use only `node:http`, `node:fs`, `node:path`, and `node:url`; export
`startServer({root, host, port})` for tests and make direct execution serve
the optional first positional root or `target/web-dist` on loopback with a
random available port by default. It prints
exactly one `http://127.0.0.1:<port>/` URL. Run `npm run test:web:coverage` and
expect 100% for the authored server.

## Task 1: Make the existing 100% coverage contract real and green

**Files:**
- Modify `.coverage-thresholds.json`
- Create `scripts/web/bootstrap.sh`
- Create `scripts/web/coverage.sh`
- Create `scripts/web/validate-llvm-coverage.mjs`
- Add behavior-preserving tests and the smallest injectable seams in the
  specific uncovered source files reported by
  `target/llvm-cov/riot-before-pwa.txt`

- [ ] **Step 1: Write RED validator fixture tests before the validator**

Create `scripts/web/test/validate-llvm-coverage.test.mjs` with four fixtures
derived from a minimal LLVM JSON `totals` object. Each fixture changes exactly
one metric—lines, functions, regions, or branches—to `99.99`; assert the CLI
exits nonzero and names that metric. Assert a `100/100/100/100` fixture exits
zero.

Run:

```bash
node --test scripts/web/test/validate-llvm-coverage.test.mjs
```

Expected: FAIL because `validate-llvm-coverage.mjs` does not exist.

- [ ] **Step 2: Implement pinned bootstrap and composite enforcement**

`bootstrap.sh` must be non-interactive and idempotently verify/install:

```text
stable 1.95.0 + rustfmt + clippy + wasm32-unknown-unknown
nightly-2026-07-01 + llvm-tools-preview
cargo-llvm-cov 0.8.7
cargo-tarpaulin 0.37.0
wasm-bindgen-cli 0.2.126
```

It must also verify the exact Node/npm versions recorded in `package.json` by
Task 0A. Implement the validator with exact integer
covered/count equality, not rounded percentages. `coverage.sh` must reject tool
version drift and run:

```bash
cargo tarpaulin --workspace --all-features --fail-under 100
cargo +nightly-2026-07-01 llvm-cov clean --workspace
cargo +nightly-2026-07-01 llvm-cov --workspace --all-features --branch \
  --json --output-path target/llvm-cov/riot.json
node scripts/web/validate-llvm-coverage.mjs target/llvm-cov/riot.json
npm run test:web:coverage
```

Update `.coverage-thresholds.json` so `scripts/web/coverage.sh` is its one
blocking command and the four thresholds remain 100.

- [ ] **Step 3: Close the exact existing Rust worklist in bounded commits**

The baseline report contains the following complete authored-source worklist.
The numbers are missing lines/functions/regions/branches. Each row names the
test location and behavior that must drive the missing code; these are not
instructions to assert line execution without behavior.

| Production source | Debt L/F/R/B | Test file and required cases |
| --- | ---: | --- |
| `riot-app-cli/src/lib.rs` | 353/50/389/61 | existing internal `tests` plus `tests/cli_pack.rs`: every `PackError`, `InspectError`, and `KeyError` Display/source arm; manifest duplicate/missing/unknown/oversize/wrong-type fields; directory/file/link/count/byte ceilings; content types; invalid paths/hex/private-key modes; all atomic publish/rename/fsync/cleanup checkpoints |
| `riot-app-cli/src/main.rs` | 44/16/70/8 | add adjacent binary `#[cfg(test)]` tests and refactor only to `main_with(args, clock, stdout, stderr) -> ExitCode`; call `main()` once without process exit and drive every dispatcher/OS-adapter line directly with injected args/clock/I/O: missing/unknown command, keygen/pack/inspect arity/options, clock-before-epoch, escaped output, success, and propagated library failure. External `tests/cli_commands.rs` may verify process behavior but is not credited as coverage for private binary functions; do not rely on subprocess coverage. |
| `xtask/src/main.rs` | 168/10/245/29 | existing internal tests: `run` missing/unknown/each valid command, workspace-root failure, binding command spawn/failure/missing outputs, every manifest/lock/schema/fixture read/parse/hash/feature mismatch |
| `riot-ffi/src/mobile_state.rs` | 93/2/269/55 | adjacent unit tests plus focused contract tests: every core→`MobileError` arm, poison/session quarantine, object closed, missing/consumed/stale draft/preview/plan/sync IDs, all organizer/member and app admission branches, bundle/listing projection empty/nonempty/error paths |
| `riot-ffi/src/mobile_api.rs` | 20/1/23/0 | adjacent unit test enumerating exact Display code for every `MobileError` variant |
| `riot-core/src/demo_fixture.rs` | 52/28/157/4 | `tests/demo_fixture_drift.rs`: every JSON accessor type/missing/range error, every optional source/field shape, invalid signer/time/hex, single and multiple bundle builders, and committed fixture byte drift |
| `riot-core/src/session.rs` | 31/1/77/19 | existing import lifecycle/concurrency/charge/path tests: each session/preview/plan consumed/stale/limit/closed transition, empty and selective commit, store failure rollback, route accounting, poison quarantine |
| `riot-core/src/apps/index.rs` | 26/2/30/14 | `tests/apps_index.rs`: no prefix, malformed path/key/value, wrong namespace/subspace, duplicates/revocations, pagination boundary, resolver success/failure |
| `riot-core/src/import/bundle.rs` | 15/0/29/13 | existing bundle tests: zero/ceiling+1 sizes, malformed/trailing frames, duplicate IDs, payload/path/route byte limits, unsupported/drop/wrong namespace |
| `riot-core/src/import/join.rs` | 13/3/20/1 | existing join/lifecycle tests: empty/live/expired join IDs, sorted live IDs, consumed/stale join, mixed namespace rejection |
| `riot-core/src/sync/state.rs` | 12/0/23/11 | `tests/sync_state.rs`: every state transition, zero/exact/over limits, duplicate/out-of-order/terminal frames, local/remote failure |
| `riot-core/src/model/mod.rs` | 11/1/81/7 | `tests/public_alert.rs`: every `AlertError` Display arm, empty/maximum/over-limit text and sources, invalid validity/expiry combinations, closed enum conversions |
| `riot-core/src/sync/wire.rs` | 11/0/49/11 | `tests/sync_wire.rs`: encode/decode every frame, truncated/unknown/trailing/wrong-length fields and each numeric boundary |
| `riot-core/src/apps/endorse.rs` | 10/0/28/7 | `tests/apps_endorse.rs`: trust/revoke valid path plus wrong app/organizer/subspace/path/payload/signature/capability |
| `riot-core/src/apps/trust.rs` | 10/0/28/11 | `tests/apps_trust.rs`: empty/malformed/duplicate/foreign markers, trust→revoke and revoke→trust ordering/tie branches |
| `riot-core/src/apps/bridge.rs` | 7/2/22/2 | `tests/apps_bridge.rs`: list empty/nonempty and session closed/failed error mapping |
| `riot-core/src/apps/bundle.rs` | 7/0/24/12 | `tests/apps_bundle.rs`: manifest/resource absence, duplicate/unexpected path, wrong hash/length/content type/entry point and exact ceilings |
| `riot-core/src/willow/entry.rs` | 7/1/10/0 | `tests/public_willow.rs`: deterministic successful alert and `expected_alert_path` at valid and invalid object/revision lengths |
| `riot-core/src/apps/mod.rs` | 6/2/7/0 | adjacent unit test for every `AppsError` Display and `From<WillowError>` arm |
| `riot-core/src/profile/mod.rs` | 6/2/7/0 | adjacent unit test for every `ProfileError` Display and `From<WillowError>` arm |
| `riot-core/src/apps/manifest.rs` | 5/0/44/6 | `tests/apps_manifest.rs`: all required/unknown/duplicate fields, type/length/UTF-8 limits and canonical round trip |
| `riot-core/src/profile/resolver.rs` | 4/0/9/4 | `tests/profile_resolver.rs`: local/foreign/no card, invalid signature/capability, newer/older/tied revision |
| `riot-core/src/willow/identity.rs` | 4/0/11/4 | `tests/public_willow.rs`: entropy failure/short fill, seed restore mismatch, organizer/member identity relationships |
| `riot-core/src/willow/clock.rs` | 3/1/5/0 | adjacent unit test for system clock success plus injected pre-epoch/failure source |
| `riot-core/src/willow/mod.rs` | 3/1/7/0 | adjacent unit test for every `WillowError` Display arm |
| `riot-core/src/apps/entry.rs` | 1/0/1/1 | `tests/apps_entry.rs`: accepted and rejected app path family |
| `riot-core/src/apps/starter.rs` | 1/0/3/1 | `tests/apps_starter.rs`: exact complete and one-missing starter pair |
| `riot-core/src/profile/card.rs` | 1/0/7/1 | `tests/profile_resolver.rs`: empty and maximum display name plus one-byte-over rejection |
| `riot-core/src/sync/reconcile.rs` | 1/0/1/2 | `tests/sync_reconcile.rs`: empty/local-only/remote-only/overlap set branches |
| `riot-core/src/willow/owned.rs` | 1/0/2/0 | `tests/public_willow.rs`: successful generated owned root with deterministic entropy |
| `riot-core/src/apps/directory.rs` | 0/0/5/2 | `tests/apps_directory.rs`: organizer/member and trusted/untrusted branch pairs |
| `riot-core/src/profile/path.rs` | 0/0/1/0 | `tests/profile_resolver.rs`: canonical profile path smoke |
| `riot-core/src/sync/ffi_bridge.rs` | 0/0/9/0 | existing sync bridge test: all frame conversions |

For `riot-app-cli`, add one private `PlatformFs` port used by the existing Unix
implementation with methods for open/create/read-dir/fsync/rename/unlink and a
scripted test implementation; keep it private to the crate. For its binary,
use the same `main_with(args, clock, stdout, stderr) -> ExitCode` seam named in
the table and make `main() -> ExitCode` its one-line OS adapter. For `xtask`,
make `run(root, args, command_runner, out, err)` the
testable unit; `command_runner` records/spawns binding commands. Core cases use
the existing conformance-only entropy/clock factories; where a currently hard-
wired call prevents deterministic failure (including organizer generation's
128-attempt exhaustion), add a crate-private `_with` function taking the
existing `EntropySource`/`ClockSource` trait and keep the public function as its
OS-backed wrapper. FFI poison/failure cases use adjacent private tests. These
ports are behavior-preserving and remain absent from the release API/feature
surface.

After every row group, rerun both reports and inspect newly introduced code.
Every new branch must receive its paired behavior test in the same commit. No
coverage-only conditional, skipped test, wildcard directory exclusion,
dead-code allowance, or generated fake execution is permitted.

- [ ] **Step 4: Prove the baseline gate is green before WASM work**

Run:

```bash
scripts/web/bootstrap.sh
scripts/web/coverage.sh
git diff --check
```

Expected: Tarpaulin lines and LLVM lines/functions/regions/branches all report
exactly 100%; authored tooling JavaScript is 100% under c8. The prototype shell
is created and added to JS coverage in Task 1A, not assumed here.
Commit per-crate remediation, then the coverage harness, without staging user
changes.

## Task 1A: Directly open the first honest visual shell

**Files:**
- Modify `package.json`, `package-lock.json`
- Create `playwright.config.js`
- Create `apps/web/index.html`, `apps/web/styles.css`,
  `apps/web/manifest.webmanifest`, `apps/web/icon.svg`
- Create `apps/web/src/view.js`, `apps/web/src/app.js`
- Create `apps/web/test/unit/view.test.js`
- Create `apps/web/test/e2e/shell.spec.js`
- Create `scripts/web/verify-static-contracts.mjs`
- Create `scripts/web/test/verify-static-contracts.test.mjs`

- [ ] **Step 1: Write RED semantic, security, and responsive shell tests**

Assert one `main`, ordered headings, three exact first-run actions including
**Try the Riverside demo**, Home/post/import/export layouts, visible focus,
44px targets, non-color status, reduced motion, 320px and 200% no-overflow, no
third-party/inline/eval/blob/data runtime sources, no `innerHTML`, and this
always-visible boundary:

```text
Prototype — storage and signing are not connected yet.
```

The manifest test requires `name`, `short_name`, `start_url`, `scope`,
`display: standalone`, `theme_color`, `background_color`, and a same-origin SVG
icon. It does not yet require a service worker or claim installability.

Write RED verifier fixtures that independently contain a remote URL, inline
script/style, `innerHTML`, `eval`, blob/data URL, secret-log token, private-group
label, and service-worker import of a vault/storage module; each must fail with
its fixed rule code, while a minimal same-origin fixture passes.

Run `npm ci`, install the lockfile-pinned Chromium/WebKit revisions, then run
the new unit files explicitly before the package glob is expanded:

```bash
node --test apps/web/test/unit/view.test.js scripts/web/test/verify-static-contracts.test.mjs
```

Then run
`./node_modules/.bin/playwright test apps/web/test/e2e/shell.spec.js`.
Expected: the relevant assertions fail because view modules/assets do not exist;
retain both RED outputs before production implementation.

- [ ] **Step 2: Implement and directly open the honest checkpoint**

Build the field-document shell with final layout vocabulary. Buttons may switch
static demonstration states but remain under the prototype boundary and cannot
generate/store identity data. Add Chromium/WebKit Playwright projects, install
their pinned browser revisions now, and set `serve:web` to the tested
`node scripts/web/serve.mjs` command with no baked-in root. Extend
`test:web:unit` with the explicit `apps/web/test/unit/*.test.js` glob and extend
`test:web:coverage` with `--all` production includes for
`scripts/web/**/*.mjs`, `apps/web/src/**/*.js`, and later `apps/web/sw.js`;
exclude only generated WASM glue/build output and Playwright E2E files.
Define `test:web:e2e` exactly as `playwright test`; every later invocation uses
that lockfile-local binary and the checked-in config/webServer.
Implement `verify-static-contracts.mjs` without network access, run it over
`apps/web`, and include it in `test:web:coverage`.

Run:

```bash
npm run test:web:coverage
./node_modules/.bin/playwright install chromium webkit
npm run serve:web -- apps/web
```

Open the printed URL directly and capture first-run, Home, review, import, and
recovery layouts at desktop and 320px, then stop that foreground server. Configure
Playwright's `webServer` to launch the same command/root noninteractively and run
`./node_modules/.bin/playwright test apps/web/test/e2e/shell.spec.js`.
Expected: the first directly openable visual is available immediately after the
mandatory baseline gate, c8 stays at 100%, and the shell E2E is GREEN in both
configured projects.

## Task 2: Establish the browser-compilable Willow/WASM graph

**Files:**
- Modify `Cargo.toml`, `Cargo.lock`, `.gitignore`
- Create `.cargo/config.toml`
- Modify `crates/xtask/src/main.rs`, `crates/xtask/Cargo.toml`
- Create `vendor/willow25-browser/**`
- Create initial `crates/riot-client/**` and `crates/riot-web/**`
- Create `scripts/web/build.sh`
- Create `scripts/web/test/build.test.mjs`

- [ ] **Step 1: Add RED graph/build contract tests**

Add an `xtask verify-willow-vendor` test fixture that fails independently for:
an upstream checksum mismatch, an unchanged-file hash mismatch, a patched-file
hash mismatch, a missing license/provenance file, and forbidden
`fjall|async-fs|lsm-tree` in the `riot-web` wasm feature graph. Add a shell test
that expects the current target build to fail before the patch is selected.

Run:

```bash
cargo test -p xtask verify_willow_vendor
node --test scripts/web/test/build.test.mjs
cargo build --locked --release -p riot-web --target wasm32-unknown-unknown
```

Expected: RED because the command/crates/vendor do not exist.

- [ ] **Step 2: Vendor only the approved upstream delta**

Copy the exact crates.io `willow25-0.6.0-alpha.3` archive into
`vendor/willow25-browser`, retain licenses, record archive SHA-256/package URL,
and commit upstream and post-patch per-file manifests. The sole source delta is:

```toml
[features]
persistent-storage = ["dep:fjall", "dep:async-fs"]

[dependencies]
fjall = { version = "3.0.3", optional = true }
async-fs = { version = "2.2.0", optional = true }
```

Gate only `storage::persistent_store` and its re-export with
`all(feature = "std", feature = "persistent-storage")`. Do not alter
`MemoryStore`, protocol codecs, Meadowcap, parameters, or cryptography.
Before selecting the path patch, extract and commit the current registry
`Cargo.lock` record and checksum
`6477a05fe4a055b0cb95d3eec60e10e5753551bc1d8fb67cb7642b61e6caf377`
into `vendor/willow25-browser/UPSTREAM.json`; download the archive from the
recorded crates.io package URL and verify that checksum. After `[patch.crates-
io]` changes the lock entry to a checksum-less path package, `xtask` validates
the archive/provenance record and per-file manifests rather than pretending the
new lock entry still carries the registry checksum.

- [ ] **Step 3: Pin the target graph and panic contracts**

Add the vendor `[patch.crates-io]`, workspace members `riot-client` and
`riot-web`, workspace exclusion for the vendor tree, target-only `getrandom
0.2` `js`, and `wasm-bindgen = 0.2.126`. Set `-C panic=abort` only under
`[target.wasm32-unknown-unknown]` in `.cargo/config.toml`; native release remains
`panic = "unwind"`.

The initial graph must be exactly:

```text
riot-web -> riot-client -> riot-core -> willow25(std, MemoryStore)
```

`riot-web` must not depend on `riot-ffi` or UniFFI.

- [ ] **Step 4: Implement the build script and pass both targets**

`scripts/web/build.sh` verifies stable/tool target/CLI versions, runs the exact
locked Rust build, runs `wasm-bindgen --target web`, hashes authored/generated
assets, and emits a release manifest consumed later by the worker. Until Task 7
wires the full host, load the existing Task 0A shell and replace its prototype
boundary with `Riot browser core loaded — storage and signing not connected
yet`; do not generate a competing page.

Run:

```bash
cargo xtask verify-willow-vendor
cargo tree -p riot-web --target wasm32-unknown-unknown -e features \
  | tee target/riot-web-tree.txt
! rg 'fjall|async-fs|lsm-tree|uniffi' target/riot-web-tree.txt
cargo build --locked --release -p riot-web --target wasm32-unknown-unknown
cargo build --locked --release -p riot-ffi
scripts/web/build.sh
node --test scripts/web/test/build.test.mjs
scripts/web/coverage.sh
```

Expected: all pass, and native unwind remains unchanged. Commit this hard gate
before browser product behavior.

## Task 3: Freeze one alert review before signing

**Files:**
- Create `crates/riot-core/tests/prepared_alert.rs`
- Modify `crates/riot-core/src/willow/entry.rs`,
  `crates/riot-core/src/willow/mod.rs`

- [ ] **Step 1: Write RED prepared-alert contract tests**

Using deterministic clock/entropy, assert that `prepare_alert` freezes object
ID, revision ID, created/expiry time, entry, capability, canonical entry bytes,
canonical capability bytes, and payload bytes without mutating the store. Assert
the review digest is exactly:

```rust
SHA256(
    b"riot/update-review/v1"
        || u32be(entry.len()) || entry
        || u32be(capability.len()) || capability
        || u32be(payload.len()) || payload
)
```

Tests must prove the Ed25519 signature message is only the retained canonical
Willow entry bytes; capability/payload participate in the digest but are not
concatenated into the signature message.

Run:

```bash
cargo test -p riot-core --all-features --test prepared_alert
```

Expected: FAIL because `PreparedAlert`/`prepare_alert` do not exist.

- [ ] **Step 2: Add failure and compatibility RED tests**

Cover a signature made by the wrong signer, modified signature bytes, expired
prepared entry, and injected signing failure; all leave the store unchanged.
Assert existing `create_signed_alert` is byte-identical to prepare-then-sign for
the same deterministic inputs. Namespace, store-generation, review-ID liveness,
and second-use rejection belong to the controller owning those concepts and are
tested RED in Task 4, not invented in `riot-core`.

Rerun before implementation:

```bash
cargo test -p riot-core --all-features --test prepared_alert
```

Expected: the newly added failure/compatibility cases remain RED.

- [ ] **Step 3: Implement prepare/sign without a second encoding path**

Make `PreparedAlert` retain validated semantic objects and exact canonical byte
arrays. `create_signed_alert` becomes a convenience call through the new path.
The commit bundle is assembled from retained components, verified against the
expected signer, and admitted through ordinary inspect → plan → commit.

- [ ] **Step 4: Verify core compatibility**

Run:

```bash
cargo test -p riot-core --all-features
cargo test -p riot-conformance --all-features
scripts/web/coverage.sh
```

Expected: all pass at 100%; existing golden bundles do not drift.

## Task 4: Build the signer-free replica, publisher controller, and exchange boundary

**Files:**
- Create/modify `crates/riot-client/**`
- Modify `crates/riot-ffi/Cargo.toml`
- Modify `crates/riot-ffi/src/mobile_state.rs`,
  `crates/riot-ffi/src/mobile_api.rs`, and focused FFI tests

### Task 4A: Signer-free replica and canonical projection

- [ ] **Step 1: Write RED replica/projection contracts**

In `replica_contract.rs`, restore real canonical alert bundles into
`CommunityReplica` without constructing an author/signer and assert namespace
verification, replay, current-live inventory, consolidated export limits, and
`CommunityV1.publisher = null`. Its API accepts only bundle bytes/hash and fixed
admission route; a compile-time/API contract proves no receipt, kind, endpoint,
or renderer parameter exists in `CommunityReplica`. Persist its bounded restore
inputs through the ordinary-Rust test fixture, drop every replica object,
restart/restore, and prove the same canonical projection with no signer or
peer-serving behavior (Acceptance 13).

In `projection.rs` tests, encode the exact Revision 12
`org.riot.community-projection/1` CBOR schema: closed integer keys/types,
explicit nulls, enum number mappings, validated text/count ceilings,
`created_at DESC, entry_id ASC` total order, and rejection of unknown/duplicate/
noncanonical fields. Check in golden bytes/digest. Run the same signed fixture
through two unrelated renderer-hostname parameters and assert byte-identical
projection while renderer-local `You`/`Community member` labels may differ.

Run:

```bash
cargo test -p riot-client --all-features --test replica_contract
cargo test -p riot-client --all-features projection::
```

Expected: FAIL because `CommunityReplica` and the projection codec do not exist.

- [ ] **Step 2: Implement the smallest signer-free replica**

Move namespace/store/replay/import/export/projection ownership into
`CommunityReplica`. It accepts only fixed `AdmissionRouteV1` values
`web-local-post|web-record-exchange`; browser receipts never enter core. Keep
publisher identity optional and keep DOM/localized labels outside canonical
projection. Do not add Viewer UI; the ordinary-Rust no-signer contract is the
MVP seam that prevents a later architectural rewrite.

### Task 4B: Publisher authority and durability quarantine

- [ ] **Step 3: Write the RED publisher/controller transition contract**

In `controller_contract.rs`, use real canonical bundles and assert:

- organizer create returns pending profile material and blocks every operation
  except confirm/abort/whole-controller close;
- a zero-record organizer publisher restore succeeds, a zero-record member
  restore is `REPLAY_FAILED`, relationship is derived from namespace versus
  signer/subspace rather than trusted stored metadata, and every partial
  controller closes on restore failure;
- publisher restore rejects profile/community/manifest namespace or signer
  mismatch, while signer-free replica restore requires neither signer nor
  organizer/member relationship;
- first-run preview requires one communal namespace and join creates a fresh
  member signer in that namespace;
- selective file import persists only selected accepted alerts in entry-ID
  order with `web-record-exchange`;
- prepare/post binds review ID, publisher, namespace, generation, and expiry;
- every core-committed bundle is quarantined from normal projection, export,
  and exchange inventory until exact-hash persistence acknowledgement;
- wrong/stale hash, abort, consumed ID, `STORE_FULL`, `PERSISTENCE_FAILED`, and
  `PERSISTENCE_PENDING` have zero unintended state change;
- confirmed whole-controller Close without saving discards the quarantined
  store and requires rebuild from the prior durable log;
- export is one bounded canonical bundle or an honest window error.

Run and retain RED output:

```bash
cargo test -p riot-client --all-features --test controller_contract
```

Expected: the focused controller test fails before implementation.

- [ ] **Step 4: Define all Revision 12 DTOs and stable errors**

Implement every approved controller/exchange DTO/enum exactly, including nested `version: 1`,
decimal-string unsigned JS times, complete lowercase hex IDs, always-present
arrays, JSON nulls, optional `PublisherIdentityV1`, fixed admission routes,
replication roles/states, projection records, and
`WebErrorV1 { version, code, field, message_key }`. Do not expose parser/debug
strings or secret/capability/signature bytes.

Keep browser-manifest/port records in JS: `TransportReceiptV1`, registry
availability, port-operation state, and renderer-local `UpdateV1` decoration do
not become `riot-client` controller state. `riot-web` may deserialize a stored
browser restore row, but must strip `transport_receipt` and pass only exact
bundle bytes/hash/fixed `AdmissionRouteV1` to `CommunityReplica`; tests vary
kind/endpoint/receipt validity and prove the Rust call, admission charge, and
projection are byte-identical.

### Task 4C: Versioned alert exchange policy without a transport

- [ ] **Step 5: Write the RED `ReplicationCoordinator` contract**

Drive two in-memory byte sessions without browser/network I/O and assert:

- the outer session policy is exactly `org.riot.alert-live-set/1`; it reuses
  inner `org.riot.conference-sync/1` frames but refuses missing/legacy/different
  policy before `Hello` and makes no native complete-inventory interop claim;
- Rust owns the exact `TransportEnvelopeV1 { version, capability, session_id,
  exchange_profile, payload_kind, payload_bytes }` validator/wrapper. Incoming
  flow is `validate_transport_envelope(bound_session_id, envelope)` followed by
  the approved `receive_replication_frame(session_id, payload_bytes)`; outgoing
  flow is the approved `take_replication_frame(session_id)` followed by
  `wrap_transport_frame(session_id, payload_bytes)`. Validation rejects wrong
  version, capability, null/wrong session, profile, payload kind, or frame
  ceiling before `ByteSyncSession` decoding;
- controller sessions know only `initiator|responder`, namespace, policy, opaque
  bytes, and state—never file/Nym/gateway/kind/endpoint;
- initiator and responder starts, simultaneous-initiation failure, outbound
  drain-before-next-action, terminal Complete/Reject, cancel, and one-active-
  session/one-inbound/one-outbound/one-pending-import limits match Revision 12;
- inventory is current live verified alerts only, bounded to 64 entries/8 MiB;
  non-alert frames fail the whole alert-policy exchange;
- before showing accept, requested IDs = pending IDs = decoded IDs = valid IDs =
  eligible IDs = planned IDs, rejected/unsupported count is zero, and the
  prospective live inventory remains bounded;
- accept returns an exact pending bundle and enters `awaiting-persistence`;
  only exact-hash acknowledgement calls `ByteSyncSession::import_accepted`;
- session close/cancel in `awaiting-persistence` returns
  `PERSISTENCE_PENDING`; crash/wrong hash never emits acceptance or advertises
  pending records.

Run:

```bash
cargo test -p riot-client --all-features --test exchange_contract
```

Expected: FAIL because the coordinator/envelope validator does not exist.

- [ ] **Step 6: Move state, not I/O or bindings, into `riot-client`**

Implement `CommunityReplica`, `PublisherAuthority`, `RiotClientController`, and
the exchange coordinator plus exact `TransportEnvelopeV1` validator/wrapper.
Do not replace the approved frame API. The browser host validates/wraps through
Rust, strips host-local routing metadata only after Rust produces the envelope,
and rebinds received peer payload to its own immutable local session before
Rust validation and `receive_replication_frame`. Extract matching profile/store/review/persistence
ownership from `riot-ffi/mobile_state.rs`. Retain UniFFI records/exports in
`riot-ffi` and adapt them mechanically so Apple/Android behavior does not drift.
`riot-client` contains no UniFFI, browser API, socket, URL, gateway, transport
kind, OAuth, or Nym dependency.

- [ ] **Step 7: Prove replica, controller, exchange, and native adapter together**

Run:

```bash
cargo test -p riot-client --all-features
cargo test -p riot-ffi --all-features
cargo check -p riot-ffi --release
scripts/web/coverage.sh
```

Expected: all pass; native mobile contract tests show no behavior drift.

## Task 5: Expose a mechanical WASM adapter and async signer boundary

**Files:**
- Create/modify `crates/riot-web/**`
- Modify `crates/riot-client/src/review.rs` and DTO/error files as needed
- Create `apps/web/test/e2e/wasm-adapter.spec.js`

- [ ] **Step 1: Write RED host adapter tests for every public operation**

Cover create, publisher restore, preview new community, join, preview/accept existing
import, prepare/post, list, export, profile
confirm/abort, bundle acknowledge, open/begin/receive/take/review/accept/reject/
persist-ack/close exchange through Rust-owned inbound/outbound envelopes, and
whole-controller close. Assert
`Uint8Array`-compatible bytes, exact DTO versions, complete IDs, decimal time
strings, null/array shape, canonical projection bytes, and every error mapping.

Run:

```bash
cargo test -p riot-web --all-features --test adapter_contract
./node_modules/.bin/playwright test apps/web/test/e2e/wasm-adapter.spec.js
```

Expected: the Rust contract and browser lifecycle assertions FAIL because the
adapter surface/mappings do not exist and the prototype shell exposes no WASM
controller.

- [ ] **Step 2: Define a synchronous local signer and asynchronous browser port**

The ordinary-Rust controller keeps the local cryptographic call synchronous:

```rust
pub trait SignerBackend {
    fn public_identity(&self) -> Result<PublicIdentity, SignerError>;
    fn sign(
        &self,
        canonical_entry_bytes: &[u8],
        context: SigningContextV1,
    ) -> Result<SignatureBytes, SignerError>;
    fn status(&self) -> SignerStatus;
}
```

`LocalSignerBackend` is the only Rust implementation. `riot-web` exports
synchronous wasm-bindgen methods. `apps/web/src/controller.js` exposes async
methods and yields one microtask before invoking the synchronous WASM call, so
UI state always treats signing as asynchronous without an executor or
`wasm-bindgen-futures`. A future remote backend requires its separately reviewed
controller extension; this slice preserves the exact canonical-entry/context
contract but does not pretend to implement remote I/O. Tests prove the context
and digest never change the signed message. Do not add OAuth, networking,
tokens, root recovery, or a remote implementation.

- [ ] **Step 3: Implement thin wasm-bindgen exports**

The adapter may serialize DTOs mechanically for JS but must not own session
state, protocol/exchange validation, canonical projection, author labels, or
error policy. Best-effort zeroize
temporary Rust buffers and overwrite reachable JS key arrays after use; document
that generated/GC copies cannot be guaranteed zeroized.

- [ ] **Step 4: Run generated-glue smoke tests**

Build, serve locally, and call create → profile-persist-confirm → prepare → post → persist-ack → project →
list → export; in a clean publisher controller call preview-new-community
→ join-reviewed-community → profile-persist-confirm; and in an existing
controller call preview-import → accept-import → persist-ack. Drive the complete in-memory initiator/responder
alert-policy exchange lifecycle through generated glue, including pending
persistence and terminal frames, then close. Run the generated-glue browser
lifecycle in both Playwright engines. Then
run:

```bash
cargo test -p riot-web --all-features
scripts/web/build.sh
./node_modules/.bin/playwright test apps/web/test/e2e/wasm-adapter.spec.js
scripts/web/coverage.sh
```

Expected: exact lifecycle succeeds without UniFFI in the wasm graph.

## Task 6: Build atomic browser persistence and the transport port boundary

**Files:**
- Create `apps/web/src/storage.js`, `vault.js`, `bundle-log.js`,
  `writer-lock.js`, `download.js`, `transport-contract.js`,
  `transport-registry.js`, and `transports/file.js`
- Create matching `apps/web/test/unit/*.test.js`
- Create `apps/web/test/e2e/persistence.spec.js` and `multi-tab.spec.js`

- [ ] **Step 1: Write RED unit tests against injected browser ports**

Cover exact schema versions and state transitions:

- non-extractable AES-GCM key, fresh nonce, fixed versioned AAD, authenticated
  decrypt failure, and no replacement identity fallback;
- manifest header and descriptor values are canonical CBOR Blobs with exact
  4-KiB/512-byte limits, at most 4096 records/1-MiB descriptor bytes/16-MiB
  staged bundle bytes, and fixed header/ordinal/hash keys;
- phased read transactions use `count()` and exact `get()` only, never enumerate
  unknown keys, never await Blob buffering/parsing/hashing inside a live IDB
  transaction, and revalidate exact manifest generation/bytes across phases;
- hostile oversized/unexpected keys are never materialized; oversized/non-Blob
  header/descriptor/bundle values, count drift, generation drift, missing,
  extra, duplicate, reorder, hash/size/total/namespace mismatch all fail before
  controller restore;
- duplicate hash retains its first local receipt and consumes no extra budget;
- writes over 16 MiB fail before mutation;
- bundle Blob + header/descriptor generation + draft deletion commit in one
  transaction;
- create-community commits protected vault + community + empty manifest in one
  transaction, then confirms the pending profile; abort closes the controller,
  zeroes every reachable key buffer, and leaves first run unchanged;
- first join commits vault + community + bundle + manifest atomically;
- transaction abort leaves every prior store unchanged;
- one writer lock and read-only second-tab states with and without a profile;
- downloads revoke object URLs and distinguish `Export prepared` from delivery;
- normal `.riot-evidence` export and unverified recovery `.bin` use different
  APIs/copy and cannot be confused;
- the registry owns bounded kind IDs and immutable session→port bindings; a port
  cannot self-report/rebind kind, silently fall back, or bind file to duplex;
- kind IDs enforce `[a-z0-9][a-z0-9.-]*` at 1–64 ASCII bytes; endpoint labels
  cap at 128 UTF-8 bytes and strip user-info, query, fragment, controls, bearer/
  cookie/OAuth data, and full Nym addresses; invalid/oversized receipts are
  dropped without rejecting an already verified record; a capture fake proves
  restore/admission receives only bytes/hash/fixed route and no receipt fields;
- port transitions are exactly unavailable or
  `ready→opening→exchanging→ready`, cancel to ready, close to terminal closed,
  failure to terminal failed; terminal retry constructs a new port; only one
  send/receive/open/cancel/close operation may be in flight;
- every event/send result carries the closed failure code/null combination and
  a localization-only `message_key`; UI logic never branches on message text;
- host-local envelope session IDs/policies are stripped before peer payload I/O
  and rebound to the receiving port's local session; mode/profile/session and
  size mismatch fail before controller input;
- bundle carriers reject declared/file size above `MAX_BUNDLE_BYTES` before
  `arrayBuffer()`, duplex ports reject declared/stream size above
  `MAX_SYNC_FRAME_BYTES` before aggregation where the API permits, and tests
  prove one bounded input/output buffer with no second unbounded copy;
- only `file` is registered as a concrete `bundle-carrier`; reserved nearby,
  direct-community-node, https-gateway, and nym IDs have no implementation and
  produce no UI; an in-memory duplex fake exists under tests only.

Run:

```bash
npm run test:web:unit -- --test-name-pattern='vault|bundle log|writer lock|transport'
```

Expected: FAIL because the modules do not exist.

- [ ] **Step 2: Implement the smallest ports that pass the state-machine tests**

Keep all IDB transaction completion/abort handling inside `storage.js`; never
report success from request success alone. `vault.js` stores only the
non-extractable structured-clone key plus encrypted wrapping key, nonce, and
sealed identity. `bundle-log.js` recomputes SHA-256 before trust and accepts only
fixed admission routes; optional bounded credential-free receipts remain
browser-local and never enter core. `transport-contract.js` mirrors the
`riot-client` envelope/state/error contract while JS adapters own async I/O
only. No service-worker module may import these ports.

- [ ] **Step 3: Prove real browser port primitives before app orchestration**

Use a minimal test harness page that imports only the Task 6 ports. In unique
Playwright contexts, exercise real IndexedDB/WebCrypto/Web Locks/Blob/download
implementations: phased bounded restore, transaction abort, hard page
termination immediately before/after a generic bundle+header+descriptor+draft
transaction, vault encrypt/reopen/authentication failure, storage clear,
corrupt/unsupported recovery classification, and single-writer/second-tab
behavior. Assert storage facts only; do not claim create/post/join/import UI or
controller acknowledgement yet. Write those orchestration E2Es RED now, but
turn them GREEN only after Task 7 wires the real controller.

- [ ] **Step 4: Keep JS and Rust gates green**

Run:

```bash
npm run test:web:coverage
npm run test:web:e2e -- --grep 'port persistence|multi-tab'
scripts/web/coverage.sh
```

Expected: 100% authored JS statements/branches/functions/lines and all Rust
metrics remain 100%.

## Task 7: Open the real community-first PWA shell

**Files:**
- Create remaining `apps/web/**` host/view/controller/validation assets
- Modify `package.json`, `package-lock.json`, `playwright.config.js`
- Extend `scripts/web/build.sh`
- Create `apps/web/test/unit/flows.test.js`, `errors.test.js`,
  `accessibility.test.js`, and `security.test.js`
- Create `apps/web/test/e2e/community-flow.spec.js`,
  `accessibility.spec.js`, and `security.spec.js`

- [ ] **Step 1: Write RED DOM/view tests for every top-level state**

Test first run, loading, ready with local durability/offline availability/manual
file exchange, empty community, cleared storage, corrupt storage, unsupported
schema, pre-commit storage full, post-commit persistence queue, another writer, install
available/instructions/installed, update available, recovery queue, and
unsupported browser. Imported/user text must be inserted with text nodes or
`textContent`, never `innerHTML`.

Before implementing controller wiring, add RED flow tests for: create durable
transaction→confirm and failure→abort/close; immutable prepare→post→atomic
persist→exact-hash acknowledge; existing import cancel with zero mutation,
empty selection disabling the CTA, each fixed rejection reason, successful
selective persist/acknowledge, and wrong-community rejection; first join
success and failed persistence with preview retained, accepted-bundle download,
and fresh restart; recovery-queue warning/Retry/confirmed whole-controller close
with every transport disabled. These
transition tests use injected controller/storage ports first and real browser
orchestration in Task 7 Step 4.

Add RED tests for every stable host error action: identity locked/corrupt
distinct unverified-recovery `.bin` without replacement identity; entropy/clock retry with
draft retained; stale review/preview return to a fresh review with values/
selection retained; pre-commit store-full export of prior durable state;
unsupported schema and replay failure distinctly labeled unverified recovery
without skipping; internal whole-controller close/reload;
invalid/expired draft return to the preserved form; unsupported-browser exact
same-origin public-renderer link `/site/`;
another-writer read-only recovery; persistence-failed/pending queue; import too
large, rejected, wrong-community, and no-eligible-entry actions; replication
window/unexpected frame; transport cancelled/unavailable/mode-mismatch/
oversize/failed without fallback; and every fixed row rejection category. Each
renders through a polite `aria-live` region without
raw bytes/debug text.

Before implementing the real flows, extend `accessibility.spec.js` and
`security.spec.js` with the final keyboard/focus/dialog/live-region/zoom/
reduced-motion/no-overflow contracts and exact header/zero-third-party/no-
signing-without-review assertions. These are RED against the prototype shell and
must turn green in Steps 2–3, not first appear in Task 9.

Assert the exact critical copy from the design, including:

```text
If you used Riot in this browser before, that signer and any organizer
authority are gone. Import restores public updates only and creates a new
member identity.

This file contains public community updates. It does not back up your identity
or organizer authority.

Saved on this browser means the record is durable here. Export prepared means
Riot created a file for you to carry; it cannot confirm anyone received it.
Exchanged is reserved for a completed exchange of authenticated records, which
this version does not provide.

Unverified recovery bytes. Riot could not verify these records; this file may
be corrupt and may not import. It does not contain an identity backup.
```

Run and retain RED output before implementing Steps 2–3:

```bash
npm run test:web:unit -- --test-name-pattern='flow|error|accessibility|security'
./node_modules/.bin/playwright test apps/web/test/e2e/community-flow.spec.js apps/web/test/e2e/accessibility.spec.js apps/web/test/e2e/security.spec.js
npm run test:web:e2e -- --grep 'orchestration interruption'
```

Expected: final flow/state/copy/accessibility/security assertions fail against
the prototype shell, and the orchestration/interruption specs fail before
controller/view wiring. Retain every RED output.

- [ ] **Step 2: Implement first-run, Home, and immutable review flows**

Wire Create community, Import community data, and Try Riverside through the
same controller transitions. Home opens directly and shows readable updates,
Post/Import/Export actions, local durability/offline availability/manual file
exchange as distinct text/icon/shape states, and IDs only under Technical
details. Do not show online/offline transport status or controls for reserved
adapters. Author labels always include the eight-hex key tag.

All file picker/import and download/export actions must call
`TransportRegistryV1.open('file')` and the registered `bundle-carrier` port;
views/controllers may not import picker/download helpers directly. Inject a
registry spy in unit tests and fail if any user flow bypasses the port.

The form persists every edit, retains invalid values, associates errors with
`aria-describedby`, and focuses the first invalid field. The review renders
only frozen DTO data. After signing begins, cancellation is unavailable; after
commit, the host persists then acknowledges the exact hash before showing:
`Saved on this browser. Export it to share.`

Register the service worker only from a `DOMContentLoaded`/interactive-or-later
path; a unit test records registration timing and fails if registration occurs
while `document.readyState === "loading"`.

Implement the error-action table from Step 1. Unverified recovery exports use
`riot-unverified-recovery-<timestamp>.bin`, never normal `.riot-evidence` copy or
the normal import picker, and contain only bounded retained bundle/manifest
bytes—never vault ciphertext, wrapping keys, or signer material. The
writer-lock unavailable first-run view automatically enables normal actions
after the lock is acquired. Unsupported browser retains one plain navigation
link with exact `href="/site/"`, matching the existing public gateway route. It
is never fetched by JavaScript, never used as sync/authority, and may be routed
by a deployment to any disposable renderer. Static verification permits this
one root-relative navigation target while continuing to reject remote runtime
URLs, external subresources, and network API endpoints;
Home says `Last local change` and never claims complete global history.

- [ ] **Step 3: Implement recovery queue and import/export UX**

If core commit succeeds but IDB fails, block all mutation, retain the pending
bundle, disable file/peer transport, and offer only Retry or confirmed
whole-controller Close without saving. Move focus to the recovery heading,
announce the non-durable state once, and never return focus to Post. First-join
persistence failure aborts the pending profile, closes and
zeroes the new identity, returns to the still-readable preview, offers the
accepted public bundle download, and labels the retry as a fresh restart with a
new member signer. Import rejects files over 8 MiB before `arrayBuffer()` and
Rust rechecks. Export filename is exactly
`riot-<full-namespace-id>.riot-evidence`.

Import rows use native labeled checkboxes in a `fieldset`; selection changes
announce the count politely, rejected rows have no fake checkbox, and duplex
review—tested with the in-memory port only—is immutable all-or-reject. A file
download success says `Export prepared`, never sent/delivered/exchanged.

- [ ] **Step 4: Turn the real orchestration/interruption E2Es GREEN**

Now use the Task 7 controller/view wiring with Task 6's real ports. Hard-
terminate immediately before/after: create's vault+community+manifest
transaction; post's bundle+manifest+draft transaction; first join's
vault+community+bundle+manifest transaction; existing-community file import;
and the test-only in-memory replicated import. Assert the exact pending-profile
confirm/abort and bundle exact-hash acknowledgements. Before acknowledgement,
normal projection/export/exchange remain on prior durable state and no peer
acceptance frame exists; after it, each record appears exactly once. Session
close while replication persistence is pending returns `PERSISTENCE_PENDING`;
confirmed whole-controller close reloads prior durable state. A not-saved newly
signed post cannot export, while a failed first join may re-download the already
imported selected public bundle. Run:

```bash
npm run test:web:e2e -- --grep 'orchestration interruption'
```

Expected: GREEN in Chromium and WebKit before the product checkpoint.

- [ ] **Step 5: Prove the first working product checkpoint in a browser**

Run:

```bash
npm ci
./node_modules/.bin/playwright install chromium webkit
scripts/web/build.sh
npm run test:web:coverage
scripts/web/coverage.sh
npm run serve:web
```

Open the printed local URL directly. Capture desktop and 320px screenshots of
first run, immutable review, durable Home, import preview, and recovery queue.
Use the visual-review skill to fix overlap, clipping, contrast, focus, and state
ambiguity before proceeding.

In `community-flow.spec.js`, use two isolated real browser contexts: browser A
creates, posts, downloads its produced export; browser B starts clean, uploads
that exact download, previews and selects it, joins, and reloads. Assert the
imported entry keeps A's complete signer/namespace IDs, B has a distinct fresh
member signer within A's namespace, only selected entries persist, and every
join/export screen says public-data import/export is not identity recovery.
Repeat the end-to-end artifact handoff in Chromium and WebKit.
Instrument the registry in that E2E: assert both the produced download and the
consumed upload pass through the registered file port, no reserved port opens,
and cancellation/oversize leaves community state unchanged.
In a third clean context, execute **Try the Riverside demo** through the real
fixture preview → readable selection → fresh-member join → persistence-confirm
path (no mocked/privileged admission), reload, and compare its resulting
projection to the committed fixture golden output.

## Task 8: Make offline releases coherent and installable

**Files:**
- Create/modify `apps/web/sw.js`, `manifest.webmanifest`, `icon.svg`
- Create/modify `apps/web/src/release-client.js`
- Extend build/static-contract scripts and tests
- Create `apps/web/test/unit/release-client.test.js`,
  `apps/web/test/unit/service-worker.test.js`,
  `apps/web/test/e2e/service-worker.spec.js`
- Create `scripts/web/test-safari-service-worker.mjs` and
  `scripts/web/test/safari-runner.test.mjs`

- [ ] **Step 1: Write RED service-worker lifecycle tests**

Cover incomplete precache deletion, controlling-release navigation, old/new
hashed asset isolation, waiting-worker notification, all-client draft/write
acknowledgement, unresponsive-client close-tabs state, explicit
`ACTIVATE_RELEASE`, `controllerchange` reload, and deletion only after no client
references the old release. Run the detailed worker lifecycle suite in Chromium,
the Playwright engine that exposes service-worker instrumentation. A separate
real-Safari proof below covers Safari's actual worker/offline behavior; do not
claim Playwright WebKit service-worker support.

Run the authored-worker unit contract and Chromium lifecycle E2E before worker
implementation:

```bash
npm run test:web:unit -- --test-name-pattern='release|service worker'
./node_modules/.bin/playwright test --project=chromium apps/web/test/e2e/service-worker.spec.js
```

Expected: FAIL because the release worker/client lifecycle is not implemented;
retain the RED output.

Before implementing the Safari runner, write RED tests with injected
`driverFactory`, `serverController`, and output streams: driver unavailable,
Remote Automation disabled, readiness timeout, server-stop failure, offline
reload failure, success/version record, and guaranteed driver quit/server
cleanup. Expected: `node --test scripts/web/test/safari-runner.test.mjs` fails
because the runner does not exist.

- [ ] **Step 2: Generate one content-hashed release**

`build.sh` emits HTML naming one release's hashed JS/WASM/CSS/manifest assets and
injects that manifest into the authored worker. The worker precaches all or
nothing, caches no mutable data, imports no vault/storage module, and answers
navigation from its controlling release cache. It never silently activates
under an open page. Add `apps/web/sw.js` to c8's explicit `--all --include`
production set and unit-test every worker message/cache branch with injected
ports; generated asset-manifest data is the only generated exclusion.

- [ ] **Step 3: Implement honest install/update UI**

Capture `beforeinstallprompt` when present, preserve a quiet retry after
dismissal, hide on `appinstalled`/standalone, and show browser-menu instructions
when the event is unavailable. Update reload is enabled only when all clients
have persisted drafts and no mutation/recovery queue is active.

Add a manifest/installability contract test for same-origin `start_url` and
`scope`, standalone display, nonempty name/short name, theme/background colors,
usable SVG icon, successful post-interactive worker registration, and a secure
context (localhost in tests, HTTPS in any future deployment). The test must
distinguish valid manifest metadata from actual browser installation.

- [ ] **Step 4: Prove offline and multi-release behavior with the right engines**

Install/load once, enter but do not post a draft, take the origin offline,
restart, unlock, prove the draft survived, complete/post it, reload, and export
in Playwright Chromium. Stage a second local release and prove no page
receives mixed assets in Chromium. Run storage/WebCrypto/UI behavior in both
Playwright Chromium and WebKit.

Then run `scripts/web/test-safari-service-worker.mjs` through the system
`safaridriver` and pinned `selenium-webdriver 4.46.0`: load the local release,
serve it on a fresh random loopback port and per-run path/database namespace,
wait for `navigator.serviceWorker.ready` plus a controlling worker, stop the
origin server, reload the same URL, and assert the controlling cached release
renders and persisted Home opens. The unique origin/path prevents prior Safari
cache/worker state from satisfying the test. The runner records Safari and driver versions and fails with one concrete
instruction if macOS Safari Remote Automation is disabled. This real-Safari
check, not Playwright WebKit, certifies Safari service-worker/offline behavior.
Implement the runner only after its injected-port tests are RED, add
`test:web:safari-sw` to `package.json`, and keep the runner itself under c8.
Then run `npm run test:web:coverage`, `scripts/web/coverage.sh`,
`npm run test:web:e2e`, and `npm run test:web:safari-sw`; Task 8 is not a green
checkpoint or commit boundary until all four pass.

## Task 9: Lock security, accessibility, and product acceptance

**Files:**
- Create/modify static-contract and Playwright tests
- Create `docs/product/riot-web-usability-results.md`

- [ ] **Step 1: Rerun and fix the exact host security contract**

Run the built site behind a local test server that emits the exact design CSP,
Referrer-Policy, nosniff, and Permissions-Policy headers. Static tests fail on
third-party URLs/assets, `innerHTML`, inline/eval/blob/data runtime paths, secret
logging tokens, service-worker vault imports, private-group UI, or a signing
path without an immutable review. Also fail if `riot-core`/`riot-client` imports
browser networking or Nym packages; if controller DTO/state contains transport
kind, endpoint, renderer domain, or gateway authority; if any adapter other than
file is implemented; if reserved transports leak into UI; or if a locally
committed record reaches normal projection/export/exchange before exact
durability acknowledgement. Verify future adapter comments cannot weaken CSP
with wildcard connectivity. Inspect the service-worker source itself and fail on
direct `indexedDB`/IDB access as well as storage/vault imports; this is an
intended-code contract, not a claimed same-origin security boundary.

Run `node scripts/web/verify-static-contracts.mjs target/web-dist` and
`npm run test:web:e2e -- --grep security`. If either fails, add the failing
fixture/behavior as a regression test, make the smallest production correction,
and rerun before proceeding.

- [ ] **Step 2: Rerun and fix accessibility and responsive behavior**

In Chromium and WebKit prove one `main`, ordered headings, labeled forms, 44px
targets, keyboard-only operation, visible focus, dialog focus restoration,
non-color status, non-stealing `aria-live`, reduced motion, 320px layout, and
200% zoom without horizontal overflow. Newly posted updates must not move
focus; expired updates live under collapsed Earlier.
Any failure first becomes a focused regression assertion in
`accessibility.spec.js`, then receives the smallest markup/style/controller
fix; rerun Chromium and WebKit before proceeding.

- [ ] **Step 3: Prepare and dry-run the two timed usability checkpoints**

Create the results sheet and facilitator script for five create → post → offline
reload → export sessions and five supplied-bundle import → understand new member
signer → Home sessions on each supported browser. It records elapsed time,
abandonment step, coaching, and whether the participant can explain that export
does not restore identity. The same participant must distinguish **Saved on this
browser**, **Export prepared**, and future-only **Exchanged**; more than one of
five calling a prepared file “delivered” fails that checkpoint. Dry-run both
scripts with automated fixtures to prove
the setup/reset steps work. Human recruitment and the required four-of-five
results are the explicit product-readiness gate before calling the slice
product-ready; they are not fabricated or treated as an autonomous code gate.
If Rabble supplies participants during execution, record the real results;
otherwise hand off the ready protocol and label the build engineering-complete,
not product-ready.

- [ ] **Step 4: Run final repository verification from a clean build**

```bash
cargo test --workspace --all-features
cargo check --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
scripts/web/bootstrap.sh
npm ci
./node_modules/.bin/playwright install --with-deps chromium webkit
scripts/web/build.sh
scripts/web/coverage.sh
node scripts/web/verify-static-contracts.mjs target/web-dist
npm run test:web:e2e
npm run test:web:safari-sw
git diff --check
git status --short
```

Expected: every command passes, Rust and authored JS coverage are exactly 100%
for all configured metrics, the built PWA makes zero third-party requests, and
only declared/claimed paths are staged.

- [ ] **Step 5: Review, commit, and hand off without touching user work**

Use `superpowers:requesting-code-review` on the final diff, resolve findings,
rerun Step 4, inspect the exact staged diff, and commit only PWA plan paths.
Release the collaboration claim. Do not create a PR, deploy, change DNS, or add
remote signer infrastructure without a separate user request.

## Plan self-review

- **Spec coverage:** Every Revision 12 acceptance criterion maps to Tasks 3–9;
  remote OAuth/recovery, concrete duplex transports, Replicator retention,
  Viewer-mode UI, private groups, analytics, and deployment remain out of scope.
- **TDD:** Every implementation task starts with a named failing test and an
  expected RED result before production edits.
- **Durability:** Pending profile and every pending bundle acknowledgement
  boundary are explicit in Rust, JavaScript transaction tests, and hard-
  termination E2E; pending records are quarantined from read/export/exchange.
- **Security:** The plan preserves the honest same-origin compromise limitation;
  it does not claim WebCrypto structured-clone keys are hardware-backed or
  externally rollback-protected.
- **Type consistency:** Times are decimal strings, bytes are byte arrays,
  complete IDs/digests are lowercase hex, admission routes/enums are closed,
  browser receipt kind IDs are bounded local metadata, and every nested DTO
  carries version 1.
- **Authority:** Community identity/projection contain no domain, endpoint, or
  transport kind. `CommunityReplica` has no signer; `PublisherAuthority` is
  optional; gateways/renderers are disposable; file is the only MVP adapter.
- **Coverage:** Authored Rust and JS remain measured. Only generated binding
  glue and the exact verified upstream vendor directory are excluded.
- **Placeholder scan:** This plan contains no unresolved marker, fake endpoint,
  invented remote signer, or deferred acceptance assertion.
