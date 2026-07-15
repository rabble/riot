# Native app CI — requirements (not yet built)

Status: **deferred, tracked.** `.github/workflows/ci.yml` covers Rust, gateway,
and web on Ubuntu. It deliberately does **not** build the iOS or Android apps.
This note records exactly what native CI needs so the next session starts from a
spec instead of rediscovering the blocker.

## Why the app builds can't just run in CI

Both native app builds consume a **generated, gitignored tree** that exists only
after a local codegen + cross-compile step:

- `.gitignore` ignores `build/`; `git ls-files build` returns zero tracked files.
- `apps/android/app/build.gradle.kts:28` adds `../../build/generated/riot-ffi/uniffi`
  as a Kotlin **source root**, and line 29 wires `jniLibs` from
  `../../build/native/android/jniLibs`. `RiotController.kt` imports
  `uniffi.riot_ffi.*`. So even `:app:testDebugUnitTest` cannot compile on a clean
  checkout — the generated Kotlin bindings are not there.
- `apps/ios/Riot.xcodeproj/project.pbxproj` references
  `build/generated/riot-ffi/riot_ffi.swift` + `riot_ffiFFI.modulemap` and links
  `-lriot_ffi` from `build/native/ios-simulator`. There are **zero
  `PBXShellScriptBuildPhase` entries** in the iOS or macOS project — Xcode does
  not regenerate any of this itself.

A fresh CI runner clones a tree with none of `build/` present, so both app builds
fail at compile before any test runs. This is invisible today because the only
thing that builds the apps is `scripts/green.sh`, run locally on a machine that
already has the artifacts.

## What a native CI job must do first

Before any `gradlew` / `xcodebuild` step:

1. Install the pinned Rust toolchain (`rust-toolchain.toml`, 1.95.0) on the runner.
2. `cargo run -p xtask -- generate-bindings` — produces
   `build/generated/riot-ffi/{riot_ffi.swift, riot_ffiFFI.modulemap, uniffi/…}`.
3. Cross-compile the native staticlibs:
   - **Android:** cargo-ndk for each ABI, output to `build/native/android/jniLibs`.
   - **Apple:** device + simulator staticlibs, output to
     `build/native/{ios-device, ios-simulator, macos}`.
4. Only then run the app builds/tests:
   - Android (ubuntu, with SDK + NDK): `./gradlew :app:testDebugUnitTest`.
   - Apple (macOS runner): `xcodebuild build` for the iOS/macOS schemes, plus
     `xcodebuild test` for the RiotKit scheme. This is the only thing that catches
     the two failure modes `scripts/green.sh` documents: a Swift file committed but
     never added to an Xcode target, and a call whose symbol definition was not
     committed.

## Cost note

The Apple job needs a macOS runner (billed at a multiplier). Scope it to build +
RiotKit test rather than the full UI suite, and consider PR-only rather than
every push if cost bites. `scripts/green.sh` and `scripts/web/bootstrap.sh`
already encode most of the toolchain setup and are the reference for pins.
