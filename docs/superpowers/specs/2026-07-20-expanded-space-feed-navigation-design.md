# Expanded-Space, Feed-First Navigation Design

**Date:** 2026-07-20
**Status:** User-approved design; design-review gate revision 2
**Scope:** Shared SwiftUI iPhone and macOS shell. No Rust, FFI, database-format, Android, or network-protocol changes.

## 1. Problem

The macOS shell currently uses its sidebar for `Home`, `Tools`, `People`, and
`Nearby`, while the communities a person belongs to are hidden in a modal
chooser. That hierarchy is inverted: communities and followed spaces are the
durable places a person moves between; routes and apps only make sense inside
one selected place.

The current Home screen also gives feed content and app shortcuts comparable
visual weight. This obscures Riot's primary community task: understand what is
happening, then contribute.

The requested hierarchy is:

1. spaces and communities in the sidebar;
2. the active community expands in place;
3. Feed is the default and primary community surface;
4. People, Content, and Apps are subordinate links inside that community; and
5. synchronization is ambient infrastructure, not content navigation.

This design supersedes the macOS route-sidebar portion of
`2026-07-13-community-first-navigation-design.md`. It realizes the selected-row
expansion variant of `2026-07-18-spaces-first-navigation-design.md`, rather than
that document's separate detail navigation.

## 2. Users and outcomes

1. **A person in several communities** wants to see and switch among their
   spaces directly, so that changing context does not require opening a modal,
   when they use Riot on macOS.
2. **A community participant** wants the active community's latest signed
   reports to dominate the screen, so that they immediately understand what is
   happening, when they select or reopen a community.
3. **A participant looking for people, older material, or tools** wants those
   destinations visibly nested under the active community, so that their scope
   is unambiguous, when they leave the Feed.
4. **An organizer or editor** wants management actions to appear in the same
   Content library available to every openable author-bearing community
   context, so that authority adds capability without creating a separate admin
   product, when core-derived authority permits the action.
5. **A person exchanging data nearby** wants healthy exchange to stay ambient,
   so that community content remains the focus while new peers and imports still
   receive explicit review, when another Riot device is in range.
6. **A person whose space cannot open** wants its row to remain visible with a
   recovery path, so that local corruption or incomplete sync never turns into
   lost-looking data or a blank pane.

## 3. Product decisions

### 3.1 Selected approach: expand the active space

The user selected the expandable-sidebar approach from three alternatives.
Spaces remain the first-level navigation. Only the active space expands to
show its supported local destinations.

```text
YOUR SPACE                     ← conditional; empty in this slice
  Rabble

COMMUNITIES
  Riverside Tenants Union       ← selected
    Feed                        ← selected by default
    People
    Content
    Apps
  Mutual Aid Network
  Community Kitchen

FOLLOWING
  City Wire

  Add or create a space
  Your profile
  Quiet sync status
```

This approach is more compact than a three-column workspace and keeps the Feed
wider. It is more contextual than a global section bar because the child
destinations are visibly owned by the selected community.

### 3.2 Canonical community destinations

The canonical order is:

1. **Feed**
2. **People**
3. **Content**
4. **Apps**

Selecting or restoring a community always opens Feed unless a valid deep link
names a more specific held record. Switching communities never preserves a
destination that the new space does not support.

### 3.3 Space kinds remain truthful

The sidebar groups existing core-derived relationships into:

- **Communities** — organizer, member, and public-reader rows; and
- **Following** — author-less followed-site rows.

An available, author-bearing community exposes Feed, People, Content, and Apps.
A public-reader row remains visible but unavailable in this shell slice:
current core selection requires a held author and exposes alerts only for the
active namespace. Its row says that read-only opening is not available on this
device yet; the shell does not pretend that it can browse.

A followed site is a terminal status/refresh row in this slice. Selecting it
opens locally held metadata, transport state, last refresh result, and its
verified refresh action when available. It has no fabricated Feed, Content,
People, or Apps child. A later lower-layer slice may make it a read surface
after the signed manifest needed by `resolve_composite_site` is durably
available through the Swift repository boundary.

