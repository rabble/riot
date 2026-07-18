# Riot Compact Core Flow Design

**Date:** 2026-07-19  
**Status:** Design review revision 1  
**Scope:** iOS and shared macOS SwiftUI surfaces only

## Problem

Riot’s core loop is present but not reliably usable. First-run Nearby changes an
invisible route, the empty-wire post action has no handler, newswire rows omit
the report body, and a successful composer cannot start another post. Home gives
equal visual weight to tools, two content systems, and a permanent composer,
obscuring the primary job: understand what is happening and contribute.

The work must improve comprehension without weakening Riot’s guarantees:
signatures establish key authorship and integrity rather than truth, display
names remain self-claimed and key-tagged, editorial and open-wire content remain
visibly distinct, and a local save is never described as global delivery.

## Users and outcomes

1. **A first-time contributor** wants to join or create a community so that they
   reach a useful Home screen, **when the app has no current community**.
2. **A reader** wants to scan and open a report so that they understand its
   content, authorship, provenance, and editorial treatment, **when Home contains
   ordinary or treated reports**.
3. **A contributor** wants to post repeatedly so that one successful local write
   does not strand the composer, **when they have just signed and saved an
   update**.
4. **A nearby participant** wants to understand the current connection step so
   that they review a peer and a concrete offered count before importing,
   **when they intentionally open Nearby from an existing community**.
5. **An organizer** wants to inspect permissions and editorial actions so that
   authority remains reviewable, **when they choose a secondary detail or signed
   action**.
6. **A member** wants to scan Tools and People so that useful names and purposes
   appear before trust internals, **when browsing directories on a phone**.

## Chosen approach

Keep the existing community-first shell and models. Add presentation state and
small typed helpers inside already-registered shared Swift files. Do not add a
navigation framework, core/database schema, FFI contract, dependency, or policy
layer. The local `PostDraft` Codable value gains backward-compatible optional
mode/expiry fields so operational work survives community switching and relaunch.

Rejected alternatives:

- A shell rewrite risks retained tab state, community switching, and macOS
  keyboard behavior.
- A unified Rust feed would blend alerts and the Newswire despite their distinct
  verified models.
- Removing trust detail would make the interface smaller by hiding accountability.
- First-run Nearby adoption cannot be made honest in a UI-only slice: the current
  pairing announce does not carry an authenticated Newswire descriptor handle,
  so adoption creates a dead follow whose Home cannot project or post. The dead
  control is removed and the protocol repair is deferred explicitly.

## Target flow

```text
Welcome
  → Join a community (QR/link)
  → Create one
  → Try demo
  → Nearby becomes available after joining

Community Home
  → visible community chooser
  → active alert summary, only when current
  → chronological/editorial newswire
  → one Post an update trigger for the current wire state
  → compact tool shortcuts

Post an update
  → compose
  → live review of identity and destination
  → local signed commit
  → “Saved and signed on this device”
  → Done | Post another

Read update
  → complete headline + body + operational metadata
  → Signed by author with key-derived tag
  → editorial checks/corrections/AI disclosure with explanations
  → replies and editorial action where authorized

Nearby (from an existing community)
  → Looking
  → peer selected and mutually confirmed
  → connected
  → preview “Add N updates”
  → accepted local import
```

## Screen design

### First run

Welcome copy becomes three short promises: read local updates, publish signed
reports, and exchange nearby without internet. “Works without a server” replaces
the absolute “No servers” claim.

Setup has one filled action: `Join with a link or QR`. Create and demo are
secondary. The exact order is optional self-claimed display name and disclosure,
Join, Create, Try the demo, then the informational line `Nearby exchange is
available after you enter a community.` A dedicated `Save name` button is
removed. Nearby is not a first-run action; the community shell retains the route.

The community-name field is not shown above the primary Join action. `Create a
community` opens a secondary sheet containing only community name, what founding
means, `Create`, and `Cancel`; save-before-exit occurs when Create is confirmed.
Join saves before presenting its link/QR sheet. Demo saves before loading.

Join, create, and demo all pass through one save-before-exit helper. An empty name
draft proceeds using the current key-derived identity. A non-empty name is
self-claimed public profile data; adjacent copy says it will be saved on this
device and shared with future peers. It must be confirmed by the repository
before the selected action begins. Any failure leaves the person on setup,
preserves their words, announces the fixed error, and starts no sheet, join,
creation, or demo load. Key-derived tags remain beside names everywhere.

