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

---

## Anchor network plan-repair session — 2026-07-19

### Task 0 — bearings, documentation, skills, and collision avoidance

- Read all 136 Markdown files visible in the repository, following their cross-references, before
  editing. Used the repository instructions plus `divine-context`, `metaswarm:start`,
  `superpowers:using-superpowers`, `superpowers:dispatching-parallel-agents`,
  `superpowers:using-git-worktrees`, `superpowers:writing-plans`,
  `metaswarm:plan-review-gate`, `metaswarm:orchestrated-execution`,
  `superpowers:test-driven-development`, and `superpowers:verification-before-completion`.
- Current code and the 2026-07-19 anchor build-state/addendum supersede older branch-era status.
  M1 is complete on `origin/main` at `1f6ecb2`; M2–M4 are the active product trunk; pilot WU-024/025
  and pilot-only operations are deferred until a scheduled pilot has human coordinators and signed
  public fixtures.
- Found two concurrent WU-013A implementations already editing `riot-anchor` in separate worktrees.
  I did not touch their files. This lane repairs the governing plan and mandatory gate only.
- **Branch assumption:** the requested exact branch `overnight/2026-07-19` is already checked out by
  the UX overnight session and contains unrelated committed work. Reusing or rewriting it would
  violate shared-checkout safety, so this isolated lane uses
  `overnight/2026-07-19-anchor-plan` from current `origin/main`. Alternative rejected: move the
  existing branch or combine unrelated histories.
- **Process-doc conflict:** `COLLABORATION.md` says to use `git pull --rebase --autostash`, but the
  newer anchor incident report proves the stash stack is shared and records a recovered foreign
  stash loss. Followed the newer, more specific rule: never stash/autostash in this repository.
- No project-local `SKILL.md` or `skills/` directory exists; the installed skills named above are
  the applicable repository workflow.
- Open questions for morning review: archive or reconcile stale collaboration claims; decide
  whether the deferred pilot should become a separate spec immediately or only when scheduled.

### Task 1 — repair the failed implementation plan

- Used the writing-plans and plan-review-gate requirements plus the latest failed feasibility/
  completeness findings. Reordered the canonical signed-directory protocol to WU-011C before the
  real client directory adapter and ordinary listing. Added explicit dependencies for directory,
  replica/gossip, daemon routing, HTTPS, and deployment.
- Replaced the fake-port-only client directory approach with a production adapter that consumes and
  verifies WU-011C canonical signed records/cursors over safe dialing; fake transports are test-only.
- Added a non-publicly-constructible `VerifiedListingAuthority` proof-token acceptance criterion so
  WU-015B cannot call the listing state machine before Willow entry/grant/Meadowcap/root verification.
- Split production packaging into reproducible daemon/renderer OCI images (WU-026A) and isolated
  deployment/recovery (WU-026B), with exact shell/Compose contracts and five-file work-unit ceilings.
- Removed pilot work from the active graph, traceability, operations, CI, secrets, release contract,
  and system tests. Former WU-024/025 are reserved for a separate future design/plan; this explicitly
  avoids pretending the missing collector report/native measurement boundaries are implemented.
- Replaced the gate-flagged vague verification phrases with named test/script commands in the
  affected units. Structural self-check: 44 active WUs; every unit declares at most five files;
  `git diff --check` is clean.
- Assumption: preserving the approved pilot requirements in the design is sufficient until a pilot
  is scheduled; keeping half-executable pilot work in this active plan was rejected because it
  repeatedly caused false completeness claims and depended on unavailable humans/fixtures.

### Task 2 — plan gate iteration 1 (FAIL) and repair

- Three fresh isolated read-only Codex reviewers ran independently through the external-tools
  adapter because the built-in agent tree was at its thread limit. Verdicts: Feasibility FAIL,
  Completeness FAIL, Scope/Alignment FAIL.
- Repaired every blocking finding:
  - added WU-020P so daemon and isolated renderer share one dependency-neutral canonical
    `AnchorWebSnapshotV1` rather than depending on the server or duplicating a grammar;
  - added WU-012C for real Release-fails-closed bootstrap verification/injection into iOS and
    Android application packages, plus required WU-022A/WU-023A runtime loading;
  - replaced the ineffective nested generic `.dockerignore` with Dockerfile-specific ignore files
    while retaining repository root as build context;
  - made OCI image build/inspection and a live local isolated Compose readiness/isolation/restart/
    recovery probe mandatory. Missing Docker blocks that future WU; static config cannot pass it;
  - replaced every gate-flagged native prose check with exact xcodebuild/Gradle/PlistBuddy commands;
  - restricted WU-027 to the named active non-pilot matrix and existing production seams, and made it
    test-only. A missing production seam returns to a separately scoped owning WU.
- This revision adds two focused units (WU-012C and WU-020P); there are now 46 active units, each
  still capped at five declared files.
- Guardrail note: the live Compose commands are future local acceptance criteria in the plan. This
  overnight session did not run them, deploy, touch production, or delete any real data.

### Task 3 — plan gate iteration 2 (FAIL) and final repair

- Three new isolated read-only reviewers evaluated commit `3c9cd63`. Scope/Alignment passed.
  Completeness failed only because WU-006B claimed production bootstrap packaging before WU-012C
  owned it. Feasibility additionally found the missing daemon-to-renderer production adapter, an
  invalid exact PlistBuddy assertion, unowned chaos-harness seams, and incomplete macOS bootstrap
  packaging.
- Repaired the bootstrap sequence: WU-006B now proves development fixture agreement only; WU-012C
  owns iOS/Android Release-fails-closed application packaging; new WU-012D owns the corresponding
  macOS application resource and shared-loader compatibility. The graph now encodes those
  dependencies and the macOS scope is explicitly compatibility, not an invented third product UI.
- Added WU-020C, a five-file production daemon/renderer boundary with a canonical durable job
  envelope, fsync+atomic rename protocol, networkless long-running renderer sidecar, daemon-only
  hostile-output validation/publication, and deterministic duplicate/crash/restart/partial-output
  recovery. Deployment images and live Compose now depend on that runtime rather than a fake port.
- Fixed the iOS background identifier probe to address array element `:0`, matching PlistBuddy's
  actual output format. Verified the macOS test target and scheme names directly from the checked-in
  Xcode scheme before retaining the exact `-only-testing:RiotKitTests-macOS/...` command.
- Moved every deterministic-harness seam into its production-owning unit: repository clock/failpoint
  (WU-013B), `WorkChallengeVerifier` (WU-014), gossip scheduler/clock (WU-018B), `AnchorKeyStore`,
  ingress limiters, and control/sync transports (WU-019), and renderer adapter (WU-020C). WU-027 is
  now strictly server-side test composition and cannot edit production code or claim native-client
  lifecycle coverage.
- Assumption: filesystem spooling is the narrowest auditable local IPC for a networkless renderer;
  a container socket, subprocess shell, or renderer-owned publication was rejected because each
  widens renderer authority and complicates crash-safe ownership transfer.
- Next gate action: commit this final revision and run iteration 3, the configured maximum. All
  three independent reviewers must pass; otherwise implementation remains blocked for morning
  escalation rather than bypassing the mandatory gate.
