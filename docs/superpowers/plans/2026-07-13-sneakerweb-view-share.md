# SneakerWeb View-and-Share Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Riot on iOS and Android open, retain, browse, inspect, block, export, and share standard SneakerWeb `.snk` collections without creating or re-signing SneakerWeb sites.

**Architecture:** A fixed-namespace `riot_core::sneakerweb` subsystem uses Willow Drop Format only at a bounded staging boundary, then atomically joins verified entries into the production SQLite database. Native iOS and Android clients own document access, isolated WebViews, system sharing, nearby transport, and accessible UI; Rust owns verification, persistence, selection, encoding, leases, carrier records, app bindings, and idempotent space commits. Community sharing uses the existing signed-app runtime and the separately landed Newswire contract; neither miniapp JavaScript nor site JavaScript receives bytes, raw IDs, database handles, or signing authority.

**Tech Stack:** Rust 2021, `willow25 0.6.0-alpha.3` Drop Format, `ufotofu 0.12.4`, SQLite/`rusqlite`, UniFFI 0.32, Swift 6/SwiftUI/WebKit, Kotlin 2.2/Android WebView, canonical CBOR, XCTest/XCUITest, JUnit/instrumentation tests, Playwright for the starter miniapp, cargo-tarpaulin, cargo-llvm-cov.

**Plan status:** **ESCALATION REQUIRED; 3/3 FRESH REVIEW ITERATIONS EXHAUSTED.**
Implementation remains blocked pending Rabble's explicit choice to override,
manually revise, simplify, or cancel after the final gate findings below.

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

Two prerequisite workstreams are part of this delivery program but retain
their own reviewed contracts:

- **P0-A — multi-space persistence:** first run the mandatory plan-review gate
  on `docs/superpowers/plans/2026-07-12-multi-space-sqlite-store.md`, incorporate
  the existing `codex/sqlite-foundation` Tasks 1–2 commits by ordinary
  cherry-pick into its isolated branch, independently validate/review them,
  then complete Tasks 3–11 through their
  own TDD/review/coverage/release gates. It must land `RiotDatabase`,
  `DatabaseSession`, `SpaceSession`, pinned `rusqlite`, and the canonical
  migration owner before SneakerWeb Task 1. SneakerWeb must not create a second
  production store.
- **P0-B — Newswire core:** execute the already approved
  `docs/superpowers/plans/2026-07-13-newswire-core-slice-1.md` through its release
  gate before Task 12. Task 12 then adds the design-approved
  `PublicAttachmentRefV1` carrier member and its Newswire integration; it no
  longer waits on an unspecified future attachment plan. File, nearby, and
  Directory slices can be reviewed/landed earlier, but Task 13 cannot pass
  until P0-B and all three Newswire matrix cells are green.

Before P0-A, add `.coverage-thresholds.json` and `SERVICE-INVENTORY.md` as one
reviewed infrastructure commit on the isolated integration branch, using the
exact current workspace contents after inspecting them for secrets and
unrelated changes. Copy no other dirty or untracked file. This makes the
coverage source of truth and service inventory available to every prerequisite
and SneakerWeb work unit without absorbing the dirty main checkout.

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
- Ordinary logs contain only internal correlation IDs, routes, lengths, and
  stable error codes—never payload text or full protocol identifiers. A full
  public SneakerWeb identifier may appear only in an explicitly confirmed,
  expiring diagnostic export that excludes private-space identifiers and raw
  OS paths/URIs and uses the protected temporary-file lifecycle.
- A carrier or binding is a Riot recommendation record, not an authorship
  claim. Carrier attribution comes only from the verified outer Willow entry.
- A synced carrier card never downloads automatically. `Get collection` is a
  separate native action.
- macOS remains product-excluded, but its project compiles shared iOS RiotKit
  sources by reference. Every task that changes `AppModel.swift`,
  `ConferenceShellView.swift`, `Transport/`, or `Apps/` must keep new
  SneakerWeb references inside `#if os(iOS)`/platform-neutral boundaries and
  run both `RiotKit-macOS` tests and a `Riot-macOS` build before commit. No
  `.snk` declaration, library route, card, or viewer is added to macOS.
- `.coverage-thresholds.json` is the only threshold source. Every work unit
  follows RED, confirms the expected failure, implements the minimum GREEN,
  runs focused and regression checks, runs `scripts/coverage-gate.sh`, passes
  adversarial review, then commits. A Task 1–13 commit is forbidden when that
  work unit's fresh combined coverage run is missing or below any configured
  100% line, branch, function, statement/region threshold.
- Every asynchronous task added by this plan joins the shared lifecycle harness:
  final native-handle drop while nonterminal is an idempotent cancel, sets the
  worker token immediately, releases reservations/temporary leases, cannot run
  detached, and is covered during validation, encoding, transfer, commit, and
  verification. Task-specific work cannot rely only on explicit `cancel()`.

### Mandatory source-by-destination matrix

The release contract is the Cartesian product below, not one representative
happy path per destination. `one site`, `multiple sites`, and `Received` mean
three distinct selection constructors whose frozen full domain sets are
asserted at the destination boundary. Every cell must prove preparation,
handoff/commit, receiver or projection resolution, exact exported domain set,
blocked-domain exclusion, cancellation, and retry/idempotency where the
destination supports it.

| Source form | System `.snk` file | Direct nearby | Newswire | Sneaker Directory |
| --- | --- | --- | --- | --- |
| One site | Task 9 native tests | Task 10 native tests | Task 12 native/core tests | Task 12 native/core tests |
| Multiple sites | Task 9 native tests | Task 10 native tests | Task 12 native/core tests | Task 12 native/core tests |
| Received collection | Task 9 native tests | Task 10 native tests | Task 12 native/core tests | Task 12 native/core tests |

The automated matrix is named `sneakerweb_share_matrix` in Rust,
`SneakerShareMatrixTests` on iOS, and `SneakerShareMatrixTest` on Android.
Task 13 requires fresh positive execution of all three suites and the physical
rehearsal records all 12 cells. P0-B makes the Newswire owner available before
Task 12; no matrix cell may remain dependency-gated at release.

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
  `scripts/test-coverage-gate.sh`,
  `scripts/sneakerweb-oracle.sh`,
  `scripts/verify-xcresult-tests.sh`, `scripts/android-sneakerweb-test-gate.sh`,
  `scripts/sneakerweb-physical-rehearsal.sh`, and their `scripts/test-*.sh`
  gate self-tests.

### Rust core and UniFFI

- Create `crates/riot-core/src/sneakerweb/{mod,protocol,codec,collection,blob,diagnostics,viewer,nearby,carrier,social,tasks}.rs`.
- Modify the landed database migration/schema owner from P0-A. The current
  foundation path is `crates/riot-core/src/store/{database,schema}.rs`; Task 0
  must confirm the landed owner and re-gate this plan if that path changes.
- Modify `crates/riot-core/src/{lib,apps/manifest,apps/bridge,apps/starter}.rs`.
- Modify the P0-B Newswire model/path/entry/projection/store owners in Task 12
  to add the attachment contract frozen by the approved SneakerWeb design.
- Create focused tests under `crates/riot-core/tests/sneakerweb_*.rs`.
- Create `crates/riot-ffi/src/sneakerweb_ffi.rs`, export it from
  `crates/riot-ffi/src/lib.rs`, and create
  `crates/riot-ffi/tests/sneakerweb_contract.rs`.
- Modify `crates/riot-core/src/import/bundle.rs` and
  `crates/riot-core/src/session.rs` in Task 11 so the existing closed admission
  and path-binding owners recognize carrier/binding records without fallback.

### iOS

- Modify `apps/ios/Riot/Info.plist`, `apps/ios/Riot/RiotApp.swift`,
  `apps/ios/Riot/AppModel.swift`, `apps/ios/Riot/ConferenceShellView.swift`,
  `apps/ios/Riot/Core/ProfileRepository.swift`,
  `apps/ios/Riot/Directory/DirectoryModel.swift`,
  `apps/ios/Riot/Apps/{AppBridgeController,AppReviewSheet,AppRuntimeView,RiotJS}.swift`, and
  the Xcode project only through Xcode-aware project editing.
- Create `apps/ios/Riot/SneakerWeb/` files for document intake, library model,
  library/details/storage/share views, loopback server, isolated viewer, share
  coordinator, and nearby channel.
- Create focused XCTest/XCUITest files named in Tasks 6, 8, 10, and 12.
- Create a Task 2 native-runtime codec activation test and Task 9/13 share
  reachability/matrix tests, all with explicit Xcode source/resource membership.

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
- Execute under separate reviewed scopes: `docs/superpowers/plans/2026-07-12-multi-space-sqlite-store.md`
- Execute under separate approved scope: `docs/superpowers/plans/2026-07-13-newswire-core-slice-1.md`
- Add unchanged, then modify on the isolated integration branch: `.coverage-thresholds.json`
- Create on the isolated integration branch: `scripts/coverage-gate.sh`
- Create on the isolated integration branch: `scripts/test-coverage-gate.sh`
- Add on the isolated integration branch: `SERVICE-INVENTORY.md`
- Read: `docs/superpowers/specs/2026-07-13-sneakerweb-view-share-design.md`
- Read: `docs/superpowers/plans/2026-07-12-multi-space-sqlite-store.md`
- Read: `docs/superpowers/specs/2026-07-13-multi-community-open-newswire-mvp-design.md`
- Read: `.coverage-thresholds.json`
- Test: existing repository state only

- [ ] **Step 1: Track the current threshold, then land both prerequisites**

On an isolated integration branch, first add the existing
`.coverage-thresholds.json` unchanged together with the reviewed
`SERVICE-INVENTORY.md`. Prove its current command is the tarpaulin command
hardcoded by the already-approved prerequisite plans, then commit only those
two files:

```sh
test "$(jq -r '.enforcement.command' .coverage-thresholds.json)" = \
  "cargo tarpaulin --fail-under 100"
git add .coverage-thresholds.json SERVICE-INVENTORY.md
git diff --cached --check
git commit -m "chore: track repository quality contracts"
```

Execute P0-A first from that integration commit plus the two
`codex/sqlite-foundation` commits, then complete its separately gated
multi-space plan and land by ordinary non-force integration. Execute P0-B from
that integration head. Each prerequisite reads and runs the exact current
`.coverage-thresholds.json` command required by its own approved plan; at this
point that command is still the existing tarpaulin command. Both programs use
the same swarm TDD/validate/fresh-review/commit discipline as the tasks below.