### Home and shell

The phone header always shows `Community name ▾`, plus profile and settings.
Long names truncate rather than displacing 44-point controls.

Home starts with a bounded active-alert summary when unexpired alerts exist.
Urgent information cannot fall below an unbounded report list.
Expired-only and empty alert sets omit the card; this slice adds no alert history.
The card renders at most two organizer-first/newest alerts. With more than two,
`View all N active alerts` opens a complete active-alert sheet and restores focus
to that overflow button on close; its VoiceOver label includes the count. The
overflow sheet uses the same deterministic active filter and row detail.

Home has exactly one composer trigger per Newswire state:

| Wire state | Composer trigger |
| --- | --- |
| empty | `Post the first update` inside the empty-state card |
| pending first sync / offline | none in the recovery card |
| has reports | `Post an update` immediately above the Newswire |

The exact Home section order is active alerts when present, the standalone
composer trigger only when reports exist, the Newswire (whose empty card may
carry its single trigger), then Tools. Pending/offline recovery shows no
standalone composer trigger.

`CommunityShellView` owns composer presentation and the existing retained
per-community `PostUpdateViewModel`. Home, empty-wire, and People callbacks open
that one sheet through a small `ComposerPresentationState` transition
(`closed → open(origin) → closed`). `NewswireSurfaceView` and `PeopleView`
initializers require an explicit handler; no visible action can be constructed
with a default `{}`. Tools follow the content.

The parent keys `CommunityShellView` with `.id(community.id)`. Keying is backed by
an explicit `CommunityTransitionGate` owned by `RiotAppModel`. The active shell
registers one tokened preparation closure; every model entry that can mutate the
active community—chooser switch, link/QR join, deep-link switch, create, archive,
leave, retry/recovery—must call `prepare()` before its repository mutation.
Preparation synchronously persists the old community’s complete draft, dismisses
composer/detail/tool presentations, invalidates callbacks, and stops Nearby.
The shell unregisters only its own token on disappearance, preventing a stale
shell from clearing a newer registration.

Only after preparation may the repository switch and the new keyed shell own its
publisher, descriptor, identity, draft, and coordinators. A Community A draft
restores only from A’s store and can never be posted by B’s model. Leaving/removal
still requires the discard guard; ordinary switching preserves the draft and
needs no destructive prompt. Tests call every mutating model entry and assert
prepare precedes the registry operation.

Front page and Open wire remain separately labeled. No ranking or blending is
introduced.

### Reading contract

`NewswirePostRow` gains the projection’s body, timestamp, source claims, coarse
location, expiry, and operational profile. An ordinary Home row contains:

- headline and at most two body lines;
- `Signed by <display name · key tag>`;
- `Editorial checks: N`, `Editorial correction`, and AI-assistance badges only
  when applicable;
- one visible `Read update` button whose VoiceOver label includes the headline.

Rows do not inline replies, Reply, editorial controls, complete metadata, the
full body, or editorial history. Detail contains the full body, timestamp,
sources, location, expiry, operational type, replies, and authorized actions.

Trust copy is exact:

- `Signature checked` — `This key authored this report. A signature does not
  prove the report is true. Display names are self-claimed.`
- `Editorial checks: N` — `Signed editorial judgments from this community’s
  current editorial roster. They are evidence notes, not proof of truth.`
- `Editorial correction` — `Editors signed a correction. Review the signed
  history.`
- `AI-assisted · human reviewed and signed` — provenance, never a substitute
  author.

An ordinary hidden report remains a warning interstitial and a tombstone remains
payload-redacted. Current core projection redacts both payloads. The hidden
placeholder is corrected so it does not promise unavailable original inspection.
The row adapter assigns body and operational fields only for `.ordinary`.
Rows, details, accessibility values, logs, and history never expose treated
payload. Restoring the normative hidden-original inspection path requires a
separate core/FFI design; this slice preserves the hide/tombstone distinction and
records the existing gap.

