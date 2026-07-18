# Spaces-First — Rung 1: Core following + personal relationships — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add the core/FFI state that lets the shell draw all three space tiers — `Following` (indymedia sites you follow) and `Personal` (your own home space) alongside today's `Organizer/Member/PublicReader` — WITHOUT a durable registry migration, and expose followed sites through a distinct author-less list.

**Architecture:** The community registry (`crates/riot-ffi/src/community_registry.rs`) is a **device-local** CBOR record (stored only in `local_state`, never exchanged over the sync wire — verified); a `REGISTRY_VERSION` bump treats old data as corrupt, so we AVOID one. New relationships are **additive enum wire codes** (safe: an old client never wrote them, `RECORD_FIELDS` unchanged). Followed composite sites have no author, so they are surfaced through a **separate `list_followed_sites()`** with a distinct row type, and are **excluded from `list_communities`** by a relationship filter; `Personal` (author-bearing) rides `CommunityRow`. No business logic leaves core; the shell renders.

**Tech Stack:** Rust (riot-ffi, riot-core), UniFFI, minicbor. Gates: `cargo test --workspace --all-features`, `cargo clippy --workspace --all-features --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo run -p xtask -- validate-contracts`.

**Spec:** `docs/superpowers/specs/2026-07-18-spaces-first-navigation-design.md` §2, §3.1, §6.
**Worktree:** `/Users/rabble/code/explorations/riot-nav`, branch `design/spaces-first-nav`.
**Shared-checkout:** `gh pr list --search "spaces-first OR following OR registry"` before AND during; pathspec commits only; `community_registry.rs`/`mobile_state.rs` are high-traffic.

---

## Data-model decisions (locked; verified against HEAD)

1. **`Following` and `Personal` are additive `Relationship` variants** (wire codes 3, 4). `REGISTRY_VERSION` stays `1`; `RECORD_FIELDS` stays `9`. No migration: the registry is device-local, no existing record carries the new codes, and `from_wire` already returns `None` (→ `RegistryCorrupt`) for unknown codes.
2. **Followed sites are author-less** — NOT in `CommunityRow` (whose `available` = author-loadable, `mobile_state.rs:2132-2145`). They persist as `Following` registry records, are surfaced by **`list_followed_sites()`**, and are **filtered OUT of `list_communities`** (which today maps every registry record — `mobile_state.rs:2300`).
3. **`Personal`** marks the one distinguished personal home space (author-bearing); it rides `CommunityRow`/`list_communities` (kept IN, not filtered). Its *contents* detail is Rung 4; which space is `Personal` (assignment) is Rung 4.

**Match-exhaustiveness note (verified):** `relationship_to_ffi` (`mobile_state.rs:2121-2127`) is an exhaustive match with **no wildcard**. Extending core `Relationship` and FFI `CommunityRelationship` therefore MUST land in the SAME commit as that mapping arm, or `riot-ffi` fails to compile (E0004). Task 1 does all three together.

## File structure

- `crates/riot-ffi/src/community_registry.rs` — `Following`/`Personal` on `Relationship` + wire codec.
- `crates/riot-ffi/src/mobile_api.rs` — `Following`/`Personal` on `CommunityRelationship`; `FollowedSiteRow` `uniffi::Record`; `list_followed_sites` method.
- `crates/riot-ffi/src/mobile_state.rs` — `relationship_to_ffi` arms; `list_communities` `Following`-exclusion filter; `list_followed_sites` impl.
- `crates/riot-ffi/tests/` — new contract tests.

---

## Task 1: Extend `Relationship` (core) + `CommunityRelationship` (FFI) + the mapping arm — ONE compiling commit

**Files:**
- Modify: `crates/riot-ffi/src/community_registry.rs:48-70` (enum + `to_wire`/`from_wire`)
- Modify: `crates/riot-ffi/src/mobile_api.rs:19-26` (`CommunityRelationship`)
- Modify: `crates/riot-ffi/src/mobile_state.rs:2121-2127` (`relationship_to_ffi` — add the two arms)
- Test: `crates/riot-ffi/src/community_registry.rs` inline + `crates/riot-ffi/tests/spaces_relationships_contract.rs` (new)

