# WU-002b — App-Data prepare/persist/finalize seam + shared prepared slot (Rust core+FFI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: metaswarm orchestrated-execution. Steps `- [ ]`. Parent: `2026-07-22-riot-microapp-family-master-plan.md`. Spec §"Durable trust and app-data transactions" (app-data protocol). Second of three WU-002 units; builds on WU-002a (trust seam, merged).

**Goal:** Give app-data writes the same prepare → (host persists receipt) → finalize protocol as trust: `prepare_app_data_put` validates + signs + encodes the receipt bundle **without mutating the live store**; `finalize_app_data_put` commits it idempotently. Generalize WU-002a's `prepared_trust` slot into a single `prepared_mutation` enum so a profile holds **at most one** prepared mutation of either kind (spec: "At most one prepared mutation exists per profile").

**Architecture:** `crates/riot-ffi/src/mobile_state.rs`. App-data receipt bytes (the `commit_local_app_entries` output, already returned today) are what the host persists and replays via the existing `replay_app_data_bundle`; the change is to return them from prepare (pre-commit) and commit in finalize (post-persist). Both the ungated `app_data_put_with_receipt` and the gated `app_execution_put_with_receipt` get a two-phase form; the existing single-shot functions are re-expressed as prepare+finalize under one lock (byte-identical). No core/native/fixture changes.

**Scope boundary (do NOT exceed):**
- Modify only: `crates/riot-ffi/src/mobile_state.rs`, `crates/riot-ffi/src/apps_ffi.rs` (thin `AppRuntimeSession`/`AppExecutionSession` method wrappers + a `PreparedAppDataRecord`), tests `crates/riot-ffi/tests/apps_contract.rs`.
- **Never** touch: core, native shells, fixtures. (If the Android host-JVM `FakeAppRuntimeSession`/`FakeAppExecutionSession` in `RiotAppsControllerTest.kt` implements the changed UniFFI interface, add the override stubs there too — that Kotlin test fake is REQUIRED to keep the "Android (host-JVM unit tests)" CI job green when a `#[uniffi::export]` interface gains a method; it is not native feature work. See the `android-host-jvm-no-so` note.)
- Keep `app_data_put`, `app_data_put_with_receipt`, `app_execution_put_with_receipt`, and `replay_app_data_bundle` behavior byte-identical — existing tests (`app_data_round_trips_through_the_ffi_layer`, `app_data_persists_across_a_fresh_profile_via_replay`, `replay_rejects_a_non_app_data_bundle`, `app_data_put_does_not_break_sync_sessions`, the execution-session suite) MUST stay green.

