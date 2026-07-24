# Community-scoped Tools (Apple) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use metaswarm:orchestrated-execution to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the shared iOS/macOS Tools route a fast, named-community surface where enabled tools open immediately, disabled tools have an honest Add/Ask outcome, and discovery/management stays secondary.

**Architecture:** First complete the WU-002c contracts that WU-002P-A consumes: Rust directory listings must reproject current-namespace trust after a community switch, and Apple must persist trust by exact namespace plus full app ID with fail-closed legacy migration. Then add a pure namespace-keyed directory snapshot and immutable action context, keep the existing flat `rows` compatibility accessor, and render the approved paper/ink poster hierarchy in SwiftUI. The model/repository mutation seams—not view dismissal alone—reject stale community actions.

**Tech Stack:** Rust 2021/UniFFI, Swift 6, SwiftUI shared by iOS and macOS, XCTest/XCUITest, existing Riot native theme components.

**Scope:** WU-002c prerequisite contracts plus WU-002P-A (Apple). WU-002P-B Android parity and WU-002P-C Legacy/v2/quota cosmetics remain separate later plans already ordered in the master plan.

**Approved design:** `docs/superpowers/specs/2026-07-24-community-scoped-tools-design.md`

---

## User-authorized manual corrections after plan-gate escalation

The three-round plan gate did not reach consensus. On 2026-07-24 Rabble chose
**Revise and proceed**. The following corrections are authoritative wherever an
older task detail below conflicts:

1. `RiotDirectoryActionContext` also carries a monotonic
   `selectionGeneration`. `RiotDirectoryModel` advances it on every selected
   community transition, including A → B → A, and rejects a context unless both
   namespace and generation match.
2. Recommend, retraction, and Make available carry
   `expectedNamespaceID` through `DirectoryPorting` into
   `RiotProfileRepository`. The repository validates it while holding the same
   app-operation lock as the mutation; a separate presentation-only guard is
   insufficient.
3. The repository lock and closed-state guard cover every app-facing path:
   `spaceApps`, `installedApps`, `directoryListings`, `appResource`,
   `appResolver`, `appDataBridge`, `endorsedAppIDs`, admission/import,
   recommendation/retraction/publishing, trust/revoke, app-data operations, and
   community switching.
4. A finalize failure after the durable trust write is **not** an Add failure.
   The repository returns a typed `durableDecisionNeedsReopen` outcome.
   `RiotAppModel` closes and reopens from `lastBootstrapArgs`, verifies the exact
   namespace/app grant, and reports recovered success. The sheet dismisses and
   says `Added <tool> to <community>`; it never says “Nothing changed” about a
   durable grant. Restart/fault coverage pins this.
5. `RiotDirectoryPrimaryAction` defines a tested `title` property.
   `DirectoryScreenState.failed` carries only data (`communityTitle`);
   `DirectoryView` calls `directory.retry()` rather than embedding a closure in
   an equatable/sendable state.
6. No test-only API or simulator scenario adapter is added to production.
   Failures and A → B → A invalidation are tested through real protocol ports,
   repository integration, and existing runtime generation tests. XCUITest
   covers the real organizer Add → permission → Open and enabled-member Open
   flows.
7. Native macOS visual evidence uses the actual app window: set it to 480×860,
   capture it with macOS `screencapture`, and inspect the PNG directly. The
   Playwright-only visual-review workflow is not used for a native window.
8. Accessibility verification includes heading traits/labels in model and
   XCUITest assertions, `performAccessibilityAudit`, successful focus
   restoration to Open, failed confirmation focus retention, and manual reading
   order inspection. Long-name fixtures plus pseudolocalized/expanded copy are
   captured. The moderated first-attempt outcome check is recorded as a named
   human release checkpoint; it is not fabricated by an agent.

These corrections remove the final feasibility/completeness blockers while
preserving the approved product design and the scope boundary above.

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

