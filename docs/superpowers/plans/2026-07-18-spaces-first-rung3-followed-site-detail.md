# Spaces-First — Rung 3: Followed-site detail (composite render) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax. This is the **render** rung — it replaces Rung 2's `FollowedSiteDetailPlaceholder` with the real Unit 4 composite detail. It is **docs-only** until executed; no code lands from this file.

**Goal:** When a **Following**-tier space is selected, the right pane renders the real composite site from core's `ResolvedCompositeSite` — **Editorial** (front page) / **Comments** / **Wire** — with moderated items as accountable placeholders (never vanish), honest degradation states per `SiteDegradation`, and — the security-load-bearing part — **non-spoofable trust-tier chrome (§4.1): the SHELL paints trust chrome (icon + shape + label, not colour) AROUND content from the core-resolved `SiteTrustTier`, so an open-wire item can NEVER look editorial.** A header carries title + the Following tier badge; the real Follow/Unfollow *action* is deferred to Rung 5 (see decision D1). iOS + Android + macOS shells over the shared Rust core; **no business logic in the shells** (spec §6.5) — every trust/treatment/degradation verdict is core-resolved and only *styled* here.

**Spec:** `docs/superpowers/specs/2026-07-18-spaces-first-navigation-design.md` — §4 (followed-site detail = Editorial/Comments/Wire), **§4.1 (non-spoofable trust-tier chrome — SECURITY-UI)**, §7 rung 3, §6.5 (shell routes/styles by core-assigned tier only).
**Prior rung:** `docs/superpowers/plans/2026-07-18-spaces-first-rung2-two-pane-shell.md` — leaves `FollowedSiteDetailPlaceholder` (iOS/macOS) + the Android by-kind followed placeholder that Rung 3 replaces.
**Branch:** `overnight/2026-07-18`.
**Shared-checkout:** `gh pr list --search "spaces-first OR rung3 OR composite OR followed-site"` before AND during; pathspec commits only; `ConferenceShellView.swift`, `NewswireEditorial.swift`, both `project.pbxproj`, and `MainActivity.kt` are high-traffic and pbxproj is merge-hostile — coordinate or use the temp-index technique if a sibling holds a pbxproj.

---

## Sequencing / dependencies (verified 2026-07-18 against `overnight/2026-07-18`)

1. **The Unit 4 composite render FFI is ON THIS BRANCH — no #46 wait.** `crates/riot-ffi/src/site_ffi.rs` carries `SiteTrustTier`, `SiteDegradation`, `SiteItemTreatment`, `ResolvedSiteItem`, `ResolvedCompositeSite`, **and** the store-wired `MobileProfile.resolve_composite_site(...)` builder (a concurrent session landed it — the spec §6.1 "#46 unmerged" prerequisite is already satisfied here). Exact signature below.
2. **Rung 3 depends on Rung 2 landing first** — Rung 2 introduces `FollowedSiteDetailPlaceholder`, the outer `NavigationSplitView` space-list shell, `SpaceDetailRoute.forTier(.following) → .followedSitePlaceholder`, and the Android by-kind follow placeholder. **Verified those symbols are NOT yet on the branch** (`grep FollowedSiteDetailPlaceholder|SpaceDetailRoute apps/` → empty). Rung 3 wires the render into that Rung-2 route; if executed before Rung 2 merges, the shell-wiring step (3.2) rebases onto it. The pure steps (3.-1/3.0/3.1) and the Android read-model do **not** depend on Rung 2 and can proceed in parallel.
3. **A concurrent session already landed most of the Swift READ-model** in `apps/ios/Riot/NewswireEditorial.swift` (see "What already exists"). Rung 3 **builds on it** — it does not re-invent `CompositeSiteReadModel`. This materially shrinks the rung.

---

## The exact FFI Rung 3 renders (verified `crates/riot-ffi/src/site_ffi.rs`)

`resolve_composite_site` is a **method on the `MobileProfile` handle** (not a free function), at `site_ffi.rs:436`:

```rust
#[uniffi::export]
impl MobileProfile {
    pub fn resolve_composite_site(
        &self,
        entry_bytes: Vec<u8>,        // ─┐ the owner-signed site /manifest, as its
        capability_bytes: Vec<u8>,   //  │ four wire fields — IDENTICAL shape to
        signature: Vec<u8>,          //  │ resolve_site_manifest (a SignedWillowEntry)
        payload_bytes: Vec<u8>,      // ─┘
        root: Vec<u8>,               // the followed site root (owned namespace id), 32 bytes
        now_unix_seconds: u64,       // clock the moderation-freshness window is judged against
    ) -> Result<ResolvedCompositeSite, MobileError>;
}
```

- **Inputs:** the owner-signed **manifest wire** (four byte vectors), the 32-byte `root`, and `now_unix_seconds`. It reads the profile's synced store *by namespace* (`with_active` → `resolve_composite_site_from_store`), loads `O:/mod/` records, applies `riot_core::site` verdicts, maps to FFI. **No decision logic** — store I/O + core calls + mapping.
- **Returns `ResolvedCompositeSite`** (`site_ffi.rs:398`): `root: String`, `degradation: SiteDegradation`, `transport_status: String` (`"available"` / `"manifest_invalid"`), `items: Vec<ResolvedSiteItem>`, `writer_cap_expired: bool`.
  - `ResolvedSiteItem` (`:384`): `entry_id`, `author_subspace`, `trust_tier: SiteTrustTier`, `treatment: SiteItemTreatment`. **⚠️ no content fields — see FFI GAP 2.**
  - `SiteTrustTier` (`:314`): `Editorial` / `OpenWire` / `Comment`.
  - `SiteDegradation` (`:337`): `None` / `MemberUnverified` / `EditorialOnly` / `ModerationLoading` / `TransportBlocked` / `ManifestRollbackAlarm` / `EquivocationAlarm` / `ManifestInvalid`.
  - `SiteItemTreatment` (`:366`): `Ordinary` / `Hidden` / `Tombstoned`.
- **Failure is a STATE, never a throw:** an invalid/tampered manifest returns a `ManifestInvalid` view with empty items and `transport_status = "manifest_invalid"` (`manifest_invalid_view`, `:462`). In-crate tests at `site_ffi.rs:843+` prove Loading holds the surface, tombstone/revoke reach communal items, and a tampered manifest resolves to the invalid state.

### ⚠️ FFI GAP 1 (blocking for LIVE data — resolved in this plan) {#ffi-gap-1}

