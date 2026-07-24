# Community-scoped Tools (Apple) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use metaswarm:orchestrated-execution to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the shared iOS/macOS Tools route a fast, named-community surface where enabled tools open immediately, disabled tools have an honest Add/Ask outcome, and discovery/management stays secondary.

**Architecture:** First complete the WU-002c contracts that WU-002P-A consumes: Rust directory listings must reproject current-namespace trust after a community switch, and Apple must persist trust by exact namespace plus full app ID with fail-closed legacy migration. Then add a pure namespace-keyed directory snapshot and immutable action context, keep the existing flat `rows` compatibility accessor, and render the approved paper/ink poster hierarchy in SwiftUI. The model/repository mutation seams—not view dismissal alone—reject stale community actions.

**Tech Stack:** Rust 2021/UniFFI, Swift 6, SwiftUI shared by iOS and macOS, XCTest/XCUITest, existing Riot native theme components.

**Scope:** WU-002c prerequisite contracts plus WU-002P-A (Apple). WU-002P-B Android parity and WU-002P-C Legacy/v2/quota cosmetics remain separate later plans already ordered in the master plan.

**Approved design:** `docs/superpowers/specs/2026-07-24-community-scoped-tools-design.md`

---

## File map

- `crates/riot-ffi/src/mobile_state.rs` — refresh current-namespace trust projection after a successful community switch.
- `crates/riot-ffi/tests/persistence_contract.rs` — real A → B → A listing/trust integration coverage.
- `apps/ios/Riot/Core/ProfileRepository.swift` — namespace-scoped durable trust, fail-closed legacy migration, persistence-first prepare/finalize, and namespace mutation guard.
- `apps/ios/RiotTests/AppRepositoryTests.swift` — restart, migration, and durable trust transaction coverage.
- `apps/ios/Riot/Directory/DirectoryModel.swift` — flat compatibility rows, scoped snapshot/sections, CTA intent, operation context, lazy admission, and named management actions.
- `apps/ios/RiotTests/DirectoryStorefrontTests.swift` — pure grouping, CTA, copy, stale-context, loading/error, and action tests.
- `apps/ios/RiotTests/DirectoryRepositoryTests.swift` — real repository integration for admission, Add, publishing, recommendation, and namespace behavior.
- `apps/ios/Riot/AppModel.swift` — result-bearing, expected-namespace Add/import seams.
- `apps/ios/RiotTests/ToolsSectionTests.swift` — AppModel success/failure and import-without-trust tests in an already compiled target.
- `apps/ios/Riot/Apps/AppReviewSheet.swift` — named permission confirmation, pending/failure state, and success-only dismissal contract.
- `apps/ios/Riot/Directory/DirectoryView.swift` — marketing/native header, scoped sections, dominant Open/Add/Ask actions, compact states, and secondary discovery/management.
- `apps/ios/RiotUITests/ChecklistFlowUITests.swift` — organizer Add → confirmation → Open and screenshot.
- `apps/ios/RiotUITests/RiversideMemberToolUITests.swift` — member immediate Open and no dead Review gate.
- `apps/ios/RiotUITests/RiotTabNavigationUITests.swift` — named header/section screenshot capture when reusable.

No new Swift file is created, so neither Xcode project file changes.

### Task 1: Make directory trust truthful after A → B → A switches (WU-002c core)

**Files:**

- Modify: `crates/riot-ffi/tests/persistence_contract.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs`

- [ ] **Step 1: Write the failing real-boundary test**

Extend `communities_are_isolated_entries_approvals_and_coordinator_do_not_leak` (or add the adjacent focused test) so it resolves the tool’s listing after each switch and compares it with `is_app_trusted`:

