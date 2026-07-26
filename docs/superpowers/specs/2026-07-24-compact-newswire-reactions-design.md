# Compact, Reliable Newswire Reactions — Design

**Date:** 2026-07-24
**Status:** Revised after design-review iteration 2; awaiting gate approval and
written-spec approval
**Scope:** Reaction reliability in Rust core/FFI plus the shared iOS/macOS
SwiftUI newswire surface

## Problem

The four communal reactions under each ordinary newswire post—Support,
Solidarity, Important, and Grief—look like inert categories or hashtags. In the
real macOS River City Wire surface, clicking them appears to do nothing.

The visible problem has two layers:

1. The controls look like metadata and discard the outcome of a reaction write,
   so accepted, rejected, and missed interactions are indistinguishable.
2. The underlying immutable reaction-event design is not safe enough to put
   behind a clear retry affordance:
   - every toggle occupies a new Willow coordinate and consumes bounded sync
     inventory;
   - the FFI path can commit durably and then report failure while tracking the
     committed entry, inviting a retry that creates another event;
   - the production clock is truncated to seconds, so rapid
     react/retract/react writes can tie and projection permanently prefers the
     retraction;
   - projection does not expose whether the current profile already reacted, so
     the selected state and the direction of the next toggle are wrong after
     relaunch.

This design fixes the durable semantics first, then makes the controls compact,
obviously interactive, and honest about pending and failure states.

## Primary use case

**Who:** a person reading a post inside a community they have already joined.
**Wants:** to add or remove one of four small communal reactions directly from
the newswire.
**So that:** they can signal support, solidarity, importance, or grief without
writing a reply or depending on a live internet connection.
**When:** online, offline, immediately after relaunch, or while the same post is
shown in both Front Page and Open Wire.

The action is local-first. A relay connection is not required for the write.
Sync later carries the resulting current reaction state.

## Product intent

Reactions are small, signed communal actions—not post categories, hashtags, or
navigation. They remain secondary to reading the report, but they must look and
behave like controls.

The selected direction is a compact, always-visible row. The controls do not
appear only on hover and are not buried under “More details.” The row is attached
to each ordinary post for which the active profile has reaction authority.

## Durable reaction state

### Exact current-state path and migration

The existing Newswire v1 protocol gains a second reaction family; this is not a
Newswire v2 migration. New local writes use this exact Willow path:

```text
["newswire", "v1", descriptor_entry_id, "reaction-state",
 parent_post_entry_id, reaction_kind_u8]
```

`reaction_kind_u8` is one byte using the existing canonical mapping: Support 0,
Solidarity 1, Important 2, Grief 3. The reacting author is the Willow subspace,
not a duplicated path component. The resulting Willow coordinate is therefore
one current state per `(namespace, author, descriptor, parent, kind)`.

`NewswirePathKind` separates `LegacyReaction { descriptor_id }` from
`ReactionState { descriptor_id, parent_id, kind }`. Classification returns
either the legacy time/digest components or the current-state parent/kind
components; callers cannot pretend both shapes have a time/digest suffix.
`load_space_records` queries both `reactions` and `reaction-state`.

The signed `NewsReactionV1` payload still says whether the reaction is active or
retracted. A later toggle writes the same logical coordinate with a strictly
newer entry timestamp, so Willow retains the current state instead of
accumulating one live coordinate per click.

The inspector validates that the parent post and reaction kind encoded in the
new path exactly match the signed payload. A mismatched or malformed
current-state reaction is rejected.

Existing timestamp-and-digest reaction paths remain readable and projectable for
backward compatibility. New local writes do not use the legacy path.

### Causal ordering

Production clock snapshots preserve microsecond precision. Before signing a
current-state reaction, the writer finds the maximum accepted timestamp for the
same `(author, descriptor, parent, kind)` across **both** legacy reaction events
and the current-state coordinate, then chooses:

