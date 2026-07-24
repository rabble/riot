# Compact, Reliable Newswire Reactions — Design

**Date:** 2026-07-24
**Status:** Revised after design-review iteration 1; awaiting gate approval and
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

### One current coordinate per logical reaction

New local writes use a versioned current-state reaction path with one Willow
coordinate per:

`(community namespace, reacting author, parent post ID, reaction kind)`

The stored signed payload still says whether the reaction is active or
retracted. A later toggle writes the same logical coordinate with a strictly
newer entry timestamp, so the store and sync inventory retain the current state
instead of accumulating one live coordinate per click.

The inspector validates that the parent post and reaction kind encoded in the
new path exactly match the signed payload. A mismatched or malformed
current-state reaction is rejected.

Existing timestamp-and-digest reaction paths remain readable and projectable for
backward compatibility. New local writes do not use the legacy path.

### Causal ordering

Production clock snapshots preserve sub-second precision. Before signing a
current-state reaction, the writer reads the latest entry at that logical
coordinate and chooses:

`max(current clock timestamp, previous entry timestamp + 1 microsecond)`

This holds across rapid toggles, clock rollback, process relaunch, and different
devices writing as the same author. If a timestamp cannot be incremented safely,
the write fails before mutation with the existing clock failure category.

Projection continues to use deterministic LWW ordering for legacy reactions and
uses the latest current-state coordinate for new reactions. When both forms
exist for the same `(author, parent, kind)`, the later signed state wins under
the same timestamp/tie-break rules.

### Admission before commit

All fallible inventory admission happens before the store mutates:

1. validate and sign the candidate;
2. compute the prospective live sync inventory, including coordinate
   replacement/pruning;
3. enforce count and byte caps;
4. commit the inspected entry;
5. install the already-admitted inventory under the same profile mutex.

Normal toggles replace one logical coordinate and therefore do not grow live
inventory. A modified client that creates many distinct logical reactions still
hits the existing bounded admission cap before commit.

Inventory installation after commit is an invariant-preserving operation, not a
second admission decision. If an unexpected reconciliation invariant is ever
violated after durable commit, the core rebuilds inventory from the store and
returns a **committed-needs-refresh** result. It must never return a retryable
“rejected” result for a write that already committed.

### Bounded, fail-soft projection

The existing projection cap remains a resource boundary. Excess reaction
records are handled as a reaction-specific quarantine/drop count rather than
causing the entire newswire to disappear. Processing and selection are
deterministic, bounded, and prefer the current-state coordinate for each logical
reaction. Descriptor, post, editorial-action, and comment validation retains
its existing fail-closed behavior.

This protects the wire from legacy reaction-event floods while keeping valid
posts readable. Tests cover the boundary, deterministic selection, and bounded
memory behavior.

### Authoritative current-profile state

Projection for an active profile returns, for every reaction tally:

- collective active count; and
- `reacted_by_me`, computed from the active profile author without exposing the
  set of reacting author IDs.

This makes selection and the next toggle direction correct after relaunch.
Core’s community-level projection remains usable without a viewer; the
profile-aware FFI projection supplies the boolean. Generated bindings and host
fakes are updated together so all platforms continue to compile.

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
the app’s UI font with a system-font fallback. A snapshot test on both target
platforms prevents accidental emoji presentation or missing-glyph boxes. The
full names remain available in macOS hover help, accessibility labels, and the
iOS long-press context menu.

Each control is a real button whose complete visible shape is hit-testable. Its
visual footprint is compact, while the interaction frame is at least 44×44
points. Counts are always shown, including zero. Counts use tabular digits and
display `0…999`, then `999+`, keeping the row stable.

### State treatment and precedence

The shared `CompactReactionButtonStyle` has one state matrix:

| State | Treatment |
| --- | --- |
| Default | quiet surface background, one-pixel neutral outline, dark glyph/count |
| Hover | stronger outline and slightly raised surface; no layout shift |
| Pressed | 0.96 scale and reduced surface brightness |
| Keyboard focused | native focus ring outside the 44-point interaction frame |
| Selected | pink fill, high-contrast foreground, plus a small check indicator so color is not the only cue |
| Pending | spinner replaces glyph, count remains; button disabled; selected fill remains when selected |
| Disabled by authority | 50% opacity, no hover/pressed treatment, accessibility disabled trait |
| Failed write | normal/selected state restored, red error outline on only that key, focus retained |

Precedence is: authority-disabled → pending → failed → selected/default, with
hover/pressed/focus layered only where meaningful. A pending write never
optimistically changes count or selection.

### Adaptive layout

The preferred layout is a single row of four controls. `ViewThatFits` falls back
to a two-by-two grid when one row cannot preserve all of these constraints:

- every control remains visible;
- no horizontal scrolling;
- no clipped glyph or count;
- at least 8 points between interaction frames;
- 44×44 minimum targets.

The two-by-two layout is expected on narrow iPhone widths, accessibility Dynamic
Type sizes, or when localized/large count content cannot fit. The order remains
Support, Solidarity, Important, Grief in reading order.

## Async state and data flow

### Typed logical key

Shared presentation state is keyed by:

`ReactionKey(postID: full entry ID, kind: ReactionKind)`

The key contains the full internal ID; it is never printed in UI or diagnostic
output. If the same post appears in both Front Page and Open Wire, both
renderings observe the same selected, pending, count, and failure state.
Accessibility identifiers add a surface qualifier solely to keep rendered
elements unique in UI tests.

### Off-main writer boundary

