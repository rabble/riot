# Riot conference Android shell

This is the native Android half of the public incident-space conference demo.
It is deliberately small: Spaces, Incident board, Compose & sign, Import
preview, and Connection are the only top-level surfaces.

## Native-core inputs

The Android project does not build Rust itself. From the repository root, run:

```sh
scripts/conference/build-native-core.sh
```

Gradle consumes the generated outputs from these exact ignored paths:

- Kotlin UniFFI source: `build/generated/riot-ffi/uniffi/`
- arm64 library: `build/native/android/jniLibs/arm64-v8a/libriot_ffi.so`
- x86_64 library: `build/native/android/jniLibs/x86_64/libriot_ffi.so`

The generated Kotlin uses JNA. The app pins the Android JNA AAR while the Rust
library remains the authority for identity, draft validation, signing,
preview, and import acceptance.

## Local persistence boundary

`AndroidKeystoreProfileStore` encodes a bounded profile snapshot, encrypts it
with AES-GCM using a non-exportable Android Keystore key, and replaces its file
atomically. Signed bundle bytes are retained so a fresh in-memory core profile
can restore entries through the same inspect, preview, plan, and accept path.
Accepted imports are persisted with their original opaque bundle and exercise
that same restore path. Both the document reader and complete encrypted profile
have a 4 MiB-class total ceiling; oversized writes fail before replacing the
last valid snapshot.
The binding-semantic device test turns no network on and asserts that full
entry, namespace, and signer IDs, freshness, and the AI-assistance disclosure
survive that reload.

The current Rust mobile boundary does not yet export a durable signing-key
container. Existing signed entries restore faithfully, but opening a fresh
core profile creates a new local signing identity for later posts. Production
key continuity therefore remains a core/FFI follow-up, not a claim of this
shell.

## Reproduce the checks

The proven environment uses JDK 17 and the API 36 SDK:

```sh
export JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"

cd apps/android
./gradlew \
  :app:testDebugUnitTest \
  :app:assembleDebug \
  :app:assembleDebugAndroidTest \
  :app:connectedDebugAndroidTest
```

The connected suite requires a running API 36 emulator or device. It exercises
the actual generated binding and packaged Rust library; it is not a fake-core
UI test.
