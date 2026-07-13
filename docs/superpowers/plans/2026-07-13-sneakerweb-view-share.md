# SneakerWeb View-and-Share Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Riot on iOS and Android open, retain, browse, inspect, block, export, and share standard SneakerWeb `.snk` collections without creating or re-signing SneakerWeb sites.

**Architecture:** A fixed-namespace `riot_core::sneakerweb` subsystem uses Willow Drop Format only at a bounded staging boundary, then atomically joins verified entries into the production SQLite database. Native iOS and Android clients own document access, isolated WebViews, system sharing, nearby transport, and accessible UI; Rust owns verification, persistence, selection, encoding, leases, carrier records, app bindings, and idempotent space commits. Community sharing uses the existing signed-app runtime and the separately landed Newswire contract; neither miniapp JavaScript nor site JavaScript receives bytes, raw IDs, database handles, or signing authority.

**Tech Stack:** Rust 2021, `willow25 0.6.0-alpha.3` Drop Format, `ufotofu 0.12.4`, SQLite/`rusqlite`, UniFFI 0.32, Swift 6/SwiftUI/WebKit, Kotlin 2.2/Android WebView, canonical CBOR, XCTest/XCUITest, JUnit/instrumentation tests, Playwright for the starter miniapp, cargo-tarpaulin, cargo-llvm-cov.

**Plan status:** **PLAN REVIEW ESCALATION REQUIRED.** Three automatic gate
rounds are exhausted. Do not execute until Rabble chooses Revise, Override,
Simplify, or Cancel.

---

## Scope and dependency stop lines

This is one delivery program with four independently useful increments:

1. protocol activation and official CLI interoperability;
2. durable open/browse/inspect/block on both mobile platforms;
3. standard file sharing and direct nearby transfer; and
4. public-community carrier, Newswire, and Sneaker Directory sharing.

Do not implement all increments as one unreviewed diff. Each numbered task is a
Metaswarm work unit and must complete its own IMPLEMENT, VALIDATE, ADVERSARIAL
REVIEW, COMMIT loop. Tasks 1–5 establish the Rust contract; Tasks 6–9 deliver
the first user-visible release slice; Tasks 10–12 add carrying and community
sharing; Task 13 is the blocking release-evidence gate.

Two dependencies are deliberately outside this plan:

- `docs/superpowers/plans/2026-07-12-multi-space-sqlite-store.md` must be fully
  landed through its release gate before Task 1. The checkout used to write
  this plan still has no `RiotDatabase`, `DatabaseSession`, `SpaceSession`, or
  `rusqlite` dependency. SneakerWeb must not create a second production store.
- The Newswire destination in Task 12 waits for an approved and landed plan
  derived from `docs/superpowers/specs/2026-07-13-multi-community-open-newswire-mvp-design.md`.
  The current `2026-07-13-newswire-core-slice-1.md` is marked escalated and has
  no attachment union. File, nearby, and Directory sharing do not wait for it.

The signed app runtime in `crates/riot-core/src/apps/`, its UniFFI surface in
`crates/riot-ffi/src/apps_ffi.rs`, and both native app hosts are landed and are
the base for Task 11. macOS remains excluded and receives no `.snk` document
declaration, viewer, or menu item.

## Cross-task invariants

- Only namespace
  `9fc4cc86cad94d11025afcf75e0dab24bc3c6c91f0cd92fbe0ca574d469c681e`
  enters the SneakerWeb collection.
- Import verifies complete payloads before one atomic commit. Failure,
  cancellation, quota rejection, or process restart cannot partially mutate
  authoritative rows.
- Export preserves original entries, capabilities, signatures, timestamps,
  and payloads. Riot never creates or signs a SneakerWeb site entry.
- Raw domain IDs, digests, signatures, and keys are complete in Details and
  absent from default cards. No UI, fixture, diagnostic, or test truncates an
  identifier.
- A carrier or binding is a Riot recommendation record, not an authorship
  claim. Carrier attribution comes only from the verified outer Willow entry.
- A synced carrier card never downloads automatically. `Get collection` is a
  separate native action.
- `.coverage-thresholds.json` is the only threshold source. Every work unit
  follows RED, confirms the expected failure, implements the minimum GREEN,
  runs focused and regression checks, passes adversarial review, then commits.

## Planned file map

### Contract, fixtures, and validation

- Modify `Cargo.toml` and `Cargo.lock` — request `drop_format` only on the
  production `riot-core` dependency edge and record the resolved graph.
- Modify `crates/riot-core/Cargo.toml` — request Drop Format and register
  focused conformance tests.
- Modify `crates/xtask/src/main.rs` — replace the Phase 0A prohibition with the
  scoped SneakerWeb feature-closure contract.
- Modify `fixtures/manifest.json` and `fixtures/feature-closure.txt` — version
  and freeze the new public-kernel closure.
- Create `fixtures/sneakerweb/` — pinned official files, hostile corpus,
  oracle/checksum manifest, and reproducible generation instructions.
- Create `scripts/sneakerweb-interop.sh`, `scripts/coverage-gate.sh`,
  `scripts/sneakerweb-oracle.sh`,
  `scripts/verify-xcresult-tests.sh`, `scripts/android-sneakerweb-test-gate.sh`,
  and `scripts/sneakerweb-physical-rehearsal.sh`.

### Rust core and UniFFI

- Create `crates/riot-core/src/sneakerweb/{mod,protocol,codec,collection,blob,viewer,nearby,carrier,social,tasks}.rs`.
- Modify the landed database migration/schema owner from the multi-space plan;
  use its actual path, do not introduce a parallel migration runner.
- Modify `crates/riot-core/src/{lib,apps/manifest,apps/bridge,apps/starter}.rs`.
- Modify the landed Newswire model/path/entry files in Task 12 only after its
  attachment contract is approved and present.
- Create focused tests under `crates/riot-core/tests/sneakerweb_*.rs`.
- Create `crates/riot-ffi/src/sneakerweb_ffi.rs`, export it from
  `crates/riot-ffi/src/lib.rs`, and create
  `crates/riot-ffi/tests/sneakerweb_contract.rs`.

### iOS

- Modify `apps/ios/Riot/Info.plist`, `apps/ios/Riot/RiotApp.swift`,
  `apps/ios/Riot/AppModel.swift`, `apps/ios/Riot/ConferenceShellView.swift`,
  `apps/ios/Riot/Apps/{AppBridgeController,AppReviewSheet,AppRuntimeView,RiotJS}.swift`, and
  the Xcode project only through Xcode-aware project editing.
- Create `apps/ios/Riot/SneakerWeb/` files for document intake, library model,
  library/details/storage/share views, loopback server, isolated viewer, share
  coordinator, and nearby channel.
- Create focused XCTest/XCUITest files named in Tasks 6, 8, 10, and 12.

### Android

- Modify `apps/android/app/src/main/AndroidManifest.xml`,
  `apps/android/app/src/main/kotlin/org/riot/evidence/{MainActivity,RiotController}.kt`,
  and `apps/android/app/src/main/kotlin/org/riot/evidence/apps/{RiotJsBridge,RiotJsShim,AppWebViewHost}.kt`.
- Create `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/` for
  document intake, library state/UI, viewer, share coordinator, and nearby
  channel.
- Create the exact unit/instrumentation suites required by the Android gate.

### Starter Sneaker Directory and docs

- Create `fixtures/apps/sneaker-directory/` source and packed artifacts.
- Create `scripts/apps/package.json` and `scripts/apps/package-lock.json` with
  exact `@playwright/test 1.61.1` development dependency.
- Modify `scripts/apps/miniapp-contracts.mjs`,
  `scripts/apps/miniapp-browser.spec.mjs`, and the starter catalog tests.
- Modify `docs/product/product-brief.md`, `README.md`, and
  `SERVICE-INVENTORY.md` only for behavior actually delivered.
- Create `docs/quality/2026-07-13-sneakerweb-release-evidence.md`.

## Task 0: Verify prerequisites and freeze the execution baseline

**Files:**
- Read: `docs/superpowers/specs/2026-07-13-sneakerweb-view-share-design.md`
- Read: `docs/superpowers/plans/2026-07-12-multi-space-sqlite-store.md`
- Read: `docs/superpowers/specs/2026-07-13-multi-community-open-newswire-mvp-design.md`
- Read: `.coverage-thresholds.json`
- Test: existing repository state only

- [ ] **Step 1: Create an isolated implementation worktree**

Invoke `superpowers:using-git-worktrees`, choose a feature branch rooted at the
approved design commit, and copy no uncommitted files from the current dirty
checkout. Record the clean starting commit:

```sh
git status --short
git rev-parse HEAD
```

Expected: no output from `git status --short`; a full 40-character commit ID.

- [ ] **Step 2: Prove the SQLite dependency is landed**

```sh
rg -n 'pub struct RiotDatabase|pub struct DatabaseSession|pub struct SpaceSession' crates/riot-core crates/riot-ffi
rg -n '^rusqlite\s*=' Cargo.toml crates/riot-core/Cargo.toml
cargo test --workspace --all-features
```

