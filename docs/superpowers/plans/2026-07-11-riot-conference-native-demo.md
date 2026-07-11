# Riot Conference Native Demo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` in this session or `superpowers:executing-plans` in a fresh session. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a real, physical-device iOS-and-Android Riot demo: people create a public incident space and alerts offline, receive local LLM drafting help, reconcile only missing Willow-backed content over a nearby local link, and render the resulting shareable app package on both phones.

**Architecture:** Keep the Rust core authoritative for signed content, canonical entry identity, preview-first import, and merge. Native shells own permissions, local persistence, rendering, BLE discovery, and a cross-platform local byte stream. A Riot-owned, bounded incremental-sync adapter exchanges signed index summaries and missing-entry/payload requests; it intentionally makes no WTP interoperability claim. BLE discovers and pairs nearby devices; the same adapter uses a local IP stream when available and bounded BLE frames otherwise. QR/file handoff is the no-radio fallback.

**Tech Stack:** Rust + UniFFI; Swift/UIKit or SwiftUI; Kotlin/Jetpack Compose; CoreBluetooth and Android Bluetooth LE; local IP stream transport; CBOR/Willow canonical bytes; encrypted native key storage; an on-device model provider plus an optional hosted-model provider behind the same draft-only interface.

---

## Conference contract

The demo must show two physical phones, not a browser mock:

1. create or join a public incident space;
2. create an alert or post, optionally using model assistance;
3. review and sign it locally;
4. discover a nearby second phone and sync missing content while offline;
5. preview then accept the import on the second phone;
6. render the same incident space/package there, with signer, freshness, local/offline state, and AI-assistance disclosure visible.

The demo is usable software, but it must remain honest about its boundary: no claim of Willow Transfer Protocol compatibility, Confidential Sync, private-group security, arbitrary code execution in packages, or autonomous model publishing.

## File and ownership map

| Area | Primary files/directories | Responsibility |
| --- | --- | --- |
| Core content/session | `crates/riot-core/src/` | Canonical content, signing, store, preview/import/merge, bounded sync facts. |
| FFI | `crates/riot-ffi/` | Small typed UniFFI handle surface; no private keys or Willow generics cross it. |
| Incremental sync | `crates/riot-core/src/sync/`, `crates/riot-core/tests/core_sync.rs` | Fixed CBOR messages, summary/diff/request/response state machine, caps, and byte-stream independence. |
| iOS app | `apps/ios/Riot/` | App lifecycle, persistence, permissions, BLE/local stream adapters, native rendering. |
| Android app | `apps/android/` | Equivalent Kotlin shell and transport adapters. |
| Shared package templates | `fixtures/conference/` | Signed/public incident-space fixture, package manifest, deterministic demo content. |
| Model providers | `apps/{ios,android}/.../Drafting/` | On-device and hosted providers that return drafts only; human review/signing remains mandatory. |
| Demo verification | `scripts/conference/`, native tests | Two-device rehearsal, offline assertion, captured facts/log redaction. |

## Task 1: Freeze the conference wire and package boundary

**Files:**
- Create: `docs/superpowers/specs/2026-07-11-riot-conference-native-demo-design.md`
- Create: `fixtures/conference/incident-space-v1.json`
- Create: `fixtures/conference/package-manifest-v1.json`
- Create: `crates/riot-core/tests/conference_fixture.rs`

- [ ] **Step 1: Write the failing fixture test**

Test that the fixture has one public namespace, two authors, a deterministic incident title, one AI-assisted draft flag, and no private-group fields. Its generated canonical bytes must be deterministic and every rendered route must remain under `/site/`.

- [ ] **Step 2: Run the fixture test as RED**

Run: `cargo test -p riot-core --test conference_fixture`

Expected: FAIL because the fixture and package manifest do not exist.

- [ ] **Step 3: Define the compact package manifest and fixture**

The manifest names only a fixed `incident-board/1` renderer profile, public namespace, title, and allowed object kinds (`alert`, `observation`, `resource`, `request`, `offer`). It contains no JavaScript, remote URLs, secrets, or private identifiers.

- [ ] **Step 4: Run GREEN and commit**

Run: `cargo test -p riot-core --test conference_fixture`

Expected: PASS with deterministic fixture bytes.

Commit: `feat: add conference incident package fixture`

## Task 2: Complete the narrow Rust/UniFFI mobile boundary

**Files:**
- Modify: `crates/riot-core/src/session.rs`
- Modify: `crates/riot-core/src/lib.rs`
- Modify: `crates/riot-ffi/src/lib.rs`
- Create: `crates/riot-ffi/tests/mobile_contract.rs`

- [ ] **Step 1: Write RED FFI contract tests**