- [ ] **Step 2: TDD and land the combined coverage infrastructure**

Write `scripts/test-coverage-gate.sh` first and observe it fail because the
wrapper is absent or the old tarpaulin-only command remains. The self-test uses
stubbed tool reports to prove missing tarpaulin/llvm-cov, every individual
dimension below 100, and statements/LLVM-regions threshold disagreement all
fail. It also proves the wrapper reads, rather than duplicates, all four values
from `.coverage-thresholds.json`.

Create `scripts/coverage-gate.sh`, change the source-of-truth enforcement
command to that script, map statements to LLVM regions explicitly, and run both
real tools. Run the self-test and real wrapper, review the isolated diff, then
commit only `.coverage-thresholds.json` and both scripts. This infrastructure commit lands on the prerequisite
integration head and must precede Task 1 and every SneakerWeb production
change. P0-B's already-executed hardcoded tarpaulin release command is therefore
never run after the source of truth changes.

```sh
scripts/test-coverage-gate.sh
scripts/coverage-gate.sh
git add .coverage-thresholds.json scripts/coverage-gate.sh \
  scripts/test-coverage-gate.sh
git diff --cached --check
git commit -m "test: enforce combined 100 percent coverage"
```

- [ ] **Step 3: Create the SneakerWeb isolated implementation worktree**

Invoke `superpowers:using-git-worktrees` and choose a feature branch rooted at
the latest shared integration commit that contains the approved design, this
revised reviewed plan, the tracked coverage/inventory files, P0-A, and P0-B.
Copy no remaining dirty file from the current checkout. Record the clean
starting commit:

```sh
git status --short
git rev-parse HEAD
```

Expected: no output from `git status --short`; a full 40-character commit ID
that is a descendant of the two prerequisite release commits and this plan's
approval commit.

- [ ] **Step 4: Prove the SQLite dependency and exact schema owner are landed**

```sh
rg -n 'pub struct RiotDatabase|pub struct DatabaseSession|pub struct SpaceSession' crates/riot-core crates/riot-ffi
rg -n '^rusqlite\s*=' Cargo.toml crates/riot-core/Cargo.toml
test -f crates/riot-core/src/store/database.rs
test -f crates/riot-core/src/store/schema.rs
cargo test --workspace --all-features
```

Expected: all three public types and the pinned SQLite dependency are found,
and the workspace is green. If any type is absent, stop this plan and complete
P0-A; do not scaffold substitute types here. If P0-A landed a different
canonical migration owner, revise every Task 3 schema path and rerun a fresh
plan-review gate before production code.

- [ ] **Step 5: Prove the Newswire prerequisite is landed**

```sh
rg -n 'struct NewsPostV1|pub mod (model|path|entry|projection|store)' crates/riot-core/src/newswire
rg -n 'pub mod apps|struct AppManifest|AppDataBridge' crates/riot-core/src
```

Expected: Newswire core and app runtime matches are present. Task 12 owns
creation of `PublicAttachmentRefV1`; it must not proceed if the Newswire core
model/path/entry/projection/store owners from P0-B are absent.

- [ ] **Step 6: Run the baseline contract and coverage commands**

```sh
cargo xtask validate-contracts
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
test -f .coverage-thresholds.json
test -f SERVICE-INVENTORY.md
test "$(jq -r '.enforcement.command' .coverage-thresholds.json)" = "scripts/coverage-gate.sh"
scripts/test-coverage-gate.sh
scripts/coverage-gate.sh
```

Expected: PASS before any SneakerWeb change. Save command output in the work
unit evidence, not in a new product file. If the inherited repository misses
any configured 100% dimension, stop before Task 1 and close that baseline gap
as a separately reviewed TDD prerequisite; never lower or bypass the threshold.

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

- [ ] **Step 4: Pin immutable offline oracle evidence**

`fixtures/sneakerweb/manifest.json` must contain the upstream crate checksum,
installed CLI SHA-256, exact full generation command, `.snk` digest, entry
count, payload count/bytes, namespace, and license. The committed fixture is
the immutable output of one recorded public-CLI run, not a claim of
bit-reproducible regeneration: SneakerWeb 1.0.1 uses `OsRng` for domains and
`Timestamp::now()` for publication. `README.md` must say this explicitly.
Its regeneration command creates a fresh semantically equivalent fixture in a
temporary directory, then verifies version, fixed namespace, structure,
signatures, payload digests/counts, and successful CLI/Riot cross-decode; it
must not compare the fresh random/time-dependent file hash to the committed
fixture hash. Ordinary tests remain fully offline and always use the committed
bytes/digest.

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
scripts/coverage-gate.sh
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
- Create: `crates/riot-ffi/src/sneakerweb_ffi.rs`
- Modify: `crates/riot-ffi/src/lib.rs`
- Create: `crates/riot-ffi/tests/sneakerweb_activation_contract.rs`
- Create: `apps/ios/RiotTests/SneakerWebInteropActivationTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/android/app/build.gradle.kts`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerWebInteropActivationTest.kt`

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
    DigestMismatch, IncompletePayload, LimitExceeded, CpuBudgetExceeded,
    Cancelled,
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

Inject a monotonic active-process-CPU clock into decoder worker tests. Exclude
blocked input waits, charge decoding/hashing/signature/capability work, poll
cancellation between bounded chunks, and reject without commit after at most
60 active CPU seconds per 100 MiB processed. Deterministic tests cover one tick
below, exactly at, and one tick above the proportional ceiling; a stalled input
producer consumes zero budget, while cancellation preempts the CPU-limit error.
The release worker uses the platform process/thread CPU clock, not wall time,
and callers cannot raise the constant.

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
the public error. CPU accounting begins when a bounded chunk enters the worker,
pauses before awaiting more input, and returns `CpuBudgetExceeded` before any
authoritative commit.

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

- [ ] **Step 4: Execute the official round trip inside both native runtimes**

Before any Task 3 schema or Task 6–8 UI work, expose the codec through the
normal generated native package as one narrow, non-signing activation contract:
`exercise_snk_codec(official_bytes) -> SnkInteropSummary`. It must decode the
fixed namespace, select the fixture's full ordered domain set, re-encode it,
and return only counts, byte lengths, full SHA-256 digests, and the emitted
bytes. It accepts no namespace, key, signer, capability, or mutable store
handle. Task 5 retains this diagnostic contract and adds asynchronous task
objects around production open/export flows.

`SneakerWebInteropActivationTests` loads the single pinned fixture through an
explicit RiotTests resource membership and calls the generated Swift binding
inside an iOS Simulator process. `SneakerWebInteropActivationTest` loads the
same repository fixture through an explicit `androidTest.assets.srcDirs`
entry and calls the generated Kotlin binding inside an Android emulator
process. Both assert official decode counts/digest and pass the emitted bytes
back through the same native binding to prove encode/decode. A host Rust test
or cross-build is not a substitute for either runtime execution.

- [ ] **Step 5: Prove official round trip and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_codec
cargo test -p riot-core --features conformance --test sneakerweb_hostile
cargo test -p riot-ffi --test sneakerweb_activation_contract
scripts/sneakerweb-interop.sh --offline --version 1.0.1 \
  --cli build/tools/sneakerweb/bin/sneakerweb --codec
scripts/conference/build-native-core.sh
scripts/conference/test-native-core-package.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerWebInteropActivationTests
(cd apps/android && ./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerWebInteropActivationTest)
cargo clippy -p riot-core --all-features --all-targets -- -D warnings
scripts/coverage-gate.sh
```

Expected: Riot decodes official bytes; the official CLI accepts Riot's selected
export with exact entry/capability/signature/payload components.

```sh
git add crates/riot-core/src/lib.rs crates/riot-core/src/sneakerweb \
  crates/riot-core/tests/sneakerweb_codec.rs crates/riot-core/tests/sneakerweb_hostile.rs \
  crates/riot-core/Cargo.toml fixtures/sneakerweb/hostile \
  crates/riot-ffi/src/sneakerweb_ffi.rs crates/riot-ffi/src/lib.rs \
  crates/riot-ffi/tests/sneakerweb_activation_contract.rs \
  apps/ios/RiotTests/SneakerWebInteropActivationTests.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj apps/android/app/build.gradle.kts \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerWebInteropActivationTest.kt
git commit -m "feat(sneakerweb): add bounded interoperable codec"
```

## Task 3: Add atomic collection persistence, provenance, block, and removal

**Files:**
- Create: `crates/riot-core/src/sneakerweb/collection.rs`
- Modify: `crates/riot-core/src/store/database.rs`
- Modify: `crates/riot-core/src/store/schema.rs`
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
Details pages. Task 12 owns community-batch concurrency because its coordinator
does not exist in this persistence task.

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
scripts/coverage-gate.sh
```

Expected: all atomicity and deterministic barrier tests pass.

```sh
git add crates/riot-core/src/sneakerweb crates/riot-core/tests/sneakerweb_collection*.rs \
  crates/riot-core/src/store/database.rs crates/riot-core/src/store/schema.rs
