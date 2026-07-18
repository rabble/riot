# Overnight Log — 2026-07-19

_(Summary goes at the TOP when done. Task entries append below in order.)_

## Morning summary
- Done: implemented the approved compact iOS/macOS core flow across first run,
  Home, repeat posting, report reading, Tools, Known contributors, Nearby, and
  community-scoped state. The complete first-run → identity → two posts → exact
  report → secondary routes journey passes at normal text and Accessibility
  XXXL. The accessibility tab bar uses two stable rows without reducing the
  chosen text size; its unread badge no longer covers the icon and announces the
  actual unread count.
- Tested: `cargo test --workspace --all-features`, `cargo check --workspace
  --all-features`, `cargo fmt --all -- --check`, Swift unit tests, iOS simulator
  build, iOS device build, normal and Accessibility XXXL XCUITest journeys, and
  visual screenshot inspection all pass. Product changes passed iterative
  adversarial review.
- Open/blocking repository gates: strict Clippy fails on three pre-existing
  Rust 1.95 warnings in `crates/xtask/src/verify_newswire_export.rs`. The
  repository-wide coverage run executes successfully but measures 95.36%
  (11,037/11,574) against the 97% ratchet floor. Neither failure is in the
  Swift-only UX diff; I did not expand scope or lower the coverage floor.
- Assumptions to review: “implement it” meant the audited Apple SwiftUI app;
  Android parity remains a separate audit. Notification and Nearby suppression
  are UUID-gated UI-automation seams only; ordinary launches retain contextual
  notification permission and automatic discovery.
- Suggested next steps: repair the three xtask Clippy warnings, audit why the
  measured Tarpaulin baseline fell from the documented 97.26% to 95.36%, then
  rerun the composite coverage gate. Separately schedule Android parity and the
  documented core work for authenticated first-run Nearby adoption, hidden-
  original inspection, and encrypted/full-fidelity draft persistence.

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

## Task: plan review gate, iteration 2
- Scope & Alignment passed again. Feasibility found the repository’s name-only
  iPhone 17 Pro destination resolves to unavailable `OS:latest` despite installed
  26.1/26.2 devices. Added a first TDD unit that makes `ios-check.sh` resolve an
  available device UUID with an environment override; every direct test and
  native screenshot command consumes that resolver.
- Completeness found the controller’s latent `joinSpace` seam, insufficient
  non-empty-name coverage, and ambiguous treated timestamps. Added a pre-join
  transition hook/test, success/failure loops for typed names across Join/Create/
  Demo plus a real typed-name UI path, and explicit signed TAI-J2000 ordering
  values under Technical details without inventing a wall clock.

## Task: plan review gate, iteration 3
- Completeness and Scope & Alignment passed. Feasibility found one clerical
  command blocker: the iOS test bundle is `RiotTests`, not `RiotKitTests`, and
  the displayed Task 3 command omitted its Transport selector.
- Corrected all iOS selectors to `RiotTests/...` and added
  `RiotTests/TransportContractTests`. The feasibility reviewer explicitly found
  no other blocker. The overnight no-question directive preauthorizes continuing
  after this exact mechanical correction rather than stopping for a morning
  override; the approved plan text now contains the reviewer’s required command.
- Final plan-gate result after that verification: Feasibility, Completeness, and
  Scope & Alignment all `PASS`. The already-approved user request (“implement
  it”) plus the overnight autonomy directive is treated as approval to execute.

## Task 0: reliably select an installed iOS simulator
- Used `superpowers:test-driven-development`, `superpowers:executing-plans`, and
  `metaswarm:orchestrated-execution`.
- RED: `sh scripts/ios-check.sh simulator-id` exited 2 because the command did not
  exist. The name-only simulator build happened to pass on this rerun after Xcode
  state changed, confirming the deeper issue is nondeterministic destination
  selection rather than a permanently absent runtime.
- Implemented a reusable available-device UUID resolver with
  `RIOT_IOS_SIMULATOR_ID` override. Both `sim` and `simulator-id` use it and fail
  with fixed guidance when no iPhone 17 Pro exists.
