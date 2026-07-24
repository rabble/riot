# Community-scoped Tools design

**Status:** Approved by Rabble on 2026-07-24  
**Applies to:** WU-002P, beginning with the shared iOS/macOS Tools surface  
**Source of truth:** This document supersedes any WU-002P presentation rule
that places tool generation or provenance ahead of selected-community status.

## Problem

The Tools route is shown inside a selected community, but the current model
renders one profile-wide list. It maps every `directoryListings()` result,
appends every locally held app, and uses the selected community only when
calculating trust and open state. The view then labels the whole screen “From
your communities,” promotes “Built in,” and presents Review, Get,
recommendation, and sharing as competing goals.

That information architecture hides the reason a person opened Tools: use a
tool in the community they selected.

## Product decision

The selected community is the scope of the screen.

For River City Wire, the visible hierarchy is:

```text
RIVER CITY WIRE
Tools

In River City Wire
  Chat                         Open Chat
  Checklist                    Open Checklist

Available to add
  Supply Board                 Add Supply Board to River City Wire

More tools
  profile-wide discovery and organizer file import
```

Community status always outranks generation, provenance, recommendations,
version, and permissions. An enabled Legacy 1 tool stays in “In River City
Wire” with an immediately visible Open action; it is never buried below a
disabled v2 tool merely because it is legacy.

## Scope and non-goals

This correction covers:

- selected-community grouping and ordering;
- goal-oriented Open/Add/member copy;
- named-community confirmation and management copy;
- removal of prominent built-in provenance;
- preservation of organizer trust and runtime gates;
- shared iOS/macOS implementation and native visual verification;
- explicit Android parity in WU-002P before WU-002P is complete; and
- ordering the correction before WU-002P’s Legacy/v2 and quota presentation.

This correction does not:

- change manifest, bundle, signature, Willow, or runtime containment rules;
- invent a request-to-organizer message transport;
- claim the source community of a tool when only opaque namespace IDs exist;
- add a global marketplace to the selected-community screen;
- redesign Home, People, Nearby, or the shell; or
- weaken incomplete-byte, trust, or organizer checks to manufacture an action.

## Presentation model

`RiotDirectoryModel` remains the pure, no-FFI presentation seam. It will expose
a selected-community snapshot while retaining a flat all-rows accessor for
`PeerProfileView`.

The existing `rows` property remains that flat accessor with its current local
`Availability` semantics, including the `.review`/`.arriving` behavior consumed
by `PeerProfileView`. The namespace-keyed snapshot partitions or references
those same rows and adds primary CTA intent; it does not silently redefine the
peer collection filter or require a `PeerProfileView.swift` change.

Each row carries facts rather than deriving section placement in SwiftUI:

- whether the current community enables the tool;
- whether a complete verified pair is locally resolvable;
- whether the profile has already admitted the tool;
- the installed app, when present;
- approval capability as organizer, ordinary member with an organizer to ask,
  or organizerless legacy profile;
- whether this profile may publish the pair into the current community; and
- secondary recommendation/provenance/version/permission metadata.

The snapshot has:

1. **In `<community>`** — rows enabled by the selected community’s recognized
   organizer trust marker. These always sort first.
2. **Available to add** — complete, verified tools not enabled in the selected
   community. This includes already-admitted tools, built-in pairs, and complete
   tools carried from elsewhere.
3. **More tools** — profile-wide discovery and the organizer-only file-import
   affordance. This section is visually secondary and omitted when empty.

The model must not label the third section “From your other communities.”
Current directory data exposes trust namespace IDs and carrier subspace IDs,
not a truthful mapping to named joined communities.

The snapshot is one atomic value keyed by the selected namespace ID. Its
community title supplies the header, section labels, CTA labels, confirmations,
errors, and receipts. Rows and title are never combined across snapshots.

The existing Rust projection exposes only verified manifest/bundle pairs:
`scan_app_index` retains manifest-only records privately as
`pending_manifests`, while `directory_listings` intentionally omits them. This
slice therefore does not invent an incomplete `AppListing` or display
unverified manifest names. A future typed pending-arrival projection may add a
generic arrival treatment, but it must remain unable to participate in trust,
supersession, or launch decisions.

### Enabled but not locally admitted

