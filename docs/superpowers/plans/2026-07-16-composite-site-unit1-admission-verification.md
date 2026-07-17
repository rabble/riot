# Composite Site — Unit 1: Owned-Namespace Admission & Verification — Implementation Plan

**Date:** 2026-07-16
**Design:** `docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md` (gate PASSED 2026-07-15), §3.1 + §8 Unit 1.
**Depends on:** Unit 0 (`docs/superpowers/plans/2026-07-15-composite-site-unit0-owned-cap-plumbing.md`) — the `OwnedMasthead` cap-minting API. **Unit 1 does not start until Unit 0 has landed on `main`.**

---

## 1. Scope

Unit 1 is the **critical-path security unlock**: it changes Riot's admission gate so an owned-namespace editorial entry, authored under a cryptographically-verified capability chain rooted at the followed site, is **admitted** — while every forgery is **rejected identically at every gate**. Today admission unconditionally rejects owned caps and delegations (Riot only accepts self-minted communal write caps).

This is **one atomic change** across the entire admission/inspection surface. Getting it partially right is worse than not doing it: a gate that admits where another rejects, or an FFI classifier that drops an admitted record, bricks the board (prior art: newswire 0B added a record family to only one classifier and rejected bundles).

**In scope:** the policy edit + gate unification + FFI classification + the adversarial verification test suite + retiring the string-roster.
**Out of scope:** the manifest record (Unit 2), moderation/revoke/tombstone (Unit 3), render (Unit 4), transport/ticket (Unit 5), native UI (Unit 6). Unit 1 admits owned editorial entries; it does not yet *compose* them into a site view.

## 2. The load-bearing invariant (do not weaken)

willow25 already does the cryptography correctly — **do not hand-roll chain verification.** `WriteCapability::is_valid()` checks the owned genesis root signature, every delegation link's signature, and strict area nesting; `does_authorise` checks `includes(entry)` over namespace + subspace + path + **time_range** and the receiver signature. The Riot-side policy is minimal:

```
if namespace_id.is_owned():
    REQUIRE capability.is_owned()                              # INVARIANT (see below)
    REQUIRE genesis.namespace_key == namespace_id == followed_site_root   # design inv 3
    then let willow25 verify_entry / does_authorise run        # the chain check
else:  # communal
    existing communal rule, UNCHANGED
```

**`capability.is_owned()` is a stated INVARIANT, not merely a test.** `NamespaceId::is_owned()` is only the LSB marker bit and is **not** bound to the cap's genesis variant — a *communal* genesis cap is unconditionally `is_valid()` and can name an owned `namespace_id`. Without the explicit `capability.is_owned()` require, anyone forges masthead writes by pointing a communal cap at the owned namespace id. This is the single most important line in the unit; it has its own RED case (§5, "marker-bit forgery").

**Root binding (design inv 3).** The owner key in the cap's genesis must equal the followed site root. A *different* owned namespace is a *different site*, never silently accepted as this one. This requires the followed root to be *available at the admission gate* — see Task 2, the one genuine new plumbing question.

## 3. Enumerated gate surface (verified against HEAD 2026-07-16)

Grepped `is_owned()`/`is_communal()`; the design's four gates + FFI split all confirmed present:

