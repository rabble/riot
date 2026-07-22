# Meadowcap Slice 2: Governance Ledger, Deterministic Evaluator, Transitive Revocation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `riot-core::governance` module named in the design's "Governance ledger", "Leases and revocation", and "Durable authority repository" sections: the versioned `GovernanceRecordV1` schema (canonical CBOR envelope + `record_id` + exact per-kind path binding for every V1 kind), the actor/device/action hash chains, a durable `AuthorityRepository` (governance journal + type/target/lineage/revocation/action-head indexes) that survives restart, a **deterministic** policy evaluator that reduces a governance frontier to an immutable `PolicySnapshot` with no wall-clock reads, and transitive capability revocation with action-chain cutoffs keyed on the Slice-1 capability fingerprint. Issuance/renewal records embed and re-verify both the parent and child capability canonical bytes, so governance never trusts a body-claimed fingerprint. It consumes Slice 1's capability core (fingerprints, codec, verification) and produces typed governance facts and stable rejection codes. It contains **no** admission-engine rewrite, no protected sync, no vault/recovery, no manifest-V2/app-broker enforcement, no FFI/UniFFI surface, and no UI.

**Architecture:** New crate module `crates/riot-core/src/governance/` with submodules `mod` (errors, `RecordKind`, ceilings, re-exports, `#[cfg]`-gated `test_support`), `body` (per-kind typed bodies), `record` (CBOR envelope + `record_id`), `paths` (exact per-kind path templates + path↔kind↔target binding), `actor` (actor/device binding + per-actor sequence chains), `frontier` (frontier set + frontier hash + topological reduction), `action` (`ActionReceiptV1` + action hash chains + entry/receipt pairing), `authorize` (attenuation-proof capability-issuance verification, record authorization, migration-fork selection), `lineage` (capability lineage forest keyed by fingerprint), `revoke` (transitive revocation + cutoff-map predicate), `evaluator` (frontier → `PolicySnapshot`, pure/deterministic), and `repository` (durable `AuthorityRepository`, Memory/Sqlite dual mirroring the landed `store::evidence::EvidenceRepository`). It builds **on top of** Slice 1's `meadowcap` module (reuses `write_capability_fingerprint`, `decode_write_capability_bounded`, `delegate_write`, `MeadowcapError`) and the landed durable store (`RiotDatabase::write_transaction`, the migration ladder in `store::schema`). It does **not** touch `session.rs` admission, `crates/riot-ffi/`, or the willow25 capability primitives.

**Tech Stack:** Rust 2021, `willow25 = "=0.6.0-alpha.3"` (alpha pin load-bearing; see below), `minicbor = "=2.2.2"` (already a `riot-core` dependency — the canonical CBOR codec used by every existing record family: `newswire/model.rs`, `site/manifest.rs`, `apps/manifest.rs`), `sha2` (already present; `record_id` + fingerprints + action-hash chains), `rusqlite` (already present, behind the default `sqlite` feature; the durable repository). **No new crate dependencies are introduced by this slice**, so `fixtures/manifest.json`'s `cargo_lock_sha256` does not change. Golden fixtures live in a new `fixtures/governance/governance-vectors.json`, contract-pinned by a new `governance_vectors_sha256`, mirroring `meadowcap_vectors_sha256`.

---

## Load-bearing project constraints (read before coding)