```rust
fn listing_trusted_in(listing: &DirectoryListing, namespace: &str) -> bool {
    listing
        .trusted_in_spaces
        .iter()
        .any(|id| hex(id) == namespace.to_ascii_lowercase())
}

fn listing_for(profile: &Arc<MobileProfile>, app_id_hex: &str) -> DirectoryListing {
    profile
        .app_runtime()
        .directory_listings()
        .expect("directory listings")
        .into_iter()
        .find(|listing| hex(&listing.app_id) == app_id_hex)
        .expect("full app id remains in directory")
}

let listing_in_a = listing_for(&profile, &a_tool_id());
assert!(listing_trusted_in(&listing_in_a, &a_ns));

profile
    .switch_community(b_ns.clone(), REGISTRY_KEY.to_vec())
    .expect("switch B");
let listing_in_b = listing_for(&profile, &a_tool_id());
assert!(!listing_trusted_in(&listing_in_b, &b_ns));

profile
    .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
    .expect("switch back A");
let listing_back_in_a = listing_for(&profile, &a_tool_id());
assert!(profile.app_runtime().is_app_trusted(a_tool_id()).unwrap());
assert!(listing_trusted_in(&listing_back_in_a, &a_ns));
```

Import `DirectoryListing`, add the local lowercase `hex(&[u8]) -> String`
helper used by other FFI integration tests, and do not compare display names.

- [ ] **Step 2: Run the test and observe RED**

Run:

```bash
cargo test -p riot-ffi --test persistence_contract communities_are_isolated_entries_approvals_and_coordinator_do_not_leak -- --exact --nocapture
```

Expected: FAIL on the listing assertion after returning to A while `is_app_trusted` is true.

- [ ] **Step 3: Rebuild the active trust projection during a successful switch**

In `switch_community`, after the target namespace is active and its content has been reprojected, refresh the cache from the store before returning:

```rust
if let Some(cached) = profile.community_entries.remove(&target_ns) {
    profile.entries = cached;
} else {
    reproject_active(profile)?;
}
refresh_app_trust_markers(profile)?;
```

Keep the existing early active-community no-op, generation bump, organizer check, and session invalidation unchanged.

- [ ] **Step 4: Run focused and nearby trust suites**

Run:

```bash
cargo test -p riot-ffi --test persistence_contract communities_are_isolated_entries_approvals_and_coordinator_do_not_leak -- --exact --nocapture
cargo test -p riot-ffi --test organizer_trust
cargo test -p riot-ffi --test apps_contract directory -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/persistence_contract.rs
git commit -m "fix: reproject tool trust after community switches"
```

### Task 2: Persist Apple trust by namespace and fail closed on legacy IDs (WU-002c Apple)

**Files:**

- Modify: `apps/ios/RiotTests/AppRepositoryTests.swift`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`

- [ ] **Step 1: Add RED persistence and migration tests**

Add snapshot helpers that read a new `trustedAppsByNamespace` array of objects with `namespaceID` and `appIDHex`. Add tests proving:

```swift
func testTrustPersistsOnlyForItsExactNamespaceAcrossReopen() throws {
    let snapshotURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("scoped-trust-\(UUID().uuidString).json")
    let keyStore = TestWrappingKeyStore()
    let first = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: snapshotURL),
        keyStore: keyStore,
        starterPacks: try starterPacks()
    )
    let a = try first.createPublicSpace(title: "Community A")
    let appID = try XCTUnwrap(first.spaceApps().first).appIDHex
    try first.trustApp(appID: appID, expectedNamespaceID: a.namespaceID)

    let other = try RiotProfileRepository.open(
        storage: try makeStorage("community-b"),
        keyStore: TestWrappingKeyStore(),
        starterPacks: try starterPacks()
    )
    let b = try other.createPublicSpace(title: "Community B")
    _ = try first.adoptSyncedNamespace(b)
    XCTAssertFalse(try XCTUnwrap(first.spaceApps().first).trusted)

    _ = try first.switchToCommunity(namespaceID: a.namespaceID)
    XCTAssertTrue(try XCTUnwrap(first.spaceApps().first).trusted)

    let reopened = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: snapshotURL),
        keyStore: keyStore,
        starterPacks: try starterPacks()
    )
    XCTAssertEqual(reopened.currentSpace?.namespaceID, a.namespaceID)
    XCTAssertTrue(try XCTUnwrap(reopened.spaceApps().first).trusted)
}

