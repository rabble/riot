# iOS Surface — Unit 3: Read alerts (`AlertsListView` + `AlertDetailSheet`, per-community) — Implementation Plan


**Plan-review gate: PASSED** (Feasibility + Scope + Completeness, 2026-07-18).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Give a reader a real screen for the signed alerts their active community carries. Today `AlertDetail` is a value struct nobody renders and the only on-screen "alert" surface is a dead `LabeledContent("Signed alerts", …)` count on the Nearby route that leads nowhere. This unit builds ONE **Alerts card on Home** (per-active-community) → `AlertsListView` (rows: headline + **core-verified** signer + freshness, organizer-first) → tap → a NEW **`AlertDetailSheet`** that renders the existing `AlertDetail` value model. Pure-Swift, no new FFI.

**Architecture:** A pure `AlertsListState` value type maps the app's already-active-community-scoped `[RiotEntry]` into organizer-first rows (`AlertRow`) — unit-testable with no store, no FFI, mirroring `PeopleSurfaceState`. `AlertRow` derives its organizer flag from the **core-verified coordinate rule** (`signerID == namespaceID`, the same namespace-coordinate rule the People projector uses), never from a self-claimed name. `AlertsListView` renders the state and presents `AlertDetailSheet`, which renders `AlertDetail(entry:)` (headline + AI flag + validity summary, with the 64-hex ids behind the existing closed **Technical details** `DisclosureGroup` pattern). The dead Nearby `LabeledContent` is demoted to a plain diagnostic count so Home stays the single entry point (anti-dead-end invariant #1: no divergent half-built surfaces).

**Tech stack:** Swift 6 / SwiftUI, XCTest. No AVFoundation, no camera, no new FFI. Design: `docs/superpowers/specs/2026-07-18-ios-surface-built-capabilities-design.md` §3 "Read alerts" + §8 Unit 3.

**Shared-checkout:** both `apps/ios/Riot.xcodeproj/project.pbxproj` + `apps/macos/Riot.xcodeproj/project.pbxproj` are hand-edited and serialize all Swift-file additions — **claim them in COLLABORATION.md before editing; no unit that adds Swift files runs while either pbxproj is dirty**. Pathspec commits; absolute `git`/`grep`. These are plain SwiftUI views (no camera) — macOS builds them unguarded, no `#if os(iOS)` needed.

---

## Ground truth (verified)