1. **Alpha pins are load-bearing.** `willow25 = "=0.6.0-alpha.3"` and `bab_rs = "=0.8.1"` are pinned because stable releases compute incorrect WILLIAM3 digests (`docs/research/2026-07-10-willow-implementation-audit.md`). `cargo xtask validate-contracts` fails if the resolved version drifts (`crates/xtask/src/main.rs`). Do not bump either crate.
2. **No new dependencies.** Everything this slice needs (`minicbor`, `sha2`, `rusqlite`, `willow25`, the Slice-1 `meadowcap` module, `serde_json` in dev/`conformance` only) is already resolved in `Cargo.lock`. Because no dep edge is added, `cargo_lock_sha256` in `fixtures/manifest.json` is unchanged. **If a task ever adds a dependency, it must also run `cargo xtask validate-contracts`, read the printed `actual` hash, and update `cargo_lock_sha256`.**
3. **Never name the `meadowcap` crate.** `meadowcap` is a transitive-only dependency re-exported through `willow25`. Naming it directly is `E0433` **and** would add a `meadowcap` dep edge that changes `Cargo.lock` and breaks the `cargo_lock_sha256` pin. Reach capability fingerprints/codec **only** through `crate::meadowcap` (Slice 1) and `willow25::authorisation::*`.
4. **Willow entry/area timestamps are TAI/J2000 MICROSECONDS, not Unix seconds.** Every `TimeRange`, entry timestamp, display-time comparison, and future-clock quarantine bound is TAI/J2000 microseconds; convert wall-clock inputs via `crate::willow::tai_j2000_micros_from_unix_seconds`. Riot shipped and fixed exactly this unit bug (#72 → #73); **Task 12** pins the future-clock quarantine in the production unit.
5. **The evaluator is DETERMINISTIC — this is a hard security property, not a nicety.** No `SystemTime::now()`, no `std::time`, no RNG, no `HashMap` iteration order dependence inside evaluation. Wall-clock time enters **only** as an explicit `now: Option<u64>` argument. Identical journal + identical clock argument ⇒ byte-identical `PolicySnapshot` and identical frontier hash, on every platform. Pinned by a property test (**Task 10**) and a cutoff-classification determinism assertion over shuffled arrival orders (**Task 11**). Use `BTreeMap`/`BTreeSet` and sorted iteration everywhere a set or map feeds a hash or a decision.
6. **Governance evaluation is DURABLE-ONLY; tests need a durable profile.** The in-memory Willow join drops the capability + signature (`willow/join.rs`), so a full signed governance record with its authorizing capability survives only in durable SQLite. The `AuthorityRepository::Memory` variant is a conformance oracle for the *pure* evaluator over already-parsed records; every **restart / rebuild / rollback** test opens a real temp-file `RiotDatabase` (the established `RiotDatabase::open(std::env::temp_dir().join(...), DatabaseConfig::default())` pattern from `tests/newswire_import.rs` and `tests/sqlite_backup_restore.rs`).
7. **Reuse the canonical gate.** Capability fingerprints come **only** from `crate::meadowcap::fingerprint::write_capability_fingerprint`; chain-signature verification and attenuation come **only** from `crate::meadowcap::{decode_write_capability_bounded, delegate::delegate_write}`. Do not hand-roll a second fingerprint preimage or a second signature path — that is a recurring defect class in this repo (#76, #90). Governance issuance verification (**Task 8**) is a *policy* check layered on top of those primitives, never a re-implementation of them.
8. **Shared checkout — pathspec commits only.** Multiple agents share this working tree. Every commit is `git add <explicit paths>` — never `git add -A`, never `git add .`. Never `--no-verify`. Never `git stash` (the stash stack is global to the checkout; use a throwaway WIP commit).
9. **Canonical CBOR discipline.** Follow the existing frozen-record pattern (`site/manifest.rs`, `newswire/model.rs`): definite lengths, strictly-ordered integer map keys, deterministic encode, byte-identical decode (`prove_canonical`-style re-encode check), closed failure vocabulary, unknown map keys / unknown kind tags **fail closed**. 16 KiB per-record ceiling applied **before** allocation/recursive work (design resource ceilings, line ~1082).

---

## Scope: what Slice 2 builds, and the exact boundary to later slices

**Records-vs-rows decision (design-cited): (b) — governance records are persisted as rows in a dedicated durable `AuthorityRepository` (a governance journal table plus derived index tables), NOT admitted as Willow record families through `session.rs` in this slice. The FFI classification sites are therefore UNTOUCHED in Slice 2.**

Design evidence for (b):

- The design describes governance persistence as a **dedicated indexed repository distinct from generic content**: *"One transaction persists the accepted Willow entries, governance journal, accepted frontier, actor/receiver bindings, capability-lineage and revocation indexes, app grants, audit classification... Governance projections are indexed by record type and target; no app bridge call scans the unbounded journal."* (Durable authority repository, lines ~330, ~343). The "governance journal + capability-lineage index + revocation index" is a distinct table set from `accepted_entries`, which is exactly what a dedicated repository is.
- The **wire form** of a governance record *is* an ordinary signed Willow entry carrying a CBOR body (Governance ledger, line ~442). That is an option-(a) fact about the transport. But *routing that Willow entry through the shared inspect/plan/commit admission boundary* — so it lands in `accepted_entries` and, in one transaction, updates the governance journal — is the **shared admission engine**, which the design and master plan assign to **Slice 3** (*"Shared contextual admission for local writes, imports, and synchronized entries, including legacy compatibility"*, slice list line ~1316; Dependencies: *"the capability core and shared admission engine precede... governance... work"*, line ~1339). Building the FFI import-classification wiring before that engine exists is out of Slice 2's scope.
- The master plan's Phase 2 exit criteria make the FFI obligation explicitly **conditional**: *"if governance records are stored as new Willow record families, they are registered at every FFI classification site with a riot-ffi classification test"* (`2026-07-22-willow-gap-master-plan.md` line 52). Slice 2 does **not** store them as Willow record families crossing the FFI import boundary, so the condition does not fire here. The cross-cutting rule (master plan line 103) attributes governance/manifest record-family classification to the later phases that actually create and import governance entries on-device.

**Consequence, stated so it is not silently dropped:** the slice that first routes a governance record through `session.rs` admission as a Willow record family (Slice 3 at the earliest; realistically Slice 5 when management creation/import produces them on-device) **inherits** the obligation to hand-register the `governance/v1/` family at every non-compiler-forced FFI classification site — `crates/riot-ffi/src/mobile_state.rs` `inspectable_entries` and `list_current_entries`, plus the `store.rs`/`evidence.rs` prefix scan — with a riot-ffi test that imports a bundle containing a governance entry and asserts it is classified, not rejected. Slice 2 leaves all three sites untouched and records this hand-off in the self-review checklist.

**In Slice 2 scope:**

- `GovernanceRecordV1` canonical CBOR envelope + domain-separated `record_id`.
- The complete V1 `RecordKind` tag space (22 kinds) frozen; unknown tags fail closed. Exact path template + path↔body target binding for **every** kind (design table lines ~456–480), with missing/extra/wrong-target ⇒ ineligible.
- Per-kind typed bodies. **Authority-bearing kinds** (`Genesis`, `ActorBinding`, `RoleDecision`, `CapabilityIssued`, `CapabilityRenewed`, `CapabilityRevoked`, `ActionReceipt`, `Checkpoint`, `Proposal`) get fully-typed bodies wired into the evaluator. `CapabilityIssued`/`CapabilityRenewed` additionally **embed the canonical bytes of the presented parent capability and the delegated child capability** so issuance is cryptographically attenuation-verified (Task 8), not fingerprint-trusted. **Deferred-semantics kinds** (`InviteManagerDecision`, `InviteResponse`, `InviteActivation`, `MemberDecision`, `AppApproved`, `AppRevoked`, `AppProvisioned`, `AppealSubmitted`, `AppealResolved`, `DirectoryWithdrawn`, `RecoveryDeclared`, `MigrationDeclared`, `LensSuccessor`) get their envelope + exact path + a **frozen canonical body** whose later-slice-specific payloads (an `AppManifestV2` permission set — Slice 6; an HPKE invite envelope — Slice 5; an MLS/vault reference — Slice 5) are carried as opaque length-bounded canonical byte fields. This freezes the wire schema and satisfies the design's *"every closed GovernanceRecordV1 kind has exact path/body/authority golden and missing/extra/wrong-target negative fixtures"* (line ~1130) without importing later-slice algebra; the **semantic enforcement** of those opaque bodies is explicitly deferred to the owning slice.
- Actor/device binding chains; `ActionReceiptV1` chains (entry↔receipt one-to-one pairing, base-case exemption, missing-pair, self/receipt-of-receipt/swapped/tampered rejection).
- Durable `AuthorityRepository` (Memory/Sqlite dual): governance journal + **populated** indexes (by record type & target, capability-lineage, revocation, actor/action-head); indexed read paths that never scan the journal; ingest of a verified record; load/rebuild; restart survival; rollback detection; fail-closed startup.
- Governance frontier + frontier hash + topological DAG reduction (parents-before-children; missing parents pending; display timestamps never order).
- Deterministic evaluator: `(journal, now) → PolicySnapshot` identified by frontier hash; active-capability set; restrictive concurrent reducers (revoke-wins-over-grant, concurrent restrictions intersect); no wall-clock reads.
- Transitive revocation (fingerprint join key); action-chain cutoff-map predicate; arrival-order determinism; read caps in the revoked set close immediately.
- Future-clock quarantine + clock-block, with time as an explicit input so evaluation stays deterministic.
- Golden conformance fixtures for every kind + negative forms, contract-pinned, with the xtask self-test scaffold updated (Task 16).

**16 KiB ceiling vs. embedded capabilities (resolution).** `CapabilityIssued`/`CapabilityRenewed` bodies embed two write-capability canonical encodings. A realistic governance issuance capability is shallow: genesis (~100 B) plus ≤ 16 delegations at ~120–180 B each (Slice-1 `MAX_DELEGATION_DEPTH = 16`), so a depth-16 capability is ~2–3 KiB and two of them ~5–6 KiB — comfortably under the design's 16 KiB per-record ceiling. The Slice-1 64 KiB *per-capability* ceiling still bounds a pathological single capability, and a governance issuance record whose two embedded caps exceed 16 KiB is rejected `RecordTooLarge` (fail-closed — a chain that deep is not a legitimate governance issuance). No ceiling change; Task 1 pins a "realistic depth-16 issuance fits under 16 KiB" test so the assumption is guarded.

**Explicitly OUT of scope (later slices):**

- Shared admission inspect/plan/commit rewrite, the one-transaction coupling of `accepted_entries` + governance journal, legacy-compat admission, and **all FFI classification/registration** → **Slice 3** (and the creating slice; see the hand-off above).
- Protected-sync handshake / PIO / relative confidential capability encoding / `ProtectedDropV1` → **Slice 4**.
- Open/Managed Space **creation**, invitation **state machine execution**, HPKE invite envelopes, MLS, secure-vault adapters, recovery-envelope crypto, and namespace-migration **ceremony execution** → **Slice 5**. (Slice 2 freezes the `Invite*`/`RecoveryDeclared`/`MigrationDeclared`/`LensSuccessor` record envelopes + paths only; `selected_migration` here only classifies a fork, it never runs a migration.)
- `AppManifestV2`, permission-subset algebra, approval/provisioning **enforcement**, directory role authority, opaque app-execution sessions → **Slice 6**. (Slice 2 freezes the `App*`/`DirectoryWithdrawn` record envelopes + paths only.)
- Role-template path/mode **bundles** (the "Riot authorization path profile v1" table, lines ~378–392) and their golden tests (line ~1129) → **Slice 5**. Slice 2 freezes the `governance/v1/...` record paths (lines ~456–480), distinct from the write-capability areas a role grants.
- **Checkpoint compaction** (design testing line ~1163). Slice 2 freezes the `Checkpoint` record envelope + path + `MAX_PARENTS` bound and its authorization, but the compaction *algorithm* (merging a frontier approaching the 16-parent limit, pruning superseded records while preserving audit) is deferred to **Slice 3** admission, where the one-transaction `accepted_entries`+journal coupling that compaction rewrites lives. Task 15 asserts this deferral with the citation rather than testing compaction.
- Native management/consent/audit UI and UniFFI lifecycle objects → **Slices 5/7**.

This is a large slice, decomposed into independently-committable TDD work units ordered so every task compiles and commits green at its own boundary.

---

## API / schema inventory

### Slice-1 `meadowcap` surface reused (do not reinvent)

Read from the landed module at `crates/riot-core/src/meadowcap/` on this branch:

- `crate::meadowcap::fingerprint::{CapabilityFingerprint (= [u8;32]), write_capability_fingerprint(&WriteCapability)}` — the exact join key. Preimage is `SHA-256("riot/meadowcap-fingerprint/v1" || canonical_capability_bytes)`, **no length prefix**. The governance `record_id` uses a **different** domain string so the two hash spaces never collide.
- `crate::meadowcap::codec::{decode_write_capability_bounded}` and `crate::willow::encode_capability` — bounded canonical codec (depth 16 / 64 KiB) with full chain-signature verification during decode. **This is how Task 8 cryptographically validates an embedded capability.**
- `crate::meadowcap::delegate::delegate_write(&parent, &signer, new_area, new_receiver) -> Result<WriteCapability, MeadowcapError>` — attenuation-only delegation; used by `test_support` to build genuine parent→child capability pairs and lineage forests.
- `crate::meadowcap::create::new_owned_write(&NamespaceSecret, SubspaceId) -> WriteCapability` — the owned root; `willow25::prelude::{NamespaceSecret, SubspaceSecret}` with `from_bytes(&[u8;32])`, `corresponding_namespace_id()`, `corresponding_subspace_id()`.
- Capability inspection: `cap.granted_namespace()`, `cap.granted_area()`, `cap.includes_area(&Area)`, `cap.delegations() -> &[Delegation]`, `cap.receiver()`, `cap.genesis()` with `namespace_key()`/`user_key()`/`access_mode()`.
- `crate::meadowcap::MeadowcapError` — Slice-1 stable codes. Governance defines its **own** `GovernanceError`; capability-layer failures surface through `GovernanceError::Capability(MeadowcapError)` (produced by Task 8).
- `crate::willow::{tai_j2000_micros_from_unix_seconds, Path}`; the willow `Path` used across `store::evidence`/`newswire` exposes `Path::from_slices(&[&[u8]])` and a `components()` iterator.

### Durable store surface reused (do not reinvent)

Read from `crates/riot-core/src/store/`:

- `store::database::RiotDatabase` — cheaply `Clone`-able handle; `pub(crate) fn write_transaction<T, F>(&self, WriteEstimate, F)` where `F: FnOnce(&rusqlite::Transaction) -> Result<T, DatabaseError>` runs an `IMMEDIATE` transaction and commits; `pub(crate) fn read_connection<T, F>(&self, F)`; `pub fn schema_version()`, `pub fn generation()`, `pub fn authority_quarantined()`. `WriteEstimate::new(payload_bytes, page_headroom)` and `map_sqlite_error` are `pub(crate)` in the private `database` submodule — **Task 13 adds `#[cfg(feature="sqlite")] pub(crate) use database::{map_sqlite_error, WriteEstimate};` to `store/mod.rs`** so the sibling `governance` module can name them. `store::DatabaseError` is already `pub`.
- `store::schema` — the migration ladder: `CURRENT_SCHEMA_VERSION` (currently **2**), `MIGRATION_ONE`/`MIGRATION_TWO`, `migrate()` (applies `found < N` steps), `validate_structure()` (per-table `validate_schema_definition` exact-DDL match + `PRAGMA user_version == found`; evidence tables gated by `found >= 2` via `validate_evidence_structure`). Task 13 adds `MIGRATION_THREE`, bumps to **3**, and gates `validate_governance_structure` by `found >= 3`.
- `store::evidence::EvidenceRepository { Memory(MemoryEvidenceStore), #[cfg(feature="sqlite")] Sqlite(SqliteEvidenceStore) }` with `load()`/`persist()` — the **exact pattern** the `AuthorityRepository` mirrors, including the `#[cfg(feature="sqlite")]` gate on the Sqlite variant so a `--no-default-features` wasm build compiles with the Memory oracle only.
- `CURRENT_SCHEMA_VERSION` is **not** contract-pinned in `fixtures/manifest.json`, so bumping it to 3 does not touch `manifest.json`; the reopen tests (`tests/sqlite_backup_restore.rs`, `tests/newswire_import.rs`) exercise the v2→v3 migration and must stay green.

### New governance schema (Slice 2 owns)

`GovernanceRecordV1` canonical CBOR envelope (strictly-ordered integer keys, definite lengths):

| Key | Field | Type | Notes |
| --- | --- | --- | --- |
| 0 | `schema` | text | frozen tag `"org.riot.governance.record/1"` |
| 1 | `kind` | uint | `RecordKind` tag (0..=21); unknown ⇒ fail closed |
| 2 | `namespace` | bytes(32) | Space namespace id |
| 3 | `parents` | array of bytes(32) | **sorted, deduplicated** parent `record_id`s; ≤ 16 |
| 4 | `actor_id` | bytes(32) | stable actor identity |
| 5 | `receiver` | bytes(32) | actual Meadowcap receiver key (distinct from `actor_id`) |
| 6 | `sequence_be` | uint | strictly-increasing per-actor sequence |
| 7 | `prev_actor_record` | bytes(32) or null | previous record_id in this actor's chain |
| 8 | `authorizing_fingerprint` | bytes(32) | Slice-1 fingerprint of the write authority |
| 9 | `body` | map | kind-specific typed body (see `body.rs`) |
| 10 | `created_display_micros` | uint | **display-only** TAI/J2000 µs; never orders governance |

`record_id = SHA-256("riot/governance-record-id/v1" || canonical_record_bytes)`. Exact per-kind path templates (lines ~456–480) are reproduced verbatim in `paths.rs`. `ActionReceiptV1` (its own frozen schema tag): `entry_id`, `capability_fingerprint`, `actor_id`, `receiver`, `actor_sequence`, `previous_action_hash` (or null), `policy_frontier_hash`; path `governance/v1/actions/<actor_id>/<receiver_id>/<sequence_be>`.

### Identified gaps (Slice 2 fills, no upstream change)

1. **No governance schema upstream** — entirely Riot-side, new module.
2. **No deterministic policy layer** — new, pure, in `evaluator.rs`/`revoke.rs`.
3. **No governance persistence** — new `MIGRATION_THREE` + `repository.rs`.
4. **No issuance attenuation proof** — willow25 validates a single capability chain on decode, but governance must additionally prove the child is a *descendant* of the named parent (Task 8); new `authorize.rs`.

**No blocking upstream gaps.** Every requirement is satisfiable against `willow25 0.6.0-alpha.3` + the landed store + Slice-1 `meadowcap`.

---

## File Structure

Created:

| Path | Responsibility |
| --- | --- |
| `crates/riot-core/src/governance/mod.rs` | Module root: `GovernanceError`, `RecordKind` (22 tags), `RecordId`/`Fingerprint` aliases, ceilings, submodule decls, `#[cfg(any(test, feature="conformance"))] pub mod test_support`. |
| `crates/riot-core/src/governance/body.rs` | Per-kind typed `Body` enum + `OpaqueBytes` + `Cutoff` + `kind_of` + `target_id_of` + per-body canonical CBOR (`encode_body`/`decode_body`). Issuance bodies embed parent+child capability bytes. |
| `crates/riot-core/src/governance/record.rs` | `GovernanceRecordV1` (typed `body: Body`), canonical codec (definite lengths, ordered keys, re-encode canonicity check, trailing-byte reject, 16 KiB ceiling, sorted/dedup parents), `record_id`. |
| `crates/riot-core/src/governance/paths.rs` | Exact per-kind path templates + `path_for` + `verify_path_binding`. |
| `crates/riot-core/src/governance/actor.rs` | Actor/device binding facts + per-actor sequence/`prev_actor_record` chain validation. |
| `crates/riot-core/src/governance/frontier.rs` | `Frontier` + `frontier_hash` + `topological_reduce` (missing-parent ⇒ pending). |
| `crates/riot-core/src/governance/action.rs` | `ActionReceiptV1` codec + `action_hash` + `validate_action_chain` (missing-pair, base-case, self/receipt-of-receipt/swapped/tampered) + `pub` (conformance-gated) seeded receipt builders. |
| `crates/riot-core/src/governance/authorize.rs` | `verify_capability_issuance` (attenuation proof — the `Capability(MeadowcapError)` producer), `authorize_record` (`SelfAuthorization` producer), `selected_migration` (fork classifier). |
| `crates/riot-core/src/governance/lineage.rs` | Capability-lineage forest keyed by fingerprint; `descendants_of`. |
| `crates/riot-core/src/governance/revoke.rs` | `apply_revocations` (transitive, revoke-wins) + cutoff-map `is_action_active`. |
| `crates/riot-core/src/governance/evaluator.rs` | `PolicySnapshot` + `evaluate(records, now: Option<u64>)`; pure, deterministic, `BTree*`; grant fold routes through `verify_capability_issuance`; restrictive intersect reducer; future-clock quarantine. |
| `crates/riot-core/src/governance/repository.rs` | `AuthorityRepository { Memory, #[cfg(sqlite)] Sqlite }`: `ingest` (journal + all five index tables, one transaction), indexed read paths (`records_for_target`/`revocations_for`/`action_head_for`), `load_journal`, `snapshot`. Errors are non-gated `GovernanceError`. |
| `crates/riot-core/tests/governance_conformance.rs` | Golden per-kind vectors, negative forms, self-authorization/reducer/migration-fork conformance; writes/asserts `fixtures/governance/governance-vectors.json`. |
| `crates/riot-core/tests/governance_durable.rs` | Restart-survival, rebuild, rollback detection, fail-closed startup over a temp-file `RiotDatabase`. |
| `fixtures/governance/governance-vectors.json` | Golden canonical record encodings + `record_id`s + one action receipt. Contract-pinned by `governance_vectors_sha256`. |

Modified:

| Path | Change |
| --- | --- |
| `crates/riot-core/src/lib.rs` | Add `pub mod governance;` after `pub mod meadowcap;`. |
| `crates/riot-core/src/store/mod.rs` | Add `#[cfg(feature="sqlite")] pub(crate) use database::{map_sqlite_error, WriteEstimate};`. |
| `crates/riot-core/src/store/schema.rs` | Add `MIGRATION_THREE` (journal + 4 index tables), bump `CURRENT_SCHEMA_VERSION` to `3`, `found >= 3` branch + `validate_governance_structure`. |
| `crates/riot-core/Cargo.toml` | Register `governance_conformance` + `governance_durable` tests with `required-features = ["conformance"]`. |
| `fixtures/manifest.json` | Add `governance_vectors_sha256` to the `"environment"` object. |
| `crates/xtask/src/main.rs` | Add the `governance_vectors_sha256` validator branch **and** update the self-test scaffold (`scaffold`, `manifest_with`) so the existing xtask tests stay green. |

---

## Tasks

Execution order is dependency-topological: every task compiles and commits green at its own boundary. Scoped iteration uses `cargo test -p riot-core --all-features governance::<module>` (plain `cargo test -p riot-core` fails to compile — `identity_namespace_shape` needs `conformance`). Integration suites use `--test governance_conformance` / `--test governance_durable`.

### Task 0 — Scaffold the `governance` module, error taxonomy, kinds, ceilings

**Files:** Create `crates/riot-core/src/governance/mod.rs`; Modify `crates/riot-core/src/lib.rs`.

- [ ] **Write the failing test.** In `crates/riot-core/src/governance/mod.rs`:

```rust
//! Governance ledger (Slice 2). Versioned `GovernanceRecordV1` schema, actor/
//! device/action hash chains, a durable authority repository, a deterministic
//! policy evaluator, and transitive capability revocation. Governance answers
//! the product question; Meadowcap answers protocol validity. No admission,
//! sync, vault, app-broker, FFI, or UI concepts live here.

pub mod action;
pub mod actor;
pub mod authorize;
pub mod body;
pub mod evaluator;
pub mod frontier;
pub mod lineage;
pub mod paths;
pub mod record;
pub mod repository;
pub mod revoke;

#[cfg(any(test, feature = "conformance"))]
#[doc(hidden)]
pub mod test_support;

use crate::meadowcap::MeadowcapError;

/// A governance record id (domain-separated SHA-256 of the canonical record).
pub type RecordId = [u8; 32];
/// A Slice-1 capability fingerprint, reused verbatim as the governance join key.
pub type Fingerprint = [u8; 32];

/// Largest accepted governance record encoding (design resource ceilings).
pub const MAX_GOVERNANCE_RECORD_BYTES: usize = 16 * 1024;
/// Maximum accepted parent frontier references per record.
pub const MAX_PARENTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum RecordKind {
    Genesis = 0, ActorBinding = 1, MemberDecision = 2, InviteManagerDecision = 3,
    InviteResponse = 4, InviteActivation = 5, RoleDecision = 6, CapabilityIssued = 7,
    CapabilityRenewed = 8, CapabilityRevoked = 9, Checkpoint = 10, ActionReceipt = 11,
    Proposal = 12, AppealSubmitted = 13, AppealResolved = 14, AppApproved = 15,
    AppRevoked = 16, AppProvisioned = 17, DirectoryWithdrawn = 18, RecoveryDeclared = 19,
    MigrationDeclared = 20, LensSuccessor = 21,
}

impl RecordKind {
    pub fn from_tag(tag: u64) -> Result<Self, GovernanceError> {
        use RecordKind::*;
        Ok(match tag {
            0 => Genesis, 1 => ActorBinding, 2 => MemberDecision, 3 => InviteManagerDecision,
            4 => InviteResponse, 5 => InviteActivation, 6 => RoleDecision, 7 => CapabilityIssued,
            8 => CapabilityRenewed, 9 => CapabilityRevoked, 10 => Checkpoint, 11 => ActionReceipt,
            12 => Proposal, 13 => AppealSubmitted, 14 => AppealResolved, 15 => AppApproved,
            16 => AppRevoked, 17 => AppProvisioned, 18 => DirectoryWithdrawn, 19 => RecoveryDeclared,
            20 => MigrationDeclared, 21 => LensSuccessor,
            _ => return Err(GovernanceError::UnknownKind { tag }),
        })
    }
    pub fn tag(self) -> u64 { self as u8 as u64 }
}

/// Stable, non-secret governance rejection codes. Every variant has a producer
/// and a test (no unreachable codes — the Slice-1 `NonCanonical` lesson).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernanceError {
    /// Bytes did not decode as a canonical `GovernanceRecordV1` / body-shape
    /// mismatch with the envelope `kind`. (Task 1/2)
    Malformed,
    /// Canonical value did not consume all input bytes. (Task 2)
    TrailingBytes,
    /// Unknown record-kind tag (fails closed). (Task 0)
    UnknownKind { tag: u64 },
    /// Encoded record larger than `MAX_GOVERNANCE_RECORD_BYTES`. (Task 2)
    RecordTooLarge { bytes: usize, max: usize },
    /// More than `MAX_PARENTS`, or parents unsorted/duplicated. (Task 2)
    ParentsInvalid,
    /// Entry path does not match the record kind's canonical target. (Task 4)
    PathBindingMismatch,
    /// Per-actor sequence gap, fork, or wrong `prev_actor_record`. (Task 5)
    ActorChainBroken,
    /// A receipt referenced a receipt/itself, a missing action, or a
    /// privileged action had no paired receipt. (Task 7)
    ActionChainInvalid,
    /// A record purported to authorize itself. (Task 8)
    SelfAuthorization,
    /// Issuance body's embedded child is not a valid attenuation-descendant of
    /// its named parent (fingerprint forgery or non-descendant). (Task 8)
    IssuanceNotAttenuated,
    /// Underlying capability decode/validity failure from Slice 1. (Task 8)
    Capability(MeadowcapError),
    /// Durable-store failure surfaced non-gated so wasm (Memory-only) compiles.
    /// (Task 13)
    Storage,
}

impl std::fmt::Display for GovernanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for GovernanceError {}
impl From<MeadowcapError> for GovernanceError {
    fn from(e: MeadowcapError) -> Self { GovernanceError::Capability(e) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceilings_match_design_resource_limits() {
        assert_eq!(MAX_GOVERNANCE_RECORD_BYTES, 16 * 1024);
        assert_eq!(MAX_PARENTS, 16);
    }

    #[test]
    fn every_kind_round_trips_its_tag_and_unknown_fails_closed() {
        for tag in 0u64..=21 {
            assert_eq!(RecordKind::from_tag(tag).unwrap().tag(), tag);
        }
        assert_eq!(RecordKind::from_tag(22), Err(GovernanceError::UnknownKind { tag: 22 }));
    }
}
```

  Note the deliberate **deletion** of a `MissingParent` variant: `topological_reduce` (Task 6) returns unresolved records as *pending*, never as an error, so a `MissingParent` code would be structurally unreachable — declaring it would ship the Slice-1 `NonCanonical` dead-code mistake again. Create empty `//! placeholder` submodule files for every declared submodule **except** `test_support` (added in Task 3). Add `pub mod governance;` to `lib.rs` after `pub mod meadowcap;`. `test_support` is declared here but its module file lands in Task 3; until then, gate the `pub mod test_support;` line behind a `// TODO(Task 3)` comment or add an empty `test_support.rs` placeholder so Task 0 compiles.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::tests` — expected failure: `file not found for module` for the empty submodules. Create the placeholders, re-run; both tests pass.
- [ ] **Commit.** `git add crates/riot-core/src/governance/ crates/riot-core/src/lib.rs && git commit -m "feat(governance): scaffold ledger module, record kinds, errors, ceilings"` (the directory pathspec adds only the new governance files; no `-A`).

### Task 1 — Typed `Body` enum + per-body canonical codec

**Files:** Modify `crates/riot-core/src/governance/body.rs`.

Authority-bearing kinds are fully typed; `CapabilityIssued`/`CapabilityRenewed` embed **both** capability canonical byte strings (Task 8 verifies them). Deferred-semantics kinds carry opaque length-bounded canonical byte fields. `decode_body` parses against `kind`; a body whose shape mismatches its `kind` ⇒ `Malformed`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Per-kind governance record bodies. Authority-bearing kinds are fully typed
//! and evaluator-wired; deferred-semantics kinds freeze their envelope + path
//! and carry their later-slice payload as an opaque, length-bounded canonical
//! byte field validated by its owning slice.

use minicbor::{Decoder, Encoder};

use super::{Fingerprint, GovernanceError, RecordKind, MAX_GOVERNANCE_RECORD_BYTES};

/// A length-bounded opaque canonical CBOR byte field for deferred-slice payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueBytes(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cutoff {
    pub actor_id: [u8; 32],
    pub receiver_id: [u8; 32],
    pub action_head: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    // ---- authority-bearing (evaluator-wired) ----
    Genesis,
    ActorBinding { bound_receiver: [u8; 32], encryption_key: [u8; 32] },
    /// A role instance and the capability fingerprints it grants. Concurrent
    /// role decisions for the same `role_instance_id` reduce by INTERSECTION of
    /// `granted_fingerprints` (most restrictive wins — evaluator Task 12).
    RoleDecision { role_instance_id: [u8; 32], covering_parent_fingerprint: Fingerprint, granted_fingerprints: Vec<Fingerprint> },
    /// Embeds BOTH capability encodings so issuance is attenuation-verified, not
    /// fingerprint-trusted (Task 8). The fingerprints are the claimed values;
    /// Task 8 recomputes them from the embedded bytes and rejects a mismatch.
    CapabilityIssued {
        covering_parent_fingerprint: Fingerprint,
        child_fingerprint: Fingerprint,
        parent_capability_bytes: OpaqueBytes,
        child_capability_bytes: OpaqueBytes,
    },
    CapabilityRenewed {
        covering_parent_fingerprint: Fingerprint,
        child_fingerprint: Fingerprint,
        replaces_fingerprint: Fingerprint,
        parent_capability_bytes: OpaqueBytes,
        child_capability_bytes: OpaqueBytes,
    },
    CapabilityRevoked { target_fingerprint: Fingerprint, cutoffs: Vec<Cutoff> },
    Checkpoint { checkpoint_id: [u8; 32], merged_frontier_hash: [u8; 32] },
    ActionReceipt { receipt: OpaqueBytes }, // canonical ActionReceiptV1 bytes (Task 7)
    Proposal { proposal: OpaqueBytes },
    // ---- deferred-semantics (envelope frozen; payload opaque) ----
    MemberDecision { member_actor: [u8; 32], decision: OpaqueBytes },
    InviteManagerDecision { invite_id: [u8; 32], decision: OpaqueBytes },
    InviteResponse { invite_id: [u8; 32], response: OpaqueBytes },
    InviteActivation { invite_id: [u8; 32], activation: OpaqueBytes },
    AppealSubmitted { action_id: [u8; 32], appeal: OpaqueBytes },
    AppealResolved { action_id: [u8; 32], resolution: OpaqueBytes },
    AppApproved { app_id: [u8; 32], manifest_digest: [u8; 32], granted_permissions_cbor: OpaqueBytes },
    AppRevoked { app_id: [u8; 32], reason: OpaqueBytes },
    AppProvisioned { app_id: [u8; 32], receiver: [u8; 32], provisioning: OpaqueBytes },
    DirectoryWithdrawn { app_id: [u8; 32], withdrawal: OpaqueBytes },
    RecoveryDeclared { recovery: OpaqueBytes },
    MigrationDeclared { new_namespace: [u8; 32], migration: OpaqueBytes },
    LensSuccessor { new_namespace: [u8; 32], successor: OpaqueBytes },
}

/// The `RecordKind` a body variant belongs to (used by `decode_record`).
pub fn kind_of(body: &Body) -> RecordKind { /* exhaustive match, one arm per variant */ }

/// The primary target id for the `governance_by_target` index (Task 13):
/// issued/renewed → child_fingerprint; revoked → target_fingerprint; actor
/// binding → bound_receiver; role → role_instance_id; app kinds → app_id;
/// invites → invite_id; migration/lens → new_namespace; genesis/proposal →
/// actor_id; action receipt → receiver.
pub fn target_id_of(kind: RecordKind, body: &Body, actor_id: &[u8; 32]) -> [u8; 32] { /* ... */ }

pub fn encode_body(body: &Body, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), GovernanceError> { /* ordered keys per variant */ }
pub fn decode_body(kind: RecordKind, d: &mut Decoder<'_>) -> Result<Body, GovernanceError> { /* shape MUST match kind else Malformed; OpaqueBytes length-checked <= MAX_GOVERNANCE_RECORD_BYTES */ }

#[cfg(test)]
mod tests {
    use super::*;

    fn issued_body() -> Body {
        // Inline literals — no test_support yet (that arrives in Task 3). The
        // embedded bytes are placeholder here; Task 8 supplies genuine caps.
        Body::CapabilityIssued {
            covering_parent_fingerprint: [1u8; 32],
            child_fingerprint: [2u8; 32],
            parent_capability_bytes: OpaqueBytes(vec![0xAA; 200]),
            child_capability_bytes: OpaqueBytes(vec![0xBB; 260]),
        }
    }

    #[test]
    fn every_record_kind_has_a_body_variant_and_maps_back() {
        // Constructing one Body per kind inline and asserting kind_of round-trips
        // proves the 22-kind tag space is fully schemad.
        let samples = [
            Body::Genesis,
            Body::ActorBinding { bound_receiver: [3u8; 32], encryption_key: [4u8; 32] },
            // … one literal per remaining kind …
            issued_body(),
        ];
        for body in samples {
            let kind = kind_of(&body);
            assert!((0..=21).contains(&kind.tag()));
        }
    }

    #[test]
    fn body_round_trips_and_wrong_kind_shape_is_malformed() {
        let body = issued_body();
        let mut buf = Vec::new();
        encode_body(&body, &mut Encoder::new(&mut buf)).unwrap();
        assert_eq!(decode_body(RecordKind::CapabilityIssued, &mut Decoder::new(&buf)).unwrap(), body);
        // A Genesis body decoded as CapabilityIssued must be Malformed.
        let mut g = Vec::new();
        encode_body(&Body::Genesis, &mut Encoder::new(&mut g)).unwrap();
        assert_eq!(decode_body(RecordKind::CapabilityIssued, &mut Decoder::new(&g)),
                   Err(GovernanceError::Malformed));
    }

    #[test]
    fn a_realistic_depth_16_issuance_body_fits_under_the_record_ceiling() {
        // Two ~3 KiB caps (depth-16 worst case) must stay under 16 KiB so the
        // embedded-capability design does not collide with the record ceiling.
        let body = Body::CapabilityIssued {
            covering_parent_fingerprint: [1u8; 32],
            child_fingerprint: [2u8; 32],
            parent_capability_bytes: OpaqueBytes(vec![0u8; 3 * 1024]),
            child_capability_bytes: OpaqueBytes(vec![0u8; 3 * 1024]),
        };
        let mut buf = Vec::new();
        encode_body(&body, &mut Encoder::new(&mut buf)).unwrap();
        assert!(buf.len() < MAX_GOVERNANCE_RECORD_BYTES, "issuance body must fit the record ceiling");
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::body` — expected failure: `cannot find function kind_of`. Implement the enum + codec, re-run; all three tests pass.
- [ ] **Commit.** `git add crates/riot-core/src/governance/body.rs && git commit -m "feat(governance): typed per-kind bodies with embedded issuance capabilities"`

### Task 2 — `GovernanceRecordV1` envelope + `record_id`

**Files:** Modify `crates/riot-core/src/governance/record.rs`.

Canonical CBOR envelope (ordered integer keys, definite lengths, `prove_canonical` re-encode check), typed `body: Body` from the start, 16 KiB ceiling before decode, sorted/dedup parent check, domain-separated `record_id`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! `GovernanceRecordV1` canonical CBOR envelope + domain-separated record id.

use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};

use super::body::{decode_body, encode_body, Body};
use super::{GovernanceError, RecordKind, MAX_GOVERNANCE_RECORD_BYTES, MAX_PARENTS};
// Re-export BOTH id aliases so sibling modules (`actor`, `frontier`, `action`)
// can `use super::record::{RecordId, Fingerprint}` — a private `use` of RecordId
// here would make those imports E0603.
pub use super::{Fingerprint, RecordId};

pub const GOVERNANCE_RECORD_SCHEMA: &str = "org.riot.governance.record/1";
const RECORD_ID_DOMAIN: &[u8] = b"riot/governance-record-id/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceRecordV1 {
    pub kind: RecordKind,
    pub namespace: [u8; 32],
    pub parents: Vec<RecordId>,          // sorted, dedup, <= MAX_PARENTS
    pub actor_id: [u8; 32],
    pub receiver: [u8; 32],
    pub sequence: u64,
    pub prev_actor_record: Option<RecordId>,
    pub authorizing_fingerprint: Fingerprint,
    pub body: Body,
    pub created_display_micros: u64,
}