`max(current clock timestamp, previous entry timestamp + 1 microsecond)`

This gives sequential writes that have observed the prior state strict causal
ordering across rapid toggles, clock rollback, process relaunch, and the first
post-upgrade write after a legacy reaction. If the resulting timestamp exceeds
the projection clock plus `MAX_FUTURE_SKEW_MICROS`, or the previous timestamp
cannot be incremented without overflow, the write fails before mutation with
the clock failure category.

Projection continues to use deterministic LWW ordering for legacy reactions and
uses Willow’s retained current-state winner for new reactions. When both forms
exist for the same `(author, parent, kind)`, the later signed state wins under
Willow’s canonical entry-recency ordering.

Two disconnected devices using the same author key cannot observe each other’s
next timestamp. Their concurrent writes promise deterministic convergence after
merge, not knowledge of which human gesture occurred last. An equal-timestamp
test proves store retention and projection choose the same canonical Willow
winner on both replicas.

### Admission before commit

All fallible inventory admission happens before the store mutates:

1. validate and sign the candidate;
2. compute the prospective live sync inventory, including coordinate
   replacement/pruning;
3. enforce count and byte caps;
4. retain the exact admitted `Vec<SignedWillowEntry>`;
5. commit the inspected entry;
6. assign that pre-admitted vector to `sync_inventory` under the same profile
   mutex.

Normal toggles replace one logical coordinate and therefore do not grow live
inventory. A modified client that creates many distinct logical reactions still
hits the existing bounded admission cap before commit.

Inventory installation after commit is an infallible assignment, not a second
admission decision. No fallible `track_committed_entry` call remains after the
commit. Durable relaunch restores exact signed proofs from the durable signed
entry table; an in-memory profile has no durable store to survive a crash. A
crash-boundary test covers durable commit before in-memory assignment and proves
that relaunch restores the proof without creating a retryable error.

UniFFI returns
`Result<NewswireReactionMutationRecord, MobileError>`, where the success record
contains the signed receipt, requested `active` state, and
`status = committed`. Every `Err` is pre-commit. Projection refresh happens
after this explicit committed result and cannot retroactively turn it into a
write rejection.

### Bounded, fail-soft projection

`EvidenceStore` gains a namespace-scoped multi-prefix bounded query. Both the
SQLite repository and in-memory join enforce the bound before allocating the
returned vector:

```text
entries_with_prefixes_in_namespace_bounded(
    namespace, prefixes, limit
) -> { entries: Vec<PrefixedEntry>, truncated: bool }
```

The SQLite schema migration adds the entry’s canonical big-endian
`timestamp_be` to each `entry_path_prefixes` row and changes the lookup index to
`(namespace_id, depth, prefix_bytes, timestamp_be DESC, entry_id DESC)`.
The query unions the requested disjoint prefixes and applies
`ORDER BY timestamp_be DESC, entry_id DESC LIMIT limit + 1` in SQL. The
in-memory implementation streams matching live entries through a fixed-size
top-K heap of `limit + 1` using the identical order; it never first builds an
unbounded match vector. The extra item detects truncation and is not decoded.
Migration, disk-accounting, and memory/SQLite parity tests are required.

Loading uses two independent budgets:

- posts + editorial actions + comments: existing shared
  `MAX_PROJECTED_RECORDS = 1_024`; exceeding it retains the existing fail-closed
  behavior;
- legacy `reactions` + current `reaction-state`: new shared
  `MAX_PROJECTED_REACTIONS = 1_024`; only the newest 1,024 live reaction entries
  in canonical order are decoded, and excess reactions set an internal
  `reactions_truncated` diagnostic flag.

Reaction entries can therefore neither allocate without bound nor consume the
non-reaction budget. The truncation flag is internal telemetry using the stable
redacted projection code; no attacker-controlled count or identifier is shown
or logged. Tests cover `limit - 1`, `limit`, `limit + 1`, both prefixes sharing
one budget, deterministic retention on two replicas, and valid posts remaining
visible under a reaction flood.

