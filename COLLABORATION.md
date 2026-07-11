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
| Codex | Conference Task 2 path/order quality repair | `crates/riot-ffi/`, `crates/riot-core/src/willow/mod.rs` | **Done, released** | No commit. RED proved a correctly signed same-namespace alert with payload IDs mismatched to its canonical path was accepted, and opposite import orders produced reversed arrays. GREEN adds a boolean Core path-binding helper and full-entry-ID sorting. Workspace tests/all-features, strict clippy, validator, binding generation, full fmt, diff-check, and generated binding forbidden-type scan pass. Files are free. |
| Codex | Public gateway foundation | `apps/gateway/`, `fixtures/conference/gateway-space/`, `scripts/conference/gateway-smoke.sh`, `docs/decisions/riot-protest-net-runbook.md` | **Done, released** | Committed `976e965` after spec PASS and quality APPROVED. 17/17 tests, smoke, compile, shell syntax, and diff checks pass; exact export/QR pins are enforced, the QR payload is decoded in-test, and remote/private/write routes are rejected. Hosting/DNS/TLS deployment remains separate. |
| Codex root | UniFFI binding generator | `Cargo.toml`, `Cargo.lock`, `crates/xtask/`, `fixtures/manifest.json`, generated binding output contract | **Done, released** | Committed `e3e1f0d`. `cargo xtask generate-bindings` emits non-empty Swift, C header/module map, and Kotlin; 12/12 xtask tests, strict clippy, and contract validator pass. Generated build output remains ignored. |
| Codex root | Conference Task 3 bounded incremental reconciliation | `crates/riot-core/src/sync/`, `crates/riot-core/src/lib.rs`, `crates/riot-core/tests/core_sync.rs`, `docs/decisions/riot-conference-sync.md` | **In progress — review** | Six focused tests and strict focused clippy pass: canonical bounded frames, missing-only exchange, identical-set no-transfer, namespace/sequence rejection, and preview-first import. Independent review is active; this slice does not edit `session.rs` or `import/join.rs`. |
| Claude | Fix P1/P2 defects in commit `d4edb77` (store byte-charge accounting) | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs` | **Done, released** | Committed `933ea14`. Stopped retaining the capability/token past inspect-time verification; split entry charge into a permanent per-seen-entry index charge + a live-only bytes charge; charge is now per-`DispositionRow` not per-receipt; `ImportContext::route` bytes are now charged and enforced; `namespace_views` (64) is now tracked and capped. `cargo test -p riot-core -p riot-conformance --all-features` all green (added two new adversarial tests: oversized route and 65th-namespace both trip real `StoreFull`); `cargo clippy -p riot-core --all-features --all-targets -- -D warnings` clean; `cargo xtask validate-contracts` PASS. This commit also carries Codex's small uncommitted `live_entry_ids`/`public_entry_identity` additions to session.rs, untouched by my edits. **session.rs and import/join.rs are free.** |
| Claude | Time-ledger reconciliation for WU2 G2 (`934004d`, `d4edb77`) | `docs/decisions/phase0a-time-ledger.json` | **Done, released** | Committed `60649cf`. Added two ledger entries: concurrency evidence (completed, 0.2h) and charge accounting (partial, 0.3h — undercounts/no namespace_views ceiling, fix queued). `python3 -m json.tool` parses clean. 0.5h drawn from the WU2 reserve; ledger file is free. |
| Claude | Implement `retained_preview_output_bytes` (2 MiB) budget | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs`, `crates/riot-core/tests/core_import_charge_budget.rs` | **Done, released** | Committed `816366e`. Charges the preview's retained entries+route and the active plan's own separate retained copy (PlanState.route is a distinct clone from PreviewState.route) plus `plan_tombstone_bytes` (256/tombstone). Enforced at `inspect()` (before installing a new preview) and `plan()` (before superseding the active one), both reject with no mutation. `cargo test -p riot-core -p riot-conformance --all-features` all green (two new adversarial tests: an oversized route rejected at inspect(), and a route that only exceeds budget once doubled by the plan's own copy, proving plan() checks independently); clippy `-D warnings` clean; `xtask validate-contracts` PASS. **session.rs and import/join.rs are free.** G2's "hard store/preview bounds" requirement is now fully covered (store: `933ea14`, preview: `816366e`). |
| Claude | Technical-debt audit + time-ledger reconciliation | `docs/decisions/phase0a-time-ledger.json` | **In progress** | Full-workspace re-verification (all crates now build/test/clippy/fmt clean except `core_sync.rs`, which is Codex's own active in-review WIP — not touching it). Cross-checking every `fixtures/manifest.json` ceiling against actual runtime enforcement (several were found and fixed this session already: `retained_store_budget_bytes`, `namespace_views`, `retained_preview_output_bytes`/`plan_tombstone_bytes`); dispatched a background research pass on the remaining 13 unverified ceilings since some may be structurally enforced by `willow25`'s const-generic types rather than riot-core's own code. Also reconciling the ledger for `933ea14`/`816366e`, which weren't recorded yet. |

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
