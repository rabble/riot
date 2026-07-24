# Compact Reaction Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `metaswarm:orchestrated-execution` to implement each work unit through
> IMPLEMENT, VALIDATE, ADVERSARIAL REVIEW, COMMIT. Steps use checkbox (`- [ ]`)
> syntax for tracking.

**Goal:** Make the four newswire reactions visibly interactive, compact,
always visible, asynchronous, accessible, and end-to-end clickable on macOS,
while fixing the existing post-commit inventory error and same-second clock
failure that can make a valid click appear inert.

**Architecture:** This is the user-authorized implementation override after the
three-round design gate. It deliberately keeps the deployed Newswire v1
timestamp/digest reaction path, avoiding the unreviewed `reaction-state`
wire-family rollout, immutable-evidence compaction, and old-peer incompatibility.
Core/FFI first make the existing write truthful and project `reacted_by_me`;
Swift then moves writes off the main actor into a serial writer, owns shared
per-reaction pending/error state, and renders a compact adaptive button row.

**Tech Stack:** Rust 2021, Willow/UniFFI, Swift 6, SwiftUI, XCTest/XCUITest,
macOS 14+, iOS, Gradle Android host compile.

---

## Override boundary and known retained risk

The human explicitly selected **override** by responding “implement it” after
the design gate’s override/defer/cancel checkpoint. This plan implements the
requested UX and the correctness fixes that are safe on the deployed wire
format. It does **not** introduce the proposed new current-state path or claim
that immutable historical reaction entries no longer consume the store’s
existing lifetime receipt/seen limits. That separate evidence-retention and
mixed-version protocol problem remains documented in the design review and must
not be disguised as solved here.

The excluded protocol work requires its own compatibility and provenance
review before it can change the deployed wire contract.

## User flow

```text
Ordinary newswire post
┌─────────────────────────────────────────────────────────┐
│ Report headline                                         │
│ Full report inside.                                     │
│ Read →                                                  │
│ [♥ 0] [✊︎ 0] [! 0] [◌ 0]                               │
│          compact; each target is at least 44×44          │
│                                                         │
│ Couldn’t save your reaction. Try again.  (only on fail) │
└─────────────────────────────────────────────────────────┘
```

1. The joined-community reader clicks one always-visible reaction.
2. Only that `(post, kind)` becomes busy; duplicate copies share the state.
3. The serial writer performs the blocking UniFFI write and reprojection off
   the main actor.
4. Success updates the authoritative count/selection. Failure restores the
   prior state and shows generic retry copy without exposing identifiers.
5. The same macOS control is pointer- and keyboard-activatable; VoiceOver gets
   the full name, count, selected state, and busy/failure feedback.

## Work-unit decomposition

| ID | Title | Depends on | Checkpoint |
| --- | --- | --- | --- |
| RX-001 | Truthful existing-wire reaction writes | — | Yes |
| RX-002 | Viewer-aware projection and bindings | RX-001 | Yes |
| RX-003 | Async shared reaction state model | RX-002 | Yes |
| RX-004 | Compact adaptive reaction controls | RX-003 | Yes |
| RX-005 | Native macOS rendered-click fixture | RX-004 | Yes |
| RX-006 | Cross-platform and coverage closure | RX-005 | Yes |

## API contracts

### Rust/FFI mutation invariant

`MobileProfile.toggle_newswire_reaction(...) -> Result<NewswireSignedRecord,
MobileError>` remains source-compatible.

- Every returned `Err` occurs before durable commit.
- Inventory count/byte admission and current-inventory completeness are checked
  before commit.
- A newly signed entry must plan as `JoinEffect::Winner`; a dominated write
  returns `InvalidInput` before commit.
- After commit, assignment of the exact pre-admitted inventory vector is
  infallible under the already-held `LocalProfile` critical section.

### Projection record

```rust
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewswireReactionTally {
    pub kind: String,
    pub count: u32,
    pub reacted_by_me: bool,
}
```

Core receives an optional viewer subspace while it still owns the per-author
winner map. Viewerless projection returns `false`; `MobileProfile` supplies its
current author.

### Swift writer

```swift
public struct ReactionKey: Hashable, Sendable {
    public let postID: String
    public let kind: ReactionKind
}

public struct ReactionWriteSnapshot: Sendable {
    public let projection: NewswireProjectionView
    public let revision: UInt64
}

public enum ReactionWriteResult: Sendable {
    case accepted(ReactionWriteSnapshot)
    case committedNeedsRefresh(active: Bool, revision: UInt64)
    case rejected(ReactionFailure)
    case cancelled
}

public protocol NewswireReactionWriting: Sendable {
    func setReaction(
        descriptorID: String,
        postID: String,
        kind: ReactionKind,
        active: Bool
    ) async -> ReactionWriteResult
}
```

