#!/bin/sh
# Installing the App Store Connect key into the location altool searches must
# succeed when the key is ALREADY that file.
#
# Regression: the release scripts ran a bare `cp "$ASC_KEY_PATH" ~/.appstore
# connect/private_keys/AuthKey_$ID.p8`. Point ASC_KEY_PATH at the canonical
# location — which is exactly where the docs tell you to keep it — and cp gets
# source == destination, exits 1, and `set -e` kills the script one line before
# the upload. The build "succeeds" and silently never ships.
set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
HELPER="$ROOT/scripts/lib/asc-key.sh"

if [ ! -r "$HELPER" ]; then
    echo "FAIL: missing $HELPER" >&2
    exit 1
fi

WORK=$(mktemp -d -t riot-asc-key)
trap 'rm -rf "$WORK"' EXIT HUP INT TERM

# shellcheck source=scripts/lib/asc-key.sh
. "$HELPER"

DEST_DIR="$WORK/private_keys"

# Case 1: key lives elsewhere and must be copied in.
SRC="$WORK/AuthKey_ABC123.p8"
echo "key-material" >"$SRC"
riot_install_asc_key "$SRC" "ABC123" "$DEST_DIR"
if [ ! -f "$DEST_DIR/AuthKey_ABC123.p8" ]; then
    echo "FAIL: key not installed from an external path" >&2
    exit 1
fi
if [ "$(cat "$DEST_DIR/AuthKey_ABC123.p8")" != "key-material" ]; then
    echo "FAIL: installed key has wrong contents" >&2
    exit 1
fi

# Case 2: the regression. ASC_KEY_PATH already IS the destination file.
if ! riot_install_asc_key "$DEST_DIR/AuthKey_ABC123.p8" "ABC123" "$DEST_DIR"; then
    echo "FAIL: installing an already-installed key returned non-zero" >&2
    exit 1
fi
if [ "$(cat "$DEST_DIR/AuthKey_ABC123.p8")" != "key-material" ]; then
    echo "FAIL: already-installed key was clobbered" >&2
    exit 1
fi

# Case 3: a private credential must not be left world-readable.
mode=$(stat -f "%Lp" "$DEST_DIR/AuthKey_ABC123.p8")
if [ "$mode" != "600" ]; then
    echo "FAIL: installed key mode is $mode, want 600" >&2
    exit 1
fi

echo "PASS: asc key install is idempotent, preserves contents, and is mode 600"
