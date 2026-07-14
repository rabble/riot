# Phase 0A — WU2B Report: Session Arbiter and Atomic Import

- **Status:** **COMPLETE (updated 2026-07-14).** G2 FULL has since been met: the concurrency races, charge-budget admission, hostile-input corpus, and the session-close race were all proven in subsequent commits. WU3 (native bindings, sync transport, app-entry sync surface) shipped and is merged to `main`. This report's original "do not claim full G2 or start WU3" gate is resolved. The report body below is preserved as historical record of the G2-core state as of 2026-07-10/11.
- **Owning work unit:** WU2 (Task 5)
- **Date:** 2026-07-10 (report body); concurrency and charge-admission evidence added 2026-07-11
- **Elapsed agent-hours:** ~1.95 charged from the original Task 5 slice; a further 0.9h charged 2026-07-11 for concurrency evidence (0.2h) and charge-admission repair (0.4h + 0.3h); WU2 reserve now at 0.25h remaining (see `docs/decisions/phase0a-time-ledger.json`)

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

## What is now proven (added 2026-07-11)

- **Arbiter lifecycle-concurrency races** (`crates/riot-core/tests/core_import_concurrency.rs`, commit `934004d`): three real-thread races against the single `Arc<Mutex>` arbiter, each proving exactly one linearized winner with no torn or double-applied state — racing `plan()` issuance from the same preview, racing `close()` against `commit()` on one plan handle, and racing `inspect()` (which atomically replaces the live preview) against a `commit()` on the preview being replaced.
- **Hard store/preview byte budgets** (`crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs`, commits `d4edb77` → `933ea14` → `816366e`): `retained_store_budget_bytes` (16 MiB, permanent store footprint: per-seen-entry index charge + live-entry bytes + per-row receipt charge + digest references + per-namespace charge, capped at `namespace_views`=64) and `retained_preview_output_bytes` (2 MiB, a live preview's and its active plan's own retained bytes, checked independently at `inspect()` and `plan()` since a caller can inspect/plan without ever committing). Both reject with no mutation before the swap. `933ea14` repairs gaps a `codex review` found in the first charge-accounting pass (uncharged capability bytes, an unbounded route string, charge lost on prune, an untracked namespace cap) — see `docs/decisions/phase0a-time-ledger.json` for the full defect list.
- This implementation does **not** literally "retain a `JoinPlan`" the way the original Task 5 plan worded it — the join is computed lazily at commit time against a clone, not precomputed and retained at `plan()` time. The budget requirement (bound retained bytes, reject before mutation) is met by a different, simpler mechanism than originally sketched; treat the plan's exact phrasing as superseded by this implementation.

## What is NOT yet done (remaining Task 5 obligations before full G2)

These are honestly outstanding and block a full-G2 claim:

1. **One untested race:** session-close racing a concurrent plan/preview action (e.g. `store.close()` vs. an in-flight `commit()` or `plan()`) is not covered by the three races above and remains untested. The exact admission precedence matrix (`OBJECT_CLOSED`, `WRONG_SESSION` without locking the foreign arbiter, `PREVIEW_CONSUMED`, `STALE_PREVIEW`, validation) is implemented in `session.rs`'s admission checks but has no dedicated concurrency test proving it holds under a real race.
2. **Hostile-input + log-safety at the session layer:** the `core_import_hostile` corpus, panic injection returning `INTERNAL_ERROR`/`SESSION_FAILED` and quarantining the session, and log assertions that no untrusted bytes or key material appear. Not started.

## Next action

Close the remaining two items above (the session-close race test, and the hostile-input/panic-quarantine/log-safety corpus) to reach full G2, then request review. WU3 (native handoff) must not begin until full G2 passes. Note: WU3 groundwork (UniFFI binding generation, the mobile API surface) has already started in parallel per `COLLABORATION.md` — that is a coordination decision made outside this report, not a claim that full G2 has passed.