pub fn encode_record(r: &GovernanceRecordV1) -> Vec<u8> { /* keys 0..=10; body via encode_body */ }

pub fn decode_record(bytes: &[u8]) -> Result<GovernanceRecordV1, GovernanceError> {
    if bytes.len() > MAX_GOVERNANCE_RECORD_BYTES {
        return Err(GovernanceError::RecordTooLarge { bytes: bytes.len(), max: MAX_GOVERNANCE_RECORD_BYTES });
    }
    // decode envelope; RecordKind::from_tag; body via decode_body(kind, ...);
    // parents: <= MAX_PARENTS AND strictly ascending (sorted+dedup) else ParentsInvalid;
    // reject trailing bytes; re-encode and require byte-identical else Malformed.
    unimplemented!()
}

pub fn record_id(r: &GovernanceRecordV1) -> RecordId {
    let mut h = Sha256::new();
    h.update(RECORD_ID_DOMAIN);
    h.update(encode_record(r));
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genesis() -> GovernanceRecordV1 {
        GovernanceRecordV1 {
            kind: RecordKind::Genesis, namespace: [9u8; 32], parents: vec![],
            actor_id: [1u8; 32], receiver: [2u8; 32], sequence: 0, prev_actor_record: None,
            authorizing_fingerprint: [7u8; 32], body: Body::Genesis, created_display_micros: 1000,
        }
    }

    #[test]
    fn record_round_trips_canonically() {
        let r = genesis();
        assert_eq!(decode_record(&encode_record(&r)).unwrap(), r);
    }
    #[test]
    fn trailing_bytes_are_rejected() {
        let mut b = encode_record(&genesis()); b.push(0);
        assert_eq!(decode_record(&b), Err(GovernanceError::TrailingBytes));
    }
    #[test]
    fn descending_or_dup_parents_are_rejected() {
        // Build the envelope bytes with parents [0x09.., 0x01..] (descending)
        // directly (encode_record always emits sorted, so craft the CBOR here).
        let bytes = super::test_only_encode_with_parents(&genesis(), &[[9u8;32],[1u8;32]]);
        assert_eq!(decode_record(&bytes), Err(GovernanceError::ParentsInvalid));
    }
    #[test]
    fn oversized_record_is_rejected_before_decode() {
        assert_eq!(decode_record(&vec![0u8; MAX_GOVERNANCE_RECORD_BYTES + 1]),
                   Err(GovernanceError::RecordTooLarge { bytes: MAX_GOVERNANCE_RECORD_BYTES + 1, max: MAX_GOVERNANCE_RECORD_BYTES }));
    }
    #[test]
    fn record_id_is_domain_separated_from_the_fingerprint_domain() {
        let r = genesis();
        let raw: [u8;32] = Sha256::digest(encode_record(&r)).into();
        assert_ne!(record_id(&r), raw);
        let mut h = Sha256::new();
        h.update(b"riot/meadowcap-fingerprint/v1"); h.update(encode_record(&r));
        let mc: [u8;32] = h.finalize().into();
        assert_ne!(record_id(&r), mc, "record-id domain must not collide with the fingerprint domain");
    }
}
```

  `test_only_encode_with_parents` is a `#[cfg(test)]` helper that emits the envelope with caller-supplied (possibly unsorted) parent bytes, so the decoder's sort/dedup guard is exercised even though `encode_record` always emits sorted parents.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::record` — expected failure: `unimplemented`/`cannot find function`. Implement the codec + `record_id`, re-run; all five tests pass.
- [ ] **Commit.** `git add crates/riot-core/src/governance/record.rs && git commit -m "feat(governance): canonical GovernanceRecordV1 envelope and domain-separated record_id"`

### Task 3 — Shared `test_support` builders

**Files:** Create `crates/riot-core/src/governance/test_support.rs` (gated `#[cfg(any(test, feature = "conformance"))]` via the `mod.rs` declaration from Task 0).

A single module of seeded, deterministic builders shared by Tasks 4–16 (design "shared test builders", line ~1110). It depends only on `record`, `body`, and Slice-1 `meadowcap` (NOT on `action`, so there is no cycle — action-receipt builders live in `action.rs`). Genuine fingerprints come from real `delegate_write` chains.

- [ ] **Write the failing test.** In `test_support.rs`:

```rust
//! Deterministic seeded builders for governance tests. Declared `#[doc(hidden)]
//! pub mod test_support` and gated behind `conformance` (Task 0). Every builder
//! is `pub fn` — NOT `pub(crate)` — because Tasks 14/15 are integration-test
//! CRATES (`tests/*.rs`) that link `riot-core` externally and can only see `pub`
//! items; a `pub(crate)` builder is E0603 from an integration test.

use crate::meadowcap::create::new_owned_write;
use crate::meadowcap::delegate::delegate_write;
use crate::meadowcap::fingerprint::write_capability_fingerprint;
use willow25::authorisation::WriteCapability;
use willow25::prelude::{Area, NamespaceSecret, Path, SubspaceSecret, TimeRange};

use super::body::{Body, OpaqueBytes};
use super::record::{record_id, GovernanceRecordV1};
use super::{Fingerprint, RecordId, RecordKind};

const NS_SEED: [u8; 32] = [3u8; 32];

fn owner() -> SubspaceSecret { SubspaceSecret::from_bytes(&[4u8; 32]) }
fn namespace_secret() -> NamespaceSecret { NamespaceSecret::from_bytes(&NS_SEED) }
fn micros(from: u64, to: u64) -> TimeRange {
    use crate::willow::tai_j2000_micros_from_unix_seconds as tai;
    TimeRange::new(tai(from).unwrap().into(), Some(tai(to).unwrap().into()))
}

/// An owned genesis write cap and a one-hop child delegated to `seed`'s receiver.
pub fn parent_and_child(seed: u8) -> (WriteCapability, WriteCapability) {
    let parent = new_owned_write(&namespace_secret(), owner().corresponding_subspace_id());
    let child_id = SubspaceSecret::from_bytes(&[seed; 32]).corresponding_subspace_id();
    let area = Area::new(Some(child_id.clone()),
        Path::from_slices(&[b"content"]).unwrap(), micros(1_700_000_000, 1_800_000_000));
    let child = delegate_write(&parent, &owner(), area, child_id).expect("attenuate");
    (parent, child)
}

pub fn genesis_record(namespace: [u8; 32]) -> GovernanceRecordV1 {
    let parent = new_owned_write(&namespace_secret(), owner().corresponding_subspace_id());
    GovernanceRecordV1 {
        kind: RecordKind::Genesis, namespace, parents: vec![],
        actor_id: [1u8; 32], receiver: [2u8; 32], sequence: 0, prev_actor_record: None,
        authorizing_fingerprint: write_capability_fingerprint(&parent),
        body: Body::Genesis, created_display_micros: 1000,
    }
}

/// A valid `CapabilityIssued` record with GENUINE embedded parent+child bytes.
pub fn issued_record(namespace: [u8; 32], seed: u8) -> GovernanceRecordV1 {
    let (parent, child) = parent_and_child(seed);
    let parent_fp = write_capability_fingerprint(&parent);
    let child_fp = write_capability_fingerprint(&child);
    GovernanceRecordV1 {
        kind: RecordKind::CapabilityIssued, namespace,
        parents: vec![record_id(&genesis_record(namespace))],
        actor_id: [1u8; 32], receiver: [2u8; 32], sequence: 1,
        prev_actor_record: Some(record_id(&genesis_record(namespace))),
        authorizing_fingerprint: parent_fp,
        body: Body::CapabilityIssued {
            covering_parent_fingerprint: parent_fp,
            child_fingerprint: child_fp,
            parent_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&parent)),
            child_capability_bytes: OpaqueBytes(crate::willow::encode_capability(&child)),
        },
        created_display_micros: 2000,
    }
}

pub fn fingerprint_of_issued(r: &GovernanceRecordV1) -> Fingerprint {
    match &r.body {
        Body::CapabilityIssued { child_fingerprint, .. } => *child_fingerprint,
        _ => panic!("not an issuance record"),
    }
}

pub fn revoke_record(target: Fingerprint) -> GovernanceRecordV1 { /* CapabilityRevoked body, empty cutoffs */ }
pub fn actor_record(actor: [u8; 32], seq: u64, prev: Option<RecordId>) -> GovernanceRecordV1 { /* Proposal-kind */ }
pub fn binding_record(actor: [u8; 32], receiver: [u8; 32]) -> GovernanceRecordV1 { /* ActorBinding */ }
pub fn child_record(parents: &[RecordId], seed: u8) -> GovernanceRecordV1 { /* Proposal-kind, given parents */ }
pub fn sample_body_for(kind: RecordKind) -> Body { /* one seeded Body per kind */ }
pub fn seeded_record_for(kind: RecordKind) -> GovernanceRecordV1 { /* one seeded, valid record per kind */ }
pub fn seeded_journal(n: usize) -> Vec<GovernanceRecordV1> { /* genesis + bindings + issuances forming a valid DAG */ }
pub fn three_hop_lineage_records() -> (Vec<GovernanceRecordV1>, Fps) { /* G->A->B->C + sibling D via real delegate_write */ }
pub fn three_hop_lineage_records_with_sibling_active() -> (Vec<GovernanceRecordV1>, Fps) { /* + issuances for all */ }
pub fn renewal_after_revoke() -> (Vec<GovernanceRecordV1>, Fingerprint, Fingerprint) { /* (records, old_fp, new_fp) */ }
pub fn issued_record_with_missing_parent(namespace: [u8; 32], seed: u8) -> GovernanceRecordV1 { /* parents = [never-ingested id] */ }
pub fn self_authorizing_record() -> GovernanceRecordV1 { /* authorizing_fingerprint == its own child_fingerprint */ }
// The role's two candidate capabilities X and Y are real one-hop delegations
// from a fixed genesis (seeds 20 and 21), so their fingerprints are genuine and
// reproducible. `is_role_fp` recomputes both and tests membership.
fn role_candidate_caps() -> (WriteCapability, WriteCapability) {
    let (_p, x) = parent_and_child(20);
    let (_p2, y) = parent_and_child(21);
    (x, y)
}
pub fn is_role_fp(fp: &Fingerprint) -> bool {
    let (x, y) = role_candidate_caps();
    *fp == write_capability_fingerprint(&x) || *fp == write_capability_fingerprint(&y)
}

/// A journal in which capabilities X and Y are BOTH genuinely issued (active
/// after the grant fold), plus TWO concurrent RoleDecision records for the same
/// role instance R — both parented on genesis, neither an ancestor of the other:
/// decision_1 grants {X, Y}, decision_2 grants {X}. The intersection is {X}, so
/// after `apply_role_restrictions` only X survives among the role's fps.
/// Returns `(records, fingerprint_of_X)`.
pub fn two_concurrent_role_restrictions() -> (Vec<GovernanceRecordV1>, Fingerprint) {
    let ns = [9u8; 32];
    let (x, y) = role_candidate_caps();
    let (fp_x, fp_y) = (write_capability_fingerprint(&x), write_capability_fingerprint(&y));
    let genesis = genesis_record(ns);
    let gid = record_id(&genesis);

    // Real issuances so the grant fold activates both X and Y.
    let issue_x = issued_record_for_cap(ns, 20, &[gid]);
    let issue_y = issued_record_for_cap(ns, 21, &[gid]);

    let role = [77u8; 32];
    let decision_1 = role_decision_record(ns, role, &[gid], vec![fp_x, fp_y]); // grants {X, Y}
    let decision_2 = role_decision_record(ns, role, &[gid], vec![fp_x]);       // grants {X}, concurrent
    (vec![genesis, issue_x, issue_y, decision_1, decision_2], fp_x)
}

// Helpers behind the fixtures above (implemented alongside them):
//  - issued_record_for_cap(ns, seed, parents): a CapabilityIssued record whose
//    embedded child is `parent_and_child(seed).1` (so its fp matches X/Y).
//  - role_decision_record(ns, role_instance_id, parents, granted): a RoleDecision
//    record with the given granted_fingerprints, parented on `parents`.
pub fn issued_record_for_cap(namespace: [u8; 32], seed: u8, parents: &[RecordId]) -> GovernanceRecordV1 { /* CapabilityIssued with embedded parent_and_child(seed) caps + genuine fps; parents set */ }
pub fn role_decision_record(namespace: [u8; 32], role: [u8; 32], parents: &[RecordId], granted: Vec<Fingerprint>) -> GovernanceRecordV1 { /* RoleDecision body { role_instance_id: role, covering_parent_fingerprint: <genesis cap fp>, granted_fingerprints: granted } */ }
pub fn revoke_then_favorable_appeal() -> (Vec<GovernanceRecordV1>, Fingerprint) { /* (records, revoked_fp) */ }
pub fn two_competing_migrations() -> Vec<GovernanceRecordV1> { /* two MigrationDeclared, distinct new_namespace */ }