- **`AlertDetail` is a VALUE STRUCT, not a view** (`apps/ios/Riot/AppModel.swift:984-1022`): `public struct AlertDetail: Equatable, Sendable` with `public init(entry: RiotEntry)`. Fields: `headline: String`, `aiAssisted: Bool`, `summary: [Row]` (Created / Valid from / Expires — shown immediately), `technical: [Row]` (Entry / Namespace / **Signer** — shown only under disclosure). `Row { label: String; value: String }`. `static let technicalDisclosureTitle = "Technical details"`. `static func timestamp(_:) -> String`. This unit **builds `AlertDetailSheet` to render it** — the struct already carries every string the sheet shows; the sheet adds no logic.
- **`RiotEntry`** (`apps/ios/Riot/Core/ProfileRepository.swift:30-40`): `entryID`, `namespaceID`, `signerID`, `headline: String`, `createdAt: UInt64`, `validFrom: UInt64?`, `expiresAt: UInt64`, `aiAssisted: Bool`. `id == entryID`. Codable/Equatable/Identifiable/Sendable.
- **`model.entries` is ALREADY active-community-scoped — verified at the FFI layer.** `RiotAppModel.entries: [RiotEntry]` (`AppModel.swift:181`) is filled from `repository.currentEntries()` (`AppModel.swift:380`, `:507`), which maps `profile.listCurrentEntries()` (`ProfileRepository.swift:486-489`). The Rust `list_current_entries` (`crates/riot-ffi/src/mobile_state.rs:874-944`) scopes the whole listing to `active_namespace = parse_entry_id(profile.space.namespace_id)` — its own comment (`:892-897`): *"Scope the whole listing to the ACTIVE namespace. One store holds every held community's entries (Unit 3), so an unscoped scan would surface another community's alert."* `switchCommunity`/`reproject_active` (`mobile_state.rs:597`) reprojects the active namespace's cache on switch. **Honest answer to the design's scoping question: the set is already the active community's — no cross-community filter is required for correctness.** The design §3 note ("`model.entries` is currently global … add a filter if the set is genuinely cross-community") described the *pre-multi-community* state; the registry work closed it. This unit still adds a **defense-in-depth namespace filter in Swift** (drop any row whose `namespaceID != activeNamespaceID`) so a future FFI regression cannot leak a foreign community's alert onto this card — belt-and-suspenders, not the primary guarantee. A test asserts the filter drops a planted foreign row.
- **Signer is core-verified, not self-claimed.** `CurrentEntry.signer_id` (`mobile_api.rs:112`) is set in `current_entry_from_signed` (`mobile_state.rs:1558-1564`) from `public_entry_identity(&signed.signed.entry_bytes).signer_id` — the cryptographic identity of the signed entry bytes. `namespace_id` comes from the same verified identity. So both `RiotEntry.signerID` and `.namespaceID` are core-verified hex.
- **Organizer-marker rule (core coordinate, not a name).** A space organizer's author subspace **equals** the namespace id by construction (`ProfileRepository.swift:1087` comment: *"the recognized organizer marked by the namespace coordinate"*; matches Unit 4a ground truth: `descriptor.namespace_id == founder id`). Therefore **`signerID.lowercased() == namespaceID.lowercased()` ⟺ organizer-signed** — computed from two core-verified fields, never from a display name. This is the identical coordinate rule the People projector uses to set `NewswireContributor.isOrganizer` (`PeopleSurfaceTests.swift:120-133`: *"the organizer flag comes ONLY from the core's coordinate rule, never from a name"*).
- **Signer display name is optional + secondary.** `RiotAppModel.rendered(for signerID: String) -> String?` (`AppModel.swift:609-611`) maps a lowercased signer id to a self-claimed display name; `postedBy(_:)` (`:614`) wraps it. The row shows the core-verified short signer tag + organizer badge as the identity; the rendered name is secondary decoration only (never drives ordering or the badge).
- **The dead surface to kill** (`ConferenceShellView.swift:1003`): `LabeledContent("Signed alerts", value: "\(model.entries.count)")`, inside the Nearby route's "On this device" diagnostic card (`:996-1006`). It is the app's only "alert" surface and is a nav dead-end. Home becomes the single tappable entry point; this label is demoted to an honest, non-navigational device diagnostic.
- **Home body** (`HomeRouteView`, `ConferenceShellView.swift:605-661`): `@ObservedObject var model: RiotAppModel`; `body` is `ScrollView { VStack(alignment:.leading, spacing:16) { shortcutsCard; NewswireSurfaceView(model:newswire); PostUpdateView(model:composer) } .padding(20) }`. Has `model.entries`, `model.space?.namespaceID`, `model.rendered(for:)`, `RiotCard {}`, and a private `eyebrow(_:)` helper (`:654`). The Alerts card slots into this VStack.
- **Reusable patterns:** `DisclosureGroup(isExpanded:)` technical-details, closed by default, monospace rows, `.accessibilityIdentifier("catalog-technical-details")` — `CatalogFailureView` (`ConferenceShellView.swift:204-238`). Sheet chrome — `YourProfileSheet` (`ConferenceShellView.swift:696`): `@ObservedObject var model`, `let onClose`, `ScrollView{VStack}`, `.riotHeader(eyebrow:_ :)`, `.toolbar` Done. State-machine surface model + value state + strings enum — `PeopleSurfaceModel`/`PeopleSurfaceState`/`PeopleStrings` (`PeopleSurfaceTests.swift`).
- **Freshness precedent:** `CommunityRelativeTime.syncFreshness(_ unixSeconds:UInt64?, now:Date) -> String` (`CommunityChooser.swift:19-58`) — a pure `UInt64?`→human-phrase function, no raw timestamps. Mirror it for alert freshness.
- **Test harness:** hostless XCTest, `@testable import RiotKit`, pure value-model/state tested directly with fixtures — no store, no FFI (`PeopleSurfaceTests.swift:1-201`, `CommunityChooserTests`).
- **pbxproj registration convention** (both hand-authored, fixed `A0…`/`F0…` ids). Closest analog = `CommunityChooser.swift` (a view file + a RiotKit-tested model): iOS uses one `PBXFileReference` + `PBXBuildFile` for the source and one pair for the test — file ref `A0F000000000000000000010`, build file `A0F000000000000000000011` in the Riot group `A00000000000000000000002.children` and sources phase `A00000000000000000000030.files`; test ref `A0F000000000000000000020`, build file `A0F000000000000000000021` in the RiotTests group `A00000000000000000000003.children` and test sources phase `A00000000000000000000031.files` (`apps/ios/Riot.xcodeproj/project.pbxproj:8,11,142-145,223-224`). macOS mirrors with the `F0…` prefix and `path = ../ios/Riot/…` (`apps/macos/Riot.xcodeproj/project.pbxproj:147`). Highest source id block currently in use tops out around `A0F0…` / `A000…0F02`; this unit claims the next free block `A0100000000000000000xxxx` (iOS) / `F010…` (macOS) — implementer confirms freedom with a grep before writing.

