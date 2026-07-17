# iOS Surface — Unit 4a: `newswire_is_editor` FFI predicate — Implementation Plan


**Plan-review gate: PASSED** (Feasibility + Scope + Completeness, 2026-07-18).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Expose a store-backed FFI predicate `MobileProfile.newswire_is_editor(descriptor_entry_id, subject_id) -> bool` so the iOS UI (Unit 4b) can show editorial controls for a real editor of any active community — *joined or created* — with a display gate that is provably identical to the core authority gate.

**Architecture:** Extract the roster-authority decision already inside `require_action_authority` into a shared `pub fn is_editorial_authority(descriptor, subject_id) -> bool` (`pub` — riot-ffi calls it cross-crate); have both `require_action_authority` (the admission gate) and the new FFI predicate call it. The FFI method loads the descriptor from the store (`load_space_descriptor`) and returns the shared predicate's answer; an unknown/not-yet-synced descriptor returns `Ok(false)`, never an error. No new `uniffi::Record`.

**Tech stack:** Rust 2021, `riot-core` newswire, proc-macro `uniffi`.

**Why this unit is not pure-Swift (gate r1):** a joined community's `editorial_roster` is not readable across FFI today — it lives only in the in-store `SpaceDescriptorV1` and as CREATE input. This unit adds the one read predicate. It requires **one coordinated binding regen + native staticlib rebuild** (checksum-coupling discipline; coordinator-centralized). It must land before Unit 4b (the Swift consumer). Design: `docs/superpowers/specs/2026-07-18-ios-surface-built-capabilities-design.md` §4.

**Shared-checkout:** claim `crates/riot-core/src/newswire/entry.rs`, `crates/riot-ffi/src/newswire_ffi.rs` in COLLABORATION.md; pathspec commits only; absolute `git`/`grep`.

---

## Ground truth (verified)

- `require_action_authority` (`crates/riot-core/src/newswire/entry.rs:212-228`) currently inlines: `author.namespace_id().as_bytes() != &descriptor.namespace_id || !descriptor.editorial_roster.contains(&signer_id)` → `Err(AuthorityInvalid)`. `signer_id: [u8;32]`.
- `SpaceDescriptorV1` (`model.rs:32-43`): `namespace_id: [u8;32]` (== founder id, by construction — `require_founding_organizer` enforces `descriptor.namespace_id == organizer subspace`), `editorial_roster: Vec<[u8;32]>`.
- `load_space_descriptor(&EvidenceStore, EntryId) -> Result<VerifiedNewswireRecord, NewswireStoreError>` (`store.rs:65`). Reach the descriptor via `match record.payload() { NewswirePayload::SpaceDescriptor(p) => p, _ => ... }`. `NewswireStoreError` has a not-found variant.
- FFI: store-backed calls are `MobileProfile` methods using `with_active(&self.inner, |profile| ...)` (e.g. `create_newswire_editorial_action`, `newswire_ffi.rs:330`). `parse_entry_id(&str) -> Result<[u8;32], MobileError>` (`newswire_ffi.rs:720`, wrong len/hex → `InvalidInput`). Errors are `Result<_, MobileError>`.
- Test helpers (`entry.rs:424`): `fn descriptor(namespace_id, roster) -> SpaceDescriptorV1`; authors via `generate_space_organizer_author()` (founder: subspace==namespace) + `generate_communal_author_for_namespace(namespace_id)` (roster candidate). Build a verified record: `build_signed(&author, snapshot(t), NewswirePayload::SpaceDescriptor(descriptor(...)))` → `inspect_news_record(&signed.signed)` → `.entry_id()`.

---

## Task 1: Extract the shared authority predicate (core, no behavior change)

**Files:** Modify `crates/riot-core/src/newswire/entry.rs`

