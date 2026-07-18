# Compact Core Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Riot’s first-run, Home, reading, posting, and secondary directories compact, understandable, and free of visible no-op actions on the shared iOS/macOS SwiftUI surface.

**Architecture:** Keep the four-route community shell and core/FFI contracts. Add pure presentation/lifecycle values inside existing registered Swift files, make every visible callback explicit, and let one keyed community shell own composer, Nearby, and notifier state. Newswire and alert projections remain authoritative; Swift only filters current display state and moves detail behind explicit sheets.

**Tech Stack:** Swift 6, SwiftUI, XCTest/XCUITest, existing RiotKit FFI bindings, `xcodebuild`, repository shell gates.

---

## File map and boundaries

- `apps/ios/Riot/PostUpdateView.swift`: complete per-community draft persistence, post-success commands, responsive mode selection, focus, and success copy.
- `apps/ios/Riot/AppModel.swift`: Boolean display-name commit, publishing-context resolver, and prepare-before-community-mutation gate.
- `apps/ios/Riot/ConferenceShellView.swift`: first-run hierarchy, keyed shell lifetime, one composer sheet, exact Home composition, current community label, contextual notification request, and compact Nearby chrome.
- `apps/ios/Riot/CommunityChooser.swift`: pure tokened transition gate and chooser callbacks that cannot bypass it.
- `apps/ios/Riot/Transport/NearbyTransportController.swift`: pre-adoption transition hook before its repository join.
- `apps/ios/Riot/AlertsListView.swift`: single-clock active-alert presentation, two-row cap, and overflow list.
- `apps/ios/Riot/NewswireEditorial.swift`: complete ordinary row adapter, compact row/detail split, trust copy, treated detail, and action lineage.
- `apps/ios/Riot/PeopleView.swift`: Known-contributors vocabulary, Riot chrome, and independently focusable Technical details.
- `apps/ios/Riot/Directory/DirectoryView.swift`, `apps/ios/Riot/Apps/AppReviewSheet.swift`, `apps/ios/Riot/Peers/PeerProfileView.swift`: compact tool disclosure and consistent tool/community vocabulary.
- `apps/ios/Riot/RiotApp.swift`: isolated per-run UI-test storage only when the XCUITest launch environment requests it.
- Existing XCTest files receive all logic tests; existing `RiotTabNavigationUITests.swift` receives the real interaction smoke test. No new source/test file and no Xcode project edit.

### Task 0: Select an actually installed iOS simulator

**Files:**
- Modify: `scripts/ios-check.sh`

- [ ] **Step 1: Reproduce the destination failure**

Run `sh scripts/ios-check.sh sim`.

Expected RED on the current machine: `Unable to find a destination matching ...
name:iPhone 17 Pro, OS:latest` even though 26.1/26.2 devices are installed.

- [ ] **Step 2: Add one reusable simulator resolver**

```sh
resolve_simulator_id() {
  if [ -n "${RIOT_IOS_SIMULATOR_ID:-}" ]; then
    printf '%s\n' "$RIOT_IOS_SIMULATOR_ID"
    return
  fi
  xcrun simctl list devices available |
    awk '/^[[:space:]]+iPhone 17 Pro \(/ {
      gsub(/[()]/, "", $4); id = $4
    } END { if (id != "") print id }'
}
```

Add `simulator-id` to print the resolved UUID and make `sim` use
`-destination "platform=iOS Simulator,id=$(resolve_simulator_id)"`. Fail with a
fixed message if none exists. `RIOT_IOS_SIMULATOR_ID` remains the CI/local
override.

- [ ] **Step 3: Verify the resolver and simulator build GREEN**

```bash
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
test -n "$SIM_ID"
xcrun simctl list devices available | grep "$SIM_ID"
sh scripts/ios-check.sh sim
```

Expected: an available UUID is printed and the simulator build succeeds.

- [ ] **Step 4: Log and commit**

Append the structured Task 0 entry to `OVERNIGHT_LOG.md`.

```bash
git add OVERNIGHT_LOG.md scripts/ios-check.sh
git commit -m "fix(ios): resolve an installed simulator"
```

### Task 1: Preserve and reset the complete composer safely

**Files:**
- Modify: `apps/ios/Riot/PostUpdateView.swift`
- Test: `apps/ios/RiotTests/PostUpdateTests.swift`

- [ ] **Step 1: Write failing draft-compatibility and reset tests**

Add tests that decode the old five-field JSON, round-trip mode/expiry, and assert every reset value:

