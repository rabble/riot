# Phase 0A — WU0 Report: Preflight, Contracts, Pins

- **Status:** PASS
- **Owning work unit:** WU0
- **Date:** 2026-07-10
- **Elapsed agent-hours:** ~1.0 of 2.0 budgeted

## What was proven

1. **Frozen environment achieved and recorded.** Every pin in `fixtures/manifest.json` reflects an actually installed, verified version: Rust 1.95.0 (toolchain-pinned), Xcode 26.2/Swift 6.2.3, JDK 17.0.19, AGP 9.0.1, Gradle 9.1.0 (wrapper committed), Build-Tools 36.0.0, NDK 28.2.13676358, Platform-Tools 37.0.0, Emulator 36.6.11, cmdline-tools zip 14742923 (sha256 recorded), system image `android-36;google_apis;arm64-v8a` revision 7 (`source.properties` sha256 recorded).
2. **The pinned arm64 AVD (`riot-phase0a`) boots** headless and reports `sys.boot_completed`.
3. **A blank instrumentation test passes on it**: `BlankInstrumentationTest.instrumentationContextTargetsEvidencePackage`, run via `gradle :app:connectedDebugAndroidTest` — 1 test, BUILD SUCCESSFUL.
4. **Empty `riot-ffi` compiles in release for all four runtime targets**: `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android`, `x86_64-linux-android` (NDK linkers wired in `.cargo/config.toml`).
5. **The full pinned dependency graph resolves and compiles**: 230 packages locked; `willow25 =0.5.0`, `uniffi =0.32.0`, `minicbor =2.2.2`, `cddl-cat =0.7.1`, `sha2 =0.10.9`, `ed25519-dalek =2.2.0` (restricted features), `rand_core =0.6.4`. `Cargo.lock` committed, sha256 in the manifest.
6. **Contract validator follows TDD**: RED run enumerated the absent contracts (`schemas/alert.cddl`, `fixtures/manifest.json`); GREEN run passes with all pins, ceilings, fixture-ownership, and report-field requirements present. Command: `cargo xtask validate-contracts`.

## Exact commands (as run)

```
rustup toolchain install 1.95.0
rustup target add --toolchain 1.95.0 aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android x86_64-linux-android
brew install openjdk@17
sdkmanager --install "platform-tools" "emulator" "build-tools;36.0.0" "system-images;android-36;google_apis;arm64-v8a"
avdmanager create avd -n riot-phase0a -k "system-images;android-36;google_apis;arm64-v8a" --device pixel_7
emulator -avd riot-phase0a -no-window -no-audio -no-boot-anim -no-snapshot
cargo xtask validate-contracts        # RED then GREEN
cargo build --workspace
cargo build -p riot-ffi --release --target <each of 4 targets>
gradle :app:connectedDebugAndroidTest # in apps/android, JAVA_HOME=JDK17
gradle wrapper --gradle-version 9.1.0
```

## Evidence paths and hashes

- `fixtures/manifest.json` — frozen pins, ceilings, fixture ownership, report fields.
- `Cargo.lock` — sha256 `5d5ea10add923766d2ec0a6021b958540ed1053a963e7ee450c7348b992200a5`.
- cmdline-tools zip — sha256 `ed304c5ede3718541e4f978e4ae870a4d853db74af6c16d920588d48523b9dee`.
- system image `source.properties` — sha256 `f53d5fbbb89420d911b9e5ed9243aff60caad24132a76e649efc8dfd96295731`.
- `apps/android/` — host app + passing blank instrumentation test.

## Deviations and notes

- The host already had an Android SDK with the exact pinned NDK; missing components were installed rather than assumed absent (the design allowed either).
- Gradle dependency locking is deferred to WU3 with the real Android host app, as recorded in the manifest (`gradle_locks_sha256: pending`).
- Deterministic-provider and forbidden-feature closure scans run in WU4 as designed; nothing in the current graph includes OpenMLS or group code.

## Next action

WU1 — deterministic alert/bundle codec, one ephemeral communal-author path, one cross-subspace denial (`cargo test -p riot-core public_`), starting from its named failing tests.
