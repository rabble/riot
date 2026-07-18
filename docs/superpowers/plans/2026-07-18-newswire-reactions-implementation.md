# Newswire Reactions — Implementation Plan

> Execution: mirror the **comments** family, which is fully built at every layer and is the exact
> template. When in doubt, grep `comment`/`NewsComment`/`commentsByParent` and add the reaction
> parallel beside it.

**Goal:** communal reactions on newswire posts — a closed set of reaction kinds, per-author toggle,
a per-post tally shown under each post. No new trust model: reactions are communal like posts/comments,
author-controlled (toggle), moderated per content if ever needed.

**Status at plan time:** Layer 1 (core) built overnight on `overnight/2026-07-18-collab` (see
OVERNIGHT_LOG Task 2 + the coder-agent output for exact signatures). Layers 2–3 below are the
remaining work. Layer 2 (FFI) is pure Rust → verifiable + committable. Layer 3 (iOS) needs Xcode
(`green.sh`) → do on a build machine.

---

## Layer 1 — Core (DONE overnight; verify before building on it)
`crates/riot-core/src/newswire/`. `NewsReactionV1 { space_descriptor_entry_id, parent_entry_id,
kind: ReactionKind, active: bool }`; `ReactionKind` closed = Support/Solidarity/Important/Grief;
`REACTION_SCHEMA = "org.riot.newswire.reaction/1"`; encode/decode/validate/`create_signed_news_reaction`
+ `_with_clock`; `NewswirePayload::NewsReaction`; path family `reactions` in `path.rs`; store prefix
scan + payload arm in `store.rs`; projection tally (distinct active authors per kind, latest-wins
per (author,parent,kind)) attached to the projected post in `projection.rs`; re-exports in `mod.rs`.
**Verify:** `cargo test -p riot-core --all-features` green; `cargo clippy -p riot-core --all-features
-- -D warnings`. Read the coder-agent output for the EXACT projected-post field name/shape (needed by
Layer 2).

## Layer 2 — FFI (`crates/riot-ffi/src/newswire_ffi.rs`) — pure Rust, verifiable

### Task 2.1 — Expose the projected tally on the FFI post
- **Files:** `crates/riot-ffi/src/newswire_ffi.rs`
- `NewswireProjectedPost` (~L146) gains a reactions field mirroring how core exposes it — e.g.
  `pub reactions: Vec<NewswireReactionTally>` where
  `pub struct NewswireReactionTally { pub kind: String, pub count: u32, pub reacted_by_me: bool }`
  (`reacted_by_me` only if core exposes viewer identity in projection; if not, omit for v1 and add
  later — DO NOT invent a viewer seam here).
- Populate it where the view is built (~L442, beside `comments: projection.comments…`) by mapping
  the core tally. `kind` → its lowercase stable string ("support"/"solidarity"/"important"/"grief").
- **TDD:** extend the existing newswire_ffi projection test to assert a post carries its tally after a
  reaction is signed. `cargo test -p riot-ffi --all-features`.

### Task 2.2 — `toggle_newswire_reaction`
- **Files:** `crates/riot-ffi/src/newswire_ffi.rs`
- Mirror `create_newswire_comment` (L352): load descriptor, parse ids, build `NewsReactionV1`, sign
  via `create_signed_news_reaction`, admit. Signature:
  ```rust
  pub fn toggle_newswire_reaction(
      &self,
      space_descriptor_entry_id: String,
      parent_entry_id: String,
      kind: String,      // parse to ReactionKind; reject unknown → MobileError
      active: bool,
  ) -> Result<NewswireSignedRecord, MobileError>
  ```
- Parse `kind` string → `ReactionKind` with a closed match; unknown string → the existing
  invalid-input `MobileError`.
- **UDL/checksum:** adding this fn + struct changes the UniFFI interface. Per
  `riot-uniffi-record-change-coupling`, the generated bindings AND the native staticlibs MUST be
  regenerated together (`scripts/conference/build-native-core.sh`) or the app aborts at runtime with a
  checksum mismatch — this is why Layer 3 is a build-machine task, not a commit-blind task.
- **TDD:** a Rust FFI test: toggle on → tally count 1; toggle same author/kind off (active:false) →
  count 0; unknown kind → error. `cargo test -p riot-ffi --all-features`.

### Task 2.3 — Repo seam (Swift side of FFI, but compiles under RiotKit only)
- **Files:** `apps/ios/Riot/Core/ProfileRepository.swift`
- Add `func toggleNewswireReaction(spaceDescriptorEntryID:parentEntryID:kind:active:) throws ->
  NewswireSignedRecord` mirroring `createNewswireComment` (L1461). Regenerated binding provides
  `profile.toggleNewswireReaction`.

## Layer 3 — iOS UI (needs Xcode / green.sh) — mirror the comment UI exactly

### Task 3.1 — Surface model
- **Files:** `apps/ios/Riot/NewswireEditorial.swift`
- Beside `commentsByParent` (L567) add `reactionsByPost: [String: [NewswireReactionTally]]` populated
  in `load()` (~L759, beside `commentsByParent = Self.groupComments(projection.comments)`) from the
  FFI post's `reactions`.
- Add `func toggleReaction(post:kind:)` mirroring the reply signer (L638): call the repo, then reload
  so the tally updates. Gate the affordance behind a `canReact` mirroring `canComment` (L625).

### Task 3.2 — Reaction bar under each post
- In `postRow` (L1031), beneath the Reply button (L1074), add a horizontal reaction bar: one small
  themed toggle per `ReactionKind` showing its count, tap → `model.toggleReaction(post:kind:)`.
  Pink-on-active (mirror the app's pink selection), a11y id `reaction-<kind>-<post.id>`. Reuse
  `RiotBadge`/mono count styling already in the row.

### Task 3.3 — Verify
- `scripts/green.sh` (builds iOS + macOS app targets + Rust). Then a RiotKit unit test for the
  tally mapping + toggle, mirroring the comment tests.

---

## Testing summary
- Core: `cargo test -p riot-core --all-features` (Layer 1, done).
- FFI: `cargo test -p riot-ffi --all-features` (Layer 2, verifiable tonight).
- Native regen: `scripts/conference/build-native-core.sh` (bindings + 5 staticlibs together).
- iOS/macOS: `scripts/green.sh` (Layer 3).
- Coverage: meet `.coverage-thresholds.json` (tarpaulin lines / llvm branches) as the ratchet floor.

## Out of scope (v1)
- `reacted_by_me` highlighting IF core doesn't already expose viewer identity in projection (add when
  a viewer seam exists — don't invent one here).
- Free-form emoji (kept closed for canonical CBOR).
- Reaction notifications (fold into the existing notify engine later, not now).
