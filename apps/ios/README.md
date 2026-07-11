# Riot iOS conference shell

This is a native SwiftUI iOS app for the fixed public `incident-board/1`
conference demo. It links the Rust core through generated UniFFI bindings; it
does not load package code, publish model output automatically, or silently use
an internet transport.

## Generated artifact contract

From the repository root, run the native packaging command first:

```sh
scripts/conference/build-native-core.sh
```

The Xcode project expects these ignored build products exactly:

- `build/generated/riot-ffi/riot_ffi.swift`
- `build/generated/riot-ffi/riot_ffiFFI.h`
- `build/generated/riot-ffi/riot_ffiFFI.modulemap`
- `build/native/ios-simulator/libriot_ffi.a`

The conference simulator archive is arm64, so the simulator project is pinned
to arm64. A device/TestFlight archive requires the corresponding iOS-device
Rust artifact and signing configuration; those are intentionally not implied
by the simulator build.

## Build and test

```sh
xcodebuild test \
  -project apps/ios/Riot.xcodeproj \
  -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-derived

xcodebuild build \
  -project apps/ios/Riot.xcodeproj \
  -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-app-derived
```

`BindingSemanticsTests` signs through the real generated API and proves a fresh
in-memory Rust profile can rehydrate the signed alert offline from protected
native storage. Full entry, namespace, and signer IDs, freshness, and the
AI-assistance disclosure survive reload. The first-launch regression also
proves an empty protected profile renders an empty board instead of asking the
core for entries before a space exists.

## Persistence boundary

UniFFI deliberately exposes no private key or opaque profile serialization.
The app therefore persists only public space metadata and portable signed
bundle bytes in an iOS Data Protection file using
`completeUntilFirstUserAuthentication`. On reload it opens a fresh Rust
profile, joins the public namespace, and runs every bundle through the existing
inspect, select, and accept path. This is suitable for the public conference
demo; stronger key continuity and device-keychain lifecycle work remain before
a production release.

The current shell includes exactly five surfaces: Spaces, Incident board,
Compose & sign, Import preview, and Connection. Nearby transport is not yet
wired; the connection surface explicitly reports offline/local-device-only.
