# Riot for macOS

A native macOS app built from the same `RiotKit` sources as the iOS app —
every shared file is compiled **by reference** from `apps/ios/Riot/`; this
project owns zero copies. See
`docs/superpowers/specs/2026-07-11-riot-macos-design.md`.

## Prerequisites

- Xcode 26.2+, Apple Silicon Mac
- Rust with the `aarch64-apple-darwin` target (`rustup target add aarch64-apple-darwin`)

## Build

```sh
# 1. Native core (produces build/native/macos/libriot_ffi.a + generated Swift)
sh scripts/conference/build-native-core.sh

# 2. Library + portable tests
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'

# 3. The app
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS -destination 'platform=macOS'
```

## Signing and the Keychain

The app target is ad-hoc signed (`CODE_SIGN_IDENTITY=-`) with App Sandbox +
data-protection entitlements. The wrapping key persists in the login
keychain (`net.protest.riot.identity-wrapping.v2`); `WrappingKeyStore` logs
a `notice` on **creation only** — silence on subsequent launches is the
pass condition. Do not judge Keychain health from archive-style installs
(see the iOS session's -34018 false-alarm note in COLLABORATION.md).

## Test-suite scope

`RiotKitTests-macOS` compiles the portable iOS suites by reference:
`BindingSemanticsTests`, `RiotTabBarTests`, `TransportContractTests`
(37 tests). Left out: `RiotThemeTests` (uses `UIColor` — iOS-only),
`ShellNavigationTests` (exercises the app shell, not the library),
`AppRuntimeHostTests` (iOS JS-apps runtime, not yet ported — the macOS JS
runtime is a gated later phase, plan Task 5).

## Verified / deferred

Verified on this machine: app builds, launches, relaunches with the
wrapping key reused (no re-creation notices), fonts bundled under
`Contents/Resources/Fonts/`. Deferred: pixel-level visual pass (headless
`screencapture` is blocked in this environment; the views are the same
RiotKit components the iOS session verified in the simulator) and
two-machine nearby-sync rehearsal.
