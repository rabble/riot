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
  profile-wide discovery, incomplete arrivals, and file import
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

Each row carries facts rather than deriving section placement in SwiftUI:

- whether the current community enables the tool;
- whether a complete verified pair is locally resolvable;
- whether the profile has already admitted the tool;
- the installed app, when present;
- whether this profile may publish the pair into the current community; and
- secondary recommendation/provenance/version/permission metadata.

The snapshot has:

1. **In `<community>`** — rows enabled by the selected community’s recognized
   organizer trust marker. These always sort first.
2. **Available to add** — complete, verified tools not enabled in the selected
   community. This includes already-admitted tools, built-in pairs, and complete
   tools carried from elsewhere.
3. **More tools** — results that are not locally actionable, including
   incomplete arrivals, plus profile-wide file import/discovery affordances.
   This section is visually secondary and omitted when empty.

The model must not label the third section “From your other communities.”
Current directory data exposes trust namespace IDs and carrier subspace IDs,
not a truthful mapping to named joined communities.

### Enabled but not locally admitted

Trust and local admission are independent. If a complete tool is enabled in the
selected community but has not been admitted by this profile, it remains in
“In `<community>`.” Its Open action lazily calls the existing verified
`getCarriedApp` admission path and then opens it. Admission grants no trust and
the runtime rechecks current trust before execution.

### Enabled but incomplete

An enabled tool whose verified bytes are incomplete remains in the selected
community’s first section with an honest “Still arriving” state. It cannot
offer a fake Open button. The acceptance criterion is therefore: every enabled
and locally complete tool has an immediately visible Open action.

## Primary actions

Primary CTA selection is a pure, testable presentation decision.

| Community state | Local state | Organizer | Primary presentation |
| --- | --- | --- | --- |
| Enabled | Installed and complete | Either | `Open Chat` |
| Enabled | Complete, not admitted | Either | `Open Chat` (verified lazy admission, then open) |
| Enabled | Incomplete | Either | `Still arriving` |
| Disabled | Complete and installed | Yes | `Add Chat to River City Wire` → permission sheet |
| Disabled | Complete, not admitted | Yes | `Add Chat to River City Wire` → verified admission → permission sheet |
| Disabled | Complete | No | `Ask an organizer to add Chat` |
| Disabled | Incomplete | Either | Secondary “More tools” arrival state |

“Ask an organizer to add Chat” is informational. Riot has no request-message
transport in this slice, so it must not imply that a request was sent.

The existing organizer trust sheet remains the security boundary. The sheet
receives the tool name and selected community title and uses named copy:

- title: `Add Chat to River City Wire?`;
- confirmation: `Add to River City Wire`;
- member explanation, if the sheet is reached defensively: `Only an organizer
  of River City Wire can add this tool.`

The sheet continues to list permissions before approval and continues to omit
the approval control for a non-organizer. Core authorization remains the final
enforcement layer.

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
- `Works offline` may be shown only when derived from a locally resolvable,
  verified pair. It is a user benefit, not a synonym for built-in provenance.
- The file picker becomes a secondary `Add a tool from a file` discovery action
  under More tools; it no longer competes with opening or enabling tools.

`canMakeAvailable` requires a selected community and a locally resolvable
verified pair. Merely having a selected community is insufficient.

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

## Empty, error, and transition behavior

- If the selected community has no enabled tools, show `No tools in River City
  Wire yet` in the first section while still rendering Available to add below.
- If no community is selected, show `Choose a community to see its tools` and
  expose no community mutation action.
- A failed refresh retains the last good snapshot and shows the existing error
  above it.
- Failed lazy admission or Add preparation leaves the row in place, preserves
  the CTA, and reports the existing plain-language arrival/error message.
- Successful approval refreshes the snapshot so the row moves from Available
  to add into In `<community>` and changes to Open.
- Switching communities recomputes all sections and CTA copy from the new
  community title; no prior title may remain onscreen.

## Trust persistence dependency

The current Apple repository persists trusted app IDs profile-wide and can
reapply them under a different active namespace after restart. This correction
does not paper over that behavior in presentation code.

WU-002c must make durable trust state namespace-scoped, or rely on durable
namespace-scoped markers without globally reissuing IDs, before the new Add
flow may ship. The presentation slice may be developed and verified in
isolation, but WU-002P remains merge/release-blocked on WU-001N and WU-002,
including WU-002c.

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

Expected test files:

- `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- `apps/ios/RiotTests/DirectoryRepositoryTests.swift`
- `apps/ios/RiotUITests/ChecklistFlowUITests.swift`
- `apps/ios/RiotUITests/RiversideMemberToolUITests.swift`
- `apps/ios/RiotUITests/RiotTabNavigationUITests.swift` only for reusable
  visual capture if needed

No new Swift file is required, so neither Xcode project file should need
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
- successful admission/approval moves a row between sections;
- community switching replaces every title-bearing string;
- built-in provenance produces no prominent badge;
- Make available / Recommend use the selected title; and
- no presentation vocabulary contains `this community`.

Repository/FFI tests retain the existing organizer/member enforcement and prove
that lazy admission does not grant trust. UI tests preserve stable Open
identifiers where possible and cover organizer Add → permission confirmation →
Open plus the member Ask state.

Native visual review records:

- the shared macOS surface at the app’s 480-point default width; and
- the iOS Tools flow at standard size and
  `accessibility-extra-extra-extra-large`, using kept XCUITest screenshot
  attachments and simulator content-size control.

Review checks section dominance, immediate Open/Add visibility, no clipped
named-community copy, no horizontal overflow, 44-point actions, correct
paper/ink poster aesthetic, and visually secondary management/discovery.

## Definition of Done

- The header and sections visibly name River City Wire.
- Enabled tools appear first.
- Every enabled, locally complete tool has an immediately visible Open action.
- Enabled but incomplete tools stay first with an honest arrival state.
- Available disabled tools use `Add … to River City Wire`; permission approval
  remains behind that action.
- Members see `Ask an organizer to add …` and cannot mutate trust.
- Profile-wide discovery is separate and secondary.
- `Built in` is not a prominent badge.
- No generic `this community` copy remains on this flow.
- Publishing and recommendation use explicit, named verbs.
- Trust, permission, admission, and runtime checks remain intact.
- Tests prove grouping, CTA selection, named copy, non-organizer behavior, and
  transitions.
- macOS-width and large-Dynamic-Type visual reviews pass in Riot’s established
  marketing/native aesthetic.
- WU-002P’s master plan and source spec put this hierarchy before cosmetic
  Legacy/v2 labels.
- WU-002c’s namespace-scoped durable trust dependency is satisfied before
  merge/release.
