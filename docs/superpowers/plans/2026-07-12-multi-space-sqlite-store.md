# Multi-Space SQLite Store Production-Cutover Plan

> **Execution:** Use `metaswarm:orchestrated-execution` task by task. Every task
> starts with a focused failing test, records RED, implements the minimum
> production behavior, records GREEN, and receives adversarial review before
> commit.

**Goal:** Make Rust-owned SQLite the only production authority for Riot's iOS
conference build's
spaces, Willow accepted/live state, app approvals, and app documents so two
namespaces survive termination and nearby sync without cross-space leakage.

**Architecture:** Preserve the existing verified inspect/plan/commit semantics,
but extract their storage operations behind a private backend boundary.
`SqliteEvidenceStore` implements that boundary with one atomic transaction for
accepted entries, the Willow live join, receipts, and document projections.
`MemoryEvidenceStore` remains only under test for differential checks. Opaque
namespace-bound sessions cross UniFFI. Swift becomes a client and stops replaying
JSON receipt arrays.

**Tech stack:** Rust 2021, pinned `rusqlite =0.40.1` with bundled SQLite,
Willow'25, UniFFI 0.32, Swift/XCTest, Keychain wrapping keys.

**Design:** `docs/superpowers/specs/2026-07-12-multi-space-sqlite-store-design.md`

**Stop line:** This plan delivers the approved conference SQLite store slice.
Legacy-container migration, full Meadowcap delegation UI, all four starter-app
UIs, Android/macOS runtime adoption, physical-phone rehearsal, and TestFlight
delivery remain dependent slices.

## Production invariants

- The iOS release construction has no in-memory or Swift JSON-replay backend
  option; deprecated cross-platform bindings remain only for deferred clients and
  are not called by the iOS target.
- Every production read/write/import is bound to an immutable namespace handle.
- A verified write/import changes accepted state, live state, receipts, and any
  JSON projection in one SQLite transaction or changes none of them.
- App approval and its generation are namespace-local and durable.
- Namespace comes from verified Willow bytes, never a caller-supplied routing
  string. Mixed-namespace bundles are split before commit or rejected atomically.
- Database, WAL, and SHM files receive iOS data protection before use.
- Corrupt/unreadable storage returns a typed recovery state; it never becomes a
  silent empty database.

## Task 1: Prove the pinned SQLite dependency and Apple linking

**Files:** `Cargo.toml`, `Cargo.lock`, `crates/riot-core/Cargo.toml`,
`crates/riot-core/src/lib.rs`, `crates/riot-core/src/database/mod.rs`,
`crates/riot-core/tests/sqlite_platform.rs`,
`scripts/conference/build-native-core.sh`,
`scripts/conference/test-native-core-package.sh`.

- [ ] Add `tempfile.workspace = true` to riot-core dev dependencies and write
  `sqlite_platform.rs` first. It imports the missing database API and asserts
  JSON extraction plus absence of `OMIT_JSON`.
- [ ] RED: `cargo test -p riot-core --test sqlite_platform -- --nocapture` must
  fail because the API does not exist.
- [ ] Pin `rusqlite = { version = "=0.40.1", default-features = false,
  features = ["bundled", "backup", "blob", "hooks", "limits", "serde_json"] }`.
  Implement only the compile-option/JSON proof API.
- [ ] GREEN: run the focused test, locked iOS arm64 and simulator builds, then
  both conference native-core scripts. Update those scripts only if the link
  proof needs an explicit SQLite symbol/load assertion.
- [ ] Verify `cargo-tarpaulin` and `cargo-llvm-cov` exist; install their locked
  releases if absent. Commit only Task 1 files.

## Task 2: Open a protected, recoverable V1 database

**Files:** `crates/riot-core/src/database/{mod.rs,schema.rs,worker.rs}`,
`crates/riot-core/tests/sqlite_open.rs`,
`crates/riot-ffi/src/database_ffi.rs`, `crates/riot-ffi/src/lib.rs`,
`crates/riot-ffi/tests/database_open_contract.rs`,
`apps/ios/Riot/Core/RiotDatabaseFile.swift`,
`apps/ios/RiotTests/RiotDatabaseFileTests.swift`,
`apps/ios/Riot.xcodeproj/project.pbxproj`.

