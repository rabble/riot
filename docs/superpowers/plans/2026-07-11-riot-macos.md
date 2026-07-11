# Riot macOS App — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a native macOS Riot app that reuses the iOS `RiotKit` sources verbatim — Spaces / newswire / evidence / import-export / nearby sync — without editing `apps/ios/Riot.xcodeproj`. The macOS app is a separate Xcode project that references the shared Swift sources and links a macOS build of the Rust core.

**Architecture:** The iOS app is pure SwiftUI + Foundation with zero UIKit; its reusable layer already lives in a `RiotKit` static-lib target (Core, Design, Transport, AppModel, generated FFI). A new `apps/macos/Riot.xcodeproj` (its own `project.pbxproj`, no contention with iOS) compiles those exact source files by relative reference, adds a macOS-only app entry + Info.plist + entitlements, and links `aarch64-apple-darwin` `libriot_ffi.a`. Fonts and Keychain — the only non-portable boundaries — are handled at the Info.plist / entitlement layer, not in code.

**Tech Stack:** Rust (riot-ffi), Swift 6 / SwiftUI, CoreBluetooth + Network, XCTest. macOS 14 (Sonoma), arm64.

**Spec:** `docs/superpowers/specs/2026-07-11-riot-macos-design.md`.

---

## Before you start

1. Run `git status --short` and read `COLLABORATION.md`. This checkout is shared with other active agents. Claim the files of the task you are starting in `COLLABORATION.md` before editing.
2. **Dependency gates.**
   - Task 1 (native core script) is unblocked now — the script is owned by "Codex root — Task 4 native core packaging," marked **Done, released**, so it is free to extend.
   - Task 2 (new `apps/macos/` project) is unblocked now — it creates a *new* `project.pbxproj` and only *references* the iOS sources; it does not edit `apps/ios/Riot.xcodeproj` or any iOS source file.
   - Tasks 3–4 build and verify the macOS shell. They reference `ConferenceShellView.swift`/`AppModel.swift`, which `docs/superpowers/plans/2026-07-11-js-apps-runtime-ios.md` will later *modify*. That is not a file-edit conflict (this plan edits neither file), but if the iOS runtime session is mid-flight, expect build churn — coordinate via `COLLABORATION.md` before changing shared behavior.
   - The **macOS JS apps runtime is out of this plan** and hard-gated on the iOS runtime plan's host tasks reaching Done (Task 5 tombstone records why).
3. **Do not edit** `apps/ios/Riot.xcodeproj/project.pbxproj` or any file under `apps/ios/Riot/` in this plan. The macOS project holds *references* to those files. If a referenced source needs a change to compile on macOS, that is a red flag — stop and reconsider (the whole premise is that the sources are already portable); do not fork them.
4. Xcode 26.2 / Swift 6.2 and the `aarch64-apple-darwin` rustup target are the macOS prerequisites (mirror of the iOS `apps/ios/README.md` preflight). Run `scripts/conference/build-native-core.sh` from the repo root after any FFI change, before building the Xcode project.

## File Structure

Rust / scripts:
- `scripts/conference/build-native-core.sh` — Task 1: add `aarch64-apple-darwin` build + `build/native/macos/libriot_ffi.a` install
- `scripts/conference/test-native-core-package.sh` — Task 1: add macOS slice to `required_files` + `lipo`/`file` assertions

macOS app (all new, under `apps/macos/`):
- `apps/macos/Riot.xcodeproj/project.pbxproj` — Task 2: hand-authored classic project, its own UUID space
- `apps/macos/Riot.xcodeproj/xcshareddata/xcschemes/RiotKit-macOS.xcscheme` — Task 2: test scheme
- `apps/macos/Riot.xcodeproj/xcshareddata/xcschemes/Riot-macOS.xcscheme` — Task 2: app scheme
- `apps/macos/Riot/RiotMacApp.swift` — Task 3: `@main` macOS entry (`WindowGroup`, optional `.commands`)
- `apps/macos/Riot/Info.plist` — Task 3: `ATSApplicationFontsPath`, usage strings
- `apps/macos/Riot/Riot.entitlements` — Task 3: App Sandbox + data-protection keychain + BLE/network
- `apps/macos/Riot/Resources/Fonts/` — Task 3: the four TTFs (bundled; source of truth stays `apps/ios/Riot/Resources/Fonts/`)
- `apps/macos/README.md` — Task 4: build/run/verify instructions mirroring `apps/ios/README.md`

