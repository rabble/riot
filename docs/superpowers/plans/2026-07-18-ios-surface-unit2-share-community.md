# iOS Surface — Unit 2: "Share this community" (generate link + QR) — Implementation Plan


**Plan-review gate: PASSED (Feasibility + Scope + Completeness, 2026-07-18).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Give a member of the active community a "Share this community" affordance in the Community settings sheet that turns the core's digest-bound join reference into (a) the system **Share sheet** for the `riot://newswire/join/v1/…` link and (b) a **locally-rendered QR** someone can scan with Unit 1's scanner. This is the *generate* half of join — the counterpart to Unit 1's *consume* half. Pure-Swift, **no new FFI**.

**Architecture:** A pure `QRImageRenderer` (CoreImage `CIQRCodeGenerator` → `CGImage`, no UIKit, cross-platform) renders any string to a raster, unit-testable without a screen. A pure `ShareCommunityModel` value type turns the active community's descriptor entry id into a `ShareCommunityContent` (`.shareable(link:)` or an honest `.unavailable`) by calling an injected resolver (the existing `newswireShareReference` FFI) — so it is testable against the real core with no view. `ShareCommunitySheet` (new SwiftUI view, mirrors `YourProfileSheet`/`CommunitySettingsSheet` chrome) consumes both: it shows the mono link + a `ShareLink`, and — `#if os(iOS)` — the QR image. Wired into `CommunitySettingsSheet` behind a "Share this community" button. No business logic in the app: the reference and its anti-substitution digest come from core.

**Tech stack:** Swift 6 / SwiftUI, CoreImage (new — `CIFilter.qrCodeGenerator()` via `CoreImage.CIFilterBuiltins`), `ShareLink` (iOS 16+/macOS 13+ — both deployment targets clear it: iOS 17.0, macOS 14.0), XCTest. Design: `docs/superpowers/specs/2026-07-18-ios-surface-built-capabilities-design.md` §3 "Generate side" + §8 Unit 2.

**Shared-checkout:** both `apps/ios/Riot.xcodeproj/project.pbxproj` + `apps/macos/Riot.xcodeproj/project.pbxproj` are hand-edited and serialize all Swift-file additions — **claim them in COLLABORATION.md before editing; no unit that adds Swift files runs while either pbxproj is dirty**. Pathspec commits only; absolute `/opt/homebrew/bin/git` / `/usr/bin/grep`.

---

## Ground truth (verified)