Expected: all three public types and the pinned SQLite dependency are found,
and the workspace is green. If any type is absent, stop this plan and complete
the approved multi-space SQLite plan; do not scaffold substitute types here.

- [ ] **Step 3: Record destination availability without weakening scope**

```sh
rg -n 'struct NewsPostV1|PublicAttachmentRefV1|attachment' crates/riot-core/src/newswire
rg -n 'pub mod apps|struct AppManifest|AppDataBridge' crates/riot-core/src
```

Expected: the app runtime matches are present. If the Newswire attachment union
is absent, mark only Task 12's Newswire substep blocked; continue through file,
nearby, carrier, and Directory work.

- [ ] **Step 4: Run the baseline contract and coverage commands**

```sh
cargo xtask validate-contracts
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
```

Expected: PASS before any SneakerWeb change. Save command output in the work
unit evidence, not in a new product file.

## Task 1: Activate Willow Drop Format with pinned official evidence

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/riot-core/Cargo.toml`
- Modify: `crates/xtask/src/main.rs`
- Modify: `fixtures/manifest.json`
- Modify: `fixtures/feature-closure.txt`
- Create: `fixtures/sneakerweb/README.md`
- Create: `fixtures/sneakerweb/manifest.json`
- Create: `fixtures/sneakerweb/official-1.0.1.snk`
- Create: `crates/riot-core/tests/sneakerweb_feature_contract.rs`
- Create: `scripts/sneakerweb-interop.sh`
- Create: `scripts/sneakerweb-oracle.sh`

- [ ] **Step 1: Write RED feature-closure and oracle tests**

Create tests that assert the current graph fails the new contract and the
official fixture cannot yet be decoded:

```rust
#[test]
fn drop_format_is_requested_only_by_riot_core() {
    let root = workspace_root();
    let report = validate_sneakerweb_feature_scope(&root);
    assert_eq!(report.requesters, vec!["crates/riot-core"]);
    assert!(!report.release_graph_has_default_features);
    assert!(!report.non_core_graph_has_drop_format);
}

#[test]
fn pinned_official_drop_has_expected_identity() {
    let manifest = official_manifest();
    assert_eq!(manifest.sneakerweb_version, "1.0.1");
    assert_eq!(manifest.crate_sha256,
        "dc3d20ffadb278e7a8c8e5a06890e10d21c5bcd6d08d8f5811877f6bc9d797c8");
    assert_eq!(sha256_file(manifest.fixture_path), manifest.fixture_sha256);
}
```

Run:

```sh
cargo test -p riot-core --test sneakerweb_feature_contract
cargo xtask validate-contracts
```

Expected: RED because `drop_format` is still forbidden and the fixture
contract is absent.

- [ ] **Step 2: Make the smallest scoped feature change**

Keep the workspace declaration at `default-features = false, features =
["std"]`; change only the normal `riot-core` requester edge:

```toml
# crates/riot-core/Cargo.toml
[dependencies]
willow25 = { workspace = true, features = ["drop_format"] }
```

Update xtask to accept exactly that production requester while continuing to
reject Drop Format from dev/build dependencies, other crates, OpenMLS,
conformance injection, default-feature widening, and version drift. Version
the fixture contract from `riot-phase0a/1` to `riot-public-kernel-sneakerweb/1`.

- [ ] **Step 3: Bootstrap and pin the public CLI oracle**

Create `scripts/sneakerweb-oracle.sh` with two explicit modes. `install`
downloads/builds exactly `sneakerweb 1.0.1` once into
`build/tools/sneakerweb/bin/sneakerweb`:

```sh
scripts/sneakerweb-oracle.sh install --version 1.0.1 \
  --crate-sha256 dc3d20ffadb278e7a8c8e5a06890e10d21c5bcd6d08d8f5811877f6bc9d797c8
```

The script downloads the crates.io `sneakerweb-1.0.1.crate` to a temporary
directory, verifies the `.crate` checksum before extraction or build, rejects
archive traversal/symlink entries, extracts it, then runs `cargo install
--path "$VERIFIED_SOURCE" --locked --root build/tools/sneakerweb`.
It records the full executable SHA-256 in the fixture manifest and fails if the
built version differs. `verify --offline` never downloads: it requires that
exact local path and verifies its recorded hash. CI/release setup runs
`install`; ordinary fixture and hostile tests do not need the executable.

- [ ] **Step 4: Pin reproducible offline oracle evidence**

`fixtures/sneakerweb/manifest.json` must contain the upstream crate checksum,
installed CLI SHA-256, exact full generation command, `.snk` digest, entry
count, payload count/bytes, namespace, and license. `README.md` must explain
how to regenerate into a temporary directory and compare hashes without
requiring the network during ordinary tests.

- [ ] **Step 5: Verify all target graphs and commit**

```sh
cargo xtask validate-contracts
cargo test -p riot-core --test sneakerweb_feature_contract
cargo build -p riot-ffi --target aarch64-apple-ios --locked
cargo build -p riot-ffi --target aarch64-apple-ios-sim --locked
cargo build -p riot-ffi --target aarch64-linux-android --locked
cargo build -p riot-ffi --target x86_64-linux-android --locked
scripts/sneakerweb-oracle.sh verify --offline --version 1.0.1
scripts/sneakerweb-interop.sh --offline --version 1.0.1 \
  --cli build/tools/sneakerweb/bin/sneakerweb --contract-only
```

Expected: every command passes and `cargo tree` shows Drop Format only in a
graph containing `riot-core`.

```sh
git add Cargo.toml Cargo.lock crates/riot-core/Cargo.toml crates/xtask/src/main.rs \
  fixtures/manifest.json fixtures/feature-closure.txt fixtures/sneakerweb \
  crates/riot-core/tests/sneakerweb_feature_contract.rs scripts/sneakerweb-interop.sh \
  scripts/sneakerweb-oracle.sh
git diff --cached --check
git commit -m "build(sneakerweb): activate pinned Drop Format contract"
```

## Task 2: Implement the bounded fixed-namespace decoder and encoder

**Files:**
- Create: `crates/riot-core/src/sneakerweb/mod.rs`
- Create: `crates/riot-core/src/sneakerweb/protocol.rs`
- Create: `crates/riot-core/src/sneakerweb/codec.rs`
- Modify: `crates/riot-core/src/lib.rs`
- Modify: `crates/riot-core/Cargo.toml`
- Create: `crates/riot-core/tests/sneakerweb_codec.rs`
- Create: `crates/riot-core/tests/sneakerweb_hostile.rs`
- Create: `fixtures/sneakerweb/hostile/manifest.json`

- [ ] **Step 1: Write RED protocol and hostile-input tests**

Define and test this closed public surface:

```rust
pub const SNEAKERWEB_NAMESPACE: [u8; 32] = [
    0x9f, 0xc4, 0xcc, 0x86, 0xca, 0xd9, 0x4d, 0x11,
    0x02, 0x5a, 0xfc, 0xf7, 0x5e, 0x0d, 0xab, 0x24,
    0xbc, 0x3c, 0x6c, 0x91, 0xf0, 0xcd, 0x92, 0xfb,
    0xe0, 0xca, 0x57, 0x4d, 0x46, 0x9c, 0x68, 0x1e,
];

pub struct DecodeLimits {
    pub encoded_bytes: u64,
    pub entries: u32,
    pub payload_bytes: u64,
    pub path_components: u16,
    pub path_component_bytes: u16,
    pub path_total_bytes: u32,
    pub capability_depth: u16,
}

pub enum SnkCodecError {
    Malformed, Truncated, WrongNamespace, ForgedEntry, ForgedCapability,
    DigestMismatch, IncompletePayload, LimitExceeded, Cancelled,
}
```

Tests must cover the official fixture, wrong namespace, altered signature,
altered WILLIAM3 digest, declared-length mismatch, incomplete payload, trailing
bytes, non-canonical termination, deep capability, excessive path, integer
overflow, cancellation at every chunk, and no production/reusable secret in
fixtures.

The release limits are constants, not caller-configurable increases: 1 GiB
encoded and decoded payload bytes, 1,024 domains, 50,000 entries, 256 MiB per
resource, 4,096 path bytes, 256 path components, and 1,024 bytes per component.
Tests exercise the value at, one below, and one above each ceiling before any
allocation or integer narrowing.

```sh
cargo test -p riot-core --features conformance --test sneakerweb_codec
cargo test -p riot-core --features conformance --test sneakerweb_hostile
```

Expected: RED because `riot_core::sneakerweb` does not exist.

- [ ] **Step 2: Implement bounded decoder ownership**

Use `willow25::drop_format::DropDecoder` with Riot-owned bounded producers.
Return staged authorised components only after verifying canonical entry and
capability encoding, Meadowcap authority, signature, complete payload length,
WILLIAM3 digest, fixed namespace, and every limit. Never call upstream
`import_drop`, never accept a caller namespace, and never pass parser text into
the public error.

- [ ] **Step 3: Implement selected-domain streaming encoding**

Expose a producer that accepts an ordered-unique set of full `[u8; 32]` domain
IDs and streams their complete original components into `DropEncoder` without
signing or mutation:

```rust
pub trait CompleteSiteSource {
    fn entries_for(&self, domains: &[[u8; 32]])
        -> Result<Box<dyn Iterator<Item = AuthorisedSiteEntry> + '_>, SnkCodecError>;
}

