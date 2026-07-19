# Spaces-First — Rung 5: Unit 6 obligations — Implementation Plan (DRAFT)

**Status:** DRAFT, overnight 2026-07-18. Not gate-ready — mostly native (planning-only overnight) + carries FFI gaps flagged below. The security-critical item (seizure disclosure) is fully speccable and pinned.

**Goal:** The composite-site Unit 6 obligations, hosted in the new spaces-first IA: editor-invite handshake, QR gen + camera scan, writer expired-cap warning, **mandatory seizure disclosure at mint-masthead**, compose-time `require:arti` notice, and the real `follow_site(ticket)` action.

**Spec:** `docs/superpowers/specs/2026-07-18-spaces-first-navigation-design.md` §4.2/§4.3/§4.4, §7 rung 5; composite design `2026-07-15-...` §9.3 (seizure), §3 (editor invite), §5.3 (require:arti). iOS + Android + macOS.

---

## FFI status (grounded against site_ffi.rs)
- **`create_owned_site(wrapping_key)` EXISTS** (`site_ffi.rs:72`) — the mint-masthead entry. **Seizure disclosure pins HERE** (blocking, before this call). ✓
- **`delegate_section` NOT in FFI** (deferred, comment `site_ffi.rs:600`) — the editor-invite handshake needs it. **GAP** → pure-Rust FFI add (cargo-verifiable), but the invite protocol is a security surface — spec it, and note it may want its own security review.
- **`follow_site(ticket)` NOT in FFI** — the real follow action. Ticket parsing/verification is Unit 5 transport (iroh ticket landed #31 — check `crates/riot-ffi`/transport for a ticket-parse entry). **GAP** → an FFI `follow_site(ticket)` that verifies the root-signed ticket floor (Unit 5 §5.1) then records a `Following` registry row (Rung 1 path).
- **`writer_cap_expired`** is on `ResolvedCompositeSite` (Rung 3) — render at compose.

## Increment sub-ladder (native = planning-only overnight; core gaps flagged executable-later)

- **5.1 — Mandatory seizure disclosure (SECURITY, §9.3 — the priority item).** A **blocking** in-app acknowledgement BEFORE `create_owned_site` mints a masthead: the required string states device seizure = full site takeover (captor can impersonate the site + revoke real editors), NOT merely "key loss." Fires ONLY on mint (NOT on follow, NOT on join — the follow/mint split from spec §4.2–4.4 / Security gate B1). **RED (iOS/Android):** minting is blocked until acknowledged; the string names impersonation + editor-revocation. This is fully speccable now (no FFI gap — create_owned_site exists). Highest priority of the rung.
- **5.2 — `require:arti` compose-time notice.** When a site's manifest `transport_policy.require == arti`, the composer shows the honest "declarable but unfollowable in v1" notice (§5.3). Reads the manifest's require token (on `ResolvedSiteManifest`/`ResolvedCompositeSite`). Native + a pure-model test.
- **5.3 — Writer expired-cap warning.** Render `ResolvedCompositeSite.writer_cap_expired` (Rung 3) at compose: "your editorial access expired on <date>"; a peer-rejected write shows failed/pending, never "published." Native.
- **5.4 — QR generation + camera scan (both platforms, NET-NEW).** No QR/camera code exists (recon this session). Encode/decode a ticket or editor key through QR gen → scan → decode. Camera-permission recovery (fail to Settings, mirror the Nearby pattern). **RED:** a ticket/key round-trips gen→scan→decode on iOS + Android; decode is byte-transport ONLY (the scan path must NOT re-parse `require` / make its own fail-closed dial decision — that stays in Unit 5/Unit 4 core; §4.1 guardrail). Device camera = state-proven-vs-assumed (no rig — mark honestly).
- **5.5 — Editor-invite two-way handshake (needs 5.0 FFI).** invitee key → owner mints `/articles/<section>` cap (`delegate_section`) → invitee holds a working editor cap. Co-presence (QR/paste), NOT async (design §10). **GATED on the `delegate_section` FFI gap.**
- **5.6 — Real `follow_site(ticket)` (needs FFI).** The Following-tier ADD action + Rung 3's Follow control. **GATED on the `follow_site(ticket)` FFI gap.**

## Core FFI gap-fills (pure-Rust, cargo-verifiable — executable later, flagged for the user)
- **5.0a — `follow_site(ticket)`**: verify root-signed ticket floor (Unit 5), record Following row. RED: a valid ticket follows; a stripped/downgraded/expired ticket fails closed, no dial (Unit 5 §5 cases).
- **5.0b — `delegate_section` FFI**: expose the owner's section-cap mint for the invite. Security surface — spec + likely its own review.

## Deferred
Async/remote editor invite (design §10 — co-presence only in v1); the arti channel itself (parked).

## Self-review
- §9.3 seizure disclosure (mandatory, blocking at mint) → 5.1. ✓ (speccable now, highest priority)
- §5.3 require:arti notice → 5.2. ✓
- writer expired-cap → 5.3. ✓ (renders Rung 3 datum)
- QR gen+scan both platforms → 5.4. ✓ (net-new native)
- editor handshake → 5.5, GATED on delegate_section FFI (5.0b).
- real follow action → 5.6, GATED on follow_site FFI (5.0a).