Trust and local admission are independent. If a complete tool is enabled in the
selected community but has not been admitted by this profile, it remains in
“In `<community>`.” Its Open action lazily calls the existing verified
`getCarriedApp` admission path. The model returns the admitted `RiotSpaceApp`
directly to `onOpen`; orchestration does not leak into SwiftUI.

Lazy Open is fail-closed in this order: resolve and verify the exact pair,
persist/admit it without granting trust, revalidate the snapshot namespace
against the currently selected namespace, re-read trust for that namespace,
then obtain a fresh core `AppExecutionSession`. A community switch, revoke, or
generation change at any point leaves the row in place and does not launch.

WU-002c must also make active-namespace directory enablement agree with
`is_app_trusted` after A → B → A community switching. Clearing the in-memory
trust-marker cache during a switch must not make a trusted, complete,
not-yet-admitted tool appear disabled. A real Rust/FFI/repository integration
test covers that case before this presentation ships.

## Primary actions

Primary CTA selection is a pure, testable presentation decision.

| Community state | Local state | Organizer | Primary presentation |
| --- | --- | --- | --- |
| Enabled | Installed and complete | Either | `Open Chat` |
| Enabled | Complete, not admitted | Either | `Open Chat` (verified lazy admission, then open) |
| Disabled | Complete and installed | Yes | `Add Chat to River City Wire` → permission sheet |
| Disabled | Complete, not admitted | Yes | `Add Chat to River City Wire` → verified admission → permission sheet |
| Disabled | Complete | No | `Ask an organizer to add Chat` |

“Ask an organizer to add Chat” is static informational text, styled and exposed
to VoiceOver as a status rather than a button. Riot has no request-message
transport in this slice, so it must not look tappable or imply that a request
was sent. An organizerless legacy profile instead says: `This profile can’t add
tools to River City Wire. Start a new profile to organize a community.`

The existing organizer trust sheet remains the security boundary. The sheet
receives the tool name and selected community title and uses named copy:

- title: `Add Chat to River City Wire?`;
- confirmation: `Add to River City Wire`;
- member explanation, if the sheet is reached defensively: `Only an organizer
  of River City Wire can add this tool.`

The sheet continues to list permissions before approval and continues to omit
the approval control for a non-organizer. Core authorization remains the final
enforcement layer.

Each pending Add operation captures the full app ID, expected namespace ID, and
community title from the same presentation snapshot. Admission may happen
before the sheet appears, but never grants trust. Confirmation enters
`Adding…`, disables duplicate taps, and calls a result-bearing `AppModel`
operation. The sheet dismisses only after durable trust succeeds, announces
`Added Chat to River City Wire`, refreshes the scoped snapshot, and restores
focus to the moved tool’s `Open Chat` action.

If admission fails, the row and Add CTA remain. If approval or trust persistence
fails, the sheet stays open, returns to an enabled confirmation control, and
says: `Couldn’t add Chat to River City Wire. Nothing changed. Try again.`
The failure is announced to VoiceOver and focus remains on, or returns to, that
re-enabled `Add to River City Wire` confirmation control.

If the selected namespace changes while admission, confirmation, or approval is
pending, Riot cancels or dismisses the old operation. Immediately before every
mutation it compares `expectedNamespaceID` with the current selected namespace
and refuses a mismatch; it never retargets River City Wire copy or intent to the
new community.

## Secondary actions and terminology

Permissions, provenance, version, recommendation, retraction, and publishing
remain in More details or an overflow menu.

- Publishing the verified manifest and bundle into the selected community is
  `Make available in River City Wire`. It does not enable the tool.
- Endorsement is `Recommend to River City Wire`.
- Retraction remains secondary and names the recommendation being withdrawn.
- No user-facing string says “this community” when the title is known.
- `Built in` is removed as a badge and as section membership. Provenance may be
  shown inside details/confirmation only if it materially supports the trust
  decision.
- The redundant `On in this space` badge is also removed; first-section
  placement already communicates enablement.
- `Works offline` may be shown only when derived from a locally resolvable,
  verified pair. It is a user benefit, not a synonym for built-in provenance.
- For this slice, `Add a tool from a file` remains organizer-only and secondary
  under More tools. Import verifies and admits bytes profile-wide but grants no
  trust. After success it opens the named Add permission sheet using permissions
  from the admitted verified manifest; members do not see the import control.