```swift
func testLegacyDraftDefaultsToUpdateWithoutExpiry() throws {
    let data = Data(#"{"headline":"H","body":"B","aiAssisted":false,"sourceClaims":[],"coarseLocation":""}"#.utf8)
    let draft = try JSONDecoder().decode(PostDraft.self, from: data)
    XCTAssertEqual(draft.mode, .freeform)
    XCTAssertNil(draft.expiresAtUnixSeconds)
}

@MainActor
func testPostAnotherClearsEveryFieldAndStoreWithoutPublishingAgain() {
    let publisher = PublisherStub()
    let store = DraftStoreSpy()
    let model = makeModel(publisher: publisher, store: store)
    model.headline = "Road closed"; model.body = "At the bridge"
    model.aiAssisted = true; model.mode = .operationalAlert
    model.sourceClaims = ["eyewitness"]; model.coarseLocation = "north bridge"
    model.expiresAt = Date(timeIntervalSince1970: 1_800_000_000)
    model.post()
    XCTAssertEqual(publisher.requests.count, 1)
    model.postAnother()
    XCTAssertEqual((model.headline, model.body), ("", ""))
    XCTAssertFalse(model.aiAssisted)
    XCTAssertEqual(model.mode, .freeform)
    XCTAssertEqual(model.sourceClaims, [])
    XCTAssertEqual(model.coarseLocation, "")
    XCTAssertNil(model.expiresAt)
    XCTAssertNil(model.errorMessage)
    XCTAssertEqual(model.status, .editing)
    XCTAssertEqual(publisher.requests.count, 1)
    XCTAssertEqual(store.clearCount, 2) // commit + post-another
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS' -only-testing:RiotKitTests-macOS/PostUpdateTests \
  -derivedDataPath build/xcode-dd CODE_SIGNING_ALLOWED=NO
```

Expected: compile/test failure because `PostDraft.mode`, `expiresAtUnixSeconds`, and `postAnother()` do not exist.

- [ ] **Step 3: Implement the backward-compatible draft and reset transition**

Make `ComposerMode` raw/Codable, add defaulting decode, include all fields in `currentDraft`/restore, and add the typed reset:

```swift
public enum ComposerMode: String, Codable, Equatable, Sendable, CaseIterable {
    case freeform, operationalAlert, operationalRequest
}

public struct PostDraft: Equatable, Codable, Sendable {
    public var headline: String
    public var body: String
    public var aiAssisted: Bool
    public var sourceClaims: [String]
    public var coarseLocation: String
    public var mode: ComposerMode
    public var expiresAtUnixSeconds: UInt64?

    public var isEmpty: Bool {
        headline.isEmpty && body.isEmpty && !aiAssisted
            && sourceClaims.isEmpty && coarseLocation.isEmpty
            && mode == .freeform && expiresAtUnixSeconds == nil
    }

    private enum CodingKeys: String, CodingKey {
        case headline, body, aiAssisted, sourceClaims, coarseLocation, mode, expiresAtUnixSeconds
    }

    public init(
        headline: String, body: String, aiAssisted: Bool,
        sourceClaims: [String], coarseLocation: String,
        mode: ComposerMode = .freeform, expiresAtUnixSeconds: UInt64? = nil
    ) {
        self.headline = headline; self.body = body; self.aiAssisted = aiAssisted
        self.sourceClaims = sourceClaims; self.coarseLocation = coarseLocation
        self.mode = mode; self.expiresAtUnixSeconds = expiresAtUnixSeconds
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        headline = try values.decode(String.self, forKey: .headline)
        body = try values.decode(String.self, forKey: .body)
        aiAssisted = try values.decode(Bool.self, forKey: .aiAssisted)
        sourceClaims = try values.decode([String].self, forKey: .sourceClaims)
        coarseLocation = try values.decode(String.self, forKey: .coarseLocation)
        mode = try values.decodeIfPresent(ComposerMode.self, forKey: .mode) ?? .freeform
        expiresAtUnixSeconds = try values.decodeIfPresent(UInt64.self, forKey: .expiresAtUnixSeconds)
    }
}

public func postAnother() {
    headline = ""; body = ""; aiAssisted = false
    mode = .freeform; sourceClaims = []; coarseLocation = ""; expiresAt = nil
    errorMessage = nil; status = .editing; draftStore.clear()
}
```

Render `Done` and `Post another` only for `.posted`, give both 44-point targets, use exact success copy `Saved and signed on this device. Exchange with someone nearby to share it.`, and focus `post-headline` after `postAnother`.