fn listing_for(profile: &Arc<MobileProfile>, app_id: &[u8]) -> DirectoryListing {
    profile
        .app_runtime()
        .directory_listings()
        .expect("directory listings")
        .into_iter()
        .find(|listing| listing.app_id == app_id)
        .expect("full app id remains in directory")
}

let runtime = profile.app_runtime();
let tool = runtime
    .directory_listings()
    .expect("starter directory")
    .into_iter()
    .next()
    .expect("verified starter listing");
let tool_id = tool.app_id.clone();
let tool_id_hex = hex(&tool_id);
runtime
    .trust_app(tool_id_hex.clone())
    .expect("approve the real listed tool in A");

let listing_in_a = listing_for(&profile, &tool_id);
assert!(listing_trusted_in(&listing_in_a, &a_ns));

profile
    .switch_community(b_ns.clone(), REGISTRY_KEY.to_vec())
    .expect("switch B");
let listing_in_b = listing_for(&profile, &tool_id);
assert!(!listing_trusted_in(&listing_in_b, &b_ns));

profile
    .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
    .expect("switch back A");
let listing_back_in_a = listing_for(&profile, &tool_id);
assert!(profile.app_runtime().is_app_trusted(tool_id_hex).unwrap());
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
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("scoped-trust-\(UUID().uuidString)", isDirectory: true)
    let snapshotURL = directory.appendingPathComponent("profile.json")
    let databasePath = directory.appendingPathComponent("riot.db").path
    let keyStore = TestWrappingKeyStore()
    let first = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: snapshotURL),
        keyStore: keyStore,
        starterPacks: try starterPacks(),
        databasePath: databasePath
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

    // Reopen while B is active: a profile-wide implementation would wrongly
    // reissue A's app ID into B here.
    let reopened = try RiotProfileRepository.open(
        storage: try ProtectedProfileStorage(fileURL: snapshotURL),
        keyStore: keyStore,
        starterPacks: try starterPacks(),
        databasePath: databasePath
    )
    XCTAssertEqual(reopened.currentSpace?.namespaceID, b.namespaceID)
    XCTAssertFalse(try XCTUnwrap(reopened.spaceApps().first).trusted)

    _ = try reopened.switchToCommunity(namespaceID: a.namespaceID)
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
Add an internal `AppTransactionTestHooks` value with `beforePersist` and
`finalizeOverride` closures plus an internal `open(..., testHooks:)` overload.
The existing public `open(...)` delegates with `testHooks: nil`, so no internal
type appears in a public signature and production behavior is unchanged. Tests
use semaphores to hold the persistence window and to inject the
otherwise-invariant finalize failure without weakening the runtime contract.

- [ ] **Step 2: Run the AppRepository suite and observe RED**

Run:

```bash
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotTests/AppRepositoryTests
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

Add the state this prerequisite does not currently have:

```swift
private let appMutationLock = NSRecursiveLock()
private var appOperationsClosed = false

private func withAppMutationLock<T>(_ operation: () throws -> T) rethrows -> T {
    appMutationLock.lock()
    defer { appMutationLock.unlock() }
    return try operation()
}

private func requireAppOperationsOpen() throws {
    guard !appOperationsClosed else { throw RepositoryError.profileClosed }
}
```

Add the reusable namespace guard:

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
    try withAppMutationLock {
        try requireAppOperationsOpen()
        let space = try requireCurrentSpace(expectedNamespaceID: expectedNamespaceID)
        let prepared = try appRuntime.prepareAppTrust(appId: appID, trusted: true)
        let prior = persisted
        let grant = PersistedAppTrust(namespaceID: space.namespaceID, appIDHex: prepared.appId)
        persisted.trustedAppsByNamespace.removeAll { $0 == grant }
        persisted.trustedAppsByNamespace.append(grant)
        do {
            try storage.save(persisted)
        } catch {
            persisted = prior
            try? appRuntime.discardPreparedTrust()
            throw error
        }
        do {
            try appRuntime.finalizeAppTrust()
        } catch {
            // Durable state is authoritative; stop every later app operation
            // until AppModel rebuilds the profile on the next bootstrap.
            appOperationsClosed = true
            throw RepositoryError.profileClosed
        }
    }
}
```

