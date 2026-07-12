#!/bin/sh
# Build Riot and install it on every connected iPhone.
#
#   sh scripts/demo-install-iphone.sh
#
# If a phone is listed but "unavailable", it is locked, not trusted, or on a
# charge-only cable: unlock it, tap Trust, and try a data cable.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

DD="$ROOT/build/ios-device-derived"

echo "==> native core (device slice)"
sh scripts/conference/build-native-core.sh >/dev/null

echo "==> building signed app for iOS"
xcodebuild build \
    -project apps/ios/Riot.xcodeproj \
    -scheme Riot \
    -destination 'generic/platform=iOS' \
    -allowProvisioningUpdates \
    -derivedDataPath "$DD" \
    >/dev/null

APP="$DD/Build/Products/Debug-iphoneos/Riot.app"
[ -d "$APP" ] || { echo "no app at $APP" >&2; exit 1; }
echo "    signed: $(codesign -dv "$APP" 2>&1 | grep -o 'TeamIdentifier=.*' || echo 'unsigned')"

# Every device that is actually reachable right now.
#
# Match the STATE COLUMN exactly. A substring match is a trap: "unavailable"
# contains "available", so a naive grep happily tries to install to a phone
# that macOS cannot even see, and fails with an opaque CoreDevice error.
DEVICES=$(xcrun devicectl list devices 2>/dev/null \
    | awk '/iPhone|iPad/ && ($(NF-1) == "available" || $(NF-1) == "connected") {print $(NF-2)}')

if [ -z "$DEVICES" ]; then
    echo ""
    echo "No reachable iPhone. It is plugged in but macOS cannot talk to it if:"
    echo "  - the phone is locked           -> unlock it"
    echo "  - you never tapped Trust        -> replug, unlock, tap Trust"
    echo "  - the cable is charge-only      -> use a data cable"
    echo ""
    xcrun devicectl list devices 2>/dev/null | head -5
    exit 1
fi

failed=0
for id in $DEVICES; do
    echo "==> installing on $id"
    if xcrun devicectl device install app --device "$id" "$APP" >/dev/null 2>&1; then
        echo "    installed"
    else
        echo "    FAILED on $id" >&2
        failed=1
    fi
done

if [ "$failed" -ne 0 ]; then
    echo ""
    echo "At least one install failed — do NOT assume the phone is ready." >&2
    exit 1
fi

echo ""
echo "Done. Open Riot on each phone -> Connect. They pair with no taps."
