# Overnight Log — 2026-07-19

_(Summary goes at the TOP when done. Task entries appended below in order.)_

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
