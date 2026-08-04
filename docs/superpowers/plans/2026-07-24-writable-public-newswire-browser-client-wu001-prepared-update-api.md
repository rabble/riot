# WU-001 — riot-core Prepared-Update API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a public prepare/sign-split news-post API to `riot-core` so the browser client can present an immutable review of exact canonical bytes and sign those same bytes later — spec requirement: "Prepared review bytes are exactly the bytes later signed… posting consumes those retained bytes instead of calling the current sign-immediately convenience path."

**Architecture:** `prepare_news_post` validates authority, captures the canonical payload encoding + `ClockSnapshot` + author binding into an immutable `PreparedNewsPost`. `sign_prepared_news_post` re-checks the author binding and signs the RETAINED bytes. The existing private `build_signed` (`crates/riot-core/src/newswire/entry.rs:142`) is refactored so its post-encoding core accepts pre-encoded bytes; all existing `create_signed_*` functions keep identical behavior (encode at call time, system clock) by delegating through the same core.

**Tech Stack:** Rust 1.95.0, riot-core (no new dependencies).

**Master plan:** `docs/superpowers/plans/2026-07-24-writable-public-newswire-browser-client-master-plan.md` (WU-001 section; stable interface I-2's `prepare_update`/`post_review` build on this).

**Execution:** worktree per `superpowers:using-git-worktrees`, branch `feat/browser-client-wu001-prepared-update-api`, base `feat/identity-handles-logging-share` @ `c2fae11c` (or later). Baseline green: 1980 tests pass, fmt/clippy clean.

---

## Load-bearing project constraints

1. **TDD mandatory.** RED first (failing compile/assertion counts), implement, GREEN, commit.
2. **Explicit `git add <paths>`** — never `-A`/`.`. Never `--no-verify`.
3. **Existing behavior untouched.** All `create_signed_*` factory functions (six always-on, including `create_signed_news_reaction_at`, plus the conformance-gated `_with_clock` variants) must produce byte-identical output for identical inputs before/after the refactor — they all delegate through `build_signed`, which keeps its signature and becomes encode + delegate.
4. **No `conformance` feature involvement.** The new API is always-on public API, not test-only.
5. **No new dependencies.** `crates/riot-core/Cargo.toml` is not modified by this unit.

## Verified code facts (checked on base `c2fae11c`)

- `crates/riot-core/src/newswire/entry.rs:142-177` — private `build_signed(author, snapshot, payload: NewswirePayload)`: encodes payload (`encode_payload`, `:113-122`), WILLIAM3 digests the bytes, builds path via `newswire_path(payload_path_kind(&payload), snapshot.tai_j2000_micros, &digest)`, builds + authorises the entry, returns `SignedNewswireRecord { signed: SignedWillowEntry { entry_bytes, capability_bytes, signature, payload_bytes }, entry_id, snapshot, payload }`.
- `entry.rs:208-221` — `require_post_authority(author, descriptor: &VerifiedNewswireRecord, post: &NewsPostV1)`: descriptor entry-id match + communal namespace membership.
- `entry.rs:303-311` — `create_signed_news_post` = `require_post_authority` → `system_snapshot()` → `build_signed`.
- `entry.rs:124-140` — `payload_path_kind` maps `NewswirePayload::NewsPost` → `NewswirePathKind::Post { space_descriptor_entry_id }`.
- `model.rs:74-86` — `NewsPostV1` is `#[derive(Debug, Clone, PartialEq, Eq)]`.
- `willow/clock.rs:11-19` — `ClockSnapshot` is `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` with pub fields `unix_seconds`, `tai_j2000_micros`, `uncertainty_seconds`.
- `newswire/mod.rs:14-33` — `NewswireError` variants include `AuthorityInvalid`, `ClockUnavailable`, `ModelInvalid`, `SigningFailed`.
- `EvidenceAuthor`: `namespace_id()` and `subspace_id()` accessors (willow/identity.rs); `generate_communal_author_for_namespace(namespace_id_bytes)` (identity.rs:266) mints additional members for tests.
- `SignedNewswireRecord` (entry.rs:29-35) and `SignedWillowEntry` (willow/entry.rs:33-38) have **pub fields, no accessor methods** (accessors exist only on `VerifiedNewswireRecord`, entry.rs:74-94). Tests use field access: `record.signed.payload_bytes`, `record.snapshot`, `record.entry_id`, `first.signed.entry_bytes`, `first.signed.signature`. The test snippets below show accessor-style calls for readability — implementer MUST adapt to field access (sanctioned deviation).
- Tests for this module are colocated in `entry.rs`'s `#[cfg(test)] mod tests` (starts :554; fixture helpers incl. `descriptor()` :561, `snapshot()` :575, outsider-author pattern :764-769 — mirror them).
- Admission round-trip for tests: `encode_bundle(&[record.signed.clone()])` → `RiotSession::open()` → `create_store()` → `store.inspect(&bytes, ImportContext::new("test"))` — `inspect` is public (session.rs:633-643), `encode_bundle` public (bundle.rs:181, re-exported import/mod.rs:8). Existing session tests use a private helper instead; the public path is confirmed viable.

## File Structure

| Path | Responsibility |
|---|---|
| `crates/riot-core/src/newswire/entry.rs` (modify) | `PreparedNewsPost` type + `prepare_news_post` + `sign_prepared_news_post`; `build_signed` refactor; colocated tests |
| `crates/riot-core/src/newswire/mod.rs` (modify) | re-export the new public items |

Nothing else. No Cargo.toml changes, no new files.

## API shape (binding for implementation)

```rust
/// An immutable prepared news post: the exact canonical payload bytes and
/// clock snapshot that a later `sign_prepared_news_post` call will sign.
/// Constructed only by `prepare_news_post`; fields are private.
pub struct PreparedNewsPost { /* post, payload_bytes, snapshot, author_subspace_id */ }

impl PreparedNewsPost {
    pub fn payload_bytes(&self) -> &[u8];
    pub fn snapshot(&self) -> ClockSnapshot;
    pub fn post(&self) -> &NewsPostV1;
}

pub fn prepare_news_post(
    author: &EvidenceAuthor,
    descriptor: &VerifiedNewswireRecord,
    post: NewsPostV1,
) -> Result<PreparedNewsPost, NewswireError>;

pub fn sign_prepared_news_post(
    author: &EvidenceAuthor,
    prepared: &PreparedNewsPost,
) -> Result<SignedNewswireRecord, NewswireError>;
```

Semantics:
- `prepare_news_post`: runs `require_post_authority` (fail = error, nothing retained), takes `system_snapshot()` (fail = `ClockUnavailable`), encodes the canonical payload (fail = `ModelInvalid`), retains all three plus `*author.subspace_id().as_bytes()` as the author binding.
- `sign_prepared_news_post`: if `*author.subspace_id().as_bytes() != prepared.author_subspace_id` → `Err(AuthorityInvalid)`; otherwise signs the RETAINED payload bytes with the RETAINED snapshot via the refactored core, producing a `SignedNewswireRecord` whose `signed.payload_bytes` are byte-identical to `prepared.payload_bytes()` and whose entry timestamp is `prepared.snapshot().tai_j2000_micros`.
- Refactor: extract the post-encoding body of `build_signed` into `fn build_signed_from_bytes(author, snapshot, path_kind: NewswirePathKind, payload_bytes: Vec<u8>, payload: NewswirePayload)`. `build_signed` keeps its signature and becomes encode + delegate.

**Clock-failure coverage note (review-mandated):** the `ClockUnavailable` path is deliberately untested. `system_snapshot()` has no fault seam in the always-on build — injectable clocks exist only behind the `conformance` feature, which this API must not depend on (constraint 4). The existing `create_signed_*` functions share this identical untestable path; the new API adds no new risk surface. Testable failure paths (authority, model validity) are covered by RED tests.

---

### Task 1: RED — failing tests for the prepare/sign-split API

**Files:**
- Modify: `crates/riot-core/src/newswire/entry.rs` (test module only)

- [ ] **Step 1: Write the failing tests** (colocated `#[cfg(test)]` module; use the module's existing test helpers for descriptor/author setup — read them first and reuse):

```rust
#[test]
fn prepared_post_signs_exactly_the_reviewed_bytes() {
    // Setup: organizer author + signed descriptor + VerifiedNewswireRecord
    // (reuse existing test fixture helpers in this module).
    let prepared = prepare_news_post(&author, &descriptor_record, sample_post())
        .expect("prepare succeeds for an authorized member");
    let record = sign_prepared_news_post(&author, &prepared).expect("sign succeeds");
    assert_eq!(record.signed().payload_bytes(), prepared.payload_bytes());
    assert_eq!(record.snapshot(), prepared.snapshot());
}

#[test]
fn signing_the_same_prepared_post_twice_is_deterministic() {
    let prepared = prepare_news_post(&author, &descriptor_record, sample_post()).unwrap();
    let first = sign_prepared_news_post(&author, &prepared).unwrap();
    let second = sign_prepared_news_post(&author, &prepared).unwrap();
    assert_eq!(first.signed().entry_bytes(), second.signed().entry_bytes());
    assert_eq!(first.signed().signature(), second.signed().signature());
    assert_eq!(first.entry_id(), second.entry_id());
}

#[test]
fn prepared_post_is_bound_to_the_preparing_author() {
    let prepared = prepare_news_post(&author, &descriptor_record, sample_post()).unwrap();
    let other = generate_communal_author_for_namespace(namespace_bytes).unwrap();
    assert!(matches!(
        sign_prepared_news_post(&other, &prepared),
        Err(NewswireError::AuthorityInvalid)
    ));
}

#[test]
fn prepare_news_post_rejects_an_unauthorized_author() {
    // Outsider author (different communal namespace, or same namespace but the
    // post names a descriptor it cannot speak for) — mirror the existing
    // outsider-author fixture pattern used for create_signed_news_post tests
    // (entry.rs:~764-769).
    let outsider = generate_communal_author().unwrap();
    assert!(matches!(
        prepare_news_post(&outsider, &descriptor_record, sample_post()),
        Err(NewswireError::AuthorityInvalid)
    ));
}

#[test]
fn prepared_post_enters_through_the_ordinary_admission_path() {
    let prepared = prepare_news_post(&author, &descriptor_record, sample_post()).unwrap();
    let record = sign_prepared_news_post(&author, &prepared).unwrap();
    let bundle = encode_bundle(&[record.signed().clone()]).unwrap();
    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    // descriptor must be admitted first so the post's path binding resolves,
    // mirroring how existing post tests admit records
    store.inspect(&descriptor_bundle, ImportContext::new("test")).unwrap();
    let outcome = store.inspect(&bundle, ImportContext::new("test")).unwrap();
    assert!(matches!(outcome, InspectOutcome::Preview(_)));
}
```
(Names/imports adjusted to the module's actual fixtures and accessor names — read existing tests in entry.rs and mirror them exactly. `sample_post()` = a minimal valid `NewsPostV1` per existing test helpers.)

- [ ] **Step 2: Watch them fail**

Run: `cargo test -p riot-core --lib newswire::entry 2>&1 | tail -20`
Expected: FAIL to compile — `prepare_news_post` / `sign_prepared_news_post` / `PreparedNewsPost` do not exist. This is the RED state.

- [ ] **Step 3: Commit the RED tests**

```bash
git add crates/riot-core/src/newswire/entry.rs
git commit -m "test(core): red tests for prepare/sign-split news post API"
```

### Task 2: Implement the API (GREEN)

**Files:**
- Modify: `crates/riot-core/src/newswire/entry.rs`
- Modify: `crates/riot-core/src/newswire/mod.rs` (re-exports)

- [ ] **Step 1: Refactor `build_signed`**

Extract its post-encoding body into:
```rust
fn build_signed_from_bytes(
    author: &EvidenceAuthor,
    snapshot: ClockSnapshot,
    path_kind: NewswirePathKind,
    payload_bytes: Vec<u8>,
    payload: NewswirePayload,
) -> Result<SignedNewswireRecord, NewswireError> {
    let digest = william3_digest(&payload_bytes);
    let path = newswire_path(path_kind, snapshot.tai_j2000_micros, &digest)?;
    // ...rest identical to current build_signed body...
}

fn build_signed(
    author: &EvidenceAuthor,
    snapshot: ClockSnapshot,
    payload: NewswirePayload,
) -> Result<SignedNewswireRecord, NewswireError> {
    let payload_bytes = encode_payload(&payload)?;
    let path_kind = payload_path_kind(&payload);
    build_signed_from_bytes(author, snapshot, path_kind, payload_bytes, payload)
}
```

- [ ] **Step 2: Add `PreparedNewsPost` + the two public functions** per the API shape above. `prepare_news_post` encodes via `encode_payload` on a cloned `NewswirePayload::NewsPost(post.clone())` and retains `post`. `sign_prepared_news_post` checks the author binding then calls `build_signed_from_bytes(author, prepared.snapshot, NewswirePathKind::Post { space_descriptor_entry_id: prepared.post.space_descriptor_entry_id }, prepared.payload_bytes.clone(), NewswirePayload::NewsPost(prepared.post.clone()))`.

- [ ] **Step 3: Re-export from `newswire/mod.rs`**

Add `PreparedNewsPost`, `prepare_news_post`, `sign_prepared_news_post` to the module's existing re-export list (mirror its style).

- [ ] **Step 4: Run the new tests — GREEN**

Run: `cargo test -p riot-core --lib newswire::entry 2>&1 | tail -10`
Expected: all five new tests PASS, all existing entry tests PASS.

- [ ] **Step 5: Prove existing behavior unchanged**

Run: `cargo test -p riot-core --all-features 2>&1 | tail -5`
Expected: full crate suite passes (existing `create_signed_*` tests green — they pin byte-level behavior through admission).

- [ ] **Step 6: Commit**

```bash
git add crates/riot-core/src/newswire/entry.rs crates/riot-core/src/newswire/mod.rs
git commit -m "feat(core): add prepare/sign-split news post API"
```

### Task 3: Full verification

**Files:** none (verification only; no commit unless fmt/clippy demand a touch-up)

- [ ] **Step 1: Gates**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p xtask -- validate-contracts
node scripts/web/check-wasm-graph.mjs
cargo tarpaulin --workspace --all-features --timeout 300 --exclude-files 'vendor/*' --fail-under "$(node -e "process.stdout.write(String(JSON.parse(require('fs').readFileSync('.coverage-thresholds.json','utf8')).thresholds.tarpaulin.lines))")"
```
Expected: all clean; workspace 1980+ passed / 0 failed (count grows by the new tests); validate-contracts PASS (no dependency change → no fixture refresh needed; if it fails on `cargo_lock_sha256`, STOP and report — this unit must not change the lockfile); graph contract still OK; **coverage floor holds** — tarpaulin line coverage meets the `.coverage-thresholds.json` floor (source of truth; never lowered). Tarpaulin is slow; use a 40-minute tool timeout.

- [ ] **Step 2: Commit only if gates required a touch-up** (e.g. fmt):
```bash
git add crates/riot-core/src/newswire/entry.rs
git commit -m "style(core): fmt/clippy touch-ups for prepared post API"
```

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p xtask -- validate-contracts
```

## Self-review checklist (requirement → task)

| Requirement | Task |
|---|---|
| Prepared review bytes are exactly the bytes later signed | Task 1 test 1, Task 2 Step 2 |
| Preparation captures canonical payload + timestamp; posting consumes retained bytes | Task 1 tests 1-2, Task 2 Steps 1-2 |
| Unauthorized prepare rejected (nothing retained) | Task 1 test 4 |
| Prepared posts enter through the ordinary admission path | Task 1 test 5 |
| Existing sign-immediately path byte-identical | Task 2 Step 5 (full crate suite) |
| Public API outside `conformance` | Task 2 (no cfg gates) |
| Author binding (prepared value usable only by preparing author) | Task 1 test 3, Task 2 Step 2 |
| Clock-failure path justified untestable (no always-on fault seam) | Semantics note |
| Coverage floor holds (`.coverage-thresholds.json` tarpaulin lines) | Task 3 Step 1 |
| Exports available to riot-client (WU-002) | Task 2 Step 3 |