Implement revoke with the same expected-namespace guard and ordering. Route
`switchToCommunity`, `installedApps`, `directoryListings`, `getCarriedApp`,
`installApp`, `shareApp`, `endorseApp`, grant/revoke, and every app-data
read/write/bridge entry point through `withAppMutationLock`; read paths call
`requireAppOperationsOpen`, and `appDataBridge` returns `nil` while closed.
This makes the lock actually span prepare → disk → finalize, so a switch,
revoke, admission, or bridge operation cannot interleave. Add fault-injection
tests that block storage save while another queue attempts a switch/bridge and
assert the second operation waits, plus a finalize-failure seam that asserts all
later app reads/mutations fail with `profileClosed`/nil. On open, restore
only records whose namespace equals `persisted.space?.namespaceID`, after
starter/carried pairs have been verified, and let Rust revalidate organizer
authority. An unexpected finalize invariant failure now creates a real
repository closed state; it never reports a rollback of a durable decision.

Add compatibility overloads in this same task so existing repository callers
remain source-compatible while directory mutations use the explicit guard:

```swift
public func trustApp(appID: String) throws {
    guard let namespaceID = currentSpace?.namespaceID else {
        throw RepositoryError.noCurrentSpace
    }
    try trustApp(appID: appID, expectedNamespaceID: namespaceID)
}

public func untrustApp(appID: String) throws {
    guard let namespaceID = currentSpace?.namespaceID else {
        throw RepositoryError.noCurrentSpace
    }
    try untrustApp(appID: appID, expectedNamespaceID: namespaceID)
}
```

The explicit overload is mandatory for captured UI actions. The compatibility
overload only captures at the repository call boundary and preserves existing
non-directory tests/callers until their own work units adopt operation contexts.

- [ ] **Step 5: Run repository and real community-switch tests**

Run:

```bash
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotTests/AppRepositoryTests
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
- complete profile-wide listings appear once in Available to add, never
  duplicated under More tools;
- organizer `moreTools` contains only the secondary verified-file import
  action, while member/legacy snapshots expose no import mutation;
- no selected community yields no snapshot and no mutation context;
- loading a new namespace clears the prior namespace snapshot;
- a same-namespace failure retains its last-good rows with scoped error/retry;
- a cross-namespace failure clears old rows and emits exact named error/retry;
- full app ID appears in accessibility identifiers; and
- no `ToolStrings.userFacingVocabulary` value contains `this community`.

- [ ] **Step 2: Run the focused suite and observe RED**

Run:

```bash
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS' -derivedDataPath build/xcode-dd -quiet \
  CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotKitTests-macOS/DirectoryStorefrontTests
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

public enum RiotDirectoryDiscoveryAction: Equatable, Sendable {
    case importVerifiedPair(title: String)
}