Reaction persistence and projection are asynchronous from SwiftUI’s point of
view. A `Sendable` reaction writer owns the thread-safe `MobileProfile` handle
and serializes blocking UniFFI work off the main actor:

1. write the requested active state;
2. if committed, refresh projection;
3. return one typed result:
   - `accepted(projection)`;
   - `committedNeedsRefresh(activeState, diagnosticCode)`;
   - `rejected(diagnosticCode)`.

`NewswireSurfaceModel` stays `@MainActor` and owns:

- `pending: Set<ReactionKey>`;
- `failures: [ReactionKey: ReactionFailurePresentation]`;
- the current projection, including authoritative `reacted_by_me`;
- a model/community generation token used to reject stale presentation updates.

On tap, the model derives the requested state from authoritative
`reacted_by_me`, inserts only that key into `pending`, and yields so SwiftUI can
render the spinner before blocking work begins. A second tap on the same key is
ignored while pending. Different keys and different posts may proceed, while
the writer safely serializes access to the profile.

When a view disappears or the active community changes, presentation tasks are
cancelled and the generation token advances. A UniFFI call already in progress
is allowed to finish; durable work is not interrupted halfway. Its result is
reported for diagnostics but is not applied to a stale model. One operation per
key plus generation checking prevents out-of-order completion from reverting
newer UI state.

### Result behavior

- **Accepted:** replace the projection, clear pending, and clear only that key’s
  prior failure.
- **Committed, refresh failed:** set selection to the known committed state,
  leave the last confirmed count, clear pending, and show
  **“Reaction saved. Count will update when the wire refreshes.”** This is not a
  retry affordance because repeating the toggle would reverse the saved state.
- **Rejected before commit:** retain count and selection, clear pending, and
  show **“Couldn’t save your reaction. Try again.”**

There is no runtime `.unavailable` branch after a row is constructed. Authority
loss arrives as a rejected write and the next projection removes or disables
the row according to the existing authority gate.

## Failure, accessibility, and diagnostics

Failure presentation is keyed per logical reaction. Success clears the failure
for that key only. Retrying the same key clears its old message as pending
begins. Navigating away destroys ephemeral failure presentation.

Only one compact line is rendered below a post: committed-refresh failure takes
priority over retryable rejection, then the most recent failure. VoiceOver
announces a failure once as it appears and moves no focus; keyboard focus remains
on the activated button. The message is associated with that button through
accessibility help/error semantics.

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
- `reaction_reconciliation`.

Rust retains exact internal error categories before mapping to the existing
mobile error boundary. Swift maps that boundary to the stable coarse code and
an injected reporter. Diagnostic payloads never include raw post IDs, namespace
IDs, Willow paths, keys, signed bytes, or user-authored content. Redaction tests
assert that those values cannot appear.

## Deterministic joined-community fixture

The interaction tests use two profiles:

1. an author creates River City Wire and publishes a post;
2. a reader imports/joins that community and becomes the active UI profile.

The fixture uses an isolated temporary profile and local bundles only—no relay,
network, production database, or shared user state. It proves the exact
joined-community authority path shown in the user’s screenshot rather than the
author’s own-space shortcut.

A test-only macOS launch argument selects this fixture. It is accepted only in
the UI-test build/runtime environment and cannot redirect a production launch.

## Test-first root-cause proof

The first executable RED test adds a macOS UI-test target to
`apps/macos/Riot.xcodeproj`, includes it in the `Riot-macOS` scheme, launches the
joined-community fixture, and:

1. locates `reaction.open-wire.support.<fixture-post>`;
2. clicks the visible control;
3. observes pending/busy state;
4. asserts count changes from 0 to 1 and selected becomes true;
5. clicks again and asserts count returns to 0 and selected becomes false.

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

- current-state path validation and legacy compatibility;
- rapid react/retract/react and clock rollback/relaunch;
- preflight rejection leaves store and inventory unchanged;
- no post-commit retryable rejection;
- bounded inventory under repeated toggles;
- fail-soft deterministic reaction projection at and above the cap;
- authoritative `reacted_by_me` across projection/relaunch;
- accepted, rejected, and committed-refresh-failed Swift results;
- pending isolation across kinds and posts;
- duplicate Front Page/Open Wire renderings sharing state;
- concurrent different-key operations and stale/out-of-order completions;
- retry and per-key error clearing;
- count formatting/pluralization;
- keyboard activation, hover/focus/pressed states, selected non-color cue,
  44-point hit targets, VoiceOver values, and failure announcement;
- privacy-safe diagnostic mapping and redaction.

## Visual verification

Native macOS screenshots are captured from the deterministic fixture at 900- and
1200-point window widths. iPhone checks cover a narrow width and an accessibility
Dynamic Type size.

The visual gate blocks release if:

- any of the four controls is absent, clipped, truncated, or horizontally
  scrollable;
- a pointer click within the visible control fails the UI test;
- a target is smaller than 44×44 points;
- normal desktop layout wraps when 900 points are available;
- constrained layout fails to use the complete two-by-two fallback;
- selected state is distinguishable only by color;
- pending or failure feedback does not appear within one rendered frame after
  the corresponding model transition.

If direct desktop `screencapture` is unavailable, the macOS UI test attaches
`XCUIScreen.main.screenshot()` to the result bundle and the image is exported
from the `.xcresult` for review. Headless capture failure is therefore not an
excuse to skip visual evidence.

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
- Rapid toggles and clock rollback always project the last accepted state.
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
