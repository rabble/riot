# Riot iOS Visual Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the `riot.protest.net` marketing site's visual identity (Anton/Work Sans/Space Mono, flat hard-bordered "protest zine" aesthetic, blue/pink accent) into the native iOS shell, fully replacing native `TabView`/navigation-title chrome with custom components, per `docs/superpowers/specs/2026-07-11-riot-ios-visual-identity-design.md`.

**Architecture:** A new `apps/ios/Riot/Design/` module (color/font tokens, tab bar, header, card, button style, badge, empty state) built inside the existing `RiotKit` library target so it's covered by `@testable import RiotKit` tests. `ConferenceShellView.swift` (app target) drops `TabView` for a manually-switched stack of persistent `NavigationStack`s plus a custom `RiotTabBar`, and each of the five screens is restyled to use the new components. `RiotDestination`/`RiotAppModel`'s public API is untouched.

**Tech Stack:** Swift 6, SwiftUI, XCTest, Xcode 26.2 / iOS 17+ deployment target. Fonts sourced from Google Fonts (Anton, Work Sans, Space Mono — all SIL OFL).

---

## Before you start

- Read `docs/superpowers/specs/2026-07-11-riot-ios-visual-identity-design.md` in full.
- `COLLABORATION.md` shows `apps/ios/` claimed by this work (see the "iOS visual design + navigation polish" row). Run `git status --short` and re-read `COLLABORATION.md` before starting in case that's changed.
- Do not touch `apps/ios/Riot/Core/` (the Keychain/identity layer landed in commit `5bb25fa`).
- Verify command used throughout: `xcodebuild -project apps/ios/Riot.xcodeproj -scheme <Riot|RiotKit> -destination 'platform=iOS Simulator,name=iPhone 17 Pro' <test|build>`. Adjust the simulator name if `iPhone 17 Pro` isn't installed (`xcrun simctl list devices available`).
- All `project.pbxproj` object IDs below were freshly allocated (`A00000000000000000000090` through `A000000000000000000000AC`) and verified not to collide with any existing ID in the file at plan-writing time. Re-run `grep -oE "A0{10,}[0-9A-F]{2,6}" apps/ios/Riot.xcodeproj/project.pbxproj | sort -u` before using them if significant time has passed and another agent may have added entries — if any of these IDs already exist, stop and pick fresh unused ones following the same pattern instead of overwriting.

---

### Task 1: Vendor fonts, add `Info.plist`, wire `UIAppFonts`

**Files:**
- Create: `apps/ios/Riot/Resources/Fonts/Anton-Regular.ttf`
- Create: `apps/ios/Riot/Resources/Fonts/WorkSans-Variable.ttf`
- Create: `apps/ios/Riot/Resources/Fonts/SpaceMono-Regular.ttf`
- Create: `apps/ios/Riot/Resources/Fonts/SpaceMono-Bold.ttf`
- Create: `apps/ios/Riot/Info.plist`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Download the fonts (SIL OFL, from the canonical google/fonts repo)**

```bash
mkdir -p apps/ios/Riot/Resources/Fonts
curl -sS -o apps/ios/Riot/Resources/Fonts/Anton-Regular.ttf \
  https://raw.githubusercontent.com/google/fonts/main/ofl/anton/Anton-Regular.ttf
curl -sS -o "apps/ios/Riot/Resources/Fonts/WorkSans-Variable.ttf" \
  "https://raw.githubusercontent.com/google/fonts/main/ofl/worksans/WorkSans%5Bwght%5D.ttf"
curl -sS -o apps/ios/Riot/Resources/Fonts/SpaceMono-Regular.ttf \
  https://raw.githubusercontent.com/google/fonts/main/ofl/spacemono/SpaceMono-Regular.ttf
curl -sS -o apps/ios/Riot/Resources/Fonts/SpaceMono-Bold.ttf \
  https://raw.githubusercontent.com/google/fonts/main/ofl/spacemono/SpaceMono-Bold.ttf
file apps/ios/Riot/Resources/Fonts/*.ttf
```

Expected: all four report `TrueType Font data`. (`WorkSans-Variable.ttf` is a
variable font whose default/Regular named instance has PostScript name
`WorkSans-Regular` — confirmed via its `METADATA.pb` in the google/fonts repo.
Only the Regular instance is used by this app; no variable-weight axis
manipulation is attempted in SwiftUI.)

- [ ] **Step 2: Create `Info.plist` declaring the fonts**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>UIAppFonts</key>
	<array>
		<string>Anton-Regular.ttf</string>
		<string>WorkSans-Variable.ttf</string>
		<string>SpaceMono-Regular.ttf</string>
		<string>SpaceMono-Bold.ttf</string>
	</array>