- [ ] **Step 4: Implement the accessibility-size mode control**

Read `dynamicTypeSize` and render the three modes as vertical labeled buttons at accessibility sizes; retain the segmented picker otherwise. Both variants bind to `model.mode` and expose `post-mode-update`, `post-mode-alert`, and `post-mode-request`.

- [ ] **Step 5: Run focused and shared tests GREEN**

Run the focused command above, then `sh scripts/ios-check.sh test`.

Expected: `PostUpdateTests` and the full shared suite pass.

- [ ] **Step 6: Commit**

Before committing, append a Task 1 entry to `OVERNIGHT_LOG.md` naming the design/plan/TDD skills used, RED and GREEN evidence, assumptions, rejected alternatives, skips/conflicts, and morning questions.

```bash
git add OVERNIGHT_LOG.md apps/ios/Riot/PostUpdateView.swift \
  apps/ios/RiotTests/PostUpdateTests.swift
git commit -m "feat(post): make repeat posting and drafts safe"
```

### Task 2: Make first run one clear, fail-closed path

**Files:**
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Test: `apps/ios/RiotTests/ShellNavigationTests.swift`

- [ ] **Step 1: Write failing setup-gate tests**

Add a pure destination and dispatcher contract:

```swift
func testSetupOrderAndUnsupportedNearbyBoundary() {
    XCTAssertEqual(OnboardingPresentation.actionOrder, [.join, .create, .demo])
    XCTAssertEqual(OnboardingPresentation.nearbyNote,
                   "Nearby exchange is available after you enter a community.")
}

func testNonEmptyNameFailureBlocksEveryExit() {
    for exit in OnboardingExit.allCases {
        var performed: [OnboardingExit] = []
        let result = OnboardingExitGate.perform(
            exit, displayName: "Ana",
            saveName: { _ in false },
            proceed: { performed.append($0) })
        XCTAssertEqual(result, .nameSaveFailed)
        XCTAssertEqual(performed, [])
    }
}

func testEmptyAndSuccessfullySavedNameCoverEveryExit() {
    for exit in OnboardingExit.allCases {
        var performed: [OnboardingExit] = []; var saved: [String] = []
        XCTAssertEqual(OnboardingExitGate.perform(
            exit, displayName: "", saveName: { _ in XCTFail(); return false },
            proceed: { performed.append($0) }), .proceeded)
        XCTAssertEqual(performed, [exit])

        performed = []
        XCTAssertEqual(OnboardingExitGate.perform(
            exit, displayName: "Ana", saveName: { saved.append($0); return true },
            proceed: { performed.append($0) }), .proceeded)
        XCTAssertEqual(saved, ["Ana"])
        XCTAssertEqual(performed, [exit])
    }
}
```

- [ ] **Step 2: Run ShellNavigationTests on the iOS simulator and verify RED**

Run:

```bash
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$(sh scripts/ios-check.sh simulator-id)" \
  -only-testing:RiotKitTests/ShellNavigationTests \
  -derivedDataPath build/xcode-dd CODE_SIGNING_ALLOWED=NO
```

Expected: missing `OnboardingPresentation`, `OnboardingExit`, and `OnboardingExitGate`.

- [ ] **Step 3: Add the pure setup state and Boolean name result**

Implement:

```swift
public enum OnboardingExit: CaseIterable, Equatable { case join, create, demo }
public enum OnboardingExitResult: Equatable { case proceeded, nameSaveFailed }
public enum OnboardingPresentation {
    public static let actionOrder: [OnboardingExit] = [.join, .create, .demo]
    public static let nearbyNote = "Nearby exchange is available after you enter a community."
}

@discardableResult
public func setDisplayName(_ name: String) -> Bool {
    guard let repository else { nameError = Self.nameRefusal; return false }
    do {
        try repository.setDisplayName(name)
        nameError = nil
        me = try repository.me()
        claimedName = repository.claimedName
        refreshDisplayNames()
        return true
    } catch {
        nameError = Self.nameRefusal
        return false
    }
}
```

`OnboardingExitGate.perform` skips saving for an empty trimmed name and otherwise proceeds only on `true`.

- [ ] **Step 4: Recompose setup**

Keep optional display name plus the disclosure `This self-claimed name is saved on this device and shared with future peers.` Remove `Save name`, `Find one nearby`, and the inline community-name field. Render:

1. filled `Join with a link or QR`;
2. secondary `Create a community`, presenting a create-name sheet;
3. secondary `Try the Riverside demo`;
4. the exact Nearby note.