func testLegacyGlobalTrustedAppIDsAreDiscardedInsteadOfAssignedToActiveSpace() throws {
    let snapshotURL = FileManager.default.temporaryDirectory
        .appendingPathComponent("legacy-global-trust-\(UUID().uuidString).json")
    let keyStore = TestWrappingKeyStore()
    let first = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: snapshotURL),
        keyStore: keyStore,
        starterPacks: try starterPacks()
    )
    let active = try first.createPublicSpace(title: "Community B")
    let appID = try XCTUnwrap(first.spaceApps().first).appIDHex
    var object = try XCTUnwrap(
        JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
    )
    object.removeValue(forKey: "trustedAppsByNamespace")
    object["trustedAppIDs"] = [appID]
    try JSONSerialization.data(withJSONObject: object)
        .write(to: snapshotURL, options: .atomic)

    let reopened = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: snapshotURL),
        keyStore: keyStore,
        starterPacks: try starterPacks()
    )
    XCTAssertEqual(reopened.currentSpace?.namespaceID, active.namespaceID)
    XCTAssertFalse(try reopened.spaceApps()[0].trusted)
    XCTAssertEqual(try scopedTrust(in: snapshotURL), [])
}
```

Also update the existing trust/untrust disk assertions to compare exact `(namespaceID, appIDHex)` records.

- [ ] **Step 2: Run the AppRepository suite and observe RED**

Run:

```bash
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS' -derivedDataPath build/xcode-dd -quiet \
  CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotKitTests/AppRepositoryTests
```

Expected: FAIL because the snapshot has only profile-wide `trustedAppIDs`.

- [ ] **Step 3: Replace global trust storage with an exact durable key**

Add:

```swift
private struct PersistedAppTrust: Codable, Equatable {
    let namespaceID: String
    let appIDHex: String

    init(namespaceID: String, appIDHex: String) {
        self.namespaceID = namespaceID.lowercased()
        self.appIDHex = appIDHex.lowercased()
    }
}
```

Replace `PersistedProfile.trustedAppIDs` with `trustedAppsByNamespace`.
In the custom decoder, decode the new field and deliberately ignore the old
`trustedAppIDs` field. Encoding writes only the new field. A missing new field
decodes to `[]`; ambiguous legacy IDs never become grants.

- [ ] **Step 4: Make grant/revoke persistence-first and namespace-bound**

Add a reusable guard:

```swift
private func requireCurrentSpace(expectedNamespaceID: String) throws -> RiotSpace {
    guard let space = persisted.space else { throw RepositoryError.noCurrentSpace }
    guard space.namespaceID.caseInsensitiveCompare(expectedNamespaceID) == .orderedSame else {
        throw RepositoryError.spaceMismatch
    }
    return space
}
```

Change the repository mutation signature and implementation:

```swift
public func trustApp(appID: String, expectedNamespaceID: String) throws {
    let space = try requireCurrentSpace(expectedNamespaceID: expectedNamespaceID)
    let prepared = try appRuntime.prepareAppTrust(appId: appID, trusted: true)
    let prior = persisted
    let grant = PersistedAppTrust(namespaceID: space.namespaceID, appIDHex: prepared.appId)
    persisted.trustedAppsByNamespace.removeAll { $0 == grant }
    persisted.trustedAppsByNamespace.append(grant)
    do {
        try storage.save(persisted)
        try appRuntime.finalizeAppTrust()
    } catch {
        persisted = prior
        try? storage.save(prior)
        try? appRuntime.discardPreparedTrust()
        throw error
    }
}
```

Implement revoke with the same expected-namespace guard and prepare/persist/finalize ordering. On open, restore only records whose namespace equals `persisted.space?.namespaceID`, after starter/carried pairs have been verified, and let Rust revalidate organizer authority.

- [ ] **Step 5: Run repository and real community-switch tests**

Run:

```bash
sh scripts/ios-check.sh test
cargo test -p riot-ffi --test persistence_contract communities_are_isolated_entries_approvals_and_coordinator_do_not_leak -- --exact
```

Expected: PASS; legacy global IDs remain off and scoped grants survive only in their namespace.

- [ ] **Step 6: Commit**

```bash
git add apps/ios/Riot/Core/ProfileRepository.swift apps/ios/RiotTests/AppRepositoryTests.swift
git commit -m "fix: scope Apple tool trust to its community"
```

### Task 3: Build the pure community-scoped presentation snapshot

**Files:**

- Modify: `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryModel.swift`

- [ ] **Step 1: Write RED tests for grouping, ordering, and CTA copy**

Add table-driven tests for:

```swift
XCTAssertEqual(snapshot.communityTitle, "River City Wire")
XCTAssertEqual(snapshot.inCommunity.map(\.name), ["Chat", "Checklist"])
XCTAssertEqual(snapshot.availableToAdd.map(\.name), ["Supply Board"])
XCTAssertEqual(snapshot.inCommunity[0].primaryAction.title, "Open Chat")
XCTAssertEqual(snapshot.availableToAdd[0].primaryAction.title,
               "Add Supply Board to River City Wire")