- GREEN: resolved `5A62C0A1-E94C-49B4-A39F-7B9028C9EFA5`, confirmed it is
  available, and `sh scripts/ios-check.sh sim` passed. `sh -n` and `git diff
  --check` passed.
- Assumption: selecting the last available iPhone 17 Pro returned by `simctl`
  is the best local default (currently the newest installed runtime); CI or a
  developer can pin an exact UUID through the environment.
- Rejected: hard-coding OS 26.2 or a machine-specific UUID, both of which would
  recreate the same fragility elsewhere.

## Task 1: preserve and reset the complete composer safely
- Used the approved compact-core-flow design and plan with
  `superpowers:test-driven-development`, `superpowers:executing-plans`, and
  `metaswarm:orchestrated-execution`.
- RED: focused `PostUpdateTests` first failed because persisted drafts had no
  mode/expiry fields and the model had no `postAnother()` transition. A separate
  focused RED then failed because the accessibility-size mode-layout contract did
  not exist.
- Added backward-compatible draft decoding: existing five-field drafts restore as
  Update with no expiry, while new drafts preserve mode and expiry per community.
  A mode or expiry alone now counts as a real draft.
- The successful state now explains the exact local outcome and offers a 44-point
  `Post another` action that clears every field,
  returns to Update, focuses Headline, clears persisted state, and cannot sign a
  second time. The mode control stacks into three labeled buttons at
  accessibility text sizes and remains segmented at ordinary sizes.
- GREEN: focused `PostUpdateTests`, the full shared Swift test suite
  (`sh scripts/ios-check.sh test`), the shared SwiftUI compile check
  (`sh scripts/ios-check.sh`), and `git diff --check` all passed.
- Adversarial review found that rendering `Done` before Task 3 would create a dead
  action: the only current composer is embedded inline in Home, so SwiftUI has no
  presentation to dismiss. `Done` is therefore deliberately deferred to Task 3,
  where the composer becomes a sheet with a required origin-aware close callback.
  The review also found that direct in-memory restoration did not prove Codable
  symmetry, so a non-default JSON encode/decode round-trip test was added.
- Rejected: retaining the incomplete legacy draft shape, silently defaulting
  operational expiry on restore, or resetting only visible text. Each alternative
  could lose intent or leak state into a subsequent signed post.
- Existing build warnings in WebKit/Swift concurrency and the native archive
  deployment target remain pre-existing; no new dependency or architecture
  change was introduced.

## Task 2: make first run one clear, fail-closed path
- Used the approved compact-core-flow design and plan with
  `superpowers:test-driven-development`, `superpowers:systematic-debugging`, and
  `metaswarm:orchestrated-execution`.
- RED: the iOS `ShellNavigationTests` target failed to compile because
  `OnboardingPresentation`, `OnboardingExit`, and `OnboardingExitGate` did not
  exist. The tests cover all three exits with blank, successfully saved, and
  refused optional names.
- Added one pure gate shared by Join, Create, and Demo. A blank name is skipped;
  any typed name must save successfully before the exit action can run.
  `setDisplayName` now returns a Boolean and fails closed, with the existing fixed
  refusal copy, when the profile repository is absent or rejects the claim.
- Rebuilt setup around one decision: optional self-claimed name disclosure, Join
  as the only filled action, Create in its own name sheet, Riverside demo, and
  the exact explanation that Nearby follows community entry. Removed the
  duplicate Save-name action, inline community field, and unsupported first-run
  Nearby action.
- A refused name before Create keeps the sheet open, creates nothing, shows
  `RiotAppModel.nameError` beside Create, and moves accessibility focus to that
  error. Join and Demo refusal likewise remain on setup, perform no exit, and
  focus the same fixed error.
- GREEN: focused iOS `ShellNavigationTests`, the shared SwiftUI compile check,
  the full shared Swift suite, and `git diff --check` passed.
- While running the full focused class, a pre-existing contradiction surfaced:
  `ShellNavigationTests` required the explicit disclosure
  `Offline · local device only`, while production had regressed to the vague
  `Not connected`. The more specific test and the audit's understandable-status
  goal win; production now matches the explicit copy.