public struct RiotDirectorySnapshot: Equatable, Sendable {
    public let namespaceID: String
    public let communityTitle: String
    public let inCommunity: [RiotDirectoryRow]
    public let availableToAdd: [RiotDirectoryRow]
    public let moreTools: [RiotDirectoryDiscoveryAction]
}
```

Add the row’s `primaryAction`, `actionContext`, local-pair/provenance facts, and full-ID accessibility identifier without removing the compatibility `availability`.

The current directory API is already profile-wide, but every production
`DirectoryListing` is a verified complete pair and `getCarriedApp` makes it
actionable in the selected community. Per the approved IA, those rows truthfully
belong in `Available to add`; they are not duplicated in `More tools`.
`More tools` models the independent profile-wide discovery affordance that
exists today—organizer import of a verified pair. Do not invent an “other
communities” source or surface unverified `pending_manifests`. Tests pin this
boundary and the absence of duplicated rows.

- [ ] **Step 4: Build rows in linear time and publish atomically**

In `refresh(approval:)`, build an installed dictionary keyed by normalized full app ID, map listings once, append unlisted held rows, and assign one namespace-keyed snapshot:

```swift
let installedByID = installed.reduce(into: [String: RiotSpaceApp]()) { result, app in
    // Starter restoration and a persisted carried copy can legitimately
    // resolve to the same content-derived ID. They are the same verified tool;
    // last restoration wins without trapping.
    result[app.appIDHex.lowercased()] = app
}
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
    availableToAdd: allRows.filter { !$0.enabledInCurrentCommunity },
    moreTools: approval == .organizer
        ? [.importVerifiedPair(title: "Add a tool from a file")]
        : []
)
rows = allRows
snapshot = next
```

Sort each section deterministically by localized case-insensitive name and then full app ID. Production listings are verified complete pairs; do not surface `pending_manifests` or unverified names.

Keep a source-compatible overload for `PeerProfileView`:

```swift
public func refresh() {
    refresh(approval: .member)
}
```

That overload refreshes the unchanged flat `rows` semantics. `DirectoryView`
always calls `refresh(approval:)` with the real organizer/member/legacy state.

- [ ] **Step 5: Add namespace-keyed loading/error behavior**

Track `isLoading`, `snapshot`, `failedNamespace`, and `errorMessage`.
Same-namespace refresh failures retain the prior snapshot; a
different-namespace failure clears it and returns exact `Couldn’t load tools for
River City Wire.`. Add a retry operation that repeats the captured namespace
load only while it is still selected. No selection clears the snapshot and
offers no operation context.

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
For fake `getCarriedApp` failures, assert the exact named messages:

```swift
XCTAssertEqual(
    model.errorMessage,
    "Couldn’t open Chat in River City Wire. Nothing changed. Try again."
)
XCTAssertEqual(
    model.errorMessage,
    "Couldn’t add Chat to River City Wire. Nothing changed. Try again."
)
XCTAssertEqual(row.primaryAction.title, originalTitle)
```

At the real repository boundary prove:

- enabled but unadmitted Open calls verified `getCarriedApp`, remains namespace-trusted, then returns the admitted `RiotSpaceApp`;
- disabled/unadmitted Add admits but does not trust before confirmation;
- member import/admission still cannot obtain an execution bridge;
- Make available publishes verified bytes but does not enable; and
- a switched namespace rejects a stale expected namespace;
- revoking between `prepareOpen` and the shell’s actual open causes the fresh
  `appDataBridge`/`AppExecutionSession` request to fail; and
- switching namespaces or advancing the execution generation between preparation
  and open likewise refuses the fresh session.

- [ ] **Step 2: Run both suites and observe RED**

Run:

```bash
sh scripts/ios-check.sh test
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotTests/DirectoryStorefrontTests \
  -only-testing:RiotTests/DirectoryRepositoryTests
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

Catch preparation failures inside the model operation, keep the snapshot and
row unchanged, clear any stale receipt, and set the exact operation-specific
named error above. A retry invokes the whole verified admission path again;
there is no intermediate “got” state and no trust mutation.

Do not cache or return an execution session from the directory model.
`DirectoryView` hands the admitted app to the existing shell `onOpen`, and the
shell asks `RiotProfileRepository.appDataBridge`/the Rust runtime for a fresh
generation-and-namespace-bound execution session. The revoke/switch tests above
pin that separation.

Make `recommend`, `retract`, and `makeAvailable` accept the captured context, call the guard immediately before the port mutation, and publish exact receipts:

```swift
"Recommended \(context.appName) to \(context.communityTitle)"
"Made \(context.appName) available in \(context.communityTitle)"
```

- [ ] **Step 4: Run the suites and commit**

Run the macOS shared-model test plus the iOS `RiotKit` command from Step 2;
expected PASS on both.

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
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotTests/ToolsSectionTests
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