`canMakeAvailable` requires a selected community and a locally resolvable
verified pair. Merely having a selected community is insufficient. Current
directory data cannot prove that an identical pair was already published into
the namespace, so the secondary action is idempotent. Its detail copy states
that publishing does not add or enable the tool.

Add, Make available, Recommend, and retraction all capture and revalidate the
snapshot’s full app ID and expected namespace ID at the mutation boundary. A
community switch invalidates their pending UI instead of applying it to the
newly selected community. One immutable operation-context value carries those
three fields for every named mutation so individual paths cannot drift. Exact
success receipts are:

- `Added Chat to River City Wire`
- `Made Chat available in River City Wire`
- `Recommended Chat to River City Wire`

## Visual direction

The Tools hierarchy must keep the visual language of Riot’s marketing site and
the existing native design system, not fall back to a generic settings list or
app-store catalog.

Reuse the existing native equivalents of the marketing system:

- paper/ink surfaces from `RiotTheme.paper` and `paper2`;
- Anton poster headings, Work Sans body copy, and Space Mono labels/actions;
- the blue/pink offset-shadow `RiotHeader`;
- hard two-point ink borders from `RiotCard`, without gradients or soft
  marketplace-style chrome;
- uppercase pink monospace eyebrows and restrained stamped status accents; and
- black primary / outlined secondary Riot buttons with 44-point minimum targets.

The community eyebrow and Tools poster title remain visually dominant. Section
headings use the poster/mono hierarchy. “More tools” is separated by strong
spacing or a hard rule and has lower visual weight. Dynamic Type may stack
buttons and metadata, but it must not collapse the section order, clip copy, or
hide Open/Add.

At regular width, each card reads in this order: tool identity, short
description, immediately visible primary action, then optional metadata/details.
At the 480-point macOS default and accessibility text sizes, the action becomes
a full-width row directly below identity/description; metadata follows rather
than displacing it. Long tool/community names wrap without truncating the named
action. Full app IDs, not names or generation labels, form row/action
accessibility identifiers; human-readable labels remain actions such as `Open
Chat`. Section titles have heading traits, and VoiceOver order is header →
In-community tools → Available to add → More tools.

## Empty, error, and transition behavior

- If the selected community has no enabled tools, show `No tools in River City
  Wire yet` as a compact inline first-section message while still rendering
  Available to add immediately below. Do not use the full-page
  `RiotEmptyState` here.
- If no community is selected, show `Choose a community to see its tools` and
  expose no community mutation action.
- A same-namespace refresh failure retains that namespace’s last good snapshot
  and shows a scoped error above it. After a community switch, a failed refresh
  never retains the prior namespace’s rows: it shows no empty-state claim and
  instead says `Couldn’t load tools for River City Wire.` with `Try again`.
- Failed lazy admission or Add preparation leaves the row in place, preserves
  the CTA, and reports a named, plain-language error.
- Successful approval refreshes the snapshot so the row moves from Available
  to add into In `<community>` and changes to Open.
- Switching communities invalidates pending actions, clears cross-namespace
  rows, and recomputes all sections and CTA copy from the new snapshot; no prior
  title or operation may remain onscreen.

## Trust persistence dependency

The current Apple repository persists trusted app IDs profile-wide and can
reapply them under a different active namespace after restart. This correction
does not paper over that behavior in presentation code.

WU-002c must key durable trust by the exact `(namespaceID, fullAppID)` pair and
restore it only while that namespace is active and core still validates
organizer authority. It may instead rely on durable signed namespace-scoped
markers, but it must never globally reissue app IDs into the active namespace.

Legacy profile-wide `trustedAppIDs` have unknowable community provenance. Their
migration is fail-closed: do not copy them into every namespace and do not
assign them to whichever namespace happens to be selected. Unless exact signed
namespace-scoped evidence recovers the original grant, leave the tool disabled
and require one-time explicit organizer re-approval. Upgrade/restart tests use
the same app ID in two communities and prove no trust crosses namespaces.

The presentation slice may be developed and verified in isolation, but WU-002P
remains merge/release-blocked on WU-001N and WU-002, including these WU-002c
restore, switching, and migration guarantees.

## WU-002P expansion and order

WU-002P now executes in this order:

1. **WU-002P-A — Community-scoped hierarchy (Apple):** pure section/CTA model,
   shared iOS/macOS view, named review copy, tests, and native visual evidence.
