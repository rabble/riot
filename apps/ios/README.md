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
core for entries before a space exists. The restart regression signs on both
sides of a simulated process death and asserts the exact full signer ID remains
unchanged while the earlier content is restored offline.

## Persistence boundary

UniFFI exposes authenticated sealed identity bytes, never raw signer or Willow
secret types. The app generates a random 32-byte wrapping key with
`SecRandomCopyBytes` and stores only that key in Keychain. It requests
`kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`; a device-only
`kSecAttrAccessibleWhenUnlockedThisDeviceOnly` fallback is used only when the
platform rejects the passcode-gated class. The simulator verification used the
stronger passcode-gated class. The Keychain access group expands from
`$(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)`, so device/TestFlight
signing supplies the real team prefix rather than relying on a hard-coded one.

The protected profile file uses `completeUntilFirstUserAuthentication` and
contains public space metadata, portable signed bundle bytes, and the 112-byte
authenticated sealed identity. It never contains the wrapping key. On reload,
the app obtains the wrapping key, restores the signer, reattaches its public
namespace, and runs every bundle through the existing inspect, select, and
accept path. Mutable wrapping-key buffers are overwritten immediately after
the FFI call returns.

Old snapshots without sealed state migrate without discarding their signed
content: the app creates and seals a new signer in the existing public
namespace, then restores the historical bundles. That migration cannot recover
the old private signer, so signer continuity begins at migration; corrupt or
wrong-key sealed state fails closed instead of silently rotating identity.

The current shell includes exactly five surfaces: Spaces, Incident board,
Compose & sign, Import preview, and Connection. Nearby transport is not yet
wired; the connection surface explicitly reports offline/local-device-only.