### Authoritative current-profile state

Core exposes:

```text
project_space_for_viewer(
    store, descriptor, clock, viewer: Option<SubspaceId>
) -> NewswireProjection
```

For every reaction tally it returns:

- collective active count; and
- `reacted_by_me`, computed from the active profile author without exposing the
  set of reacting author IDs.

`reacted_by_viewer` is computed while the per-author logical reaction map still
exists; reactor IDs never leave core. Viewerless callers pass `None` and receive
`false`. The profile-aware FFI passes its current author and exports
`NewswireReactionTallyRecord { kind, count, reacted_by_me }`.

Both legacy and current-state active winners contribute to `reacted_by_me`, so
selection and the next toggle direction are correct before and after upgrade or
relaunch. UniFFI Swift/Kotlin bindings and the Android host-JVM fake are updated
in the same work unit and compiled in acceptance.

## Compact interaction design

Every ordinary post with reaction authority shows these four controls in this
fixed order:

| Meaning | Visible glyph | Spoken/hover label |
| --- | --- | --- |
| Support | `♥` (U+2665) | Support |
| Solidarity | `✊︎` (U+270A + text presentation) | Solidarity |
| Important | `!` | Important |
| Grief | `◌` (U+25CC) | Grief |

These are text glyphs, not platform emoji or remotely loaded assets. They use
the system text font at 15 points, semibold; the explicit text-presentation
selector on the fist prevents emoji styling. Counts use `SpaceMono-Regular` at
13 points with tabular digits. Snapshot tests on both targets prevent emoji
presentation, missing-glyph boxes, or baseline drift. The full names remain
available in macOS hover help, accessibility labels, and the iOS long-press
context menu.

On the first eligible post shown after install, a dismissible compact legend
appears once: **“Reactions: ♥ Support · ✊ Solidarity · ! Important · ◌ Grief.”**
This teaches the two less-obvious glyphs to sighted touch users without making
every post verbose. Dismissal is local UI preference only and does not affect
community data.

Each control is a real button whose complete visible shape is hit-testable. Its
visual footprint is compact, while the interaction frame is at least 44×44
points. Counts are always shown, including zero. Counts use tabular digits and
display `0…999`, then `999+`, keeping the row stable.

### State treatment and precedence

The shared `CompactReactionButtonStyle` uses host `RiotTheme` semantic roles:
`card`, `paper2`, `ink`, `inkSoft`, and `pink`, plus two reaction roles:

- `onReactionAccent`: `#F6F2E9` in light and `#131209` in dark;
- `danger`: `#B3261E` in light and `#FFB4AB` in dark.

The six microapp CSS presets do not theme native SwiftUI chrome and are
therefore outside this control’s rendering matrix. Light and dark host-role
values are both tested even though the current app shell prefers light.

Exact geometry is:

- visible capsule height 32 points, minimum width 58 points, 10-point horizontal
  padding, 6-point glyph/count spacing, and 16-point corner radius;
- transparent interaction padding expands each target to at least 44×44;
- default outline is one point; selected and error outlines are two points;
- spinner is 14×14 and replaces the glyph without moving the count;
- the selected check is a 9-point `checkmark` system symbol after the count;
- all content is vertically centered on one baseline.

The state matrix is:

| State | Treatment |
| --- | --- |
| Default | `card` fill, 1-point `inkSoft` outline, `ink` glyph/count |
| Hover | `paper2` fill and 2-point `ink` outline; no layout shift |
| Pressed | default/selected colors, 0.96 scale, 0.88 opacity |
| Keyboard focused | native focus ring outside the 44-point target |
| Selected | `pink` fill, `onReactionAccent` foreground, 2-point `ink` outline, trailing check |
| Pending | 14-point spinner replaces glyph, count remains; selected fill/check remain; button disabled |
| Authority unavailable | 50% opacity, no hover/pressed treatment, disabled trait |
| Failed write | default/selected fill restored, 2-point `danger` outline on only that key, focus retained |

