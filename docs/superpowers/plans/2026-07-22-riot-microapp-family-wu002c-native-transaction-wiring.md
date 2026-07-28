# WU-002c — Native prepare/persist/finalize wiring + alert copy + fault injection (iOS/macOS/Android) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: metaswarm orchestrated-execution (4-phase loop). Steps use checkbox (`- [ ]`) syntax. Parent: `2026-07-22-riot-microapp-family-master-plan.md`. Spec: `docs/superpowers/specs/2026-07-22-riot-microapp-family-design.md` §"Durable trust and app-data transactions", §"State, failure, and conflict behavior", §"Existing-user presentation". Third and last of the three WU-002 units (002a trust seam #102 · 002b app-data seam #106 · **002c native wiring + alert copy + fault injection**). Builds on WU-001N (#115/#116, commits `9b33aa6`/`13093da`) which exposes the Android `encodedSize` preflight and durable generation marker.
>
> **REVISION 2026-07-27 (against `origin/main` `82316db`).** Re-verified every anchor; re-scoped against **#146** (community-scoped tools), which landed roughly half of this plan's original Apple scope; resolved the `discardPreparedAppData` placeholder (see "Discard routing — RESOLVED"); dropped Task 0 (androidTest repair landed in #116); updated all Android paths for the `org.riot.evidence` → `net.protest.riot` rename (#137) and the Compose shell migration (#139/#140). This is still a DRAFT pending the 3-agent plan-review gate.

