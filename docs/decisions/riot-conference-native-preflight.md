# Riot Conference Native Preflight

Date: 2026-07-11

This note records the local build environment proven before the conference
native shells are expanded. It is a toolchain baseline, not evidence that the
Riot mobile bindings or physical-device handoff are complete.

## iOS

- Xcode 26.2 (`17C52`)
- Swift 6.2.3
- Available iOS 26.1 and 26.2 simulator runtimes
- Installed Rust targets: `aarch64-apple-ios`, `aarch64-apple-ios-sim`

## Android

- SDK: `/Users/rabble/Library/Android/sdk`
- JDK 17: `/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home`
- Gradle 9.1.0 / Android Gradle Plugin 9.0.1
- compile/target SDK 36; min SDK 26
- Installed Rust targets: `aarch64-linux-android`, `x86_64-linux-android`
- AVDs: `riot-phase0a`, `Medium_Phone_API_36.1`

The login shell defaults to JDK 26 and does not export the Android SDK paths.
Native build/rehearsal scripts must set the pinned paths explicitly.

## Verified commands

With `JAVA_HOME`, `ANDROID_HOME`, `ANDROID_SDK_ROOT`, and `PATH` set to the
locations above:

```sh
cd apps/android
./gradlew :app:assembleDebug :app:assembleDebugAndroidTest
./gradlew :app:connectedDebugAndroidTest
```

Results:

- Debug application APK assembled.
- Debug instrumentation APK assembled.
- `riot-phase0a` API 36 emulator booted successfully.
- Existing instrumentation baseline ran 1 test with 0 failures.

## Remaining proof

- Generated Swift and Kotlin UniFFI bindings must compile in the native apps.
- The Rust libraries must be packaged for simulator/emulator and physical
  device targets.
- The two-device incremental handoff has not yet run.
- TestFlight/Play testing signing and upload are intentionally outside this
  repository's credentials and must use the established release accounts.
