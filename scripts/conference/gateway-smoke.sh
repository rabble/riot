#!/usr/bin/env sh
# Verify the dependency-free public gateway against a loopback server.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
PORT=${PORT:-$(python3 - <<'PY'
import socket

with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)}
LOG_FILE=$(mktemp)
PID=""

cleanup() {
    if [ -n "$PID" ]; then
        kill "$PID" 2>/dev/null || true
        wait "$PID" 2>/dev/null || true
    fi
    rm -f "$LOG_FILE"
}
trap cleanup EXIT HUP INT TERM

python3 "$ROOT/apps/gateway/server.py" --host 127.0.0.1 --port "$PORT" >"$LOG_FILE" 2>&1 &
PID=$!

READY=""
for _ in $(seq 1 30); do
    if python3 - "$PORT" <<'PY' >/dev/null 2>&1
from urllib.request import urlopen
import sys

with urlopen(f"http://127.0.0.1:{sys.argv[1]}/site/", timeout=0.2) as response:
    assert response.status == 200
PY
    then
        READY=1
        break
    fi
    sleep 0.1
done

if [ -z "$READY" ]; then
    cat "$LOG_FILE" >&2
    exit 1
fi

python3 - "$PORT" "$ROOT/fixtures/conference/gateway-space/public-export-v1.json" <<'PY'
import hashlib
import json
from pathlib import Path
import sys
from urllib.error import HTTPError
from urllib.request import Request, urlopen

port, export_path = sys.argv[1:]
base = f"http://127.0.0.1:{port}"
with urlopen(f"{base}/site/", timeout=2) as response:
    page = response.read().decode("utf-8")
    assert response.status == 200
    assert response.headers["Content-Security-Policy"] == "default-src 'none'; script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'"
    assert response.headers["X-Content-Type-Options"] == "nosniff"
    assert response.headers["Referrer-Policy"] == "no-referrer"

for required in (
    "Harbor District Evacuation",
    "incident-board/1",
    "Claimed author (unverified fixture):",
    "Freshness:",
    "AI-assisted draft",
    "Available offline from this local public export",
    "Open in Riot",
    "<svg",
    "data-qr-value=\"riot://open?namespace=3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c\"",
):
    assert required in page, required

for request, expected_status in (
    (Request(f"{base}/private/incident-board"), 404),
    (Request(f"{base}/site/", method="POST", data=b"{}"), 405),
):
    try:
        urlopen(request, timeout=2)
    except HTTPError as error:
        assert error.code == expected_status, error.code
        assert error.headers["Content-Security-Policy"] == "default-src 'none'; script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'"
        assert error.headers["X-Content-Type-Options"] == "nosniff"
        assert error.headers["Referrer-Policy"] == "no-referrer"
        error.close()
    else:
        raise AssertionError(f"expected HTTP {expected_status}")

raw = Path(export_path).read_bytes()
revision = json.loads(raw)["export_revision"]
print(f"gateway-smoke: local revision={revision} sha256={hashlib.sha256(raw).hexdigest()}")
PY