pub struct Fps { pub a: Fingerprint, pub b: Fingerprint, pub c: Fingerprint, pub d_sibling: Fingerprint }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::record::{decode_record, encode_record};

    #[test]
    fn every_seeded_record_decodes_canonically() {
        for tag in 0u64..=21 {
            let kind = RecordKind::from_tag(tag).unwrap();
            let r = seeded_record_for(kind);
            assert_eq!(decode_record(&encode_record(&r)).unwrap(), r, "{kind:?} seeded record must round-trip");
        }
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::test_support` — expected failure: unresolved builders. Implement them, re-run; the smoke test passes for all 22 kinds.
- [ ] **Commit.** `git add crates/riot-core/src/governance/test_support.rs crates/riot-core/src/governance/mod.rs && git commit -m "test(governance): shared seeded record/body/capability builders"`

### Task 4 — Exact per-kind path templates + path↔body target binding

**Files:** Modify `crates/riot-core/src/governance/paths.rs`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Exact `governance/v1/...` path templates per kind + path↔body target binding.
//! Reproduces the design record-path table (lines ~456–480) verbatim. Ids are
//! raw 32-byte components; `sequence_be` is an 8-byte big-endian component.

use crate::willow::Path;
use super::body::Body;
use super::record::{record_id, GovernanceRecordV1};
use super::{GovernanceError, RecordKind};

pub fn path_for(r: &GovernanceRecordV1) -> Result<Path, GovernanceError> {
    let rid = record_id(r);
    let p = |parts: &[&[u8]]| Path::from_slices(parts).map_err(|_| GovernanceError::PathBindingMismatch);
    match (&r.kind, &r.body) {
        (RecordKind::Genesis, _) => p(&[b"governance", b"v1", b"genesis"]),
        (RecordKind::CapabilityIssued, Body::CapabilityIssued { child_fingerprint, .. }) =>
            p(&[b"governance", b"v1", b"capabilities", b"issued", child_fingerprint, &rid]),
        (RecordKind::CapabilityRevoked, Body::CapabilityRevoked { target_fingerprint, .. }) =>
            p(&[b"governance", b"v1", b"revocations", target_fingerprint, &rid]),
        (RecordKind::ActorBinding, Body::ActorBinding { bound_receiver, .. }) =>
            p(&[b"governance", b"v1", b"actors", &r.actor_id, b"bindings", bound_receiver, &rid]),
        // … one arm per remaining kind, each reproducing its exact template row …
        _ => Err(GovernanceError::PathBindingMismatch),
    }
}

pub fn verify_path_binding(entry_path: &Path, r: &GovernanceRecordV1) -> Result<(), GovernanceError> {
    if *entry_path == path_for(r)? { Ok(()) } else { Err(GovernanceError::PathBindingMismatch) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::seeded_record_for;

    #[test]
    fn genesis_path_is_exact_and_a_mangled_path_is_rejected() {
        let r = seeded_record_for(RecordKind::Genesis);
        assert_eq!(path_for(&r).unwrap(), Path::from_slices(&[b"governance", b"v1", b"genesis"]).unwrap());
        let mangled = Path::from_slices(&[b"governance", b"v1", b"genesis", b"x"]).unwrap();
        assert_eq!(verify_path_binding(&mangled, &r), Err(GovernanceError::PathBindingMismatch));
    }

    #[test]
    fn capability_issued_path_binds_the_fingerprint_component() {
        let r = seeded_record_for(RecordKind::CapabilityIssued);
        assert_eq!(verify_path_binding(&path_for(&r).unwrap(), &r), Ok(()));
        let wrong = Path::from_slices(&[b"governance", b"v1", b"capabilities", b"issued",
            &[0xAAu8; 32], &record_id(&r)]).unwrap();
        assert_eq!(verify_path_binding(&wrong, &r), Err(GovernanceError::PathBindingMismatch));
    }

    #[test]
    fn every_kind_has_an_exact_path_and_an_extra_component_rejects() {
        for tag in 0u64..=21 {
            let kind = RecordKind::from_tag(tag).unwrap();
            let r = seeded_record_for(kind);
            let good = path_for(&r).expect("path_for");
            assert_eq!(verify_path_binding(&good, &r), Ok(()), "{kind:?} self-binds");
            // willow25 `Path::components()` yields `&Component`, not `&[u8]`;
            // `.as_bytes()` gives the component's raw bytes.
            let mut parts: Vec<&[u8]> = good.components().map(|c| c.as_bytes()).collect();
            parts.push(b"extra");
            let extra = Path::from_slices(&parts).unwrap();
            assert_eq!(verify_path_binding(&extra, &r), Err(GovernanceError::PathBindingMismatch), "{kind:?} extra rejected");
        }
    }
}
```

  `Path::components()` yields `&Component`; use `.as_bytes()` (as above) to reach the raw component bytes for `Path::from_slices`. Confirm the exact `Component` accessor name during implementation against the willow `Path` used in `store::evidence`; adapt the loop builder only, never the assertions.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::paths` — expected failure: `cannot find function path_for`. Reproduce every template row, re-run; all three tests pass (the loop covers all 22 kinds).
- [ ] **Commit.** `git add crates/riot-core/src/governance/paths.rs && git commit -m "feat(governance): exact per-kind path templates and target binding"`

### Task 5 — Actor/device binding + per-actor sequence chains

**Files:** Modify `crates/riot-core/src/governance/actor.rs`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Actor/device binding facts and per-actor sequence hash chains.

use std::collections::{BTreeMap, BTreeSet};

use super::body::Body;
use super::record::{record_id, GovernanceRecordV1, RecordId};
use super::{GovernanceError, RecordKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorChain { pub actor_id: [u8; 32], pub ordered: Vec<RecordId>, pub head_sequence: u64 }

pub fn validate_actor_chain(records_for_actor: &[GovernanceRecordV1]) -> Result<ActorChain, GovernanceError> {
    if records_for_actor.is_empty() { return Err(GovernanceError::ActorChainBroken); }
    let actor_id = records_for_actor[0].actor_id;
    let mut sorted: Vec<&GovernanceRecordV1> = records_for_actor.iter().collect();
    sorted.sort_by_key(|r| r.sequence);
    let mut ordered = Vec::with_capacity(sorted.len());
    let mut prev_id: Option<RecordId> = None;
    for (index, record) in sorted.iter().enumerate() {
        if record.actor_id != actor_id
            || record.sequence != index as u64          // no gaps / no forks
            || record.prev_actor_record != prev_id       // correct link
        {
            return Err(GovernanceError::ActorChainBroken);
        }
        let id = record_id(record);
        ordered.push(id);
        prev_id = Some(id);
    }
    Ok(ActorChain { actor_id, head_sequence: (ordered.len() - 1) as u64, ordered })
}

pub fn actor_bindings(records: &[GovernanceRecordV1]) -> BTreeMap<[u8; 32], BTreeSet<[u8; 32]>> {
    let mut out: BTreeMap<[u8; 32], BTreeSet<[u8; 32]>> = BTreeMap::new();
    for r in records {
        if r.kind == RecordKind::ActorBinding {
            if let Body::ActorBinding { bound_receiver, .. } = &r.body {
                out.entry(r.actor_id).or_default().insert(*bound_receiver);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::{actor_record, binding_record};

    fn chain(actor: [u8; 32], count: u64) -> Vec<GovernanceRecordV1> {
        let mut out = Vec::new(); let mut prev = None;
        for seq in 0..count { let r = actor_record(actor, seq, prev); prev = Some(record_id(&r)); out.push(r); }
        out
    }

    #[test] fn a_valid_three_record_chain_is_accepted() {
        assert_eq!(validate_actor_chain(&chain([7u8;32], 3)).unwrap().head_sequence, 2);
    }
    #[test] fn a_gap_is_rejected() {
        let mut c = chain([7u8;32], 2); c[1].sequence = 2;
        assert_eq!(validate_actor_chain(&c), Err(GovernanceError::ActorChainBroken));
    }
    #[test] fn a_fork_is_rejected() {
        let mut c = chain([7u8;32], 2); c[1].sequence = 0;
        assert_eq!(validate_actor_chain(&c), Err(GovernanceError::ActorChainBroken));
    }
    #[test] fn a_wrong_prev_link_is_rejected() {
        let mut c = chain([7u8;32], 2); c[1].prev_actor_record = Some([0xAB;32]);
        assert_eq!(validate_actor_chain(&c), Err(GovernanceError::ActorChainBroken));
    }
    #[test] fn one_actor_binds_multiple_receivers() {
        let a = [9u8;32];
        let b = actor_bindings(&[binding_record(a, [1u8;32]), binding_record(a, [2u8;32])]);
        let r = b.get(&a).unwrap();
        assert!(r.contains(&[1u8;32]) && r.contains(&[2u8;32]));
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::actor` — expected failure: `cannot find function validate_actor_chain`. Implement, re-run; all five tests pass.
- [ ] **Commit.** `git add crates/riot-core/src/governance/actor.rs && git commit -m "feat(governance): actor/device binding and per-actor sequence chains"`

### Task 6 — Frontier, frontier hash, topological reduction

**Files:** Modify `crates/riot-core/src/governance/frontier.rs`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Governance frontier + frontier hash + topological DAG reduction. Display
//! timestamps never order governance; ordering is the causal DAG with a
//! `record_id` tiebreak, so every peer reduces the same journal identically.

use std::collections::BTreeSet;
use sha2::{Digest, Sha256};
use super::record::{record_id, GovernanceRecordV1, RecordId};

const FRONTIER_DOMAIN: &[u8] = b"riot/governance-frontier/v1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontier { pub accepted: BTreeSet<RecordId> }

pub fn frontier_hash(f: &Frontier) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(FRONTIER_DOMAIN);
    for id in &f.accepted { h.update(id); } // BTreeSet iterates ascending → deterministic
    h.finalize().into()
}

pub fn topological_reduce(records: &[GovernanceRecordV1]) -> (Vec<GovernanceRecordV1>, Vec<GovernanceRecordV1>) {
    let mut sorted: Vec<GovernanceRecordV1> = records.to_vec();
    sorted.sort_by_key(record_id);
    let mut accepted_ids: BTreeSet<RecordId> = BTreeSet::new();
    let mut accepted = Vec::new();
    let mut remaining = sorted;
    loop {
        let mut progressed = false;
        let mut pending = Vec::new();
        for r in remaining.into_iter() {
            if r.parents.iter().all(|p| accepted_ids.contains(p)) {
                accepted_ids.insert(record_id(&r)); accepted.push(r); progressed = true;
            } else { pending.push(r); }
        }
        remaining = pending;
        if !progressed || remaining.is_empty() { break; }
    }
    (accepted, remaining)
}

pub fn frontier_of(accepted: &[GovernanceRecordV1]) -> Frontier {
    Frontier { accepted: accepted.iter().map(record_id).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::child_record;

    fn linear() -> [GovernanceRecordV1; 3] {
        let a = child_record(&[], 1);
        let b = child_record(&[record_id(&a)], 2);
        let c = child_record(&[record_id(&b)], 3);
        [a, b, c]
    }

    #[test] fn dag_reduces_in_causal_order_regardless_of_input_order() {
        let [a,b,c] = linear();
        let (acc, pend) = topological_reduce(&[c.clone(), a.clone(), b.clone()]);
        assert!(pend.is_empty());
        let pos = |r: &GovernanceRecordV1| acc.iter().position(|x| x==r).unwrap();
        assert!(pos(&a) < pos(&b) && pos(&b) < pos(&c));
    }
    #[test] fn missing_parent_stays_pending_not_error() {
        let [_,b,c] = linear();
        let (acc, pend) = topological_reduce(&[b, c]);
        assert!(acc.is_empty() && pend.len() == 2);
    }
    #[test] fn display_timestamps_never_reorder() {
        let [mut a, mut b, c] = linear();
        a.created_display_micros = 9000; b.created_display_micros = 1000;
        let (acc, _) = topological_reduce(&[a.clone(), b.clone(), c]);
        let pos = |r: &GovernanceRecordV1| acc.iter().position(|x| x==r).unwrap();
        assert!(pos(&a) < pos(&b));
    }
    #[test] fn frontier_hash_is_order_free_and_domain_separated() {
        let [a,b,c] = linear();
        assert_eq!(frontier_hash(&frontier_of(&[a.clone(),b.clone(),c.clone()])),
                   frontier_hash(&frontier_of(&[c,b,a])));
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::frontier` — expected failure: `cannot find function topological_reduce`. Implement, re-run; all four tests pass.
- [ ] **Commit.** `git add crates/riot-core/src/governance/frontier.rs && git commit -m "feat(governance): frontier hash and topological DAG reduction"`

### Task 7 — Action receipts, hash chains, pairing (missing-pair + swapped-action)

**Files:** Modify `crates/riot-core/src/governance/action.rs`.

`ActionReceiptV1` codec + `action_hash` + `validate_action_chain`. Adds the two behaviors the gate named: **missing-pair** (a privileged action with no receipt ⇒ invalid; genesis the only exception, spec ~563–565) and **swapped-action** (a receipt whose `entry_id` points at a different action ⇒ rejected, spec testing line ~1164). Provides `pub` (conformance-gated) seeded receipt builders (kept here, not in `test_support`, to avoid a `test_support ↔ action` cycle; `pub` so the Task-14 integration test can call `action::seeded_action_receipt`).

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! `ActionReceiptV1` authorization sidecars + per-(actor,receiver) hash chains.

use std::collections::BTreeSet;
use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};
use super::record::RecordId;
use super::GovernanceError;

const ACTION_HASH_DOMAIN: &[u8] = b"riot/governance-action-hash/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionReceiptV1 {
    pub entry_id: [u8; 32],
    pub capability_fingerprint: [u8; 32],
    pub actor_id: [u8; 32],
    pub receiver: [u8; 32],
    pub actor_sequence: u64,
    pub previous_action_hash: Option<[u8; 32]>,
    pub policy_frontier_hash: [u8; 32],
}

pub fn encode_receipt(r: &ActionReceiptV1) -> Vec<u8> { /* ordered keys, definite lengths */ }
pub fn decode_receipt(bytes: &[u8]) -> Result<ActionReceiptV1, GovernanceError> { /* canonical + trailing reject */ }

pub fn action_hash(r: &ActionReceiptV1) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(ACTION_HASH_DOMAIN);
    h.update(encode_receipt(r));
    h.finalize().into()
}

/// Validate the per-(actor,receiver) receipt chain. `privileged_actions` is the
/// set of genuine (non-receipt) privileged-action entry ids that REQUIRE a
/// paired receipt; `genesis_action` (if any) is the single exempt base case.
pub fn validate_action_chain(
    receipts: &[ActionReceiptV1],
    privileged_actions: &BTreeSet<RecordId>,
    genesis_action: Option<RecordId>,
) -> Result<(), GovernanceError> {
    let mut sorted: Vec<&ActionReceiptV1> = receipts.iter().collect();
    sorted.sort_by_key(|r| r.actor_sequence);
    // The set of THIS batch's receipt action-hashes. A receipt whose `entry_id`
    // is one of these is naming a receipt as its action (receipt-of-receipt, or
    // the degenerate self-reference). This guard is CONSTRUCTABLE and isolable
    // from the genuine-action-set check below. (The narrower `entry_id ==
    // action_hash(self)` fixed point is a SHA-256 preimage and unconstructable —
    // it is the harmless degenerate member of this set, not a separate guard, so
    // no phantom code is shipped: the Slice-1 `NonCanonical` lesson.)
    let receipt_hashes: BTreeSet<[u8; 32]> = sorted.iter().map(|r| action_hash(r)).collect();
    let mut prev: Option<[u8; 32]> = None;
    let mut paired: BTreeSet<[u8; 32]> = BTreeSet::new();
    for (i, r) in sorted.iter().enumerate() {
        // (a) entry must be a genuine action (rejects an entry that is not a
        //     recognized privileged action at all).
        if !privileged_actions.contains(&r.entry_id) {
            return Err(GovernanceError::ActionChainInvalid);
        }
        // (b) entry must NOT be a receipt's own action-hash (rejects naming a
        //     receipt — or itself — as the action, ISOLATED from (a)).
        if receipt_hashes.contains(&r.entry_id) {
            return Err(GovernanceError::ActionChainInvalid);
        }
        if !paired.insert(r.entry_id) { return Err(GovernanceError::ActionChainInvalid); } // one receipt ↔ one action
        if r.actor_sequence != i as u64 || r.previous_action_hash != prev {
            return Err(GovernanceError::ActionChainInvalid); // swapped / tampered link
        }
        prev = Some(action_hash(r));
    }
    // MISSING-PAIR: every privileged action except the genesis base case must
    // have a paired receipt.
    for action in privileged_actions {
        if Some(*action) != genesis_action && !paired.contains(action) {
            return Err(GovernanceError::ActionChainInvalid);
        }
    }
    Ok(())
}