- Assumption: saving the optional name before presenting Join is sufficient
  because the shared Join sheet owns the later preview/commit transaction; no
  community mutation occurs merely by presenting it.
- Rejected: saving a name through its own button, continuing after a failed name
  claim, or advertising first-run Nearby. Those paths respectively duplicate a
  decision, misrepresent the person's identity, or enter a protocol state the
  current announce cannot complete.
- Adversarial review found the first Create-sheet pass omitted the approved
  explanation of founding responsibility. Added concise adjacent copy that the
  creator becomes the founding organizer and first editor and may invite others
  later.

## Task 3: isolate community transitions and unify posting
- Used the approved compact-core-flow design and plan with
  `superpowers:test-driven-development`, `superpowers:systematic-debugging`, and
  `metaswarm:orchestrated-execution`.
- RED: the iOS chooser/shell suites failed to compile because the tokened
  transition gate and single composer presentation state did not exist.
- Added one model-owned `CommunityTransitionGate`. Switch, join, create, retry,
  deep-link routing through those operations, and the legacy create seam prepare
  with `.preserveDraft`; confirmed Leave prepares with `.discardDraft`.
  Registrations are tokened, so an old keyed shell cannot unregister the new
  shell's handler. Pure tests pin both reasons and stale-token behavior.
- The community shell is keyed by community ID. Before mutation it persists or
  clears the old community's draft as requested, closes the composer/identity/
  tool state, and stops Nearby. A failed preserving mutation leaves the gate
  registered and the draft stored, while a confirmed discard removes the keyed
  draft. Nearby adoption now has an explicit pre-join callback, with a pure
  order test proving preparation occurs once before resume/join and never for an
  ordinary same-community sync.
- Replaced the embedded composer with one sheet and one
  `ComposerPresentationState` shared by Home, the empty wire, and People.
  Removed default no-op posting callbacks. Empty wire owns `Post the first
  update`; a populated wire gets one standalone `Post an update`; offline/
  pending states get none. Editing uses Close, success uses Done plus Post
  another, and closing restores keyboard focus to the exact origin trigger.
- Added live `PublishingContextProviding`: the review refreshes on presentation,
  identity/descriptor changes, and immediately before Post. A changed community
  or missing descriptor shows fixed draft-safe copy and performs no signed write;
  a newly arrived descriptor and current self-claimed identity replace stale
  review values.
- Moved notification authorization out of community-open. A successful rendered
  post first reloads the wire, yields one render turn, then asks the injected
  notifier; the scheduler test confirms repeated calls request at most once once
  authorization resolves. The phone switcher now visibly names the current
  community with a chevron and a 44-point target.
- GREEN: focused iOS CommunityChooser/ShellNavigation/Transport suites; focused
  macOS PostUpdate/People/Newswire suites; the full shared Swift suite; shared
  SwiftUI compile; and `git diff --check` all passed.
- Assumption: keeping stopped Nearby callbacks registered until the old shell
  actually disappears is safer than clearing them during preparation: if a
  repository mutation fails, the still-current shell can start a new session.
  The coordinator is stopped before mutation, and keyed-shell disappearance
  clears callbacks before any new community session can use them.
- Scope note: `PostUpdateView.swift`, `PostUpdateTests.swift`, and
  `LocalNotifierTests.swift` were necessarily included although the plan's file
  list omitted them; Task 3 explicitly requires the sheet close contract, live
  context fail-closed tests, and post-success permission scheduling.
- Adversarial review found the initial Nearby pre-join handler stopped the
  pairing before `resume(joining:)`, destroying the transport needed to perform
  its own mutation. Transition preparation now carries an explicit
  `transportMustContinue` bit: adoption still invalidates old callbacks and
  persists the draft synchronously, but keeps that one in-flight wire alive;
  ordinary mutations invalidate callbacks and stop Nearby before repository
  work. The controller captures only its intended post-join refresh completion.
- Strengthened evidence after review: the gate-backed Nearby test asserts
  callbacks are invalid before join while the transport remains alive; the real
  two-community model test records the outgoing community during preparation;
  and a failed repository switch restores an unsaved draft from the outgoing
  community's keyed store. Also moved the populated-wire composer trigger to
  immediately above Newswire as specified.