**The shell can select a followed site but cannot obtain the manifest wire `resolve_composite_site` demands.** A Following row is a `FollowedSiteRow` (`mobile_api.rs:62`) carrying **only** `root` / `title` / `state` / `transport_blocked` — **no manifest bytes.** `resolve_composite_site` (and `resolve_site_manifest`) require the caller to pass the owner-signed manifest's four wire fields *in*. **There is no FFI that, given a followed `root`, fetches the stored `O:/manifest` entry from the synced store and resolves it** (verified: no manifest-fetch-by-root export in `mobile_api.rs`; the only store reads of a manifest are internal to `resolve_composite_site_from_store`). So from a selected row the shell has a `root` and nothing to hand `resolve_composite_site`.

Feasible to close with a **small store-reading FFI** — the manifest already lives in the store at the root namespace's `[MANIFEST_COMPONENT]` path (exactly where `resolve_composite_site_from_store` and the in-crate tests read owner entries). **Resolution — Step 3.-1 adds a one-arg `MobileProfile` method that fetches-then-resolves by root:**

```rust
// NEW in site_ffi.rs (UniFFI-gated: binding regen + native staticlib rebuild in ONE commit).
pub fn resolve_followed_site(&self, root: Vec<u8>, now_unix_seconds: u64)
    -> Result<ResolvedCompositeSite, MobileError>
// impl: load the single owner-signed O:/manifest entry from `root` ns → rebuild the
// SignedWillowEntry → delegate to resolve_composite_site_from_store. A missing/failed
// manifest returns the same ManifestInvalid STATE (fail-closed), never a throw.
```

**Decision:** the pure UI/read-model/chrome work is **unblocked and fully testable today** against a `CompositeSiteResolving` seam with stub data; the **`RiotProfileRepository` live conformance is BLOCKED** until `resolve_followed_site` lands (a root alone cannot drive `resolve_composite_site`). Rung 3 therefore carries `resolve_followed_site` as its first step (3.-1, UniFFI-gated) so the live conformance is real by the end of the rung. Mirrors Rung 2's honesty convention: the *UI* lands now; the *live path* is gated on a named, tracked core addition.

### ⚠️ FFI GAP 2 (content-shape design decision — flagged for the maintainer) {#ffi-gap-2}

**`ResolvedSiteItem` carries NO content** — its fields are `entry_id`, `author_subspace`, `trust_tier`, `treatment` (`site_ffi.rs:384`): **no headline, no body, no timestamp.** This is deliberate on the landed contract — the in-crate comment (`NewswireEditorial.swift:1402`) calls the composite resolve "the moderation/trust view, not the article reader." Consequences:

- Rung 3 as scoped here **renders the trust/moderation view**: per item, a trust-tier badge (§4.1) + an author tag + a Hidden/Tombstoned accountable placeholder + the site-level honest degradation. This is coherent and fully testable, and matches the landed `CompositeItemRow` (`authorTag` + `tier` + `display`, no body).
- It **cannot show an editorial article's headline or body.** The spec §4 mock shows headline snippets ("Report from the port…"); to render those, `ResolvedSiteItem` must gain content fields (e.g. `title`/`summary`/`created_unix_seconds`, size-ceiled body) **or** a companion per-`entry_id` content projection (mirroring how the newswire surface projects `ProjectedPost` content).

**This is a view-model contract decision the maintainer should make** (what content crosses the FFI, size ceilings, unlinkability of author identity in the byline). It extends Unit 4's contract, so this plan does **not** unilaterally add content fields. **Decision for this plan (D3):** Rung 3 ships the **trust/moderation view** (headline-less), which is a complete, shippable slice against the landed model; the **richer article render (headlines/body) is deferred** to a follow-up gated on closing GAP 2. If the maintainer wants headlines *in* Rung 3, add an optional Step 3.-1b (content fields on `ResolvedSiteItem` + resolver population from the entry payload, UniFFI-gated) before 3.2 — the view already renders per-item rows, so it absorbs content fields mechanically.

---

## What already exists (verified — Rung 3 builds ON these, does not recreate them)

A concurrent session landed the composite READ-model in `apps/ios/Riot/NewswireEditorial.swift` and its tests in `apps/ios/RiotTests/NewswireSurfaceTests.swift`:

| Symbol | File:line | What it gives Rung 3 |
|---|---|---|
| `CompositeContentHold` (`.shown` / `.held(reason:)`) | `NewswireEditorial.swift:1395` | the hold contract the view honours |
| `CompositeItemRow(_ item: ResolvedSiteItem)` | `:1406` | id, `authorTag` (8-char prefix), `tier: SiteTrustTier`, `display: NewswirePostDisplay` |
| `NewswirePostDisplay.fromSite(_:)` | `:1420` | maps `SiteItemTreatment` → the shared accountable-placeholder display |
| `CompositeSiteReadModel.from(_:)` + `isContentHeld` + `holdFor` + `banner(for:)` | `:1435` | the pure read model over `ResolvedCompositeSite` — hold decision + honest banner copy per degradation |
| `SiteTrustTier.label` (`"Editorial"`/`"Open wire"`/`"Comment"`) | `:1492` | text label only — **§4.1 needs icon+shape too (Rung 3 adds it)** |
| `CompositeSiteReadModelTests` (loading holds; moderated→placeholder; invalid holds; mild shows-with-notice) | `NewswireSurfaceTests.swift:881` | the read-model tests Rung 3 extends |
| `SiteModerationModel` + validator/review/authoring seam (owner WRITE path) | `:1503`+ | owner moderation — **out of Rung 3 scope** (read detail only) |

**Rung 3's actual delta** is narrow: (1) the §4.1 **trust-tier chrome** value type (icon+shape+label), (2) **Editorial/Comments/Wire grouping** of the flat `items`, (3) a `CompositeSiteResolving` **seam + `CompositeSiteSurfaceModel`** (load/publish, mirroring `NewswireSurfaceModel`), (4) the actual **`FollowedSiteDetailView`** replacing the placeholder + header/Following-badge, (5) **shell wiring**, (6) **Android parity**, and (7) the **`resolve_followed_site` FFI** (GAP 1).

Precedent to mirror for (3)/(4): `NewswireSurfaceModel` (`NewswireEditorial.swift:549`) + `NewswireSurfaceView` (`:867`) consuming a `NewswireProjectionView` via the `NewswireProjecting` seam (`CommunityShell.swift:455`). The composite detail is the direct analogue: `CompositeSiteResolving` seam → `ResolvedCompositeSite` → `CompositeSiteReadModel` → view.

---

## Decisions locked for this plan