Call the shared gate before presenting Join, confirming Create, or loading Demo.
The Create sheet observes `model.nameError` itself: a failed name save leaves the
sheet open, creates nothing, shows the fixed error beside Create, and focuses or
announces it. Join/demo failure stays on setup with the same behavior.

- [ ] **Step 5: Run focused tests and macOS compile GREEN**

Run the iOS focused command above and `sh scripts/ios-check.sh`.

Expected: tests pass and both shared views compile.

- [ ] **Step 6: Commit**

Append the structured Task 2 entry to `OVERNIGHT_LOG.md`, including the exact
create-sheet failure behavior and RED/GREEN commands.

```bash
git add OVERNIGHT_LOG.md apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/RiotTests/ShellNavigationTests.swift
git commit -m "feat(onboarding): make setup compact and fail closed"
```

### Task 3: Isolate community transitions and unify composer entry

**Files:**
- Modify: `apps/ios/Riot/CommunityChooser.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/Riot/PeopleView.swift`
- Modify: `apps/ios/Riot/NewswireEditorial.swift`
- Modify: `apps/ios/Riot/Transport/NearbyTransportController.swift`
- Test: `apps/ios/RiotTests/CommunityChooserTests.swift`
- Test: `apps/ios/RiotTests/ShellNavigationTests.swift`
- Test: `apps/ios/RiotTests/PeopleSurfaceTests.swift`
- Test: `apps/ios/RiotTests/NewswireSurfaceTests.swift`
- Test: `apps/ios/RiotTests/TransportContractTests.swift`

- [ ] **Step 1: Write failing transition and composer-state tests**

```swift
func testStaleShellCannotUnregisterNewTransitionPreparation() {
    let gate = CommunityTransitionGate()
    var calls: [String] = []
    let old = gate.register { _ in calls.append("old") }
    let new = gate.register { _ in calls.append("new") }
    gate.unregister(old)
    gate.prepare(.preserveDraft)
    XCTAssertEqual(calls, ["new"])
    gate.unregister(new)
}

func testEveryComposerOriginUsesOneOpenState() {
    var state = ComposerPresentationState.closed
    for origin in ComposerOrigin.allCases {
        state.open(origin)
        XCTAssertEqual(state, .open(origin))
        state.close()
        XCTAssertEqual(state, .closed)
    }
}
```

Add spy-backed model tests asserting `prepare` records before repository
`switch`, `join`, `create`, `leave`, and `retry`. Pin the transition reason:
switch/join/create/retry/deep link use `.preserveDraft`; confirmed Leave uses
`.discardDraft`. A failed preserving mutation leaves the persisted old draft
available; a confirmed discard clears it and cannot reappear.

Add a Nearby controller contract: before `pairing.resume(joining:)` can call the
host’s `joinSpace`, `onBeforeSpaceJoin` fires once. The shell wires it to
`gate.prepare(.preserveDraft)`. Assert preparation precedes join and a Community
A draft remains in A’s keyed store. First-run Nearby is removed and an
existing-community peer cannot adopt a different namespace, but this closes the
transport’s remaining mutation seam.

- [ ] **Step 2: Run the four focused suites RED**

Run `CommunityChooserTests` and `ShellNavigationTests` on the iOS `RiotKit`
scheme:

```bash
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$(sh scripts/ios-check.sh simulator-id)" \
  -only-testing:RiotKitTests/CommunityChooserTests \
  -only-testing:RiotKitTests/ShellNavigationTests \
  -derivedDataPath build/xcode-dd CODE_SIGNING_ALLOWED=NO
```

Run `PeopleSurfaceTests` and `NewswireSurfaceTests` on the macOS scheme with
their `RiotKitTests-macOS` selectors. Run `TransportContractTests` on the iOS
scheme in the same simulator test invocation.

Expected: missing gate/presentation types and default no-op initializer assertions fail.

- [ ] **Step 3: Implement the tokened transition gate**

```swift
public enum CommunityTransitionReason { case preserveDraft, discardDraft }
public final class CommunityTransitionGate {
    public struct Token: Equatable { fileprivate let id: UUID }
    private var active: (Token, (CommunityTransitionReason) -> Void)?
    public func register(_ prepare: @escaping (CommunityTransitionReason) -> Void) -> Token {
        let token = Token(id: UUID()); active = (token, prepare); return token
    }
    public func unregister(_ token: Token) {
        if active?.0 == token { active = nil }
    }
    public func prepare(_ reason: CommunityTransitionReason) { active?.1(reason) }
}
```