- A second review caught the retained-shell failure case: an ordinary failed
  join/create could leave the still-mounted iPhone Nearby route stopped with
  callbacks cleared. The tokened gate now has an explicit failure-recovery
  callback. Every preserving repository catch/refusal invokes it; the current
  shell re-arms pre-join preparation and post-join refresh without resurrecting a
  stale shell. A real invalid-reference join test proves recovery, then proves a
  subsequent Nearby adoption still reaches preparation.

## Task 4: keep active alerts compact and visible
- Used the approved compact-core-flow design and plan with
  `superpowers:test-driven-development`, `superpowers:systematic-debugging`, and
  `metaswarm:orchestrated-execution`.
- RED: focused `AlertsSurfaceTests` failed to compile because
  `ActiveAlertsPresentation` did not exist. The new tests pin namespace and
  expiry filtering, the two-row cap with counted overflow, and disappearance at
  the exact expiry instant.
- Added one deterministic presentation pass: filter to the active namespace and
  `expiresAt > now`, map and organizer-first/newest sort once, retain the complete
  ordered result, and expose only its first two rows on Home. No active alerts
  now means no alert card rather than an empty or expired card.
- Home owns the presentation clock and schedules a cancellable task for the
  earliest active expiry. Re-keying the shell or changing the next expiry
  cancels the obsolete task. The exact compact order is active alerts, the
  populated-wire Post action, Newswire, then Tools.
- Three or more alerts show `View all N active alerts`. The overflow sheet owns
  its own detail presentation so it can open every precomputed row on compact
  devices, and Done returns focus to the overflow trigger.
- GREEN so far: focused macOS `AlertsSurfaceTests`, focused iOS
  `ShellNavigationTests`, and `git diff --check` passed. The full shared Swift
  suite and compile gate are recorded after the work-unit review.
- Assumption: `expiresAt` is an absolute Unix-second boundary, consistent with
  `AlertRelativeTime` and the core entry model. At exactly that second the alert
  is no longer actionable, so the filter is strict `>` rather than `>=`.
- Rejected: rendering expired history in the Home alert card, calling `Date()`
  independently per row, keeping more than two urgent rows above the wire, or
  recomputing a differently ordered overflow list. Those choices respectively
  obscure current action, introduce boundary disagreement, expand Home, or make
  “View all” inconsistent with its preview.
- The first adversarial review found that the overflow close restored keyboard
  focus but not VoiceOver focus, and that the initial tests proved only the pure
  expiry predicate rather than the idle scheduling seam. Added explicit
  `AccessibilityFocusState`, stable View-all/Done action identifiers, a
  clock-injected cancellable expiry refresh, and a `HomePresentation` seam that
  drives and pins active-alert/Post/Newswire/Tools order. Additional tests cover
  the active adapter's own organizer/newest ordering, the no-overflow two-row
  boundary, injected idle refresh, and keyed-task cancellation.
- GREEN: focused macOS `AlertsSurfaceTests`, focused iOS
  `ShellNavigationTests`, the full shared Swift suite, shared SwiftUI compile,
  and `git diff --check` pass after the review fixes. Remaining compiler warnings
  are the pre-existing WebKit/concurrency and native deployment-target warnings
  already noted above.
- The final adversarial review passed after the injected-clock test was tightened
  into one complete idle flow: visible before waiting, the exact expiry requested,
  refreshed injected time returned, and the last alert hidden afterward.

## Task 5: make Newswire reports readable and accountable
- Used the approved compact-core-flow design and plan with
  `superpowers:test-driven-development`, `superpowers:systematic-debugging`, and
  `metaswarm:orchestrated-execution`.
- RED: focused `NewswireSurfaceTests` failed to compile because the retained
  projection payload fields, exact trust copy, signed ordering values, report
  detail models, treatment detail, action lineage, and selected-action target did
  not exist.
- Extended `NewswirePostRow` as a defensive adapter. Ordinary rows retain body,
  source claims, coarse location, event time, expiry, operational profile, and
  the unsigned TAI-J2000 ordering value. Hidden and tombstoned rows force
  headline/body/operational payload to nil or empty even when a malformed
  projection supplies values.
