# iOS Surface — Unit 5: Add-a-tool (iOS tool import) + Tools empty-state action — Implementation Plan


**Plan-review gate: PASSED (Feasibility + Scope + Completeness, 2026-07-18).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Give an organizer a way to add a tool from a file on iOS — the flow Android already has — and stop the Tools tab's empty state from being a dead-end. Today `DirectoryView`'s only empty branch is `RiotEmptyState(title: "No apps yet", …)` with **no action** (`DirectoryView.swift:40`): a person with no tools yet has nowhere to go. This unit adds an **"Add a tool"** affordance (visible in the Tools route header AND therefore in the empty state) → a document picker → `installApp(manifest, bundle)` → the tool appears in Tools **untrusted** → the existing `AppReviewSheet` makes the organizer trust decision. **No auto-trust.**

**Architecture:** Pure-Swift, **no new FFI**. The install wrapper `appRuntime.installApp(manifestBytes:bundleBytes:)` already exists and is exercised at open (`ProfileRepository.swift:646`); this unit adds a thin **public** `RiotProfileRepository.installApp(manifest:bundle:)` that mirrors `getCarriedApp(appID:)` exactly (install → decode/retain → dedup the `installed` registry → persist as a `PersistedAppPack` so it survives relaunch → post `.riotHeldAppsDidChange`), a thin `RiotAppModel.installTool(manifest:bundle:)` forwarder (business logic stays in the repository/FFI), and the `DirectoryView` UI: an organizer-gated "Add a tool" button + two chained `.fileImporter`s (manifest, then bundle) that read security-scoped bytes and hand them to the model. Trust is a **separate** decision left to the already-wired `AppReviewSheet` (`DirectoryView.swift:60-72`).

**Tech stack:** Swift 6 / SwiftUI, `.fileImporter` + `UniformTypeIdentifiers` (new to this app — no existing `.fileImporter`/`UIDocumentPicker` usage anywhere), XCTest. Design: `docs/superpowers/specs/2026-07-18-ios-surface-built-capabilities-design.md` §5 + §7 (Tools empty-state) + §8 Unit 5.

**Shared-checkout:** This unit adds **no new Swift file** — it edits `DirectoryView.swift`, `AppModel.swift`, `ProfileRepository.swift`, and appends tests to the **existing** `ToolsSectionTests.swift` + `DirectoryRepositoryTests.swift`. Both pbxproj serialize file *additions*; because nothing is added, **neither pbxproj is touched** — no COLLABORATION.md claim on the pbxproj is required for this unit. (Both pbxproj use hand-authored fixed IDs and are NOT synchronized groups: `grep -c PBXFileSystemSynchronizedRootGroup` = 0 — a new file WOULD need registration, which is why this unit deliberately avoids one.) Pathspec commits; absolute `git`/`grep`.

---

## Ground truth (verified)

- **The dead-end (the point of the unit).** `DirectoryView.body` renders, when there are no tools:
  ```swift
  if directory.rows.isEmpty {
      RiotEmptyState(
          title: "No apps yet",
          message: "Apps your communities carry will show up here. Nothing runs until an organizer turns one on for a space."
      )                                               // DirectoryView.swift:40-43 — NO action
  } else { … }
  ```
  `RiotEmptyState` itself takes only `title` + `message` and offers **no action affordance** (`RiotEmptyState.swift:8` `init(title:message:)`). Fix by adding the "Add a tool" button to the **always-rendered** part of `DirectoryView`'s `VStack` (before the `if/else`), so it shows in BOTH the empty and populated states — this is the Tools-route-header affordance AND the empty-state action in one.
