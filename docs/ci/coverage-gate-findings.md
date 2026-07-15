# Coverage gate — audit findings (2026-07-15)

Status: **the documented 100% coverage gate is not real.** This note records the
measured truth and what a genuine gate would take. The fix is deferred to its own
tracked unit because it is a system change, not a threshold edit.

## Measured coverage (full local `scripts/web/coverage.sh` run)

| Tool / metric | Covered | Actual |
|---|---|---|
| tarpaulin — lines | 8214 / 8683 | **94.60%** |
| llvm-cov — lines | 15574 / 16229 | 95.96% |
| llvm-cov — functions | 1456 / 1527 | 95.35% |
| llvm-cov — regions | 23034 / 24876 | 92.60% |
| llvm-cov — branches | 1535 / 1844 | **83.24%** |
| c8 (JS) | — | not run (`c8` devDependency not installed) |

469 uncovered lines (tarpaulin) across 52 files. Worst offenders:
`riot-ffi/src/mobile_state.rs` (83), `xtask/src/sign_conference_fixture.rs`
(0/62 — zero tests), `xtask/src/verify_conference_export.rs` (48),
`riot-core/src/store/database.rs` (46), `riot-core/src/store/evidence.rs` (33).

## Why the gate is a fiction

1. **Nothing reads `.coverage-thresholds.json`.** It is referenced only by
   `CLAUDE.md`, `AGENTS.md`, and design docs — no code. `grep -r coverage-thresholds`
   over `scripts/`, `crates/`, `package.json` returns nothing.
2. **Three independent hardcoded gates, all set to 100%:**
   - `scripts/web/coverage.sh` — `cargo tarpaulin … --fail-under 100`.
   - `scripts/web/validate-llvm-coverage.mjs` — requires `covered === count`
     ("exact 100% is required").
   - `package.json` — `c8 --100`.
3. **`coverage.sh` is never run in any automated context** (there was no CI until
   this change). So the three gates that would fail at 94.60% simply never execute.

## What a real ratchet gate requires (the deferred unit)

Not a number edit — a redesign, because three tools with **disjoint metric sets**
are involved and "lines" means three different numbers (tarpaulin 94.60 vs
llvm-cov 95.96 vs JS):

- Restructure `.coverage-thresholds.json` into a **per-tool** schema
  (tarpaulin.lines; llvm.{lines,functions,regions,branches}; c8.{lines,…}).
- Make all three enforcement points **read** it: `coverage.sh` (`--fail-under`
  from JSON — the script already shells to `node`, or use `jq`),
  `validate-llvm-coverage.mjs` (compare against per-metric thresholds), and a
  wrapper for `c8` (an npm script string cannot read JSON).
- **Rewrite the tests that hardcode 100%**, not just extend them.
  `scripts/web/test/validate-llvm-coverage.test.mjs:263` asserts the literal
  string `"cargo tarpaulin --workspace --all-features --fail-under 100"`, and
  ~6 tests assert exact-100 behavior. Changing `coverage.sh` breaks them by
  construction. The shell tests also drive `coverage.sh` inside a synthetic root
  (`createFakeToolRepository`) that must be given a `.coverage-thresholds.json`.
- **Fail closed** on missing/malformed `.coverage-thresholds.json` — do not let a
  parse failure silently produce a no-op gate.
- **Enforce the ratchet**, not just document it. A comment in JSON is honor-system;
  a future PR can lower a number and go green. Needs a mechanical check (e.g. a
  test asserting thresholds never decrease vs the committed baseline, or review-only
  ownership of the file).

## Doc reconciliation owed by that unit

- `CLAUDE.md:54` / `AGENTS.md:50` state Rust coverage is
  `cargo tarpaulin --fail-under 100`; the real enforcement is the composite
  `scripts/web/coverage.sh`.
- `CLAUDE.md` / `AGENTS.md` call `.coverage-thresholds.json` the "single source of
  truth" and forbid silent divergence — currently false on both counts (nothing
  reads it; actual coverage diverges from its declared 100%).
