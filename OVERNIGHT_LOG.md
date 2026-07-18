# Overnight Work Log — 2026-07-18 (spaces-first navigation)

Append-only. Newest entries at the bottom (morning summary goes at TOP when done).

## Context at start
Marathon session already delivered: composite-site Units 0–5 merged to main; spaces-first
navigation design-gate-passed (spec `docs/superpowers/specs/2026-07-18-spaces-first-navigation-design.md`);
Rung 1 (core following+personal relationships) executed subagent-driven, CI-green, **merged to
main as #59 (ae9ec47) moments BEFORE the overnight brief arrived** — so it predates the "never
commit to main" rule; from here, ALL work on branch `overnight/2026-07-18`, pushed only as that branch.

## Plan for the night (guardrails)
- PLAN Rungs 2–5 of spaces-first through the plan-review gate (3/3). Doc work, gate-verifiable.
- Execute only pure-Rust-core slivers that `cargo test` can confirm.
- DO NOT commit unverified native (iOS/Android/macOS) code — cannot run Xcode/Gradle on-device
  overnight to confirm it passes (brief: confirm tests pass before moving on). Native execution
  is planned + logged for morning, not built.
- No merges to main, no force-push, no history rewrite, no new deps without logging.
- Self-paced loop via ScheduleWakeup; each wake advances one rung plan or gate round.

---

## Log

### Wake 1 — bearings + prior-art scan
Read all spaces/nav/personal .md docs. KEY FINDINGS (doc-wins discipline):
- **`specs/2026-07-12-personal-spaces-and-pages-design.md` (gate-passed) + `plans/2026-07-12-personal-spaces-slice1.md`**: personal spaces = OWNED Willow namespaces (root secret in secure storage; `pub`/`con` visibility path segments; `kind: page` bundles). This is DIRECTLY the "your space" tier of the spaces-first design (Rung 4). **OPEN QUESTION for morning:** is spaces-first "your space" the SAME feature as personal-spaces-and-pages? Assumption I'm proceeding on: YES — Rung 4 = surfacing/entering the existing personal-space concept in the new two-pane IA, NOT a new concept. Rung 4 plan will reference + build on the personal-spaces design rather than reinvent. Flagging because it's a design-reconciliation the user may want to steer.
- **`specs/2026-07-13-community-first-navigation-design.md` (gate-passed)**: the "Riot is organized around a community" design that produced today's single-community shell — the exact IA the spaces-first reshape reverses. Spaces-first spec §11 Decision C already records this reversal. No conflict; spaces-first supersedes it for the top-level anchor.
- **`plans/2026-07-18-ios-surface-unit6-compose-modes.md` (gate-passed)**: a DIFFERENT "Unit 6" — iOS composer mode picker (Update/Alert/Request). Naming collision with composite-site Unit 6. Separate track; relevant to Rung 5 compose flows only. Not a conflict.

Decision: proceed to PLAN Rung 2 (two-pane shell) first — biggest, most valuable, independent of the personal-spaces reconciliation. Native EXECUTION deferred (can't verify on-device overnight); planning is gate-verifiable and safe.
