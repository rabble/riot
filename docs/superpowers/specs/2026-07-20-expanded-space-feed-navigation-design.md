# Expanded-Space, Feed-First Navigation Design

**Date:** 2026-07-20  
**Status:** User-approved design; pending metaswarm design-review gate  
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
   Content library that everyone can browse, so that authority adds capability
   without creating a separate admin product, when core-derived authority
   permits the action.
5. **A person exchanging data nearby** wants healthy exchange to happen without
   becoming the content focus, while still reviewing new peers and imports when
   consent is required.
6. **A person whose space cannot open** wants its row to remain visible with a
   recovery path, so that local corruption or incomplete sync never turns into
   lost-looking data or a blank pane.

## 3. Product decisions

### 3.1 Selected approach: expand the active space

The user selected the expandable-sidebar approach from three alternatives.
Spaces remain the first-level navigation. Only the active space expands to
show its supported local destinations.

```text
YOUR SPACE
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

- **Your space** — personal home rows;
- **Communities** — organizer, member, and public-reader rows; and
- **Following** — author-less followed-site rows.

A community exposes Feed, People, Content, and Apps. Personal spaces and
followed sites expose only destinations supported by their real model. The
shell must not fabricate People, Apps, posting, or management capabilities.

## 4. Platform behavior

### 4.1 macOS

The `NavigationSplitView` sidebar becomes the space tree. Selecting a community
runs the existing community transition gate, selects that row, expands it, and
opens Feed. Selecting one of the expanded children changes only the active
community destination.

Only one space is expanded at a time. Collapsed rows may show an unread or
attention badge, but badges never change selection. The footer contains global
actions such as Add or create a space and Your profile.

Community settings remain contextual to the selected community. They appear as
a secondary toolbar/header action rather than a peer destination.

### 4.2 iPhone

The phone cannot use a persistent sidebar, so it keeps the community selector
in the header. The bottom destinations align with the same product model:
Feed, People, Content, and Apps. Selecting a different community defaults to
Feed.

This keeps route names and scope consistent across the shared SwiftUI shell
without forcing desktop chrome onto a compact device.

### 4.3 Keyboard and accessibility

- Command-K focuses/selects the space list.
- Command-1 through Command-4 select Feed, People, Content, and Apps.
- Arrow-key navigation follows the visible tree order.
- Expansion, selection, unread state, and availability use text, icon/shape,
  and accessibility state; color is never the only signal.
- A row announces tier, name, relationship where relevant, availability, and
  unread state without exposing or truncating a namespace ID.
- The selected space and selected child each expose the correct selected and
  expanded accessibility traits.
- Dynamic Type and VoiceOver must not hide the child destinations or global
  profile action.

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

Capabilities are additive and authority-derived:

- public readers browse;
- members with a current author may publish an update;
- recognized editors receive existing editorial actions;
- organizers receive existing community management actions; and
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

When human action is required, the shell may present a transient sheet,
popover, notification, or compact warning. A blocked or failed exchange offers
plain-language recovery from that affordance. It does not replace the Feed with
a sync screen.

Feed recovery actions that previously navigated to Nearby instead open the
same contextual sync/recovery presentation.

## 7. Architecture and data flow

### 7.1 Pure sidebar projection

Add a pure, host-testable sidebar projection that merges:

- `RiotAppModel.communities` / core `CommunityRow` values; and
- `RiotAppModel.followedSites` / core `FollowedSiteRow` values.

The projection produces tiered rows, row state, supported children, unread
badge, selected/expanded state, and accessibility text. It does not infer
relationships, authority, or transport policy.

### 7.2 Navigation state

Replace the current primary `RiotDestination` values with Feed, People,
Content, and Apps. Keep route selection separate from the broad
`RiotAppModel.objectWillChange` stream, preserving the existing performance
contract.

Space selection and local-destination selection are distinct:

```text
core registry/following lists
  → RiotAppModel reload
  → pure sidebar projection
  → select space
  → CommunityTransitionGate.prepare
  → repository switch/open
  → keyed CommunityShellView
  → Feed default
  → select child destination without switching core context
```

Accepted store changes refresh the active Feed, Content projection, known
contributors, unread counts, and relevant row badges through the existing local
data-changed signal.

### 7.3 View reuse

The change recomposes existing views:

- Feed reuses active-alert and newswire surfaces;
- People reuses `PeopleView`;
- Apps reuses `DirectoryView` and `AppRuntimeView`; and
- Content adapts existing locally held newswire/alert projections into an
  archive-oriented list with current authority-gated actions.

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
  Selecting its Retry action uses the existing in-place recovery path; an
  unavailable row does not silently switch.
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
- **Community leave/removal:** retain the existing destructive discard guard.
- **Sync requiring attention:** show one contextual affordance. Dismissal or
  failure does not alter Feed selection or imply data was accepted.
- **Deep link:** switch to the held community through the transition gate,
  default to Feed, and open the named held record only after the existing local
  verification path succeeds.

## 9. Testing and verification

TDD is mandatory. Each implementation slice starts with a focused failing test,
then the smallest production change, then refactoring with the focused suite
green.

### 9.1 Pure model tests

- canonical tier order is Your space, Communities, Following;
- only the selected space expands;
- selecting or switching a community defaults to Feed;
- communities expose Feed, People, Content, and Apps in canonical order;
- personal and followed rows expose only supported children;
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
- Content browses held updates/alerts and adapts actions for reader, member,
  editor, and organizer authority;
- community switching prepares before repository mutation, preserves drafts,
  closes stale tool/detail callbacks, and stops/rebinds the old sync scope;
- healthy sync creates no primary route or Feed card;
- consent/import/failure states still require explicit review and cannot commit
  to a different community;
- unavailable, pending, and degraded states never render a blank pane;
- Command-K and Command-1 through Command-4 select the expected targets.

### 9.3 Accessibility and visual verification

- VoiceOver and keyboard traversal cover the expanded hierarchy;
- selected/expanded/unavailable states are color-independent;
- accessibility Dynamic Type retains all destinations and 44-point actions;
- visual review captures macOS expanded/collapsed states and iPhone navigation
  at ordinary and accessibility sizes.

### 9.4 Commands

Focused XCTest commands are defined by the implementation plan. The final
Apple verification includes:

```sh
sh scripts/ios-check.sh test
sh scripts/ios-check.sh
xcodebuild test \
  -project apps/macos/Riot.xcodeproj \
  -scheme RiotKit-macOS \
  -destination 'platform=macOS'
```

Because this design changes no Rust or FFI code, workspace Rust verification is
not a mechanical dependency of the shell slice. The repository's required
pre-PR quality and coverage gates still run before delivery.

## 10. Success and failure criteria

The redesign succeeds when:

- a macOS user can see and switch among all held spaces directly in the sidebar;
- the active community alone expands and Feed is selected by default;
- Feed is the dominant content surface and contains no app shortcut block;
- People, Content, and Apps are visibly scoped to the active community;
- Content adapts to current reader/member/editor/organizer authority without
  inventing capabilities;
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
- Adding generic files, pages, or media types to Content.
- Changing Nostr/Willow records, Meadowcap authority, app approval, or signing.
- Claiming operating-system background execution.
- Adding internet relay fallback to nearby exchange.
- Rebuilding the Android debug shell in this slice.
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