| # | Location | Role | Edit |
|---|---|---|---|
| 1 | `crates/riot-core/src/import/bundle.rs:496` (`verify_frame`, fn at :445) | **THE policy chokepoint** | Edit the policy **here only**. |
| 2 | `crates/riot-core/src/session.rs:658` (`into_authorised_entry`) | routes through `decode_bundle`→`verify_frame` | **Test seam only** — do NOT duplicate policy. |
| 3 | `crates/riot-core/src/sync/state.rs:277` (namespace-id equality) | routes through the same | **Test seam only.** |
| 4a | `crates/riot-core/src/newswire/entry.rs:326` (`if capability.is_owned()`) | **separate 4th gate** — newswire inspection/projection. ~~editorial articles hit this~~ **CORRECTED (as implemented, #14): they do NOT.** Gate 4 is behind `is_newswire_prefix` (`["newswire","v1",…]`), disjoint from editorial `["articles",…]`. | **LEFT AS-IS (still refuses owned caps).** Relaxing it would admit owned *newswire* records, which design §2 forbids. #14 pins gate-1/gate-4 agreement with a test instead of unifying them. |
| 4b | `crates/riot-core/src/newswire/entry.rs:356` (`inspect_verified_components_bounded`, `NonCommunalNamespace`) | **reached AFTER :326 passes** (:339→:349→:352) — independently rejects non-communal namespaces | **Must move together with :326** — an owned entry admitted at :326 still dies here otherwise. (Feasibility-gate finding.) |
| 5 | `crates/riot-ffi/src/mobile_state.rs` — `inspectable_entries` (:1424) **and** `list_current_entries` (:827) | FFI alert/non-alert classification split | Add the **`/articles/` prefix ONLY** to **BOTH** or bundles reject / board bricks. Manifest/revoke/tombstone families are Units 2/3 — do NOT wire them here (they don't exist yet). |

**Extract a single policy predicate.** To keep gates 1 and 4 from drifting, factor the owned-vs-communal decision into one `pub(crate)` helper in `import/bundle.rs` (or `willow/`), called by both. The cross-gate test (Task 5) is what proves they never diverge.

## 4. Tasks (TDD — RED first, per CLAUDE.md)

Each task: write the RED test, watch it fail, implement, green. Coverage honors the `.coverage-thresholds.json` ratchet floor (CI-enforced via `cargo-llvm-cov --fail-under-lines`, NOT tarpaulin — see the coverage-gate findings).

- **Task 1 — `verify_frame` owned-namespace policy (the chokepoint).** Replace the blanket owned/delegation rejection at `bundle.rs:496` with the §2 policy. RED: an owned editorial entry authored under a valid `OwnedMasthead` `/articles/<section>` delegated cap is currently REJECTED; after the edit it is ADMITTED. Preserve the communal path byte-for-byte (regression: existing communal admission tests stay green).
- **Task 2 — thread `followed_site_root` to the gate (BOTH admission paths).** The root-binding require needs the expected root at `verify_frame`. The concept does not exist yet (grep for `followed_site`/`FollowedSite` → zero hits), so define the minimal carrier: a single followed-root field on the import/session context (do NOT invent a manifest — that's Unit 2). **CRITICAL (Feasibility finding): the followed root must reach BOTH admission paths — the session import context AND the sync admission path (`sync/state.rs:268`, gate 3).** A session-only root check lets a wrong-root owned entry arriving via sync bypass it, violating the Task-5 keystone and reproducing the brick scenario §1 warns about. `verify_frame`/`decode_bundle` are context-free free fns with ~25 call sites (7 prod, 18 test); use a root-aware decode variant on only the two admission paths, leaving rootless callers (CLI pack, FFI inspectors) on the existing `decode_bundle` — avoids a 25-site ripple.
  - RED (wrong root): an entry under a valid owned cap for a *different* owned root than followed → REJECTED, identically via session import AND via sync.
  - RED (**absent root — FAIL CLOSED**, Completeness finding): an owned entry reaches the gate with `followed_site_root == None` → **REJECTED**, never admitted. The `Option` must fail closed; an admit-on-`None` default is an open hole. De-risk this seam FIRST — it is the only genuine unknown.
- **Task 3 — unify the 4th gate (`newswire/entry.rs:326` AND `:356`).** Apply the same predicate (via the shared helper from §3) at :326, and update `inspect_verified_components_bounded` (:356, `NonCommunalNamespace`) in the SAME change — it is reached after :326 passes and would otherwise still reject the admitted owned entry. RED: an owned editorial entry accepted at `verify_frame` is also accepted/classified through BOTH :326 and :356; a forgery is rejected at all of them.
- **Task 4 — FFI classification (both sites).** Add the owned record-family path prefixes to `inspectable_entries` AND `list_current_entries` in `mobile_state.rs`. RED (contract test): a committed owned editorial entry appears in the inspectable/current listing; omission from either fn drops it. Mirror the newswire-0B pattern.
- **Task 5 — cross-gate consistency test (BOTH directions).** One test asserting a *valid* owned editorial entry is accepted/classified identically at gates 1–4 (+FFI), AND a *forgery* is rejected identically at all of them (no gate stricter or looser than another). This is the unit's keystone — it is what makes "one atomic change" verifiable.
- **Task 6 — ~~retire the string-roster~~ DROPPED (category error; not done in #14).** `editorial_roster` (`newswire_ffi.rs:43`) governs the *communal newswire* founding roster — a different namespace and write model than owned-namespace editorial delegation, with which it coexists (design §2). The owned cryptographic cap check governs owned articles, not communal newswire membership, so it does not replace this field. It is also a `uniffi::Record` field (native-rebuild trap). The landed Unit 1 (#14) correctly left it untouched.

## 5. Adversarial RED cases (the security core — §8.1 Unit 1)

Every one must be RED-then-green, driving the REAL willow25 verifier (forge raw caps/entries as a hostile peer would — not via the friendly minting API):

1. **Forged delegation chain** — a delegation link with a bad signature → rejected by `is_valid()`.
2. **Over-broad area** — a cap whose area escapes `/articles/<section>` → the entry's path not `includes`d → `does_authorise` false.
3. **Expired cap** — entry timestamp outside the cap `time_range` → rejected.
4. **Wrong root** — valid owned cap for a *different* owned root than followed → rejected (Task 2).
5. **Communal-cap-naming-an-owned-namespace (marker-bit forgery)** — a valid communal genesis cap pointed at the owned `namespace_id`; MUST be rejected by the explicit `capability.is_owned()` invariant (this is the load-bearing line).
6. **Cross-namespace cap reuse** — a cap valid for site A used to author into site B's owned namespace → rejected.
7. **Delegation loops** — a self/cyclic delegation → rejected by `is_valid()` (confirm willow25 handles it; if not, it's a finding).
8. **Receiver mismatch** — a fully valid cap chain, but the entry is signed by a key that is NOT the cap's terminal `receiver()` → rejected. Distinct from forged-chain (bad link sig) and cross-namespace reuse (right author/wrong namespace). Spec §8 lists "confirm `receiver()` == final delegatee" as an explicit obligation; do not rely on `does_authorise` implicitly — assert it with its own RED. (Completeness-gate finding.)
9. **Absent followed root — fail closed** — an owned entry reaching the gate with no known followed root → rejected, not admitted (Task 2). (Completeness-gate finding.)
10. **Cross-gate consistency, both directions** (Task 5) — the meta-assertion over all of the above.

## 6. File scope (claim in COLLABORATION.md before editing)

`crates/riot-core/src/import/bundle.rs`, `crates/riot-core/src/newswire/entry.rs`, `crates/riot-core/src/session.rs` (context/seam), `crates/riot-core/src/sync/state.rs` (seam), `crates/riot-ffi/src/mobile_state.rs` (×2 classifiers), `crates/riot-ffi/src/newswire_ffi.rs` (roster retirement), new adversarial test files under `crates/riot-core/tests/` and `crates/riot-ffi/tests/`. **No new `uniffi::Record` expected** (Unit 1 admits existing entry shapes under new auth) — if the FFI surface changes, the UniFFI regen + native staticlib rebuild must land in the SAME commit (checksum-abort trap). Coordinate: `mobile_state.rs` and `newswire/entry.rs` are high-traffic across sessions — claim explicitly, pathspec commits only, work in a worktree.

## 7. Verification gates

- `cargo test --workspace --all-features` green; `cargo clippy --workspace --all-features -- -D warnings`; `cargo fmt --all -- --check`.
- `cargo run -p xtask -- validate-contracts` (pins intact).
- Coverage at the `.coverage-thresholds.json` floor.
- Every adversarial RED case (§5) demonstrably RED before the fix, green after.
- The cross-gate consistency test (Task 5) is the merge gate: without it, "unified all gates" is unproven.

## 8. Sequencing & hazards

1. **Hard dependency on Unit 0** — needs `OwnedMasthead::{generate, owner_write_capability, delegate_section}`. Do not start until Unit 0 is on `main`.
2. **Unblocks Units 2/3** — the manifest (Unit 2) and moderation (Unit 3) both admit owned records through this gate; Unit 1 is their prerequisite (design §8: "Unit 1 is the critical-path security unlock").
3. **Atomicity** — gates 1–5 must land together. A partial commit that updates `verify_frame` but not the FFI classifiers bricks the board.
4. **Shared-checkout** — `mobile_state.rs`/`newswire/entry.rs` are touched by many sessions; other tracks (security residuals, join-descriptor) also edit core. Rebase on `main` before claiming; STOP on foreign edits in claimed files.
5. **Followed-root plumbing (Task 2) is the only genuine unknown** — everything else is a policy edit + tests. De-risk it first: confirm where the followed site root lives in the import/session context before writing Task-1 code, because the wrong-root RED case depends on it.
