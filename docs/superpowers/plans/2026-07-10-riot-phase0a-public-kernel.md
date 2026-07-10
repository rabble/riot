# Riot Phase 0A Public Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` in this session or `superpowers:executing-plans` in a fresh session. Use `superpowers:test-driven-development` for every behavior change and `superpowers:verification-before-completion` before claiming a gate.

**Goal:** Prove that native iOS and Android runtimes can create, inspect, authorize, and atomically import the same signed Riot alert using canonical Willow entry/capability bytes and corrected WILLIAM3 digests.

**Architecture:** A shared Rust core owns deterministic alert encoding, ephemeral communal-author authority, the Riot evidence-bundle codec, and a bounded in-memory import transaction. The store is a Riot wrapper around namespace-local Willow join state plus seen-entry and receipt indexes. UniFFI exposes synchronous, session-owned handles to minimal XCTest and Android instrumentation hosts; shell scripts transfer opaque bundle bytes and compare facts.

**Tech Stack:** Rust 1.95, `willow25 =0.6.0-alpha.3`, `bab_rs =0.8.1`, `minicbor`, Ed25519, SHA-256, UniFFI 0.32, Swift/XCTest, Kotlin/Android instrumentation, Bash orchestration.

---

## Read First

- Design: `docs/superpowers/specs/2026-07-10-riot-evidence-sprint-design.md`
- Willow audit: `docs/research/2026-07-10-willow-implementation-audit.md`
- Frozen environment: `fixtures/manifest.json`
- WU0 evidence: `docs/decisions/phase0a-wu0-report.md`

Do not execute the superseded Swift/JSON prototype plan. Do not add Drop Format, OpenMLS, a UI, persistent storage, or more object schemas.

## Shared-Worktree Boundary

At plan publication, WU0 is committed at `69ea94f`. WU0R and WU1 alert-codec work are already present as uncommitted shared-worktree changes in:

- `Cargo.toml`, `Cargo.lock`, and `crates/riot-core/Cargo.toml`
- `crates/riot-core/src/model/mod.rs`
- `crates/riot-core/tests/public_alert.rs`
- `fixtures/objects/alert-golden-1.cbor`

Treat those paths as owned by their current worker until committed. Inspect and extend them; never replace, revert, or re-bless their fixtures without explaining the byte change. Before every task, run `git status --short`, record the current HEAD, and stage only the files named by that task.

## Fixed Public Contracts

```rust
pub const EVIDENCE_MAGIC: &[u8; 6] = b"RIOTE1";
pub const EVIDENCE_CODEC: &str = "org.riot.evidence-bundle/1";
pub const ALERT_SCHEMA: &str = "org.riot.alert/1";
pub const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_BUNDLE_ENTRIES: usize = 64;
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

pub type NamespaceId = [u8; 32];
pub type SubspaceId = [u8; 32];
pub type EntryDigest = [u8; 32];
pub type ObjectDigest = [u8; 32];
pub type BundleDigest = [u8; 32];
```

The alert path is exactly `[b"objects", b"alert", object_id_16_raw_bytes, revision_id_16_raw_bytes]`. The outer bundle item stores canonical entry bytes, canonical capability bytes, the exact 64-byte signature, and exact payload bytes. It does not re-encode Willow or Meadowcap fields.

## Task 0: Close WU0R — Corrected Willow Basis

**Files:**

- Modify: `Cargo.toml`, `Cargo.lock`, `crates/riot-core/Cargo.toml`
- Modify: `crates/xtask/src/main.rs`, `fixtures/manifest.json`
- Create: `fixtures/willow/william3-vectors.json`
- Create: `crates/riot-conformance/tests/william3_vectors.rs`
- Modify: `docs/decisions/phase0a-wu0-report.md`

- [ ] **Step 1: Make the contract validator reject the obsolete graph**

Add a unit-testable `validate_contents(root) -> Vec<String>` helper. Require `willow25 =0.6.0-alpha.3`, `bab_rs =0.8.1`, a non-empty `william3_vectors_sha256`, and disabled `drop_format`. Its test feeds an old manifest and asserts a failure naming `willow25 =0.5.0`.

Run `cargo test -p xtask`. Expected RED: the dependency-specific checks do not exist.

- [ ] **Step 2: Freeze corrected WILLIAM3 vectors**

The fixture JSON contains input hex, expected 32-byte digest hex, `bab_rs` version, and source provenance for empty bytes, ASCII `riot`, and the committed alert golden bytes. The test computes each digest through `bab_rs` and asserts the alert payload is byte-identical to `fixtures/objects/alert-golden-1.cbor`.