Precedence is: authority-disabled → pending → failed → selected/default, with
hover/pressed/focus layered only where meaningful. A pending write never
optimistically changes count or selection.

Automated contrast tests require at least 4.5:1 for glyph/count text and 3:1 for
control boundaries, focus, spinner, check, and error cues against their adjacent
surface in both host schemes. The `danger` role cannot land until those tests
pass.

### Adaptive layout

The preferred layout is a single row of four controls with 8-point spacing.
`ViewThatFits` falls back to a two-column `LazyVGrid` with 8-point row and column
spacing when one row cannot preserve all of these constraints:

- every control remains visible;
- no horizontal scrolling;
- no clipped glyph or count;
- at least 8 points between interaction frames;
- 44×44 minimum targets.

The constrained contract is tested at a 320-point iPhone viewport with 288
points of available card content and at Dynamic Type
`UIContentSizeCategory.accessibility3`. Both use the complete two-by-two layout.
The normal desktop contract is tested with at least 500 points of available card
content and stays one row. The order remains Support, Solidarity, Important,
Grief in reading order.

## Async state and data flow

### Typed logical key

Shared presentation state is keyed by:

`ReactionKey(postID: full entry ID, kind: ReactionKind)`

The key contains the full internal ID; it is never printed in UI or diagnostic
output. If the same post appears in both Front Page and Open Wire, both
renderings observe the same selected, pending, count, and failure state.
Accessibility identifiers add a surface qualifier solely to keep rendered
elements unique in UI tests. Their post component is an ephemeral opaque
presentation token assigned by the model—not the raw post ID, a prefix of it, or
a reversible encoding. The deterministic fixture uses the fixed alias
`fixture-post-1`.

### Off-main writer boundary

Reaction persistence and projection are asynchronous from SwiftUI’s point of
view. A `Sendable` reaction writer owns the thread-safe `MobileProfile` handle
and serializes blocking UniFFI work off the main actor:

1. discard the request without mutation if its Swift task was cancelled while
   queued;
2. write the requested active state;
3. if committed, refresh projection;
4. increment the actor’s completion revision and return one typed result:
   - `accepted(projection, revision)`;
   - `committedNeedsRefresh(activeState, revision,
     reaction_projection_refresh)`;
   - `rejected(code, revision)`.

`NewswireSurfaceModel` stays `@MainActor` and owns:

- `pending: Set<ReactionKey>`;
- `failures: [ReactionKey: ReactionFailurePresentation]`;
- the current projection, including authoritative `reacted_by_me`;
- a model/community generation token used to reject stale presentation updates.
- `lastAppliedWriterRevision`, used to reject an older full projection that
  resumes on the main actor after a newer result.

On tap, the model derives the requested state from authoritative
`reacted_by_me`, inserts only that key into `pending`, and yields so SwiftUI can
render the spinner before blocking work begins. A second tap on the same key is
ignored while pending. Different keys and different posts may proceed, while
the writer safely serializes access to the profile.

When a view disappears or the active community changes, presentation tasks are
cancelled and the generation token advances. A request cancelled while still
queued at the writer actor checks cancellation and never calls UniFFI. A
synchronous UniFFI call already in progress is allowed to finish; durable work
is not interrupted halfway. Its result is reported for diagnostics but is not
applied to a stale model. One operation per key, community generation checking,
and monotonically increasing writer revisions prevent out-of-order full
projections from reverting newer UI state.

### Capability and authority presentation

For this communal namespace, core admits reactions from any valid community
member; no editorial role is required. Swift’s initial capability source is the
presence of the repository’s profile-backed reaction writer plus a valid active
descriptor. If either is absent, the reaction row is omitted.

