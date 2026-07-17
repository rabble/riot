# iOS Surface — Unit 7: Dead-end fixes (Open-wire no-op, offlineStale loop, pending-first-sync) — Implementation Plan


**Plan-review gate: PASSED** (Feasibility + Scope + Completeness, 2026-07-18).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Kill the three newswire-surface dead-ends called out in design §7 — every terminal state must offer a reachable next action, never a dead no-op button and never a silent retry loop. (1) The no-op "Open wire" button (`NewswireEditorial.swift:652`, empty `{}`) → remove it; the open wire already renders directly below. (2) The `offlineStale` "Try again" loop (`NewswireEditorial.swift:637` → `model.load()` re-projecting the same empty descriptor id) → re-derive the descriptor id first, and if it is still absent (a nearby-joined community with no `descriptorEntryId`), offer a real forward path instead of re-looping. (3) The post-join "pending first sync" state → an honest waiting message that leads with the verified-working path (rejoin with a link) and keeps the red-on-main Nearby path secondary. NOT the chooser Create/Find-nearby no-ops — those are Unit 1.

**Architecture:** All three fixes are modeled as **pure data on `NewswireSurfaceModel`** so the view carries no dead closures and every terminal state is unit-testable as a value. A new `NewswireWireForwardAction` enum enumerates the forward paths (`retry` / `postFirstUpdate` / `rejoinWithLink` / `syncWithPeer`); a computed `forwardActions: [NewswireWireForwardAction]` decides — purely, over the model's already-loaded state — WHICH actions each wire state offers and IN WHAT ORDER. The view renders buttons straight from that array (no inline `(label, {})` literals survive). `load()` gains a **descriptor re-derive seam** (`descriptorResolver: (() -> String?)?`, appended onto Unit 4b's post-refactor `init`, injected by the shell, defaulting to `nil` so 4b's own constructions are unaffected): a community whose registry row gained a `descriptorEntryId` after this model was built (a switch/join whose sync landed) is picked up on `load()`/`retry()` instead of re-looping on the empty id. **This unit lands on top of Unit 4b** — see "Cross-unit coupling (gate r1)" for the merged `init`/shell/test signatures (`authority:` from 4b + `descriptorResolver:` from Unit 7, no `roster:`). The shell wires the resolver to a new on-demand `RiotAppModel.rederivedNewswireDescriptorID()` (the same `listCommunities()` derivation `reload()` already does) and passes `onSyncWithPeer` / `onRejoinWithLink` navigation callbacks into `NewswireSurfaceView`. No new FFI, no `uniffi::Record`, no new Swift file.

**Tech stack:** Swift 6 / SwiftUI, XCTest. Design: `docs/superpowers/specs/2026-07-18-ios-surface-built-capabilities-design.md` §7 (dead-end fixes) + §3 (pending-first-sync ordering) + §2 anti-dead-end invariants.

**Shared-checkout:** **No new Swift files** → **no `project.pbxproj` edit on either project** (both edited files — `NewswireEditorial.swift`, `AppModel.swift`, `ConferenceShellView.swift` — and the test file `NewswireSurfaceTests.swift` are already registered). No COLLABORATION.md pbxproj claim is required. But this unit **collides with Unit 4b** on `NewswireEditorial.swift` / `ConferenceShellView.swift` / `NewswireSurfaceTests.swift` (see "Cross-unit coupling" below) → **claim `apps/ios/Riot/NewswireEditorial.swift`, `apps/ios/Riot/ConferenceShellView.swift`, `apps/ios/Riot/AppModel.swift`, `apps/ios/RiotTests/NewswireSurfaceTests.swift` in COLLABORATION.md before editing; Unit 4b and Unit 7 must NOT run concurrently on these files.** Pathspec commits; absolute `git`/`grep`.

---

## Cross-unit coupling (gate r1)

**Unit 7 collides with Unit 4b** and both edit `NewswireSurfaceModel.init`, the shell construction (`ConferenceShellView.swift:305-312`), and `NewswireSurfaceTests.swift`. Reconciliation is AGREED and mirrored in the Unit 4b plan (`docs/superpowers/plans/2026-07-18-ios-surface-unit4b-editor-ungate.md` §"Cross-unit coupling", lines 33-34):