</dict>
</plist>
```

Save this as `apps/ios/Riot/Info.plist`. The target already has
`GENERATE_INFOPLIST_FILE = YES`; setting `INFOPLIST_FILE` on top of that
(next step) makes Xcode merge this file's keys into the generated Info.plist
rather than replacing it — the existing `INFOPLIST_KEY_CFBundleDisplayName`
etc. build settings keep working unchanged.

- [ ] **Step 3: Register the new files in `project.pbxproj`**

Add these five `PBXFileReference` lines immediately after the line starting
`A0000000000000000000001E = {isa = PBXFileReference; ... Transport/NearbyTransportController.swift` (i.e. right before the `PBXBuildFile` block that starts with `A00000000000000000000020`):

```
		A00000000000000000000099 = {isa = PBXFileReference; lastKnownFileType = file; path = Resources/Fonts/Anton-Regular.ttf; sourceTree = "<group>";};
		A0000000000000000000009A = {isa = PBXFileReference; lastKnownFileType = file; path = Resources/Fonts/WorkSans-Variable.ttf; sourceTree = "<group>";};
		A0000000000000000000009B = {isa = PBXFileReference; lastKnownFileType = file; path = Resources/Fonts/SpaceMono-Regular.ttf; sourceTree = "<group>";};
		A0000000000000000000009C = {isa = PBXFileReference; lastKnownFileType = file; path = Resources/Fonts/SpaceMono-Bold.ttf; sourceTree = "<group>";};
		A0000000000000000000009D = {isa = PBXFileReference; lastKnownFileType = text.plist.xml; path = Info.plist; sourceTree = "<group>";};
```

Add these four `PBXBuildFile` lines immediately after the line starting
`A00000000000000000000038 = {isa = PBXBuildFile; fileRef = A0000000000000000000001E;};` (the last line of the existing `PBXBuildFile` block, right before the `PBXResourcesBuildPhase`/`PBXSourcesBuildPhase` section):

```
		A000000000000000000000A9 = {isa = PBXBuildFile; fileRef = A00000000000000000000099;};
		A000000000000000000000AA = {isa = PBXBuildFile; fileRef = A0000000000000000000009A;};
		A000000000000000000000AB = {isa = PBXBuildFile; fileRef = A0000000000000000000009B;};
		A000000000000000000000AC = {isa = PBXBuildFile; fileRef = A0000000000000000000009C;};
```

(No `PBXBuildFile` for `Info.plist` — it's referenced only via the
`INFOPLIST_FILE` build setting, not a Resources copy phase.)

Find this line (the `Riot` group):

```
		A00000000000000000000002 = {isa = PBXGroup; path = Riot; sourceTree = "<group>"; children = (A00000000000000000000012, A00000000000000000000017, A00000000000000000000019, A0000000000000000000001A, A0000000000000000000001B, A0000000000000000000001C, A0000000000000000000001D, A0000000000000000000001E, A00000000000000000000014, A00000000000000000000015, A00000000000000000000016);};
```

Replace it with (adds the five new file refs to the group's children):

```
		A00000000000000000000002 = {isa = PBXGroup; path = Riot; sourceTree = "<group>"; children = (A00000000000000000000012, A00000000000000000000017, A00000000000000000000019, A0000000000000000000001A, A0000000000000000000001B, A0000000000000000000001C, A0000000000000000000001D, A0000000000000000000001E, A00000000000000000000014, A00000000000000000000015, A00000000000000000000016, A00000000000000000000099, A0000000000000000000009A, A0000000000000000000009B, A0000000000000000000009C, A0000000000000000000009D);};
```

Find this line (the Riot app target's empty Resources phase):

```
		A00000000000000000000037 = {isa = PBXResourcesBuildPhase; buildActionMask = 2147483647; files = (); runOnlyForDeploymentPostprocessing = 0;};
```

Replace it with (adds the four font build files):

```
		A00000000000000000000037 = {isa = PBXResourcesBuildPhase; buildActionMask = 2147483647; files = (A000000000000000000000A9, A000000000000000000000AA, A000000000000000000000AB, A000000000000000000000AC); runOnlyForDeploymentPostprocessing = 0;};
```

Find the Riot target's Debug config (contains `PRODUCT_BUNDLE_IDENTIFIER = net.protest.riot;` and `name = Debug;`) and Release config (same but `name = Release;`). In **both**, find:

```
GENERATE_INFOPLIST_FILE = YES; HEADER_SEARCH_PATHS =
```

Replace with:

```
GENERATE_INFOPLIST_FILE = YES; INFOPLIST_FILE = Riot/Info.plist; HEADER_SEARCH_PATHS =
```

- [ ] **Step 4: Build and verify the fonts load**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`. This only proves the project file is valid
and the resources copy correctly — actual font rendering is verified
visually in Task 14.

- [ ] **Step 5: Commit**

```bash
git add apps/ios/Riot/Resources/Fonts apps/ios/Riot/Info.plist apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): vendor Anton/Work Sans/Space Mono and register UIAppFonts"
```

---

### Task 2: `RiotTheme` — color and font tokens

**Files:**
- Create: `apps/ios/Riot/Design/RiotTheme.swift`
- Create: `apps/ios/RiotTests/RiotThemeTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Write the failing tests**

```swift
import XCTest
import SwiftUI
@testable import RiotKit

final class RiotThemeTests: XCTestCase {
    func testPaperColorMatchesMarketingSiteTokens() {
        assertColor(RiotTheme.paper(for: .light), hex: 0xEAE6DA)
        assertColor(RiotTheme.paper(for: .dark), hex: 0x131209)
    }

    func testInkColorMatchesMarketingSiteTokens() {
        assertColor(RiotTheme.ink(for: .light), hex: 0x17160F)
        assertColor(RiotTheme.ink(for: .dark), hex: 0xEFE9D8)
    }

    func testAccentColorsMatchMarketingSiteTokens() {
        assertColor(RiotTheme.blue(for: .light), hex: 0x22399F)
        assertColor(RiotTheme.blue(for: .dark), hex: 0x6D84FF)
        assertColor(RiotTheme.pink(for: .light), hex: 0xD1216E)
        assertColor(RiotTheme.pink(for: .dark), hex: 0xFF5F9E)
    }

    func testFontRolesMapToExpectedPostScriptNames() {
        XCTAssertEqual(RiotFontRole.poster.postScriptName, "Anton-Regular")
        XCTAssertEqual(RiotFontRole.body.postScriptName, "WorkSans-Regular")
        XCTAssertEqual(RiotFontRole.mono.postScriptName, "SpaceMono-Regular")
        XCTAssertEqual(RiotFontRole.monoBold.postScriptName, "SpaceMono-Bold")
    }

    private func assertColor(_ color: Color, hex: UInt32, file: StaticString = #filePath, line: UInt = #line) {
        let resolved = UIColor(color)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        resolved.getRed(&r, green: &g, blue: &b, alpha: &a)
        let expectedR = CGFloat((hex >> 16) & 0xFF) / 255
        let expectedG = CGFloat((hex >> 8) & 0xFF) / 255
        let expectedB = CGFloat(hex & 0xFF) / 255
        XCTAssertEqual(r, expectedR, accuracy: 0.01, "red", file: file, line: line)
        XCTAssertEqual(g, expectedG, accuracy: 0.01, "green", file: file, line: line)
        XCTAssertEqual(b, expectedB, accuracy: 0.01, "blue", file: file, line: line)
    }
}
```

- [ ] **Step 2: Wire the test file into the project, run, confirm it fails**

Add this `PBXFileReference` after the `A0000000000000000000009D` line added in Task 1:

```
		A00000000000000000000097 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = RiotThemeTests.swift; sourceTree = "<group>";};
