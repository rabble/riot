# Remove the Dead Import Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the nonfunctional Import destination while leaving nearby-sync review in Connect unchanged.

**Architecture:** Treat `RiotDestination.phoneTabs` as the single source of shell destinations. Delete the unused enum case, model state, and view, then let the existing tab bar and ZStack derive the five remaining surfaces from `allCases`. Do not touch transport, persistence, repository, Rust, or Android behavior.

**Tech Stack:** Swift 6, SwiftUI, XCTest, Xcode 26 iOS simulator

---

### Task 1: Remove the dead shell destination

**Files:**
- Modify: `apps/ios/RiotTests/ShellNavigationTests.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`

- [ ] **Step 1: Write the failing navigation test**

Change the navigation expectation to the five actual product surfaces:

```swift
func testConferenceShellExposesOnlyWorkingSurfaces() {
    XCTAssertEqual(
        RiotDestination.phoneTabs.map(\.title),
        [
            "Spaces",
            "App directory",
            "Incident board",
            "Compose & sign",
            "Connection",
        ]
    )
    XCTAssertEqual(
        RiotDestination.phoneTabs.map(\.tabTitle),
        ["Spaces", "Apps", "Board", "Compose", "Connect"]
    )
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
xcodebuild test \
  -project apps/ios/Riot.xcodeproj \
  -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-derived \
  -only-testing:RiotKitTests/ShellNavigationTests
```

Expected: `testConferenceShellExposesOnlyWorkingSurfaces` fails because `phoneTabs` still includes `Import preview` / `Import`.

- [ ] **Step 3: Remove the destination and unused state**

In `RiotDestination`, delete:

```swift
case importPreview
```

Delete the `.importPreview` branches from `title`, `tabTitle`, and `systemImage`. Delete this unused property from `RiotAppModel`:

```swift
@Published public private(set) var importEntries: [RiotEntry] = []
```

Update the performance-contract prose from six destinations to five destinations.

In `ConferenceShellView.destinationView`, delete:

```swift
case .importPreview: ImportPreviewView(model: model)
```

Delete the complete private `ImportPreviewView` declaration. Update the shell performance comment from “five other destination views” to “four other destination views.” Preserve all unrelated concurrent edits in both production files.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run the Step 2 command again.

Expected: `ShellNavigationTests` passes with zero failures.

- [ ] **Step 5: Run the full iOS library test suite**

Run:

```bash
xcodebuild test \
  -project apps/ios/Riot.xcodeproj \
  -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-derived
```

Expected: all RiotKit tests pass with zero failures.

- [ ] **Step 6: Inspect scope and commit only this task**

Run:

```bash
git diff --check -- \
  apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/RiotTests/ShellNavigationTests.swift

git diff -- \
  apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/RiotTests/ShellNavigationTests.swift
```

Confirm the diff removes only the dead Import surface and adjusts destination-count comments; nearby-sync controls and transport code remain unchanged. Because the production files contain concurrent uncommitted work, stage only the exact removal hunks, then commit:

```bash
git commit -m "fix(ios): remove dead import tab"
```
