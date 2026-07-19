# Overnight Work Log — 2026-07-20 (Anchor M2: hosting MVP, server crate)

Append-only. Newest at bottom. Morning summary goes at TOP when done. (Distinct filename — the tracked
`OVERNIGHT_LOG.md` belongs to the 2026-07-19 session; not clobbering it.)
Branch: `overnight/2026-07-20-anchor-m2` (off origin/main @ 1f6ecb2, which has anchor M1 complete).
Worktree: `/Users/rabble/code/explorations/riot-wt-m2`. Never main, never force-push. Pathspec commits.

## Bearings (I coordinated M1 this session, so bearings are current)
- **Anchor M1 COMPLETE on main**: `riot-anchor-protocol` (canonical wire, ticket/listing authority,
  control/descriptors/receipts/82-limits, routed `sync/2` FSM) + `riot-transport` ALPN router +
  Rust/TS/Swift/Kotlin conformance vectors. Design spec + M2 plan (WU-008–016) under
  `docs/superpowers/`. Security carry-forward: `docs/research/2026-07-19-wu003b-security-findings.md`
  — the `resolve_listing` HIGH is a **WU-015 acceptance criterion**.

## Overnight plan (safe, verifiable, low-blast-radius)
Build the greenfield **`riot-anchor` SERVER crate** (M2 hosting-MVP chain, PURE RUST, cargo-verifiable):
WU-013A (crate + forward-only schema) → 013B (repository quotas/eviction/recovery) → 014 (control
admission/idempotency/work/Prepare) → 015 (staged sync + composite Commit + receipt recovery; carries
the WU-015 security criterion) → 015B/016 (atomic listing; reserved removal + checkpoints).

**Deliberately SKIPPED overnight:** WU-008–010 client-storage-ownership refactor — moves profile
storage out of FFI into core, rewrites the SHIPPING app's persistence/import/sync/close paths (highest
blast radius; flagged for hardest review + Checkpoint B). Not safe unattended. Left for a supervised
session.

## Discipline
- One `general-purpose` subagent per WU (2 `metaswarm:coder-agent` dispatches died today with 0-work
  injection payloads — avoid that type). Subagents TDD, do NOT commit; I verify (`cargo test -p
  riot-anchor --all-features` + `cargo build --workspace` + clippy + fmt) and commit via pathspec.
- New crate adds `rusqlite` (already in lock via riot-core) → Cargo.lock gains a crate entry → refresh
  `fixtures/manifest.json` cargo_lock_sha256 (`xtask validate-contracts` prints the actual).
- PR each unit; merge on green CI (watcher). Verify before commit — never commit unverified native
  (all M2 server work is pure Rust, so fully verifiable).

---

## Log

### Task 1 — bearings + plan + worktree (done). Dispatching WU-013A (anchor crate + forward-only schema).
