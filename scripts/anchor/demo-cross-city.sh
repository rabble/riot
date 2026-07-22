#!/usr/bin/env bash
# scripts/anchor/demo-cross-city.sh — the cross-city hosting demo.
#
# Drives the full anchor hosting lifecycle, printing each stage:
#   HOST:   PrepareHost (riot/anchor/1) -> riot/sync/2 push of O/C/W -> CommitHost
#   FOLLOW: ReadCommitted riot/sync/2 pull of the committed site, re-verified
#           client-side, using only the root-signed ticket the host printed.
#
# Modes:
#   --local             one-machine smoke run: starts a loopback anchor daemon
#                       (the demo_anchor example) itself, then runs both roles.
#   (default: remote)   against a REMOTE anchor. Set one of:
#                         ANCHOR_ADDR=<node_id_hex>@<ip:port>[,...]  (direct)
#                         ANCHOR_NODE_ID=<64 hex>                    (discovery)
#   --host              remote mode, HOST role only — prints TICKET=... to hand
#                       to the other city.
#   --follow <ticket>   remote mode, FOLLOW role only — pulls with that ticket.
#
# Two-city walkthrough: run `--host` in city A, copy the TICKET line, run
# `--follow <ticket>` in city B. The host can be offline by then — the anchor
# serves what was committed (store-and-forward).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

MODE=both
LOCAL=0
TICKET_ARG=""
case "${1:-}" in
  --local) LOCAL=1 ;;
  --host) MODE=host ;;
  --follow)
    MODE=follow
    TICKET_ARG="${2:-}"
    [[ -n "$TICKET_ARG" ]] || { echo "usage: $0 --follow <ticket-hex>" >&2; exit 2; }
    ;;
  "") ;;
  *) echo "usage: $0 [--local | --host | --follow <ticket-hex>]" >&2; exit 2 ;;
esac

say() { printf '\n=== %s ===\n' "$*"; }

say "building demo examples (cargo build -p riot-anchor --features daemon --examples)"
cargo build -p riot-anchor --features daemon --examples
BIN="target/debug/examples"

ANCHOR_PID=""
WORK=""
cleanup() {
  # `kill` sends SIGTERM; the demo_anchor daemon now catches it and relinquishes
  # its deployment lease cleanly (matching the production binary), so teardown
  # never leaves a standing lease on the throwaway db.
  if [[ -n "$ANCHOR_PID" ]] && kill -0 "$ANCHOR_PID" 2>/dev/null; then
    kill "$ANCHOR_PID" 2>/dev/null || true
    wait "$ANCHOR_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

if [[ "$LOCAL" == 1 ]]; then
  WORK="$(mktemp -d "${TMPDIR:-/tmp}/riot-anchor-demo.XXXXXX")"
  DB="$WORK/anchor.sqlite3"
  LOG="$WORK/demo_anchor.log"
  # Fresh throwaway identities for the loopback anchor; the config surface is
  # the production one (crates/riot-anchor/src/config.rs).
  RIOT_ANCHOR_OPERATOR_KEY_HEX="$(openssl rand -hex 32)"
  RIOT_ANCHOR_ENDPOINT_KEY_HEX="$(openssl rand -hex 32)"
  export RIOT_ANCHOR_OPERATOR_KEY_HEX RIOT_ANCHOR_ENDPOINT_KEY_HEX
  export RIOT_ANCHOR_DISPLAY_LABEL="demo-local-anchor"

  say "starting local loopback anchor (db: $DB)"
  "$BIN/demo_anchor" --db "$DB" >"$LOG" 2>&1 &
  ANCHOR_PID=$!

  for _ in $(seq 1 200); do
    grep -q '^ANCHOR_ADDR=' "$LOG" 2>/dev/null && break
    kill -0 "$ANCHOR_PID" 2>/dev/null || { echo "anchor daemon died:" >&2; cat "$LOG" >&2; exit 1; }
    sleep 0.1
  done
  grep -q '^ANCHOR_ADDR=' "$LOG" || { echo "anchor never printed its address:" >&2; cat "$LOG" >&2; exit 1; }

  ANCHOR_NODE_ID="$(sed -n 's/^ANCHOR_NODE_ID=//p' "$LOG" | head -1)"
  ANCHOR_ADDR="$(sed -n 's/^ANCHOR_ADDR=//p' "$LOG" | head -1)"
  export ANCHOR_NODE_ID ANCHOR_ADDR
  echo "anchor node id: $ANCHOR_NODE_ID"
  echo "anchor addr:    $ANCHOR_ADDR"
fi

if [[ -z "${ANCHOR_ADDR:-}" && -z "${ANCHOR_NODE_ID:-}" ]]; then
  echo "set ANCHOR_ADDR (<node_id_hex>@<ip:port>) or ANCHOR_NODE_ID (64 hex)," >&2
  echo "or pass --local for a one-machine smoke run" >&2
  exit 2
fi

TICKET="$TICKET_ARG"

if [[ "$MODE" == both || "$MODE" == host ]]; then
  say "HOST: PrepareHost -> riot/sync/2 push -> CommitHost"
  HOST_LOG="$(mktemp "${TMPDIR:-/tmp}/riot-demo-host.XXXXXX")"
  "$BIN/demo_host" | tee "$HOST_LOG"
  TICKET="$(sed -n 's/^TICKET=//p' "$HOST_LOG" | head -1)"
  rm -f "$HOST_LOG"
  [[ -n "$TICKET" ]] || { echo "demo_host did not print a TICKET" >&2; exit 1; }
fi

if [[ "$MODE" == both || "$MODE" == follow ]]; then
  say "FOLLOW: ReadCommitted riot/sync/2 pull with the root-signed ticket"
  "$BIN/demo_follow" "$TICKET"
fi

if [[ "$LOCAL" == 1 ]]; then
  say "stopping local anchor"
  cleanup
  ANCHOR_PID=""
  echo "anchor log: $LOG (db kept at $DB for inspection)"
fi

say "demo complete"
