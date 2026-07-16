# Newswire → Web: Closing the Publish-Anywhere Loop — Program Plan

> **For agentic workers:** this is a PROGRAM plan spanning several subsystems. Per `superpowers:writing-plans` scope rules, each workstream (WS1–WS4) gets its OWN detailed TDD plan + the design-review + plan-review gates (see CLAUDE.md) BEFORE execution. This document defines the goal, the interfaces between workstreams, the sequencing, and the definition of done — the map, not the turn-by-turn.

**Goal:** A newswire post created in the Riot app appears on the public web mirror (`riot-newswire-dev.protestnet.workers.dev`) as **real signed content** (not demo), a reader can **"Open in Riot →" to verify** the signed copy, and the same post **distributes device-to-device** over nearby sync. That is the indymedia loop: publish anywhere, read anywhere, verify anywhere.

**Architecture:** The Python gateway (`apps/gateway/`) is the **stateless renderer** of a signed `riot-public-gateway-export/2` JSON — the single source of truth; the Cloudflare worker (`deploy/cf-mirror`) is a dumb mirror that "cannot forge content." The board already proves the full pipeline end-to-end (Rust `sign-conference-fixture` → `fixtures/conference/.../public-export-v1.json` → `verify-conference-export` → gateway renders → web mirrors). The newswire home is the ONLY half still on `newswire.sample_view()` (demo). **The mission is to give the newswire the same real, signed pipeline the board already has.**

**Tech stack:** Rust (`riot-core`/`riot-ffi`/`xtask` — signing + export), Python 3 (`apps/gateway` — rendering), Cloudflare Worker + `build.py` (static mirror), Swift/Kotlin (app publish + "Open in Riot" deep link).

---

## Current state (verified 2026-07-16)

| Piece | State |
|---|---|
| App: create newswire community + post + editorial | ✅ on `main` (units 1A/1B/1C/1E/2C) |
| App: cross-device publishing (posts reach a second DEVICE) | 🔧 **WS0 — in flight** on `feat/join-descriptor` (Risk 15 + 16 + recovered iOS join) |
| Gateway renders a signed export (BOARD) | ✅ `riot_gateway.PublicGateway.from_file()` renders `public-export-v1.json` |
| Gateway renders the NEWSWIRE | ⚠️ `newswire.sample_view()` — **DEMO data, not signed** |
| Signed NEWSWIRE export (app records → gateway JSON) | ❌ **does not exist** — only the conference/board export does |
| Web mirror (cf-mirror worker) | ✅ deployed; renders whatever the gateway emits; footer flags "demo · not signed" |
| "Open in Riot →" verify deep link | ⚠️ present in markup; not wired to app open + digest verify |
| Owned-namespace signed site manifest | 🔧 composite-site Unit 1/2 — separate session, Unit 1 landing on `main` |

**The gap in one sentence:** everything to RENDER and MIRROR signed newswire exists; nothing PRODUCES a signed newswire export from the app's real records, so the web shows demo.

---

## Workstreams

### WS0 — Cross-device publishing (in flight)
Publishing reaches a second DEVICE. Risk 15 (`join_newswire_community` carries the descriptor) + Risk 16 (`import_signed_newswire` enters the sync inventory so `open_sync_session` works for newswire) + the recovered iOS join UI. Branch `feat/join-descriptor`. **DoD:** the end-to-end test — create A → publish → follower joins by share-ref → sync → follower's Home shows the post. Ships as TF v2. *(Owned by coordinator; nearly done.)*

### WS1 — Signed newswire gateway export (THE core link) — Rust/xtask
Produce a `riot-public-gateway-export/2` **newswire** export from real signed newswire records, exactly as the board does for the conference.
- **Files:** new `crates/xtask/src/export_newswire.rs` (mirror `sign_conference_fixture.rs` + `verify_conference_export.rs`); register in `crates/xtask/src/main.rs`; a golden fixture `fixtures/newswire/gateway-space/newswire-export-v1.json`; a signing input (real signed E/W records from a newswire store or the composite site — see WS4).
- **Interface (the contract the gateway consumes):** `{ "schema": "riot-public-gateway-export/2", "export_revision": "newswire-gateway-export-v1", "entries": [ { …record…, "verification_status": … } ] }` — an `entries[]` array with a per-entry `verification_status` the gateway stamps (proof-free, per the conference pattern). Editorial (E, `signed by the collective`) and open-wire (W, `unverified`) are distinguished by an entry field the gateway reads.
- **DoD:** `cargo run -p xtask -- export-newswire` writes a valid export; a new `verify-newswire-export` xtask validates it (entry count, digests, verification_status) exactly as `verify-conference-export` does for the board; golden fixture committed; workspace green.
- **Needs its own TDD plan** (research `sign_conference_fixture.rs`/`verify_conference_export.rs` for the exact signing + verification code to mirror).

