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

Signer continuity uses two explicit encryption layers. Rust seals its opaque
112-byte identity state with a random 32-byte wrapping key. That wrapping key
and sealed state exist only inside the bounded profile plaintext, whose entire
envelope is encrypted by the non-exportable Android Keystore AES-GCM key. On
restart, Android decrypts the profile and passes temporary byte-array copies to
the narrow `openProfileFromSealedIdentity` binding; temporary wrapping-key
copies are zeroed in `finally` after both seal and open calls. The encoded and
decrypted profile plaintext arrays are likewise cleared after AES-GCM or decode,
and the codec wipes its internal byte-stream buffer on close. The codec computes
the exact cumulative encoded size before allocating that buffer, and encrypted
file length is rejected from metadata before `AtomicFile.readFully()`. This is not a
claim that Rust receives or holds a direct Android Keystore key handle.

Version-one profiles without identity state migrate safely: their content and
space restore first, the resulting local identity is sealed into version two,
and subsequent restarts retain that signer. The device suite signs before and
after a fresh controller/core restart and compares the complete 64-character
signer identifier. Its migration fixture is a real AES-GCM-encrypted v1 profile
written with the same Android Keystore alias, not an identity-less v2 stand-in.

## Nearby phone transport

The Connection surface advertises and scans at the same time, shows only a
short session name such as `Blue Kite`, and requires a person on each phone to
confirm. After both confirmations, the phones exchange a numeric local address
over Bluetooth and make one direct local-network socket attempt. A successful
socket is fixed for that session; otherwise the session is fixed to bounded,
ordered Bluetooth frames. It never switches transport per message and never
tries an internet host.

Android requires the `INTERNET` manifest permission for direct LAN sockets as
well as internet sockets. Riot constrains this permission in code: the socket
connector rejects DNS names and public IPv4/IPv6 addresses before dialing. On
Android 12 and newer, discovery requests scan, connect, and advertise Bluetooth
permissions. Android 11 and older use fine location because that is the
platform requirement for Bluetooth scanning on those versions.

`SyncCoordinator` moves opaque frames between the selected nearby connection
and the generated Rust session. A reviewed canonical bundle is preflighted and
written through the encrypted profile store before core acceptance. Completion
closes the generated handle and the nearby connection; failure cancels both.
The screen deliberately uses plain connection language rather than protocol or
radio terminology.

The JVM suite proves friendly naming, explicit confirmation, remote-confirmation
waiting, bounded byte-for-byte framing, disconnect/retry, the single local
attempt, session-fixed fallback, and public-address rejection. The emulator
suite proves manifest and Android-version permission wiring. Real discovery,
pairing, and transfer between two physical phones remains the documented
hardware rehearsal gate because Android emulators cannot validate the radios.

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