```

Cover:

- enabled first regardless of input order, built-in flag, endorsement, or version;
- enabled installed and enabled complete/not-admitted both select Open;
- organizer disabled selects exact named Add;
- ordinary member selects static `Ask an organizer to add Chat`;
- legacy profile selects `This profile can’t add tools to River City Wire. Start a new profile to organize a community.`;
- no badge is `Built in` or `On in this space`;
- `Works offline` only for a locally resolvable verified pair;
- `rows` retains its flat local `Availability` semantics for `PeerProfileView`;
- full app ID appears in accessibility identifiers; and
- no `ToolStrings.userFacingVocabulary` value contains `this community`.

- [ ] **Step 2: Run the focused suite and observe RED**

Run:

```bash
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS' -derivedDataPath build/xcode-dd -quiet \
  CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotKitTests/DirectoryStorefrontTests
```

Expected: FAIL because no scoped snapshot/CTA model exists.

- [ ] **Step 3: Add the snapshot and action types**

Define these pure values in `DirectoryModel.swift`:

```swift
public enum RiotDirectoryApprovalCapability: Equatable, Sendable {
    case organizer
    case member
    case unavailable
}

public struct RiotDirectoryActionContext: Equatable, Sendable {
    public let appID: Data
    public let appIDHex: String
    public let appName: String
    public let namespaceID: String
    public let communityTitle: String
}

public enum RiotDirectoryPrimaryAction: Equatable, Sendable {
    case open(title: String)
    case add(title: String)
    case ask(title: String)
    case unavailable(message: String)
}

public struct RiotDirectorySnapshot: Equatable, Sendable {
    public let namespaceID: String
    public let communityTitle: String
    public let inCommunity: [RiotDirectoryRow]
    public let availableToAdd: [RiotDirectoryRow]
}
```

Add the row’s `primaryAction`, `actionContext`, local-pair/provenance facts, and full-ID accessibility identifier without removing the compatibility `availability`.

- [ ] **Step 4: Build rows in linear time and publish atomically**

In `refresh(approval:)`, build an installed dictionary keyed by normalized full app ID, map listings once, append unlisted held rows, and assign one namespace-keyed snapshot:

```swift
let installedByID = Dictionary(
    uniqueKeysWithValues: installed.map { ($0.appIDHex.lowercased(), $0) }
)
let listed = try port.directoryListings().map { listing in
    let appIDHex = RiotDirectoryRow.hex(listing.appId)
    return RiotDirectoryRow.make(
        listing: listing,
        installed: installedByID[appIDHex],
        space: space,
        approval: approval,
        endorsedByMe: endorsed.contains(appIDHex)
    )
}
let listedIDs = Set(listed.map(\.appIDHex))
let held = installed
    .filter { !listedIDs.contains($0.appIDHex.lowercased()) }
    .map {
        RiotDirectoryRow.held(
            $0,
            space: space,
            approval: approval,
            endorsedByMe: endorsed.contains($0.appIDHex.lowercased())
        )
    }
