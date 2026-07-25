#!/bin/sh
# Asserts the ORDER of the developerid packaging steps.
#
# The ordering is the whole point. Notarizing only the .dmg leaves the .app
# inside it without a stapled ticket: Gatekeeper clears the app while the .dmg
# is around, but once a user drags the app to /Applications and trashes the
# .dmg, the first launch needs a network round trip to Apple and fails offline.
# The app must be notarized and stapled BEFORE it is sealed into the .dmg, and
# the .dmg notarized and stapled after.
#
# xcrun/hdiutil/ditto are stubbed so this runs offline with no Apple account.
set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
HELPER="$ROOT/scripts/lib/macos-dmg.sh"

if [ ! -r "$HELPER" ]; then
    echo "FAIL: missing $HELPER" >&2
    exit 1
fi

WORK=$(mktemp -d -t riot-macos-dmg)
trap 'rm -rf "$WORK"' EXIT HUP INT TERM

CALLS="$WORK/calls.log"
: >"$CALLS"

# Stubs record what the flow asks for, in order, and nothing else.
xcrun() {
    case "$1 $2" in
        "notarytool submit")
            # the submitted artifact is the third positional arg
            echo "notarize $(basename "$3")" >>"$CALLS" ;;
        "stapler staple")
            echo "staple $(basename "$3")" >>"$CALLS" ;;
        "stapler validate")
            echo "validate $(basename "$3")" >>"$CALLS" ;;
        *) echo "unexpected-xcrun $*" >>"$CALLS" ;;
    esac
}
hdiutil() {
    for a in "$@"; do
        case "$a" in *.dmg) echo "create $(basename "$a")" >>"$CALLS"; touch "$a"; return 0 ;; esac
    done
    echo "unexpected-hdiutil $*" >>"$CALLS"
}
ditto() {
    # the destination archive is the last positional arg
    last=""
    eval "last=\${$#}"
    # shellcheck disable=SC2154  # assigned by the eval above
    echo "zip $(basename "$last")" >>"$CALLS"
    touch "$last"
}

APP="$WORK/Riot.app"
mkdir -p "$APP/Contents/MacOS"
DMG="$WORK/Riot-1.dmg"

# shellcheck source=scripts/lib/macos-dmg.sh
. "$HELPER"

riot_package_developerid "$APP" "$DMG" "$WORK/stage" \
    "key-path" "key-id" "issuer-id"

actual=$(tr '\n' ' ' <"$CALLS" | sed 's/ *$//')
expected="zip Riot.zip notarize Riot.zip staple Riot.app validate Riot.app create Riot-1.dmg notarize Riot-1.dmg staple Riot-1.dmg validate Riot-1.dmg"

if [ "$actual" != "$expected" ]; then
    echo "FAIL: wrong packaging order" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    exit 1
fi

# The staged app must be the STAPLED one. If the flow copies the app into the
# staging dir before stapling it, the ticket never reaches the shipped copy —
# which is exactly the bug this test exists to prevent, and it is invisible in
# the call order alone.
staple_line=$(grep -n "^staple Riot.app$" "$CALLS" | cut -d: -f1)
create_line=$(grep -n "^create Riot-1.dmg$" "$CALLS" | cut -d: -f1)
if [ "$staple_line" -ge "$create_line" ]; then
    echo "FAIL: app stapled at line $staple_line, dmg built at line $create_line" >&2
    exit 1
fi

echo "PASS: developerid packaging staples the app before sealing it into the dmg"
