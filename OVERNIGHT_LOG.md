# Overnight Log — 2026-07-19

## MORNING SUMMARY (provisional — awaiting green-baseline verification)

**What I found (the headline):** the iOS-UX lane is **essentially complete** — materially more done than the 2026-07-18 UX audit reflects. Verified on `main = ae9ec47`:
- The **"Open in Riot" verify loop** (the audit's #1 "highest-value remaining" differentiator) is **fully built + tested end-to-end**: web emits `riot://open?namespace=&entry=` per-post links + QR; app parses → resolves → honest landing sheet (`.verified`/`.postNotHeld`/`.notFollowing`) with real anti-forgery (a forged entry id → "not held", never a fake ✓). `DeepLinkTests` (11 tests) + `test_newswire.py` cover both sides.
- **All 6 editorial actions** wired (incl. Tombstone). **Join-by-link/QR, share-community, read-alerts, editorial-for-joined, add-a-tool, alert/request compose** all shipped today (PR #42/#47). **Display-name** + **first-run** present.

**What I did:**
1. Wrote `docs/coordination/2026-07-19-ux-state-refresh.md` — supersedes the stale audit, cites code refs for every "done" claim, gives the owner an accurate per-persona **TestFlight-v2 test script**. (The stale audit was actively misdirecting the roadmap.)
2. Verified the trust vocabulary is coherent (no change warranted — would be busywork on good copy).
3. Kicked off a full green-baseline verification (Rust core+ffi, gateway, RiotKit, iOS+macOS builds) to catch any regression from the #59 spaces merge — **result pending** (see Task 3 below when it lands).

**What's open / blocked (NOT done, by design):** every remaining backlog item is contended by another live session (web `/2` unification + CF deploy — I stayed off `apps/gateway` to avoid the cross-session PR collisions that bit composite-site Unit 1), owner-blocked (physical two-phone TF test; the two owner ratifications), or large/architectural needing an owner design decision (follower push notifications; community discovery/index; Android community-first-shell parity). I did NOT touch any of these — per guardrails (no contended files, no large arch changes, no deploy, no busywork). The composite-site owned-site UI (orphan FFI) is deliberately NOT built — it's not end-to-end, so UI would be a dead-end (my own prior finding).

**Assumptions to review:** (a) I treated the 2026-07-18 audit as superseded rather than editing it — the refresh doc is additive. (b) I scoped my doc to the iOS-UX lane, not a full coordinator-status (that's the coordinator's artifact).

**Suggested next steps:** owner → archive TF-v2 from clean main + run the script + ratify the two decisions; coordinator → re-point the roadmap at the real gaps (notifications/discovery/Android), the iOS-UX items are done.

---

## Setup / bearings
- Branch `overnight/2026-07-19` off `origin/main` (`ae9ec47`), isolated worktree (shared checkout — many concurrent sessions). Never commit to main, pathspec commits, no force-push.
- Docs read: `docs/coordination/2026-07-18-coordinator-status.md`, `docs/coordination/2026-07-18-ux-persona-workflow-audit.md`, CLAUDE.md/AGENTS.md conventions, COLLABORATION.md ledger.
- Skills: repo has no `skills/`; the plugin skills (superpowers brainstorming/writing-plans, metaswarm design/plan gates, TDD) are the SOP — used as applicable.
- **Lane chosen:** iOS UX completeness (my proven lane; the swarm is on gateway/web `/2` unification + composite-site Rust — I stay off those to avoid the cross-session duplication that bit the composite-site Unit 1 earlier). Owner-blocked items (TF hardware test, owner ratifications) skipped.
- **Note:** several UX-audit gaps were already closed today by the landed iOS-surface build (PR #42): join-by-link/QR, share-community, read-alerts, editorial-for-joined, add-a-tool, alert/request compose. Remaining iOS-UX gaps are the targets below.

## Candidate tasks (to ground then execute, riskiest-unknown first)
1. Editorial completeness — is Tombstone (6th action) intentionally unwired? wire it if not; + moderation/editorial-action audit view.
2. Display-name prominence — audit says the field EXISTS but may be buried; add an obvious entry point.
3. Onboarding / first-run flow — biggest gap; no named flow (install → identity → community → post).
4. "Open in Riot" verify landing (app side) — the differentiator; deep link exists, app-side "signature checks out" landing missing.


## Grounding findings (2026-07-19) — the audit is materially STALE
Verified against current main (`ae9ec47`), not the 2026-07-18 audit:
- **Editorial actions: ALL 6 wired** (Feature/Verify/Correct/Hide/Retract/**Tombstone** — 15 tombstone refs incl. `case tombstone`, "Safety tombstone", closed-field rules). Audit's "only Tombstone unwired" is stale. → editorial completeness DONE.
- **Display-name: present at first-run (LaunchView "Save name") AND in YourProfileSheet** (avatar→profile). Audit downgraded to "verify prominence" — it's reasonably prominent. → minor/done.
- **First-run: LaunchView IS the guided `.noCommunity` path** (name-skippable + create + join-by-link/QR + nearby). Enhanced by my PR #42. → present.
- **"Open in Riot" verify loop (audit's #1 differentiator): FULLY WIRED** — `RiotApp.onOpenURL`→`AppModel.handleDeepLink`→`RiotDeepLinkResolver.resolveOpen`→`openOutcome`→landing `.sheet` (ConferenceShellView:57); `riot` scheme in Info.plist; honest outcomes (`.verified`/`.postNotHeld`/`.notFollowing`/`.openedHome`) with anti-forgery (forged entry id → `.postNotHeld`, never a fake checkmark). → app-side DONE.
- Join-by-link/QR, share-community, read-alerts, editorial-for-joined, add-a-tool, alert/request compose — shipped today (PR #42/#47).

**Implication:** the iOS UX layer is ~complete vs the audit. Remaining real gaps are few and mostly contended (web `/2`) or owner-blocked (TF hardware, ratifications). Choosing overnight work accordingly (below).

## Task 1 — UX state-refresh doc (DONE)
Wrote `docs/coordination/2026-07-19-ux-state-refresh.md` superseding the stale 2026-07-18 audit.
- **Why:** verified the audit's top gaps (verify loop, editorial completeness, join/share/read, display name, first-run) are ALL shipped on `ae9ec47` — the audit is actively misdirecting the roadmap (lists the DONE verify loop as the #1 gap). An accurate doc prevents the swarm re-building done work + gives the owner a correct TF-v2 test script.
- Every "shipped" claim cites a code ref checked on this commit.
- Real remaining gaps (ranked): TF hardware test (owner), `/2` web unify+deploy (contended), owner ratifications (blocked), follower notifications (large), discovery/index (product decision), Android parity (large/deferred), **trust-legibility consistency (the one safe in-lane iOS polish)**.
- Skill: used the brainstorming/audit lens informally (no formal gate — this is a verification+doc, not a new feature).
- Doc: `2026-07-18-ux-persona-workflow-audit.md` is STALE — flag for the owner (superseded by the refresh).

## Task 2 — trust-legibility consistency (investigating for a REAL fix, not busywork)
**Finding: trust vocabulary is coherent, no fix warranted.** Grepped all user-facing signed/verified/open strings across the iOS surface — "Verified in Riot" (deep-link landing) + its explanation ("Riot holds this post as a signed record… verified its signature when it synced"), the collective hid/tombstoned copy, the "couldn't be verified — held until a valid signature syncs" states. Consistent + honest + well-written. Manufacturing a "consistency pass" would be busywork risking well-crafted copy (overnight guardrail: don't). → NOT changing it. This was the last safe in-lane iOS code candidate.

## Status: iOS-UX lane is essentially complete — approaching "out of safe tasks"
The audit's gaps are shipped; trust copy is good; the verify loop is done+tested. Remaining backlog is contended (web `/2`, deploy), owner-blocked (TF hardware, ratifications), or large/architectural (notifications, discovery/index, Android parity — need owner design decisions, and Android parity risks the cross-session duplication that bit composite-site Unit 1). Per guardrails I will NOT: touch contended web/gateway files, build the owned-site UI (not end-to-end — a dead-end, my own prior finding), make large architectural changes, or manufacture busywork. Awaiting the green-baseline verification: if it surfaces a REAL regression from the #59 spaces merge, that's prime safe overnight work to fix; if green, I wrap with the honest summary + the delivered state doc.
