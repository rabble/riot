# Riot Phase 0A Public Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` in this session or `superpowers:executing-plans` in a fresh session. Use `superpowers:test-driven-development` for every behavior change and `superpowers:verification-before-completion` before claiming a gate.

**Goal:** Prove that native iOS and Android runtimes can create, inspect, authorize, and atomically import the same signed Riot alert using canonical Willow entry/capability bytes and corrected WILLIAM3 digests.

**Architecture:** A shared Rust core owns deterministic alert encoding, ephemeral communal-author authority, the Riot evidence-bundle codec, and a bounded in-memory import transaction. The store is a Riot wrapper around namespace-local Willow join state plus seen-entry and receipt indexes. UniFFI exposes synchronous, session-owned handles to minimal XCTest and Android instrumentation hosts; shell scripts transfer opaque bundle bytes and compare facts.

**Tech Stack:** Rust 1.95, `willow25 =0.6.0-alpha.3`, `bab_rs =0.8.1`, `hifitime =4.3.0`, `minicbor`, Ed25519, SHA-256, UniFFI 0.32, Swift/XCTest, Kotlin/Android instrumentation, Bash orchestration.

---

> **STATUS 2026-07-10:** Tasks 0–4 COMPLETE and PASS. Gate state: **G0 PASS, G1 PASS**; **WU2A (Task 4) join** done and pending independent review (`docs/decisions/phase0a-wu2a-report.md`). Next executable task is **Task 5 (WU2B — arbiter, transaction, preview/plan/commit, receipts, provenance)**, which reaches gate **G2**. Test commands require the conformance feature: `cargo test -p riot-core --features conformance` (release-surface containment test runs without it). 67 tests green across the workspace.

---

## Read First

- Design: `docs/superpowers/specs/2026-07-10-riot-evidence-sprint-design.md`
- Willow audit: `docs/research/2026-07-10-willow-implementation-audit.md`
- Frozen environment: `fixtures/manifest.json`
- WU0 evidence: `docs/decisions/phase0a-wu0-report.md`

Do not execute the superseded Swift/JSON prototype plan. Do not add Drop Format, OpenMLS, a UI, persistent storage, or more object schemas.

## Shared-Worktree Boundary

Implementation continued while this plan was being reviewed. Current code baseline commits are `50083bb` (claimed WU0R plus alert/authority) and `b484ce8` (claimed G1 bundle codec); the existing bundle implementation is committed under `crates/riot-core/src/import/mod.rs`. Review reopened both claims: the executable dependency validator still accepts the obsolete basis, vectors lack independent provenance, the cross-subspace test actually creates a second namespace, signer generation is infallible, bundle coverage is incomplete, and the release profile uses `panic = "abort"` despite a catch-and-quarantine requirement.

Before every task, run `git status --short`, record HEAD, and preserve any newly appearing concurrent edits. Never replace, revert, or re-bless fixtures without explaining byte changes. Stage only files named by that task plus the globally owned time ledger. A green test from the current baseline is evidence of implemented behavior, not proof that a reopened gate is complete.