pub fn encode_selected_drop<S: CompleteSiteSource, W: std::io::Write>(
    source: &S,
    domains: &[[u8; 32]],
    output: W,
    cancel: &CancellationToken,
) -> Result<EncodeSummary, SnkCodecError>;
```

- [ ] **Step 4: Prove official round trip and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_codec
cargo test -p riot-core --features conformance --test sneakerweb_hostile
scripts/sneakerweb-interop.sh --offline --version 1.0.1 \
  --cli build/tools/sneakerweb/bin/sneakerweb --codec
cargo clippy -p riot-core --all-features --all-targets -- -D warnings
```

Expected: Riot decodes official bytes; the official CLI accepts Riot's selected
export with exact entry/capability/signature/payload components.

```sh
git add crates/riot-core/src/lib.rs crates/riot-core/src/sneakerweb \
  crates/riot-core/tests/sneakerweb_codec.rs crates/riot-core/tests/sneakerweb_hostile.rs \
  crates/riot-core/Cargo.toml fixtures/sneakerweb/hostile
git commit -m "feat(sneakerweb): add bounded interoperable codec"
```

## Task 3: Add atomic collection persistence, provenance, block, and removal

**Files:**
- Create: `crates/riot-core/src/sneakerweb/collection.rs`
- Modify: landed `RiotDatabase` schema/migration owner from Task 0
- Modify: `crates/riot-core/src/sneakerweb/mod.rs`
- Create: `crates/riot-core/tests/sneakerweb_collection.rs`
- Create: `crates/riot-core/tests/sneakerweb_collection_races.rs`

- [ ] **Step 1: Write RED migration and atomicity tests**

Reuse the landed canonical `accepted_entries`, payload, `live_entries`, path,
and receipt tables. Add the exact design tables `sneaker_sources`,
`sneaker_source_domains`, `sneaker_source_entries`, `sneaker_sites`,
`sneaker_blocks`, `sneaker_removals`, `storage_reservations`, `portable_blobs`,
`space_blob_refs`, `space_sneaker_share_ops`,
`app_public_collection_bindings`, and `blob_chunks`. Use the landed migration
runner and its naming conventions.

Tests must prove first migration/reopen, fixed namespace CHECK constraints,
100 overlapping files converging without duplicate cards, arbitrary merge
order, stale/equal/newer Willow joins, rollback after every injected SQL and
storage fault, quota reservation, cancellation before commit, restart cleanup,
source/site/blob removal accounting, persistent block generation, block versus
commit/read/export barriers, and reserved namespace rejection from ordinary
space constructors/projectors.

Assert four distinct valid zero-change outcomes while retaining an inspectable
receipt: `AlreadyUpToDate`, `OnlyOlderVersions`, `AllSitesBlocked`, and
`EmptyCollection`. Corrupt one already accepted payload after commit and prove
the next read returns `INVALID_DROP` with recovery action `RemoveLocalCopy`
and safe fact `StoredPayloadDigestMismatch`, offers open-another-file, never serves the bytes, and
leaves independent sources untouched.

Budget tests also cover 2 GiB retained SneakerWeb payload/blob storage while
leaving 512 MiB device free, 250,000 accepted entries, 10,000 receipts,
1,000,000 source-entry associations, 3 GiB combined database/CAS, one Open,
one Export, two transfers, 1 GiB aggregate reservations, 100-row library and
Details pages, 32 community destinations, and four active space-share children.

- [ ] **Step 2: Implement the collection transaction boundary**

Expose database-owned methods with no raw SQL or path in public DTOs:

```rust
impl RiotDatabase {
    pub fn begin_sneaker_stage(&self, route: SnkOpenRoute, expected: u64)
        -> Result<SneakerStage, SneakerError>;
    pub fn commit_sneaker_stage(&self, stage: VerifiedSneakerStage)
        -> Result<OpenSnkOutcome, SneakerError>;
    pub fn list_sneaker_sites(&self, cursor: Option<PageCursor>)
        -> Result<SneakerSitePage, SneakerError>;
    pub fn block_sneaker_domain(&self, domain: [u8; 32])
        -> Result<BlockOutcome, SneakerError>;
}
```

The commit transaction inserts accepted components/payload references,
recomputes live winners, updates materialized site/source/storage rows, bumps
collection generation, and releases reservation together. Rebuildable Riot
space/app/newswire projectors must ignore a quarantined reserved namespace.

- [ ] **Step 3: Implement page snapshots and complete Details**

Authenticate cursors over `(database_generation, collection_generation,
last_sort_key)`. Default site/received DTOs omit raw IDs and digests. Details
pages expose complete namespace, domain, entry, capability, signature, digest,
timestamp, received source, and integrity labels. Mutation returns
`STALE_CURSOR`; native restarts from page one without merging snapshots.

- [ ] **Step 4: Verify transaction/race behavior and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_collection
cargo test -p riot-core --features conformance --test sneakerweb_collection_races
cargo test -p riot-core --all-features
```

Expected: all atomicity and deterministic barrier tests pass.

```sh
git add crates/riot-core/src/sneakerweb crates/riot-core/tests/sneakerweb_collection*.rs \
  crates/riot-core/src/database
git commit -m "feat(sneakerweb): persist atomic collection state"
```

The SQLite prerequisite is required to own its migration runner under
`crates/riot-core/src/database/`. If the landed prerequisite uses a different
directory, stop before Task 1, revise every affected path in this plan, and run
the mandatory plan gate again; do not improvise a second migration boundary.

## Task 4: Add portable blob CAS, leases, export, and crash reconciliation

**Files:**
- Create: `crates/riot-core/src/sneakerweb/blob.rs`
- Create: `crates/riot-core/src/sneakerweb/tasks.rs`
- Modify: `crates/riot-core/src/sneakerweb/{mod,codec,collection}.rs`
- Create: `crates/riot-core/tests/sneakerweb_blob.rs`
- Create: `crates/riot-core/tests/sneakerweb_export.rs`

- [ ] **Step 1: Write RED lease/CAS/export tests**

Cover exact selected domains, blocked/unavailable domain rejection, 15-minute
idle expiry refreshed only by successful read/retain, `read_range <= 1 MiB`,
idempotent close, post-close error, temporary cleanup after cancel/crash,
retain-as-portable-blob, reference counting, same-digest thread/process
contention, every fsync/install/transaction crash point, winner/loser startup
reconciliation, unsupported filesystem mapping, and exact CLI cross-import.

- [ ] **Step 2: Implement exact no-replace publication**

Define one platform adapter:

```rust
pub trait AtomicBlobInstaller: Send + Sync {
    fn install_no_replace(&self, staging: &Path, digest_path: &Path)
        -> Result<InstallOutcome, BlobInstallError>;
}

pub enum InstallOutcome { Installed, AlreadyPresent }
pub enum BlobInstallError { Io, CrossDevice, UnsupportedFilesystem, Cancelled }
```

Use a process-local per-digest lock plus Linux/Android
`renameat2(RENAME_NOREPLACE)` when available and a same-filesystem
link/create-exclusive fallback that never replaces an existing digest path.
On Apple platforms use a same-volume exclusive link/create sequence with
directory fsync. Map inability to prove no-replace semantics to
`UnsupportedFilesystem`; do not fall back to replacing rename.

- [ ] **Step 3: Implement task and lease state machines**

Implement `SnkExportTask`, `ExportLease`, `PreparedBlobLease`, and
`PortableBlobLease` exactly as specified. Every repeated `finish` returns the
same lease identity or terminal error. Every read/retain rechecks selected
domain block generations. Owner/database close makes noncommitted handles
`OWNER_CLOSED`.

- [ ] **Step 4: Verify and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_blob
cargo test -p riot-core --features conformance --test sneakerweb_export
scripts/sneakerweb-interop.sh --offline --version 1.0.1 \
  --cli build/tools/sneakerweb/bin/sneakerweb --round-trip
```

```sh
git add crates/riot-core/src/sneakerweb crates/riot-core/tests/sneakerweb_blob.rs \
  crates/riot-core/tests/sneakerweb_export.rs
git commit -m "feat(sneakerweb): add leased portable blob storage"
```

## Task 5: Expose closed asynchronous UniFFI contracts

**Files:**
- Create: `crates/riot-ffi/src/sneakerweb_ffi.rs`
- Modify: `crates/riot-ffi/src/lib.rs`
- Modify: `crates/riot-ffi/Cargo.toml`
- Create: `crates/riot-ffi/tests/sneakerweb_contract.rs`
- Create: `crates/riot-ffi/tests/sneakerweb_races.rs`

