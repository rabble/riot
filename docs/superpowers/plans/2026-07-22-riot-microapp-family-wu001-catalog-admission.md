# WU-001 — Catalog Split + Legacy Resolver + Capacity Preflight (Rust core + FFI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: metaswarm orchestrated-execution (4-phase loop) per the user's chosen execution method. Steps use checkbox (`- [ ]`) syntax. Parent: `2026-07-22-riot-microapp-family-master-plan.md`. Spec: `docs/superpowers/specs/2026-07-22-riot-microapp-family-design.md` §"Content identity and upgrades".

**Goal:** Give the Rust core a two-catalog model (advertised `CURRENT_STARTER_CATALOG` + non-advertised `LEGACY_BUILTIN_CATALOG`), a generation-aware bootstrap, a raised 32-app count cap, a 3 MiB aggregate pair-byte quota, and one pure host-agnostic admission report — all enforced *before* any runtime/store mutation.

**Architecture:** All logic in `crates/riot-core` (catalog + pure admission report) and `crates/riot-ffi` (in-memory `LocalProfile` generation field, capacity preflight in `install_pair`). No native/disk changes here — persisting the generation marker to Android/iOS/macOS storage and the Android 4 MiB codec-ceiling preflight are **WU-001N** (separate JIT plan). No visual work.

**Tech Stack:** Rust 2021, existing `riot-core::apps` module, `riot-ffi` UniFFI state.

**Scope boundary (do NOT exceed):**
- Create/modify only: `crates/riot-core/src/apps/starter.rs`, `crates/riot-core/src/apps/admission.rs` (new), `crates/riot-core/src/apps/inventory.rs` (new), `crates/riot-core/src/apps/mod.rs`, `crates/riot-ffi/src/mobile_state.rs`, and these test files: `crates/riot-core/tests/apps_starter.rs`, `crates/riot-core/tests/apps_admission.rs` (new), `crates/riot-core/tests/apps_inventory.rs` (new), `crates/riot-ffi/tests/apps_contract.rs`, `crates/riot-ffi/tests/mobile_fail_closed.rs`, `crates/riot-ffi/tests/mobile_refusal_surface.rs`.
- **Never** touch: `fixtures/apps/checklist.*` (byte-frozen), any native shell source, `fixtures/apps/*.cbor` bytes.
- **`STARTER_CATALOG` stays as a plain alias of `CURRENT_STARTER_CATALOG`** (NO `#[deprecated]` — that attribute fails `clippy -D warnings` via `demo_fixture.rs`). Every existing `STARTER_CATALOG` use (`demo_fixture.rs:45/309/399`, `mobile_state.rs:3766/3770/3933/4343/4381/4433/4481/4593`, and all test-file uses) then compiles unchanged and stays correct — they all want the advertised catalog, which the alias points at. Do NOT migrate them; that keeps this WU inside `demo_fixture.rs`-free scope.
- `CURRENT_STARTER_CATALOG` is **structurally seeded from the existing v1 bytes** this WU. Real v2 bytes do not exist yet and MUST NOT be invented (spec forbids pre-pinning v2 IDs). Each Slice-4 WU re-points one `CURRENT_STARTER_CATALOG` entry to its generated v2 pair.

**Verified anchors (origin/main 49dbe38):** `STARTER_CATALOG` `starter.rs:81`; `verify_starter_catalog` `starter.rs:94`; `starter_pair_bytes(catalog, id)` `index.rs:200`; `MAX_INSTALLED_APPS=16` `mobile_state.rs:46`; `install_pair` `mobile_state.rs:2408`, cap check `:2426`; `StoredInstalledApp{app_id,manifest_bytes,bundle_bytes}` `:196`; `MobileError::{SessionLimit,StoreFull}` `mobile_api.rs:190`; `LocalProfile` `mobile_state.rs:68`.

---

## File structure

| File | Responsibility |
| --- | --- |
| `crates/riot-core/src/apps/starter.rs` | Two catalog consts + generation→catalog selector; keep `verify_starter_catalog` |
| `crates/riot-core/src/apps/admission.rs` (new) | Pure `AdmissionReport` — count + aggregate-byte preflight, host-agnostic, no I/O |
| `crates/riot-core/src/apps/mod.rs` | `pub mod admission;` + re-exports |
| `crates/riot-ffi/src/mobile_state.rs` | `starter_catalog_generation` field, 32-cap, 3 MiB quota via `admission::preflight`, generation-aware bootstrap |
| `crates/riot-core/tests/apps_admission.rs` (new) | Pure admission-report boundary tests |
| `crates/riot-core/tests/apps_starter.rs` | Catalog-split assertions (update existing) |
| `crates/riot-ffi/tests/apps_contract.rs` | 32-cap + quota + generation bootstrap FFI tests (extend) |

---

## Task 1: Split the catalog into current + legacy consts

**Files:**
- Modify: `crates/riot-core/src/apps/starter.rs:80-90`
- Test: `crates/riot-core/tests/apps_starter.rs`

- [ ] **Step 1: Write the failing test** — append to `crates/riot-core/tests/apps_starter.rs`:

```rust
use riot_core::apps::starter::{
    verify_starter_catalog, CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG,
};

#[test]
fn current_and_legacy_catalogs_each_have_exactly_eight_pairs() {
    assert_eq!(CURRENT_STARTER_CATALOG.len(), 8);
    assert_eq!(LEGACY_BUILTIN_CATALOG.len(), 8);
}

#[test]
fn every_current_and_legacy_pair_verifies() {
    // Invalid pairs are silently dropped by verify_starter_catalog, so a
    // full-length result proves all eight in each catalog are valid.
    assert_eq!(verify_starter_catalog(CURRENT_STARTER_CATALOG).len(), 8);
    assert_eq!(verify_starter_catalog(LEGACY_BUILTIN_CATALOG).len(), 8);
}

#[test]
fn current_catalog_is_seeded_from_legacy_bytes_until_v2_lands() {
    // WU-001 seeds CURRENT from the same v1 bytes; each Slice-4 WU re-points
    // one entry. Until then the two catalogs derive identical app IDs.
    let current: Vec<_> = verify_starter_catalog(CURRENT_STARTER_CATALOG)
        .into_iter().map(|a| a.app_id).collect();
    let legacy: Vec<_> = verify_starter_catalog(LEGACY_BUILTIN_CATALOG)
        .into_iter().map(|a| a.app_id).collect();
    assert_eq!(current, legacy);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core --test apps_starter current_and_legacy`
Expected: FAIL — `CURRENT_STARTER_CATALOG`/`LEGACY_BUILTIN_CATALOG` unresolved.

- [ ] **Step 3: Implement** — in `starter.rs`, replace the single `STARTER_CATALOG` (lines 80-90) with two named consts sharing the same byte constants, and keep a back-compat alias so existing call sites compile:

```rust
/// The advertised, auto-installed catalog for a fresh generation-2 profile.
/// Seeded from the v1 bytes in WU-001; each Slice-4 work unit re-points one
/// entry to its generated v2 pair. Never resolve a held app from name/version —
/// only by exact app ID.
pub const CURRENT_STARTER_CATALOG: &[(&[u8], &[u8])] = &[
    (CHECKLIST_MANIFEST, CHECKLIST_BUNDLE),
    (SUPPLY_BOARD_MANIFEST, SUPPLY_BOARD_BUNDLE),
    (ROLL_CALL_MANIFEST, ROLL_CALL_BUNDLE),
    (QUICK_POLL_MANIFEST, QUICK_POLL_BUNDLE),
    (CHAT_MANIFEST, CHAT_BUNDLE),
    (DISPATCHES_MANIFEST, DISPATCHES_BUNDLE),
    (WIKI_MANIFEST, WIKI_BUNDLE),
    (PHOTO_WALL_MANIFEST, PHOTO_WALL_BUNDLE),
];

/// The frozen v1 built-ins. Never advertised as starters and never assigned a
/// synthetic directory timestamp; it exists only to resolve an already-held v1
/// ID for a generation-1/existing profile.
pub const LEGACY_BUILTIN_CATALOG: &[(&[u8], &[u8])] = &[
    (CHECKLIST_MANIFEST, CHECKLIST_BUNDLE),
    (SUPPLY_BOARD_MANIFEST, SUPPLY_BOARD_BUNDLE),
    (ROLL_CALL_MANIFEST, ROLL_CALL_BUNDLE),
    (QUICK_POLL_MANIFEST, QUICK_POLL_BUNDLE),
    (CHAT_MANIFEST, CHAT_BUNDLE),
    (DISPATCHES_MANIFEST, DISPATCHES_BUNDLE),
    (WIKI_MANIFEST, WIKI_BUNDLE),
    (PHOTO_WALL_MANIFEST, PHOTO_WALL_BUNDLE),
];

/// Plain back-compat alias: the advertised catalog. Every pre-split use
/// (`demo_fixture.rs`, the directory merge at `mobile_state.rs:3770`, test
/// fixtures) references the advertised catalog, which is exactly
/// `CURRENT_STARTER_CATALOG`, so the alias keeps them correct AND compiling with
/// zero migration. NOT `#[deprecated]` — that attribute would fail
/// `clippy -- -D warnings` on the same-crate `demo_fixture.rs` uses.
pub const STARTER_CATALOG: &[(&[u8], &[u8])] = CURRENT_STARTER_CATALOG;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core --test apps_starter current_and_legacy`
Expected: PASS. Then `cargo build -p riot-core -p riot-ffi` — Expected: builds cleanly; the plain alias leaves every existing `STARTER_CATALOG` reference valid with no warning. (Verify `grep -rn "STARTER_CATALOG" crates | grep -v "CURRENT_STARTER\|LEGACY_BUILTIN"` still resolves — all point at the alias.)

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/apps/starter.rs crates/riot-core/tests/apps_starter.rs
git commit -m "feat(core): split starter catalog into current + legacy built-in sets"
```

---

## Task 2: Generation→catalog selector (pure)