`MobileProfileReactionWriter` is an actor that owns only the generated
thread-safe `MobileProfile`, calls the synchronous UniFFI write, reprojects, and
increments `revision`.

## Security considerations

| Boundary | Rule | Verification |
| --- | --- | --- |
| Reaction inputs | Closed `ReactionKind`; complete internal IDs only | Rust/Swift invalid-kind tests |
| Commit/inventory | Preflight equality and limits before mutation | fault/boundary tests |
| Main actor | No blocking UniFFI call on main | delayed-writer test observes busy |
| Diagnostics | Fixed public code/copy; no IDs, paths, signed bytes, or content | redaction test |
| UI fixture | Valid UUID selects isolated temp directory only | invalid-UUID UI test |
| Duplicate renderings | State keyed by full internal `(post, kind)` | shared-state unit test |

There are no external services, credentials, relay dependencies, or new
third-party libraries.

The diagnostic-redaction test injects an error whose description contains four
sentinels: a full post ID, a local database path, signed-record bytes, and post
body text. It asserts that `ReactionFailure.publicCode`, visible copy, the
sequence-numbered accessibility announcement, and captured reporter output
contain none of those sentinels. Only the fixed public code and fixed copy may
cross the model/UI boundary.

---

### RX-001: Truthful existing-wire reaction writes

**Files:**
- Modify: `crates/riot-core/src/willow/clock.rs`
- Modify: `crates/riot-core/src/newswire/entry.rs`
- Modify: `crates/riot-core/src/newswire/mod.rs`
- Modify: `crates/riot-core/src/session.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/src/newswire_ffi.rs`
- Test: `crates/riot-ffi/src/newswire_ffi.rs`

**Definition of done:**

- Inventory admission and exact live-ID equality occur before commit.
- A post-commit inventory failure is impossible.
- A dominated candidate is rejected before commit.
- Production clock preserves microseconds.
- A per-logical-reaction timestamp floor makes rapid existing-wire
  react/retract/react strictly ordered even when the wall clock repeats.

- [ ] **Step 1: Add RED clock tests**

Add unit tests in `clock.rs` that call `snapshot_from_unix_duration` with
`Duration::new(1_700_000_000, 123_456_000)` and assert:

```rust
let with_fraction =
    snapshot_from_unix_duration(Ok(Duration::new(1_700_000_000, 123_456_000))).unwrap();
let whole =
    snapshot_from_unix_duration(Ok(Duration::from_secs(1_700_000_000))).unwrap();
assert_eq!(with_fraction.unix_seconds, whole.unix_seconds);
assert_eq!(
    with_fraction.tai_j2000_micros - whole.tai_j2000_micros,
    123_456
);
```

- [ ] **Step 2: Run the RED clock test**

Run:

```sh
cargo test -p riot-core willow::clock::tests::system_time_adapter_preserves_microseconds -- --exact
```

Expected: FAIL because `duration.as_secs()` discards the fractional value.

- [ ] **Step 3: Preserve the one-read fractional instant**

Refactor the internal converter to accept a `Duration`, construct
`hifitime::Epoch::from_unix_seconds(duration.as_secs_f64())`, and keep
`unix_seconds = duration.as_secs()`. Reject seconds beyond `i64::MAX` before the
floating conversion.

- [ ] **Step 4: Run the focused clock suite**

Run:

```sh
cargo test -p riot-core willow::clock::tests
```

Expected: PASS.

- [ ] **Step 5: Add RED preflight tests around `import_signed_newswire`**

In the existing `newswire_ffi.rs` test module add tests that:

1. fill `sync_inventory` to `MAX_SYNC_IDS` with live entries, attempt a new
   reaction, and assert receipt count, live IDs, generation, and inventory are
   unchanged;
2. deliberately desynchronize inventory from active live IDs, attempt a
   reaction, and assert the same unchanged snapshot;
3. submit a candidate dominated by an existing same-coordinate entry and assert
   no receipt is created.

Use the existing in-crate access to `with_active`, `receipt_count`,
`live_entry_ids`, and `sync_inventory`; do not expose a test-only production
API.

- [ ] **Step 6: Add a RED rapid-toggle projection test**

Freeze the input clock to one snapshot, then drive `active: true`, `false`,
`true` for the same author/post/kind. Assert the three signed entry timestamps
are strictly increasing and the final projected Support count is one. The test
must fail against `create_signed_news_reaction`, whose three calls can share one
timestamp.

- [ ] **Step 7: Run the RED FFI tests**

Run:

```sh
cargo test -p riot-ffi newswire_ffi::tests::reaction_preflight -- --nocapture
```

Expected: FAIL because `import_signed_newswire` currently commits before
`track_committed_entry`, and no logical-reaction timestamp floor exists.

- [ ] **Step 8: Add explicit-snapshot reaction signing**

In `entry.rs`, add `create_signed_news_reaction_at` taking a `ClockSnapshot`;
it performs the same authority check and calls `build_signed`. Keep
`create_signed_news_reaction` as the production wrapper that passes
`system_snapshot()`. Re-export the new function from `newswire/mod.rs`.

In `newswire_ffi.rs`, add `next_reaction_snapshot(profile, descriptor_id,
parent_id, kind)`. It takes one `system_snapshot`, scans the bounded held
newswire records for the same signer/descriptor/parent/kind, and raises only
`tai_j2000_micros` to `max(system, previous + 1)`. Overflow maps to
`MobileError::ClockUnavailable`. Call the explicit-snapshot signer while still
inside `with_active`, so two callers cannot select the same floor.

- [ ] **Step 9: Expose preflight/install helpers crate-locally**

Change only visibility in `mobile_state.rs`:

```rust
pub(crate) fn prospective_sync_inventory(...)
    -> Result<Vec<SignedWillowEntry>, MobileError>

pub(crate) fn active_namespace_live_ids(...)
    -> Result<Vec<EntryId>, MobileError>

pub(crate) fn install_sync_inventory(...)
    -> Result<(), MobileError>
```

Add a pure helper that compares the current inventory IDs with the current live
IDs before any write:

```rust
pub(crate) fn ensure_complete_sync_inventory(
    profile: &LocalProfile,
) -> Result<(), MobileError>
```

Reuse the existing equality semantics; never relax equality to subset/superset.

- [ ] **Step 10: Preflight the exact prospective inventory and join effect**

In `import_signed_newswire`:

1. call `ensure_complete_sync_inventory(profile)`;
2. compute `next_inventory` before inspection/commit;
3. inspect/plan;
4. call a new `ImportPlan::preflight_effects()` method in `session.rs`; this
   method performs the same live-plan/generation checks as `commit_inner`,
   computes `plan_join_with_payloads` against a cloned pre-state, and returns
   the resulting `Vec<(EntryId, JoinEffect)>` without consuming or mutating the
   plan;
5. require `JoinEffect::Winner` or a live `AlreadyPresent`; reject `NotLive`;
6. commit;
7. for `Committed`, compare the preflight-predicted live IDs with the
   pre-admitted inventory IDs before Step 6, then perform the post-commit
   `profile.sync_inventory = next_inventory` assignment directly; it has no
   fallible operation;
8. for `NoChanges`, require the duplicate result’s `all_entry_ids_live` and
   leave inventory unchanged.

Add a focused session test proving `preflight_effects()` leaves generation,
receipt count, live IDs, and plan liveness unchanged before the subsequent
commit. Add a public read-only `DuplicateResult::all_entry_ids_live()` getter so
FFI never reaches across the crate boundary to a private field.

- [ ] **Step 11: Run RX-001 validation**

Run:

```sh
cargo fmt --all -- --check
cargo clippy -p riot-core -p riot-ffi --all-features --all-targets -- -D warnings
cargo test -p riot-core willow::clock
cargo test -p riot-ffi newswire_ffi
```

Expected: all PASS.

- [ ] **Step 12: Adversarial review and commit**

Review the RX-001 diff against its DoD. On PASS:

```sh
git add crates/riot-core/src/willow/clock.rs \
  crates/riot-core/src/newswire/entry.rs \
  crates/riot-core/src/newswire/mod.rs \
  crates/riot-core/src/session.rs \
  crates/riot-ffi/src/mobile_state.rs \
  crates/riot-ffi/src/newswire_ffi.rs
git commit -m "fix(newswire): preflight reaction writes before commit"
```

Human checkpoint: confirm the deployed wire format did not change.

---

### RX-002: Viewer-aware projection and generated bindings

**Files:**
- Modify: `crates/riot-core/src/newswire/projection.rs`
- Modify: `crates/riot-core/src/newswire/store.rs`
- Modify: `crates/riot-core/src/newswire/mod.rs`
- Modify: `crates/riot-ffi/src/newswire_ffi.rs`
- Modify: `apps/ios/RiotTests/NewswireSurfaceTests.swift`
- Generated: `build/generated/riot-ffi/**`

**Definition of done:**

- Core computes viewer state without exposing reactor IDs.
- FFI exports `reacted_by_me`.
- Swift selection and next direction survive a fresh model/reprojection.
- Swift and Android generated consumers compile.

