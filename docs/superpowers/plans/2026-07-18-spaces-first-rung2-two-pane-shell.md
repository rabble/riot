# Spaces-First — Rung 2: Two-pane shell skeleton — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax. This is a **skeleton** rung — relocate, do not redesign; land the detail *renders* in Rungs 3/4.

**Goal:** Make the **tiered space list the navigation root** on iOS + Android + macOS — Your space / Communities / Following — with the existing per-community routes (Home/People/Nearby/Tools) **relocated verbatim** under a selected-space detail, launch-restore of the last-active space, the §3.1 row-state vocabulary (icon+text, colour-independent) and §3.3 list-as-nav accessibility. **Tools leaves the top level.** By-kind detail routing lands with **placeholders** for the followed-site render (Rung 3) and the personal home (Rung 4); the actual renders are explicitly deferred.

**Architecture:** The Rung 1 core is already on `main` (commit `ae9ec47`): `CommunityRelationship::{Following,Personal}`, `FollowedSiteRow` (`root`/`title`/`state`/`transport_blocked`), `MobileProfile.list_followed_sites()`, and the `list_communities` Following-exclusion filter (`crates/riot-ffi/src/mobile_api.rs:27,29,62,394`). Rung 2 adds **no core/FFI logic** — it consumes those two lists in the shells and merges them into tiered groups with a **pure, host-testable** view-model. No policy in Swift/Kotlin (spec §6.5): the tier is core-assigned (`CommunityRow.relationship` / the author-less `FollowedSiteRow`), the shell only routes by it. The macOS shell shares the iOS sources verbatim, so every new Swift file is registered in **both** `apps/ios/Riot.xcodeproj/project.pbxproj` **and** `apps/macos/Riot.xcodeproj/project.pbxproj` (serialization hazard — CLAUDE.md rule 5 / spec Risk 2).

**Tech Stack:** Swift 6 / SwiftUI + XCTest (iOS/macOS, shared `RiotKit`); Kotlin 2.2 + JUnit (Android host-JVM). No Rust changes.

**Spec:** `docs/superpowers/specs/2026-07-18-spaces-first-navigation-design.md` §3 (two-pane anchor), §3.1 (row vocab), §3.3 (a11y), §4 (by-kind detail), §7 (rung 2), §11 (decisions).
**Prior rung:** `docs/superpowers/plans/2026-07-18-spaces-first-rung1-core-relationships.md` (landed as PR #59).
**Branch:** `overnight/2026-07-18`.
**Shared-checkout:** `gh pr list --search "spaces-first OR rung2 OR shell"` before AND during; pathspec commits only; `ConferenceShellView.swift`, `AppModel.swift`, both `project.pbxproj`, and `MainActivity.kt` are high-traffic and pbxproj is merge-hostile — coordinate or use the temp-index technique if a sibling holds a pbxproj.

---

## Verified grounding (line refs on THIS branch — note drift from the spec's older refs)

Spec §3/§4/§7 cite pre-overnight line numbers; the overnight checkout has drifted. **Reference by symbol, not line.** Verified 2026-07-18 against `overnight/2026-07-18`:

| Symbol | File:line (this branch) | Spec cited |
|---|---|---|
| `RiotDestination` enum (home/tools/people/nearby) | `apps/ios/Riot/AppModel.swift:11` | :11 ✓ |
| `RiotAppModel.select(_:)` | `AppModel.swift:366` | :366 ✓ |
| `openCommunityChooser()` | `AppModel.swift:633` | :633 ✓ |
| `isCommunityChooserPresented` | `AppModel.swift:976` | :976 ✓ |
| `switchCommunity(namespaceID:)` | `AppModel.swift:679` | — |
| `community: CommunityContext?` | `AppModel.swift:959` | — |
| `launchState` | `AppModel.swift:982` | — |
| `refreshCommunities()` (reload seam) | `AppModel.swift:625` | — |
| `CommunityReturnOutcome.decide(active:all:)` | `apps/ios/Riot/CommunityChooser.swift:165` | :165 ✓ |
| `CommunityChooserRow` / `.from(_:)` | `CommunityChooser.swift:55,100` | — |
| `CommunityChooserView` (modal `List`) | `CommunityChooser.swift:216` | :216 ✓ |
| `CommunityShellView` (community-parameterized detail) | `apps/ios/Riot/ConferenceShellView.swift:696` | :499/:504 (drift) |
| `macShell` (`NavigationSplitView`) | `ConferenceShellView.swift:912` | :640 (drift) |
| `routeView(_:)` (Home/Tools/People/Nearby switch) | `ConferenceShellView.swift:1112` | — |
| `RiotProfileRepository: CommunityRegistry` | `apps/ios/Riot/Core/ProfileRepository.swift:977` | — |
| `listCommunities()` / `activeCommunity()` wrappers | `ProfileRepository.swift:985,998` | — |
| `FollowedSiteRow` / `list_followed_sites()` (FFI) | `crates/riot-ffi/src/mobile_api.rs:62,394` | §6.2 |

**Three gaps the skeleton must close (all verified absent):**
1. **The Swift `RiotProfileRepository` wrapper does NOT expose `listFollowedSites()`** — only `listCommunities`/`activeCommunity` (`ProfileRepository.swift:985,998`). The generated `MobileProfile` binding gains `listFollowedSites()` after `generate-bindings`, but the app-facing wrapper needs a method. **Rung 2 Step 1 adds it.**
2. **Android's `CommunityRelationship` `when` is exhaustive with no `else`** (`apps/android/.../CommunityChooser.kt:16-20`). Regenerating the Android binding to include `FOLLOWING`/`PERSONAL` (Rung 1's additive variants) **breaks compilation** until arms are added — the Kotlin analogue of Rung 1's Rust match-exhaustiveness note. **Rung 2 Step 3 fixes it first, before any UI.**
3. **iOS twin: `CommunityChooser.swift`'s `plainLabel` `switch` is exhaustive over the three old cases with NO `default`** (`apps/ios/Riot/CommunityChooser.swift:9-15` — `case .organizer / .member / .publicReader`). Rung 2's own Swift tests can only see `.personal` / `FollowedSiteRow` **after the iOS/macOS binding is regenerated** (`cargo run -p xtask -- generate-bindings`), and that same regen adds `.following`/`.personal` to the generated `CommunityRelationship` enum → the `plainLabel` switch goes **non-exhaustive → Swift compile error** (the exact analogue of the Android `when` break). **Rung 2 Step 2.0 fixes it first, before any new model code**, by regenerating bindings and healing the switch with `.following => "Following"` and `.personal => "Your space"`. **Verified this is the ONLY landed iOS switch over `CommunityRelationship` that lacks a `default`:** `AlertsListView.swift` and `PeopleView.swift` merely *reference* the type (`.organizer` badge tests / booleans, never a `switch`), and `NewswireEditorial.swift`'s switches are over a different (trust-tier) enum — so no other iOS site breaks at regen time.