```

Add this `PBXBuildFile` after the `A000000000000000000000AC` line added in Task 1:

```
		A000000000000000000000A7 = {isa = PBXBuildFile; fileRef = A00000000000000000000097;};
```

Find the `RiotTests` group:

```
		A00000000000000000000003 = {isa = PBXGroup; path = RiotTests; sourceTree = "<group>"; children = (A00000000000000000000010, A00000000000000000000013, A00000000000000000000018);};
```

Replace with:

```
		A00000000000000000000003 = {isa = PBXGroup; path = RiotTests; sourceTree = "<group>"; children = (A00000000000000000000010, A00000000000000000000013, A00000000000000000000018, A00000000000000000000097);};
```

Find the `RiotTests` target's `PBXSourcesBuildPhase`:

```
		A00000000000000000000031 = {isa = PBXSourcesBuildPhase; buildActionMask = 2147483647; files = (A00000000000000000000020, A00000000000000000000024, A0000000000000000000002A); runOnlyForDeploymentPostprocessing = 0;};
```

Replace with:

```
		A00000000000000000000031 = {isa = PBXSourcesBuildPhase; buildActionMask = 2147483647; files = (A00000000000000000000020, A00000000000000000000024, A0000000000000000000002A, A000000000000000000000A7); runOnlyForDeploymentPostprocessing = 0;};
```

Run:

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test 2>&1 | tail -40
```

Expected: **BUILD FAILED** — `RiotThemeTests.swift` references `RiotTheme`
and `RiotFontRole`, which don't exist yet.

- [ ] **Step 3: Write `RiotTheme.swift`**

```swift
import SwiftUI

public enum RiotTheme {
    public static func paper(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0x131209) : hex(0xEAE6DA)
    }

    public static func paper2(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0x1C1A10) : hex(0xE1DCCB)
    }

    public static func ink(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0xEFE9D8) : hex(0x17160F)
    }

    public static func inkSoft(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0xBEB69E) : hex(0x4A473B)
    }

    public static func blue(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0x6D84FF) : hex(0x22399F)
    }

    public static func pink(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0xFF5F9E) : hex(0xD1216E)
    }

    public static func line(for scheme: ColorScheme) -> Color {
        ink(for: scheme).opacity(scheme == .dark ? 0.16 : 0.18)
    }

    public static func lineStrong(for scheme: ColorScheme) -> Color {
        ink(for: scheme).opacity(scheme == .dark ? 0.36 : 0.4)
    }

    private static func hex(_ value: UInt32) -> Color {
        Color(
            red: Double((value >> 16) & 0xFF) / 255,
            green: Double((value >> 8) & 0xFF) / 255,
            blue: Double(value & 0xFF) / 255
        )
    }
}

public enum RiotFontRole {
    case poster
    case body
    case mono
    case monoBold

    var postScriptName: String {
        switch self {
        case .poster: return "Anton-Regular"
        case .body: return "WorkSans-Regular"
        case .mono: return "SpaceMono-Regular"
        case .monoBold: return "SpaceMono-Bold"
        }
    }
}

public extension Font {
    static func riot(_ role: RiotFontRole, size: CGFloat, relativeTo textStyle: Font.TextStyle = .body) -> Font {
        .custom(role.postScriptName, size: size, relativeTo: textStyle)
    }
}
```

Add this `PBXFileReference` after the `A00000000000000000000097` line added in Step 2 above:

```
		A00000000000000000000090 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = Design/RiotTheme.swift; sourceTree = "<group>";};
```

Add this `PBXBuildFile` after the `A000000000000000000000A7` line added in Step 2 above:

```
		A000000000000000000000A0 = {isa = PBXBuildFile; fileRef = A00000000000000000000090;};
```

Find the `Riot` group (already modified in Task 1 — use its current children list) and append `A00000000000000000000090` to the end of its `children = (...)` list, right before the closing `);`.

Find the `RiotKit` target's `PBXSourcesBuildPhase`:

```
		A00000000000000000000030 = {isa = PBXSourcesBuildPhase; buildActionMask = 2147483647; files = (A00000000000000000000021, A00000000000000000000023, A00000000000000000000025, A00000000000000000000029, A0000000000000000000002B, A0000000000000000000002C, A0000000000000000000002D, A0000000000000000000002E, A0000000000000000000002F, A00000000000000000000038); runOnlyForDeploymentPostprocessing = 0;};
```

Replace with:

```
		A00000000000000000000030 = {isa = PBXSourcesBuildPhase; buildActionMask = 2147483647; files = (A00000000000000000000021, A00000000000000000000023, A00000000000000000000025, A00000000000000000000029, A0000000000000000000002B, A0000000000000000000002C, A0000000000000000000002D, A0000000000000000000002E, A0000000000000000000002F, A00000000000000000000038, A000000000000000000000A0); runOnlyForDeploymentPostprocessing = 0;};
```

- [ ] **Step 4: Run tests, confirm they pass**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test 2>&1 | tail -40
```

Expected: `RiotThemeTests` — 4/4 pass, plus the existing 7 tests still pass
(11 total).

- [ ] **Step 5: Commit**

```bash
git add apps/ios/Riot/Design/RiotTheme.swift apps/ios/RiotTests/RiotThemeTests.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): add RiotTheme color and font tokens"
```

---

### Task 3: `RiotCard`

**Files:**
- Create: `apps/ios/Riot/Design/RiotCard.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

No new tests — this is a pure declarative layout wrapper with no logic
beyond applying `RiotTheme` tokens (already tested in Task 2); correctness
is confirmed visually in Task 14.

- [ ] **Step 1: Write `RiotCard.swift`**

```swift
import SwiftUI

public struct RiotCard<Content: View>: View {
    @Environment(\.colorScheme) private var colorScheme
    private let content: Content

    public init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    public var body: some View {
        content
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RiotTheme.paper2(for: colorScheme))
            .overlay(
                Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2)
            )
    }
}
```

- [ ] **Step 2: Wire into the project**

Add `PBXFileReference`:

```
		A00000000000000000000091 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = Design/RiotCard.swift; sourceTree = "<group>";};
```