- [ ] **Step 1: Write RED generated-surface tests**

Require every DTO to carry `schema_version = 1`, every domain argument to be
exactly 32 bytes, no namespace parameter, no filesystem path/URI, full typed
errors, panic containment, stale cursors, repeated terminal results, and
finish/cancel/block/owner-close race linearization.

- [ ] **Step 2: Implement the actor and worker boundary**

One short-lived actor linearizes commands and submits bounded transactions to
the database worker. A bounded two-worker executor handles streaming I/O,
Drop parsing/encoding, hashing, and verification under cancellation tokens.
At this point expose only the task methods whose core implementations exist:

```text
OpenSnkTask: write_chunk, progress, finish, cancel
SnkExportTask: progress, finish, cancel
ExportLease: metadata, read_range, retain_as_portable_blob, close
PreparedBlobLease: metadata, retain, release
PortableBlobLease: metadata, read_range, close
ResourceLease: metadata, read_range, close
```

Expose the exact factories and database calls `begin_snk_open`,
`begin_snk_open_from_blob`, `undo_snk_open`, `list_sneaker_sites`,
`list_received_sneaks`, `list_blocked_sneaker_sites`, `list_sneaker_storage`,
`get_sneaker_site`, `get_sneaker_details`, `get_received_sneak_details`,
`resolve_sneaker_resource`, `block_sneaker_domain`,
`unblock_sneaker_domain`, `remove_sneaker_source`, `remove_sneaker_site`,
`remove_portable_blob`, and `create_snk_export`. Tasks 10 and 11 extend this
same file only after their corresponding core task types exist. Do not expose
generic parser errors or raw handles.

- [ ] **Step 3: Prove native bindings contain only the closed API**

```sh
cargo test -p riot-ffi --test sneakerweb_contract
cargo test -p riot-ffi --test sneakerweb_races
cargo xtask generate-bindings
rg -n 'OpenSnkTask|SnkExportTask|ResourceLease' build/generated/riot-ffi
! rg -n 'PathBuf|file_path|content_uri|namespace_id.*begin_snk' build/generated/riot-ffi
```

Expected: tests pass, required generated types exist, forbidden types do not.

- [ ] **Step 4: Commit**

```sh
git add crates/riot-ffi/src/sneakerweb_ffi.rs crates/riot-ffi/src/lib.rs \
  crates/riot-ffi/Cargo.toml crates/riot-ffi/tests/sneakerweb_contract.rs \
  crates/riot-ffi/tests/sneakerweb_races.rs
git commit -m "feat(ffi): expose SneakerWeb task contracts"
```

## Task 6: Ship zero-ceremony iOS document opening and library management

**Files:**
- Modify: `apps/ios/Riot/Info.plist`
- Modify: `apps/ios/Riot/RiotApp.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerDocumentIntake.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerLibraryModel.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerLibraryView.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerDetailsView.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerStorageView.swift`
- Create: `apps/ios/RiotTests/SneakerWebCoreTests.swift`
- Create: `apps/ios/RiotTests/SneakerLibraryViewModelTests.swift`
- Create: `apps/ios/RiotUITests/SneakerDocumentUITests.swift`

- [ ] **Step 1: Write RED document and accessible-library tests**

Tests must drive an actual `.snk` document URL, stream through a
security-scoped handle, release scope on every terminal path, make the Sites
segment available but route an external open into the new Received record immediately after
successful open, show `N added, N updated, N unchanged, N
blocked · X MB stored`, preserve zero mutation on invalid/cancel/quota failure,
offer Undo, and reopen persisted state after process reconstruction.

Drive foreground loss both ways: an OS-granted bounded read may finish, while
suspension cancels staging and returns to `Preparation interrupted` with Retry
and the exact safe document metadata preserved. Verify the four zero-change
messages, the retained Received receipt, non-retryable cryptographic/wrong-
namespace error until bytes change, and `This local copy is damaged` recovery.

UI tests require global profile/sidebar placement rather than a community tab;
Sites/Received/Blocked/Storage empty/loading/failure states; selection count;
hidden default raw IDs; complete copyable Details; no digest/domain IDs in a
Received summary; stable two-word accessibility disambiguators; Dynamic Type,
keyboard/focus restoration, 44-point targets, and non-color status.

- [ ] **Step 2: Register only `.snk` document intake**

Add an imported `UTType` for `.snk` and route `onOpenURL` into
`SneakerDocumentIntake`. Do not claim editor/exported-type ownership. Keep
macOS Info.plist and project settings unchanged.

- [ ] **Step 3: Implement the global library state model**

`SneakerLibraryModel` owns pages, generation restart, selection, open summary,
undo, block/unblock, and storage recovery. It stores no raw SQL and never turns
full byte IDs into shortened display strings. The profile menu and large-screen
sidebar link to the same `SneakerLibraryView`.

- [ ] **Step 4: Verify iOS model/UI and commit**

```sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerWebCoreTests \
  -only-testing:RiotTests/SneakerLibraryViewModelTests
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotUITests/SneakerDocumentUITests
```

```sh
git add apps/ios/Riot/Info.plist apps/ios/Riot/RiotApp.swift apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/ConferenceShellView.swift apps/ios/Riot/SneakerWeb \
  apps/ios/RiotTests/SneakerWebCoreTests.swift \
  apps/ios/RiotTests/SneakerLibraryViewModelTests.swift \
  apps/ios/RiotUITests/SneakerDocumentUITests.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): open and manage SneakerWeb collections"
```

## Task 7: Ship matching Android document opening and library management

**Files:**
- Modify: `apps/android/app/src/main/AndroidManifest.xml`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/RiotController.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerDocumentIntake.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerLibraryViewModel.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerLibraryScreen.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerLibraryViewModelTest.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerDocumentOpenTest.kt`

- [ ] **Step 1: Write RED Android parity tests**

Mirror Task 6's open/summary/undo/persistence, Sites/Received/Details/Blocked/
Storage, raw-ID visibility, selection, stale-page, accessibility, and failure
contracts. Add content-resolver tests proving the core receives bytes and safe
display metadata, never a content URI or arbitrary path.

- [ ] **Step 2: Register bounded VIEW intent handling**

Declare `.snk` VIEW/OPENABLE handling with the narrow MIME/extension contract,
open the `ContentResolver` stream, copy chunks of at most 256 KiB, enforce
expected length when known, and close provider/task handles on success,
failure, cancellation, and Activity destruction.

- [ ] **Step 3: Implement UI parity**

Use the existing Android presentation style rather than adding a new framework.
The global navigation placement, semantics, messages, recovery actions, and
identifier policy must match iOS and the design, while preserving platform
focus and back behavior.

- [ ] **Step 4: Verify and commit**

```sh
cd apps/android
./gradlew testDebugUnitTest --tests '*SneakerLibraryViewModelTest'
./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerDocumentOpenTest
./gradlew lintDebug
```

```sh
git add apps/android/app/src/main/AndroidManifest.xml \
  apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
  apps/android/app/src/main/kotlin/org/riot/evidence/RiotController.kt \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb
git commit -m "feat(android): open and manage SneakerWeb collections"
```

## Task 8: Add the isolated offline reader on both platforms

**Files:**
- Create: `crates/riot-core/src/sneakerweb/viewer.rs`
- Modify: `crates/riot-core/src/sneakerweb/mod.rs`
- Create: `crates/riot-core/tests/sneakerweb_viewer.rs`
- Create: `apps/ios/Riot/SneakerWeb/{SneakerLoopbackServer,SneakerWebViewHost,SneakerReaderView}.swift`
- Create: `apps/ios/RiotTests/SneakerWebViewIsolationTests.swift`
- Modify: `apps/ios/Riot/Info.plist`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/{SneakerLoopbackServer,SneakerWebViewHost,SneakerReaderScreen}.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerWebViewIsolationTest.kt`
- Modify: `apps/android/app/src/main/AndroidManifest.xml`
- Create: `apps/android/app/src/main/res/xml/sneakerweb_network_security_config.xml`

- [ ] **Step 1: Write RED path/server/WebView adversarial tests**

Cover canonical URL/path conversion, `/index.html` fallback, missing home,
linked absent domain, MIME, GET/HEAD only, Host/cookie rejection, request/body/
rate/shape limits, absolute-form requests, no non-loopback bind, port
teardown/rebind, CSP/COOP/CORP/Referrer-Policy/no-store headers, external
navigations with confirmation, forms, popups, permissions, service workers,
cross-site reads, DNS rebinding attempts, process failure, and absence of every
Riot/miniapp bridge.