- [ ] Write Rust RED tests for first open, reopen, ordered migration, concurrent
  clone use, corrupt non-SQLite bytes, unwritable path, and a failed migration
  leaving the prior schema intact. Write Swift RED for protection attributes on
  the database plus `-wal`/`-shm` sidecars.
- [ ] RED: run `cargo test -p riot-core --test sqlite_open`,
  `cargo test -p riot-ffi --test database_open_contract`, and the focused XCTest.
- [ ] Implement static migrations on one named thread-confined writer connection
  with `foreign_keys=ON`, WAL, `synchronous=FULL`, 2500 ms busy timeout, bounded
  read snapshots, `quick_check`, controlled checkpoints, and closed open errors:
  `CorruptDatabase`, `MigrationRequired`, `BusyRetryable`, `StorageFull`,
  `KeyUnavailable`, `Internal`.
- [ ] Use a two-phase open handshake: Rust creates/migrates the main database in
  rollback-journal mode and closes it without application mutation; Swift applies
  `.completeUntilFirstUserAuthentication`; Rust starts the worker, enters WAL,
  materializes the sidecars, and keeps that connection alive but command-blocked.
  Swift protects and verifies all three live files, then acknowledges the
  path/inode-bound phase token; only that acknowledgement unlocks application
  commands. Cancel/error closes without mutation. Test this exact bundled-SQLite
  lifecycle on simulator because an unheld WAL/SHM may disappear. Never delete
  on failure.
- [ ] GREEN: repeat all three focused test commands and a 25-iteration concurrent
  open/write/reopen stress test. Commit Task 2 files.

## Task 3: Persist space registry, identity references, and selection

**Files:** `crates/riot-core/src/database/{mod.rs,schema.rs,spaces.rs}`,
`crates/riot-core/tests/sqlite_spaces.rs`.

- [ ] Write RED tests that register a created space and a joined/communal space,
  select/switch/archive/unarchive them, terminate all handles, reopen, and retain
  exact 32-byte IDs, relationship, protocol kind, key availability, and fallback
  selection. Include malformed ID/title and unknown-space failures.
- [ ] RED: `cargo test -p riot-core --test sqlite_spaces`.
- [ ] Add `spaces`, `space_identities`, `namespace_roots`, `sealed_signers`, and
  `local_state` with STRICT tables, width/FK/check constraints, opaque secure
  storage references, signer role/lifecycle, and monotonic session generation.
  Never store plaintext signer or wrapping-key bytes.
- [ ] Implement bounded stable `SpacePage`, create/register/open/archive APIs,
  and selected-space fallback without replaying entries.
- [ ] GREEN: focused tests plus reopen loop. Commit Task 3 files.

## Task 4: Cut the Willow accepted/live store to SQLite

**Files:** `crates/riot-core/src/session.rs`,
`crates/riot-core/src/import/join.rs`,
`crates/riot-core/src/database/{mod.rs,schema.rs,evidence.rs,queries.rs}`,
`crates/riot-core/tests/sqlite_evidence.rs`,
`crates/riot-core/tests/sqlite_memory_differential.rs`,
`crates/riot-core/Cargo.toml`, `crates/riot-ffi/src/mobile_state.rs`, and
`crates/riot-app-cli/{Cargo.toml,src/lib.rs,tests/cli_pack.rs}`.

- [ ] Move verification and join/pruning calculations into pure functions first;
  characterization tests must prove unchanged inspect/plan/commit outcomes.
- [ ] Write RED tests named
  `live_and_seen_match_memory_store_after_pruning_and_forgetting`, and
  `same_coordinates_are_isolated_by_namespace`. Fixtures use a new shared
  `#[cfg(feature = "conformance")]` helper; declare these integration tests with
  `required-features = ["conformance"]` and run them with `--all-features`.
- [ ] RED: `cargo test -p riot-core --all-features --test sqlite_evidence` and
  `cargo test -p riot-core --all-features --test sqlite_memory_differential`.