Add `PBXBuildFile`:

```
		A000000000000000000000A1 = {isa = PBXBuildFile; fileRef = A00000000000000000000091;};
```

Append `A00000000000000000000091` to the `Riot` group's `children` list, and
`A000000000000000000000A1` to the `RiotKit` target's `PBXSourcesBuildPhase`
`files` list (same two lists edited in Task 2 Step 3).

- [ ] **Step 3: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 4: Commit**

```bash
git add apps/ios/Riot/Design/RiotCard.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): add RiotCard flat bordered container"
```

---

### Task 4: `RiotButtonStyle`

**Files:**
- Create: `apps/ios/Riot/Design/RiotButtonStyle.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Write `RiotButtonStyle.swift`**

```swift
import SwiftUI

public enum RiotButtonEmphasis {
    case primary
    case secondary
}

public struct RiotButtonStyle: ButtonStyle {
    @Environment(\.colorScheme) private var colorScheme
    private let emphasis: RiotButtonEmphasis

    public init(_ emphasis: RiotButtonEmphasis = .primary) {
        self.emphasis = emphasis
    }

    public func makeBody(configuration: Configuration) -> some View {
        let ink = RiotTheme.ink(for: colorScheme)
        let paper = RiotTheme.paper(for: colorScheme)
        let pink = RiotTheme.pink(for: colorScheme)
        let isPrimary = emphasis == .primary
        let fill: Color = configuration.isPressed ? pink : (isPrimary ? ink : Color.clear)
        let foreground: Color = (isPrimary || configuration.isPressed) ? paper : ink
        let border: Color = configuration.isPressed ? pink : ink

        return configuration.label
            .font(.riot(.mono, size: 13, relativeTo: .footnote))
            .textCase(.uppercase)
            .tracking(1)
            .padding(.horizontal, 22)
            .padding(.vertical, 14)
            .foregroundStyle(foreground)
            .background(fill)
            .overlay(Rectangle().strokeBorder(border, lineWidth: 2))
    }
}

public extension ButtonStyle where Self == RiotButtonStyle {
    static var riotPrimary: RiotButtonStyle { RiotButtonStyle(.primary) }
    static var riotSecondary: RiotButtonStyle { RiotButtonStyle(.secondary) }
}
```

- [ ] **Step 2: Wire into the project**

Add `PBXFileReference`:

```
		A00000000000000000000092 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = Design/RiotButtonStyle.swift; sourceTree = "<group>";};
```

Add `PBXBuildFile`:

```
		A000000000000000000000A2 = {isa = PBXBuildFile; fileRef = A00000000000000000000092;};
```

Append `A00000000000000000000092` to the `Riot` group's `children`, and
`A000000000000000000000A2` to the `RiotKit` `PBXSourcesBuildPhase` `files`.

- [ ] **Step 3: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 4: Commit**

```bash
git add apps/ios/Riot/Design/RiotButtonStyle.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): add RiotButtonStyle"
```

---

### Task 5: `RiotBadge`

**Files:**
- Create: `apps/ios/Riot/Design/RiotBadge.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Write `RiotBadge.swift`**

```swift
import SwiftUI

public struct RiotBadge: View {
    @Environment(\.colorScheme) private var colorScheme
    private let text: String
    private let stamped: Bool

    public init(_ text: String, stamped: Bool = false) {
        self.text = text
        self.stamped = stamped
    }

    public var body: some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .multilineTextAlignment(.leading)
            .foregroundStyle(RiotTheme.ink(for: colorScheme))
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .overlay(Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2))
            .rotationEffect(stamped ? .degrees(-2) : .zero)
    }
}
```

- [ ] **Step 2: Wire into the project**

Add `PBXFileReference`:

```
		A00000000000000000000093 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = Design/RiotBadge.swift; sourceTree = "<group>";};
```

Add `PBXBuildFile`:

```
		A000000000000000000000A3 = {isa = PBXBuildFile; fileRef = A00000000000000000000093;};
```

Append `A00000000000000000000093` to the `Riot` group's `children`, and
`A000000000000000000000A3` to the `RiotKit` `PBXSourcesBuildPhase` `files`.

- [ ] **Step 3: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 4: Commit**

```bash
git add apps/ios/Riot/Design/RiotBadge.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): add RiotBadge stamp/chip component"
```

---

### Task 6: `RiotHeader`

**Files:**
- Create: `apps/ios/Riot/Design/RiotHeader.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Write `RiotHeader.swift`**

```swift
import SwiftUI

public struct RiotHeader: View {
    @Environment(\.colorScheme) private var colorScheme
    private let eyebrow: String?
    private let title: String

    public init(eyebrow: String? = nil, title: String) {
        self.eyebrow = eyebrow
        self.title = title
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let eyebrow {
                Text(eyebrow)
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .textCase(.uppercase)
                    .tracking(1)
                    .foregroundStyle(RiotTheme.pink(for: colorScheme))
            }
            Text(title)
                .font(.riot(.poster, size: 34, relativeTo: .largeTitle))
                .textCase(.uppercase)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .shadow(color: RiotTheme.blue(for: colorScheme), radius: 0, x: 2, y: 2)
                .shadow(color: RiotTheme.pink(for: colorScheme), radius: 0, x: -2, y: -2)
        }
        .padding(.horizontal, 20)
        .padding(.top, 20)
        .padding(.bottom, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RiotTheme.paper(for: colorScheme))
    }
}

public extension View {
    func riotHeader(eyebrow: String? = nil, _ title: String) -> some View {
        self
            .toolbar(.hidden, for: .navigationBar)
            .safeAreaInset(edge: .top, spacing: 0) {
                RiotHeader(eyebrow: eyebrow, title: title)
            }
    }
}
```

- [ ] **Step 2: Wire into the project**

Add `PBXFileReference`:

```
		A00000000000000000000094 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = Design/RiotHeader.swift; sourceTree = "<group>";};
```

Add `PBXBuildFile`:

```
		A000000000000000000000A4 = {isa = PBXBuildFile; fileRef = A00000000000000000000094;};
