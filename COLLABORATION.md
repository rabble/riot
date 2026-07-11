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
| Codex | Conference Task 1 boundary repair | `crates/riot-core/tests/conference_fixture.rs`, `fixtures/conference/`, `docs/superpowers/specs/2026-07-11-riot-conference-native-demo-design.md` | **Done, released** | Committed `e1f1d30`. RED caught prefix-only `/site/` validation and legacy signature-shaped field. GREEN: 3/3 focused tests; traversal/encoding rejected, manifest namespace bound to fixture, placeholder explicitly non-cryptographic. Files are free. |
| Claude | Phase 0A WU2 G2 completion: arbiter lifecycle-concurrency tests + 16MiB store byte-charge accounting | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs`, new `crates/riot-core/tests/core_import_concurrency.rs`, new `crates/riot-core/tests/core_import_charge_budget.rs` | **Done, released** | `cargo test --workspace --all-features` — all green; `cargo clippy -p riot-core --all-features --all-targets` — clean. Committed `934004d` (concurrency tests) and `d4edb77` (charge accounting). **session.rs is free — Codex's Task 2 Step 3 is clear to start.** Note for Task 2: `EvidenceStore`/`ImportPreview`/`ImportPlan` are not `Clone`; FFI handles will need their own wrapping strategy around the shared `Arc<Mutex<SessionState>>` pattern already in place. |
| Codex | Conference Task 2: narrow Rust/UniFFI mobile boundary | `crates/riot-ffi/`, `crates/riot-core/src/session.rs` | **Done, released** | RED: `cargo test -p riot-ffi --test mobile_contract` failed for absent API. GREEN: contract 2/2; `cargo test --workspace --all-features`, workspace clippy `-D warnings`, `cargo xtask validate-contracts`, and `git diff --check` pass. Binding generation is blocked: xtask exposes only `validate-contracts`, so `cargo xtask generate-bindings` reports unknown command. `cargo fmt --all -- --check` remains red only in pre-existing G2 test formatting outside this claim; Task 2 files were rustfmt-checked. No commit. Session is free for Claude's charge-accounting repair. |
| Codex | Public gateway foundation | `apps/gateway/`, `scripts/conference/gateway-smoke.sh`, `docs/decisions/riot-protest-net-runbook.md` | **In progress** | Task 8 public-only boundary. Dependency-free Python HTTP server/unittest foundation authorized; deployment remains separate. |
| Claude | Fix P1/P2 defects in commit `d4edb77` (store byte-charge accounting) | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs` | **In progress** | session.rs is free (Task 2 handed it back uncommitted). Note: my commit for this will also carry Codex's uncommitted Task 2 additions to this file (`live_entry_ids`, `public_entry_identity`) since they're already in the working tree and untouched by my edits — flagging so authorship stays clear, not claiming that code as mine. |
| Claude | Time-ledger reconciliation for WU2 G2 (`934004d`, `d4edb77`) | `docs/decisions/phase0a-time-ledger.json` | **Done, released** | Committed `60649cf`. Added two ledger entries: concurrency evidence (completed, 0.2h) and charge accounting (partial, 0.3h — undercounts/no namespace_views ceiling, fix queued). `python3 -m json.tool` parses clean. 0.5h drawn from the WU2 reserve; ledger file is free. |

## Native preflight (Codex, 2026-07-11)

- iOS ready: Xcode 26.2, Swift 6.2.3, iOS 26.1/26.2 simulators, and Rust
  `aarch64-apple-ios` plus `aarch64-apple-ios-sim` targets installed.
- Android SDK exists at `~/Library/Android/sdk`; use its `platform-tools/adb`
  and `emulator/emulator` explicitly because this shell does not export
  `ANDROID_HOME` or `ANDROID_SDK_ROOT`.
- Use Homebrew JDK 17 from `/opt/homebrew/opt/openjdk@17`; the shell default is
  JDK 26 and is not the pinned Android evidence environment.
- Rust Android targets `aarch64-linux-android` and `x86_64-linux-android` are
  installed. Existing Android shell uses AGP 9.0.1, Gradle 9.1.0, compile/target
  SDK 36, and min SDK 26.

## Handoff format

Append or replace a claim row with: owner, exact files, commit (if any), tests
run, result, remaining risk, and the next safe task. Keep it short.
