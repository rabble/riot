# Spaces-First — Rung 1: Core following + personal relationships — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add the core/FFI state that lets the shell draw all three space tiers — `Following` (indymedia sites you follow) and `Personal` (your own home space) alongside today's `Organizer/Member/PublicReader` — WITHOUT a durable registry migration, and expose followed sites through a distinct author-less list.

**Architecture:** The community registry (`crates/riot-ffi/src/community_registry.rs`) is a **device-local** CBOR record; a `REGISTRY_VERSION` bump treats old data as corrupt, so we AVOID one. New relationships are added as **additive enum wire codes** (safe: an old client never wrote them). Followed composite sites have no author, so they are surfaced through a **separate `list_followed_sites()`** with a distinct row type, not the author-derived `CommunityRow`. No business logic leaves core; the shell renders.

**Tech Stack:** Rust (riot-ffi, riot-core), UniFFI, minicbor. Gates: `cargo test --workspace --all-features`, `cargo clippy --workspace --all-features --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo run -p xtask -- validate-contracts`.

**Spec:** `docs/superpowers/specs/2026-07-18-spaces-first-navigation-design.md` §2, §3.1, §6.
**Worktree:** `/Users/rabble/code/explorations/riot-nav`, branch `design/spaces-first-nav`.
**Shared-checkout:** `gh pr list --search "spaces-first OR following OR registry"` before AND during; pathspec commits only; `community_registry.rs`/`mobile_state.rs` are high-traffic.

---

## Data-model decisions (locked; resolve Architect REQUIRED-2)

1. **`Following` and `Personal` are additive `Relationship` variants** (wire codes 3, 4). `REGISTRY_VERSION` stays `1`; `RECORD_FIELDS` stays `9`. No migration: the registry is device-local, so no existing record carries the new codes, and `from_wire` already returns `None` (→ `RegistryCorrupt`) for unknown codes — unchanged for old clients.
2. **Followed sites are author-less** — they do NOT go in `CommunityRow` (whose `available` = "author loadable"). Add a **parallel `list_followed_sites() -> Vec<FollowedSiteRow>`**; the shell merges the two lists into the tiered view.
3. **`Personal`** marks the one distinguished personal home space (author-bearing — it is a real owned space); it rides `CommunityRow` with the new relationship. Its *contents* (drafts/profile) are existing surfaces; Rung 4 builds the detail.

## File structure

- `crates/riot-ffi/src/community_registry.rs` — add `Following`/`Personal` to `Relationship` + wire codec.
- `crates/riot-ffi/src/mobile_api.rs` — add `Following`/`Personal` to `CommunityRelationship`; new `FollowedSiteRow` `uniffi::Record`; `list_followed_sites` on the profile.
- `crates/riot-ffi/src/mobile_state.rs` — `list_followed_sites` implementation; map new relationships in `community_row()`.
- `crates/riot-ffi/tests/` — new contract tests.

---

## Task 1: Add `Following` + `Personal` to the registry `Relationship` (additive, no migration)

