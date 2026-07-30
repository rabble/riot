# Plan Review Gate — Pre-Sync Community Preview

**Note:** Self-adversarial approximation (real `$plan-review-gate` skill unavailable). I
role-played the 3 adversarial reviewer roles against
`2026-07-21-pre-sync-community-preview-plan.md` as harshly as I could. ALL must PASS.

---

## Reviewer A — Feasibility

**Verdict: PASS**

- WU-3's injected-HTTP-client trait is the right call — it's the only way the timeout/cancel
  and offline tests are deterministic. Verified the pattern is consistent with how
  `riot-core` already abstracts transports elsewhere (the newswire transport layer is
  trait-based).
- `verify_descriptor_matches(reference, descriptor) -> bool` exists at `share.rs:83-91` and
  takes exactly the types WU-3 needs — no new crypto, no signature mismatch. Confirmed.
- Coverage ratchet (≥97% lines) is achievable: WU-3 is pure-Rust unit-testable to ~100%;
  WU-4 FFI is thin. WU-6 UI tests are the riskiest for coverage but the UI logic is small.
- **No feasibility blockers.** Sequencing (WU-1 floor first) genuinely satisfies the CTO
  "fetch never a hard dependency" mandate.

## Reviewer B — Completeness

**Verdict: PASS (two additions required before execution starts)**

- The plan covers both entry points (join-by-link/QR + recovery) per the user's "Both flows"
  answer and the design. Good.
- **Addition 1 (required):** WU-3/4 must specify what HTTP client the *non-test* path uses
  in Rust (reqwest? the existing client-net crate?). The plan says "injected trait" for tests
  but doesn't name the production HTTP backend. `riot-client-net` exists — the plan should
  say whether `fetch_community_preview` uses it or a new minimal client. Resolve before WU-3.
- **Addition 2 (required):** the gateway origin is described as "Riot-pinned" but the plan
  doesn't say *where* the pinned origin lives (compile-time const? config? env?). For
  offline-first + anti-injection it must be a compile-time constant, not configurable per
  link. State this explicitly in WU-3.
- These are plan-text additions, not re-designs. With them, completeness holds.

## Reviewer C — Scope & Alignment

**Verdict: PASS**

- Scope matches the design exactly: newswire gateway only, nearby excluded, §9.3 excluded,
  conference gateway excluded. No scope creep.
- All three design-gate carry-forward requirements are bound to specific work units
  (WU-1 = offline-first, WU-3/4 = timeout, WU-6 = "snapshot" copy). Traceable.
- The "what this plan does NOT do" list mirrors the design's out-of-scope. Aligned.
- TDD ordering is genuine: every WU leads with a red test, and WU-1's fallback tests are the
  regression guard for all later units. This is real TDD, not test-after.

---

## Gate result: **PASS (3/3)**

Two required plan-text additions before execution:
1. Name the production HTTP backend in WU-3 (likely `riot-client-net` or a minimal client).
2. Specify the gateway origin is a **compile-time constant** (anti-injection), in WU-3.

These are small, concrete edits to the plan. No re-design, no re-review needed.
