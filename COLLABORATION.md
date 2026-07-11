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
- Last committed conference-plan change: `1f71b46 docs: add public Riot gateway track`
- Conference plan: `docs/superpowers/plans/2026-07-11-riot-conference-native-demo.md`
- Phase 0A evidence work remains separate and must not be weakened for the
  conference demo.

## Active claims

| Owner | Scope | Files | State | Evidence / handoff |
| --- | --- | --- | --- | --- |
| Codex | Conference Task 1 review and commit | `crates/riot-core/tests/conference_fixture.rs`, `fixtures/conference/`, `docs/superpowers/specs/2026-07-11-riot-conference-native-demo-design.md` | Review passed; awaiting quality review and commit | `cargo test -p riot-core --test conference_fixture` — 2 passed |
| Codex | Conference Task 2 planning after Task 1 lands | `crates/riot-ffi/`, generated binding path | Not started | Do not edit until the Task 1 commit is recorded here. |
| Unassigned | Public gateway | `apps/gateway/`, `scripts/conference/gateway-smoke.sh`, `docs/decisions/riot-protest-net-runbook.md` | Planned | Task 8 in the conference plan; public-only boundary. |

## Handoff format

Append or replace a claim row with: owner, exact files, commit (if any), tests
run, result, remaining risk, and the next safe task. Keep it short.