**Verified anchors (origin/main incl. WU-002a #102):** `prepared_trust: Option<PreparedTrust>` field `mobile_state.rs:145`, `struct PreparedTrust` `:226`, init `:475`; `prepare_app_trust`/`finalize_app_trust`/`discard_prepared_trust` `~:3270-3300`; `app_execution_put_with_receipt` `:3484` (guard revalidate → sync guard → timestamp → `app_data_path` → `sign_local_app_entry` → `commit_local_app_entries` → return bytes); `app_data_put_with_receipt` `:3521` (same, ungated, `parse_entry_id` first); `replay_app_data_bundle` `:3550`; `commit_local_app_entries` `:3849`; `sign_local_app_entry` `:3820`. App-data deliberately does NOT bump `app_execution_generation`.

---

## Design (read before Task 1)

### Shared prepared slot (spec: at most one prepared mutation per profile)

Replace the trust-only slot with one enum so trust and app-data cannot both be prepared at once:

```rust
enum PreparedMutation {
    Trust(PreparedTrust),
    AppData(PreparedAppData),
}
// LocalProfile: `prepared: Option<PreparedMutation>` (renamed from `prepared_trust`).

struct PreparedAppData {
    /// `app_execution_generation` at prepare; finalize refuses if it moved
    /// (a community/namespace switch happened in between).
    generation: u64,
    /// The signed app-data entry, committed unchanged in finalize.
    signed: SignedWillowEntry,
    /// Canonical receipt bundle bytes — what the host persists and replays.
    receipt: Vec<u8>,
}
```

WU-002a's `prepare_app_trust`/`finalize_app_trust`/`discard_prepared_trust` are updated to read/write `Some(PreparedMutation::Trust(..))`; `finalize_app_trust` takes the slot and errors if it holds a non-Trust (or nothing). A new prepare of EITHER kind supersedes any held mutation (sets `prepared = None` first). This structurally enforces the singleton invariant N1 flagged.

### App-data prepare/finalize (mirror trust, receipt = bytes)

```
prepare_app_data_put(app_id, key, value)  [ungated]:
  with_active:
    - prepared = None                       (supersede)
    - reject if sync session active         (unchanged guard)
    - app_id = parse_entry_id; timestamp = next_app_write_timestamp
    - path = app_data_path(app_id, key); signed = sign_local_app_entry(...)
    - receipt = encode_bundle(&[signed.clone()])   (== what commit would return; pre-commit)
    - prepared = Some(AppData(PreparedAppData { generation, signed, receipt.clone() }))
    - return PreparedAppDataRecord { receipt }      (host persists receipt)
    // NO commit_local_app_entries.

finalize_app_data_put():
  with_active:
    - AppData(p) = take() matching, else Err
    - if p.generation != profile.app_execution_generation → Err (fail closed)
    - commit_local_app_entries(profile, vec![p.signed])?     (store mutates, post-persist)
    - Ok(())                                                 (app-data does NOT bump generation)
  // Idempotent: re-committing the same signed entry is an LWW no-op; a crash-retry
  // that re-runs prepare+finalize (or replays the receipt via replay_app_data_bundle)
  // cannot double-apply.
```

Gated `app_execution_put_with_receipt` gets a two-phase form `prepare_app_execution_put` / `finalize_app_execution_put` that additionally runs `revalidate_execution(profile, snap)` in prepare and stores `snap.generation`. Both single-shot originals become `with_active(|p| { let pre = prepare_*_locked(p, ..)?; finalize_appdata_locked(p, pre) })` — byte-identical.

Factor lock-free helpers `prepare_app_data_locked(&mut LocalProfile, app_id:[u8;32], key, value) -> Result<PreparedAppData>` and `finalize_app_data_locked(&mut LocalProfile, PreparedAppData) -> Result<()>` so both the single-shot and two-phase callers share one body.

`receipt` note: verify the exact bytes `commit_local_app_entries` returns are `encode_bundle(&[signed])`. If commit adds inventory framing, capture the receipt the SAME way commit produces it (extract that encode step) so the receipt a host persists is byte-identical to today's `app_data_put_with_receipt` return — existing replay/persist tests depend on it.

---

## Task 1: Generalize the prepared slot to `PreparedMutation` (refactor WU-002a, stays green)

**Files:** `crates/riot-ffi/src/mobile_state.rs`.

- [ ] **Step 1: Write the failing test** — the singleton invariant: preparing app-data after preparing trust supersedes the trust (only one prepared mutation exists):

```rust
#[test]
fn a_second_prepare_supersedes_the_first_across_kinds() {
    let (_profile, runtime, app_id) = organizer_with_installed_untrusted_app();
    runtime.prepare_app_trust(app_id.clone(), true).unwrap();
    // Preparing an app-data write supersedes the pending trust.
    runtime.prepare_app_data_put(app_id.clone(), "items/a".into(), b"{}".to_vec()).unwrap();
    // Finalizing now finalizes the app-data write, and the trust was dropped:
    runtime.finalize_app_data_put().unwrap();
    assert!(!runtime.is_app_trusted(app_id).unwrap(), "superseded trust never committed");
}
```

- [ ] **Step 2: Run to verify fail** (no `prepare_app_data_put`).

- [ ] **Step 3: Implement the rename + enum** (no behavior change to trust): add `enum PreparedMutation { Trust(PreparedTrust), AppData(PreparedAppData) }` and `struct PreparedAppData {..}` near `PreparedTrust`. Rename `LocalProfile.prepared_trust` → `prepared: Option<PreparedMutation>` (field `:145`, init `:475`, doc-comment). Update WU-002a's fns:
  - `prepare_app_trust`: `profile.prepared = None; ... profile.prepared = Some(PreparedMutation::Trust(prepared));`
  - `finalize_app_trust`: `match profile.prepared.take() { Some(PreparedMutation::Trust(p)) => finalize_trust_locked(profile, p), _ => Err(MobileError::Internal) }`.
  - `discard_prepared_trust`: keep clearing `profile.prepared = None` (rename doc; consider a shared `discard_prepared_mutation`, but keep the existing pub name for WU-002c compatibility and add `discard_prepared_app_data` as an alias that also clears).
  - `set_app_trust` (one-lock) is unchanged (it never touches the slot).
  - The inline generation-guard test + all WU-002a integration tests keep passing (only the slot's type changed).

- [ ] **Step 4: Run to verify pass** — the WU-002a trust suite + inline guard still green; the new supersede test fails only until Task 2 adds `prepare_app_data_put`.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/apps_contract.rs
git commit -m "refactor(ffi): one prepared-mutation slot shared by trust + app-data (WU-002b)"
```

---

## Task 2: `prepare_app_data_put` / `finalize_app_data_put` (ungated)

**Files:** `crates/riot-ffi/src/mobile_state.rs`, `crates/riot-ffi/src/apps_ffi.rs`, tests.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn prepare_app_data_does_not_mutate_and_finalize_commits() {
    let (_p, runtime, app_id) = organizer_with_installed_untrusted_app();
    // (app-data does not require trust; a fresh installed app can write its own data)
    let rec = runtime.prepare_app_data_put(app_id.clone(), "items/a".into(), b"{\"done\":false}".to_vec()).unwrap();
    assert!(!rec.receipt.is_empty());
    // prepare must NOT commit: the value is not readable yet.
    assert_eq!(runtime.app_data_get(app_id.clone(), "items/a".into()).unwrap(), None);
    runtime.finalize_app_data_put().unwrap();
    assert_eq!(runtime.app_data_get(app_id, "items/a".into()).unwrap(), Some(b"{\"done\":false}".to_vec()));
}

#[test]
fn prepare_app_data_without_finalize_leaves_store_untouched() {
    let (_p, runtime, app_id) = organizer_with_installed_untrusted_app();
    runtime.prepare_app_data_put(app_id.clone(), "items/a".into(), b"x".to_vec()).unwrap();
    assert_eq!(runtime.app_data_get(app_id, "items/a".into()).unwrap(), None);
}

#[test]
fn the_prepared_app_data_receipt_replays_to_the_same_value() {
    // The receipt a host persists is admissible via the existing replay path.
    let (_p, runtime, app_id) = organizer_with_installed_untrusted_app();
    let rec = runtime.prepare_app_data_put(app_id.clone(), "items/a".into(), b"v".to_vec()).unwrap();
    runtime.finalize_app_data_put().unwrap();
    // A fresh profile that only replays the receipt sees the same value (mirror
    // app_data_persists_across_a_fresh_profile_via_replay).
    // ... build second profile, replay rec.receipt, assert get == "v".
}
```

- [ ] **Step 2: Run to verify fail.**

- [ ] **Step 3: Implement** the lock-free helpers `prepare_app_data_locked` / `finalize_app_data_locked`, the public `prepare_app_data_put` / `finalize_app_data_put` (store `PreparedMutation::AppData`), re-express `app_data_put_with_receipt` as prepare+finalize under one lock, and add `AppRuntimeSession::prepare_app_data_put`/`finalize_app_data_put` + `PreparedAppDataRecord { receipt: Vec<u8> }` to `apps_ffi.rs`.

- [ ] **Step 4: Run to verify pass** — new tests + `app_data_round_trips_through_the_ffi_layer` + `app_data_persists_across_a_fresh_profile_via_replay` + `replay_rejects_a_non_app_data_bundle` green.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/src/apps_ffi.rs crates/riot-ffi/tests/apps_contract.rs
git commit -m "feat(ffi): app-data prepare/persist/finalize seam (ungated)"
```

---

## Task 3: Gated `prepare_app_execution_put` / `finalize_app_execution_put`

**Files:** `crates/riot-ffi/src/mobile_state.rs`, `crates/riot-ffi/src/apps_ffi.rs` (`AppExecutionSession` methods), tests.

- [ ] **Step 1: Write the failing test** — the gated variant revalidates the snapshot in prepare and fails closed if trust is revoked between prepare and finalize (revoke bumps the generation):

```rust
#[test]
fn revoking_between_prepare_and_finalize_app_execution_fails_closed() {
    // open an execution session for a trusted app, prepare a put, revoke the app
    // (bumps generation), then finalize -> Err, value never written.
}
```

- [ ] **Step 2/3:** Implement `prepare_app_execution_put(inner, snap, key, value)` = `revalidate_execution` + `prepare_app_data_locked` capturing `profile.app_execution_generation`, stored as `PreparedMutation::AppData`; `finalize_app_execution_put(inner, snap)` = `revalidate_execution` again + `finalize_app_data_locked`. Re-express `app_execution_put_with_receipt` as prepare+finalize under one lock. Expose on `AppExecutionSession`.

- [ ] **Step 4: Run to verify pass** — new test + the execution-session suite (`revoke_fails_the_next_app_execution_read_and_commit`, `namespace_replacement_fails_stale_app_execution_access`, `stale_approval_generation_...`) green.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/src/apps_ffi.rs crates/riot-ffi/tests/apps_contract.rs
git commit -m "feat(ffi): gated app-execution prepare/finalize seam"
```

---

## Task 4: Android host-JVM fakes + full gate

- [ ] **Step 1:** If `RiotAppsControllerTest.kt`'s `FakeAppRuntimeSession` implements the changed interface, add override stubs for `prepareAppDataPut`/`finalizeAppDataPut` (and any `AppExecutionSession` fake for the gated pair). Grep `AppRuntimeSessionInterface`/`AppExecutionSessionInterface` implementors first. (Kotlin only — no `.so`.)
- [ ] **Step 2:** `cargo fmt --all -- --check` → PASS (run `cargo fmt --all` first to avoid the line-length surprise that bit WU-002a).
- [ ] **Step 3:** `cargo clippy --workspace --all-features -- -D warnings` → PASS.
- [ ] **Step 4:** `cargo test --workspace --all-features` → PASS.
- [ ] **Step 5: Coverage:** `cargo llvm-cov --workspace --all-features --fail-under-lines $(jq -r '.thresholds.llvm.lines' .coverage-thresholds.json)` (=95) → PASS. (NOT tarpaulin.)
- [ ] **Step 6:** open PR; the CI merge-when-green pattern lands it.

## Definition of Done

- One `prepared: Option<PreparedMutation>` slot holds at most one trust OR app-data prepared mutation; a second prepare of either kind supersedes; WU-002a trust behavior unchanged.
- `prepare_app_data_put`/`finalize_app_data_put` (ungated) and `prepare_app_execution_put`/`finalize_app_execution_put` (gated) split sign-from-commit: prepare returns the receipt bytes without mutating the store; finalize commits idempotently, generation-guarded (fail closed on a switch/revoke between the two).
- `app_data_put(_with_receipt)` + `app_execution_put_with_receipt` re-expressed as prepare+finalize under one lock, byte-identical — all existing app-data/replay/execution tests green.
- Android host-JVM fakes updated; `fmt`/`clippy`/`test --workspace`/`llvm-cov ≥95` green. No core/native-feature/fixture changes.

## Explicitly deferred

- **WU-002c:** native iOS/macOS/Android wiring of BOTH seams — the shared authority/persistence lock spanning host persist, persist-first ordering, exact storage-full/save-failed/device-persistence alert copy, session-invalidation-before-WebView-destroy on revoke, and fault injection at every boundary (prepare, persist, finalize, session-invalidation, teardown, termination, rebuild).