**Shared sources referenced by the macOS project** (relative `path` file references, `SOURCE_ROOT`-relative, never copied):
`../ios/Riot/Core/ProfileRepository.swift`, `../ios/Riot/Core/WrappingKeyStore.swift`,
`../ios/Riot/AppModel.swift`, `../ios/Riot/ConferenceShellView.swift`,
`../ios/Riot/Transport/*.swift` (6 files), `../ios/Riot/Design/*.swift` (7 files),
and the generated `../../build/generated/riot-ffi/riot_ffi.swift`.

**Xcode project note (applies to Task 2):** `apps/macos/Riot.xcodeproj/project.pbxproj` is hand-managed classic format, mirroring the iOS project's conventions. Use a fresh sequential UUID space (`C00000000000000000000xxx`) so the two projects never share IDs even if a tool ever merges them. Each source is a `PBXFileReference` (with the relative `path` above and `sourceTree = SOURCE_ROOT` for the `../` refs), a `PBXBuildFile`, an entry in the owning target's `PBXSourcesBuildPhase`, and a child of the owning `PBXGroup`. The RiotKit-macOS static-lib target compiles the shared reusable sources (Core, Transport, Design, AppModel, generated FFI); the Riot-macOS app target compiles `ConferenceShellView.swift` (referenced) + `RiotMacApp.swift` (new) and links `-lriot_ffi` + RiotKit-macOS. WebKit is not linked in v1 (no JS runtime).

---

### Task 1: macOS Rust core slice

**Files:**
- Modify: `scripts/conference/build-native-core.sh`
- Modify: `scripts/conference/test-native-core-package.sh`

The package test is the RED/GREEN harness here — it already asserts exact built artifacts and fails when one is missing.

- [ ] **RED:** add `build/native/macos/libriot_ffi.a` to `required_files` in `test-native-core-package.sh`, plus a `lipo -info ... | grep -q 'architecture: arm64'` and `file ... | grep -q 'ar archive'` assertion for it. Run `sh scripts/conference/test-native-core-package.sh` from the repo root — it must FAIL on the missing macOS artifact (proving the assertion is wired before the producer exists).
- [ ] **GREEN:** in `build-native-core.sh`, add `aarch64-apple-darwin` to the installed-target preflight loop, add `cargo build -p riot-ffi --lib --release --locked --target aarch64-apple-darwin`, create `build/native/macos`, and `install -m 0644 target/aarch64-apple-darwin/release/libriot_ffi.a build/native/macos/libriot_ffi.a`. Update the trailing `echo` to mention macOS.
- [ ] Verify: `rustup target add aarch64-apple-darwin` if absent, then `sh scripts/conference/test-native-core-package.sh` — must print `native-core-package: PASS`.

Do not commit the built artifacts (they are gitignored, same as the iOS/Android slices).

```
git add scripts/conference/build-native-core.sh scripts/conference/test-native-core-package.sh
git commit -m "build(macos): add aarch64-apple-darwin riot-ffi slice to native core packaging"
```

---

### Task 2: macOS Xcode project skeleton (no shell yet)

**Files:**
- Create: `apps/macos/Riot.xcodeproj/project.pbxproj`
- Create: `apps/macos/Riot.xcodeproj/xcshareddata/xcschemes/RiotKit-macOS.xcscheme`

Goal: a `RiotKit-macOS` static-lib target that compiles the shared reusable sources on macOS and nothing else — this proves the sources are portable before any app chrome exists. Build settings mirror the iOS RiotKit target but retarget the platform:

- `SDKROOT = macosx`, `MACOSX_DEPLOYMENT_TARGET = 14.0`, `ARCHS = arm64`, `SWIFT_VERSION = 6.0`, `DEFINES_MODULE = YES`, `PRODUCT_NAME = RiotKit`, `CODE_SIGNING_ALLOWED = NO`, `SKIP_INSTALL = YES`.
- `HEADER_SEARCH_PATHS = $(SRCROOT)/../../build/generated/riot-ffi` and `OTHER_SWIFT_FLAGS = $(inherited) -Xcc -fmodule-map-file=$(SRCROOT)/../../build/generated/riot-ffi/riot_ffiFFI.modulemap` (identical to iOS — `$(SRCROOT)` is `apps/macos`, so `../../build` resolves to the repo build dir).
- Source phase: the 15 shared reusable sources (generated FFI + Core `ProfileRepository`/`WrappingKeyStore` + `AppModel` + 6 Transport + 7 Design). **Not** `ConferenceShellView.swift` (app-target) or `RiotApp.swift` (iOS entry).

- [ ] Write `project.pbxproj` with the RiotKit-macOS target and the shared file references (fresh `C0…` UUID space). Add the shared test scheme.
- [ ] Verify the sources compile on macOS (this is the load-bearing check for the whole plan):
  ```
  sh scripts/conference/build-native-core.sh            # ensures generated FFI + macOS lib exist
  xcodebuild build -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'
  ```
  Must succeed with no UIKit / platform errors. If a shared source fails to compile on macOS, STOP — the portability premise is broken; record the specific file/API in `COLLABORATION.md` and reconsider (do not `#if os()`-fork an iOS source from this project).

```
git add apps/macos/Riot.xcodeproj
git commit -m "feat(macos): RiotKit-macOS static-lib target compiling shared iOS sources"
```

---

### Task 3: macOS app target, entry, Info.plist, entitlements, fonts

