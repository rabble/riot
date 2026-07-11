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
| Codex | Conference Task 2 path/order quality repair | `crates/riot-ffi/`, `crates/riot-core/src/willow/mod.rs` | **Done, released** | Committed `58b50cd` after spec PASS and quality APPROVED. Signed path/payload mismatch is rejected, current entries sort by full ID, 11 focused tests pass, generated Swift/Kotlin succeed, and the full workspace gate is green. Files are free. |
| Codex | Public gateway foundation | `apps/gateway/`, `fixtures/conference/gateway-space/`, `scripts/conference/gateway-smoke.sh`, `docs/decisions/riot-protest-net-runbook.md` | **Done, released** | Committed `976e965` after spec PASS and quality APPROVED. 17/17 tests, smoke, compile, shell syntax, and diff checks pass; exact export/QR pins are enforced, the QR payload is decoded in-test, and remote/private/write routes are rejected. Hosting/DNS/TLS deployment remains separate. |
| Codex root | UniFFI binding generator | `Cargo.toml`, `Cargo.lock`, `crates/xtask/`, `fixtures/manifest.json`, generated binding output contract | **Done, released** | Committed `e3e1f0d`. `cargo xtask generate-bindings` emits non-empty Swift, C header/module map, and Kotlin; 12/12 xtask tests, strict clippy, and contract validator pass. Generated build output remains ignored. |
| Codex root | Conference Task 3 bounded incremental reconciliation | `crates/riot-core/src/sync/`, `crates/riot-core/src/lib.rs`, `crates/riot-core/tests/core_sync.rs`, `docs/decisions/riot-conference-sync.md` | **Done, released** | Committed `8fafeeb` after independent PASS. Nine tests prove canonical bounded frames, bidirectional divergent-set convergence, missing-only transfer, preview-first promotion, identical-set no-transfer, replay/namespace/request mismatch preservation, and Reject lifecycle. Full workspace tests and strict clippy pass. |
| Codex iOS agent | Task 4 iOS durable signer wiring | `apps/ios/` | **Done, released** | Landed `5bb25fa` + `55ff180`, independent review APPROVED. Eight tests plus actual entitled simulator add/relaunch prove exact signer continuity, strongest Keychain class, offline content, migration, no profile key leakage, and no alert. |
| Codex Android agent | Task 4 Android durable signer wiring | `apps/android/` | **Done, released** | Landed `c690836` + `a1a9cba`, independent review APPROVED. Fifteen JVM + ten API36 tests prove exact signer continuity, true encrypted-v1 migration, pre-allocation/file bounds, error-path key/plaintext wiping, fail-closed atomicity, and exact paired ABIs. |
| Codex root | Conference Task 4 native core packaging | `scripts/conference/build-native-core.sh`, `scripts/conference/test-native-core-package.sh`, `docs/decisions/riot-native-core-packaging.md`, generated/ignored native artifacts | **Done, released** | Committed `df44a36`, independently approved. Regenerated after sealed identity; locked Swift/Kotlin plus iOS device/simulator and Android arm64/x86_64 package test passes. |
| Codex identity agent | Task 4 durable signer identity | `Cargo.toml`, `Cargo.lock`, `fixtures/manifest.json`, `crates/riot-core/src/willow/identity.rs`, `crates/riot-core/src/willow/mod.rs`, `crates/riot-ffi/` | **Done, released** | Committed `1fabe48` plus hardening `347af09`; independent security review APPROVED. Full workspace tests, strict clippy, bindings/secret scan, contract validation, cargo-audit, and fixed lock hash pass. Native two-layer wrapping-key integration is active. |
| Codex identity agent | Task 5 mobile sync FFI bridge | `crates/riot-core/src/sync/ffi_bridge.rs`, `crates/riot-core/src/sync/mod.rs`, `crates/riot-ffi/` | **Done, released — independently approved** | Landed `794b0ca` + `3ac6fb6` + `8efad91`. Twenty-three mobile contracts and nine core sync tests cover exact canonical bundle persistence, cancellation/rejection non-mutation, terminal invalidation, stable snapshots, partial-inventory refusal, and preservation of an active session when a second open is refused. Full locked workspace tests, strict clippy, generated Swift/Kotlin bindings, validator, and byte-only surface review pass. Files are free. |
| Codex iOS agent | Task 5 iOS nearby transport | `apps/ios/` | **In progress — RED first** | Loopback transport contract, CoreBluetooth discovery/pair confirmation, one-shot local-IP handoff with BLE fallback, plain-language coordinator/UI. No Android/core edits. |
| Codex Android agent | Task 5 Android nearby transport hardening | `apps/android/` | **Post-review protocol correction in progress** | `d36a964` hardened callback races/radio bounds. Follow-up TDD now models real terminal `FRAME_READY`: drains final accept/reject frames, preserves accepted state until peer completion, and avoids early async-GATT disconnect. Physical two-phone radios remain deferred. No iOS/core edits. |
| Claude | Fix P1/P2 defects in commit `d4edb77` (store byte-charge accounting) | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs` | **Done, released** | Committed `933ea14`. Stopped retaining the capability/token past inspect-time verification; split entry charge into a permanent per-seen-entry index charge + a live-only bytes charge; charge is now per-`DispositionRow` not per-receipt; `ImportContext::route` bytes are now charged and enforced; `namespace_views` (64) is now tracked and capped. `cargo test -p riot-core -p riot-conformance --all-features` all green (added two new adversarial tests: oversized route and 65th-namespace both trip real `StoreFull`); `cargo clippy -p riot-core --all-features --all-targets -- -D warnings` clean; `cargo xtask validate-contracts` PASS. This commit also carries Codex's small uncommitted `live_entry_ids`/`public_entry_identity` additions to session.rs, untouched by my edits. **session.rs and import/join.rs are free.** |
| Claude | Time-ledger reconciliation for WU2 G2 (`934004d`, `d4edb77`) | `docs/decisions/phase0a-time-ledger.json` | **Done, released** | Committed `60649cf`. Added two ledger entries: concurrency evidence (completed, 0.2h) and charge accounting (partial, 0.3h — undercounts/no namespace_views ceiling, fix queued). `python3 -m json.tool` parses clean. 0.5h drawn from the WU2 reserve; ledger file is free. |
| Claude | Implement `retained_preview_output_bytes` (2 MiB) budget | `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/join.rs`, `crates/riot-core/tests/core_import_charge_budget.rs` | **Done, released** | Committed `816366e`. Charges the preview's retained entries+route and the active plan's own separate retained copy (PlanState.route is a distinct clone from PreviewState.route) plus `plan_tombstone_bytes` (256/tombstone). Enforced at `inspect()` (before installing a new preview) and `plan()` (before superseding the active one), both reject with no mutation. `cargo test -p riot-core -p riot-conformance --all-features` all green (two new adversarial tests: an oversized route rejected at inspect(), and a route that only exceeds budget once doubled by the plan's own copy, proving plan() checks independently); clippy `-D warnings` clean; `xtask validate-contracts` PASS. **session.rs and import/join.rs are free.** G2's "hard store/preview bounds" requirement is now fully covered (store: `933ea14`, preview: `816366e`). |
| Claude | Technical-debt audit + time-ledger reconciliation | `docs/decisions/phase0a-time-ledger.json`, `docs/decisions/phase0a-wu2b-report.md`, `docs/superpowers/specs/2026-07-10-riot-evidence-sprint-design.md` | **Done, released** | Committed `a8501f9` (ledger) and `1620a92` (stale gate-status reports). Full-workspace verification: all crates build/test/clippy/fmt/`xtask validate-contracts` clean. Background research audited the 13 remaining `fixtures/manifest.json` ceilings — see next row for the one actionable finding; the rest (cbor_nesting, decoded_cbor_nodes, store_encoded_entry_bytes, transaction_snapshot_bytes) are unreachable-today defense-in-depth gaps, logged here for whoever eventually tightens them but not fixed now (same low-severity class, no current exploit path). |
| Claude | Fix path/payload binding bypass in core import (security) | `crates/riot-core/src/session.rs`, `crates/riot-core/tests/core_import_transaction.rs`, `crates/riot-core/tests/core_import_lifecycle.rs`, new `crates/riot-core/tests/core_import_path_binding.rs` | **Done, released** | Committed `0c8d276`. Wired `alert_entry_path_matches_payload` into `EvidenceStore::inspect()` (previously only called from the FFI's `inspectable_alert_entries` listing helper, never from the actual commit-capable path) — an entry whose payload is now decoded and checked against its own path is ineligible on mismatch, same silent-exclusion pattern as invalid items. This exposed a pervasive pre-existing test-fixture bug: `core_import_transaction.rs`/`core_import_lifecycle.rs`'s `signed()`/`signed_distinct()` helpers hardcoded a fixed payload object/revision id independent of the path params, so nearly every existing test entry was already mismatched — repaired both helpers. `core_import_charge_budget.rs`, `core_import_concurrency.rs`, `core_import_join.rs`, and Codex's `core_sync.rs` were already consistent, untouched. `cargo test --workspace --all-features` all green (98 tests); clippy `-D warnings` clean; `xtask validate-contracts` PASS. **Did not touch `crates/riot-ffi/`** (Codex/Task-4 territory) — its existing check stays as defense-in-depth now that the core layer also enforces it. **session.rs is free.** |
| Claude | Enforce path size ceilings in bundle import (security) | `crates/riot-core/src/import/bundle.rs`, new `crates/riot-core/tests/core_import_path_size_ceilings.rs` | **Done, released** | Committed `b65a60f`. `path_components`=64/`path_component_bytes`=256/`path_total_bytes`=2048 are now checked in `verify_frame` right after entry decode (new `DiagnosticCode::PathBoundsExceeded`, same per-item isolation as every other check there). Four new tests each construct a path violating exactly one ceiling while staying under `willow25`'s own looser bounds (MCL=MCC=MPL=4096) so `Path::from_slices` itself doesn't refuse construction, plus a sanity check on an ordinary in-bounds alert path. `cargo test --workspace --all-features` all green, no collateral breakage this time; clippy `-D warnings` clean; `xtask validate-contracts` PASS. `bundle.rs` is free. All actionable findings from the 13-ceiling debt audit are now closed — only the four low-severity, currently-unreachable gaps remain (see below), unclaimed. |

## Status: Codex out of tokens (2026-07-11, ~7:30am)

Claude is now driving solo. Landed while securing Codex's uncommitted
work: gateway hardening (`ce68c55`), Android shell (`e36e02a`, by
another still-active process), iOS shell (`674732a`, same), sealed
opaque signer identity (`1fabe48` + `347af09`), gateway signature-
verification design doc (`f68be92`, by another still-active process),
four backhaul/resilience research docs (`4890243`), and a trivial
pre-existing fmt fix (`497bf63`). Full workspace — build, every test
binary, clippy `-D warnings`, fmt, `xtask validate-contracts` — is
clean as of this entry. `docs/site/index.html` is untracked and left
alone: looks like an unrelated Claude-Artifact-style mockup, not part
of any tracked task — flagged to the user rather than committed or
deleted blindly.

Some Codex processes are still intermittently active (confirmed via
concurrent commits landing during this cleanup) even though token
budget is reportedly low — check `git log` and `ps aux | grep codex`
before assuming a file is safe to edit uncontested.

## Known outstanding debt (not yet fixed, logged 2026-07-11)

- **Four lower-severity, currently-unreachable gaps** (defense-in-depth only, given today's other fixed ceilings): `cbor_nesting`, `decoded_cbor_nodes`, `store_encoded_entry_bytes`, `transaction_snapshot_bytes` — none have an explicit runtime check; all are incidentally bounded by other already-enforced ceilings. Same class as `retained_store_budget_bytes` before it was fixed this session — worth closing eventually, not urgent. (Path size ceilings — `path_components`/`path_component_bytes`/`path_total_bytes` — were the fifth item in this category; fixed in `b65a60f`, no longer outstanding.)

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

## Active claim: signed JS apps platform (2026-07-11, new)

| Owner | Scope | Files | State | Evidence / handoff |
| --- | --- | --- | --- | --- |
| Claude (this session) | New feature, outside Phase 0A budget: signed, space-trusted JS apps that read/write their own Willow namespace and sync over the existing nearby transport. First app is a shared checklist. | New: `crates/riot-core/src/apps/`, new FFI surface in `crates/riot-ffi/`, new `apps/ios/Riot/Apps/`, new `apps/android/.../apps/`, new `apps/checklist/`. Does not touch existing `import/`, `sync/`, or nearby-transport files. | **Planning** | Design doc committed: `docs/superpowers/specs/2026-07-11-signed-js-apps-design.md`. Writing the implementation plan now (`docs/superpowers/plans/`); will claim specific file paths per-task before editing. `apps/ios/` and `apps/android/` remain otherwise as claimed by Task 5 agents above — this claim is additive (new subdirectories only), not a takeover. |

## Handoff format

Append or replace a claim row with: owner, exact files, commit (if any), tests
run, result, remaining risk, and the next safe task. Keep it short.

## Active claim: public marketing site (2026-07-11, new)

| Owner | Scope | Files | State | Evidence / handoff |
| --- | --- | --- | --- | --- |
| Claude (this session) | Public marketing site for `riot.protest.net` — static only, not the Willow gateway | `marketing/` (new dir), `docs/site/` (pre-existing untracked evidence-dispatch mockup, left as-is) | **In progress** | Plain static HTML/CSS, no backend, no build step, no deployment attempted. Does not touch `apps/gateway/` — that remains the read-only Willow `/site/` content server per `docs/decisions/riot-protest-net-runbook.md`, whose stated deployment prerequisites (DNS/TLS owner, approved hosting path, egress/edge policy review) are still unmet and out of scope for this session. |

## Status: Codex iOS/Android agents out of tokens, Claude taking over `apps/ios/` (2026-07-11, new)

The user confirmed the Codex iOS and Android agents are out of tokens for
the next ~2 hours. Their in-progress identity-wiring work was left green in
the working tree; Claude (this session) verified both suites and committed
on their behalf: iOS `5bb25fa` (7/7 `RiotTests`), Android `c690836`
(`./gradlew testDebugUnitTest` green). See the updated Task 4 rows above.

Note: while committing this, a new untracked file appeared —
`docs/superpowers/plans/2026-07-11-conference-gateway-signature-verification.md`
— indicating some Codex process (root, not iOS/Android) may still be
intermittently active elsewhere. Left untouched; not in this session's scope.

## Active claim: iOS visual design + navigation polish (2026-07-11, new)

| Owner | Scope | Files | State | Evidence / handoff |
| --- | --- | --- | --- | --- |
| Claude (this session) | Design pass on the native iOS shell (visual styling + tab/navigation structure) requested by rabble | `apps/ios/Riot/ConferenceShellView.swift`, new `apps/ios/Riot/Design/` module, `apps/ios/Riot/Resources/Fonts/`, `apps/ios/Riot/Info.plist` | **Done, released** | Spec `docs/superpowers/specs/2026-07-11-riot-ios-visual-identity-design.md`, plan `docs/superpowers/plans/2026-07-11-riot-ios-visual-identity.md`, executed as 14 commits (`0010e47`..`acada8c`). Ports the marketing site's Anton/Work Sans/Space Mono + flat hard-bordered identity into a new `Design/` module (`RiotTheme`, `RiotCard`, `RiotButtonStyle`, `RiotBadge`, `RiotHeader`, `RiotEmptyState`, `RiotTabBar`) and fully replaces native `TabView` chrome with a custom docked bar. All five screens restyled, including adapting `ConnectionStatusView` to the real nearby-pairing UI the Transport agent landed concurrently (preserved, not overwritten). `xcodebuild test` (scheme `RiotKit`) 36/36 green including 5 new tests (`RiotThemeTests`, `RiotTabBarTests`). Visually verified in simulator: Spaces screen confirmed correct in both light and dark appearance (custom fonts rendering, flat 2px-bordered card, pink stamp tab-bar selection), reproduced clean on two separate simulator devices/OS versions. Board/Compose/Import/Connection not independently screenshotted (all reuse the same verified components; couldn't safely automate taps on this desktop — too many overlapping windows from unrelated apps to risk blind coordinate clicks, tried twice and both landed on the wrong window without touching the simulator) — worth a follow-up look next time someone's at the simulator directly. **Correction to an earlier note in this row:** I initially reported a Keychain `status(-34018)` (`errSecMissingEntitlement`) as a pre-existing `Core/`-layer bug. It wasn't — `55ff180` (landed before my Task 1, already an ancestor of all my commits) had already fixed it. The error was caused by my own verification method: `xcodebuild ... install` produces an archive-style artifact that isn't properly registered with `simctl` (subsequent `simctl launch` either throws that Keychain error or, after an uninstall, fails outright with `FBSOpenApplicationServiceErrorDomain` code 4). Using `xcrun simctl install <device> <path-to-Debug-iphonesimulator/Riot.app>` instead launches clean with no error, confirmed on both devices that previously showed it. No action needed from the `Core/` owner — sorry for the false alarm. Files are free. |

## Status: Task 5 (nearby transport) verified, fixed, and landed (2026-07-11, new session)

A separate Claude session (this entry's author) brainstormed and wrote
`docs/superpowers/specs/2026-07-11-nearby-transport-design.md` for Task 5, then
found that Codex's "nearby transport swarm" (`5e16811` onward) had already
built almost the entire feature concurrently — Rust FFI sync bridge, iOS
BLE+local-IP transport, Android BLE+local-IP transport — matching the design
closely (including the plain-language UI requirement) without direct
coordination. Rather than duplicate the work, this session verified and
fixed it instead:

- The native library packages (`scripts/conference/build-native-core.sh`)
  were stale relative to fast-moving Rust FFI changes multiple times; each
  time this surfaced as iOS linker errors (`undefined symbol
  _uniffi_riot_ffi_...`) that were actually "needs a rebuild," not a real
  bug. Regenerated repeatedly as new Rust methods (`MobileSyncSession::cancel()`)
  landed.
- Real bug found and fixed: `CoreBluetoothFrameChannel` was missing
  `@unchecked Sendable`, unlike its sibling `LocalTCPFrameChannel` — a Swift 6
  strict-concurrency violation caught only by actually building, not by
  reading the diff. Fixed to match the established pattern in the same file.
- One flaky Android test (`NearbyTransportContractTest.
  failedChosenLocalSessionNeverSwitchesPerMessageOrToInternet`) failed once
  under heavy concurrent build load (cargo + xcodebuild + gradle running
  simultaneously) and passed cleanly on every rerun — treated as a timing
  flake, not a real bug, after confirming the file was stable and the retry
  was clean.
- Landed in four commits: `794b0ca` (Rust FFI bridge), `306b7c3` (Android
  transport), `8efad91` (end-to-end wiring across all three surfaces —
  `MobileSyncSession::cancel()`, the generated-adapter persist-before-accept
  bridge on both platforms, UI wiring), `544dddb` (iOS test parity).

Final state: `cargo test --workspace --all-features` 129 tests green,
clippy/fmt/`xtask validate-contracts` clean; `xcodebuild test` (RiotKit)
19/19 passed; `./gradlew testDebugUnitTest` 39/39 passed, both debug APKs
assemble. Physical two-device BLE verification remains deferred per the
design doc — not achievable in this environment.

## Active claim: reflect today's research into living design specs (2026-07-11, new)

| Owner | Scope | Files | State | Evidence / handoff |
| --- | --- | --- | --- | --- |
| Claude (this session) | Fold this session's 5 research docs' design implications into the actual specs (not just leaving them as standalone research docs) | `docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`, `docs/architecture/willow-architecture.md` | **Done, released** | Committed `d3f9535`, pushed to `origin/main`. Added a 2026-07-11 addendum to the dual-mode spec (matching the existing addendum format) and two grounded notes to willow-architecture.md (Object Types: media authenticity via `verification`; Phase 2 WTP: Arti/data-mule transport candidates). Docs-only, no code touched. Both files are free. |

## Reconciled: public marketing site claim (2026-07-11, update)

The `marketing/` row above (claimed by "Claude (this session)", filed as
**In progress**) is updated: content — including a "For the technically
curious" section (real sync-protocol/admission-boundary/keystore summary)
and an expanded 6-step "how it works" flow — was added by another
concurrent session directly to `marketing/public/index.html` without a
handoff note here; it landed inside commit `a6112d0` alongside this
session's original build. Verified structurally sound (balanced tags) and
redeployed to `https://riot-protest-net-marketing.protestnet.workers.dev`
(Version `e00dc22e`) — live now byte-matches the committed file. Status:
**Live, matches `main`.** `marketing/` remains unclaimed/free for further
edits; please leave a handoff row here if you touch it next so redeploys
don't lag content again.
