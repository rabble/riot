# Spaces-First — Rung 3: Followed-site detail — Implementation Plan (DRAFT — has FFI-gap prerequisites)

**Status:** DRAFT, authored overnight 2026-07-18. **NOT yet plan-review-gated** (agent flakiness overnight; gate in the morning). Surfaces two real FFI gaps the user should see before this rung executes.

**Goal:** Replace Rung 2's `FollowedSiteDetailPlaceholder` with the real followed-site detail (Editorial / Comments / Wire) rendering the Unit 4 `ResolvedCompositeSite`, with non-spoofable trust-tier chrome (§4.1) and the honest degradation states.

**Depends on:** Rung 2 (the shell + the `.following` → placeholder route) + Unit 4 FFI (on main). iOS + Android + macOS, shared Rust core.

**Spec:** `docs/superpowers/specs/2026-07-18-spaces-first-navigation-design.md` §4, §4.1, §7 rung 3.

---

## ⚠️ FFI GAPS — prerequisites, flagged for the user (found during grounding)

The Unit 4 FFI on main (`crates/riot-ffi/src/site_ffi.rs`) is **not sufficient to render a followed site's content** as-is. Two gaps, both pure-Rust-core (cargo-verifiable), both really Unit-4 view-model follow-ups:

1. **No store-backed resolve-by-root.** `resolve_composite_site(&self, entry_bytes, capability_bytes, signature, payload_bytes, root, now)` (`site_ffi.rs:436`) requires the caller to SUPPLY the signed manifest bytes. But the shell, given a followed root, has no way to fetch that synced manifest from the profile store. **Need:** `resolved_site_for_root(root: Vec<u8>, now: u64) -> Result<ResolvedCompositeSite, MobileError>` that pulls the synced manifest (+ the O/C/W entries) from the store by root and resolves it. Without this, the Following-tier detail has nothing to render from a mere root.

2. **`ResolvedSiteItem` has no content.** Its fields are `entry_id`, `author_subspace`, `trust_tier`, `treatment` (`site_ffi.rs:385`) — **no title, no body, no timestamp.** So the shell can render trust-tier chrome + a Hidden/Tombstoned placeholder + degradation, but **cannot show an article's headline or text**. **Need:** either add content fields to `ResolvedSiteItem` (title/summary/body/created_ts), or a companion per-entry content projection keyed by `entry_id` (mirroring how the newswire surface projects post content). This is a **design decision on the view-model contract** (what content crosses FFI, size ceilings) — arguably the user's call; it extends Unit 4's contract.

**Recommendation:** Rung 3 splits into **3a (core FFI gap-fills, pure-Rust, cargo-verifiable, executable)** and **3b (native render, planning-only until 3a lands)**. 3a is the real unblock; 3b is mechanical once the view-model carries content.

---

## Rung 3a — core FFI to make a followed site renderable (pure Rust; execute + cargo-verify)

- **3a.1 — `resolved_site_for_root`.** New `#[uniffi::export]` method: given a followed root + now, load the synced manifest entry + O/C/W entries from the store (the same store the sync path writes; grep `list_communities`/the store read path), call the existing resolver, return `ResolvedCompositeSite`. RED: a profile that has synced a followed site's manifest resolves it by root; a root with nothing synced → a `moderation-loading`/`pending` degradation (fail-closed, honest), NOT an error. Contract test in `crates/riot-ffi/tests/`.
- **3a.2 — item content on the view-model.** Add `title: String`, `summary: String`, `created_unix_seconds: u64` (and a bounded `body`/`preview`) to `ResolvedSiteItem`, populated by the resolver from the entry payload (reuse the newswire projection's content extraction — grep `NewswireProjection`/`ProjectedPost`). Size-ceil the body. RED: a resolved editorial item carries its headline + summary; an over-long body is truncated to the ceiling. **DESIGN NOTE for the user:** this extends the Unit 4 view-model contract — confirm the content shape.
- **3a.3 — UniFFI regen + native rebuild** (same-commit gate); full workspace gates + validate-contracts.

## Rung 3b — native followed-site detail render (PLANNING ONLY overnight; execute after 3a + with the user)

- **3b.1 — iOS/macOS `FollowedSiteDetailView`** replacing `FollowedSiteDetailPlaceholder` (Rung 2). Three sections Editorial / Comments / Wire, driven by `ResolvedCompositeSite.items` grouped by `trust_tier`. Mirror the newswire surface pattern (`NewswireEditorial.swift` / `NewswireSurfaceModel`). New Swift file → BOTH pbxproj.
- **3b.2 — non-spoofable trust-tier chrome (§4.1, SECURITY-UI).** The shell paints a trust badge (icon + shape + label — NOT colour alone; grayscale/colourblind/reshare-safe) AROUND each item from `item.trust_tier`; content never supplies its own tier. **RED (security):** an open-wire item does NOT carry the editorial badge/chrome; a test asserts the chrome is derived from `trust_tier` and an open-wire item renders the open-wire badge, never editorial. Mutation-style: if the view keyed chrome off anything but `trust_tier`, the test fails.
- **3b.3 — moderation treatment + degradation.** `treatment == Hidden/Tombstoned` → accountable placeholder row (never vanish); `ResolvedCompositeSite.degradation` → the honest state copy (moderation-loading / editorial-only / transport-blocked / manifest alarms), reusing Rung 2's `ShellRecoveryView`-style convention. `writer_cap_expired` → the compose-time warning datum (rendered in Rung 5's compose).
- **3b.4 — Follow/Unfollow control.** The header carries a Following badge; **Unfollow** removes the `Following` registry record (there IS a Rung-1 registry path — an `unfollow_site` core method is a small pure-Rust add, or reuse archive semantics). **Real `follow_site(ticket)` (the ADD) stays Rung 5** (ticket/transport parsing = Unit 5). Document.
- **3b.5 — Android** mirror (skeleton, given Android is on the old debug shell): a followed-site surface consuming the same FFI.

## Deferred to Rung 5
Real `follow_site(ticket)`, QR gen/scan, seizure disclosure (mint), `require:arti` compose notice.

## Self-review (§4/§4.1/§7 → tasks)
- §4 Editorial/Comments/Wire render → 3b.1; treatment placeholders → 3b.3; degradation → 3b.3. ◐ (needs 3a.2 content)
- §4.1 non-spoofable trust-tier chrome (SECURITY-UI) → 3b.2 with the anti-impersonation test. ✓
- §7 rung 3 followed-site detail → 3a (unblock) + 3b (render). Content render GATED on 3a.2 (FFI content gap).