**Files:**
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj` (own project — free)
- Create: `apps/macos/Riot/RiotMacApp.swift`
- Create: `apps/macos/Riot/Info.plist`
- Create: `apps/macos/Riot/Riot.entitlements`
- Add: `apps/macos/Riot/Resources/Fonts/` (the four TTFs)

`RiotMacApp.swift` mirrors `RiotApp.swift` — `@main struct RiotMacApp: App { @StateObject … ; WindowGroup { ConferenceShellView(model:).task { model.bootstrap() } } }`. Add `.defaultSize` and optionally a `.commands { }` menu block; keep it minimal for v1.

`Info.plist`: `ATSApplicationFontsPath = Fonts`, `NSBluetoothAlwaysUsageDescription` and `NSLocalNetworkUsageDescription` with the same strings the iOS target sets via `INFOPLIST_KEY_*`. No `UIAppFonts`, no launch-screen key.

`Riot.entitlements`: `com.apple.security.app-sandbox = true`, `com.apple.developer.default-data-protection = NSFileProtectionComplete`, `keychain-access-groups = [$(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)]`, `com.apple.security.device.bluetooth = true`, `com.apple.security.network.client = true`, `com.apple.security.network.server = true`.

App-target build settings mirror the iOS app target but: `SDKROOT = macosx`, `MACOSX_DEPLOYMENT_TARGET = 14.0`, `PRODUCT_BUNDLE_IDENTIFIER = net.protest.riot` (or `.mac` if a distinct bundle id is wanted), `LIBRARY_SEARCH_PATHS = $(SRCROOT)/../../build/native/macos`, `OTHER_LDFLAGS = $(inherited) -lriot_ffi`, `CODE_SIGN_ENTITLEMENTS = Riot/Riot.entitlements`, `INFOPLIST_FILE = Riot/Info.plist`, `RIOT_KEYCHAIN_ACCESS_GROUP = $(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)`.

- [ ] Add the Riot-macOS app target (compiles referenced `ConferenceShellView.swift` + new `RiotMacApp.swift`, links RiotKit-macOS + `-lriot_ffi`), a Copy-Files/Resources phase bundling `Fonts/`, and the app scheme.
- [ ] Verify build:
  ```
  xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS -destination 'platform=macOS'
  ```

```
git add apps/macos/Riot apps/macos/Riot.xcodeproj
git commit -m "feat(macos): Riot-macOS app target with entry, entitlements, and bundled fonts"
```

---

### Task 4: verify on macOS — tests, launch, keychain persistence

**Files:**
- Create: `apps/macos/README.md`

The iOS logic tests live in the `RiotTests` bundle against the iOS project. For macOS v1, prove the reusable layer runs on macOS by adding the platform-independent tests to a `RiotKitTests-macOS` target (reference the same `apps/ios/RiotTests/*.swift` that have no iOS-only assumptions — `BindingSemanticsTests`, `RiotThemeTests`, `RiotTabBarTests`, `TransportContractTests`; skip anything that asserts iOS-only behavior). If a test references iOS-only API, leave it out of the macOS bundle and note it in the README.

- [ ] Add a `RiotKitTests-macOS` unit-test target to `project.pbxproj` referencing the portable test sources, wired into the `RiotKit-macOS` scheme's test action.
- [ ] Run the macOS test suite:
  ```
  xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'
  ```
  Must be green.
- [ ] **Signed launch + keychain check** (the -34018 risk). Build a signed local app and launch it — do NOT rely on a bare `xcodebuild install` (the iOS session documented that unsigned/archive-style installs throw spurious Keychain entitlement errors):
  ```
  xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
    -destination 'platform=macOS' -derivedDataPath build/macos-dd
  open build/macos-dd/Build/Products/Debug/Riot.app
  ```
  Confirm: the app window opens on the Spaces screen, Anton/Work Sans/Space Mono render, the flat 2px-bordered card + tab-bar identity is correct, and the wrapping key persists across a relaunch (no repeated key creation in the log — filter `log stream --predicate 'subsystem == "net.protest.riot"'` for the `identity-keychain` category; expect the `when-passcode-set`/`when-unlocked` notice exactly once per fresh install, not per launch).
- [ ] Write `apps/macos/README.md`: prerequisites (`aarch64-apple-darwin` target, Xcode 26.2), the `build-native-core.sh` step, the two `xcodebuild` commands above, and the signing note.

```
git add apps/macos/README.md apps/macos/Riot.xcodeproj
git commit -m "test(macos): portable RiotKit tests on macOS + signed-launch keychain verification"
```

---

### Task 5 (tombstone / later phase): macOS JS apps runtime

**Not implemented in this plan.** The JS apps runtime host on iOS
(`docs/superpowers/plans/2026-07-11-js-apps-runtime-ios.md`, Tasks 6–10) builds
`apps/ios/Riot/Apps/{RiotJS,AppSchemeHandler,AppBridgeController,AppRuntimeView}.swift`
and adds app methods to `ProfileRepository.swift`. Of those, `AppRuntimeView.swift` is a
`UIViewRepresentable` WKWebView host — **iOS-only**. The macOS runtime is a *separate,
gated* effort:

- **Gate:** the iOS runtime plan's host tasks must be **Done** (files exist, `apps/ios/`
  rows updated in `COLLABORATION.md`).
- **Then:** the macOS project references `RiotJS.swift`, `AppSchemeHandler.swift`,
  `AppBridgeController.swift` (all WebKit, portable) and adds a new
  `apps/macos/Riot/Apps/AppRuntimeView.swift` that is an **`NSViewRepresentable`**
  wrapping the same `WKWebView` + `riot-app://` scheme + `window.riot` bridge. Link
  WebKit (`import WebKit`, no explicit link entry). This is its own spec/plan when the
  gate opens.

---

## Done criteria (v1)

- `sh scripts/conference/test-native-core-package.sh` — PASS (includes macOS slice).
- `xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'` — green.
- `Riot.app` launches on macOS, Spaces screen renders with fonts + identity, wrapping key persists across relaunch on a signed build.
- `apps/ios/Riot.xcodeproj` and every `apps/ios/Riot/` source: **unchanged** by this plan.