Test native-facing operations: open local profile, create public space, create draft alert, sign, list current entries, inspect/import bytes, accept a selected plan, and expose full IDs/freshness/AI-assistance. Assert no key material or generic Willow type appears in the FFI type surface.

- [ ] **Step 2: Run RED**

Run: `cargo test -p riot-ffi --test mobile_contract`

Expected: FAIL because the mobile boundary does not exist.

- [ ] **Step 3: Implement only typed handles and records**

Keep all mutation in the core arbiter. Export byte arrays and typed records, never raw private keys, Meadowcap internals, or panic-capable callbacks.

- [ ] **Step 4: Run GREEN and generate bindings**

Run:

```bash
cargo test -p riot-ffi --test mobile_contract
cargo xtask generate-bindings
```

Expected: both commands pass and produce Swift/Kotlin bindings from the locked UniFFI version.

Commit: `feat: expose conference mobile core through uniffi`

## Task 3: Build Riot-owned bounded incremental reconciliation

**Files:**
- Create: `crates/riot-core/src/sync/{mod.rs,wire.rs,reconcile.rs}`
- Modify: `crates/riot-core/src/lib.rs`
- Create: `crates/riot-core/tests/core_sync.rs`

- [ ] **Step 1: Write RED reconciliation tests**

Test a two-store exchange with overlapping entries: a summary carries namespace and canonical entry IDs/digests; the receiver requests only missing IDs; the sender returns only requested canonical entry/capability/payload facts; the receiver runs the existing preview-first import path. Test duplicate summaries produce no transfer, an over-cap request is rejected, and every frame is length-bounded.

- [ ] **Step 2: Run RED**

Run: `cargo test -p riot-core --test core_sync`

Expected: FAIL because no sync types or reconciler exist.

- [ ] **Step 3: Implement a byte-stream-independent state machine**

Use fixed, canonical CBOR frames: `Hello`, `Summary`, `Request`, `Entries`, `Complete`, and `Reject`. The state machine owns no radio code and delegates final acceptance to the existing inspect/plan/commit boundary. It calls itself `Riot conference sync`, not WTP.

- [ ] **Step 4: Run GREEN and adversarial bounds**

Run: `cargo test -p riot-core --test core_sync`

Expected: two stores converge using only missing facts; malformed, oversized, duplicate, and out-of-sequence frames return typed rejections without mutation.

Commit: `feat: add bounded incremental conference sync`

## Task 4: Create both native shells and local persistence

**Files:**
- Create: `apps/ios/Riot/`
- Create: `apps/android/`
- Create: `apps/ios/RiotTests/BindingSemanticsTests.swift`
- Create: `apps/android/app/src/androidTest/.../BindingSemanticsTest.kt`

- [ ] **Step 1: Write RED binding-semantic tests**

Each native test creates an alert through generated bindings, persists and reloads the profile, and asserts full IDs, signer/freshness, and AI-assistance survive without a network.

- [ ] **Step 2: Run RED**

Run the platform-native test command for each app.

Expected: FAIL because the apps and packaged bindings do not exist.

- [ ] **Step 3: Implement minimal native app surfaces**

Build five screens only: spaces, incident board, compose/review/sign, import preview, and connection status. Persist encrypted local app state through native facilities; render only the fixed incident-board profile.

- [ ] **Step 4: Run GREEN on simulator/emulator**

Run the two native binding-semantic suites.

Expected: both pass offline with the same fixture and binding results.

Commit: `feat: add native Riot conference shells`

## Task 5: Add real nearby transport adapters

**Files:**
- Create: `apps/ios/Riot/Transport/`
- Create: `apps/android/app/src/main/.../transport/`
- Create: `apps/ios/RiotTests/TransportContractTests.swift`
- Create: `apps/android/app/src/androidTest/.../TransportContractTest.kt`

- [ ] **Step 1: Write RED adapter contract tests**

Test a transport adapter against a loopback byte stream: discovery exposes an ephemeral peer display name, pairing requires user confirmation, each frame preserves bytes/order, disconnect is recoverable, and the app never falls back to internet transport silently.

- [ ] **Step 2: Run RED**

Run each native transport-contract suite.

Expected: FAIL because no adapters exist.

- [ ] **Step 3: Implement transport priority and fallback**

Use BLE for discovery/pairing and bounded frame transfer. When a direct local IP stream is available after pairing, use it for larger payloads; otherwise continue the same framed protocol over BLE. QR/file export-import invokes the same sync/import records when radios are unavailable. All adapters are explicit in the UI.

- [ ] **Step 4: Run GREEN on two physical phones**