- **Mint FFI (exists, tested):** `RiotProfileRepository.newswireShareReference(spaceDescriptorEntryID: String) throws -> NewswireShareReference` (`apps/ios/Riot/Core/ProfileRepository.swift:1097`), forwarding to the generated `profile.newswireShareReference(spaceDescriptorEntryId:)`. `NewswireShareReference { namespaceId, descriptorEntryId, contentDigest, encoded }` — `encoded.hasPrefix("riot://newswire/join/v1/")`, `contentDigest.count == 64` (proven in `apps/ios/RiotTests/NewswireShareTests.swift:80-114`). **No new FFI.**
- **Active community's descriptor id:** `CommunityContext.newswireDescriptorEntryID: String?` (`apps/ios/Riot/CommunityShell.swift:21`) — the signed `SpaceDescriptorV1` entry id, `nil` before it is known. It is re-derived on **every** `reload()` from `listCommunities().first { $0.namespaceId == … }?.descriptorEntryId` (`apps/ios/Riot/AppModel.swift:501-503`) and threaded into `CommunityContext` (`AppModel.swift:704`) — so for a **joined or switched** community it is populated once the registry knows the descriptor, and is `nil` only pre-first-sync. `CommunityRow.descriptorEntryId` is the registry field (`ProfileRepository.swift:704-709`, `listCommunities()`/`activeCommunity()`).
- **Repository handle on the model:** `RiotAppModel.profileRepository: RiotProfileRepository?` (`apps/ios/Riot/AppModel.swift:272`).
- **Settings sheet to edit:** `CommunitySettingsSheet` (`apps/ios/Riot/ConferenceShellView.swift:744-794`) — `@ObservedObject var model: RiotAppModel`, `let community: CommunityContext`, `let onLeave`, `let onClose`; body is `ScrollView { VStack(alignment:.leading, spacing:16) { RiotCard{About…}; DisclosureGroup{Technical}; Button("Leave …") } }`, `.riotHeader(eyebrow: "Community", …)`, `.toolbar { ToolbarItem(.confirmationAction) { Button("Done", action: onClose) } }`. Presented at `ConferenceShellView.swift:331-337`. Chrome helpers used here and reusable: `RiotCard`, `.riotHeader(eyebrow:_:)`, `RiotTheme.ink/inkSoft(for:)`, `.riot(.body|.mono, size:relativeTo:)`. **`IdentifierRow` is `private struct` in ConferenceShellView (`:1073`) — NOT reusable across files;** render the link as a mono `Text` with `.textSelection(.enabled)` (same treatment `YourProfileSheet` uses at `:708-712`).
- **QR render is 100% new:** `/usr/bin/grep` for `CIFilter`/`CIQRCodeGenerator`/`CoreImage` across `apps/ios` + `apps/macos` matches **only** derived-data (`apps/ios/build/…`) — nothing in app source. `CoreImage`/`CIFilter.qrCodeGenerator()` are available on **both** iOS and macOS; `CGImage` is platform-neutral, so the renderer needs **no** platform guard and its test runs on both. Only the sheet's QR *display* is `#if os(iOS)` (design §3 frames QR as the iOS scan counterpart; macOS shows link + `ShareLink` only).
- **Share sheet:** `ShareLink` — no prior use in the app (`/usr/bin/grep ShareLink apps/ios/Riot apps/macos` → none). `String` conforms to `Transferable`, so `ShareLink(item: link)` shares the canonical string verbatim (what the QR encodes and what Unit 1 pastes) — no `URL(string:)` parse risk on the custom `riot://` scheme.
- **Test harness:** hostless XCTest, `@testable import RiotKit`, `try openLocalProfile()` + real FFI; mint a reference exactly as `NewswireShareTests.testHeldDescriptorMintsVerifiableShareReference` does — `profile.createNewswireSpace(input: NewswireSpaceInput(...))` → `space.entryId` → `profile.newswireShareReference(spaceDescriptorEntryId: space.entryId).encoded`. `newswireDecodeShareReference(encoded:)` round-trips it.
- **pbxproj registration** (both files, hand-authored fixed ids): per Swift source file — 1 `PBXFileReference` (iOS `path = Riot/…`; macOS `path = ../ios/Riot/…`), **two** `PBXBuildFile` on iOS (RiotKit `…11` + app `…21`) / one on macOS RiotKit, each build-file id added to its target's `PBXSourcesBuildPhase` `files=()` and the file-ref id to the `Riot`/`RiotKit` `PBXGroup` `children`. Example verified: `CommunityChooser.swift` fileref `A0F000000000000000000010`, iOS buildfiles `A0F000000000000000000011` (in sources phase `A00000000000000000000030` = RiotKit) + `A0F000000000000000000021` (phase `A00000000000000000000031` = app); macOS fileref `F0F000000000000000000010` + buildfile `F0F000000000000000000011`. iOS `Riot` group = `A00000000000000000000002`; iOS `RiotTests` group = `A00000000000000000000003`, its sources phase = `A00000000000000000000031`… (test buildfiles live there, e.g. `NewswireShareTests` fileref `A0D0000000000000000000F1`). Next free source-letter is `A0G…`. Note: `NewswireShareTests` is registered **iOS-only**; the macOS RiotKitTests-macOS target holds a subset.

---

## Task 1: `QRImageRenderer` — pure CoreImage QR raster (no view, cross-platform)

**Files:** Create `apps/ios/Riot/QRImageRenderer.swift`; Test `apps/ios/RiotTests/QRImageRendererTests.swift`