- **Install FFI wrapper exists, is NOT public, and only runs at `open`.** `RiotProfileRepository.installPack` (`ProfileRepository.swift:641`) calls `let record = try appRuntime.installApp(manifestBytes: manifest, bundleBytes: bundle)` — but it is `private static` and only invoked for the starter catalog + carried apps during `open`. **There is no runtime "install a chosen pair after open" method today.** Unit 5 adds a public one. `appRuntime.installApp(manifestBytes:bundleBytes:)` is the generated FFI on `AppRuntimeSession` — **already exists, so no new FFI.**
- **The exact shape to mirror — `getCarriedApp(appID:)`** (`ProfileRepository.swift:945-974`): admits via the runtime, `Self.retain(record:bundle:)` decodes + entry-point-checks the bundle, dedups the `installed` registry by lowercased app id, writes a `PersistedAppPack` (`appIDHex`, `manifest`, `bundle`) into `persisted.carriedApps` so it survives relaunch, `try storage.save(persisted)`, then `NotificationCenter.default.post(name: .riotHeldAppsDidChange, object: self)`, and returns `try spaceApp(app)`. Its doc-comment states the trust contract verbatim: *"Getting an app turns nothing on. It joins the held apps as UNTRUSTED, so the review sheet still stands between a neighbour's app and a WebView."* — the same contract Unit 5's install must honour. `retain` (`:654`, `private static`) and `spaceApp` (`:508`, `private`) are same-class-accessible.
- **No auto-trust — trust is a separate step.** `spaceApp(_:)` sets `trusted: try appRuntime.isAppTrusted(appId:)` (`:515`) — a freshly installed app is untrusted until `appRuntime.trustApp(appId:)` runs. `RiotProfileRepository.trustApp(appID:)` (`:536`) is the only path that flips it, invoked from `RiotAppModel.trustApp(appID:)` (`AppModel.swift:826`) which is invoked from `AppReviewSheet`'s `onApprove` (`DirectoryView.swift:65-68`). The install path must NOT call `trustApp`.
- **`AppReviewSheet` is already wired into `DirectoryView`** via `.sheet(item: $reviewing)` (`DirectoryView.swift:60-72`), reached from a row's `.review(app)` availability branch: `Button("Review \(row.name)") { reviewing = app }` (`DirectoryView.swift:161-164`). An installed-but-untrusted tool lands in exactly that `.review` branch — so the trust gate is reused with zero new UI. `AppReviewSheet` draws "Let everyone in this space use this" only when `canApprove` (organizer); otherwise the honest unavailable sentence (`AppReviewSheet.swift:77-89`).
- **Organizer predicate + how the view knows.** `RiotAppModel.canApproveApps` is `(try? repository?.isOrganizer()) ?? false` (`AppModel.swift:803`), published (`:795`), refreshed in `refreshOrganizerState()`. `isOrganizer()` forwards to `appRuntime.isOrganizer()` (`ProfileRepository.swift:522`). Gate the "Add a tool" button on `model.canApproveApps` — matching design §5 "(organizer)" and the existing `ShellRecoveryState.noTools(isOrganizer:)` whose `primaryActionLabel` is already `isOrganizer ? "Add a tool" : "Find nearby"` (`CommunityShell.swift:272`).
- **`refreshApps()` is space-gated.** `apps = (try? repository?.spaceApps()) ?? []` (`AppModel.swift:855-857`), and `spaceApps()` returns `[]` while `currentSpace == nil` (`ProfileRepository.swift:493-496`). Because "Add a tool" is organizer-gated and an organizer has a space, `model.apps` will reflect the newly installed tool. Model-layer tests must `createSpace(...)` before `installTool(...)`, exactly as `ToolsSectionTests` does.
- **Android parity (read-only reference).** `MainActivity.kt:234` — `content.addView(action("Add a tool (choose manifest, then bundle)") { startActivityForResult(openDocumentIntent(), PICK_APP_MANIFEST) })`; `openDocumentIntent()` (`:391`) is `ACTION_OPEN_DOCUMENT` / `CATEGORY_OPENABLE` / `type = "application/octet-stream"`. Android picks the **manifest first, then the bundle** (two sequential document picks). iOS mirrors this with **two chained `.fileImporter`s**. The Android empty branch is `body("No tools yet.")` (`:280`) — the same dead-end this unit fixes on iOS.
- **`.fileImporter` bytes-reading — the detail to confirm.** No existing `.fileImporter`/`UIDocumentPicker` usage in the app (grep empty). A `.fileImporter` completion hands back a `URL` that is **security-scoped**: reading it requires `url.startAccessingSecurityScopedResource()` / `defer { url.stopAccessingSecurityScopedResource() }` around `Data(contentsOf: url)`, else the read fails outside the sandbox. This is the one runtime detail to verify on-device/simulator. `.data` UTType requires `import UniformTypeIdentifiers`.
- **Test harness.** Hostless XCTest, `@testable import RiotKit`. Fixtures resolved four `deletingLastPathComponent()` levels up from `#filePath` to `fixtures/apps/checklist.manifest.cbor` + `checklist.bundle.cbor` (`ToolsSectionTests.swift:11-21`, `DirectoryRepositoryTests.swift:13-22`). Model tests drive `RiotAppModel.bootstrap(storageDirectory:keyStore:starterPacks:)` (`ToolsSectionTests` full pattern); repository tests drive `RiotProfileRepository.open(storage:keyStore:starterPacks:)`. **Append** new tests to `ToolsSectionTests.swift` (model) and `DirectoryRepositoryTests.swift` (repository) — no new test file, no pbxproj edit. The doc-picker/`.fileImporter` UI is NOT unit-testable; test the model+repository logic (install given bytes → listing shows it **untrusted**; trust → **trusted**).

