# Phase 0A — WU2B Report: Session Arbiter and Atomic Import

- **Status:** G2 CORE PASS / G2 FULL INCOMPLETE — core transaction semantics proven; the exhaustive lifecycle-race, hostile-corpus, and charge-accounting matrix from Task 5 is not yet implemented. **Do not claim full G2 or start WU3 on this report alone.**
- **Owning work unit:** WU2 (Task 5)
- **Date:** 2026-07-10
- **Elapsed agent-hours:** ~1.1 of the remaining WU2 reserve

## What is proven (core, 13 tests in `core_import_transaction.rs`)

`crates/riot-core/src/session.rs` implements the single-arbiter, copy-on-write import transaction. All state lives behind one `Arc<Mutex<SessionState>>`; handles (`EvidenceStore`, `ImportPreview`, `ImportPlan`) carry only ids plus the arbiter and acquire it before any check or mutation.

- **Open + store limit:** one store per session; a second `create_store` returns `SessionLimit`.
- **Valid import:** decode+verify happens before any mutation; a valid alert previews, plans, and commits to an `AppliedAtCommit` disposition; generation goes 0→1; one receipt.
- **Unknown trust:** eligible entries are `UnknownTrust` (Phase 0A carries no trust set into inspect) while signature/capability facts remain valid.
- **Rejected bundle → `InspectOutcome::Rejected`** with no preview and no state change.
- **Duplicate-only → `NoChanges`:** re-importing an identical bundle does not change generation and creates no second receipt.
- **Dominated + new in one commit:** a bundle with a newer entry (pruning a pre-state entry) plus a fresh entry increments generation exactly once; the pruned entry leaves the live set; both new ids get dispositions.
- **Stale preview:** a preview whose base generation no longer matches the store returns `StalePreview` on plan.
- **Plan consumed:** committing a plan twice returns `PlanConsumed`.
- **Closed store:** actions on a closed store return `ObjectClosed`.
- **Copy-on-write rollback:** `commit_with_injected_failure_for_tests` builds the entire next snapshot and receipt, then fails before the pointer swap; generation and live set are byte-for-byte unchanged. This is the atomicity proof.
- **Provenance:** separates cryptographic facts (`signature_valid`, `capability_valid`) and local receipt facts (route, first receipt) from current `LiveStatus`, and asserts no truth (`asserts_truth == false`).
- **History preserved on later pruning:** an entry pruned by a later commit keeps its receipt; its current status becomes `NotLive` while the receipt count grows.

Design details honoring the plan:

- The join is computed on a **clone** via `plan_join` (WU2A) and installed with one pointer swap; nothing observable changes before the swap.
- `JoinEffect` maps to `EntryDisposition`: `Winner → AppliedAtCommit`, `NotLive → DominatedAtCommit`, `AlreadyPresent → AlreadyPresent { insertion_receipt_id }`.
- Generation increments once per commit that accepts ≥1 previously-unseen entry; duplicate-only commits are `NoChanges` with no receipt.
- A store-identity guard (`require_store(store_id)`) rejects a foreign/stale store handle as `WrongSession`.
- `MAX_RECEIPTS` (256) enforced with `StoreFull` before the swap.

80 tests pass workspace-wide (`cargo test -p riot-core --features conformance` + release-surface + william3 + xtask); clippy clean; release `riot-ffi` builds without the conformance surface; `cargo xtask validate-contracts` PASS.

## What is NOT yet done (remaining Task 5 obligations before full G2)

These are honestly outstanding and block a full-G2 claim:

1. **Lifecycle race linearization:** commit/reject race, close/commit race, plan supersession/commit, session-close vs child action — each asserting one terminal winner, no deadlock within 2 seconds, at most one swap/receipt. The exact admission precedence matrix (`OBJECT_CLOSED`, `WRONG_SESSION` without locking the foreign arbiter, `PREVIEW_CONSUMED`, `STALE_PREVIEW`, validation) is only partially implemented.
2. **Plan tombstone accounting:** 64 plans per preview, one live, 256-byte-charged tombstones bounding terminal reasons to 16 KiB; `PLAN_SUPERSEDED` vs `PLAN_CONSUMED` distinctions.
3. **Retained-store charge model:** the hard 16 MiB budget across entry/index/receipt/namespace/digest-reference allocations, checked before mutation; exact/one-over boundary tests.
4. **Hostile-input + log-safety at the session layer:** the `core_import_hostile` corpus, panic injection returning `INTERNAL_ERROR`/`SESSION_FAILED` and quarantining the session, and log assertions that no untrusted bytes or key material appear.
5. **`ImportPreview.plan(selection)`** with an explicit selection (all-or-nothing, eligible-only, foreign/stale selection rejection). Current code implements `plan_all`; per-entry selection is not yet modeled.

## Next action

Complete the Task 5 matrix above (lifecycle races, tombstone/charge accounting, hostile corpus, selection) to reach full G2, then request review. WU3 (native handoff) must not begin until full G2 passes.