---

## Task 1: `AlertsListState` — per-community rows + core-verified signer (pure, no FFI)

**Files:** Create `apps/ios/Riot/AlertsListView.swift`; Test `apps/ios/RiotTests/AlertsSurfaceTests.swift`

- [ ] **Step 1: Failing test.** Pin the four contracts: active-community scoping (defense-in-depth filter), organizer-first ordering from the coordinate rule, core-verified signer identity (never a name), and an actionable empty state.
```swift
import XCTest
@testable import RiotKit

/// Unit 3 — the Alerts surface, tested in isolation (no store, no FFI). Contracts:
/// the card shows ONLY the active community's alerts; the organizer flag + the
/// organizer-first order come ONLY from the core coordinate rule
/// (signerID == namespaceID), never from a self-claimed name; the signer shown is
/// the core-verified id, and an empty community shows an actionable empty state.
final class AlertsSurfaceTests: XCTestCase {
    private static let activeNS = String(repeating: "a", count: 64)
    private static let otherNS  = String(repeating: "b", count: 64)

    private static func entry(
        _ headline: String,
        entryID: String = String(repeating: "e", count: 64),
        namespaceID: String = activeNS,
        signerID: String,
        createdAt: UInt64 = 100,
        expiresAt: UInt64 = 1_000,
        aiAssisted: Bool = false
    ) -> RiotEntry {
        RiotEntry(entryID: entryID, namespaceID: namespaceID, signerID: signerID,
                  headline: headline, createdAt: createdAt, validFrom: nil,
                  expiresAt: expiresAt, aiAssisted: aiAssisted)
    }

    // MARK: - Per-community scoping (defense in depth)

    func testForeignCommunityAlertsAreFilteredOut() {
        // model.entries is already active-scoped at the FFI, but the Swift filter is
        // the belt-and-suspenders guarantee: a planted foreign-namespace row never shows.
        let mine    = Self.entry("Road closed", signerID: Self.activeNS)          // organizer of active
        let foreign = Self.entry("Not mine", namespaceID: Self.otherNS, signerID: Self.otherNS)
        guard case let .populated(rows) = AlertsListState.from([mine, foreign], activeNamespaceID: Self.activeNS) else {
            return XCTFail("expected populated")
        }
        XCTAssertEqual(rows.map(\.headline), ["Road closed"])
        XCTAssertFalse(rows.contains { $0.namespaceID == Self.otherNS })
    }

    // MARK: - Organizer flag + order from the coordinate, never a name

    func testOrganizerFlagComesFromCoordinateNotName() {
        // signerID == namespaceID ⟺ organizer. A member who NAMES themselves the
        // organizer's hex in the headline is still a member.
        let organizer = Self.entry("A", signerID: Self.activeNS)                       // subspace == namespace
        let member    = Self.entry("B", signerID: String(repeating: "c", count: 64))   // subspace != namespace
        let orgRow = AlertRow(organizer, activeNamespaceID: Self.activeNS)
        let memRow = AlertRow(member, activeNamespaceID: Self.activeNS)
        XCTAssertTrue(orgRow.isOrganizer)
        XCTAssertFalse(memRow.isOrganizer)
    }

    func testOrganizerFirstThenNewestOrdering() {
        let orgOld  = Self.entry("org-old",  entryID: String(repeating: "1", count: 64), signerID: Self.activeNS, createdAt: 10)
        let memNew  = Self.entry("mem-new",  entryID: String(repeating: "2", count: 64), signerID: String(repeating: "c", count: 64), createdAt: 50)
        let orgNew  = Self.entry("org-new",  entryID: String(repeating: "3", count: 64), signerID: Self.activeNS, createdAt: 40)
        guard case let .populated(rows) = AlertsListState.from([memNew, orgOld, orgNew], activeNamespaceID: Self.activeNS) else {
            return XCTFail("expected populated")
        }
        // Organizers first (newest organizer before older organizer), then members newest-first.
        XCTAssertEqual(rows.map(\.headline), ["org-new", "org-old", "mem-new"])
        XCTAssertEqual(rows.map(\.isOrganizer), [true, true, false])
    }

    // MARK: - Core-verified signer identity, never the raw hex as the display

    func testRowCarriesVerifiedSignerTagAndPlainHeadline() {
        let e = Self.entry("**not** a link", signerID: Self.activeNS)
        let row = AlertRow(e, activeNamespaceID: Self.activeNS)
        // Short signer tag is the core-verified id (first 8 hex), not the full id.
        XCTAssertEqual(row.signerTag, String(Self.activeNS.prefix(8)))
        XCTAssertEqual(row.signerID, Self.activeNS)      // full id retained for detail/pinning only
        // Headline is carried verbatim (rendered as plain Text by the view — no markdown auto-link).
        XCTAssertEqual(row.headline, "**not** a link")
    }

    // MARK: - Actionable / benign empty state

    func testNoAlertsIsABenignEmptyStateNotABlankList() {
        guard case let .empty(empty) = AlertsListState.from([], activeNamespaceID: Self.activeNS) else {
            return XCTFail("an empty community must show the empty state, not .populated([])")
        }
        XCTAssertEqual(empty.title, AlertsStrings.emptyTitle)
        XCTAssertFalse(empty.message.isEmpty)
    }

    // MARK: - Freshness is a human phrase, never a raw epoch

    func testFreshnessDescribesExpiryWithoutRawTimestamps() {
        let now = Date(timeIntervalSince1970: 500)
        let live = Self.entry("live", signerID: Self.activeNS, createdAt: 100, expiresAt: 1_000)
        let dead = Self.entry("dead", signerID: Self.activeNS, createdAt: 100, expiresAt: 200)
        XCTAssertFalse(AlertRelativeTime.freshness(live, now: now).contains("1000"))
        XCTAssertEqual(AlertRelativeTime.freshness(dead, now: now), AlertsStrings.expired)
    }
}
```