---

## Task 1: `RiotProfileRepository.installApp(manifest:bundle:)` — install chosen bytes → held untrusted

**Files:** Modify `apps/ios/Riot/Core/ProfileRepository.swift`; append tests to `apps/ios/RiotTests/DirectoryRepositoryTests.swift`

- [ ] **Step 1: Failing test** (drive the repository against real FFI with the real checklist fixture; assert it appears UNTRUSTED and survives relaunch — no auto-trust):
```swift
func testInstallAppAddsAnUntrustedToolThatSurvivesRelaunch() throws {
    let dir = FileManager.default.temporaryDirectory
        .appendingPathComponent("add-tool-\(UUID().uuidString)")
    let keyStore = TestWrappingKeyStore()   // fixed 32-byte key, per file convention
    let file = dir.appendingPathComponent("profile.json")

    // Open with NO starter packs, then install the fixture pair at runtime.
    let repo = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: file),
        keyStore: keyStore,
        starterPacks: []
    )
    XCTAssertTrue(try repo.installedApps().isEmpty)

    let (manifest, bundle) = try XCTUnwrap(try Self.starterPacks().first)
    let installed = try repo.installApp(manifest: manifest, bundle: bundle)

    // Appears, and appears UNTRUSTED — install turns nothing on.
    XCTAssertEqual(installed.name, "Checklist")
    XCTAssertFalse(installed.trusted, "a freshly added tool must be untrusted until the organizer trusts it")
    XCTAssertEqual(try repo.installedApps().count, 1)
    XCTAssertFalse(try XCTUnwrap(try repo.installedApps().first).trusted)

    // Persisted as a carried pack -> present after a reopen (still untrusted).
    let reopened = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: file),
        keyStore: keyStore,
        starterPacks: []
    )
    XCTAssertEqual(try reopened.installedApps().count, 1)
    XCTAssertFalse(try XCTUnwrap(try reopened.installedApps().first).trusted)
}

func testInstallAppRejectsStructurallyBrokenBytesWithoutRetaining() throws {
    let dir = FileManager.default.temporaryDirectory
        .appendingPathComponent("add-tool-bad-\(UUID().uuidString)")
    let repo = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: dir.appendingPathComponent("profile.json")),
        keyStore: TestWrappingKeyStore(),
        starterPacks: []
    )
    var (manifest, bundle) = try XCTUnwrap(try Self.starterPacks().first)
    bundle.removeLast(bundle.count / 2)   // corrupt the bundle
    XCTAssertThrowsError(try repo.installApp(manifest: manifest, bundle: bundle))
    XCTAssertTrue(try repo.installedApps().isEmpty, "a rejected pair must not be retained")
}
```
> Use the file's existing `starterPacks(file:)` helper + its `TestWrappingKeyStore`. If `DirectoryRepositoryTests` opens differently (it has a `makeRepository` helper around `:128`), reuse that helper's storage/keyStore wiring instead of re-deriving it — keep the two-open reopen explicit as above.