Parse and assert each CSP directive independently, especially `form-action 'none'`
and `frame-ancestors 'none'`; also require
`X-Content-Type-Options: nosniff`, path-derived Content-Type,
`Cross-Origin-Resource-Policy: same-origin`, `Referrer-Policy: no-referrer`,
`Cache-Control: no-store`, and a Permissions-Policy denying camera,
microphone, geolocation, payment, USB, Bluetooth, sensors, and clipboard.
Packet-level platform tests must prove DNS prefetch, preconnect, speculative
navigation, and renderer-created sockets never leave loopback.

Add reader-state tests for Back, Forward, Library, Details, and Share. They
must preserve the current full domain, canonical path, per-history-entry scroll
position, prior library page/row, and invoking focus across reader, Details,
Share, process-failure Retry, and Back transitions.

Add a corpus and fuzz/property suite for the non-executing `/index.html` title
extractor: read at most 256 KiB, cap nesting/tokenizer work/text output, reject
invalid UTF-8/control/bidi scalars, collapse whitespace, decode entities once,
prefer a valid `<title>`, then bounded visible text, then the native fallback.
No script, CSS, subresource, or `sneakerweb.html` value may become the native
or accessibility title.

- [ ] **Step 2: Implement the native loopback capability boundary**

Bind only to `127.0.0.1`/`::1` on exact port `1312`; if neither loopback family
can bind that port, return `VIEWER_UNAVAILABLE` and do not substitute another
port because canonical SneakerWeb links depend on 1312. Create one 256-bit
host-only cookie per reader and install it before first navigation. Require
exact `sneakerweb.localhost:1312` or a full 64-hex-domain `.localhost:1312` Host,
serve only `ResourceLease` bytes, and stop the listener before closing leases.

- [ ] **Step 3: Harden WebViews and title preview extraction**

iOS uses ephemeral `WKWebsiteDataStore`, app-bound navigation/content rules,
and loopback-only ATS. Android permits only the loopback origin and disables
external resource access, service workers, permissions, file/content access,
and bridge injection. Render `sneakerweb.html` offscreen in a scriptless
sandbox with strict byte/time/node limits as a decorative visual preview only.
Use the bounded `/index.html` extractor for the native and accessibility title.
Implement the reader toolbar/history contract in `SneakerReaderView` and
`SneakerReaderScreen` with the state restoration tested in Step 1.

In `Info.plist`, keep arbitrary loads disabled and add an ATS exception only
for `localhost` including subdomains on insecure loopback HTTP. In Android,
set `android:networkSecurityConfig="@xml/sneakerweb_network_security_config"`;
the XML uses `cleartextTrafficPermitted="false"` as its base and permits only
`localhost` with subdomains. Platform tests inspect these files and prove a
non-loopback cleartext URL remains blocked.

- [ ] **Step 4: Verify and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_viewer
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerWebViewIsolationTests
(cd apps/android && ./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerWebViewIsolationTest)
```

```sh
git add crates/riot-core/src/sneakerweb crates/riot-core/tests/sneakerweb_viewer.rs \
  apps/ios/Riot/SneakerWeb apps/ios/Riot/Info.plist \
  apps/ios/RiotTests/SneakerWebViewIsolationTests.swift apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerWebViewIsolationTest.kt \
  apps/android/app/src/main/AndroidManifest.xml \
  apps/android/app/src/main/res/xml/sneakerweb_network_security_config.xml
git commit -m "feat(sneakerweb): add isolated offline reader"
```

## Task 9: Add accessible selection, preparation, and system file sharing

**Files:**
- Create: `apps/ios/Riot/SneakerWeb/SneakerShareCoordinator.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerShareView.swift`
- Create: `apps/ios/RiotTests/SneakerShareTaskTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerShareCoordinator.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerShareScreen.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerShareCoordinatorTest.kt`

- [ ] **Step 1: Write RED review/preparation/share tests**

Cover one site, several sites, and a Received collection; ordered-unique exact
selection; blocked-site exclusion/cancellation; required normalized title of
1–80 Unicode scalars; optional note of at most 1,024 UTF-8 bytes; public
content/authorship disclosure; sanitized filename; collision suffix delegated
to OS; progress/cancel/interruption/retry; protected temporary cleanup after OS
sheet completion/cancel/expiry/relaunch; and focus restoration.

For Received sharing, test `N blocked sites won't be included`, `Updated since
received`, and disabled Share with a route to Blocked sites when no unblocked
domain remains.

- [ ] **Step 2: Implement one preserved native review model**

The coordinator freezes full domain IDs and block generations in native memory,
shows only safe titles/count/size by default, creates `SnkExportTask`, streams
its lease to a protected backup-excluded temporary file, and presents the OS
sheet only in `Ready to share`. A failed handoff preserves the selection and
review fields; a block generation change returns to selection.

- [ ] **Step 3: Verify and commit**

```sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerShareTaskTests
(cd apps/android && ./gradlew testDebugUnitTest --tests '*SneakerShareCoordinatorTest')
```

```sh
git add apps/ios/Riot/SneakerWeb apps/ios/RiotTests/SneakerShareTaskTests.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerShareCoordinatorTest.kt
git commit -m "feat(mobile): share standard SneakerWeb files"
```

## Task 10: Multiplex direct nearby and portable-blob transfer

**Files:**
- Create: `crates/riot-core/src/sneakerweb/nearby.rs`
- Modify: `crates/riot-core/src/sneakerweb/{mod,tasks,blob}.rs`
- Create: `crates/riot-core/tests/sneakerweb_nearby.rs`
- Modify: `crates/riot-ffi/src/sneakerweb_ffi.rs`
- Modify: `crates/riot-ffi/tests/sneakerweb_contract.rs`
- Modify: `apps/ios/Riot/Transport/{FrameCodec,SyncCoordinator,NearbyTransportController}.swift`
- Create: `apps/ios/Riot/Transport/RiotNearbyEnvelopeV2.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerNearbyChannel.swift`
- Modify: `apps/ios/Riot/SneakerWeb/{SneakerShareCoordinator,SneakerShareView}.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Create: `apps/ios/RiotTests/SneakerNearbyTransferTests.swift`
- Create: `apps/ios/RiotUITests/SneakerNearbyFlowUITests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/transport/{BleFrameCodec,SyncCoordinator,AndroidNearbyController}.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/transport/RiotNearbyEnvelopeV2.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerNearbyChannel.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/{SneakerShareCoordinator,SneakerShareScreen}.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerNearbyTransferTest.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerNearbyFlowTest.kt`

- [ ] **Step 1: Write RED multiplexer and lifecycle tests**

Require negotiation that preserves V1 ordinary sync, bounded `space_sync` and
`portable_blob` channels, one receive owner, random 256-bit one-shot direct
capability, authenticated sender/title/site-count/size preview, Accept/Reject,
256 KiB frames, at most two unacknowledged chunks, 32 MiB verified checkpoints,
pause/resume only in the same confirmed session, zero restart after integrity
failure, verified receiver ack before sender `Sent`, peer loss/retry with new
confirmation, no note/raw IDs/digests/community metadata, and no inventory
enumeration.

- [ ] **Step 2: Implement separate send and receive task outcomes**

Expose and test:

```text
DirectSnkSendTask -> SendReceipt only after verified ack
DirectSnkReceiveTask -> PortableBlobLease
```

Receiving never mutates `SneakerCollection`; native must explicitly pass the
lease to `begin_snk_open_from_blob`. Local TCP is preferred; BLE is an honest
resumable public-content fallback, not confidentiality.

Extend `sneakerweb_ffi.rs` only now with factories `begin_direct_snk_send`,
`accept_direct_snk_receive`, and
`reject_direct_snk_receive`, plus these generated methods:

```text
DirectSnkSendTask: progress, finish, pause, resume, cancel
DirectSnkReceiveTask: progress, finish, pause, resume, cancel
```

Keep the verified-carrier `request_space_blob` factory and
`SpaceBlobReceiveTask` out of FFI until Task 11 creates carrier validation.

- [ ] **Step 3: Implement the native nearby destination and recipient journey**

Extend both Task 9 share coordinators/views and their shell owners. Sender UX
must cover Preparing, Waiting for confirmation, Connecting, Transferring,
Pause/Resume/Cancel, Waiting for verified ack, Sent, Interrupted, and Retry.
Recipient UX must present confirmed sender handle, bounded authenticated title,
site count, encoded size, and `Public SneakerWeb collection` before Accept or
Reject, then Connecting, Transferring, Verifying, Received, and Open. It must
not show the note, digest, raw IDs, or community relationships. Peer loss,
session expiry, integrity failure, rejection, and cancellation restore the
preserved review/focus exactly as specified.

