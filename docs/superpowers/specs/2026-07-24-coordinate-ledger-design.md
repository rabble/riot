# Coordinate Ledger — Design & Work-Unit Decomposition

Date: 2026-07-24
Status: Draft for design-review gate. Buildable — WU-1 can start immediately.
Scoped against: `origin/main` @ `c3808ce` ("fix(ios): make Home's section model total").
Author: Designer agent (room-as-ledger reshape track).

Related:
- Vision memory: `riot-nav-reshape-room-as-ledger` (owner-endorsed 2026-07-24) — a community room is a **working ledger**, not a flat wire, run across differentiated channels.
- Object model: `docs/research/2026-07-10-dual-mode-research-addendum.md` §Object Vocabulary (the `request`/`offer`/`task`/`observation`/`event`/`resource`/`document` kinds we grow into).
- Prototype: `scratchpad/riot-nav-prototype.html` (clickable, per-screen built/partial/proposed badges).

---

## 1. What we are building

A community room gains **channels** instead of one flat post wire:

| Channel | Primitive | Source |
| --- | --- | --- |
| **Alerts** | broadcast, moderated, expiring | REUSE existing alert entries (`list_current_entries`) |
| **Coordinate** | asks · offers · tasks with `Open → Claimed → Done` lifecycle | **NET-NEW** `coordinate/v1/` family (this doc) |
| **Discuss** | freeform posts + replies + reactions | REUSE existing `newswire/v1/` family (`NewswireEditorial.swift`) |
| **Guide** | evergreen, multilingual, user-editable manual | REUSE `UsingRiotGuide` pattern (out of scope here) |

The engineering heart of this doc is the **Coordinate ledger**: three new signed record families —
**need** (ask), **offer**, and **task** — carried as one `CoordinateItemV1` object kind, plus a
signed **status-transition** family (`CoordinateStatusV1`) that drives `Open → Claimed → Done`, plus a
first-class **verify/dispute** family (`CoordinateVerificationV1`). Moderation reuses the newswire
hide/tombstone discipline (hides, never deletes).

### Why a new module, not a newswire profile

`newswire::RequestProfileV1` (`crates/riot-core/src/newswire/model.rs:61`) already tags a post as
`Need`/`Offer` — but it is a **flat annotation with no lifecycle, no claimant, no capacity**. The
Coordinate ledger needs *mutable derived state* (a claim record changes an item's status without
rewriting the immutable item), which is a status-record + latest-wins-projection shape, not a post
profile. We therefore add a sibling module `crates/riot-core/src/coordinate/` that mirrors newswire's
proven structure (`model` / `path` / `entry` / `projection` / `store` / `mod`) with its own path prefix
`coordinate/v1/…`. Newswire is left untouched; `RequestProfileV1` stays as a Discuss-channel post tag.

### Ask ≠ Offer (product constraint, load-bearing)

Asking for help is harder than offering it. The Coordinate surface makes **"Ask for help" the prominent
primary action**; there is **no symmetric ask/offer compose UX** and **no visible credit/reputation
ledger**. This is a UI and copy constraint (WU-8/WU-9), not a wire-format one — the `CoordinateKind`
enum is symmetric on the wire; the asymmetry lives in the Swift surface.

---

## 2. Record families (core wire model)

All three live under one CBOR object kind so the codec/path/inspect machinery is written once. Schema
strings follow the newswire convention (`crates/riot-core/src/newswire/model.rs:10-14`).

```
pub const COORDINATE_ITEM_SCHEMA:   &str = "org.riot.coordinate.item/1";
pub const COORDINATE_STATUS_SCHEMA: &str = "org.riot.coordinate.status/1";
pub const COORDINATE_VERIFY_SCHEMA: &str = "org.riot.coordinate.verification/1";
pub const COORDINATE_ACTION_SCHEMA: &str = "org.riot.coordinate.editorial-action/1"; // moderation
```

### 2.1 `CoordinateItemV1` — the ask / offer / task

```rust
pub enum CoordinateKind { Need = 0, Offer = 1, Task = 2 }

pub struct CoordinateItemV1 {
    pub space_descriptor_entry_id: [u8; 32], // binds item to the community room (newswire descriptor)
    pub kind: CoordinateKind,
    pub title: String,
    pub body: String,
    pub language: String,
    pub category_tags: Vec<String>,          // Events / Help / Info categorization
    pub coarse_location: Option<String>,     // location-minimized, like a post
    pub capacity: Option<u32>,               // Task/Offer: how many claimants can fill it; None = single
    pub needed_by_unix_seconds: Option<u64>, // soft deadline (display + sort)
    pub expires_at_unix_seconds: Option<u64>,// hard expiry — item drops off the open ledger after
    pub contact_instructions: String,        // how to reach the author (asks are harder → prominent)
    pub source_claims: Vec<String>,
    pub ai_assisted: bool,                    // MANDATORY flag (validation error if the field is absent)
}
```