- [ ] **Step 1: Add RED core projection tests**

Extend the existing reaction fixtures with:

```rust
assert!(tally_for(&viewer_projection, post, ReactionKind::Support).reacted_by_viewer);
assert!(!tally_for(&other_projection, post, ReactionKind::Support).reacted_by_viewer);
assert!(!tally_for(&viewerless_projection, post, ReactionKind::Support).reacted_by_viewer);
```

Cover an active winner and a later retraction.

- [ ] **Step 2: Run the RED core tests**

Run:

```sh
cargo test -p riot-core newswire::projection::tests::viewer_reaction_state -- --nocapture
```

Expected: compile/test failure because the viewer-aware API and field do not
exist.

- [ ] **Step 3: Add viewer-aware pure projection**

Add:

```rust
pub fn project_for_viewer(
    descriptor: &VerifiedNewswireRecord,
    records: &[VerifiedNewswireRecord],
    clock: ProjectionClockV1,
    viewer: Option<[u8; 32]>,
) -> Result<NewswireProjection, NewswireProjectionError>
```

Keep `project(...)` as a wrapper passing `None`. While iterating
`latest_reactions`, populate a `BTreeSet<(parent, kind)>` for active keys whose
signer equals `viewer`. Add `reacted_by_viewer: bool` to every emitted tally.

- [ ] **Step 4: Add the store and FFI viewer seam**

Add `project_space_for_viewer(...)` in `store.rs`; retain `project_space` as the
viewerless wrapper. In `MobileProfile::project_newswire_space`, pass
`Some(*profile.author.subspace_id().as_bytes())` and map:

```rust
NewswireReactionTally {
    kind: reaction_kind_name(tally.kind).to_string(),
    count: tally.count,
    reacted_by_me: tally.reacted_by_viewer,
}
```

- [ ] **Step 5: Add RED Swift relaunch tests**

Update tally constructors with `reactedByMe:` and add a model test that loads a
projection with Support selected, constructs a fresh model, and asserts:

```swift
XCTAssertTrue(model.isReacted(post: "p1", kind: .support))
```

The next write must request `active == false`.

- [ ] **Step 6: Make the model read authoritative selection**

Remove the session-local `reactedByMe` set. `isReacted` reads the matching
tally’s `reactedByMe`; the successful writer projection becomes the only source
of count and selected state.

- [ ] **Step 7: Regenerate and compile bindings**

Run:

```sh
cargo run --locked -p xtask -- generate-bindings
sh scripts/conference/build-native-core.sh
xcodebuild test -project apps/macos/Riot.xcodeproj \
  -scheme RiotKit-macOS -destination 'platform=macOS'
(cd apps/android && ./gradlew :app:testDebugUnitTest :app:compileDebugKotlin)
```

Expected: all PASS. Generated files remain build artifacts and are not
committed; the tracked Android controller consumes the regenerated type without
constructing `NewswireReactionTally` directly.

- [ ] **Step 8: Adversarial review and commit**

On PASS:

```sh
git add crates/riot-core/src/newswire \
  crates/riot-ffi/src/newswire_ffi.rs \
  apps/ios/RiotTests/NewswireSurfaceTests.swift
git commit -m "feat(newswire): project the current profile reaction state"
```

Human checkpoint: fresh-model selection and Android compile are green.

---

### RX-003: Async shared reaction state model

**Files:**
- Create: `apps/ios/Riot/NewswireReactionWriter.swift`
- Modify: `apps/ios/Riot/NewswireEditorial.swift`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/RiotTests/NewswireSurfaceTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`

**Definition of done:**

- Blocking UniFFI work is off the main actor.
- Pending/failure state is shared by `ReactionKey`.
- Same-key duplicate taps are suppressed.
- Different keys remain independently usable.
- Older projection revisions cannot overwrite newer ones.
- Failure copy appears and clears only for the failed key.
- A committed write followed by failed projection is never shown as rejected.
- Queued work cancels on teardown; an already-started commit may finish but
  cannot mutate a stale model.

- [ ] **Step 1: Add RED async model tests**

Create a controllable actor fake implementing `NewswireReactionWriting`. Tests
must cover:

```swift
await model.toggleReaction(post: row, kind: .support, surface: .openWire)
XCTAssertTrue(model.isPending(key))
XCTAssertFalse(model.isPending(otherKey))
```

Hold completion with `CheckedContinuation`, assert duplicate same-key calls stay
at one, release with revision 1, and assert pending clears. Add rejection,
committed-needs-refresh, different-key, duplicate-surface, stale-revision,
queued-cancellation, completion-after-teardown, no-writer, and empty-projection
tests. Add the sentinel diagnostic-redaction test from the security table and
the committed-needs-refresh → accepted reconciliation/clearing sequence.

- [ ] **Step 2: Run the RED Swift model tests**

Run:

```sh
xcodebuild test -project apps/macos/Riot.xcodeproj \
  -scheme RiotKit-macOS -destination 'platform=macOS' \
  -only-testing:RiotKitTests-macOS/NewswireSurfaceTests