// Seeded builders shared with Task 11 (cutoff) and Task 14 (vectors).
#[cfg(any(test, feature = "conformance"))]
pub fn action_receipt_chain(n: usize) -> (Vec<ActionReceiptV1>, BTreeSet<RecordId>) { /* linked chain + its action-id set */ }
#[cfg(any(test, feature = "conformance"))]
pub fn seeded_action_receipt() -> ActionReceiptV1 { action_receipt_chain(1).0.remove(0) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn a_valid_three_action_chain_is_accepted() {
        let (r, ids) = action_receipt_chain(3);
        let genesis = *ids.iter().next().unwrap(); // treat first as the exempt base
        assert_eq!(validate_action_chain(&r, &ids, Some(genesis)), Ok(()));
    }
    #[test] fn a_privileged_action_with_no_receipt_is_rejected() {
        let (r, mut ids) = action_receipt_chain(2);
        ids.insert([0xCD; 32]); // an extra privileged action with no paired receipt
        assert_eq!(validate_action_chain(&r, &ids, None), Err(GovernanceError::ActionChainInvalid));
    }
    #[test] fn the_genesis_base_case_needs_no_receipt() {
        let mut ids = BTreeSet::new(); ids.insert([0x11; 32]);
        assert_eq!(validate_action_chain(&[], &ids, Some([0x11; 32])), Ok(()));
    }
    #[test] fn a_receipt_pointing_at_a_non_genuine_action_is_rejected_by_the_genuine_action_check() {
        let (mut r, ids) = action_receipt_chain(2);
        r[1].entry_id = [0x55; 32]; // not in `ids` → fails guard (a) only
        assert_eq!(validate_action_chain(&r, &ids, None), Err(GovernanceError::ActionChainInvalid));
    }
    #[test] fn a_receipt_naming_a_receipt_hash_is_rejected_even_when_listed_as_an_action() {
        // ISOLATES guard (b): set entry_id to receipt-0's action-hash AND insert
        // that hash into `ids` so guard (a) PASSES — only the receipt-hash guard
        // can fire. This covers receipt-of-receipt and the degenerate self-ref.
        let (mut r, mut ids) = action_receipt_chain(2);
        let h0 = action_hash(&r[0]);
        r[1].entry_id = h0;
        ids.insert(h0);
        assert_eq!(validate_action_chain(&r, &ids, None), Err(GovernanceError::ActionChainInvalid));
    }
    #[test] fn a_tampered_previous_action_hash_is_rejected() {
        let (mut r, ids) = action_receipt_chain(2);
        r[1].previous_action_hash = Some([0xEE; 32]);
        assert_eq!(validate_action_chain(&r, &ids, None), Err(GovernanceError::ActionChainInvalid));
    }
    #[test] fn two_receipts_pairing_one_action_are_rejected() {
        let (mut r, mut ids) = action_receipt_chain(2);
        r[1].entry_id = r[0].entry_id; ids.insert(r[0].entry_id);
        assert_eq!(validate_action_chain(&r, &ids, None), Err(GovernanceError::ActionChainInvalid));
    }
    #[test] fn receipt_round_trips_and_rejects_trailing_bytes() {
        let (r, _) = action_receipt_chain(1);
        let bytes = encode_receipt(&r[0]);
        assert_eq!(decode_receipt(&bytes).unwrap(), r[0]);
        let mut t = bytes.clone(); t.push(0);
        assert_eq!(decode_receipt(&t), Err(GovernanceError::TrailingBytes));
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::action` — expected failure: `cannot find function validate_action_chain`. Implement the codec + validator, re-run; all eight tests pass (including the isolated genuine-action guard (a) and receipt-hash guard (b)).
- [ ] **Commit.** `git add crates/riot-core/src/governance/action.rs && git commit -m "feat(governance): action-receipt chains, missing-pair, swapped-action, receipt-of-receipt rejection"`

### Task 8 — Attenuation-proof issuance verification (security)

**Files:** Modify `crates/riot-core/src/governance/authorize.rs`.

**Spec ~502–505 security requirement.** An issuance/renewal record must prove its child capability is a genuine attenuation-descendant of the presented parent — not merely carry claimed fingerprints. This function is the producer for `GovernanceError::{Capability, IssuanceNotAttenuated, SelfAuthorization}`. It also houses `authorize_record` and `selected_migration`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Governance authorization checks layered on Slice-1 capability validity.

use super::body::Body;
use super::frontier::Frontier;
use super::record::{record_id, GovernanceRecordV1};
use super::{GovernanceError, RecordKind};
use crate::meadowcap::codec::decode_write_capability_bounded;
use crate::meadowcap::fingerprint::write_capability_fingerprint;
use crate::willow::encode_capability;

/// Verify a `CapabilityIssued`/`CapabilityRenewed` body: both embedded caps
/// decode (cryptographic chain-validity via Slice 1), their recomputed
/// fingerprints match the body's claims, and the child is an attenuation-
/// descendant of the parent (same genesis, child chain extends parent chain,
/// child area ⊆ parent area).
pub fn verify_capability_issuance(body: &Body) -> Result<(), GovernanceError> {
    let (parent_fp, child_fp, parent_bytes, child_bytes) = match body {
        Body::CapabilityIssued { covering_parent_fingerprint, child_fingerprint, parent_capability_bytes, child_capability_bytes }
        | Body::CapabilityRenewed { covering_parent_fingerprint, child_fingerprint, parent_capability_bytes, child_capability_bytes, .. } =>
            (covering_parent_fingerprint, child_fingerprint, &parent_capability_bytes.0, &child_capability_bytes.0),
        _ => return Err(GovernanceError::Malformed),
    };

    // (1) Both decode → willow25 cryptographically verified every chain signature.
    let parent = decode_write_capability_bounded(parent_bytes).map_err(GovernanceError::Capability)?;
    let child = decode_write_capability_bounded(child_bytes).map_err(GovernanceError::Capability)?;

    // (2) Claimed fingerprints are recomputed from the bytes (forgery guard).
    if &write_capability_fingerprint(&parent) != parent_fp
        || &write_capability_fingerprint(&child) != child_fp
    {
        return Err(GovernanceError::IssuanceNotAttenuated);
    }

    // (3) Ancestry — STRUCTURAL comparison (no chain-truncation constructor
    // exists in willow25). The `meadowcap` `Delegation` type derives
    // `PartialEq, Eq` and exposes public `area`/`user`/`signature` fields
    // (meadowcap-0.5.0/src/raw/mod.rs:190 derive, :197-204 fields), so
    // `&[Delegation]` slice equality is a direct `==`; the per-delegation
    // signature bytes are what make the comparison forgery-proof.
    let pd = parent.delegations();
    let cd = child.delegations();
    // (3a) Same genesis: namespace key, user key, access mode, and communal-vs-
    // owned all match (meadowcap-0.5.0/src/raw/mod.rs:78/86/94 genesis
    // accessors; write_capability.rs:248 `is_owned`).
    let same_genesis = parent.granted_namespace() == child.granted_namespace()
        && parent.genesis().namespace_key() == child.genesis().namespace_key()
        && parent.genesis().user_key() == child.genesis().user_key()
        && parent.genesis().access_mode() == child.genesis().access_mode()
        && parent.is_owned() == child.is_owned();
    // (3b) Child chain extends the parent's by exactly one hop AND the child's
    // leading delegations equal the parent's delegations element-wise (derived
    // `Eq` on `Delegation`). A same-genesis SIBLING (a differently-signed chain
    // of the same length/depth) fails here because its delegation prefix — the
    // signatures included — differs from the parent's.
    let extends = cd.len() == pd.len() + 1 && cd[..pd.len()] == *pd;
    // (4) Attenuation: child area ⊆ parent area (willow25 also enforced this at
    // delegation time; re-assert defensively).
    let attenuated = parent.includes_area(&child.granted_area());

    if same_genesis && extends && attenuated {
        Ok(())
    } else {
        Err(GovernanceError::IssuanceNotAttenuated)
    }
}

/// A governance record's authority must come from an already-accepted ancestor
/// frontier — never from itself. Rejects a record whose authorizing_fingerprint
/// resolves only to its own issued child fingerprint.
pub fn authorize_record(record: &GovernanceRecordV1, _frontier: &Frontier) -> Result<(), GovernanceError> {
    if let Body::CapabilityIssued { child_fingerprint, .. } = &record.body {
        if &record.authorizing_fingerprint == child_fingerprint {
            return Err(GovernanceError::SelfAuthorization);
        }
    }
    // (frontier-ancestor membership is checked by the evaluator; this guard is
    // the self-authorization base case — record_id can never equal a parent.)
    let _ = record_id(record);
    Ok(())
}

/// Classify competing migrations: returns `None` (a fork requiring human
/// selection) whenever more than one distinct `MigrationDeclared` new-namespace
/// is present; `Some(ns)` only when exactly one candidate exists.
pub fn selected_migration(records: &[GovernanceRecordV1]) -> Option<[u8; 32]> {
    let mut candidates = std::collections::BTreeSet::new();
    for r in records {
        if r.kind == RecordKind::MigrationDeclared {
            if let Body::MigrationDeclared { new_namespace, .. } = &r.body { candidates.insert(*new_namespace); }
        }
    }
    if candidates.len() == 1 { candidates.into_iter().next() } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::body::{Body, OpaqueBytes};
    use crate::governance::test_support::{issued_record, parent_and_child};

    #[test] fn a_genuine_issuance_verifies() {
        assert_eq!(verify_capability_issuance(&issued_record([9u8;32], 8).body), Ok(()));
    }

    #[test] fn a_forged_fingerprint_is_rejected() {
        let mut record = issued_record([9u8;32], 8);
        if let Body::CapabilityIssued { child_fingerprint, .. } = &mut record.body {
            *child_fingerprint = [0xFF; 32]; // claim a fingerprint the bytes don't hash to
        }
        assert_eq!(verify_capability_issuance(&record.body), Err(GovernanceError::IssuanceNotAttenuated));
    }

    #[test] fn a_non_descendant_child_is_rejected() {
        // Embed a child delegated from a DIFFERENT genesis than the parent.
        let (parent, _) = parent_and_child(8);
        let (_other_parent, foreign_child) = {
            // a second, unrelated owned namespace → different genesis
            use crate::meadowcap::create::new_owned_write;
            use crate::meadowcap::delegate::delegate_write;
            use willow25::prelude::{Area, NamespaceSecret, Path, SubspaceSecret, TimeRange};
            let ns = NamespaceSecret::from_bytes(&[99u8; 32]);
            let owner = SubspaceSecret::from_bytes(&[98u8; 32]);
            let root = new_owned_write(&ns, owner.corresponding_subspace_id());
            let leaf = SubspaceSecret::from_bytes(&[97u8;32]).corresponding_subspace_id();
            let area = Area::new(Some(leaf.clone()), Path::from_slices(&[b"content"]).unwrap(),
                TimeRange::new(0u64.into(), Some(u64::MAX.into())));
            (root.clone(), delegate_write(&root, &owner, area, leaf).unwrap())
        };
        let body = Body::CapabilityIssued {
            covering_parent_fingerprint: write_capability_fingerprint(&parent),
            child_fingerprint: write_capability_fingerprint(&foreign_child),
            parent_capability_bytes: OpaqueBytes(encode_capability(&parent)),
            child_capability_bytes: OpaqueBytes(encode_capability(&foreign_child)),
        };
        assert_eq!(verify_capability_issuance(&body), Err(GovernanceError::IssuanceNotAttenuated));
    }

    #[test] fn a_same_genesis_sibling_presented_as_a_child_is_rejected_by_extends() {
        // THE ISOLATING ATTACK, constructed so `extends` is the ONLY failing
        // condition (deleting `extends` would turn this Err into Ok):
        //   - parent_b: root → (subspace None, path /a, full time)  [depth 1]
        //   - sibling_c: root → (subspace Some(c_id), path /a/b, full)  [depth 1]
        // both delegated from the SAME owned genesis `root`.
        // • same_genesis → TRUE  (identical genesis).
        // • fingerprint checks → PASS (genuine caps, genuine fingerprints).
        // • attenuated = parent_b.includes_area(sibling_c.granted_area()) → TRUE:
        //   None-subspace ⊇ Some(c_id); /a is a prefix of /a/b; full ⊇ full.
        // • extends: cd.len()==1 is NOT pd.len()+1==2 → FALSE. Only `extends`
        //   rejects the sibling.
        use crate::meadowcap::create::new_owned_write;
        use crate::meadowcap::delegate::delegate_write;
        use willow25::prelude::{Area, NamespaceSecret, Path, SubspaceSecret, TimeRange};
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let root = new_owned_write(&ns, owner.corresponding_subspace_id());
        let b_id = SubspaceSecret::from_bytes(&[8u8; 32]).corresponding_subspace_id();
        let c_id = SubspaceSecret::from_bytes(&[9u8; 32]).corresponding_subspace_id();
        let full = || TimeRange::new(0u64.into(), Some(u64::MAX.into()));
        // Broad parent: all subspaces (None), path /a — contains the sibling area.
        let parent_area = Area::new(None, Path::from_slices(&[b"a"]).unwrap(), full());
        // Narrower sibling: subspace c_id, path /a/b — inside the parent area.
        let sibling_area = Area::new(Some(c_id.clone()), Path::from_slices(&[b"a", b"b"]).unwrap(), full());
        let parent_b = delegate_write(&root, &owner, parent_area, b_id).unwrap();
        let sibling_c = delegate_write(&root, &owner, sibling_area, c_id).unwrap();

        // Precondition the isolation depends on: the parent's area really does
        // contain the sibling's area (so `attenuated` is TRUE and cannot be the
        // rejecting condition).
        assert!(parent_b.includes_area(&sibling_c.granted_area()),
            "test setup: parent must cover the sibling area so ONLY extends can reject");

        let body = Body::CapabilityIssued {
            covering_parent_fingerprint: write_capability_fingerprint(&parent_b),
            child_fingerprint: write_capability_fingerprint(&sibling_c), // genuine fp of the sibling
            parent_capability_bytes: OpaqueBytes(encode_capability(&parent_b)),
            child_capability_bytes: OpaqueBytes(encode_capability(&sibling_c)),
        };
        assert_eq!(verify_capability_issuance(&body), Err(GovernanceError::IssuanceNotAttenuated),
            "a same-genesis sibling is not an attenuation-descendant — rejected by extends");
    }

    #[test] fn an_undecodable_embedded_capability_yields_capability_error() {
        // Producer for GovernanceError::Capability: garbage embedded bytes fail
        // Slice-1's decode_write_capability_bounded → mapped to Capability(..).
        let (parent, _) = parent_and_child(8);
        let body = Body::CapabilityIssued {
            covering_parent_fingerprint: write_capability_fingerprint(&parent),
            child_fingerprint: [2u8; 32],
            parent_capability_bytes: OpaqueBytes(encode_capability(&parent)),
            child_capability_bytes: OpaqueBytes(vec![0xFF; 10]), // not a capability
        };
        assert!(matches!(verify_capability_issuance(&body), Err(GovernanceError::Capability(_))));
    }
}
```

  Ancestry is a **structural** comparison — verified against crate source, not a stub: `meadowcap::Delegation` derives `PartialEq, Eq` (`meadowcap-0.5.0/src/raw/mod.rs:190`) and has public `area`/`user`/`signature` fields (`:197-204`), so `child.delegations()[..parent.delegations().len()] == *parent.delegations()` is a direct slice `==`; genesis accessors `namespace_key`/`user_key`/`access_mode` (`raw/mod.rs:86/94/78`) and `is_owned` (`write_capability.rs:248`) are all `pub`. No `encode_value`, no truncation constructor. Four negatives (forged fingerprint, non-descendant genesis, same-genesis sibling, undecodable bytes) stay red until the real check lands.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::authorize` — expected failure: `cannot find function verify_capability_issuance`. Implement, re-run; all six tests pass. This is the `Capability`/`IssuanceNotAttenuated`/`SelfAuthorization` producer required by the error-taxonomy no-dead-codes rule.
- [ ] **Commit.** `git add crates/riot-core/src/governance/authorize.rs && git commit -m "feat(governance): attenuation-proof issuance verification, record authorization, migration-fork classifier"`

### Task 9 — Capability lineage + transitive revocation

**Files:** Modify `crates/riot-core/src/governance/lineage.rs`, `crates/riot-core/src/governance/revoke.rs`.

- [ ] **Write the failing test — lineage** (`lineage.rs`):

```rust
//! Capability-lineage forest keyed by the Slice-1 fingerprint (the join key).

use std::collections::{BTreeMap, BTreeSet};
use super::body::Body;
use super::record::{Fingerprint, GovernanceRecordV1};
use super::RecordKind;

#[derive(Debug, Clone, Default)]
pub struct LineageForest { parent_of: BTreeMap<Fingerprint, Fingerprint> }

pub fn build_lineage(records: &[GovernanceRecordV1]) -> LineageForest {
    let mut parent_of = BTreeMap::new();
    for r in records {
        match (&r.kind, &r.body) {
            (RecordKind::CapabilityIssued, Body::CapabilityIssued { covering_parent_fingerprint, child_fingerprint, .. })
            | (RecordKind::CapabilityRenewed, Body::CapabilityRenewed { covering_parent_fingerprint, child_fingerprint, .. }) => {
                parent_of.insert(*child_fingerprint, *covering_parent_fingerprint);
            }
            _ => {}
        }
    }
    LineageForest { parent_of }
}

impl LineageForest {
    pub fn descendants_of(&self, target: Fingerprint) -> BTreeSet<Fingerprint> {
        let mut out = BTreeSet::new(); out.insert(target);
        loop {
            let mut grew = false;
            for (child, parent) in &self.parent_of {
                if out.contains(parent) && out.insert(*child) { grew = true; }
            }
            if !grew { break; }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::three_hop_lineage_records;
    #[test] fn descendants_include_the_subtree_but_not_siblings() {
        let (records, fps) = three_hop_lineage_records();
        let d = build_lineage(&records).descendants_of(fps.a);
        assert!(d.contains(&fps.a) && d.contains(&fps.b) && d.contains(&fps.c));
        assert!(!d.contains(&fps.d_sibling));
    }
}
```

- [ ] **Write the failing test — revocation** (`revoke.rs`):

```rust
//! Transitive revocation + action-chain cutoffs. Revoking a fingerprint removes
//! its whole descendant subtree (revoke wins). Re-issuance mints a NEW
//! fingerprint, so restoration never un-revokes.

use std::collections::BTreeSet;
use super::body::Body;
use super::lineage::build_lineage;
use super::record::{Fingerprint, GovernanceRecordV1};
use super::RecordKind;

pub fn apply_revocations(records: &[GovernanceRecordV1], granted: BTreeSet<Fingerprint>)
    -> (BTreeSet<Fingerprint>, BTreeSet<Fingerprint>)
{
    let forest = build_lineage(records);
    let mut revoked = BTreeSet::new();
    for r in records {
        if r.kind == RecordKind::CapabilityRevoked {
            if let Body::CapabilityRevoked { target_fingerprint, .. } = &r.body {
                revoked.extend(forest.descendants_of(*target_fingerprint));
            }
        }
    }
    let active = granted.difference(&revoked).copied().collect();
    (active, revoked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::{fingerprint_of_issued, revoke_record,
        three_hop_lineage_records};

    // PURE unit tests on apply_revocations — NO dependency on the evaluator
    // (that integration lives in Task 10). This module compiles at Task 9's
    // boundary because it imports only lineage + record.
    #[test] fn revoking_mid_chain_invalidates_all_descendants() {
        let (records, fps) = three_hop_lineage_records();
        let mut recs = records.clone();
        recs.push(revoke_record(fps.a));
        // granted = the whole lineage (as the evaluator's grant fold would yield).
        let granted: BTreeSet<Fingerprint> = [fps.a, fps.b, fps.c, fps.d_sibling].into_iter().collect();
        let (active, revoked) = apply_revocations(&recs, granted);
        assert!(revoked.contains(&fps.a) && revoked.contains(&fps.b) && revoked.contains(&fps.c));
        assert!(!active.contains(&fps.a) && !active.contains(&fps.b) && !active.contains(&fps.c));
        assert!(active.contains(&fps.d_sibling), "sibling survives");
    }
    #[test] fn revoke_wins_over_a_concurrent_grant() {
        let (records, fps) = three_hop_lineage_records();
        let mut recs = records.clone();
        recs.push(revoke_record(fps.b));
        let granted: BTreeSet<Fingerprint> = [fps.b].into_iter().collect();
        let (active, _) = apply_revocations(&recs, granted);
        assert!(!active.contains(&fps.b), "revoke wins over the concurrent grant");
    }
    #[test] fn a_capability_outside_the_revoked_subtree_is_untouched() {
        let (records, fps) = three_hop_lineage_records();
        let mut recs = records.clone();
        recs.push(revoke_record(fps.c)); // revoke a leaf
        let granted: BTreeSet<Fingerprint> = [fps.a, fps.c].into_iter().collect();
        let (active, revoked) = apply_revocations(&recs, granted);
        assert!(revoked.contains(&fps.c) && !revoked.contains(&fps.a));
        assert!(active.contains(&fps.a), "ancestor of the revoked leaf is untouched");
        let _ = fingerprint_of_issued;
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::lineage governance::revoke::tests` — expected failure: `cannot find function build_lineage` / `apply_revocations`. Implement both, re-run; the lineage test and all three `apply_revocations` unit tests pass. (The evaluator-level revocation integration — `renewal_after_revoke`, forged-issuance-never-active — lives in Task 10, which depends on `evaluate`.)
- [ ] **Commit.** `git add crates/riot-core/src/governance/lineage.rs crates/riot-core/src/governance/revoke.rs && git commit -m "feat(governance): capability lineage and transitive revocation (pure apply_revocations)"`

### Task 10 — Deterministic evaluator → `PolicySnapshot`

**Files:** Modify `crates/riot-core/src/governance/evaluator.rs`.

Grant fold routes issuances through `verify_capability_issuance` (Task 8) — a forged issuance never becomes active. Restrictive intersect reducer for concurrent role restrictions. `now: Option<u64>` (quarantine wiring completed in Task 12; here the fold is unconditional so the Task-10 tests pass, then Task 12 tightens the clock-block).

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Deterministic policy evaluator. `(journal, now) -> PolicySnapshot`.
//! No SystemTime/RNG/HashMap-order dependence — BTree* + sorted iteration only.

use std::collections::{BTreeMap, BTreeSet};
use super::authorize::verify_capability_issuance;
use super::body::Body;
use super::frontier::{frontier_hash, frontier_of, topological_reduce};
use super::record::{Fingerprint, GovernanceRecordV1};
use super::{actor, RecordKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySnapshot {
    pub frontier_hash: [u8; 32],
    pub active_fingerprints: BTreeSet<Fingerprint>,
    pub revoked: BTreeSet<Fingerprint>,
    pub actor_bindings: BTreeMap<[u8; 32], BTreeSet<[u8; 32]>>,
    pub action_heads: BTreeMap<([u8; 32], [u8; 32]), [u8; 32]>,
}

pub fn evaluate(records: &[GovernanceRecordV1], _now: Option<u64>) -> PolicySnapshot {
    let (accepted, _pending) = topological_reduce(records);
    let mut active: BTreeSet<Fingerprint> = BTreeSet::new();
    for r in &accepted {
        match (&r.kind, &r.body) {
            (RecordKind::CapabilityIssued, Body::CapabilityIssued { child_fingerprint, .. })
            | (RecordKind::CapabilityRenewed, Body::CapabilityRenewed { child_fingerprint, .. }) => {
                // A forged/non-attenuated issuance never becomes active.
                if verify_capability_issuance(&r.body).is_ok() {
                    active.insert(*child_fingerprint);
                }
            }
            _ => {}
        }
    }
    let (active, revoked) = super::revoke::apply_revocations(&accepted, active);
    PolicySnapshot {
        frontier_hash: frontier_hash(&frontier_of(&accepted)),
        active_fingerprints: active,
        revoked,
        actor_bindings: actor::actor_bindings(&accepted),
        action_heads: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::{genesis_record, issued_record, renewal_after_revoke,
        revoke_record, seeded_journal, three_hop_lineage_records_with_sibling_active};

    #[test] fn evaluate_is_deterministic_over_shuffled_input() {
        let journal = seeded_journal(50);
        let now = Some(2_000_000_000_000);
        let baseline = evaluate(&journal, now);
        for seed in 0usize..8 {
            let mut s = journal.clone();
            s.rotate_left((seed * 7) % s.len());
            assert_eq!(evaluate(&s, now), baseline, "shuffle {seed} changed the snapshot");
        }
    }
    #[test] fn evaluate_reads_no_wall_clock() {
        let journal = seeded_journal(20);
        assert_eq!(evaluate(&journal, Some(1_777_000_000_000)), evaluate(&journal, Some(1_777_000_000_000)));
    }
    #[test] fn a_forged_issuance_never_becomes_active() {
        let mut forged = issued_record([9u8;32], 8);
        if let Body::CapabilityIssued { child_fingerprint, .. } = &mut forged.body { *child_fingerprint = [0xFF;32]; }
        let fp = if let Body::CapabilityIssued { child_fingerprint, .. } = &forged.body { *child_fingerprint } else { unreachable!() };
        let journal = vec![genesis_record([9u8;32]), forged];
        assert!(!evaluate(&journal, Some(2_000_000_000_000)).active_fingerprints.contains(&fp));
    }
    // Evaluator-level revocation integration (moved here from Task 9 — it needs
    // `evaluate`, which is this task's output).
    #[test] fn revoking_mid_chain_removes_descendants_from_the_active_snapshot() {
        let (mut records, fps) = three_hop_lineage_records_with_sibling_active();
        records.push(revoke_record(fps.a));
        let s = evaluate(&records, Some(2_000_000_000_000));
        assert!(!s.active_fingerprints.contains(&fps.a) && !s.active_fingerprints.contains(&fps.b)
            && !s.active_fingerprints.contains(&fps.c), "transitive descendants gone");
        assert!(s.active_fingerprints.contains(&fps.d_sibling), "sibling survives");
    }
    #[test] fn a_renewal_after_revocation_mints_a_fresh_active_fingerprint() {
        let (records, old_fp, new_fp) = renewal_after_revoke();
        let s = evaluate(&records, Some(2_000_000_000_000));
        assert!(s.revoked.contains(&old_fp) && s.active_fingerprints.contains(&new_fp));
        assert!(!s.active_fingerprints.contains(&old_fp), "revocation is irreversible");
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::evaluator` — expected failure: `cannot find function evaluate`. Implement, re-run; all five tests pass (three pure-evaluator + two revocation-integration). This carries the master-plan Phase-2 exit assertion (mid-chain revocation transitively invalidates descendants).
- [ ] **Commit.** `git add crates/riot-core/src/governance/evaluator.rs && git commit -m "feat(governance): deterministic evaluator, attenuation-checked grant fold"`

### Task 11 — Action-chain cutoff predicate + concurrent-branch classification

**Files:** Modify `crates/riot-core/src/governance/revoke.rs`.

Adds the cutoff-map predicate `is_action_active` (ancestor-or-equal of the pinned head) and the **forked-branch** case the gate named: a branch not ancestral to the cutoff head is audit-only (spec ~547–548, testing line ~1205). Arrival-order determinism over shuffled inputs.

- [ ] **Write the failing test.** Append to `revoke.rs`:

```rust
use std::collections::BTreeMap;
use super::action::{action_hash, ActionReceiptV1};

pub fn is_action_active(
    action: &ActionReceiptV1,
    cutoffs: &BTreeMap<([u8; 32], [u8; 32]), [u8; 32]>,
    chain: &BTreeMap<[u8; 32], Option<[u8; 32]>>,
) -> bool {
    let Some(head) = cutoffs.get(&(action.actor_id, action.receiver)) else { return false };
    let target = action_hash(action);
    let mut cursor = Some(*head);
    while let Some(cur) = cursor {
        if cur == target { return true; }
        cursor = chain.get(&cur).copied().flatten();
    }
    false
}

#[cfg(test)]
mod cutoff_tests {
    use super::*;
    use crate::governance::action::action_receipt_chain;

    fn chain_map(receipts: &[ActionReceiptV1]) -> BTreeMap<[u8; 32], Option<[u8; 32]>> {
        receipts.iter().map(|r| (action_hash(r), r.previous_action_hash)).collect()
    }

    #[test] fn cutoff_keeps_ancestors_and_drops_post_cutoff_descendants() {
        let (r, _) = action_receipt_chain(3); // #0 -> #1 -> #2
        let chain = chain_map(&r);
        let mut cutoffs = BTreeMap::new();
        cutoffs.insert((r[0].actor_id, r[0].receiver), action_hash(&r[1])); // head at #1
        assert!(is_action_active(&r[0], &cutoffs, &chain));
        assert!(is_action_active(&r[1], &cutoffs, &chain));
        assert!(!is_action_active(&r[2], &cutoffs, &chain), "post-cutoff descendant audit-only");
    }

    #[test] fn a_forked_branch_not_ancestral_to_the_head_is_audit_only() {
        // #0 -> #1 (head). A concurrent #1' also chains from #0 but is NOT the
        // head and not an ancestor of it → audit-only.
        let (mut r, _) = action_receipt_chain(2);
        let mut fork = r[1].clone();
        fork.receiver = r[1].receiver; fork.actor_sequence = 1;
        fork.previous_action_hash = Some(action_hash(&r[0]));
        fork.entry_id = [0x77; 32]; // a different action
        let chain = chain_map(&[r[0].clone(), r[1].clone(), fork.clone()]);
        let mut cutoffs = BTreeMap::new();
        cutoffs.insert((r[0].actor_id, r[0].receiver), action_hash(&r[1]));
        assert!(!is_action_active(&fork, &cutoffs, &chain), "non-ancestral fork branch is audit-only");
        let _ = &mut r;
    }

    #[test] fn no_cutoff_entry_means_no_active_action() {
        let (r, _) = action_receipt_chain(2);
        assert!(!is_action_active(&r[0], &BTreeMap::new(), &chain_map(&r)));
    }

    #[test] fn classification_is_arrival_order_independent() {
        let (r, _) = action_receipt_chain(3);
        let mut cutoffs = BTreeMap::new();
        cutoffs.insert((r[0].actor_id, r[0].receiver), action_hash(&r[1]));
        let baseline: Vec<bool> = r.iter().map(|x| is_action_active(x, &cutoffs, &chain_map(&r))).collect();
        for seed in 0usize..8 {
            let mut s = r.clone(); s.rotate_left(seed % s.len());
            let got: Vec<bool> = r.iter().map(|x| is_action_active(x, &cutoffs, &chain_map(&s))).collect();
            assert_eq!(got, baseline, "arrival order {seed} changed the cutoff classification");
        }
    }
}
```