**Android reality check:** Android is still on the *old* debug shell — `ConferenceSurface` is a flat 7-value enum (`Spaces/App directory/Incident board/Newswire/Compose & sign/Import preview/Connection`, `ConferenceSurface.kt:3-11`) driven by an imperative `MainActivity.show(surface)` (`MainActivity.kt:172-187`, plain Android `View`/`addView`, not Compose). It already carries the pure-Kotlin `CommunityChooserRow` + `CommunityReturnOutcome.decide` mirrors (`CommunityChooser.kt`). So Android Rung 2 = pure-model parity + a space-list *root surface* skeleton, host-JVM tested like `ConferenceSurfaceTest.kt`.

---

## The increment sub-ladder

The design + CTO flagged big-bang risk (spec §7, §10 Risk 1). Rung 2 is **four independently-landable steps**, each green on its own, each behind rewritten nav test suites:

- **2.0 — Regenerate iOS/macOS bindings + heal the switch, then the shared pure tier/row-state model (Swift, no UI).** Regenerate the iOS/macOS bindings (`cargo run -p xtask -- generate-bindings`) so `.personal` / `FollowedSiteRow` exist, and heal the now-non-exhaustive `CommunityChooser.swift` `plainLabel` switch (`.following => "Following"`, `.personal => "Your space"`) so `RiotKit` compiles. Then add `SpaceTier`, `SpaceRowState` vocabulary (§3.1), `SpaceListRow`, and a pure `groupedSpaceList(...)` merge of `[CommunityRow]` + `[FollowedSiteRow]` with the §3.3 VoiceOver announcement. Fully unit-tested without a window or the FFI. First because it de-risks everything above it; the only landed-shell edit is the one-line-per-arm `plainLabel` heal (unavoidable — the regen forces it, exactly as the Android `when` is forced in 2.3).
- **2.1 — iOS repository + app-model plumbing.** Add `listFollowedSites()` to the `RiotProfileRepository` wrapper + a `SpaceListing` seam; publish a tiered `spaceList` on `RiotAppModel`, refreshed in `reload()`. No UI. Additive → green.
- **2.2 — iOS/macOS two-pane shell.** Space list as root (reuse `NavigationSplitView`), `CommunityShellView` carried **verbatim** as the community detail, by-kind routing to followed/personal **placeholders**, launch-restore via `decide()`, Tools off the top level. Rewrites the nav test suites.
- **2.3 — Android.** Exhaustive-`when` fix + `FollowedSiteRow`/tier Kotlin mirrors + a space-list root surface skeleton routing by kind. JUnit-tested.

Steps 2.0→2.2 are iOS/macOS and sequential; 2.3 (Android) is independent and may land in parallel.

---

## Step 2.0 — Regenerate bindings + heal the iOS switch, then the shared pure tier + row-state model (Swift)

**Why first:** pure value types, no UI, no live FFI — provable with `swift`/XCTest alone. It is the *safest* first commit, but **not** a zero-shell-edit one: the binding regen this step performs (needed so the tests can name `.personal` / `FollowedSiteRow`) adds `.following`/`.personal` to the generated `CommunityRelationship` enum, which forces the landed `CommunityChooser.swift` `plainLabel` switch non-exhaustive. This step therefore **also heals that switch** (one arm each for `.following` / `.personal`) — otherwise `RiotKit` will not compile and Steps 2.1/2.2 are all blocked. This is the iOS twin of the Android `when` heal in Step 2.3, and (verified) `CommunityChooser.swift:9-15` is the only landed iOS `switch` over `CommunityRelationship` without a `default` — `AlertsListView`/`PeopleView` only reference the type, they do not switch on it.

**Files:**
- New: `apps/ios/Riot/SpaceList.swift`
- New test: `apps/ios/RiotTests/SpaceListModelTests.swift`
- Modify: `apps/ios/Riot/CommunityChooser.swift` (heal the `plainLabel` switch: add `case .following: return "Following"` and `case .personal: return "Your space"`).
- Regenerate (not hand-edited): the iOS/macOS UniFFI binding via `cargo run -p xtask -- generate-bindings` — brings `.personal`/`.following` and `FollowedSiteRow` into `riot_ffi.swift`.
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj` **and** `apps/macos/Riot.xcodeproj/project.pbxproj` (register both new files in each — see the pbxproj recipe below).

### - [ ] Step 0: Regenerate iOS/macOS bindings + heal the switch (before any test runs)
- `cargo run -p xtask -- generate-bindings` — regenerates `riot_ffi.swift` so `CommunityRelationship` gains `.following`/`.personal` and `FollowedSiteRow` exists (Rung 1's additive variants). Without this the Step 1 tests can't even name `.personal` / `FollowedSiteRow`.
- Heal `apps/ios/Riot/CommunityChooser.swift` `plainLabel` (`:9-15`): add `case .following: return "Following"` and `case .personal: return "Your space"` so the switch is exhaustive again and `RiotKit` compiles. (This is the RED-then-GREEN twin of Android Step 2.3 Step 1 — the regen is the forcing function; the heal is the fix.)

### - [ ] Step 1: Write the failing tests

`apps/ios/RiotTests/SpaceListModelTests.swift`:
```swift
import XCTest
@testable import RiotKit