Run `cargo test -p riot-conformance william3_`. Expected RED before fixture/test implementation, then GREEN with all vectors equal.

- [ ] **Step 3: Finish corrected pins and frozen hashes**

```toml
willow25 = { version = "=0.6.0-alpha.3", default-features = false, features = ["std"] }
bab_rs = { version = "=0.8.1", default-features = false, features = ["william3"] }
```

Regenerate `Cargo.lock`, update `cargo_lock_sha256`, record the vector-file hash, and make the WU0 report distinguish platform PASS from corrected-dependency PASS.

- [ ] **Step 4: Prove all five Rust targets and feature closure**

```bash
cargo xtask validate-contracts
cargo test -p riot-conformance william3_
cargo check --workspace --all-targets --locked
cargo check -p riot-core --target aarch64-apple-ios-sim --locked
cargo check -p riot-core --target aarch64-apple-ios --locked
cargo check -p riot-core --target aarch64-linux-android --locked
cargo check -p riot-core --target x86_64-linux-android --locked
mkdir -p build/evidence
cargo tree -p riot-ffi -e features > build/evidence/wu0r-feature-tree.txt
! rg 'willow25 feature "drop_format"|bab_rs v0\.[0-7]\.' build/evidence/wu0r-feature-tree.txt
```

Expected: every command exits 0 and the negative search finds nothing. Record output and SHA-256 hashes in the WU0 report.

- [ ] **Step 5: Commit only WU0R**

```bash
git add Cargo.toml Cargo.lock crates/riot-core/Cargo.toml crates/xtask/src/main.rs fixtures/manifest.json fixtures/willow crates/riot-conformance/tests/william3_vectors.rs docs/decisions/phase0a-wu0-report.md
git commit -m "fix: use corrected Willow digest basis"
```

Stop condition: no Willow entry, capability, bundle, or join code proceeds until this task is GREEN.

## Task 1: Finish WU1A — Deterministic Alert Payload

**Files:**

- Modify: `crates/riot-core/src/model/mod.rs`
- Modify: `crates/riot-core/tests/public_alert.rs`
- Modify: `fixtures/objects/alert-golden-1.cbor`
- Create: `fixtures/objects/alert-golden-1.json`

- [ ] **Step 1: Adopt the existing public test suite**

Do not recreate the already-started test file. Ensure it covers deterministic round-trip, golden bytes, absent optionals, expiry order, required source claims, maximum lengths/counts, unknown and duplicate keys, non-shortest integers, indefinite containers, trailing bytes, truncation, and the 1 MiB ceiling.

Run `cargo test -p riot-core --test public_alert`. Expected RED: list the exact missing rejection behavior; do not loosen a test to obtain GREEN.

- [ ] **Step 2: Complete strict canonical encoding and decoding**

`encode_alert` validates first and emits a definite map with ascending integer keys. `decode_alert` rejects over-limit input before allocation, parses known keys once, validates the object, re-encodes it, and requires byte-for-byte equality plus EOF. Keep author, namespace, trust, route, and receipt facts out of `AlertPayload`.

- [ ] **Step 3: Freeze a readable projection without making it authoritative**

Create `alert-golden-1.json` containing the same field values and the CBOR SHA-256. Tests may use JSON only to diagnose fixture drift; CBOR remains the signed form.

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --check
cargo clippy -p riot-core --all-targets -- -D warnings
cargo test -p riot-core --test public_alert
git add crates/riot-core/src/model/mod.rs crates/riot-core/tests/public_alert.rs fixtures/objects/alert-golden-1.cbor fixtures/objects/alert-golden-1.json
git commit -m "feat: add deterministic alert payload codec"
```

## Task 2: WU1B — Communal Author, Clock, and Canonical Willow Components

**Files:**

- Modify: `crates/riot-core/src/willow/mod.rs`
- Create: `crates/riot-core/src/willow/{clock,identity,entry,digest}.rs`
- Create: `crates/riot-core/tests/public_willow.rs`
- Create: `fixtures/willow/communal-author.json`, `fixtures/willow/communal-entry.bin`

- [ ] **Step 1: Write public authority tests**

Prove full 32-byte communal namespace/subspace IDs; separate UTC Unix seconds and Willow TAI/J2000 microseconds from one clock snapshot; the exact four-component path; canonical entry and capability decode with no trailing bytes; author-subspace success and different-subspace denial; payload length/WILLIAM3 checks before schema decode; and no debug, serialization, or public accessor for the signing key.

Run `cargo test -p riot-core --test public_willow`. Expected RED: modules and public types are absent.

- [ ] **Step 2: Implement Riot-owned wrappers**

```rust
pub struct ClockSnapshot { pub unix_seconds: u64, pub uncertainty_seconds: u32 }