### WS2 — Gateway renders the real newswire export — Python
Make the deployed newswire render from the WS1 export instead of `sample_view()`.
- **Files:** `apps/gateway/newswire.py` — add `newswire_view_from_export(export: dict) -> NewswireView` (build the two-column E/W `NewswireView` from the export `entries`, mapping `verification_status` → the "signed by the collective" vs "unverified · read with care" treatment); keep `sample_view()` for the standalone demo. `deploy/cf-mirror/build.py:28` — the deployed `index.html` renders `newswire_view_from_export(load(newswire_export))` (not `sample_view()`), and drop the "demo · sample content" footer flag when serving a real export.
- **DoD:** `build.py` renders real signed E/W from the WS1 export; the deployed site shows real content with correct signed/unverified treatment; `apps/gateway` tests cover `newswire_view_from_export`. Python unittest green.
- **Needs its own TDD plan** (research `newswire.py` NewswireView shape + render_newswire).

### WS3 — The verify loop ("Open in Riot →")
Bind the mirrored copy to the signed record and let a reader verify it in the app.
- **Files:** gateway markup already emits "Open in Riot →" — wire it to a `riot://` deep link carrying the entry's namespace + digest (reuse the 1E share-reference codec — `newswireShareReference` / `decode_share_reference`); iOS/Android URL-scheme handler opens the app to the post and verifies the content digest against the signed record (fail-closed if the mirror forged content).
- **DoD:** clicking "Open in Riot" on the web opens the app to that post; the app verifies the digest matches the signed record and shows verified/failed honestly. The mirror provably cannot forge a verified post.
- **Needs its own TDD plan** (+ a light security review — this is an anti-forgery boundary).

### WS4 — Owned-namespace publishing source (dependency; coordinate)
The signed site/manifest that owned-namespace publishing emits. Composite-site Unit 1/2 (separate session). **Action:** agree the export interface between composite Unit 1/2's signed site output and WS1's export input so WS1 can consume real owned-namespace content (not a throwaway fixture). **Owned by the composite session; coordinator syncs the interface.**

---

## Sequencing & dependencies

```
WS0 (in flight) ─────────────────────────────► TF v2 (publishing distributes to devices)
WS1 (newswire export) ──► WS2 (gateway renders real) ──► web shows real signed content
                                                    └──► WS3 (verify loop) ──► reader verifies in app
WS4 (composite signed-site source) ──► feeds WS1's input (coordinate the interface)
```

- **WS1 → WS2** is the hard dependency (can't render what isn't produced).
- **WS3** depends on WS2 (real content to verify) + reuses 1E's share-ref codec.
- **WS4** is a parallel dependency for the *owned-namespace* content source; WS1 can start against a committed signed-newswire fixture and swap to WS4's real source when ready.
- **WS0** is independent (device-to-device) and already in flight.

## Program definition of done

A newswire post created in the app: (1) **distributes to another device** (WS0); (2) appears on `riot-newswire-dev` as **real signed content**, not demo (WS1+WS2); (3) is **verifiable** via "Open in Riot →" (WS3); with the mirror unable to forge a verified post. The two halves — app publishing and public web — become one distributed, signed newswire.

## Execution note

Per CLAUDE.md + the writing-plans scope rule: each of WS1/WS2/WS3 gets its own detailed TDD plan, run through the design-review gate (if it changes a contract) and the plan-review gate, then the owner picks the execution method. WS0 finishes first (it's in flight and unblocks TF v2). WS1 is the next unit to plan in detail — it's the true missing link and its pattern is already proven by the board export.