Give `RiotAppModel` one gate and call `prepare()` synchronously before every active-community mutation. Key `CommunityShellView` with `.id(community.id)`. Register a closure that persists `composer`, closes sheets/tools, clears callbacks, and stops Nearby; unregister its own token on disappear.
For `.discardDraft`, clear the draft store instead of persisting. Preparation
does not unregister itself, so a failed repository mutation leaves the current
shell able to continue safely.

- [ ] **Step 4: Implement one composer sheet**

Define `ComposerOrigin` (`home`, `emptyWire`, `people`) and `ComposerPresentationState`. Remove the embedded `PostUpdateView`. Require explicit `onPostUpdate` in `NewswireSurfaceView` and `PeopleView` initializers. Pass `openComposer(origin)` from all call sites and restore focus to the matching trigger when the sheet closes.

Use per-wire placement: empty card owns `Post the first update`; pending/offline has none; populated wire has one standalone `Post an update`.

Give the editing sheet a `Close` toolbar action; after success replace it with
`Done` plus `Post another`. `Close`/`Done` dismiss through one `onDone` callback
and restore focus to the exact `ComposerOrigin`; `Post another` stays open and
focuses Headline.

Add a `PublishingContextProviding` resolver that returns current identity,
community ID, and descriptor ID. Refresh on sheet presentation, on observed
`model.me`/descriptor changes, and immediately before Post. A context mismatch
sets fixed failure copy and performs no write; a newly arrived descriptor makes a
previous pending-first-sync composer usable without rebuilding the app.

- [ ] **Step 5: Move notification permission to rendered success**

Inject a notifier factory at `ConferenceShellView`; remove the community-open `.task`. From the posted success branch, yield one render turn and invoke the notifier’s `requestAuthorizationIfNeeded()`. Add scheduler-spy tests proving only the first undetermined success requests.
The same success callback calls `newswire.load()` before notification work, so
the locally committed report becomes immediately readable on Home.

- [ ] **Step 6: Show the visible community name**

Replace the icon-only phone chooser with a 44-point `community.name + chevron.down` label, line-limit one, while retaining profile/settings controls and `community-name`.

- [ ] **Step 7: Run all affected tests and shared compile GREEN**

Run the four focused suites, `sh scripts/ios-check.sh test`, and
`sh scripts/ios-check.sh`.

- [ ] **Step 8: Commit**

Append the structured Task 3 log entry, including preserve-versus-discard
semantics, any failed mutation evidence, and the community-isolation assumption.

```bash
git add OVERNIGHT_LOG.md apps/ios/Riot/CommunityChooser.swift \
  apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/ConferenceShellView.swift apps/ios/Riot/PeopleView.swift \
  apps/ios/Riot/NewswireEditorial.swift \
  apps/ios/Riot/Transport/NearbyTransportController.swift \
  apps/ios/RiotTests/CommunityChooserTests.swift \
  apps/ios/RiotTests/ShellNavigationTests.swift \
  apps/ios/RiotTests/PeopleSurfaceTests.swift \
  apps/ios/RiotTests/NewswireSurfaceTests.swift \
  apps/ios/RiotTests/TransportContractTests.swift
git commit -m "feat(shell): isolate communities and unify posting"
```

### Task 4: Put only current, bounded alerts first

**Files:**
- Modify: `apps/ios/Riot/AlertsListView.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Test: `apps/ios/RiotTests/AlertsSurfaceTests.swift`

- [ ] **Step 1: Write failing deterministic presentation tests**

```swift
func testExpiredAndForeignAlertsAreOmitted() {
    let now = Date(timeIntervalSince1970: 100)
    let state = ActiveAlertsPresentation.from(
        [entry(namespace: active, expiry: 100),
         entry(namespace: foreign, expiry: 200)],
        activeNamespaceID: active, now: now)
    XCTAssertEqual(state, .hidden)
}