- [ ] **Step 4: Verify and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_nearby
scripts/conference/build-native-core.sh
scripts/conference/test-native-core-package.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerNearbyTransferTests
(cd apps/android && ./gradlew testDebugUnitTest --tests '*SneakerNearbyTransferTest')
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotUITests/SneakerNearbyFlowUITests
(cd apps/android && ./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerNearbyFlowTest)
```

```sh
git add crates/riot-core/src/sneakerweb crates/riot-core/tests/sneakerweb_nearby.rs \
  crates/riot-ffi/src/sneakerweb_ffi.rs crates/riot-ffi/tests/sneakerweb_contract.rs \
  apps/ios/Riot/Transport apps/ios/Riot/SneakerWeb/SneakerNearbyChannel.swift \
  apps/ios/Riot/SneakerWeb/SneakerShareCoordinator.swift \
  apps/ios/Riot/SneakerWeb/SneakerShareView.swift apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/ConferenceShellView.swift apps/ios/RiotTests/SneakerNearbyTransferTests.swift \
  apps/ios/RiotUITests/SneakerNearbyFlowUITests.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/android/app/src/main/kotlin/org/riot/evidence/transport \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerNearbyChannel.kt \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerShareCoordinator.kt \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerShareScreen.kt \
  apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerNearbyTransferTest.kt \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerNearbyFlowTest.kt
git commit -m "feat(nearby): carry SneakerWeb collections directly"
```

## Task 11: Add carrier records, manifest v2 capability, and app-bound host

**Files:**
- Create: `crates/riot-core/src/sneakerweb/carrier.rs`
- Create: `crates/riot-core/src/sneakerweb/social.rs`
- Modify: `crates/riot-core/src/apps/{manifest,bridge,index,directory,starter}.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-core/src/demo_fixture.rs`
- Modify: `crates/riot-core/examples/{pack_starter,pack_checklist}.rs`
- Modify: `crates/riot-app-cli/src/lib.rs`
- Modify: `crates/riot-app-cli/tests/cli_pack.rs`
- Modify: `crates/riot-core/tests/{apps_codec_hostile,apps_directory,apps_index_io,apps_manifest,apps_starter,core_import_app_index_entries}.rs`
- Modify: `crates/riot-ffi/tests/apps_contract.rs`
- Modify: `crates/riot-core/src/sneakerweb/{mod,tasks}.rs`
- Create: `crates/riot-core/tests/sneakerweb_carrier.rs`
- Create: `crates/riot-core/tests/apps_manifest_v2.rs`
- Create: `crates/riot-core/tests/sneakerweb_social_host.rs`
- Modify: `crates/riot-ffi/src/{apps_ffi,sneakerweb_ffi}.rs`
- Modify: `crates/riot-ffi/tests/{apps_contract,sneakerweb_contract}.rs`
- Modify: `apps/ios/Riot/Apps/{AppBridgeController,AppBundleCodec,AppReviewSheet,AppRuntimeView,RiotJS}.swift`
- Modify: `apps/ios/RiotTests/{AppRepositoryTests,AppRuntimeHostTests}.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerCarrierCardView.swift`
- Create: `apps/ios/RiotTests/SneakerCarrierCardTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/apps/{AppBundleCodec,CanonicalCbor,RiotJsBridge,RiotJsShim,AppWebViewHost}.kt`
- Modify: `apps/android/app/src/test/kotlin/org/riot/evidence/apps/{AppBundleCodecTest,RiotJsBridgeTest}.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerCarrierCard.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerCarrierCardTest.kt`

- [ ] **Step 1: Write RED canonical carrier/binding/manifest tests**

Require a closed carrier payload with digest, exact length, ordered unique full
domain set, one bounded safe label aligned to every domain, exact site count,
required title, optional note, schema, and no private metadata. Reject missing,
extra, misordered, or count-mismatched labels/domains.
Require `AppPublicCollectionBindingV1` at exact path
`objects/app-public-collections/<64-lowercase-hex-app-id>/<32-lowercase-hex-listing-id>`
with space, app, listing ID, full carrier entry ID, signer, and created time.
Remote verification must work from signed entries alone.

Manifest tests require v1 byte compatibility and no capability; v2 canonical
key `9` present even empty; definite sorted duplicate-free closed tokens; exact
`portable_public_collections`; unknown-token rejection; and
`riot/app-id/v2` separation. Generic app writes to the reserved binding family
must fail before signing.

Update every ordinary manifest consumer, not only the codec: app-index pair
validation, directory projection, starter catalog, mobile installation/state,
and FFI installed/directory records must carry the version and closed machine
capabilities without confusing them with author-supplied permission copy.
The strict `riot-app.json` packer accepts existing v1 documents unchanged and
requires both `"manifest_version": 2` and sorted unique
`"machine_capabilities"` for v2; unknown keys/tokens remain rejecting. Both
native manifest decoders and their review models display host-owned capability
copy rather than bundle-supplied wording.

- [ ] **Step 2: Implement canonical carrier, binding, and manifest v2 codecs**

Encode and decode the exact deterministic-CBOR carrier and binding maps from
the approved design, including exact path shape, same-space/same-signer checks,
public-space enforcement, strict safe text, ordered domain/label alignment,
and unknown/malformed rejection. Extend `AppManifest` as a versioned closed
contract while keeping v1 bytes and app IDs unchanged; propagate version and
machine capabilities through app index, directory, starter, mobile state, and
FFI. Keep organizer review copy host-owned.

- [ ] **Step 3: Implement the closed app host surface**

Register only when a currently trusted v2 app in the active public space holds
the exact capability:

```js
riot.collections.list(cursor)
riot.collections.watch(generation, callback)
riot.collections.pick()
riot.collections.open(listingHandle)
```

The app sees only opaque handles, safe title/note, count, encoded size,
availability, and carrier display attribution. It receives no bytes, raw
domain IDs, digest, global-library enumeration, filesystem/database handle, or
low-level task/selection/commit method.

- [ ] **Step 4: Implement the native-only picker event model**

Use a queued-event model with a single native consumer. The generated FFI
surface is:

```text
AppCollectionPickerTask.take_native_event
AppCollectionPickerTask.submit_native_selection
AppCollectionPickerTask.submit_native_review
AppCollectionPickerTask.progress
AppCollectionPickerTask.finish
AppCollectionPickerTask.retry
AppCollectionPickerTask.cancel
```

None is registered in the JS dispatcher. `take_native_event` yields
`PresentPicker { max_sites: 1024 }` once, then `NO_EVENT` until a transition.
Rust generates a 16-byte listing ID and 16-byte idempotency key after accepted
review, retrying a CSPRNG collision at most eight times before a typed
non-mutating failure. Identical duplicate submissions return the accepted
result; different duplicates fail without mutation.

Carrier creation independently generates 16-byte share and revision IDs with
the same eight-attempt collision bound. Native multi-community orchestration
generates one 16-byte child idempotency key per selected space with the same
bound. Tests inject seven collisions followed by success and eight collisions
followed by a typed non-mutating failure for listing, share, revision, and
idempotency IDs.

- [ ] **Step 5: Implement per-space atomic share and carrier-scoped receive**

Implement `SpaceSneakerShareTask` as `Preparing -> SigningCarrier ->
SigningDestinations -> Committing -> Shared(CarrierReceipt)` with cancellation,
owner close, repeated terminal result, preserved retryable lease, and exact
`(space, signer, idempotency_key)` replay. One public-space transaction commits
the carrier plus one app binding in this task; different request
facts under the same key return `IDEMPOTENCY_CONFLICT`. Implement
`SpaceBlobReceiveTask` only through a request scoped to a verified same-space
carrier. Add deterministic barriers for cancel/finish, owner-close/commit,
block/commit, and trust-revocation/commit.

- [ ] **Step 6: Add the sharing FFI only after core carrier/social tasks exist**

Extend `sneakerweb_ffi.rs` with `begin_space_sneaker_share`,
`request_space_blob`,
`begin_app_collection_pick`, `list_app_collection_bindings`,
`watch_app_collection_bindings`, and `open_app_collection_binding`. Generated
opaque methods are:

```text
SpaceSneakerShareTask: progress, finish, cancel
SpaceBlobReceiveTask: progress, finish, pause, resume, cancel
AppCollectionPickerTask: take_native_event, submit_native_selection,
  submit_native_review, progress, finish, retry, cancel