Both treated rows have a `Review treatment` action. Its payload-redacted detail
shows treatment type, signed author/tag, timestamp, target entry under Technical
details, and the report’s signed action lineage. Lineage begins with direct
actions whose `targetEntryID == report.id`, then repeatedly includes retractions
whose targets are any already-included action IDs. This keeps retractions visible
without admitting unrelated global history.

Authorized editors retain `Editorial action`. New actions target the report ID;
`Retract` is offered beside a selected active editorial action and signs against
that action’s ID, never the report ID. Replies and all report/operational payload
remain absent.

### Posting contract

The composer’s `Review before posting` card is a live summary of current identity
and destination, not an immutable prepared request. The retained model receives
a `PublishingContextProviding` resolver from `RiotAppModel`.
`CommunityShellView` already observes that model; every published identity change
calls `composer.refreshPublishingContext()`, and the composer also resolves once
on presentation and again at Post. Destination is immutable for the keyed shell.
A display-name change therefore refreshes the visible review; a key/destination
mismatch invalidates posting rather than signing under stale copy.

The single Post tap resolves context again, validates current fields, constructs
one complete `PostUpdateRequest`, and passes that exact value to the publisher
once. The design makes no separate confirmation-step claim.

After a successful commit, the composer says: `Saved and signed on this device.
Exchange with someone nearby to share it.` The notification prompt, if needed,
occurs only after this success is visible and explains that notifications concern
newly received community updates. Denial never changes posting success.

`Done` dismisses without signing again. `Post another` clears headline, body, AI
choice, operational type, source claims, location, expiry, validation/write
errors, persisted draft fields, and posted state, then focuses Headline. It
cannot invoke signing.

The exact reset is `headline = ""`, `body = ""`, `aiAssisted = false`,
`mode = .freeform`, `sourceClaims = []`, `coarseLocation = ""`,
`expiresAt = nil`, `errorMessage = nil`, `status = .editing`, and an empty draft
store. `Done` is presentation-only: it neither writes nor clears a partially
edited draft.

Draft persistence is explicit:

| Event | persisted store | in-memory model | identity/signature |
| --- | --- | --- | --- |
| sheet dismissal / route change | all draft fields retained per community | unchanged | never in draft |
| app relaunch / community switch away and back | all fields restored, including type/expiry | rebuilt from that community’s store | never in draft |
| successful commit | cleared immediately | posted values retained behind success state | committed record only |
| `Done` after success | remains clear | posted success remains if reopened; no write/reset | unaffected |
| `Post another` | remains clear | exact reset to editing defaults | unaffected |
| explicit discard / community removal | cleared | discarded with community shell | unaffected |

`PostDraft` adds `mode` and an expiry Unix second. Decoding an older value defaults
to `.freeform` and `nil`; a test proves the old five-field JSON still restores.
This additive local preference shape is rollback-safe: an older binary ignores
the unknown JSON keys. It does not alter the Willow/core database.

Drafts never enter Nearby or the Newswire before signed commit. Plaintext
`UserDefaults` and backup exposure is an existing residual privacy risk; at-rest
hardening is not hidden inside this compact-UX slice.

### Tools, People, Nearby

Tool cards show name, purpose, trust/status badges, and one availability action.
Permissions and recommendation/share controls move under `More details` or the
existing review sheet. User-facing `app`, `space app`, and `space` strings in
`DirectoryView`, `AppReviewSheet`, and peer recommendation surfaces become
`tool` and `community`; protocol and type names stay unchanged.

People uses the exact anti-membership vocabulary `Known contributors`,
`No known contributors yet`, and `Known contributors appear here once people
post updates.` It uses Riot typography/chrome instead of a system-large title.
Empty `Post the first update` opens the shared composer. Each row summary has one
composed spoken label, while `Technical details for <name>` remains a separate
focusable disclosure. The full identifier is absent from the accessibility tree
and visible hierarchy until expansion, then remains selectable and untruncated.

Nearby retains automatic discovery but never auto-connects or auto-accepts. It
is limited to eligible public communal communities, advertises only its existing
randomized opaque transport identity before bilateral confirmation, preserves
namespace-bound core preview/admission, and commits nothing on rejection. `N` is
an offered item count, never a trusted or verified count.

The screen removes `Renderer: incident-board/1`, shortens repeated explanation,
uses `Nearby devices` and truthful `People you’ve synced with`, and labels
acceptance `Add N updates`.