- [ ] **Step 2: Run → FAIL** (`installApp(manifest:bundle:)` undefined). `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/DirectoryRepositoryTests`

- [ ] **Step 3: Implement** — add to the `extension RiotProfileRepository: DirectoryPorting` block, right beside `getCarriedApp` (`ProfileRepository.swift:945`), mirroring it byte-for-byte except the admission call (raw bytes, not an app id already in the directory):
```swift
/// Installs a tool from a manifest+bundle pair the organizer chose from a
/// file — the "Add a tool" flow. Rust's `installApp` is the integrity oracle;
/// we then decode the bundle for serving and confirm its entry point before
/// retaining a resolver. The pair is written to the profile snapshot as a
/// carried pack so it survives a relaunch (the store is in-memory), and the
/// held-apps change is posted so Tools refreshes.
///
/// Installing turns NOTHING on. The tool joins the held apps UNTRUSTED, so the
/// review sheet (`AppReviewSheet`) still stands between it and a WebView —
/// exactly like `getCarriedApp`. There is no auto-trust here by design.
@discardableResult
public func installApp(manifest: Data, bundle: Data) throws -> RiotSpaceApp {
    // Admission first: a pair Rust refuses is never retained or written to disk.
    let record = try appRuntime.installApp(manifestBytes: manifest, bundleBytes: bundle)
    let app = try Self.retain(record: record, bundle: bundle)

    if let existing = installed.firstIndex(where: {
        $0.record.appId.lowercased() == record.appId.lowercased()
    }) {
        installed[existing] = app
    } else {
        installed.append(app)
    }

    let pack = PersistedAppPack(
        appIDHex: record.appId.lowercased(),
        manifest: manifest,
        bundle: bundle
    )
    persisted.carriedApps.removeAll { $0.appIDHex == pack.appIDHex }
    persisted.carriedApps.append(pack)
    try storage.save(persisted)

    // Held and saved; anything showing held apps (the Tools card, the only route
    // to Open) is stale as of this line. Posted after the save so no observer can
    // read a set a later throw would undo — same ordering as getCarriedApp.
    NotificationCenter.default.post(name: .riotHeldAppsDidChange, object: self)

    return try spaceApp(app)
}
```
> `retain`, `spaceApp`, `PersistedAppPack`, and `.riotHeldAppsDidChange` all already exist in this file. This method touches no FFI that `open` doesn't already touch — **no binding regen, no staticlib rebuild** (record-change coupling not triggered).

- [ ] **Step 4: Run → PASS** (both tests). **Step 5: Commit** `apps/ios/Riot/Core/ProfileRepository.swift` + `apps/ios/RiotTests/DirectoryRepositoryTests.swift`.

---

## Task 2: `RiotAppModel.installTool(manifest:bundle:)` — thin forwarder + honest failure

**Files:** Modify `apps/ios/Riot/AppModel.swift`; append tests to `apps/ios/RiotTests/ToolsSectionTests.swift`