func testThreeActiveAlertsCapAtTwoWithCountedOverflow() {
    let state = ActiveAlertsPresentation.from(threeActiveEntries,
        activeNamespaceID: active, now: Date(timeIntervalSince1970: 100))
    guard case let .visible(rows, total) = state else { return XCTFail() }
    XCTAssertEqual(rows.count, 2)
    XCTAssertEqual(total, 3)
    XCTAssertEqual(state.overflowLabel, "View all 3 active alerts")
}
```

- [ ] **Step 2: Run AlertsSurfaceTests RED**

Expected: missing `ActiveAlertsPresentation` and expired-only currently populates.

- [ ] **Step 3: Implement one filter/clock/result**

Filter `namespaceID` and `expiresAt > now`, map/sort once organizer-first then newest, retain `allRows`, expose `prefix(2)` and total. Make `AlertsListView` accept this presentation rather than raw entries/`Date()`.
Expose `nextExpiryDate`. Home owns one `now` state and a cancellable `.task(id:
nextExpiryDate)` that sleeps until that exact instant, advances `now`, and
recomputes; switching communities cancels the old task. Add a clock-injected test
that the last active row disappears at its expiry while Home is otherwise idle.

- [ ] **Step 4: Render exact Home order and overflow**

Home order is active alerts, populated-wire post trigger, Newswire, Tools. Omit hidden alerts. For three or more show `View all N active alerts`, open the complete precomputed list, and restore focus to the overflow button on Done.

- [ ] **Step 5: Run tests and compile GREEN**

Run `AlertsSurfaceTests`, `ShellNavigationTests`, and `sh scripts/ios-check.sh`.

- [ ] **Step 6: Commit**

Append the structured Task 4 log entry before committing.

```bash
git add OVERNIGHT_LOG.md apps/ios/Riot/AlertsListView.swift \
  apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/RiotTests/AlertsSurfaceTests.swift
git commit -m "feat(home): keep active alerts compact and visible"
```

### Task 5: Make Newswire reports readable and accountable

**Files:**
- Modify: `apps/ios/Riot/NewswireEditorial.swift`
- Test: `apps/ios/RiotTests/NewswireSurfaceTests.swift`

- [ ] **Step 1: Write failing row, redaction, trust, and lineage tests**

```swift
func testOrdinaryRowCarriesReadableAndOperationalFields() {
    let row = NewswirePostRow(projectedPost())
    XCTAssertEqual(row.body, "Full body")
    XCTAssertEqual(row.sourceClaims, ["eyewitness"])
    XCTAssertEqual(row.coarseLocation, "north bridge")
    XCTAssertEqual(row.operationalProfile, .alert)
    XCTAssertEqual(row.taiJ2000Micros, 42)
}

func testTreatedRowDropsEveryPayloadField() {
    for treatment in [NewswirePostTreatment.hidden, .tombstoned] {
        let row = NewswirePostRow(projectedPost(treatment: treatment))
        XCTAssertNil(row.headline); XCTAssertNil(row.body)
        XCTAssertEqual(row.sourceClaims, []); XCTAssertNil(row.coarseLocation)
        XCTAssertNil(row.operationalProfile)
    }
}

func testActionLineageIncludesRetractionOfDirectAction() {
    let rows = EditorialActionLineage.forReport("post", in: [
        action(id: "hide", target: "post", kind: .hide),
        action(id: "undo", target: "hide", kind: .retract),
        action(id: "other", target: "elsewhere", kind: .verify)])
    XCTAssertEqual(rows.map(\.id), ["hide", "undo"])
}
```

Pin exact signature/editorial/AI copy and assert Retract calls
`sign(targetEntryID: selectedAction.id)`. Retain `taiJ2000Micros` on
`EditorialHistoryRow`; treatment detail labels report/action values as
`Signed ordering value (TAI-J2000 microseconds)` under Technical details. Tests
pin the exact unsigned values and prove no date is fabricated from them.

- [ ] **Step 2: Run NewswireSurfaceTests RED**

Expected: row body/metadata and lineage/detail types do not exist.

- [ ] **Step 3: Extend the defensive row adapter**

For `.ordinary`, map body, event time, expiry, sources, location, and operational profile from `NewswireProjectedPost`. For treated states assign nil/empty regardless of projected values. Change hidden copy to `The collective hid this report. Its signed treatment record remains available.`

- [ ] **Step 4: Build compact row and ordinary detail**

Row: headline, two-line body excerpt, `Signed by <rendered>`, conditional badges,
and `Read update` with accessibility label `Read <headline>`. Detail: full body,
source claims, location, expiry, operational type, replies, and authorized
actions. Show a human event time only when `eventTimeUnixSeconds` exists;
otherwise show `Event time not provided`. Never convert `taiJ2000Micros` into a
wall clock in Swift. Add exact signature/editorial explanations from the
approved design.

- [ ] **Step 5: Build payload-redacted treatment detail**

Add `Review treatment` for hidden/tombstoned rows. Show type, signed author/tag,
optional event time or `Event time not provided`, Technical details ID and signed
TAI ordering value, and `EditorialActionLineage` with each action’s ordering
value. Keep action-scoped Retract beside active lineage actions; never render
body, operational metadata, or replies.
Remove the existing inline `commentsSection`, Reply, and generic editorial
controls from every feed row. Ordinary detail owns replies/actions; treated
detail owns only treatment history and action-scoped editorial controls.

- [ ] **Step 6: Run focused/full shared tests GREEN**

Run `NewswireSurfaceTests` and `sh scripts/ios-check.sh test`.

- [ ] **Step 7: Commit**

Append the structured Task 5 log entry, explicitly recording the conditional
event-time assumption and the deferred hidden-original core gap.

```bash
git add OVERNIGHT_LOG.md apps/ios/Riot/NewswireEditorial.swift \
  apps/ios/RiotTests/NewswireSurfaceTests.swift