```

Append `A00000000000000000000094` to the `Riot` group's `children`, and
`A000000000000000000000A4` to the `RiotKit` `PBXSourcesBuildPhase` `files`.

- [ ] **Step 3: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 4: Commit**

```bash
git add apps/ios/Riot/Design/RiotHeader.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): add RiotHeader poster-style navigation header"
```

---

### Task 7: `RiotEmptyState`

**Files:**
- Create: `apps/ios/Riot/Design/RiotEmptyState.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Write `RiotEmptyState.swift`**

```swift
import SwiftUI

public struct RiotEmptyState: View {
    @Environment(\.colorScheme) private var colorScheme
    private let title: String
    private let message: String

    public init(title: String, message: String) {
        self.title = title
        self.message = message
    }

    public var body: some View {
        VStack(spacing: 14) {
            Text(title)
                .font(.riot(.poster, size: 26, relativeTo: .title))
                .textCase(.uppercase)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .multilineTextAlignment(.center)
            Text(message)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                .multilineTextAlignment(.center)
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
```

- [ ] **Step 2: Wire into the project**

Add `PBXFileReference`:

```
		A00000000000000000000095 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = Design/RiotEmptyState.swift; sourceTree = "<group>";};
```

Add `PBXBuildFile`:

```
		A000000000000000000000A5 = {isa = PBXBuildFile; fileRef = A00000000000000000000095;};
```

Append `A00000000000000000000095` to the `Riot` group's `children`, and
`A000000000000000000000A5` to the `RiotKit` `PBXSourcesBuildPhase` `files`.

- [ ] **Step 3: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 4: Commit**

```bash
git add apps/ios/Riot/Design/RiotEmptyState.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): add RiotEmptyState"
```

---

### Task 8: `RiotTabBar` and shell wiring (replaces native `TabView` chrome)

**Files:**
- Create: `apps/ios/Riot/Design/RiotTabBar.swift`
- Create: `apps/ios/RiotTests/RiotTabBarTests.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import RiotKit

final class RiotTabBarTests: XCTestCase {
    func testItemsMatchPhoneTabsInOrder() {
        XCTAssertEqual(RiotTabBar.items.map(\.destination), RiotDestination.phoneTabs)
        XCTAssertEqual(RiotTabBar.items.map(\.label), RiotDestination.phoneTabs.map(\.tabTitle))
        XCTAssertEqual(RiotTabBar.items.map(\.systemImage), RiotDestination.phoneTabs.map(\.systemImage))
    }
}
```

- [ ] **Step 2: Wire the test file in, run, confirm it fails**

Add `PBXFileReference`:

```
		A00000000000000000000098 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = RiotTabBarTests.swift; sourceTree = "<group>";};
```

Add `PBXBuildFile`:

```
		A000000000000000000000A8 = {isa = PBXBuildFile; fileRef = A00000000000000000000098;};
```

Append `A00000000000000000000098` to the `RiotTests` group's `children`
(edited in Task 2 Step 2), and `A000000000000000000000A8` to the
`RiotTests` target's `PBXSourcesBuildPhase` `files` list (also edited in
Task 2 Step 2).

Run:

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test 2>&1 | tail -40
```

Expected: **BUILD FAILED** — `RiotTabBar` doesn't exist yet.

- [ ] **Step 3: Write `RiotTabBar.swift`**

```swift
import SwiftUI

public struct RiotTabItem: Identifiable, Equatable {
    public let destination: RiotDestination
    public let label: String
    public let systemImage: String
    public var id: RiotDestination { destination }
}

public struct RiotTabBar: View {
    @Environment(\.colorScheme) private var colorScheme
    @Binding private var selection: RiotDestination

    public static let items: [RiotTabItem] = RiotDestination.phoneTabs.map {
        RiotTabItem(destination: $0, label: $0.tabTitle, systemImage: $0.systemImage)
    }

    public init(selection: Binding<RiotDestination>) {
        self._selection = selection
    }

    public var body: some View {
        HStack(spacing: 0) {
            ForEach(Self.items) { item in
                Button {
                    selection = item.destination
                } label: {
                    tabLabel(for: item)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(item.label)
                .accessibilityAddTraits(item.destination == selection ? [.isButton, .isSelected] : .isButton)
            }
        }
        .padding(.top, 10)
        .padding(.bottom, 6)
        .background(RiotTheme.paper(for: colorScheme))
        .overlay(alignment: .top) {
            Rectangle().fill(RiotTheme.ink(for: colorScheme)).frame(height: 2)
        }
    }

    @ViewBuilder
    private func tabLabel(for item: RiotTabItem) -> some View {
        let isSelected = item.destination == selection
        VStack(spacing: 4) {
            Image(systemName: item.systemImage)
                .font(.system(size: 20, weight: .bold))
            Text(item.label)
                .font(.riot(.mono, size: 10, relativeTo: .caption2))
                .textCase(.uppercase)
                .tracking(0.5)
        }
        .foregroundStyle(isSelected ? RiotTheme.paper(for: colorScheme) : RiotTheme.ink(for: colorScheme))
        .frame(maxWidth: .infinity)
        .padding(.vertical, 6)
        .background {
            if isSelected {
                Rectangle()
                    .fill(RiotTheme.pink(for: colorScheme))
                    .rotationEffect(.degrees(-2))
                    .padding(.horizontal, 4)
            }
        }
    }
}
```

Add `PBXFileReference`:

```
		A00000000000000000000096 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = Design/RiotTabBar.swift; sourceTree = "<group>";};
```

Add `PBXBuildFile`:

```
		A000000000000000000000A6 = {isa = PBXBuildFile; fileRef = A00000000000000000000096;};
```

Append `A00000000000000000000096` to the `Riot` group's `children`, and
`A000000000000000000000A6` to the `RiotKit` `PBXSourcesBuildPhase` `files`.

- [ ] **Step 4: Run tests, confirm they pass**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test 2>&1 | tail -40
```

Expected: `RiotTabBarTests` — 1/1 pass, all prior tests (11) still pass (12
total).

- [ ] **Step 5: Commit**

```bash
git add apps/ios/Riot/Design/RiotTabBar.swift apps/ios/RiotTests/RiotTabBarTests.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): add RiotTabBar with tested item ordering"
```

- [ ] **Step 6: Replace `ConferenceShellView`'s `TabView` with `RiotTabBar`**

