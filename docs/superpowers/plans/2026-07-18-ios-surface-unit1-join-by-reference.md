# iOS Surface — Unit 1: Join by link / QR — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Let a user follow an existing community by pasting a `riot://newswire/join/v1/…` link or scanning its QR — the #1 dead-end today (join = Nearby only, which is red on main). Also fix the community chooser's dead Create/Find-nearby no-ops.

**Architecture:** A pure `JoinReferenceModel` value/observable type owns decode + validation + duplicate-detection + camera-permission state (unit-testable without a camera). `QRScannerView` (new AVFoundation, riot://-only, length-bounded, torn down on dismiss) feeds it a scanned string. `JoinByReferenceSheet` presents paste/scan, an honest pre-sync preview (namespace only — the share ref carries no title), and commits the join via the existing FFI. Reused from Launch + Chooser. No new FFI, no `uniffi::Record`.

**Tech stack:** Swift 6 / SwiftUI, AVFoundation (new), CoreImage (QR render is Unit 2 — scan only here), XCTest. Design: `docs/superpowers/specs/2026-07-18-ios-surface-built-capabilities-design.md` §3 + §7.

**Shared-checkout:** both `apps/ios/Riot.xcodeproj/project.pbxproj` + `apps/macos/Riot.xcodeproj/project.pbxproj` are hand-edited and serialize all Swift-file additions — **claim them in COLLABORATION.md before editing; no unit that adds Swift files runs while either pbxproj is dirty**. Pathspec commits; absolute `git`/`grep`.

---

## Ground truth (verified)

- **Decode FFI (exists, tested):** `newswireDecodeShareReference(encoded: String) throws -> NewswireShareReference` where `NewswireShareReference { namespaceId, descriptorEntryId, contentDigest, encoded }` (all hex `String`; `contentDigest.count == 64`; `encoded.hasPrefix("riot://newswire/join/v1/")`). Malformed input throws (`NewswireShareTests.testDecodeRejectsMalformedReference`).
- **Join commit (exists):** the app joins via `try profile.joinPublicSpace(space: PublicSpace(namespaceId:, title:, isPublic: true), wrappingKey:)` inside `withWrappingKey(from: keyStore)` (`ProfileRepository.swift:288-293`). The share ref carries **no title** → pass an empty/placeholder title; the real name arrives on first sync (honest "pending first sync"). Confirm whether a dedicated redeem path exists; if not, `joinPublicSpace(namespace)` is the join.
- **Duplicate detection:** `listCommunities() -> [CommunityRow]` (`ProfileRepository.swift:704`); `CommunityRow.namespaceId`. `switchToCommunity(namespaceID:)` (`:707`) to route to an already-joined one.
- **Entry points:** `LaunchView` buttons (`ConferenceShellView.swift:90-108`, pattern `Button(...) { model.<action>() }.buttonStyle(.riotPrimary|.riotSecondary).accessibilityIdentifier(...)`); chooser presented as `CommunityChooserView(model: model)` (`ConferenceShellView.swift:351`) — its `onCreate`/`onFindNearby` default to `{}` (dead no-ops, `CommunityChooser.swift:183-190`, buttons `:207-212`).
- **Sheet + recovery patterns:** `YourProfileSheet` (`ConferenceShellView.swift:696`, `@ObservedObject var model`, `let onClose`, `ScrollView{VStack}`, `.riotHeader`, `.toolbar` Done). `permissionRecoveryCard` (`:884`) is the exact camera-denied analog — copy + `settingsURL` live in a testable `NearbyPermissionRecovery` value type; mirror with a `CameraPermissionRecovery` type.
- **Test harness:** hostless XCTest, `@testable import RiotKit`, `openLocalProfile()` + real FFI (`NewswireShareTests.swift:80`); pure view-models tested directly (`CommunityChooserTests`). Fixtures via `Bundle(for: Self.self).url(forResource:)`.
- **No AVFoundation anywhere in the app today** — `QRScannerView` + `NSCameraUsageDescription` are 100% new.
- **pbxproj registration** (both files, hand-authored fixed IDs `A0X…`): per new file — 1 PBXFileReference, N PBXBuildFile (one per target), add build-file id to each target's PBXSourcesBuildPhase `files=()` and to the `Riot` PBXGroup `children`. Both projects reference iOS sources via `path = ../ios/Riot/…`.

---

## Task 1: `JoinReferenceModel` — pure decode/validate/duplicate (no camera)

**Files:** Create `apps/ios/Riot/JoinReferenceModel.swift`; Test `apps/ios/RiotTests/JoinReferenceTests.swift`

- [ ] **Step 1: Failing test.**
```swift
@testable import RiotKit
import XCTest
final class JoinReferenceTests: XCTestCase {
    func testDecodeProducesHonestPreviewWithNoTitle() throws {
        // real ref via the FFI (mirror NewswireShareTests): create space -> mint ref
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Riverside", summary: "s", languages: ["en"], geographicTags: [], topicTags: [], editorialRoster: []))
        let ref = try profile.newswireShareReference(spaceDescriptorEntryId: space.entryId)

        let model = JoinReferenceModel()
        let preview = try model.preview(fromPastedString: ref.encoded)   // decodes
        XCTAssertEqual(preview.namespaceIdHex, ref.namespaceId)
        XCTAssertNil(preview.title, "share ref carries no title; UI must not fabricate one")
        XCTAssertFalse(preview.shortNamespace.isEmpty)
    }
    func testMalformedStringSurfacesActionableError() {
        let model = JoinReferenceModel()
        XCTAssertThrowsError(try model.preview(fromPastedString: "https://example.com/nope"))
        XCTAssertThrowsError(try model.preview(fromPastedString: "riot://newswire/join/v1/abc"))
    }
    func testNonRiotScannedPayloadRejected() {
        let model = JoinReferenceModel()
        XCTAssertThrowsError(try model.preview(fromScannedString: "WIFI:S:foo;;")) // scan path: riot:// only
    }
    func testTooLongPayloadRejected() {
        let model = JoinReferenceModel()
        let huge = "riot://newswire/join/v1/" + String(repeating: "a", count: 5000) // > maxLen (4096)
        XCTAssertThrowsError(try model.preview(fromScannedString: huge)) { error in
            XCTAssertEqual(error as? JoinReferenceError, .tooLong)
        }
        XCTAssertThrowsError(try model.preview(fromPastedString: huge)) { error in
            XCTAssertEqual(error as? JoinReferenceError, .tooLong)
        }
    }
    func testDuplicateJoinIsDetected() throws {
        let model = JoinReferenceModel()
        let existing = [CommunityRowStub(namespaceId: "abc123")]  // or a real CommunityRow
        XCTAssertTrue(model.isAlreadyJoined(namespaceIdHex: "abc123", within: existing.map(\.namespaceId)))
        XCTAssertFalse(model.isAlreadyJoined(namespaceIdHex: "zzz999", within: existing.map(\.namespaceId)))
    }
}
```

- [ ] **Step 2: Run → FAIL** (`JoinReferenceModel` undefined). `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/JoinReferenceTests`

- [ ] **Step 3: Implement `JoinReferenceModel.swift`.**
```swift
import Foundation

public struct JoinPreview: Equatable {
    public let namespaceIdHex: String
    public let descriptorEntryIdHex: String
    public let contentDigestHex: String
    public let encoded: String
    public var title: String? { nil }               // share ref carries no title (anti-spoof)
    public var shortNamespace: String { String(namespaceIdHex.prefix(8)) + "…" }
}

public enum JoinReferenceError: Error, Equatable { case notARiotJoinLink, decodeFailed, tooLong }

public final class JoinReferenceModel {
    private static let scheme = "riot://newswire/join/"
    private static let maxLen = 4096
    public init() {}

    /// Paste path: decode via the FFI; any decode failure → actionable error.
    public func preview(fromPastedString s: String) throws -> JoinPreview {
        let trimmed = s.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count <= Self.maxLen else { throw JoinReferenceError.tooLong }
        let ref: NewswireShareReference
        do { ref = try newswireDecodeShareReference(encoded: trimmed) }
        catch { throw JoinReferenceError.decodeFailed }
        return JoinPreview(namespaceIdHex: ref.namespaceId, descriptorEntryIdHex: ref.descriptorEntryId,
                           contentDigestHex: ref.contentDigest, encoded: ref.encoded)
    }

    /// Scan path: enforce riot:// scheme + length BEFORE decode (hostile QR input).
    public func preview(fromScannedString s: String) throws -> JoinPreview {
        guard s.count <= Self.maxLen else { throw JoinReferenceError.tooLong }
        guard s.hasPrefix(Self.scheme) else { throw JoinReferenceError.notARiotJoinLink }
        return try preview(fromPastedString: s)
    }

    public func isAlreadyJoined(namespaceIdHex: String, within existing: [String]) -> Bool {
        existing.contains(namespaceIdHex)
    }
}
```
> `newswireDecodeShareReference` + `NewswireShareReference` are RiotKit-visible generated symbols. If the test needs a `CommunityRow` stub, use a small local struct or a real `CommunityRow` from `listCommunities()`.

- [ ] **Step 4: Run → PASS** (all 4 tests). **Step 5: Commit** `apps/ios/Riot/JoinReferenceModel.swift` + test (pbxproj registration in Task 5).

---

## Task 2: `QRScannerView` (AVFoundation, hardened)

**Files:** Create `apps/ios/Riot/QRScannerView.swift`, `apps/ios/Riot/CameraPermissionRecovery.swift`; Test `apps/ios/RiotTests/CameraPermissionTests.swift`

- [ ] **Step 1: Failing test** (permission-state + recovery copy are the testable part; the live capture is not unit-tested):
```swift
final class CameraPermissionTests: XCTestCase {
    func testRecoveryCarriesCameraCopyAndSettingsURL() {
        XCTAssertTrue(CameraPermissionRecovery.message.localizedCaseInsensitiveContains("camera"))
        XCTAssertNotNil(CameraPermissionRecovery.settingsURL)     // UIApplication.openSettingsURLString
        XCTAssertFalse(CameraPermissionRecovery.message.localizedCaseInsensitiveContains("bluetooth"))
    }
}
```

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement.** `CameraPermissionRecovery` mirrors `NearbyPermissionRecovery` (a value type owning `message: String` + `settingsURL: URL?` from `UIApplication.openSettingsURLString`). `QRScannerView`: `UIViewControllerRepresentable` wrapping an `AVCaptureSession` with an `AVCaptureMetadataOutput` (`metadataObjectTypes = [.qr]`); on a `.qr` object, read `stringValue`, hand it to a `onScanned: (String) -> Void` callback; **stop the session on `dismiss`/`viewWillDisappear` and on background** (no lingering capture). Gate start on `AVCaptureDevice.authorizationStatus(for: .video)` (request if `.notDetermined`; `.denied`/`.restricted` → show `CameraPermissionRecovery` card, never start capture). The scheme/length filtering is NOT here — the raw string goes to `JoinReferenceModel.preview(fromScannedString:)` which enforces riot://-only + length.

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** the two files + test.

---

## Task 3: `JoinByReferenceSheet` — paste/scan → preview → join

**Files:** Create `apps/ios/Riot/JoinByReferenceSheet.swift`; Test `apps/ios/RiotTests/JoinByReferenceSheetTests.swift`

- [ ] **Step 1: Failing test** (drive the commit path against real FFI; assert honest pending-sync + duplicate routing):
```swift
func testJoiningAReferenceCreatesAPendingMemberCommunity() throws {
    // Build a ref in profile A (or a fixture), decode + join in profile B, assert it lands as a member row.
    // Assert: no fabricated title shown pre-sync; community appears in listCommunities as a member.
}
func testJoiningAnAlreadyJoinedReferenceSwitchesInsteadOfDuplicating() throws {
    // Join once, then join same ref again -> routes to the existing community (switch), not a 2nd row.
}
```

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement** the sheet (mirror `YourProfileSheet` chrome: `@ObservedObject var model`, `let onClose`, `.riotHeader(eyebrow: "Follow", "Join with a link")`, `.toolbar` Done). Body: a segmented `Picker` (Paste / Scan) → paste `TextField` or `QRScannerView(onScanned:)`. **macOS build (gate r1):** `QRScannerView` is `#if os(iOS)` — so the **Scan segment and the `QRScannerView(onScanned:)` embed must ALSO be `#if os(iOS)`**; on macOS the sheet shows **paste-only** (Picker collapses to the paste field, no Scan option). This keeps the macOS app BUILD SUCCEEDED (§Task 6). Then: on input → `JoinReferenceModel.preview(from…)`; render the **honest preview** ("Join community `\(preview.shortNamespace)`? Its name and posts arrive on first sync.", namespace in monospace `IdentifierRow` treatment); error state renders `JoinReferenceError` as actionable copy (invalid link / not a Riot link / too long); camera-denied → `CameraPermissionRecovery` card (paste still available). **Confirm** → if `model.isAlreadyJoined(preview.namespaceIdHex, within: listCommunities().map(\.namespaceId))` → `switchToCommunity(namespaceID:)` + dismiss; else call a new `model.joinByReference(preview)` on `RiotAppModel` that does `joinPublicSpace(PublicSpace(namespaceId: preview.namespaceIdHex, title: "", isPublic: true), wrappingKey:)` and routes into the shell showing the **pending-first-sync** state (lead with the honest explanation; Nearby is a secondary option, NOT the headline — nearby is red). Add the thin `RiotAppModel.joinByReference(_:)` method (business logic stays in the FFI/repository; the model just forwards).

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** the sheet + test.

---

## Task 4: Entry points + fix the chooser dead no-ops

**Files:** Modify `apps/ios/Riot/ConferenceShellView.swift`, `apps/ios/Riot/CommunityChooser.swift`

- [ ] **Step 1: Failing test** — a `CommunityChooserTests` case asserting the chooser exposes a working Create / Find-nearby / Join-another action (no dead `{}`), e.g. via a presenter flag or by asserting the wired closures are non-nil / invoke the model. Plus a `LaunchView`-level assertion that a "Join with a link/QR" affordance is present.

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement.**
  - **LaunchView** (`ConferenceShellView.swift` ~:90): add a third button `Button("Join with a link or QR") { showJoinSheet = true }.buttonStyle(.riotSecondary).accessibilityIdentifier("launch-join-by-reference")` + a `.sheet(isPresented: $showJoinSheet) { JoinByReferenceSheet(model: model, onClose: { showJoinSheet = false }) }`.
  - **Chooser call site** (`ConferenceShellView.swift:351`): pass real closures — `CommunityChooserView(model: model, onCreate: { /* present create */ }, onFindNearby: { model.select(.nearby) })` — fixing the dead `{}` defaults (the gate-r1 dead-end). Add a **"+ Join another" row** in the chooser's action section (`CommunityChooser.swift:207`) that presents `JoinByReferenceSheet`.
  - Wire `onCreate` to the existing create flow (present the create field/sheet the LaunchView uses) so it is no longer a no-op.

- [ ] **Step 4: Run → PASS** + existing `CommunityChooserTests` stay green. **Step 5: Commit** both files.

---

## Task 5: Info.plist camera key + pbxproj registration (BOTH projects)

**Files:** Modify `apps/ios/Riot/Info.plist` (or the target's plist), `apps/ios/Riot.xcodeproj/project.pbxproj`, `apps/macos/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1** Add `NSCameraUsageDescription` = "Scan a community's QR code to follow it." to the iOS app target's Info.plist. (macOS: only if the scan view compiles there; QR scan is iOS-only — guard `QRScannerView` with `#if os(iOS)` so macOS builds without a camera target.)
- [ ] **Step 2** Register the 4 new Swift files (`JoinReferenceModel.swift`, `QRScannerView.swift`, `CameraPermissionRecovery.swift`, `JoinByReferenceSheet.swift`) in **both** pbxproj: fixed-ID PBXFileReference + per-target PBXBuildFile + add to each target's PBXSourcesBuildPhase `files=()` + the `Riot` PBXGroup `children`. `JoinReferenceModel`/`CameraPermissionRecovery` go into RiotKit (tested); the views into the app + RiotKit as the neighbors do. Follow the `A0X…` id convention.
- [ ] **Step 3** `plutil -lint` both pbxproj → OK. **Step 4** Commit plist + both pbxproj.

---

## Task 6: Build + full test both platforms
- [ ] iOS: `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'` — new tests green, full RiotKit only the known-red Bonjour.
- [ ] iOS app + macOS app **BUILD SUCCEEDED** (`#if os(iOS)` around the camera view keeps macOS building).
- [ ] Commit any fixups.

---

## Self-Review
- **Spec coverage (§3 reader-join + §7 chooser no-ops):** paste + scan ✅ T1/T2/T3; riot://-only + length-bound + teardown ✅ T2 + T1(scan path); honest no-title preview ✅ T1/T3; duplicate-join → switch ✅ T1/T3; pending-first-sync honest, Nearby secondary ✅ T3; camera-denied recovery + paste fallback ✅ T2/T3; **chooser dead Create/Find-nearby wired** ✅ T4; entry points Launch + Chooser ✅ T3/T4; camera key + both pbxproj ✅ T5. No new FFI ✅ (decode + join + list all exist).
- **Placeholder scan:** the join-commit method (T3) is specified as `joinPublicSpace(namespace)` with a flagged "confirm whether a redeem path exists" — the one API to verify on implement; the fixtures in T3 tests are sketched (build a ref in one profile, join in another) — the implementer fleshes the two-profile harness mirroring `NewswireShareTests`.
- **Type consistency:** `JoinPreview`/`JoinReferenceModel`/`JoinReferenceError` used consistently across T1/T3; `CameraPermissionRecovery` (T2) rendered in T3; `newswireDecodeShareReference`→`NewswireShareReference` field names (namespaceId/descriptorEntryId/contentDigest/encoded) match the generated binding.
- **Dependency order:** T1 (model) → T2 (scanner) → T3 (sheet, uses both) → T4 (entry points, uses sheet) → T5 (registration) → T6 (build). All within one unit; pbxproj claimed for the whole unit.