All values generated by the four fixed controls are canonical, so a
`reaction_authority_or_input` failure during an on-screen write is treated as
runtime authority loss. The existing row remains in place but all four controls
become disabled for the lifetime of that surface model, preserving focus and
explaining the transition. A newly constructed model with no writer omits the
row. There is no ambiguous remove-or-disable transition.

### Result behavior

- **Accepted:** if its revision is newest, replace the projection, clear
  pending, and clear only that key’s prior failure. VoiceOver announces
  “Support reaction added” or the corresponding removed message once.
- **Committed, refresh failed:** set selection to the known committed state,
  leave the last confirmed count, clear pending, and show
  **“Reaction saved. Count will update when the wire refreshes.”** This is not a
  retry affordance because repeating the toggle would reverse the saved state.
- **Retryable persistence rejection before commit:** retain count and selection,
  clear pending, and show **“Couldn’t save your reaction. Try again.”**
- **Authority/input rejection:** retain count and selection, disable the row,
  and show **“Reactions aren’t available for this post.”**
- **Capacity rejection:** retain count and selection, disable only that key
  until the next full projection/navigation, and show
  **“This community can’t hold another reaction right now.”**
- **Clock rejection:** retain count and selection, disable only that key until
  the next full projection/navigation, and show
  **“Check this device’s Date & Time before reacting.”**

There is no runtime `.unavailable` branch after a row is constructed.

### Pending duration

Pending appears in the first rendered update after insertion and duplicate taps
remain blocked for the entire operation. If a local write is still pending after
2 seconds, the inline status becomes **“Still saving on this device…”** and is
announced once; the spinner remains and no retry is offered because commit state
is unknown. Navigation may cancel a queued write, but an already-started write
continues under the writer contract above.

## Failure, accessibility, and diagnostics

Failure presentation is keyed per logical reaction and records the surface that
initiated the operation. Success clears the failure for that key only. Retrying
the same key clears its old message as pending begins. Navigating away destroys
ephemeral failure presentation.

Only one compact line is rendered below a post: committed-refresh failure takes
priority over retryable rejection, then the most recent failure. When the post
is duplicated, shared failed styling and accessibility error value appear on
both buttons, but visible copy appears only under the initiating surface.
The model emits one sequence-numbered accessibility announcement consumed only
by that surface. It moves no focus; keyboard focus remains on the activated
button. This prevents duplicate messages and VoiceOver announcements.

Each control exposes:

- role: button;
- label: full reaction name;
- value: “No reactions,” “1 reaction,” or “N reactions”;
- selected trait when `reacted_by_me`;
- busy and disabled semantics while pending;
- hint: “Adds your reaction” or “Removes your reaction”;
- stable, surface-qualified accessibility identifier.

The diagnostic channel uses stable, non-sensitive codes only:

- `reaction_authority_or_input`;
- `reaction_capacity`;
- `reaction_clock`;
- `reaction_persistence`;
- `reaction_projection_refresh`;
- `reaction_projection_truncated`.

Rust retains exact internal error categories before mapping to the existing
mobile error boundary. The explicit UniFFI mutation record makes committed
versus rejected state unambiguous. Swift maps errors to the stable coarse code
and an injected reporter. Diagnostic payloads never include raw post IDs,
namespace IDs, Willow paths, keys, signed bytes, user-authored content, or exact
attacker-controlled truncation counts. Redaction tests assert that those values
cannot appear.

## Deterministic joined-community fixture

The interaction tests use two profiles:

1. an author creates River City Wire and publishes a post;
2. a reader imports/joins that community and becomes the active UI profile.

Fixture bootstrap performs the real sequence before the first newswire frame:
the author creates the namespace and descriptor, signs one post, exports the
descriptor/post bundle; the reader opens a separate profile, joins/imports the
bundle, selects that joined community, and supplies its profile-backed writer
to `NewswireSurfaceModel`.

The fixture uses an isolated temporary profile and local bundles only—no relay,
network, production database, or shared user state. It proves the exact
joined-community authority path shown in the user’s screenshot rather than the
author’s own-space shortcut.

