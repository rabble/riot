# Compact Newswire Reactions — Design

**Date:** 2026-07-24  
**Status:** User-approved visual direction; awaiting written-spec approval  
**Scope:** Shared SwiftUI newswire surface on iOS and macOS

## Problem

The four communal reactions under each ordinary newswire post—Support,
Solidarity, Important, and Grief—look like inert categories or hashtags. In the
real macOS River City Wire surface, tapping them appears to do nothing.

The current view does create a `Button` for every reaction, but it discards the
`NewswireReactionOutcome` returned by `toggleReaction`. A rejected core/FFI
operation is therefore silent and indistinguishable from a hit-testing failure.
The existing tests call the model directly with fake reactors; none proves that
the rendered macOS control receives a click or that a real joined-community
reaction reaches core.

## Product intent

Reactions are small, signed communal actions—not post categories and not freeform
hashtags. They should remain secondary to reading the report while being visibly
interactive and immediately understandable once used.

The user selected the compact-icon direction and required that the controls
remain visible rather than appearing only on hover or focus.

## Interaction design

Every ordinary post with reaction authority shows one compact, always-visible
row after the Read action:

| Meaning | Compact control |
| --- | --- |
| Support | heart icon + count |
| Solidarity | raised-fist icon + count |
| Important | exclamation icon + count |
| Grief | mourning/tear icon + count |

The row does not add a large heading or another card. Each control:

- is a real button whose entire visible shape is hit-testable;
- has a minimum 44-point interaction target without visually becoming a large
  pill;
- shows its count, including zero, so all four controls have a stable layout;
- uses the existing pink accent and selected trait when this device's reaction
  is active;
- exposes the full reaction name in macOS hover help and in its accessibility
  label;
- shows hover, pressed, keyboard-focus, pending, selected, and disabled states;
- blocks duplicate taps while its write is pending.

The fixed icon-to-meaning mapping is centralized alongside `ReactionKind`, not
redeclared in the view. The mapping uses native symbols where they communicate
the meaning; any non-native glyph must render consistently on both target
platforms and retain the full textual accessibility label.

## State and data flow

A tap sends one `(post, reaction kind)` toggle through
`NewswireSurfaceModel.toggleReaction`.

1. The view marks that control pending and prevents another tap.
2. The model calls the wired `NewswireReacting` implementation.
3. Core signs and commits `active = true` or `active = false`.
4. On `.reacted` or `.retracted`, the model reloads projection tallies and the
   control updates its selected state and count.
5. Pending state clears regardless of outcome.

Only the tapped control is pending. Other posts and reaction kinds remain usable.
No optimistic count is shown before core accepts the write.

The current session-local `reactedByMe` behavior remains in scope for this fix.
Persisting per-viewer selection across relaunch would require a separate core
projection/API design and is explicitly deferred.

## Failure behavior

The view must handle every `NewswireReactionOutcome`; it may not discard the
result.

- `.reacted` / `.retracted`: clear any prior error for that post.
- `.rejected`: show compact inline copy under that post:
  **“Couldn’t save your reaction. Try again.”**
- `.unavailable`: render the controls disabled only when authority can disappear
  after construction; otherwise omit the row, matching the existing `canReact`
  gate.

The rejected state does not change the selected appearance or count. A later tap
retries. The UI does not expose namespace IDs, descriptor IDs, FFI errors, or
other protocol language.

For diagnosis and tests, the real underlying error must not disappear before it
can be logged or asserted. User-facing copy remains generic and retryable.

## Root-cause verification

Before changing production presentation code, add a failing interaction test
that distinguishes the two possible failure layers:

1. prove the rendered Support control invokes the model;
2. prove a real joined-community repository call is accepted and changes the
   projected tally;
3. if the rendered action does not fire, repair hit testing;
4. if it fires and core rejects, capture the concrete error and fix that
   authority/data-path defect rather than masking it with UI state.

This test-first diagnosis is blocking. The implementation must not assume the
failure is purely visual.

## Accessibility

Each control is exposed as a button with:

- label: the complete reaction name;
- value: “0 people”, “1 person”, or “N people”;
- selected trait when active;
- disabled/busy semantics while pending;
- a stable per-post accessibility identifier.

Keyboard activation on macOS must behave identically to pointer activation.
Color is not the only selected-state signal.

## Scope

### In scope

- Compact always-visible icon reaction controls.
- Hover, pressed, focus, pending, selected, and rejected states.
- Visible retryable failure copy.
- Root-cause diagnosis of the real macOS click path.
- Shared iOS/macOS model, view, and tests needed for the behavior.
- Native visual verification at the macOS layout shown by the user.

### Out of scope

- New reaction kinds or protocol changes.
- Freeform hashtags, topic filtering, or category navigation.
- Editorial actions or reply redesign.
- Rebuilding the newswire card hierarchy.
- Durable “reacted by me” projection across relaunch.
- The separate sidebar/header sync-freshness disagreement visible in the
  screenshot.

## Verification and acceptance

- The visible Support control can be activated in the real macOS surface.
- Acceptance changes its selected state and increments the projected count.
- A second activation retracts it and decrements the count.
- A rejected write shows the compact inline retry message and does not change the
  count or selection.
- The complete visible capsule/icon area is clickable.
- All four controls stay visible with stable ordering and counts.
- Hover help, keyboard activation, VoiceOver labels, values, and selected state
  are correct.
- Existing Rust reaction projection/authority tests remain green.
- Swift model and presentation tests cover every outcome.
- A native macOS screenshot confirms that the compact row does not overpower the
  report content.