- [ ] **Step 1: Failing test.**
```swift
import XCTest
import CoreGraphics
@testable import RiotKit

final class QRImageRendererTests: XCTestCase {
    func testRendersANonEmptyImageForAValidJoinLink() throws {
        let link = "riot://newswire/join/v1/00112233445566778899aabbccddeeff"
        let image = try XCTUnwrap(
            QRImageRenderer.makeQRCode(from: link),
            "a valid join link must render a QR raster"
        )
        XCTAssertGreaterThan(image.width, 0)
        XCTAssertGreaterThan(image.height, 0)
    }

    func testBlankInputRendersNothing() {
        XCTAssertNil(QRImageRenderer.makeQRCode(from: ""))
        XCTAssertNil(QRImageRenderer.makeQRCode(from: "   \n  "))
    }

    func testLongerPayloadIsAtLeastAsDense() throws {
        // A denser payload needs at least as many QR modules => not a smaller raster.
        let short = try XCTUnwrap(QRImageRenderer.makeQRCode(from: "riot://newswire/join/v1/aa"))
        let long = try XCTUnwrap(
            QRImageRenderer.makeQRCode(from: "riot://newswire/join/v1/" + String(repeating: "a", count: 200))
        )
        XCTAssertGreaterThanOrEqual(long.width, short.width)
    }
}
```

- [ ] **Step 2: Run → FAIL** (`QRImageRenderer` undefined).
`xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/QRImageRendererTests`

- [ ] **Step 3: Implement `QRImageRenderer.swift`.**
```swift
import CoreImage
import CoreImage.CIFilterBuiltins
import CoreGraphics

/// Renders a string into a QR-code raster locally — no network, no external
/// service, no fabricated content. Pure CoreImage (`CIQRCodeGenerator`), so it
/// compiles and runs on BOTH iOS and macOS and is unit-testable without a screen.
/// Returns a platform-neutral `CGImage`; the SwiftUI layer wraps it in
/// `Image(decorative:scale:)`. A blank input or a CoreImage failure yields `nil`,
/// so the caller renders an honest "nothing to share yet" state, never a broken
/// image.
public enum QRImageRenderer {
    /// `correctionLevel` "M" (~15% recovery) balances density against scan
    /// resilience for a `riot://` join payload; `scale` nearest-neighbour-magnifies
    /// the raw ~1-module-per-pixel matrix so the code is crisp at display size.
    public static func makeQRCode(from string: String, scale: CGFloat = 12) -> CGImage? {
        let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let payload = trimmed.data(using: .utf8) else { return nil }

        let filter = CIFilter.qrCodeGenerator()
        filter.message = payload
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }

        let scaled = output.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        return CIContext().createCGImage(scaled, from: scaled.extent)
    }
}
```
> `CIFilter.qrCodeGenerator()` (from `CoreImage.CIFilterBuiltins`) exposes `message: Data` + `correctionLevel: String`. `CGImage` needs no `#if canImport(UIKit)` guard — CoreImage + CGImage are both cross-platform; only the sheet's QR *display* is iOS-gated (Task 3).

- [ ] **Step 4: Run → PASS** (3 tests). **Step 5: Commit** (pbxproj registration in Task 4):
```bash
/opt/homebrew/bin/git add apps/ios/Riot/QRImageRenderer.swift apps/ios/RiotTests/QRImageRendererTests.swift
/opt/homebrew/bin/git commit -m "feat(ios): QRImageRenderer — local CoreImage QR raster for share (Unit2)"
```

---

## Task 2: `ShareCommunityModel` — descriptor id → shareable link (real FFI, no view)

**Files:** Create `apps/ios/Riot/ShareCommunityModel.swift`; Test `apps/ios/RiotTests/ShareCommunityModelTests.swift`