- [ ] Add namespace-first `capabilities`, `accepted_entries`, `live_entries`,
  `entry_path_prefixes`, `import_receipts`, and `forgotten_entries`. Store u64
  timestamps as sortable big-endian bytes, exact canonical entry/auth/payload
  bytes, source route, disposition, and permanent seen identity. Capabilities
  retain canonical bytes, fingerprint, namespace, receiver, mode, area, lineage,
  and policy state with receiver/fingerprint indexes. Enforce payload, path,
  receipt, and store ceilings before mutation.
- [ ] Implement `SqliteEvidenceStore` behind the existing private
  inspect/plan/commit contract. Commit selected verified entries, recency/pruning,
  prefix rows, capability lineage/policy, receipts, and forget/restore state in
  one transaction. Revalidate capability namespace, receiver, lineage,
  revocation/policy, and area before every commit. Exact duplicates are
  idempotent; mixed namespaces cannot partly commit. Add rollback, namespace
  isolation, archive/reopen, revoked-lineage, and transplanted-capability tests.
  Accepted identity and immutable receipts survive pruning, forgetting, and
  restart. Reimporting a forgotten non-pruned entry clears the marker and records
  `RestoredAfterForget`; an ordinary live duplicate records `AlreadyPresent`;
  protocol-pruned entries never resurrect on duplicate import.
- [ ] Add an explicit SQLite constructor taking database handle plus namespace
  while preserving the inspect/plan/commit handle shape. Convert the CLI in this
  task. Do **not** mirror commits between backends. Existing pathless FFI profile
  construction remains temporarily on its single memory backend so the workspace
  compiles until Task 7 can replace constructor and caller atomically; it is not
  a completed/releasable configuration. Keep affected integration tests on the
  existing oracle during this transition. Task 7 introduces database ownership
  and path injection; Task 8 converts every iOS caller while retaining deprecated
  compatibility symbols only for explicitly deferred platforms.
- [ ] Convert CLI bundle inspection and `cli_pack` to the explicit SQLite-backed
  constructor too. The CLI first verifies/decodes the bundle, derives its exact
  namespace from canonical entry bytes, creates a `tempfile` database scoped to
  that invocation, inspects through the same SQLite admission path, and drops it
  on exit. It never accepts a caller-supplied namespace and never exposes or
  selects the conformance memory oracle.
- [ ] GREEN: characterization, both focused suites, existing import tests, and
  25 differential randomized sequences covering insert, recency, pruning,
  prefixes, payload lookup, forgetting, and reopen. Commit Task 4 files.

## Task 4A: Implement namespace signer and Keychain lifecycle

**Files:** `crates/riot-core/src/database/{mod.rs,signer.rs}`,
`crates/riot-core/src/willow/identity.rs`,
`crates/riot-core/src/apps/{bridge.rs,index.rs,trust.rs,endorse.rs}`,
`crates/riot-core/src/profile/resolver.rs`,
`crates/riot-core/tests/sqlite_signers.rs`,
`crates/riot-ffi/src/{database_ffi.rs,mobile_state.rs}`,
`crates/riot-ffi/tests/database_signer_contract.rs`,
`apps/ios/Riot/Core/WrappingKeyStore.swift`,
`apps/ios/RiotTests/WrappingKeyStoreTests.swift`,
`apps/ios/Riot.xcodeproj/project.pbxproj`.

- [ ] Write RED tests for created-space signer generation, joined-space signer
  import, reopen/sign, wrong or missing Keychain item, envelope transplant across
  namespace/role, interrupted Keychain-first and SQLite-first staging, rollback,
  archive/reopen, and byte-for-byte zeroization hooks after sign/failure.
- [ ] RED: run `cargo test -p riot-core --all-features --test sqlite_signers`,
  `cargo test -p riot-ffi --all-features --test database_signer_contract`, and
  focused `WrappingKeyStoreTests`.
- [ ] Store only a versioned XChaCha20-Poly1305 sealed seed and opaque Keychain
  reference. AAD binds envelope version, namespace ID, public key, signer role,
  and lifecycle operation. Implement prepare/commit/abort states and launch-time
  reconciliation so neither system can claim a usable signer alone.
- [ ] Wrapping bytes cross FFI only for one unwrap/seal/sign call, live in a
  zeroizing Rust buffer, and are overwritten in Swift immediately after return.
  Validate derived public key before sign and fail closed on any mismatch.