```

No native-only picker method or carrier/blob resolver is registered in the
JavaScript dispatcher.

The Task 11 `destinations` DTO is closed to one
`AppPublicCollectionBinding { trusted_app_id, listing_id }`. It contains no
Newswire placeholder or open string tag. Task 12 adds the Newswire union member
only after the approved attachment contract exists, then regenerates bindings.

`request_space_blob` verifies an active public `SpaceSession`, the full
same-space carrier entry, digest, length, and current authority before creating
`SpaceBlobReceiveTask -> PortableBlobLease`; receiving still does not mutate
the collection.

- [ ] **Step 7: Implement and verify the shared native carrier card**

Both native app-runtime integration owners mount the same host card for a
Newswire attachment or Directory listing. Tests cover `Available locally / Open`,
`Not downloaded / Get collection`, `Waiting for nearby holder / Retry`,
transferring progress/cancel, interrupted/resume, verifying, invalid/remove
reference locally, storage full/manage storage, all domains blocked/view
blocked sites, and ready/open. Rendering a synced reference never downloads;
only the explicit `Get collection` gesture calls `request_space_blob`, and Open
passes the resulting lease to `begin_snk_open_from_blob`.

Derive `listing_handle` as unpadded base64url of the full SHA-256 of the
canonical binding entry and resolve it only inside the active app/space/session.
List/watch stale generation replaces the safe snapshot. Open requires a
single-use genuine gesture token and first presents this native card.

- [ ] **Step 8: Test revocation and final commit linearization**

Before commit, trust/capability/session/profile loss cancels the child, writes
nothing, sends no app callback, and routes native UI to Tools. The atomic
carrier/binding commit wins over later revocation. After commit, native reports
shared, sends nothing into a revoked app, and routes to Tools; retrust later
projects the signed binding.

- [ ] **Step 9: Verify and commit**

```sh
cargo test -p riot-core --features conformance --test apps_manifest_v2
cargo test -p riot-core --features conformance --test sneakerweb_carrier
cargo test -p riot-core --features conformance --test sneakerweb_social_host
cargo test -p riot-ffi --test sneakerweb_contract
scripts/conference/build-native-core.sh
scripts/conference/test-native-core-package.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerCarrierCardTests
(cd apps/android && ./gradlew testDebugUnitTest --tests '*SneakerCarrierCardTest')
```

```sh
git add crates/riot-core/src/apps crates/riot-core/src/sneakerweb \
  crates/riot-core/src/demo_fixture.rs crates/riot-core/examples \
  crates/riot-core/tests/apps_manifest_v2.rs crates/riot-core/tests/sneakerweb_carrier.rs \
  crates/riot-core/tests/sneakerweb_social_host.rs crates/riot-core/tests/apps_*.rs \
  crates/riot-core/tests/core_import_app_index_entries.rs \
  crates/riot-app-cli/src/lib.rs crates/riot-app-cli/tests/cli_pack.rs \
  crates/riot-ffi/src crates/riot-ffi/tests \
  apps/ios/Riot/Apps apps/ios/RiotTests/AppRepositoryTests.swift \
  apps/ios/RiotTests/AppRuntimeHostTests.swift apps/ios/Riot/SneakerWeb/SneakerCarrierCardView.swift \
  apps/ios/RiotTests/SneakerCarrierCardTests.swift apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/android/app/src/main/kotlin/org/riot/evidence/apps \
  apps/android/app/src/test/kotlin/org/riot/evidence/apps \
  apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerCarrierCard.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerCarrierCardTest.kt
git commit -m "feat(apps): host public SneakerWeb collection bindings"
```

## Task 12: Ship Sneaker Directory and multi-community destination orchestration

**Files:**
- Create: `fixtures/apps/sneaker-directory/{riot-app.json,index.html,tokens.css,style.css,app.js}`
- Create: packed Sneaker Directory manifest/bundle artifacts beside other starters
- Modify: `crates/riot-core/src/apps/starter.rs`
- Modify: `crates/riot-core/tests/apps_starter.rs`
- Modify: `scripts/apps/{miniapp-contracts.mjs,miniapp-browser.spec.mjs}`
- Create: `scripts/apps/package.json`
- Create: `scripts/apps/package-lock.json`
- Modify: `apps/ios/Riot/SneakerWeb/{SneakerShareCoordinator,SneakerShareView}.swift`
- Modify: `apps/ios/Riot/{AppModel,ConferenceShellView}.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/{SneakerShareCoordinator,SneakerShareScreen}.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Create: `apps/ios/RiotTests/SneakerDirectoryPermissionTests.swift`
- Create: `apps/ios/RiotUITests/SneakerDirectoryPermissionUITests.swift`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerDirectoryPermissionTest.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerCommunityShareTest.kt`
- Modify when the attachment dependency is present:
  `crates/riot-core/src/newswire/{model,path,entry,projection,store}.rs`
- Modify when the attachment dependency is present:
  `crates/riot-core/tests/{newswire_codec,newswire_entry,newswire_import,newswire_projection,newswire_end_to_end}.rs`

- [ ] **Step 1: Write RED ordinary-miniapp tests**

Require the Directory to pack, derive a normal v2 app ID, appear through the
ordinary starter/app-index flow, require independent trust per community, use
only `riot.collections.list/watch/pick/open`, render chronological host-verified
listings, replace its snapshot on stale cursor/generation, and fail to access
raw bytes/IDs/digests/global library/cross-app/cross-space handles. No privileged
built-in switch is permitted.

On iOS, require both packed Directory artifacts to have explicit file
references and `PBXResourcesBuildPhase` membership, and require every new
Swift/XCTest/XCUITest file from Tasks 8–12 to appear in the correct explicit
source/test phase. On Android, the existing fixtures asset source directory
must contain both packed artifacts at instrumentation/runtime.

Pin browser tooling locally before running RED:

```json
{
  "private": true,
  "devDependencies": { "@playwright/test": "1.61.1" }
}
```

Generate and commit `scripts/apps/package-lock.json` with `npm install
--package-lock-only --prefix scripts/apps`; every later browser command begins
with `npm ci --prefix scripts/apps` and uses `npm --prefix scripts/apps exec`.

- [ ] **Step 2: Build the Directory source and packed artifacts**

The source uses framework-free HTML/CSS/JS and the existing shared token/accessibility
contracts. It can initiate native picker, render safe summaries, and open a
native card. Categories/comments/ratings/hide/delete are absent from this
slice. Its `riot-app.json` declares `manifest_version: 2` and the sole machine
capability `portable_public_collections`, separate from human-readable
permissions. Repack through the updated ordinary starter/app CLI and update
drift hashes.

- [ ] **Step 3: Implement multi-community native orchestration**

The initial destination picker lists writable public spaces with an enabled
Directory only. Generate one 128-bit idempotency key per space. Each
child atomically commits its carrier and Directory binding, reporting
`Shared`, `Needs retry`, or `No longer allowed`. Retry reuses the same key and
cannot duplicate. One community failure never rolls back another.

An organizer enabling Directory returns to the preserved share selection and
destination row; a member sees `Ask an organizer to turn on Sneaker Directory.`

- [ ] **Step 4: Integrate Newswire only against its landed attachment union**

When `PublicAttachmentRefV1` exists, extend it with the approved same-space
carrier reference and construct an ordinary `NewsPostV1` whose editable
headline defaults to collection title. Verify same-space and same-signer
resolution. Editorial actions affect the post projection, never site entries
or an independent Directory listing. If the attachment union is still absent,
leave this substep and its Newswire-specific tests unexecuted and do not claim
the Newswire route in release evidence.

Only in this dependency-present branch, extend the closed per-space destination
union with `NewswireAttachment { headline, body }`, allow at most one Newswire
and one app binding in the same atomic carrier transaction, extend the UniFFI
DTO, regenerate bindings, and rerun carrier/social/Newswire cross-space and
same-signer tests. Extend the native destination picker with independent
Newswire and Directory choices; selecting both in one community reuses one
carrier. Screen-reader tests distinguish both controls and their independent
selected states. No Newswire type is referenced by Tasks 1–11.

- [ ] **Step 5: Verify and commit the available destination set**

```sh
node scripts/apps/miniapp-contracts.mjs
npm ci --prefix scripts/apps
npm --prefix scripts/apps exec -- playwright test \
  --config playwright.config.mjs --grep 'sneaker-directory'
cargo test -p riot-core --all-features
scripts/conference/build-native-core.sh
scripts/conference/test-native-core-package.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotUITests/SneakerDirectoryPermissionUITests
(cd apps/android && ./gradlew testDebugUnitTest --tests '*SneakerDirectoryPermissionTest' \
  connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerCommunityShareTest)
```

```sh
git add fixtures/apps/sneaker-directory fixtures/apps/sneaker-directory.bundle.cbor \
  fixtures/apps/sneaker-directory.manifest.cbor crates/riot-core/src/apps/starter.rs \
  crates/riot-core/tests/apps_starter.rs scripts/apps \
  apps/ios/Riot/SneakerWeb apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/ConferenceShellView.swift apps/ios/RiotTests/SneakerDirectoryPermissionTests.swift \
  apps/ios/RiotUITests/SneakerDirectoryPermissionUITests.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb \
  apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerDirectoryPermissionTest.kt \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerCommunityShareTest.kt
if test -d crates/riot-core/src/newswire; then
  git add crates/riot-core/src/newswire crates/riot-core/tests/newswire_*.rs
