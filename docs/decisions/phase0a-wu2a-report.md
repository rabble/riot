# Phase 0A — WU2A Report: Namespace-Local Willow Join

- **Status:** PASS (WU2A) — pending independent review
- **Owning work unit:** WU2 (Task 4)
- **Date:** 2026-07-10
- **Elapsed agent-hours:** ~0.6 of the 4.0 WU2 reserve

## What was proven

`crates/riot-core/src/import/join.rs` implements the order-independent, namespace-local Willow join. 8 tests in `core_import_join.rs` pass under `--features conformance`; clippy clean; the no-feature release build has zero warnings.

### Semantics

The join uses Willow's own prune predicate via `willow25::entry::EntrylikeExt::prunes`: `a` prunes `b` iff `a` is newer than `b` (timestamp, then payload digest, then payload length — the exact `wdm_cmp_recency` order) and they share a namespace and subspace and `a.path` is a prefix of `b.path`. The live set is the maximal antichain of `(pre-state ∪ batch)`: an entry is live iff no other entry in the union prunes it.

Phase 0A honesty note: all alert entries sit at the fixed four-component path `objects/alert/<object_id>/<revision_id>`, so distinct revisions are incomparable and pruning reduces to same-coordinate replacement (a path is a prefix of itself; newer wins). The join is nonetheless written against the general predicate so it stays correct if the path scheme grows, and the differential oracle exercises the full predicate.

### Batch determinism

`join_batch(&mut state, batch) -> Vec<JoinEffect>` derives the live set and every entry's disposition from `(pre-state ∪ batch)` as one set — never from sequential intermediate states. Effects:

- `Winner { pruned_entry_ids }` — live in the result; names only the **pre-state** live entries it pruned (never same-batch candidates), per the plan.
- `NotLive { dominating_entry_ids }` — accepted into history but not live; names the final-live entries that prune it.
- `AlreadyPresent` — the exact canonical entry id was already accepted before this batch.

### Test coverage

- distinct subspaces do not prune (subspace-scoped);
- newer prunes older across two batches — `Winner.pruned_entry_ids` populated only in the cross-batch case;
- older at the same coordinate is `NotLive` with the newer as dominator;
- equal coordinate resolves to exactly one survivor (digest-then-length tiebreak);
- duplicate insertion is `AlreadyPresent`;
- **order independence over all 24 permutations of four interacting entries** — identical live set every time;
- idempotence and commutativity;
- **differential oracle**: for all 24 permutations, Riot's live set equals `willow25::storage::MemoryStore` fed the same entries in the same order (via `insert_entry` + `get_area(Area::full())` read-back). This is the authoritative check that Riot's join matches Willow's canonical semantics.

### Ceilings

`MAX_STORE_ENTRIES` (1024) and `MAX_REFERENCES` (1024) from the manifest are enforced. A join that would exceed either ceiling returns a typed capacity error before state installation; effect reference lists are never truncated.

## Notes and scope

- `entry_id` (the domain-separated value identity from WU1) is the join/dedup key, matching the plan's "stable EntryId is the domain-separated hash of canonical Entry bytes."
- This is WU2A only: the arbiter, copy-on-write transaction store, preview/plan/commit lifecycle, receipts, and provenance are WU2B (Task 5). `MemoryStore` remains a test-only oracle — it is `Rc`-based, unbounded, and not the session's transactional state.

## Next action

WU2B (Task 5) — `RiotSession` arbiter, bounded copy-on-write snapshot transaction, `ImportPreview`/`ImportPlan` lifecycle, receipts, and provenance, mapping `JoinEffect` to the planned/receipt disposition vocabulary.