- [ ] **Step 2: Run → FAIL** (`AlertsListState`/`AlertRow`/`AlertsStrings`/`AlertRelativeTime` undefined).
`xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/AlertsSurfaceTests`

- [ ] **Step 3: Implement** the pure surface types in `AlertsListView.swift` (the view itself lands in Step 3b/Task 3; state + strings first):
```swift
import SwiftUI

public enum AlertsStrings {
    public static let title = "Alerts"
    public static let organizerBadge = "Organizer"
    public static let emptyTitle = "No alerts yet"
    public static let emptyMessage = "Signed alerts from this community will appear here."
    public static let expired = "Expired"
    public static let active = "Active"
    public static func expires(inSeconds: Int64) -> String {
        // Human phrase, never a raw epoch. Coarse buckets are enough for a board row.
        let mins = inSeconds / 60
        if mins < 60 { return "Expires in \(max(mins, 1))m" }
        let hours = mins / 60
        if hours < 24 { return "Expires in \(hours)h" }
        return "Expires in \(hours / 24)d"
    }
}

/// Freshness as a human phrase derived from the alert's validity window — a pure
/// function of the entry + now, mirroring `CommunityRelativeTime.syncFreshness`.
public enum AlertRelativeTime {
    public static func freshness(_ entry: RiotEntry, now: Date = Date()) -> String {
        let nowSecs = Int64(now.timeIntervalSince1970)
        let remaining = Int64(entry.expiresAt) - nowSecs
        if remaining <= 0 { return AlertsStrings.expired }
        return AlertsStrings.expires(inSeconds: remaining)
    }
}

/// One alert row. `isOrganizer` and the ordering come ONLY from the core-verified
/// coordinate rule (signer subspace == namespace id); the display name is never
/// consulted for either. The full `signerID`/`entry` are retained for the detail
/// sheet and pinning, never as the row's display string.
public struct AlertRow: Equatable, Identifiable, Sendable {
    public let entry: RiotEntry
    public var id: String { entry.entryID }
    public var headline: String { entry.headline }
    public var namespaceID: String { entry.namespaceID }
    public var signerID: String { entry.signerID }
    public var aiAssisted: Bool { entry.aiAssisted }
    public var signerTag: String { String(entry.signerID.prefix(8)) }
    public let isOrganizer: Bool
    public let freshness: String

    public init(_ entry: RiotEntry, activeNamespaceID: String, now: Date = Date()) {
        self.entry = entry
        // Coordinate rule: an organizer signs with the author subspace that equals
        // the space namespace id (both fields are core-verified identity).
        self.isOrganizer = entry.signerID.lowercased() == entry.namespaceID.lowercased()
        self.freshness = AlertRelativeTime.freshness(entry, now: now)
    }
}

public struct AlertsEmpty: Equatable, Sendable {
    public let title: String
    public let message: String
    public static let noAlerts = AlertsEmpty(title: AlertsStrings.emptyTitle,
                                             message: AlertsStrings.emptyMessage)
}

public enum AlertsListState: Equatable, Sendable {
    case empty(AlertsEmpty)
    case populated([AlertRow])

    /// Maps the app's (already active-scoped) entries into organizer-first rows.
    /// The `namespaceID == activeNamespaceID` filter is defense in depth: the FFI
    /// `list_current_entries` already scopes to the active namespace, but a Swift
    /// filter guarantees a future FFI regression can never leak a foreign alert.
    public static func from(_ entries: [RiotEntry], activeNamespaceID: String, now: Date = Date()) -> AlertsListState {
        let scoped = entries.filter { $0.namespaceID.lowercased() == activeNamespaceID.lowercased() }
        guard !scoped.isEmpty else { return .empty(.noAlerts) }
        let rows = scoped
            .map { AlertRow($0, activeNamespaceID: activeNamespaceID, now: now) }
            .sorted { lhs, rhs in
                if lhs.isOrganizer != rhs.isOrganizer { return lhs.isOrganizer } // organizers first
                if lhs.entry.createdAt != rhs.entry.createdAt {
                    return lhs.entry.createdAt > rhs.entry.createdAt              // then newest first
                }
                return lhs.entry.entryID < rhs.entry.entryID                     // stable tiebreak
            }
        return .populated(rows)
    }
}
```