- [ ] **Step 1: Failing test** (drive the mint path against real FFI; assert an honest link + `.unavailable` fallbacks):
```swift
import XCTest
@testable import RiotKit

@MainActor
final class ShareCommunityModelTests: XCTestCase {
    private struct ResolverFailed: Error {}

    func testHeldCommunityProducesAShareableRiotLink() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Riverside Commons", summary: "A community newswire.",
            languages: ["en"], geographicTags: ["riverside"], topicTags: ["local"],
            editorialRoster: []))

        let model = ShareCommunityModel()
        let content = model.content(descriptorEntryID: space.entryId) { id in
            try profile.newswireShareReference(spaceDescriptorEntryId: id).encoded
        }

        guard case let .shareable(link) = content else {
            return XCTFail("a held descriptor must be shareable")
        }
        XCTAssertTrue(link.hasPrefix("riot://newswire/join/v1/"))
        // The shared link round-trips through the core decoder (anti-fabrication:
        // the string we hand out is the same one Unit 1 decodes back).
        XCTAssertEqual(try newswireDecodeShareReference(encoded: link).encoded, link)
        // And it renders a QR locally.
        XCTAssertNotNil(QRImageRenderer.makeQRCode(from: link))
    }

    func testUnknownDescriptorIsUnavailableNotAFabricatedLink() {
        let model = ShareCommunityModel()
        // nil descriptor id (a joined community before its descriptor is known)
        XCTAssertEqual(
            model.content(descriptorEntryID: nil, resolveEncoded: { _ in "x" }),
            .unavailable)
        // resolver throws (profile closed / descriptor not held) => unavailable, no crash
        XCTAssertEqual(
            model.content(descriptorEntryID: "deadbeef", resolveEncoded: { _ in throw ResolverFailed() }),
            .unavailable)
        // a resolver that returns a non-riot string is rejected (never shared)
        XCTAssertEqual(
            model.content(descriptorEntryID: "deadbeef", resolveEncoded: { _ in "https://evil.example/x" }),
            .unavailable)
    }
}
```

- [ ] **Step 2: Run → FAIL** (`ShareCommunityModel` undefined).
`xcodebuild test … -only-testing:RiotTests/ShareCommunityModelTests`

- [ ] **Step 3: Implement `ShareCommunityModel.swift`.**
```swift
import Foundation

/// The generate side of join, as a pure value the sheet renders. No view
/// dependency and no FFI dependency of its own — the caller injects a closure
/// that resolves a descriptor entry id to the canonical share-reference string
/// (`RiotProfileRepository.newswireShareReference(...).encoded`). That keeps this
/// testable against the real core with an `openLocalProfile()` and no UI, and
/// keeps the anti-substitution digest inside core where it belongs.
public enum ShareCommunityContent: Equatable {
    /// The community's descriptor id isn't known on this device yet (a freshly
    /// joined community before first sync) — nothing to share; show an honest note.
    case unavailable
    /// A canonical `riot://newswire/join/v1/...` link ready to share + encode.
    case shareable(link: String)
}

public struct ShareCommunityModel {
    public init() {}

    /// Resolve the share content for the active community. A nil descriptor id, a
    /// resolver that throws (profile closed / descriptor not held), or a resolver
    /// that returns a non-`riot://` string all collapse to `.unavailable` — never a
    /// crash, never a fabricated or foreign link.
    public func content(
        descriptorEntryID: String?,
        resolveEncoded: (String) throws -> String
    ) -> ShareCommunityContent {
        guard let id = descriptorEntryID,
              let encoded = try? resolveEncoded(id),
              encoded.hasPrefix("riot://newswire/join/v1/") else {
            return .unavailable
        }
        return .shareable(link: encoded)
    }
}
```

- [ ] **Step 4: Run → PASS** (2 tests). **Step 5: Commit:**
```bash
/opt/homebrew/bin/git add apps/ios/Riot/ShareCommunityModel.swift apps/ios/RiotTests/ShareCommunityModelTests.swift
/opt/homebrew/bin/git commit -m "feat(ios): ShareCommunityModel — descriptor id -> honest share link (Unit2)"
```

---

## Task 3: `ShareCommunitySheet` + wire into `CommunitySettingsSheet`

**Files:** Create `apps/ios/Riot/ShareCommunitySheet.swift`; Modify `apps/ios/Riot/ConferenceShellView.swift`

The sheet is SwiftUI chrome over the two already-tested pure types (Tasks 1–2), so its correctness rests on those tests + the both-platform build (Task 5) + no regression of existing `ConferenceShellView` tests — there is no bespoke view-model seam to unit-test here (same treatment the Unit 1 plan gave its view-only wiring). Add an `accessibilityIdentifier` on every state so a later UI test can reach it.

- [ ] **Step 1: Implement `ShareCommunitySheet.swift`.**
```swift
import SwiftUI