let allRows = listed + held
let next = RiotDirectorySnapshot(
    namespaceID: space.namespaceID,
    communityTitle: space.title,
    inCommunity: allRows.filter(\.enabledInCurrentCommunity),
    availableToAdd: allRows.filter { !$0.enabledInCurrentCommunity }
)
rows = allRows
snapshot = next
```

Sort each section deterministically by localized case-insensitive name and then full app ID. Production listings are verified complete pairs; do not surface `pending_manifests` or unverified names.

- [ ] **Step 5: Add namespace-keyed loading/error behavior**

Track `isLoading`, `snapshot`, and `errorMessage`. Same-namespace refresh failures retain the prior snapshot; a different-namespace failure clears it and returns exact `Couldn’t load tools for River City Wire.`. Add `retry`.

- [ ] **Step 6: Run the focused suite and commit**

Run the same `DirectoryStorefrontTests` command; expected PASS.

```bash
git add apps/ios/Riot/Directory/DirectoryModel.swift apps/ios/RiotTests/DirectoryStorefrontTests.swift
git commit -m "feat: model tools by selected community"
```

### Task 4: Add namespace-bound Open/Add/management operations

**Files:**

- Modify: `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- Modify: `apps/ios/RiotTests/DirectoryRepositoryTests.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryModel.swift`

- [ ] **Step 1: Write RED operation tests**

Test an immutable context captured in A against a mutable fake port switched to B:

```swift
let context = try XCTUnwrap(row.actionContext)
port.currentSpace = RiotSpace(namespaceID: bID, title: "Community B")

XCTAssertThrowsError(try model.prepareAdd(row, context: context))
XCTAssertEqual(port.endorseCalls, [])
XCTAssertEqual(port.shareCalls, [])
```

Cover stale Add, Make available, Recommend, and retraction; exact receipts; `canMakeAvailable` only with a locally resolvable pair; and failed actions leaving row/CTA unchanged.

At the real repository boundary prove:

- enabled but unadmitted Open calls verified `getCarriedApp`, remains namespace-trusted, then returns the admitted `RiotSpaceApp`;
- disabled/unadmitted Add admits but does not trust before confirmation;
- member import/admission still cannot obtain an execution bridge;
- Make available publishes verified bytes but does not enable; and
- a switched namespace rejects a stale expected namespace.

- [ ] **Step 2: Run both suites and observe RED**

Run:

```bash
sh scripts/ios-check.sh test
```

Expected: FAIL on missing operation-context APIs and old generic receipts.

- [ ] **Step 3: Implement one guarded operation seam**

Add:

```swift
private func requireCurrentContext(_ context: RiotDirectoryActionContext) throws {
    guard let current = port?.currentSpace,
          current.namespaceID.caseInsensitiveCompare(context.namespaceID) == .orderedSame
    else { throw RepositoryError.spaceMismatch }
}
```

`prepareOpen` and `prepareAdd` return a `RiotSpaceApp`: reuse the installed app or call `getCarriedApp`, then re-check context. `prepareOpen` also requires the refreshed row to remain enabled; the shell obtains its fresh execution session only after `onOpen`.

Make `recommend`, `retract`, and `makeAvailable` accept the captured context, call the guard immediately before the port mutation, and publish exact receipts:

```swift
"Recommended \(context.appName) to \(context.communityTitle)"
"Made \(context.appName) available in \(context.communityTitle)"
```

- [ ] **Step 4: Run the suites and commit**

Run `sh scripts/ios-check.sh test`; expected PASS.

```bash
git add apps/ios/Riot/Directory/DirectoryModel.swift \
  apps/ios/RiotTests/DirectoryStorefrontTests.swift \
  apps/ios/RiotTests/DirectoryRepositoryTests.swift
git commit -m "feat: bind tool actions to their community"
```

### Task 5: Make AppModel approval/import result-bearing

**Files:**

- Modify: `apps/ios/RiotTests/ToolsSectionTests.swift`
- Modify: `apps/ios/Riot/AppModel.swift`