The last refresh result is session-local unless a separate persistence design
lands later; the row never implies that transient feedback survives relaunch.

`CommunityRelationship.personal` exists in the generated contract, but
production code creates no Personal registry record. The **Your space** section
is therefore omitted when empty and is not an acceptance requirement for this
slice. Creating and selecting a personal home is deferred lower-layer work.

## 4. Platform behavior

### 4.1 macOS

The `NavigationSplitView` sidebar becomes the space tree. Selecting a community
runs the existing community transition gate, selects that row, expands it, and
opens Feed. Selecting one of the expanded children changes only the active
community destination.

Only one available author-bearing community is expanded at a time. Collapsed
rows may show an unread or attention badge, but badges never change selection.
The footer's **Add or create a space** menu has four explicit branches: Create
community, Join with link or QR, Follow a site, and Find nearby. **Your
profile** is a separate global action.

Every row has one anatomy: disclosure affordance when it has children, primary
name, secondary relationship/state text, optional unread/attention badge, and
an inline Retry only when unavailable. Rows and child actions have at least a
44-point target. Long names wrap to two lines before truncating; they never
push the badge or recovery action out of reach.

Community settings remain contextual to the selected community. They appear as
a secondary toolbar/header action rather than a peer destination.

### 4.2 iPhone

The phone cannot use a persistent sidebar, so it keeps the community selector
in the header. The bottom destinations align with the same product model:
Feed, People, Content, and Apps. Selecting a different community defaults to
Feed.

The iPhone chooser exposes the same four add-space branches as macOS: Create
community, Join with link or QR, Follow a site, and Find nearby.

This keeps route names and scope consistent across the shared SwiftUI shell
without forcing desktop chrome onto a compact device.

### 4.3 Keyboard and accessibility

- Command-K focuses the selected row in the space `List`; if nothing is
  selected, it focuses the first actionable row.
- Command-1 through Command-4 select Feed, People, Content, and Apps.
- Arrow-key navigation and disclosure behavior come from a semantic
  `List(selection:)` with `DisclosureGroup` community rows, not the current
  custom `VStack` of buttons. Custom row backgrounds may preserve Riot's visual
  selection treatment without replacing native outline/focus semantics.
- Expansion, selection, unread state, and availability use text, icon/shape,
  and accessibility state; color is never the only signal.
- A row announces tier, name, relationship where relevant, availability, and
  unread state without exposing or truncating a namespace ID.
- The selected space and selected child each expose the correct selected and
  expanded accessibility traits.
- Dynamic Type and VoiceOver must not hide the child destinations or global
  profile action.
- At accessibility Dynamic Type sizes, the existing iPhone tab bar keeps its
  two-by-two grid behavior.

## 5. Destination contracts

### 5.1 Feed

Feed replaces Home as the primary destination. It contains:

1. bounded active alerts when present;
2. one context-appropriate Post an update action when the current relationship
   can post;
3. the community newswire's Front page, Open wire, and editorial history; and
4. honest empty, pending-first-sync, and degraded states.

Feed does not contain app shortcuts. Apps no longer compete with the community
feed for primary visual hierarchy.

### 5.2 People

People uses the existing known-contributors projection. It does not claim to be
a complete membership or presence directory.

Visible copy may use the short navigation label **People**, but the screen and
empty states must say **Known contributors** where a membership claim could
otherwise be inferred. Real members, roles, and roster semantics remain
deferred until the core can prove them.

### 5.3 Content

Content is a browse-and-manage library over records already held and verified
by the current core. The first version contains:

- **Updates** — locally held newswire material, including material that has
  fallen out of the immediate Feed;
- **Alerts** — locally held community alerts, separated from ordinary updates;
  and
- existing signed editorial/treatment history reachable from the relevant
  record.

The design does not invent generic files, pages, or media records that the core
does not model.

Capabilities are additive and authority-derived for an author-bearing active
community:

- members with a current author may publish an update;
- recognized editors receive existing editorial actions;
- organizers may open Community settings from Content/header context; app
  approval stays in Apps rather than being duplicated in Content; and
- no caller-supplied Boolean or stale Swift cache grants authority.