Every task additionally owns one append/update to `docs/decisions/phase0a-time-ledger.json`, even when omitted from its file list. Record newly charged active time, commits, and evidence before starting the next task. Stop at the 1.5-hour G0/G1 repair checkpoint and hour-14 WU4 boundary.

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
pub type EntryId = [u8; 32];
pub type EvidenceDigest = [u8; 32];
pub type ObjectDigest = [u8; 32];
pub type BundleDigest = [u8; 32];
```

The alert path is exactly `[b"objects", b"alert", object_id_16_raw_bytes, revision_id_16_raw_bytes]`. The outer bundle item stores canonical entry bytes, canonical capability bytes, the exact 64-byte signature, and exact payload bytes. It does not re-encode Willow or Meadowcap fields.

## Task 0: Reopen and Close WU0R — Executable Corrected Basis

**Files:**

- Modify: `Cargo.toml`, `Cargo.lock`, `crates/riot-core/Cargo.toml`
- Modify: `crates/riot-conformance/Cargo.toml`
- Modify: `crates/xtask/src/main.rs`, `fixtures/manifest.json`
- Replace: `fixtures/willow/william3-vectors.txt` with `fixtures/willow/william3-vectors.json`
- Create: `crates/riot-conformance/tests/william3_vectors.rs`
- Modify: `docs/decisions/phase0a-wu0-report.md`
- Create: `docs/decisions/phase0a-time-ledger.json`

- [ ] **Step 1: Make the contract validator reject the obsolete graph**

Independent review reopening: validation is structural TOML/JSON parsing, but exact ceiling values and the resolved feature graph are not enforced. In isolated worktrees, `artifact_bytes = 1` passed, and enabling `willow25/drop_format` in `riot-core`, regenerating the lock, and refreshing its hash also passed. Enforce the exact frozen ceiling table (including 4 KiB Entry, exact 64-byte signature, and 16 MiB next-snapshot charge), inspect locked resolved features, and add both mutations as regression tests.

Add a unit-testable `validate_contents(root) -> Vec<String>` helper. Structurally parse TOML/JSON rather than substring matching. Require `willow25 =0.6.0-alpha.3`, `bab_rs =0.8.1`, direct `hifitime =4.3.0`, a non-empty `william3_vectors_sha256`, disabled `drop_format`, the manifest's actual `Cargo.lock` hash, the new namespace/reference/plan/store-charge ceilings, and `panic = "unwind"`. Its tests feed the old manifest/lock/profile independently and assert a specific failure for each regression.

Run `cargo test -p xtask`. Expected RED: the dependency-specific checks do not exist.

- [x] **Step 2: Freeze corrected WILLIAM3 vectors**

The fixture JSON contains input recipe/hex, expected 32-byte digest hex, `bab_rs` version, source URL/commit, and provenance for empty bytes, ASCII `riot`, the committed alert golden bytes, 700 bytes of `0xAB` (partial block), and `(0..5000).map(i % 251)` (multi-block). At least one expected value must be copied from or cross-checked against the independently implemented `Deln0r/willow-go` corrected-WILLIAM3 commit `9d848ee`; values blessed only by the Rust dependency under test do not close G0. The test computes each digest through `bab_rs`, exercises input shorter and longer than the 1,024-byte WILLIAM3 chunk, and asserts the alert payload is byte-identical to `fixtures/objects/alert-golden-1.cbor`.

Add `bab_rs = { workspace = true }` to `riot-conformance` dev-dependencies; tests never rely on an undeclared transitive crate.

Run `cargo test -p riot-conformance william3_`. Expected RED before fixture/test implementation, then GREEN with all vectors equal.

- [x] **Step 3: Finish corrected pins and frozen hashes**

```toml
willow25 = { version = "=0.6.0-alpha.3", default-features = false, features = ["std"] }
bab_rs = { version = "=0.8.1", default-features = false, features = ["william3"] }
hifitime = "=4.3.0"
```

Regenerate `Cargo.lock`, update `cargo_lock_sha256`, record the vector-file hash, and make the WU0 report distinguish platform PASS from corrected-dependency PASS.

Change the Phase 0A evidence release profile to `panic = "unwind"`; `panic = "abort"` makes the required FFI catch/quarantine result impossible. Record actual WU0R and WU1 active wall time in the ledger. Overlapping agents are summed; unreconstructable completed work is charged its full work-unit budget.

- [x] **Step 4: Prove all five Rust targets and feature closure**

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
rg 'panic = "unwind"' Cargo.toml
```

Expected: every command exits 0 and the negative search finds nothing. Record output and SHA-256 hashes in the WU0 report.

- [x] **Step 5: Commit only WU0R**

```bash
git add Cargo.toml Cargo.lock crates/riot-core/Cargo.toml crates/riot-conformance/Cargo.toml crates/xtask/src/main.rs fixtures/manifest.json fixtures/willow crates/riot-conformance/tests/william3_vectors.rs docs/decisions/phase0a-wu0-report.md docs/decisions/phase0a-time-ledger.json
git commit -m "fix: make corrected Willow basis executable"
```

Stop condition: no Willow entry, capability, bundle, or join code proceeds until this task is GREEN.

## Task 1: Finish WU1A — Deterministic Alert Payload

**Files:**

