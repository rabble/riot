# Riot macOS App — Design

**Status:** Draft (2026-07-11). Requested by rabble: "a macOS app version too."

**Scope of this doc:** decide *how* Riot reaches macOS, define a v1 feature cut, and
fix the file layout so the macOS work does not collide with the concurrently-active
iOS runtime session. No code; the plan is `docs/superpowers/plans/2026-07-11-riot-macos.md`.

## Context: what the iOS app actually is

The iOS app is unusually portable. Evidence from `apps/ios/Riot/`:

- **Zero UIKit.** Every source imports only `SwiftUI`, `Foundation`, `OSLog`,
  `Security`, `CoreBluetooth` (`@preconcurrency`), `Network` (`@preconcurrency`),
  `Darwin`, and `RiotKit`. Grep for `UIKit`/`UIApplication`/`UIViewController`/
  `UIScreen` returns nothing. The entry point is a plain SwiftUI `App`
  (`RiotApp.swift`: `WindowGroup { ConferenceShellView(...) }`) — App lifecycle, not
  `@UIApplicationDelegate`.
- **RiotKit is a static-library target** (`com.apple.product-type.library.static`)
  that compiles the entire reusable layer: the generated FFI Swift
  (`build/generated/riot-ffi/riot_ffi.swift`), `Core/ProfileRepository.swift`,
  `Core/WrappingKeyStore.swift`, `AppModel.swift`, all six `Transport/*.swift`, and
  all seven `Design/*.swift`. The **app target** compiles only `RiotApp.swift` +
  `ConferenceShellView.swift` and links `-lriot_ffi` + RiotKit.
- **The Design system is pure SwiftUI** (`RiotTheme`, `RiotCard`, `RiotButtonStyle`,
  `RiotBadge`, `RiotHeader`, `RiotEmptyState`, `RiotTabBar`) — no `UIColor`/`UIFont`/
  `UIScreen`. Portable as-is.
- **Nearby transport is CoreBluetooth + Network**, both first-class on macOS. The BLE
  channel (`CoreBluetoothNearby.swift`) and LAN channel (`LocalNetworkChannel.swift`,
  Darwin sockets + `Network`) have no iOS-only dependency.

The two places the app is *not* portable are at the platform-boundary layers, not in
the logic:

1. **Info.plist** uses iOS-only keys: `UIAppFonts` (font registration) and
   `INFOPLIST_KEY_UILaunchScreen_Generation`. macOS registers bundled fonts via
   `ATSApplicationFontsPath` and has no launch screen.
2. **Keychain accessibility.** `Core/WrappingKeyStore.swift` stores the wrapping key
   with `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly` (fallback
   `WhenUnlockedThisDeviceOnly`) and the entitlement
   `keychain-access-groups = $(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)`.
   Those `kSecAttrAccessible` classes require the **data-protection keychain**, which
   on macOS only exists for apps that are sandboxed and carry the data-protection
   entitlement. Handled at the entitlement layer (below) so the code is reused verbatim.

## Decision: separate `apps/macos/` Xcode project referencing the shared sources

Add a **new `apps/macos/Riot.xcodeproj` with its own `project.pbxproj`** that:

- compiles the **same RiotKit source files by relative reference**
  (`../ios/Riot/Core/*`, `../ios/Riot/Transport/*`, `../ios/Riot/Design/*`,
  `AppModel.swift`, `ConferenceShellView.swift`, and the generated
  `../../build/generated/riot-ffi/riot_ffi.swift`) — no copies, no source forks;
- adds a **macOS-only app entry** (`apps/macos/Riot/RiotMacApp.swift`), macOS
  `Info.plist` (`ATSApplicationFontsPath`), and macOS `Riot.entitlements`
  (App Sandbox + data-protection keychain);
- links a **macOS build of `libriot_ffi.a`** (`aarch64-apple-darwin`).

Nothing in `apps/ios/Riot.xcodeproj` is edited. The macOS project *references* the iOS
Swift files; references are not edits and do not conflict with the iOS session's own
edits to those files (a reference simply compiles whatever is on disk).

### Alternatives weighed and rejected

- **Mac Catalyst.** Rejected. Catalyst exists to bring UIKit apps to the Mac; this app
  has no UIKit, so Catalyst buys nothing and imposes iPad-shaped chrome and Catalyst
  keychain/entitlement quirks. Worse, enabling it means setting
  `SUPPORTS_MACCATALYST = YES` on the **existing Riot app target** — a build-settings
  edit to the exact `Riot.xcodeproj/project.pbxproj` the iOS runtime session will be
  editing. Direct contention for no benefit.
- **One multiplatform target** (add `macosx` to the Riot target's
  `SUPPORTED_PLATFORMS`). Rejected. Heavy edits to the existing target, and the
  iOS-only Info.plist keys (`UIAppFonts`, launch screen) would need `#if`/per-platform
  plists inside the contended project.
- **Extract RiotKit to a Swift package.** Cleanest long-term, and worth doing once the
  iOS runtime session is idle — but it requires *moving* the sources out of
  `apps/ios/Riot/` and re-pointing the iOS project at the package, which is maximal
  contention right now. Deferred; noted as the eventual convergence point for both apps.

## v1 feature cut

Ship the flows that RiotKit + a macOS `riot-ffi` already support with zero UIKit:

- **Spaces / newswire / evidence** screens — the existing `ConferenceShellView` tabs,
  driven by `RiotAppModel` and `ProfileRepository`.