- [ ] **Step 1: Write the failing tests**

Inline in `community_registry.rs` test module:
```rust
#[test]
fn following_and_personal_round_trip_through_wire_without_a_version_bump() {
    for r in [
        Relationship::Organizer, Relationship::Member, Relationship::PublicReader,
        Relationship::Following, Relationship::Personal,
    ] {
        assert_eq!(Relationship::from_wire(r.to_wire()), Some(r));
    }
    assert_eq!(Relationship::from_wire(5), None);
    assert_eq!(REGISTRY_VERSION, 1);
    assert_eq!(RECORD_FIELDS, 9);
}
```
New `crates/riot-ffi/tests/spaces_relationships_contract.rs`:
```rust
use riot_ffi::CommunityRelationship;
#[test]
fn community_relationship_has_following_and_personal() {
    let _ = CommunityRelationship::Following;
    let _ = CommunityRelationship::Personal;
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p riot-ffi following_and_personal_round_trip --all-features` → FAIL (`no variant Following`). The FFI crate will also refuse to compile once the core variants are added without the mapping arm — which is exactly why Step 3 does all three edits at once.

- [ ] **Step 3: Implement all three together**

`community_registry.rs` — extend enum (it already derives `PartialEq, Eq` at :47) + both wire maps:
```rust
pub(crate) enum Relationship {
    Organizer, Member, PublicReader,
    /// A composite indymedia site the user follows (author-less; surfaced via
    /// list_followed_sites, filtered out of list_communities).
    Following,
    /// The user's own distinguished personal home space (author-bearing).
    Personal,
}
// to_wire: Following => 3, Personal => 4
// from_wire: 3 => Some(Following), 4 => Some(Personal)
```
`mobile_api.rs` — add to `CommunityRelationship` (uniffi::Enum) with doc comments:
```rust
    /// A composite indymedia site the user follows (read-mostly).
    Following,
    /// The user's own personal home space.
    Personal,
```
`mobile_state.rs:2121` — add the two arms to the exhaustive `relationship_to_ffi` match:
```rust
    Relationship::Following => CommunityRelationship::Following,
    Relationship::Personal => CommunityRelationship::Personal,
```

- [ ] **Step 4: Run to verify they pass AND the crate compiles**

Run: `cargo test -p riot-ffi --all-features following_and_personal_round_trip` and `cargo test -p riot-ffi --test spaces_relationships_contract --all-features` → both PASS. `cargo build -p riot-ffi --all-features` → compiles (no E0004).

- [ ] **Step 5: Commit** (one green commit — core enum + FFI enum + mapping arm together)

```bash
git add crates/riot-ffi/src/community_registry.rs crates/riot-ffi/src/mobile_api.rs crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/spaces_relationships_contract.rs
git commit -m "feat(spaces/rung1): additive Following+Personal relationships (core+FFI+mapping, no migration)"
```

---

## Task 2: Backward-compat proof — a pre-Following registry record still decodes

**Files:** Test: `crates/riot-ffi/src/community_registry.rs` (inline)

- [ ] **Step 1: Write the test** (real API is `CommunityRegistry::encode(&self) -> Vec<u8>` / `CommunityRegistry::decode(&[u8])`; both exist, `CommunityRegistry`/`CommunityRecord` derive `PartialEq, Eq`):

```rust
#[test]
fn a_pre_following_registry_round_trips_after_the_additive_variants() {
    // Build a registry using ONLY the historical relationships, encode, decode
    // with the extended enum compiled in — must be byte-identical round-trip.
    let mut reg = CommunityRegistry::default(); // or the existing constructor — grep
    reg.upsert(/* a record with Relationship::Organizer */);
    reg.upsert(/* a record with Relationship::Member */);
    reg.upsert(/* a record with Relationship::PublicReader */);
    let blob = reg.encode();
    assert_eq!(CommunityRegistry::decode(&blob).expect("old record decodes"), reg);
}
```
*(Engineer note: match the real `CommunityRegistry` constructor/insert method names — grep `impl CommunityRegistry`. The primitives exist; this is a naming seam, not a placeholder.)*

