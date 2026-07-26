#!/usr/bin/env bash
# Release build for the Riot Android app — a signed AAB for Google Play, and a
# signed APK for direct install.
#
# The Apple twin of this is scripts/testflight-release.sh, and it works the same
# way: build the native core for every shipped ABI, refuse a dirty tree, derive a
# monotonic version code, then build.
#
# SIGNING. The upload keystore lives OUTSIDE the checkout
# (~/riot-release-key.jks) and its password lives in the macOS keychain, not in
# a file. This script reads the password with `security find-generic-password`
# and hands it to Gradle through the release process environment, so the secret
# never lands on disk, enters the repo, or appears in the process argument list.
# This release entrypoint fails closed when signing is unavailable; contributors
# can still run Gradle directly when they need an unsigned local build.
#
#   security add-generic-password -s riot-android-keystore \
#     -a riot-release-key -w '<password>'
#
# Usage:
#   sh scripts/android-release.sh          # signed AAB + APK
#   ALLOW_DIRTY=1 sh scripts/android-release.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# ONE RELEASE BUILD AT A TIME, PER CHECKOUT.
#
# All three release scripts regenerate build/generated/riot-ffi and write
# build/native — shared paths. Running two at once means one deletes the
# bindings the other is mid-compile against, and the failure looks like a
# missing file rather than a race:
#   error opening input file '.../build/generated/riot-ffi/riot_ffi.swift'
# mkdir is atomic on every filesystem this runs on, so it is the lock.
LOCK_DIR="$ROOT/build/.release-lock"
if ! mkdir "$LOCK_DIR" 2>/dev/null; then
  echo "ERROR: another release build is already running in this checkout." >&2
  echo "       ($LOCK_DIR exists — remove it if a previous run was killed.)" >&2
  exit 1
fi
trap 'rmdir "$LOCK_DIR" 2>/dev/null || true' EXIT


KEYSTORE="${RIOT_KEYSTORE_PATH:-$HOME/riot-release-key.jks}"
KEYCHAIN_SERVICE="riot-android-keystore"
KEYCHAIN_ACCOUNT="riot-release-key"
EXPECTED_CERT_SHA256="8552c62a629638292884aafb2431fde89032210c606c2be44d766cf8daeac084"

# Play rejects a version code it has already seen, so it must increase on every
# upload. Commit count is monotonic and needs no clock — the same rule the iOS
# and macOS release scripts use, so all three platforms advance together.
VERSION_CODE="$(git rev-list --count HEAD)"

# Refuse to ship an unknown mix. Cargo and Gradle build the WORKING TREE, and
# root manifests, toolchain files, xtask, or local build configuration can all
# change the artifact just as surely as app or crate sources can.
if [ -n "$(git status --porcelain)" ] && [ "${ALLOW_DIRTY:-0}" != "1" ]; then
  echo "ERROR: the checkout has uncommitted changes." >&2
  echo "       The release would not correspond to the reported Git revision." >&2
  echo "       Commit/stash first, or ALLOW_DIRTY=1." >&2
  git status --short >&2
  exit 1
fi

if [ ! -f "$KEYSTORE" ]; then
  echo "ERROR: Android release keystore not found at $KEYSTORE." >&2
  echo "       See docs/release/google-play-setup.md." >&2
  exit 1
fi
if ! command -v security >/dev/null 2>&1; then
  echo "ERROR: macOS Keychain command 'security' is unavailable." >&2
  exit 1
fi
if ! PASSWORD="$(security find-generic-password \
  -s "$KEYCHAIN_SERVICE" -a "$KEYCHAIN_ACCOUNT" -w 2>/dev/null)" ||
  [ -z "$PASSWORD" ]; then
  echo "ERROR: Android release keystore password is absent from the macOS keychain." >&2
  echo "       Expected service '$KEYCHAIN_SERVICE', account '$KEYCHAIN_ACCOUNT'." >&2
  exit 1
fi

echo "==> Riot Android, version code $VERSION_CODE, at $(git rev-parse --short HEAD)"

echo "==> native core (arm64-v8a + x86_64, net-enabled)"
# BOTH ABIs, or the bundle ships an app that dies on launch on the architecture
# it is missing. `--features net` + net bindings because the app calls
# bindNetRuntime: a mismatch between generated bindings and the built .so links
# fine and dies at the first FFI call.
SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Library/Android/sdk}}"
NDK_ROOT="${ANDROID_NDK_HOME:-$SDK_ROOT/ndk/28.2.13676358}"
TOOLCHAIN="$NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin"
test -x "$TOOLCHAIN/aarch64-linux-android26-clang" || {
  echo "ERROR: NDK clang absent at $TOOLCHAIN" >&2
  exit 1
}