Unauthorized controls are omitted, not disabled as a teasing dead end. Every
write is revalidated by the existing Rust-owned session/authority boundary at
commit time.

### 5.4 Apps

Apps contains the existing community tool directory, review flow, and runtime.
Opening a tool remains scoped to the active community. App approval and
execution authority remain core-derived.

The current deterministic Home shortcuts move out of Feed. No app package,
approval, or execution contract changes in this work.

## 6. Ambient synchronization

Synchronization is a community-scoped shell service, not a destination. The
existing coordinator remains alive above Feed, People, Content, and Apps so a
route change does not tear down a valid session.

Healthy exchange is visually quiet. At most, the shell shows a compact status
near the sidebar footer or community settings. It does not show a Feed card,
primary button, or permanent route.

This is a presentation decision, not a claim that the process runs while the
operating system has suspended Riot.

Existing consent and fail-closed behavior remain mandatory:

- discovery never silently accepts a new peer;
- peer confirmation remains explicit;
- offered content is previewed before acceptance;
- an import can commit only to the community whose session produced it; and
- switching communities prepares and tears down the old community scope before
  the new one becomes active.

The community shell owns a `NearbyPresentationState` adapter over
`NearbyConnectionState`. When an available community becomes active it starts
discovery automatically, matching the current `ConnectionStatusView.onAppear`
behavior. It also installs `onSpaceJoined`, reannouncement, and teardown
callbacks before discovery begins.

Automatic discovery is gated by a device-local
`NearbyDiscoveryPreference`. It defaults to enabled to preserve current
behavior. An explicit Stop sets the preference to paused; paused survives route
changes, community switches, keyed-shell reconstruction, and app relaunch
until the person explicitly chooses Restart. The preference contains no
community or peer identity.

Every active transport scope receives a typed
`NearbyScopeToken(generation, communityID)`. The owner above the keyed
`CommunityShellView` increments and invalidates the generation before stopping
or switching. BLE discovery, local-network discovery, route selection, pairing
decisions, coordinator state, space adoption, preview, import acceptance, and
data-changed callbacks capture the token and require equality before reading
`currentSpace` or changing UI/store state. A mismatch cancels/disconnects that
work without disclosing metadata. Teardown order is: invalidate token, clear
callbacks, stop/disconnect transport, unregister the transition token, then
construct the new scope.

State maps to presentation exactly:

- `idle`, `looking`, `connecting`, `gettingLatest`, `caughtUp`,
  `alreadyCurrent`, `differentSpace`, `nothingToShare`, and `outOfRange` remain
  quiet status; `nothingToShare` while an active community exists reannounces
  that community exactly as the current route does;
- discovered phones make the footer status actionable (`N nearby`); opening it
  shows the existing peer list and Stop/Start controls;
- inbound `.confirm` presents one peer-confirmation prompt;
- a proposed space join presents one join-confirmation prompt;
- `.preview` presents the exact offered count with Add and Not now;
- permission denial and `.failed` mark the footer with attention and open the
  existing Settings/Retry recovery; and
- stopped or dismissed work stays inert until the person restarts discovery.

While paused, the footer says **Nearby exchange paused** and offers the
secondary **Restart** action in its popover/settings detail. It never promotes
Restart into Feed or primary navigation.

The adapter serializes attention from the controller's single state: only one
sheet/popover is active, and a newer state replaces or dismisses an obsolete
one. macOS uses a popover for inspection/status and a sheet or confirmation
dialog for consent; iPhone uses sheets/confirmation dialogs. These
presentations do not replace the Feed.

Bilateral confirmation completes before either namespace metadata or sync
frames are disclosed. Generation checks supplement rather than replace
`MutualConfirmationGate` and `NearbyImportAdmission`.

Feed recovery actions that previously navigated to Nearby instead open the
same contextual sync/recovery presentation.

## 7. Architecture and data flow

### 7.1 Pure sidebar projection

Add a pure, host-testable sidebar projection that merges:

- raw core `CommunityRow` values, preserving typed
  `CommunityRelationship` rather than classifying display strings; and