/// "Share this community" — the generate side of join (design §3). Mints the
/// community's digest-bound `riot://newswire/join/v1/...` reference from core
/// (`newswireShareReference`), offers it to the system Share sheet, and — on iOS —
/// renders a local QR for someone to scan with Unit 1's scanner. No new FFI and no
/// business logic here: the reference and its anti-substitution digest come from
/// core; this view only presents and encodes the canonical string.
struct ShareCommunitySheet: View {
    @ObservedObject var model: RiotAppModel
    let community: CommunityContext
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    private let builder = ShareCommunityModel()

    private var content: ShareCommunityContent {
        builder.content(descriptorEntryID: community.newswireDescriptorEntryID) { id in
            guard let repository = model.profileRepository else { throw RepositoryError.profileClosed }
            return try repository.newswireShareReference(spaceDescriptorEntryID: id).encoded
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                switch content {
                case .unavailable:
                    unavailableCard
                case let .shareable(link):
                    shareableCard(link: link)
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Community", "Share this community")
        .toolbar {
            ToolbarItem(placement: .confirmationAction) { Button("Done", action: onClose) }
        }
    }

    private var unavailableCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 8) {
                Text("Sharing becomes available once this community's descriptor is known on this device.")
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                Text("A community you just joined can be shared after it completes its first sync.")
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
        }
        .accessibilityIdentifier("share-community-unavailable")
    }

    private func shareableCard(link: String) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            RiotCard {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Anyone with this link or QR can follow \(community.name). Their device verifies its identity and posts from the link's digest — no name is taken on trust.")
                        .font(.riot(.body, size: 14, relativeTo: .body))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    Text(link)
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        .textSelection(.enabled)
                        .accessibilityIdentifier("share-community-link")
                    ShareLink(item: link) {
                        Label("Share link", systemImage: "square.and.arrow.up")
                    }
                    .buttonStyle(.riotPrimary)
                    .accessibilityIdentifier("share-community-sharelink")
                }
            }
            #if os(iOS)
            if let qr = QRImageRenderer.makeQRCode(from: link) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Scan to follow")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        Image(decorative: qr, scale: 1)
                            .interpolation(.none)
                            .resizable()
                            .scaledToFit()
                            .frame(maxWidth: 240)
                            .accessibilityIdentifier("share-community-qr")
                    }
                }
            }
            #endif
        }
    }
}
```
> `ShareLink(item: link)` shares the `String` verbatim (String conforms to `Transferable`) — no `URL(string:)` parse of the custom scheme. `Image(decorative:scale:)` takes the `CGImage`; `.interpolation(.none)` keeps the QR crisp. On macOS the `#if os(iOS)` block drops out → link + `ShareLink` only (design §3).

- [ ] **Step 2: Wire into `CommunitySettingsSheet`** (`ConferenceShellView.swift:744-794`). Add a presentation flag + button + sheet:
```swift
private struct CommunitySettingsSheet: View {
    @ObservedObject var model: RiotAppModel
    let community: CommunityContext
    let onLeave: () -> Void
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var showingTechnical = false
    @State private var showingShare = false      // NEW

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotCard { /* …existing About card, unchanged… */ }

                // NEW — generate side of join (design §3). Any member can share the
                // public join reference; no organizer gate.
                Button("Share this community") { showingShare = true }
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("share-community")

                DisclosureGroup(isExpanded: $showingTechnical) { /* …unchanged… */ }
                    .accessibilityIdentifier("community-technical-details")

                Button("Leave this community", role: .destructive, action: onLeave)
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("leave-community")
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Community", ShellIdentityDestination.communitySettings.label)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) { Button("Done", action: onClose) }
        }
        .sheet(isPresented: $showingShare) {      // NEW
            ShareCommunitySheet(model: model, community: community, onClose: { showingShare = false })
        }
    }
    // …eyebrow(_:) unchanged…
}
```