- **D1 — Follow/Unfollow control: render the Following badge, DEFER the action to Rung 5.** There is **no `unfollow_site` / `follow_site` FFI** (verified: only `archive_community`/`restore_community` exist, and those are author-bearing community-only, `mobile_api.rs:419/424` — a followed site is author-less, so they do not apply). Fabricating an unfollow in a shell rung needs net-new core work that belongs with Rung 5's `follow_site(ticket)`/QR track (spec §7 rung 5). So Rung 3 fills the §4 header chrome contract's *relationship-control slot* with a **non-actionable "Following" tier badge** (state, not button); the Unfollow *action* lands in Rung 5 alongside `unfollow_site`. Mirrors Rung 2 (pinned Join/Create, deferred Follow-a-site to Rung 5).
- **D2 — `transport_blocked` at the row is Rung 1/2, not re-done here.** The Following *row's* "requires Tor — unavailable" honesty lands in Rung 1's `FollowedSiteRow.transport_blocked` field + Rung 2's `SpaceRowState.transportBlocked`. Rung 3 handles the **detail-pane** `SiteDegradation.transportBlocked` (a held surface with the banner), not the row.
- **D3 — Rung 3 ships the trust/moderation view (headline-less); richer article content is gated on [GAP 2](#ffi-gap-2).** See above.
- **D4 — Chrome is a pure value type the SHELL owns; content never supplies tier styling (§4.1).** `SiteTrustChrome` maps a core `SiteTrustTier` → (icon, shape, label). The view reads `CompositeItemRow.tier` (straight from `ResolvedSiteItem.trust_tier`, a core verdict) and paints chrome around the row. No item field can select/override its own chrome — the only input is the core tier. The mutation-style test fails if the view/chrome ever reads the tier wrong.

---

## The increment sub-ladder

The design + CTO flagged big-bang risk (spec §7, §10 Risk 1/5). Rung 3 is **five independently-landable steps**, each green on its own, each behind the composite read-model + shell test suites. Steps 3.-1 → 3.2 are iOS/macOS + core and sequential; 3.3 (Android) is independent and may land in parallel with 3.0/3.1.

- **3.-1 — `resolve_followed_site` FFI (Rust, UniFFI-gated).** Close [GAP 1](#ffi-gap-1): fetch the owner-signed `O:/manifest` from the root namespace and delegate to the existing resolver. Rust-tested in-crate (mirrors `site_ffi.rs:843+`). Binding regen + native staticlib rebuild in the SAME commit (record-change coupling — a new export without the rebuild is a runtime checksum abort, not a compile error). Makes the live conformance real. *(Optional 3.-1b: content fields on `ResolvedSiteItem` — only if the maintainer closes GAP 2 for headlines in this rung.)*
- **3.0 — Trust-tier CHROME + Editorial/Comments/Wire grouping (Swift, pure, no view/FFI).** The SECURITY-UI core (§4.1) and the §4 section split. Add `SiteTrustChrome` (icon+shape+label per tier) + grouping helpers on `CompositeSiteReadModel`. XCTest incl. the **mutation-style open-wire ≠ editorial** assertion. First, because it de-risks the anti-impersonation guarantee before any view exists.
- **3.1 — `CompositeSiteResolving` seam + `CompositeSiteSurfaceModel` (Swift, stub-driven).** Load/publish model mirroring `NewswireSurfaceModel`; `RiotProfileRepository` conforms via 3.-1's FFI. Stub-driven tests (no live FFI needed to prove the model).
- **3.2 — `FollowedSiteDetailView` + shell wiring (iOS/macOS).** Replace `FollowedSiteDetailPlaceholder` with the real detail (header + Following badge + banner + three sections with painted chrome + accountable placeholders). Wire `SpaceDetailRoute.followedSitePlaceholder → FollowedSiteDetailView`. pbxproj both projects.
- **3.3 — Android parity.** Kotlin mirrors of chrome + grouping + read-model + a followed-site detail surface skeleton routing Editorial/Comments/Wire with trust chrome. JUnit incl. the open-wire ≠ editorial mutation test.

---

## Step 3.-1 — `resolve_followed_site` FFI (Rust; closes GAP 1)

**Why first:** without a fetch-by-root resolve, the shell's live conformance is impossible (a `FollowedSiteRow` has no manifest wire). Pure store I/O + a delegate to the existing resolver — no new decision logic.

**Files:**
- Modify: `crates/riot-ffi/src/site_ffi.rs` (add the `resolve_followed_site` method + in-crate tests).
- Regenerate (not hand-edited): iOS/macOS `riot_ffi.swift` + Android binding via `cargo run -p xtask -- generate-bindings`, and rebuild native staticlibs (`scripts/conference/build-native-core.sh`) — **same commit** (record-change coupling).

### - [ ] Step 1: Write the failing in-crate tests
In `site_ffi.rs`'s `resolve_composite_tests` module (`:843`), reusing `owner_sign` / `manifest_wire` / `import_owned` / `open_local_profile`:
```rust
/// resolve_followed_site fetches the stored O:/manifest by root and resolves the same
/// view resolve_composite_site would, WITHOUT the caller supplying manifest wire.
#[test]
fn resolve_followed_site_fetches_the_stored_manifest_by_root() {
    let masthead = OwnedMasthead::generate().unwrap();
    let root = *masthead.namespace_id().as_bytes();
    let manifest = manifest_wire(&masthead, vec![masthead_member(root)]);
    let profile = open_local_profile().unwrap();
    import_owned(&profile, root, &manifest);                 // manifest now in the store
    import_owned(&profile, root, &heartbeat(&masthead, 1, NOW, &[]));

    let resolved = profile.resolve_followed_site(root.to_vec(), NOW).expect("resolve");
    assert_eq!(resolved.root, hex(&root));
    assert_eq!(resolved.degradation, SiteDegradation::None);  // fresh empty heartbeat = Current
}

/// A root with no stored manifest is the fail-closed ManifestInvalid STATE, not an error.
#[test]
fn resolve_followed_site_with_no_stored_manifest_is_invalid_state() {
    let profile = open_local_profile().unwrap();
    let resolved = profile.resolve_followed_site([7u8; 32].to_vec(), NOW).expect("resolve");
    assert_eq!(resolved.degradation, SiteDegradation::ManifestInvalid);
    assert!(resolved.items.is_empty());
}
```

### - [ ] Step 2: Run → FAIL (no method `resolve_followed_site`)
```
cargo test -p riot-ffi resolve_composite_tests::resolve_followed_site
```

### - [ ] Step 3: Implement
- Add to the `#[uniffi::export] impl MobileProfile` block in `site_ffi.rs`:
  - Parse `root` → `[u8;32]` (→ `InvalidInput`).
  - `with_active(&self.inner, |profile| { ... })`: load the single owner `O:/manifest` entry from the root namespace — `store.entries_with_prefix_in_namespace(&root, &Path::from_slices(&[MANIFEST_COMPONENT])?)`; absent or payload not retained → `manifest_invalid_view(root)`.
  - Rebuild a `SignedWillowEntry` from the stored entry (`entry_bytes = encode_entry(entry)`, `payload_bytes = payload`, plus the stored capability + signature — reuse the encode helpers the in-crate tests use at `site_ffi.rs:874+`). Cannot reassemble → `manifest_invalid_view(root)`.
  - Delegate to `resolve_composite_site_from_store(&profile.store, &signed, root, now_unix_seconds)`.
- **Implementation risk to resolve (do NOT stub green):** rebuilding a `SignedWillowEntry` from root alone needs the entry's authorisation token (capability + signature) retained alongside the held manifest. **Verify** whether `EvidenceStore` retains those bytes for a held entry. If NOT: escalate — either the store must retain them, or `resolve_composite_site_from_store` must re-validate from the held entry without an external signature. This is the one real risk in the step.

### - [ ] Step 4: Regenerate bindings + rebuild native core (same commit)
```
cargo run -p xtask -- generate-bindings
scripts/conference/build-native-core.sh
```

### - [ ] Step 5: Run → PASS + coverage gate
```
cargo test -p riot-ffi
cargo tarpaulin --workspace --all-features --fail-under <thresholds.tarpaulin.lines>   # from .coverage-thresholds.json
```

### - [ ] Step 6: Commit
```bash
git add crates/riot-ffi/src/site_ffi.rs <regenerated riot_ffi.swift> <regenerated android binding> <rebuilt staticlib artifacts>
git commit -m "feat(spaces/rung3): resolve_followed_site — fetch stored manifest by root + resolve (closes followed-detail FFI gap)"
```

---

## Step 3.0 — Trust-tier chrome + Editorial/Comments/Wire grouping (Swift, pure) — SECURITY-UI

**Why:** the §4.1 anti-impersonation guarantee and the §4 section split are pure value types — provable with XCTest alone, before any view. Highest-security surface in the rung, proven first.

**Files:**
- Modify: `apps/ios/Riot/NewswireEditorial.swift` (add `SiteTrustChrome` + grouping — same file as the landed `CompositeSiteReadModel`, so no new pbxproj entry).
- Modify test: `apps/ios/RiotTests/NewswireSurfaceTests.swift` (extend `CompositeSiteReadModelTests`).

### - [ ] Step 1: Write the failing tests
Add to `CompositeSiteReadModelTests` (`NewswireSurfaceTests.swift:881`):
```swift
// §4.1 SECURITY-UI: the three tiers carry DISTINCT icon AND shape AND label — not
// colour alone — so the badge survives grayscale, colourblindness, and a reshare.
func testEachTrustTierHasADistinctIconShapeAndLabel() {
    let tiers: [SiteTrustTier] = [.editorial, .openWire, .comment]
    let chrome = tiers.map(SiteTrustChrome.for)
    XCTAssertEqual(Set(chrome.map(\.systemImage)).count, 3, "distinct icon per tier")
    XCTAssertEqual(Set(chrome.map(\.shape)).count, 3, "distinct shape per tier")
    XCTAssertEqual(Set(chrome.map(\.label)).count, 3, "distinct text label per tier")
    for c in chrome { XCTAssertFalse(c.label.isEmpty, "label carries text, never colour alone") }
}

// §4.1 MUTATION-STYLE: an open-wire item must NOT carry editorial chrome. If the
// view/chrome ever read the tier wrong (open-wire styled as editorial), this fails.
func testOpenWireItemDoesNotCarryEditorialChrome() {
    let wire = CompositeItemRow(item("11".repeated(32), .ordinary, tier: .openWire))
    let editorialChrome = SiteTrustChrome.for(.editorial)
    let wireChrome = SiteTrustChrome.for(wire.tier)   // wire.tier came from the CORE verdict
    XCTAssertNotEqual(wireChrome, editorialChrome, "open-wire can NEVER present editorial chrome")
    XCTAssertEqual(wire.tier, .openWire, "the row reads the core tier, never infers it")
    XCTAssertNotEqual(wireChrome.systemImage, editorialChrome.systemImage)
    XCTAssertNotEqual(wireChrome.shape, editorialChrome.shape)
}

// §4 — the flat items split into Editorial / Comments / Wire by the CORE tier only.
func testItemsGroupIntoEditorialCommentsAndWireByCoreTier() {
    let model = CompositeSiteReadModel.from(resolved(.none, items: [
        item("11".repeated(32), .ordinary, tier: .editorial),
        item("22".repeated(32), .ordinary, tier: .comment),
        item("33".repeated(32), .ordinary, tier: .openWire),
        item("44".repeated(32), .ordinary, tier: .editorial),
    ]))
    XCTAssertEqual(model.editorialItems.map(\.id), ["11".repeated(32), "44".repeated(32)])
    XCTAssertEqual(model.commentItems.map(\.id), ["22".repeated(32)])
    XCTAssertEqual(model.wireItems.map(\.id), ["33".repeated(32)])
    XCTAssertEqual(model.editorialItems.count + model.commentItems.count + model.wireItems.count,
                   model.items.count)   // the sections partition items — nothing dropped/double-counted
}
```
> The `item(_:_:tier:)` and `resolved(_:items:)` helpers already exist in `CompositeSiteReadModelTests` (`NewswireSurfaceTests.swift:882,889`) with a `tier:` parameter — reuse them.

### - [ ] Step 2: Run → FAIL (`cannot find 'SiteTrustChrome'` / `editorialItems`)
```
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/CompositeSiteReadModelTests
```

### - [ ] Step 3: Implement (in `NewswireEditorial.swift`)
- `public struct SiteTrustChrome: Equatable, Sendable { public let systemImage: String; public let shape: TierShape; public let label: String }` with `public enum TierShape: Equatable, Sendable { case shield, chevron, bubble }` — **three distinct shapes** so the badge is legible in grayscale and reshare-safe (colour is optional decoration on top, never the signal).
  - `public static func `for`(_ tier: SiteTrustTier) -> SiteTrustChrome`:
    - `.editorial` → `SiteTrustChrome(systemImage: "checkmark.seal.fill", shape: .shield, label: "Editorial")` (verified masthead).
    - `.openWire` → `SiteTrustChrome(systemImage: "dot.radiowaves.left.and.right", shape: .chevron, label: "Open wire")` (untrusted, open publishing).
    - `.comment` → `SiteTrustChrome(systemImage: "bubble.left", shape: .bubble, label: "Comment")`.
  - Reuse the landed `SiteTrustTier.label` (`:1492`) as the label source so there is ONE label string per tier.
- Extend `CompositeSiteReadModel` with pure computed sections (order = core's emitted order, no shell re-sort — §6.5):
  - `public var editorialItems: [CompositeItemRow] { items.filter { $0.tier == .editorial } }`
  - `public var commentItems: [CompositeItemRow] { items.filter { $0.tier == .comment } }`
  - `public var wireItems: [CompositeItemRow] { items.filter { $0.tier == .openWire } }`

### - [ ] Step 4: Run → PASS
```
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/CompositeSiteReadModelTests
```

### - [ ] Step 5: Commit
```bash
git add apps/ios/Riot/NewswireEditorial.swift apps/ios/RiotTests/NewswireSurfaceTests.swift
git commit -m "feat(spaces/rung3): non-spoofable trust-tier chrome (icon+shape+label, §4.1) + Editorial/Comments/Wire grouping"
```

---

## Step 3.1 — `CompositeSiteResolving` seam + `CompositeSiteSurfaceModel` (Swift, stub-driven)

**Why:** the view (3.2) needs a load/publish model behind a test-stubbable seam, mirroring `NewswireSurfaceModel` ← `NewswireProjecting`.

**Files:**
- New: `apps/ios/Riot/CompositeSiteDetail.swift` (seam + model; kept out of the already-large `NewswireEditorial.swift`).
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift` (conform `RiotProfileRepository` to `CompositeSiteResolving` via 3.-1's `resolve_followed_site`).
- New test: `apps/ios/RiotTests/CompositeSiteSurfaceTests.swift`.
- pbxproj: register `CompositeSiteDetail.swift` + `CompositeSiteSurfaceTests.swift` in **both** projects (recipe below).

**Decision:** a **sibling `CompositeSiteResolving` protocol** — `func resolveFollowedSite(root: String, nowUnixSeconds: UInt64) throws -> ResolvedCompositeSite` — NOT a widening of `NewswireProjecting` (a followed site is author-less, resolved by root, not by a community descriptor — spec §6.2 "do NOT shoehorn a followed site into `CommunityRow`" applies to the seam too). `RiotProfileRepository` conforms via `try handle.resolveFollowedSite(root: ..., nowUnixSeconds: ...)` (the 3.-1 FFI). Tests inject a stub returning a canned `ResolvedCompositeSite`.

### - [ ] Step 1: Write the failing tests
`apps/ios/RiotTests/CompositeSiteSurfaceTests.swift`:
```swift
import XCTest
@testable import RiotKit

@MainActor
final class CompositeSiteSurfaceTests: XCTestCase {
    struct StubResolver: CompositeSiteResolving {
        let view: ResolvedCompositeSite
        func resolveFollowedSite(root: String, nowUnixSeconds: UInt64) throws -> ResolvedCompositeSite { view }
    }
    private func resolved(_ deg: SiteDegradation, _ items: [ResolvedSiteItem]) -> ResolvedCompositeSite {
        ResolvedCompositeSite(root: "cd".repeated(32), degradation: deg,
                              transportStatus: "available", items: items, writerCapExpired: false)
    }

    func testLoadPublishesTheReadModelFromTheSeam() {
        let item = ResolvedSiteItem(entryId: "11".repeated(32), authorSubspace: "ab".repeated(32),
                                    trustTier: .editorial, treatment: .ordinary)
        let model = CompositeSiteSurfaceModel(root: "cd".repeated(32), title: "indymedia",
                                              resolver: StubResolver(view: resolved(.none, [item])))
        model.load()
        XCTAssertEqual(model.readModel?.editorialItems.count, 1)
        XCTAssertFalse(model.readModel?.isContentHeld ?? true)
    }

    func testModerationLoadingFromCoreHoldsTheSurface() {
        let model = CompositeSiteSurfaceModel(root: "cd".repeated(32), title: "x",
                                              resolver: StubResolver(view: resolved(.moderationLoading, [])))
        model.load()
        XCTAssertTrue(model.readModel?.isContentHeld ?? false)
        XCTAssertNotNil(model.readModel?.bannerMessage)
    }

    func testAResolverThrowShowsTheInvalidStateNotACrash() {
        struct Throwing: CompositeSiteResolving {
            func resolveFollowedSite(root: String, nowUnixSeconds: UInt64) throws -> ResolvedCompositeSite {
                throw NSError(domain: "test", code: 1)
            }
        }
        let model = CompositeSiteSurfaceModel(root: "cd".repeated(32), title: "x", resolver: Throwing())
        model.load()
        XCTAssertTrue(model.readModel?.isContentHeld ?? false, "a resolve failure holds the surface")
    }
}
```

### - [ ] Step 2: Run → FAIL (`cannot find 'CompositeSiteSurfaceModel'`)

### - [ ] Step 3: Implement (`CompositeSiteDetail.swift`)
- `public protocol CompositeSiteResolving { func resolveFollowedSite(root: String, nowUnixSeconds: UInt64) throws -> ResolvedCompositeSite }`.
- `@MainActor public final class CompositeSiteSurfaceModel: ObservableObject`:
  - `@Published public private(set) var readModel: CompositeSiteReadModel?`; `public let root: String`, `public let title: String`; `private let resolver: CompositeSiteResolving`.
  - `public func load()`: `let now = UInt64(Date().timeIntervalSince1970)`; `do { readModel = .from(try resolver.resolveFollowedSite(root: root, nowUnixSeconds: now)) } catch { readModel = .invalid(root: root) }` — a throw becomes the same held invalid STATE core returns for a bad manifest (never a blank, never a crash). Add a `CompositeSiteReadModel.invalid(root:)` convenience (a `ManifestInvalid` view over `root`).
  - `public func retry() { load() }` (mirrors `NewswireSurfaceModel.retry()`).
- `ProfileRepository.swift`: `extension RiotProfileRepository: CompositeSiteResolving { public func resolveFollowedSite(root: String, nowUnixSeconds: UInt64) throws -> ResolvedCompositeSite { try handle.resolveFollowedSite(root: <root bytes per the generated signature>, nowUnixSeconds: nowUnixSeconds) } }` — a thin passthrough to the 3.-1 FFI. *No logic; core owns the resolve.*

### - [ ] Step 4: Register both files in BOTH pbxproj (recipe) — build green
### - [ ] Step 5: Run → PASS (+ full `RiotKit` iOS run stays green; `-scheme RiotKit-macOS` build proves the macOS pbxproj entry)

### - [ ] Step 6: Commit
```bash
git add apps/ios/Riot/CompositeSiteDetail.swift apps/ios/Riot/Core/ProfileRepository.swift \
        apps/ios/RiotTests/CompositeSiteSurfaceTests.swift \
        apps/ios/Riot.xcodeproj/project.pbxproj apps/macos/Riot.xcodeproj/project.pbxproj
git commit -m "feat(spaces/rung3): CompositeSiteResolving seam + surface model (load/publish over resolve_followed_site)"
```

---

## Step 3.2 — `FollowedSiteDetailView` + shell wiring (iOS/macOS)

**Why:** the visible payoff — replace Rung 2's `FollowedSiteDetailPlaceholder` with the real detail.

**Files:**
- New: `apps/ios/Riot/CompositeSiteDetailView.swift` (the SwiftUI view; separate from the model file).
- Modify: `apps/ios/Riot/ConferenceShellView.swift` (route `SpaceDetailRoute.followedSitePlaceholder` → `FollowedSiteDetailView(model:)`, building the model from the selected `FollowedSiteRow` + the repository resolver).
- New test: `apps/ios/RiotTests/FollowedSiteDetailTests.swift` (pure routing/section-assembly assertions — no live window).
- pbxproj: register `CompositeSiteDetailView.swift` + `FollowedSiteDetailTests.swift` in **both** projects.

**View contract (spec §4 / §4.1):**
- **Header** (the §4 cross-kind chrome contract slot): `title` + a **Following tier badge** (section-level chrome distinguishing an untrusted followed site from an owned/organized space — §4.1 "Following is never mistakable for a space you own", Security S3) + the relationship-control slot showing a non-actionable **"Following ✓"** (D1 — real Unfollow is Rung 5).
- **Banner:** when `readModel.isContentHeld` or a mild degradation, render `readModel.bannerMessage` as a control (not decoration) above the content; under a hold the sections are gated (the landed `CompositeContentHold` contract).
- **Three sections — Editorial / Comments / Wire:** from `readModel.editorialItems` / `.commentItems` / `.wireItems`; each row draws **`SiteTrustChrome.for(row.tier)`** (icon + shape + label) painted by the shell AROUND the row (§4.1) — the row never styles itself. A `.hiddenInterstitial` / `.tombstoned` `display` draws the accountable-placeholder interstitial (reuse the `NewswireTreatmentCopy` interstitial at `NewswireEditorial.swift:1142`), never a vanish. **(Headline-less per D3 — rows show tier chrome + author tag + treatment; headlines land when GAP 2 closes.)**
- **Empty/first-run + held:** an empty resolved site shows a neutral "nothing here yet" state (§9), never a fabricated feed.
- `.accessibilityLabel` per row combines the tier chrome label + treatment ("Open wire, ordinary" / "Editorial, hidden — moderated"), colour-independent (§4.1 / §3.3).

### - [ ] Step 1: Write the failing tests
`apps/ios/RiotTests/FollowedSiteDetailTests.swift`:
```swift
import XCTest
@testable import RiotKit

@MainActor
final class FollowedSiteDetailTests: XCTestCase {
    func testDetailSectionsMirrorTheReadModelTierSplit() {
        let items = [
            ResolvedSiteItem(entryId: "11".repeated(32), authorSubspace: "ab".repeated(32), trustTier: .editorial, treatment: .ordinary),
            ResolvedSiteItem(entryId: "22".repeated(32), authorSubspace: "ab".repeated(32), trustTier: .openWire, treatment: .hidden),
        ]
        let rm = CompositeSiteReadModel.from(ResolvedCompositeSite(
            root: "cd".repeated(32), degradation: .none, transportStatus: "available",
            items: items, writerCapExpired: false))
        let sections = FollowedSiteDetailSections(readModel: rm)   // pure view helper
        XCTAssertEqual(sections.editorial.map(\.id), ["11".repeated(32)])
        XCTAssertEqual(sections.wire.map(\.id), ["22".repeated(32)])
        XCTAssertEqual(sections.wire.first?.display, .hiddenInterstitial)  // hidden ≠ dropped
    }

    // The header shows the Following badge; the relationship control is the deferred
    // non-actionable "Following" state (real Unfollow is Rung 5 — D1).
    func testHeaderShowsFollowingBadgeAndDeferredControl() {
        let header = FollowedSiteDetailHeader(title: "indymedia")
        XCTAssertEqual(header.title, "indymedia")
        XCTAssertEqual(header.tierBadgeLabel, "Following")
        XCTAssertFalse(header.hasActionableUnfollow, "Unfollow action is deferred to Rung 5")
    }
}
```

### - [ ] Step 2: Run → FAIL
### - [ ] Step 3: Implement
- `CompositeSiteDetailView.swift`: `public struct FollowedSiteDetailView: View`, `@ObservedObject var model: CompositeSiteSurfaceModel`; pure helpers `FollowedSiteDetailSections(readModel:)` (`.editorial`/`.comments`/`.wire`) and `FollowedSiteDetailHeader(title:)` (`title`, `tierBadgeLabel = "Following"`, `hasActionableUnfollow = false`) so the layout is testable without a window (the `ShellNavigationTests` pure-decision-type pattern). `.onAppear { model.load() }`.
- `ConferenceShellView.swift`: in Rung 2's by-kind detail switch, replace `FollowedSiteDetailPlaceholder` with `FollowedSiteDetailView(model: makeCompositeModel(for: selectedFollowedRow))`, where `makeCompositeModel` builds `CompositeSiteSurfaceModel(root: row.root, title: row.title, resolver: repository)`. Preserve the empty/first-run neutral state (§9).

### - [ ] Step 4: Register both new files in BOTH pbxproj — build green
### - [ ] Step 5: Run → PASS
```
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS
```

### - [ ] Step 6: Commit
```bash
git add apps/ios/Riot/CompositeSiteDetailView.swift apps/ios/Riot/ConferenceShellView.swift \
        apps/ios/RiotTests/FollowedSiteDetailTests.swift \
        apps/ios/Riot.xcodeproj/project.pbxproj apps/macos/Riot.xcodeproj/project.pbxproj
git commit -m "feat(spaces/rung3): FollowedSiteDetailView (Editorial/Comments/Wire + trust chrome + Following badge) replaces placeholder"
```

---

## Step 3.3 — Android parity (Kotlin, host-JVM tested)

**Why independent:** Android shares no Swift; it mirrors the pure models + adds a followed-site detail surface, host-JVM tested (like `ConferenceSurfaceTest.kt`). Android is still on the debug shell (per Rung 2's Android reality check), so this is pure-model parity + a detail surface skeleton.

**Files:**
- New: `apps/android/app/src/main/kotlin/org/riot/evidence/CompositeSiteDetail.kt` (`SiteTrustChrome`, grouping, read-model mirror over the generated `uniffi.riot_ffi.ResolvedCompositeSite`).
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/ConferenceSurface.kt` + `MainActivity.kt` (a `showFollowedSite(...)` skeleton routing the three tier-sections with trust chrome; reuse the Rung-2 followed placeholder slot).
- New test: `apps/android/app/src/test/kotlin/org/riot/evidence/CompositeSiteDetailTest.kt`.

### - [ ] Step 1: Regenerate the Android binding (brings `ResolvedCompositeSite`/`SiteTrustTier`/`SiteDegradation`/`SiteItemTreatment` + `resolveFollowedSite`)
- `cargo run -p xtask -- generate-bindings` + the Android staticlib/jni build (`scripts/conference/build-native-core.sh`). (3.-1's export is why this regen is needed on Android too.)

### - [ ] Step 2: Write the failing JUnit tests
`apps/android/app/src/test/kotlin/org/riot/evidence/CompositeSiteDetailTest.kt`:
```kotlin
package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test
import uniffi.riot_ffi.SiteTrustTier

class CompositeSiteDetailTest {
    // §4.1 SECURITY-UI mutation test: open-wire chrome must never equal editorial chrome.
    @Test fun openWireChromeIsNeverEditorialChrome() {
        val wire = SiteTrustChrome.forTier(SiteTrustTier.OPEN_WIRE)
        val editorial = SiteTrustChrome.forTier(SiteTrustTier.EDITORIAL)
        assertNotEquals(wire, editorial)
        assertNotEquals(wire.icon, editorial.icon)
        assertNotEquals(wire.shape, editorial.shape)
        assertNotEquals(wire.label, editorial.label)
    }

    // §4 — items partition into Editorial / Comments / Wire by the CORE tier.
    @Test fun itemsGroupByCoreTier() {
        val model = compositeReadModel(
            degradation = uniffi.riot_ffi.SiteDegradation.NONE,
            items = listOf(
                siteItem("11".repeat(32), SiteTrustTier.EDITORIAL),
                siteItem("22".repeat(32), SiteTrustTier.COMMENT),
                siteItem("33".repeat(32), SiteTrustTier.OPEN_WIRE),
            ))
        assertEquals(listOf("11".repeat(32)), model.editorialItems.map { it.id })
        assertEquals(listOf("22".repeat(32)), model.commentItems.map { it.id })
        assertEquals(listOf("33".repeat(32)), model.wireItems.map { it.id })
    }

    // §4 — a moderation-loading verdict HOLDS the surface (mirrors the Swift/Rust contract).
    @Test fun moderationLoadingHoldsTheSurface() {
        val model = compositeReadModel(uniffi.riot_ffi.SiteDegradation.MODERATION_LOADING, emptyList())
        assertEquals(true, model.isContentHeld)
    }
}
```

### - [ ] Step 3: Implement
- `CompositeSiteDetail.kt`: `data class SiteTrustChrome(icon, shape, label)` with `enum TierShape` and `fun forTier(SiteTrustTier)` — three distinct (icon, shape, label) triples (grayscale/colourblind/reshare-safe, §4.1); a `CompositeReadModel` mirror with `editorialItems`/`commentItems`/`wireItems` + `isContentHeld` + `bannerMessage`, pure over the generated `ResolvedCompositeSite` (same hold precedence as Swift `holdFor`).
- `ConferenceSurface.kt`/`MainActivity.kt`: a `showFollowedSite(root)` skeleton — call `handle.resolveFollowedSite(root, nowUnixSeconds)`, build the read-model, render the three tier-sections with trust chrome + accountable placeholders for hidden/tombstoned. Skeleton — relocate, don't redesign the debug surfaces.

### - [ ] Step 4: Run → PASS
```
cd apps/android && ./gradlew :app:testDebugUnitTest
./gradlew :app:compileDebugKotlin
```

### - [ ] Step 5: Commit
```bash
git add apps/android/app/src/main/kotlin/org/riot/evidence/CompositeSiteDetail.kt \
        apps/android/app/src/main/kotlin/org/riot/evidence/ConferenceSurface.kt \
        apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt \
        apps/android/app/src/test/kotlin/org/riot/evidence/CompositeSiteDetailTest.kt
git commit -m "feat(spaces/rung3): android composite detail — trust chrome + Editorial/Comments/Wire + hold"
```

---

## pbxproj recipe (BOTH projects, every new Swift file) — CLAUDE.md rule 5 / spec Risk 2

New Swift **source** files (`CompositeSiteDetail.swift`, `CompositeSiteDetailView.swift`) and **test** files (`CompositeSiteSurfaceTests.swift`, `FollowedSiteDetailTests.swift`):

1. **iOS** `apps/ios/Riot.xcodeproj/project.pbxproj` — add a `PBXFileReference` (`path = Riot/<File>.swift; sourceTree = SOURCE_ROOT;` for sources; `path = RiotTests/<File>.swift` for tests — mirror `NewswireEditorial.swift`), a `PBXBuildFile`, membership in the correct target's **Sources** build phase (RiotKit for sources, RiotTests for tests), and a `PBXGroup` child entry.
2. **macOS** `apps/macos/Riot.xcodeproj/project.pbxproj` — same, but paths are prefixed `../ios/Riot/<File>.swift` (sources) / `../ios/RiotTests/<File>.swift` (tests); target names are the `-macOS` variants.
3. **Verify** by building the macOS project (`xcodebuild build -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS`) — a missing macOS entry surfaces as a compile/link error, not a silent drop.
4. **Shared-checkout:** if a sibling session holds a pbxproj, do NOT interleave — coordinate or use the temp-index/`git add -p` pathspec technique; never a broad `git add -A`.

*(3.0 adds no new files — it edits the already-registered `NewswireEditorial.swift`/`NewswireSurfaceTests.swift`, so no pbxproj change there.)*

---

## What is DEFERRED (explicitly out of Rung 3)

- **[GAP 2](#ffi-gap-2) — article content on the view-model.** `ResolvedSiteItem` carries no headline/body/timestamp. Rung 3 ships the trust/moderation view (D3); the richer article render is gated on a maintainer decision on the content-shape contract (fields on `ResolvedSiteItem` vs a companion per-`entry_id` projection, size ceilings, byline unlinkability). If wanted in this rung, add Step 3.-1b before 3.2.
- **Rung 5 — real Follow/Unfollow action + `unfollow_site`/`follow_site(ticket)` FFI + QR gen/camera scan.** Rung 3 renders the Following tier badge (D1); the actionable Unfollow needs a net-new `unfollow_site` FFI (none exists). The Following tier is empty on a shipped build until Rung 5's `follow_site` populates it — so on-device the detail is exercised only once a site is followed; the render is proven now via stubs + the in-crate Rust resolve tests.
- **Rung 5 — writer expired-cap warning + compose-time `require:arti` notice + mandatory seizure disclosure (mint).** `ResolvedCompositeSite.writer_cap_expired` is surfaced by core but the compose/write surface is not this rung; the seizure disclosure fires at mint (an owned masthead under Communities, spec §4.4), not on a followed detail.
- **Owner moderation authoring UI** (`SiteModerationModel`, landed `NewswireEditorial.swift:1683`) — the owner WRITE surface, hosted under an owned site in Communities, not the followed (read) detail.
- **Row `transport_blocked` honesty** — Rung 1 (field) + Rung 2 (`SpaceRowState`), not re-done here (D2). Rung 3 handles only the detail-pane `SiteDegradation.transportBlocked` held state.
- **macOS nested-split-view polish** — inherited from Rung 2's accepted skeleton tradeoff.

---

## FFI-gap summary (flagged per task requirement)

| Gap | Detail | Resolution in this plan |
|---|---|---|
| **GAP 1 — no manifest-fetch-by-root FFI** | `resolve_composite_site` needs the owner-signed manifest wire passed in; a selected `FollowedSiteRow` carries only `root`/`title`/`state`/`transport_blocked` (`mobile_api.rs:62`) — the shell has a root and no manifest bytes, and no export turns a root into the resolved view. | **Step 3.-1** adds `resolve_followed_site(root, now)` — fetch `O:/manifest` from the root ns, rebuild the `SignedWillowEntry`, delegate to `resolve_composite_site_from_store`. UniFFI-gated. Pure UI/model testable against a stub seam meanwhile. |
| **GAP 1 sub-risk — store cap/signature retention** | Rebuilding a `SignedWillowEntry` from a stored entry needs its authorisation token bytes; verify `EvidenceStore` retains them. | **Flagged in 3.-1 Step 3** as the one real implementation risk — resolve (retain token bytes, or refactor the resolver to validate from the held entry) before claiming green; do not stub. |
| **GAP 2 — `ResolvedSiteItem` has no content** | `entry_id`/`author_subspace`/`trust_tier`/`treatment` only (`site_ffi.rs:384`) — no headline/body/timestamp; the composite resolve is the "moderation/trust view, not the article reader." | **D3**: Rung 3 ships the headline-less trust/moderation view (complete, shippable). Headlines are a maintainer view-model-contract decision → optional Step 3.-1b (content fields + resolver population), else a follow-up. |
| **GAP 3 — no `unfollow_site`/`follow_site` FFI** | No follow/unfollow export; `archive_/restore_community` are author-bearing community-only (`mobile_api.rs:419/424`), inapplicable to author-less followed sites. | **D1**: render the non-actionable Following badge; the Unfollow action is deferred to Rung 5 with its follow/QR track. |

---

## Self-review — spec §4 / §4.1 / §7 rung-3 requirements mapped to tasks

| Spec requirement | Landed in | Status |
|---|---|---|
| §4 followed-site detail = **Editorial / Comments / Wire** from `ResolvedCompositeSite` | 3.0 (grouping) + 3.2 (`FollowedSiteDetailView` sections) / 3.3 (Android) | ✓ (headline-less per D3/GAP 2) |
| §4 moderated items → **accountable placeholders (never vanish)** | 3.0 (`fromSite` treatment, landed) + 3.2 (interstitial reuse) + tests | ✓ |
| §4 **degradation states** rendered per `SiteDegradation` (moderation-loading / editorial-only / transport-blocked / manifest alarms) | landed `CompositeSiteReadModel.holdFor`/`banner`; 3.1 surfaces via the model; view honours the hold | ✓ |
| §4 header = title + tier badge + relationship control | 3.2 (`FollowedSiteDetailHeader`: title + Following badge + deferred control) | ✓ (Unfollow action → Rung 5, D1) |
| **§4.1 SECURITY-UI: shell paints trust chrome AROUND content from core `SiteTrustTier`; content can't self-style** | 3.0 `SiteTrustChrome.for(tier)` + 3.2 view paints it around each row; §6.5 no shell policy | ✓ |
| **§4.1 icon + shape + label (not colour alone); grayscale/colourblind/reshare-safe** | 3.0 `SiteTrustChrome` (distinct icon+shape+label per tier) + test | ✓ |
| **§4.1 open-wire can NEVER look editorial (anti-impersonation)** | 3.0 **mutation-style test** `testOpenWireItemDoesNotCarryEditorialChrome` + 3.3 Android twin | ✓ |
| §4.1 tiers distinct at row/section level (Following ≠ owned/organized — S3) | 3.2 section-level Following badge; per-row tier chrome | ✓ |
| §6.5 shell routes/styles by **core-assigned tier only**, no policy in shell | 3.0/3.1 pure over core verdicts; `resolve_followed_site` owns no decisions (delegates to core) | ✓ |
| §7 rung 3 = wire the Unit 4 `ResolvedCompositeSite` render + trust-tier chrome, replacing Rung 2's placeholder | 3.2 shell wiring | ✓ |
| §6.1 prerequisite (Unit 4 render FFI on main) | verified present in `site_ffi.rs` (no #46 wait) | ✓ |
| **FFI gap: root → resolved view** | 3.-1 `resolve_followed_site` | ✓ (new UniFFI-gated FFI) |
| Follow/Unfollow — decide + document | D1 (badge now, action Rung 5) | ✓ |
| Native iOS + Android + macOS | 3.0–3.2 (iOS/macOS shared sources) + 3.3 (Android) | ✓ |
| TDD (XCTest/JUnit), real tests, exact xcodebuild/gradle commands | every step (RED→GREEN) | ✓ |
| pbxproj both projects for new Swift files | recipe + 3.1/3.2 steps | ✓ |
| §7 increment sub-ladder, each landable + green | 3.-1 → 3.3 (five commits, TDD) | ✓ |

**Known partials (by design):** the trust/moderation view is headline-less until [GAP 2](#ffi-gap-2) closes (D3); the Following tier is empty on a shipped build until Rung 5's `follow_site` (render proven now via stubs + in-crate Rust resolve tests); the Unfollow *action* is a Following badge until Rung 5 (D1). These are the deferrals above, not gaps.