- **Import / export** of evidence bundles (core FFI; no platform surface beyond a
  file picker, which is SwiftUI `.fileImporter`/`.fileExporter`).
- **Nearby sync** — CoreBluetooth + Network transport, macOS-native. Requires the
  `NSBluetoothAlwaysUsageDescription` / `NSLocalNetworkUsageDescription` usage strings
  (already present as `INFOPLIST_KEY_*` on iOS) plus the sandbox
  `com.apple.security.device.bluetooth` / `network.client` + `network.server`
  entitlements.

**Out of v1: the JS apps runtime.** The iOS runtime is a WKWebView host whose view
layer (`AppRuntimeView.swift`) is a `UIViewRepresentable` — iOS-only. On macOS it needs
an `NSViewRepresentable` twin. The bridge code (`RiotJS`, `AppSchemeHandler`,
`AppBridgeController`) is WebKit and portable, so a macOS runtime is *reuse the bridge +
new host view*, but it can only start once those files exist. See gating.

## Platform-boundary handling (the only non-reused pieces)

- **Fonts.** macOS `Info.plist` sets `ATSApplicationFontsPath = Fonts` and the target
  bundles the four TTFs from `apps/ios/Riot/Resources/Fonts/` into a `Fonts/` resource
  dir. No code change — `RiotTheme` already refers to the families by name.
- **Keychain.** macOS target gets `Riot.entitlements` with **App Sandbox**
  (`com.apple.security.app-sandbox`) + **data-protection keychain**
  (`com.apple.developer.default-data-protection = NSFileProtectionComplete`) +
  `keychain-access-groups`. With those, `WrappingKeyStore.swift` runs unmodified. The
  existing `WhenUnlocked...` fallback already covers the passcode-less path.
- **Window / navigation.** `WindowGroup` + the custom `RiotTabBar` work on macOS as-is
  for v1. A macOS-idiomatic `NavigationSplitView` sidebar and a menu-bar `.commands`
  block are polish, not v1.
- **Deployment target.** `MACOSX_DEPLOYMENT_TARGET = 14.0` (Sonoma — the Swift-6 /
  iOS-17 era peer), `ARCHS = arm64`, `SWIFT_VERSION = 6.0`, matching the iOS target.

## Rust core for macOS

`scripts/conference/build-native-core.sh` today builds `aarch64-apple-ios`,
`aarch64-apple-ios-sim`, `aarch64-linux-android`, `x86_64-linux-android` — **no
macOS slice.** The macOS app needs `target/aarch64-apple-darwin/release/libriot_ffi.a`.
Plan Task 1 extends that script (and its package test) to emit a
`build/native/macos/libriot_ffi.a`. The script is owned by "Codex root — Task 4 native
core packaging," marked **Done, released** in `COLLABORATION.md`, so it is free to
extend. `riot-ffi` is platform-neutral Rust; the macOS build is expected to be a clean
target addition, but this is unverified until built (see risks). Optional
`x86_64-apple-darwin` (Intel) is deferred — the repo is arm64-only everywhere else.

## Interlock with the iOS runtime session (gating)

`COLLABORATION.md` marks every `apps/ios/` row **Done, released**, so the files are
technically free — but `docs/superpowers/plans/2026-07-11-js-apps-runtime-ios.md` will
*later* create `apps/ios/Riot/Apps/*.swift` and **modify** `ProfileRepository.swift`,
`AppModel.swift`, `ConferenceShellView.swift`, and `Riot.xcodeproj/project.pbxproj`.

- **Task 1** (extend `build-native-core.sh` + `test-native-core-package.sh`): free now.
- **Task 2** (new `apps/macos/Riot.xcodeproj` skeleton + macOS entry/Info.plist/
  entitlements, referencing the current RiotKit sources): free now — its own pbxproj,
  references not edits.
- **Task 3+** (build/verify/screenshot the macOS shell): references
  `ConferenceShellView.swift`/`AppModel.swift`, which the iOS session will edit. This is
  not a file-edit conflict (the macOS project holds references, makes no edits to those
  files), but it is *semantically* gated — if a referenced file is mid-refactor, expect
  build churn and coordinate before changing behavior.
- **macOS JS apps runtime** (later phase, not in this plan): **hard-gated** on the iOS
  runtime plan reaching Done for its host tasks (so `Apps/RiotJS.swift`,
  `AppSchemeHandler.swift`, `AppBridgeController.swift` and the `ProfileRepository` app
  methods exist), then adds an `NSViewRepresentable` macOS host.

## Biggest risks

1. **macOS Rust lib is unproven.** `aarch64-apple-darwin` must be an installed rustup
   target and `riot-ffi` must build clean for it. Expected to be trivial, but until
   Task 1 runs it is an assumption. First failure mode to check.
2. **Keychain on an unsigned/dev macOS build.** The data-protection `kSecAttrAccessible`
   classes require a signed, entitled app; an unsigned local run can throw
   `errSecMissingEntitlement` (-34018) — the same false alarm the iOS visual-identity
   session chased. Mitigated by the sandbox + data-protection entitlements and the
   existing `WhenUnlocked` fallback, but verification must use a properly signed local
   build, not a bare `xcodebuild install`.

## Success criteria

`xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS
-destination 'platform=macOS'` is green (RiotKit sources compile + existing logic tests
run on macOS), the macOS app launches showing the Spaces screen with the Riot fonts and
flat-bordered identity rendering, and the wrapping key persists across relaunch on a
signed local build.
