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
| Codex | Conference Task 2 quality repair | `crates/riot-ffi/`, `crates/riot-core/src/willow/identity.rs`, `crates/riot-core/src/willow/mod.rs` | **In progress** | TDD repair for shared public namespace/two-author flow, admission bounds, handle lifecycle, stable mobile errors, and rejected-inspect preservation. Claude retains exclusive ownership of `session.rs`/`import/join.rs`; this repair will not edit them. No commit. |
| Codex | Public gateway foundation | `apps/gateway/`, `fixtures/conference/gateway-space/`, `scripts/conference/gateway-smoke.sh`, `docs/decisions/riot-protest-net-runbook.md` | **In progress — spec repair** | First review rejected an unpinned duplicate export, QR payload text without a QR, and protocol-relative remote content. Repair must pin the fixture-bound export, render a real local SVG QR, and reject remote-reference forms. Deployment remains separate. |
| Codex root | UniFFI binding generator | `Cargo.toml`, `Cargo.lock`, `crates/xtask/`, `fixtures/manifest.json`, generated binding output contract | **In progress** | Focused command test and strict xtask clippy are green. Root owns the locked in-process Swift/Kotlin generator, its tests, and the required Cargo.lock reproducibility-hash update; do not edit mobile API/state files. |
| Codex root | Conference Task 3 bounded incremental reconciliation | `crates/riot-core/src/sync/`, `crates/riot-core/src/lib.rs`, `crates/riot-core/tests/core_sync.rs` | **In progress — RED first** | Implementing the plan's byte-stream-independent canonical bounded frames and missing-only reconciliation. This slice will not edit Claude's claimed `session.rs` or `import/join.rs`. |
| Claude | Fix P1/P2 defects in commit `d4edb77` (store byte-charge accounting) | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs` | **Done, released** | Committed `933ea14`. Stopped retaining the capability/token past inspect-time verification; split entry charge into a permanent per-seen-entry index charge + a live-only bytes charge; charge is now per-`DispositionRow` not per-receipt; `ImportContext::route` bytes are now charged and enforced; `namespace_views` (64) is now tracked and capped. `cargo test -p riot-core -p riot-conformance --all-features` all green (added two new adversarial tests: oversized route and 65th-namespace both trip real `StoreFull`); `cargo clippy -p riot-core --all-features --all-targets -- -D warnings` clean; `cargo xtask validate-contracts` PASS. This commit also carries Codex's small uncommitted `live_entry_ids`/`public_entry_identity` additions to session.rs, untouched by my edits. **session.rs and import/join.rs are free.** |
| Claude | Time-ledger reconciliation for WU2 G2 (`934004d`, `d4edb77`) | `docs/decisions/phase0a-time-ledger.json` | **Done, released** | Committed `60649cf`. Added two ledger entries: concurrency evidence (completed, 0.2h) and charge accounting (partial, 0.3h — undercounts/no namespace_views ceiling, fix queued). `python3 -m json.tool` parses clean. 0.5h drawn from the WU2 reserve; ledger file is free. |
| Claude | Implement `retained_preview_output_bytes` (2 MiB) budget | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs`, new/modified `crates/riot-core/tests/core_import_charge_budget.rs` | **In progress** | The G2 gate requires "hard store/preview bounds" — plural. `933ea14` fixed the store half only. The preview half (`retained_preview_output_bytes`=2 MiB, covering the retained join plan/effect bytes and every 256-byte plan tombstone per `docs/superpowers/plans/2026-07-10-riot-phase0a-public-kernel.md:471`) is still fully unimplemented. Not touching `crates/riot-core/src/lib.rs` or `crates/riot-core/src/sync/` — no overlap with Task 3. |

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