fi
git commit -m "feat(sneakerweb): share collections across public communities"
```

The conditional stages Newswire paths only when Step 4 ran against the landed
module. Otherwise state Directory-only community support in the work-unit
evidence; the final release remains blocked from claiming all design
destinations until Newswire lands.

## Task 13: Enforce coverage, platform freshness, interoperability, and field evidence

**Files:**
- Modify: `.coverage-thresholds.json`
- Create: `scripts/coverage-gate.sh`
- Create: `scripts/verify-xcresult-tests.sh`
- Create: `scripts/android-sneakerweb-test-gate.sh`
- Create: `scripts/sneakerweb-physical-rehearsal.sh`
- Modify: `docs/product/product-brief.md`
- Modify: `README.md`
- Modify: `SERVICE-INVENTORY.md`
- Create: `docs/quality/2026-07-13-sneakerweb-release-evidence.md`

- [ ] **Step 1: Write RED gate self-tests**

Test that the coverage wrapper fails when tarpaulin or llvm-cov is absent,
maps statements to LLVM regions only when configured thresholds agree, and
fails any dimension below `.coverage-thresholds.json`. Test that iOS and
Android gate parsers delete old outputs, record run start, reject stale,
missing, malformed, zero-test, and skipped-only results, and require every
named SneakerWeb suite.

The iOS core result must positively execute `SneakerWebCoreTests`,
`SneakerLibraryViewModelTests`, `SneakerWebViewIsolationTests`,
`SneakerShareTaskTests`, `SneakerNearbyTransferTests`,
`SneakerCarrierCardTests`, and `SneakerDirectoryPermissionTests`; the UI result
must execute `SneakerDocumentUITests`, `SneakerNearbyFlowUITests`, and
`SneakerDirectoryPermissionUITests`. Android unit results must execute
`SneakerLibraryViewModelTest`, `SneakerShareCoordinatorTest`,
`SneakerNearbyTransferTest`, `SneakerCarrierCardTest`, and
`SneakerDirectoryPermissionTest`; connected results must execute
`SneakerDocumentOpenTest`, `SneakerWebViewIsolationTest`, and
`SneakerNearbyFlowTest`, and `SneakerCommunityShareTest`.

- [ ] **Step 2: Version the combined coverage command**

Change only the source-of-truth enforcement command:

```json
{
  "enforcement": {
    "command": "scripts/coverage-gate.sh",
    "blockPRCreation": true,
    "blockTaskCompletion": true
  }
}
```

The script reads all four 100% thresholds, runs tarpaulin for its supported
metrics and llvm-cov for lines/functions/regions/branches, and reports the
statements-to-regions mapping explicitly.

- [ ] **Step 3: Run the full blocking automated matrix**

```sh
cargo xtask validate-contracts
cargo test --workspace --all-features
scripts/coverage-gate.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
scripts/conference/build-native-core.sh
scripts/conference/test-native-core-package.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -enableCodeCoverage YES -resultBundlePath build/snk-riotkit.xcresult
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -enableCodeCoverage YES -resultBundlePath build/snk-riot.xcresult
scripts/verify-xcresult-tests.sh build/snk-riotkit.xcresult --require-tests \
  --require-suite SneakerWebCoreTests \
  --require-suite SneakerLibraryViewModelTests \
  --require-suite SneakerWebViewIsolationTests \
  --require-suite SneakerShareTaskTests \
  --require-suite SneakerNearbyTransferTests \
  --require-suite SneakerCarrierCardTests \
  --require-suite SneakerDirectoryPermissionTests
scripts/verify-xcresult-tests.sh build/snk-riot.xcresult --require-tests \
  --require-suite SneakerDocumentUITests \
  --require-suite SneakerNearbyFlowUITests \
  --require-suite SneakerDirectoryPermissionUITests
(cd apps/android && ../../scripts/android-sneakerweb-test-gate.sh)
scripts/sneakerweb-oracle.sh verify --offline --version 1.0.1
scripts/sneakerweb-interop.sh --offline --version 1.0.1 \
  --cli build/tools/sneakerweb/bin/sneakerweb
```

Expected: every command passes with fresh positive test counts and 100% of
every configured coverage dimension.

- [ ] **Step 4: Record physical/performance/cohort evidence**

Run release builds on iPhone 17 Pro/iOS 26.2 and Pixel 9-class/API 36. Record
ten cold and ten warm runs of fixed 10 MiB and 100 MiB fixtures, wall time,
peak RSS, encoded/retained bytes, transport, completion, and final digests.
The 100 MiB open must be at most 10 seconds and 512 MiB peak RSS; a 1,000-site
library must become interactive within 300 ms after database open.

At least ten non-builders run airplane-mode receive → open → named page → share
→ second-device open. Require 9/10 uncoached within two minutes for 10 MiB,
8/10 within five minutes for 100 MiB, and 19/20 intact deliveries split five
each across file, nearby, Newswire, and Directory, including a two-community
partial-failure/retry exercise. If Newswire is not landed, this evidence cannot
pass and the full feature release remains blocked.

- [ ] **Step 5: Update delivered docs and commit evidence**

Document the narrow user-opened fixed-public-namespace exception to
preview-before-ingest, `Content is intact` versus trust, public sharing,
inspectable full IDs, block/removal behavior, and only the routes that passed.
Update the service inventory with the new core modules, FFI task objects,
native hosts, and Directory app.

```sh
git add .coverage-thresholds.json scripts/coverage-gate.sh \
  scripts/verify-xcresult-tests.sh scripts/android-sneakerweb-test-gate.sh \
  scripts/sneakerweb-physical-rehearsal.sh docs/product/product-brief.md README.md \
  SERVICE-INVENTORY.md docs/quality/2026-07-13-sneakerweb-release-evidence.md
git diff --cached --check
git commit -m "test(sneakerweb): record mobile release evidence"
```

## Requirement-to-task traceability

| Approved requirement | Tasks |
| --- | --- |
| Official `.snk` decode/encode without publishing | 1, 2, 4, 13 |
| Transparent atomic open and durable global library | 3, 5, 6, 7 |
| Browse offline with safe fallback and isolated renderer | 8 |
| IDs/keys hidden by default, complete in Details | 3, 6, 7 |
| Sites, Received, blocked sites, storage, undo/removal | 3, 6, 7 |
| Select/share one, many, or a Received collection | 4, 9 |
| Standard system file sharing | 9 |
| Direct nearby sharing without a shared community | 10 |
| On-demand public-space blobs and carrier attribution | 4, 10, 11 |
| Ordinary signed Sneaker Directory miniapp | 11, 12 |
| Newswire attachment using ordinary editorial semantics | 12, 13 dependency gate |
| Multiple public communities and duplicate-free retry | 11, 12 |
| App cannot access bytes/raw IDs/global library/cross-space data | 11, 12 |
| Block revokes reads/exports/transfers | 3, 4, 8, 9, 10 |
| iOS and Android parity; macOS excluded | 6–10, 12, 13 |
| Accessibility, performance, cohort, physical-device evidence | 6–10, 12, 13 |

## Rollback and compatibility

- Manifest v1 decode and `riot/app-id/v1` remain byte-identical; removing v2
  capability dispatch leaves legacy apps unchanged.
- SneakerWeb rows live in a fixed reserved collection and never become Riot
  spaces. Rolling back native UI leaves verified rows inert; schema downgrade
  must refuse rather than delete them.
- Carrier and binding records are signed public history. Rollback removes local
  projections/handlers, not already published records.
- Directory app trust can be revoked independently per community. Revocation
  makes bindings inert to that app but does not erase signed public records.
- No release advertises Newswire sharing until its separate dependency and the
  Task 13 delivery cohort both pass.

## Final completion rule

Do not mark this plan complete, create a PR, or claim SneakerWeb support until
Task 13 passes every automated and physical gate against the exact landed
dependency graph. If file/open/direct/Directory increments are merged earlier,
name their delivered subset precisely and keep the remaining tasks open.

## Plan review gate record

| Iteration | Feasibility | Completeness | Scope & Alignment |
| --- | --- | --- | --- |
| 1 | FAIL | FAIL | FAIL |
| 2 | FAIL | FAIL | PASS |
| 3 | FAIL | FAIL | PASS |

Round 3 left these blocking issues:

1. Task 1 cross-builds the four mobile targets but does not execute the official
   decode/encode fixture inside each iOS/Android native runtime before schema/UI
   work.
2. Tests do not explicitly cover the full three source forms (one site,
   multi-site, Received) by four destinations (file, nearby, Newswire,
   Directory) matrix.
3. Task 11 omits `crates/riot-core/src/import/bundle.rs` and
   `crates/riot-core/src/session.rs`, the current closed admission/path-binding
   owners that must recognize carrier and app-binding records.
4. Task 11 omits `apps/ios/Riot/Core/ProfileRepository.swift` and
   `apps/ios/Riot/Directory/DirectoryModel.swift`, so manifest-v2 machine
   capabilities cannot reach the host-owned review/runtime model.
5. Task 9 creates system-share views/coordinators but does not modify
   `AppModel.swift`, `ConferenceShellView.swift`, or `MainActivity.kt` to make
   that destination reachable until Task 10.
6. The Task 12 Playwright command must use the repository-relative
   `scripts/apps/playwright.config.mjs`; npm `--prefix` does not change the
   process working directory.

Recommended decision: **Revise**. These are bounded plan corrections, not
design disputes or reasons to reduce the approved product scope. A revision
requires a new human-authorized gate cycle because the automatic three-round
limit has been reached.