Validation rules (mirror `newswire` `validate_post`, new closed `CoordinateModelError` variants):
- `Task`/`Offer` with `capacity == Some(0)` → `CapacityZero`.
- `Need` (ask) → `capacity` MUST be `None` (`AskHasCapacity`) — asks are not "filled N times".
- Every kind requires a non-empty `expires_at_unix_seconds` when it carries a `coarse_location`
  (matches the alert/request location+expiry discipline in `newswire::model`).
- `ai_assisted` is a required CBOR key (a decode with the key absent → `MissingKey`).

The item is **immutable** once signed. Status changes never rewrite it — they are separate records.

### 2.2 `CoordinateStatusV1` — the lifecycle transition (latest-wins)

```rust
pub enum CoordinateTransition { Claim = 0, Release = 1, Complete = 2, Cancel = 3 }

pub struct CoordinateStatusV1 {
    pub space_descriptor_entry_id: [u8; 32],
    pub target_entry_id: [u8; 32],   // the CoordinateItem being acted on
    pub transition: CoordinateTransition,
    pub note: Option<String>,
}
```

- **Claim / Release** are author-driven (any communal member), exactly like `NewsReactionV1`
  (`crates/riot-core/src/newswire/model.rs:118`) — dedup per `(signer subspace, target)`, **latest
  wins** by the total-order rule below. The signer of the latest active `Claim` is a *claimant*.
- **Complete** may be signed by the item author OR any active claimant (checked in projection). It
  marks the item `Done`.
- **Cancel** may be signed by the item author only; marks the item `Cancelled`.
- **Total-order tie rule** (avoids the same-µs flake — see memory `riot-inmemory-clock-same-timestamp-tiebreak`):
  rank = `(tai_j2000_micros, transition_priority, entry_id)` where `Release`/`Cancel` outrank `Claim`
  at an identical timestamp (a claim+release in the same µs resolves to Released). Mirror
  `ReactionRank` at `crates/riot-core/src/newswire/projection.rs:167`.

### 2.3 Derived lifecycle (computed in projection, never stored)

Given an item + its status records + clock:

```
active_claimants = { latest status per (signer, item) == Claim, and not Released }
Open      : no active claimants, not Complete/Cancel, not expired
Claimed   : 1 ≤ active_claimants < capacity (or capacity None and ≥1)
Full      : capacity Some(n) and active_claimants == n
Done      : an active Complete transition exists (author or claimant)
Cancelled : an active Cancel by the author
Expired   : now > expires_at (terminal, dropped from open ledger → "earlier")
```

### 2.4 `CoordinateVerificationV1` — verify / dispute on reports

```rust
pub enum VerificationStance { Verify = 0, Dispute = 1 }

pub struct CoordinateVerificationV1 {
    pub space_descriptor_entry_id: [u8; 32],
    pub target_entry_id: [u8; 32],
    pub stance: VerificationStance,
    pub note: Option<String>,
}
```

Author-driven, latest-wins per `(signer, target)`. Projection surfaces `verify_count` / `dispute_count`
(distinct signers) on the item. "Two independent eyewitness verifies promote" is a **display threshold
in Swift** (WU-8), not a wire rule.

### 2.5 Moderation — reuse the hide-not-delete discipline

Moderation is a `CoordinateEditorialActionV1` copying newswire's `EditorialActionKind`
(`Hide`/`Tombstone`/`Retract` — `crates/riot-core/src/newswire/model.rs:127`) restricted to the
community's editorial roster. Projection **redacts** (`title`/`body`/`contact` → `None`) but keeps the
row and its identity accountable — identical to `ProjectedContent::from(post, is_ordinary)` at
`crates/riot-core/src/newswire/projection.rs:181`. Nothing is deleted from the store.

---

## 3. Signing & admission — REUSE the canonical gate

Every Coordinate record signs and admits **exactly like a newswire post**: build a communal Willow
entry, sign it, and route the bytes through the same preview → plan → commit boundary
(`inspect_core` in `mobile_state.rs`) so it is bound by the same byte budgets and admission checks.