git commit -m "feat(sneakerweb): persist atomic collection state"
```

P0-A is required to retain its canonical migration runner at the verified
`crates/riot-core/src/store/{database,schema}.rs` paths. If the landed
prerequisite uses a different owner, stop before Task 1, revise every affected
path in this plan, and run the mandatory plan gate again; do not improvise a
second migration boundary.

## Task 4: Add portable blob CAS, leases, export, and crash reconciliation

**Files:**
- Create: `crates/riot-core/src/sneakerweb/blob.rs`
- Create: `crates/riot-core/src/sneakerweb/diagnostics.rs`
- Create: `crates/riot-core/src/sneakerweb/tasks.rs`
- Modify: `crates/riot-core/src/sneakerweb/{mod,codec,collection}.rs`
- Create: `crates/riot-core/tests/sneakerweb_blob.rs`
- Create: `crates/riot-core/tests/sneakerweb_export.rs`
- Create: `crates/riot-core/tests/sneakerweb_diagnostics.rs`

- [ ] **Step 1: Write RED lease/CAS/export tests**

Cover exact selected domains, blocked/unavailable domain rejection, 15-minute
idle expiry refreshed only by successful read/retain, `read_range <= 1 MiB`,
idempotent close, post-close error, temporary cleanup after cancel/crash,
retain-as-portable-blob, reference counting, same-digest thread/process
contention, every fsync/install/transaction crash point, winner/loser startup
reconciliation, unsupported filesystem mapping, and exact CLI cross-import.

Diagnostic tests first prove ordinary log events expose only a random internal
correlation ID, route, bounded lengths, and stable error code. They reject
payload text, full namespace/domain/entry/digest/signature/key values, private
space identifiers, and OS paths/URIs. The explicit export is bounded to the
newest 10,000 structured events and 5 MiB of canonical UTF-8 JSON, names its
schema and truncation state, may contain complete public SneakerWeb identifiers,
and redacts private-space identifiers and raw paths/URIs even if a hostile event
producer supplies them. Tests cover empty output, Unicode, malicious path/URI
strings, exact size/event boundaries, cancellation, expiry, crash cleanup, and
no export creation before confirmed consent.

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

Implement a bounded structured `SneakerDiagnosticSink` and
`DiagnosticExportTask -> DiagnosticExportLease`. The sink keeps sensitive
public protocol fields out of ordinary platform logging while retaining only
the bounded structured values needed for an explicit export. Export generation
requires an unexpired, single-use consent nonce bound to the displayed metadata
disclosure; cancellation or replay is non-mutating. The lease uses the same
protected, backup-excluded, 15-minute idle expiry and crash reconciliation as
the `.snk` export lease, but it can never be retained as a portable collection
blob.

- [ ] **Step 4: Verify and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_blob
cargo test -p riot-core --features conformance --test sneakerweb_export
cargo test -p riot-core --features conformance --test sneakerweb_diagnostics
scripts/sneakerweb-interop.sh --offline --version 1.0.1 \
  --cli build/tools/sneakerweb/bin/sneakerweb --round-trip
scripts/coverage-gate.sh
```

```sh
git add crates/riot-core/src/sneakerweb crates/riot-core/tests/sneakerweb_blob.rs \
  crates/riot-core/tests/sneakerweb_export.rs \
  crates/riot-core/tests/sneakerweb_diagnostics.rs
git commit -m "feat(sneakerweb): add leased portable blob storage"
```

## Task 5: Expose closed asynchronous UniFFI contracts

**Files:**
- Modify: `crates/riot-ffi/src/sneakerweb_ffi.rs`
- Modify: `crates/riot-ffi/src/lib.rs`
- Modify: `crates/riot-ffi/Cargo.toml`
- Create: `crates/riot-ffi/tests/sneakerweb_contract.rs`
- Create: `crates/riot-ffi/tests/sneakerweb_races.rs`

- [ ] **Step 1: Write RED generated-surface tests**

Require every DTO to carry `schema_version = 1`, every domain argument to be
exactly 32 bytes, no namespace parameter, no filesystem path/URI, full typed
errors, panic containment, stale cursors, repeated terminal results, and
finish/cancel/block/owner-close race linearization.

Create one parameterized lifecycle harness for `OpenSnkTask`, `SnkExportTask`,
and `DiagnosticExportTask` plus every task type added later. For each available
type, deterministically drop the final native handle during validation,
encoding, commit, transfer, and verification states that apply; assert the
cancellation token is visible by the next checkpoint, repeated cleanup is
idempotent, reservations/staging/leases are released, no callback or commit
appears late, and startup reconciliation finds no detached work. Tasks 10 and
11 must register their new types in this same harness before their commits.

Require `prepare_sneaker_diagnostic_export` to return only fixed host-owned
disclosure copy and a short-lived opaque preparation handle. Only
`confirm_sneaker_diagnostic_export` consumes that handle and returns a
single-use consent nonce; `begin_sneaker_diagnostic_export` consumes the nonce
once. Tests cover cancel, expiry at 30 seconds, replay, wrong profile/session,
owner close, confirm/begin races, and prove none of these methods are registered
in the miniapp JavaScript dispatcher.

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
DiagnosticExportTask: progress, finish, cancel
DiagnosticExportLease: metadata, read_range, close
```

Expose the exact factories and database calls `begin_snk_open`,
`begin_snk_open_from_blob`, `undo_snk_open`, `list_sneaker_sites`,
`list_received_sneaks`, `list_blocked_sneaker_sites`, `list_sneaker_storage`,
`get_sneaker_site`, `get_sneaker_details`, `get_received_sneak_details`,
`resolve_sneaker_resource`, `block_sneaker_domain`,
`unblock_sneaker_domain`, `remove_sneaker_source`, `remove_sneaker_site`,
`remove_portable_blob`, `create_snk_export`,
`prepare_sneaker_diagnostic_export`, `confirm_sneaker_diagnostic_export`, and
`begin_sneaker_diagnostic_export`. Tasks 10 and 11 extend this
same file only after their corresponding core task types exist. Do not expose
generic parser errors or raw handles.

- [ ] **Step 3: Prove native bindings contain only the closed API**

```sh
cargo test -p riot-ffi --test sneakerweb_contract
cargo test -p riot-ffi --test sneakerweb_races
cargo xtask generate-bindings
rg -n 'OpenSnkTask|SnkExportTask|ResourceLease|DiagnosticExportTask' build/generated/riot-ffi
! rg -n 'PathBuf|file_path|content_uri|namespace_id.*begin_snk' build/generated/riot-ffi
! rg -n 'prepare_sneaker_diagnostic_export|begin_sneaker_diagnostic_export' \
  apps/ios/Riot/Apps/RiotJS.swift \
  apps/android/app/src/main/kotlin/org/riot/evidence/apps/RiotJsShim.kt
scripts/coverage-gate.sh
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
- Create: `apps/ios/Riot/SneakerWeb/SneakerDiagnosticExport.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerAccessibility.swift`
- Create: `apps/ios/RiotTests/SneakerWebCoreTests.swift`
- Create: `apps/ios/RiotTests/SneakerLibraryViewModelTests.swift`
- Create: `apps/ios/RiotTests/SneakerAccessibilityContractTests.swift`
- Create: `apps/ios/RiotTests/SneakerDiagnosticExportTests.swift`
- Create: `apps/ios/RiotUITests/SneakerDocumentUITests.swift`

- [ ] **Step 1: Write RED document and accessible-library tests**

Tests must drive an actual `.snk` document URL, stream through a
security-scoped handle, release scope on every terminal path, make the Sites
segment available but route an external open into the new Received record immediately after
successful open, show `N added, N updated, N unchanged, N
blocked · X MB stored`, preserve zero mutation on invalid/cancel/quota failure,
offer Undo, and reopen persisted state after process reconstruction.

Use an injected monotonic clock and collection generation to prove Undo remains
available through 30 seconds after commit unless another collection mutation
begins first. Tests cover 29.999 seconds, exactly 30 seconds, expiry immediately
after a second open/block/remove mutation, process reconstruction within the
window, overlapping retained sources, and a post-expiry typed no-op. UI focus
and announcement restoration are asserted after successful and unavailable
Undo.

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

Add a shared native `SneakerAccessibility` contract used by Open, Export,
nearby transfer, space-blob receive, and community share. Tests at a 320-point
content width and every accessibility Dynamic Type size require reflow without
horizontal clipping or hidden actions; keyboard/Switch Control traversal must
show visible focus; Increase Contrast/high-contrast mode must preserve text,
focus, and non-color state distinctions; Reduce Motion must replace decorative
motion with static state changes while preserving semantic progress.

The progress announcer emits the operation/stage at start, then at the less
frequent of each new 10% bucket and ten active seconds, and exactly one terminal
success, cancellation, or failure result. For determinate work this is an AND
gate: both a new 10% bucket and ten active seconds since the last announcement
must be satisfied. Indeterminate work announces every ten active seconds;
pause/suspension does not advance the clock; resume does not duplicate the last
bucket; regressions/retries start a newly named stage. Fake-clock tests cover
0/9/10/19/20/100%, 9.999/10 seconds, percentage-only, time-only, both-thresholds,
pause/resume, cancel, failure, retry, and terminal deduplication. Task-specific
suites in Tasks 9–12 assert their visible stage names are routed through this
contract rather than posting ad hoc announcements.

Diagnostic export tests begin from the global Storage screen and prove the
core preparation handle is not confirmed or consumed until a genuine native
confirmation action. The confirmation names full public identifiers, digests,
signatures, timestamps, route names, lengths, and stable error codes as exposed
metadata, and states that payload text, private-space identifiers, and device
paths are excluded. Test Cancel, confirmation expiry/replay, protected and
backup-excluded temporary creation, system-sheet success/cancel, 15-minute
expiry, relaunch cleanup, focus restoration, and exact exported redactions.
An injected native logging seam also proves ordinary iOS logs never receive
payload text or a full namespace/domain/entry/digest/signature/key value.

- [ ] **Step 2: Register only `.snk` document intake**

Add an imported `UTType` for `.snk` and route `onOpenURL` into
`SneakerDocumentIntake`. Do not claim editor/exported-type ownership. Keep
macOS Info.plist and project settings unchanged.

- [ ] **Step 3: Implement the global library state model**

`SneakerLibraryModel` owns pages, generation restart, selection, open summary,
undo, block/unblock, and storage recovery. It stores no raw SQL and never turns
full byte IDs into shortened display strings. The profile menu and large-screen
sidebar link to the same `SneakerLibraryView`.

- [ ] **Step 4: Implement confirmed diagnostic export**

Add `SneakerDiagnosticExport` to the global Storage surface, hidden behind the
explicit disclosure confirmation. It streams `DiagnosticExportLease` into the
same protected temporary-file coordinator as standard file sharing, uses the
system share sheet, and destroys the native file and lease on completion,
cancel, expiry, relaunch, or owner close. It is not a `.snk` file, cannot be
opened as a collection, and never appears in Sites or Received.

- [ ] **Step 5: Verify iOS model/UI and commit**

```sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerWebCoreTests \
  -only-testing:RiotTests/SneakerLibraryViewModelTests \
  -only-testing:RiotTests/SneakerAccessibilityContractTests \
  -only-testing:RiotTests/SneakerDiagnosticExportTests
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotUITests/SneakerDocumentUITests
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS'
scripts/coverage-gate.sh
```

```sh
git add apps/ios/Riot/Info.plist apps/ios/Riot/RiotApp.swift apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/ConferenceShellView.swift apps/ios/Riot/SneakerWeb \
  apps/ios/RiotTests/SneakerWebCoreTests.swift \
  apps/ios/RiotTests/SneakerLibraryViewModelTests.swift \
  apps/ios/RiotTests/SneakerAccessibilityContractTests.swift \
  apps/ios/RiotTests/SneakerDiagnosticExportTests.swift \
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
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerDiagnosticExport.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerAccessibility.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerLibraryViewModelTest.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerAccessibilityContractTest.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerDiagnosticExportTest.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerDocumentOpenTest.kt`

