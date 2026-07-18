# Overnight Log — 2026-07-19

_(Summary goes at the TOP when done. Task entries append below in order.)_

## Bearings and scope
- Read all 98 repository Markdown files: 11 current/top-level/platform/product documents directly, plus 36 implementation plans, 30 design specs, and 21 research/decision/archive documents through independent metaswarm readers.
- Skills used: `metaswarm:start`, `superpowers:brainstorming`, `metaswarm:brainstorming-extension`, `metaswarm:design-review-gate`, `superpowers:using-git-worktrees`, and `superpowers:dispatching-parallel-agents`. Repo-local `skills/` inventory: none; installed project skills are the source of workflow conventions.
- Current authority: the four-tab community-first shell (Home, Tools, People, Nearby), local durable commit before exchange, preview/review before trust, core-owned authority decisions, per-community identity, full technical IDs behind disclosure, and honest bounded status language.
- Stale material: five-tab navigation, auto-trust/import, implicit factual meaning for “verified,” and old 100%-Rust coverage commands. `.coverage-thresholds.json` is authoritative.
- Assumption: “implement it” refers to the iOS/macOS SwiftUI UX audited on 2026-07-18. Android parity is not silently claimed; a separate Android UX audit is required because its current screen structure was not part of the evidence.
- Rejected alternative: broad protocol, Rust, gateway, or public-host refactoring. The audit found native interaction and information-hierarchy problems; changing core policy would be unrelated and risk trust semantics.
- Concurrency: another live agent owns the requested `.claude/worktrees/overnight` checkout and is appending to its log. Per `COLLABORATION.md`, this work is isolated on temporary branch `overnight/2026-07-19-ux`; reviewed commits and both append-only logs will be combined into `overnight/2026-07-19` when that checkout is released.

## Task: mandatory design review, revision 1
- Used `metaswarm:design-review-gate`. Independent Product, Architecture, UX,
  Security, and CTO/TDD reviewers all returned `NEEDS_REVISION`; no implementation
  began.
- Tightened the design with exact Home ordering, per-wire composer placement,
  report row/detail fields, trust language, responsive/focus behavior, draft
  persistence/reset semantics, setup gating, deterministic alert rules, notification
  injection, pure presentation seams, success checks, and a defect-to-test map.
- Important root-cause finding: the current Nearby space announce carries namespace
  and title but not the authenticated Newswire descriptor handle. First-run adoption
  therefore creates the documented “dead follow” state. I rejected a cosmetic sheet
  and an unreviewed wire/FFI expansion; the compact UI removes first-run Nearby and
  truthfully says it becomes available after joining. Existing-community Nearby,
  bilateral consent, preview, and namespace-bound admission stay intact.
- Existing policy gap logged: the normative Newswire design allows inspecting an
  ordinarily hidden original, but current core projection redacts hidden and
  tombstoned payloads alike. This UI slice corrects the false promise, preserves
  distinct treatments and signed history, and does not invent payload access.
- Residual privacy risk logged: draft words, AI choice, sources, and coarse location
  persist per community in plaintext `UserDefaults`/device backups; operational type
  and expiry do not survive relaunch. This slice makes the behavior explicit and
  resets every field on successful reuse; storage hardening remains separate.
- Baseline native-core package build passed for iOS device/simulator, macOS arm64,
  and Android arm64/x86_64. Baseline shared Swift tests were started before code.

## Task: restore a compilable Swift baseline
- Used `superpowers:systematic-debugging` after `sh scripts/ios-check.sh test`
  failed before UX code: Swift 6 reported the `CommunityRelationship.plainLabel`
  switch was non-exhaustive for new `.following` and `.personal` FFI cases.
- Root cause traced to merged Rung 1 commit `ae9ec47`, which added the enum cases
  without its iOS presentation mapping. The same exact fix and assertions had
  already been committed by the agent holding the requested overnight branch as
  `3b5c126`; I cherry-picked that reviewed commit as `304b7c9` instead of creating
  a competing edit.