pub struct AuthorIdentity {
    pub namespace_id: [u8; 32],
    pub subspace_id: [u8; 32],
    pub namespace_kind: NamespaceKind,
    pub signing_key_id: [u8; 32],
}

pub struct SignedWillowEntry {
    pub entry_bytes: Vec<u8>,
    pub capability_bytes: Vec<u8>,
    pub signature: [u8; 64],
    pub payload_bytes: Vec<u8>,
}
```

Keep concrete Willow generics private. The signer owns zeroizing secret material and is neither `Clone` nor `Debug`. Discard the privilege-less communal namespace secret and create a zero-delegation capability for the author subspace.

- [ ] **Step 3: Implement digest domains exactly**

Use corrected WILLIAM3 for `payload_digest`, SHA-256 over alert bytes for `object_digest`, and this stream for `entry_digest`:

```text
"riot/entry-digest/v1" || u32be(entry_bytes.len) || entry_bytes ||
u32be(capability_bytes.len) || capability_bytes || signature[64]
```

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --check
cargo clippy -p riot-core --all-targets -- -D warnings
cargo test -p riot-core public_
cargo test -p riot-conformance william3_
```

Commit task files with `feat: add communal Willow authority`.

## Task 3: WU1C — Bounded Riot Evidence Bundle

**Files:**

- Create: `crates/riot-core/src/willow/bundle.rs`
- Modify: `crates/riot-core/src/willow/mod.rs`
- Create: `crates/riot-core/tests/public_bundle.rs`
- Create: `fixtures/willow/bundle-golden-1.riot-evidence`

- [ ] **Step 1: Write RED codec tests**

Cover deterministic one-item bytes, 64-entry success, 65-entry rejection, 8 MiB rejection before decode, bad magic/version/codec, duplicate/unknown keys, indefinite containers, trailing bytes, non-64-byte signature, non-canonical entry/capability, payload length/digest mismatch, invalid authorization, and a valid unknown signer remaining eligible.

Run `cargo test -p riot-core --test public_bundle`. Expected RED: codec absent.

- [ ] **Step 2: Implement a two-stage bounded decoder**

Enforce artifact size, outer counts, field lengths, authorization totals, and nesting/node limits before Willow decoding. Validate in this order:

```text
outer bounds/canonical CBOR → canonical Entry → canonical WriteCapability
→ 64-byte signature → payload length/corrected WILLIAM3
→ Meadowcap authorization → Riot alert schema
```

Return stable reason codes; error strings never contain input bytes or payload text.

- [ ] **Step 3: Verify and commit**

```bash
cargo test -p riot-core --test public_bundle
cargo test -p riot-core public_
cargo clippy -p riot-core --all-targets -- -D warnings
```

Commit with `feat: add bounded Riot evidence bundle codec`. G1 is PASS only after Tasks 1–3 pass; a failure stops WU2.

## Task 4: WU2A — Namespace-Local Willow Join

**Files:**

- Create: `crates/riot-core/src/import/join.rs`
- Modify: `crates/riot-core/src/import/mod.rs`
- Create: `crates/riot-core/tests/core_import_join.rs`
- Create: `fixtures/imports/join-cases.json`

- [ ] **Step 1: Write join-law and edge-case tests**

Cover newer-prefix pruning, a candidate immediately dominated by a newer prefix, equal coordinate tie by greatest WILLIAM3 digest then greatest payload length, distinct subspace, distinct namespace, duplicate insertion, and every permutation of at least four interacting entries.

For each permutation compare Riot's live-entry set with `willow25::storage::MemoryStore` alpha.3. Assert commutativity, associativity, and idempotence of the live view.

Run `cargo test -p riot-core --test core_import_join`. Expected RED: join types are absent.

- [ ] **Step 2: Implement namespace-local state**

Use a map keyed by full namespace ID. Within a namespace, compare only entries in the same subspace for prefix pruning. Return:

```rust
pub enum JoinEffect {
    Applied { pruned_entry_digests: Vec<[u8; 32]> },
    Dominated { dominating_entry_digests: Vec<[u8; 32]> },
    AlreadyPresent,
}
```

