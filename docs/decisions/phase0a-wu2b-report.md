# Phase 0A — WU2B Report: Session Arbiter and Atomic Import

- **Status:** G2 CORE PASS / G2 FULL INCOMPLETE — transaction, explicit selection, and bounded plan-lifecycle semantics are proven; the remaining lifecycle-race, hostile-corpus, and conservative charge-admission matrix from Task 5 is not yet implemented. **Do not claim full G2 or start WU3 on this report alone.**
- **Owning work unit:** WU2 (Task 5)
- **Date:** 2026-07-10
- **Elapsed agent-hours:** ~1.95 charged from WU2; ~1.45 remaining

## What is proven (transaction core)

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

## WU2B continuation: selection and plan lifecycle

Commit `3e790ee` (`feat: add bounded import plan lifecycle`) adds an explicit `ImportSelection`: a selection must be nonempty, unique, and eligible; invalid selections return typed errors without state change. A selected-only plan commits only its chosen entries. The canonical policy is a 64-plan budget for each live preview, not a session-wide plan cap; a later preview starts a fresh budget.

While a parent preview remains live, the same single mutex arbiter records durable child terminals: replacement by another plan produces `PlanSuperseded`, explicit close produces `PlanClosed`, and commit produces `PlanConsumed`, each retaining its exact result. Replacing the preview consumes it and every child plan; all later actions on old preview/plan handles return `PreviewConsumed`, which takes precedence over any child terminal code. That replacement releases the preview's tombstones, bounding retained records at 64. Closing a plan and creating a replacement plan make neither a receipt nor a store mutation. The inspection vector clone was removed.

`2ab47a4` (`test: gate transaction suite on conformance`) is the prerequisite registration fix: the default workspace test run now correctly skips the conformance-only transaction integration target. Fresh independent evidence before `3e790ee`: `cargo test -p riot-core --features conformance` passed 74 tests; `cargo fmt --check`, `cargo check --workspace --all-targets --locked`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check` all passed. The earlier generic workspace command is now green because of `2ab47a4`.

Commit `3b719b3` (`fix: scope import plans to previews`) repairs a specification ambiguity: it replaces the session-wide interpretation with the canonical per-preview policy, parent-consumption precedence, and replacement-time tombstone release. The repair followed TDD and fresh independent spec and quality approval; `cargo test -p riot-core --features conformance` passed 75 tests. This repairs policy clarity and bounded retention only; it does not satisfy the remaining full-G2 blockers below.

## What is NOT yet done (remaining Task 5 obligations before full G2)

These are honestly outstanding and block a full-G2 claim:

1. **Lifecycle race linearization:** commit/reject race, close/commit race, plan supersession/commit, session-close vs child action — each asserting one terminal winner, no deadlock within 2 seconds, at most one swap/receipt. The exact admission precedence matrix (`OBJECT_CLOSED`, `WRONG_SESSION` without locking the foreign arbiter, `PREVIEW_CONSUMED`, `STALE_PREVIEW`, validation) remains only partially implemented.
2. **Retained-store charge admission:** retain `JoinPlan` and conservative admission before mutation, with the hard 16 MiB budget across entry/index/receipt/namespace/digest-reference allocations and exact/one-over boundary tests.
3. **Hostile-input + log-safety at the session layer:** the `core_import_hostile` corpus, panic injection returning `INTERNAL_ERROR`/`SESSION_FAILED` and quarantining the session, and log assertions that no untrusted bytes or key material appear.

## Next action

Complete the remaining Task 5 matrix above (lifecycle races, `JoinPlan`/conservative charge admission with exact bounds, and hostile corpus/panic quarantine) to reach full G2, then request review. WU3 (native handoff) must not begin until full G2 passes.
