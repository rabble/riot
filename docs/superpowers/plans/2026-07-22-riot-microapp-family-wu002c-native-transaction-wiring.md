# WU-002c — Native prepare/persist/finalize wiring + alert copy + fault injection (iOS/macOS/Android) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: metaswarm orchestrated-execution (4-phase loop). Steps use checkbox (`- [ ]`) syntax. Parent: `2026-07-22-riot-microapp-family-master-plan.md`. Spec: `docs/superpowers/specs/2026-07-22-riot-microapp-family-design.md` §"Durable trust and app-data transactions", §"State, failure, and conflict behavior", §"Existing-user presentation". Third and last of the three WU-002 units (002a trust seam #102 · 002b app-data seam #106 · **002c native wiring + alert copy + fault injection**). Builds on WU-001N (#, commit `09df242`) which exposes the Android `encodedSize` preflight and durable generation marker.

**Goal:** Wire the already-landed Rust two-phase seams into the iOS, macOS, and Android hosts so trust grant/revoke and every app-data write run **prepare → (host durably persists) → finalize** under one profile authority/persistence lock, with the durable write as the linearization point. Deliver persist-first ordering, the Android `encodedSize` capacity preflight, session-invalidation-before-WebView-destroy on revoke, the exact native trust storage-full/save-failed/rebuild alert copy, a typed app-data failure category across the JS bridge, and fault-injection tests at every transaction boundary. No Rust ABI change; no visual redesign.

**Architecture:** The Rust FFI already exposes everything this WU consumes — `AppRuntimeSession.prepareAppTrust/finalizeAppTrust/discardPreparedTrust/prepareAppDataPut/finalizeAppDataPut/discardPreparedAppData` and `AppExecutionSession.prepareAppExecutionPut/finalizeAppExecutionPut` (verified in `crates/riot-ffi/src/apps_ffi.rs:283-396`). WU-002c is **native-only**: it inverts the current commit-first ordering in `ProfileRepository.swift` (Apple) and `RiotController.kt` + `UniffiAppDataPort` (Android) into prepare → persist → finalize, introduces an explicit persistence lock on Apple to match Android's `persistLock`, routes the durable growth through Android's `PersistedProfileCodec.encodedSize`, and adds a test-only fault-injection seam in each host's durable-save path so every boundary can be exercised.

**Tech Stack:** Swift 6/SwiftUI + Foundation (Apple, shared iOS/macOS sources), Kotlin 2.2/JVM binary codec + Android Views (no Compose), XCTest (RiotTests / RiotKitTests-macOS), JUnit host-JVM (`:app:testDebugUnitTest`) + instrumented (`connectedDebugAndroidTest`).

---

## Scope boundary (do NOT exceed)

**IN scope**
- Apple (shared source): explicit persistence lock in `ProfileRepository.swift` spanning prepare→persist→finalize; persist-first inversion for trust grant, trust revoke, and app-data; storage-full detection via atomic-write failure; session-invalidation-before-WebView-destroy on revoke; the three native trust alert strings + three rebuild-status strings; finalize-invariant-failure profile rebuild; a test-only fault-injection hook in `ProtectedProfileStorage.save`.
- Android: route trust grant/revoke and app-data through `mutatePersisted`/`persistLock` in prepare→persist→finalize order; call `PersistedProfileCodec.encodedSize(prospective)` as the pre-mutation capacity preflight (reject storage-full before any core/disk mutation; never materialize a generation marker on a grandfathered `null`/v3 profile); session-invalidation-before-WebView-destroy on revoke; the same native trust/rebuild alert copy; a test-only fault-injection hook in the store save path.
- Both: a **typed** app-data failure category (`storageFull` / `devicePersistence` / `generic`) returned across the JS bridge so a v2 page can later render the correct inline copy (the inline strings themselves are WU-007+ web work).
- Repair the two drifted `androidTest` files so the `androidTest` source set compiles (Task 0).