Cap pruned/dominating references at 1,024 and reject state growth before mutation. Do not use `MemoryStore` in production state; it is a differential oracle only.

- [ ] **Step 3: Verify and commit**

```bash
cargo test -p riot-core --test core_import_join
cargo test -p riot-core core_import_
```

Commit with `feat: implement namespace-local Willow join`.

## Task 5: WU2B — Preview, Atomic Commit, Receipts, and Provenance

**Files:**

- Create: `crates/riot-core/src/import/{error,store,preview,receipt,provenance}.rs`
- Create: `crates/riot-core/src/session.rs`
- Modify: `crates/riot-core/src/import/mod.rs`, `crates/riot-core/src/lib.rs`
- Create: `crates/riot-core/tests/core_import_transaction.rs`
- Create: `crates/riot-core/tests/core_import_lifecycle.rs`
- Create: `crates/riot-conformance/tests/core_import_hostile.rs`

- [ ] **Step 1: Write transactional RED tests**

Cover valid unknown-signer preview; hard-ineligible signature/capability/schema; all-or-nothing selection; mixed new/duplicate; duplicate-only `NoChanges`; new dominated entry increments generation and gets a first receipt; stale preview precedes duplicate detection; foreign IDs; store full; injected pre-swap failure; rollback byte-for-byte equality; provenance separation of cryptographic facts from trust; and retained-byte ceilings.

Run `cargo test -p riot-core core_import_`. Expected RED: store/session types are absent.

- [ ] **Step 2: Implement one arbiter and copy-on-write swap**

All session, store, preview, signer, generation, index, and receipt state lives behind one `Arc<Mutex<SessionState>>`. Child handles contain only IDs and the arbiter. Build a bounded next snapshot, then perform one pointer swap. No observable state changes before that swap.

```rust
pub enum EntryDisposition {
    Applied { entry_id: u64, pruned_entry_digests: Vec<[u8; 32]> },
    Dominated { dominating_entry_digests: Vec<[u8; 32]> },
    AlreadyPresent { entry_id: u64, insertion_receipt_id: u64 },
}

pub enum ImportCommitResult {
    Committed(ImportReceipt),
    NoChanges(DuplicateResult),
}
```

The seen index permanently records the first receipt for every accepted entry, including entries dominated on arrival.

- [ ] **Step 3: Prove lifecycle linearization**

Race commit/reject, close/commit, and session-close/child-action. Assert one terminal winner, documented error precedence, no deadlock within two seconds, and no second receipt/swap.

- [ ] **Step 4: Prove hostile-input and log safety**

Mutate every truncation boundary, bad lengths, nesting, strings/arrays, unknown/duplicate keys, invalid UTF-8, malformed Willow components, signatures, and payload digests. Add panic injection under `cfg(test)`. Capture logs and assert no hostile marker or key material appears. A panic returns `INTERNAL_ERROR` and quarantines the session.

- [ ] **Step 5: Verify and commit G2 core**

```bash
cargo test -p riot-core core_import_
cargo test -p riot-conformance core_import_
cargo test -p riot-conformance hostile_bundle_
cargo test -p riot-conformance hostile_alert_
cargo clippy --workspace --all-targets -- -D warnings
```

Commit with `feat: add atomic preview-first evidence import`. G2 must be GREEN before native binding work.

## Task 6: WU3A — Stable UniFFI Boundary

**Files:**

- Modify: `crates/riot-ffi/Cargo.toml`, `crates/riot-ffi/src/lib.rs`
- Create: `crates/riot-ffi/build.rs`, `crates/riot-ffi/tests/ffi_contract.rs`
- Modify: `crates/xtask/src/main.rs`
- Create: `scripts/phase0a/generate-bindings.sh`

- [ ] **Step 1: Write RED boundary tests**

Test the design surface: open, identity, create store, encode alert, create/inspect bundle, commit/reject, provenance, close/idempotent close, wrong session, stale/consumed preview, sanitized errors, and panic quarantine. No private key or Willow generic type may be exported.

- [ ] **Step 2: Implement synchronous handle wrappers**

Every exported entrypoint catches unwind, maps internal errors to stable codes, and delegates admission/mutation to the core arbiter. Swift/Kotlin records contain full IDs and byte arrays; never truncate identifiers.

- [ ] **Step 3: Generate and compile bindings**

```bash
cargo test -p riot-ffi
cargo xtask generate-bindings
cargo build -p riot-ffi --release --target aarch64-apple-ios
cargo build -p riot-ffi --release --target aarch64-apple-ios-sim
cargo build -p riot-ffi --release --target aarch64-linux-android
cargo build -p riot-ffi --release --target x86_64-linux-android
```

