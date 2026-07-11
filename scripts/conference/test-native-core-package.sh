#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)

mkdir -p "$ROOT/build/native/android/jniLibs/stale-abi"
: > "$ROOT/build/native/android/jniLibs/stale-abi/libstale.so"
"$ROOT/scripts/conference/build-native-core.sh"

test ! -e "$ROOT/build/native/android/jniLibs/stale-abi/libstale.so" || {
    echo "native-core-package: stale output survived rebuild" >&2
    exit 1
}

required_files="
build/generated/riot-ffi/riot_ffi.swift
build/generated/riot-ffi/riot_ffiFFI.h
build/generated/riot-ffi/riot_ffiFFI.modulemap
build/generated/riot-ffi/uniffi/riot_ffi/riot_ffi.kt
build/native/ios-device/libriot_ffi.a
build/native/ios-simulator/libriot_ffi.a
build/native/macos/libriot_ffi.a
build/native/android/jniLibs/arm64-v8a/libriot_ffi.so
build/native/android/jniLibs/x86_64/libriot_ffi.so
"

for relative in $required_files; do
    test -s "$ROOT/$relative" || {
        echo "native-core-package: missing $relative" >&2
        exit 1
    }
done

file "$ROOT/build/native/ios-device/libriot_ffi.a" | grep -q 'ar archive'
file "$ROOT/build/native/ios-simulator/libriot_ffi.a" | grep -q 'ar archive'
file "$ROOT/build/native/macos/libriot_ffi.a" | grep -q 'ar archive'
lipo -info "$ROOT/build/native/ios-device/libriot_ffi.a" | grep -q 'architecture: arm64'
lipo -info "$ROOT/build/native/ios-simulator/libriot_ffi.a" | grep -q 'architecture: arm64'
lipo -info "$ROOT/build/native/macos/libriot_ffi.a" | grep -q 'architecture: arm64'
otool -l "$ROOT/build/native/ios-device/libriot_ffi.a" | grep -q 'LC_VERSION_MIN_IPHONEOS'
otool -l "$ROOT/build/native/ios-simulator/libriot_ffi.a" | grep -A3 -m1 'LC_BUILD_VERSION' | grep -q 'platform 7'
file "$ROOT/build/native/android/jniLibs/arm64-v8a/libriot_ffi.so" | grep -q 'ELF 64-bit LSB shared object, ARM aarch64'
file "$ROOT/build/native/android/jniLibs/x86_64/libriot_ffi.so" | grep -q 'ELF 64-bit LSB shared object, x86-64'

echo "native-core-package: PASS"