- [ ] **Step 1: Write the failing test.** Add to entry.rs `#[cfg(test)] mod tests`:
```rust
#[test]
fn is_editorial_authority_matches_admission_for_member_and_nonmember() {
    let organizer = generate_space_organizer_author().unwrap();
    let ns = *organizer.namespace_id().as_bytes();
    let editor = generate_communal_author_for_namespace(ns).unwrap();
    let editor_id = *editor.subspace_id().as_bytes();
    let outsider = generate_communal_author_for_namespace(ns).unwrap();
    let outsider_id = *outsider.subspace_id().as_bytes();

    let d = descriptor(ns, vec![editor_id]);
    assert!(is_editorial_authority(&d, &editor_id), "roster member is an editor");
    assert!(!is_editorial_authority(&d, &outsider_id), "non-member is not an editor");

    // Founder + empty roster: locks whatever admission actually does (see Task note).
    let empty = descriptor(ns, vec![]);
    let founder_id = ns; // founder subspace == namespace id
    assert_eq!(
        is_editorial_authority(&empty, &founder_id),
        FOUNDER_EMPTY_ROSTER_IS_EDITOR, // const defined in Step 3 to match require_action_authority
    );
    assert!(!is_editorial_authority(&empty, &outsider_id), "empty roster: outsider not an editor");
}
```

- [ ] **Step 2: Run — expect FAIL** (`is_editorial_authority` not defined).
Run: `cargo test -p riot-core --features conformance newswire::entry::tests::is_editorial_authority -- --nocapture`
Expected: FAIL, "cannot find function `is_editorial_authority`".

- [ ] **Step 3: Extract the predicate + refactor `require_action_authority` to call it.**
First **read `require_action_authority` (entry.rs:212-228) exactly** to determine the founder-empty-roster truth: it currently requires `roster.contains(signer_id)` with no founder special-case, so a founder NOT in the roster is REJECTED → set `const FOUNDER_EMPTY_ROSTER_IS_EDITOR: bool = false;` (in the test module). If reading the code shows a founder special-case, set it `true` and encode that in the predicate. Then:
```rust
/// The single source of truth for "may this subject take editorial actions in this space".
/// Reused by both the admission gate (`require_action_authority`) and the FFI display
/// predicate (`newswire_is_editor`) so the two can never diverge.
/// `pub` (not `pub(crate)`) — the riot-ffi crate calls it via a `crate::newswire` re-export.
pub fn is_editorial_authority(descriptor: &SpaceDescriptorV1, subject_id: &[u8; 32]) -> bool {
    descriptor.editorial_roster.contains(subject_id)
    // Matches require_action_authority EXACTLY (verified: no founder special-case there —
    // a founder with an empty roster is rejected at admission, so display==authority==false).
    // If a future product decision makes the founder always an editor, add
    // `|| *subject_id == descriptor.namespace_id` HERE and in require_action_authority in the
    // SAME edit so admission and display stay identical (design §4, corrected r1).
}
```
Refactor `require_action_authority` to compute `signer_id` as today, keep the namespace-match check, and replace the inline `!descriptor.editorial_roster.contains(&signer_id)` with `!is_editorial_authority(descriptor, &signer_id)`. No behavior change.

- [ ] **Step 4: Run — expect PASS** + no regression.
Run: `cargo test -p riot-core --features conformance newswire:: 2>&1 | grep "test result"`
Expected: all ok, 0 failed (existing `authority_checks_reject_*` tests still green — behavior unchanged).

- [ ] **Step 5: Commit.**
```bash
git add crates/riot-core/src/newswire/entry.rs
git commit -m "refactor(newswire): extract is_editorial_authority shared by admission + (coming) FFI predicate"
```

---

## Task 2: The FFI predicate `MobileProfile.newswire_is_editor`

**Files:** Modify `crates/riot-ffi/src/newswire_ffi.rs`

