# Riot native core packaging

Status: implemented for the conference native-shell build.

`scripts/conference/build-native-core.sh` is the single macOS entry point for
the generated UniFFI sources and native Rust libraries. It uses the locked
Cargo graph, iOS targets installed through rustup, Android API 26, and NDK
28.2.13676358 unless `ANDROID_NDK_HOME` explicitly selects another installed
NDK. The script recreates its owned `build/native/` tree so stale ABI products
cannot leak into an app package. Build products stay under the repository's
ignored `build/` directory.

## Artifact contract

- Swift: `build/generated/riot-ffi/riot_ffi.swift`
- C header/module map: `build/generated/riot-ffi/riot_ffiFFI.{h,modulemap}`
- Kotlin: `build/generated/riot-ffi/uniffi/riot_ffi/riot_ffi.kt`
- iOS device: `build/native/ios-device/libriot_ffi.a`
- Apple-silicon iOS simulator: `build/native/ios-simulator/libriot_ffi.a`
- Android arm64: `build/native/android/jniLibs/arm64-v8a/libriot_ffi.so`
- Android x86_64 emulator: `build/native/android/jniLibs/x86_64/libriot_ffi.so`

The iOS and Android projects reference these paths. Generated sources and
binaries are recreated before native builds rather than committed.

## TDD and verification

The package test first failed because the build entry point did not exist. It
now builds all four release targets and verifies each expected source/library
is non-empty and has the expected archive or ELF architecture.

```bash
scripts/conference/test-native-core-package.sh
```

This is a build/linkage contract only. Keychain/Keystore persistence, app UI,
and runtime binding semantics remain native-app responsibilities.