Copy `inspect_news_record` (`crates/riot-core/src/newswire/entry.rs:416`) verbatim into
`coordinate::entry::inspect_coordinate_record`, including the **closed communal-cap check** at
`entry.rs:433-441` (`is_owned() || !delegations().is_empty() || granted_namespace != namespace ||
receiver != subspace || !includes(entry)` → reject). Do **not** hand-roll a subset — see memory
`riot-reuse-canonical-gate` (the recurring bug is a new entry point re-implementing a partial gate).
The FFI `create_coordinate_*` methods delegate to `create_signed_coordinate_*` and import the signed
bytes through the existing boundary, never bypassing it.

**Time-unit trap** (memory `riot-willow-entry-time-unit-trap`): item/status timestamps on the Willow
entry are **TAI/J2000 microseconds**, not Unix seconds. The `*_unix_seconds` payload fields are display
metadata only; the path/entry time comes from the `ClockSnapshot`, exactly as `build_signed` does at
`crates/riot-core/src/newswire/entry.rs:142-160`. Stamp all fixtures in the production unit.

**Durable-only trap** (memory `riot-signed-entries-durable-only`): in-memory Willow join drops the
cap+signature; the full `SignedWillowEntry` persists only in SQLite `accepted_entries`. Any test that
reprojects status **after a reopen** must use a durable profile
(`open_local_profile_with_database`), not the in-memory fixture.

---

## 4. The FIVE registration sites (per new family) — checklist

A new record family is registered at **five sites inside the module**; four are compiler-forced, the
fifth (the store prefix scan) is **silent** — miss it and records admit but never project
(memory `newswire-record-family-registration`). For the Coordinate module the sites are:

1. **`coordinate/model.rs`** — `SCHEMA` const + struct + `encode_*`/`decode_*` canonical-CBOR codec +
   `CoordinateModelError` variants. *(Not compiler-forced; a missing codec only fails when referenced.)*
2. **`coordinate/path.rs`** — add a `CoordinatePathKind` variant + a `coordinate_path` match arm
   (**compiler-forced**, exhaustive match) + a `classify_coordinate_path` if-chain arm
   (**NOT** forced — an if-chain; a miss silently returns `None` / falls through). Mirror
   `newswire/path.rs:78`.
3. **`coordinate/entry.rs`** — add a `CoordinatePayload` variant (**compiler-forces** three matches:
   `encode_payload` `:113`, `payload_path_kind` `:124`, `inspect_verified_components_bounded` `:466`)
   + a `create_signed_*` factory + inspect arm.
4. **`coordinate/projection.rs`** — handle the new payload in `project()` (an `Eligible*` bucket or a
   status/verify fold). Partially forced via the pinned-descriptor match copied from
   `store.rs decode_scanned_entries` `:169`.
5. **`coordinate/store.rs`** — **the silent 5th site.** `load_ledger_records` builds one prefix per
   family and scans it (mirror `load_space_records` `:80-111`). A new family needs a **new prefix added
   here** or its entries are admitted, retained, and never scanned into the projection. Grep target:
   the `entries_with_prefix_in_namespace` block.

Because item/status/verify/action are introduced across WU-1…WU-5, **each WU that adds a family walks
all five sites** and its acceptance test asserts the record actually appears in a projection (the
end-to-end check that catches a missed 5th site).

---

## 5. FFI — the BOTH-sites classification (per new family)

`coordinate/v1/…` is a NEW top-level path family, so mobile's alert-vs-non-alert classifier must learn
it in **both** places or the board bricks / imports reject (memory `riot-ffi-nonalert-classification`).
Add a `riot_core::coordinate::is_coordinate_prefix(path)` helper (mirror
`newswire::is_newswire_prefix`, `crates/riot-core/src/newswire/mod.rs:46`) and reference it at **both**:

- **`crates/riot-ffi/src/mobile_state.rs:1053-1060`** — `list_current_entries` filter chain (the
  `!is_app_data_entry && … && !is_newswire_prefix …` predicate). Add `&& !is_coordinate_prefix(path)`
  so a locally-authored Coordinate entry is not misread as an alert and does not brick the alert board.
- **`crates/riot-ffi/src/mobile_state.rs:1963-1968`** — `inspectable_entries` `is_non_alert` disjunction.
  Add `|| is_coordinate_prefix(decoded_entry.path())` so a synced Coordinate entry is classified
  non-alert and not force-decoded as an alert (which would reject the whole import).

