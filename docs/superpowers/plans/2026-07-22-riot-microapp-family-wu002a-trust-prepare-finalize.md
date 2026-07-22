# WU-002a — Trust Grant/Revoke prepare/persist/finalize seam (Rust core+FFI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: metaswarm orchestrated-execution. Steps `- [ ]`. Parent: `2026-07-22-riot-microapp-family-master-plan.md`. Spec: `docs/superpowers/specs/2026-07-22-riot-microapp-family-design.md` §"Durable trust and app-data transactions". First of three WU-002 units (002a trust seam · 002b app-data seam · 002c native wiring + alert copy + fault injection).

**Goal:** Split FFI trust grant/revoke from a single commit-first mutation into a **prepare → (host persists) → finalize** protocol: `prepare_app_trust` validates authority + signs the marker + returns the app id + decision (which the host records in its durable trusted-ID set) while holding the signed marker under a generation-bound token **without mutating the live store**; `finalize_app_trust` idempotently commits it. The existing single-shot `set_app_trust` is re-expressed as prepare+finalize under one lock, so current behavior and native callers are unchanged this WU.

**Architecture:** All in `crates/riot-ffi/src/mobile_state.rs` (the trust commit is FFI-side; core `trust.rs`/`session.rs` untouched). A new `LocalProfile.prepared_trust: Option<PreparedTrust>` slot holds at most one prepared mutation across the prepare/finalize call pair. No store mutation happens in prepare; the durable write (host's job, WU-002c) becomes the linearization point; finalize commits the exact signed marker and is crash-safe idempotent (re-admitting the same signed entry is an LWW no-op). No native/disk changes here.

**Tech Stack:** Rust 2021, `riot-ffi` UniFFI state, existing `sign_local_app_entry` / `commit_local_app_entries` / `app_execution_generation`.

**Scope boundary (do NOT exceed):**
- Modify only: `crates/riot-ffi/src/mobile_state.rs`, and tests `crates/riot-ffi/tests/apps_contract.rs` (+ optionally new `crates/riot-ffi/tests/apps_trust_txn.rs`).
- Optionally expose the new FFI methods in `crates/riot-ffi/src/apps_ffi.rs` (thin wrappers) so WU-002c can call them — but do NOT wire any native shell here.
- **Never** touch: core `trust.rs`/`session.rs`/`bridge.rs`, app-data paths (that's WU-002b), any native shell, `fixtures/`.
- Keep `set_app_trust` behavior byte-identical (existing tests `trust_lifecycle_is_lww_per_app`, `trust_toggles_never_exhaust_the_marker_cap`, `trust_store_full_leaves_cache_...`, the execution-generation suite MUST stay green).

**Verified anchors (origin/main incl. #96):** `set_app_trust` `mobile_state.rs:3149`; `bump_app_execution_generation` `:3220`; `commit_local_app_entries` `:3762`; `app_data_put_with_receipt` `:3434`; `LocalProfile` struct with `app_execution_generation` and `#[allow(dead_code)] starter_catalog_generation` (WU-001); single lock via `with_active`. `set_app_trust` today: drop sync session → organizer gate → cap check → timestamp → build+encode marker → `sign_local_app_entry` → `commit_local_app_entries` → `bump_app_execution_generation`.

---

## Design (read before Task 1)

Current `set_app_trust` (one `with_active`): validate → sign → **commit (store mutates)** → bump generation. The split keeps every validate/sign step in **prepare** but stops before commit; **finalize** does commit + bump.

```
prepare_app_trust(app_id, trusted):
  with_active:
    - if prepared_trust already Some → discard it (last-prepare-wins; a new prepare
      supersedes an abandoned one; log nothing sensitive)
    - drop active sync session; organizer gate; marker-cap check   (unchanged validations)
    - timestamp = next_app_write_timestamp
    - build + encode TrustMarker; path = trust marker path
    - signed = sign_local_app_entry(...)
    - profile.prepared_trust = Some(PreparedTrust { generation: profile.app_execution_generation,
                                                    signed, app_id, trusted })
    - return PreparedTrustRecord { app_id, trusted }   (host records this in its trusted-ID set)
    // NO commit_local_app_entries — the live store is untouched.

finalize_app_trust():
  with_active:
    - prepared = profile.prepared_trust.take() OR return MobileError::AppRejected (nothing prepared)
    - if prepared.generation != profile.app_execution_generation → return MobileError::SessionLimit? 
      NO — use a stale/authority error; the slot is already taken (discarded), store unchanged.
    - commit_local_app_entries(profile, vec![prepared.signed])   (store mutates HERE, post-persist)
    - bump_app_execution_generation(profile)
    - Ok(())
  // Idempotent: committing the same signed marker again is an LWW no-op at its coordinate,
  // so a crash-retry that replays the persisted bytes cannot double-apply.

discard_prepared_trust():
  with_active: profile.prepared_trust = None; Ok(())

set_app_trust(app_id, trusted)  [unchanged public behavior]:
  with_active: prepare_trust_locked(p, app_id, trusted)?; finalize_trust_locked(p)
  // one lock, validate→sign→commit→bump exactly as today.
```

Factor the bodies into lock-free helpers `prepare_trust_locked(&mut LocalProfile, ...)` and `finalize_trust_locked(&mut LocalProfile)` so `set_app_trust` runs both under a single `with_active`, while the public `prepare_app_trust`/`finalize_app_trust` each take the lock once (the two-phase native path; the shared authority lock that spans the host persist is WU-002c's native lock).

Slot lifecycle: `prepared_trust` is cleared by `discard_prepared_trust`, by a superseding `prepare`, by `finalize` (via `take`), and on the `Active→Failed`/profile-close transition (memory hygiene). It is NOT eagerly cleared at generation-bump sites — instead `finalize_trust_locked`'s generation guard rejects a stale prepared mutation lazily. This keeps the guard a LIVE, coverable check rather than an unreachable phantom (see Task 3).

Signatures: keep `set_app_trust(inner, app_id: String, trusted: bool)` and the internal `parse_entry_id(&str) -> [u8;32]` exactly as today (behavior byte-identical). `prepare_trust_locked` therefore also takes `app_id: String` and derives `[u8;32]` via `parse_entry_id`; `PreparedTrust.app_id` stores that `[u8;32]`, and `PreparedTrustRecord.app_id` is `hex(&prepared.app_id)`.

Test harness: the two-phase entrypoints are exposed as `AppRuntimeSession` methods in `apps_ffi.rs` (thin wrappers over the `pub(crate)` fns, mirroring `trust_app`) so the `tests/apps_contract.rs` integration crate can reach them — `pub(crate)` free fns are invisible to integration tests. In the test snippets below, `prepare_app_trust(&profile, …)`/`finalize_app_trust(&profile)`/`discard_prepared_trust(&profile)` are shorthand for those `runtime.prepare_app_trust(…)` etc. methods on the runtime handle, and `is_app_trusted(&profile, &app_id)` / the `organizer_with_installed_untrusted_app` setup mirror the trust-observation and setup already in `trust_lifecycle_is_lww_per_app` (open → `install_app` → not-yet-trusted; trust visible via the directory listing's `trusted` flag). Adapt, don't paste.

---

## Task 1: PreparedTrust slot + lock-free prepare/finalize helpers

**Files:** Modify `crates/riot-ffi/src/mobile_state.rs`.

- [ ] **Step 1: Write the failing test** — add to `crates/riot-ffi/tests/apps_contract.rs` (inline crate tests can also live in `mobile_state.rs`'s `#[cfg(test)] mod tests`; use the integration test for pub-FFI-observable behavior). This first test asserts prepare does NOT change trust and finalize does:

```rust
#[test]
fn prepare_trust_does_not_mutate_and_finalize_commits() {
    // organizer profile that can grant trust to a held app (mirror the setup in
    // trust_lifecycle_is_lww_per_app in this file: open profile, install a starter,
    // become organizer of the listed space).
    let (profile, app_id) = organizer_with_installed_starter(); // existing-style helper
    // prepare: returns the {app_id, trusted} record for the host's trusted-ID set;
    // trust is still OFF (live store untouched).
    let prepared = prepare_app_trust(&profile, app_id.clone(), true).expect("prepare");
    assert!(prepared.trusted);
    assert!(!prepared.app_id.is_empty());
    assert!(!is_app_trusted(&profile, &app_id), "prepare must not grant trust");
    // finalize: trust flips ON.
    finalize_app_trust(&profile).expect("finalize");
    assert!(is_app_trusted(&profile, &app_id), "finalize must commit the grant");
}
```

`is_app_trusted` is an existing test-observable (mirror how `trust_lifecycle_is_lww_per_app` checks trust — via the directory/listing `trusted` flag or `resolve_is_trusted` surfaced in a listing). If no pub accessor exists, assert via the same listing field those tests already use.

- [ ] **Step 2: Run to verify fail** — `cargo test -p riot-ffi --test apps_contract prepare_trust_does_not_mutate` → FAIL (`prepare_app_trust`/`finalize_app_trust` unresolved).

- [ ] **Step 3: Implement** in `mobile_state.rs`:

1. Add the struct near `StoredInstalledApp`:

```rust
/// A trust mutation validated + signed but NOT yet committed to the live store.
/// Lives only between `prepare_app_trust` and `finalize_app_trust`/`discard`
/// (at most one per profile); the host persists `persistable` durably in between,
/// making that durable write the linearization point. Cleared on finalize,
/// discard, a superseding prepare, and any generation/namespace change.
struct PreparedTrust {
    /// `app_execution_generation` captured at prepare; finalize refuses if it moved.
    generation: u64,
    /// The signed trust marker, ready to commit unchanged in finalize.
    signed: SignedWillowEntry,
    /// The app id + decision the host records in its durable trusted-ID SET
    /// (spec: "persists the prospective trusted-ID set"). Trust restart re-issues
    /// trust per persisted id — it does NOT replay marker bytes (that receipt/replay
    /// model is app-data's, WU-002b). So no marker bytes are returned here.
    app_id: [u8; 32],
    trusted: bool,
}
```

2. Add the field to `LocalProfile` (near `app_execution_generation`):

```rust
    /// At-most-one prepared-but-uncommitted trust mutation (WU-002a). `None`
    /// except between prepare and finalize/discard. WU-002c's native lock spans
    /// the host persist that sits between those two calls.
    prepared_trust: Option<PreparedTrust>,
```

Initialize `prepared_trust: None` in the single `LocalProfile { .. }` literal (the `profile_with_author_and_db` constructor from WU-001).

3. Refactor: extract the current `set_app_trust` body (everything inside `with_active`) into two lock-free helpers, moving the `commit_local_app_entries` + `bump_app_execution_generation` into the finalize half:

```rust
fn prepare_trust_locked(
    profile: &mut LocalProfile,
    app_id_bytes: Vec<u8>,
    trusted: bool,
) -> Result<PreparedTrust, MobileError> {
    // (verbatim from set_app_trust up to and including sign_local_app_entry):
    // drop active sync session; organizer gate; parse app id; marker-cap check;
    // next_app_write_timestamp; build + encode marker; path; sign_local_app_entry.
    let signed = /* the existing signed entry */;
    let app_id = exact_app_id(&app_id_bytes)?;
    Ok(PreparedTrust { generation: profile.app_execution_generation, signed, app_id, trusted })
}

fn finalize_trust_locked(profile: &mut LocalProfile, prepared: PreparedTrust) -> Result<(), MobileError> {
    if prepared.generation != profile.app_execution_generation {
        return Err(MobileError::Internal); // authority moved under us; caller already dropped the slot
    }
    commit_local_app_entries(profile, vec![prepared.signed])?;
    bump_app_execution_generation(profile);
    Ok(())
}
```

4. Re-express `set_app_trust` (single lock, unchanged behavior):

```rust
pub(crate) fn set_app_trust(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: Vec<u8>,
    trusted: bool,
) -> Result<(), MobileError> {
    with_active(inner, |profile| {
        let prepared = prepare_trust_locked(profile, app_id, trusted)?;
        finalize_trust_locked(profile, prepared)
    })
}
```

5. Add the two-phase public entrypoints + discard:

```rust
pub(crate) fn prepare_app_trust(
    inner: &Arc<Mutex<ProfileState>>,
    app_id: Vec<u8>,
    trusted: bool,
) -> Result<crate::apps_ffi::PreparedTrustRecord, MobileError> {
    with_active(inner, |profile| {
        profile.prepared_trust = None; // supersede any abandoned prepare
        let prepared = prepare_trust_locked(profile, app_id.clone(), trusted)?;
        let record = crate::apps_ffi::PreparedTrustRecord {
            app_id: hex(&prepared.app_id),
            trusted: prepared.trusted,
        };
        profile.prepared_trust = Some(prepared);
        Ok(record)
    })
}

pub(crate) fn finalize_app_trust(inner: &Arc<Mutex<ProfileState>>) -> Result<(), MobileError> {
    with_active(inner, |profile| {
        let prepared = profile.prepared_trust.take().ok_or(MobileError::Internal)?;
        finalize_trust_locked(profile, prepared)
    })
}

pub(crate) fn discard_prepared_trust(inner: &Arc<Mutex<ProfileState>>) -> Result<(), MobileError> {
    with_active(inner, |profile| { profile.prepared_trust = None; Ok(()) })
}
```

Add `PreparedTrustRecord { app_id: String, trusted: bool }` to `crates/riot-ffi/src/apps_ffi.rs` next to `InstalledAppRecord`. (The host records `app_id`→`trusted` in its durable trusted-ID set between prepare and finalize; no marker bytes cross the FFI for trust.)

- [ ] **Step 4: Run to verify pass** — `cargo test -p riot-ffi --test apps_contract prepare_trust_does_not_mutate` → PASS. Then run the whole trust suite to confirm `set_app_trust` is unchanged: `cargo test -p riot-ffi --test apps_contract trust_ app_data_ replay_ revoke_ namespace_ stale_` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/src/apps_ffi.rs crates/riot-ffi/tests/apps_contract.rs
git commit -m "feat(ffi): trust prepare/finalize seam (set_app_trust re-expressed, store-mutation deferred to finalize)"
```

---

## Task 2: Crash-before-finalize safety + discard + idempotent finalize

**Files:** Modify `crates/riot-ffi/tests/apps_contract.rs`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn prepare_without_finalize_leaves_trust_untouched() {
    // Simulates a crash between the durable persist and finalize: the live store
    // was never mutated by prepare, so trust stays OFF and no marker exists.
    let (profile, app_id) = organizer_with_installed_starter();
    let _ = prepare_app_trust(&profile, app_id.clone(), true).unwrap();
    assert!(!is_app_trusted(&profile, &app_id));
    // A second, independent grant of a DIFFERENT decision still works (slot superseded).
    let _ = prepare_app_trust(&profile, app_id.clone(), true).unwrap();
    finalize_app_trust(&profile).unwrap();
    assert!(is_app_trusted(&profile, &app_id));
}

#[test]
fn discard_clears_a_prepared_grant() {
    let (profile, app_id) = organizer_with_installed_starter();
    prepare_app_trust(&profile, app_id.clone(), true).unwrap();
    discard_prepared_trust(&profile).unwrap();
    // finalize now has nothing to commit.
    assert!(finalize_app_trust(&profile).is_err());
    assert!(!is_app_trusted(&profile, &app_id));
}

#[test]
fn re_issuing_trust_for_an_already_trusted_app_is_idempotent() {
    // Trust restart = the host re-issues trust per persisted id (NOT a byte replay).
    // Re-issuing for an already-trusted app must stay trusted with no marker-cap
    // growth (LWW at the same coordinate), proving the crash-retry restore is safe.
    let (profile, app_id) = organizer_with_installed_starter();
    prepare_app_trust(&profile, app_id.clone(), true).unwrap();
    finalize_app_trust(&profile).unwrap();
    assert!(is_app_trusted(&profile, &app_id));
    // Restart replay: re-issue via the ordinary path.
    prepare_app_trust(&profile, app_id.clone(), true).unwrap();
    finalize_app_trust(&profile).unwrap();
    assert!(is_app_trusted(&profile, &app_id), "still trusted, exactly once");
    // (Mirror trust_toggles_never_exhaust_the_marker_cap: repeated re-issue must
    // not exhaust MAX_APP_TRUST_MARKERS.)
}
```

This matches how WU-002c restores trust on restart (re-issue per persisted id), consistent with the existing restore paths (`ProfileRepository.swift` re-`trustApp` loop; Android `restore` re-`trustApp`). No byte-replay path is added for trust; app-data's receipt/replay is WU-002b.

- [ ] **Step 2: Run to verify fail**, then **Step 3: Implement** whichever idempotent-replay path Step 1 settled on (only if not already covered by re-prepare+finalize), **Step 4: Run to verify pass**.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/tests/apps_contract.rs crates/riot-ffi/src/mobile_state.rs
git commit -m "test(ffi): crash-before-finalize, discard, and idempotent trust replay"
```

---

## Task 3: Generation/namespace change clears the prepared slot (fail-closed)

**Files:** Modify `crates/riot-ffi/src/mobile_state.rs`, tests in `apps_contract.rs`.

The generation guard in `finalize_trust_locked` is the LIVE fail-closed mechanism (a lazy, tested check), NOT eager slot-clearing at every bump site. Do NOT sprinkle `profile.prepared_trust = None;` across the bump sites (`:676/:799/:837/:3038`) — that would make the guard an unreachable phantom the ≥95 coverage gate flags, and it is the exact "named guard that bounds nothing" defect class this repo watches for. Instead: a real generation bump between prepare and finalize leaves the (now stale) slot in place, and `finalize_trust_locked` rejects it via the generation mismatch, uncommitted. Clear the slot only on the `Active→Failed`/profile-close transition (memory hygiene, not the security path).

- [ ] **Step 1: Write the failing test** — a real community switch (which bumps `app_execution_generation`) between prepare and finalize must make finalize reject via the generation guard, with trust NOT granted:

```rust
#[test]
fn a_generation_bump_between_prepare_and_finalize_fails_closed_via_the_guard() {
    // Two communities so a real switch bumps the generation (mirror an existing
    // switch_community test's setup in this file).
    let (profile, app_id) = organizer_with_installed_untrusted_app();
    prepare_app_trust(&profile, app_id.clone(), true).unwrap();
    switch_to_other_community(&profile);           // real bump path, does NOT clear the slot
    let err = finalize_app_trust(&profile).unwrap_err(); // guard fires on generation mismatch
    assert!(matches!(err, MobileError::Internal));  // or the chosen stale-authority variant
    assert!(!is_app_trusted(&profile, &app_id), "stale prepared trust must not commit");
}
```

- [ ] **Step 2: Run to verify fail** — before the guard exists / if the slot were eagerly cleared this asserts the wrong path. Confirm it fails for the right reason.

- [ ] **Step 3: Implement** — ensure `finalize_trust_locked` compares `prepared.generation` against the CURRENT `profile.app_execution_generation` and returns the stale-authority error (leaving trust uncommitted) when they differ (already in the Task 1 helper). Add slot-clear ONLY to the `ProfileState::Active → Failed` transition / profile-close for hygiene. Verify by grep that no bump site clears the slot, so the guard is genuinely reachable and covered by the test above.

- [ ] **Step 4: Run to verify pass** — the new test + the full execution-generation suite (`revoke_fails_the_next_app_execution_read_and_commit`, `namespace_replacement_fails_stale_app_execution_access`, `stale_approval_generation_...`) green. `llvm-cov` shows the guard branch covered (not dead).

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/apps_contract.rs
git commit -m "fix(ffi): clear prepared trust on generation/namespace change, fail closed"
```

---

## Task 4: Full-suite green + clippy/fmt/coverage gate

- [ ] **Step 1:** `cargo fmt --all -- --check` → PASS.
- [ ] **Step 2:** `cargo clippy --workspace --all-features -- -D warnings` → PASS (mark `prepared_trust`/new methods `#[allow(dead_code)]` ONLY if genuinely unreferenced until WU-002c — but the two-phase entrypoints ARE referenced by the new tests, so they should not be dead).
- [ ] **Step 3:** `cargo test --workspace --all-features` → PASS (build `--workspace`).
- [ ] **Step 4: Coverage — CI-enforced gate (NOT tarpaulin):** `cargo llvm-cov --workspace --all-features --fail-under-lines $(jq -r '.thresholds.llvm.lines' .coverage-thresholds.json)` (= 95) → PASS. Do NOT use `cargo tarpaulin --fail-under 97` (fiction floor, hangs on this workspace).

## Definition of Done

- `prepare_app_trust` validates + signs + stores a `PreparedTrust` and returns `{app_id, trusted}` (for the host's durable trusted-ID set) WITHOUT mutating the live store (trust unchanged after prepare). No marker bytes cross the FFI for trust.
- `finalize_app_trust` commits the exact signed marker + bumps generation; idempotent on crash-retry (re-issue is an LWW no-op).
- `finalize`'s generation guard is the LIVE fail-closed mechanism: a real generation bump (community switch) between prepare and finalize makes finalize reject with the marker uncommitted, and a dedicated test exercises that branch (not a phantom guard). `discard_prepared_trust` clears the slot; a superseding prepare replaces it; the `Active→Failed`/close transition clears it for hygiene.
- `set_app_trust` re-expressed as prepare+finalize under one lock, behavior byte-identical — all existing trust/app-data/execution-generation tests stay green.
- The two-phase entrypoints are exposed as `AppRuntimeSession` methods (`apps_ffi.rs`) so integration tests and WU-002c can call them; `PreparedTrustRecord` is `{app_id: String, trusted: bool}`.
- `fmt`/`clippy`/`test --workspace`/`llvm-cov ≥95` all green. No core/native/fixture changes.

## Explicitly deferred

- **WU-002b:** app-data `prepare_app_data_put` (receipt without live mutation) + finalize, same pattern on `app_data_put_with_receipt`. **NOTE (Scope reviewer N1):** WU-002b MUST NOT add a second independent prepared slot — the spec's "at most one prepared mutation per profile" requires trust and app-data to SHARE one `prepared_mutation` slot (or enforce mutual exclusion). Generalize `prepared_trust` into a single `prepared_mutation: Option<PreparedMutation>` enum in WU-002b rather than adding a parallel field.
- **WU-002c:** native iOS/macOS/Android wiring — the shared authority/persistence lock spanning host persist, persist-first ordering, the exact storage-full / save-failed alert copy, session invalidation before WebView destroy on revoke, and fault-injection tests at every boundary (prepare, persist, finalize, session-invalidation, teardown, process-termination, rebuild).