- [ ] **Step 4: Run → PASS** (all 6 tests). **Step 5: Commit** `AlertsListView.swift` + `AlertsSurfaceTests.swift` (pbxproj registration in Task 4).

---

## Task 2: `AlertDetailSheet` — render the `AlertDetail` value model

**Files:** Modify `apps/ios/Riot/AlertsListView.swift` (add the sheet view); Test `apps/ios/RiotTests/AlertsSurfaceTests.swift` (extend)

- [ ] **Step 1: Failing test.** `AlertDetail` (the value the sheet renders) already exists — pin the mapping the sheet depends on + the anti-injection guarantee, and add a helper the view uses so the disclosure default is testable without rendering SwiftUI.
```swift
extension AlertsSurfaceTests {
    func testAlertDetailModelDrivesTheSheetContent() {
        let e = Self.entry("Bridge out on 5th", signerID: Self.activeNS, aiAssisted: true)
        let detail = AlertDetail(entry: e)
        XCTAssertEqual(detail.headline, "Bridge out on 5th")
        XCTAssertTrue(detail.aiAssisted)
        // Summary is the act-on-it window; the 64-hex ids live only under technical.
        XCTAssertTrue(detail.summary.contains { $0.label == "Expires" })
        XCTAssertTrue(detail.technical.contains { $0.label == "Signer" && $0.value == Self.activeNS })
        XCTAssertFalse(detail.summary.contains { $0.value == Self.activeNS }, "full ids never lead the sheet")
    }

    func testSheetTechnicalDisclosureStartsClosedByContract() {
        // The sheet binds its DisclosureGroup to this default; a full id must never
        // be visible until a person opts in (navigation accessibility contract).
        XCTAssertFalse(AlertDetailSheet.technicalStartsExpanded)
        XCTAssertEqual(AlertDetail.technicalDisclosureTitle, "Technical details")
    }

    func testHeadlineIsCarriedVerbatimForPlainTextRendering() {
        // Anti-injection: a markdown-looking headline is preserved literally; the
        // view renders it as plain Text (verbatim:), never AttributedString auto-link.
        let e = Self.entry("[tap here](http://evil.example)", signerID: Self.activeNS)
        XCTAssertEqual(AlertDetail(entry: e).headline, "[tap here](http://evil.example)")
        XCTAssertEqual(AlertRow(e, activeNamespaceID: Self.activeNS).headline, "[tap here](http://evil.example)")
    }
}
```

- [ ] **Step 2: Run → FAIL** (`AlertDetailSheet.technicalStartsExpanded` undefined).