Run a two-device harness: create on phone A, disable cellular/Wi-Fi internet, discover/pair, reconcile to phone B, preview/accept, and compare canonical IDs and rendered board state.

Commit: `feat: sync Riot spaces over nearby transport`

## Task 6: Add draft-only model assistance

**Files:**
- Create: `apps/ios/Riot/Drafting/`
- Create: `apps/android/app/src/main/.../drafting/`
- Create: native drafting tests on both platforms

- [ ] **Step 1: Write RED provider tests**

Test a local provider and a hosted provider returning the same `AlertDraft` shape. Assert provider failure leaves a local draft editable, output is marked AI-assisted before signing, and no provider can call sign/import/sync APIs.

- [ ] **Step 2: Run RED**

Run native drafting tests.

Expected: FAIL because the provider interface does not exist.

- [ ] **Step 3: Implement the provider boundary**

Define `DraftingProvider.generate(prompt, context) -> DraftSuggestion`; context is the current public space only. Local is preferred. Hosted use requires an explicit user opt-in and displays the network disclosure. A human must review and press Sign.

- [ ] **Step 4: Run GREEN**

Run native drafting tests with a deterministic local fake and an opt-in hosted-provider integration test guarded by configured credentials.

Commit: `feat: add draft-only model assistance`

## Task 7: Rehearse and package the conference build

**Files:**
- Create: `scripts/conference/two-device-rehearsal.sh`
- Create: `docs/decisions/riot-conference-demo-runbook.md`
- Modify: `README.md`

- [ ] **Step 1: Write a failing rehearsal assertion**

The script must fail unless both device facts contain the same complete entry IDs, receipt/import facts, AI-assistance flag, and rendered package revision after a radio-off sync.

- [ ] **Step 2: Run RED**

Run: `scripts/conference/two-device-rehearsal.sh`

Expected: FAIL until device artifacts are captured.

- [ ] **Step 3: Capture a successful physical-device rehearsal**

Record only safe facts, versions, hashes, and timing; scan captured logs for payload and key sentinels. Document the tap-by-tap reset/recovery path for the live demo.

- [ ] **Step 4: Package TestFlight and Play testing builds**

Run the project-specific signed build/release commands after native platform setup. Record build numbers and artifact hashes in the runbook; never store signing credentials in the repository.

- [ ] **Step 5: Final verification and commit**

Run core, FFI, iOS, Android, and two-device rehearsal checks. Commit the runbook and the truthful README status update.

Commit: `docs: record Riot conference native rehearsal`

## Task 8: Ship the public Riot gateway at `riot.protest.net`

**Files:**
- Create: `apps/gateway/`
- Create: `apps/gateway/tests/`
- Create: `fixtures/conference/gateway-space/`
- Create: `scripts/conference/gateway-smoke.sh`
- Create: `docs/decisions/riot-protest-net-runbook.md`

- [ ] **Step 1: Write RED public-boundary tests**

Test that the gateway renders only the fixed public `incident-board/1` package from a signed/public fixture; `/site/` routes resolve; signer, freshness, AI-assistance, and an offline/open-in-Riot affordance render; private-group fields, capabilities, receipts, private identifiers, remote-code fields, and arbitrary package profiles are rejected before rendering.

- [ ] **Step 2: Run RED**

Run the gateway test command.

Expected: FAIL because the gateway does not exist.

- [ ] **Step 3: Implement a stateless public reader**

The gateway reads only a versioned, public export produced by the same conference fixture/sync boundary. It owns no canonical content state, no signing authority, no private key, no private-group route, and no write API. Every page exposes the public namespace ID as a QR code plus an `Open in Riot` affordance. A signed drop/download may be served only as an opaque public artifact.

- [ ] **Step 4: Run GREEN locally**

Run the gateway tests and `scripts/conference/gateway-smoke.sh` against a local server.

Expected: public content renders at a stable route; private/malformed content is refused; no network fetch is needed to render the fixture.

- [ ] **Step 5: Deploy and verify the domain**

Deploy only through the repository's documented hosting path once it exists. Verify `https://riot.protest.net` serves the public incident board, has no write endpoint, and its smoke script records the deployed revision and content hash. Never commit domain credentials, API tokens, signing material, or deployment secrets.

Commit: `feat: add public Riot conference gateway`

## Explicit deferrals

- Upstream WTP/Drop Format/Confidential Sync interoperability claims.
- Private-group MLS, invites, and the bridge.
- Arbitrary executable app packages or remote code loading.
- Automatic publication, autonomous agents, or model-held cross-space memory.
- Mesh relay routing beyond the paired/direct nearby transport demo.
- Private content, group invitations, private rendezvous, or authenticated write APIs at `riot.protest.net`.