- `RiotAppModel.followedSites` / core `FollowedSiteRow` values.

The projection receives unread counts as a separate input because registry rows
do not carry them. It produces tiered rows, row state, supported children,
unread badge, selected/expanded state, and accessibility text. It does not
infer relationships, authority, or transport policy.

### 7.2 Navigation state

Replace the current primary `RiotDestination` values with Feed, People,
Content, and Apps. Keep route selection separate from the broad
`RiotAppModel.objectWillChange` stream, preserving the existing performance
contract.

Use typed identities and separate navigation from repository state:

```swift
enum SidebarItemID: Hashable, Sendable {
    case community(namespaceID: String)
    case communityDestination(namespaceID: String, destination: RiotDestination)
    case followedSite(root: String)
}

enum SpaceSelectionState: Equatable, Sendable {
    case none
    case activeCommunity(namespaceID: String, destination: RiotDestination)
    case followedSite(root: String)
    case attemptingCommunity(target: String, previous: String?)
    case failedCommunity(target: String, previous: String?, code: String)
}
```

`coreActiveCommunity`, sidebar selection, and a failed target are deliberately
distinct. The macOS spaces shell sits above the keyed community detail so a
failed target cannot replace the entire sidebar.

Space selection and local-destination selection flow as follows:

```text
core registry/following lists
  → RiotAppModel reload
  → pure sidebar projection
  → select space
  → SpaceSelectionState.attemptingCommunity
  → CommunityTransitionGate.prepare
  → repository switch/open
  → success: keyed CommunityShellView + Feed default
  → thrown error: reconcile actual active namespace
      → target active: degraded success + persistence recovery
      → previous active: target row shows Retry
      → other/unknown active: fail-closed mismatch recovery
  → select child destination without switching core context
```

Community activation never equates a thrown error with "the old context is
still active." The repository reconciles every outcome against
`activeCommunity()` after the mutation attempt:

```swift
enum CommunityActivationOutcome: Equatable, Sendable {
    case activated(namespaceID: String)
    case activatedPersistenceDegraded(namespaceID: String, code: String)
    case targetUnavailable(target: String, previous: String?, code: String)
    case activeNamespaceMismatch(expected: String, actual: String?)
}
```

If Rust activated the target but the Swift snapshot failed to persist, the
shell follows the reconciled target and shows a persistence-recovery warning;
it never continues drawing the old community over the new core context. If the
old namespace remains active, sidebar/detail selection returns to it while the
failed target row retains inline attention and Retry. An unexpected third or
unknown namespace is a fail-closed mismatch: stale detail is removed and
recovery names no content as active. When no previous community exists, the
sidebar remains and the detail pane shows target-scoped recovery. Launch-level
recovery is reserved for the case where there is no usable active context.

Selecting a followed-site row never calls `switchToCommunity`. It selects the
terminal followed-site status/refresh detail. On iPhone that detail is pushed
from the chooser and Back returns to the previously active community shell.

Within one community, route views keep their current retained ZStack lifecycle.
Switching communities rebuilds the keyed shell, resets the local destination
to Feed, and does not promise to retain scroll position or navigation stacks;
community-keyed drafts remain preserved.

Accepted store changes refresh the active Feed, Content projection, known
contributors, unread counts, and relevant row badges through the existing local
data-changed signal.

`reload()` refreshes both raw community rows and followed-site rows.
`refreshFromStore()` does the same, so bootstrap, sync import, follow/refresh,
and community changes cannot leave either sidebar section stale.
The two reads form one `SidebarSnapshot`, but each section preserves its
last-known rows if the other metadata call transiently fails; one failed list
call cannot make unrelated retained rows appear to disappear.

Deep-link activation consumes `CommunityActivationOutcome`. Riot resolves or
presents a held record only for `.activated` or
`.activatedPersistenceDegraded` after a second active-namespace equality check.
Unavailable or mismatch outcomes emit no verified/detail result.

### 7.3 View reuse

The change recomposes existing views:

- Feed reuses active-alert and newswire surfaces;
- People reuses `PeopleView`;
- Apps reuses `DirectoryView` and `AppRuntimeView`; and
- Content adapts existing locally held newswire/alert projections into an
  archive-oriented list with current authority-gated actions.