- **Land order: Unit 4b BEFORE Unit 7.** Unit 7 builds ON TOP of 4b's already-refactored `NewswireSurfaceModel`. Unit 4b and Unit 7 must not run concurrently on the shared files (claim them in COLLABORATION.md).
- **What 4b changes first:** 4b **removes** `roster: [String]?` from `NewswireSurfaceModel.init` and **adds** `authority: NewswireEditorAuthorityChecking`; it **deletes** `CommunityContext.editorialRoster` (and the `EditorialAuthority` enum). So after 4b lands there is **no `roster:` init param and no `community.editorialRoster` field**.
- **The final `init` carries BOTH new params:** 4b's `authority:` (which replaced `roster:`) **and** Unit 7's `descriptorResolver:`. Unit 7 appends `descriptorResolver: (() -> String?)? = nil` after `initialDraftKind` onto 4b's post-refactor signature:
```swift
public init(
    projector: NewswireProjecting,
    editor: NewswireEditorialActing,
    authority: NewswireEditorAuthorityChecking,   // 4b (replaced roster:)
    spaceDescriptorEntryID: String,
    communityName: String,
    myKeyHex: String,
    initialDraftKind: EditorialActionKind = .feature,
    descriptorResolver: (() -> String?)? = nil    // Unit 7 (this plan)
)
```
- **Shell wiring (`ConferenceShellView.swift:305-312`) post-4b passes `authority:` and `descriptorResolver:`, NEVER `roster: community.editorialRoster`** (that field is deleted by 4b). Task 4's construction below is written against the post-4b signature.
- **`load()` ordering:** 4b computes `isEditor` at the TOP of `load()` off `spaceDescriptorEntryID`. Unit 7's descriptor re-derive must run **before** that `isEditor` computation, so the predicate sees the freshly re-derived id (a community whose descriptor just landed is BOTH projected and correctly editor-gated in one `load()`). Task 2's `load()` rewrite places the re-derive as the first statement, ahead of 4b's `isEditor = (try? authority.newswireIsEditor(...))`.
- **Tests:** every `NewswireSurfaceModel(...)` construction in this plan uses the post-4b signature — `authority:` supplied, **no `roster:`**. Unit 7's pure wire-state tests pass a trivial `StubAuthority` (editor status is irrelevant to wire state); the `descriptorResolver` default stays `nil` so 4b's own offline/stale constructions (which don't pass it) are unaffected.
- **If Unit 7 somehow must land BEFORE 4b (not recommended):** keep `roster: [String]?` additively in the init and pass `roster: community.editorialRoster` in the shell, and note 4b will remove both — but prefer 4b-first.

**Cross-unit dependency on Unit 1 (separate — flagged, see Self-Review):** the `rejoinWithLink` forward path presents **Unit 1's `JoinByReferenceSheet`**, which does not exist yet (`apps/ios/Riot/JoinByReferenceSheet.swift` absent as of this plan). Unit 7's **model + tests are fully independent of Unit 1** (they assert the pure `forwardActions` data, never the sheet). Only the **shell wiring of `onRejoinWithLink` → present `JoinByReferenceSheet`** (Task 4) depends on Unit 1. **Sequence Unit 1 before Unit 7.** If Unit 7 must land first, wire `onRejoinWithLink` to the same forward action the Unit 1 Launch button will use once it exists, and keep `syncWithPeer` (→ Nearby, a real existing screen) as the interim reachable action — never leave `rejoinWithLink` a dead `{}`. (Combined order: **4b → 1 → 7**, or at minimum **4b → 7** with the Unit 1 interim wiring.)

---

## Ground truth (verified)

- **The three dead-ends live in `NewswireEditorial.swift`:**
  - No-op Open-wire button — `postsButNoFeature` renders `wireEmpty(..., primary: (NewswireWireCopy.noFeatureLink, {}))` (`:646-653`). `NewswireWireCopy.noFeatureLink = "Open wire"` (`:449`). The `openWireCard(openWire)` renders **directly below it in the same `VStack`** (`:654`), and that card's eyebrow is already `"Open wire"` (`:698`) — the button is a redundant no-op pointing at content that is already visible and already labeled.
  - `offlineStale` "Try again" loop — `wireEmpty(..., primary: ("Try again", { model.load() }))` (`:632-638`). `load()` (`:526-544`) guards `!spaceDescriptorEntryID.isEmpty`; when the id is empty it sets `wire = .offlineStale` and returns, so "Try again" re-projects the SAME empty id and re-loops silently.
  - The model is built with `spaceDescriptorEntryID` as a `private let` captured once at shell init (`:491`, `init` `:496-514`); a descriptor that lands after construction is never picked up.
- **`wireEmpty(id:title:message:primary:)`** (`:665-683`) draws one `Button(primary.label, action: primary.action).buttonStyle(.riotPrimary).frame(minHeight: 44)` inside a `RiotCard`. `primary` is a non-optional `(label: String, action: () -> Void)`. `.riotPrimary` / `.riotSecondary` button styles both exist in-file (`.riotSecondary` used at `:743`).
- **`NewswireWireState`** (`:401-439`) is the four-state enum: `offlineStale` / `emptyWire` / `postsButNoFeature(openWire:)` / `featured(frontPage:openWire:)`, each with a distinct `accessibilityID` (`:431-438`). `NewswireWireCopy` (`:443-453`) pins the copy: `offlineTitle`/`offlineMessage` ("This community's wire is offline or has not synced yet. What you already have is still here."), `emptyTitle`/`emptyMessage`, `noFeatureTitle`/`noFeatureMessage`, `noFeatureLink`.
- **`NewswireSurfaceView`** (`:605-811`) is `init(model:onPostUpdate:)` (`onPostUpdate` defaults to `{}`); `wireSection` (`:629-663`) switches on `model.wire`. `emptyWire`'s primary is `("Post the first update", onPostUpdate)`; in the shell `NewswireSurfaceView(model: newswire)` passes no `onPostUpdate`, so that button is already a `{}` no-op — **out of scope** (§7 lists Open-wire / offlineStale / pending-sync only), preserved as-is by routing it through the existing `onPostUpdate` seam.
- **`load()` degradation is the honest pattern to preserve** (`:526-544`): a missing descriptor id or a projection throw both become `.offlineStale` — "never a raw internal error, never invented content." `NewswireSurfaceTests` proves it: `testMissingDescriptorIsOfflineStaleNeverAFabricatedEmptyWire` (`:241`) and `testProjectionFailureIsOfflineStaleNeverARawError` (`:251`).
- **`AppModel.reload()`** (`:490-517`) already re-derives the descriptor per reload: `newswireDescriptorEntryID = (try? repository.listCommunities())?.first { $0.namespaceId == namespaceID }?.descriptorEntryId` (`:501-503`), else `nil` (`:505`). The comment (`:494-499`) states the exact intent: "any community reached after an app relaunch or a switch had a permanently dead newswire" — this unit extends that re-derive to the `offlineStale` recovery path so a model built before the sync can pick the id up without a full shell rebuild. `RiotAppModel` exposes `select(_:)` (`:358`) for `.nearby` navigation and `newswireDescriptorEntryID` / `space` / `profileRepository`.
- **Shell construction** (`ConferenceShellView.swift:305-312`, as it exists TODAY, pre-4b): `_newswire = StateObject(wrappedValue: NewswireSurfaceModel(projector:editor:spaceDescriptorEntryID: community.newswireDescriptorEntryID ?? "", communityName:myKeyHex:roster:))`. **Unit 7 lands AFTER 4b, which replaces `roster: community.editorialRoster` with `authority:` — so Unit 7 edits the post-4b construction (`authority:` present, no `roster:`) and only adds `descriptorResolver:`.** `HomeRouteView` (`:605-628`) holds `@ObservedObject var model: RiotAppModel` and renders `NewswireSurfaceView(model: newswire)` (`:622`) with `PostUpdateView` and the shortcuts card in the same `ScrollView`. The Home route is built at `routeView(.home)` (`:524-529`).
- **`NewswireSurfaceTests`** (the mirror): pure surface logic is asserted as VALUES, no store — e.g. `testEmptyWirePostsButNoFeatureAndOfflineStaleAreThreeDistinctStates` (`:215`) asserts the four `accessibilityID`s are distinct and the three copy messages differ. Stubs `ThrowingProjector` / `ThrowingEditor` (`:494-506`) always throw. The `String.repeated(_:)` helper (`:509-512`) builds full-length hex ids. **Post-4b these stubs/constructions already carry the `authority:` seam (4b added it + a test authority stub); Unit 7 mirrors that and adds `descriptorResolver:` where needed.** The `descriptorResolver:` default (`nil`) means 4b's own constructions that don't pass it stay valid.
- **No `Nearby` sync claim is made:** two-peer nearby sync is red on main (MEMORY: `riot-two-peer-sync-red`). The `syncWithPeer` action only NAVIGATES to the existing Nearby screen (a real reachable action); it never asserts sync succeeds, and per §3 it is never the headline.
- **`JoinByReferenceSheet` does not exist yet** (`ls apps/ios/Riot/JoinByReferenceSheet.swift` → absent). It is Unit 1's deliverable — the `rejoinWithLink` wiring dependency (see the flagged cross-unit note above).

---

## Task 1: Kill the no-op "Open wire" button — model the forward actions as data

**Files:** Modify `apps/ios/Riot/NewswireEditorial.swift`; Test `apps/ios/RiotTests/NewswireSurfaceTests.swift`

Introduce `NewswireWireForwardAction` + a pure `forwardActions` property so no wire state carries a dead closure, and `postsButNoFeature` offers NO button (its next action is the open wire rendered directly below).

- [ ] **Step 1: Failing test.**
```swift
// MARK: - Dead-end fixes (Unit 7)

func testPostsButNoFeatureOffersNoDeadButtonTheOpenWireIsTheNextAction() {
    let post = projectedPost(id: "a1", headline: "Report", treatment: .ordinary)
    let model = NewswireSurfaceModel(
        projector: FixedProjector(projection(openWire: [post], frontPage: [])),
        editor: ThrowingEditor(),
        authority: StubAuthority(),                 // 4b seam; editor status irrelevant here
        spaceDescriptorEntryID: "desc", communityName: "Riverside",
        myKeyHex: "aa".repeated(32))
    model.load()
    guard case let .postsButNoFeature(openWire) = model.wire else {
        return XCTFail("posts but no feature")
    }
    // The next action is the visible open wire, not a button.
    XCTAssertFalse(openWire.isEmpty, "the open wire content IS the reachable next action")
    XCTAssertTrue(model.forwardActions.isEmpty, "the dead 'Open wire' no-op button is gone")
}

func testEveryTerminalWireStateHasAReachableNextActionAndNoDeadNoOp() {
    // emptyWire → post the first update; offlineStale → a forward path; the two
    // content states → the content itself. No state offers a no-op button.
    let empty = NewswireSurfaceModel(
        projector: FixedProjector(projection(openWire: [], frontPage: [])),
        editor: ThrowingEditor(), authority: StubAuthority(),
        spaceDescriptorEntryID: "desc",
        communityName: "R", myKeyHex: "aa".repeated(32))
    empty.load()
    XCTAssertEqual(empty.wire, .emptyWire)
    XCTAssertEqual(empty.forwardActions, [.postFirstUpdate])
}
```

- [ ] **Step 2: Run → FAIL** (`NewswireWireForwardAction`, `FixedProjector`, `forwardActions` undefined). `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/NewswireSurfaceTests`

- [ ] **Step 3: Implement.** Add the pure action type near `NewswireWireCopy` in `NewswireEditorial.swift`:
```swift
/// A forward action a wire state offers, as pure data. The view maps each kind
/// to a handler; the model decides WHICH are offered and IN WHAT ORDER. Modeled
/// as data (never an inline `(label, {})` closure) so a test can assert every
/// terminal state has at least one reachable next action, none is a dead no-op,
/// and the red-on-main Nearby path is never a state's headline action.
public enum NewswireWireForwardAction: String, Equatable, Sendable, CaseIterable {
    /// Re-derive the descriptor id and reproject — a known-descriptor community
    /// that is transiently offline.
    case retry
    /// The empty wire's call to the composer (routed through `onPostUpdate`).
    case postFirstUpdate
    /// The verified-working join path (Unit 1's `JoinByReferenceSheet`) — the
    /// pending-first-sync headline.
    case rejoinWithLink
    /// Nearby — offered but SECONDARY, never a headline: two-peer nearby sync is
    /// red on main, so it must never be a state's first action.
    case syncWithPeer

    public var label: String {
        switch self {
        case .retry: "Try again"
        case .postFirstUpdate: "Post the first update"
        case .rejoinWithLink: "Rejoin with a link"
        case .syncWithPeer: "Sync with a peer"
        }
    }

    /// A stable per-action id so each forward button is individually addressable.
    public var accessibilityID: String { "newswire-action-\(rawValue)" }

    /// True for the red-on-main Nearby path. A state must NEVER place this first.
    public var isNearbyPath: Bool { self == .syncWithPeer }
}
```
Remove the now-dead `noFeatureLink` constant (`:449`) — confirm `grep -rn "noFeatureLink" apps/ios apps/macos` shows only its definition and the `:652` use before deleting both.

Add the `forwardActions` computed property to `NewswireSurfaceModel` (uses `descriptorRecoverable` from Task 2 — for this task, stub it as `true` so Task 1 compiles, then Task 2 fills it in; OR land Tasks 1–3 as one model change and split only the tests):
```swift
/// The ordered forward actions the current wire state offers — pure over the
/// model's loaded state. `postsButNoFeature`/`featured` return `[]` because the
/// next action is the visible content itself (the open wire renders directly
/// below, already labeled "Open wire" — the redundant no-op button is gone).
public var forwardActions: [NewswireWireForwardAction] {
    switch wire {
    case .offlineStale:
        // Task 2 sets `descriptorRecoverable`. A known descriptor that is merely
        // offline → Try again; a community with no derivable descriptor
        // (nearby-joined / pending first sync) → a real forward path, never a
        // silent re-loop, verified path first (Task 3).
        return descriptorRecoverable ? [.retry] : [.rejoinWithLink, .syncWithPeer]
    case .emptyWire:
        return [.postFirstUpdate]
    case .postsButNoFeature, .featured:
        return []
    }
}
```
Rewrite `wireEmpty` to render buttons from an action list instead of a single `primary` tuple, dropping the dead-closure parameter entirely:
```swift
private func wireEmpty(
    id: String,
    title: String,
    message: String,
    actions: [NewswireWireForwardAction]
) -> some View {
    RiotCard {
        VStack(alignment: .leading, spacing: 10) {
            eyebrow(title)
            Text(message)
                .font(.riot(.body, size: 15, relativeTo: .callout))
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
            ForEach(actions, id: \.self) { action in
                Button(action.label) { perform(action) }
                    .buttonStyle(action.isNearbyPath ? .riotSecondary : .riotPrimary)
                    .frame(minHeight: 44)
                    .accessibilityIdentifier(action.accessibilityID)
            }
        }
    }
    .accessibilityIdentifier(id)
}
```
Update `wireSection` so `postsButNoFeature` no longer passes a button (only title + message + the `openWireCard` below), and `offlineStale`/`emptyWire` pass `actions: model.forwardActions`:
```swift
case .offlineStale:
    wireEmpty(
        id: model.wire.accessibilityID,
        title: model.offlineTitle,          // Task 3 selects pending-vs-offline copy
        message: model.offlineMessage,       // Task 3
        actions: model.forwardActions
    )
case .emptyWire:
    wireEmpty(
        id: model.wire.accessibilityID,
        title: NewswireWireCopy.emptyTitle,
        message: NewswireWireCopy.emptyMessage,
        actions: model.forwardActions
    )
case let .postsButNoFeature(openWire):
    VStack(alignment: .leading, spacing: 12) {
        wireEmpty(
            id: model.wire.accessibilityID,
            title: NewswireWireCopy.noFeatureTitle,
            message: NewswireWireCopy.noFeatureMessage,
            actions: []                       // the open wire below IS the next action
        )
        openWireCard(openWire)
    }
```
Add the `perform(_:)` handler + the `onSyncWithPeer`/`onRejoinWithLink` callbacks to `NewswireSurfaceView` (see Task 4 for the full signature); for Task 1 the map is:
```swift
private func perform(_ action: NewswireWireForwardAction) {
    switch action {
    case .retry: model.retry()               // Task 2
    case .postFirstUpdate: onPostUpdate()
    case .rejoinWithLink: onRejoinWithLink()  // Task 4 callback (Unit 1 sheet)
    case .syncWithPeer: onSyncWithPeer()      // Task 4 callback (→ Nearby)
    }
}
```
Add the `FixedProjector` test stub next to `ThrowingProjector` in `NewswireSurfaceTests.swift`, plus a trivial `StubAuthority` for the post-4b `authority:` seam (editor status is irrelevant to Unit 7's wire-state assertions — if 4b already left a `false`-returning authority stub in the file, reuse it instead of adding this):
```swift
/// Returns a fixed projection so a wire-state can be driven without a store.
private struct FixedProjector: NewswireProjecting {
    let projection: NewswireProjectionView
    init(_ projection: NewswireProjectionView) { self.projection = projection }
    func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
        projection
    }
}

/// Satisfies Unit 4b's `authority:` seam for wire-state tests that don't exercise
/// editor gating — always "not an editor", so it never colors a wire-state result.
private struct StubAuthority: NewswireEditorAuthorityChecking {
    func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool { false }
}
```

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** `apps/ios/Riot/NewswireEditorial.swift` + `apps/ios/RiotTests/NewswireSurfaceTests.swift`.

---

## Task 2: `offlineStale` re-derive — a forward path, never a silent loop

**Files:** Modify `apps/ios/Riot/NewswireEditorial.swift`; Test `apps/ios/RiotTests/NewswireSurfaceTests.swift`

Add the descriptor re-derive seam so `load()`/`retry()` pick up a descriptor that landed after construction, and set `descriptorRecoverable` so an unrecoverable community shows a forward path instead of re-looping.

- [ ] **Step 1: Failing test** (RED-then-green — the re-derive AND the no-silent-loop invariant):
```swift
func testOfflineStaleReDerivesADescriptorThatLandedInsteadOfLooping() {
    // Built with "" (the shell's pre-sync case), but the registry now HAS a
    // descriptor (a joined/switched community whose sync just landed). load()
    // must pick it up and project — not re-loop on the empty id.
    let post = projectedPost(id: "a1", headline: "Landed", treatment: .ordinary)
    let model = NewswireSurfaceModel(
        projector: FixedProjector(projection(openWire: [post], frontPage: [])),
        editor: ThrowingEditor(), authority: StubAuthority(),
        spaceDescriptorEntryID: "", communityName: "Riverside",
        myKeyHex: "aa".repeated(32),
        descriptorResolver: { "desc-that-just-synced" })
    model.load()
    guard case .postsButNoFeature = model.wire else {
        return XCTFail("a re-derived descriptor must project, not stay offlineStale")
    }
}

func testOfflineStaleWithNoDerivableDescriptorOffersAForwardPathNotASilentLoop() {
    // A nearby-joined community: it never gets a descriptorEntryId, so the
    // resolver yields nil. The state must offer real forward paths and MUST NOT
    // offer the silent .retry re-loop.
    let model = NewswireSurfaceModel(
        projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
        spaceDescriptorEntryID: "", communityName: "Riverside",
        myKeyHex: "aa".repeated(32),
        descriptorResolver: { nil })
    model.load()
    XCTAssertEqual(model.wire, .offlineStale)
    XCTAssertFalse(model.forwardActions.contains(.retry),
                   "no silent re-loop when there is nothing to re-derive")
    XCTAssertFalse(model.forwardActions.isEmpty, "offlineStale is never a dead end")
}

func testKnownDescriptorThatIsMerelyOfflineStillOffersRetry() {
    // A descriptor we DO have, but projection throws (transient offline). Retry
    // is the honest action here — reproject the id we already hold.
    let model = NewswireSurfaceModel(
        projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
        spaceDescriptorEntryID: "desc", communityName: "Riverside",
        myKeyHex: "aa".repeated(32))
    model.load()
    XCTAssertEqual(model.wire, .offlineStale)
    XCTAssertEqual(model.forwardActions, [.retry])
}
```

- [ ] **Step 2: Run → FAIL** (`descriptorResolver` param + `retry()` + `descriptorRecoverable` undefined).

- [ ] **Step 3: Implement** in `NewswireSurfaceModel`. Make `spaceDescriptorEntryID` a `var`, add the resolver + a stored recoverability flag, and re-derive in `load()`:
```swift
private var spaceDescriptorEntryID: String
/// Re-derives the community's descriptor id on demand (the shell wires this to
/// `RiotAppModel.rederivedNewswireDescriptorID()` — the same `listCommunities()`
/// derivation `reload()` uses). `nil` in tests/constructions that never need it.
private let descriptorResolver: (() -> String?)?
/// Set by `load()`: true when a descriptor id is in hand (retry can reproject),
/// false when none is derivable (a nearby-joined community — the wire must offer
/// a forward path, not the silent .retry re-loop). Drives `forwardActions`.
private var descriptorRecoverable = false
```
Append `descriptorResolver: (() -> String?)? = nil` to the **post-4b `init`** (after `initialDraftKind` — see "Cross-unit coupling"), and store it. Rewrite `load()` — **the re-derive is the FIRST statement, ahead of 4b's `isEditor` computation**, so the predicate sees the freshly re-derived id (one `load()` both projects the wire AND editor-gates correctly for a community whose descriptor just landed):
```swift
public func load() {
    // A descriptor may have landed since this model was built (a switched or
    // joined community whose registry row now carries one; the shell built this
    // model with "" before the sync). Picking it up here is what turns a silent
    // offlineStale re-loop into a real projection — and it must precede the
    // editor-status read below so the predicate answers off the re-derived id.
    if spaceDescriptorEntryID.isEmpty, let resolved = descriptorResolver?(), !resolved.isEmpty {
        spaceDescriptorEntryID = resolved
    }
    // 4b: editor status is core's descriptor answer, resolved once per load; an
    // unknown / not-yet-synced descriptor (or a closed profile) answers false.
    isEditor = (try? authority.newswireIsEditor(
        spaceDescriptorEntryID: spaceDescriptorEntryID, subjectID: myKeyHex)) ?? false

    guard !spaceDescriptorEntryID.isEmpty else {
        // No descriptor to project — the forward-path state (rejoin / sync),
        // never invented content, never a silent retry.
        wire = .offlineStale
        history = []
        descriptorRecoverable = false
        return
    }
    do {
        let projection = try projector.projectNewswire(
            spaceDescriptorEntryID: spaceDescriptorEntryID
        )
        wire = .from(projection)
        history = projection.editorialHistory.map(EditorialHistoryRow.init)
        descriptorRecoverable = true
    } catch {
        // We hold a descriptor id but it is transiently offline — retry can
        // reproject the id we already have.
        wire = .offlineStale
        history = []
        descriptorRecoverable = true
    }
}

/// The offlineStale "Try again" action: re-derive + reproject. A no-op if the
/// community still has no derivable descriptor (the view then shows the forward
/// paths, not this button).
public func retry() { load() }
```
`retry()` replaces the inline `{ model.load() }` closure via the Task 1 `perform(.retry)` path.

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** both files.

---

## Task 3: Honest pending-first-sync — verified path first, Nearby secondary

**Files:** Modify `apps/ios/Riot/NewswireEditorial.swift`; Test `apps/ios/RiotTests/NewswireSurfaceTests.swift`

Give the unrecoverable `offlineStale` state honest pending-sync copy and assert the forward-action ORDER: the verified-working `rejoinWithLink` leads, the red `syncWithPeer` (Nearby) is never the headline (design §3).

- [ ] **Step 1: Failing test.**
```swift
func testPendingFirstSyncLeadsWithVerifiedPathAndKeepsNearbySecondary() {
    let model = NewswireSurfaceModel(
        projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
        spaceDescriptorEntryID: "", communityName: "Riverside",
        myKeyHex: "aa".repeated(32),
        descriptorResolver: { nil })
    model.load()
    let actions = model.forwardActions
    XCTAssertEqual(actions.first, .rejoinWithLink, "the verified-working path is the headline")
    XCTAssertFalse(actions.first?.isNearbyPath ?? true, "the red Nearby path is never first")
    XCTAssertEqual(actions.last, .syncWithPeer, "Nearby is offered but secondary")
    XCTAssertEqual(actions, [.rejoinWithLink, .syncWithPeer])
}

func testPendingSyncCopyIsHonestAndDistinctFromTransientOfflineCopy() {
    // The pending-first-sync message explains WHY there is nothing yet and names
    // the forward paths; it is not the same string as the transient-offline copy.
    XCTAssertTrue(
        NewswireWireCopy.pendingSyncMessage.localizedCaseInsensitiveContains("sync")
        || NewswireWireCopy.pendingSyncMessage.localizedCaseInsensitiveContains("peer"))
    XCTAssertNotEqual(NewswireWireCopy.pendingSyncMessage, NewswireWireCopy.offlineMessage)
    XCTAssertNotEqual(NewswireWireCopy.pendingSyncTitle, NewswireWireCopy.offlineTitle)
}

func testOfflineStaleCopySelectionFollowsRecoverability() {
    // Unrecoverable (pending first sync) → pending copy; recoverable (transient
    // offline of a known descriptor) → the existing offline copy.
    let pending = NewswireSurfaceModel(
        projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
        spaceDescriptorEntryID: "", communityName: "R",
        myKeyHex: "aa".repeated(32), descriptorResolver: { nil })
    pending.load()
    XCTAssertEqual(pending.offlineTitle, NewswireWireCopy.pendingSyncTitle)
    XCTAssertEqual(pending.offlineMessage, NewswireWireCopy.pendingSyncMessage)

    let offline = NewswireSurfaceModel(
        projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
        spaceDescriptorEntryID: "desc", communityName: "R",
        myKeyHex: "aa".repeated(32))
    offline.load()
    XCTAssertEqual(offline.offlineTitle, NewswireWireCopy.offlineTitle)
    XCTAssertEqual(offline.offlineMessage, NewswireWireCopy.offlineMessage)
}
```

- [ ] **Step 2: Run → FAIL** (`pendingSyncTitle`/`pendingSyncMessage`, `offlineTitle`/`offlineMessage` model accessors undefined).

- [ ] **Step 3: Implement.** Add the pending-sync copy to `NewswireWireCopy` (design §3 wording — honest, names the forward paths, no fabricated content):
```swift
public static let pendingSyncTitle = "Waiting for the first sync"
public static let pendingSyncMessage =
    "You've joined this community, but no posts have arrived yet. They appear once a peer or seed connects. Rejoin with a link, or sync with a peer nearby."
```
The ORDER in `forwardActions` (Task 1) already leads with `rejoinWithLink` — confirm the unrecoverable branch is `[.rejoinWithLink, .syncWithPeer]` (verified primary, Nearby secondary), and that `.riotPrimary`/`.riotSecondary` styling in `wireEmpty` (Task 1) reinforces it (`isNearbyPath` → `.riotSecondary`). Add the copy-selection accessors so the view draws pending-vs-offline copy off recoverability:
```swift
/// The offlineStale title: the honest pending-first-sync headline when no
/// descriptor is derivable, the transient-offline headline when one is in hand.
public var offlineTitle: String {
    descriptorRecoverable ? NewswireWireCopy.offlineTitle : NewswireWireCopy.pendingSyncTitle
}
/// The offlineStale message, matched to the title.
public var offlineMessage: String {
    descriptorRecoverable ? NewswireWireCopy.offlineMessage : NewswireWireCopy.pendingSyncMessage
}
```
The Task 1 `wireSection` already reads `model.offlineTitle` / `model.offlineMessage` for the `offlineStale` branch.

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** both files.

---

## Task 4: Wire the shell — descriptor resolver + Nearby / rejoin callbacks

**Files:** Modify `apps/ios/Riot/ConferenceShellView.swift`, `apps/ios/Riot/AppModel.swift`

Feed the model the on-demand re-derive seam and give the view its two navigation callbacks. (This task carries the flagged Unit 1 dependency: `onRejoinWithLink` presents `JoinByReferenceSheet`.)

- [ ] **Step 1: Failing test** — an `AppModelTests` (or the nearest existing AppModel suite) case asserting `rederivedNewswireDescriptorID()` returns the active community's descriptor from the registry and `nil` when it has none, so the resolver is honest:
```swift
func testRederivedNewswireDescriptorReadsTheRegistryForTheActiveCommunity() throws {
    // Open a profile, create a community (which persists a descriptorEntryId on
    // its CommunityRow), and assert the on-demand re-derive returns it; a profile
    // with no selected space returns nil (drives the forward-path state).
    // Mirror the existing AppModel/create test harness.
}
```
(If no AppModel unit suite exercises `reload()`'s derivation directly, assert through the existing community-create flow that `model.rederivedNewswireDescriptorID()` equals `model.newswireDescriptorEntryID` after a create, and is `nil` after `leaveCommunity()`.)

- [ ] **Step 2: Run → FAIL** (`rederivedNewswireDescriptorID` undefined).

- [ ] **Step 3: Implement.**
  - **`AppModel.swift`** — add the on-demand re-derive (the same `listCommunities()` lookup `reload()` at `:501-503` performs, callable without a full reload so the offlineStale "Try again" picks up a descriptor that just landed):
```swift
/// Re-derives the active community's newswire descriptor id from the registry
/// on demand — the offlineStale "Try again" path. Returns nil when the community
/// still carries none (a nearby-joined community), which is what puts the wire
/// into its forward-path (rejoin / sync) state instead of a silent re-loop.
/// Publishes the fresh value so the shell and Home agree.
@discardableResult
public func rederivedNewswireDescriptorID() -> String? {
    guard let repository, let namespaceID = space?.namespaceID else {
        newswireDescriptorEntryID = nil
        return nil
    }
    let derived = (try? repository.listCommunities())?
        .first { $0.namespaceId == namespaceID }?
        .descriptorEntryId
    newswireDescriptorEntryID = derived
    return derived
}
```
  (Optional DRY: have `reload()` call this in place of its inline `:500-506` block — behaviorally identical; keep as a follow-up if it widens the diff.)
  - **`ConferenceShellView.swift`** — pass the resolver when building the model (`:305-312`), capturing the `RiotAppModel` weakly to avoid a cycle. **Post-4b: `authority:` is already present and `roster:`/`community.editorialRoster` are gone (see "Cross-unit coupling") — add ONLY `descriptorResolver:`, never re-introduce `roster:`:**
```swift
let authority: NewswireEditorAuthorityChecking = model.profileRepository ?? UnavailableEditor()  // from 4b
_newswire = StateObject(wrappedValue: NewswireSurfaceModel(
    projector: wireProjector,
    editor: editor,
    authority: authority,                          // 4b (replaced roster:)
    spaceDescriptorEntryID: community.newswireDescriptorEntryID ?? "",
    communityName: community.name,
    myKeyHex: me.id,
    descriptorResolver: { [weak model] in model?.rederivedNewswireDescriptorID() }  // Unit 7
))
```
  - **`NewswireSurfaceView`** — extend the initializer with the two navigation callbacks (defaults `{}` so existing/preview call sites compile), and add the `perform(_:)` map from Task 1:
```swift
public init(
    model: NewswireSurfaceModel,
    onPostUpdate: @escaping () -> Void = {},
    onSyncWithPeer: @escaping () -> Void = {},
    onRejoinWithLink: @escaping () -> Void = {}
) {
    self.model = model
    self.onPostUpdate = onPostUpdate
    self.onSyncWithPeer = onSyncWithPeer
    self.onRejoinWithLink = onRejoinWithLink
}
```
  - **`HomeRouteView`** — wire the callbacks; `onSyncWithPeer` → the real existing Nearby screen, `onRejoinWithLink` → present Unit 1's `JoinByReferenceSheet`:
```swift
@State private var showRejoinSheet = false
// …
NewswireSurfaceView(
    model: newswire,
    onSyncWithPeer: { model.select(.nearby) },
    onRejoinWithLink: { showRejoinSheet = true }
)
// …in HomeRouteView.body, attach:
.sheet(isPresented: $showRejoinSheet) {
    JoinByReferenceSheet(model: model, onClose: { showRejoinSheet = false })
}
```
  **Unit 1 dependency:** `JoinByReferenceSheet` is Unit 1's file. If Unit 1 has landed, this compiles as written and Unit 7 depends on it. If Unit 7 must land first, replace the `.sheet` body with the Launch-side join affordance Unit 1 will add, and — until it exists — set `onRejoinWithLink` to `{ model.select(.nearby) }` as an interim reachable action (NEVER `{}`), leaving a `// TODO(Unit 1): present JoinByReferenceSheet` marker. Do not ship `rejoinWithLink` as a dead closure.

- [ ] **Step 4: Run → PASS** + existing `NewswireSurfaceTests` stay green (`testMissingDescriptorIsOfflineStaleNeverAFabricatedEmptyWire` / `testProjectionFailureIsOfflineStaleNeverARawError` still hold — the `descriptorResolver` defaults to `nil`, so a missing descriptor stays `offlineStale`). **Step 5: Commit** `apps/ios/Riot/ConferenceShellView.swift` + `apps/ios/Riot/AppModel.swift`.

---

## Task 5: Build + full test both platforms
- [ ] iOS: `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'` — new Unit 7 tests green, full `NewswireSurfaceTests` green, only the known-red Bonjour two-peer test still red.
- [ ] iOS app **BUILD SUCCEEDED** (`xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot -destination '…'`) and macOS app **BUILD SUCCEEDED** — Unit 7 adds no AVFoundation and no platform-only API (`model.select(.nearby)` exists on both; `JoinByReferenceSheet` — once Unit 1 lands — builds on macOS as its paste-only variant), so **no `#if os(iOS)` guard is needed**. Confirm no new file was added → **no `project.pbxproj` edit on either project**.
- [ ] Commit any fixups.

---

## Self-Review

- **§7 dead-end coverage (three dead-ends → tasks):**
  - No-op "Open wire" button (`:652` empty `{}`) → **Task 1**: removed; `postsButNoFeature.forwardActions == []`; the `openWireCard` (eyebrow already "Open wire") is the reachable next action; dead `noFeatureLink` constant deleted. ✅
  - `offlineStale` "Try again" silent loop (`:637` re-projecting the empty id) → **Task 2**: `load()` re-derives via the resolver; a landed descriptor projects; an unrecoverable community drops `.retry` for `[.rejoinWithLink, .syncWithPeer]` — a forward path, never a re-loop. ✅
  - Post-join "pending first sync" (§3) → **Task 3**: honest `pendingSyncTitle`/`pendingSyncMessage`; `forwardActions` order leads with the verified `rejoinWithLink` and keeps `syncWithPeer` (Nearby) last + `.riotSecondary`. ✅
  - Explicitly NOT the chooser Create/Find-nearby no-ops — those are Unit 1 (§7 last bullet); untouched here. ✅
- **Anti-dead-end ASSERTION (design §2 invariant 2 — every terminal state offers a reachable next action or an honest waiting state), one per fix, all testable:**
  - `testPostsButNoFeatureOffersNoDeadButtonTheOpenWireIsTheNextAction` — no no-op button; content is the action.
  - `testOfflineStaleWithNoDerivableDescriptorOffersAForwardPathNotASilentLoop` — `!forwardActions.contains(.retry)` and `!forwardActions.isEmpty`.
  - `testPendingFirstSyncLeadsWithVerifiedPathAndKeepsNearbySecondary` — `first == .rejoinWithLink`, `first` is not the Nearby path, `last == .syncWithPeer`.
  - `testEveryTerminalWireStateHasAReachableNextActionAndNoDeadNoOp` — the cross-state invariant.
- **Placeholder scan:** the one API to confirm on implement is the Task 4 AppModel test harness — whether an existing AppModel suite drives `reload()`'s derivation directly, or the assertion must go through the create flow (`rederivedNewswireDescriptorID() == newswireDescriptorEntryID` after create, `nil` after `leaveCommunity()`); flagged in Task 4 Step 1. No fabricated content anywhere — the pending-sync copy names paths, never invents a title/post. The out-of-scope `emptyWire` "Post the first update" `{}` no-op in the shell is preserved (routed through the existing `onPostUpdate` seam), noted honestly, not silently "fixed."
- **Cross-unit dependency (flagged as requested):** YES — the `rejoinWithLink` forward path presents **Unit 1's `JoinByReferenceSheet`** (does not exist yet). **Unit 7's model + all Unit 7 tests are independent of Unit 1** (they assert the pure `forwardActions` data and copy, never the sheet). Only **Task 4's shell wiring of `onRejoinWithLink`** depends on Unit 1. **Ordering: land Unit 1 before Unit 7.** If Unit 7 lands first, wire `onRejoinWithLink` to an interim reachable action (`model.select(.nearby)`) with a `TODO(Unit 1)`, never a dead `{}`, and swap in `JoinByReferenceSheet` when Unit 1 lands. `syncWithPeer` → Nearby has no Unit dependency (the screen exists; navigation is a real action even though two-peer sync is red on main — no sync success is claimed).
- **Type/behavior consistency:** `NewswireWireForwardAction` (T1) drives `forwardActions` (T1/T2/T3), rendered by `wireEmpty` + `perform(_:)` (T1) with callbacks from `NewswireSurfaceView.init` (T4); `descriptorResolver`/`retry()`/`descriptorRecoverable` (T2) feed `forwardActions` recoverability and `offlineTitle`/`offlineMessage` (T3); `rederivedNewswireDescriptorID()` (T4) is the resolver's backing. Existing `offlineStale` degradation tests (`:241`, `:251`) stay green because `descriptorResolver` defaults to `nil`.
- **Cross-unit coupling (gate r1):** Unit 7 collides with **Unit 4b** on `NewswireSurfaceModel.init`, the shell construction, and `NewswireSurfaceTests.swift`. **Land 4b before 7**; the merged `init` carries 4b's `authority:` AND Unit 7's `descriptorResolver:` (no `roster:`); every construction in this plan (shell + tests) uses the post-4b signature; `load()` runs the descriptor re-derive AHEAD of 4b's `isEditor` read. Claim `NewswireEditorial.swift` / `ConferenceShellView.swift` / `AppModel.swift` / `NewswireSurfaceTests.swift` in COLLABORATION.md — **4b and 7 must not run concurrently** on these files. Separately, Unit 7's `rejoinWithLink` wiring depends on **Unit 1** (`JoinByReferenceSheet`); combined order **4b → 1 → 7**.
- **Dependency order (within Unit 7):** T1 (action data + Open-wire removal) → T2 (re-derive seam) → T3 (pending copy + order) → T4 (shell wiring, the Unit 1 touch point) → T5 (build). All model/copy changes land in `NewswireEditorial.swift`; the shell/AppModel edits are isolated to T4. No new Swift file → no pbxproj edit — but the shared-source COLLABORATION claim above IS required (the 4b collision), unlike a file-adding unit.