- [ ] **Step 1: Write RED Android parity tests**

Mirror Task 6's open/summary/undo/persistence, Sites/Received/Details/Blocked/
Storage, raw-ID visibility, selection, stale-page, accessibility, and failure
contracts. Add content-resolver tests proving the core receives bytes and safe
display metadata, never a content URI or arbitrary path.

Mirror the exact injected-clock Undo boundary cases from Task 6: 29.999
seconds, exactly 30 seconds, intervening collection mutation, reconstruction,
overlapping sources, expired no-op, focus, and announcement behavior.

Mirror Task 6's full accessibility contract with Android font scale/display
size and narrow-window reflow, visible keyboard/Switch Access focus, high-text-
contrast/non-color semantics, animator-duration-scale/Reduce Motion behavior,
and the identical fake-clock start/less-frequent-10%-and-ten-second/terminal
announcement cadence. The Android announcer is the one shared path for Tasks 7
and 9–12.

Mirror the Task 6 diagnostic disclosure, genuine-confirmation boundary,
single-use consent/expiry, protected cache file, backup exclusion where the
platform supports it, share-sheet completion/cancel, relaunch/15-minute
cleanup, focus restoration, and byte-for-byte redaction assertions. The test
must prove no export task begins when the user cancels the disclosure. An
injected Android logging seam proves ordinary Logcat output never receives
payload text or a full namespace/domain/entry/digest/signature/key value.

- [ ] **Step 2: Register bounded VIEW intent handling**

Declare `.snk` VIEW/OPENABLE handling with the narrow MIME/extension contract,
open the `ContentResolver` stream, copy chunks of at most 256 KiB, enforce
expected length when known, and close provider/task handles on success,
failure, cancellation, and Activity destruction.

- [ ] **Step 3: Implement UI parity**

Use the existing Android presentation style rather than adding a new framework.
The global navigation placement, semantics, messages, recovery actions, and
identifier policy must match iOS and the design, while preserving platform
focus and back behavior. Add the same confirmed diagnostic export to global
Storage; it shares a non-`.snk` protected temporary file and never inserts a
Sites or Received record.

- [ ] **Step 4: Verify and commit**

```sh
cd apps/android
./gradlew testDebugUnitTest --tests '*SneakerLibraryViewModelTest' \
  --tests '*SneakerAccessibilityContractTest' \
  --tests '*SneakerDiagnosticExportTest'
./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerDocumentOpenTest
./gradlew lintDebug
cd ../..
scripts/coverage-gate.sh
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

External-handoff RED cases require a main-frame navigation plus a single-use
native gesture token. The shared parser accepts only `http`/`https`, caps the
original UTF-8 URL at 2,048 bytes, and rejects userinfo, C0/C1 controls, bidi
controls, invalid/ambiguous bracketed or numeric IP/host syntax, backslash
authority ambiguity, and custom/file/content/data/javascript/intent schemes.
Tests cover synthetic clicks, subframe/window/popup/form redirects, token
replay, normalization collisions, default/non-default ports, empty path,
percent encoding, Unicode IDNA labels, and fuzzed hostile URLs. The safe DTO
contains separately bidi-isolated normalized scheme, Unicode hostname,
ASCII/punycode hostname, explicit port, path, query, and fragment strings; it
never contains HTML and cannot itself open a URL.

Parse and assert each CSP directive independently, especially `form-action 'none'`
and `frame-ancestors 'none'`; also require
`X-Content-Type-Options: nosniff`, path-derived Content-Type,
`Cross-Origin-Resource-Policy: same-origin`, `Referrer-Policy: no-referrer`,
`Cache-Control: no-store`, and a Permissions-Policy denying camera,
microphone, geolocation, payment, USB, Bluetooth, sensors, and clipboard.
Packet-level platform tests must prove DNS prefetch, preconnect, speculative
navigation, and renderer-created sockets never leave loopback.

HTTP/cookie tests require the fixed-name capability cookie to be 256 random
bits, host-only with omitted `Domain`, `Path=/`, `HttpOnly`, `SameSite=Strict`,
and non-persistent on both platform cookie stores. A first-page and subresource
load must carry it without exposing it to JavaScript; reader close, block, and
domain transition synchronously revoke both server and WebView state before
the old binding can read again. `HEAD` returns GET-equivalent status/headers
with an empty body. Exactly one satisfiable bounded byte range returns `206`
with correct `Content-Range`/length. Accept only `bytes=<start>-<end>` with both
unsigned decimal bounds present, `start <= end`, and at most 1 MiB inclusive;
return indistinguishable `416` for suffix, open-ended, overflow,
unsatisfiable, over-1-MiB, and multi-range inputs without allocating or reading
outside the `ResourceLease`. Duplicate Cookie headers remain rejecting.

Add reader-state tests for Back, Forward, Library, Details, and Share. They
must preserve the current full domain, canonical path, per-history-entry scroll
position, prior library page/row, and invoking focus across reader, Details,
Share, process-failure Retry, and Back transitions.

Add a corpus and fuzz/property suite for the non-executing `/index.html` title
extractor: read at most 256 KiB, cap nesting/tokenizer work/text output, reject
invalid UTF-8/control/bidi scalars, collapse whitespace, decode entities once,
accept only a valid HTML `<title>`, and otherwise return exactly `Untitled
site`. Absent, empty, malformed, non-HTML, or any exhausted-limit input uses
that same fallback; body/visible text, the full domain key, script, CSS,
subresource, and `sneakerweb.html` values may never become the native or
accessibility title.

Preview tests require a separate offscreen host with an opaque origin and a
single-use capability bound to the exact reader/domain/block/session generation.
Wrong, expired, replayed, or cross-reader capabilities render no preview. Fix
the viewport and scale in both platforms, cap input/decoded raster bytes,
terminate on the injected time and memory watchdogs, flatten the successful
result to a native image, then prove the preview WebView/process and capability
are gone before the library displays it. Cover hostile oversized images/fonts,
network attempts, memory termination, process crash, repeated main-thread
stall, backgrounding, and block/reader close. No test may find a live iframe,
WebView, cookie, bridge, or navigable element in the library card afterward.

- [ ] **Step 2: Implement the native loopback capability boundary**

Bind only to `127.0.0.1`/`::1` on exact port `1312`; if neither loopback family
can bind that port, return `VIEWER_UNAVAILABLE` and do not substitute another
port because canonical SneakerWeb links depend on 1312. Create one 256-bit
host-only, `HttpOnly`, `SameSite=Strict`, `Path=/`, non-persistent cookie per
reader and install it through the native cookie store before first navigation.
Omit `Domain`. Revoke it synchronously in server and WebView state on
close/block/domain transition. Require
exact `sneakerweb.localhost:1312` or a full 64-hex-domain `.localhost:1312` Host,
serve only `ResourceLease` bytes, and stop the listener before closing leases.

Implement bodyless `HEAD` and one bounded single-byte-range response over
`ResourceLease::read_range`; reject malformed, multiple, overflow, or
policy-exceeding ranges with the specified non-enumerating error and never
materialize a whole resource to answer a range.

- [ ] **Step 3: Harden WebViews and title preview extraction**

iOS uses ephemeral `WKWebsiteDataStore`, app-bound navigation/content rules,
and loopback-only ATS. Android permits only the loopback origin and disables
external resource access, service workers, permissions, file/content access,
and bridge injection. Render `sneakerweb.html` in a disposable, offscreen,
scriptless, sandboxed opaque-origin host authorized by a single-use
reader/domain/block/session capability. Use a fixed viewport/scale plus strict
input, decoded-raster, node, two-active-second, and 64 MiB watchdog limits;
crash, stall, background, or memory termination closes the host. On success,
snapshot to a bounded native bitmap, tear down the WebView/process/cookie and
capability, and give only the inert image to the library—never a live site
iframe or view. Use the bounded `/index.html` extractor for the native and accessibility title.
Implement the reader toolbar/history contract in `SneakerReaderView` and
`SneakerReaderScreen` with the state restoration tested in Step 1.

In `Info.plist`, keep arbitrary loads disabled and add an ATS exception only
for `localhost` including subdomains on insecure loopback HTTP. In Android,
set `android:networkSecurityConfig="@xml/sneakerweb_network_security_config"`;
the XML uses `cleartextTrafficPermitted="false"` as its base and permits only
`localhost` with subdomains. Platform tests inspect these files and prove a
non-loopback cleartext URL remains blocked.

Implement external handoff in the shared Rust `viewer.rs` parser and both
native navigation delegates. Site JavaScript can only trigger a rejected
navigation event; only a natively confirmed main-frame gesture mints the
single-use token. Present a native inert confirmation that labels and displays
every DTO component, including both Unicode and punycode hostnames, before
passing the already-validated `http`/`https` URL to the system browser. iOS and
Android isolation suites assert every rejection class above, component display,
cancel/focus restoration, token replay failure, and that no site-controlled
string is interpreted as rich text or an app URL handler.

- [ ] **Step 4: Verify and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_viewer
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerWebViewIsolationTests
(cd apps/android && ./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerWebViewIsolationTest)
scripts/coverage-gate.sh
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
- Create: `apps/ios/RiotTests/SneakerSystemShareReachabilityTests.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerShareCoordinator.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerShareScreen.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerShareCoordinatorTest.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerSystemShareReachabilityTest.kt`

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

Both native share suites inject the Task 6/7 accessibility announcer and assert
Preparing starts, less-frequent-10%-and-ten-second updates, Ready, OS-sheet
cancellation, and terminal failure/success announcements without duplicates. Review and sheet
layouts rerun the reflow/focus/high-contrast/Reduce Motion contract.

The exact file column of the mandatory matrix is exercised by
`shareFile_oneSite`, `shareFile_multipleSites`, and
`shareFile_receivedCollection` in both native test stacks. Each test asserts
the full domain set in the bytes handed to the OS sheet rather than merely
asserting that the sheet was requested.

- [ ] **Step 2: Implement one preserved native review model**

The coordinator freezes full domain IDs and block generations in native memory,
shows only safe titles/count/size by default, creates `SnkExportTask`, streams
its lease to a protected backup-excluded temporary file, and presents the OS
sheet only in `Ready to share`. A failed handoff preserves the selection and
review fields; a block generation change returns to selection.

- [ ] **Step 3: Wire system sharing into both app shells now**

Add the user-reachable Site, multi-select Library, and Received routes to
`AppModel.swift` and `ConferenceShellView.swift`, and the matching destinations
in Android `MainActivity.kt`. The route must construct the preserved review
model and reach the Task 9 system-share coordinator without relying on Task 10
nearby code. The reachability suites launch each source route, invoke Share,
observe the OS-share presentation seam, cancel it, and verify focus/selection
restoration. This is an integration contract, not a view-only unit test.

- [ ] **Step 4: Verify and commit**

```sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerShareTaskTests \
  -only-testing:RiotTests/SneakerSystemShareReachabilityTests