- Modify: `crates/riot-core/src/model/mod.rs`
- Modify: `crates/riot-core/tests/public_alert.rs`
- Modify: `fixtures/objects/alert-golden-1.cbor`
- Create: `fixtures/objects/alert-golden-1.json`

- [x] **Step 1: Adopt the existing public test suite**

Do not recreate the already-started test file. Ensure it covers deterministic round-trip, golden bytes, absent optionals, expiry order, required source claims, maximum lengths/counts, unknown and duplicate keys, non-shortest integers, indefinite containers, trailing bytes, truncation, and the 1 MiB ceiling.

Run `cargo test -p riot-core --test public_alert`. Expected RED: list the exact missing rejection behavior; do not loosen a test to obtain GREEN.

- [x] **Step 2: Complete strict canonical encoding and decoding**

`encode_alert` validates first and emits a definite map with ascending integer keys. `decode_alert` rejects over-limit input before allocation, parses known keys once, validates the object, re-encodes it, and requires byte-for-byte equality plus EOF. Keep author, namespace, trust, route, and receipt facts out of `AlertPayload`.

- [x] **Step 3: Freeze a readable projection without making it authoritative**

Create `alert-golden-1.json` containing the same field values and the CBOR SHA-256. Tests may use JSON only to diagnose fixture drift; CBOR remains the signed form.

- [x] **Step 4: Verify and commit**

```bash
cargo fmt --check
cargo clippy -p riot-core --all-targets -- -D warnings
cargo test -p riot-core --test public_alert
git add crates/riot-core/src/model/mod.rs crates/riot-core/tests/public_alert.rs fixtures/objects/alert-golden-1.cbor fixtures/objects/alert-golden-1.json docs/decisions/phase0a-time-ledger.json
git commit -m "feat: add deterministic alert payload codec"
```

## Task 2: Repair WU1B — Communal Author, Clock, and Canonical Willow Components

**Files:**

- Modify: `crates/riot-core/src/willow/mod.rs`
- Create: `crates/riot-core/src/willow/{clock,identity,entry,digest}.rs`
- Create: `crates/riot-core/tests/public_willow.rs`
- Create: `fixtures/willow/communal-author.json`, `fixtures/willow/communal-entry.bin`

- [x] **Step 1: Write public authority tests**

Prove full 32-byte communal namespace/subspace IDs; separate UTC Unix seconds and Willow TAI/J2000 microseconds from one clock snapshot; the exact four-component path; canonical entry and capability decode with no trailing bytes; author-subspace success and different-subspace denial; payload length/WILLIAM3 checks before schema decode; and no debug, serialization, or public accessor for the signing key.

Adopt the committed `public_willow` suite, then add the missing tests first. The denial fixture must create a second subspace under the same communal namespace; two independently generated communal authors are two namespaces and do not prove the area restriction. At this layer, test the fallible author factory directly: entropy failure returns `ENTROPY_UNAVAILABLE` and constructs no author. Test the clock/entry factory directly: `CLOCK_UNAVAILABLE` constructs no signed/allocated partial entry and covers pre-epoch and UTC/TAI range failure. Session/open and inspection retention assertions belong to Task 5 after those types exist. Run `cargo test -p riot-core --test public_willow`. Expected RED: current authority construction is infallible, no clock adapter exists, and the current denial fixture uses a different namespace.

- [ ] **Step 2: Implement Riot-owned wrappers**

Independent review reopening: production wrappers are not yet production-only. The injectable `EntropySource`/`ClockSource` APIs and `from_parts_for_tests` are present in the normal release Rust API. Gate them behind test/conformance-only compilation unavailable to `riot-ffi`, add non-injectable OS-entropy/system-clock production factories, zeroize temporary secret byte arrays, and add a release-feature/API regression check before marking this step complete.