final class SpaceListModelTests: XCTestCase {
    // §3.1 — every state has a distinct icon AND text; never colour alone.
    func testEveryRowStateHasAnIconAndTextAndIsColourIndependent() {
        let states: [SpaceRowState] = [
            .available, .syncing, .pendingFirstSync,
            .quarantined, .transportBlocked, .degraded,
        ]
        let icons = states.map(\.systemImage)
        XCTAssertEqual(Set(icons).count, icons.count, "each state has a distinct SF Symbol")
        for s in states {
            XCTAssertFalse(s.label.isEmpty, "each state carries a text label, not just colour")
        }
        // The require:arti fail-closed-at-the-row honesty (S1).
        XCTAssertTrue(SpaceRowState.transportBlocked.label.localizedCaseInsensitiveContains("unavailable"))
    }

    // §3 — the two core lists merge into exactly the three tiers, in order.
    func testCommunitiesAndFollowedSitesMergeIntoTheThreeTiersInOrder() {
        let personal = Self.communityRow(ns: "p", title: "You", relationship: .personal)
        let community = Self.communityRow(ns: "c", title: "Richmond", relationship: .member)
        let followed = FollowedSiteRow(root: String(repeating: "a", count: 64),
                                       title: "indymedia", state: "pending-first-sync",
                                       transportBlocked: false)
        let sections = groupedSpaceList(communities: [personal, community],
                                        followed: [followed], now: Date())
        XCTAssertEqual(sections.map(\.tier), [.yourSpace, .communities, .following],
                       "tiers appear in the canonical Your space → Communities → Following order")
        XCTAssertEqual(sections.first { $0.tier == .yourSpace }?.rows.map(\.name), ["You"])
        XCTAssertEqual(sections.first { $0.tier == .communities }?.rows.map(\.name), ["Richmond"])
        XCTAssertEqual(sections.first { $0.tier == .following }?.rows.map(\.name), ["indymedia"])
    }

    // §3.3 — a VoiceOver row announces tier + name + state ("Richmond, community, syncing").
    func testVoiceOverAnnouncementCombinesTierNameAndState() {
        let row = SpaceListRow(id: "c", tier: .communities, name: "Richmond", state: .syncing)
        XCTAssertEqual(row.accessibilityLabel, "Richmond, community, Syncing")
    }

    // An empty Following tier is dropped, never rendered as a blank section (§9 empty state).
    func testAnEmptyFollowingTierIsOmitted() {
        let sections = groupedSpaceList(communities: [], followed: [], now: Date())
        XCTAssertTrue(sections.isEmpty, "no tiers render when there is nothing in them")
    }

    // §3.2 — the search field is a PURE name-filter over the grouped list; the
    // UI TextField just binds a query to this function (no policy in the view).
    func testSearchFieldFiltersRowsByNameAndDropsEmptiedTiers() {
        let you = Self.communityRow(ns: "p", title: "You", relationship: .personal)
        let richmond = Self.communityRow(ns: "c", title: "Richmond", relationship: .member)
        let oakland = Self.communityRow(ns: "d", title: "Oakland", relationship: .member)
        let sections = groupedSpaceList(communities: [you, richmond, oakland],
                                        followed: [], now: Date())
        // Case-insensitive substring match on the row name.
        let filtered = filteredSpaceList(sections, query: "rich")
        XCTAssertEqual(filtered.first { $0.tier == .communities }?.rows.map(\.name), ["Richmond"])
        // A tier with no surviving rows is dropped, not shown empty (§9).
        XCTAssertNil(filtered.first { $0.tier == .yourSpace },
                     "the Your space tier is omitted when its only row is filtered out")
        // An empty/whitespace query returns the list unchanged.
        XCTAssertEqual(filteredSpaceList(sections, query: "  ").map(\.tier), sections.map(\.tier))
    }