```

Expected: compile failure because the async writer/state types do not exist.

- [ ] **Step 3: Implement `NewswireReactionWriter.swift`**

Define `ReactionKey`, `ReactionSurface`, `ReactionFailure`, the async protocol,
and:

```swift
public actor MobileProfileReactionWriter: NewswireReactionWriting {
    private let profile: MobileProfile
    private var revision: UInt64 = 0

    public func setReaction(...) async -> ReactionWriteResult {
        if Task.isCancelled { return .cancelled }
        do {
            _ = try profile.toggleNewswireReaction(...)
        } catch {
            return .rejected(ReactionFailure(error))
        }
        revision &+= 1
        do {
            let projection = try profile.projectNewswireSpace(...)
            return .accepted(
                ReactionWriteSnapshot(projection: projection, revision: revision)
            )
        } catch {
            return .committedNeedsRefresh(active: active, revision: revision)
        }
    }
}
```

The actor serializes the generated thread-safe handle; it does not capture the
mutable repository.

- [ ] **Step 4: Move model state to typed keys**

Make `ReactionKind` conform to `Hashable`. In `NewswireSurfaceModel` add
published `pending`, `failures`, `reactionAnnouncements`, and
`lastAppliedReactionRevision`, an ephemeral
`committedSelectionOverrides: [ReactionKey: Bool]`, plus owned keyed `Task`
values. `isReacted` consults a committed override first and authoritative
projection second. `toggleReaction`
must:

1. ignore an already-pending key;
2. clear only that key’s prior failure;
3. insert pending before creating the task;
4. request the inverse of authoritative `reacted_by_me`;
5. apply only revisions newer than the current revision;
6. clear pending in every outcome;
7. on `.committedNeedsRefresh`, set the known selected direction, retain the old
   count, and show `Reaction saved. Count will update when the wire refreshes.`;
8. retain generic copy `Couldn’t save your reaction. Try again.` only on
   pre-commit rejection;
9. emit one sequence-numbered accessibility announcement for the initiating
   surface.

The next accepted projection clears every committed override it authoritatively
replaces **and** clears the corresponding saved-but-refresh-needed informational
message. Add a result-sequence test for
`.committedNeedsRefresh(active: true, revision: 4)` followed by an accepted
projection at revision 5: the temporary selected override and informational
copy are both gone, and the projection remains selected.

- [ ] **Step 5: Define teardown behavior**

Add `cancelReactionTasks()` to cancel and remove every owned task and increment
the model generation. Every completion compares its captured generation before
applying state. A cancelled queued actor request returns `.cancelled`; an
already-started synchronous call completes but its stale result is ignored.

- [ ] **Step 6: Wire the real writer at the actual construction site**

Expose `makeNewswireReactionWriter()` on `RiotProfileRepository`. In
`ConferenceShellView`’s `CommunityShellView.init`, obtain the writer from
`model.profileRepository` and pass it to `NewswireSurfaceModel`. Call
`newswire.cancelReactionTasks()` from the community surface’s `onDisappear`.
Keep the synchronous repository reaction method for non-view callers.

- [ ] **Step 7: Add both new source references**

Add `NewswireReactionWriter.swift` to the iOS Riot/RiotKit build phases and the
macOS by-reference RiotKit source phase. Do not duplicate the file.

- [ ] **Step 8: Run RX-003 validation**

Run the focused macOS and iOS `NewswireSurfaceTests`. Expected: PASS with no
sleep-based assertions.

- [ ] **Step 9: Adversarial review and commit**

On PASS:

```sh
git add apps/ios/Riot/NewswireReactionWriter.swift \
  apps/ios/Riot/NewswireEditorial.swift \
  apps/ios/Riot/Core/ProfileRepository.swift \
  apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/RiotTests/NewswireSurfaceTests.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/macos/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ui): make newswire reactions asynchronous and observable"