- [ ] **Step 1: Write the failing test.** In the newswire_ffi tests (or `crates/riot-ffi/tests/newswire_is_editor_contract.rs` — mirror an existing contract test's harness that opens a profile + creates a newswire space):
```rust
#[test]
fn newswire_is_editor_true_for_member_false_for_outsider_and_unknown() {
    let profile = open_local_profile();
    // create a space whose roster includes a chosen editor id (use the create path + a known roster)
    let space = profile.create_newswire_space(NewswireSpaceInput {
        name: "Riverside".into(), summary: "s".into(), languages: vec!["en".into()],
        geographic_tags: vec![], topic_tags: vec![], editorial_roster: vec![EDITOR_HEX.into()],
    }).unwrap();
    // member
    assert!(profile.newswire_is_editor(space.entry_id.clone(), EDITOR_HEX.into()).unwrap());
    // outsider
    assert!(!profile.newswire_is_editor(space.entry_id.clone(), OUTSIDER_HEX.into()).unwrap());
    // unknown descriptor id → Ok(false), NOT an error
    assert_eq!(profile.newswire_is_editor(UNKNOWN_ID_HEX.into(), EDITOR_HEX.into()).unwrap(), false);
}

#[test]
fn newswire_is_editor_founder_with_empty_roster_is_false_matching_admission() {
    // A space whose founder created it with an EMPTY roster. The founder's own id
    // (== the space namespace id) must be false — display==authority; admission rejects it.
    let profile = open_local_profile();
    let space = profile.create_newswire_space(NewswireSpaceInput {
        name: "Empty".into(), summary: "s".into(), languages: vec!["en".into()],
        geographic_tags: vec![], topic_tags: vec![], editorial_roster: vec![], // empty
    }).unwrap();
    let founder_hex = /* hex of this profile's author subspace id == the space namespace id */;
    assert_eq!(profile.newswire_is_editor(space.entry_id, founder_hex).unwrap(), false);
}

#[test]
fn newswire_is_editor_non_descriptor_entry_id_is_false() {
    // An entry id that resolves to a NON-descriptor record (e.g. a post or editorial action)
    // → Ok(false) via the let-else branch, not an error.
    let profile = open_local_profile();
    let space = profile.create_newswire_space(/* ...roster: [] */).unwrap();
    let post = profile.create_newswire_post(/* a post in `space` */).unwrap();
    assert_eq!(profile.newswire_is_editor(post.entry_id, EDITOR_HEX.into()).unwrap(), false);
}
```
> Build `EDITOR_HEX`/`OUTSIDER_HEX` from generated author subspace ids (hex of `[u8;32]`); `UNKNOWN_ID_HEX` = a syntactically-valid but not-in-store entry id. Match the harness of the nearest existing `crates/riot-ffi/tests/newswire_*_contract.rs`.

- [ ] **Step 2: Run — expect FAIL** (`no method newswire_is_editor`).
Run: `cargo test -p riot-ffi newswire_is_editor -- --nocapture`

- [ ] **Step 3: Implement** inside the `#[uniffi::export] impl MobileProfile` block in newswire_ffi.rs:
```rust
/// True iff `subject_id` may take editorial actions in the space identified by
/// `descriptor_entry_id`. Display gate for the UI; the SAME authority the core enforces
/// at admission (via the shared `is_editorial_authority`). An unknown / not-yet-synced
/// descriptor returns Ok(false) — never an error — so the UI can render a "not yet an
/// editor / appears after first sync" state off a defined false.
pub fn newswire_is_editor(
    &self,
    descriptor_entry_id: String,
    subject_id: String,
) -> Result<bool, MobileError> {
    with_active(&self.inner, |profile| {
        let descriptor_id = parse_entry_id(&descriptor_entry_id)?;
        let subject = parse_entry_id(&subject_id)?;
        let record = match riot_core::newswire::load_space_descriptor(&profile.store, descriptor_id) {
            Ok(r) => r,
            Err(e) if is_descriptor_not_found(&e) => return Ok(false), // unknown/unsynced → false
            Err(e) => return Err(map_newswire_store_error(e)),
        };
        let riot_core::newswire::NewswirePayload::SpaceDescriptor(descriptor) = record.payload() else {
            return Ok(false); // entry id resolves to a non-descriptor record → not an editor
        };
        Ok(riot_core::newswire::is_editorial_authority(descriptor, &subject))
    })
}
```
Re-export `is_editorial_authority` from `crate::newswire` (add to `newswire/mod.rs` `pub use`). Add/confirm an `is_descriptor_not_found(&NewswireStoreError) -> bool` (match the not-found variant) — read `NewswireStoreError` to get the exact variant name. `NewswirePayload` is already reachable (`newswire_ffi.rs` uses it); qualify as needed.

- [ ] **Step 4: Run — expect PASS.** `cargo test -p riot-ffi newswire_is_editor` → 3 assertions green. Full `cargo test -p riot-ffi` → 0 failed. `cargo build -p riot-ffi` clean.

- [ ] **Step 5: Commit.**
```bash
git add crates/riot-ffi/src/newswire_ffi.rs crates/riot-core/src/newswire/mod.rs
git commit -m "feat(ffi): newswire_is_editor predicate (display gate == admission authority) (Unit4a)"
```

---

## Task 3: Regenerate bindings + rebuild the native staticlib (COORDINATED)

New `#[uniffi::export]` method changes the FFI checksum → the generated binding + native staticlib must rebuild **together** or the app hits a runtime checksum abort (documented coupling hazard). This is coordinator-centralized.

- [ ] **Step 1** Run the repo's binding-generation + native build (`scripts/conference/build-native-core.sh` or the coordinator's `generate-bindings` task) so `newswireIsEditor` appears in the generated `riot_ffi.swift` and the staticlib carries it.
- [ ] **Step 2** Smoke: an iOS `RiotTests` call `try profile.newswireIsEditor(spaceDescriptorEntryId:..., subjectId:...)` compiles + loads (a full test is Unit 4b). Confirm no checksum abort on FFI load.
- [ ] **Step 3** Commit the regenerated binding artifacts per the coordinator's convention (do NOT hand-edit generated files).