The macOS runner supplies:

```text
RIOT_UI_TEST_RUN_ID=<valid UUID>
RIOT_UI_TEST_FIXTURE=reactions-joined
RIOT_UI_TEST_REACTION_GATE=1
```

The existing UUID gate selects
`temporaryDirectory/riot-ui-<UUID>` and the fixture flag is ignored without a
valid run ID. Therefore even a manually supplied fixture flag cannot select or
mutate a production profile. A negative test launches with an invalid/missing
UUID and proves the fixture and gate remain inactive.

For deterministic pending observation, the real writer pauses immediately
before its UniFFI call on a macOS
`DistributedNotificationCenter` gate whose notification name includes the run
UUID. XCUITest observes busy state, posts the matching release notification,
and then the unmodified joined-community core/FFI write proceeds. The gate has a
5-second test-only fail-closed timeout and does not fake the result.

## Test-first root-cause proof

The first executable RED test adds a macOS UI-test target to
`apps/macos/Riot.xcodeproj`, includes it in the `Riot-macOS` scheme, launches the
joined-community fixture, and:

1. locates `reaction.open-wire.support.fixture-post-1`;
2. clicks the visible control;
3. observes pending/busy state;
4. asserts count changes from 0 to 1 and selected becomes true;
5. clicks again and asserts count returns to 0 and selected becomes false.

The deterministic time contract is:

- busy state becomes observable within 1 second of the click;
- after the gate release, accepted or rejected completion appears within 5
  seconds;
- the separate stalled-write test holds the gate for 2 seconds and observes
  “Still saving on this device…” within the next 1 second.

Timeout is a test failure, never silently extended.

The command is:

```sh
xcodebuild -project apps/macos/Riot.xcodeproj \
  -scheme Riot-macOS \
  -destination 'platform=macOS' \
  test
```

Before implementation this fails because the target/fixture and observable
interaction contract do not exist. It becomes the end-to-end proof that the
rendered control receives a real pointer event and the joined-community write
reaches core.

Lower-level RED/GREEN tests cover:

- exact current-state path classification, payload/path mismatch rejection, and
  legacy compatibility;
- rapid react/retract/react, clock rollback/relaunch, and a first current-state
  write overriding the newest admissible future-dated legacy reaction;
- same-author disconnected replicas producing equal-timestamp current states
  and converging on Willow’s same winner;
- preflight rejection leaves store and inventory unchanged;
- exact 64-entry admission boundaries, no post-commit retryable rejection, and
  durable crash/relaunch between commit and in-memory inventory assignment;
- bounded inventory under 1,000 repeated toggles of one logical key;
- bounded, fail-soft deterministic reaction loading at 1,023, 1,024, and 1,025
  entries, with legacy/current prefixes sharing one quota and valid posts never
  crowded out;
- authoritative legacy and current-state `reacted_by_me` across
  projection/relaunch, plus viewerless projection returning false;
- accepted, retryable persistence, authority, capacity, clock, and
  committed-refresh-failed Swift results and exact copy/disable behavior;
- pending isolation across kinds and posts;
- duplicate Front Page/Open Wire renderings sharing state, showing one visible
  error, and emitting one accessibility announcement from the initiating
  surface;
- concurrent different-key operations, actor revisions, stale/out-of-order
  completion suppression, queued cancellation before mutation, and
  already-started completion after navigation;
- retry and per-key error clearing;
- count formatting/pluralization at 0, 1, 999, and 1,000 (`999+`);
- keyboard activation, hover/focus/pressed states, selected non-color cue,
  44-point hit targets, VoiceOver values, failure announcement, and the one-time
  touch legend;
- exact 320/288-point accessibility layout and 500-point desktop layout;
- light/dark semantic-role contrast and cross-platform text-glyph snapshots;
- privacy-safe diagnostic mapping and redaction.