- [ ] **Step 1: Failing test** (mirror `ToolsSectionTests.testAppsRefreshAfterSpaceCreationAndTrustFlipsListing` — install shows the tool untrusted, then the trust step flips it; and a bad pair surfaces a message, never a silent no-op):
```swift
func testInstallToolAddsUntrustedToolThenAppReviewTrustPathFlipsIt() throws {
    let model = RiotAppModel()
    model.bootstrap(
        storageDirectory: isolatedDirectory(),
        keyStore: TestWrappingKeyStore(),
        starterPacks: []                       // start with NO tools -> the empty state
    )
    model.createSpace(title: "Berlin Mutual Aid")
    XCTAssertTrue(model.apps.isEmpty)          // the dead-end this unit fixes

    let (manifest, bundle) = try XCTUnwrap(try Self.starterPacks().first)
    model.installTool(manifest: manifest, bundle: bundle)

    XCTAssertEqual(model.apps.count, 1)
    XCTAssertEqual(model.apps[0].name, "Checklist")
    XCTAssertFalse(model.apps[0].trusted, "installed via Add-a-tool must be untrusted until AppReviewSheet trusts it")
    XCTAssertNil(model.errorMessage)

    // The AppReviewSheet trust decision — the same path DirectoryView wires to onApprove.
    model.trustApp(appID: model.apps[0].appIDHex)
    XCTAssertTrue(model.apps[0].trusted)
    XCTAssertNil(model.errorMessage)
}

func testInstallToolWithBrokenBytesSurfacesAMessageNotASilentNoOp() throws {
    let model = RiotAppModel()
    model.bootstrap(storageDirectory: isolatedDirectory(),
                    keyStore: TestWrappingKeyStore(), starterPacks: [])
    model.createSpace(title: "Berlin Mutual Aid")

    var (manifest, bundle) = try XCTUnwrap(try Self.starterPacks().first)
    bundle.removeLast(bundle.count / 2)
    model.installTool(manifest: manifest, bundle: bundle)

    XCTAssertTrue(model.apps.isEmpty)
    XCTAssertNotNil(model.errorMessage, "a rejected file must say why, never vanish silently (the InvalidInput bug)")
}
```

- [ ] **Step 2: Run → FAIL** (`installTool` undefined). `-only-testing:RiotTests/ToolsSectionTests`

- [ ] **Step 3: Implement** in `RiotAppModel` beside `trustApp` (`AppModel.swift:826`):
```swift
/// Adds a tool the organizer chose from a file, then refreshes Tools so the new
/// tool shows with its "Review" action. Installing turns nothing on — the tool
/// is UNTRUSTED until the organizer approves it in `AppReviewSheet`; this method
/// never trusts. A rejected file surfaces a plain message rather than the silent
/// no-op that let a failed install "just not appear".
public func installTool(manifest: Data, bundle: Data) {
    guard let repository else { return }
    do {
        _ = try repository.installApp(manifest: manifest, bundle: bundle)
        errorMessage = nil
        refreshApps()
    } catch {
        errorMessage = Self.toolImportFailureMessage(error)
    }
}

/// Why a chosen file could not be added as a tool, in words a person can act on.
static func toolImportFailureMessage(_ error: Error) -> String {
    "That file couldn’t be added as a tool. Choose the tool’s manifest, then its bundle."
}
```
> Do NOT reuse `approvalFailureMessage` — that copy is about the organizer trust gate, not a malformed file. Keep `installTool` a pure forwarder (business logic stays in the repository/FFI). No `refreshOrganizerState()` needed — installing does not change organizer status.

- [ ] **Step 4: Run → PASS**. **Step 5: Commit** `apps/ios/Riot/AppModel.swift` + `apps/ios/RiotTests/ToolsSectionTests.swift`.

---

## Task 3: `DirectoryView` — "Add a tool" button (header + empty-state) + chained `.fileImporter`s

**Files:** Modify `apps/ios/Riot/Directory/DirectoryView.swift`

No pure-view unit test (SwiftUI body + `.fileImporter` are not unit-testable); the install→appears→trust logic is covered by Task 1/Task 2. This task is the wiring, and the **anti-dead-end guarantee**: the button renders in the always-visible part of the `VStack`, so it is present in BOTH the empty and populated branches. Manual/build verification in Task 4.

