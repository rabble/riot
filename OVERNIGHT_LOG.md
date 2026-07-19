# Overnight Log — 2026-07-19

## MORNING SUMMARY

**THE HEADLINE — main's iOS build was RED; I fixed it.** The green-baseline verification caught **two regressions that PR #59 (spaces/following) left on `main`**, both breaking the iOS build/test. Fixed both on branch `overnight/2026-07-19`, RiotKit now **441/441 green**, iOS app BUILD SUCCEEDED. **A PR is open — this needs to land on main promptly** (main's iOS build is currently broken for every session).

1. **`3b5c126` — compile break.** #59 added `.following`/`.personal` to the `CommunityRelationship` FFI enum but left `CommunityChooser.swift`'s `plainLabel` switch at 3 cases → Swift 6 "switch must be exhaustive" → **RiotKit + Riot app would not compile on main.** Added the 2 missing cases ("Following" / "Personal space") + extended `CommunityChooserTests`.
2. **`57021a3` — stale test.** Commit `9870bff` deliberately dropped the "local device only" tech phrasing (offline copy is now "Not connected") but didn't update `ShellNavigationTests.testConnectionStartsExplicitlyOffline`, which still asserted the old string. The #59 compile break masked the whole suite, so this only surfaced once compilation was restored. Test now matches the shipped copy.

**Secondary finding — the iOS-UX lane is essentially complete**, materially more done than the 2026-07-18 UX audit reflects (verified on `main`):
- The **"Open in Riot" verify loop** (the audit's #1 differentiator) is **fully built + tested end-to-end**: web emits `riot://open?namespace=&entry=` per-post links + QR; app parses → resolves → honest landing sheet (`.verified`/`.postNotHeld`/`.notFollowing`) with real anti-forgery (a forged entry id → "not held", never a fake ✓). `DeepLinkTests` (11 tests) + `test_newswire.py` cover both sides.
- **All 6 editorial actions** wired (incl. Tombstone). **Join-by-link/QR, share-community, read-alerts, editorial-for-joined, add-a-tool, alert/request compose** all shipped (PR #42/#47). **Display-name** + **first-run** present.
- Wrote `docs/coordination/2026-07-19-ux-state-refresh.md` superseding the stale audit (cites a code ref for every "done" claim; includes a per-persona TestFlight-v2 test script). Verified the trust vocabulary is coherent (no change warranted — editing good copy would be busywork).

**What's open / blocked (NOT done, by design):** every remaining backlog item is contended by another live session (web `/2` unification + CF deploy — I stayed off `apps/gateway`), owner-blocked (physical two-phone TF test; the two owner ratifications), or large/architectural needing an owner design decision (follower push notifications; community discovery/index; Android community-first-shell parity). I did NOT touch any — per guardrails (no contended files, no large arch changes, no deploy, no busywork). The composite-site owned-site UI (orphan FFI) is deliberately NOT built (not end-to-end → a dead-end).

**Assumptions to review:** (a) fixed the stale connection-copy test to match the code, judging `9870bff`'s copy change ("Not connected") as the intentional current state and the test as simply not updated — the alternative (code regressed, test right) is contradicted by 9870bff's commit message "drop 'local device only' tech phrasing". (b) Treated the 2026-07-18 audit as superseded via an additive refresh doc rather than editing it. (c) Fixing #59's regressions is slightly outside my nominal iOS-UX lane, but a red main build blocks every session — highest-value safe overnight work.

**Suggested next steps:** **maintainer → review + merge the `overnight/2026-07-19` PR to un-break main's iOS build (urgent).** Then owner → archive TF-v2 from clean main + run the refresh doc's script + ratify the two pending decisions; coordinator → re-point the roadmap at the real gaps (notifications/discovery/Android), the iOS-UX items are done.

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

## Task 3 — green-baseline verification (DONE) — found + fixed 2 real #59 regressions
Ran the iOS half of the baseline (RiotKit build+test, iOS app build) to catch regressions from the #59 spaces/following merge on `main`. It was NOT green — #59 left two defects:

**Regression A (`3b5c126`) — compile break.** #59 added `.following`/`.personal` to the `CommunityRelationship` FFI enum (`crates/riot-ffi/src/mobile_api.rs`) but `CommunityChooser.swift:10` `plainLabel` stayed at 3 cases. Swift 6 exhaustive-switch → **RiotKit + Riot app did not compile on main.** Fix: added `.following` → "Following", `.personal` → "Personal space"; extended `CommunityChooserTests.testRelationshipsRenderInPlainLanguageNotTechnicalTerms` (TDD: the 2 new assertions fail-first against the 3-case switch, pass after). Verified: `CommunityChooserTests` 20/20; iOS app BUILD SUCCEEDED.

**Regression B (`57021a3`) — stale test.** With A fixed the suite finally compiled and ran → 1 failure: `ShellNavigationTests.testConnectionStartsExplicitlyOffline` expected `"Offline · local device only"` but `AppModel.connectionDisclosure` returns `"Not connected"`. `git log -S` showed `9870bff` ("fix(ios): drop 'local device only' tech phrasing from the connection string") deliberately changed the copy and only touched `AppModel.swift` — the test was never updated. The #59 compile break had masked the whole suite so this stale assertion was invisible until now. Fix: assert `"Not connected"` (match shipped copy). Not flaky (deterministic string) — a genuine stale test.

**Result: RiotKit 441/441, 0 failures; iOS app BUILD SUCCEEDED.** Both fixes are isolated, TDD-backed, committed on the branch. Not run tonight (out of iOS lane / time): Rust `cargo test`, gateway unittest, macOS build — the #59 change is iOS-Swift-consuming-an-FFI-enum, so the Rust/gateway sides are unaffected by these two defects; a full cross-stack baseline is a reasonable follow-up but the iOS breakage was the live fire.

## Status: iOS-UX lane complete + main's iOS build un-broken
Delivered tonight: the UX state-refresh doc (Task 1), the trust-copy verification (Task 2, no change needed), and — the real win — the two #59 regression fixes that restore a green iOS build (Task 3). Branch `overnight/2026-07-19` is pushed with a PR flagging the red-main urgency. Remaining backlog is contended (web `/2`, deploy), owner-blocked (TF hardware, ratifications), or large/architectural (notifications, discovery/index, Android parity) — none safe to touch overnight per guardrails. Nothing further safe to execute; wrapping with the honest summary above.