- [ ] **Step 1: Write RED result tests**

Update existing calls to assert the return value and add:

```swift
XCTAssertTrue(
    model.trustApp(appID: app.appIDHex, expectedNamespaceID: space.namespaceID)
)
XCTAssertFalse(
    model.trustApp(appID: app.appIDHex, expectedNamespaceID: otherNamespace)
)
XCTAssertFalse(try XCTUnwrap(model.apps.first).trusted)
```

Add an import test that returns the admitted app, verifies it is untrusted, and proves the trust sheet remains the next step.

- [ ] **Step 2: Run and observe RED**

Run:

```bash
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS' -derivedDataPath build/xcode-dd -quiet \
  CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotKitTests/ToolsSectionTests
```

Expected: compile/test FAIL because the methods return `Void` and accept no expected namespace.

- [ ] **Step 3: Implement result-bearing methods**

Use:

```swift
@discardableResult
public func trustApp(appID: String, expectedNamespaceID: String) -> Bool {
    guard let repository else { return false }
    do {
        try repository.trustApp(
            appID: appID,
            expectedNamespaceID: expectedNamespaceID
        )
        errorMessage = nil
        refreshApps()
        refreshOrganizerState()
        return true
    } catch {
        errorMessage = Self.approvalFailureMessage(error)
        refreshOrganizerState()
        return false
    }
}
```

Make `installTool(manifest:bundle:) -> RiotSpaceApp?` return the admitted verified app and never trust it. Preserve a compatibility trust overload only where existing non-directory callers need it, and have that overload capture the current namespace before delegating.

- [ ] **Step 4: Run tests and commit**

Run `sh scripts/ios-check.sh test`; expected PASS.

```bash
git add apps/ios/Riot/AppModel.swift apps/ios/RiotTests/ToolsSectionTests.swift
git commit -m "feat: report named tool approval outcomes"
```

### Task 6: Implement the named permission sheet state machine

**Files:**

- Modify: `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- Modify: `apps/ios/Riot/Apps/AppReviewSheet.swift`

- [ ] **Step 1: Add RED copy/state tests**

Extract/test pure sheet copy/state helpers:

```swift
XCTAssertEqual(copy.title, "Add Chat to River City Wire?")
XCTAssertEqual(copy.confirmation, "Add to River City Wire")
XCTAssertEqual(copy.memberReason,
               "Only an organizer of River City Wire can add this tool.")
XCTAssertEqual(copy.legacyReason,
               "This profile can’t add tools to River City Wire. Start a new profile to organize a community.")
```

Test that `.adding` disables duplicate approval, `.failed` keeps the sheet open with `Couldn’t add Chat to River City Wire. Nothing changed. Try again.`, and `.succeeded` is the only dismissal result.

- [ ] **Step 2: Run and observe RED**

Run the focused `DirectoryStorefrontTests` xcodebuild command; expected FAIL on old generic copy and synchronous callback.

- [ ] **Step 3: Implement the sheet**

Pass `RiotDirectoryActionContext`, use local `isAdding`/`failureMessage`, and make approval return `Bool`:

```swift
Button(isAdding ? "Adding…" : "Add to \(context.communityTitle)") {
    guard !isAdding else { return }
    isAdding = true
    if onApprove(context) {
        onApproved(context)
    } else {
        failureMessage =
            "Couldn’t add \(context.appName) to \(context.communityTitle). Nothing changed. Try again."
        isAdding = false
    }
}
.disabled(isAdding)
```

Announce failure with SwiftUI accessibility live-region semantics, keep/restore focus on the re-enabled confirmation, list verified manifest permissions before the button, and keep provenance only in details when useful.

- [ ] **Step 4: Run tests and commit**

Run `sh scripts/ios-check.sh test`; expected PASS.

```bash
git add apps/ios/Riot/Apps/AppReviewSheet.swift \
  apps/ios/RiotTests/DirectoryStorefrontTests.swift