(cd apps/android && ./gradlew testDebugUnitTest --tests '*SneakerShareCoordinatorTest')
(cd apps/android && ./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerSystemShareReachabilityTest)
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS'
scripts/coverage-gate.sh
```

```sh
git add apps/ios/Riot/SneakerWeb apps/ios/RiotTests/SneakerShareTaskTests.swift \
  apps/ios/RiotTests/SneakerSystemShareReachabilityTests.swift \
  apps/ios/Riot/AppModel.swift apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb \
  apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerShareCoordinatorTest.kt \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerSystemShareReachabilityTest.kt
git commit -m "feat(mobile): share standard SneakerWeb files"
```

## Task 10: Multiplex direct nearby and portable-blob transfer

**Files:**
- Create: `crates/riot-core/src/sneakerweb/nearby.rs`
- Modify: `crates/riot-core/src/sneakerweb/{mod,tasks,blob}.rs`
- Create: `crates/riot-core/tests/sneakerweb_nearby.rs`
- Create: `fixtures/sneakerweb/nearby-envelope-v2.json`
- Modify: `crates/riot-ffi/src/sneakerweb_ffi.rs`
- Modify: `crates/riot-ffi/tests/sneakerweb_contract.rs`
- Modify: `crates/riot-ffi/tests/sneakerweb_races.rs`
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

With an injected monotonic clock, assert a direct one-shot capability is bound
to confirmed session, digest, length, sender, and recipient; it expires on one
completion, cancel, session close, and at exactly ten minutes. Assert the V2
multiplexer stays open while either bounded channel is active, then closes
exactly once at 60 seconds idle (59.999/open, 60/close), cancels orphaned blob
tasks, and cannot be revived by a late frame. These timeout tests run for both
local TCP and BLE adapters without sleeping in wall time.

Freeze one canonical envelope/frame/checkpoint transcript in
`fixtures/sneakerweb/nearby-envelope-v2.json`. Rust, Swift, and Kotlin tests
must each encode to the exact bytes and decode the other side's named vectors,
including resume and verified-ack frames. This catches codec drift before
physical pairing; the Task 13 device gate still proves both live directions.

The direct-nearby column of the mandatory matrix is exercised by
`shareNearby_oneSite`, `shareNearby_multipleSites`, and
`shareNearby_receivedCollection` on both native stacks. Each test runs sender
preparation through verified receiver acknowledgement and asserts that the
receiver's portable lease decodes to the exact expected unblocked domain set.

Register `DirectSnkSendTask` and `DirectSnkReceiveTask` in Task 5's shared
final-handle-drop harness. Add deterministic block-generation barriers before
the first chunk, between two 256 KiB chunks, immediately before/after a verified
32 MiB checkpoint, and during final verification/ack. A newly blocked selected
domain cancels sender and receiver work before the next chunk publication,
publishes no verified ack or portable lease, releases reservations/staging,
and returns the preserved selection with the blocked site excluded. Repeated
cancel/drop and late frames remain non-mutating.

- [ ] **Step 2: Implement separate send and receive task outcomes**

Expose and test:

```text
DirectSnkSendTask -> SendReceipt only after verified ack
DirectSnkReceiveTask -> PortableBlobLease
```

Receiving never mutates `SneakerCollection`; native must explicitly pass the
lease to `begin_snk_open_from_blob`. Local TCP is preferred; BLE is an honest
resumable public-content fallback, not confidentiality.

Both direct tasks carry the frozen selected-domain block generations and
recheck them before publishing or accepting every chunk and before verified
ack/lease publication. A mismatch sets cancellation immediately, discards
unverified staging, and cannot be resumed by the old one-shot capability.

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

Both native nearby suites inject the Task 6/7 accessibility announcer for the
sender and recipient state machines. They assert one start announcement for
every new stage; determinate updates only after both a new 10% bucket and ten
active seconds; no cadence while paused; no duplicate on resume; one terminal announcement for
Sent, Received, Reject, Cancel, expiry, interruption, or integrity failure; and
a new stage announcement on Retry or restart at zero. The UI suites repeat the
narrow reflow, visible focus, high-contrast, and Reduce Motion assertions while
transferring and while confirmation controls are visible.

- [ ] **Step 4: Verify and commit**

```sh
cargo test -p riot-core --features conformance --test sneakerweb_nearby
cargo test -p riot-ffi --test sneakerweb_contract
cargo test -p riot-ffi --test sneakerweb_races
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
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS'
scripts/coverage-gate.sh
```

```sh
git add crates/riot-core/src/sneakerweb crates/riot-core/tests/sneakerweb_nearby.rs \
  fixtures/sneakerweb/nearby-envelope-v2.json \
  crates/riot-ffi/src/sneakerweb_ffi.rs crates/riot-ffi/tests/sneakerweb_contract.rs \
  crates/riot-ffi/tests/sneakerweb_races.rs \
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
- Modify: `crates/riot-core/src/import/bundle.rs`
- Modify: `crates/riot-core/src/session.rs`
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
- Create: `crates/riot-core/tests/sneakerweb_space_blob_multi_peer.rs`
- Modify: `crates/riot-ffi/src/{apps_ffi,sneakerweb_ffi}.rs`
- Modify: `crates/riot-ffi/tests/{apps_contract,sneakerweb_contract,sneakerweb_races}.rs`
- Modify: `apps/ios/Riot/Apps/{AppBridgeController,AppBundleCodec,AppReviewSheet,AppRuntimeView,RiotJS}.swift`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryModel.swift`
- Modify: `apps/ios/RiotTests/{AppRepositoryTests,AppRuntimeHostTests}.swift`
- Modify: `apps/ios/RiotTests/{DirectoryRepositoryTests,DirectoryStorefrontTests}.swift`
- Modify: `apps/ios/RiotTests/AppSyncReplicationTests.swift`
- Create: `apps/ios/Riot/SneakerWeb/SneakerCarrierCardView.swift`
- Create: `apps/ios/RiotTests/SneakerCarrierCardTests.swift`
- Create: `apps/ios/RiotTests/SneakerSpaceBlobMultiPeerTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/apps/{AppBundleCodec,CanonicalCbor,RiotJsBridge,RiotJsShim,AppWebViewHost,RiotAppsController,InstalledAppsStore,DirectoryController,UniffiDirectoryPort}.kt`
- Modify: `apps/android/app/src/test/kotlin/org/riot/evidence/apps/{AppBundleCodecTest,RiotJsBridgeTest,InstalledAppsStoreTest,DirectoryControllerTest}.kt`
- Modify: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppRuntimeEndToEndTest.kt`
- Modify: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppPersistenceRestartTest.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerCarrierCard.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerCarrierCardTest.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerSpaceBlobMultiPeerTest.kt`

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

Keep the exact pre-v2 parser as a version-specific `decode_manifest_v1` path,
not a permissive fallback. Feed one canonical v2 manifest byte vector to both
paths: the v1 path must reject the unknown version/key without installation or
trust mutation, while the new version-dispatching decoder accepts it and
derives the v2 app ID. Run the same old-client-reject/new-client-accept vector
through Rust and both native manifest decoders; v1 vectors remain byte-identical.

Update every ordinary manifest consumer, not only the codec: app-index pair
validation, directory projection, starter catalog, mobile installation/state,
and FFI installed/directory records must carry the version and closed machine
capabilities without confusing them with author-supplied permission copy.
The strict `riot-app.json` packer accepts existing v1 documents unchanged and
requires both `"manifest_version": 2` and sorted unique
`"machine_capabilities"` for v2; unknown keys/tokens remain rejecting. Both
native manifest decoders and their review models display host-owned capability
copy rather than bundle-supplied wording.

Update every direct generated-record constructor in Android tests, including
`InstalledAppsStoreTest.kt` and `DirectoryControllerTest.kt`, and assert that
manifest version/capabilities survive repository retention, directory mapping,
trust changes, and bridge registration rather than merely compiling with new
default values.

Update the shipping-host reproductions in iOS `AppSyncReplicationTests.swift`
and Android `AppRuntimeEndToEndTest.kt`. Each must mount the real
`AppBridgeController`/`RiotJsBridge` plus WebView host, prove a trusted v1 app
still receives no `riot.collections`, prove a trusted v2 app gains the surface
only with `portable_public_collections`, and prove revoke/profile/space changes
remove it without breaking ordinary sync or legacy app methods.

Extend Android `AppPersistenceRestartTest.kt` with process reconstruction of
trusted v1, trusted-capable v2, untrusted v2, and revoked v2 records. Manifest
version and closed capabilities must survive durable restart, but bridge
registration is recomputed from current per-space trust/session state; stale
pre-restart capability handlers never survive revocation or space change.

- [ ] **Step 2: Implement canonical carrier, binding, and manifest v2 codecs**

Encode and decode the exact deterministic-CBOR carrier and binding maps from
the approved design, including exact path shape, same-space/same-signer checks,
public-space enforcement, strict safe text, ordered domain/label alignment,
and unknown/malformed rejection. Extend `AppManifest` as a versioned closed
contract while keeping v1 bytes and app IDs unchanged; propagate version and
machine capabilities through app index, directory, starter, mobile state, and
FFI. Keep organizer review copy host-owned.

Extend the existing closed import/admission classifiers in
`crates/riot-core/src/import/bundle.rs` and the local-write/path-binding checks
in `crates/riot-core/src/session.rs`. The reserved carrier and
`objects/app-public-collections/` families must be recognized before generic
alert/app handling; malformed reserved paths or payloads must reject and can
never fall through. Synced and local writes apply identical same-space,
same-signer, exact-path, capability, and canonical-payload checks. Generic app
writes to either reserved family fail before signing. Add focused remote
projection, local-write rejection, malformed-fallback, foreign-space, and
foreign-signer tests to `sneakerweb_carrier.rs` and
`core_import_app_index_entries.rs`.

Propagate manifest version and closed machine capabilities through
`InstalledAppRecord` and `DirectoryListing` into
`ProfileRepository.swift`'s `RiotSpaceApp`, `DirectoryModel.swift` rows,
`AppReviewSheet`, and bridge registration. `DirectoryRepositoryTests` and
`DirectoryStorefrontTests` must prove host-owned review copy, per-space trust,
capability-gated bridge installation, v1 absence, v2 presence, and unknown
capability rejection. Native code must not infer capability from display text.

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
The bridge-created picker gesture token is single-use, bound to the exact
active trusted app, space, profile, and session generation, and expires if the
native picker has not appeared at exactly 30 seconds. Fake-clock tests cover
29.999 seconds, 30 seconds, replay, wrong app/space/profile/session, delayed
native consumer, and expiry racing `take_native_event`; the linearization
winner is deterministic and the loser observes a typed non-mutating result.
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

Register `SpaceSneakerShareTask`, `SpaceBlobReceiveTask`, and
`AppCollectionPickerTask` in the shared final-handle-drop lifecycle harness.
Drop each task during every applicable prepare/sign/commit/receive/verify
state, proving idempotent cancellation, no late native/app callback, no orphan
carrier/binding, and complete reservation/staging/lease cleanup.

The space-blob receive coordinator accepts bounded holder offers only from
currently connected peers that independently prove the same active public
space, full carrier entry, digest, expected length, and current authority. It
never advertises or queries a global digest inventory. Try at most eight
authorized offers sequentially; rejection, timeout, authority loss, or peer
loss advances to the next offer. A new holder may resume only from the last
locally verified 32 MiB checkpoint for the exact carrier/digest/length tuple;
otherwise restart at zero. Each holder rechecks authority before every chunk.
Requester and holder also recheck every selected domain's frozen block
generation before each chunk and before ready-lease publication.

Three-peer deterministic tests use requester A, first holder B, and second
holder C. B disconnects before data, mid-chunk, and after a verified checkpoint;
C then serves the same carrier and completes with the exact digest. Also cover
B authority revocation, C wrong carrier/digest/length, all holders unavailable,
eight-offer exhaustion, cancellation during handoff, duplicate/late chunks,
and a fourth unrelated/private-space peer that remains invisible. Native tests
assert `Waiting for nearby holder`, handoff without a second Get gesture,
checkpoint-resume progress, honest restart-at-zero, Retry after exhaustion,
and one terminal Open lease with no automatic collection mutation.

For space-blob transfer, add the same deterministic block-generation barriers
as direct transfer before a chunk, between chunks, around a verified checkpoint,
and during verification. A requester-side block or a holder-side generation
change cancels publication before the next chunk, invalidates handoff/resume,
emits no ready lease, and cannot be bypassed by a later authorized holder or a
late frame. Unblocking never resumes the old task; Retry starts a fresh request
against the new generation.

The carrier-card and multi-peer native suites inject the Task 6/7 accessibility
announcer. They assert the shared less-frequent-10%-and-ten-active-second cadence
and exactly one terminal announcement through waiting, holder handoff, checkpoint resume,
restart at zero, transfer, verification, cancellation, exhaustion, failure,
Retry, and ready-to-open. Card and progress layouts repeat narrow reflow,
visible focus, high-contrast, and Reduce Motion checks; holder changes never
expose or announce a raw peer, carrier, digest, domain, or key identifier.

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
against the P0-B core using the approved SneakerWeb attachment contract, then
regenerates bindings.

`request_space_blob` verifies an active public `SpaceSession`, the full
same-space carrier entry, digest, length, and current authority before creating
`SpaceBlobReceiveTask -> PortableBlobLease`; receiving still does not mutate
the collection.

- [ ] **Step 7: Implement and verify the Directory native carrier card**

Both native app-runtime integration owners mount the host card for a Directory
listing. Task 12 later adapts Newswire attachments into this already-tested
card model without introducing a Newswire type here. Tests cover `Available locally / Open`,
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
cargo test -p riot-core --features conformance --test sneakerweb_space_blob_multi_peer
cargo test -p riot-ffi --test sneakerweb_contract
cargo test -p riot-ffi --test sneakerweb_races
scripts/conference/build-native-core.sh
scripts/conference/test-native-core-package.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerCarrierCardTests \
  -only-testing:RiotTests/SneakerSpaceBlobMultiPeerTests \
  -only-testing:RiotTests/AppSyncReplicationTests \
  -only-testing:RiotTests/AppRepositoryTests \
  -only-testing:RiotTests/AppRuntimeHostTests \
  -only-testing:RiotTests/DirectoryRepositoryTests \
  -only-testing:RiotTests/DirectoryStorefrontTests
(cd apps/android && ./gradlew testDebugUnitTest \
  --tests '*SneakerCarrierCardTest' --tests '*SneakerSpaceBlobMultiPeerTest')
(cd apps/android && ./gradlew testDebugUnitTest \
  --tests '*InstalledAppsStoreTest' --tests '*DirectoryControllerTest' \
  --tests '*AppBundleCodecTest' --tests '*RiotJsBridgeTest')
(cd apps/android && ./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.apps.AppRuntimeEndToEndTest)
(cd apps/android && ./gradlew connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.apps.AppPersistenceRestartTest)
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS'
scripts/coverage-gate.sh
```

