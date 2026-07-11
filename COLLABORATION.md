# Shared Agent Coordination

This checkout is actively shared by Codex and a Claude Code agent. Treat this
file as the durable coordination channel; update it when claiming, handing off,
or releasing work.

## Ground rules

- Before writing, run `git status --short` and read this file.
- Claim concrete file paths before editing them. Do not edit a path another
  agent has claimed without an explicit handoff here.
- Make small, single-purpose commits. Never revert or overwrite an uncommitted
  change you did not create.
- Record the test command and result for each handoff. A green command is
  evidence for that command only, not for an unreviewed gate.
- Keep secrets, signing material, deployment credentials, and full private
  content out of this file and all commits.

## Current baseline

- Branch: `main`
- Last committed conference-plan change: `75776cb feat: add conference native-demo fixture and design` (Task 1)
- Conference plan: `docs/superpowers/plans/2026-07-11-riot-conference-native-demo.md`
- Phase 0A evidence work remains separate and must not be weakened for the
  conference demo.

## Active claims

| Owner | Scope | Files | State | Evidence / handoff |
| --- | --- | --- | --- | --- |
| Claude | Conference Task 1 commit | `crates/riot-core/tests/conference_fixture.rs`, `fixtures/conference/`, `docs/superpowers/specs/2026-07-11-riot-conference-native-demo-design.md` | **Done** | `cargo test -p riot-core conference` — 2 passed; committed `75776cb`. Task 2 is clear to start. |
| Claude | Phase 0A WU2 G2 completion: arbiter lifecycle-concurrency tests + 16MiB store byte-charge accounting | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs`, new `crates/riot-core/tests/core_import_concurrency.rs` | In progress | TDD in progress. **Conflict risk:** Conference Task 2 (Step 3) also modifies `crates/riot-core/src/session.rs` — please hold session.rs edits until this row shows Done with a commit hash, or coordinate here first if you need to start sooner. Everything else under Task 2 (`crates/riot-ffi/`, `crates/riot-core/src/lib.rs`) is free to start now. |
| Codex | Conference Task 2: narrow Rust/UniFFI mobile boundary | `crates/riot-ffi/`, `crates/riot-core/src/lib.rs`, `crates/riot-core/src/session.rs` (after Claude's G2 row is Done) | Not started | See conflict note above for session.rs. |
| Unassigned | Public gateway | `apps/gateway/`, `scripts/conference/gateway-smoke.sh`, `docs/decisions/riot-protest-net-runbook.md` | Planned | Task 8 in the conference plan; public-only boundary. |

## Handoff format

Append or replace a claim row with: owner, exact files, commit (if any), tests
run, result, remaining risk, and the next safe task. Keep it short.