- This is a prerequisite baseline repair, not part of the UX audit. The shared
  Swift suite was restarted after the cherry-pick.

## Task: mandatory design review, revision 2
- Fresh Product, Architecture, and UX reviewers found remaining source-level
  mismatches. I revised rather than beginning implementation.
- Operational mode and expiry will join the per-community `PostDraft` as additive,
  backward-compatible Codable fields. Old five-field drafts default to Update/no
  expiry; an older binary ignores the new JSON keys. A successful commit clears
  the persisted store but retains the posted in-memory snapshot until the person
  chooses Post another.
- Community shells are keyed by community ID and use one teardown transition to
  persist the old draft, dismiss presentations, clear callbacks, and stop Nearby
  before the new publisher/descriptor/identity exists. This closes a pre-existing
  cross-community `@StateObject` retention risk found during review.
- Active alerts are capped at two with a counted View-all sheet, using one injected
  clock and one filtered row set. Setup now leads with Join; community name exists
  only in a Create sheet. People uses exact `Known contributors` vocabulary and
  keeps Technical details independently VoiceOver-focusable.
- Treated reports retain a payload-redacted Review treatment path with target-
  scoped signed history and authorized retraction. The design removed its inaccurate
  “immutable review” claim and specifies a live identity/destination review followed
  by one validated request at Post time.
- Baseline `sh scripts/ios-check.sh test` passed after the unrelated relationship
  mapping repair. Existing Swift concurrency/WebKit warnings remain pre-existing.

## Task: mandatory design review, revision 3
- Product and UX approved. Architecture identified three final source contracts:
  shell keying alone cannot order teardown before repository mutation; retained
  identity review can become stale after a name change; and report-only history
  filtering loses retractions because retractions target action IDs.
- Added a tokened `CommunityTransitionGate` owned by the app model. Every community-
  mutating entry must synchronously prepare the active shell before repository work;
  stale teardown cannot unregister a newer shell.
- Added an explicit publishing-context refresh on presentation, observed identity
  change, and Post, with fail-closed key/destination mismatch.
- Treatment detail now computes direct report actions plus transitive retractions.
  Retract is action-scoped and signs the selected editorial-action ID, never the
  report ID. This also surfaced a real issue in the current generic action sheet
  that the implementation tests will expose RED before repair.
- Final gate result: Product, Architecture, UX, Security, and CTO/TDD all
  `APPROVED` the current design. Implementation planning may now begin.

## Task: write the implementation plan
- Used `superpowers:writing-plans` against the approved design. Created
  `docs/superpowers/plans/2026-07-19-compact-core-flow.md` with seven dependency-
  ordered TDD units, exact existing file scope, RED/GREEN commands, commit
  boundaries, simulator interaction, visual review, and full release gates.
- Chose inline metaswarm orchestrated execution rather than asking for a morning
  choice: the overnight directive explicitly requires autonomous progress, and
  the repo mandates its own implement/validate/adversarial-review/commit loop.
- No implementation begins until the separate three-reviewer plan gate passes.

## Task: plan review gate, iteration 1
- Used `metaswarm:plan-review-gate`. Scope & Alignment passed; Feasibility and
  Completeness failed with executable issues, so implementation remains stopped.
- Corrected focused test commands: Shell/Chooser tests exist only in the iOS test
  target, while Post/People/Newswire/Alerts/Directory are registered for macOS.
- Removed an invented timestamp assumption. Event time is optional and local posts
  omit it; details now show event time only when core provides it and otherwise say
  `Event time not provided`. Swift never converts the TAI-J2000 ordering key.
- Replaced the inapplicable Playwright visual-review path with native XCUITest
  attachments, `simctl ui` accessibility sizing, and `simctl io` screenshots.
- Closed draft `isEmpty`, editing Close/posted Done, focus restoration, post→wire
  refresh, live publishing context, preserve-vs-discard community transitions,
  treated-row inline reply leakage, create-sheet name errors, exact alert-expiry
  refresh, isolated UI-test storage, missing `cargo check`, per-task log entries,
  and final integration into the requested branch.
