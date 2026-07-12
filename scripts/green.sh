#!/bin/sh
# Is main actually green? Run this before you commit, and after you pull.
#
#   sh scripts/green.sh          # everything
#   sh scripts/green.sh fast     # skip the Rust suite (~2 min faster)
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
set -u

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

FAST="${1:-}"
fail=0
say() { printf '%-22s %s\n' "$1" "$2"; }

if [ "$FAST" != "fast" ]; then
    if cargo test --workspace --all-features >/tmp/green-rust.log 2>&1; then
        say "rust workspace" "GREEN ($(grep -c 'test result: ok' /tmp/green-rust.log) suites)"
    else
        say "rust workspace" "RED — see /tmp/green-rust.log"; fail=1
    fi
fi

if xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot \
        -destination 'generic/platform=iOS' >/tmp/green-ios.log 2>&1; then
    say "iOS app (phone)" "GREEN"
else
    say "iOS app (phone)" "RED — $(grep -m1 'error:' /tmp/green-ios.log | sed 's|.*/||')"; fail=1
fi

if xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
        -destination 'platform=macOS' >/tmp/green-macos.log 2>&1; then
    say "macOS app (demo rig)" "GREEN"
else
    say "macOS app (demo rig)" "RED — $(grep -m1 'error:' /tmp/green-macos.log | sed 's|.*/||')"; fail=1
fi

# A suite that runs NOTHING must never read as a pass.
#
# TwoPeerNearbySyncTests — our headline "two whole phones" test — crashed its host
# on launch (TCC: the test bundle had no NSBluetoothAlwaysUsageDescription, so
# touching CoreBluetooth killed it), and xcodebuild then cheerfully printed
# "Test Suite passed / Executed 0 tests". It read green for days while proving
# nothing. Zero executed tests is a RED, not a pass.
check_ran() {
    log="$1"; name="$2"
    if grep -q "Executed 0 tests" "$log" 2>/dev/null; then
        say "$name" "RED — a suite executed ZERO tests (crashed host?). A green tick that proves nothing is worse than a red one."
        return 1
    fi
    return 0
}

# The macOS test suite — the one that runs the transport/sync logic. This is where
# the zero-executed-tests lie lived, so it is checked here whether it passes or not.
if [ "$FAST" != "fast" ]; then
    xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
        -destination 'platform=macOS' >/tmp/green-mactests.log 2>&1
    rc=$?
    if ! check_ran /tmp/green-mactests.log "macOS tests"; then
        fail=1
    elif [ "$rc" -eq 0 ]; then
        say "macOS tests" "GREEN ($(grep -oE 'Executed [0-9]+ tests' /tmp/green-mactests.log | tail -1))"
    else
        say "macOS tests" "RED — see /tmp/green-mactests.log"; fail=1
    fi
fi

if [ -d apps/android ]; then
    if (cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew testDebugUnitTest \
            >/tmp/green-android.log 2>&1); then
        say "android unit tests" "GREEN"
    else
        say "android unit tests" "RED — see /tmp/green-android.log"; fail=1
    fi
fi

echo ""
if [ "$fail" -eq 0 ]; then
    echo "GREEN. Safe to commit."
else
    echo "RED. Do not commit on top of this — fix it or tell the coordinator." >&2
    exit 1
fi
