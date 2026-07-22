# Mobile iroh Transport — Non-Local Sync on the Phone (Phase 3, issue #107)

**Status:** design draft, revision 2 (after design-review-gate round 1). Owner directive settled the transport; this doc settles the mobile architecture, the security posture, and the build discipline, and presents the sequencing for a decision.

**Revision 2 changelog** (resolving round-1 blockers): committed the FFI seam to FFI-owns-socket + internal runtime and restated the surface honestly (Designer); routed every dial through the transport-floor gate and made durable NodeId an explicit anonymity design, not a deferred policy call, with anchor-baseline-first sequencing (Security); took a relay position; added the Cargo.lock re-pin, the `net`-vs-coverage decision, and per-slice host/device test labels (CTO); added the community/group sync story and a mobile Checkpoint B (PM).

## Owner directive (2026-07-23)

iroh is the **general non-local transport**, running on the phone — peer-to-peer for peers that know each other (known NodeId → direct dial), and non-local sync in general via **both** relay and direct connect. Mobile stops being BLE/LAN-only.

## What is already proven (do not re-litigate)

- Anchor daemon serves `riot/sync/2` over real iroh — **PR #108, merged (`b2b197c`)**; `ALPN_SYNC_V2` registered/served (`daemon.rs:652,823`).
- **Checkpoint A passed on real cloud (2026-07-23):** GCE anchor, driven from a NAT'd Mac over the public internet — full push→commit→pull, plus kill/restart with the publisher gone and the follower still pulling byte-identical committed data. Public transport (n0 relay, discovery, NAT traversal) works. *Checkpoint A proved the transport, not an on-phone outcome — see Checkpoint B below.*

## Product framing: what non-local sync actually serves

Riot's flagship unit is a **community** (many members), not a 1:1 pair. The primary scenario:

> Members of an activist community meet, join the community together, then scatter. They must keep seeing each other's posts with no co-presence — including when they are never online at the same moment.

**Direct hole-punch and relay both require both endpoints live simultaneously** (a relay forwards live packets; it is not storage). So the **default, baseline mechanism for all non-local sync — community and 1:1 — is the anchor's store-and-forward committed store**: a member pushes while online, other members pull later, nobody needs anyone else live. Direct peer-dial is a **latency/independence optimization for the 1:1 case when both peers happen to be online**, and (per Security, below) a later opt-in capability, not the baseline.

**Checkpoint B (the mobile success proof, distinct from A):** two physical phones join a community together, leave (different networks, never co-present again), and later both reconcile byte-identical community state through the anchor. This is the on-phone proof the transport work must reach.

## Core architectural decision: iroh in `riot-ffi` behind a `net` feature; the FFI OWNS the socket

**"iroh on mobile" = feature-gated iroh inside `riot-ffi`, reusing `riot-transport`.** This is a **deliberate reversal** of the prior "riot-ffi has zero iroh / mobile is BLE+LAN-only" invariant — stated up front so downstream agents treat the new dep edge as intentional.

Why legal and why here:
1. The "no iroh/tokio" rule protects **`riot-core`'s wasm-cleanliness** (it gates `rusqlite` behind a default `sqlite` feature so `--no-default-features` builds for wasm32). `riot-core` is a leaf; deps flow toward it. `riot-ffi` is a `staticlib`/`cdylib` that never targets wasm. Feature-gated iroh in `riot-ffi` cannot contaminate `riot-core`. (Architect-verified: `riot-core/Cargo.toml:22-40`, `riot-ffi/Cargo.toml:8`.)
2. Precedent: `riot-anchor` gates iroh/tokio behind its `daemon` feature (`riot-anchor/Cargo.toml:59`). `riot-ffi` mirrors it with a `net` feature, off by default.
3. Keeps the trust-critical **fail-closed dial gate in Rust** — `admit_dial`/the ticket floor live in pure `riot-core` (`site/ticket.rs:286`), callable from the FFI with no iroh dep. A native iroh would force reimplementing that gate twice (Swift + Kotlin). Reject.
4. Reject embedding `riot-client-net` (a socket-owning lifecycle manager, no frame seam). Depend on `riot-transport` directly.

### The FFI seam (committed — resolves the round-1 contradiction)

Nearby BLE/LAN sync is *native-owns-socket*: the host owns the radio and pumps opaque frames through `MobileSyncSession` (`receive_frame`/`take_outbound_frame`). **iroh cannot use that shape** — iroh hands you async connections and streams, not raw framed datagrams the host can shuttle. Therefore, with iroh inside `riot-ffi`:

- **`riot-ffi` OWNS the iroh endpoint and an internal single-threaded tokio runtime** (mirroring `riot-client-net`'s `TokioTaskSpawner`/`IrohEndpointFactory`, but depending on `riot-transport` directly). The native host does **not** touch the socket.
- The native surface is **trigger + observe**, not frame-shuttle. Concretely (illustrative, not final signatures):
  - `sync_with_anchor(anchor_ref) -> SyncOutcome` — FFI dials the anchor (through the gate), drives the `sync/2` session to completion on its runtime via `block_on`, returns an outcome. Synchronous from the caller's view; internally async. **This keeps the drive loop host-unit-testable** (a blocking call over a deterministic FSM).
  - Later (peer slice): `sync_with_peer(peer_ref) -> SyncOutcome` and an **inbound listener** lifecycle (`start_peer_listener`/`stop_peer_listener` + an outcome callback) — a genuinely new concept with no nearby-sync analog (a phone accepting unsolicited dials).
- **UniFFI churn, honestly:** the *core sync logic* is reuse (`ByteSyncSession` for peers, `Sync2Session::initiator` for anchors — pure, tested FSMs). But the socket-owning **entry points are new `uniffi` functions/objects** (dial, listener lifecycle, `SyncOutcome` variants), which trigger the bindings+staticlib coordinated rebuild on both platforms. The near-zero-churn claim was wrong; the churn is bounded but real and lands in the `net` FFI surface.
- **Runtime ownership decision (resolves risk 1):** FFI-internal runtime + `block_on` per operation is the recommendation — it matches the synchronous nearby pump, keeps signatures simple, and makes the Rust drive loop testable without async harness. iOS background-suspension mid-operation is handled by treating a suspended operation as a resumable/retryable failure (the anchor is durable; a dropped sync just retries), NOT by trying to keep a socket alive in the background.

## Security posture (resolves the Security blockers — first-class, not deferred)

This is an activist tool; de-anonymization is a primary harm. Three hard requirements:

### 1. EVERY dial goes through the transport-floor gate
`admit_dial` (the `require:none`/`require:arti` fail-closed floor) is today wired ONLY into `dial_with_ticket` (`iroh.rs:179`), NOT `sync_connect` (`iroh.rs:184`). **The FFI `net` feature MUST NOT expose raw `sync_connect`.** Every mobile dial — anchor `sync/2` and peer `sync/1` — is wrapped in a `dial_with_ticket`-style gate that runs `admit_dial` before any packet, so a `require:arti` site can never be dialed over cleartext iroh from the phone. A peer dial is authenticated by a ticket/record carrying a transport floor (peers dialing by bare NodeId with no floor is disallowed). **Acceptance:** a test mirroring `ticket_gate.rs:54` for the peer path (require:arti ⇒ refused, no dial).

### 2. Inbound admission does full canonical verification — never accept-all
Seed/test paths use `|_| true` accept-all closures (`seed.rs:112`). The phone's inbound admission closure MUST run the canonical gate (`verify_anchor_item` / `verify_entry` / `validate_site_manifest`) on every received frame — content is trusted because it is root-signed and verified, never because of who served it. Hard requirement in the slice-2 acceptance criteria.

### 3. Durable NodeId is an explicit anonymity design, and direct peer-dial is opt-in and LATER
`bind()` is deliberately ephemeral for unlinkability (`iroh.rs:61-65`). A durable, dialable phone NodeId (a) makes the device trackable across sessions, (b) exposes the phone's IP to any peer holding the NodeId (direct) and to the relay operator (relay). Therefore:
- The **anchor baseline needs no durable phone NodeId** — the phone is the *dialer*, the anchor is the stable endpoint. Ship the baseline first with ephemeral phone identity intact.
- **Direct peer-dial (phone as responder) is a later, opt-in slice** gated behind an explicit anonymity design: per-follow-graph pseudonymous identities (NOT one global stable NodeId — aligns with the existing per-community author partitioning), a rotation policy, opt-in with an explicit IP/linkability warning shown to the user, and a default that never silently trades away unlinkability.
- **IP exposure is documented and surfaced**, not implicit: direct dial reveals IP to the peer; relay reveals it to the relay operator.

### 4. Relay position
Default to an **anchor-operated / self-hosted relay** for activist deployments; if the n0 default relay is ever used, the design carries a threat-model paragraph on the social-graph metadata it exposes, and relay selection is a conscious, surfaced choice — never n0-by-default silently.

### 5. Tor/arti is refuse-only on mobile (documented, not implied usable)
There is no `arti` transport in the tree; `caps.arti=false` on the phone, so `require:arti` sites fail closed (the SAFE outcome, `ticket_gate.rs:54`). Real anonymous mobile transport is **unbuilt and unscheduled** — filed as explicit future work so the safe refusal is never mistaken for a shipped anonymity guarantee.

### 6. Inbound DoS bound
The phone-as-responder (peer slice) adds a remote QUIC/TLS attack surface. Beyond the router's existing concurrency bound (Semaphore/handshake-deadline/single-stream/lifetime, `router.rs:322-335`), restrict inbound dials to a **known follow-graph allowlist** and add a per-peer handshake rate limit.

## Build & test discipline (resolves the CTO blockers)

- **Cargo.lock contract pin:** adding `riot-transport`/iroh/tokio to `riot-ffi` adds packages to `Cargo.lock`, breaking `fixtures/manifest.json:cargo_lock_sha256` (byte-compared at `xtask main.rs:626`). **Slice 1 owns** regenerating `Cargo.lock` + re-recording the sha to the printed `actual`, and updating any dep pins.
- **`net` vs `--all-features` / coverage:** the CI gate is `cargo llvm-cov --workspace --all-features --fail-under-lines 95` (blocking, `blockPRCreation`). Decision: **`net` is a non-default feature deliberately EXCLUDED from the coverage `--all-features` invocation** (or the gate switches to an explicit feature list), so device-only network I/O does not drag the line floor below 95/97. The **host-testable seam** (the Rust drive loop over the pure FSMs + the phone-store `Sync2Repository`) IS covered; the device-only native socket driver is documented as excluded. State the exact gate invocation change in slice 1.
- **xtask boundary rule:** extend `check_resolved_feature_graph` (`main.rs:407`) to assert iroh is **ABSENT from the default `riot-ffi` closure** (`cargo tree -p riot-ffi -e features`), and present ONLY under `--features net`. Lands in slice 1, same slice as the feature.
- **TDD host-vs-device labeling (per slice):** the pure FSMs (`ByteSyncSession`, `Sync2Session::initiator`) and the new `Sync2Repository`-over-phone-store are **host-unit-testable in Rust** (deterministic, no device). The native iroh socket driver + inbound listener are **device/instrumented-only** (Android host-JVM can't load the `.so`; hostless XCTest on iOS). Every slice labels which layer it touches so it can be driven test-first at the right layer.

## EXISTS vs BUILD

| Piece | Status |
|---|---|
| Opaque-frame FFI FSM + native nearby frame-pump hosts | EXISTS |
| Peer core `ByteSyncSession` + `sync_connect`/`sync_accept` | EXISTS |
| `Sync2Session::initiator` pure FSM (iroh/tokio-free) | EXISTS |
| Ticket NodeId hint + fail-closed `admit_dial` (in pure riot-core) | EXISTS |
| n0 relay/discovery bind, proven (Checkpoint A) | EXISTS |
| HTTPS verify/import (`follow_site` + `import_followed_site_bundle`) | EXISTS |
| `riot-ffi` `net` feature + FFI-owned tokio runtime + gated dial wrappers | BUILD |
| Every-dial-through-admit_dial wrapper (no raw sync_connect exposed) | BUILD (security-critical) |
| `Sync2Repository` over the phone's willow store + drive loop | BUILD (host-testable) |
| Native iroh socket driver (dial) on iOS + Android | BUILD (device-only) |
| Inbound peer listener + follow-graph allowlist + rate limit | BUILD (later, opt-in) |
| Per-follow-graph pseudonymous durable identity + rotation + opt-in UX | BUILD (later, gated) |
| Cargo.lock re-pin + xtask boundary rule + coverage-gate change | BUILD (slice 1) |
| Anchor HTTPS ReadCommitted-bundle endpoint (only if HTTPS interim chosen) | BUILD (anchor-side) |

## Sequencing (recommended; the decision for owner + gate)

1. **Slice 1 — scaffold, no behavior:** `riot-ffi` `net` feature (off by default) pulling `riot-transport`; FFI-owned runtime skeleton; Cargo.lock re-pin; xtask boundary rule; coverage-gate change. Compiles; nothing dials yet. Host-unit only.
2. **iroh-on-mobile viability spike (HARD GATE, runs parallel to slice 1):** battery, iOS background-execution/socket-kill, binary size. A "no" invalidates the native-iroh direction before expensive platform work. Orthogonal to the HTTPS interim.
3. **Slice 2 — anchor `sync/2` client (the baseline, the leave-the-room win):** `Sync2Repository` over the phone store (host-tested) + the gated `sync_with_anchor` FFI entry (every dial through `admit_dial`; inbound N/A — phone is dialer) + a device-level native dial on ONE platform to reach **Checkpoint B**. No durable NodeId needed.
4. **Slice 3 — second platform** native dial; background/opportunistic anchor sync scheduling (sets the leave-the-room UX promise honestly).
5. **Slice 4 — direct peer-dial (opt-in, anonymity-gated):** per-follow-graph pseudonymous identity + rotation + opt-in IP-warning UX; inbound listener + follow-graph allowlist + rate limit; peer `sync/1` over the gated dial. Only after the anonymity design is settled.
6. **(Optional, can lead) HTTPS anchor-pull interim:** smallest de-anon surface (no durable NodeId, no inbound listener, TLS to a known anchor) — the safest early-value path for at-risk users; reuses `follow_site` + `import_followed_site_bundle`; needs an anchor HTTPS ReadCommitted-bundle endpoint.

**The sequencing question for the owner:** anchor baseline first (slices 1→3, delivers Checkpoint B / community + offline-offline) — recommended — vs HTTPS interim first (slice 6, fastest, smallest de-anon surface, anchor-only) vs direct-peer first (rejected first: highest de-anon risk, needs the anonymity design up front, and doesn't solve offline-offline).

## Open decisions for the gate

- Sequencing (above).
- Runtime ownership: FFI-internal `block_on` recommended — confirm.
- Durable-identity: per-follow-graph pseudonymous + opt-in + IP-warning — confirm the default and that it doesn't collide with per-community author partitioning.
- Relay: anchor/self-hosted default — confirm.
- Whether the viability spike is a hard gate on slices 3+ — recommended yes.