- [ ] **Step 3: Run → build + existing tests green.** iOS: `xcodebuild build -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'` (fails until Task 4 registers the file — expected; commit after Task 4). **Step 4: Commit** the sheet + the `ConferenceShellView` edit together with Task 4's pbxproj (a source file and its registration land atomically):
```bash
/opt/homebrew/bin/git add apps/ios/Riot/ShareCommunitySheet.swift apps/ios/Riot/ConferenceShellView.swift
# (staged; committed with Task 4)
```

---

## Task 4: pbxproj registration (BOTH projects) + macOS build guard

**Files:** Modify `apps/ios/Riot.xcodeproj/project.pbxproj`, `apps/macos/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: No Info.plist change** — Unit 2 uses no camera and `ShareLink` needs no usage key (unlike Unit 1's `NSCameraUsageDescription`). Confirm nothing to add.
- [ ] **Step 2: Register the 4 new Swift files** in **both** pbxproj, following the verified `A0F…`/`F0F…` convention (grep each candidate id first to confirm no collision, e.g. `/usr/bin/grep A0G000000000000000000010 apps/ios/Riot.xcodeproj/project.pbxproj`):
  - **Sources → RiotKit + app** (compiled into RiotKit on both platforms): `QRImageRenderer.swift`, `ShareCommunityModel.swift`, `ShareCommunitySheet.swift`.
    - iOS each: 1 `PBXFileReference` (`path = Riot/<name>`), 2 `PBXBuildFile` (RiotKit `…11` into phase `A00000000000000000000030`; app `…21` into phase `A00000000000000000000031`), file-ref id into `Riot` group `A00000000000000000000002` `children`. Suggested filerefs: `A0G…010` (QRImageRenderer), `A0H…010` (ShareCommunityModel), `A0I…010` (ShareCommunitySheet).
    - macOS each: 1 `PBXFileReference` (`path = ../ios/Riot/<name>`) + 1 `PBXBuildFile` into the macOS RiotKit sources phase (mirror where `F0F000000000000000000011` lives) + the `RiotKit` group `C00000000000000000000002` `children`. Suggested filerefs `F0G…010`, `F0H…010`, `F0I…010`.
  - **Tests → RiotTests target:** `QRImageRendererTests.swift`, `ShareCommunityModelTests.swift`.
    - iOS: `PBXFileReference` (`path = RiotTests/<name>`) + `PBXBuildFile` into the RiotTests sources phase `A00000000000000000000031`… (where `A0D0000000000000000000F2`-style test buildfiles live) + `RiotTests` group `A00000000000000000000003` `children`. Suggested filerefs `A0G…0F1`, `A0H…0F1`.
    - macOS: register in the `RiotKitTests-macOS` target too (both tests are cross-platform and run on macOS — they exercise `QRImageRenderer`/`ShareCommunityModel` with no iOS API) so macOS coverage includes them; mirror the macOS test-file registration pattern. If the macOS test target does not currently register `RiotTests` sources, keep them iOS-only (as `NewswireShareTests` is) and note it.
- [ ] **Step 3: `#if os(iOS)` guard already in place** (Task 3): only the QR `Image` block is iOS-gated; `QRImageRenderer`, `ShareCommunityModel`, and the link/`ShareLink` compile on macOS. macOS build shows link + Share only.
- [ ] **Step 4:** `plutil -lint apps/ios/Riot.xcodeproj/project.pbxproj apps/macos/Riot.xcodeproj/project.pbxproj` → both OK. **Step 5: Commit** both pbxproj with the staged Task 3 files:
```bash
/opt/homebrew/bin/git add apps/ios/Riot/ShareCommunitySheet.swift apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj apps/macos/Riot.xcodeproj/project.pbxproj
/opt/homebrew/bin/git commit -m "feat(ios): Share this community sheet (link + QR) wired into CommunitySettings (Unit2)"
```

---

## Task 5: Build + full test both platforms
- [ ] iOS tests: `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'` — the 5 new tests (Task 1 ×3, Task 2 ×2) green; full RiotKit only the known-red Bonjour two-peer test still red (see MEMORY: two-peer sync is RED on main).
- [ ] iOS app + macOS app **BUILD SUCCEEDED** (`#if os(iOS)` around the QR `Image` keeps macOS building; link + `ShareLink` present on both).
- [ ] macOS: `xcodebuild build -project apps/macos/Riot.xcodeproj -scheme RiotKit -destination 'platform=macOS'` (and the app scheme) → SUCCEEDED; RiotKitTests-macOS runs the two cross-platform tests if registered.
- [ ] Commit any fixups.