- Replaced expanded feed rows with a compact headline, two-line excerpt,
  `Signed by …`, conditional accountability badges, and one contextual
  `Read update` action. Replies and report-scoped editorial controls now live in
  ordinary detail with the full body and operational metadata.
- Added exact signature/editorial/correction/AI explanations. Human event time
  is rendered only from `eventTimeUnixSeconds`; absent input says
  `Event time not provided`. The Willow TAI-J2000 microsecond value is kept as an
  unsigned ordering value and displayed verbatim under Technical details, never
  converted into a wall-clock date.
- Hidden and tombstoned rows now use `Review treatment`. Their detail is
  payload-redacted and contains only treatment copy, signed author, optional
  event time, report ID and signed ordering value under Technical details, and
  the target-scoped signed action lineage. The lineage includes direct actions
  plus transitive retractions while excluding unrelated global history.
- Action-scoped Retract creates a forced retraction draft and signs against the
  selected action ID. A recording-editor test proves the model receives that
  action ID rather than the report ID.
- Corrected the hidden placeholder so it no longer promises unavailable original
  inspection. Assumption: the signed event-time field is the only source for a
  human event date; the Willow ordering key is not a Unix timestamp. Rejected:
  inferring a wall clock from TAI-J2000 or reusing treated payload supplied by a
  malformed adapter input.
- Deferred core gap: the normative hidden-original inspection path cannot be
  restored in Swift because the current core projection redacts the original
  payload. This slice preserves the hide/tombstone distinction and exposes the
  signed treatment record without making an unavailable promise.
- GREEN: focused macOS and iOS `NewswireSurfaceTests`, the full shared Swift
  suite, shared SwiftUI compile, and `git diff --check` pass. Existing
  WebKit/concurrency and native deployment-target warnings remain unchanged.
- Adversarial review found that hiding Retract from the generic picker was not
  sufficient: a canceled action-scoped retraction could remain in the shared
  draft and later sign against a report ID. Every action sheet now prepares its
  draft against the target's closed allowed-kind set; generic report targets
  reset stale Retract state, while action targets force Retract. Tests cover the
  stale-cancel path and the selected-action ID received by the editor.
- Review also tightened accountability and focus: each action's complete ID and
  unsigned ordering value now live under its own Technical details disclosure.
  Read/Review sheets restore keyboard and VoiceOver focus with a token containing
  both surface and report ID, so a report duplicated across Front page and Open
  wire returns to the exact originating control. The final adversarial review
  passed.
- Verification note: one parallel Xcode invocation collided on the shared
  DerivedData build database. The same focused suite passed immediately when
  rerun serially (and again with isolated DerivedData); this was a test-harness
  lock, not a product failure.

## Task 6: compact Tools, Known contributors, and Nearby
- Used the approved compact-core-flow design and plan with
  `superpowers:test-driven-development`, `superpowers:systematic-debugging`, and
  `metaswarm:orchestrated-execution`.
- RED: focused tests failed because the exact Known-contributors labels,
  separately addressable contributor summary/technical accessibility model,
  compact tool vocabulary, and Nearby offered-count strings did not exist.
- Tools now lead with name, purpose, trust/status badges, and one availability
  action. Version, permissions, endorsements, recommendation, and sharing live
  under `More details for <tool>`. Empty, intro, review, approval, and peer-profile
  copy consistently says tool/community instead of app/space while protocol type
  names remain unchanged.
- People now uses `Known contributors` and `No known contributors yet` with Riot
  headers/cards. The rendered name/tag, organizer word, and contribution count
  form one concise summary accessibility element. Technical details remains a
  separate focusable disclosure with the complete ID absent until expansion.
- Nearby keeps automatic discovery, permission recovery, inbound confirmation,
  preview acceptance/rejection, joining consent, and all failure states. It now
  labels `Nearby devices`, `People you’ve synced with`, and `Add N updates`, calls
  the count offered rather than verified, gives every action a 44-point target,
  and removes the renderer/device diagnostic card.