- [ ] **Step 3: Implement `AlertDetailSheet`** in `AlertsListView.swift`, mirroring `YourProfileSheet` chrome + the `CatalogFailureView` `DisclosureGroup`:
```swift
public struct AlertDetailSheet: View {
    /// The disclosure default, exposed for the contract test (full ids stay hidden until opt-in).
    public static let technicalStartsExpanded = false

    public let detail: AlertDetail
    public let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var showingTechnical = AlertDetailSheet.technicalStartsExpanded

    public init(detail: AlertDetail, onClose: @escaping () -> Void) {
        self.detail = detail
        self.onClose = onClose
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // Plain Text(verbatim:) — never markdown/AttributedString auto-link (anti-injection).
                Text(verbatim: detail.headline)
                    .font(.riot(.body, size: 20, relativeTo: .title3))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityAddTraits(.isHeader)
                if detail.aiAssisted {
                    Text("AI-assisted")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        .accessibilityIdentifier("alert-detail-ai-assisted")
                }
                ForEach(detail.summary, id: \.label) { row in
                    LabeledContent(row.label, value: row.value)
                }
                DisclosureGroup(isExpanded: $showingTechnical) {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(detail.technical, id: \.label) { row in
                            VStack(alignment: .leading, spacing: 2) {
                                Text(row.label).font(.riot(.mono, size: 11, relativeTo: .caption2))
                                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                                Text(verbatim: row.value).font(.riot(.mono, size: 12, relativeTo: .caption))
                                    .textSelection(.enabled)
                            }
                        }
                    } label: {
                        Text(AlertDetail.technicalDisclosureTitle)
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    }
                    .accessibilityIdentifier("alert-detail-technical")
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Alert", detail.headline)
        .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", action: onClose) } }
    }
}
```
> If `.riotHeader(eyebrow:_:)` truncates a long headline, the `Text(verbatim:)` body remains the authoritative full headline — the header is chrome. Confirm the `riotHeader` signature at `ConferenceShellView.swift:627,733` and match it exactly.

- [ ] **Step 4: Run → PASS** (3 new tests). **Step 5: Commit** the sheet + extended test.

---

## Task 3: Home Alerts card + `AlertsListView` + demote the dead Nearby `LabeledContent`

**Files:** Modify `apps/ios/Riot/AlertsListView.swift` (add `AlertsListView`), `apps/ios/Riot/ConferenceShellView.swift`

- [ ] **Step 1: Failing test** — assert the surface is reachable from Home (single entry point) and that the Nearby count is no longer the alert entry point. Since the card/list are SwiftUI, drive the state seam + assert the wiring shape:
```swift
extension AlertsSurfaceTests {
    func testAlertsListStateIsBuiltFromActiveModelEntries() {
        // The Home card feeds AlertsListState.from(model.entries, model.space?.namespaceID)
        // — the exact call the view makes — so a green state here is the card's content.
        let e = Self.entry("Water main break", signerID: Self.activeNS)
        guard case let .populated(rows) = AlertsListState.from([e], activeNamespaceID: Self.activeNS) else {
            return XCTFail("expected populated")
        }
        XCTAssertEqual(rows.first?.headline, "Water main break")
        XCTAssertEqual(rows.first.map { AlertDetail(entry: $0.entry).headline }, "Water main break")
    }

    func testNoActiveSpaceYieldsEmptyStateNotACrash() {
        // Home renders the card with activeNamespaceID = "" before a community is joined.
        guard case .empty = AlertsListState.from([], activeNamespaceID: "") else {
            return XCTFail("no active space must be the benign empty state")
        }
    }
}
```

- [ ] **Step 2: Run → FAIL** (until `AlertsListView` exists; the state tests may already pass — the point is the view compiles against them in Step 4).

- [ ] **Step 3: Implement `AlertsListView`** (in `AlertsListView.swift`) and wire it into Home.
  - **`AlertsListView`** — renders `AlertsListState` inside a `RiotCard`, organizer-first rows, each a `Button` opening `AlertDetailSheet`; empty state shows `AlertsStrings.emptyTitle`/`.emptyMessage`. Headline + signer render as **plain `Text(verbatim:)`** (no auto-link). Signer line leads with the core-verified `signerTag` + organizer badge; the optional rendered name (via an injected `displayName:(String)->String?`) is secondary:
```swift
public struct AlertsListView: View {
    public let entries: [RiotEntry]
    public let activeNamespaceID: String
    /// Self-claimed display name for a signer, if known — decoration only.
    public let displayName: (String) -> String?
    @State private var selected: RiotEntry?
    @Environment(\.colorScheme) private var colorScheme

    public init(entries: [RiotEntry], activeNamespaceID: String,
                displayName: @escaping (String) -> String? = { _ in nil }) {
        self.entries = entries
        self.activeNamespaceID = activeNamespaceID
        self.displayName = displayName
    }

    public var body: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text(AlertsStrings.title.uppercased())
                    .font(.riot(.mono, size: 12, relativeTo: .caption)).tracking(1)
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                switch AlertsListState.from(entries, activeNamespaceID: activeNamespaceID) {
                case .empty(let empty):
                    Text(empty.title).font(.riot(.body, size: 15, relativeTo: .callout))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    Text(empty.message).font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                case .populated(let rows):
                    ForEach(rows) { row in
                        Button { selected = row.entry } label: { rowLabel(row) }
                            .buttonStyle(.riotSecondary)
                            .accessibilityIdentifier("alert-\(row.id)")
                    }
                }
            }
        }
        .accessibilityIdentifier("home-alerts-card")
        .sheet(item: $selected) { entry in
            AlertDetailSheet(detail: AlertDetail(entry: entry), onClose: { selected = nil })
        }
    }

    @ViewBuilder private func rowLabel(_ row: AlertRow) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(verbatim: row.headline).font(.riot(.body, size: 17, relativeTo: .body))
            HStack(spacing: 6) {
                if row.isOrganizer {
                    Text(AlertsStrings.organizerBadge).font(.riot(.mono, size: 11, relativeTo: .caption2))
                }
                Text(verbatim: displayName(row.signerID) ?? row.signerTag)
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                Spacer()
                Text(row.freshness).font(.riot(.mono, size: 11, relativeTo: .caption2))
            }
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
    }
}
```
  - **Home wiring** (`HomeRouteView.body`, `ConferenceShellView.swift:614-626`): insert the card into the `VStack`, after `NewswireSurfaceView`:
```swift
NewswireSurfaceView(model: newswire)
AlertsListView(entries: model.entries,
               activeNamespaceID: model.space?.namespaceID ?? "",
               displayName: { model.rendered(for: $0) })
PostUpdateView(model: composer)
```
  - **Demote the dead Nearby label** (`ConferenceShellView.swift:1003`): the single tappable alert entry point is now the Home card, so the Nearby "On this device" label stays a **plain, non-navigational diagnostic** — relabel it honestly so it is no longer the app's only (dead-end) alert surface:
```swift
LabeledContent("Alerts on this device", value: "\(model.entries.count)")
```
  This keeps the diagnostic count (device transport context) without a second divergent entry point (anti-dead-end invariant #1). Do NOT make it tappable — Home owns the navigation.

- [ ] **Step 4: Run → PASS** + iOS RiotKit build compiles the new view. **Step 5: Commit** both files.

---

## Task 4: pbxproj registration (BOTH projects)

**Files:** Modify `apps/ios/Riot.xcodeproj/project.pbxproj`, `apps/macos/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1** Confirm the id block is free: `grep -c "A01000000000000000000010" apps/ios/Riot.xcodeproj/project.pbxproj` → 0 (pick the next free `A010…` block if taken).
- [ ] **Step 2 (iOS)** Register the two new Swift files — mirror the `CommunityChooser.swift` 4-part pattern exactly (`project.pbxproj:142-145,223-224` + groups `:8,:11`):
  - `AlertsListView.swift`: `PBXFileReference A0100000000000000000010` (`name = AlertsListView.swift; path = Riot/AlertsListView.swift; sourceTree = SOURCE_ROOT`) + `PBXBuildFile A0100000000000000000011 (fileRef = …010)`; add `…010` to the Riot group `A00000000000000000000002.children`; add `…011` to the sources phase `A00000000000000000000030.files`.
  - `AlertsSurfaceTests.swift`: `PBXFileReference A0100000000000000000020` (`path = RiotTests/AlertsSurfaceTests.swift`) + `PBXBuildFile A0100000000000000000021`; add `…020` to the RiotTests group `A00000000000000000000003.children`; add `…021` to the test sources phase `A00000000000000000000031.files`.
  - (One view file only — no separate model file — so one source pair + one test pair, exactly like `CommunityChooser`.)
- [ ] **Step 3 (macOS)** Mirror with the `F0…` prefix and `path = ../ios/Riot/AlertsListView.swift` / `../ios/RiotTests/AlertsSurfaceTests.swift` — file ref `F010…010` + build file `F010…011` into the macOS Riot group + its app/RiotKit sources phase; test ref `F010…020` + build file `F010…021` into the macOS RiotTests group + test sources phase (match `CommunityChooser`'s `F0F0…` entries at `apps/macos/Riot.xcodeproj/project.pbxproj:147` and follow its build-file/group/phase placement).
- [ ] **Step 4** `plutil -lint apps/ios/Riot.xcodeproj/project.pbxproj apps/macos/Riot.xcodeproj/project.pbxproj` → both OK. **Step 5** Commit both pbxproj.

---

## Task 5: Build + full test both platforms
- [ ] iOS: `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'` — new `AlertsSurfaceTests` green; full RiotKit green except the known-red Bonjour two-peer test.
- [ ] iOS app + macOS app **BUILD SUCCEEDED** (plain SwiftUI views, no camera — no `#if os(iOS)` guard needed).
- [ ] Commit any fixups.

---

## Self-Review
- **Spec coverage (§3 "Read alerts" + §8 Unit 3):**
  - NEW `AlertDetailSheet` renders the `AlertDetail` value model, reusing the `DisclosureGroup` technical-details pattern ✅ Task 2 (headline + AI flag + validity summary; 64-hex ids behind the closed disclosure).
  - `AlertsListView` rows = headline + core-verified signer + freshness, organizer-first ✅ Task 1 (`AlertRow`, `AlertsListState.from` ordering) + Task 3 (row rendering).
  - **Anti-spoof:** signer + organizer-first ordering come from the core coordinate rule (`signerID == namespaceID`, both core-verified from `public_entry_identity`), never a self-claimed author field ✅ Task 1 (`testOrganizerFlagComesFromCoordinateNotName`); headline/name render as plain `Text(verbatim:)` — no markdown auto-link ✅ Task 2/3 (`testHeadlineIsCarriedVerbatim…`).
  - **ONE Alerts card on Home (per-community); no second entry point** ✅ Task 3 (card into `HomeRouteView`; dead Nearby `LabeledContent` demoted to a non-navigational diagnostic).
  - **Empty = benign "No alerts yet"** ✅ Task 1 (`AlertsEmpty.noAlerts`).
  - No new FFI ✅ (`list_current_entries` + `AlertDetail` + `rendered(for:)` all exist); both pbxproj registered ✅ Task 4; macOS builds ✅ Task 5.
- **Scoping decision (the key risk — stated explicitly):** VERIFIED that `model.entries` is **already active-community-scoped** at the FFI (`list_current_entries` scopes to `active_namespace` from `profile.space.namespace_id`, `mobile_state.rs:874-944`; `reproject_active` on switch). The design §3's "entries are global, add a filter" note reflects the pre-multi-community state and is now stale. This unit still adds a **Swift `namespaceID == activeNamespaceID` defense-in-depth filter** (`AlertsListState.from`) with a test that plants a foreign-namespace row and asserts it is dropped (`testForeignCommunityAlertsAreFilteredOut`) — so a future FFI regression cannot leak a foreign community's alert onto the card. Primary correctness = FFI scoping; the Swift filter = belt-and-suspenders. **If the implementer finds `list_current_entries` no longer scopes (regression), the Swift filter already covers it — but flag it, because it would mean the FFI contract changed.**
- **Placeholder scan:** none. Every symbol is real: `AlertDetail`/`RiotEntry`/`RiotAppModel.entries`/`.rendered(for:)`/`.space?.namespaceID`/`RiotCard`/`.riotHeader`/`DisclosureGroup`/`LabeledContent` are all cited with file:line. The one implement-time confirmation is the exact `.riotHeader(eyebrow:_:)` signature (flagged in Task 2) and the next-free pbxproj id block (grep-confirmed in Task 4, Step 1) — both checks, not guesses.
- **Type consistency:** `AlertsListState`/`AlertRow`/`AlertsEmpty`/`AlertsStrings`/`AlertRelativeTime`/`AlertDetailSheet`/`AlertsListView` used consistently across T1–T3; `AlertRow.init(_:activeNamespaceID:now:)` and `AlertsListState.from(_:activeNamespaceID:now:)` signatures match every call site; `AlertDetail(entry:)` and `RiotEntry` field names (`entryID`/`namespaceID`/`signerID`/`headline`/`createdAt`/`validFrom`/`expiresAt`/`aiAssisted`) match `ProfileRepository.swift:30-40`.
- **Dependency order:** T1 (pure state/rows) → T2 (detail sheet, uses `AlertDetail`) → T3 (list view + Home wiring, uses T1+T2) → T4 (both pbxproj) → T5 (build). All one unit; both pbxproj claimed for the whole unit.