**Files:**
- Modify: `crates/riot-core/src/apps/starter.rs`
- Test: `crates/riot-core/tests/apps_starter.rs`

- [ ] **Step 1: Write the failing test**

```rust
use riot_core::apps::starter::bootstrap_catalog;

#[test]
fn generation_one_bootstraps_legacy_and_two_bootstraps_current() {
    // None == generation 1 (durable zero-byte encoding); Some(2) == fresh.
    assert!(std::ptr::eq(bootstrap_catalog(None), LEGACY_BUILTIN_CATALOG));
    assert!(std::ptr::eq(bootstrap_catalog(Some(1)), LEGACY_BUILTIN_CATALOG));
    assert!(std::ptr::eq(bootstrap_catalog(Some(2)), CURRENT_STARTER_CATALOG));
}
```

- [ ] **Step 2: Run to verify fail** — `cargo test -p riot-core --test apps_starter generation_one_bootstraps` → FAIL (unresolved `bootstrap_catalog`).

- [ ] **Step 3: Implement** — append to `starter.rs`:

```rust
/// Selects the built-in catalog a profile bootstraps by its persisted starter
/// generation. `None` is the durable encoding of generation 1 (an old profile
/// that predates the marker); it and an explicit `Some(1)` resolve the frozen
/// legacy built-ins. Generation 2 (fresh profiles) resolves the current v2
/// starters. Any unknown future generation falls back to legacy: an old binary
/// must never advertise a catalog it cannot fully resolve.
pub fn bootstrap_catalog(generation: Option<u8>) -> &'static [(&'static [u8], &'static [u8])] {
    match generation {
        Some(2) => CURRENT_STARTER_CATALOG,
        _ => LEGACY_BUILTIN_CATALOG,
    }
}
```

- [ ] **Step 4: Run to verify pass** — `cargo test -p riot-core --test apps_starter generation_one_bootstraps` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/apps/starter.rs crates/riot-core/tests/apps_starter.rs
git commit -m "feat(core): select bootstrap catalog by starter generation"
```

---

## Task 3: Pure admission report (count + aggregate bytes)

**Files:**
- Create: `crates/riot-core/src/apps/admission.rs`
- Modify: `crates/riot-core/src/apps/mod.rs`
- Test: `crates/riot-core/tests/apps_admission.rs` (new)

- [ ] **Step 1: Write the failing test** — create `crates/riot-core/tests/apps_admission.rs`:

```rust
use riot_core::apps::admission::{preflight, AdmissionOutcome, MAX_INSTALLED_APPS, MAX_AGGREGATE_PAIR_BYTES};

// Held pairs are (already_installed_app_ids_len, aggregate_bytes). preflight
// takes the current held count + held aggregate pair bytes and the prospective
// pair's (app_id_already_held, pair_bytes) and returns an outcome. No I/O.

#[test]
fn accepts_when_under_both_limits() {
    assert_eq!(
        preflight(31, 1000, false, 1000),
        AdmissionOutcome::Admit
    );
}

#[test]
fn count_boundary_31_to_32_admits_but_32_to_33_refuses_on_count() {
    assert_eq!(preflight(31, 0, false, 10), AdmissionOutcome::Admit);
    assert_eq!(preflight(32, 0, false, 10), AdmissionOutcome::RefuseCount);
}

#[test]
fn aggregate_byte_boundary_is_exact() {
    // Exactly 3 MiB total admits; one over refuses on bytes.
    assert_eq!(
        preflight(1, MAX_AGGREGATE_PAIR_BYTES - 10, false, 10),
        AdmissionOutcome::Admit
    );
    assert_eq!(
        preflight(1, MAX_AGGREGATE_PAIR_BYTES - 10, false, 11),
        AdmissionOutcome::RefuseBytes
    );
}

#[test]
fn reinstalling_a_held_id_is_idempotent_and_adds_no_bytes() {
    // An already-held ID neither increments count nor adds pair bytes, even at
    // the ceilings — idempotent restoration, not new admission.
    assert_eq!(preflight(32, MAX_AGGREGATE_PAIR_BYTES, true, 999_999), AdmissionOutcome::Admit);
}

#[test]
fn count_is_checked_before_bytes() {
    // Over on both: count wins so callers can map distinct copy deterministically.
    assert_eq!(
        preflight(32, MAX_AGGREGATE_PAIR_BYTES + 1, false, 1),
        AdmissionOutcome::RefuseCount
    );
}

#[test]
fn limits_match_spec() {
    assert_eq!(MAX_INSTALLED_APPS, 32);
    assert_eq!(MAX_AGGREGATE_PAIR_BYTES, 3 * 1024 * 1024);
}
```

- [ ] **Step 2: Run to verify fail** — `cargo test -p riot-core --test apps_admission` → FAIL (module missing).

- [ ] **Step 3: Implement** — create `crates/riot-core/src/apps/admission.rs`:

```rust
//! Pure, host-agnostic install-capacity preflight. Apple and Android call the
//! same function with their own current counts/bytes so they cannot diverge on
//! whether a pair fits. Byte and count limits are enforced here BEFORE any
//! runtime, trust, serving-store, or disk mutation. This module performs no I/O
//! and holds no state; the caller owns the profile lock.