```

---

### RX-004: Compact adaptive reaction controls

**Files:**
- Create: `apps/ios/Riot/Design/CompactReactionBar.swift`
- Modify: `apps/ios/Riot/Design/RiotTheme.swift`
- Modify: `apps/ios/Riot/NewswireEditorial.swift`
- Create: `apps/ios/RiotTests/CompactReactionBarTests.swift`
- Modify: both Apple project files to include the shared sources/tests

**Definition of done:**

- Four controls are always visible when reacting is available.
- Exact glyphs are `♥`, `✊︎`, `!`, `◌`.
- Counts including zero are visible.
- Layout and targets are state-invariant and accessible.
- Selected, pending, failure, hover, press, focus, and disabled states exist.
- Failure is visible; buttons no longer look like tags.
- Failure/success is announced once without moving keyboard/VoiceOver focus.

- [ ] **Step 1: Add RED presentation tests**

Test the closed presentation table, count formatting, 44-point minimum target,
state-invariant width, accessibility strings, and one-shot announcement
sequence:

```swift
XCTAssertEqual(ReactionKind.allCases.map(\.glyph), ["♥", "✊︎", "!", "◌"])
XCTAssertEqual(ReactionCountFormatter.string(1_000), "999+")
XCTAssertEqual(CompactReactionMetrics.minimumTarget, 44)
```

- [ ] **Step 2: Run the RED component tests**

Expected: compile failure because the component types do not exist.

- [ ] **Step 3: Implement scalable fixed-slot metrics**

Use `@ScaledMetric` for glyph/count sizes and reserve fixed glyph/spinner and
check slots in every state. At normal sizes each visual capsule is 72 points
wide × 32 high inside a 44-point target. At accessibility sizes visual height
grows to the scaled content plus 12 points and the target grows with it.

The single-row breakpoint is `4 × 72 + 3 × 8 = 312` points. Available width
below 312 uses a two-column grid; therefore the specified 288-point iPhone card
is deterministically 2×2. Count, selected check, and spinner never change the
reserved width.

- [ ] **Step 4: Add semantic colors**

Add `onReactionAccent` and `danger` to `RiotTheme` using the design’s exact
light/dark hex values. Add contrast tests for selected foreground and danger
against adjacent surfaces.

- [ ] **Step 5: Replace the existing tag-like bar**

Render `CompactReactionBar` after Read, passing model-derived count, selected,
pending, and failure values. Each button launches the async model action with
its surface. Use ephemeral row tokens for accessibility IDs; never expose the
post ID.

Add `ReactionAccessibilityAnnouncementHost`: it observes the model’s
sequence-numbered announcement for its own surface and posts
`AccessibilityNotification.Announcement(message)` once. The button remains in
place and retains focus during pending/failure. Tests assert a duplicate post
produces one initiating-surface announcement, not two.

Every control also applies `.help(kind.label)` so pointer hover reveals the
full persistent name (`Support`, `Solidarity`, `Important`, or `Grief`) after
the one-time legend has been dismissed. The macOS rendered test hovers each
control and asserts its help element exposes the corresponding full name.

- [ ] **Step 6: Add the one-time legend**

Store one app-install-scoped boolean in `UserDefaults` under
`riot.reactionLegendDismissed.v1`. Only the first eligible row in the active
model renders the legend, even when a post is duplicated. The legend includes a
plain `Got it` button that writes `true` and removes the legend. Tests use an
injected suite, activate `Got it`, and assert dismissal survives model
recreation.

- [ ] **Step 7: Validate layout and accessibility**

The unit suite tests the pure presentation/metrics decisions at 288- and
500-point proposed widths and `accessibility3`: 2×2 versus 1×4, fixed slot
widths, count strings, the four stable identifiers, and full
label/value/hint/help strings. It also exercises the no-writer and empty
projection branches: no writer omits the bar; an eligible empty tally supplies
four zero-count controls.

Runtime SwiftUI assertions belong to RX-005, where XCUITest can inspect the
rendered accessibility tree without a third-party view-inspection library. That
suite asserts no horizontal scroll, four buttons, selected trait/value,
busy/disabled semantics, hover help, and keyboard focus retention. Run the
unit tests with:

```sh
xcodebuild test -project apps/macos/Riot.xcodeproj \
  -scheme RiotKit-macOS -destination 'platform=macOS' \
  -only-testing:RiotKitTests-macOS/CompactReactionBarTests
```

- [ ] **Step 8: Adversarial review and commit**

After PASS:

```sh
git add apps/ios/Riot/Design/CompactReactionBar.swift \
  apps/ios/Riot/Design/RiotTheme.swift \
  apps/ios/Riot/NewswireEditorial.swift \
  apps/ios/RiotTests/CompactReactionBarTests.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/macos/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ui): render compact interactive newswire reactions"
