# Design Review Gate — Pre-Sync Community Preview

**Note:** Self-adversarial approximation. The real `$design-review-gate` skill was
unavailable in this session (metaswarm plugin not loaded). I role-played each of the 5
reviewer roles against `2026-07-21-pre-sync-community-preview-design.md` as harshly as I
could. Any BLOCKER below would have stopped the real gate.

---

## Reviewer 1 — Product Manager

**Verdict: PASS (with one concern noted, non-blocking)**

- Problem is real and user-validated (the user literally asked "why not join and give me a
  summary"). The current "Sync again" dead-end is a documented UX failure.
- Tiered trust ladder maps to a clear user story: "see what it is, then decide."
- **Concern (non-blocking):** the "member_count" and "recent_post_titles" in tier 2 come from
  the gateway, which serves them from the signed export. That's fine, but the design should
  be explicit that these are *export-time* counts/titles, not live — a community could have
  grown since the export. The design's "stale is accepted" note covers this but the *user
  copy* must say "as of the last published snapshot," not imply liveness. Flag for plan.

## Reviewer 2 — Architect

**Verdict: PASS**

- Reuses the followed-site HTTP-pull architecture (`import_followed_site_bundle`) as the
  trust template — correct precedent, same "hostile mirror can serve stale/empty, never
  forge" bound.
- Gateway origin is Riot-pinned, not link-supplied — closes the URL-injection vector. Good.
- `verify_descriptor_matches` as the trust anchor is the right call; it's the existing
  WILLIAM3 re-digest check.
- New `preview.rs` in core keeps fetch+verify in Rust (auditable, testable), FFI stays thin.
- **One requirement:** the FFI fetch must be cancellable/timeout-bounded so a slow gateway
  can't hang the join UI. Add to plan: explicit timeout + cancellation, tested.

## Reviewer 3 — Designer

**Verdict: PASS**

- Two distinct visual tiers (untrusted coordinates vs. verified-summary chip vs. signed) are
  the right call — never blur the trust boundary in the UI.
- Primary action relabeled to "Sync to join" (honest about what it does) vs "Sync again" —
  correct.
- **Requirement:** the "verified summary" chip must be visually distinct from the
  post-sync signed data, AND the tier-1 honest note must be visible whenever tier 2 fails.
  The design says this; plan must pin it with accessibility-identifier tests so it can't
  regress to a bare button.

## Reviewer 4 — Security

**Verdict: PASS — this is the most important review and it holds**

- The load-bearing anti-spoof invariant (`JoinPreview.title == nil`, with the existing
  `XCTAssertNil(preview.title)` test) is **preserved, not broken**: tier-2 name only appears
  *after* `verify_descriptor_matches` passes against the link's digest. The nil invariant on
  the *link-derived* preview is untouched; the verified name is a separate, digest-gated
  field. Correct.
- Namespace-id in the fetch URL leaks interest to the gateway operator. Design flags this
  and bounds it to the followed-site precedent (operator already sees requests). Acceptable,
  documented. The privacy page should be updated.
- **The one thing I checked hardest:** could a hostile gateway serve a *different
  community's* valid descriptor that happens to match the digest? No — the digest is over the
  descriptor's canonical bytes; matching means it's the same descriptor bytes, bit-for-bit.
  A different community has a different descriptor ⇒ different digest ⇒ mismatch ⇒ tier-1
  fallback. Sound.
- Stale-descriptor risk is accepted and bounded (tier-3 signed data overrides on conflict).

## Reviewer 5 — CTO

**Verdict: PASS**

- Scope is bounded: newswire gateway only (conference gateway's pinned-fixture purity
  preserved), nearby flow excluded (its human-trust model is separate), §9.3 untouched.
- File scope is concrete and matches the architecture.
- Risks (privacy leak, stale) are named with mitigations, not hand-waved.
- **Mandate:** the plan must sequence so the offline/tier-1 fallback is built and tested
  *first* (no regression to current behavior) before any fetch path lands. Don't let the
  fetch become a hard dependency of the join flow.

---

## Gate result: **PASS (5/5)**

No blockers. Three carry-forward requirements for the plan:
1. **PM:** tier-2 copy must say "as of last published snapshot," not imply liveness.
2. **Architect:** FFI fetch must be timeout-bounded + cancellable, tested.
3. **CTO:** offline/tier-1 fallback built and green *first*; fetch never a hard dependency.