| Nearby state | Meaning and action |
| --- | --- |
| looking, no peers | `Looking nearby…`; `Stop` |
| permission denied | fixed offline explanation + `Open Settings` |
| inbound request | named peer + `Accept` / `Decline`; metadata still withheld |
| peer selected | named peer profile + `Connect` / `Cancel` |
| connecting | named peer + progress; `Cancel` |
| community offered | peer and community named + `Join` / `Not now` |
| import preview | offered count + `Add N updates` / `Not now` |
| different community | explicit refusal; return to looking |
| failure / vanished peer | fixed retry copy + `Try again` |
| cancellation | tear down session/callbacks; return to looking |
| accepted | exact imported count when known; refresh after committed import |

## Error, privacy, and lifecycle rules

- No raw persistence, transport, or hostile payload error reaches display copy.
- Unsupported first-run Nearby adoption is absent, not cosmetically enabled.
- Name-save failure blocks every setup exit and never implies the draft was used.
- Treated post payloads never enter detail, accessibility, logs, or history.
- Nearby cancellation invalidates callbacks, stops the active session, and cannot
  permit a late import. A selected-community change still fails namespace-bound
  admission.
- Notification authorization is optional, contextual, and requested at most once
  through the existing notifier.
- Complete identifiers remain behind deliberate Technical details, not in Nearby
  advertisements, ordinary cards, accessibility labels, analytics, or error copy.

## Accessibility and responsive behavior

- Every new action has a stable identifier and 44×44-point target.
- At accessibility Dynamic Type sizes, Update/Alert/Request becomes a vertical
  radio-style choice; row actions and tool badges stack rather than clip or
  horizontally scroll.
- Essential small text uses ink/ink-soft rather than pink on cream. Status is
  never color-only.
- Sheets restore focus to the exact opening trigger. `Post another` focuses
  Headline. Name, validation, and write errors receive focus or announcement.
- Repeated controls have contextual labels: `Read <headline>` and `More details
  for <tool>`.
- Full IDs remain selectable and untruncated only under Technical details.
- Reduced-motion behavior and existing Riot theme tokens remain unchanged.

## Testable presentation seams

Small pure states live in already-registered source files:

- `OnboardingExit` plus one save-before-exit dispatcher covers `.join`,
  `.create`, and `.demo`;
- `ComposerPresentationState` records closed/open and the originating Home,
  empty-wire, or People trigger, allowing exact focus restoration;
- `HomePresentation.sections(wire:alerts:now:)` returns the ordered section list
  and its single composer placement, filtering active alerts with injected time;
- `PostSuccessCommand` maps `.done` to dismissal only and `.postAnother` to the
  field reset above.
- `CommunityTransitionGate` has tokened register/unregister and a synchronous
  prepare-before-mutation contract.
- `EditorialActionLineage` computes direct actions plus transitive retractions;
  `Retract` carries a selected editorial-action ID.

`ConferenceShellView` accepts an internal test-visible notifier factory, defaulting
to production, and passes the one community-shell notifier to the composer.
`PostUpdateView` exposes a success-presented async callback from its rendered
success branch; it yields one render turn before calling a small
`NotificationPermissionCoordinator`. Tests inject a scheduler/factory and prove
opening, reading, drafting, and failed writes do not request; first rendered
success requests only while undetermined; denial does not block; later success
does not re-prompt. The current community-open `.task` prompt is removed.

`ActiveAlertsPresentation.from(entries:activeNamespaceID:now:)` performs
namespace and expiry filtering, organizer/newest ordering, two-row capping, and
overflow count once. Home and the overflow sheet render those exact rows; neither
calls `Date()` or re-filters independently, so expiry cannot change between
composition and display.

## TDD contract

1. Onboarding tests prove unsupported first-run Nearby is absent, all three exits
   share save-before-exit gating, and a failed non-empty name save starts nothing.
2. Shell/Home tests prove the empty-wire and People actions open the retained
   composer and every wire state has no duplicate composer trigger. A keyed-shell
   switch test proves A’s model is torn down/persisted before B opens and cannot
   publish A’s draft into B. Gate tests cover chooser, join, deep link, create,
   archive, leave, and retry order plus stale-token unregister.