```sh
git add crates/riot-core/src/apps crates/riot-core/src/sneakerweb \
  crates/riot-core/src/import/bundle.rs crates/riot-core/src/session.rs \
  crates/riot-core/src/demo_fixture.rs crates/riot-core/examples \
  crates/riot-core/tests/apps_manifest_v2.rs crates/riot-core/tests/sneakerweb_carrier.rs \
  crates/riot-core/tests/sneakerweb_social_host.rs \
  crates/riot-core/tests/sneakerweb_space_blob_multi_peer.rs \
  crates/riot-core/tests/apps_*.rs \
  crates/riot-core/tests/core_import_app_index_entries.rs \
  crates/riot-app-cli/src/lib.rs crates/riot-app-cli/tests/cli_pack.rs \
  crates/riot-ffi/src crates/riot-ffi/tests \
  apps/ios/Riot/Apps apps/ios/Riot/Core/ProfileRepository.swift \
  apps/ios/Riot/Directory/DirectoryModel.swift \
  apps/ios/RiotTests/AppRepositoryTests.swift \
  apps/ios/RiotTests/AppRuntimeHostTests.swift \
  apps/ios/RiotTests/AppSyncReplicationTests.swift \
  apps/ios/RiotTests/DirectoryRepositoryTests.swift \
  apps/ios/RiotTests/DirectoryStorefrontTests.swift \
  apps/ios/Riot/SneakerWeb/SneakerCarrierCardView.swift \
  apps/ios/RiotTests/SneakerCarrierCardTests.swift \
  apps/ios/RiotTests/SneakerSpaceBlobMultiPeerTests.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/android/app/src/main/kotlin/org/riot/evidence/apps \
  apps/android/app/src/test/kotlin/org/riot/evidence/apps \
  apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/SneakerCarrierCard.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerCarrierCardTest.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerSpaceBlobMultiPeerTest.kt \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppRuntimeEndToEndTest.kt \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppPersistenceRestartTest.kt
git commit -m "feat(apps): host public SneakerWeb collection bindings"
```

## Task 12: Ship Sneaker Directory and multi-community destination orchestration