```rust
pub struct ClockSnapshot {
    pub unix_seconds: u64,
    pub tai_j2000_micros: u64,
    pub uncertainty_seconds: u32,
}

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

Keep concrete Willow generics private. The signer owns zeroizing secret material and is neither `Clone` nor `Debug`. The author factory is fallible and accepts only an OS-randomness provider in production; deterministic/failing providers are `cfg(test)` or conformance-only. The clock/entry factory accepts a fallible production `ClockSource`; one system read plus pinned `hifitime` produces UTC and TAI/J2000 values. System/pre-epoch/range/conversion failure maps to `CLOCK_UNAVAILABLE`. It sets `created_at` and entry time from that snapshot and validates draft validity fields. Task 5 moves these factories behind `RiotSession` without changing the tested semantics. Discard the privilege-less communal namespace secret and create a zero-delegation capability for the author subspace.

- [x] **Step 3: Implement digest domains exactly**

Use corrected WILLIAM3 for `payload_digest`, SHA-256 over alert bytes for `object_digest`, and separate value/proof identities:

```text
entry_id = SHA256("riot/willow-entry-id/v1" || u32be(entry_bytes.len) || entry_bytes)
evidence_digest = SHA256(
  "riot/evidence-digest/v1" || u32be(entry_bytes.len) || entry_bytes ||
  u32be(capability_bytes.len) || capability_bytes || signature[64]
)
```

- [x] **Step 4: Verify and commit**

```bash
cargo fmt --check
cargo clippy -p riot-core --all-targets -- -D warnings
cargo test -p riot-core public_
cargo test -p riot-conformance william3_
```

Commit task files with `feat: add communal Willow authority`.

## Task 3: Reopen G1 — Complete the Bounded Riot Evidence Bundle

**Files:**

- Refactor: `crates/riot-core/src/import/mod.rs`
- Create: `crates/riot-core/src/import/bundle.rs`
- Modify: `crates/riot-core/tests/public_bundle.rs`
- Create: `fixtures/willow/bundle-golden-1.riot-evidence`
- Modify: `docs/decisions/phase0a-wu1-report.md`

- [ ] **Step 1: Write RED codec tests**

Independent review reopening: the suite still lacks a valid canonical bundle at exactly 8 MiB, invalid UTF-8 in the codec string, direct indefinite byte/text strings, and combined fatal-precedence cases. Add these as blocking tests. Nesting/node and exact authorization-boundary cases may move to WU4 only with a documented fixed-shape-parser unreachability proof; otherwise add them here.

Cover deterministic one-item bytes, 64-entry success, 65-entry rejection, 8 MiB rejection before decode, bad magic/version/codec, duplicate/unknown keys, indefinite containers, trailing bytes, non-64-byte signature, non-canonical entry/capability, payload length/digest mismatch, and invalid authorization. Signer trust/eligibility belongs to Task 5's store-backed preview.

Adopt the committed suite without overwriting concurrent changes. Run `cargo test -p riot-core --test public_bundle`; then add the missing cases before implementation. Expected RED must enumerate at least 64/65 entries, exact/one-over 8 MiB, canonical outer key order and shortest integers, duplicate/unknown keys, indefinite maps/arrays/strings, nesting/nodes, signature length 63/65, auth per-item/aggregate limits, non-canonical/trailing Entry and capability bytes, mixed valid/invalid sibling items, and sanitized structured diagnostics. The existing seven green tests alone do not close G1.

- [ ] **Step 2: Implement a two-stage bounded decoder**

Independent review reopening: canonical outer framing must be established before unsupported-codec and cumulative-limit decisions; all byte-bearing result types must remove or redact `Debug`; and the 4 KiB Entry, capability, and exact 64-byte signature ceilings must be separate and structurally frozen. The current 64 KiB shared Entry/auth limit does not satisfy the contract.

Enforce artifact size, outer counts, field lengths, authorization totals, and nesting/node limits before Willow decoding. Validate in this order:

```text
outer bounds/canonical CBOR → canonical Entry → canonical WriteCapability
→ 64-byte signature → payload length/corrected WILLIAM3
→ Meadowcap authorization → Riot alert schema
```

At this pure codec layer return `BundleDecodeOutcome = Decoded(DecodedBundle) | Rejected(BundleRejection)` and item `BundleDiagnostic { code, scope, component }` values; no store, session, trust, or `ImportPreview` type is referenced. Task 5 maps a decoded outcome plus destination store/context into `InspectBundleResult = Preview | Rejected`. Error strings never contain input bytes or payload text. Bundle framing/cumulative failures, including repeated canonical `entry_id`, reject globally. Freeze fatal precedence: size, magic, malformed/noncanonical outer frame, unsupported version/codec, cumulative limit in encounter order, duplicate entry ID. Once a bounded canonical item frame is isolated, component/signature/payload/authorization/schema failures stay on decoded item records without hiding valid siblings. Accept only communal namespace plus zero-delegation communal capability for the entry subspace; owned/delegated/alternate proofs are `UNSUPPORTED_CAPABILITY`. Use only Willow's checked authorisation conversion; unchecked conversion is forbidden.

Move `BundleItem::from_raw_parts` and `encode_bundle_raw` out of the release API. Hostile framing belongs in `riot-conformance` or `cfg(test)` so production callers cannot bypass encode-side validation.

- [x] **Step 3: Verify and commit**

```bash
cargo test -p riot-core --test public_bundle
cargo test -p riot-core public_
cargo clippy -p riot-core --all-targets -- -D warnings
```

Replace the provisional contradictions in the WU1 report with fresh commands/hashes and PASS or INCONCLUSIVE. Commit with `feat: repair public Willow evidence gate`. G1 is PASS only after Tasks 1–3 pass; a failure stops WU2.

## Task 4: WU2A — Namespace-Local Willow Join

**Files:**

- Create: `crates/riot-core/src/import/join.rs`
- Modify: `crates/riot-core/src/import/mod.rs`
- Create: `crates/riot-core/tests/core_import_join.rs`
- Create: `fixtures/imports/join-cases.json`

- [ ] **Step 1: Write join-law and edge-case tests**

Cover newer-prefix pruning, a candidate immediately dominated by a newer prefix, equal coordinate tie by greatest WILLIAM3 payload digest then greatest payload length, distinct subspace, distinct namespace, duplicate insertion, and every permutation of at least four interacting entries. Batch tests start from the same pre-state and require identical live state and dispositions keyed by canonical entry ID; only receipt row presentation order may follow input order.

For each permutation compare Riot's live-entry set with `willow25::storage::MemoryStore` alpha.3. Assert commutativity, associativity, and idempotence of the live view.

Run `cargo test -p riot-core --test core_import_join`. Expected RED: join types are absent.

- [ ] **Step 2: Implement namespace-local state**

Use a map keyed by full namespace ID. Within a namespace, compare only entries in the same subspace for prefix pruning. Validate the complete selection, partition it by namespace, compute one final join of pre-state plus the whole selected batch, and then derive effects from pre-state/final-state. Never derive receipts from sequential intermediate states. Stable `EntryId` is the domain-separated hash of canonical Entry bytes. Return:

```rust
pub enum JoinEffect {
    Winner { pruned_entry_ids: Vec<EntryId> },
    NotLive { dominating_entry_ids: Vec<EntryId> },
    AlreadyPresent,
}
```

For `Winner`, name only directly dominated entries removed from the pre-commit live view, never same-batch candidates. For `NotLive`, name dominators from the final live view. Map these internal effects to the fixed planned/receipt vocabulary. Cap references at 1,024 and reject state growth before mutation. Do not use `MemoryStore` in production state; it is a differential oracle only.

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

Cover `RiotSession.open` entropy failure with no handle; `CLOCK_UNAVAILABLE` during session encode with no partial entry; `CLOCK_UNAVAILABLE` during inspection before input/preview retention or store mutation; mapping pure `BundleDecodeOutcome::Rejected` to typed `InspectBundleResult::Rejected`; valid unknown-signer preview; hard-ineligible signature/capability/schema; rejection of repeated canonical entry IDs in one artifact; generation-bound planned effects; one live opaque plan, supersession, and the 64/65 issuance boundary; all-or-nothing selection; mixed new/duplicate; duplicate-only `NoChanges`; new dominated entry increments generation and gets a stable entry ID/first receipt; later pruning preserves historical provenance; stale preview precedes duplicate detection; host plan construction impossible; foreign plan/IDs; store full; injected pre-swap failure; rollback byte-for-byte equality; provenance separation of cryptographic facts from trust; and exact/one-over retained-store charges.

Run `cargo test -p riot-core core_import_`. Expected RED: store/session types are absent.

- [ ] **Step 2: Implement one arbiter and copy-on-write swap**

All session, store, preview, signer, generation, index, and receipt state lives behind one `Arc<Mutex<SessionState>>`. Child handles contain only IDs and the arbiter. Build a bounded next snapshot, then perform one pointer swap. No observable state changes before that swap.

```rust
pub enum PlannedEffect {
    WouldApply { entry_id: EntryId, pruned_entry_ids: Vec<EntryId> },
    WouldBeDominated { entry_id: EntryId, dominating_entry_ids: Vec<EntryId> },
    AlreadyPresent { entry_id: EntryId, insertion_receipt_id: u64 },
}

