# Multi-session coordination snapshot — 2026-07-15

Many Claude sessions share this checkout via `git worktree`. This is the live map
so any session (or the owner) can see who owns what, what is sequenced behind what,
and where the merge hazards are. Update it when a branch lands or a track changes.

## Worktrees / branches

| Worktree | Branch | Base | Owner / purpose | Status |
|---|---|---|---|---|
| `riot` (main checkout) | `design/composite-site-manifest` | — | Another session — composite-site web viewer design | design gate PASSED; not this coordinator's |
| `riot-wt-security` | `sec/residuals` | main | **Coordinator** — security residuals (Risk 9 + 13) | Rust done + red-green proven (uncommitted); native rebuild + platform verify + PR in progress |
| `riot-wt-3e` | `feat/join-descriptor` | main | **Coordinator** — Risk 15 join-descriptor fix | HELD — must rebase on `sec/residuals` after it lands (both edit `join_public_space`) |
| `riot-wt-cov` | `test/raise-reachable-coverage` | (own remote) | **Coordinator (gate) + another session (tests)** | PR #3 open; CI re-running on llvm-cov switch |
| `.claude/worktrees/cov-tests` | `coverage-reachable-tests` | — | Another session — coverage-raising tests (feeds PR #3 branch) | local, not pushed |
| (branch) | `chore/ci-and-doc-truth` | — | Another session — CI + app-icon + doc-truth | pushed; app-icon already merged to main |

## Open PRs

- **PR #3** `test/raise-reachable-coverage → main` — "Coverage: honest ratchet gate + raise reachable coverage." Two workstreams on one branch: (1) coordinator's honest coverage gate [done]; (2) another session's coverage-raising tests [in progress]. CI: gateway+web green; rust+coverage(llvm-cov) re-running. **Do not merge until the coverage-raising author confirms their half.**

## Coordinator's agents

- `agent-sec-residuals` — ACTIVE in `riot-wt-security`. Risk 9 (WebRTC bundle scan at `verify_app_pair`, deny-closed) + Risk 13 (seal-inline-on-join) both Rust-complete + red-green proven; native rebuild → verify → PR next. **Changes `join_public_space` signature (adds `wrapping_key`).**
- `agent-3-registry` — HELD in `riot-wt-3e`. Risk 15 (join-descriptor). Confirmed scope; waiting for the ping that `sec/residuals` has merged to main, then rebases on the new join signature.

## Sequencing / merge hazards

1. **`join_public_space` is edited by TWO tracks** — Risk 13 (security, adds `wrapping_key` + inline seal) and Risk 15 (join-descriptor, registers the descriptor id). They MUST land serially: **security PR first → main → Risk 15 rebases on it.** Building both concurrently = conflict on the function + its ~40 call sites. This is why Risk 15 is held.
2. **PR #3 is independent** of the security/Risk-15 work (touches coverage config + tests, not `join_public_space`). Can merge on its own timeline.
3. **The main checkout wanders** — other sessions switch its branch (seen it on `main`, `test/raise-reachable-coverage`, and now `design/composite-site-manifest`). Never assume the main checkout is on `main`; work in a named worktree and commit by explicit pathspec.

## CI note (coverage gate)

The CI coverage job uses **cargo-llvm-cov** (`--fail-under-lines`), NOT tarpaulin: tarpaulin's ptrace engine hangs on this workspace under CI (>25 min, orphaned `core_import_transaction`). llvm-cov is source-based, reliable. Floor `thresholds.llvm.lines` = 95 (measured 97.75%). Local `coverage.sh` still runs tarpaulin (`--timeout 300`) + the full composite. See `docs/ci/coverage-gate-findings.md`.
