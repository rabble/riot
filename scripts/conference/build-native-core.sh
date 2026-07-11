#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
SDK_ROOT=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-"$HOME/Library/Android/sdk"}}
NDK_ROOT=${ANDROID_NDK_HOME:-"$SDK_ROOT/ndk/28.2.13676358"}
API_LEVEL=26

if [ "$(uname -s)" != Darwin ]; then
    echo "native-core-package: macOS is required for the combined Apple/Android build" >&2
    exit 1
fi
NDK_HOST=darwin-x86_64

TOOLCHAIN="$NDK_ROOT/toolchains/llvm/prebuilt/$NDK_HOST/bin"
AARCH64_CLANG="$TOOLCHAIN/aarch64-linux-android${API_LEVEL}-clang"
X86_64_CLANG="$TOOLCHAIN/x86_64-linux-android${API_LEVEL}-clang"

test -x "$AARCH64_CLANG" || {
    echo "native-core-package: Android NDK clang absent at $AARCH64_CLANG" >&2
    exit 1
}
test -x "$X86_64_CLANG" || {
    echo "native-core-package: Android NDK clang absent at $X86_64_CLANG" >&2
    exit 1
}

for target in aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android x86_64-linux-android; do
    rustup target list --installed | grep -qx "$target" || {
        echo "native-core-package: rust target not installed: $target" >&2
        exit 1
    }
done

cd "$ROOT"
cargo run --locked --package xtask -- generate-bindings
cargo build -p riot-ffi --lib --release --locked --target aarch64-apple-ios
cargo build -p riot-ffi --lib --release --locked --target aarch64-apple-ios-sim

CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$AARCH64_CLANG" \
    CC_aarch64_linux_android="$AARCH64_CLANG" \
    cargo build -p riot-ffi --lib --release --locked --target aarch64-linux-android
CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$X86_64_CLANG" \
    CC_x86_64_linux_android="$X86_64_CLANG" \
    cargo build -p riot-ffi --lib --release --locked --target x86_64-linux-android

rm -rf "$ROOT/build/native"
mkdir -p \
    build/native/ios-device \
    build/native/ios-simulator \
    build/native/android/jniLibs/arm64-v8a \
    build/native/android/jniLibs/x86_64

install -m 0644 target/aarch64-apple-ios/release/libriot_ffi.a \
    build/native/ios-device/libriot_ffi.a
install -m 0644 target/aarch64-apple-ios-sim/release/libriot_ffi.a \
    build/native/ios-simulator/libriot_ffi.a
install -m 0644 target/aarch64-linux-android/release/libriot_ffi.so \
    build/native/android/jniLibs/arm64-v8a/libriot_ffi.so
install -m 0644 target/x86_64-linux-android/release/libriot_ffi.so \
    build/native/android/jniLibs/x86_64/libriot_ffi.so

echo "native-core-package: built iOS device/simulator and Android arm64/x86_64"