pub enum EntryDisposition {
    AppliedAtCommit { entry_id: EntryId, pruned_entry_ids: Vec<EntryId> },
    DominatedAtCommit { entry_id: EntryId, dominating_entry_ids: Vec<EntryId> },
    AlreadyPresent { entry_id: EntryId, insertion_receipt_id: u64 },
}

pub enum ImportCommitResult {
    Committed(ImportReceipt),
    NoChanges(DuplicateResult),
}
```

`ImportPreview.plan(selection)` retains the full plan in the arbiter and returns an opaque `ImportPlan` handle. One plan may be live and at most 64 may be issued per preview; each terminal plan leaves a fixed 256-byte-charged tombstone until preview close, bounding exact terminal reasons to 16 KiB. A new plan supersedes the prior handle. `ImportPlan.commit()` reads retained state only and must reproduce the exact effects or return `STALE_PREVIEW`; host code cannot construct or substitute plan fields. The seen index retains bounded immutable bytes/facts and the first receipt for every accepted entry, including dominated/pruned history. Provenance separates immutable receipt disposition from current `Live | NotLive { DominatedOnArrival | PrunedLater }` status.

Charge a hard 16 MiB retained-store budget across buffer capacities and fixed conservative charges: 512 bytes per entry/index record, 256 per namespace, 256 per receipt and row, and 32 per digest reference. Check the full next-snapshot charge before allocation or mutation; freeze the 64-namespace and 1,024-reference ceilings in `fixtures/manifest.json`.

- [ ] **Step 3: Prove lifecycle linearization**

Race commit/reject, close/commit, plan supersession/commit, and session-close/child-action. Assert one terminal winner, no deadlock within two seconds, and no second receipt/swap. Test exact admission: owning-session failed, owning-session closed, immutable foreign owner ID (`WRONG_SESSION` without locking its arbiter), local child closed, superseded/committed plan, consumed preview, stale preview, validation. Constructor panic returns `INTERNAL_ERROR` without a handle; normal parent close yields `OBJECT_CLOSED`; close remains idempotently available after session failure.

Freeze this plan matrix: superseded handle → `PLAN_SUPERSEDED`; repeated winning commit → `PLAN_CONSUMED`; active plan after preview rejection → `PREVIEW_CONSUMED`; explicit plan close → `OBJECT_CLOSED` and the preview may create a replacement; store/session close → `OBJECT_CLOSED`; session panic → `SESSION_FAILED` before child terminal state.

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

- Modify: `Cargo.toml`, `Cargo.lock`
- Modify: `fixtures/manifest.json`
- Modify: `crates/riot-ffi/Cargo.toml`, `crates/riot-ffi/src/lib.rs`
- Create: `crates/riot-ffi/build.rs`, `crates/riot-ffi/tests/ffi_contract.rs`
- Modify: `crates/xtask/Cargo.toml`, `crates/xtask/src/main.rs`
- Create: `scripts/phase0a/generate-bindings.sh`

- [ ] **Step 1: Write RED boundary tests**

Test the design surface: open, identity, create store, encode alert, create bundle, typed `InspectBundleResult::Preview | Rejected`, plan selection, commit/reject, provenance for live/dominated/later-pruned entries, close/idempotent close, wrong session/plan, stale/consumed preview, structured sanitized diagnostics, and panic quarantine. No private key or Willow generic type may be exported.

- [ ] **Step 2: Implement synchronous handle wrappers**

Every exported entrypoint catches unwind from the unwind-capable evidence profile, maps internal errors to stable codes, and delegates admission/mutation to the core arbiter. No unwind crosses UniFFI. Swift/Kotlin records contain full IDs and byte arrays; never truncate identifiers.

- [ ] **Step 3: Own the frozen binding generator**

Pin `camino = "=1.2.4"` in workspace dependencies. Add these xtask dependencies so binding generation uses the locked library API, not an ambient Cargo subcommand or globally installed executable:

```toml
[dependencies]
uniffi = { workspace = true, features = ["bindgen"] }
camino = { workspace = true }
```

`cargo xtask generate-bindings` first builds the host `riot-ffi` cdylib, then calls `uniffi::generate(GenerateOptions { languages: [Swift, Kotlin], source: host_cdylib, ... })` into a clean run-specific directory. Test the command from a PATH with no `uniffi-bindgen` executable.

- [ ] **Step 4: Generate and compile bindings**

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
- Create: `apps/ios/RiotEvidence/RiotEvidence/{AppDelegate.swift,Info.plist}`
- Create: `apps/ios/RiotEvidence/RiotEvidenceTests/{BindingSemantics,IOSCreatesBundle,IOSImportsAndroidBundle}.swift`
- Create: `apps/ios/RiotEvidence/RiotEvidenceTests/BoundedFileRead.swift`
- Modify: `apps/android/build.gradle.kts`, `apps/android/app/build.gradle.kts`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/{BindingSemanticsTest,CrossRuntimeHandoffTest}.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/BoundedFileRead.kt`
- Create: generated `apps/android/**/gradle.lockfile`
- Modify: `fixtures/manifest.json`
- Create: `fixtures/gradle-locks.sha256`
- Modify: `crates/xtask/src/main.rs`
- Create: `scripts/phase0a/{package-ios,package-android,cross-runtime-handoff}.sh`

