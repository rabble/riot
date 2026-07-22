# Riot

Riot is a native app for offline civic information during internet shutdowns, protests, disasters, and other moments where chat is not enough.

The core idea is to preload the app before a crisis, then let people create, sign, render, exchange, and merge local-first information packets while offline. Packets can behave like small local apps or websites: alerts, resource maps, legal guides, medical checklists, mutual aid boards, and evolving incident pages.

Riot has two sides, built as parallel subsystems joined only by an explicit bridge:

- An **open newswire** for public publishing and movement media: per-incident open spaces anyone can post to, and publication spaces where a pseudonymous collective is the publisher. Participant-held copies and replaceable gateways can reduce dependence on one server, but Riot does not guarantee that a complete reachable copy exists or that publishing, access, persistence, or censorship resistance survives.
- **Private groups — Direction, not shipped**: intended encrypted, unlinkable Willow namespaces for affinity groups, coops, and crews, with in-person QR or portable encrypted invite-file joining. Do not rely on this mode today.

The design spec is [2026-07-10-riot-dual-mode-design.md](file:///Users/rabble/code/explorations/riot/docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md).

---

## Repository Layout

The project is structured as a monorepo containing the following components:

- [apps/](file:///Users/rabble/code/explorations/riot/apps/)
  - [android/](file:///Users/rabble/code/explorations/riot/apps/android/) - Native Kotlin Android app (Jetpack Compose, Android Keystore, JNA)
  - [gateway/](file:///Users/rabble/code/explorations/riot/apps/gateway/) - Python mirror server for rendering newswire site bundles
  - [ios/](file:///Users/rabble/code/explorations/riot/apps/ios/) - Native SwiftUI iOS app (Keychain, BLE/LAN SyncCoordinator, UniFFI)
  - [macos/](file:///Users/rabble/code/explorations/riot/apps/macos/) - Native macOS app compiling the iOS components by reference
- [crates/](file:///Users/rabble/code/explorations/riot/crates/)
  - [riot-core/](file:///Users/rabble/code/explorations/riot/crates/riot-core/) - Rust engine (Willow protocol, schemas, signing, verification)
  - [riot-ffi/](file:///Users/rabble/code/explorations/riot/crates/riot-ffi/) - Rust FFI boundary (UniFFI binding target)
  - [riot-app-cli/](file:///Users/rabble/code/explorations/riot/crates/riot-app-cli/) - `riot-app` command line utility
  - [riot-conformance/](file:///Users/rabble/code/explorations/riot/crates/riot-conformance/) - Test and conformance fixtures
  - [xtask/](file:///Users/rabble/code/explorations/riot/crates/xtask/) - Rust workspace task runner (binding generation, contract verification)
- [docs/](file:///Users/rabble/code/explorations/riot/docs/) - Research, product framing, and technical designs
- [fixtures/](file:///Users/rabble/code/explorations/riot/fixtures/) - Frozen test data, including WILLIAM3 test vectors
- [schemas/](file:///Users/rabble/code/explorations/riot/schemas/) - Schema validation definitions (e.g., [alert.cddl](file:///Users/rabble/code/explorations/riot/schemas/alert.cddl))

---

## Build & Test Instructions

### Workspace Prerequisites
- **Rust Toolchain**: Stable release channel
- **Python**: 3.10+ (for gateway server and tests)
- **Xcode**: 26.2+ (for Apple simulator and device builds)
- **Java JDK 17 & Android SDK**: (with API 36 and NDK 28.2+) for Android targets

---

### 1. Rust Engine & CLI Tool

All Rust components are managed within the root cargo workspace.

```sh
# Run all workspace tests (TDD enforced, 100% coverage target)
cargo test --workspace --all-features

# Run static analysis and clippy checks
cargo clippy --workspace --all-features -- -D warnings

# Validate environment contracts (locked pins, ceiling limits, schemas)
cargo xtask validate-contracts
```

To build and use the command-line packager (`riot-app`):
```sh
# Show CLI usage instructions
cargo run -p riot-app-cli -- --help

# Generate a public/private signer keypair
cargo run -p riot-app-cli -- keygen --out ~/.riot-keys

# Pack a local directory into a Willow site bundle
cargo run -p riot-app-cli -- pack <app-dir> --key-dir ~/.riot-keys --out app.bundle

# Inspect a generated site bundle file
cargo run -p riot-app-cli -- inspect app.bundle
```

* **Build Output Location**:
  * Binary executable: [target/debug/riot-app](file:///Users/rabble/code/explorations/riot/target/debug/riot-app) (or `target/release/riot-app` with `--release` flags)

---

### 2. Mobile FFI Bindings

The mobile apps consume a unified library built from `crates/riot-ffi` using UniFFI bindings.

To generate bindings only:
```sh
cargo xtask generate-bindings
```

To build the native binary library for all target architectures (macOS, iOS, iOS Simulator, Android arm64/x86_64) and compile bindings:
```sh
sh scripts/conference/build-native-core.sh
```

* **Build Output Locations**:
  * **UniFFI Generated Bindings**: [build/generated/riot-ffi/](file:///Users/rabble/code/explorations/riot/build/generated/riot-ffi/)
  * **iOS Device Library**: [build/native/ios-device/libriot_ffi.a](file:///Users/rabble/code/explorations/riot/build/native/ios-device/libriot_ffi.a)
  * **iOS Simulator Library**: [build/native/ios-simulator/libriot_ffi.a](file:///Users/rabble/code/explorations/riot/build/native/ios-simulator/libriot_ffi.a)
  * **macOS Library**: [build/native/macos/libriot_ffi.a](file:///Users/rabble/code/explorations/riot/build/native/macos/libriot_ffi.a)
  * **Android arm64 Library**: [build/native/android/jniLibs/arm64-v8a/libriot_ffi.so](file:///Users/rabble/code/explorations/riot/build/native/android/jniLibs/arm64-v8a/libriot_ffi.so)
  * **Android x86_64 Library**: [build/native/android/jniLibs/x86_64/libriot_ffi.so](file:///Users/rabble/code/explorations/riot/build/native/android/jniLibs/x86_64/libriot_ffi.so)

---

### 3. iOS & macOS Apps

Ensure you have run `sh scripts/conference/build-native-core.sh` first to generate native library dependencies and bindings.

#### iOS
```sh
# Run unit tests in the iOS Simulator
xcodebuild test \
  -project apps/ios/Riot.xcodeproj \
  -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-derived

# Build the iOS Simulator App
xcodebuild build \
  -project apps/ios/Riot.xcodeproj \
  -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-app-derived
```

#### macOS
```sh
# Run unit tests on macOS
xcodebuild test \
  -project apps/macos/Riot.xcodeproj \
  -scheme RiotKit-macOS \
  -destination 'platform=macOS'

# Build the macOS App
xcodebuild build \
  -project apps/macos/Riot.xcodeproj \
  -scheme Riot-macOS \
  -destination 'platform=macOS' \
  -derivedDataPath build/macos-derived
```

* **Build Output Locations**:
  * **iOS Simulator App Bundle**: [build/ios-app-derived/Build/Products/Debug-iphonesimulator/Riot.app](file:///Users/rabble/code/explorations/riot/build/ios-app-derived/Build/Products/Debug-iphonesimulator/Riot.app)
  * **macOS App Bundle**: [build/macos-derived/Build/Products/Debug/Riot.app](file:///Users/rabble/code/explorations/riot/build/macos-derived/Build/Products/Debug/Riot.app)

---

### 4. Android App

Ensure Android NDK toolchain is configured in your environment and run `sh scripts/conference/build-native-core.sh` to populate native libraries in `build/native/android`.

```sh
cd apps/android

# Run JVM unit tests and build both the application and Android test APKs
./gradlew \
  :app:testDebugUnitTest \
  :app:assembleDebug \
  :app:assembleDebugAndroidTest
```

To run device/instrumentation tests, ensure you have an API 36 emulator or physical device connected:
```sh
./gradlew :app:connectedDebugAndroidTest
```

* **Build Output Locations**:
  * **Debug Application APK**: [apps/android/app/build/outputs/apk/debug/app-debug.apk](file:///Users/rabble/code/explorations/riot/apps/android/app/build/outputs/apk/debug/app-debug.apk)
  * **Instrumented Test APK**: [apps/android/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk](file:///Users/rabble/code/explorations/riot/apps/android/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk)

---

### 5. Public Web Gateway

The gateway serves signed site bundles locally to standard web browsers.

```sh
cd apps/gateway

# Run Python gateway test suite
python3 -m unittest discover -s tests

# Start the local gateway web server serving a demo workspace export
python3 server.py \
  --export ../../fixtures/conference/gateway-space/public-export-v1.json \
  --port 8080
```
Open your browser to `http://127.0.0.1:8080/site/` to browse the gateway output.

---

## Development Utility Scripts

The project includes several high-quality helper scripts in the [scripts/](file:///Users/rabble/code/explorations/riot/scripts/) directory to make developer workflows smoother and provide a better UX:

### 1. All-in-One Verification Wrapper (`green.sh`)
An extremely useful verification tool that validates that the main branch is "green" before committing or after pulling. It compiles/tests the Rust workspace, compiles the iOS application for physical phones (to ensure no missing target files), compiles the macOS demo app, and runs the Android unit tests. It isolates build logs under `/tmp` to keep stdout clean, formats output, and asserts that Apple tests run more than zero suites (preventing silent crashes from returning success).
```sh
# Run the complete test suite across all platforms
sh scripts/green.sh

# Run platform builds/tests but skip Rust cargo tests for speed
sh scripts/green.sh fast
```

### 2. Multi-Instance Testing (`run-instances.sh`)
Testing offline synchronization between identical applications on a single machine can be tricky since multiple windows normally share a single profile. This macOS script spins up $N$ distinct isolated instances of the macOS app with custom `RIOT_PROFILE_ID` env parameters so they behave as unique Bonjour peers.
```sh
# 1. Build native core and macOS application
sh scripts/conference/build-native-core.sh
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS -destination 'platform=macOS' -derivedDataPath build/macos-derived

# 2. Launch 2 distinct instances
sh scripts/run-instances.sh 2
```

### 3. Gateway Smoke Tests (`gateway-smoke.sh`)
A robust integration test script for the Python web gateway. It dynamically binds to a free loopback port, starts the HTTP server, checks endpoints for correct HTML nodes (e.g. Incident Board, QR values), asserts exact security headers (Content-Security-Policy, X-Content-Type-Options, Referrer-Policy), and terminates child processes safely on completion or exit.
```sh
sh scripts/conference/gateway-smoke.sh
```