Run the iOS `RiotKit` `ToolsSectionTests` command from Step 2, then
`sh scripts/ios-check.sh test`; expected PASS.

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
Also assert verified permissions precede the confirmation model and that member
and legacy states contain no approval action.

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

Use the repository’s established shared SwiftUI focus mechanism:

```swift
@AccessibilityFocusState private var approvalFocused: Bool
```

On failure, attach the complete failure as the re-enabled button’s accessibility
hint/label, set `approvalFocused = true`, and apply
`.accessibilityFocused($approvalFocused)`. This causes VoiceOver to encounter
the named failure while returning focus to the actionable control without
depending on a nonexistent SwiftUI live-region API. List verified manifest
permissions before the button, omit the approval control for member/legacy
states, and keep provenance only in details when useful.

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
XCTAssertEqual(ToolStrings.loadFailure("River City Wire"),
               "Couldn’t load tools for River City Wire.")
XCTAssertEqual(ToolStrings.chooseCommunity,
               "Choose a community to see its tools")
```

Assert the vocabulary contains no `From your communities`, `Built in`, `Review Chat`, `Share with this community`, or generic `this community`.
Add pure `DirectoryScreenState` decisions for `.chooseCommunity`, `.loading`,
`.failed(title:retry:)`, and `.content(snapshot:error:)`; tests assert the failed
state never simultaneously selects the inline no-tools state.

- [ ] **Step 2: Run focused tests and observe RED**

Run the focused `DirectoryStorefrontTests` command; expected FAIL on old strings.

- [ ] **Step 3: Replace the flat catalog with scoped sections**

Render:

```swift
switch screenState {
case .chooseCommunity:
    Text("Choose a community to see its tools")
case .loading(let title):
    ProgressView("Loading tools for \(title)…")
case .failed(let title):
    VStack(alignment: .leading) {
        Text("Couldn’t load tools for \(title).")
        Button("Try again", action: retry)
            .buttonStyle(.riotPrimary)
    }
case .content(let snapshot, let retainedError):
    scopedContent(snapshot, retainedError: retainedError)
}
```

The content branch uses:

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

When a same-namespace refresh fails, `retainedError` renders above the
last-good sections with `Try again`; do not render a misleading empty claim.
Keep `More tools` secondary and render only `snapshot.moreTools`, which today is
the organizer-only `Add a tool from a file` discovery action. Omit the whole
section for member/legacy snapshots. After verified import, open the named
permission sheet; do not claim import enabled the tool.

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

- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryView.swift`
- Modify: `apps/ios/RiotUITests/ChecklistFlowUITests.swift`
- Modify: `apps/ios/RiotUITests/RiversideMemberToolUITests.swift`
- Modify: `apps/ios/RiotUITests/RiotTabNavigationUITests.swift` only if its reusable Tools capture is used

- [ ] **Step 1: Add a deterministic runtime-gated Tools UI scenario seam**

Follow the existing `RIOT_UI_TEST_RUN_ID` convention. Only when that run ID is
present may `RIOT_UI_TEST_TOOLS_SCENARIO` select a deterministic scenario:

- `approval-fails-once` — the first result-bearing approval returns failure
  without calling trust; retry calls the real operation;
- `disabled-member` — renders the verified disabled fixture with `.member`
  presentation capability and no mutation control;
- `switch-during-review` — supplies two named fixture snapshots and changes the
  selected namespace after the A confirmation opens; and
- `lazy-open-fails-once` / `add-preparation-fails-once` — the verified admission
  port fails once before returning the real admitted app on retry; and
- `load-failure` — fails the first scoped refresh, then succeeds on Try again.