- [ ] **Step 1: Make native tests fail on missing packaged bindings**

Create a minimal UIKit XCTest host application target with bundle ID `org.riot.evidence`, one `AppDelegate`, no UI flow, and a test-host relationship that installs an app data container. The iOS and Android semantic suites call generated bindings, never reimplement CBOR/Willow. Expected RED is a missing native package/library or generated binding symbol, not a missing host container.

- [ ] **Step 2: Package each release-shaped native library**

Copy only required release libraries, bindings, headers/module maps, and notices. Android packages `arm64-v8a` for runtime and `x86_64` as compile-only evidence. iOS packages simulator runtime plus unsigned-device compile evidence.

- [ ] **Step 3: Freeze Gradle dependency resolution**

Enable `dependencyLocking { lockAllConfigurations() }`, then run:

```bash
cd apps/android
export JAVA_HOME="$(/opt/homebrew/bin/brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
export PATH="$JAVA_HOME/bin:$PATH"
java -version 2>&1 | rg '17\.0\.19'
./gradlew :app:dependencies --write-locks
rg --files -g '**/gradle.lockfile' | LC_ALL=C sort | xargs shasum -a 256 > ../../fixtures/gradle-locks.sha256
shasum -a 256 ../../fixtures/gradle-locks.sha256
```