Commit with `feat: expose Riot evidence core through UniFFI`.

## Task 7: WU3B — Native Runtime Hosts and Two-Way Handoff

**Files:**

- Create: `apps/ios/RiotEvidence/RiotEvidence.xcodeproj/`
- Create: `apps/ios/RiotEvidence/RiotEvidenceTests/{IOSCreatesBundle,IOSImportsAndroidBundle}.swift`
- Modify: `apps/android/app/build.gradle.kts`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/CrossRuntimeHandoffTest.kt`
- Create: `scripts/phase0a/{package-ios,package-android,cross-runtime-handoff}.sh`

- [ ] **Step 1: Make native tests fail on missing packaged bindings**

The iOS producer and ordered Android scenario call generated bindings, never reimplement CBOR/Willow. Expected RED is a missing native package/library or generated binding symbol.

- [ ] **Step 2: Package each release-shaped native library**

Copy only required release libraries, bindings, headers/module maps, and notices. Android packages `arm64-v8a` for runtime and `x86_64` as compile-only evidence. iOS packages simulator runtime plus unsigned-device compile evidence.

- [ ] **Step 3: Implement opaque-byte handoff**

Follow the design's eight-step protocol exactly. The shell hashes and transports `.riot-evidence` bytes but never decodes them. Producer/consumer facts agree on full IDs, timestamps, corrected WILLIAM3, bundle/entry/object digests, byte count, signature/capability validity, `UnknownTrust`, and `Applied`. The two producer authors and objects differ.

- [ ] **Step 4: Run G3 and commit**

```bash
scripts/phase0a/cross-runtime-handoff.sh
```

Expected: both native legs pass, bytes remain identical in transit, and `build/evidence/g3-runtime-handoff.json` records hashes and versions. Commit with `test: prove two-way native Riot handoff`.

## Task 8: WU4 — Adversarial Verification and Decision Report

**Files:**

- Create: `scripts/phase0a/verify.sh`
- Create: `docs/decisions/phase0a-gate-report.md`
- Modify: `README.md`

- [ ] **Step 1: Encode every gate as a command**

`verify.sh` runs every command in the design, captures output/status, hashes artifacts, scans dependency features/native packages, and writes per-gate results. Missing or skipped commands are INCONCLUSIVE, never PASS.

- [ ] **Step 2: Scan release closure**

Reject Drop Format, `bab_rs <0.8.1`, OpenMLS/group code, deterministic production providers, forbidden Ed25519 features, plaintext fixture secrets, and secret-bearing symbols/log strings. Feature-tree policy is authoritative; symbol scanning is defense in depth.

- [ ] **Step 3: Run clean verification**

```bash
rm -rf build/evidence
scripts/phase0a/verify.sh
git status --short
```

Expected: G0–G3 each say PASS with commands, environment, evidence paths, hashes, elapsed aggregate agent-hours, and next action. If required evidence cannot run, record INCONCLUSIVE and stop claiming GO.

- [ ] **Step 4: Update status honestly**

Replace README's stale “Planning only” text with the achieved evidence boundary. Do not claim field readiness, production security, Drop interoperability, private groups, persistent storage, or physical-device proof.

- [ ] **Step 5: Review and finish**

Use `superpowers:requesting-code-review`, address findings with `superpowers:receiving-code-review`, rerun `superpowers:verification-before-completion`, then use `superpowers:finishing-a-development-branch` to choose merge/PR/cleanup.

## Execution Order and Parallelism

```text
WU0R → alert codec → communal authority → bundle → join → transaction
→ FFI → native handoff → adversarial report
```

Safe parallelism is narrow: alert-codec completion may run beside WU0R because it does not hash with Willow; native scaffolding may start after FFI records freeze but gate claims wait for G2; report templates may be prepared early but contain only fresh results.

Do not parallelize edits to `Cargo.toml`, `Cargo.lock`, `fixtures/manifest.json`, `crates/riot-core/src/lib.rs`, or module indexes without explicit ownership. Never commit another worker's unstaged changes.

## Stop Rules

- G0 failure stops all Willow implementation.
- G1 failure stops WU2.
- G2 failure stops native expansion.
- WU4 begins at aggregate hour 14 even if earlier work is incomplete.
- At 16 aggregate agent-hours, unfinished required evidence is INCONCLUSIVE, never stretch scope.