`CommunityContentModel` is a host-testable projection with:

```swift
struct CommunityContentCapabilities {
    let canPost: Bool
    let canEditorialAct: Bool
    let canOpenCommunitySettings: Bool
}

struct CommunityContentAlertRow {
    let id: String
    let headline: String
    let summary: String
    let createdAt: Date
    let expiresAt: Date
    let isActive: Bool
}

enum CommunityContentState {
    case loading
    case loaded(updates: [NewswirePostRow],
                alerts: [CommunityContentAlertRow],
                capabilities: CommunityContentCapabilities)
    case failed(code: String)
}
```

The default Content view is one scroll surface with **Updates** first and
**Alerts** second. Updates are de-duplicated by full entry ID and ordered by
signed creation time descending across Front page, Open wire, and Earlier.
Alerts show active first, then expired, each newest first. Each section has its
own empty copy; if both are empty the screen shows one library-level empty
state. Record detail opens from the row. Editorial/treatment history and
authorized editorial actions live in update detail, not as a third top-level
collection.

Content is built only from current treatment-aware core projections. It never
reconstructs raw payloads: hidden records remain interstitials, tombstoned
records expose no payload, correction plaintext remains redacted where the
projection redacts it, and future-quarantined entries never enter the archive.
Post, editorial, and tool callbacks retain their expected full community
identity or execution-session scope and recheck it at commit; hiding a stale
control is not treated as authorization or cancellation by itself.

The current modal `CommunityChooserView` remains useful on iPhone and as a
fallback/add-space flow. It is no longer macOS's primary way to switch among
held spaces.

### 7.4 No lower-layer migration

The necessary relationship variants, community rows, followed-site rows, and
list methods already exist. This design adds no protocol event, database
migration, FFI field, secret handling, or internet fallback.

## 8. Recovery and edge cases

- **No spaces:** show the existing create/join/demo launch path. The sidebar
  never fabricates a selected row.
- **Unavailable or quarantined community:** keep the row visible and marked.
  Selecting its Retry action uses target-row-scoped recovery. Rust's old active
  context remains authoritative and the persistent sidebar never disappears.
- **Public reader:** keep the row visible with an honest read-only-unavailable
  state; do not offer Feed/Content until a core read-only context exists.
- **Followed site:** open status/refresh detail only. A blocked transport retains
  the row and explains why Refresh is absent.
- **Pending first sync:** open an honest Feed empty state that explains content
  has not arrived. Do not show fake posts or an empty white pane.
- **Followed-site transport blocked:** retain the row with a text-and-icon
  unavailable state and its truthful recovery.
- **Projection failure:** degrade only the affected Feed or Content section;
  preserve other locally held surfaces.
- **Authority change:** recompute visible Content and Apps actions. A control
  that disappears must not leave a stale callback capable of committing.
- **Community switch with a draft:** persist the old community's draft through
  the transition gate; never repoint it to the new community.
- **Failure after core activation:** reconcile to `activeCommunity()`. Follow a
  target that actually activated with a degraded-persistence warning; never
  draw the old detail over a new core namespace.
- **Community leave/removal:** retain the existing destructive discard guard.
- **Sync requiring attention:** show one contextual affordance. Dismissal or
  failure does not alter Feed selection or imply data was accepted.
- **Deep link:** switch to the held community through the transition gate,
  default to Feed, and open the named held record only after the existing local
  verification path and post-switch namespace equality both succeed.

## 9. Testing and verification

TDD is mandatory. Each implementation slice starts with a focused failing test,
then the smallest production change, then refactoring with the focused suite
green.

The implementation plan must decompose the work into four landable RED/GREEN
slices:

1. **Typed sidebar/navigation projection** — pure raw-row grouping,
   `SidebarItemID`, `SpaceSelectionState`, native outline semantics, and
   target-row failure. RED: focused sidebar/selection model tests fail on
   missing types and failure behavior. GREEN: pure models plus macOS root shell.