- [ ] **Step 2: Run** → `cargo test -p riot-ffi a_pre_following_registry --all-features` → PASS (additive change is compatible).
- [ ] **Step 3: If RED**, the change was not additive — STOP, do not bump the version.
- [ ] **Step 4: Commit**

```bash
git add crates/riot-ffi/src/community_registry.rs
git commit -m "test(spaces/rung1): prove pre-Following registry records still decode"
```

---

## Task 3: `FollowedSiteRow` + `list_followed_sites()` + `list_communities` exclusion filter

**Files:**
- Modify: `crates/riot-ffi/src/mobile_api.rs` (`FollowedSiteRow` record + `list_followed_sites` method)
- Modify: `crates/riot-ffi/src/mobile_state.rs` (`list_followed_sites` impl; **add `Following`-exclusion filter to `list_communities` at ~:2300**)
- Test: `crates/riot-ffi/tests/followed_sites_contract.rs` (new)

- [ ] **Step 1: Write the failing contract test** — a followed site appears in `list_followed_sites` (by owned-root hex) and is EXCLUDED from `list_communities`:

```rust
use riot_ffi::open_local_profile;
#[test]
fn a_followed_site_is_in_list_followed_sites_and_excluded_from_list_communities() {
    let profile = open_local_profile().unwrap();
    let root_hex = profile.follow_site_for_test(vec![0x11; 32]).unwrap();
    assert!(profile.list_followed_sites().unwrap().iter().any(|r| r.root == root_hex));
    // author-less: filtered OUT of the community list
    assert!(profile.list_communities().unwrap().iter().all(|c| c.namespace_id != root_hex));
}
```

- [ ] **Step 2: Run** → `cargo test -p riot-ffi --test followed_sites_contract --all-features` → FAIL (`no method list_followed_sites`).

- [ ] **Step 3: Implement**

Record (transport-safe: hex ids, stable tokens, NO secret/`Vec<u8>` field):
```rust
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FollowedSiteRow {
    /// Owned site root (namespace id), lowercase hex (64 chars).
    pub root: String,
    /// Core-resolved title (from the resolved manifest, or a placeholder token
    /// until first resolve). Never caller-asserted.
    pub title: String,
    /// Honest row-state token, aligned to spec §3.1 where meaningful for a
    /// followed site: "available" / "pending-first-sync" / "transport-blocked"
    /// / "degraded". (syncing/quarantined are community-only.)
    pub state: String,
    /// True iff the site requires an unavailable transport (require:arti) — the
    /// row shows "requires Tor — unavailable" without drilling in (S1).
    pub transport_blocked: bool,
}
```
- `list_followed_sites`: read `Following` roots from the registry, build a `FollowedSiteRow` per root (title/state from the resolved manifest where available, else `pending-first-sync`).
- **`list_communities` filter (the fix):** at `mobile_state.rs:2300` where every registry record is mapped, skip records whose relationship is `Following` (`.filter(|rec| rec.relationship != Relationship::Following)`), so a followed root never appears as a `CommunityRow`. `Personal` is NOT filtered (rides `CommunityRow`).
- **Test seam:** the real `follow_site(ticket)` is Rung 5 (ticket/transport parsing is Unit 5 territory). Add a `#[cfg(test)] follow_site_for_test(root: Vec<u8>) -> Result<String, MobileError>` that persists a `Following` registry record and returns its root hex, so the row/list/filter are testable now. Document this seam in a code comment naming Rung 5 as the production entry.