/// Hard cap on distinct installed app IDs. Matches Android's persisted-profile
/// count cap so the disk format and runtime agree.
pub const MAX_INSTALLED_APPS: usize = 32;

/// Aggregate ceiling on the sum of installed manifest + bundle byte lengths
/// across all held IDs. Exactly 3 MiB. Pre-upgrade over-quota profiles are
/// restore-only grandfathered by the caller (not this function).
pub const MAX_AGGREGATE_PAIR_BYTES: usize = 3 * 1024 * 1024;

/// The distinct outcomes of a capacity preflight. Count and bytes are never
/// collapsed: the two conditions have different user-facing copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOutcome {
    Admit,
    RefuseCount,
    RefuseBytes,
}

/// Decide whether a prospective pair may be admitted.
///
/// * `held_count` — distinct app IDs already installed.
/// * `held_aggregate_bytes` — sum of held manifest+bundle byte lengths.
/// * `pair_already_held` — the prospective pair's ID is already installed
///   (idempotent restoration: no count/byte increase, always admits).
/// * `pair_bytes` — prospective pair's manifest.len() + bundle.len().
///
/// Count is checked before bytes so a caller mapping errors gets a
/// deterministic reason when both are exceeded.
pub fn preflight(
    held_count: usize,
    held_aggregate_bytes: usize,
    pair_already_held: bool,
    pair_bytes: usize,
) -> AdmissionOutcome {
    if pair_already_held {
        return AdmissionOutcome::Admit;
    }
    if held_count + 1 > MAX_INSTALLED_APPS {
        return AdmissionOutcome::RefuseCount;
    }
    if held_aggregate_bytes.saturating_add(pair_bytes) > MAX_AGGREGATE_PAIR_BYTES {
        return AdmissionOutcome::RefuseBytes;
    }
    AdmissionOutcome::Admit
}
```

Add to `crates/riot-core/src/apps/mod.rs` (with the other `pub mod` lines):

```rust
pub mod admission;
```

- [ ] **Step 4: Run to verify pass** — `cargo test -p riot-core --test apps_admission` → PASS (all 6).

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/apps/admission.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_admission.rs
git commit -m "feat(core): pure host-agnostic install capacity preflight (32 count, 3 MiB bytes)"
```

---

## Task 4: Wire preflight into FFI install_pair (32-cap + 3 MiB quota)

**Files:**
- Modify: `crates/riot-ffi/src/mobile_state.rs:46` (drop local `MAX_INSTALLED_APPS`), `:2408-2434` (`install_pair`)
- Test: `crates/riot-ffi/tests/apps_contract.rs`

