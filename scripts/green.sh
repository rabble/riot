#!/bin/sh
# Is main actually green? Run this before you commit, and after you pull.
#
#   sh scripts/green.sh          # everything
#   sh scripts/green.sh fast     # skip the Rust suite + macOS tests (~2 min faster)
#
# It checks the things that have ACTUALLY broken during this project:
#   - the Rust workspace
#   - the iOS app (a phone build — this is what ships to the demo phones)
#   - the macOS app (the demo rig)
#   - the Android unit tests
#
# Two failure modes it exists to catch, both of which cost us hours:
#   - a Swift file committed but never added to an Xcode target
#   - a call committed whose symbol's definition was not
#
# The checks are independent, so they run in PARALLEL (one lane each: rust, iOS,
# macOS build+tests, android). Wall time is the slowest lane, not the sum.
# Output is still printed in a fixed order once every lane finishes.
set -u

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

FAST="${1:-}"
say() { printf '%-22s %s\n' "$1" "$2"; }

# Each lane writes its human-readable result line to /tmp/green-<key>.status.
# A line beginning with "RED" counts as a failure when we tally at the end.
status() { printf '%s\n' "$2" >"/tmp/green-$1.status"; }

# A suite that runs NOTHING must never read as a pass.
#
# TwoPeerNearbySyncTests — our headline "two whole phones" test — crashed its host
# on launch (TCC: the test bundle had no NSBluetoothAlwaysUsageDescription, so
# touching CoreBluetooth killed it), and xcodebuild then cheerfully printed
# "Test Suite passed / Executed 0 tests". It read green for days while proving
# nothing. Zero executed tests is a RED, not a pass.

lane_rust() {
    if cargo test --workspace --all-features >/tmp/green-rust.log 2>&1; then
        status rust "GREEN ($(grep -c 'test result: ok' /tmp/green-rust.log) suites)"
    else
        status rust "RED — see /tmp/green-rust.log"
    fi
}

lane_ios() {
    if xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot \
            -destination 'generic/platform=iOS' >/tmp/green-ios.log 2>&1; then
        status ios "GREEN"
    else
        status ios "RED — $(grep -m1 'error:' /tmp/green-ios.log | sed 's|.*/||')"
    fi
}

# macOS build and macOS tests share the same Xcode project, so they run
# sequentially within one lane (parallel xcodebuild on one project contends on
# DerivedData). This whole lane runs in parallel with rust / iOS / android.
lane_macos() {
    if xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
            -destination 'platform=macOS' >/tmp/green-macos.log 2>&1; then
        status macos "GREEN"
    else
        status macos "RED — $(grep -m1 'error:' /tmp/green-macos.log | sed 's|.*/||')"
    fi

    [ "$FAST" = "fast" ] && return

    xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
        -destination 'platform=macOS' >/tmp/green-mactests.log 2>&1
    rc=$?
    if grep -q "Executed 0 tests" /tmp/green-mactests.log 2>/dev/null; then
        status mactests "RED — a suite executed ZERO tests (crashed host?). A green tick that proves nothing is worse than a red one."
    elif [ "$rc" -eq 0 ]; then
        status mactests "GREEN ($(grep -oE 'Executed [0-9]+ tests' /tmp/green-mactests.log | tail -1))"
    else
        status mactests "RED — see /tmp/green-mactests.log"
    fi
}

lane_android() {
    if (cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest \
            >/tmp/green-android.log 2>&1); then
        status android "GREEN"
    else
        status android "RED — see /tmp/green-android.log"
    fi
}

# Fixed print order regardless of which lane finishes first.
# One "key:Label" per line — labels may contain spaces, so iterate line by line.
ORDER=$(printf '%s\n' \
    "rust:rust workspace" \
    "ios:iOS app (phone)" \
    "macos:macOS app (demo rig)" \
    "mactests:macOS tests" \
    "android:android unit tests")

# Clear stale status files so a lane we skip this run can't show an old result.
printf '%s\n' "$ORDER" | while IFS= read -r pair; do
    rm -f "/tmp/green-${pair%%:*}.status"
done

# Launch the lanes.
[ "$FAST" != "fast" ] && lane_rust &
lane_ios &
lane_macos &
[ -d apps/android ] && lane_android &
wait

# Tally and print.
fail=0
rm -f /tmp/green-failflag
printf '%s\n' "$ORDER" | while IFS= read -r pair; do
    key=${pair%%:*}; label=${pair#*:}
    [ -f "/tmp/green-$key.status" ] || continue
    result=$(cat "/tmp/green-$key.status")
    say "$label" "$result"
    case "$result" in RED*) printf 1 >/tmp/green-failflag ;; esac
done
[ -f /tmp/green-failflag ] && fail=1

echo ""
if [ "$fail" -eq 0 ]; then
    echo "GREEN. Safe to commit."
else
    echo "RED. Do not commit on top of this — fix it or tell the coordinator." >&2
    exit 1
fi