FFI projection surface (WU-6), mirroring `newswire_ffi.rs`:
- `MobileProfile::create_coordinate_item(input: CoordinateItemInput) -> NewswireSignedRecord`
- `MobileProfile::transition_coordinate_item(input: CoordinateTransitionInput) -> NewswireSignedRecord`
- `MobileProfile::verify_coordinate_item(input: CoordinateVerificationInput) -> …`
- `MobileProfile::moderate_coordinate_item(input: CoordinateModerationInput) -> …`
- `MobileProfile::project_coordinate_ledger(descriptor_id: String) -> CoordinateLedgerProjection`
- UniFFI records: `CoordinateItemInput`, `CoordinateKind` (enum), `CoordinateTransitionInput`,
  `ProjectedCoordinateItem { status, claimant_ids, capacity, verify_count, dispute_count, treatment }`,
  `CoordinateLedgerProjection { open, claimed, done, earlier }`.

Reminder (memory `riot-uniffi-record-change-coupling` + `riot-native-core-staleness`): the generated
binding and the native staticlib must rebuild **together** (`generate-bindings` + cross-compile) or the
app hits a runtime checksum abort — but a host-JVM/Rust-only WU needs only `generate-bindings`
(memory `android-host-jvm-no-so`).

---

## 6. Swift channel structure (iOS)

Primary surface files: `apps/ios/Riot/CommunityShell.swift` (the in-room container),
`apps/ios/Riot/NewswireEditorial.swift` (the current wire UI, 2496 lines — becomes the **Discuss**
channel), `apps/ios/Riot/CommunityChooser.swift` (unchanged).

- **WU-7** adds a `ChannelPicker` inside `CommunityShell` with four tabs (Alerts / Coordinate / Discuss
  / Guide). Alerts binds to the existing alert board; Discuss re-hosts `NewswireEditorial`; Guide reuses
  the `UsingRiotGuide` view; Coordinate is a placeholder until WU-8.
- **WU-8** builds `CoordinateLedgerView` — the ask/offer/task list grouped by derived status
  (Open / Claimed / Done / Earlier), with **claim** and **mark-done** buttons wired to
  `transition_coordinate_item`, capacity shown as `claimed/N`, and verify/dispute affordances on report
  items. "Ask for help" is the prominent primary CTA; offers/tasks are a secondary, less-prominent path.