```

---

### RX-005: Native macOS rendered-click fixture

**Files:**
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot.xcodeproj/xcshareddata/xcschemes/Riot-macOS.xcscheme`
- Create: `apps/macos/RiotUITests/ReactionControlsUITests.swift`
- Create: `apps/ios/Riot/Demo/ReactionUITestFixture.swift`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/NewswireReactionWriter.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/Riot/RiotApp.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Create: `apps/ios/RiotTests/CompactReactionBarNativeSnapshotTests.swift`
- Modify: `apps/macos/Riot/RiotMacApp.swift`
- Modify: `apps/macos/README.md`

**Definition of done:**

- A real macOS XCUITest clicks Support through SwiftUI → model → UniFFI → core.
- The fixture is a two-profile joined-community flow.
- Pending is deterministic; success changes count/selection.
- Invalid fixture activation cannot touch production storage.
- Screenshots exist for 900- and 1200-point windows.
- Native iOS screenshots exist at an exact 320-point host width at normal and
  accessibility Dynamic Type.

- [ ] **Step 1: Add the RED UI target and test**

Create a macOS UI-test target and scheme TestAction. The first test launches
with a UUID and `RIOT_UI_TEST_FIXTURE=reactions-joined`, finds
`reaction.open-wire.support.fixture-post-1`, clicks it, and expects value
`1 reaction, selected`.

Add the target dependency, source build-phase member, product, and
`TargetAttributes` entries to the macOS project; add its testable reference to
the `Riot-macOS` scheme. Run:

```sh
xcodebuild test -project apps/macos/Riot.xcodeproj \
  -scheme Riot-macOS -destination 'platform=macOS' \
  -only-testing:RiotUITests-macOS/ReactionControlsUITests \
  -resultBundlePath build/ui-reactions-red.xcresult
```

Expected: FAIL because fixture bootstrap does not exist.

- [ ] **Step 2: Implement the isolated joined fixture**

Move the existing UUID-derived wrapping-key helper from private `RiotApp.swift`
scope into `ReactionUITestFixture.swift` and reuse it on both platforms.
`RiotMacApp` mirrors the iOS UUID temp-directory bootstrap instead of calling
plain `model.bootstrap()`.

Implement an internal, DEBUG-only
`RiotProfileRepository.makeReactionUITestPair(baseDirectory:keyStore:)` so it
can use its private `MobileProfile` handles without widening production APIs.
It opens `author/` and `reader/` repositories, the author creates River City
Wire and `fixture-post-1`, and the reader joins the returned public-space
namespace with the returned descriptor entry ID. A bounded
`ReactionFixtureSyncPump` opens both repositories’ existing
`MobileSyncSessionBoundary` values, begins only the author side, alternately
drains `takeOutboundFrame()` into the opposite side’s `receive(_:)`, calls
`acceptImport()` whenever either side reports preview-ready, and stops on both
terminal outcomes or fails after 64 transfers. It then asserts the reader’s
projection contains `fixture-post-1` and returns the reader repository plus the
fixture IDs. `RiotAppModel.bootstrapReactionUITestFixture` installs that reader
repository through an internal DEBUG-only method and calls its normal `reload`;
the fixture never assigns or reflects the private profile directly. No network
or relay is used.

Add focused repository tests for the pump’s begin/answer asymmetry, the 64-frame
cap, import acceptance, and the final reader projection before connecting it to
the app entry points.

- [ ] **Step 3: Add deterministic pending control**

The runner supplies `RIOT_UI_TEST_REACTION_DELAY_MS=750`.
`ConferenceShellView` passes a validated DEBUG-only fixture configuration into
`MobileProfileReactionWriter`; the writer honors the delay only when both the
reactions fixture flag and a valid UUID exist, then performs the real UniFFI
call automatically. A second closed fixture mode injects the same actor with a
deterministic pre-commit rejection. XCUITest has a deterministic
750-millisecond window to observe busy without cross-sandbox IPC.

- [ ] **Step 4: Prove add and remove**

The UI test asserts 0→1 selected, clicks again, and asserts 1→0 unselected.
Add pointer and keyboard activation cases and a rejected-writer fixture showing
the retry copy. Before activation, focus Support with Tab and assert
`hasKeyboardFocus == true`; assert the same focused element remains focused
while its value is busy, after the success announcement, and after a rejected
write. Hover all four buttons and assert the exposed help/tooltip contains the
full reaction name. These rendered assertions also prove there is no horizontal
scroll and that all four controls remain in the accessibility tree.

- [ ] **Step 5: Prove invalid fixture activation fails closed**

Launch with `RIOT_UI_TEST_FIXTURE=reactions-joined` and an invalid/missing run
UUID. The app renders `ui-fixture-invalid` and does not call production
bootstrap. Assert neither fixture nor production community content appears.

- [ ] **Step 6: Capture macOS and iOS visual evidence**