- [ ] **Step 1: Implement.**
  - Add `import UniformTypeIdentifiers` at the top (for `UTType.data`).
  - Add state to `DirectoryView`:
    ```swift
    @State private var isImportingManifest = false
    @State private var isImportingBundle = false
    @State private var pendingManifest: Data?
    ```
  - In `body`, add the button to the **always-rendered** part of the `VStack` — after `status`, **before** the `if directory.rows.isEmpty` (`DirectoryView.swift:38-49`), so it shows in the empty state AND above the populated list:
    ```swift
    status
    if model.canApproveApps {
        Button("Add a tool") { isImportingManifest = true }
            .buttonStyle(.riotSecondary)
            .accessibilityIdentifier("directory-add-tool")
    }
    if directory.rows.isEmpty {
        RiotEmptyState(title: "No apps yet", message: …)   // unchanged copy
    } else { … }
    ```
  - Attach two chained `.fileImporter`s to the `ScrollView` (alongside the existing `.sheet(item: $reviewing)`), mirroring Android's manifest-then-bundle order:
    ```swift
    .fileImporter(isPresented: $isImportingManifest, allowedContentTypes: [.data]) { result in
        guard case let .success(url) = result, let bytes = Self.readSecurityScoped(url) else { return }
        pendingManifest = bytes
        isImportingBundle = true            // now pick the bundle
    }
    .fileImporter(isPresented: $isImportingBundle, allowedContentTypes: [.data]) { result in
        defer { pendingManifest = nil }
        guard case let .success(url) = result,
              let manifest = pendingManifest,
              let bundle = Self.readSecurityScoped(url) else { return }
        model.installTool(manifest: manifest, bundle: bundle)
        directory.refresh()                 // pull the new (untrusted) row into the list
    }
    ```
  - Add the security-scoped reader (the flagged detail):
    ```swift
    /// A `.fileImporter` URL is security-scoped: reading it outside the sandbox
    /// requires bracketing the read with start/stopAccessingSecurityScopedResource.
    private static func readSecurityScoped(_ url: URL) -> Data? {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        return try? Data(contentsOf: url)
    }
    ```
- [ ] **Step 2:** The new tool arrives untrusted → it renders in the `.review(app)` availability branch (`DirectoryView.swift:161`) → `Button("Review …") { reviewing = app }` → the already-wired `AppReviewSheet` (`:60-72`) makes the trust decision. **Confirm no code change is needed there** — the reuse is the design. (`.fileImporter` works on iOS AND macOS, so no `#if os(iOS)` guard; both targets build.)
- [ ] **Step 3: Commit** `apps/ios/Riot/Directory/DirectoryView.swift`.

---

## Task 4: Build both platforms + verify no pbxproj / no new file

- [ ] Confirm **no new Swift file** was added (`git status` shows only the four edited files + two edited test files). Therefore **neither `apps/ios/Riot.xcodeproj/project.pbxproj` nor `apps/macos/Riot.xcodeproj/project.pbxproj` is modified.** If the implementer chose to extract a helper into a NEW file, STOP and register it in BOTH pbxproj (fixed-ID `PBXFileReference` + per-target `PBXBuildFile` + each `PBXSourcesBuildPhase` `files=()` + the target `PBXGroup` children), claim both pbxproj in COLLABORATION.md first, and `plutil -lint` both — but the plan is written to avoid this.
- [ ] iOS tests green: `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'` — new Task 1/Task 2 tests pass; full RiotKit only the known-red Bonjour two-peer test still red.
- [ ] iOS app **BUILD SUCCEEDED** and macOS app **BUILD SUCCEEDED** (`DirectoryView` is shared; `.fileImporter` + `UniformTypeIdentifiers` are cross-platform).
- [ ] Commit any fixups.

---

## Self-Review

- **Spec coverage (§5 add-a-tool + §7 Tools empty-state + §8 Unit 5):**
  - "Add a tool" button on the Tools route header AND the empty-state action (§5, §7) ✅ Task 3 — one button in the always-rendered `VStack`, present in both branches; the empty-state dead-end (`DirectoryView.swift:40`, no action) is fixed.
  - document picker → `installApp(manifest, bundle)` (§5, §8) ✅ Task 3 (two chained `.fileImporter`s, manifest-then-bundle) → Task 2 (`installTool`) → Task 1 (`RiotProfileRepository.installApp`, wrapping the existing `appRuntime.installApp` FFI).
  - tool appears in Tools ✅ Task 1 (`installedApps()` shows it) / Task 2 (`model.apps` shows it) / Task 3 (`directory.refresh()` renders the row).
  - existing `AppReviewSheet` handles the organizer trust decision, **no auto-trust** (§5, §8) ✅ Task 1 asserts `trusted == false` on install; Task 2 asserts install-untrusted → `trustApp` → trusted via the same path `AppReviewSheet.onApprove` uses; Task 3 confirms the `.review(app)` → `AppReviewSheet` reuse needs no change.
  - mirrors Android's flow (§5) ✅ manifest-then-bundle order + `.data`/octet-stream content type, matching `MainActivity.kt:234/:391`.
  - Anti-dead-end assertion (§7) ✅ Task 2's first test asserts `model.apps.isEmpty` (the empty state) then a reachable `installTool` produces a row — the empty state stops being terminal.
