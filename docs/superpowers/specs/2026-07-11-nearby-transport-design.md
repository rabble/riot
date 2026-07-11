# Nearby transport design (conference plan Task 5)

## Purpose

Every other piece of the conference demo is built: the content model, signing
and preview-first import, the transport-agnostic sync/reconciliation
protocol (`crates/riot-core/src/sync/`), both native shells wired to durable
signer identity, and a public read-only gateway. The one missing piece is
the actual wire between two phones — without it, the sync protocol has
nothing to carry its bytes over, and the demo can't do the one thing that
makes Riot Riot: work with no internet, no cables, no server.

This design scopes exactly that: BLE for discovery and pairing, a direct
local-network connection for the actual data once paired, with BLE as a
fallback if that connection can't be made. Never falls back to the
internet under any failure.

## Users are not technical

Every user-facing string in this feature is plain language. No "BLE," "sync,"
"reconciliation," "peer," "namespace," or similar jargon ever reaches the UI.
Nearby phones are shown by a friendly generated name (e.g. "Blue Kite",
"Quiet River") never a technical identifier. See "UI language" below for the
full set of states and their wording.

## Scope

**In scope:**
- A new Rust FFI bridge exposing the existing `ReconcileSession` sync engine
  to native code (it exists in `crates/riot-core/src/sync/` today but nothing
  can call it — this is a real gap, not a design choice).
- Native BLE discovery, pairing, and local-IP handoff on both iOS and
  Android, one implementation per platform (this cannot reasonably be
  shared — BLE and socket APIs are fundamentally platform-specific and
  callback-heavy).
- Loopback-based contract tests proving the transport layer's behavioral
  guarantees without needing real radios.

**Out of scope (deferred):**
- Real two-device BLE verification. BLE does not work reliably between
  simulators/emulators; this needs physical hardware. Everything below is
  built and tested to the loopback/unit level; the rabble verifies on real
  phones once both are available, with Claude walking through the rehearsal
  at that point.
- Any change to the sync protocol itself, the content model, or the
  preview-first import/commit flow. This design adds no new protocol
  logic — the FFI bridge exposes what already exists; native code only
  moves bytes.
- Model-assisted drafting (Task 6) and packaging/rehearsal docs (Task 7).

## Architecture

