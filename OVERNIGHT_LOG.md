# Overnight Work Log — 2026-07-18 (spaces-first navigation)

Append-only. Newest entries at the bottom (morning summary goes at TOP when done).

---

# ☀️ MORNING SUMMARY (read this first)

**All on branch `overnight/2026-07-18` (pushed). Nothing merged to main overnight** (Rung 1 #59 hit main moments before your overnight brief — pre-dates the "no main" rule).

## Done + verified
- **Rung 1 (core following+personal relationships) — MERGED (#59), CI-green.** Executed subagent-driven earlier: additive `Following`/`Personal` (no registry migration), `FollowedSiteRow` + `list_followed_sites()` + `list_communities` Following-exclusion, S2 exposure guard. Workspace 1086/0.
- **Rung 2 (two-pane shell skeleton) plan — GATE-PASSED 3/3.** Ready for native execution (by you, on-device). A 4-step increment ladder (pure model → iOS plumbing → iOS/macOS shell → Android), each green-keeping.

## Drafted, NOT gated — each blocked on a decision or FFI gap (your call)
- **Rung 3 (followed-site detail) — DRAFT (grounded, 562 lines) + 3 FFI GAPS.** (1) no fetch-manifest-by-root FFI → resolved in-plan by a small `resolve_followed_site(root, now)` store-read method; (2) `ResolvedSiteItem` has NO title/body content → a **view-model contract decision (yours)** — Rung 3 ships the headline-less trust/moderation view without it; (3) no `unfollow`/`follow` FFI → render a non-actionable Following badge, defer the action to Rung 5. **KEY: a concurrent session already landed most of the Swift composite read-model** (`CompositeSiteReadModel`/`CompositeItemRow` in `NewswireEditorial.swift` + tests) — Rung 3 builds on it, so it's materially smaller than expected. Trust-tier chrome (§4.1 security-UI) with the open-wire≠editorial mutation test is speccable now. (This is the late planning agent's richer version; it superseded my hand-written stub.)
- **Rung 4 (your-space) — DRAFT + BLOCKED.** A prior **gate-passed `personal-spaces-and-pages` design already exists** (owned namespace + published page, GeoCities-style) — "your space" and that feature are the same object. **3 questions for you:** (1) is your-space == personal-spaces? (2) how much of personal-spaces is built (owned.rs exists; page/store slices look unfinished)? (3) bounded-home (spaces-first §11 B) vs full-CMS (personal-spaces design) — which governs? They conflict.
- **Rung 5 (Unit 6 obligations) — DRAFT.** Seizure disclosure (§9.3, mandatory, blocking at `create_owned_site` mint) is fully speccable + is the priority. FFI gaps: `delegate_section` (editor handshake) and `follow_site(ticket)` are NOT in FFI yet. QR/camera net-new both platforms.

## ⚠️ Needs a fix (found overnight): Rung 1 has TWO latent NATIVE build breaks
Adding `CommunityRelationship::{Following,Personal}` (merged in #59) makes two landed switches non-exhaustive once native bindings regenerate — **invisible to Rust CI** (native app CI is deferred), so #59 went green with them latent:
1. iOS `apps/ios/Riot/CommunityChooser.swift` `plainLabel` (organizer/member/publicReader, no default).
2. Android `CommunityChooser.kt` `plainLabel()` `when` (+ a second `when`).
Rung 2 step 2.0/2.3 heal both. If you regenerate native bindings BEFORE executing Rung 2, add the `.following`/`.personal` arms first. (I did not fix these overnight — native, unverifiable on-device.)

## Assumptions I made (review)
- "Your space" == the personal-spaces feature (Rung 4 is the nav entry; personal-spaces is the contents). If wrong, Rung 4 rescopes.
- Planned-not-executed all native rungs (can't build iOS/Android on-device overnight). No unverified native committed.

## Suggested next steps (morning)
1. Answer the Rung 4 reconciliation (biggest unblock) + the Rung 3 view-model-content decision.
2. Decide whether to hotfix the two Rung-1 native switch breaks now vs. let Rung 2 execution heal them.
3. Execute Rung 2 (gate-passed) on-device; then Rung 3a FFI gap-fills (pure-Rust, once the content shape is decided).

Loop stopped after this summary (further progress needs your decisions).

---

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

### Wake 5 — Rung 3 drafted; TWO REAL FFI GAPS surfaced (morning decision)
Rung 3 draft written (main thread — the Rung-3 planning agent stalled at 144B; agents flaky overnight). Grounding the followed-site detail against the real Unit 4 FFI (site_ffi.rs) found the composite render is NOT sufficient to show a followed site's content:
- **GAP 1: no store-backed resolve-by-root.** `resolve_composite_site` (:436) requires the caller to SUPPLY signed manifest bytes; there's no `resolved_site_for_root(root, now)` that fetches the synced manifest from the store. The shell, given a followed root, can't render without this.
- **GAP 2: `ResolvedSiteItem` has NO content** (:385 — only entry_id/author/trust_tier/treatment; no title/body/timestamp). So the detail can render trust-tier chrome + moderation placeholder + degradation, but NOT article headlines/text. Adding content to the view-model is a **design decision on the Unit 4 FFI contract — USER'S CALL** (what content crosses FFI, ceilings). Overlaps Unit 4.
Rung 3 plan splits: **3a** = pure-Rust FFI gap-fills (resolved_site_for_root + item content) — cargo-verifiable + executable, BUT 3a.2 is a contract decision so I did NOT execute it overnight (guardrail: no big design decisions without user). **3b** = native render, planning-only. Marked DRAFT / gate-deferred pending the user's view-model decision. The non-spoofable trust-tier chrome (§4.1 security-UI) is speccable now (3b.2, anti-impersonation test).
NEXT: draft Rung 4 (reconcile with the prior personal-spaces-and-pages design) + Rung 5.

### Wake 5 (cont) — Rung 4 drafted; BLOCKED on personal-spaces reconciliation (morning decision)
Read the prior `personal-spaces-and-pages` design (gate-passed 2026-07-12) + slice1 plan. Finding: "personal space" is already a SUBSTANTIAL feature — owned namespace + published PAGE (HTML/CSS, public/connections-only), hard-dependent on the multi-space SQLite store + full-meadowcap + signed-js-apps designs. owned.rs primitives exist; page/store slices look UNFINISHED. My spaces-first "your space" (bounded pinned home, §11 B) is the SAME object at a narrower scope.
**3 BLOCKING questions for the user (Rung 4 not gate-ready until answered):** (1) is your-space == the personal-spaces feature? (2) how much of personal-spaces is actually built? (3) bounded-home (spaces-first §11 B) vs full-CMS (personal-spaces design) — which governs? These CONFLICT; doc-wins suggests personal-spaces governs contents, spaces-first bound governs what v1 nav surfaces.
Rung 4 draft = navigation-only skeleton that holds under either answer (4.1 Personal-tier assignment core; 4.2 bounded YourSpaceDetailView; 4.3 exposure boundary; 4.4 Android). Did NOT execute (blocked on decision, per guardrail). Committed as DRAFT.

### Wake 5 (cont) — Rung 5 drafted
Rung 5 (Unit 6 obligations) draft. FFI status grounded: create_owned_site EXISTS (seizure disclosure pins there — fully speccable, highest priority 5.1); delegate_section NOT in FFI (editor-handshake gap 5.0b); follow_site(ticket) NOT in FFI (follow-action gap 5.0a); QR/camera net-new both platforms. Native = planning-only; core gap-fills flagged executable-later.
All 4 rung plans now exist: Rung 2 GATE-PASSED 3/3; Rungs 3/4/5 DRAFTED with gaps+blockers surfaced. Remaining progress needs USER DECISIONS (personal-spaces reconciliation; view-model content shape) — so stopping the loop after the morning summary, per mandate.

### Wake 6 (final) — Rung 3 draft upgraded
The Rung-3 planning agent (thought stalled at 144B) actually finished at ~700s and produced a far richer, code-grounded Rung 3 that superseded my hand-written stub (committed 6cde56b). It confirms the 2 FFI gaps, adds a 3rd (no unfollow/follow FFI), resolves the fetch-by-root gap in-plan (`resolve_followed_site`), and found that a concurrent session already landed most of the Swift composite read-model — shrinking Rung 3. Morning-summary Rung 3 line updated. Loop remains stopped; all rungs planned/drafted, remaining work needs user decisions.