    private static func communityRow(ns: String, title: String,
                                     relationship: CommunityRelationship) -> CommunityRow {
        CommunityRow(namespaceId: ns, title: title, relationship: relationship,
                     recentActivityUnixSeconds: nil, syncFreshnessUnixSeconds: nil,
                     available: true, archived: false, quarantined: false,
                     descriptorEntryId: nil)
    }
}
```
> Engineer note: match the real `CommunityRow` `uniffi::Record` field list from the generated `riot_ffi.swift` (grep `struct CommunityRow`); the stub above lists the known fields, adjust to the exact initializer.

### - [ ] Step 2: Run to verify they fail
```
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SpaceListModelTests
```
→ FAIL (`cannot find 'SpaceRowState'` / `groupedSpaceList`).

### - [ ] Step 3: Implement `SpaceList.swift`

`apps/ios/Riot/SpaceList.swift` — pure, `RiotKit`-public, no view code:
- `enum SpaceTier: String { case yourSpace, communities, following }` with `title` ("Your space"/"Communities"/"Following") and a VoiceOver noun (`"your space"`/`"community"`/`"followed site"`).
- `enum SpaceRowState { case available, syncing, pendingFirstSync, quarantined, transportBlocked, degraded }` — each with a distinct `systemImage` (icon) and a colour-independent `label` (§3.1). `transportBlocked.label` = `"Requires Tor — unavailable"` (§3.1 / Security S1). A `quarantined` row is still **selectable** (its detail carries recovery — never vanished).
- `struct SpaceListRow: Identifiable, Equatable { id; tier; name; state; namespaceOrRoot }` with `accessibilityLabel = "\(name), \(tier.voiceOverNoun), \(state.label)"` (§3.3).
- `struct SpaceListSection: Identifiable { tier; rows }`.
- `func groupedSpaceList(communities: [CommunityRow], followed: [FollowedSiteRow], now: Date) -> [SpaceListSection]`:
  - Split communities by `relationship == .personal` → Your space, else → Communities (organizer/member/publicReader).
  - Map each `CommunityRow` to a `SpaceRowState` by the **core-provided** signals (`quarantined` → `.quarantined`; `!available` → `.degraded`; the `CommunityChooserRow.isPendingFirstSync` rule → `.pendingFirstSync`; else `.available`). *Reuse the existing `CommunityChooserRow.isPendingFirstSync` derivation so there is one pending rule, not two.*
  - Map each `FollowedSiteRow` to a state from its `state` token + `transportBlocked` (§6.2 note: Rung 1 persists the default `pending-first-sync`; the true transport-blocked path is Rung 5, but the mapping is complete now).
  - Emit only non-empty tiers, in `[.yourSpace, .communities, .following]` order.
- `func filteredSpaceList(_ sections: [SpaceListSection], query: String) -> [SpaceListSection]` — the pure §3.2 search filter: trims `query`; an empty/whitespace query returns `sections` unchanged; otherwise keeps rows whose `name` contains the query case-insensitively and **drops any tier left with no rows** (§9 — never a blank section). The 2.2 `SpaceListView` search `TextField` binds only to this — no filtering logic in the view.

### - [ ] Step 4: Register both files in BOTH pbxproj (see recipe) — build green
### - [ ] Step 5: Run to verify pass
```
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/SpaceListModelTests
```
→ PASS. Then a full `-scheme RiotKit` run and a `-project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS` build to prove the macOS pbxproj entry is correct.

### - [ ] Step 6: Commit
```bash
git add apps/ios/Riot/SpaceList.swift apps/ios/RiotTests/SpaceListModelTests.swift \
        apps/ios/Riot/CommunityChooser.swift \
        apps/ios/Riot.xcodeproj/project.pbxproj apps/macos/Riot.xcodeproj/project.pbxproj \
        <regenerated riot_ffi.swift binding path(s)>
git commit -m "feat(spaces/rung2): regen bindings + heal plainLabel switch; pure tiered space-list model + row-state vocabulary (§3.1/§3.3)"
```

---

## Step 2.1 — iOS repository wrapper + app-model plumbing (no UI)

**Why:** the shell (2.2) needs both lists behind a test-stubbable seam; the `RiotProfileRepository` wrapper doesn't expose followed sites yet.

**Files:**
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift` (add `listFollowedSites()`)
- Modify: `apps/ios/Riot/CommunityChooser.swift` (extend the `CommunityRegistry` protocol OR add a sibling `SpaceListing` protocol — see decision)
- Modify: `apps/ios/Riot/AppModel.swift` (publish `spaceList`, refresh in `reload()`)
- Modify test: `apps/ios/RiotTests/CommunityChooserTests.swift` (or a new `SpaceListingTests.swift`)
- pbxproj: only if a NEW file is added (if you choose a new `SpaceListing` file/test, register in both).

**Decision (lock in the plan):** add a **sibling `SpaceListing` protocol** (`func listFollowedSites() throws -> [FollowedSiteRow]`) rather than widening `CommunityRegistry` — followed sites are author-less and not `CommunityRow` (spec §6.2 "do NOT shoehorn a followed site into `CommunityRow`"). `RiotProfileRepository` conforms to both; tests inject a stub conforming to both.

### - [ ] Step 1: Write the failing test (stub-driven, no live FFI)
```swift
@MainActor
func testReloadPublishesBothTiersFromTheTwoCoreLists() {
    let model = RiotAppModel()
    let listing = StubSpaceListing(
        communities: [ /* one .personal, one .member CommunityRow */ ],
        followed: [ FollowedSiteRow(root: String(repeating: "a", count: 64),
                                    title: "indymedia", state: "pending-first-sync",
                                    transportBlocked: false) ])
    model.injectSpaceListingForTest(listing)      // test seam, mirrors existing bootstrap stubs
    model.refreshSpaceListForTest()
    XCTAssertEqual(model.spaceList.map(\.tier), [.yourSpace, .communities, .following])
}
```

### - [ ] Step 2: Run → FAIL (`no member spaceList`).
```
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/CommunityChooserTests/testReloadPublishesBothTiersFromTheTwoCoreLists
```

### - [ ] Step 3: Implement
- `ProfileRepository.swift`: `extension RiotProfileRepository: SpaceListing { public func listFollowedSites() throws -> [FollowedSiteRow] { try handle.listFollowedSites() } }` — a thin passthrough to the generated `MobileProfile.listFollowedSites()` (mirrors `listCommunities()` at :985). *No logic; core owns the list.*
- `AppModel.swift`: add `@Published public private(set) var spaceList: [SpaceListSection] = []`; add a private `refreshSpaceList()` that reads `listCommunities()` + `listFollowedSites()` (best-effort, `try?` — a followed-list failure leaves communities, never blanks) and calls `groupedSpaceList(...)`; call it from `reload()` (next to `refreshCommunities()` at :625) and from `refreshFromStore()`.
- Add the `injectSpaceListingForTest` / `refreshSpaceListForTest` seams alongside the existing test-only bootstrap seams.

