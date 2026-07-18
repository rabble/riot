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

### Wake 2 — Rung 2 plan drafted
Architect agent drafted `plans/2026-07-18-spaces-first-rung2-two-pane-shell.md` — a 4-step increment sub-ladder (2.0 shared pure tier/row model XCTest-only; 2.1 iOS repo/app-model plumbing; 2.2 iOS/macOS two-pane shell; 2.3 Android mirror-models + root skeleton). Grounded against overnight branch (symbols, corrected line table).
Real gaps it found (for morning): (a) Swift RiotProfileRepository has no listFollowedSites() wrapper yet; (b) **Rung-1 FOLLOW-UP: Android's CommunityRelationship `when` is exhaustive with no else — regenerating Android bindings with Rung 1's new variants breaks Kotlin compile.** Not caught by Rust CI (Android app CI is deferred). Healed in Rung 2 step 2.3; flagged here so it's not a surprise. (c) macOS shares iOS sources → both pbxproj need entries per new Swift file (recipe included). (d) Android still on the OLD debug shell (flat ConferenceSurface enum, Views not Compose) → its Rung 2 is a skeleton, not a redesign.
Committing the draft; running the plan-review gate (3 reviewers) next.

### Wake 3 — Rung 2 gate: 2 PASS (Scope, Completeness), 1 FAIL (Feasibility)
Feasibility caught a real defect + a Rung-1 latent break:
- **RUNG-1 LATENT NATIVE BUILD BREAKS (morning action needed).** Rung 1 (#59, merged) added CommunityRelationship::{Following,Personal}. Regenerating NATIVE bindings breaks two landed switches that have no default/else:
  1. iOS `apps/ios/Riot/CommunityChooser.swift` `plainLabel` switch (organizer/member/publicReader) → non-exhaustive → Swift compile error.
  2. Android `CommunityChooser.kt:16` + a second `when` — same.
  Both invisible to Rust CI (native app CI is DEFERRED per memory), so #59 went green with these latent. The exhaustive-match trap AGAIN, native side. Rung 2 step 2.0/2.3 heal them, but if Rung 2 slips, someone regenerating iOS bindings hits it first. LOG PROMINENTLY.
- Rung 2 plan FAIL (fixable): Step 2.0 claims "touches no landed shell / safest first commit" — FALSE, it must also heal CommunityChooser.swift:plainLabel + regenerate iOS bindings (the plan gave Android a regen step but not iOS). Fix: add an iOS gap item mirroring Android; pull CommunityChooser.swift into Step 2.0 scope.
- Completeness non-blocking (fold in): (1) state the cross-kind chrome contract §4 lands with the renders (Rung 3/4), placeholders carry a tier-badge header stub; (2) add a pure-model row-filter test for §3.2 search; (3) note Android launch-restore rides the existing CommunityReturnOutcome mirror; (4) 2.1 add the explicit xcodebuild command.
Dispatching a fix to the Rung 2 plan, then re-review feasibility.

### Wake 4 — Rung 2 plan GATE-PASSED 3/3
Feasibility re-review PASS after the iOS plainLabel fix. Rung 2 (two-pane shell skeleton) plan is now 3/3 (Scope + Completeness + Feasibility). Marked gate-passed in the plan header. Ready for native execution BY THE USER (I don't build native overnight).
Rung 3 (followed-site detail) draft dispatched in parallel — pending.