Keep the scenario adapter internal to `RiotAppModel`/`DirectoryView` and make it
delegate to the ordinary model APIs after the single injected event. The Xcode
project does not define a `DEBUG` compilation condition, so do not use
`#if DEBUG`. Compile the adapter only under
`#if targetEnvironment(simulator)`; inside that branch, the hard runtime gate
is the conjunction of a unique `RIOT_UI_TEST_RUN_ID` and a recognized scenario
value. Device and macOS production binaries contain no adapter, and ordinary
simulator launches without both environment keys use no scenario. Unit tests
assert the environment gate cannot activate from
`RIOT_UI_TEST_TOOLS_SCENARIO` alone. No Xcode project edit is required.

Implement this as an internal `DirectoryPorting` decorator selected by
`DirectoryView.sync`: it delegates every ordinary call to
`model.profileRepository`, changes only the one requested response/callback,
and records mutations for the scenario assertion. The member scenario changes
only the presentation capability passed to `refresh(approval:)`; it does not
grant organizer authority. The switch scenario changes the decorator’s
`currentSpace` and listings as one atomic fixture snapshot, exercising the same
`onChange` cancellation path as a real switch.

- [ ] **Step 2: Update XCUITest identifiers and assertions**

Resolve Checklist’s full app ID from the rendered action query/prefix rather than its display name. Assert:

- `River City Wire`/created community title is visible;
- `In <community>` precedes `Available to add`;
- enabled Checklist has immediate `Open Checklist`;
- disabled organizer Checklist has `Add Checklist to <community>`, then the named permission sheet, then Open;
- member demo has Open and no Add/Review;
- disabled-member scenario has static `Ask an organizer to add Checklist`,
  exposes no Add/approve control, and does not claim a request was sent;
- the permission sheet exposes the fixture’s permissions before its Add control;
- approval-fails-once keeps the sheet open, shows/announces the exact named
  error, re-enables Add, and the second tap succeeds;
- lazy-open-fails-once and add-preparation-fails-once keep their original
  Open/Add CTA visible, show the exact named error, and succeed on retry;
- switch-during-review dismisses A’s sheet and leaves B unchanged;
- load-failure shows no false empty state, exposes `Try again`, and then renders
  the named sections; and
- no `Built in`, `From your communities`, or `Share with this community` is visible.

- [ ] **Step 3: Run the iOS UI tests at normal size**

Run:

```bash
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotUITests/ChecklistFlowUITests \
  -only-testing:RiotUITests/RiversideMemberToolUITests
```

Expected: PASS with kept Tools screenshots. Run the new scenario methods in
`ChecklistFlowUITests`/`RiversideMemberToolUITests` explicitly if the class-wide
selection is not used.

- [ ] **Step 4: Run at accessibility-extra-extra-extra-large**

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

- [ ] **Step 5: Capture and review macOS at 480-point width**

Run the Riot macOS app with its default 480×860 window, create/select River City Wire, open Tools, and capture with the visual-review workflow. Inspect:

- community eyebrow + poster `Tools`;
- `In River City Wire` first and immediate Open;
- `Available to add` next;
- More tools visually secondary;
- paper/ink, hard borders, Anton/Work Sans/Space Mono, and blue/pink offset header;
- long tool/community names and large Dynamic Type reflow;
- no clipped copy, horizontal overflow, or sub-44-point action.

Save review artifacts under `artifacts/visual-review/community-scoped-tools/` and record the exact configuration in `review-notes.md`.

- [ ] **Step 6: Commit UI coverage and evidence notes**

```bash
git add apps/ios/RiotUITests/ChecklistFlowUITests.swift \
  apps/ios/RiotUITests/RiversideMemberToolUITests.swift \
  apps/ios/RiotUITests/RiotTabNavigationUITests.swift \
  apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/Directory/DirectoryView.swift \
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
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination "platform=iOS Simulator,id=$SIM_ID" \
  -derivedDataPath build/xcode-dd -quiet CODE_SIGNING_ALLOWED=NO \
  -only-testing:RiotUITests/ChecklistFlowUITests \
  -only-testing:RiotUITests/RiversideMemberToolUITests
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