**Files:**
- Create: `fixtures/apps/sneaker-directory/{riot-app.json,index.html,tokens.css,style.css,app.js}`
- Create: packed Sneaker Directory manifest/bundle artifacts beside other starters
- Modify: `crates/riot-core/src/apps/starter.rs`
- Modify: `crates/riot-core/tests/apps_starter.rs`
- Create: `crates/riot-core/tests/sneakerweb_share_matrix.rs`
- Modify: `crates/riot-core/src/sneakerweb/{social,tasks}.rs`
- Modify: `crates/riot-ffi/src/{lib,sneakerweb_ffi}.rs`
- Create: `crates/riot-ffi/src/newswire_ffi.rs`
- Modify: `crates/riot-ffi/tests/sneakerweb_contract.rs`
- Create: `crates/riot-ffi/tests/newswire_contract.rs`
- Modify: `scripts/apps/{miniapp-contracts.mjs,miniapp-browser.spec.mjs}`
- Create: `scripts/apps/package.json`
- Create: `scripts/apps/package-lock.json`
- Modify: `apps/ios/Riot/SneakerWeb/{SneakerShareCoordinator,SneakerShareView}.swift`
- Modify: `apps/ios/Riot/{AppModel,ConferenceShellView}.swift`
- Create: `apps/ios/Riot/Newswire/{NewswireModel,OpenWireView,NewswireAttachmentRow}.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb/{SneakerShareCoordinator,SneakerShareScreen}.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/newswire/{NewswireViewModel,OpenWireScreen,NewswireAttachmentCard}.kt`
- Create: `apps/ios/RiotTests/SneakerDirectoryPermissionTests.swift`
- Create: `apps/ios/RiotTests/SneakerShareMatrixTests.swift`
- Create: `apps/ios/RiotTests/SneakerNewswireAttachmentTests.swift`
- Create: `apps/ios/RiotUITests/SneakerDirectoryPermissionUITests.swift`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerDirectoryPermissionTest.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerShareMatrixTest.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerNewswireAttachmentTest.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerCommunityShareTest.kt`
- Modify: `crates/riot-core/src/newswire/{model,path,entry,projection,store}.rs`
- Modify: `crates/riot-core/tests/{newswire_codec,newswire_entry,newswire_import,newswire_projection,newswire_end_to_end}.rs`

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

Add the shared matrix harness now. `SneakerShareMatrixTests` and
`SneakerShareMatrixTest` reuse the already-proven file and nearby adapters and
add `shareDirectory_{oneSite,multipleSites,receivedCollection}` plus
`shareNewswire_{oneSite,multipleSites,receivedCollection}`. The Directory
cases assert one carrier plus one binding, safe projection, exact selected
domain set, and duplicate-free per-space retry. The Newswire cases assert one
carrier plus one attachment, exact selected domain set, ordinary editorial
projection, and independent multi-community retry. The Rust
`sneakerweb_share_matrix` test owns the same 12-case table so missing enum
members or destination adapters fail at compile/test time.

P0-B guarantees the Newswire owners exist before this task. All 12 cells must
be registered and pass in Task 12; Task 13 independently rejects any missing,
ignored, or skipped matrix cell.

Add fake-clock/scheduler batch tests in both native stacks here, where the
coordinator exists. Accept at most 32 reviewed community destinations, keep at
most four `SpaceSneakerShareTask` children active, and leave the remainder in
an ordered cancellable queue with zero reservation, signature, or record before
start. Cover 0/1/4/5/31/32/33 destinations, cancel queued child, cancel all,
active completion starting exactly one next child, active failure starting the
next child, final-handle drop, and late callback suppression.

For every failed child, preserve its prepared lease, review fields, focus, and
original idempotency key through 14:59.999 of idle time and expire them at
exactly 15:00.000. Only a successful lease read or explicit Retry refreshes the
idle clock; screen observation and accessibility announcements do not. Test
retry before expiry, expiry racing retry/cancel, app background/foreground,
process death/startup reconciliation, and per-child expiry without disturbing
active or queued siblings. Process death releases the lease and reconstructs a
safe `Preparation interrupted` review rather than pretending the old timer
survived. Expiry releases reservations and requires fresh preparation; it never
silently posts or duplicates a prior unknown commit.

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
Directory only and rejects a 33rd reviewed destination before creating any
child. Generate one 128-bit idempotency key per space. Start at most four child
tasks and keep the rest in stable review order in a native cancellable queue;
queued rows own no core reservation or signing authority until promoted. Each
child atomically commits its carrier and Directory binding, reporting
`Shared`, `Needs retry`, or `No longer allowed`. Retry reuses the same key and
cannot duplicate. One community failure never rolls back another.

A failed row retains the prepared lease and review state only for its visible
15-minute idle lease. Show the remaining lifetime, release it exactly on expiry,
and route post-expiry Retry through fresh preparation while preserving safe
review text. Cancelling a queued row or dropping the batch never starts it;
dropping an active child uses the shared idempotent task-drop contract.

An organizer enabling Directory returns to the preserved share selection and
destination row; a member sees `Ask an organizer to turn on Sneaker Directory.`

The multi-community native suites inject the Task 6/7 accessibility announcer
per child space rather than sharing one global timer. Each row announces its
own stage start, determinate updates only after both a new 10% bucket and ten
active seconds, and exactly one `Shared`, `Needs retry`, `No longer allowed`, cancelled, or
failed terminal result. Retrying one child starts a new stage without replaying
sibling announcements. Destination rows repeat narrow reflow, visible focus,
high-contrast, and Reduce Motion checks.

- [ ] **Step 4: Add the carrier attachment union to the landed Newswire core**

Create the design-approved `PublicAttachmentRefV1` in the P0-B Newswire model,
add its same-space carrier member, and construct an ordinary `NewsPostV1` whose editable
headline defaults to collection title. Verify same-space and same-signer
resolution. Editorial actions affect the post projection, never site entries
or an independent Directory listing. Unknown attachment variants reject
canonically and legacy attachment-free Newswire bytes remain unchanged.

Extend the closed per-space destination union with
`NewswireAttachment { headline, body }`, allow at most one Newswire
and one app binding in the same atomic carrier transaction, extend the UniFFI
DTO, regenerate bindings, and rerun carrier/social/Newswire cross-space and
same-signer tests. Extend the native destination picker with independent
Newswire and Directory choices; selecting both in one community reuses one
carrier. Screen-reader tests distinguish both controls and their independent
selected states. No Newswire type is referenced by Tasks 1–11.

Add `shareNewswireAndDirectory_sameCommunityAtomic` in Rust, iOS, and Android.
It selects both destinations, proves one carrier plus both references become
visible in one transaction, injects failure before each carrier/Newswire/
binding write and at commit to prove zero visible orphan records, then replays
the original idempotency key after an unknown commit result and receives the
same receipt with no duplicate post, listing, or carrier. A changed combined
request under that key must return `IDEMPOTENCY_CONFLICT` without mutation.

Expose the landed Open Wire projection through `newswire_ffi.rs` as bounded,
space-session-scoped pages/watches with safe post fields and an optional opaque
attachment handle. Add native global/community Open Wire routes on iOS and
Android. A remote imported post with a verified carrier attachment renders its
ordinary headline/body/editorial state plus `NewswireAttachmentRow`/
`NewswireAttachmentCard`, which adapts the attachment handle into the same
native carrier-card state model built in Task 11. It never auto-downloads:
`Get collection` is the only request action, uses `request_space_blob`, and
then `Open` passes the lease to `begin_snk_open_from_blob`. Invalid, hidden,
corrected, tombstoned, foreign-space, foreign-signer, and unavailable-holder
states follow the Newswire projection and carrier-card contracts without
revealing raw IDs by default.

`SneakerNewswireAttachmentTests` on both platforms import the same signed
remote Newswire fixture into a second profile, observe it in Open Wire, prove
zero request before a genuine Get gesture, download through the scoped carrier,
open a named page, and exercise edit/hide/tombstone, blocked-all, invalid,
retry, and focus/accessibility states. This is the receiver half of the
Newswire route; sender-only destination tests are insufficient.

Those Newswire receiver tests use the same accessibility announcer and cadence
for not-downloaded, waiting-for-holder, transfer, verification, Retry, failure,
and ready-to-open. The attachment card and per-community Newswire destination
row repeat narrow reflow, visible focus, high-contrast, and Reduce Motion
assertions without announcing hidden identifiers.

- [ ] **Step 5: Verify and commit the available destination set**

```sh
node scripts/apps/miniapp-contracts.mjs
npm ci --prefix scripts/apps
npm --prefix scripts/apps exec -- playwright test \
  --config scripts/apps/playwright.config.mjs --grep 'sneaker-directory'
cargo test -p riot-core --all-features --test sneakerweb_share_matrix
cargo test -p riot-core --all-features
cargo test -p riot-ffi --test sneakerweb_contract
cargo test -p riot-ffi --test newswire_contract
scripts/conference/build-native-core.sh
scripts/conference/test-native-core-package.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SneakerDirectoryPermissionTests \
  -only-testing:RiotTests/SneakerShareMatrixTests \
  -only-testing:RiotTests/SneakerNewswireAttachmentTests
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotUITests/SneakerDirectoryPermissionUITests
(cd apps/android && ./gradlew testDebugUnitTest \
  --tests '*SneakerDirectoryPermissionTest' \
  --tests '*SneakerShareMatrixTest' \
  --tests '*SneakerNewswireAttachmentTest' \
  connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.sneakerweb.SneakerCommunityShareTest)
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS'
scripts/coverage-gate.sh
```

```sh
git add fixtures/apps/sneaker-directory fixtures/apps/sneaker-directory.bundle.cbor \
  fixtures/apps/sneaker-directory.manifest.cbor crates/riot-core/src/apps/starter.rs \
  crates/riot-core/tests/apps_starter.rs crates/riot-core/tests/sneakerweb_share_matrix.rs \
  crates/riot-core/src/sneakerweb/social.rs crates/riot-core/src/sneakerweb/tasks.rs \
  crates/riot-ffi/src/lib.rs crates/riot-ffi/src/sneakerweb_ffi.rs \
  crates/riot-ffi/src/newswire_ffi.rs crates/riot-ffi/tests/sneakerweb_contract.rs \
  crates/riot-ffi/tests/newswire_contract.rs \
  scripts/apps \
  apps/ios/Riot/SneakerWeb apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/Newswire \
  apps/ios/Riot/ConferenceShellView.swift apps/ios/RiotTests/SneakerDirectoryPermissionTests.swift \
  apps/ios/RiotTests/SneakerShareMatrixTests.swift \
  apps/ios/RiotTests/SneakerNewswireAttachmentTests.swift \
  apps/ios/RiotUITests/SneakerDirectoryPermissionUITests.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/android/app/src/main/kotlin/org/riot/evidence/sneakerweb \
  apps/android/app/src/main/kotlin/org/riot/evidence/newswire \
  apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerDirectoryPermissionTest.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerShareMatrixTest.kt \
  apps/android/app/src/test/kotlin/org/riot/evidence/sneakerweb/SneakerNewswireAttachmentTest.kt \
  apps/android/app/src/androidTest/kotlin/org/riot/evidence/sneakerweb/SneakerCommunityShareTest.kt