Commit every generated lockfile and the sorted per-file digest manifest, replace the pending `gradle_locks_sha256` with the digest-manifest hash in `fixtures/manifest.json`, and make `cargo xtask validate-contracts` recompute both layers. A changed or missing lock fails G3.

- [ ] **Step 4: Prove capped reads, hostile diagnostics, and result vocabulary natively**

Swift and Kotlin file helpers read at most 8 MiB + 1 byte without trusting file metadata; exact-limit input reaches Rust and one-over input is rejected before FFI with no retained bytes. Both native suites feed a malformed bundle containing a unique hostile marker, decode and assert `InspectBundleResult::Preview | Rejected`, `WouldApply`/`AppliedAtCommit`, `WouldBeDominated`/`DominatedAtCommit`, `AlreadyPresent`/`NoChanges`, a later transition from receipt `AppliedAtCommit` to current status `NotLive { PrunedLater }`, and one item-scoped canonical diagnostic. Emit separately named `ios_binding_semantics` and `android_binding_semantics` sections under facts schema `org.riot.handoff-facts/1`.

The handoff script clears logs before each suite, captures XCTest/process output, `xcrun simctl spawn "$IOS_UDID" log show`, Gradle/instrumentation output, and `adb logcat -d`, then asserts the hostile marker, payload, and secret sentinels occur nowhere in captured logs.