Read `apps/ios/Riot/ConferenceShellView.swift`. Replace the `ConferenceShellView`
struct's body (everything from `struct ConferenceShellView: View {` through
its closing `}`, i.e. lines 4–52 as of this plan's writing — re-locate by
matching the code below if the file has shifted) with:

```swift
struct ConferenceShellView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                ForEach(RiotDestination.phoneTabs) { destination in
                    NavigationStack {
                        destinationView(destination)
                    }
                    .opacity(model.destination == destination ? 1 : 0)
                    .allowsHitTesting(model.destination == destination)
                }
            }
            connectionDisclosureBar
            RiotTabBar(selection: $model.destination)
        }
        .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
        .alert("Riot couldn’t finish that", isPresented: errorBinding) {
            Button("OK") { model.dismissError() }
        } message: {
            Text(model.errorMessage ?? "Unknown local error")
        }
    }

    private var connectionDisclosureBar: some View {
        Text(model.connectionDisclosure)
            .font(.riot(.mono, size: 11, relativeTo: .caption2))
            .textCase(.uppercase)
            .tracking(0.5)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
            .background(RiotTheme.paper2(for: colorScheme))
            .overlay(alignment: .top) {
                Rectangle().fill(RiotTheme.line(for: colorScheme)).frame(height: 1)
            }
    }

    @ViewBuilder
    private func destinationView(_ destination: RiotDestination) -> some View {
        switch destination {
        case .spaces: SpacesView(model: model)
        case .board: IncidentBoardView(model: model)
        case .compose: ComposeReviewSignView(model: model)
        case .importPreview: ImportPreviewView(model: model)
        case .connection: ConnectionStatusView(model: model)
        }
    }

    private var errorBinding: Binding<Bool> {
        Binding(
            get: { model.errorMessage != nil },
            set: { isPresented in
                if !isPresented { model.dismissError() }
            }
        )
    }
}
```

This drops `TabView` entirely: all five destinations get a persistent
`NavigationStack` (so each tab keeps its own push-navigation state across
switches, matching the old behavior), only the currently-selected one is
visible/hit-testable, and `RiotTabBar` drives `model.destination` directly —
the same property the old `TabView(selection:)` bound to, so
`ShellNavigationTests` (which only exercises `model.select`/`model.destination`,
never the view) needs no changes.

- [ ] **Step 7: Build and run the full existing test suite**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
xcodebuild -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test 2>&1 | tail -40
```

Expected: build succeeds; all 12 tests still pass (the shell restructuring
doesn't touch anything `ShellNavigationTests`/`BindingSemanticsTests` assert
on).

- [ ] **Step 8: Commit**

```bash
git add apps/ios/Riot/ConferenceShellView.swift
git commit -m "feat(ios): replace native TabView chrome with RiotTabBar"
```

---

### Task 9: Restyle `SpacesView`

**Files:**
- Modify: `apps/ios/Riot/ConferenceShellView.swift`

- [ ] **Step 1: Replace the `SpacesView` and `IdentifierRow` private structs**

Replace:

```swift
private struct SpacesView: View {
    @ObservedObject var model: RiotAppModel
    @State private var title = "Berlin Mutual Aid"

    var body: some View {
        Form {
            Section("Public incident space") {
                if let space = model.space {
                    LabeledContent("Title", value: space.title)
                    IdentifierRow(label: "Namespace", value: space.namespaceID)
                    Text("Public content · fixed incident-board/1 renderer")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else {
                    TextField("Space title", text: $title)
                    Button("Create public space") { model.createSpace(title: title) }
                        .buttonStyle(.borderedProminent)
                }
            }
        }
        .navigationTitle("Spaces")
    }
}
```

With:

```swift
private struct SpacesView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var title = "Berlin Mutual Aid"

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Public incident space")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        if let space = model.space {
                            LabeledContent("Title", value: space.title)
                            IdentifierRow(label: "Namespace", value: space.namespaceID)
                            Text("Public content · fixed incident-board/1 renderer")
                                .font(.riot(.body, size: 13, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        } else {
                            TextField("Space title", text: $title)
                                .font(.riot(.body, size: 17, relativeTo: .body))
                            Button("Create public space") { model.createSpace(title: title) }
                                .buttonStyle(.riotPrimary)
                        }
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Riot", "Spaces")
    }
}
```

Replace:

```swift
private struct IdentifierRow: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            Text(value)
                .font(.caption.monospaced())
                .textSelection(.enabled)
        }
    }
}
```

With:

```swift
private struct IdentifierRow: View {
    @Environment(\.colorScheme) private var colorScheme
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label)
                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                .textCase(.uppercase)
                .tracking(0.5)
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            Text(value)
                .font(.riot(.mono, size: 13, relativeTo: .footnote))
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .textSelection(.enabled)
        }
    }
}
```

(`IdentifierRow` is used by four other screens later in this plan — moving
it once here is correct; do not duplicate it.)

- [ ] **Step 2: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`. (`IncidentBoardView` and others still
reference the old `IdentifierRow` signature, which is unchanged in shape —
only its internal styling changed — so nothing else breaks.)

- [ ] **Step 3: Commit**

```bash
git add apps/ios/Riot/ConferenceShellView.swift
git commit -m "feat(ios): restyle Spaces screen with Riot design system"
```

---

### Task 10: Restyle `IncidentBoardView`

**Files:**
- Modify: `apps/ios/Riot/ConferenceShellView.swift`

- [ ] **Step 1: Replace the `IncidentBoardView` private struct**

Replace:

```swift
private struct IncidentBoardView: View {
    @ObservedObject var model: RiotAppModel

    var body: some View {
        Group {
            if model.entries.isEmpty {
                ContentUnavailableView(
                    "No alerts yet",
                    systemImage: "exclamationmark.bubble",
                    description: Text("Create and review an alert on this device. It stays local until you explicitly sync it.")
                )
            } else {
                List(model.entries) { entry in
                    VStack(alignment: .leading, spacing: 10) {
                        Text(entry.headline).font(.headline)
                        if entry.aiAssisted {
                            Label("AI-assisted draft · human reviewed and signed", systemImage: "person.crop.circle.badge.checkmark")
                                .font(.caption.weight(.semibold))
                        }
                        Text("Created \(Date(timeIntervalSince1970: TimeInterval(entry.createdAt)), style: .relative)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        IdentifierRow(label: "Entry", value: entry.entryID)
                        IdentifierRow(label: "Signer", value: entry.signerID)
                    }
                    .padding(.vertical, 6)
                }
            }
        }
        .navigationTitle(model.space?.title ?? "Incident board")
    }
}
```