RIOT_FFI_NET_BINDINGS=1 cargo run --locked --package xtask -- generate-bindings

CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TOOLCHAIN/aarch64-linux-android26-clang" \
  CC_aarch64_linux_android="$TOOLCHAIN/aarch64-linux-android26-clang" \
  AR_aarch64_linux_android="$TOOLCHAIN/llvm-ar" \
  cargo build --locked -p riot-ffi --lib --release --features net --target aarch64-linux-android

CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$TOOLCHAIN/x86_64-linux-android26-clang" \
  CC_x86_64_linux_android="$TOOLCHAIN/x86_64-linux-android26-clang" \
  AR_x86_64_linux_android="$TOOLCHAIN/llvm-ar" \
  cargo build --locked -p riot-ffi --lib --release --features net --target x86_64-linux-android

mkdir -p build/native/android/jniLibs/arm64-v8a build/native/android/jniLibs/x86_64
install -m 0644 target/aarch64-linux-android/release/libriot_ffi.so \
  build/native/android/jniLibs/arm64-v8a/libriot_ffi.so
install -m 0644 target/x86_64-linux-android/release/libriot_ffi.so \
  build/native/android/jniLibs/x86_64/libriot_ffi.so

echo "==> signing with $KEYSTORE (password from the macOS keychain)"
echo "==> bundleRelease (the AAB Play takes) + assembleRelease (APK for direct install)"
cd apps/android
RIOT_KEYSTORE_PATH="$KEYSTORE" \
  RIOT_KEYSTORE_PASSWORD="$PASSWORD" \
  RIOT_KEY_ALIAS="riot" \
  ./gradlew :app:bundleRelease :app:assembleRelease "-PversionCode=$VERSION_CODE"
unset PASSWORD

AAB="$ROOT/apps/android/app/build/outputs/bundle/release/app-release.aab"
APK="$ROOT/apps/android/app/build/outputs/apk/release/app-release.apk"
echo
[ -f "$AAB" ] || {
  echo "ERROR: Gradle did not produce $AAB" >&2
  exit 1
}
[ -f "$APK" ] || {
  echo "ERROR: Gradle did not produce $APK" >&2
  exit 1
}
echo "==> AAB: $AAB"
echo "==> APK: $APK"

# Prove both signatures rather than trusting the build log: the AAB is the
# artifact Play receives, while the APK is what a direct-download user runs.
APKSIGNER=""
for candidate in "$SDK_ROOT"/build-tools/*/apksigner; do
  [ -x "$candidate" ] && APKSIGNER="$candidate"
done
if [ -z "$APKSIGNER" ]; then
  echo "ERROR: apksigner not found under $SDK_ROOT/build-tools" >&2
  exit 1
fi

APK_CERTS="$("$APKSIGNER" verify --print-certs "$APK")"
printf '%s\n' "$APK_CERTS" | sed -n '1,4p'
APK_CERT_SHA256="$(printf '%s\n' "$APK_CERTS" |
  sed -n 's/^Signer #1 certificate SHA-256 digest: //p' | head -1)"
if [ "$APK_CERT_SHA256" != "$EXPECTED_CERT_SHA256" ]; then
  echo "ERROR: APK was signed by an unexpected certificate." >&2
  echo "       Expected SHA-256: $EXPECTED_CERT_SHA256" >&2
  echo "       Actual SHA-256:   ${APK_CERT_SHA256:-missing}" >&2
  exit 1
fi

command -v jarsigner >/dev/null 2>&1 || {
  echo "ERROR: jarsigner is required to verify the AAB." >&2
  exit 1
}
command -v keytool >/dev/null 2>&1 || {
  echo "ERROR: keytool is required to inspect the AAB certificate." >&2
  exit 1
}
jarsigner -verify "$AAB"
AAB_CERTS="$(keytool -J-Duser.language=en -J-Duser.country=US \
  -printcert -jarfile "$AAB")"
AAB_CERT_SHA256="$(printf '%s\n' "$AAB_CERTS" |
  sed -n 's/^[[:space:]]*SHA256: //p' | head -1 |
  tr -d ':' | tr '[:upper:]' '[:lower:]')"
if [ "$AAB_CERT_SHA256" != "$EXPECTED_CERT_SHA256" ]; then
  echo "ERROR: AAB was signed by an unexpected certificate." >&2
  echo "       Expected SHA-256: $EXPECTED_CERT_SHA256" >&2
  echo "       Actual SHA-256:   ${AAB_CERT_SHA256:-missing}" >&2
  exit 1
fi

echo
echo "Upload the AAB at https://play.google.com/console — Verse Communication PBC."