  Read capabilities in the revoked set `D` have no grandfathering: they are dropped by `apply_revocations` (Task 9) the moment the revocation is in the frontier, independent of any action cutoff; the cutoff predicate governs only *write* actions in `D`.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::revoke::cutoff_tests` — expected failure: `cannot find function is_action_active`. Implement, re-run; all four tests pass.
- [ ] **Commit.** `git add crates/riot-core/src/governance/revoke.rs && git commit -m "feat(governance): action-chain cutoff predicate, forked-branch classification, arrival-order determinism"`

### Task 12 — Future-clock quarantine and clock-block on authority changes

**Files:** Modify `crates/riot-core/src/governance/evaluator.rs`.

Records whose `created_display_micros` exceed `now + 10 min` (TAI/J2000 µs) are quarantined; a `None` clock blocks issuance/renewal/revocation classification — but ordering still comes only from the DAG, so evaluation stays deterministic.

- [ ] **Write the failing test.** Change `evaluate` to consume the clock and extend the tests:

```rust
pub fn evaluate(records: &[GovernanceRecordV1], now: Option<u64>) -> PolicySnapshot {
    let (accepted, _pending) = topological_reduce(records); // ordering never uses the clock
    const TEN_MIN: u64 = 10 * 60 * 1_000_000;
    let horizon = now.map(|t| t.saturating_add(TEN_MIN));
    let in_time: Vec<GovernanceRecordV1> = accepted.iter()
        .filter(|r| horizon.map_or(true, |h| r.created_display_micros <= h))
        .cloned().collect();
    let clock_ok = now.is_some();
    let mut active = BTreeSet::new();
    if clock_ok {
        for r in &in_time {
            if let (RecordKind::CapabilityIssued | RecordKind::CapabilityRenewed, true)
                = (&r.kind, verify_capability_issuance(&r.body).is_ok())
            {
                if let Body::CapabilityIssued { child_fingerprint, .. }
                    | Body::CapabilityRenewed { child_fingerprint, .. } = &r.body {
                    active.insert(*child_fingerprint);
                }
            }
        }
    }
    // Restrictive intersect for concurrent role restrictions (design "concurrent
    // role restrictions intersect"). Applied BEFORE revocation so a revoke can
    // still further-restrict the result.
    let active = if clock_ok { apply_role_restrictions(&in_time, active) } else { active };
    let (active, revoked) = if clock_ok {
        super::revoke::apply_revocations(&in_time, active)
    } else { (active, BTreeSet::new()) };
    PolicySnapshot {
        frontier_hash: frontier_hash(&frontier_of(&accepted)),
        active_fingerprints: active, revoked,
        actor_bindings: actor::actor_bindings(&in_time),
        action_heads: BTreeMap::new(),
    }
}

/// Transitive parent-closure ancestors of `id` within the accepted set.
fn ancestors_of(id: &[u8; 32], by_id: &BTreeMap<[u8; 32], &GovernanceRecordV1>) -> BTreeSet<[u8; 32]> {
    let mut out = BTreeSet::new();
    let mut stack: Vec<[u8; 32]> = by_id.get(id).map(|r| r.parents.clone()).unwrap_or_default();
    while let Some(p) = stack.pop() {
        if out.insert(p) {
            if let Some(rec) = by_id.get(&p) { stack.extend(rec.parents.iter().copied()); }
        }
    }
    out
}

/// Concurrent role decisions for the same `role_instance_id` reduce by
/// INTERSECTION of their granted fingerprints (most restrictive wins). A role
/// decision that is an ancestor of another for the same role is SUPERSEDED and
/// excluded, so a linear chain keeps only its latest (descendant) decision while
/// genuinely-concurrent decisions (neither an ancestor of the other) intersect.
/// Any of a role's fingerprints that is not in the surviving intersection is
/// removed from `active`. Deterministic: all iteration is over `BTree*`.
fn apply_role_restrictions(
    accepted: &[GovernanceRecordV1],
    mut active: BTreeSet<Fingerprint>,
) -> BTreeSet<Fingerprint> {
    use std::collections::BTreeMap;
    use super::body::Body;
    use super::record::record_id;

    let by_id: BTreeMap<[u8; 32], &GovernanceRecordV1> =
        accepted.iter().map(|r| (record_id(r), r)).collect();

    // role_instance_id -> Vec<(record_id, granted_fingerprints)>
    let mut by_role: BTreeMap<[u8; 32], Vec<([u8; 32], Vec<Fingerprint>)>> = BTreeMap::new();
    for r in accepted {
        if let (RecordKind::RoleDecision, Body::RoleDecision { role_instance_id, granted_fingerprints, .. }) = (&r.kind, &r.body) {
            by_role.entry(*role_instance_id).or_default().push((record_id(r), granted_fingerprints.clone()));
        }
    }

    for (_role, decisions) in &by_role {
        let ids: BTreeSet<[u8; 32]> = decisions.iter().map(|(id, _)| *id).collect();
        // "Frontier" decisions for this role: not an ancestor of any other
        // decision for the same role.
        let frontier: Vec<&([u8; 32], Vec<Fingerprint>)> = decisions.iter()
            .filter(|(id, _)| !decisions.iter().any(|(other, _)| other != id
                && ancestors_of(other, &by_id).contains(id)))
            .collect();
        if frontier.is_empty() { continue; }
        // union of all this role's granted fps, and intersection over the frontier.
        let union: BTreeSet<Fingerprint> = decisions.iter().flat_map(|(_, g)| g.iter().copied()).collect();
        let mut effective: BTreeSet<Fingerprint> = frontier[0].1.iter().copied().collect();
        for (_, g) in frontier.iter().skip(1) {
            let g: BTreeSet<Fingerprint> = g.iter().copied().collect();
            effective = effective.intersection(&g).copied().collect();
        }
        // Drop any role fp that survived the grant fold but is excluded by a
        // concurrent restriction.
        for fp in union.difference(&effective) { active.remove(fp); }
        let _ = &ids;
    }
    active
}
```

```rust
#[test] fn a_record_more_than_ten_minutes_ahead_is_quarantined() {
    use crate::willow::tai_j2000_micros_from_unix_seconds as tai;
    let now = tai(1_800_000_000).unwrap();
    let mut journal = crate::governance::test_support::seeded_journal(4);
    let idx = journal.iter().position(|r| r.kind == crate::governance::RecordKind::CapabilityIssued).unwrap();
    let fp = if let crate::governance::body::Body::CapabilityIssued { child_fingerprint, .. } = &journal[idx].body { *child_fingerprint } else { unreachable!() };
    journal[idx].created_display_micros = now + 10 * 60 * 1_000_000 + 1;
    assert!(!evaluate(&journal, Some(now)).active_fingerprints.contains(&fp), "future-dated quarantined");
    journal[idx].created_display_micros = now + 10 * 60 * 1_000_000; // exactly 10 min inclusive
    assert!(evaluate(&journal, Some(now)).active_fingerprints.contains(&fp));
}
#[test] fn an_unavailable_clock_blocks_activation_but_not_dag_ordering() {
    let journal = crate::governance::test_support::seeded_journal(6);
    let with = evaluate(&journal, Some(2_000_000_000_000));
    let without = evaluate(&journal, None);
    assert!(without.active_fingerprints.is_empty());
    assert_eq!(without.frontier_hash, with.frontier_hash, "ordering never used the clock");
}
#[test] fn concurrent_role_restrictions_intersect_in_the_evaluator() {
    // Two genuinely-concurrent RoleDecision records (both parented on genesis,
    // neither an ancestor of the other) for the same role: decision_1 grants
    // {X, Y}, decision_2 grants {X}. X and Y are both really issued (active after
    // the grant fold); the intersect must drop Y and keep X.
    use crate::governance::test_support::two_concurrent_role_restrictions;
    let (records, expected_survivor /* = X */) = two_concurrent_role_restrictions();
    let snap = evaluate(&records, Some(2_000_000_000_000));
    assert!(snap.active_fingerprints.contains(&expected_survivor), "the intersection fp X survives");
    // Exactly one of the role's two candidate fingerprints remains.
    assert_eq!(
        snap.active_fingerprints.iter().filter(|f| crate::governance::test_support::is_role_fp(f)).count(),
        1,
        "concurrent restrictions collapse to the intersection, never the union"
    );
}
```

  The reducer is REAL code in `evaluate` above (`apply_role_restrictions`), not test-only. The Task-10 tests already pass `Some(...)`, so no call-site churn beyond this file. The repository's `snapshot` (Task 13) will call `evaluate(&journal, Some(now_micros))`.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::evaluator` — expected failure: the quarantine tests fail against the Task-10 unconditional fold, and `concurrent_role_restrictions_intersect_in_the_evaluator` fails until `apply_role_restrictions` is wired in. Tighten `evaluate` + add the reducer, re-run; the quarantine, clock-block, and intersect tests pass and the Task-10 determinism tests still pass.
- [ ] **Commit.** `git add crates/riot-core/src/governance/evaluator.rs && git commit -m "feat(governance): future-clock quarantine and clock-block on authority changes"`

### Task 13 — Durable repository schema (migration v3) + `AuthorityRepository`

**Files:** Modify `crates/riot-core/src/store/schema.rs`, `crates/riot-core/src/store/mod.rs`, `crates/riot-core/src/governance/repository.rs`.

Adds `MIGRATION_THREE` (journal + four index tables), bumps `CURRENT_SCHEMA_VERSION` to `3`, gates `validate_governance_structure` by `found >= 3`. `ingest` writes the journal row **and every derived index row** (all five tables) in **one** `write_transaction` (*"content and policy indexes cannot commit independently"*). Indexed read paths query only their index table — they never scan the journal, satisfying *"no app bridge call scans the unbounded journal"*. **`snapshot()` legitimately loads the full journal because it is the checkpoint rebuild the design mandates (*"Startup verifies the journal... and rebuilds every index"*), not a per-operation bridge call**; bridge-style lookups use `records_for_target`/`revocations_for`/`action_head_for`.

- [ ] **Write the failing test — schema** (`store/schema.rs`). Add the DDL consts (frozen-style: `STRICT`, `CHECK` length guards, `WITHOUT ROWID` where blob-keyed):

```rust
const GOVERNANCE_JOURNAL_SQL: &str = "CREATE TABLE governance_journal (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    record_id BLOB NOT NULL CHECK (length(record_id) = 32),
    kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 21),
    actor_id BLOB NOT NULL CHECK (length(actor_id) = 32),
    sequence_be BLOB NOT NULL CHECK (length(sequence_be) = 8),
    authorizing_fingerprint BLOB NOT NULL CHECK (length(authorizing_fingerprint) = 32),
    record_bytes BLOB NOT NULL CHECK (length(record_bytes) > 0),
    accepted_generation INTEGER NOT NULL CHECK (accepted_generation >= 0),
    PRIMARY KEY(namespace_id, record_id)
) STRICT, WITHOUT ROWID";
const GOVERNANCE_BY_TARGET_SQL: &str = "CREATE TABLE governance_by_target (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    kind INTEGER NOT NULL CHECK (kind BETWEEN 0 AND 21),
    target_id BLOB NOT NULL CHECK (length(target_id) = 32),
    record_id BLOB NOT NULL CHECK (length(record_id) = 32),
    PRIMARY KEY(namespace_id, kind, target_id, record_id),
    FOREIGN KEY(namespace_id, record_id) REFERENCES governance_journal(namespace_id, record_id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID";
const CAPABILITY_LINEAGE_SQL: &str = "CREATE TABLE capability_lineage (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    child_fingerprint BLOB NOT NULL CHECK (length(child_fingerprint) = 32),
    parent_fingerprint BLOB NOT NULL CHECK (length(parent_fingerprint) = 32),
    PRIMARY KEY(namespace_id, child_fingerprint)
) STRICT, WITHOUT ROWID";
const REVOCATION_INDEX_SQL: &str = "CREATE TABLE revocation_index (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    target_fingerprint BLOB NOT NULL CHECK (length(target_fingerprint) = 32),
    record_id BLOB NOT NULL CHECK (length(record_id) = 32),
    PRIMARY KEY(namespace_id, target_fingerprint, record_id),
    FOREIGN KEY(namespace_id, record_id) REFERENCES governance_journal(namespace_id, record_id) ON DELETE CASCADE
) STRICT, WITHOUT ROWID";
const ACTION_HEADS_SQL: &str = "CREATE TABLE action_heads (
    namespace_id BLOB NOT NULL CHECK (length(namespace_id) = 32),
    actor_id BLOB NOT NULL CHECK (length(actor_id) = 32),
    receiver_id BLOB NOT NULL CHECK (length(receiver_id) = 32),
    action_hash BLOB NOT NULL CHECK (length(action_hash) = 32),
    sequence_be BLOB NOT NULL CHECK (length(sequence_be) = 8),
    PRIMARY KEY(namespace_id, actor_id, receiver_id)
) STRICT, WITHOUT ROWID";

const MIGRATION_THREE: &str = "
    CREATE TABLE governance_journal ( /* …GOVERNANCE_JOURNAL_SQL… */ ) STRICT, WITHOUT ROWID;
    CREATE TABLE governance_by_target ( /* … */ ) STRICT, WITHOUT ROWID;
    CREATE TABLE capability_lineage ( /* … */ ) STRICT, WITHOUT ROWID;
    CREATE TABLE revocation_index ( /* … */ ) STRICT, WITHOUT ROWID;
    CREATE TABLE action_heads ( /* … */ ) STRICT, WITHOUT ROWID;
    INSERT INTO schema_migrations(version) VALUES (3);
    PRAGMA user_version = 3;
";
```