For macOS, a valid reaction fixture accepts
`RIOT_UI_TEST_WINDOW_WIDTH=900|1200`. `RiotMacApp` applies that value only in the
UUID-gated fixture branch to both `.defaultSize` and a root test-only fixed
frame, so persisted production window state cannot change the requested size.
The UI test asserts `app.windows.firstMatch.frame.width` within one point,
attaches `app.windows.firstMatch.screenshot()`, and exports both images from the
`.xcresult`.

For iOS, add `CompactReactionBarNativeSnapshotTests`. It hosts the real
`CompactReactionBar` in a `UIHostingController`, lays the controller’s view out
at exactly `320 × 568` points, and renders with `UIGraphicsImageRenderer`. One
test uses the normal content-size category and one injects
`.accessibilityExtraExtraExtraLarge`; each asserts the host bounds are exactly
320 points and attaches the native PNG with `XCTAttachment`. This is a native
UIKit/SwiftUI screenshot, not a metrics-only assertion and does not depend on a
simulator model having a 320-point screen.

Run and retain the exact results:

```sh
RIOT_UI_TEST_WINDOW_WIDTH=900 xcodebuild test \
  -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS' \
  -only-testing:RiotUITests-macOS/ReactionControlsUITests/testReactionAt900 \
  -resultBundlePath build/reaction-900.xcresult

RIOT_UI_TEST_WINDOW_WIDTH=1200 xcodebuild test \
  -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS' \
  -only-testing:RiotUITests-macOS/ReactionControlsUITests/testReactionAt1200 \
  -resultBundlePath build/reaction-1200.xcresult

xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 16 Pro Max,OS=26.2' \
  -only-testing:RiotTests/CompactReactionBarNativeSnapshotTests \
  -resultBundlePath build/reaction-ios-320.xcresult
```

Update README with those commands and exported artifact locations.

- [ ] **Step 7: Adversarial review and commit**

Commit the UI target/fixture only after the native test passes twice
consecutively without sleeps.

```sh
git add apps/macos/Riot.xcodeproj \
  apps/macos/RiotUITests/ReactionControlsUITests.swift \
  apps/ios/Riot/Demo/ReactionUITestFixture.swift \
  apps/ios/Riot/Core/ProfileRepository.swift \
  apps/ios/Riot/AppModel.swift \
  apps/ios/Riot/NewswireReactionWriter.swift \
  apps/ios/Riot/ConferenceShellView.swift \
  apps/ios/Riot/RiotApp.swift \
  apps/ios/Riot.xcodeproj/project.pbxproj \
  apps/ios/RiotTests/CompactReactionBarNativeSnapshotTests.swift \
  apps/macos/Riot/RiotMacApp.swift \
  apps/macos/README.md
git commit -m "test(ui): prove macOS reaction controls click through core"
```

---

### RX-006: Cross-platform and coverage closure

**Files:**
- No production files. A failure returns to the owning work unit; RX-006 never
  patches around a failed gate.
- `.coverage-thresholds.json` remains unchanged unless measured coverage has
  increased enough to raise a floor in a separately reviewed commit.

- [ ] **Step 1: Run formatting, contracts, and strict Rust gates**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
cargo xtask validate-contracts
```

- [ ] **Step 2: Run binding and Android gates**

```sh
cargo run --locked -p xtask -- generate-bindings
(cd apps/android && ./gradlew :app:testDebugUnitTest :app:compileDebugKotlin)
```

- [ ] **Step 3: Run both Apple gates**

```sh
sh scripts/conference/build-native-core.sh
xcodebuild test -project apps/macos/Riot.xcodeproj \
  -scheme Riot-macOS -destination 'platform=macOS'
xcodebuild test -project apps/ios/Riot.xcodeproj \
  -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 16 Pro Max,OS=26.2'
```

- [ ] **Step 4: Run the coverage source of truth**

```sh
scripts/web/coverage.sh
```

Expected: every tool meets the floor in `.coverage-thresholds.json`.

- [ ] **Step 5: Perform native visual review**

Inspect exported 900/1200 macOS and 320/accessibility iOS screenshots. Block on
missing controls, tag-like appearance, clipping, horizontal scrolling,
sub-44-point targets, color-only selection, or missing busy/failure feedback.

- [ ] **Step 6: Final adversarial review and integration commit**

Review the complete diff against the user flow and every WU DoD. Record the
known override boundary in the PR: the deployed legacy immutable reaction
history ceiling remains and no mixed-version wire change was introduced.

## Human checkpoints

Pause after every work unit. RX-001 and RX-002 require protocol/data review;
RX-004 requires visual approval; RX-005 requires reviewing the real screenshot;
RX-006 requires explicit approval to open/merge a PR.