With:

```swift
private struct IncidentBoardView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Group {
            if model.entries.isEmpty {
                RiotEmptyState(
                    title: "No alerts yet",
                    message: "Create and review an alert on this device. It stays local until you explicitly sync it."
                )
            } else {
                ScrollView {
                    VStack(spacing: 12) {
                        ForEach(model.entries) { entry in
                            RiotCard {
                                VStack(alignment: .leading, spacing: 10) {
                                    Text(entry.headline)
                                        .font(.riot(.body, size: 17, relativeTo: .headline))
                                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                                    if entry.aiAssisted {
                                        RiotBadge("AI-assisted · human reviewed and signed")
                                    }
                                    Text("Created \(Date(timeIntervalSince1970: TimeInterval(entry.createdAt)), style: .relative)")
                                        .font(.riot(.mono, size: 11, relativeTo: .caption2))
                                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                                    IdentifierRow(label: "Entry", value: entry.entryID)
                                    IdentifierRow(label: "Signer", value: entry.signerID)
                                }
                            }
                        }
                    }
                    .padding(20)
                }
            }
        }
        .riotHeader(eyebrow: "Public incident space", model.space?.title ?? "Incident board")
    }
}
```

- [ ] **Step 2: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 3: Commit**

```bash
git add apps/ios/Riot/ConferenceShellView.swift
git commit -m "feat(ios): restyle Incident board screen with Riot design system"
```

---

### Task 11: Restyle `ComposeReviewSignView`

**Files:**
- Modify: `apps/ios/Riot/ConferenceShellView.swift`

- [ ] **Step 1: Replace the `ComposeReviewSignView` private struct**

Replace:

```swift
private struct ComposeReviewSignView: View {
    @ObservedObject var model: RiotAppModel
    @State private var headline = "Water available at the east entrance"
    @State private var details = "Bring a bottle. Volunteers are refilling the tank."
    @State private var aiAssisted = true

    var body: some View {
        Form {
            Section("Draft") {
                TextField("Headline", text: $headline, axis: .vertical)
                TextField("What people need to know", text: $details, axis: .vertical)
                    .lineLimit(4...8)
                Toggle("Started with model assistance", isOn: $aiAssisted)
            }
            Section("Review before signing") {
                Text("Signing publishes this alert into your local public space. A model cannot press this button or sync for you.")
                    .font(.callout)
                Button("Review complete — sign locally") {
                    model.sign(headline: headline, description: details, aiAssisted: aiAssisted)
                }
                .buttonStyle(.borderedProminent)
                .disabled(model.space == nil || headline.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .navigationTitle("Compose & sign")
    }
}
```

With:

```swift
private struct ComposeReviewSignView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var headline = "Water available at the east entrance"
    @State private var details = "Bring a bottle. Volunteers are refilling the tank."
    @State private var aiAssisted = true

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 14) {
                        Text("Draft")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        TextField("Headline", text: $headline, axis: .vertical)
                            .font(.riot(.body, size: 17, relativeTo: .body))
                        TextField("What people need to know", text: $details, axis: .vertical)
                            .font(.riot(.body, size: 15, relativeTo: .body))
                            .lineLimit(4...8)
                        Toggle("Started with model assistance", isOn: $aiAssisted)
                            .tint(RiotTheme.pink(for: colorScheme))
                    }
                }
                RiotCard {
                    VStack(alignment: .leading, spacing: 14) {
                        Text("Review before signing")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        Text("Signing publishes this alert into your local public space. A model cannot press this button or sync for you.")
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        Button("Review complete — sign locally") {
                            model.sign(headline: headline, description: details, aiAssisted: aiAssisted)
                        }
                        .buttonStyle(.riotPrimary)
                        .disabled(model.space == nil || headline.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Draft, review, sign", "Compose & sign")
    }
}
```

- [ ] **Step 2: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 3: Commit**

```bash
git add apps/ios/Riot/ConferenceShellView.swift
git commit -m "feat(ios): restyle Compose & sign screen with Riot design system"
```

---

### Task 12: Restyle `ImportPreviewView`

**Files:**
- Modify: `apps/ios/Riot/ConferenceShellView.swift`

- [ ] **Step 1: Replace the `ImportPreviewView` private struct**

Replace:

```swift
private struct ImportPreviewView: View {
    @ObservedObject var model: RiotAppModel

    var body: some View {
        List {
            Section("Preview first") {
                Label("Nothing is accepted automatically", systemImage: "checkmark.shield")
                Text("Nearby and file transports will place signed public entries here. You choose what enters this local space.")
                    .foregroundStyle(.secondary)
            }
            if model.importEntries.isEmpty {
                ContentUnavailableView("No pending import", systemImage: "tray")
            } else {
                ForEach(model.importEntries) { entry in
                    VStack(alignment: .leading) {
                        Text(entry.headline).font(.headline)
                        IdentifierRow(label: "Signer", value: entry.signerID)
                    }
                }
            }
        }
        .navigationTitle("Import preview")
    }
}
```

With:

```swift
private struct ImportPreviewView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 10) {
                        RiotBadge("Nothing is accepted automatically")
                        Text("Nearby and file transports will place signed public entries here. You choose what enters this local space.")
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    }
                }
                if model.importEntries.isEmpty {
                    RiotEmptyState(
                        title: "No pending import",
                        message: "Incoming signed entries will appear here for you to preview before they touch your board."
                    )
                } else {
                    ForEach(model.importEntries) { entry in
                        RiotCard {
                            VStack(alignment: .leading, spacing: 8) {
                                Text(entry.headline)
                                    .font(.riot(.body, size: 16, relativeTo: .headline))
                                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                                IdentifierRow(label: "Signer", value: entry.signerID)
                            }
                        }
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Preview first", "Import preview")
    }
}
```