**OUT of scope (do NOT touch)**
- **WU-002P** owns all Tools-listing presentation: `Redesigned · Version 2` / `<name> · Legacy 1` cards, the collapsed **Legacy tools (Version 1)** section, the install confirmation warning, and the **install** count-full vs storage-full copy. WU-002c introduces no Legacy/v2 card layout and no install-admission copy.
- **WU-007+** owns the microapp inline state copy ("This profile's offline storage is full. Your draft is still here…", "Riot couldn't save that on this device…") rendered by the web `_shared` helper. WU-002c only returns the failure *category* across the bridge and unit-tests that mapping; it renders none of that web copy.
- No Rust core/FFI/`crates/**` change, no fixture bytes, no theme/font/toolbar files, no `starterCatalogGeneration` semantics change. If any UniFFI signature is found to be genuinely required (it should not be), STOP and re-scope — do not add ABI in this WU.

**Verified anchors (worktree `feat/microapp-family-wu001n`, origin/main incl. #106 + `09df242`)**
- FFI two-phase surface: `apps_ffi.rs:294` `prepare_app_trust`, `:305` `finalize_app_trust`, `:311` `discard_prepared_trust`, `:359` `prepare_app_data_put`, `:369` `finalize_app_data_put`, `:374` `discard_prepared_app_data`, `:183` `prepare_app_execution_put`, `:195` `finalize_app_execution_put`.
- Apple (shared): `apps/ios/Riot/Core/ProfileRepository.swift` — `ProtectedProfileStorage.save(_:) throws` `:234` (the ONLY durable write; no lock exists); `trustApp(appID:)` `:884` (Rust-first then save); `untrustApp(appID:)` `:896`; `persistAppDataBundle(_:) throws` `:944`; `appDataPut(...)` `:953`; `appDataBridge(appID:) -> AppDataBridging?` `:924`; open-time trust re-issue loop `:493`; app-data replay loop `:512`; `PersistedProfile.trustedAppIDs` `:114`, `.appDataBundles` `:120`. `RecoveryReport` in `apps/ios/Riot/Core/RecoveryQuarantine.swift:262`. Bridge put path `apps/ios/Riot/Apps/AppBridgeController.swift:96` (`AppRuntimeDataBridge.put` → `execution.appDataPutWithReceipt` → `onCommitted`); `teardownSession()` `:132` (`execution.invalidate()`). WebView teardown `apps/ios/Riot/Apps/AppRuntimeView.swift:735` (`tearDown`, destroys `AppExecutionSession`); mount state `apps/ios/Riot/CommunityShell.swift:307` (`AppRuntimeMountState.replace()`/`tearDownNow()`); invalidation notification path `AppRuntimeView.swift:248/:364`. VM: `apps/ios/Riot/AppModel.swift:1325` `trustApp`, `:1365` `untrustApp`.
- Android: `apps/android/app/src/main/kotlin/org/riot/evidence/RiotController.kt` — `persistLock` `:51`, `mutatePersisted` `:501`, `mutatePersistedIfPresent` `:509`, `persist(...)` `:516`, `onAppTrusted`→`recordAppTrust` `:169`, `onAppUntrusted`→`recordAppUntrust` `:173`, `onAppDataCommitted`→`recordAppData` `:181`, `openAppRuntime()` `:147`, `openAppExecution(appIdHex)` `:155`. Trust wiring `apps/android/app/src/main/kotlin/org/riot/evidence/apps/RiotAppsController.kt:64` `trust`, `:73` `untrust`. App-data port `apps/android/app/src/main/kotlin/org/riot/evidence/apps/AppDataPort.kt:47` `UniffiAppDataPort(execution: AppExecutionSession, onCommitted)`; `put` `:51`. WebView host `apps/android/app/src/main/kotlin/org/riot/evidence/apps/AppWebViewHost.kt:129` `destroy()` (bridge teardown then `webView.destroy()`); mount/close `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt:430` `openApp`, `:459` `closeApp`, row states `:285` `showTools`, `:415` `showAppReview`. Preflight: `apps/android/app/src/main/kotlin/org/riot/evidence/PersistedProfile.kt:295` `internal fun encodedSize`, ceiling `MAX_ENCODED_BYTES = 4*1024*1024-64` `:71`.

---

## File responsibilities

| File | Responsibility |
| --- | --- |
| `apps/ios/Riot/Core/ProfileRepository.swift` | Add explicit persistence lock; invert trust grant/revoke + app-data to prepare→persist→finalize; storage-full detection; test-only fault-injection hook on the save path; finalize-invariant rebuild |
| `apps/ios/Riot/Apps/AppBridgeController.swift` | Rewrite `AppRuntimeDataBridge.put` to prepare→persist→finalize via `AppExecutionSession`; return typed failure category |
| `apps/ios/Riot/Apps/AppRuntimeView.swift` / `apps/ios/Riot/CommunityShell.swift` | Deterministic session-invalidate-before-WebView-destroy on revoke (invoked inside the locked revoke transaction, not lazily) |
| `apps/ios/Riot/AppModel.swift` | Transaction transient state (Turning on…/Turning off… that blocks launch), native alert copy + announce-once + focus return, rebuild status |
| `apps/android/.../RiotController.kt` | Prepare→persist→finalize under `persistLock`; `encodedSize` capacity preflight; no marker materialization; fault-injection hook; rebuild |
| `apps/android/.../apps/RiotAppsController.kt` | Replace direct `trustApp`/`untrustApp` with the prepare/persist/finalize sequence + typed failures |
| `apps/android/.../apps/AppDataPort.kt` | `UniffiAppDataPort.put` → prepare→persist→finalize via `AppExecutionSession`; typed failure |
| `apps/android/.../MainActivity.kt` | Transient row state, session-invalidate-before-destroy on revoke, native alert copy, rebuild status |
| `apps/android/.../apps/AppPersistenceRestartTest.kt`, `AppRuntimeEndToEndTest.kt` | Task 0: repair drifted `UniffiAppDataPort` construction so `androidTest` compiles |
| XCTest / JUnit test files | RED-first tests for every boundary; fault-injection coverage |

---

## Design (read before Task 0)

**One authority/persistence lock per host.** Android already has `persistLock` + `mutatePersisted` (read-modify-write) — the transaction runs inside one `mutatePersisted { … }` closure. Apple has **no lock**; add an explicit `NSRecursiveLock` (or a private serial mechanism) held by `RiotProfileRepository` for the whole prepare→persist→finalize triple. Apple bridge callbacks already run on the main thread, so this lock makes the existing de-facto serialization explicit and testable; it must not be re-entered by WebView teardown (teardown performs no persistence).

**Persist-first ordering (the inversion).** Today both hosts are commit-first: Apple `trustApp` = `appRuntime.trustApp` (commits) then `storage.save`; app-data = `appDataPutWithReceipt` (commits) then `persistAppDataBundle`. WU-002c inverts to:

```
grant(appID):                       revoke(appID):
  lock                                lock
  row = Turning on… (block launch)    row = Turning off… (block launch + block bridge)
  prepare_app_trust(appID, true)      prepare_app_trust(appID, false)
  [Android] encodedSize preflight     [Android] encodedSize preflight (shrink/zero-growth → passes)
    on prospective trusted-ID set       on prospective trusted-ID set
  persist trusted-ID set (atomic)     persist trusted-ID set with ID removed (atomic)
    fail → discard + storage/save alert  fail → discard + revoke save-failed alert (still On)
  finalize_app_trust()                finalize_app_trust()   (bumps generation → invalidates sessions)
  row = Open                          invalidate mounted execution session for appID
  unlock                              destroy the mounted WebView
                                      unlock; row leaves to Tools
```

App-data write (page or host path):

```
put(app, key, value):
  lock
  prepare_app_execution_put(key, value)  (or prepare_app_data_put for the ungated host path) → receipt
  [Android] encodedSize preflight on prospective (app_id,key) receipt replacement
  persist receipt (atomic)  fail → discard + return typed failure (storageFull | devicePersistence)
  finalize_app_execution_put()
  unlock; return ok
```

The durable write is the linearization point: a crash before finalize replays exactly the persisted trusted-ID set (re-issued per WU-002a) or the persisted receipt (`replayAppDataBundle`, WU-002b) on restart, never double-applying.

**Storage-full vs save-failed.** Android detects storage-full *before* mutation via `encodedSize(prospective) > MAX_ENCODED_BYTES`; any other failure is a save-failed. Apple has no codec ceiling, so storage-full surfaces only as an atomic-write failure — Apple maps a write failure to the save-failed strings, and (for app-data across the bridge) to `devicePersistence`; a genuine disk-full `NSError` (`NSFileWriteOutOfSpaceError` / `ENOSPC`) maps to `storageFull`. This keeps "the two conditions are never collapsed" true on both hosts.

**Native alert copy (this WU owns these exact strings — trust + rebuild only).** From spec §"Durable trust and app-data transactions". `<name>` is the tool's display name.
- Grant storage-full: `This device's offline storage is full, so Riot couldn't turn on <name>. The tool is still off and your tools did not change.`
- Grant other-persistence-failure: `Riot couldn't save that change on this device. <name> is still off. Try again.`
- Revoke persistence failure: `Riot couldn't save that change on this device. <name> is still on. Try again.`
- Rebuild after finalize-invariant failure (one announced status + focus in Tools):
  - `<name> was turned on. Riot reopened this profile to finish safely.`
  - `<name> was turned off. Riot reopened this profile to finish safely.`
  - `Your change was saved. Riot reopened this profile to finish safely.`

Each alert is **announced once**, focus returns to the originating **Turn on**/**Turn off** action, and a user-activated retry reruns the whole transaction. Never expose token/transaction/codec/raw-storage language.

**App-data failure copy is web, not native.** The State-table inline strings render inside the microapp (WU-007+). WU-002c returns only the failure *category* over the bridge and unit-tests the mapping; it renders no app-data alert copy natively.

**Fault-injection seam.** Add a test-only hook in each host's durable-save function that can (a) throw a chosen error (storage-full vs generic) and (b) simulate process termination by aborting *after* the atomic write commits but *before* returning to the caller (so the next open must converge). On Apple wrap `ProtectedProfileStorage.save`; on Android wrap the `store.save`/`persist` path (reuse the existing `encodeWithHooksForTest` style). Boundaries to cover for both grant and revoke: before-prepare, after-prepare, before-persist, after-persist(=post-commit termination), before-finalize, after-finalize, session-invalidation, WebView-destruction, process-termination, profile-rebuild; for app-data: before/after prepare, before/after persist, before/after finalize.

**Scope decision — transient row states.** The `Turning on…`/`Turning off…` labels and the block-launch-during-transaction / return-to-Review-or-Open-on-failure behavior are the transaction's *observable contract* (spec pins them in the transaction section), so they are IN this WU. The Legacy/v2 card redesign and install copy remain WU-002P.

**Scope decision — the two drifted `androidTest` files (Task 0, IN scope).** `AppPersistenceRestartTest.kt` and `AppRuntimeEndToEndTest.kt` fail to compile because they call the removed `UniffiAppDataPort(AppRuntimeSession, appId[, onCommitted])` constructor (app-data I/O moved onto the gated `AppExecutionSession` — current ctor is `UniffiAppDataPort(execution: AppExecutionSession, onCommitted)`). These are app-runtime persistence/restart/end-to-end tests squarely in 002c's surface: 002c changes exactly the persist-first ordering + receipt persistence they assert, and 002c must add its new instrumented fault-injection tests *beside* them — which is impossible while the source set won't compile. The fix is mechanical (open an execution session via `controller.openAppExecution(appIdHex)` / `profile.openAppExecution(appId)` and construct the port over it), independent of the new seam, so it lands first as Task 0 rather than as a separate WU. **Constraint:** Android instrumented tests cannot run in this environment (no device). Task 0's success criterion is therefore **`androidTest` compiles** (`assembleDebugAndroidTest`), with the run recorded as a CI/device-required blocker — do not delete or `@Ignore` the tests, and do not downgrade them to compile-only assertions.

---

## Task 0: Repair drifted `androidTest` so the source set compiles (pre-task)

**Files:** `apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppPersistenceRestartTest.kt`, `apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppRuntimeEndToEndTest.kt`.

- [ ] **Step 1: Confirm RED (compile failure).** Run the androidTest compile and observe the unresolved constructor:

```text
cd apps/android
JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
ANDROID_HOME=$HOME/Library/Android/sdk \
./gradlew :app:assembleDebugAndroidTest
```

Expected: FAIL — `UniffiAppDataPort(AppRuntimeSession, String, …)` / `(AppRuntimeSession, String)` unresolved (`AppPersistenceRestartTest.kt:45-49,:65`; `AppRuntimeEndToEndTest.kt:68`).

- [ ] **Step 2: Repair mechanically.** In each file, obtain a scoped `AppExecutionSession` via `controller.openAppExecution(appId)` (or `profile.openAppExecution(appId)`) and construct `UniffiAppDataPort(execution = <session>, onCommitted = { key, bundle -> controller.onAppDataCommitted(appId, key, bundle) })`. The port's methods are now single-arg (`get(key)`, `put(key, value)`), so drop the `appId` argument at call sites. Preserve each test's original assertion (persist/restart convergence, end-to-end round-trip); do not change what they prove. `AppRuntimeEndToEndTest.kt:96` `session.appDataGet(appId, key)` on `AppRuntimeSession` still exists — leave it.

- [ ] **Step 3: Confirm GREEN compile.** Re-run `:app:assembleDebugAndroidTest` → BUILD SUCCESSFUL. Record `adb devices` output; note the instrumented **run** as a CI/device blocker (cannot execute here). Do NOT `@Ignore`.

- [ ] **Step 4: Commit**

```bash
git add apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppPersistenceRestartTest.kt \
        apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppRuntimeEndToEndTest.kt
git commit -m "test(android): repair UniffiAppDataPort drift so androidTest compiles (WU-002c Task 0)"
```

---

## Task 1: Apple — persistence lock + persist-first trust grant + native alerts

**Files:** `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/Riot/AppModel.swift`; tests `apps/ios/RiotTests/AppRepositoryTests.swift` (+ register any new test file in BOTH `apps/ios/Riot.xcodeproj` and `apps/macos/Riot.xcodeproj`).

- [ ] **Step 1: Write failing tests** in `AppRepositoryTests.swift` (use a durable-DB profile so the real persist path runs; mirror `testTrustSurvivesReopen`):
  - `grant_persists_trusted_id_before_core_finalize` — after a grant, the trusted-ID set is on disk AND `isAppTrusted` is true; assert via an injected fault that **failing the persist leaves both disk and core untrusted** (prepare produced no commit).
  - `grant_storage_full_returns_to_review_and_reports_no_change` — inject a disk-full (`NSFileWriteOutOfSpaceError`) failure on the grant persist; assert trust stays OFF, the trusted-ID set is unchanged on disk, and the surfaced error equals the exact grant storage-full string.
  - `grant_other_save_failure_uses_the_still_off_copy` — inject a generic write failure; assert the exact grant save-failed string and trust OFF.

- [ ] **Step 2: Run RED** (focused):

```text
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/AppRepositoryTests
```

- [ ] **Step 3: Implement.**
  1. Add `private let persistenceLock = NSRecursiveLock()` to `RiotProfileRepository`; add a `withPersistenceLock<T>(_ body: () throws -> T) rethrows -> T` helper wrapping lock/unlock. Add a test-only injection hook to `ProtectedProfileStorage.save` (e.g. `var faultHook: ((Data) throws -> Void)?` invoked before the atomic write) exposed only to tests.
  2. Rewrite `trustApp(appID:)` to the inversion under `withPersistenceLock`: `try appRuntime.prepareAppTrust(appId:, trusted: true)` → build the prospective `trustedAppIDs` (append) → `try storage.save(prospectiveProfile)` → on save success `try appRuntime.finalizeAppTrust()` and commit `persisted = prospectiveProfile`; on save throw call `appRuntime.discardPreparedTrust()` and rethrow a typed `TrustPersistError` carrying storage-full vs save-failed.
  3. In `AppModel.trustApp(appID:)` map `TrustPersistError` to the exact grant alert strings, announce once, and return focus to the originating action.

- [ ] **Step 4: Run GREEN** (iOS focused above) then the shared macOS scheme:

```text
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS' \
  -only-testing:RiotKitTests-macOS/AppRepositoryTests
```

- [ ] **Step 5: Commit** (pathspec: the two source files + test file + any pbxproj registration).

---

## Task 2: Apple — persist-first revoke, fail-closed, session-invalidate-before-destroy

**Files:** `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/Riot/AppModel.swift`, `apps/ios/Riot/Apps/AppRuntimeView.swift` / `apps/ios/Riot/CommunityShell.swift`; tests `AppRepositoryTests.swift`, `AppBreadcrumbTests.swift` (teardown ordering).

- [ ] **Step 1: Write failing tests:**
  - `revoke_persists_removal_before_core_finalize` — durable trusted-ID set has the ID removed and core is untrusted after revoke.
  - `revoke_save_failure_keeps_tool_on_and_reports_no_change` — inject a persist failure; assert trust stays ON, the mounted runtime is untouched, and the surfaced error equals the exact revoke save-failed string.
  - `revoke_invalidates_session_before_destroying_webview` — with a mounted app, assert the execution session is invalidated **before** the WebView teardown runs (observe ordering via a spy on the mount-state teardown vs `AppExecutionSession.isValid()`), and both occur before the lock is released.

- [ ] **Step 2: Run RED** (iOS focused).
- [ ] **Step 3: Implement.** Rewrite `untrustApp(appID:)` under `withPersistenceLock`: `prepareAppTrust(appId:, trusted: false)` → persist trusted-ID set with the ID removed → on failure `discardPreparedTrust()` + rethrow revoke save-failed (tool stays On); on success `finalizeAppTrust()` (bumps generation → invalidates execution sessions) → then **deterministically** invalidate the mounted `AppExecutionSession` for `appID` and tear down its WebView (drive `AppRuntimeMountState.tearDownNow()` / the bridge `teardownSession()`), all before returning/unlocking. Keep the existing invalidation-notification path as a fallback for out-of-band revokes but do not rely on it for the ordered teardown.
- [ ] **Step 4: Run GREEN** (iOS + macOS focused).
- [ ] **Step 5: Commit.**

---

## Task 3: Apple — app-data prepare/persist/finalize + typed bridge failure

**Files:** `apps/ios/Riot/Apps/AppBridgeController.swift`, `apps/ios/Riot/Core/ProfileRepository.swift`; tests `AppRuntimeHostTests.swift`, `AppRepositoryTests.swift`.

- [ ] **Step 1: Write failing tests:**
  - `app_data_put_persists_receipt_before_core_finalize` — after a bridge `put`, the receipt is on disk and `appDataGet` returns the value; injecting a persist failure leaves the store unmutated (value not readable) and the prepared token discarded.
  - `app_data_put_disk_full_returns_storageFull_category` and `app_data_put_write_failure_returns_devicePersistence_category` — assert the bridge result carries the correct typed category (no native alert copy rendered here).
  - Keep `testBridgePutPersistsAcrossReopen` / `testAppDataSurvivesReopen` green (byte-identical receipt).

- [ ] **Step 2: Run RED** (iOS focused: `-only-testing:RiotTests/AppRuntimeHostTests`).
- [ ] **Step 3: Implement.** Rewrite `AppRuntimeDataBridge.put(key:valueJSON:)` to call, under the repository lock: `execution.prepareAppExecutionPut(key:, value:)` → `persistAppDataBundle(receipt)` → `execution.finalizeAppExecutionPut()`; on persist failure `execution.discardPreparedAppData()` (or the runtime `discardPreparedAppData`) and return a typed `AppDataFailure` category (`storageFull`/`devicePersistence`/`generic`). Update the ungated host `appDataPut(...)` similarly via `prepareAppDataPut`/`finalizeAppDataPut`. The receipt persisted is the exact bytes prepare returns (WU-002b guarantees they equal today's commit output).
- [ ] **Step 4: Run GREEN** (iOS + macOS focused).
- [ ] **Step 5: Commit.**

---

## Task 4: Apple — finalize-invariant rebuild + fault injection at every boundary

**Files:** `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/Riot/AppModel.swift`; tests new `apps/ios/RiotTests/AppTransactionFaultTests.swift` (register in both pbxproj + the macOS test target).

- [ ] **Step 1: Write failing tests** driving the fault hook for **grant and revoke** at each boundary (before/after prepare, before/after persist, before/after finalize, session-invalidation, WebView-destruction, process-termination, profile-rebuild) and for **app-data** (before/after prepare, persist, finalize). Assertions:
  - No reported failure changes live or restarted trust/app-data values (reopen and read the prior committed value).
  - A post-commit termination (fault after the atomic write, before finalize) converges on reopen: trust/app-data reflects the durable state exactly once, and Tools shows exactly one of the three rebuild-status strings with focus in Tools.
  - An unexpected finalize-invariant failure closes the profile and rebuilds from the already-durable snapshot before any tool can reopen; no durable decision is rolled backward.

- [ ] **Step 2: Run RED.**
- [ ] **Step 3: Implement** the rebuild path: on a finalize-invariant failure, close the profile and rebuild runtime state from the durable snapshot (reuse the `static open(...)` recovery pipeline + `RecoveryReport`), then surface exactly one rebuild-status string via the announce-once path in `AppModel`.
- [ ] **Step 4: Run GREEN** (iOS + macOS): run the full `RiotTests` and `RiotKitTests-macOS` suites; confirm no regression beyond the known pre-existing red guide tests.
- [ ] **Step 5: Commit.**

---

## Task 5: Android — persist-first trust grant under `persistLock` + `encodedSize` preflight + alerts

**Files:** `apps/android/.../RiotController.kt`, `apps/android/.../apps/RiotAppsController.kt`, `apps/android/.../MainActivity.kt`; tests `apps/android/app/src/test/kotlin/org/riot/evidence/apps/RiotAppsControllerTest.kt`, `AppPersistenceTest.kt`.

- [ ] **Step 1: Write failing host-JVM tests** (the `FakeAppRuntimeSession` already implements the prepare/finalize interface, `RiotAppsControllerTest.kt:46-81`):
  - grant runs prepare → persist → finalize in order and blocks launch until finalize;
  - grant calls `PersistedProfileCodec.encodedSize(prospective)` before any mutation; a prospective set exceeding `MAX_ENCODED_BYTES` is rejected with the grant storage-full string and **no** core/disk mutation and **no** generation-marker materialization on a grandfathered `null`/v3 profile;
  - a generic persist failure yields the grant save-failed string, trust OFF.

- [ ] **Step 2: Run RED:**

```text
cd apps/android
JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
ANDROID_HOME=$HOME/Library/Android/sdk \
./gradlew :app:testDebugUnitTest --tests org.riot.evidence.apps.RiotAppsControllerTest
```

- [ ] **Step 3: Implement.** In `RiotAppsController.trust`, replace `session.trustApp(...)` with: `prepareAppTrust(appId, true)` → hand the prospective trusted-ID set to the controller's persist seam, which runs inside `mutatePersisted { snapshot -> … }` and calls `PersistedProfileCodec.encodedSize(prospective)` before committing (reject storage-full pre-mutation; preserve `starterCatalogGeneration` exactly — never materialize on `null`) → on persist success `finalizeAppTrust()`; on failure `discardPreparedTrust()` + typed error. Surface the exact alert strings from `MainActivity` and add the `Turning on…`/block-launch transient row state to `showTools`/`showAppReview`.
- [ ] **Step 4: Run GREEN** (host-JVM focused, then full `:app:testDebugUnitTest`).
- [ ] **Step 5: Commit.**

---

## Task 6: Android — persist-first revoke + session-invalidate-before-destroy

**Files:** `apps/android/.../RiotController.kt`, `apps/android/.../apps/RiotAppsController.kt`, `apps/android/.../MainActivity.kt`, `apps/android/.../apps/AppWebViewHost.kt`; tests `RiotAppsControllerTest.kt`, `AppPersistenceTest.kt`, `RiotJsBridgeTest.kt`.

- [ ] **Step 1: Write failing tests:** revoke persists the removal before finalize; a persist failure keeps the tool ON with the exact revoke save-failed string and an untouched runtime; on success the mounted execution session is invalidated **before** `AppWebViewHost.destroy()` and both happen before the lock is released.
- [ ] **Step 2: Run RED** (focused `:app:testDebugUnitTest --tests …`).
- [ ] **Step 3: Implement.** Rewrite `RiotAppsController.untrust` to `prepareAppTrust(appId, false)` → persist removal (under `persistLock`, `encodedSize` preflight trivially passes on shrink) → `finalizeAppTrust()` → invalidate the mounted `AppExecutionSession` for `appId` then `runningApp?.destroy()` (ordered) before releasing the lock; wire `MainActivity` "Turn off" to the transient `Turning off…` block-launch state.
- [ ] **Step 4: Run GREEN.**
- [ ] **Step 5: Commit.**

---

## Task 7: Android — app-data prepare/persist/finalize + typed failure + fault injection + rebuild

**Files:** `apps/android/.../apps/AppDataPort.kt`, `apps/android/.../RiotController.kt`, `apps/android/.../MainActivity.kt`; tests `apps/android/app/src/test/kotlin/org/riot/evidence/apps/RiotJsBridgeTest.kt`, `PersistedProfileCodecTest.kt` (preflight), and instrumented `AppPersistenceRestartTest.kt` / `AppRuntimeEndToEndTest.kt` (compile-only here, CI-run).

- [ ] **Step 1: Write failing tests:** `UniffiAppDataPort.put` runs prepare → persist → finalize; a persist failure leaves the store unmutated, discards the prepared token, and returns the typed category (`storageFull` when `encodedSize` preflight rejects, else `devicePersistence`); post-commit termination converges on restart via `replayAppDataBundle`. Add fault-injection host-JVM tests through a store save hook covering the app-data boundaries and the trust boundaries reachable host-side; add the three rebuild-status strings.
- [ ] **Step 2: Run RED** (focused host-JVM).
- [ ] **Step 3: Implement.** Rewrite `UniffiAppDataPort.put(key, value)` to `execution.prepareAppExecutionPut(key, value)` → persist the receipt through the controller (under `persistLock`, `encodedSize` preflight on the prospective `(appId,key)` replacement) → `execution.finalizeAppExecutionPut()`; on failure `execution.discardPreparedAppData?()` + typed category. Add the store fault-injection hook to `RiotController.persist`/`store.save` and the finalize-invariant rebuild (close + rebuild from durable snapshot + one announced status). Update the instrumented tests to the new ordering (compile-verify only).
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
- [ ] **Scope audit:** `git status --short` + `git diff --check`; confirm changed paths are only the files in the responsibilities table; NO `crates/**`, `fixtures/**`, theme/font files. Commit with exact pathspecs (shared checkout — never `git add -A`, never `git stash`).

---

## Definition of Done

- Trust grant, trust revoke, and every app-data write run **prepare → durable persist → finalize** under one authority/persistence lock on each host; the pre-existing commit-first ordering is no longer reachable on Apple or Android.
- Apple has an explicit persistence lock spanning the triple; Android uses `persistLock`/`mutatePersisted`. A concurrent trust/profile change cannot interleave.
- Android calls `PersistedProfileCodec.encodedSize(prospective)` before any durable growth, rejects storage-full **before** core/disk mutation, and never materializes a `starterCatalogGeneration` marker on a grandfathered `null`/v3 profile. Apple maps disk-full vs generic write failures to storage-full vs save-failed.
- Revoke is fail-closed and deterministic: on success the app's execution session is invalidated **before** the WebView is destroyed, both inside the locked transaction.
- The three native trust alert strings and three rebuild-status strings match the spec byte-for-byte, are announced once, and return focus to the originating action; no token/codec/raw-storage language is exposed. App-data failures cross the bridge as a typed category (no native app-data copy — that is WU-007+).
- A finalize-invariant failure closes and rebuilds the profile from durable state without rolling a durable decision backward.
- Fault injection covers before/after prepare, persist, finalize, session-invalidation, WebView-destruction, process-termination, and profile-rebuild for grant and revoke, plus the app-data boundaries; every reported failure preserves both live and restarted values.
- The two drifted `androidTest` files compile (Task 0); the `androidTest` source set assembles; the instrumented run is recorded as a CI/device blocker.
- `fmt`/`clippy`/`cargo test --workspace` stay green (no Rust change); iOS `RiotKit`, macOS `RiotKit-macOS`, Android host-JVM all green; coverage floors hold; scope audit clean.

## Explicitly deferred

- **WU-002P:** Tools-listing presentation — `Redesigned · Version 2` / `<name> · Legacy 1` cards, the collapsed **Legacy tools (Version 1)** section, install confirmation warning, and install count-full vs storage-full copy (spec §"Existing-user presentation").
- **WU-007+:** the microapp inline state copy rendered by the `_shared` helper for app-data write failures ("…Your draft is still here…"); WU-002c only supplies the typed category over the bridge.
- **Android instrumented execution:** the `connectedDebugAndroidTest` fault-injection/restart run requires a device/CI runner; Task 0 + Task 7 land the tests compile-green and record the run as a required CI blocker.