git add crates/riot-core/src/newswire crates/riot-core/tests/newswire_*.rs
git commit -m "feat(sneakerweb): share collections across public communities"
```

Task 12 cannot commit with a missing or skipped Newswire path; P0-B and all 12
matrix cells are blocking dependencies.

## Task 13: Enforce coverage, platform freshness, interoperability, and field evidence

**Files:**
- Read: `.coverage-thresholds.json`
- Read: `scripts/{coverage-gate,test-coverage-gate}.sh`
- Create: `scripts/verify-xcresult-tests.sh`
- Create: `scripts/test-verify-xcresult-tests.sh`
- Create: `scripts/android-sneakerweb-test-gate.sh`
- Create: `scripts/test-android-sneakerweb-test-gate.sh`
- Create: `scripts/sneakerweb-physical-rehearsal.sh`
- Create: `scripts/test-sneakerweb-physical-rehearsal.sh`
- Modify: `docs/product/product-brief.md`
- Modify: `README.md`
- Modify: `SERVICE-INVENTORY.md`
- Create: `docs/quality/2026-07-13-sneakerweb-release-evidence.md`

- [ ] **Step 1: Write RED native/rehearsal gate self-tests**

Task 0 already froze and exercised the combined coverage wrapper before any
production work; rerun its self-test and real gate here. The iOS helper has a
two-phase `prepare`/`verify` contract: `prepare` deletes the named result bundle,
creates an unguessable run token plus monotonic start record outside that
bundle, and returns the token; only then may `xcodebuild` run. `verify` requires
that token, consumes it once, and rejects stale, pre-start, missing, malformed,
zero-test, ignored, or skipped required results. Tests cover token replay,
wrong bundle/token, clock equality, failed build with no result, and an old
bundle planted between prepare and build. The Android runner owns the same
delete/start/run/verify sequence around Gradle. Require every named SneakerWeb
suite.

The iOS core result must positively execute `SneakerWebCoreTests`,
`SneakerWebInteropActivationTests`,
`SneakerLibraryViewModelTests`, `SneakerAccessibilityContractTests`,
`SneakerDiagnosticExportTests`,
`SneakerWebViewIsolationTests`,
`SneakerShareTaskTests`, `SneakerNearbyTransferTests`,
`SneakerCarrierCardTests`, `SneakerSpaceBlobMultiPeerTests`,
`AppSyncReplicationTests`, `AppRepositoryTests`, `AppRuntimeHostTests`,
`DirectoryRepositoryTests`, `DirectoryStorefrontTests`, `SneakerDirectoryPermissionTests`,
`SneakerShareMatrixTests`, and `SneakerNewswireAttachmentTests`; the UI result
must execute `SneakerDocumentUITests`, `SneakerNearbyFlowUITests`, and
`SneakerDirectoryPermissionUITests`, plus
`SneakerSystemShareReachabilityTests` in its configured native test target.
Android unit results must execute
`SneakerLibraryViewModelTest`, `SneakerAccessibilityContractTest`,
`SneakerDiagnosticExportTest`,
`SneakerShareCoordinatorTest`,
`SneakerNearbyTransferTest`, `SneakerCarrierCardTest`,
`SneakerSpaceBlobMultiPeerTest`,
`AppBundleCodecTest`, `RiotJsBridgeTest`, `InstalledAppsStoreTest`,
`DirectoryControllerTest`,
`SneakerDirectoryPermissionTest`, `SneakerShareMatrixTest`, and
`SneakerNewswireAttachmentTest`; connected
results must execute `SneakerWebInteropActivationTest` and
`SneakerSystemShareReachabilityTest`,
`SneakerDocumentOpenTest`, `SneakerWebViewIsolationTest`, and
`SneakerNearbyFlowTest`, `SneakerCommunityShareTest`, and
`AppRuntimeEndToEndTest`, and `AppPersistenceRestartTest`. The macOS gate must freshly test `RiotKit-macOS` and
build `Riot-macOS` while confirming no `.snk` declaration or SneakerWeb route
appears in the product.

The physical-rehearsal gate self-test rejects missing or same-platform-only
system-file evidence. It requires distinct `ios_to_android_system_file` and
`android_to_ios_system_file` records, each with sender export digest, receiver
open digest, exact full-domain-set digest, named-page open, device/OS, and
fresh timestamp.

- [ ] **Step 2: Reconfirm the frozen combined coverage command**

Fail if the source-of-truth enforcement command is no longer:

```json
{
  "enforcement": {
    "command": "scripts/coverage-gate.sh",
    "blockPRCreation": true,
    "blockTaskCompletion": true
  }
}
```

`scripts/test-coverage-gate.sh` must still prove that the script reads all four
100% thresholds, runs tarpaulin for its supported metrics and llvm-cov for
lines/functions/regions/branches, and reports the statements-to-regions mapping
explicitly. Task 13 does not introduce or repair this infrastructure after the
feature code; any drift is a release failure.

- [ ] **Step 3: Run the full blocking automated matrix**

```sh
cargo xtask validate-contracts
cargo test --workspace --all-features
scripts/test-coverage-gate.sh
scripts/coverage-gate.sh
scripts/test-verify-xcresult-tests.sh
scripts/test-android-sneakerweb-test-gate.sh
scripts/test-sneakerweb-physical-rehearsal.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
scripts/conference/build-native-core.sh
scripts/conference/test-native-core-package.sh
IOS_CORE_RUN=$(scripts/verify-xcresult-tests.sh prepare build/snk-riotkit.xcresult)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -enableCodeCoverage YES -resultBundlePath build/snk-riotkit.xcresult
IOS_UI_RUN=$(scripts/verify-xcresult-tests.sh prepare build/snk-riot.xcresult)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -enableCodeCoverage YES -resultBundlePath build/snk-riot.xcresult
MAC_CORE_RUN=$(scripts/verify-xcresult-tests.sh prepare build/snk-riotkit-macos.xcresult)
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS' -resultBundlePath build/snk-riotkit-macos.xcresult
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS'
scripts/verify-xcresult-tests.sh verify build/snk-riotkit.xcresult \
  --run-token "$IOS_CORE_RUN" --require-tests \
  --require-suite SneakerWebCoreTests \
  --require-suite SneakerWebInteropActivationTests \
  --require-suite SneakerLibraryViewModelTests \
  --require-suite SneakerAccessibilityContractTests \
  --require-suite SneakerDiagnosticExportTests \
  --require-suite SneakerWebViewIsolationTests \
  --require-suite SneakerShareTaskTests \
  --require-suite SneakerNearbyTransferTests \
  --require-suite SneakerCarrierCardTests \
  --require-suite SneakerSpaceBlobMultiPeerTests \
  --require-suite AppSyncReplicationTests \
  --require-suite AppRepositoryTests \
  --require-suite AppRuntimeHostTests \
  --require-suite DirectoryRepositoryTests \
  --require-suite DirectoryStorefrontTests \
  --require-suite SneakerDirectoryPermissionTests \
  --require-suite SneakerShareMatrixTests \
  --require-suite SneakerNewswireAttachmentTests \
  --require-suite SneakerSystemShareReachabilityTests
scripts/verify-xcresult-tests.sh verify build/snk-riot.xcresult \
  --run-token "$IOS_UI_RUN" --require-tests \
  --require-suite SneakerDocumentUITests \
  --require-suite SneakerNearbyFlowUITests \
  --require-suite SneakerDirectoryPermissionUITests
scripts/verify-xcresult-tests.sh verify build/snk-riotkit-macos.xcresult \
  --run-token "$MAC_CORE_RUN" --require-tests
! /usr/libexec/PlistBuddy -c 'Print :CFBundleDocumentTypes' apps/macos/Riot/Info.plist 2>/dev/null | rg -i 'snk|sneaker'
! rg -n 'SneakerLibraryView|SneakerReaderView|\.snk' apps/macos/Riot
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

In addition to the cohort totals, `scripts/sneakerweb-physical-rehearsal.sh`
emits one evidence row for every source/destination cell in the mandatory 3×4
matrix. Each row records source constructor, selected full-domain-set digest,
destination, sender result, receiver/projection result, resolved full-domain-set
digest, blocked exclusions, retry/idempotency result, device/OS, and timestamp.
All 12 rows must be present and digest-equal before the gate can pass.

The system-file column additionally requires two physical cross-client
handoffs. iOS exports the standard `.snk` through its real system share sheet
to a Files/provider artifact that Android receives through its declared
VIEW/OPENABLE intent; Android exports through its real share sheet to a
provider artifact that iOS receives through its registered document-open path.
Each direction uses a fresh 100 MiB multi-site fixture, records the exported
file digest and frozen full-domain-set digest, proves the receiver creates the
expected Received record, opens a named page offline, and preserves exact
domains/signatures/payload digests. Simulator-only, seam-only, same-platform,
or one-direction evidence cannot satisfy this gate.

The nearby rows additionally require four live cross-client transcripts:
iOS sender → Android receiver and Android sender → iOS receiver over preferred
local TCP, then both directions with local TCP disabled so BLE fallback is
actually used. Each transcript begins with fresh bilateral confirmation,
transfers the 100 MiB multi-site fixture, verifies the receiver lease digest,
opens a named page, and records the canonical envelope version. Same-platform
or one-direction-only runs cannot satisfy this gate.

- [ ] **Step 5: Update delivered docs and commit evidence**

Document the narrow user-opened fixed-public-namespace exception to
preview-before-ingest, `Content is intact` versus trust, public sharing,
inspectable full IDs, block/removal behavior, and only the routes that passed.
Update the service inventory with the new core modules, FFI task objects,
native hosts, and Directory app.

```sh
git add scripts/verify-xcresult-tests.sh scripts/test-verify-xcresult-tests.sh \
  scripts/android-sneakerweb-test-gate.sh scripts/test-android-sneakerweb-test-gate.sh \
  scripts/sneakerweb-physical-rehearsal.sh scripts/test-sneakerweb-physical-rehearsal.sh \
  docs/product/product-brief.md README.md \
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
| Confirmed redacted diagnostic export and ordinary-log boundary | 4–7, 13 |
| Select/share one, many, or a Received collection | 4, 9 |
| Standard system file sharing and bidirectional physical handoff | 9, 13 |
| Direct nearby sharing without a shared community | 10 |
| On-demand public-space blobs and carrier attribution | 4, 10, 11 |
| Ordinary signed Sneaker Directory miniapp | 11, 12 |
| Newswire attachment using ordinary editorial semantics | 12, 13 dependency gate |
| Multiple public communities and duplicate-free retry | 11, 12 |
| App cannot access bytes/raw IDs/global library/cross-space data | 11, 12 |
| Block revokes reads/exports/direct and carrier transfers | 3, 4, 8–11 |
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

## Plan Review Gate: ESCALATION REQUIRED (3/3 iterations exhausted)

### Remaining blocking issues

#### Feasibility

- Task 1's bare Android-target Cargo commands do not supply the NDK `CC_*` and
  `AR_*` environment required by `libsqlite3-sys`; P0-A's native build script
  supplies those variables, while `.cargo/config.toml` supplies only Rust
  linkers.
- Xcode 26.2 WebKit does not expose a dedicated per-`WKWebView` process, a
  per-view 64 MiB memory cap, or reliable process termination. Task 8 therefore
  cannot prove the specified iOS preview-process isolation with its selected
  API.

#### Completeness

- Task 12 initially filters community candidates by Directory availability and
  does not explicitly include/test a writable public Newswire-only community.
- Task 11 does not compare the signed carrier's ordered domain set with the
  completed valid `.snk` domain set before decode/open.
- Tasks 6/7 omit the exact accessibility collision behavior: deterministic
  full-key phrase, visual phrase only for duplicate titles, and stable
  `item N of M` fallback when both title and phrase collide.
- Task 13 does not require the 100 MiB performance fixture to contain 1,000
  entries, report median and worst case, or record the UX/transport revision
  required after a cohort threshold failure.

#### Scope & Alignment

- None; the final scope reviewer passed.

### Iteration history

| Iteration | Feasibility | Completeness | Scope & Alignment |
| --- | --- | --- | --- |
| 1 | PASS | FAIL | FAIL |
| 2 | FAIL | FAIL | PASS |
| 3 | FAIL | FAIL | PASS |

### User decision required

1. **Override** — proceed as-is and accept the six remaining issues as known risks.
2. **Revise** — authorize a new manual revision/review cycle.
3. **Simplify** — reduce scope to remove the disputed contracts.
4. **Cancel** — abandon this plan and start fresh.