**Files:**
- Modify: `crates/riot-ffi/src/community_registry.rs:48-70` (`Relationship` enum + `to_wire`/`from_wire`)
- Test: `crates/riot-ffi/src/community_registry.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test** (add to the existing test module)

```rust
#[test]
fn following_and_personal_round_trip_through_wire_without_a_version_bump() {
    for r in [
        Relationship::Organizer,
        Relationship::Member,
        Relationship::PublicReader,
        Relationship::Following,
        Relationship::Personal,
    ] {
        assert_eq!(Relationship::from_wire(r.to_wire()), Some(r));
    }
    // Additive codes; unknown stays rejected.
    assert_eq!(Relationship::from_wire(5), None);
    // Migration guard: the version constant did not move.
    assert_eq!(REGISTRY_VERSION, 1);
    assert_eq!(RECORD_FIELDS, 9);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p riot-ffi following_and_personal_round_trip --all-features`
Expected: FAIL — `no variant named Following` / `Personal`.

- [ ] **Step 3: Implement** — extend the enum + both wire maps (codes 3, 4):

```rust
pub(crate) enum Relationship {
    Organizer,
    Member,
    PublicReader,
    /// A composite indymedia site the user follows (author-less; surfaced via
    /// list_followed_sites, but a held row may still record the relation).
    Following,
    /// The user's own distinguished personal home space.
    Personal,
}
// to_wire: Following => 3, Personal => 4
// from_wire: 3 => Some(Following), 4 => Some(Personal)
```

Add `#[derive(PartialEq, Eq)]` to `Relationship` if not already present (the test compares).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p riot-ffi following_and_personal_round_trip --all-features`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/community_registry.rs
git commit -m "feat(spaces/rung1): additive Following+Personal registry relationships (no migration)"
```

---

## Task 2: Backward-compat proof — an old (pre-Following) registry record still decodes

**Files:**
- Test: `crates/riot-ffi/src/community_registry.rs` (inline test)

- [ ] **Step 1: Write the failing test** — encode a record the OLD way (codes 0-2 only), decode with the new code, assert success. (This is the migration-safety guarantee.)

```rust
#[test]
fn a_pre_following_record_still_decodes_after_the_additive_variants() {
    // Build a registry blob using only the historical relationships, then decode
    // it with the extended enum in place. Must round-trip unchanged.
    let historical = /* construct a registry with Organizer+Member+PublicReader
                        rows using the existing encode helper */;
    let blob = encode_registry(&historical);
    let decoded = decode_registry(&blob).expect("old record must still decode");
    assert_eq!(decoded, historical);
}
```

*(Engineer note: use whatever the existing `encode_registry`/`decode_registry` (or equivalently-named) helpers are — grep the file; the point is a real encode→decode over the pre-existing variants with the new enum compiled in.)*

- [ ] **Step 2: Run to verify it fails or passes** — if the helpers make it pass immediately, that's the correct outcome (additive change is compatible); keep the test as a regression guard. If it does not compile, fix the helper names.

Run: `cargo test -p riot-ffi a_pre_following_record_still_decodes --all-features`
Expected: PASS (additive change is compatible).

- [ ] **Step 3: (no impl needed if green)** — if red, the change was not additive; STOP and reconsider (do not bump the version silently).

- [ ] **Step 4: Commit**

```bash
git add crates/riot-ffi/src/community_registry.rs
git commit -m "test(spaces/rung1): prove pre-Following registry records still decode"
```

---

## Task 3: Extend the FFI `CommunityRelationship` enum (UniFFI)

**Files:**
- Modify: `crates/riot-ffi/src/mobile_api.rs:19-26` (`CommunityRelationship`)
- Modify: `crates/riot-ffi/src/mobile_state.rs` (`community_row()` mapping ~:2132 — add the new arms)
- Test: `crates/riot-ffi/tests/` (new `spaces_relationships_contract.rs`)

- [ ] **Step 1: Write the failing contract test**

```rust
use riot_ffi::CommunityRelationship;

#[test]
fn community_relationship_has_following_and_personal() {
    // The FFI enum the shells switch on must carry the two new tiers.
    let _ = CommunityRelationship::Following;
    let _ = CommunityRelationship::Personal;
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p riot-ffi --test spaces_relationships_contract --all-features`
Expected: FAIL — `no variant Following`.

- [ ] **Step 3: Implement** — add both variants to `CommunityRelationship` with doc comments; map from core `Relationship` (add the two arms wherever `Relationship`→`CommunityRelationship` is converted — grep for `CommunityRelationship::Organizer`). Do NOT add logic; it is a pure tag passthrough (relationship is core-derived, `mobile_api.rs:16`).

- [ ] **Step 4: Run to verify it passes** (and the whole workspace still compiles — this is a `uniffi::Enum` change).

Run: `cargo test -p riot-ffi --test spaces_relationships_contract --all-features`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_api.rs crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/spaces_relationships_contract.rs
git commit -m "feat(spaces/rung1): FFI CommunityRelationship gains Following + Personal"
```

---

## Task 4: `FollowedSiteRow` record + `list_followed_sites()` (author-less, parallel list)

**Files:**
- Modify: `crates/riot-ffi/src/mobile_api.rs` (new `FollowedSiteRow` `uniffi::Record` + `list_followed_sites` method on the profile)
- Modify: `crates/riot-ffi/src/mobile_state.rs` (implementation)
- Test: `crates/riot-ffi/tests/followed_sites_contract.rs` (new)

- [ ] **Step 1: Write the failing contract test** — a profile with a followed site surfaces it in `list_followed_sites` with the site's owned-root id (hex) and honest state; NOT in `list_communities`.

```rust
use riot_ffi::open_local_profile;

#[test]
fn a_followed_site_appears_in_list_followed_sites_not_list_communities() {
    let profile = open_local_profile().unwrap();
    // follow a site by ticket (Rung-1 minimal follow entry; see step 3 note)
    let root_hex = profile.follow_site_for_test(vec![0x11; 32]).unwrap();
    let followed = profile.list_followed_sites().unwrap();
    assert!(followed.iter().any(|r| r.root == root_hex));
    // author-less: it is NOT a community row
    let communities = profile.list_communities().unwrap();
    assert!(communities.iter().all(|c| c.namespace_id != root_hex));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p riot-ffi --test followed_sites_contract --all-features`
Expected: FAIL — `no method list_followed_sites` / `FollowedSiteRow` undefined.

- [ ] **Step 3: Implement**

Define the record (transport-safe: hex ids, stable tokens, no secrets):

```rust
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FollowedSiteRow {
    /// Owned site root (namespace id), lowercase hex (64 chars).
    pub root: String,
    /// Best-known title (from the resolved manifest, or a placeholder token
    /// until first resolve). Core-resolved, never caller-asserted.
    pub title: String,
    /// Honest row state token: "available" / "pending-first-sync" /
    /// "transport-blocked" / "degraded". Drives §3.1 row rendering.
    pub state: String,
    /// True when the site requires an unavailable transport (require:arti) —
    /// the row shows "requires Tor — unavailable" without drilling in (S1).
    pub transport_blocked: bool,
}
```

Add `list_followed_sites` on the profile + `mobile_state::list_followed_sites`, reading followed roots from the registry (`Following` rows) / the store. **Rung-1 minimal follow entry:** since the real `follow_site(ticket)` FFI is a later step (and the ticket/transport parsing is Unit 5 territory), gate the persistence path behind a `#[cfg(test)]` `follow_site_for_test(root: Vec<u8>)` that records a `Following` root, so this row/list surface is testable now; the production `follow_site(ticket)` entry point is Rung 5 (§7). Document this seam explicitly in a code comment.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p riot-ffi --test followed_sites_contract --all-features`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_api.rs crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/followed_sites_contract.rs
git commit -m "feat(spaces/rung1): FollowedSiteRow + list_followed_sites (author-less parallel list)"
```

---

## Task 5: Exposure-boundary guard — no secrets in the followed-site or personal rows (Security S2)

**Files:**
- Test: `crates/riot-ffi/tests/spaces_exposure_boundary.rs` (new)

- [ ] **Step 1: Write the test** — assert the row records carry only ids/tokens/bools, never key material. (A structural guard: the records have no `Vec<u8>` secret field; and a runtime check that a followed row's `root` is the site root, not any owner/root SECRET.)

```rust
#[test]
fn followed_site_row_exposes_only_public_identifiers() {
    let profile = riot_ffi::open_local_profile().unwrap();
    let root_hex = profile.follow_site_for_test(vec![0x22; 32]).unwrap();
    let row = profile.list_followed_sites().unwrap().into_iter()
        .find(|r| r.root == root_hex).unwrap();
    // root is a 32-byte id as 64 hex chars; nothing secret-sized or non-hex.
    assert_eq!(row.root.len(), 64);
    assert!(row.root.chars().all(|c| c.is_ascii_hexdigit()));
    // (Compile-time: FollowedSiteRow has no Vec<u8>/secret field — reviewed.)
}
```

- [ ] **Step 2-4: Run / (green expected) / Commit**

Run: `cargo test -p riot-ffi --test spaces_exposure_boundary --all-features` → PASS.

```bash
git add crates/riot-ffi/tests/spaces_exposure_boundary.rs
git commit -m "test(spaces/rung1): followed/personal rows expose only public ids (S2 boundary)"
```

---

## Task 6: Regenerate bindings + verify native rebuild (UniFFI gate)

**Files:** none committed (generated artifacts are gitignored).

- [ ] **Step 1: Regenerate bindings** (new `uniffi::Record`/`Enum`):

Run: `cargo run --locked --package xtask -- generate-bindings`
Expected: `generate-bindings: PASS`; `FollowedSiteRow`/`Following`/`Personal` present in `build/generated/riot-ffi/riot_ffi.swift`.

- [ ] **Step 2: Full native cross-compile** (proves no checksum drift on all 5 targets):

Run: `sh scripts/conference/build-native-core.sh`
Expected: `native-core-package: built iOS device/simulator, macOS arm64, and Android arm64/x86_64`.

- [ ] **Step 3: Full workspace gates**

Run: `cargo test --workspace --all-features` (all pass) · `cargo clippy --workspace --all-features --all-targets -- -D warnings` (clean) · `cargo fmt --all -- --check` (clean) · `cargo run -p xtask -- validate-contracts` (PASS).

- [ ] **Step 4: Commit** (docs/notes only if any; the source commits already landed).

---

## Self-review (against spec §2, §3.1, §6)

- §6.2 followed-site state (new follow/list FFI, not author-derived) → Tasks 4/5. ✓
- §6.3 durable registry format decision (additive, no migration) → Tasks 1/2 (proven by the backward-compat test). ✓
- §2 three tiers' relationship tags → Tasks 1/3. ✓
- §3.1 row honest state + transport-blocked-at-row (S1) → `FollowedSiteRow.state`/`transport_blocked` (Task 4). ✓
- §6.4 exposure boundary (S2) → Task 5. ✓
- §6.5 no logic in shell → all tags are core-derived passthroughs. ✓
- UniFFI gate → Task 6. ✓
- **Deferred to later rungs (correct):** the real `follow_site(ticket)` entry (Rung 5), the shell tiered list (Rung 2), the followed-site detail render (Rung 3, gated on PR #46), the personal-home detail (Rung 4).

---

## Roadmap — rungs 2–5 (each gets its own plan when its predecessor lands)

- **Rung 2 — two-pane shell skeleton** (iOS+Android+macOS): space list as root (reuse `NavigationSplitView` `ConferenceShellView.swift:640`), merge `list_communities` + `list_followed_sites` into tiered groups, relocate Home/People/Nearby/Tools under a selected-space detail **verbatim**, launch-restore (`CommunityReturnOutcome.decide` `CommunityChooser.swift:165`), row state vocabulary + a11y (§3.1/§3.3), Tools off the top level. pbxproj hazard; rewrite Shell/Tab nav tests. Big rung — plan it as its own sub-ladder.
- **Rung 3 — followed-site detail** (GATED on PR #46 merged): render `ResolvedCompositeSite`/`SiteTrustTier`/`SiteDegradation`/`SiteItemTreatment` in the right pane; non-spoofable trust-tier chrome (§4.1, icon+shape+label, shell paints around content); Follow/Unfollow control.
- **Rung 4 — "your space" personal home** (bounded, §2.1): the `Personal` tier detail (profile/drafts/posts/settings); exposure boundary (§6.4).
- **Rung 5 — Unit 6 obligations:** editor-invite handshake (needs `delegate_section` FFI — a gap noted in the Unit 6 plan), QR gen + camera scan (both platforms), writer expired-cap warning (renders Unit 4 `writer_cap_state`), **seizure disclosure pinned to mint-masthead** (§4.4, blocking), compose-time `require:arti` notice.