git commit -m "feat: confirm adding tools to a named community"
```

### Task 7: Render the approved hierarchy and marketing/native aesthetic

**Files:**

- Modify: `apps/ios/Riot/Directory/DirectoryView.swift`
- Modify: `apps/ios/RiotTests/DirectoryStorefrontTests.swift`

- [ ] **Step 1: Pin exact visible vocabulary RED**

Update `ToolStrings` tests to require:

```swift
XCTAssertEqual(ToolStrings.sectionIn("River City Wire"), "In River City Wire")
XCTAssertEqual(ToolStrings.sectionAvailable, "Available to add")
XCTAssertEqual(ToolStrings.sectionMore, "More tools")
XCTAssertEqual(ToolStrings.emptyIn("River City Wire"),
               "No tools in River City Wire yet")
```

Assert the vocabulary contains no `From your communities`, `Built in`, `Review Chat`, `Share with this community`, or generic `this community`.

- [ ] **Step 2: Run focused tests and observe RED**

Run the focused `DirectoryStorefrontTests` command; expected FAIL on old strings.

- [ ] **Step 3: Replace the flat catalog with scoped sections**

Render:

```swift
.riotHeader(eyebrow: snapshot.communityTitle.uppercased(), "Tools")

section("In \(snapshot.communityTitle)") {
    if snapshot.inCommunity.isEmpty {
        inlineEmpty("No tools in \(snapshot.communityTitle) yet")
    } else {
        ForEach(snapshot.inCommunity) { card(for: $0) }
    }
}

section("Available to add") {
    ForEach(snapshot.availableToAdd) { card(for: $0) }
}

moreTools(snapshot)
```

Keep `More tools` secondary and show organizer-only `Add a tool from a file` there. After verified import, open the named permission sheet; do not claim import enabled the tool.

- [ ] **Step 4: Make Open/Add/Ask dominant and management secondary**

Handle `row.primaryAction` before the details disclosure:

- Open calls `prepareOpen`, then `onOpen`.
- Add calls `prepareAdd`, then presents `AppReviewSheet`.
- Ask/legacy render static status text, not a button.

Move permissions, version, provenance, Recommend, retraction, and Make available into details/overflow. Remove prominent `Built in` and `On in this space`. Rename all management controls and receipts with the captured community title.

- [ ] **Step 5: Implement card reflow and accessibility**

Use existing `RiotHeader`, `RiotCard`, `.riotPrimary`, `.riotSecondary`, Anton/Work Sans/Space Mono, paper/ink, blue/pink offset shadow, and hard two-point borders. At regular width place identity/description then primary action; use `ViewThatFits` or a vertical layout so at 480 points and accessibility sizes the action is full-width immediately below identity. Add heading traits, full-ID identifiers such as:

```swift
"directory-open-\(row.appIDHex)"
"directory-add-\(row.appIDHex)"
```

Keep human labels such as `Open Chat`. Long names wrap; actions remain at least 44 points.

- [ ] **Step 6: Cancel stale sheets/imports on community switch**

Observe the selected namespace, clear the old review context/import state, call `directory.refresh(approval:)`, and never retarget an old sheet to the new snapshot. Success dismisses, refreshes, posts `Added …`, and restores focus to the full-ID Open control.

- [ ] **Step 7: Compile, test, and commit**

Run:

```bash
sh scripts/ios-check.sh test
sh scripts/ios-check.sh sim
```

Expected: PASS.

```bash
git add apps/ios/Riot/Directory/DirectoryView.swift \
  apps/ios/RiotTests/DirectoryStorefrontTests.swift
