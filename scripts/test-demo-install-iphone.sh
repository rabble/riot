#!/bin/sh
set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
HELPER="$ROOT/scripts/lib/demo-device-list.sh"

if [ ! -r "$HELPER" ]; then
    echo "FAIL: missing $HELPER" >&2
    exit 1
fi

# shellcheck source=scripts/lib/demo-device-list.sh
. "$HELPER"

fixture=$(mktemp -t riot-demo-devices)
trap 'rm -f "$fixture"' EXIT HUP INT TERM

cat >"$fixture" <<'JSON'
{
  "result": {
    "devices": [
      {
        "identifier": "iphone-device-id",
        "hardwareProperties": {"deviceType": "iPhone", "reality": "physical"}
      },
      {
        "identifier": "simulator-id",
        "hardwareProperties": {"deviceType": "iPhone", "reality": "virtual"}
      },
      {
        "identifier": "ipad-device-id",
        "hardwareProperties": {"deviceType": "iPad", "reality": "physical"}
      },
      {
        "identifier": "mac-device-id",
        "hardwareProperties": {"deviceType": "Mac", "reality": "physical"}
      }
    ]
  }
}
JSON

expected='iphone-device-id
ipad-device-id'
actual=$(list_physical_ios_device_ids_from_json "$fixture")

if [ "$actual" != "$expected" ]; then
    echo "FAIL: expected physical iPhone/iPad identifiers:" >&2
    printf '%s\n' "$expected" >&2
    echo "got:" >&2
    printf '%s\n' "$actual" >&2
    exit 1
fi

echo "PASS: demo installer selects physical iPhone and iPad identifiers from devicectl JSON"