---

## Self-Review
- **Spec coverage (§3 "Generate side — Share this community" + §8 Unit 2):** "Share this community" affordance in Community settings ✅ T3 (`share-community` button → `ShareCommunitySheet`); `newswire_share_reference(active id)` → `riot://` string ✅ T2 (via `newswireShareReference(spaceDescriptorEntryID:)`, active id = `CommunityContext.newswireDescriptorEntryID`); iOS **Share sheet** for the link ✅ T3 (`ShareLink(item: link)`); locally-rendered **QR image** ✅ T1 (`QRImageRenderer`, CoreImage `CIQRCodeGenerator`) + T3 (`Image(decorative:)`, iOS-gated); no fabricated data — honest `.unavailable` when descriptor unknown ✅ T2/T3 (anti-dead-end invariant #2/#3); **no new FFI** ✅ (mint + decode both exist).
- **Placeholder scan:** none. All Swift/API is real: `CIFilter.qrCodeGenerator()`+`message`/`correctionLevel`, `ShareLink(item:String)`, `Image(decorative:scale:)`, `newswireShareReference(spaceDescriptorEntryID:)`→`.encoded`, `CommunityContext.newswireDescriptorEntryID`, `RiotAppModel.profileRepository`. pbxproj ids are suggested-with-collision-check, not invented blindly (verified `A0F`/`F0F` convention + real group/phase ids quoted). The one implementer decision: whether the macOS RiotKitTests-macOS target registers the two new test files (flagged in T4 Step 2 — both options specified, defaulting to iOS-only parity with `NewswireShareTests` if the macOS test target doesn't hold `RiotTests` sources).
- **Type consistency:** `ShareCommunityContent`/`ShareCommunityModel` used identically in T2/T3; `QRImageRenderer.makeQRCode(from:) -> CGImage?` consumed in T1 (test), T2 (test), T3 (view); `NewswireShareReference.encoded` (String) is the single currency across mint→QR→ShareLink→decode; `CommunityContext.newswireDescriptorEntryID: String?` is the descriptor source in T3.
- **Dependency order:** T1 (renderer) → T2 (model, uses renderer in test) → T3 (sheet, uses both + edits settings) → T4 (registration, atomic with T3's source) → T5 (build/test both). All within one unit; both pbxproj claimed for the whole unit.
- **Risks flagged:**
  1. **`ShareLink` availability — cleared.** iOS 16+/macOS 13+; deployment targets are iOS 17.0 / macOS 14.0 (verified in both pbxproj). Applying `.buttonStyle(.riotPrimary)` to `ShareLink` is supported (it renders as a button); if a Swift 6 style-conformance issue surfaces, drop the custom style — the default `ShareLink` chrome still ships the link.
  2. **QR testability — mitigated.** Reading the QR back (CIDetector) is flaky in a headless bundle, so tests assert `CGImage` non-nil + `width/height > 0` + monotonic density (denser payload ⇒ ≥ raster width), per the design's "assert size>0 else" guidance. The end-to-end scan is proven by Unit 1's scanner consuming the same canonical string, not re-decoded here.
  3. **`newswireDescriptorEntryID == nil` for a just-joined community** (pre-first-sync) — handled as the honest `.unavailable` state, not an error/blank (design §3 pending-first-sync; MEMORY: two-peer sync red on main means this state is real and must be honest).
  4. **QR on macOS is CoreImage-capable but display is `#if os(iOS)`** by design (§3 frames QR as the iOS scan counterpart; macOS shows link + Share). `QRImageRenderer` stays cross-platform so its test runs on macOS; un-gating the Mac QR display later is a one-line change.
  5. **Shared-checkout:** both `project.pbxproj` serialize Swift-file additions across sessions — claim in COLLABORATION.md; land T3+T4 as one atomic commit.
