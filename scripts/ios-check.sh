#!/bin/sh
# Fast iOS dev loop — skip the ~5-10min iOS device build for most changes.
#
#   sh scripts/ios-check.sh          # fast: compile-check the shared SwiftUI on the macOS scheme
#   sh scripts/ios-check.sh test     # macOS RiotKit unit tests (logic)
#   sh scripts/ios-check.sh sim      # iOS SIMULATOR build (catches most iOS-only bits, faster than device)
#   sh scripts/ios-check.sh ios      # full iOS DEVICE build (target membership + iOS-only) — the final gate
#   sh scripts/ios-check.sh clean    # wipe the shared DerivedData (force a cold rebuild)
#
# WHY macOS-first: apps/macos builds the SAME apps/ios SwiftUI sources
# (ConferenceShellView, CommunityShell, the shell, …). A native macOS build has
# no simulator boot, no app packaging, no code signing — so it catches ~every
# type error, missing symbol, and API misuse in seconds-to-a-minute on a warm
# build, versus 5-10 min for `-destination generic/platform=iOS`.
#
# THE LOOP: iterate with `ios-check.sh` (fast) until it's green, THEN run
# `ios-check.sh ios` ONCE to confirm iOS target membership + anything behind
# `#if os(iOS)`. For pure layout iteration, use SwiftUI `#Preview` in Xcode's
# canvas (instant, no build at all).
#
# SPEED comes from two things this script does that a bare xcodebuild doesn't:
#   1. builds the macOS scheme (native) instead of the iOS device slice;
#   2. a PERSISTENT DerivedData dir (build/xcode-dd) that is never auto-cleaned,
#      so only the FIRST build in a worktree is cold; every later build is an
#      incremental recompile of just what changed.
# Override the DerivedData location (e.g. to share one warm cache across
# worktrees when you know builds won't overlap) with RIOT_DERIVED_DATA=/path.
set -u

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

DD="${RIOT_DERIVED_DATA:-$ROOT/build/xcode-dd}"
CMD="${1:-fast}"

# Build-only, no signing, no index store — we want the compiler's verdict fast.
set -- -derivedDataPath "$DD" -quiet \
       COMPILER_INDEX_STORE_ENABLE=NO CODE_SIGNING_ALLOWED=NO

case "$CMD" in
  fast)
    echo "macOS compile-check (shared SwiftUI) — DerivedData: $DD"
    xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
      -destination 'platform=macOS' "$@"
    ;;
  test)
    xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
      -destination 'platform=macOS' "$@"
    ;;
  sim)
    xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot \
      -destination 'platform=iOS Simulator,name=iPhone 17 Pro' "$@"
    ;;
  ios)
    echo "full iOS device build (the final gate) — DerivedData: $DD"
    xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot \
      -destination 'generic/platform=iOS' "$@"
    ;;
  clean)
    rm -rf "$DD" && echo "removed $DD"
    ;;
  *)
    echo "usage: ios-check.sh [fast|test|sim|ios|clean]" >&2
    exit 2
    ;;
esac
