# Implementation Plan — Pre-Sync Community Preview

**Design:** `docs/superpowers/specs/2026-07-21-pre-sync-community-preview-design.md`
**Design review:** PASS 5/5 (`docs/superpowers/reviews/2026-07-21-pre-sync-community-preview-design-review.md`)

## Carry-forward requirements from the design gate (binding)

1. Tier-2 copy must say "as of last published snapshot," not imply liveness.
2. FFI fetch must be timeout-bounded + cancellable, tested.
3. Offline/tier-1 fallback built and green **first**; the fetch is never a hard dependency of join.

## TDD ordering principle

Each work unit: **red test → implement → green → refactor.** Coverage ratchet (tarpaulin
lines ≥ 97%) must stay green after every unit. The offline/fallback path (WU-1) lands and is
green before any fetch code exists (WU-3+), satisfying requirement 3 structurally.

## Work units

### WU-1 — Honest tier-1 copy refactor (no new fetch, no regression)
**Files:** `JoinByReferenceSheet.swift`, `ConferenceShellView.swift` (recovery view),
`CommunityShell.swift` (copy strings), `RiotTests/`.
- Red: test that the join sheet and the recovery view render an honest tier-1 message that
  explains *why* no name is shown and *what "sync" will do* (accessibility identifiers pin
  the copy). Today they render bare coordinates / "couldn't be opened."
- Green: rewrite the tier-1 copy on both screens. Join sheet: "Community `<short-ns>`. Its
  name and members arrive after you sync — the link only carries cryptographic coordinates,
  not a name anyone could spoof." Primary button relabeled "Sync to join." Recovery:
  "`<name>` couldn't be opened. Retry will re-attempt loading what this device already holds."
- This unit changes **no data flow** — it's the floor that must never regress. After WU-1,
  every later unit must keep these tests green (the fallback is always reachable).

### WU-2 — `CommunityUnavailable` enrichment + tests (data only, no UI yet)
**Files:** `CommunityShell.swift`, `AppModel.swift` (the failure sites that set
`communityUnavailable`), `RiotTests/`.
- Red: test that `CommunityUnavailable` carries optional `namespaceIdHex` + `contentDigestHex`
  and that the failure sites populate them when known (currently dropped).
- Green: add the two optional fields; thread coordinates from the failure sites. UI untouched.
- This enables the recovery screen to fetch a preview in WU-5 without a second data-model pass.

### WU-3 — Core: `preview.rs` — fetch + verify (the trust anchor)
**Files:** `crates/riot-core/src/newswire/preview.rs` (new), `newswire/mod.rs`,
`crates/riot-core/tests/`.
- Red: tests for (a) digest-mismatch ⇒ `None`, (b) offline/timeout ⇒ `None`, (c) valid
  descriptor matching digest ⇒ `Some(CommunityPreviewSummary)`, (d) timeout is bounded
  (requirement 2), (e) cancel safety.
- Green: `pub fn fetch_community_preview(reference, gateway_origin, timeout) ->
  Option<CommunityPreviewSummary>`. Uses an injected HTTP client (trait, so tests inject a
  fake) + `verify_descriptor_matches` as the gate. `CommunityPreviewSummary { name,
  member_count, recent_post_titles[] }`. No FFI yet — pure core, fully unit-tested.
- **"As of last published snapshot"** is the only liveness claim the type permits (its doc).
- **Production HTTP backend (plan-review requirement):** core takes an injected
  `PreviewHttpClient` trait; `riot-client-net` does **not** expose plain HTTP today (it's an
  iroh/Tokio runtime host, verified), so the production impl of the trait lives in `riot-ffi`
  behind a minimal `reqwest` blocking-async bridge (one new dependency, scoped to the FFI
  crate, not core). Core stays dependency-free and fully testable with fakes.
- **Gateway origin (plan-review requirement):** a **compile-time `const`** in `riot-ffi`
  (e.g. `const PREVIEW_GATEWAY_ORIGIN: &str = "https://riot-protest-net-marketing.protestnet.workers.dev";`).
  Never configurable per link — closes URL-injection. Core receives it as an opaque
  `&str` parameter; tests pass their test-server origin.

### WU-4 — FFI: `newswire_fetch_preview` (thin, timeout-bounded)
**Files:** `crates/riot-ffi/src/newswire_ffi.rs`, `crates/riot-ffi/tests/`.
- Red: FFI contract test — returns the summary or nil, never panics, never hangs (the timeout
  from WU-3 is enforced through the FFI boundary).
- Green: thin wrapper exposing `fetch_community_preview` over UniFFI; regenerates bindings.

### WU-5 — Gateway: `/preview/<ns>.json` endpoint
**Files:** `apps/gateway/newswire.py`, `apps/gateway/tests/test_newswire.py`.
- Red: test the route returns `{namespace, name, member_count, recent_post_titles[]}` derived
  from the existing `_from_v2` space block; 404 for unknown ns; correct cache headers.
- Green: add the route + handler. Conference gateway untouched.

### WU-6 — iOS/macOS UI: tier-2 verified-summary card
**Files:** `JoinReferenceModel.swift`, `JoinByReferenceSheet.swift`, `ConferenceShellView.swift`,
`RiotTests/`.
- Red: tests pin (a) tier-2 chip renders **only** after a verified fetch, (b) chip copy says
  "verified summary · as of last published snapshot," (c) on mismatch/offline the UI shows
  tier-1 (WU-1 tests still green), (d) tier-3 signed data overrides tier-2 on conflict.
- Green: wire the FFI fetch into the join sheet + recovery view; render the tier-2 card with
  the distinct chip styling; signed-overrides priority in the sync-merge path.

### WU-7 — Privacy page + docs
**Files:** `marketing/privacy/index.html` (+ public mirror), `marketing/README.md`.
- Add the privacy note: a preview fetch reveals the namespace id to the gateway operator
  (bounded same as followed-site). Update the marketing contract if needed.

## Final review + PR
- Full workspace test: `cargo test --workspace --all-features`.
- `cargo fmt --all -- --check` + strict Clippy (`-D warnings`).
- Coverage gate: `cargo tarpaulin --workspace --all-features --fail-under 97`.
- iOS/macOS tests green; contract script green.
- PR with this plan + design doc + review records.

## What this plan deliberately does NOT do
- No verified members/posts before sync (tier-3 stays authoritative).
- No nearby-flow preview change.
- No conference-gateway change.
- No §9.3 seizure-disclosure change.