**Goal:** Finish wiring the landed Rust two-phase seams into the iOS, macOS, and Android hosts so trust grant/revoke and every app-data write run **prepare → (host durably persists) → finalize** under one profile authority/persistence lock, with the durable write as the linearization point. Apple trust grant/revoke already runs this ordering (landed in #146); what remains on Apple is the **app-data** inversion, the ordered revoke teardown, the native alert/rebuild copy, storage-full mapping, and fault injection. Android remains fully commit-first and needs the whole inversion plus the `encodedSize` capacity preflight. Both hosts get session-invalidation-before-WebView-destroy on revoke, the exact native trust storage-full/save-failed/rebuild alert copy, a typed app-data failure category across the JS bridge, and fault-injection tests at every transaction boundary. No Rust ABI change; no visual redesign.

**Architecture:** The Rust FFI already exposes everything this WU consumes — `AppRuntimeSession.prepareAppTrust/finalizeAppTrust/discardPreparedTrust/prepareAppDataPut/finalizeAppDataPut/discardPreparedAppData` and `AppExecutionSession.prepareAppExecutionPut/finalizeAppExecutionPut` (verified in `crates/riot-ffi/src/apps_ffi.rs:183-374` on `82316db`). Apple (#146) already inverted trust grant/revoke under `appMutationLock` with save-before-finalize and a finalize-failure reopen path; WU-002c extends the same lock to the app-data path, adds the missing ordered teardown + copy + fault seam, and **does not touch the landed trust flow**. Android inverts the commit-first ordering in `RiotController.kt` + `UniffiAppDataPort` into prepare → persist → finalize inside new single-`persistLock`-acquisition `*WithPersist` transaction methods, routes durable growth through `PersistedProfileCodec.encodedSize`, and adds a test-only fault-injection seam beside the existing `encodeWithHooksForTest`.

**Tech Stack:** Swift 6/SwiftUI + Foundation (Apple, shared iOS/macOS sources), Kotlin 2.2/JVM binary codec + Android Views (legacy rows inside the Compose shell's `LegacySurface`), XCTest (RiotTests / RiotKitTests-macOS), JUnit host-JVM (`:app:testDebugUnitTest`) + instrumented (`connectedDebugAndroidTest`).

---

## Scope boundary (do NOT exceed)

**Already landed — do NOT redo:**

- **#146 (Apple trust, `df634a0`):** per-community trust storage (`trustedAppsByNamespace`, legacy profile-wide `trustedAppIDs` deliberately not decoded — fails closed); `appMutationLock` (`NSRecursiveLock`) + `withAppMutationLock` spanning every app mutation; trust grant/revoke already run **prepare → save → finalize** with `discardPreparedTrust` on save failure, rollback of `persisted`, organizer gating, and `appOperationsClosed` + `RepositoryError.durableDecisionNeedsReopen` on finalize-invariant failure; `AppModel.recoverDurableTrustDecision` reopens and re-verifies; **active-namespace listing truth across A→B→A switches** — `switchToCommunity` (`ProfileRepository.swift:1320-1328`) re-issues only the target namespace's grants after every switch, so returning to A restores A without assigning its grants to B (this was the master plan's third WU-002c obligation; #146 delivered it — verified in tree 2026-07-27, unblocking WU-002P). WU-002c builds on this exact pattern for app-data and adds only what #146 did not deliver (below).
- **#116 (Android test repair, `13093da`):** `AppPersistenceRestartTest.kt` / `AppRuntimeEndToEndTest.kt` already construct `UniffiAppDataPort(execution: …)` via `openAppExecution` — the `androidTest` source set compiles. Former Task 0 is DONE.

**IN scope**

- Apple (shared source): app-data persist-first inversion in `AppRuntimeDataBridge.put` and the ungated `appDataPut` (under the existing `appMutationLock`); typed app-data failure category (`storageFull` / `devicePersistence` / `generic`) across the bridge; storage-full vs save-failed mapping for app-data (`NSFileWriteOutOfSpaceError`/`ENOSPC` → `storageFull`); deterministic session-invalidate-before-WebView-destroy on revoke (inside the locked transaction); the three native trust alert strings + three rebuild-status strings (none exist in the tree today — verified by grep 2026-07-27); transient `Adding…`/`Turning off…` row states blocking launch; a test-only fault-injection hook in `ProtectedProfileStorage.save`; app-data finalize-invariant rebuild reusing the #146 reopen pipeline.
- Android: route trust grant/revoke and app-data through new single-lock-acquisition `RiotController.*WithPersist` transaction methods in prepare→persist→finalize order; call `PersistedProfileCodec.encodedSize(prospective)` as the pre-mutation capacity preflight (reject storage-full before any core/disk mutation; never materialize a generation marker on a grandfathered `null`/v3 profile); session-invalidate-before-WebView-destroy on revoke; the same native trust/rebuild alert copy + transient row states (still anchored in `MainActivity`'s legacy surfaces — the Compose migration did not move tool rows); a test-only fault-injection hook in the store save path.
- Both: the **typed** app-data failure category returned across the JS bridge so a v2 page can later render the correct inline copy (the inline strings themselves are WU-007+ web work).

**OUT of scope (do NOT touch)**

- The #146 Apple trust grant/revoke flow itself (organizer gating, namespace verification, per-community storage). Only additive: ordered teardown after `finalizeAppTrust`, alert copy at the VM layer, fault hook. No behavioral change to what #146 lands on disk.
- **WU-002P** owns all Tools-listing presentation: `Redesigned · Version 2` / `<name> · Legacy 1` cards, the collapsed **Legacy tools (Version 1)** section, the install confirmation warning, and the **install** count-full vs storage-full copy. WU-002c introduces no Legacy/v2 card layout and no install-admission copy.
- **WU-007+** owns the microapp inline state copy ("This profile's offline storage is full. Your draft is still here…", "Riot couldn't save that on this device…") rendered by the web `_shared` helper. WU-002c only returns the failure *category* across the bridge and unit-tests that mapping; it renders none of that web copy.
- No Rust core/FFI/`crates/**` change, no fixture bytes, no theme/font/toolbar files, no `starterCatalogGeneration` semantics change. If any UniFFI signature is found to be genuinely required (it is not — see "Discard routing"), STOP and re-scope — do not add ABI in this WU.

**Verified anchors (worktree `docs/microapp-wu002c-plan-revision` @ `origin/main` `82316db`, 2026-07-27)**

- FFI two-phase surface (`crates/riot-ffi/src/apps_ffi.rs`): `prepare_app_execution_put` `:183`, `finalize_app_execution_put` `:195` (on `AppExecutionSession`); `prepare_app_trust` `:294`, `finalize_app_trust` `:305`, `discard_prepared_trust` `:311`, `prepare_app_data_put` `:359`, `finalize_app_data_put` `:369`, `discard_prepared_app_data` `:374` (on `AppRuntimeSession`). **No discard on `AppExecutionSession` — by design; see "Discard routing".**
- Apple (shared): `apps/ios/Riot/Core/ProfileRepository.swift` — `ProtectedProfileStorage.save(_:) throws` `:251` (the ONLY durable write; **no fault hook today**); `appMutationLock` `:293` + `withAppMutationLock` `:342` + `appOperationsClosed`/`requireAppOperationsOpen()` `:296`/`:348`; `trustApp(appID:expectedNamespaceID:)` `:1014` (convenience `:1046`); `untrustApp(appID:expectedNamespaceID:)` `:1058` (convenience `:1106`) — **both already prepare→save→finalize**; `appDataBridge(appID:)` `:1138`; `persistAppDataBundle(_:) throws` `:1159`; `appDataPut(...)` `:1171` (**still commit-first**: `appDataPutWithReceipt` → `persistAppDataBundle`); `PersistedAppTrust{namespaceID, appIDHex}` `:113-121`; `trustedAppsByNamespace` `:127`; `appDataBundles` `:136`; open-time namespace-filtered trust re-issue `:537-543`; app-data replay loop `:560-568`. `RecoveryReport` in `apps/ios/Riot/Core/RecoveryQuarantine.swift:262`. Bridge put path `apps/ios/Riot/Apps/AppBridgeController.swift:96-103` (`AppRuntimeDataBridge.put` → `execution.appDataPutWithReceipt` → `onCommitted`); `teardownSession()` `:132` (`execution.invalidate()`). WebView teardown `apps/ios/Riot/Apps/AppRuntimeView.swift:735` (`tearDown`); invalidation notification `AppRuntimeView.swift:248` (name) / `:252` (post) / `:364-365` (receive); mount state `apps/ios/Riot/CommunityShell.swift:327` (`AppRuntimeMountState`), `replace()` `:333`, `tearDownNow()` `:339`. VM: `apps/ios/Riot/AppModel.swift` — `trustApp(appID:expectedNamespaceID:)` `:1763` → `approveTool` `:1771` → `persistTrustDecision` `:1816`; `recoverDurableTrustDecision` `:1846`; compat `trustApp(appID:)` `:1835`; `untrustApp(appID:)` `:2014`.
- Android (package `net.protest.riot`, root `apps/android/app/src/`): `main/kotlin/net/protest/riot/RiotController.kt` — `persistLock` `:51`, `mutatePersisted` `:501`, `mutatePersistedIfPresent` `:509`, `persist(...)` `:516` (`store.save` call site `:523`), `onAppTrusted`→`recordAppTrust` `:169` (helper `AppPersistence.kt:37`), `onAppUntrusted`→`recordAppUntrust` `:173`, `onAppDataCommitted`→`recordAppData` `:181`, `openAppRuntime()` `:147`, `openAppExecution(appIdHex)` `:155`. Trust wiring `main/kotlin/net/protest/riot/apps/RiotAppsController.kt:64` `trust`, `:73` `untrust` (**both still commit-first**: `session.trustApp(...)` then `onTrusted(...)`). App-data port `apps/AppDataPort.kt:47` `UniffiAppDataPort(execution: AppExecutionSession, onCommitted)`; `put` `:51` (**commit-first**). WebView host `apps/AppWebViewHost.kt:129` `destroy()`. Legacy surfaces in `MainActivity.kt` (half-migrated: Compose shell `:164-213`, legacy rows via `LegacySurface` `:195`): `showTools` `:413` (untrust action `:425-430`), `showDirectory` `:447`, `showAppReview` `:543` (trust action `:549-554`), `openApp` `:558`, `closeApp` `:587`. Preflight: `PersistedProfile.kt` — `encodedSize` `:295`, `MAX_ENCODED_BYTES = 4*1024*1024-64` `:71`, existing test hook `encodeWithHooksForTest` `:98`. Host-JVM tests under `src/test/kotlin/net/protest/riot/`: `apps/RiotAppsControllerTest.kt` (`FakeAppRuntimeSession` `:23-89` — already implements `prepareAppTrust` `:46`, `finalizeAppTrust` `:51`, `discardPreparedTrust` `:57`, `prepareAppDataPut` `:75`, `finalizeAppDataPut` `:80`, `discardPreparedAppData` `:81`), `AppPersistenceTest.kt`, `apps/RiotJsBridgeTest.kt`, `PersistedProfileCodecTest.kt`. Instrumented: `src/androidTest/kotlin/net/protest/riot/apps/AppPersistenceRestartTest.kt`, `AppRuntimeEndToEndTest.kt` (repaired, compile-green).

---

## File responsibilities

| File | Responsibility |
| --- | --- |
| `apps/ios/Riot/Core/ProfileRepository.swift` | Invert app-data (`appDataPut`, bridge persist path) to prepare→persist→finalize under the existing `appMutationLock`; storage-full vs save-failed mapping; test-only fault-injection hook on `ProtectedProfileStorage.save`; app-data finalize-invariant rebuild (reuse #146 reopen pipeline) |
| `apps/ios/Riot/Apps/AppBridgeController.swift` | Rewrite `AppRuntimeDataBridge.put` to prepare→persist→finalize via `AppExecutionSession`; return typed failure category; discard via repository-held `AppRuntimeSession` (see "Discard routing") |
| `apps/ios/Riot/Apps/AppRuntimeView.swift` / `apps/ios/Riot/CommunityShell.swift` | Deterministic session-invalidate-before-WebView-destroy on revoke (invoked inside the locked revoke transaction, not lazily via the notification fallback) |
| `apps/ios/Riot/AppModel.swift` + `apps/ios/Riot/Directory/DirectoryView.swift` | Transaction transient state (`Adding…`/`Turning off…` blocking launch), native trust alert copy + announce-once + focus return, rebuild status strings (`DirectoryView` is the view owning the Add/Review rows — the `approveTool` call site) |
| `apps/android/.../RiotController.kt` | Prepare→persist→finalize under `persistLock`; `encodedSize` capacity preflight; no marker materialization; fault-injection hook beside `encodeWithHooksForTest`; rebuild |
| `apps/android/.../apps/RiotAppsController.kt` | Replace direct `trustApp`/`untrustApp` with the prepare/persist/finalize sequence + typed failures |
| `apps/android/.../apps/AppDataPort.kt` | `UniffiAppDataPort.put` → prepare→persist→finalize via `AppExecutionSession`; typed failure; discard via controller-held `AppRuntimeSession` |
| `apps/android/.../MainActivity.kt` | Transient row state, session-invalidate-before-destroy on revoke, native alert copy, rebuild status (legacy surfaces only — no Compose changes) |
| XCTest / JUnit test files | RED-first tests for every boundary; fault-injection coverage |

---

## Design (read before Task 1)

**One authority/persistence lock per host — both exist.** Android has `persistLock` (`RiotController.kt:51`) — each transaction runs in ONE acquisition inside a new `RiotController.*WithPersist` method (see "One controller method per transaction" below; `mutatePersisted`'s pure-transform shape stays for simple non-transactional mutations only). Apple has `appMutationLock` (`NSRecursiveLock`, recursive because bridge writes re-enter via `persistAppDataBundle`) — WU-002c reuses it; it must not be re-entered by WebView teardown (teardown performs no persistence).

**Persist-first ordering.** Apple trust already runs it (#146). Remaining commit-first paths to invert: Apple app-data (`AppRuntimeDataBridge.put`, ungated `appDataPut`), and **all** Android trust/app-data paths:

```
grant(appID):                       revoke(appID):
  lock                                lock
  row = Adding… (cannot launch)       row = Turning off… (block launch + block bridge)
  prepare_app_trust(appID, true)      prepare_app_trust(appID, false)
  [Android] encodedSize preflight     [Android] encodedSize preflight (shrink/zero-growth → passes)
    on prospective trusted-ID set       on prospective trusted-ID set
  persist trusted-ID set (atomic)     persist trusted-ID set with ID removed (atomic)
    fail → discard + storage/save alert  fail → discard + revoke save-failed alert (still On)
  finalize_app_trust()                finalize_app_trust()   (invalidates every execution session
  row = Open                            for the app before runtime trust is removed)
                                      invalidate mounted execution session for appID
                                      destroy the mounted WebView — before releasing the lock
                                      unlock; row leaves to Tools
```

App-data write (page or host path):

```
put(app, key, value):
  lock
  prepare_app_execution_put(key, value)  (or prepare_app_data_put for the ungated host path) → receipt
  [Android] encodedSize preflight on prospective (app_id,key) receipt replacement
  persist receipt (atomic)  fail → discard (via runtime session) + return typed failure (storageFull | devicePersistence)
  finalize_app_execution_put()
  unlock; return ok
```

The durable write is the linearization point: a crash before finalize replays exactly the persisted trusted-ID set (re-issued per WU-002a, namespace-filtered per #146) or the persisted receipt (`replayAppDataBundle`, WU-002b) on restart, never double-applying.

**One controller method per transaction (Android lock consolidation).** `mutatePersisted`'s pure-transform signature `(PersistedProfile) -> PersistedProfile` cannot express fallible prepare/finalize/discard side effects, and splitting prepare (controller caller) / persist (inside `mutatePersisted`) / finalize (caller) across the lock boundary lets a second prepare **supersede** the shared prepared slot mid-transaction (the slot is single, per-profile). So each Android transaction is ONE new `RiotController` method — `grantAppTrustWithPersist(appId)`, `revokeAppTrustWithPersist(appId)`, `putAppDataWithPersist(appId, key, value)` — that acquires `persistLock` **once** and runs prepare → preflight → persist → finalize (or discard-on-failure) entirely inside, operating on the snapshot + `persist()` directly rather than through `mutatePersisted`'s transform. `RiotAppsController.trust/untrust` and `UniffiAppDataPort.put` become thin delegates to these methods. Apple's `withAppMutationLock` already spans the whole triple in one acquisition — same invariant, no change needed.

**Ordered revoke teardown seam (Apple).** `RiotProfileRepository` holds no handle to the mounted `AppExecutionSession`/WebView, so the spec's "destroys the WebView before releasing the lock" needs an explicit seam: add a settable `appTeardownHandler: ((String) -> Void)?` on the repository, wired by the mount layer (`CommunityShell`/`AppModel`) whenever an app mounts (and cleared on unmount). After a successful `finalizeAppTrust()` in `untrustApp`, the repository invokes the handler **inside** `withAppMutationLock`, before returning. The handler (a) synchronously invalidates the bridge's `AppExecutionSession` (`execution.invalidate()` is an atomic flag flip — thread-safe, no MainActor hop) and (b) drives `AppRuntimeMountState.tearDownNow()` for the WebView destruction. **Threading contract:** `tearDownNow()` and `AppRuntimeMountState` are `@MainActor` (`CommunityShell.swift:326-327`) while the repository is nonisolated — so the handler is wired **main-thread-only** and invokes the teardown via `MainActor.assumeIsolated` (the production revoke path is already main-actor: `RiotAppModel` is `@MainActor`, sole prod caller `AppModel.swift:2017`). Tests use a plain spy handler and may drive revoke off-main. Deadlock cannot occur: the handler never blocks on the main thread *from* a background thread — it is only ever invoked on the thread already holding the lock, and the main-thread wiring never waits for a second lock acquisition (WebView teardown performs no persistence). On persist or finalize failure the handler is NOT invoked (tool stays On, runtime untouched). The notification path (`AppRuntimeView.swift:248/:364`) stays as fallback for out-of-band revokes only. Android mirrors this inside the controller's `revokeAppTrustWithPersist`: invalidate the mounted session, then `runningApp?.destroy()`, before releasing `persistLock`.

**Discard routing — RESOLVED (no ABI change).** `AppExecutionSession` exposes only `prepareAppExecutionPut`/`finalizeAppExecutionPut`; both discards live on `AppRuntimeSession`. This is not a gap: `discard_prepared_app_data` (`apps_ffi.rs:374`) clears **"the shared prepared-mutation slot"** on the same `inner` profile state that `prepare_app_execution_put` (`:183`) fills — the prepared slot is per-profile, not per-session (verified: `mobile_state.rs` — single `prepared: Option<PreparedMutation>` on `LocalProfile`; discard clears it unconditionally, no generation guard; a contract test at `apps_contract.rs:1086` pins the alias). Therefore the correct route is: the host component that owns the transaction (Apple `RiotProfileRepository`, Android `RiotController`) already holds the `AppRuntimeSession` (`appRuntime` / `profile.appRuntime()`); on persist failure it calls `runtime.discardPreparedAppData()` **under the same persistence lock**, then surfaces the typed failure. One benign caveat: discard clears *any* prepared variant (Trust or AppData) — safe only because the host lock serializes, which this plan mandates. **This changes two constructors** (acknowledged, in scope): `AppRuntimeDataBridge` (`AppBridgeController.swift:86-94`, currently `(execution, profiles, onCommitted)`) gains a repository transaction seam so persist+discard route through `RiotProfileRepository` under `appMutationLock` — call sites `ProfileRepository.swift:1147`, `AppRuntimeHostTests.swift:73`; `UniffiAppDataPort` (`AppDataPort.kt:47`) delegates `put` to `RiotController.putAppDataWithPersist` instead of calling `execution.appDataPutWithReceipt` + `onCommitted` directly — call sites `MainActivity.kt:566`, `AppPersistenceRestartTest.kt:45,64`, `AppRuntimeEndToEndTest.kt:68`. `FakeAppRuntimeSession` already implements `discardPreparedAppData` (`RiotAppsControllerTest.kt:81`), so Android fakes need no changes. **Confirmed: no Rust ABI change.**

**Storage-full vs save-failed.** Android detects storage-full *before* mutation via `encodedSize(prospective) > MAX_ENCODED_BYTES`; any other failure is a save-failed. Apple has no codec ceiling, so storage-full surfaces only as an atomic-write failure — Apple maps a genuine disk-full `NSError` (`NSFileWriteOutOfSpaceError` / `ENOSPC`) to `storageFull`, any other write failure to the save-failed strings, and (for app-data across the bridge) to `devicePersistence`. This keeps "the two conditions are never collapsed" true on both hosts.

**Native alert copy (this WU owns these exact strings — trust + rebuild only).** Byte-for-byte from spec §"Durable trust and app-data transactions" (re-verified against the spec 2026-07-27). `<name>` is the tool's display name; `<community>` is the active community's display name. Verified absent from both trees 2026-07-27.
- Grant storage-full: `This device's offline storage is full, so Riot couldn't add <name> to <community>. The tool is still off and your tools did not change.`
- Grant other-persistence-failure: `Riot couldn't save that change on this device. <name> was not added to <community>. Nothing changed. Try again.`
- Revoke persistence failure: `Riot couldn't save that change on this device. <name> is still on. Try again.`
- Rebuild after finalize-invariant failure (one announced status + focus in Tools):
  - `Added <name> to <community>. Riot reopened this profile to finish safely.`
  - `<name> was removed from <community>. Riot reopened this profile to finish safely.`
  - `Your change was saved. Riot reopened this profile to finish safely.`

Each grant alert returns focus to the originating **Add <name> to <community>** action (row returns to its named **Add** action); each revoke alert returns focus to **Turn off** (row returns to **Open**). Every alert is **announced once**, and a user-activated retry reruns the whole transaction. Never expose token/transaction/codec/raw-storage language.

**App-data failure copy is web, not native.** The State-table inline strings render inside the microapp (WU-007+). WU-002c returns only the failure *category* over the bridge and unit-tests the mapping; it renders no app-data alert copy natively.

**Fault-injection seam.** Add a test-only hook in each host's durable-save function with **two injection points**: (a) `beforeWrite: ((Data) throws -> Void)?` — throw a chosen error (storage-full vs generic) before the atomic write; (b) `afterWriteBeforeReturn: (() -> Void)?` — invoked after the atomic write commits but before returning to the caller, letting a test simulate process termination at exactly the post-commit boundary (the test then abandons the instance and reopens to assert convergence). On Apple wrap `ProtectedProfileStorage.save` (`ProfileRepository.swift:251`); on Android wrap the `store.save` path (`RiotController.kt:523`), beside the existing `encodeWithHooksForTest` (`PersistedProfile.kt:98`). Boundaries to cover for both grant and revoke: before-prepare, after-prepare, before-persist, after-persist(=post-commit termination), before-finalize, after-finalize, session-invalidation, WebView-destruction, process-termination, profile-rebuild; for app-data: before/after prepare, before/after persist, before/after finalize.

**Scope decision — transient row states.** The `Adding…`/`Turning off…` labels and the block-launch-during-transaction / return-to-Add-or-Open-on-failure behavior are the transaction's *observable contract* (spec pins them in the transaction section), so they are IN this WU. The Legacy/v2 card redesign and install copy remain WU-002P.

**Scope decision — revoke no-op and organizer gating are #146's.** Apple's `untrustApp` already skips redundant revokes (live-state no-op check) and enforces organizer authorization. WU-002c adds only the ordered teardown and failure copy on top; it does not re-litigate those decisions. Android has no equivalent gating yet and simply inverts ordering.

---

## Task 1: Apple — app-data prepare/persist/finalize + typed bridge failure

**Files:** `apps/ios/Riot/Apps/AppBridgeController.swift`, `apps/ios/Riot/Core/ProfileRepository.swift`; tests `apps/ios/RiotTests/AppRuntimeHostTests.swift`, `apps/ios/RiotTests/AppRepositoryTests.swift` (register any new test file in BOTH `apps/ios/Riot.xcodeproj` and `apps/macos/Riot.xcodeproj`).

- [ ] **Step 1: Write failing tests:**
  - `app_data_put_persists_receipt_before_core_finalize` — after a bridge `put`, the receipt is on disk and `appDataGet` returns the value; injecting a persist failure (hook added in Task 4 — for this task use a throwing storage double) leaves the store unmutated (value not readable) and the prepared slot discarded (a subsequent `finalizeAppDataPut`/`finalizeAppExecutionPut` without prepare fails).
  - `app_data_put_disk_full_returns_storageFull_category` and `app_data_put_write_failure_returns_devicePersistence_category` — assert the bridge result carries the correct typed category (no native alert copy rendered here).
  - Spec exact-boundary cases (spec §"Durable trust and app-data transactions" final paragraph): replacement of an existing key and a brand-new key (distinct receipts, both persist); a live read immediately after the `put` result returns the new value; a read after close/reopen returns exactly the persisted value.
  - `app_data_put_serializes_with_concurrent_revoke` — drive a bridge `put` and an `untrustApp` from two queues against the same profile; assert the `appMutationLock` serializes them (no torn prepared slot: exactly one of {put commits, put returns typed failure} and trust state matches the revoke outcome after reopen).
  - `app_data_put_serializes_with_concurrent_put` — two concurrent bridge `put`s (same profile, distinct keys) both commit, both receipts persist, no prepared-slot supersede (a second prepare must never make the first finalize take the wrong mutation). And `app_data_put_serializes_with_concurrent_grant` — a `put` racing `trustApp` (grant path) similarly serializes with both outcomes durable. Together with the revoke case these are the implementing tests for DoD "a concurrent trust/profile change cannot interleave".
  - Keep `testBridgePutPersistsAcrossReopen` / `testAppDataSurvivesReopen` green (byte-identical receipt).

- [ ] **Step 2: Run RED** (focused):

```text
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/AppRuntimeHostTests
```

- [ ] **Step 3: Implement.** Rewrite `AppRuntimeDataBridge.put(key:valueJSON:)` (`AppBridgeController.swift:96`) to route through a new repository transaction seam (constructor change, acknowledged in Design §"Discard routing": the bridge gains a closure/protocol handle so persist+discard run inside `RiotProfileRepository`'s `withAppMutationLock`): `execution.prepareAppExecutionPut(key:, value:)` → repository persists the receipt via `persistAppDataBundle` → `execution.finalizeAppExecutionPut()`; on persist failure the repository calls `appRuntime.discardPreparedAppData()` (same lock) and the bridge returns a typed `AppDataFailure` category (`storageFull` when the write error is `NSFileWriteOutOfSpaceError`/`ENOSPC`, else `devicePersistence`; bridge-level rejects map to `generic`). Update the ungated host `appDataPut(...)` (`ProfileRepository.swift:1171`) similarly via `prepareAppDataPut`/`finalizeAppDataPut` + `discardPreparedAppData`. Update the two bridge construction call sites (`ProfileRepository.swift:1147`, `AppRuntimeHostTests.swift:73`). The receipt persisted is the exact bytes prepare returns (WU-002b guarantees they equal today's commit output).
- [ ] **Step 4: Run GREEN** (iOS focused above) then the shared macOS scheme:

```text
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS' \
  -only-testing:RiotKitTests-macOS/AppRuntimeHostTests
```

- [ ] **Step 5: Commit** (pathspec: the two source files + test files + any pbxproj registration).

---

## Task 2: Apple — ordered revoke teardown (session-invalidate-before-WebView-destroy)

**Files:** `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/Riot/AppModel.swift`, `apps/ios/Riot/Apps/AppRuntimeView.swift` / `apps/ios/Riot/CommunityShell.swift`; tests `AppRepositoryTests.swift`, `AppBreadcrumbTests.swift` (teardown ordering).

- [ ] **Step 1: Write failing tests:**
  - `revoke_invalidates_session_before_destroying_webview` — with a mounted app, revoke and assert the execution session is invalidated **before** the WebView teardown runs (observe ordering via a spy on the mount-state teardown vs `AppExecutionSession.isValid()`), and both occur before `appMutationLock` is released.
  - `revoke_save_failure_keeps_tool_on_and_reports_no_change` — inject a persist failure (storage double); assert trust stays ON, the mounted runtime is untouched (no invalidate, no teardown), and the surfaced error maps to the revoke save-failed string.
  - Keep #146's revoke tests green (no-op revoke, organizer gating, `durableDecisionNeedsReopen`).

- [ ] **Step 2: Run RED** (iOS focused: `-only-testing:RiotTests/AppRepositoryTests`).
- [ ] **Step 3: Implement.** Add the `appTeardownHandler: ((String) -> Void)?` seam on `RiotProfileRepository` (see Design §"Ordered revoke teardown seam"): the mount layer (`CommunityShell`/`AppModel`) sets it when an app mounts and clears it on unmount; the handler synchronously invalidates the bridge's `AppExecutionSession` (`execution.invalidate()` — atomic, thread-safe) then drives `AppRuntimeMountState.tearDownNow()` (`CommunityShell.swift:339`). After the landed #146 revoke flow (`untrustApp(appID:expectedNamespaceID:)` `ProfileRepository.swift:1058`) completes `finalizeAppTrust()` successfully, invoke the handler for `appID` **inside** `withAppMutationLock`, before returning. On persist or finalize failure, do NOT invoke it (tool stays On). Keep the invalidation-notification path (`AppRuntimeView.swift:248/:364`) as a fallback for out-of-band revokes (namespace switch, generation bump from elsewhere) but do not rely on it for the ordered teardown. Wire `AppModel.untrustApp` (`:2014`) to surface the revoke save-failed alert copy on `storage.save` failure.
- [ ] **Step 4: Run GREEN** (iOS + macOS focused).
- [ ] **Step 5: Commit.**

---

## Task 3: Apple — native alert copy + transient row states

**Files:** `apps/ios/Riot/AppModel.swift`, `apps/ios/Riot/Directory/DirectoryView.swift` (the view owning the Add/Review rows — the `approveTool` call site); tests `apps/ios/RiotTests/ToolsSectionTests.swift`.

- [ ] **Step 1: Write failing tests:** for grant storage-full, grant save-failed, and revoke save-failed, assert the VM surfaces the exact spec string (byte-for-byte, incl. `<community>`), announces once, and the row returns to its named **Add** action / **Open**; assert `Adding…` cannot launch and `Turning off…` blocks new launches for the duration of the transaction, focus returns to the originating **Add <name> to <community>** / **Turn off** action, and a user retry reruns the whole transaction.
- [ ] **Step 2: Run RED** (iOS focused).
- [ ] **Step 3: Implement.** Add the transient row state and the announce-once alert plumbing in `AppModel` + the row rendering in `DirectoryView`; map `storage.save` failures from `persistTrustDecision` (`:1816`) — disk-full → grant storage-full string, other → grant save-failed string; revoke failure → revoke save-failed string (all byte-for-byte per the Design section). No token/codec/raw-storage language.
- [ ] **Step 4: Run GREEN** (iOS + macOS focused).
- [ ] **Step 5: Commit.**

---

## Task 4: Apple — fault injection + finalize-invariant rebuild for app-data

**Files:** `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/Riot/AppModel.swift`; tests new `apps/ios/RiotTests/AppTransactionFaultTests.swift` (register in both pbxproj + the macOS test target).

- [ ] **Step 1: Write failing tests** driving a new test-only fault hook on `ProtectedProfileStorage.save` (`:251`) for **grant and revoke** at each boundary (before/after prepare, before/after persist, before/after finalize, session-invalidation, WebView-destruction, process-termination, profile-rebuild) and for **app-data** (before/after prepare, persist, finalize). Assertions:
  - No reported failure changes live or restarted trust/app-data values (reopen and read the prior committed value).
  - A post-commit termination (fault after the atomic write, before finalize) converges on reopen: trust/app-data reflects the durable state exactly once, and Tools shows exactly one of the three rebuild-status strings with focus in Tools.
  - An unexpected app-data finalize-invariant failure closes the profile and rebuilds from the already-durable snapshot before any tool can reopen (reuse `appOperationsClosed` + the #146 reopen pipeline); no durable decision is rolled backward.

- [ ] **Step 2: Run RED.**
- [ ] **Step 3: Implement** the two-point fault hook (`beforeWrite: ((Data) throws -> Void)?` + `afterWriteBeforeReturn: (() -> Void)?`, per the Design §"Fault-injection seam", exposed only to tests) and the app-data rebuild path mirroring #146's `durableDecisionNeedsReopen` → `recoverDurableTrustDecision` (`AppModel.swift:1846`), surfacing exactly one rebuild-status string via the announce-once path.
- [ ] **Step 4: Run GREEN** (iOS + macOS): full `RiotTests` and `RiotKitTests-macOS`; confirm no regression beyond known pre-existing red guide tests.
- [ ] **Step 5: Commit.**

---

## Task 5: Android — persist-first trust grant under `persistLock` + `encodedSize` preflight + alerts

**Files:** `apps/android/app/src/main/kotlin/net/protest/riot/RiotController.kt`, `.../apps/RiotAppsController.kt`, `.../MainActivity.kt`; tests `apps/android/app/src/test/kotlin/net/protest/riot/apps/RiotAppsControllerTest.kt`, `AppPersistenceTest.kt`.

- [ ] **Step 1: Write failing host-JVM tests** (`FakeAppRuntimeSession` already implements the full two-phase interface, `RiotAppsControllerTest.kt:23-89`):
  - grant runs prepare → persist → finalize in order and blocks launch until finalize;
  - grant calls `PersistedProfileCodec.encodedSize(prospective)` before any mutation; a prospective set exceeding `MAX_ENCODED_BYTES` is rejected with the grant storage-full string and **no** core/disk mutation and **no** generation-marker materialization on a grandfathered `null`/v3 profile;
  - a generic persist failure yields the grant save-failed string, trust OFF, and `discardPreparedTrust` invoked.

- [ ] **Step 2: Run RED:**

```text
cd apps/android
JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
ANDROID_HOME=$HOME/Library/Android/sdk \
./gradlew :app:testDebugUnitTest --tests net.protest.riot.apps.RiotAppsControllerTest
```

- [ ] **Step 3: Implement.** Add `RiotController.grantAppTrustWithPersist(appId)` per Design §"One controller method per transaction": it acquires `persistLock` **once** and inside runs `prepareAppTrust(appId, true)` → builds the prospective trusted-ID set from the snapshot → `PersistedProfileCodec.encodedSize(prospective)` preflight (reject storage-full pre-mutation; preserve `starterCatalogGeneration` exactly — never materialize on `null`) → `persist(prospective)` → `finalizeAppTrust()`; on failure `discardPreparedTrust()` (same lock) + typed error. Rewrite `RiotAppsController.trust` (`:64`) to a thin delegate. Surface the exact alert strings from `MainActivity` (`showAppReview` `:543`) and add the `Adding…`/cannot-launch transient row state to `showTools` (`:413`)/`showAppReview`.
- [ ] **Step 4: Run GREEN** (host-JVM focused, then full `:app:testDebugUnitTest`).
- [ ] **Step 5: Commit.**

---

## Task 6: Android — persist-first revoke + session-invalidate-before-destroy

**Files:** `apps/android/app/src/main/kotlin/net/protest/riot/RiotController.kt`, `.../apps/RiotAppsController.kt`, `.../MainActivity.kt`; tests `RiotAppsControllerTest.kt`, `AppPersistenceTest.kt`, `apps/RiotJsBridgeTest.kt`. (`AppWebViewHost.kt:129` `destroy()` is a read-only anchor — the task calls it, unchanged.)

- [ ] **Step 1: Write failing tests:** revoke persists the removal before finalize; a persist failure keeps the tool ON with the exact revoke save-failed string and an untouched runtime (no invalidate, no destroy); on success the mounted execution session is invalidated **before** `AppWebViewHost.destroy()` (`:129`) and both happen before `persistLock` is released.
- [ ] **Step 2: Run RED** (focused `:app:testDebugUnitTest --tests …`).
- [ ] **Step 3: Implement.** Add `RiotController.revokeAppTrustWithPersist(appId)` (single `persistLock` acquisition, per Design §"One controller method per transaction"): `prepareAppTrust(appId, false)` → persist removal (`encodedSize` preflight trivially passes on shrink) → `finalizeAppTrust()` → invalidate the mounted `AppExecutionSession` for `appId` then `runningApp?.destroy()` (ordered) — all before releasing the lock; on persist/finalize failure skip teardown (tool stays On, runtime untouched) and surface the typed error. Rewrite `RiotAppsController.untrust` (`:73`) to a thin delegate; wire `MainActivity` "Turn off" (`showTools` `:425-430`) to the transient `Turning off…` block-launch state.
- [ ] **Step 4: Run GREEN.**
- [ ] **Step 5: Commit.**

---

## Task 7a: Android — app-data prepare/persist/finalize + typed failure

**Files:** `apps/android/app/src/main/kotlin/net/protest/riot/apps/AppDataPort.kt`, `.../RiotController.kt`; tests `apps/android/app/src/test/kotlin/net/protest/riot/apps/RiotJsBridgeTest.kt`, `PersistedProfileCodecTest.kt` (preflight), `AppPersistenceTest.kt`.

- [ ] **Step 1: Write failing tests:**
  - `UniffiAppDataPort.put` runs prepare → persist → finalize inside a single `persistLock` acquisition; a persist failure leaves the store unmutated, clears the prepared slot (via the controller-held runtime session — see "Discard routing"), and returns the typed category (`storageFull` when the `encodedSize` preflight rejects, else `devicePersistence`).
  - Spec exact-boundary cases: replacement of an existing `(appId,key)` receipt and a new key; a live read immediately after the result; a read after restart converging via `replayAppDataBundle`; concurrent pairs serializing under `persistLock` with no torn prepared slot — put-vs-put (distinct keys, both commit), put-vs-grant, and put-vs-revoke (implementing tests for DoD "cannot interleave"); a **grandfathered storage-full profile** (`null` generation marker + prospective over the ceiling → rejected storage-full, marker stays `null`, no core/disk mutation); the **largest permitted Photo Wall receipt** (prospective just under `MAX_ENCODED_BYTES` passes; at/over rejects).
- [ ] **Step 2: Run RED** (focused host-JVM).
- [ ] **Step 3: Implement.** Add `RiotController.putAppDataWithPersist(appId, key, value)` (single `persistLock` acquisition, per Design §"One controller method per transaction"): `execution.prepareAppExecutionPut(key, value)` → `encodedSize` preflight on the prospective `(appId,key)` replacement → persist the receipt → `execution.finalizeAppExecutionPut()`; on failure `openAppRuntime().discardPreparedAppData()` (same lock) + typed category. Rewrite `UniffiAppDataPort.put(key, value)` (`AppDataPort.kt:51`) to delegate (constructor change acknowledged in Design §"Discard routing"); update call sites `MainActivity.kt:566`, `AppPersistenceRestartTest.kt:45,64`, `AppRuntimeEndToEndTest.kt:68`.
- [ ] **Step 4: Run GREEN** host-JVM (`:app:testDebugUnitTest`), and `:app:assembleDebugAndroidTest` compile-green.
- [ ] **Step 5: Commit.**

---

## Task 7b: Android — fault injection + finalize-invariant rebuild

**Files:** `apps/android/app/src/main/kotlin/net/protest/riot/RiotController.kt`, `.../MainActivity.kt`; tests `apps/android/app/src/test/kotlin/net/protest/riot/apps/RiotAppsControllerTest.kt`, `AppPersistenceTest.kt`, and instrumented `AppPersistenceRestartTest.kt` / `AppRuntimeEndToEndTest.kt` (compile-only here, CI-run).

- [ ] **Step 1: Write failing tests:** fault-injection host-JVM tests through the store save hook (two injection points — `beforeWrite` throw + `afterWriteBeforeReturn` termination simulation — beside `encodeWithHooksForTest`, `PersistedProfile.kt:98`) covering the app-data boundaries (before/after prepare, persist, finalize) and the trust boundaries reachable host-side; post-commit termination converges on restart; the three rebuild-status strings surface (byte-for-byte, announced once, focus in Tools).
- [ ] **Step 2: Run RED** (focused host-JVM).
- [ ] **Step 3: Implement.** Add the two-point store fault-injection hook to `RiotController.persist`/`store.save` (`:516`/`:523`) and the finalize-invariant rebuild (close + rebuild from durable snapshot + one announced status). Extend the instrumented tests with the new ordering assertions (compile-verify only here).
- [ ] **Step 4: Run GREEN** host-JVM (`:app:testDebugUnitTest`), and `:app:assembleDebugAndroidTest` compile-green; record instrumented run as CI/device blocker.
- [ ] **Step 5: Commit.**

---

## Task 8: Full cross-platform quality gate

- [ ] **Rust (no change expected):** `cargo fmt --all -- --check`; `cargo clippy --workspace --all-features -- -D warnings`; `cargo test --workspace --all-features` — all must stay green (proves no accidental core/FFI edit).
- [ ] **Bindings/native libs:** if any Rust ABI changed (it must not), rerun `ANDROID_HOME=$HOME/Library/Android/sdk scripts/conference/build-native-core.sh`; otherwise confirm current bindings via `cargo run --locked -p xtask -- generate-bindings` (drift check only).
- [ ] **iOS:** `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'` — full `RiotTests`, no regression beyond known pre-existing red guide tests.
- [ ] **macOS:** `xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'`.
- [ ] **Android host-JVM:** `cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home ANDROID_HOME=$HOME/Library/Android/sdk ./gradlew :app:testDebugUnitTest`.
- [ ] **Android instrumented:** `./gradlew :app:assembleDebugAndroidTest` (compile-green). Record `adb devices`; the `connectedDebugAndroidTest` run is a CI/device-required blocker — document it, do not silently skip.
- [ ] **Coverage (source of truth):** `scripts/web/coverage.sh`; `.coverage-thresholds.json` floors hold (Rust untouched → unchanged).
- [ ] **Scope audit:** `git status --short` + `git diff --check`; confirm changed paths are only the files in the responsibilities table; NO `crates/**`, `fixtures/**`, theme/font files, no Compose `design/` files. Commit with exact pathspecs (shared checkout — never `git add -A`, never `git stash`).

---

## Definition of Done

- Trust grant, trust revoke, and every app-data write run **prepare → durable persist → finalize** under one authority/persistence lock on each host; the pre-existing commit-first ordering is no longer reachable on Apple or Android. (Apple trust already complies via #146; this WU closes Apple app-data and all Android paths.)
- Apple app-data runs under the existing `appMutationLock`; Android runs each transaction in a single `persistLock` acquisition via the new `RiotController.*WithPersist` methods. A concurrent trust/profile change cannot interleave — proven by the concurrency tests in Task 1 (put-vs-revoke, put-vs-put, put-vs-grant) and Task 7a (the same three pairs under `persistLock`).
- Android calls `PersistedProfileCodec.encodedSize(prospective)` before any durable growth, rejects storage-full **before** core/disk mutation, and never materializes a `starterCatalogGeneration` marker on a grandfathered `null`/v3 profile. Apple maps disk-full vs generic write failures to storage-full vs save-failed.
- Revoke is fail-closed and deterministic: on success the app's execution session is invalidated **before** the WebView is destroyed, both inside the locked transaction; on failure nothing is torn down and the tool stays On.
- The three native trust alert strings and three rebuild-status strings match the spec byte-for-byte, are announced once, and return focus to the originating action; no token/codec/raw-storage language is exposed. App-data failures cross the bridge as a typed category (no native app-data copy — that is WU-007+).
- A finalize-invariant failure (trust or app-data) closes and rebuilds the profile from durable state without rolling a durable decision backward.
- Fault injection covers before/after prepare, persist, finalize, session-invalidation, WebView-destruction, process-termination, and profile-rebuild for grant and revoke, plus the app-data boundaries; every reported failure preserves both live and restarted values. On Android the host-JVM suite covers every boundary reachable without a device; the WebView-destruction/process-termination boundaries additionally land as instrumented tests that are compile-green here with execution deferred to CI/device (recorded blocker, not silently skipped).
- The `androidTest` source set assembles (repair already landed in #116); the instrumented run is recorded as a CI/device blocker.
- `fmt`/`clippy`/`cargo test --workspace` stay green (no Rust change); iOS `RiotKit`, macOS `RiotKit-macOS`, Android host-JVM all green; coverage floors hold; scope audit clean.
- Discard routing is exactly as specified: discards issued through the host-held `AppRuntimeSession` under the transaction lock; no UniFFI signature added.

## Explicitly deferred

- **WU-002P:** Tools-listing presentation — `Redesigned · Version 2` / `<name> · Legacy 1` cards, the collapsed **Legacy tools (Version 1)** section, install confirmation warning, and install count-full vs storage-full copy (spec §"Existing-user presentation").
- **WU-007+:** the microapp inline state copy rendered by the `_shared` helper for app-data write failures ("…Your draft is still here…"); WU-002c only supplies the typed category over the bridge.
- **Android instrumented execution:** the `connectedDebugAndroidTest` fault-injection/restart run requires a device/CI runner; the tests land compile-green and the run is recorded as a required CI blocker.