- [ ] **Step 4: Run** → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_api.rs crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/followed_sites_contract.rs
git commit -m "feat(spaces/rung1): FollowedSiteRow + list_followed_sites + list_communities Following-exclusion"
```

---

## Task 4: Exposure-boundary guard — no secrets in the followed-site row (Security S2)

**Files:** Test: `crates/riot-ffi/tests/spaces_exposure_boundary.rs` (new)

- [ ] **Step 1: Write the test**

```rust
#[test]
fn followed_site_row_exposes_only_public_identifiers() {
    let profile = riot_ffi::open_local_profile().unwrap();
    let root_hex = profile.follow_site_for_test(vec![0x22; 32]).unwrap();
    let row = profile.list_followed_sites().unwrap().into_iter()
        .find(|r| r.root == root_hex).unwrap();
    assert_eq!(row.root.len(), 64);
    assert!(row.root.chars().all(|c| c.is_ascii_hexdigit()));
    // Compile-time: FollowedSiteRow has no Vec<u8>/secret field — reviewed.
}
```

- [ ] **Step 2-4: Run (PASS) / Commit**

```bash
git add crates/riot-ffi/tests/spaces_exposure_boundary.rs
git commit -m "test(spaces/rung1): followed rows expose only public ids (S2 boundary)"
```

---

## Task 5: Regenerate bindings + verify native rebuild (UniFFI gate)

**Files:** none committed (generated artifacts gitignored).

- [ ] **Step 1:** `cargo run --locked --package xtask -- generate-bindings` → `generate-bindings: PASS`; `FollowedSiteRow`/`Following`/`Personal` present in `build/generated/riot-ffi/riot_ffi.swift`.
- [ ] **Step 2:** `sh scripts/conference/build-native-core.sh` → `built iOS device/simulator, macOS arm64, and Android arm64/x86_64`.
- [ ] **Step 3:** `cargo test --workspace --all-features` (pass) · `cargo clippy --workspace --all-features --all-targets -- -D warnings` (clean) · `cargo fmt --all -- --check` (clean) · `cargo run -p xtask -- validate-contracts` (PASS).
- [ ] **Step 4:** Commit any doc/notes; source commits already landed.

---

## Self-review (against spec §2, §3.1, §6)

- §6.2 followed-site state (new, author-less, filtered out of communities) → Task 3. ✓
- §6.3 durable registry format (additive, no migration) → Tasks 1/2 (backward-compat proof). ✓
- §2 three-tier relationship tags → Task 1 (core+FFI+mapping, one compiling commit). ✓
- §3.1 row honest state + transport-blocked-at-row (S1) → `FollowedSiteRow.state`/`transport_blocked` (Task 3). ✓
- §6.4 exposure boundary (S2) → Task 4. ✓
- §6.5 no logic in shell → core-derived passthroughs. ✓
- UniFFI gate → Task 5. ✓
- **Deferred to later rungs:** real `follow_site(ticket)` (Rung 5); shell tiered list (Rung 2); followed-site render (Rung 3, PR #46-gated — now on main); personal detail + Personal-assignment (Rung 4).

## Roadmap — rungs 2–5 (each planned when its predecessor lands)

- **Rung 2 — two-pane shell skeleton** (iOS+Android+macOS): space list root (reuse `NavigationSplitView` `ConferenceShellView.swift:640`), merge `list_communities` + `list_followed_sites` into tiered groups, relocate Home/People/Nearby/Tools under a selected-space detail **verbatim**, launch-restore (`CommunityReturnOutcome.decide` `CommunityChooser.swift:165`), row state vocabulary + a11y (§3.1/§3.3), Tools off the top level. pbxproj hazard; rewrite Shell/Tab nav tests. (Large — its own sub-ladder.)
- **Rung 3 — followed-site detail** (Unit 4 `ResolvedCompositeSite` + store-wired `resolve_composite_site` are now on main): render editorial/comments/wire in the right pane; non-spoofable trust-tier chrome (§4.1); Follow/Unfollow.
- **Rung 4 — "your space" personal home** (bounded, §2.1): Personal-tier assignment + detail (profile/drafts/posts/settings); personal-contents exposure boundary (§6.4).
- **Rung 5 — Unit 6 obligations:** editor-invite handshake (needs `delegate_section` FFI), QR gen + camera scan, writer expired-cap warning (renders `writer_cap_state`), **seizure disclosure pinned to mint-masthead** (§4.4, blocking), compose-time `require:arti` notice, real `follow_site(ticket)`.