- [ ] The per-write flow is explicit: native code loads the opaque signer
  reference from secure storage into a mutable byte buffer, calls a sensitive
  UniFFI method such as `put_document(..., wrapping_key)` exactly once, and
  overwrites its buffer in unconditional cleanup. UniFFI immediately moves its
  copy into `Zeroizing<Vec<u8>>`; Rust unwraps/signs and drops it before return.
  `DatabaseSession` retains only the secure-storage reference, never key bytes or
  a key-bearing callback. Test wrong/missing keys and cleanup on every exit.
- [ ] GREEN: all focused suites plus failure-injection reopen loop. Commit. Tasks
  5 and 6 must use this API and cannot construct raw secret keys in production.

## Task 5: Persist namespace-local app packages and approvals

**Files:** `crates/riot-core/src/database/{mod.rs,schema.rs,apps.rs}`,
`crates/riot-core/src/apps/{bundle.rs,index.rs,trust.rs}`,
`crates/riot-core/tests/sqlite_app_state.rs`.

- [ ] Write RED for global content-addressed package caching but independent
  per-space availability, approval version, revoke, reopen, and stale app session
  after approval-generation change.
- [ ] RED: `cargo test -p riot-core --all-features --test sqlite_app_state`.
- [ ] Add immutable `app_packages` and namespace-first `space_app_state`. Package
  hashes are immutable; approval/revocation increments its generation in the
  transaction that appends the change.
- [ ] Implement bounded list/approve/revoke/open-app operations. `AppSession`
  contains database generation, namespace, app ID, receiver/subspace, and
  approval generation; every operation rechecks them transactionally.
- [ ] GREEN: focused tests and restart loop. Commit.

## Task 6: Make signed app documents an atomic projection

**Files:** `crates/riot-core/src/database/{mod.rs,schema.rs,evidence.rs,documents.rs,changes.rs}`,
`crates/riot-core/src/apps/{entry.rs,bridge.rs}`, `crates/riot-core/src/session.rs`,
`crates/riot-core/tests/sqlite_documents.rs`.

- [ ] Write RED for the same app/collection/document in two namespaces, signed
  local put/reopen, two authoring subspaces using the same namespace/app/
  collection/document without row collapse, deterministic resolver behavior,
  imported put/reopen, stale-sequence conflict, invalid/large
  JSON, rollback injection (including
  `failed_import_rolls_back_entry_and_projection`), prefix paging, change-feed
  reset, pruning/forget/restore projection lifecycle, and a watch canceled by
  close/switch. Direct rows without a live `source_entry_id` must fail their FK.
- [ ] RED: `cargo test -p riot-core --all-features --test sqlite_documents`.
- [ ] Add `documents` and `change_log`, namespace-first keys, live-entry FK,
  author subspace as part of full document identity, sequence, bounded canonical
  JSON text, indexed collection/prefix queries, and bounded retention. Resolver
  ordering follows Willow recency/tie rules without deleting losing authors.
- [ ] Change app-data local writes to build/sign canonical Willow bytes first,
  run ordinary verification, then update accepted/live/receipt/projection/change
  rows inside that one commit. Imported app-data follows the same projector.
  Remove any API that writes a free-standing authoritative JSON document.
- [ ] In the same evidence transaction, Willow pruning or local forgetting
  removes affected projections and appends change records; restore rebuilds the
  projection from retained/reimported canonical payload and appends a distinct
  restore record. Rolled-back prune/forget/restore produces no visible change or
  watch event.
- [ ] Implement `Any`, `IfAbsent`, `IfSequence`, `DocumentPage`, and resumable
  `ChangePage` with opaque query-bound cursors and `reset_required`.
- [ ] Implement a cancelable filtered watch over the durable change sequence.
  Its token is bound to database generation, namespace, app, collection, prefix,
  and approval generation; close, space switch, approval revoke, logout, and
  database shutdown wake it once with `SessionStale`/`ObjectClosed` and release
  its worker resources. Test post-commit notification only—rolled-back rows must
  never wake a watcher.
- [ ] GREEN: focused suite plus existing app-data import/list tests. Commit.

## Task 7: Expose the complete bounded native contract