git commit -m "feat(newswire): add compact readable report details"
```

### Task 6: Compact Tools, Known contributors, and Nearby

**Files:**
- Modify: `apps/ios/Riot/Directory/DirectoryView.swift`
- Modify: `apps/ios/Riot/Apps/AppReviewSheet.swift`
- Modify: `apps/ios/Riot/Peers/PeerProfileView.swift`
- Modify: `apps/ios/Riot/PeopleView.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Test: `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- Test: `apps/ios/RiotTests/PeopleSurfaceTests.swift`
- Test: `apps/ios/RiotTests/ShellNavigationTests.swift`

- [ ] **Step 1: Write failing vocabulary/disclosure tests**

Pin `Known contributors`, `No known contributors yet`, `People you’ve synced with`, `Nearby devices`, and `Add 3 updates`; assert no user-facing `Renderer: incident-board/1`, `Recently synced`, or generic `space app`.

Add a `PersonRowAccessibility` test:

```swift
XCTAssertEqual(PersonRowAccessibility.summary(row).label,
               "Ana · a3f91122, Organizer, 2 contributions")
XCTAssertEqual(PersonRowAccessibility.technicalLabel(row),
               "Technical details for Ana · a3f91122")
XCTAssertFalse(PersonRowAccessibility.summary(row).label.contains(row.id))
```

- [ ] **Step 2: Run focused tests RED**

Expected: current Contributors strings and renderer/inline tool detail assertions fail.

- [ ] **Step 3: Compact Tools**

Render name, purpose, trust/status badges, and one action. Move permissions and recommendation/share controls under `More details for <tool>` or existing review. Replace visible app/space-app/space vocabulary with tool/community in the three scoped files; do not rename protocol types.

- [ ] **Step 4: Fix Known contributors and VoiceOver**

Use Riot header/card typography and exact Known-contributors copy. Do not put the whole row under `.accessibilityElement(children: .ignore)`. Give the summary its composed label and the disclosure a separate focusable label; keep the full ID absent until expansion.

- [ ] **Step 5: Compact Nearby**

Keep automatic discovery and all consent/recovery states. Shorten repeated explanation, use truthful headings, remove renderer diagnostics, preserve offered count and `Add N updates`, and keep essential small text in ink/ink-soft rather than pink.

- [ ] **Step 6: Run tests and shared compile GREEN**

Run affected suites, `sh scripts/ios-check.sh test`, and `sh scripts/ios-check.sh`.

- [ ] **Step 7: Commit**

Append the structured Task 6 log entry before committing.

```bash
git add OVERNIGHT_LOG.md apps/ios/Riot/Directory/DirectoryView.swift \
  apps/ios/Riot/Apps/AppReviewSheet.swift \
  apps/ios/Riot/Peers/PeerProfileView.swift \
  apps/ios/Riot/PeopleView.swift apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/RiotTests/DirectoryStorefrontTests.swift \
  apps/ios/RiotTests/PeopleSurfaceTests.swift \
  apps/ios/RiotTests/ShellNavigationTests.swift