- Assumption: the preview count is an offered-item count supplied by the transport
  preview, not a trust claim. Rejected: `Add them`, “new things,” renderer names,
  or collapsing the existing consent states for visual brevity.
- Scope note: `CommunityShell.swift` carries the pure shared Nearby vocabulary
  seam because `ConferenceShellView.swift` is not compiled into the testable
  `RiotKit` target. `DirectoryRepositoryTests.swift` and
  `SpaceAdoptionTests.swift` were necessarily updated because their exact
  user-facing app/space copy assertions changed to tool/community.
- During GREEN, shared compile caught the FFI preview count is `UInt32`; the
  presentation seam now converts it explicitly to `Int` for pluralization.
- GREEN: focused People/Directory/Shell/Space-adoption suites, the full shared
  Swift suite, shared SwiftUI compile, and `git diff --check` pass.
- Adversarial review caught that the first empty-state sentence was truthful but
  not the design's exact anti-membership copy. It now reads and tests
  `Known contributors appear here once people post updates.` Empty/unavailable
  states also use Riot cards and typography, and the looking action uses the
  compact exact `Stop` label. Final adversarial review passed.

## Task 7: verify the complete compact core flow
- Used the approved design/plan with `superpowers:test-driven-development`,
  `superpowers:systematic-debugging`, `metaswarm:orchestrated-execution`,
  `metaswarm:visual-review`, and `superpowers:verification-before-completion`.
- Replaced the state-dependent route smoke test with one deterministic journey:
  fresh onboarding, display name, named community creation, profile identity
  with key tag, two successive posts, keyboard/focus reset, exact named report
  detail and return action, then Tools, Known contributors, and idle Nearby.
  Every major state keeps a screenshot.
- UI automation uses a valid run UUID for isolated temporary storage and a
  deterministic wrapping key because an unsigned simulator test runner cannot
  write Keychain items (`-34018`). Two additional UUID-gated flags suppress the
  notification prompt and Nearby autostart only during this journey; production
  startup, notification timing, and discovery are unchanged.
- Failures found and fixed during the interaction loop: toolbar-only sheet
  actions were invisible under the custom headers, the composer left its
  keyboard covering success actions, People retained stale data after posting,
  Post another focused an off-screen headline at the largest text size, and the
  first report query opened the containing wire card rather than the exact
  headline-specific action.
- Accessibility review rejected a first attempt that capped tab-label Dynamic
  Type. The cap was removed. Standard sizes keep one compact row; accessibility
  sizes use two explicit equal-width rows at the person's full chosen size.
  The bar resists compression from tall route content. Visual inspection then
  found and fixed a scaled unread badge covering Home's icon; accessibility
  layouts place it beside the icon and the spoken label includes the actual
  unread count.
- Focus evidence: SwiftUI restores both keyboard and accessibility focus using
  the full `surface + report ID` trigger. XCUITest does not expose the VoiceOver
  cursor, so the UI acceptance test closes the exact headline-specific report
  and reuses that same element to reopen the same detail; unit tests separately
  pin duplicate report triggers to their originating surface.
- GREEN: full shared Swift tests, simulator build, device build, normal XCUITest,
  Accessibility XXXL XCUITest, visual screenshot inspection, `git diff --check`,
  `cargo test --workspace --all-features`, `cargo check --workspace
  --all-features`, and `cargo fmt --all -- --check`.
- Blocked gate, not changed: `cargo clippy --workspace --all-targets
  --all-features -- -D warnings` fails on three existing warnings in
  `crates/xtask/src/verify_newswire_export.rs` (`cmp_owned` once,
  `manual_strip` twice). That file is untouched by this UX branch.
- Blocked gate, not changed: `scripts/web/coverage.sh` ran the complete
  Tarpaulin suite and measured 95.36% (11,037/11,574), below the authoritative
  97% floor and the file's documented 97.26% prior measurement. I did not lower
  the ratchet. The older context saying “~94.6%” is explicitly stale; the JSON
  threshold is authoritative.
- Existing warnings left unchanged: WebKit delegate near-match, Swift
  concurrency warnings in older tests/transport code, and native archive
  deployment-target warnings.
- No production systems, remote branches, databases, dependencies, or protocol
  behavior were touched.