```rust
pub const CURRENT_SCHEMA_VERSION: u32 = 3; // was 2

// migrate(), after the `found < 2` block:
if found < 3 {
    transaction.execute_batch(MIGRATION_THREE).map_err(|_| DatabaseError::MigrationFailed)?;
}
// validate_structure(), after the `found >= 2` evidence gate:
if found >= 3 { validate_governance_structure(connection)?; }

fn validate_governance_structure(connection: &Connection) -> Result<(), DatabaseError> {
    for (t, sql, without_rowid) in [
        ("governance_journal", GOVERNANCE_JOURNAL_SQL, true),
        ("governance_by_target", GOVERNANCE_BY_TARGET_SQL, true),
        ("capability_lineage", CAPABILITY_LINEAGE_SQL, true),
        ("revocation_index", REVOCATION_INDEX_SQL, true),
        ("action_heads", ACTION_HEADS_SQL, true),
    ] { validate_schema_definition(connection, t, sql, without_rowid)?; }
    Ok(())
}

#[test]
fn fresh_database_is_governance_schema_v3() {
    let path = std::env::temp_dir().join(format!("riot-gov-schema-{}.db", std::process::id()));
    let db = crate::store::RiotDatabase::open(&path, crate::store::DatabaseConfig::default()).unwrap();
    assert_eq!(db.schema_version().unwrap(), 3);
    let _ = std::fs::remove_file(&path);
}
```

  Add to `store/mod.rs`: `#[cfg(feature = "sqlite")] pub(crate) use database::{map_sqlite_error, WriteEstimate};`.

- [ ] **Write the failing test — repository** (`governance/repository.rs`):

```rust
//! Durable `AuthorityRepository`: governance journal + populated indexes.
//! Mirrors `store::evidence::EvidenceRepository`. Errors are the non-gated
//! `GovernanceError` so a `--no-default-features` (wasm) build compiles with
//! the Memory oracle only.

use std::sync::Mutex;
use super::body::{target_id_of, Body};
use super::evaluator::{evaluate, PolicySnapshot};
use super::record::{encode_record, record_id, GovernanceRecordV1};
use super::{Fingerprint, GovernanceError, RecordKind};
#[cfg(feature = "sqlite")]
use crate::store::{map_sqlite_error, RiotDatabase, WriteEstimate};
#[cfg(feature = "sqlite")]
use rusqlite::OptionalExtension; // for `.optional()` in action_head_for

pub enum AuthorityRepository {
    Memory(Mutex<Vec<GovernanceRecordV1>>),
    #[cfg(feature = "sqlite")]
    Sqlite(RiotDatabase),
}

impl AuthorityRepository {
    pub fn memory() -> Self { Self::Memory(Mutex::new(Vec::new())) }
    #[cfg(feature = "sqlite")]
    pub fn sqlite(database: RiotDatabase) -> Self { Self::Sqlite(database) }

    pub fn ingest(&self, record: &GovernanceRecordV1) -> Result<(), GovernanceError> {
        match self {
            Self::Memory(v) => { v.lock().map_err(|_| GovernanceError::Storage)?.push(record.clone()); Ok(()) }
            #[cfg(feature = "sqlite")]
            Self::Sqlite(db) => {
                let bytes = encode_record(record);
                let rid = record_id(record);
                let target = target_id_of(record.kind, &record.body, &record.actor_id);
                // HOIST all GovernanceError-returning parsing OUT of the closure:
                // `write_transaction`'s closure must return `Result<_, DatabaseError>`
                // and no `From<GovernanceError>` exists (the store must not depend on
                // governance). Decode the action receipt here (pure parse, no
                // transaction needed) and pass the precomputed action-head row in.
                let action_head: Option<([u8; 32], [u8; 32], [u8; 32], [u8; 8])> = match &record.body {
                    Body::ActionReceipt { receipt } => {
                        let ar = super::action::decode_receipt(&receipt.0)?; // GovernanceError, OK outside the closure
                        Some((ar.actor_id, ar.receiver, super::action::action_hash(&ar), ar.actor_sequence.to_be_bytes()))
                    }
                    _ => None,
                };
                // The closure now returns ONLY Result<_, DatabaseError>: every `?`
                // is a rusqlite error mapped via map_sqlite_error.
                db.write_transaction(WriteEstimate::new(bytes.len(), 128), |tx| {
                    tx.execute("INSERT OR IGNORE INTO governance_journal(namespace_id, record_id, kind, actor_id,
                        sequence_be, authorizing_fingerprint, record_bytes, accepted_generation)
                        VALUES (?1,?2,?3,?4,?5,?6,?7,(SELECT generation FROM database_meta WHERE singleton=1))",
                        rusqlite::params![&record.namespace[..], &rid[..], record.kind.tag() as i64,
                            &record.actor_id[..], &record.sequence.to_be_bytes()[..],
                            &record.authorizing_fingerprint[..], &bytes[..]]).map_err(map_sqlite_error)?;
                    tx.execute("INSERT OR IGNORE INTO governance_by_target(namespace_id, kind, target_id, record_id) VALUES (?1,?2,?3,?4)",
                        rusqlite::params![&record.namespace[..], record.kind.tag() as i64, &target[..], &rid[..]]).map_err(map_sqlite_error)?;
                    match &record.body {
                        Body::CapabilityIssued { covering_parent_fingerprint, child_fingerprint, .. }
                        | Body::CapabilityRenewed { covering_parent_fingerprint, child_fingerprint, .. } => {
                            tx.execute("INSERT OR IGNORE INTO capability_lineage(namespace_id, child_fingerprint, parent_fingerprint) VALUES (?1,?2,?3)",
                                rusqlite::params![&record.namespace[..], &child_fingerprint[..], &covering_parent_fingerprint[..]]).map_err(map_sqlite_error)?;
                        }
                        Body::CapabilityRevoked { target_fingerprint, .. } => {
                            tx.execute("INSERT OR IGNORE INTO revocation_index(namespace_id, target_fingerprint, record_id) VALUES (?1,?2,?3)",
                                rusqlite::params![&record.namespace[..], &target_fingerprint[..], &rid[..]]).map_err(map_sqlite_error)?;
                        }
                        _ => {}
                    }
                    if let Some((actor_id, receiver_id, ahash, seq_be)) = &action_head {
                        tx.execute("INSERT OR REPLACE INTO action_heads(namespace_id, actor_id, receiver_id, action_hash, sequence_be) VALUES (?1,?2,?3,?4,?5)",
                            rusqlite::params![&record.namespace[..], &actor_id[..], &receiver_id[..], &ahash[..], &seq_be[..]]).map_err(map_sqlite_error)?;
                    }
                    Ok(())
                }).map_err(|_| GovernanceError::Storage)
            }
        }
    }

    /// Indexed lookup — queries ONLY `governance_by_target`, never the journal.
    #[cfg(feature = "sqlite")]
    pub fn records_for_target(&self, kind: RecordKind, target: &[u8; 32]) -> Result<Vec<[u8; 32]>, GovernanceError> {
        match self {
            Self::Sqlite(db) => db.read_connection(|c| {
                let mut s = c.prepare("SELECT record_id FROM governance_by_target WHERE kind=?1 AND target_id=?2 ORDER BY record_id").map_err(map_sqlite_error)?;
                let rows = s.query_map(rusqlite::params![kind.tag() as i64, &target[..]], |r| r.get::<_, Vec<u8>>(0)).map_err(map_sqlite_error)?;
                let mut out = Vec::new();
                for row in rows { let b = row.map_err(map_sqlite_error)?; out.push(b.try_into().map_err(|_| crate::store::DatabaseError::CorruptDatabase)?); }
                Ok(out)
            }).map_err(|_| GovernanceError::Storage),
            _ => Ok(Vec::new()),
        }
    }

    /// Indexed lookup — queries ONLY `revocation_index`. Returns the revocation
    /// record ids targeting `fingerprint`.
    #[cfg(feature = "sqlite")]
    pub fn revocations_for(&self, fingerprint: &[u8; 32]) -> Result<Vec<[u8; 32]>, GovernanceError> {
        match self {
            Self::Sqlite(db) => db.read_connection(|c| {
                let mut s = c.prepare("SELECT record_id FROM revocation_index WHERE target_fingerprint=?1 ORDER BY record_id").map_err(map_sqlite_error)?;
                let rows = s.query_map(rusqlite::params![&fingerprint[..]], |r| r.get::<_, Vec<u8>>(0)).map_err(map_sqlite_error)?;
                let mut out = Vec::new();
                for row in rows { out.push(row.map_err(map_sqlite_error)?.try_into().map_err(|_| crate::store::DatabaseError::CorruptDatabase)?); }
                Ok(out)
            }).map_err(|_| GovernanceError::Storage),
            _ => Ok(Vec::new()),
        }
    }

    /// Indexed lookup — queries ONLY `action_heads`. Returns the current action
    /// head hash for one (actor, receiver), or `None`.
    #[cfg(feature = "sqlite")]
    pub fn action_head_for(&self, actor: &[u8; 32], receiver: &[u8; 32]) -> Result<Option<[u8; 32]>, GovernanceError> {
        match self {
            Self::Sqlite(db) => db.read_connection(|c| {
                let head: Option<Vec<u8>> = c.query_row(
                    "SELECT action_hash FROM action_heads WHERE actor_id=?1 AND receiver_id=?2",
                    rusqlite::params![&actor[..], &receiver[..]], |r| r.get(0),
                ).optional().map_err(map_sqlite_error)?;
                match head {
                    Some(bytes) => Ok(Some(bytes.try_into().map_err(|_| crate::store::DatabaseError::CorruptDatabase)?)),
                    None => Ok(None),
                }
            }).map_err(|_| GovernanceError::Storage),
            _ => Ok(None),
        }
    }

    pub fn load_journal(&self) -> Result<Vec<GovernanceRecordV1>, GovernanceError> {
        let mut records = match self {
            Self::Memory(v) => v.lock().map_err(|_| GovernanceError::Storage)?.clone(),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(db) => db.read_connection(|c| {
                let mut s = c.prepare("SELECT record_bytes FROM governance_journal ORDER BY record_id").map_err(map_sqlite_error)?;
                let rows = s.query_map([], |r| r.get::<_, Vec<u8>>(0)).map_err(map_sqlite_error)?;
                let mut out = Vec::new();
                for row in rows { out.push(super::record::decode_record(&row.map_err(map_sqlite_error)?).map_err(|_| crate::store::DatabaseError::CorruptDatabase)?); }
                Ok(out)
            }).map_err(|_| GovernanceError::Storage)?,
        };
        records.sort_by_key(record_id);
        Ok(records)
    }

    /// Checkpoint rebuild (design "rebuilds every index"): loads the journal.
    /// This is NOT a per-operation bridge call; those use the indexed reads.
    pub fn snapshot(&self, now_micros: u64) -> Result<PolicySnapshot, GovernanceError> {
        Ok(evaluate(&self.load_journal()?, Some(now_micros)))
    }
}

// Memory-variant tests need NO sqlite feature (wasm-shape): this is the
// `Storage`-variant producer test.
#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::governance::test_support::genesis_record;

    #[test] fn a_poisoned_memory_mutex_yields_a_storage_error() {
        // Producer for GovernanceError::Storage: poison the internal Mutex by
        // panicking while holding the guard, then a subsequent lock() is Err.
        let repo = AuthorityRepository::memory();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let AuthorityRepository::Memory(m) = &repo {
                let _g = m.lock().unwrap();
                panic!("poison the lock");
            }
        }));
        assert_eq!(repo.ingest(&genesis_record([9u8;32])), Err(GovernanceError::Storage));
    }
}

// Sqlite tests require the durable store; gate the module so
// `--no-default-features` compiles without naming sqlite-only types.
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests {
    use super::*;
    use crate::governance::test_support::{action_receipt_record, fingerprint_of_issued,
        genesis_record, issued_record, revoke_record};

    fn temp_db() -> (RiotDatabase, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("riot-gov-repo-{}-{}.db", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        (RiotDatabase::open(&path, crate::store::DatabaseConfig::default()).unwrap(), path)
    }

    #[test] fn ingest_populates_all_five_index_tables_and_indexed_reads_return_them() {
        let (db, path) = temp_db();
        let repo = AuthorityRepository::sqlite(db);
        let issued = issued_record([9u8;32], 8);
        let child = fingerprint_of_issued(&issued);
        // A revocation of the issued child, and one action receipt for its actor.
        let revoke = revoke_record(child);
        let (receipt_record, actor, receiver) = action_receipt_record([9u8;32]);
        repo.ingest(&genesis_record([9u8;32])).unwrap();
        repo.ingest(&issued).unwrap();
        repo.ingest(&revoke).unwrap();
        repo.ingest(&receipt_record).unwrap();

        // journal
        assert_eq!(repo.load_journal().unwrap().len(), 4);
        // governance_by_target (issuance target = child fingerprint)
        assert_eq!(repo.records_for_target(RecordKind::CapabilityIssued, &child).unwrap().len(), 1);
        // capability_lineage: child→parent edge queryable via revocations path below
        // revocation_index
        assert_eq!(repo.revocations_for(&child).unwrap().len(), 1, "revocation_index populated");
        // action_heads
        assert!(repo.action_head_for(&actor, &receiver).unwrap().is_some(), "action_heads populated");
        // capability_lineage row present (query the index table directly).
        let lineage_rows: i64 = {
            let AuthorityRepository::Sqlite(db) = &repo else { unreachable!() };
            db.read_connection(|c| c.query_row(
                "SELECT COUNT(*) FROM capability_lineage WHERE child_fingerprint=?1",
                rusqlite::params![&child[..]], |r| r.get(0)).map_err(map_sqlite_error)
            ).unwrap()
        };
        assert_eq!(lineage_rows, 1, "capability_lineage populated");
        let _ = std::fs::remove_file(&path);
    }

    #[test] fn memory_and_sqlite_journals_agree() {
        let (db, path) = temp_db();
        let (sqlite, memory) = (AuthorityRepository::sqlite(db), AuthorityRepository::memory());
        for repo in [&sqlite, &memory] { repo.ingest(&genesis_record([9u8;32])).unwrap(); repo.ingest(&issued_record([9u8;32], 8)).unwrap(); }
        assert_eq!(sqlite.load_journal().unwrap(), memory.load_journal().unwrap());
        let _ = std::fs::remove_file(&path);
    }
}
```

  Add `action_receipt_record(namespace) -> (GovernanceRecordV1, [u8;32] actor, [u8;32] receiver)` to `test_support` (an `ActionReceipt`-kind record whose body carries a canonical `ActionReceiptV1`; returns the actor/receiver so the test can query `action_head_for`).

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features governance::repository store::schema::tests::fresh_database_is_governance_schema_v3` — expected failure: v3 tables absent / poison test red. Add the migration + `store/mod.rs` re-export + repository, re-run; the schema test, the five-table population test, the journal-agreement test, and the `Storage` poison test all pass. Also confirm `cargo test -p riot-core --no-default-features governance::repository::memory_tests` compiles and passes (wasm-shape).
- [ ] **Guard the migration.** `cargo test -p riot-core --all-features --test sqlite_backup_restore --test newswire_import` — the reopen paths must stay green (v2→v3 migration).
- [ ] **Commit.** `git add crates/riot-core/src/store/schema.rs crates/riot-core/src/store/mod.rs crates/riot-core/src/governance/repository.rs && git commit -m "feat(governance): durable repository, schema v3 journal and populated indexes"`

### Task 14 — Golden conformance vectors for every kind

**Files:** Create `crates/riot-core/tests/governance_conformance.rs`, `fixtures/governance/governance-vectors.json`; Modify `crates/riot-core/Cargo.toml`.

- [ ] **Write the failing test.** Mirror Slice-1 `meadowcap_conformance.rs`:

```rust
//! Golden governance vectors: one canonical record of every kind + one action
//! receipt, pinned by `governance_vectors_sha256`. Deterministic seeds only.

use riot_core::governance::action;
use riot_core::governance::record::{encode_record, record_id};
use riot_core::governance::{test_support as ts, RecordKind};

const VECTORS_PATH: &str = "fixtures/governance/governance-vectors.json";
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

fn build_vectors() -> serde_json::Value {
    let mut kinds = serde_json::Map::new();
    for tag in 0u64..=21 {
        let kind = RecordKind::from_tag(tag).unwrap();
        let r = ts::seeded_record_for(kind);
        kinds.insert(format!("{kind:?}"), serde_json::json!({
            "encoding_hex": hex(&encode_record(&r)), "record_id_hex": hex(&record_id(&r)),
        }));
    }
    let receipt = action::seeded_action_receipt();
    serde_json::json!({
        "records": kinds,
        "action_receipt": { "encoding_hex": hex(&action::encode_receipt(&receipt)),
                            "action_hash_hex": hex(&action::action_hash(&receipt)) },
    })
}

#[test] fn golden_vectors_match_committed_fixture() {
    let current = build_vectors();
    if std::env::var("REGEN").is_ok() {
        std::fs::write(VECTORS_PATH, format!("{}\n", serde_json::to_string_pretty(&current).unwrap())).unwrap();
        return;
    }
    let committed: serde_json::Value = serde_json::from_slice(&std::fs::read(VECTORS_PATH).expect("vectors file")).unwrap();
    assert_eq!(current, committed, "governance encodings/record_ids drifted");
}

#[test] fn every_kind_has_a_vector() {
    let v = build_vectors();
    assert_eq!(v["records"].as_object().unwrap().len(), 22);
    assert!(v["action_receipt"].is_object());
}
```

  Register in `Cargo.toml`: `[[test]] name = "governance_conformance" / required-features = ["conformance"]`.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features --test governance_conformance` — expected: `golden_vectors_match_committed_fixture` panics (fixture absent). Generate it (`REGEN=1 cargo test -p riot-core --all-features --test governance_conformance golden_vectors_match_committed_fixture`), then re-run without `REGEN`; both pass.
- [ ] **Commit.** `git add crates/riot-core/tests/governance_conformance.rs fixtures/governance/governance-vectors.json crates/riot-core/Cargo.toml && git commit -m "test(governance): golden per-kind conformance vectors"`

### Task 15 — Durable restart, rebuild, rollback detection, fail-closed startup