- **No new FFI ✅** `appRuntime.installApp(manifestBytes:bundleBytes:)` already exists and runs at `open` (`ProfileRepository.swift:646`); Unit 5 only exposes it publicly in Swift. No `uniffi::Record` change → no binding regen, no coordinated staticlib rebuild (record-change / checksum-coupling discipline not triggered).
- **No new Swift file ✅ → no pbxproj edit** (Task 4 gate). Tests appended to existing `ToolsSectionTests.swift` + `DirectoryRepositoryTests.swift`; UI edits in existing `DirectoryView.swift`/`AppModel.swift`/`ProfileRepository.swift`. Confirmed the projects are NOT synchronized groups, so this avoidance is load-bearing.
- **SECURITY (cite the security-review requirement — install routes through the trust gate, no auto-trust):** installing a tool from an arbitrary file is untrusted-by-default. `RiotProfileRepository.installApp` deliberately does NOT call `trustApp`; the app lands UNTRUSTED (`spaceApp` reads `appRuntime.isAppTrusted`), so `AppReviewSheet` — the host-side organizer gate — stays between the added tool and a running WebView, identical to `getCarriedApp`'s stated contract ("Getting an app turns nothing on … the review sheet still stands between … and a WebView"). Rust remains the integrity oracle: a corrupt/unsigned/entry-point-mismatched pair throws at `appRuntime.installApp`/`retain` and is never retained or persisted (Task 1's second test + Task 2's second test assert this and that the failure surfaces a message, not a silent no-op — the exact `InvalidInput`-vanishes bug called out in `AppModel.swift:809-811`). The button is organizer-gated (`model.canApproveApps`) so a non-organizer is never offered an action they cannot complete, mirroring `AppReviewSheet`'s draw-only-if-`canApprove` rule.
- **Detail to CONFIRM on implement (flagged):** `.fileImporter` returns a **security-scoped** URL — the bytes read MUST bracket `start/stopAccessingSecurityScopedResource()` (Task 3's `readSecurityScoped`), else `Data(contentsOf:)` fails for files outside the sandbox. No existing `.fileImporter` in the app to copy, so verify this on simulator/device: pick manifest → pick bundle → row appears untrusted → Review → trust → Open. Also confirm the two-picker chaining UX (does the second sheet present cleanly after the first dismisses) reads acceptably; if not, fall back to a single `allowsMultipleSelection` importer requiring exactly two files (but the manifest-vs-bundle assignment is then ambiguous by name — the sequential picks are preferred and match Android).
- **Placeholder scan:** no `TODO`/`…`/`<placeholder>` in shipped code; every type is real and verified — `installApp`/`installTool`/`toolImportFailureMessage`/`readSecurityScoped` are the only new symbols, `retain`/`spaceApp`/`PersistedAppPack`/`.riotHeldAppsDidChange`/`canApproveApps`/`AppReviewSheet`/`.review(app)` all pre-exist at the cited lines.
- **Type consistency:** `installApp(manifest:bundle:) -> RiotSpaceApp` (repository) ↔ `installTool(manifest:bundle:)` (model, forwards) ↔ `model.installTool(manifest:bundle:)` (view) ↔ fixture `(manifest: Data, bundle: Data)` from `starterPacks()`; `RiotSpaceApp.trusted` is the field the untrusted-then-trusted assertions read.
- **Dependency order:** T1 (repository install) → T2 (model forwarder, uses T1) → T3 (view, uses T2) → T4 (build/verify). All within one unit; no pbxproj, no FFI.