- [ ] **Step 2: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 3: Commit**

```bash
git add apps/ios/Riot/ConferenceShellView.swift
git commit -m "feat(ios): restyle Import preview screen with Riot design system"
```

---

### Task 13: Restyle `ConnectionStatusView`

**Files:**
- Modify: `apps/ios/Riot/ConferenceShellView.swift`

- [ ] **Step 1: Replace the `ConnectionStatusView` private struct**

Replace:

```swift
private struct ConnectionStatusView: View {
    @ObservedObject var model: RiotAppModel

    var body: some View {
        List {
            Section("Current path") {
                Label(model.connectionDisclosure, systemImage: "iphone.slash")
                    .font(.headline)
                Text("Internet fallback is off. Nearby pairing and bounded local sync are added as an explicit next transport layer.")
                    .foregroundStyle(.secondary)
            }
            Section("On this device") {
                LabeledContent("Signed alerts", value: "\(model.entries.count)")
                LabeledContent("Renderer", value: "incident-board/1")
            }
        }
        .navigationTitle("Connection")
    }
}
```

With:

```swift
private struct ConnectionStatusView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotBadge(model.connectionDisclosure, stamped: true)
                RiotCard {
                    Text("Internet fallback is off. Nearby pairing and bounded local sync are added as an explicit next transport layer.")
                        .font(.riot(.body, size: 15, relativeTo: .callout))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                RiotCard {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("On this device")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        LabeledContent("Signed alerts", value: "\(model.entries.count)")
                        LabeledContent("Renderer", value: "incident-board/1")
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Transport", "Connection")
    }
}
```

- [ ] **Step 2: Build**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
```

Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 3: Commit**

```bash
git add apps/ios/Riot/ConferenceShellView.swift
git commit -m "feat(ios): restyle Connection screen with Riot design system"
```

---

### Task 14: Full regression, manual verification, and handoff

**Files:**
- Modify: `COLLABORATION.md`

- [ ] **Step 1: Run the full test suite**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test 2>&1 | tail -60
```

Expected: all tests pass — the original 7 (`BindingSemanticsTests` ×3,
`ShellNavigationTests` ×4) plus `RiotThemeTests` ×4 and `RiotTabBarTests` ×1
= 12 total, 0 failures.

- [ ] **Step 2: Manual simulator verification (per the `verify` skill)**

```bash
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' build
xcrun simctl boot "iPhone 17 Pro" 2>/dev/null || true
open -a Simulator
xcodebuild -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' install run 2>&1 | tail -20
```

Walk through, taking a screenshot at each step
(`xcrun simctl io booted screenshot <path>.png`):

1. Each of the five tabs, confirming Anton/Work Sans/Space Mono render
   (not falling back to San Francisco — Anton in particular is
   unmistakably different: bold uppercase condensed vs. the system
   default), the flat 2px-bordered cards show with no rounded corners or
   drop shadows, and the custom tab bar (not the native floating capsule)
   is docked at the bottom with the selected tab's pink stamp visible.
2. Toggle the simulator to dark appearance
   (`xcrun simctl ui booted appearance dark`) and re-check all five tabs —
   colors should flip to the dark palette tokens, contrast should still
   read clearly.
3. Toggle to the largest accessibility text size
   (Settings → Accessibility → Display & Text Size → Larger Text, or
   `xcrun simctl` has no direct toggle — set it via the Simulator's
   Settings app) and confirm the Anton poster headline and mono tab
   labels don't clip or overlap.

If anything clips, misrenders, or a font visibly falls back to system
default, fix it before proceeding — do not defer visual defects found here.

- [ ] **Step 3: Update `COLLABORATION.md`**

Find the row added for this work:

```
| Claude (this session) | Design pass on the native iOS shell (visual styling + tab/navigation structure) requested by rabble | `apps/ios/Riot/ConferenceShellView.swift`, `apps/ios/Riot/AppModel.swift`, plus new SwiftUI view/style files under `apps/ios/Riot/` | **Starting — brainstorming design first** | Taking over the now-free `apps/ios/` claim per user direction. Will not touch `apps/ios/Riot/Core/` (Keychain/identity layer, just landed in `5bb25fa`) or `crates/`/FFI. Design doc will land under `docs/superpowers/specs/` before implementation. |
```

Replace its state/evidence cell with a summary of what landed: the new
`apps/ios/Riot/Design/` module, the `RiotTabBar`-based shell restructuring,
all five screens restyled, the full test count (12/12 green), and the
actual commit range from this plan (`git log --oneline` the commits made
across Tasks 1–13). Mark it **Done, released**.

- [ ] **Step 4: Commit**

```bash
git add COLLABORATION.md
git commit -m "docs: record iOS visual identity work as done in COLLABORATION.md"
```

---

## Plan self-review notes

- **Spec coverage:** design tokens (Task 2), tab bar (Task 8), navigation
  header (Task 6), cards/rows (Tasks 3, 9–13), buttons (Task 4), badges
  (Task 5, used in Tasks 10/12/13), empty states (Task 7, used in Tasks
  10/12), font sourcing (Task 1), accessibility labels on the tab bar
  (Task 8) and manual Dynamic-Type/contrast check (Task 14), testing
  strategy (Tasks 2, 8, 14) — all spec sections have a task.
- **Deferred spec question resolved:** the spec left "how to hide native
  `TabView` chrome" open; Task 8 resolves it by dropping `TabView` entirely
  in favor of a manually-switched `ZStack` of persistent `NavigationStack`s,
  which is simpler and avoids fighting private/undocumented tab-bar-hiding
  behavior.
- **Type consistency:** `RiotFontRole`, `Font.riot(_:size:relativeTo:)`,
  `RiotTheme.{paper,paper2,ink,inkSoft,blue,pink,line,lineStrong}`,
  `RiotCard`, `RiotButtonStyle`/`.riotPrimary`/`.riotSecondary`,
  `RiotBadge`, `RiotHeader`/`.riotHeader(eyebrow:_:)`, `RiotEmptyState`,
  `RiotTabBar`/`RiotTabItem` are each defined exactly once (Tasks 2–8) and
  used with identical signatures in every later task.