3. Newswire tests prove ordinary rows carry body/operational metadata, the
   excerpt/detail split, contextual labels, exact trust copy, and defensive
   treated-payload redaction. Treatment detail tests prove target-scoped history
   plus transitive retractions remain reachable without payload, and Retract signs
   the selected action ID rather than the report ID.
4. Composer tests prove posted → post-another clears every field without signing,
   while Done creates no second write.
5. Alerts tests prove empty and expired-only states omit Home alerts while an
   unexpired alert appears before the Newswire. Time is injected. Empty,
   foreign-only, expiry-at-now, expired-only, and active inputs are covered by the
   pure Home composition predicate. Two rows have no overflow; three rows cap at
   two and expose `View all 3 active alerts`, whose close restores focus.
6. Directory/People/Nearby tests pin compact vocabulary, disclosure placement,
   offered-count wording, exact `Known contributors` copy, and that Technical
   details is independently reachable while its ID is hidden before expansion.
7. Accessibility tests or inspection pin the large-text picker, stacked controls,
   contextual labels, focus transitions, announcements, and 44-point targets.
8. Focused suites run RED then GREEN. Shared macOS tests, iOS simulator build,
   iOS device compile, project-file lint, and repository coverage gates run
   before completion. At least one simulator interaction taps the actual
   accessibility identifiers for first run, composer entry, read detail, and
   post-another; source-order checks are not accepted as interaction evidence.

## Audited dead-end map

| Defect and current evidence | Expected transition | RED → GREEN evidence |
| --- | --- | --- |
| setup `Find one nearby` only changes an invisible route while launch state remains onboarding | unsupported action is absent; truthful after-join boundary remains | onboarding contract finds no first-run Nearby CTA and all visible exits act |
| `NewswireSurfaceView.onPostUpdate` defaults to `{}` and Home supplies none | empty-wire CTA opens the retained composer | required-handler compile/call-site tests plus simulator tap |
| `NewswirePostRow` drops projected body/provenance | ordinary row shows excerpt and Read opens complete detail | row mapping/detail tests plus simulator Read tap |
| posted composer renders status with no Done/Post another transition | Done dismisses; Post another resets/focuses without a write | command/model tests plus simulator tap |
| display name needs a separate Save and exit actions can imply unsaved identity | one fail-closed save-before-exit gate; no Save button | join/create/demo success/failure branch tests |

## Success and release checks

Baseline is the 2026-07-19 simulator audit: one invisible first-run action, one
empty-wire no-op, body-less reports, no repeat-post path, overweight Home, and a
notification prompt merely on community open.

This slice passes only when:

- Join, Create, and Demo reach Home in at most three primary choices after setup,
  while a failed name save reaches none;
- every audited visible CTA has a tested state transition (zero no-op actions);
- after opening one ordinary report, simulator review can identify its content,
  signed author, and that signing is not proof of truth;
- `Post another` returns to empty, focused Headline without a second write;
- Home has one filled composer action, active alerts above the wire, and no
  empty/expired-only alert card;
- four core routes have no clipped text or horizontal scrolling at an
  accessibility size and pass VoiceOver label/focus inspection.
- first-run setup order is name disclosure, Join with link/QR, Create, Demo, then
  the Nearby boundary; the community-name field appears only after Create.

Evaluation is immediate: automated checks and simulator/VoiceOver inspection
must pass before branch completion. The next TestFlight usability session should
repeat comprehension checks; any dead CTA, hidden ordinary body, truth claim, or
inaccessible/clipped core action fails release. The repo has no product-analytics
baseline, so the design does not invent conversion percentages.

## Definition of done

- The five audited dead ends are fixed by implementation or, for unsupported
  first-run Nearby adoption, removal with honest explanation.
- Home’s urgent/current information and readable feed precede secondary tools.
- Empty alerts and the embedded composer no longer consume Home.
- Current community is visible on every phone route.
- Tools, People, and Nearby remove audited clutter while preserving consent,
  recovery, provenance, and trust detail.
- Four-tab order, Front page/Open wire separation, per-community draft safety,
  key tags, and local-first wording remain intact.
- No dependency, schema, FFI surface, or core policy is introduced. No new shared
  Swift file is required.
- Android is explicitly reported as not audited or changed by this iOS/macOS
  slice.