**Files:** `crates/riot-core/{Cargo.toml,src/session.rs}`,
`crates/riot-ffi/src/{lib.rs,database_ffi.rs,mobile_api.rs,mobile_state.rs,apps_ffi.rs,demo_ffi.rs,profile_ffi.rs}`,
`crates/riot-ffi/tests/database_contract.rs`.

- [ ] Write an app-facing RED contract that opens a database, creates/registers
  two spaces, opens immutable sessions, approves one app per space, writes
  distinct documents, closes every handle, reopens, and observes exact isolation.
  Add archive/unarchive, stale-session, closed error mapping, bounded pages,
  change reset, join/import, watch filter/cancel/invalidation, and every recovery
  state plus its fixed recovery class.
- [ ] RED: `cargo test -p riot-ffi --all-features --test database_contract`.
- [ ] Expose versioned DTOs and opaque `DatabaseSession`, `SpaceSession`, and
  `AppSession`. `DatabaseSession.open(path)` owns the worker and is
  the required parent for create/open/import operations; it routes every profile
  or space handle by full namespace. Sensitive signer-backed methods accept one
  transient wrapping-key byte array. IDs cross as exact bytes. The new
  multi-space APIs delegate to bound SQLite sessions. Keep the old pathless
  exports deprecated and unchanged so deferred Android/macOS clients and their
  generated bindings still compile; do not mirror either backend. Task 8 removes
  every use of those exports from the iOS release target without deleting the
  cross-platform compatibility symbols.
  Expose `ChangeWatch.cancel()` with exactly-once terminal delivery. Map every
  closed error to `retry`, `unlock`, `reopen`, `review`, or `support/export`; raw
  SQLite/Willow/crypto errors never cross FFI.
- [ ] Regenerate bindings and GREEN with the focused FFI suite plus
  `cargo xtask validate-contracts`. Commit.

## Task 8: Cut Swift startup, navigation, and app bridges off JSON replay

**Files:** `apps/ios/Riot/Core/ProfileRepository.swift`,
`apps/ios/Riot/Core/RiotDatabaseRepository.swift`,
`apps/ios/Riot/AppModel.swift`, `apps/ios/Riot/ConferenceShellView.swift`,
`apps/ios/Riot/Demo/DemoMode.swift`,
`apps/ios/Riot/Apps/AppBridgeController.swift`,
`apps/ios/Riot/Apps/AppRuntimeView.swift`,
`apps/ios/RiotTests/RiotDatabaseRepositoryTests.swift`,
`apps/ios/RiotTests/AppBridgeNamespaceTests.swift`,
`apps/ios/RiotTests/AppRepositoryTests.swift`,
`apps/ios/RiotTests/BindingSemanticsTests.swift`,
`apps/ios/RiotTests/LocalNetworkNearbyTests.swift`,
`apps/ios/RiotTests/AppSyncReplicationTests.swift`,
`apps/ios/RiotTests/DemoModeTests.swift`,
`apps/ios/RiotTests/AppRuntimeHostTests.swift`,
`apps/ios/Riot.xcodeproj/project.pbxproj`.

- [ ] Write Swift RED tests through actual startup: two spaces with separate
  approvals/documents survive repository deallocation and reopen; switching does
  not replay bundles; an old WebView session cannot write after switch/revoke;
  corrupt database shows recovery rather than onboarding/empty state; filtered
  watches deliver only after commit and stop on switch/revoke/logout/closure;
  every closed native error renders the fixed recovery action.
- [ ] RED: run those focused XCTest classes on the configured simulator.
- [ ] Make `RiotDatabaseRepository` the production repository. Delete
  `PersistedProfile.appDataBundles`, trusted-app/carried-app authority, and their
  startup replay loops. Keep only an explicit read-only legacy source for the
  deferred migration path; it cannot construct the release repository.
- [ ] Derive the protected application-support database path in Swift and pass it
  through the completed protection handshake to `DatabaseSession.open`; Swift
  never invents or passes a namespace during database open.
- [ ] For every signer-backed write, iOS loads its Keychain item into a mutable
  byte buffer, passes it only to that sensitive UniFFI
  call, and overwrites it in unconditional cleanup. No repository/session retains
  wrapping-key bytes.