git commit -m "feat: scope Tools to the selected community"
```

### Task 8: Update end-to-end behavior and capture visual evidence

**Files:**

- Modify: `apps/ios/RiotUITests/ChecklistFlowUITests.swift`
- Modify: `apps/ios/RiotUITests/RiversideMemberToolUITests.swift`
- Modify: `apps/ios/RiotUITests/RiotTabNavigationUITests.swift` only if its reusable Tools capture is used

- [ ] **Step 1: Update XCUITest identifiers and assertions**

Resolve Checklist’s full app ID from the rendered action query/prefix rather than its display name. Assert:

- `River City Wire`/created community title is visible;
- `In <community>` precedes `Available to add`;
- enabled Checklist has immediate `Open Checklist`;
- disabled organizer Checklist has `Add Checklist to <community>`, then the named permission sheet, then Open;
- member demo has Open and no Add/Review;
- no `Built in`, `From your communities`, or `Share with this community` is visible.

- [ ] **Step 2: Run the iOS UI tests at normal size**

Run:

```bash
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotUITests/ChecklistFlowUITests \
  -only-testing:RiotUITests/RiversideMemberToolUITests
```

Expected: PASS with kept Tools screenshots.

- [ ] **Step 3: Run at accessibility-extra-extra-extra-large**

Run:

```bash
xcrun simctl ui "$SIM_ID" content_size accessibility-extra-extra-extra-large
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotUITests/ChecklistFlowUITests
xcrun simctl ui "$SIM_ID" content_size medium
```

Expected: PASS; screenshot shows section order, wrapped named action, no clipping/overflow, and primary action before details.

- [ ] **Step 4: Capture and review macOS at 480-point width**

Run the Riot macOS app with its default 480×860 window, create/select River City Wire, open Tools, and capture with the visual-review workflow. Inspect:

- community eyebrow + poster `Tools`;
- `In River City Wire` first and immediate Open;
- `Available to add` next;
- More tools visually secondary;
- paper/ink, hard borders, Anton/Work Sans/Space Mono, and blue/pink offset header;
- long tool/community names and large Dynamic Type reflow;
- no clipped copy, horizontal overflow, or sub-44-point action.

Save review artifacts under `artifacts/visual-review/community-scoped-tools/` and record the exact configuration in `review-notes.md`.

- [ ] **Step 5: Commit UI coverage and evidence notes**

```bash
git add apps/ios/RiotUITests/ChecklistFlowUITests.swift \
  apps/ios/RiotUITests/RiversideMemberToolUITests.swift \
  apps/ios/RiotUITests/RiotTabNavigationUITests.swift \
  artifacts/visual-review/community-scoped-tools
git commit -m "test: verify community-scoped Tools UX"
```

Omit `RiotTabNavigationUITests.swift` from `git add` if unchanged.

### Task 9: Final quality, coverage, and adversarial review

**Files:**

- Verify all files changed in Tasks 1–8
- Read: `.coverage-thresholds.json`

- [ ] **Step 1: Run formatting and static checks**

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
sh scripts/ios-check.sh fast
sh scripts/ios-check.sh sim
sh scripts/ios-check.sh ios
```

Expected: PASS. Existing warnings may be recorded, but no new warning/error is accepted.

- [ ] **Step 2: Run the complete test suites**

Run:

```bash
cargo test --workspace --all-features
sh scripts/ios-check.sh test
```

Expected: PASS.

- [ ] **Step 3: Run the coverage source-of-truth gate**

Read the current floors from `.coverage-thresholds.json`, then run:

```bash
scripts/web/coverage.sh
```

Expected: PASS at or above every recorded tarpaulin, llvm, and JS-tooling floor. Do not lower a threshold.

- [ ] **Step 4: Run the final adversarial code and visual review**

Use `metaswarm:orchestrated-execution`’s VALIDATE and ADVERSARIAL REVIEW phases plus `superpowers:requesting-code-review`. Require explicit confirmation of:

- current-space grouping and exact CTA/copy;
- organizer/member/legacy behavior;
- namespace-bound stale-action refusal;
- fail-closed legacy trust migration;
- lazy admission never granting trust;
- primary Open/Add dominance;
- no prominent Built in/generic this-community copy;
- macOS and accessibility-size marketing/native aesthetic.

- [ ] **Step 5: Verify the worktree and commit any review fixes**

Run:

```bash
git diff --check
git status --short
git log --oneline --decorate -12
```

Expected: only intentional review artifacts/changes, no generated bindings or build output tracked, and no uncommitted production/test changes.