- [ ] **Step 5: Implement opaque-byte handoff**

Follow the design's eight-step protocol exactly. The shell hashes and transports `.riot-evidence` bytes but never decodes them. Versioned producer/consumer facts have preview, plan, commit, and post-commit provenance sections and agree on full IDs, timestamps, corrected WILLIAM3, `entry_id`, `evidence_digest`, bundle/object digests, byte count, signature/capability validity, `UnknownTrust`, `WouldApply`, `AppliedAtCommit`, and current `Live`. The two producer authors and objects differ.

At script startup, resolve the pinned Homebrew JDK 17 path as above, export it for every Gradle invocation, and fail unless `java -version` contains `17.0.19`; the host's default JDK is never evidence identity.

- [ ] **Step 6: Run G3 and commit**

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
EVIDENCE_RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)" scripts/phase0a/verify.sh
git status --short
```

Expected: the script creates a new `build/evidence/$EVIDENCE_RUN_ID` without deleting prior evidence. G0–G3 each say PASS with commands, environment, evidence paths, hashes, elapsed aggregate agent-hours, and next action. If required evidence cannot run, record INCONCLUSIVE and stop claiming GO.

- [ ] **Step 4: Update status honestly**

Replace README's stale “Planning only” text with the achieved evidence boundary. Do not claim field readiness, production security, Drop interoperability, private groups, persistent storage, or physical-device proof.

- [ ] **Step 5: Review and finish**

Use `superpowers:requesting-code-review`, address findings with `superpowers:receiving-code-review`, rerun `superpowers:verification-before-completion`, then use `superpowers:finishing-a-development-branch` to choose merge/PR/cleanup.

## Execution Order and Parallelism

```text
WU0R → alert codec → communal authority → bundle → join → transaction
→ FFI → native handoff → adversarial report
```

The reopened G0/G1 repair tasks run sequentially so each task can atomically update and commit the authoritative time ledger. After G2, native project scaffolding may run in parallel only with report-template preparation; they use disjoint files, and report templates contain no claimed results until fresh commands run.

Do not parallelize edits to `Cargo.toml`, `Cargo.lock`, `fixtures/manifest.json`, `crates/riot-core/src/lib.rs`, or module indexes without explicit ownership. Never commit another worker's unstaged changes.

## Rebaselined Remaining Hours

| Charge | Hours |
| --- | ---: |
| WU0 spent | 1.0 |
| combined WU0R+WU1 baseline spent | 2.0 |
| reopened G0/G1 repair reserved | 1.5 |
| WU2 reserved | 4.0 |
| WU3 reserved | 4.0 |
| integration contingency reserved | 1.5 |
| WU4 reserved | 2.0 |
| total | 16.0 |

The combined baseline duration is charged once. Repair workers record new active time separately in `docs/decisions/phase0a-time-ledger.json`; the prior charge does not prepay repairs. WU4 still starts at total hour 14.

## Compatibility and Rollback

Phase 0A has no production users or persisted store migration. Preserve committed evidence and make changes in task commits; rollback means reverting one reviewed task commit, never resetting the shared worktree. Do not downgrade to Willow 0.5.0: a G0 failure records REVISE/INCONCLUSIVE. If a repair changes outer `.riot-evidence` bytes rather than only validation/results, increment the development codec version, regenerate its golden fixture once with old/new hashes in the report, and reject the prior version explicitly. Canonical Willow component bytes and corrected WILLIAM3 vectors never receive silent compatibility shims.

## Stop Rules

- G0 failure stops all Willow implementation.
- G1 failure stops WU2.
- G2 failure stops native expansion.
- WU4 begins at aggregate hour 14 even if earlier work is incomplete.
- At 16 aggregate agent-hours, unfinished required evidence is INCONCLUSIVE, never stretch scope.