---

## Task 4: Workspace gates
- [ ] `cargo fmt -p riot-core -p riot-ffi -- --check`; `cargo clippy -p riot-core -p riot-ffi --all-features -- -D warnings`; `cargo test -p riot-core --features conformance` + `cargo test -p riot-ffi` → 0 failed.
- [ ] Coverage floor honored (`.coverage-thresholds.json`, CI-enforced).

---

## Self-Review
- **Spec coverage (§4a):** new `newswire_is_editor` method ✅ Task 2; descriptor-authenticated roster read reusing the existing check ✅ Task 1 (shared `is_editorial_authority`, called by both admission and the predicate — display==authority by construction); unknown/not-yet-synced → false ✅ Task 2; empty-roster semantic locked to admission behavior ✅ Task 1 (const pinned by reading `require_action_authority`); one coordinated rebuild ✅ Task 3; no new `uniffi::Record` ✅ (string args, bool return).
- **Placeholder scan:** the `FOUNDER_EMPTY_ROSTER_IS_EDITOR` const + the predicate's founder-special-case comment are the ONE decision the implementer resolves by reading `require_action_authority` (Step 3) — flagged, not left vague; both branches are specified.
- **Type consistency:** ids are `[u8;32]` in core / hex `String` across FFI (`parse_entry_id`); `is_editorial_authority(&SpaceDescriptorV1, &[u8;32]) -> bool` used identically in entry.rs and newswire_ffi.rs.
- **Open point for the implementer:** confirm the exact `NewswireStoreError` not-found variant name for `is_descriptor_not_found`, and the `create_newswire_space` roster is stored verbatim so the Task 2 fixture's `EDITOR_HEX` lands in the descriptor roster.