git commit -m "feat(ux): compact secondary community surfaces"
```

### Task 7: Prove the real interaction and release gates

**Files:**
- Modify: `apps/ios/Riot/RiotApp.swift`
- Modify: `apps/ios/RiotUITests/RiotTabNavigationUITests.swift`
- Modify: `OVERNIGHT_LOG.md`

- [ ] **Step 1: Update the existing simulator smoke flow**

Make the UI test deterministic: set `RIOT_UI_TEST_RUN_ID` to a UUID in
`XCUIApplication.launchEnvironment`; `RiotApp` resolves that only under UI tests
to a unique app-sandbox temporary storage directory and otherwise uses production
bootstrap. Drive one concrete path: Get started → verify no
`find-nearby`/`launch-save-display-name` → type display name `Ana` → Create sheet
→ enter community name → Home visible community name → open Your profile and
assert `Ana` plus its key-derived tag → the single composer entry → enter
headline/body → post success → Post another → focused Headline → enter/post a
second report → Done → Read update → close to exact trigger.

- [ ] **Step 2: Run the UI flow RED then GREEN**

Run:

```bash
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination "platform=iOS Simulator,id=$(sh scripts/ios-check.sh simulator-id)" \
  -only-testing:RiotUITests/RiotTabNavigationUITests \
  -derivedDataPath build/xcode-dd CODE_SIGNING_ALLOWED=NO
```

Expected before final identifier/focus adjustments: at least one audited transition fails. Make only the minimal identifier/focus adjustments in already scoped files, rerun, and require PASS.

- [ ] **Step 3: Capture and inspect native simulator screenshots**

The Playwright-based `metaswarm:visual-review` skill cannot drive a native
SwiftUI app, so do not invoke it. Add `XCTAttachment` screenshots at first run,
populated Home, ordinary report detail, post success, Tools, Known contributors,
and Nearby. For accessibility size, boot the named simulator and run:

```bash
SIM_ID=$(sh scripts/ios-check.sh simulator-id)
xcrun simctl boot "$SIM_ID" 2>/dev/null || true
xcrun simctl ui "$SIM_ID" content_size accessibility-extra-extra-extra-large
```

Rerun the UI flow and inspect kept result-bundle attachments plus an on-demand
native capture:

```bash
xcrun simctl io "$SIM_ID" screenshot /tmp/riot-ux-accessibility.png
```

Reject clipping, horizontal scrolling, duplicate filled actions, buried alerts,
missing focus labels, or more than two inline alerts. Record observations in the
log; do not commit screenshots or DerivedData. Restore simulator content size to
`large` afterward with `xcrun simctl ui "$SIM_ID" content_size large`.

- [ ] **Step 4: Run all Apple gates**

```bash
sh scripts/ios-check.sh test
sh scripts/ios-check.sh sim
sh scripts/ios-check.sh ios
plutil -lint apps/ios/Riot.xcodeproj/project.pbxproj
plutil -lint apps/macos/Riot.xcodeproj/project.pbxproj
cargo test --workspace --all-features
cargo check --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
sh scripts/web/coverage.sh
```

Expected: all commands pass at the committed `.coverage-thresholds.json` floors.

- [ ] **Step 5: Write the morning summary at the top of the append-only log**

Prepend a short summary listing completed behavior/tests, remaining Android and physical-BLE limitations, assumptions, and next steps. Preserve every chronological task entry below it.

- [ ] **Step 6: Commit the final verification**

```bash
git add apps/ios/Riot/RiotApp.swift \
  apps/ios/RiotUITests/RiotTabNavigationUITests.swift OVERNIGHT_LOG.md
git commit -m "test(ux): verify the compact core flow"
```

- [ ] **Step 7: Integrate into the requested branch without overwriting foreign work**

Confirm `/Users/rabble/code/explorations/riot/.claude/worktrees/overnight` is
clean and still owns `overnight/2026-07-19`. Merge
`overnight/2026-07-19-ux` there with `--no-ff`. Resolve only the expected
`OVERNIGHT_LOG.md` add/add conflict by preserving the existing agent’s summary
and chronological entries plus every entry from this plan; do not rewrite either
history. Re-run `sh scripts/ios-check.sh test` from the requested branch and
commit the combined log/merge normally. Do not push or deploy.

## Plan self-review

- Spec coverage: first run, setup identity, Home hierarchy, active alerts, one composer, repeat posting, complete ordinary detail, treated accountability, exact trust language, community isolation, notification timing, Tools, Known contributors, Nearby, accessibility, visual review, and release gates each map to a task.
- Placeholder scan: no TBD/TODO/“similar to” steps; each change step names concrete types, copy, behavior, tests, and commands.
- Type consistency: `CommunityTransitionGate`, `ComposerPresentationState`, `ActiveAlertsPresentation`, `EditorialActionLineage`, `OnboardingExitGate`, and `postAnother()` have one spelling and ownership throughout.
- Scope: no new dependency, core/FFI/database policy, source file, or project-file registration. The only persistence shape change is backward-compatible local draft JSON.