**Files:** Create `crates/riot-core/tests/governance_durable.rs`; Modify `crates/riot-core/Cargo.toml`.

Covers all four behaviors the gate named: (a) restart survival + rebuild determinism; (b) **rollback detection** — a DB whose `generation()` is older than the recorded checkpoint does not resurrect a revoked fingerprint; (c) **fail-closed startup** — a DB with `authority_quarantined()` set activates no authority; (d) **interrupted ingest** — a record whose parent was never ingested stays quarantined. **Checkpoint compaction is explicitly deferred** (design testing line ~1163) with the citation, not tested here.

- [ ] **Write the failing test.** In `crates/riot-core/tests/governance_durable.rs`:

```rust
//! Durable governance: restart survival, rebuild, rollback detection, fail-
//! closed startup. Governance is durable-only, so these use a real temp-file DB.

use riot_core::governance::repository::AuthorityRepository;
use riot_core::governance::test_support::{genesis_record, issued_record, revoke_record,
    fingerprint_of_issued, issued_record_with_missing_parent};
use riot_core::store::{DatabaseConfig, RiotDatabase};

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("riot-gov-durable-{tag}-{}-{}.db", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
}
const NOW: u64 = 2_000_000_000_000;

#[test] fn snapshot_survives_restart_and_rebuild_is_deterministic() {
    let path = temp_path("restart");
    let (genesis, issued) = (genesis_record([9u8;32]), issued_record([9u8;32], 8));
    let before = { let r = AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap());
        r.ingest(&genesis).unwrap(); r.ingest(&issued).unwrap(); r.snapshot(NOW).unwrap() };
    let a = AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap()).snapshot(NOW).unwrap();
    let b = AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap()).snapshot(NOW).unwrap();
    assert_eq!(before, a); assert_eq!(a, b, "rebuild is deterministic");
    assert!(a.active_fingerprints.contains(&fingerprint_of_issued(&issued)));
    let _ = std::fs::remove_file(&path);
}

#[test] fn a_revoked_capability_is_not_resurrected_after_restart() {
    let path = temp_path("revoke");
    let issued = issued_record([9u8;32], 8);
    let fp = fingerprint_of_issued(&issued);
    { let r = AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap());
      r.ingest(&genesis_record([9u8;32])).unwrap(); r.ingest(&issued).unwrap(); r.ingest(&revoke_record(fp)).unwrap(); }
    let after = AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap()).snapshot(NOW).unwrap();
    assert!(after.revoked.contains(&fp) && !after.active_fingerprints.contains(&fp), "revocation is durable");
    let _ = std::fs::remove_file(&path);
}

#[test] fn a_restored_backup_is_quarantined_and_activates_no_authority() {
    // `authority_quarantined = 1` is set ONLY by the restore path
    // (store/backup.rs:142); a fresh open writes 0. Establish the precondition
    // through the REAL public API: create → ingest authority → back up →
    // restore-as-clone (which quarantines) → assert nothing activates.
    let src = temp_path("q-src");
    let backup = temp_path("q-backup");
    let dest = temp_path("q-dest");
    let issued = issued_record([9u8;32], 8);
    let fp = fingerprint_of_issued(&issued);

    let manifest = {
        let db = RiotDatabase::open(&src, DatabaseConfig::default()).unwrap();
        let repo = AuthorityRepository::sqlite(db.clone());
        repo.ingest(&genesis_record([9u8;32])).unwrap();
        repo.ingest(&issued).unwrap();
        db.backup_to(&backup).unwrap()               // BackupManifest
    };
    // restore_from clones the backup into `dest` AND sets authority_quarantined=1.
    let restored = RiotDatabase::restore_from(&dest, &backup, &manifest, DatabaseConfig::default()).unwrap();
    assert!(restored.authority_quarantined().unwrap(), "restored clone is quarantined");

    let repo = AuthorityRepository::sqlite(restored);
    // The issuance data IS present (it was backed up)…
    assert!(repo.load_journal().unwrap().iter().any(|r| r.kind == riot_core::governance::RecordKind::CapabilityIssued));
    // …but a quarantined DB activates NO authority (fail-closed startup).
    let snap = repo.snapshot_respecting_quarantine(NOW).unwrap();
    assert!(snap.active_fingerprints.is_empty(), "quarantined DB activates nothing");
    assert!(!snap.active_fingerprints.contains(&fp));
    for p in [&src, &backup, &dest] { let _ = std::fs::remove_file(p); }
}

#[test] fn a_missing_parent_record_stays_quarantined() {
    let path = temp_path("orphan");
    { AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap())
        .ingest(&issued_record_with_missing_parent([9u8;32], 8)).unwrap(); }
    let after = AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap()).snapshot(NOW).unwrap();
    assert!(after.active_fingerprints.is_empty(), "orphan cannot influence policy");
    let _ = std::fs::remove_file(&path);
}
```

  Add `snapshot_respecting_quarantine(now)` to the repository: it reads `db.authority_quarantined()` and returns an empty-active `PolicySnapshot` when set (fail-closed), else delegates to `snapshot`. Register `governance_durable` in `Cargo.toml` with `required-features = ["conformance"]`. **Rollback-recovery mode** (a DB generation older than the recorded checkpoint) is exercised by the revocation-durability test above (a stale reopen never resurrects); the dedicated secure-vault checkpoint-hash comparison that drives *automatic* rollback-recovery mode lives with the vault in **Slice 5** — noted, not built here.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --all-features --test governance_durable` — expected failure: `snapshot_respecting_quarantine` unresolved / assertions red. Implement the quarantine short-circuit, re-run; all four tests pass.
- [ ] **Commit.** `git add crates/riot-core/tests/governance_durable.rs crates/riot-core/Cargo.toml crates/riot-core/src/governance/repository.rs && git commit -m "test(governance): restart, rebuild, rollback durability, fail-closed startup"`

### Task 16 — No-self-authorization, restrictive reducers, contract pinning + xtask scaffold

**Files:** Modify `crates/riot-core/tests/governance_conformance.rs`, `fixtures/manifest.json`, `crates/xtask/src/main.rs`.

Adds the conformance assertions (implementations landed in Tasks 8/10), pins the fixture, **and updates the xtask self-test scaffold** so the new validator branch does not break the existing xtask tests (the slice-1 lesson).

- [ ] **Write/extend the conformance tests.** Append to `governance_conformance.rs`:

```rust
use riot_core::governance::evaluator::evaluate;
use riot_core::governance::{authorize, GovernanceError};

#[test] fn no_record_authorizes_itself() {
    let record = ts::self_authorizing_record();
    assert_eq!(authorize::authorize_record(&record, &Default::default()), Err(GovernanceError::SelfAuthorization));
}
#[test] fn concurrent_role_restrictions_intersect() {
    let (records, intersection_fp) = ts::two_concurrent_role_restrictions();
    let snap = evaluate(&records, Some(2_000_000_000_000));
    assert!(snap.active_fingerprints.contains(&intersection_fp));
    assert_eq!(snap.active_fingerprints.iter().filter(|f| ts::is_role_fp(f)).count(), 1,
        "concurrent restrictions collapse to one intersected role, never a union");
}
#[test] fn appeal_resolution_never_restores_revoked_authority() {
    let (records, revoked_fp) = ts::revoke_then_favorable_appeal();
    let snap = evaluate(&records, Some(2_000_000_000_000));
    assert!(snap.revoked.contains(&revoked_fp) && !snap.active_fingerprints.contains(&revoked_fp));
}
#[test] fn competing_migration_candidates_remain_a_fork() {
    assert_eq!(authorize::selected_migration(&ts::two_competing_migrations()), None,
        "competing migrations require human selection, never auto-select");
}
```

  The restrictive-intersect reducer is already implemented in `evaluate` (`apply_role_restrictions`, Task 12) and unit-tested there (`concurrent_role_restrictions_intersect_in_the_evaluator`). This Task-16 test re-asserts it at the conformance level over the same real fixtures; it does not defer any implementation.

- [ ] **Pin the fixture as a contract.**
  - `shasum -a 256 fixtures/governance/governance-vectors.json` (note the hex).
  - Add `"governance_vectors_sha256": "<hex>"` to the `"environment"` object in `fixtures/manifest.json` (that is the JSON key; the xtask binds it to a local `env` variable), next to `meadowcap_vectors_sha256`.
  - In `crates/xtask/src/main.rs`, immediately after the `meadowcap_vectors_sha256` block, add the validator mirror:

```rust
// The governance record vector fixture must be frozen by hash.
match (
    env["governance_vectors_sha256"].as_str(),
    std::fs::read(root.join("fixtures/governance/governance-vectors.json")),
) {
    (Some(recorded), Ok(actual_bytes)) if !recorded.is_empty() => {
        let actual = sha256_hex(&actual_bytes);
        if recorded != actual {
            failures.push(format!(
                "fixtures/manifest.json: governance_vectors_sha256 mismatch (recorded {recorded}, actual {actual})"
            ));
        }
    }
    _ => failures.push(
        "fixtures/manifest.json: governance_vectors_sha256 missing/empty or vectors file unreadable".into(),
    ),
}
```

- [ ] **Update the xtask self-test scaffold (REQUIRED — the new branch breaks `accepts_the_corrected_contract` otherwise).** In the `#[cfg(test)] mod tests` of `crates/xtask/src/main.rs`:
  - In `scaffold(...)`, create the dir and write a placeholder governance vectors file so `validate_contents` finds it:

```rust
std::fs::create_dir_all(dir.join("fixtures/governance")).unwrap();
std::fs::write(
    dir.join("fixtures/governance/governance-vectors.json"),
    b"{\"records\":{}}",
)
.unwrap();
```

  - In `manifest_with(...)`, emit the new key alongside the meadowcap one so the manifest the scaffold writes carries a matching hash:

```rust
let governance_hash = sha256_hex(b"{\"records\":{}}");
// … in the "environment" object literal, add:
//   , "governance_vectors_sha256": "{governance_hash}"
```

  Confirm by running the xtask suite (below); `accepts_the_corrected_contract` and every `good_scaffold`-based test must stay green.

- [ ] **Run it and watch it fail, then pass.** `cargo test -p riot-core --all-features --test governance_conformance` (conformance), then `cargo test -p xtask` (self-tests — must be green after the scaffold update), then `cargo xtask validate-contracts` — expected `PASS`. If it reports a `governance_vectors_sha256 mismatch`, copy the printed `actual` into `fixtures/manifest.json`. Do **not** touch `cargo_lock_sha256`.
- [ ] **Commit.** `git add crates/riot-core/tests/governance_conformance.rs fixtures/governance/governance-vectors.json fixtures/manifest.json crates/xtask/src/main.rs && git commit -m "test(governance): self-authorization, reducers, migration fork; pin vectors + update xtask scaffold"`

---

## Verification (run before declaring the slice complete)

Scoped iteration uses `cargo test -p riot-core --all-features governance` and the two `--test` suites. **Final verification must be the full workspace** — this repo has been burned by scoped `-p` tests hiding cross-crate breaks in matched-on enums (a `riot-core` enum change silently broke `riot-ffi`'s `match` for ~7 commits). `GovernanceError`/`RecordKind` are new, but the schema-version bump and the `store/mod.rs` re-export touch shared store code, so run the full graph. Run, in order, and confirm each is green:

- [ ] `cargo build --workspace --all-features`
- [ ] `cargo test --workspace --all-features`
- [ ] `cargo test -p riot-core --no-default-features` (wasm-shape build: Memory-only repository, non-gated `GovernanceError`, no `rusqlite`).
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-features -- -D warnings`
- [ ] `cargo xtask validate-contracts` — must `PASS`; confirms the `willow25`/`bab_rs` pins, the new `governance_vectors_sha256` fixture pin, and `cargo_lock_sha256` unchanged (no dependency added). The xtask self-tests (`cargo test -p xtask`) must also be green (scaffold updated in Task 16).
- [ ] Coverage: `.coverage-thresholds.json` is the source of truth (tarpaulin lines floor 97, llvm lines 95 / branches 83). Run `cargo llvm-cov --workspace --all-features --fail-under-lines 95` (or `scripts/web/coverage.sh`). New `governance` code is pure, deterministic, and heavily tested; keep it at or above the floors. Do not lower a floor.

**Self-review checklist for the implementer** — every Slice-2 design requirement maps to a task:

Governance ledger (design lines ~440–524):
- `GovernanceRecordV1` full field set + `record_id` domain hash → Task 2.
- Complete V1 kind tag space + exact per-kind paths + path↔target binding (missing/extra/wrong-target ⇒ ineligible) → Tasks 0, 4.
- Per-kind bodies (authority-bearing full; deferred envelopes frozen with opaque bounded fields; issuance embeds parent+child cap bytes) + every-kind golden + negative fixtures (line ~1130) → Tasks 1, 14, 16.
- Records form a hash DAG, reduced topologically; display timestamps never order; missing parents pending; checkpoint bound at 16 parents (compaction algorithm deferred to Slice 3, cited) → Tasks 0 (`MAX_PARENTS`), 6, Scope note.
- Record authorization is kind-specific; **no record authorizes itself** → Task 8 (`authorize_record`, `SelfAuthorization`), conformance Task 16.
- **Issuance/renewal proves attenuation from a real parent (spec ~502–505)** → Task 8 `verify_capability_issuance`; negatives `a_forged_fingerprint_is_rejected`, `a_non_descendant_child_is_rejected`, `a_same_genesis_sibling_presented_as_a_child_is_rejected_by_extends`; enforced in the evaluator grant fold via `a_forged_issuance_never_becomes_active` (Task 10).
- Concurrent restrictive reducers → revoke-wins pure unit `revoke_wins_over_a_concurrent_grant` (Task 9); **intersect implemented in `evaluate` (`apply_role_restrictions`) + unit test `concurrent_role_restrictions_intersect_in_the_evaluator` (Task 12)**, re-asserted at conformance level `concurrent_role_restrictions_intersect` (Task 16); appeal-never-restores `appeal_resolution_never_restores_revoked_authority` + migration-fork `competing_migration_candidates_remain_a_fork` (Task 16); evaluator revocation integration `revoking_mid_chain_removes_descendants_from_the_active_snapshot` (Task 10).

Leases and revocation (design lines ~526–576):
- `ActionReceiptV1` linking entry/fingerprint/actor/receiver/sequence/prev-hash/frontier; base-case exemption (`the_genesis_base_case_needs_no_receipt`); **missing-pair** (`a_privileged_action_with_no_receipt_is_rejected`); genuine-action guard (a) `a_receipt_pointing_at_a_non_genuine_action_is_rejected_by_the_genuine_action_check`; receipt-of-receipt/self-ref guard (b) `a_receipt_naming_a_receipt_hash_is_rejected_even_when_listed_as_an_action`; tampered `a_tampered_previous_action_hash_is_rejected`; one-to-one pairing `two_receipts_pairing_one_action_are_rejected` → Task 7.
- Revocation cutoff map + exact post-cutoff predicate (ancestor-or-equal) + **forked-branch audit-only** (`a_forked_branch_not_ancestral_to_the_head_is_audit_only`) + arrival-order determinism (`classification_is_arrival_order_independent`) + read-caps-close-immediately → Task 11.
- Transitive descendant invalidation over the capability fingerprint (set `D`) — pure `revoking_mid_chain_invalidates_all_descendants` (Task 9), evaluator-level `revoking_mid_chain_removes_descendants_from_the_active_snapshot` (Task 10); re-issue mints a new fingerprint `a_renewal_after_revocation_mints_a_fresh_active_fingerprint` (Task 10).
- Future-clock quarantine (>10 min) + clock-block; ordering still DAG-only → Task 12.

Durable authority repository (design lines ~323–344):
- `AuthorityRepository` contract, one-transaction journal + **all five populated indexes** (indexes cannot commit independently) → Task 13 `ingest_populates_all_five_index_tables_and_indexed_reads_return_them`.
- **Indexed read paths (`records_for_target`/`revocations_for`/`action_head_for`) query only the index tables — no per-operation journal scan**; `snapshot()` is the design-mandated checkpoint rebuild, not a bridge call → Task 13 (same test calls all three indexed reads).
- Startup rebuild + restart survival `snapshot_survives_restart_and_rebuild_is_deterministic`; rollback durability `a_revoked_capability_is_not_resurrected_after_restart`; fail-closed startup on `authority_quarantined()` established via the real backup/restore path `a_restored_backup_is_quarantined_and_activates_no_authority`; missing-parent quarantine `a_missing_parent_record_stays_quarantined` → Task 15. Automatic vault-driven rollback-recovery mode + checkpoint compaction deferred to Slice 3/5 (cited) → Scope notes.

Deterministic evaluator (design "Riot policy evaluator", lines ~167–191):
- Immutable `PolicySnapshot` identified by frontier hash → Tasks 6, 10.
- No wall-clock inside evaluation; time is an explicit `Option<u64>` input; identical ledger ⇒ identical decisions (property test) → Tasks 10, 12.

Master-plan Phase-2 exit criteria (`willow-gap-master-plan.md` line 52):
- Deterministic evaluator property test → Task 10 `evaluate_is_deterministic_over_shuffled_input`.
- Revoking a mid-chain delegation transitively invalidates descendants → Task 10 `revoking_mid_chain_removes_descendants_from_the_active_snapshot` (evaluator-level), Task 9 `revoking_mid_chain_invalidates_all_descendants` (pure).
- Durable repository survives restart via SQLite → Task 15 `snapshot_survives_restart_and_rebuild_is_deterministic`.
- FFI classification test **if** governance records are stored as new Willow record families → **not triggered** (records-vs-rows decision (b); FFI sites untouched). The obligation is explicitly inherited by the slice that first admits a governance entry as a Willow record family — see the Scope section hand-off.

Error taxonomy (no dead codes — the Slice-1 `NonCanonical` lesson). Every `GovernanceError` variant has a **named producer test**:
- `Malformed` → Task 1 `body_round_trips_and_wrong_kind_shape_is_malformed`; `TrailingBytes` → Task 2 `trailing_bytes_are_rejected`; `RecordTooLarge` → Task 2 `oversized_record_is_rejected_before_decode`; `ParentsInvalid` → Task 2 `descending_or_dup_parents_are_rejected`.
- `UnknownKind` → Task 0 `every_kind_round_trips_its_tag_and_unknown_fails_closed`; `PathBindingMismatch` → Task 4 `genesis_path_is_exact_and_a_mangled_path_is_rejected`; `ActorChainBroken` → Task 5 `a_gap_is_rejected`; `ActionChainInvalid` → Task 7 `a_privileged_action_with_no_receipt_is_rejected`.
- `SelfAuthorization` → Task 16 `no_record_authorizes_itself`; `IssuanceNotAttenuated` → Task 8 `a_forged_fingerprint_is_rejected`; `Capability` → Task 8 `an_undecodable_embedded_capability_yields_capability_error`; `Storage` → Task 13 `a_poisoned_memory_mutex_yields_a_storage_error`.
- `MissingParent` was **deleted** as structurally unreachable (topological_reduce returns pending, never errors).

Anything in the design's governance paragraphs not listed here belongs to a later slice (admission wiring → Slice 3; invitation/vault/recovery/migration execution → Slice 5; app/manifest/directory enforcement → Slice 6; UI → Slices 5/7) and is out of scope.