Two layers, split along the same line the original conference plan already
drew ("Native shells own permissions, local persistence, rendering, BLE
discovery, and a cross-platform local byte stream"):

- **Rust core (shared):** a thin FFI bridge over the existing,
  already-tested `ReconcileSession`/`SyncFrame` state machine. Stays
  transport-agnostic — it only ever sees raw bytes in and raw bytes out.
- **Native (Swift/Kotlin, one implementation per platform):** BLE
  discovery, pairing confirmation, and the local-IP handoff. Presents a
  single `send(bytes)` / `onReceive(bytes)` interface upward so the UI and
  the FFI bridge never know which physical transport is actually carrying
  the data.

## Components

### Rust: `crates/riot-core/src/sync/ffi_bridge.rs` (new)

Exposed through `crates/riot-ffi/src/mobile_api.rs` / `mobile_state.rs`,
following the same opaque-handle pattern as the rest of the mobile API
(no raw Willow/session types cross the boundary):

- `MobileProfile::begin_sync() -> SyncSession` — opens a session against the
  profile's current store entries.
- `SyncSession::next_outbound() -> Option<Vec<u8>>` — the next frame to
  send, if the protocol has one queued.
- `SyncSession::receive(bytes: Vec<u8>) -> SyncOutcome` — feeds one inbound
  frame. `SyncOutcome` is a small enum: `SendMore`, `ReadyToPreview { count:
  u32 }`, `Done`, `Failed`.
- `SyncSession::accept_import() -> Result<ImportSummary, MobileError>` /
  `reject_import()` — thin wrappers over the *existing* preview/plan/commit
  path already used for file-based import. No new import logic.

This bridge adds zero new protocol behavior. Its own tests replay the
existing `core_sync` test fixtures through the bridge and assert nothing is
lost, reordered, or duplicated crossing the FFI boundary.

### Native: `apps/ios/Riot/Transport/` and `apps/android/.../transport/` (new)

One implementation per platform, same three responsibilities:

- **`NearbyAdvertiser` / `NearbyScanner`** — advertise and scan
  simultaneously over BLE (either phone can discover either other; whichever
  connection completes first wins). Surfaces discovered phones as
  `(friendlyName, connectionHandle)` pairs. The friendly name is generated
  locally per session (an adjective+noun pair from a small fixed word list)
  and is never a real identifier.
- **`NearbyConnection`** — after both people confirm pairing: exchanges a
  local IP address and port over a BLE characteristic, attempts one direct
  TCP connection over the shared WiFi network, and — if that single attempt
  fails — falls back to framed BLE characteristic writes/notifications for
  the rest of that session (no per-message switching between the two).
  Presents `send(bytes)` / `onReceive(bytes)` upward; callers never see
  which physical transport is active.
- **`SyncCoordinator`** — glues `NearbyConnection` to the FFI `SyncSession`:
  pushes inbound bytes in, sends `next_outbound()` out, drives the loop
  until `SyncOutcome` reports done or failed, and translates protocol state
  into the plain-language UI states below.

## Data flow

```
Person taps "Find nearby"
  → advertise + scan (BLE) → friendly names appear in a list
Person taps a name; both people confirm on their own phone
  → BLE pairing confirmed
  → exchange IP:port over the BLE link → attempt a direct connection
  → (if that fails, keep going over BLE — never fall back to the internet)
SyncCoordinator drives the FFI SyncSession: send/receive frames until
  the protocol reports convergence
  → if new content arrived: plain-language preview ("2 new updates from
     Blue Kite") → person taps "Add them" → existing accept/reject path
     commits it, exactly like accepting a file import today
```

## UI language

| Technical state | What the person sees |
| --- | --- |
| Advertising/scanning | "Looking for nearby phones..." |
| Peer discovered | Friendly name in a list (e.g. "Blue Kite") |
| Pairing requested | "Connect with Blue Kite?" with Confirm/Cancel |
| BLE connected, exchanging IP | "Connecting..." |
| Direct connection established | (no separate state shown — same as above) |
| Sync in progress | "Getting the latest from Blue Kite..." |
| New content ready | "2 new updates from Blue Kite" with Add/Not now |
| Import committed | "All caught up" |
| Any transport failure | "Couldn't connect — try again" |
| Peer moved out of range | "Blue Kite went out of range" |
| No content to exchange | "You're already up to date" |

Underlying causes (a specific BLE error, a socket timeout, a rejected or
malformed frame) are logged for debugging but never surfaced raw to the
person using the app.

## Error handling

Every failure path collapses to one of the plain states above. Specifically:

- A BLE connection that never completes pairing: "Couldn't connect — try
  again," no partial state retained.
- A local-IP connection attempt that fails or times out: silently continue
  over BLE; the person never sees this as a distinct failure, only as
  slightly slower syncing.
- A malformed or rejected frame (the existing `SyncFrame`/`SyncError`
  machinery already handles this at the protocol level): the transport
  layer surfaces it as "Couldn't connect — try again" and tears down the
  session cleanly; no partial import state.
- The app never silently falls back to internet transport under any nearby
  transport failure. This is a hard requirement carried over from the
  original conference plan's transport-contract tests.

## Testing

Per the original plan's own Task 5 Step 1, the transport contract is tested
against a loopback byte stream — no real radio required:

- Discovery exposes a friendly peer name.
- Pairing requires explicit confirmation (not automatic).
- Every frame sent is received with bytes and order preserved.
- Disconnect mid-session is recoverable (no corrupted state, can retry).
- The app never silently falls back to internet transport.

The Rust FFI bridge gets its own tests replaying existing `core_sync`
fixtures through `SyncSession::receive`/`next_outbound`, asserting the
bridge itself introduces no data loss, reordering, or protocol drift.

Real two-device BLE verification (discover, pair, sync, accept, compare
canonical IDs and rendered board state on both phones) is deferred per
"Scope" above — this is a physical-hardware step, not something buildable
or verifiable in this environment alone.

## Commit sequence (mirrors the original plan's TDD structure)

1. RED: FFI bridge tests against existing `core_sync` fixtures (fail —
   bridge doesn't exist).
2. GREEN: implement `ffi_bridge.rs` + `mobile_api.rs`/`mobile_state.rs`
   exports. Commit: `feat: expose sync reconciliation through the mobile FFI`.
3. RED: iOS transport contract tests against a loopback stream (fail — no
   adapter exists).
4. GREEN: implement `apps/ios/Riot/Transport/`. Commit: `feat: add iOS
   nearby transport (BLE + local-IP)`.
5. RED: Android transport contract tests against a loopback stream (fail).
6. GREEN: implement `apps/android/.../transport/`. Commit: `feat: add
   Android nearby transport (BLE + local-IP)`.
7. Wire `SyncCoordinator` + plain-language UI states into each shell's
   existing conference surface. Commit per platform.
8. Physical two-device rehearsal (deferred, done later with the rabble).