### - [ ] Step 4: Run → PASS.  Full `RiotKit` iOS test run stays green (additive).
```
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'
```
### - [ ] Step 5: Commit
```bash
git add apps/ios/Riot/Core/ProfileRepository.swift apps/ios/Riot/CommunityChooser.swift apps/ios/Riot/AppModel.swift apps/ios/RiotTests/CommunityChooserTests.swift
git commit -m "feat(spaces/rung2): expose listFollowedSites + publish tiered spaceList on the model"
```

---

## Step 2.2 — iOS/macOS two-pane shell: space list root, by-kind detail, launch-restore

**Why:** the visible IA turn. Space list becomes the root; the community detail is carried **verbatim**; Tools leaves the top level.

**Files:**
- New: `apps/ios/Riot/SpaceListView.swift` (left pane: tiered sections, row-state chrome, a11y)
- New: `apps/ios/Riot/SpaceDetailPlaceholders.swift` (`FollowedSiteDetailPlaceholder` for Rung 3, `YourSpaceDetailPlaceholder` for Rung 4)
- Modify: `apps/ios/Riot/ConferenceShellView.swift` (root becomes a `NavigationSplitView` over the space list; detail routes by kind)
- Modify: `apps/ios/Riot/AppModel.swift` (a `selectedSpace` selection + `restoreLastActiveSpace()` using `CommunityReturnOutcome.decide`)
- New test: `apps/ios/RiotTests/SpaceShellTests.swift`
- Modify test: `apps/ios/RiotTests/ShellNavigationTests.swift` (the four-routes assertions stay — routes are relocated, not removed — but add: Tools is not a *top-level* nav slot; the root is the space list)
- pbxproj: register `SpaceListView.swift`, `SpaceDetailPlaceholders.swift`, `SpaceShellTests.swift` in **both** projects.

**Layout decision (skeleton, "relocate don't redesign" — spec §7):**
- The **outer** shell is a `NavigationSplitView` whose sidebar is `SpaceListView` and whose detail routes **by core-assigned kind**:
  - community selected → `CommunityShellView(model:, community:)` **verbatim** (keeps its own inner four-route tab bar on phone / route sidebar on mac — untouched).
  - followed site → `FollowedSiteDetailPlaceholder` (Rung 3 render slot).
  - your space (a `.personal` row) → `YourSpaceDetailPlaceholder` (Rung 4).
- **Phone:** the split view collapses to a drill-in automatically (space list is the root column; tapping a row pushes the detail; back returns to the list) — native `NavigationSplitView` phone behaviour, no custom stack.
- **macOS/iPad:** both panes visible. The community detail's *inner* `macShell` `NavigationSplitView` is nested under the outer one. **Flag (accepted skeleton tradeoff):** nested split views are visually busy; polishing the mac layout to a single 3-column split (spaces → routes → content) is **deferred as Rung 2 follow-up**, explicitly *not* a skeleton blocker (spec §7 "relocate, don't redesign").
- **Tools off the top level:** the top-level nav is now the space list; the four `RiotDestination` routes exist **only inside** `CommunityShellView` (already true — `routeView` at :1112). No top-level Tools slot remains. The old modal chooser (`openCommunityChooser`/`isCommunityChooserPresented`) is **superseded** by the persistent pane; keep the model members (deep-link/Command-K still call them) but the pane is the primary selection surface. Command-K may focus the sidebar; a full remap is optional polish.

### - [ ] Step 1: Write the failing tests

`apps/ios/RiotTests/SpaceShellTests.swift`:
```swift
import XCTest
@testable import RiotKit

@MainActor
final class SpaceShellTests: XCTestCase {
    // §7 — the navigation ROOT is the tiered space list, not a single community.
    func testTheNavigationRootIsTheTieredSpaceList() {
        let model = RiotAppModel()
        // A fresh model exposes a spaceList surface (empty is a valid root — §9).
        XCTAssertNotNil(model.spaceList)
        XCTAssertTrue(SpaceShellRoot.isSpaceList, "the shell root is the space list, not a community")
    }

    // §4 — by-kind detail routing: the shell picks a detail surface from the
    // core-assigned tier only (no policy in Swift — §6.5).
    func testDetailRoutingIsByCoreAssignedKind() {
        XCTAssertEqual(SpaceDetailRoute.forTier(.communities), .community)
        XCTAssertEqual(SpaceDetailRoute.forTier(.following), .followedSitePlaceholder)  // Rung 3 slot
        XCTAssertEqual(SpaceDetailRoute.forTier(.yourSpace), .yourSpacePlaceholder)     // Rung 4 slot
    }

    // §3 — launch restores the last-active space (reuse decide()); a single-space
    // user pays no extra tap.
    func testLaunchRestoresLastActiveCommunityDirectly() {
        let active = Self.row(ns: "richmond", available: true)
        let outcome = CommunityReturnOutcome.decide(active: active, all: [active])
        XCTAssertEqual(outcome, .openCommunity(namespaceID: "richmond"),
                       "a held, available last-active space opens straight to its detail")
    }

    // §7 — Tools is NOT a top-level destination; it lives inside a community detail.
    func testToolsIsNotATopLevelNavigationSlot() {
        // The top-level nav is the space list; the four routes live under a community.
        XCTAssertFalse(SpaceShellRoot.topLevelSlots.contains(.tools),
                       "Tools is relocated to the per-community detail, not the shell root")
        // The community routes themselves are unchanged (relocated verbatim).
        XCTAssertEqual(RiotDestination.phoneTabs, [.home, .tools, .people, .nearby])
    }

    private static func row(ns: String, available: Bool) -> CommunityRow {
        CommunityRow(namespaceId: ns, title: ns, relationship: .member,
                     recentActivityUnixSeconds: nil, syncFreshnessUnixSeconds: nil,
                     available: available, archived: false, quarantined: false,
                     descriptorEntryId: nil)
    }
}
```
> `SpaceShellRoot` / `SpaceDetailRoute` are small pure decision types (like the existing `Onboarding`/`ShellRecoveryState` enums) so the routing is provable without a live window — the same pattern `ShellNavigationTests` uses.