2. **Feed/People/Content/Apps recomposition** — destination rename, Feed without
   shortcuts, typed `CommunityContentModel`, and authority-adaptive detail.
   RED: destination/content projection tests. GREEN: recomposed existing views.
3. **Ambient sync and transition/deep-link safety** — shell-owned discovery and
   exact presentation mapping, wrong-community admission, explicit switch
   reconciliation result, generation-guarded callback teardown, and persistent
   Stop intent. RED: state mapping, failure-after-core-activation, failed
   deep-link, A→B→C delayed callbacks, Stop→switch→switch-back→relaunch, and
   import-admission regressions. GREEN: relocate the existing flows without
   weakening consent.
4. **Adaptive/accessibility delivery** — iPhone tab alignment, semantic macOS
   outline, Dynamic Type, keyboard commands, UI tests, and visual review. RED:
   shell/UI assertions. GREEN: both platforms and target membership.

Named seams include `CommunityRegistry` test stubs,
`CommunityTransitionGate`, `NewswireProjecting`,
`NewswireEditorAuthorityChecking`, `NewswirePostPublishing`,
`NearbyTransportController`, and `NearbyImportAdmission`. New shared source and
test files must be registered in both the iOS and macOS Xcode projects when
their targets consume them.

### 9.1 Pure model tests

- visible tier order is Communities, then Following; the deferred empty Your
  space tier is not rendered;
- only the selected space expands;
- selecting or switching a community defaults to Feed;
- communities expose Feed, People, Content, and Apps in canonical order;
- followed rows expose no fabricated community children;
- public-reader rows remain visible but cannot fabricate a selectable context;
- followed rows select status detail without switching the active community;
- unavailable rows remain present and carry actionable recovery state;
- unread badges do not change selection;
- accessibility text includes name, tier, state, and unread count;
- no technical ID is truncated or presented as a human label.

### 9.2 Shell and integration tests

- macOS renders spaces as the sidebar root rather than route rows;
- iPhone renders Feed, People, Content, and Apps as its bottom destinations;
- Feed contains alerts/newswire and no app-shortcut card;
- Apps opens the existing directory/runtime under the selected community;
- People is labeled and copied as Known contributors;
- Content browses held updates/alerts and adapts actions for member, editor, and
  organizer authority where an author-bearing context exists;
- hidden/tombstoned/future-quarantined content never regains raw payload;
- community switching prepares before repository mutation, preserves drafts,
  closes stale tool/detail callbacks, and stops/rebinds the old sync scope;
- a post-mutation persistence failure reconciles to the actual active namespace
  instead of showing stale old-community detail;
- healthy sync creates no primary route or Feed card;
- consent/import/failure states still require explicit review and cannot commit
  to a different community;
- stale BLE, local-route, pairing, coordinator, preview, and import callbacks
  with an invalid scope generation are inert and disconnect;
- Stop survives routes, community switches, keyed-shell rebuilds, and relaunch
  until explicit Restart;
- a failed deep-link switch emits no verified/detail outcome;
- rapid A→B→C switching makes delayed A/B callbacks inert;
- unavailable, pending, and degraded states never render a blank pane;
- Command-K and Command-1 through Command-4 select the expected targets.

### 9.3 Accessibility and visual verification

- VoiceOver and keyboard traversal cover the expanded hierarchy;
- selected/expanded/unavailable states are color-independent;
- accessibility Dynamic Type retains all destinations and 44-point actions;
- visual review captures macOS expanded/collapsed states and iPhone navigation
  at ordinary and accessibility sizes.

### 9.4 Commands

Each plan slice names its exact `-only-testing` class before implementation.
The final Apple verification includes the real iOS XCTest suite, simulator and
device builds, macOS tests/build, repository green suite, and the coverage
source of truth:

```sh
xcodebuild test \
  -project apps/ios/Riot.xcodeproj \
  -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$(sh scripts/ios-check.sh simulator-id)" \
  -derivedDataPath build/xcode-dd \
  CODE_SIGNING_ALLOWED=NO
xcodebuild test \
  -project apps/ios/Riot.xcodeproj \
  -scheme Riot \
  -destination "platform=iOS Simulator,id=$(sh scripts/ios-check.sh simulator-id)" \
  -only-testing:RiotUITests/ExpandedSpaceNavigationUITests \
  -derivedDataPath build/xcode-ui-dd \
  CODE_SIGNING_ALLOWED=NO
sh scripts/ios-check.sh sim
sh scripts/ios-check.sh ios
sh scripts/ios-check.sh test
sh scripts/ios-check.sh
xcodebuild test \
  -project apps/macos/Riot.xcodeproj \
  -scheme RiotKit-macOS \
  -destination 'platform=macOS'
sh scripts/green.sh
sh scripts/web/coverage.sh
```

`.coverage-thresholds.json` remains the single source of truth. No floor is
lowered for this work. Verification must confirm the UI-test command executed a
nonzero test count; a zero-test success is a failure.

## 10. Success and failure criteria

The redesign succeeds when:

- a macOS user can see every held row and directly switch among every available
  author-bearing community in the sidebar; followed-site rows open status
  detail, while public-reader and unavailable rows remain visible and truthful;
- a seeded multi-space macOS usability fixture switches communities with one
  direct sidebar action and no modal;
- the active community alone expands and Feed is selected by default;
- Feed is the dominant content surface and contains no app shortcut block;
- People, Content, and Apps are visibly scoped to the active community;
- Content adapts to current member/editor/organizer authority without inventing
  a public-reader context;
- healthy synchronization has no primary navigation or content card;
- all consent, wrong-community, and authority checks remain fail-closed; and
- focused and full Apple verification plus visual review pass.

The redesign fails if:

- community switching still requires the macOS modal chooser;
- Apps or sync remain global/peer destinations;
- a membership roster is implied without core proof;
- sync acceptance or organizer controls become presentation-authorized;
- a draft, import, or tool session crosses communities; or
- any unavailable/pending state becomes a blank or disappearing row.

## 11. Deliberate non-goals

- Building a complete member/role directory.
- Creating a production Personal registry record or personal-home detail.
- Opening public-reader communities without an author-bearing core context.
- Rendering followed-site content before its signed manifest has a durable
  resolver path; this slice provides status and refresh only.
- Adding generic files, pages, or media types to Content.
- Changing Nostr/Willow records, Meadowcap authority, app approval, or signing.
- Claiming operating-system background execution.
- Adding internet relay fallback to nearby exchange.
- Rebuilding the Android debug shell in this slice.
- Sidebar search/filtering for very large space collections.
- Redesigning the visual identity beyond the approved information hierarchy.

## 12. Alternatives considered

1. **Spaces sidebar plus horizontal section bar** — keeps the sidebar pure and
   the Feed widest. Rejected by the user in favor of visibly nesting local
   destinations under the selected space.
2. **Three-column workspace** — separates spaces, local destinations, and
   content most explicitly. Rejected because it consumes Feed width and adds
   persistent chrome.
3. **Keep route sidebar and modal community chooser** — smallest code change.
   Rejected because it preserves the hierarchy inversion that prompted this
   work.

## 13. Design-review revision record

Round 1 approved Product and Security. Architecture, UX, and CTO requested
revision. This version resolves their blockers by:

- making Personal explicitly deferred and public-reader opening unavailable in
  the no-core-change MVP;
- defining terminal followed-site status/refresh selection;
- separating core-active, sidebar-selected, attempting, and failed-target state;
- moving every Nearby lifecycle and consent responsibility into a precise
  shell-owned presentation contract;
- defining Content ordering, states, capabilities, redaction, and detail flow;
- requiring semantic `List`/`DisclosureGroup` accessibility behavior; and
- adding an executable four-slice TDD ladder and complete pre-delivery commands.

Round 2 approved Product, Architecture, UX, and CTO. Security requested
revision. This version resolves its blockers by:

- reconciling every switch against the actual active namespace, including a
  distinct degraded-success outcome for post-activation persistence failure;
- generation-guarding every asynchronous Nearby callback before it may read the
  active community, disclose metadata, present UI, or commit; and
- persisting the person's Stop/Restart discovery preference across shell
  reconstruction and relaunch.
