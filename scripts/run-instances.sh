#!/bin/sh
# Launch N Riot instances on this Mac as N different people.
#
# Two windows of the same app share one container — one profile, one identity —
# so syncing them is a no-op. RIOT_PROFILE_ID gives each its own profile inside
# the container (an arbitrary path would be refused by App Sandbox), so they are
# genuinely distinct peers.
#
# They find each other over Bonjour, not Bluetooth: a single BLE controller never
# hears its own advertisement, so same-machine discovery over the radio is
# impossible by construction.
#
#   sh scripts/run-instances.sh 2
#
# Then in each window: Connect -> "Find nearby phones". Each should list the
# other by its friendly name. Confirm on both sides; nothing syncs without it.

set -eu

COUNT="${1:-2}"
APP="${RIOT_APP:-build/macos-derived/Build/Products/Debug/Riot.app}"
BIN="$APP/Contents/MacOS/Riot"

if [ ! -x "$BIN" ]; then
    echo "No macOS build at $APP" >&2
    echo "Build it first:" >&2
    echo "  sh scripts/conference/build-native-core.sh" >&2
    echo "  xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \\" >&2
    echo "    -destination 'platform=macOS' -derivedDataPath build/macos-derived" >&2
    exit 1
fi

i=1
while [ "$i" -le "$COUNT" ]; do
    echo "launching instance $i (RIOT_PROFILE_ID=instance-$i)"
    RIOT_PROFILE_ID="instance-$i" "$BIN" &
    i=$((i + 1))
done

echo ""
echo "$COUNT instance(s) running. Each has its own identity."
echo "Reset an instance's profile by deleting its directory under:"
echo "  ~/Library/Containers/net.protest.riot/Data/Library/Application Support/instances/"
echo ""
echo "Stop them all:  pkill -f 'Riot.app/Contents/MacOS/Riot'"

wait