Also add to `ShellNavigationTests.swift`: keep `testTheShellExposesExactlyTheFourCommunityRoutes` (routes are relocated, not removed) and add an assertion that the **root** is the space list.

### - [ ] Step 2: Run → FAIL (`cannot find SpaceShellRoot` / `SpaceDetailRoute`).

### - [ ] Step 3: Implement
- `SpaceListView.swift`: renders `model.spaceList` as `Section`s per tier; each row shows icon+text state (`SpaceRowState`), a pinned search field (§3.2 — a `TextField` bound to a `@State` query string that feeds the pure `filteredSpaceList(_:query:)` from 2.0; no filtering logic in the view), and pinned Join/Create affordances (reuse `model.requestCreateCommunity()` / `requestJoinByReference()`); each row carries `.accessibilityLabel(row.accessibilityLabel)` (§3.3) and `.accessibilityIdentifier("space-row-\(row.id)")`. Selecting a row sets `model.selectedSpace`.
- `SpaceDetailPlaceholders.swift`: two neutral, honest placeholder views (§9 "never a blank or a fabricated feed"), each naming what lands next ("Site view arrives soon" / "Your space arrives soon"). **§4 chrome scope:** the full cross-kind chrome *contract* (header = title + **tier badge** + relationship control) **lands with the renders in Rung 3/4**, not here — but each placeholder cheaply carries a **tier-badge header stub now** (the selected row's title + a `SpaceTier`-derived badge label, e.g. "Following" / "Your space"), so the header slot exists and is testable before the render fills it. No relationship control (Follow/Unfollow) in Rung 2 — that is Rung 3/5.
- `ConferenceShellView.swift`: wrap the existing launch switch in an outer `NavigationSplitView { SpaceListView(model:) } detail: { detailForSelection }`. `detailForSelection` uses `SpaceDetailRoute.forTier(selectedTier)` → `CommunityShellView` (verbatim) / placeholder. Preserve the existing `.loading`/`.noCommunity`/`.unavailable` launch states as the **empty/first-run** root (§9): no spaces → the space list shows the empty state + the neutral welcome detail.
- `AppModel.swift`: `@Published var selectedSpace: SpaceSelection?`; `restoreLastActiveSpace()` calls `CommunityReturnOutcome.decide(active: try? activeCommunity(), all: try? listCommunities())` and sets the selection (community tiers only — extending `decide()` to followed/personal is **deferred to Rung 3/4**, noted below). Call it once when the profile opens.

### - [ ] Step 4: Register the 3 new files in BOTH pbxproj — build green
### - [ ] Step 5: Run → PASS
```
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS
```
→ iOS tests PASS; macOS builds (proves both pbxproj entries and the shared-source shell compile on mac).

### - [ ] Step 6: Commit
```bash
git add apps/ios/Riot/SpaceListView.swift apps/ios/Riot/SpaceDetailPlaceholders.swift \
        apps/ios/Riot/ConferenceShellView.swift apps/ios/Riot/AppModel.swift \
        apps/ios/RiotTests/SpaceShellTests.swift apps/ios/RiotTests/ShellNavigationTests.swift \
        apps/ios/Riot.xcodeproj/project.pbxproj apps/macos/Riot.xcodeproj/project.pbxproj
git commit -m "feat(spaces/rung2): space-list root + by-kind detail + launch-restore; Tools off top level"
```

---

## Step 2.3 — Android: exhaustive-when fix + tier mirrors + space-list root skeleton

**Why independent:** Android shares no Swift; it mirrors the pure models and adds a root surface, host-JVM tested. It must **first** heal the exhaustive-`when` break the Rung 1 variants introduce at binding-regen time.

**Files:**
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/CommunityChooser.kt` (add `FOLLOWING`/`PERSONAL` arms; add `FollowedSiteRow` mirror + tier grouping)
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/ConferenceSurface.kt` (add a `SPACE_LIST("Your Spaces")` root surface — distinct label, since `SPACES("Spaces")` already exists) and `MainActivity.kt` (a `showSpaceList()` skeleton routing by kind)
- New test: `apps/android/app/src/test/kotlin/org/riot/evidence/SpaceListTest.kt`
- Modify test: `apps/android/app/src/test/kotlin/org/riot/evidence/ConferenceSurfaceTest.kt` (update the expected surface list)

### - [ ] Step 1: Regenerate the Android binding, watch the break
- Run the Android binding regen (the Kotlin analogue of Rung 1 Task 5 — `generate-bindings` + the Android staticlib/jni build via `scripts/conference/build-native-core.sh`). The `uniffi.riot_ffi.CommunityRelationship` enum now has `FOLLOWING`/`PERSONAL`.
- `./gradlew :app:compileDebugKotlin` → **FAIL**: `CommunityRelationship.plainLabel()` `when` (`CommunityChooser.kt:16-20`) is non-exhaustive. This is the intended RED — it proves the hazard the plan flagged.

### - [ ] Step 2: Write the failing JUnit tests

`apps/android/app/src/test/kotlin/org/riot/evidence/SpaceListTest.kt`:
```kotlin
package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.CommunityRelationship

class SpaceListTest {
    @Test fun everyRelationshipHasAPlainLabelIncludingTheNewTiers() {
        // No exception, no missing arm — proves the exhaustive-when is healed.
        assertEquals("Following", CommunityRelationship.FOLLOWING.plainLabel())
        assertEquals("Your space", CommunityRelationship.PERSONAL.plainLabel())
    }

    @Test fun communitiesAndFollowedSitesMergeIntoTheThreeTiersInOrder() {
        val sections = groupedSpaceList(
            communities = listOf(
                communityRow("p", "You", CommunityRelationship.PERSONAL),
                communityRow("c", "Richmond", CommunityRelationship.MEMBER),
            ),
            followed = listOf(FollowedSiteRow("a".repeat(64), "indymedia", "pending-first-sync", false)),
            nowUnixSeconds = 0L,
        )
        assertEquals(listOf(SpaceTier.YOUR_SPACE, SpaceTier.COMMUNITIES, SpaceTier.FOLLOWING),
                     sections.map { it.tier })
    }

    @Test fun aVoiceOverStyleAnnouncementCombinesTierNameAndState() {
        val row = SpaceListRow("c", SpaceTier.COMMUNITIES, "Richmond", SpaceRowState.SYNCING)
        assertEquals("Richmond, community, Syncing", row.contentDescription)
    }

    @Test fun transportBlockedRowIsHonestAtTheRow() {
        assertTrue(SpaceRowState.TRANSPORT_BLOCKED.label.contains("unavailable", ignoreCase = true))
    }
}
```

### - [ ] Step 3: Implement
- `CommunityChooser.kt`: add the two `when` arms (`FOLLOWING -> "Following"`, `PERSONAL -> "Your space"`) — heals compilation.
- New pure Kotlin in a `SpaceList.kt` (or extend `CommunityChooser.kt`): `SpaceTier`, `SpaceRowState` (icon token + `label`), `SpaceListRow` (`contentDescription = "$name, ${tier.talkBackNoun}, ${state.label}"` — TalkBack §3.3), `data class FollowedSiteRow(root, title, state, transportBlocked)` mirroring the FFI record (or use the generated `uniffi.riot_ffi.FollowedSiteRow` directly if the regen produced it — prefer the generated type), and `groupedSpaceList(...)` mirroring the Swift merge.
- `ConferenceSurface.kt`: add the new tiered root surface as `SPACE_LIST("Your Spaces")` — **use a distinct label, NOT `"Spaces"`**, because the enum already carries `SPACES("Spaces")` (verified: `ConferenceSurface.kt` line 4) and two members sharing a `label` string would collide in any label-driven lookup/`ConferenceSurfaceTest` assertion. `MainActivity.show(...)` gains a `SPACE_LIST -> showSpaceList()` arm rendering tiered rows and routing by kind to the existing surfaces (community → the existing board/newswire surfaces; followed/personal → a placeholder `TextView`). Skeleton — relocate, don't redesign the debug surfaces.
- Update `ConferenceSurfaceTest.kt`'s expected label list to include the new root.
- **Launch-restore:** no new Android restore logic is needed — Android already carries the pure-Kotlin `CommunityReturnOutcome.decide(active:all:)` mirror (`CommunityChooser.kt`, per the Android reality check above), so the space-list root reuses it to pick the last-active space exactly as iOS Step 2.2 does. Rung 2 just wires the existing mirror to the new root surface; the followed/personal `decide()` extension is deferred to Rung 3/4 (same as iOS).

### - [ ] Step 4: Run → PASS
```
cd apps/android && ./gradlew :app:testDebugUnitTest
```
Then `./gradlew :app:compileDebugKotlin` (or `:app:assembleDebug`) to prove the app builds with the regenerated bindings.

### - [ ] Step 5: Commit
```bash
git add apps/android/app/src/main/kotlin/org/riot/evidence/CommunityChooser.kt \
        apps/android/app/src/main/kotlin/org/riot/evidence/ConferenceSurface.kt \
        apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
        apps/android/app/src/test/kotlin/org/riot/evidence/SpaceListTest.kt \
        apps/android/app/src/test/kotlin/org/riot/evidence/ConferenceSurfaceTest.kt
# (+ any new SpaceList.kt)
git commit -m "feat(spaces/rung2): android tier mirrors + space-list root; heal Following/Personal when"
```

---

## pbxproj recipe (BOTH projects, every new Swift file) — CLAUDE.md rule 5 / spec Risk 2

For each new Swift **source** file (`SpaceList.swift`, `SpaceListView.swift`, `SpaceDetailPlaceholders.swift`) and each new **test** file (`SpaceListModelTests.swift`, `SpaceShellTests.swift`):

1. **iOS** `apps/ios/Riot.xcodeproj/project.pbxproj` — add a `PBXFileReference` (`path = Riot/<File>.swift; sourceTree = SOURCE_ROOT;` for sources; `path = RiotTests/<File>.swift` for tests — mirror `CommunityChooser.swift` at :160), a `PBXBuildFile`, membership in the correct target's **Sources** build phase (RiotKit for sources, RiotTests for tests), and a `PBXGroup` child entry.
2. **macOS** `apps/macos/Riot.xcodeproj/project.pbxproj` — same, but the path is prefixed `../ios/Riot/<File>.swift` (sources) / `../ios/RiotTests/<File>.swift` (tests) — mirror `ConferenceShellView.swift` at :10 and the shared `RiotTests` refs (12 already present). Target names are the `-macOS` variants (`RiotKit-macOS`).
3. **Verify** by building the macOS project (`xcodebuild build -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS`) — a missing macOS entry surfaces as a compile/link error, not a silent drop.
4. **Shared-checkout:** if a sibling session holds a pbxproj, do NOT interleave — either coordinate or use the temp-index/`git add -p` pathspec technique; never a broad `git add -A`.

---

## What is DEFERRED (explicitly out of Rung 2)

Rung 2 lands the **skeleton and routing**; the detail *renders* are later rungs (spec §7):

- **Rung 3 — followed-site detail render.** `FollowedSiteDetailPlaceholder` is replaced by the Unit 4 `ResolvedCompositeSite` render (Editorial/Comments/Wire) + non-spoofable trust-tier chrome (§4.1) + Follow/Unfollow. The composite render FFI (`resolve_composite_site`) is now on main (per Rung 1's roadmap note), so Rung 3 is unblocked. **Rung 2 renders only the placeholder.**
- **Rung 4 — "your space" personal home.** `YourSpaceDetailPlaceholder` is replaced by the profile/drafts/posts/settings detail; **Personal-tier assignment** (which space is `.personal`) is Rung 4 — so in Rung 2 the Your space tier is empty/absent unless a `.personal` row already exists. `decide()` extension to restore a followed/personal last-active selection is deferred here.
- **Rung 5 — Unit 6 obligations.** Real `follow_site(ticket)` (so the Following tier actually populates on-device — until then `list_followed_sites()` returns empty on a shipped build, and the tier only renders under the pure-model/stub tests), QR gen + camera scan, the mandatory **seizure disclosure** pinned to mint-masthead (§4.4), writer expired-cap warning, compose-time `require:arti` notice, and the three-distinct-actions Add/Join/**Follow** split (§4.2 — Rung 2 pins Join/Create; Follow-a-site is Rung 5).
- **§3.2 scale polish** (collapsible tier groups, activity-desc ordering, unread surfacing) — Rung 2 lands a pinned search field + pinned Join/Create; ordering/collapse polish is a follow-up.
- **macOS nested-split-view → single 3-column** layout polish (§3) — accepted skeleton tradeoff, Rung 2 follow-up.

---

## Self-review — spec §3 / §4 / §7 requirements mapped to tasks

| Spec requirement | Landed in | Status |
|---|---|---|
| §3 two-pane anchor; space list on the left, detail on the right | 2.2 (outer `NavigationSplitView`) | ✓ |
| §3 macOS/iPad both panes; phone drill-in | 2.2 (native split-view collapse) | ✓ (nested-mac polish deferred) |
| §3 launch-restore last-active space via `CommunityReturnOutcome.decide` | 2.2 `restoreLastActiveSpace()` + `SpaceShellTests` | ✓ (communities; followed/personal extension → Rung 3/4) |
| §3.1 single row-state vocabulary, icon **and** text, never colour alone | 2.0 `SpaceRowState` + `SpaceListModelTests` | ✓ |
| §3.1 require:arti "requires Tor — unavailable" **at the row** (S1) | 2.0 `.transportBlocked.label` | ✓ (fields present; true path Rung 5) |
| §3.1 quarantined space stays visible + selectable | 2.0 (`.quarantined` selectable) / 2.2 | ✓ |
| §3.1 pendingFirstSync one rule (reuse `isPendingFirstSync`) | 2.0 (reuses `CommunityChooserRow.isPendingFirstSync`) | ✓ |
| §3.3 list-as-nav a11y: rows announce tier+name+state | 2.0 `accessibilityLabel` / 2.3 `contentDescription` + tests | ✓ |
| §4 by-kind detail routing (community/followed/your space) | 2.2 `SpaceDetailRoute.forTier` + placeholders | ✓ (renders → 3/4) |
| §4 cross-kind chrome contract (title + tier badge + relationship control) | Rung 3/4 renders; 2.2 placeholders carry a tier-badge header **stub** now | ✓ (contract lands with renders; header slot stubbed now) |
| §3.2 search field = pure name-filter over the grouped list | 2.0 `filteredSpaceList(_:query:)` + `SpaceListModelTests` filter test; 2.2 `TextField` binds to it | ✓ |
| §4 community detail = Home/People/Nearby/Tools relocated **verbatim** | 2.2 (`CommunityShellView` unchanged) | ✓ |
| §6.5 shell routes by core-assigned tier only, no policy in shell | 2.0/2.1 (core lists → pure merge) | ✓ |
| §7 space list as the ROOT (replace modal chooser) | 2.2 (persistent pane supersedes `CommunityChooserView`) | ✓ |
| §7 merge `list_communities` + `list_followed_sites` into tiered groups | 2.0 `groupedSpaceList` + 2.1 plumbing | ✓ |
| §7 Tools OFF the top level | 2.2 `SpaceShellRoot.topLevelSlots` excludes `.tools` + test | ✓ |
| §7 increment sub-ladder, each landable + green | 2.0→2.3 (four commits, TDD) | ✓ |
| §7/§10 pbxproj both-projects for every new Swift file | pbxproj recipe in 2.0/2.2 | ✓ |
| §2/§6 Rung-1 core consumed, no new core/FFI logic | (Rung 1 on main `ae9ec47`; Rung 2 Swift/Kotlin only) | ✓ |
| iOS exhaustive-`switch` heal (`plainLabel` `.following`/`.personal`; regen-forced twin of the Android `when`) | 2.0 Step 0 | ✓ (verified only landed iOS switch lacking a `default`) |
| Android exhaustive-`when` heal (Kotlin analogue of Rung 1 match note) | 2.3 Step 1–3 | ✓ |
| Android `SPACE_LIST` root label distinct from existing `SPACES("Spaces")` | 2.3 (`"Your Spaces"` label) | ✓ |
| Android launch-restore reuses existing `CommunityReturnOutcome.decide` mirror | 2.3 (no new restore logic) | ✓ (communities; followed/personal → Rung 3/4) |

**Known partials (by design):** the Following tier is empty on a shipped build until Rung 5's real `follow_site(ticket)` — proven now only via the pure model + stubs; the followed-site and your-space **detail renders** are placeholders (Rungs 3/4). These are the deferrals above, not gaps.