- **WU-9** builds `CoordinateComposeView` — **typed compose**: the user first picks the object kind
  (Ask / Offer / Task / Report), then sees the kind-specific fields; the `ai_assisted` toggle is
  **mandatory** and defaults visible; dual authorship line ("published by the collective · signed by
  <key>") reuses the newswire actor rendering.

Android has no Compose in-room surface today (memory `riot-composite-unit6-rung5-blocked`); Android
parity is a follow-on track, out of scope for these WUs. Core + FFI (WU-1…WU-6) are platform-neutral and
unblock it.

---

## 7. Test strategy & coverage

- **TDD, per five-site discipline.** Every core WU: write the failing round-trip / projection test
  first. Each family-adding WU asserts a record travels model → path → entry → store scan → projection
  (the end-to-end assertion that catches a missed silent 5th site).
- **Coverage gate** (`.coverage-thresholds.json`, BLOCKING): tarpaulin lines ≥ 97, llvm lines ≥ 95 /
  functions ≥ 95 / regions ≥ 92 / **branches ≥ 83**. New closed error enums and the projection
  branch-heavy lifecycle are the branch-coverage risk — cover every transition and every reject arm.
- **Lifecycle matrix** (WU-4): Open→Claim→Claimed; Claim→Release→Open; capacity-full; author Complete;
  claimant Complete; author Cancel; non-author Cancel rejected in projection; same-µs claim+release
  tie → Released (run the test repeatedly, memory `riot-inmemory-clock-same-timestamp-tiebreak`).
- **Durable reprojection** (WU-4/WU-6): reopen a `open_local_profile_with_database` profile and confirm
  claims survive (memory `riot-signed-entries-durable-only`).
- **Workspace build** after any WU that adds a `CoordinatePayload`/path variant: `cargo test
  --workspace --all-features` — a scoped `-p riot-core` hides the downstream FFI match break
  (memory `riot-scoped-test-hides-cross-crate-break`).
- **FFI**: Rust unit tests on `create_coordinate_*` + both classification sites (assert a coordinate
  entry does NOT appear on the alert board and DOES round-trip through import); `generate-bindings`
  green; iOS `RiotKit` unit tests for the projection mapping.
- **Register** `pub mod coordinate;` in `crates/riot-core/src/lib.rs:18` area (WU-1).

---

## 8. Work-unit decomposition (ordered, each independently shippable)

Scoped against `origin/main@c3808ce`. Rung C = core, F = FFI, S = Swift.

| WU | Title | Ships | File scope |
| --- | --- | --- | --- |
| **WU-1** (C) | Coordinate module + `CoordinateItemV1` model & path | encode/decode/classify round-trip for the item family; module registered | `crates/riot-core/src/lib.rs` (add `pub mod coordinate`), `crates/riot-core/src/coordinate/mod.rs` (new), `coordinate/model.rs` (new), `coordinate/path.rs` (new) |
| **WU-2** (C) | Item sign + admit + store scan | `create_signed_coordinate_item` + `inspect_coordinate_record` (canonical gate copy) + `load_ledger_records` scanning the item prefix; end-to-end admit→scan test | `coordinate/entry.rs` (new), `coordinate/store.rs` (new), `coordinate/mod.rs` |
| **WU-3** (C) | `CoordinateStatusV1` transition family | signed Claim/Release/Complete/Cancel records; all 5 sites walked; store scans the status prefix | `coordinate/model.rs`, `coordinate/path.rs`, `coordinate/entry.rs`, `coordinate/store.rs` |
| **WU-4** (C) | Ledger projection (derived lifecycle) | `project_ledger` → `ProjectedCoordinateItem` with Open/Claimed/Full/Done/Cancelled/Expired, claimant set, capacity, latest-wins + tie rule; lifecycle matrix + durable reprojection tests | `coordinate/projection.rs` (new), `coordinate/store.rs`, `coordinate/mod.rs` |
| **WU-5** (C) | Verify/dispute + moderation (hide-not-delete) | `CoordinateVerificationV1` + `CoordinateEditorialActionV1`; projection surfaces counts + redacts hidden items keeping the row | `coordinate/model.rs`, `coordinate/path.rs`, `coordinate/entry.rs`, `coordinate/store.rs`, `coordinate/projection.rs` |
| **WU-6** (F) | FFI projection + BOTH classification sites + typed compose | `is_coordinate_prefix` added at both `mobile_state.rs` sites; `create/transition/verify/moderate/project` FFI + uniffi records; bindings regenerated; classification tests | `crates/riot-core/src/coordinate/mod.rs` (add `is_coordinate_prefix`), `crates/riot-ffi/src/coordinate_ffi.rs` (new), `crates/riot-ffi/src/mobile_state.rs` (lines ~1053-1060 & ~1963-1968), `crates/riot-ffi/src/lib.rs` |
| **WU-7** (S) | In-room channel structure | Alerts/Coordinate/Discuss/Guide tabs; Discuss re-hosts newswire, Alerts binds existing board, Guide reuses UsingRiotGuide, Coordinate placeholder | `apps/ios/Riot/CommunityShell.swift`, new `apps/ios/Riot/ChannelPicker.swift` |
| **WU-8** (S) | Coordinate ledger surface | list grouped by status; claim/mark-done buttons; capacity `claimed/N`; verify/dispute on reports; "Ask for help" prominent primary | new `apps/ios/Riot/CoordinateLedgerView.swift`, `CommunityShell.swift` |
| **WU-9** (S) | Typed compose | pick-kind-first compose; mandatory `ai_assisted`; dual-authorship line; asks have no capacity field | new `apps/ios/Riot/CoordinateComposeView.swift` |

Dependencies: WU-1 → WU-2 → {WU-3 → WU-4} → WU-5 → WU-6 → WU-7 → WU-8 → WU-9. WU-7 depends only on
WU-6 for the Coordinate binding but can land its Alerts/Discuss/Guide tabs earlier if the Coordinate tab
ships as a placeholder. Each WU is a single PR with its own tests and passes the coverage gate.

---

## 9. Open questions for the design-review gate

1. **Capacity semantics for `Offer`** — does an offer with `capacity: Some(3)` mean "3 people can take
   me up on this"? Proposed: yes, same as Task; `Need`/ask never carries capacity.
2. **Who can `Complete`** — author + active claimants (proposed). Reviewer: is claimant-complete a
   griefing vector? Mitigation: Complete is latest-wins and reversible by the author via a new item, and
   moderation can Hide.
3. **Report as an object kind** — the vision lists `observation` (report). This doc folds "reports" into
   the Discuss channel (newswire posts) with verify/dispute layered via `CoordinateVerificationV1`
   targeting a newswire entry_id. Alternative: a full `observation` family. Deferred; verify/dispute is
   built kind-agnostic (`target_entry_id` is any entry) so either path stays open.
4. **Cross-channel counts** — should the room header show "3 open asks"? Needs the ledger projection at
   room-list time (a cheap count). Proposed follow-on, not in these WUs.
</content>