- [ ] **Step 1: Write the failing test** — append to `crates/riot-ffi/tests/apps_contract.rs`. Use the existing in-memory profile + starter-install helpers already used by `every_built_in_in_the_catalog_installs_and_serves_its_pages_unsynced` (mirror that test's setup for constructing installable pairs):

```rust
#[test]
fn install_count_cap_is_thirty_two_not_sixteen() {
    // Install 32 distinct valid pairs, then assert the 33rd is refused with the
    // count-specific error (SessionLimit), not the byte error.
    let profile = /* open in-memory profile — mirror existing helper */;
    let pairs = distinct_valid_pairs(33); // helper: 33 unique tiny valid manifest+bundle pairs
    for pair in pairs.iter().take(32) {
        install_app(&profile, pair.manifest.clone(), pair.bundle.clone())
            .expect("first 32 install");
    }
    let err = install_app(&profile, pairs[32].manifest.clone(), pairs[32].bundle.clone())
        .expect_err("33rd refused");
    assert!(matches!(err, MobileError::SessionLimit));
}

#[test]
fn install_refuses_when_aggregate_pair_bytes_exceed_three_mib() {
    // Install large-but-valid pairs whose running total crosses 3 MiB before the
    // count cap; assert StoreFull (byte-specific), distinct from SessionLimit.
    let profile = /* open in-memory profile */;
    let big = valid_pair_near_one_mib(); // ~1 MiB bundle, count stays < 32
    install_app(&profile, big.manifest.clone(), big.bundle.clone()).unwrap();
    install_app(&profile, big2.manifest.clone(), big2.bundle.clone()).unwrap();
    let err = install_app(&profile, big3.manifest.clone(), big3.bundle.clone())
        .expect_err("aggregate over 3 MiB refused");
    assert!(matches!(err, MobileError::StoreFull));
}

#[test]
fn reinstalling_a_held_pair_is_idempotent_at_the_cap() {
    // Fill to 32, then reinstall a held ID: succeeds, count unchanged.
    // (Asserted via a follow-up install of a new ID still refusing on count.)
}
```

Note for implementer: `distinct_valid_pairs`/`valid_pair_near_one_mib` are test helpers — build them from the existing bundle/manifest encoders used elsewhere in this test file (search `verify_app_pair` / bundle construction helpers already present). Do not add production helpers.

- [ ] **Step 2: Run to verify fail** — `cargo test -p riot-ffi --test apps_contract install_count_cap_is_thirty_two` → FAIL (still capped at 16 / no byte check).

- [ ] **Step 3: Implement** — in `mobile_state.rs`:

1. Delete the local `const MAX_INSTALLED_APPS: usize = 16;` at line 46.
2. Rewrite the cap block in `install_pair` (`:2421-2434`):

```rust
    use riot_core::apps::admission::{preflight, AdmissionOutcome};

    let already_held = profile.installed_apps.iter().any(|app| app.app_id == app_id);
    let held_aggregate_bytes: usize = profile
        .installed_apps
        .iter()
        .map(|app| app.manifest_bytes.len() + app.bundle_bytes.len())
        .sum();
    let pair_bytes = manifest_bytes.len() + bundle_bytes.len();

    match preflight(
        profile.installed_apps.len(),
        held_aggregate_bytes,
        already_held,
        pair_bytes,
    ) {
        AdmissionOutcome::RefuseCount => return Err(MobileError::SessionLimit),
        AdmissionOutcome::RefuseBytes => return Err(MobileError::StoreFull),
        AdmissionOutcome::Admit => {}
    }

    if !already_held {
        profile.installed_apps.push(StoredInstalledApp {
            app_id,
            manifest_bytes,
            bundle_bytes,
        });
    }
```

Keep the trailing `Ok(InstalledAppRecord { .. })` block unchanged (note: it currently consumes `manifest.name` etc. by value; the `decode_manifest` at `:2419` runs before the push and is unaffected).

- [ ] **Step 4: Run to verify pass** — `cargo test -p riot-ffi --test apps_contract install_count_cap install_refuses_when_aggregate reinstalling_a_held` → PASS.

- [ ] **Step 4b: Update the two existing 16-cap tests to 32 (they now FAIL because the 17th install succeeds).** They assert the OLD cap and are in scope:

  1. `crates/riot-ffi/tests/mobile_fail_closed.rs:430-440` — change `for _ in 0..16` to `0..32`; keep the trailing assertion that the **33rd** install → `Err(MobileError::SessionLimit)`. Update the `// MAX_INSTALLED_APPS == 16` comment to `== 32`.
  2. `crates/riot-ffi/tests/mobile_refusal_surface.rs:577-598` — change `for index in 0..16` to `0..32`; the installed-count assertion `16` to `32`; `let (manifest, bundle) = pair(16)` to `pair(32)`; and the doc comment at `:541` "the seventeenth distinct app is refused" to "the thirty-third". Keep the `Err(MobileError::SessionLimit)` expectation.

  **Byte-quota caution:** these tests install 32 *small* synthetic pairs; confirm each `pair(index)` is well under `3 MiB / 32 ≈ 96 KiB` so the aggregate quota does not reject one mid-loop before the count boundary (existing helpers build tiny pairs — verify, do not assume). `preflight` checks count before bytes, so the 33rd still returns `SessionLimit`.

- [ ] **Step 4c: Run to verify pass** — `cargo test -p riot-ffi --test mobile_fail_closed --test mobile_refusal_surface` → PASS. Then the full `apps_contract` file + `cargo test -p riot-ffi` for no other regression (the `SessionLimit` assertions at `apps_contract.rs:437/509/548/584` are `MAX_SYNC_IDS`/inventory-byte tests, unaffected by the install cap).

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/apps_contract.rs \
        crates/riot-ffi/tests/mobile_fail_closed.rs crates/riot-ffi/tests/mobile_refusal_surface.rs
git commit -m "feat(ffi): enforce 32-app count cap and 3 MiB aggregate quota via core preflight"
```

---

## Task 5: Generation field on LocalProfile + generation-aware bootstrap resolver

**Files:**
- Modify: `crates/riot-ffi/src/mobile_state.rs` — `LocalProfile` (`:68-178`), the shared constructor helpers `profile_with_author` (`:395`) + `profile_with_author_and_db` (`:399`, the single `LocalProfile{}` literal at `:406`), the four public constructor call sites, the `starter_pair_bytes` resolver call site (`:3949`), and the crate's **inline** `#[cfg(test)] mod tests`
- Test: inline `#[cfg(test)]` test in `crates/riot-ffi/src/mobile_state.rs` (fresh-profile generation) + `crates/riot-ffi/tests/apps_contract.rs` (legacy-resolve via pub FFI)

**Constructor reality (verified):** exactly ONE `LocalProfile{}` literal, at `mobile_state.rs:406`, reached by every constructor through `profile_with_author_and_db`. There is NO `restore_local_profile_with_database`. Thread the generation as a parameter through both helpers and set it at each real call site.

**Test-visibility constraint (verified, load-bearing):** `ProfileState`/`MobileProfile.inner` are `pub(crate)` and `#[cfg(test)]` lib items are **invisible to integration tests** (documented precedent: the `follow_site_for_test` seam note at `mobile_state.rs:4710-4714`). So the assertion that reads the private `starter_catalog_generation` field MUST live in the crate's inline `#[cfg(test)] mod tests`, NOT in `tests/apps_contract.rs`. Do NOT add a `pub(crate)` accessor and call it from an integration test — it won't compile.

- [ ] **Step 1: Write the failing tests**

Inline test, added to `mobile_state.rs`'s existing `#[cfg(test)] mod tests` (same module → the private field is reachable directly; mirror how nearby inline tests lock `inner` / match `ProfileState::Active`):

```rust
#[test]
fn fresh_profile_is_generation_two() {
    let profile = open_local_profile().unwrap();
    let guard = profile.inner.lock().unwrap();
    let ProfileState::Active(p) = &*guard else { panic!("profile should be active") };
    assert_eq!(p.starter_catalog_generation, Some(2));
}
```

Integration test in `apps_contract.rs`, using ONLY pub FFI (no private access) — a restored profile is generation-1 (`None`) yet still resolves a held built-in ID by exact bytes:

```rust
#[test]
fn a_restored_generation_one_profile_still_serves_a_held_built_in() {
    // open_profile_from_sealed_identity takes the restore path, which sets
    // generation = None (gen-1). It must still resolve/serve a held built-in
    // pair by exact ID via the dual-catalog resolver.
    // Build a sealed identity + install a starter, restart via the sealed path,
    // then assert app_pair_bytes(starter_id) is Ok. Mirror the existing
    // sealed-identity round-trip helper already used in this test file.
}
```

- [ ] **Step 2: Run to verify fail** — inline test FAILS (no `starter_catalog_generation` field); integration test FAILS/does-not-compile until the field + resolver land. Run `cargo test -p riot-ffi --lib fresh_profile_is_generation_two`.

- [ ] **Step 3: Implement**

1. Add the field to `LocalProfile` (after `app_execution_generation` at `:130`), documented to disambiguate from the unrelated `app_execution_generation`:

```rust
    /// The starter-catalog generation this profile was created under. `None` is
    /// the durable encoding of generation 1 (a profile predating this marker);
    /// fresh profiles record `Some(2)`. It selects which built-in catalog
    /// bootstrap resolves and is NEVER derived from a community, author, or
    /// device identifier. Distinct from `app_execution_generation` above, which
    /// is a per-session sandbox-invalidation counter.
    starter_catalog_generation: Option<u8>,
```

2. Thread a `generation: Option<u8>` parameter through both shared helpers and set it in the single literal. Change the signatures:

```rust
fn profile_with_author(
    store: EvidenceStore,
    author: EvidenceAuthor,
    starter_catalog_generation: Option<u8>,
) -> Arc<MobileProfile> {
    profile_with_author_and_db(store, author, None, starter_catalog_generation)
}

fn profile_with_author_and_db(
    store: EvidenceStore,
    author: EvidenceAuthor,
    db: Option<riot_core::store::RiotDatabase>,
    starter_catalog_generation: Option<u8>,
) -> Arc<MobileProfile> {
    // ... existing body; in the LocalProfile { .. } literal at :406 add:
    //     starter_catalog_generation,
}
```

Then set the value at each of the FOUR real call sites (grep `profile_with_author` first to confirm the set — verified 4 callers, all in `mobile_state.rs`, no test-only callers):

| Call site | Kind | Value |
| --- | --- | --- |
| `open_local_profile` body (`:298`) → `profile_with_author(store, author, Some(2))` | fresh | `Some(2)` |
| `open_local_profile_with_database` (`:346`) → `profile_with_author_and_db(store, author, Some(db_handle), Some(2))` | fresh | `Some(2)` |
| `open_profile_from_sealed_identity` (`:315`) → `profile_with_author(store, author, None)` | restore | `None` |
| `open_profile_from_sealed_identity_with_database` (`:373`) → `profile_with_author_and_db(store, author, Some(db_handle), None)` | restore | `None` |

`None` on the two restore paths is correct **for this WU**: WU-001N threads the persisted value through the restore FFI signatures; until then a restored (pre-upgrade) profile is generation 1, the intended default. If the grep surfaces any additional `profile_with_author*` caller, pass `Some(2)` unless it explicitly exercises restore.

3. No production accessor is needed. The fresh-profile assertion (Step 1) reads the private field directly from the inline `#[cfg(test)] mod tests`. The production reads of `starter_catalog_generation` are: the resolver (below) and, later, WU-001N persistence.

4. **Leave the directory-merge call site (`:3770`) unchanged.** The spec advertises the current catalog to *everyone* ("advertised in the directory" — spec L641; only *auto-install* is generation-gated, and auto-install is not implemented in this in-memory path). `STARTER_CATALOG` there already aliases `CURRENT_STARTER_CATALOG`, which is the correct advertised set. Do NOT gate advertisement by generation — that would hide the redesign from existing users, contradicting the "Install redesigned version" flow.

5. At the resolver call site (`:3949`, inside `resolve_app_payload_bytes`, currently `starter_pair_bytes(STARTER_CATALOG, ..)`) resolve a held ID against **both** catalogs — a generation-2 profile can still hold a carried legacy ID, and a generation-1 profile must resolve legacy — so replace the single lookup with current-then-legacy:

```rust
    use riot_core::apps::starter::{CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG};
    // after the installed-apps lookup, before the carried-store lookup:
    if let Some(pair) = riot_core::apps::index::starter_pair_bytes(CURRENT_STARTER_CATALOG, app_id)
        .or_else(|| riot_core::apps::index::starter_pair_bytes(LEGACY_BUILTIN_CATALOG, app_id))
    {
        return Ok(pair);
    }
```

(Resolution is by exact ID, so trying both catalogs never returns the wrong bytes — `starter_pair_bytes` only matches a pair whose bytes re-derive the requested ID.)

- [ ] **Step 4: Run to verify pass** — `cargo test -p riot-ffi --lib fresh_profile_is_generation_two` PASS and `cargo test -p riot-ffi --test apps_contract a_restored_generation_one` PASS; then the whole `apps_contract` file + `cargo test -p riot-core` to confirm directory-listing and starter tests still pass. Because CURRENT==LEGACY bytes this WU, listings are unchanged for existing tests. (Note: the `.or_else(LEGACY_BUILTIN_CATALOG)` branch is inert while CURRENT==LEGACY — it becomes live/coverable only once a Slice-4 WU diverges a v2 entry; do not chase its line coverage this WU.)

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/apps_contract.rs
git commit -m "feat(ffi): generation-aware starter bootstrap + dual-catalog held-app resolver"
```

---

## Task 6: Generated inventory report (current/legacy descriptors)

**Files:**
- Create: `crates/riot-core/src/apps/inventory.rs`
- Modify: `crates/riot-core/src/apps/mod.rs`
- Test: `crates/riot-core/tests/apps_inventory.rs` (new)

- [ ] **Step 1: Write the failing test** — the spec requires a generated catalog report recording, per app: current/legacy membership, app ID, manifest+bundle SHA-256, encoded size, resource count, data-namespace prefix. This WU emits the machine-checkable core of it.

```rust
use riot_core::apps::inventory::{catalog_inventory, CatalogMembership};
use riot_core::apps::starter::{CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG};

#[test]
fn inventory_reports_eight_current_and_eight_legacy_entries() {
    let inv = catalog_inventory(CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG);
    assert_eq!(inv.iter().filter(|e| e.membership.current).count(), 8);
    assert_eq!(inv.iter().filter(|e| e.membership.legacy).count(), 8);
}

#[test]
fn inventory_entries_carry_app_id_and_pair_sha256_and_byte_sizes() {
    let inv = catalog_inventory(CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG);
    let e = &inv[0];
    assert_eq!(e.app_id.len(), 32);
    assert_eq!(e.manifest_sha256.len(), 32);
    assert_eq!(e.bundle_sha256.len(), 32);
    assert!(e.manifest_bytes_len > 0 && e.bundle_bytes_len > 0);
}

#[test]
fn until_v2_lands_current_and_legacy_share_membership_per_id() {
    // Same bytes today => each app ID is a member of BOTH catalogs.
    let inv = catalog_inventory(CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG);
    assert!(inv.iter().all(|e| e.membership.current && e.membership.legacy));
}
```

- [ ] **Step 2: Run to verify fail** — module missing.

- [ ] **Step 3: Implement** — create `crates/riot-core/src/apps/inventory.rs`:

```rust
//! Generated current/legacy catalog inventory. Authoritative machine-checkable
//! record used by the repack `--check` audit and the existing-user presentation
//! descriptors. Runtime selection/authorization always uses the full app ID,
//! never name or semantic version — this report is documentation + audit only.

use sha2::{Digest, Sha256};

use super::bundle::decode_app_bundle;
use super::index::app_bundle_digest;
use super::manifest::{app_id_for, decode_manifest, AppId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CatalogMembership {
    pub current: bool,
    pub legacy: bool,
}

#[derive(Debug, Clone)]
pub struct InventoryEntry {
    pub app_id: AppId,
    pub name: String,
    pub version: String,
    pub manifest_sha256: [u8; 32],
    pub bundle_sha256: [u8; 32],
    pub manifest_bytes_len: usize,
    pub bundle_bytes_len: usize,
    pub resource_count: usize,
    pub membership: CatalogMembership,
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

/// Build the inventory for the two catalogs. Invalid pairs are skipped, mirroring
/// `verify_starter_catalog`. Membership is merged by app ID so a pair present in
/// both catalogs (the pre-v2 state) reports `current && legacy`.
pub fn catalog_inventory(
    current: &[(&[u8], &[u8])],
    legacy: &[(&[u8], &[u8])],
) -> Vec<InventoryEntry> {
    let mut out: Vec<InventoryEntry> = Vec::new();
    for (in_current, catalog) in [(true, current), (false, legacy)] {
        for (manifest_bytes, bundle_bytes) in catalog {
            let Ok(manifest) = decode_manifest(manifest_bytes) else { continue };
            let Ok(bundle) = decode_app_bundle(bundle_bytes) else { continue };
            if manifest.entry_point != bundle.entry_point {
                continue;
            }
            let Ok(app_id) = app_id_for(&manifest, &app_bundle_digest(bundle_bytes)) else { continue };
            if let Some(existing) = out.iter_mut().find(|e| e.app_id == app_id) {
                existing.membership.current |= in_current;
                existing.membership.legacy |= !in_current;
                continue;
            }
            out.push(InventoryEntry {
                app_id,
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                manifest_sha256: sha256(manifest_bytes),
                bundle_sha256: sha256(bundle_bytes),
                manifest_bytes_len: manifest_bytes.len(),
                bundle_bytes_len: bundle_bytes.len(),
                resource_count: bundle.resources.len(),
                membership: CatalogMembership { current: in_current, legacy: !in_current },
            });
        }
    }
    out
}
```

Add `pub mod inventory;` to `crates/riot-core/src/apps/mod.rs`. (Verify `bundle.resources` is the field name / `sha2` is already a dep — it is used by `app_id_for`; if the bundle field differs, match the real `decode_app_bundle` return shape at `bundle.rs`.)

- [ ] **Step 4: Run to verify pass** — `cargo test -p riot-core --test apps_inventory` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/apps/inventory.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_inventory.rs
git commit -m "feat(core): generated current/legacy catalog inventory report"
```

---

## Task 7: Full-suite green + clippy/fmt gate

- [ ] **Step 1:** `cargo fmt --all -- --check` → PASS (fix with `cargo fmt --all` if needed, re-run).
- [ ] **Step 2:** `cargo clippy --workspace --all-features -- -D warnings` → PASS. There should be NO `deprecated` warnings — `STARTER_CATALOG` is a plain alias, not `#[deprecated]`. If any appear, you added the attribute by mistake; remove it (do not add `#[allow(deprecated)]`, do not touch `demo_fixture.rs`).
- [ ] **Step 3:** `cargo test --workspace --all-features` → PASS. (Build `--workspace`: Task 1 touches a widely-used const; a scoped `-p` run can hide a downstream break — see the shared-checkout scoped-test hazard.)
- [ ] **Step 4: Coverage — use the CI-ENFORCED gate, NOT tarpaulin.** CI (`.github/workflows/ci.yml` "Rust coverage (llvm-cov line floor)") runs `cargo llvm-cov --workspace --all-features --fail-under-lines <thresholds.llvm.lines>` (floor 95). tarpaulin is FICTION here: `thresholds.tarpaulin.lines` is 97 but tarpaulin only measures ~94.5% (its ptrace engine also hangs/undercounts on this workspace — see the CI comment), so main itself fails tarpaulin-97. Run:
  ```bash
  floor=$(jq -r '.thresholds.llvm.lines' .coverage-thresholds.json)   # 95
  cargo llvm-cov --workspace --all-features --fail-under-lines "$floor"
  ```
  → PASS (main measures ~97.75% llvm lines). Do not lower the floor. Do NOT gate on `cargo tarpaulin --fail-under 97`.
- [ ] **Step 5: Commit** any fmt-only changes:

```bash
git add -u crates/
git commit -m "chore: fmt + clippy clean for catalog-admission WU"
```

---

## Definition of Done

- `CURRENT_STARTER_CATALOG` + `LEGACY_BUILTIN_CATALOG` both exactly 8 valid pairs; `bootstrap_catalog(None/Some(1))==legacy`, `Some(2)==current`.
- Pure `admission::preflight` enforces 32 count + 3 MiB aggregate; count checked before bytes; held-ID reinstall idempotent. Boundary tests: 31→32 admit, 32→33 refuse-count, exact 3 MiB admit, +1 refuse-bytes.
- FFI `install_pair` uses the pure preflight; `SessionLimit`≠`StoreFull` kept distinct. The two existing 16-cap tests (`mobile_fail_closed.rs`, `mobile_refusal_surface.rs`) are updated to the 32 boundary and green.
- `STARTER_CATALOG` is a plain (non-deprecated) alias of `CURRENT_STARTER_CATALOG`; `clippy -D warnings` stays green with zero migration of existing uses.
- `LocalProfile.starter_catalog_generation` present, threaded through `profile_with_author`/`profile_with_author_and_db`; fresh==`Some(2)`, restore==`None` (WU-001N threads the persisted value). Directory advertises the current catalog to everyone (unchanged); held IDs resolve against both catalogs by exact ID.
- Inventory report emits per-app id/sha256/sizes/resource-count/membership.
- `cargo fmt/clippy/test --workspace` + tarpaulin floor all green. No native/disk/fixture-byte changes.

## Explicitly deferred (NOT this WU)

- **WU-001N:** persist `starter_catalog_generation` in Android `PersistedProfile.kt` + iOS/macOS `ProfileRepository.swift`; thread it through FFI restore signatures; Android full-profile 4,194,240-byte codec-ceiling preflight (`encodedSize`) + conformance that prospective size equals actual encoding.
- **WU-002:** prepare/persist/finalize trust grant/revoke + app-data transactions.
- Current/legacy **presentation UI** (Legacy 1 vs Version 2 cards), install-warning copy — native, later WU.
- Populating real v2 catalog entries — per Slice-4 WU.