- [ ] Convert every iOS caller to the new database/session APIs. Keep deprecated
  compatibility exports solely for deferred platform clients, regenerate
  bindings, and add a source/build contract proving the iOS target contains no
  call to pathless profile open or Swift JSON replay. Run affected Rust FFI and
  Swift suites before committing.
- [ ] Rewrite the existing repository, binding-semantics, local-network, and app-
  sync assertions that inspect the old JSON fields/replay behavior so they prove
  the equivalent SQLite-backed restart and namespace-bound behavior instead.
- [ ] Bind navigation and app bridges to an immutable `SpaceSession`/`AppSession`.
  All native calls run off the main actor and typed results return to it. Update
  Xcode file references/build phases.
- [ ] GREEN: focused tests and full RiotKit tests. The existing native packaging
  smoke still proves deferred platform bindings/artifacts compile. Commit.

## Task 9: Route existing nearby transport into namespace transactions

**Files:** `crates/riot-core/src/database/{mod.rs,import.rs}`,
`crates/riot-core/tests/sqlite_transport_import.rs`,
`crates/riot-ffi/src/mobile_state.rs`,
`crates/riot-ffi/tests/database_transport_contract.rs`,
`apps/ios/Riot/Transport/SyncCoordinator.swift`,
`apps/ios/Riot/Transport/NearbyTransportController.swift`,
`apps/ios/RiotTests/AppSyncReplicationTests.swift`.

- [ ] Write RED at core, FFI, and Swift transport boundaries: validated bundles
  for A and B arrive through existing framing, namespace is derived from decoded
  entries, each commits only to its registered namespace, mixed bundles split by
  namespace before independent transactions or reject without partial mutation,
  and both survive reopen.
- [ ] RED: run the three focused suites.
- [ ] Connect confirmed sync acceptance to the database importer; remove the
  callback that commits only to the currently selected `MobileProfile` store.
  An unavailable namespace returns a typed review/import state rather than
  redirecting into the visible space.
- [ ] GREEN: focused suites plus existing CoreBluetooth/Wi-Fi/BLE framing and sync
  tests. Commit.

## Task 10: Conference fixture, performance, coverage, and release proof

**Files:** `crates/riot-core/tests/sqlite_conference_gate.rs`,
`crates/riot-core/benches/sqlite_store.rs`, `SERVICE-INVENTORY.md`,
`docs/decisions/riot-sqlite-store-slice-report.md`.

- [ ] Add a deterministic Release fixture that creates two communal namespaces,
  approves the representative app separately, commits different documents,
  imports signed transport data into both, closes/reopens, and asserts exact
  accepted/live/document/approval counts with zero leakage.
- [ ] RED before wiring the final production constructor:
  `cargo test -p riot-core --release --all-features --test sqlite_conference_gate`.
- [ ] Add measured fixtures for cold open, dozens-of-spaces listing, thousands-of-
  documents open/query, indexed app list, and bounded bulk import. Record actual
  device/simulator evidence; do not silently relax approved budgets.
- [ ] GREEN and blocking gates:

  ```sh
  cargo test --workspace --all-features
  cargo fmt --all -- --check
  cargo clippy --workspace --all-features -- -D warnings
  cargo tarpaulin --fail-under 100
  cargo llvm-cov --workspace --all-features --branch \
    --fail-under-lines 100 --fail-under-functions 100 \
    --fail-under-regions 100 --fail-under-branches 100
  cargo test -p riot-core --release --all-features --test sqlite_conference_gate
  cargo run --locked --package xtask -- generate-bindings
  xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
    -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
    -derivedDataPath build/ios-derived -enableCodeCoverage YES
  xcrun xccov view --report build/ios-derived/Logs/Test/*.xcresult
  xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
    -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
    -derivedDataPath build/ios-ui-derived
  scripts/conference/build-native-core.sh
  scripts/conference/test-native-core-package.sh
  ```

- [ ] `.coverage-thresholds.json` is authoritative; any coverage miss blocks
  completion. The report records commit, RED/GREEN evidence, tool versions,
  schema/SQLite options, counts, coverage, performance, exact IDs, recovery
  checks, and deferred stop line. Update the inventory to name Rust/SQLite as
  authority and Swift/Kotlin as clients.
- [ ] Request final adversarial code review, correct findings, rerun affected
  gates, then commit the evidence files.