2. **WU-002P-B — Android parity:** the same selected-community hierarchy and
   CTA vocabulary in Compose, with equivalent controller tests.
3. **WU-002P-C — Existing-user generation/quota presentation:** Legacy 1 /
   Redesigned Version 2 metadata, separate-version warnings, and distinct
   count-full/storage-full errors.

Generation may refine card metadata within a community-status section, but may
never reorder the screen ahead of selected-community status or hide an enabled
tool’s Open action.

## Apple implementation scope

Expected production files:

- `apps/ios/Riot/Directory/DirectoryModel.swift`
- `apps/ios/Riot/Directory/DirectoryView.swift`
- `apps/ios/Riot/Apps/AppReviewSheet.swift`
- `apps/ios/Riot/AppModel.swift` for result-bearing approval and
  namespace-bound operation seams

Expected test files:

- `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- `apps/ios/RiotTests/DirectoryRepositoryTests.swift`
- `apps/ios/RiotUITests/ChecklistFlowUITests.swift`
- `apps/ios/RiotUITests/RiversideMemberToolUITests.swift`
- `apps/ios/RiotUITests/RiotTabNavigationUITests.swift` only for reusable
  visual capture if needed

Result-bearing `AppModel` coverage is added to an existing compiled test file,
so no new Swift file is required and neither Xcode project file should need
editing. `DirectoryStorefrontTests.swift` is already compiled by both the iOS
and macOS test targets.

## Test and review contract

Tests must be written first and observed failing before implementation.

Pure presentation tests prove:

- current-space trust determines the first section;
- enabled tools precede available and discovery rows regardless of input order,
  built-in provenance, recommendation count, or generation;
- every enabled and complete state selects Open;
- available organizer states select exact named Add copy;
- the non-organizer state selects exact Ask-an-organizer copy and no mutation;
- the organizerless legacy state selects its exact remediation and no mutation;
- successful admission/approval moves a row between sections;
- community switching replaces every title-bearing string and invalidates stale
  Add, Make available, Recommend, and retraction actions;
- built-in provenance produces no prominent badge;
- Make available / Recommend use the selected title; and
- no presentation vocabulary contains `this community`.

Repository/FFI tests retain the existing organizer/member enforcement and prove
that lazy admission does not grant trust, active-namespace listings agree with
`is_app_trusted` after A → B → A switching, and the same app ID cannot inherit
trust between communities across upgrade/restart. UI tests preserve stable Open
identifiers where possible and cover organizer Add → permission confirmation →
Open, approval failure/retry, stale-sheet cancellation, and the member Ask
state.

Native visual review records:

- the shared macOS surface at the app’s 480-point default width; and
- the iOS Tools flow at standard size and
  `accessibility-extra-extra-extra-large`, using kept XCUITest screenshot
  attachments and simulator content-size control.

Review checks section dominance, immediate Open/Add visibility, no clipped
named-community copy, no horizontal overflow, 44-point actions, correct
paper/ink poster aesthetic, and visually secondary management/discovery.
The visual matrix also includes long tool/community names and localized text
expansion. A moderated outcome check confirms that a participant can identify
and open an enabled tool on the first attempt and can distinguish Open, Add,
and Ask without prompting. Prompting, opening management/details before the
primary action, confusing profile availability with community enablement, or
treating Ask as a sent request are recorded as failures.

## Definition of Done

- The header and sections visibly name River City Wire.
- Enabled tools appear first.
- Every surfaced enabled tool has an immediately visible Open action; only
  verified complete pairs enter this production projection.
- Available disabled tools use `Add … to River City Wire`; permission approval
  remains behind that action.
- Members see `Ask an organizer to add …` and cannot mutate trust.
- Profile-wide discovery is separate and secondary.
- `Built in` is not a prominent badge.
- No generic `this community` copy remains on this flow.
- Publishing and recommendation use explicit, named verbs.
- Trust, permission, admission, and runtime checks remain intact.
- Tests prove grouping, CTA selection, named copy, non-organizer behavior, and
  namespace-bound transitions.
- macOS-width and large-Dynamic-Type visual reviews pass in Riot’s established
  marketing/native aesthetic.
- WU-002P’s master plan and source spec put this hierarchy before cosmetic
  Legacy/v2 labels.
- WU-002c’s namespace-scoped durable trust dependency is satisfied before
  merge/release.