## Visual verification

Native macOS screenshots are captured from the deterministic fixture at 900- and
1200-point window widths, producing at least 500 points of card content. iPhone
checks use a 320-point viewport/288-point card width in normal type and
`accessibility3`.

The visual gate blocks release if:

- any of the four controls is absent, clipped, truncated, or horizontally
  scrollable;
- a pointer click within the visible control fails the UI test;
- a target is smaller than 44×44 points;
- normal desktop layout wraps when 900 points are available;
- constrained layout fails to use the complete two-by-two fallback;
- selected state is distinguishable only by color;
- pending, stalled, or failure feedback misses the explicit 1/2/5-second test
  thresholds above;
- any light/dark role pairing misses the 4.5:1 text or 3:1 non-text contrast
  threshold.

If direct desktop `screencapture` is unavailable, the macOS UI test attaches
`XCUIScreen.main.screenshot()` to the result bundle and the image is exported
from the `.xcresult` for review. Headless capture failure is therefore not an
excuse to skip visual evidence.

## Repository quality gates

Both work units use TDD and independently run their proportional gates. Before
integration, the combined branch must pass:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
cargo xtask validate-contracts
scripts/web/coverage.sh

xcodebuild -project apps/macos/Riot.xcodeproj \
  -scheme Riot-macOS -destination 'platform=macOS' test

xcodebuild -project apps/ios/Riot.xcodeproj \
  -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 16 Pro' test
```

The coverage script reads the ratchet floors from
`.coverage-thresholds.json`; no task substitutes a hand-written threshold.
Binding regeneration is followed by the repository’s Android host-JVM compile
and fake tests named by the implementation plan, preventing another UniFFI
interface/fake drift break.

## Work-unit boundary

Implementation is deliberately ordered:

1. **Core reaction reliability:** versioned current-state coordinates,
   monotonic timestamps, preflight/commit invariants, bounded fail-soft
   projection, profile-aware `reacted_by_me`, FFI/binding/fake updates, and Rust
   tests.
2. **Compact async UX:** off-main writer, shared keyed state, compact controls,
   failure/accessibility behavior, deterministic macOS fixture/UI target, Swift
   tests, and native visual verification.

The UX work unit cannot ship before the core work unit is green because making
retry prominent on top of ambiguous commit semantics would amplify data and
inventory defects.

## Scope

### In scope

- The two ordered work units above.
- Rust core/FFI protocol evolution with legacy read compatibility.
- Generated bindings and host fakes required by the FFI change.
- Shared iOS/macOS newswire model, view, and tests.
- A deterministic macOS rendered-click harness and screenshots.

### Out of scope

- New reaction kinds, freeform reactions, hashtags, or topic filtering.
- Editorial actions, replies, or newswire card hierarchy redesign.
- Relay availability, relay retry policy, or the sidebar/header sync-freshness
  disagreement visible in the screenshot.
- Displaying who reacted.
- Migrating or rewriting legacy signed reaction records.

## Acceptance

- A joined-community reader can add and remove every reaction offline.
- Sequential observed toggles and clock rollback always project the last
  accepted state; unseen concurrent same-author writes converge
  deterministically.
- Repeated toggles do not grow live sync inventory.
- No pre-commit rejection mutates durable state, and no durable commit is
  reported as retryable rejection.
- A reaction flood cannot make otherwise valid posts disappear from projection.
- Counts and `reacted_by_me` are correct after relaunch.
- All four compact controls remain visible, accessible, and subordinate to post
  content.
- Duplicate renderings share state; unrelated controls remain usable while one
  write is pending.
- Rejected and committed-refresh-failed writes have distinct, accurate copy.
- The native macOS UI test proves the visible control is clickable end to end.
- Rust and Swift tests, formatting, strict Clippy, coverage floors from
  `.coverage-thresholds.json`, generated-binding host builds, and native visual
  gates all pass.
