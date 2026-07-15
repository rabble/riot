# Collaboration — File Claim Ledger

Multiple agent sessions share this checkout. This ledger is the concurrency control.

## Rules

1. **Claim before you edit.** Add a row below with your agent name, unit, and the *exact* file paths you will touch. Release it (status → `DONE`) when your work is committed.
2. **Explicit pathspecs only.** `git add <exact paths>` — **never** `git add -A` / `git add .`. Another agent's work is almost certainly in the tree.
3. **Foreign edits = STOP.** If you find uncommitted changes inside files you claimed that you did not make, **stop and report**. Do not merge, do not stash, do not "fix" them.
4. **Never `--no-verify`. Never `git push --force`.**
5. **No unit starts while either Xcode project file is claimed or dirty** — `apps/ios/Riot.xcodeproj/project.pbxproj` and `apps/macos/Riot.xcodeproj/project.pbxproj` are hand-edited and serialize all Swift file additions.
6. `git pull --rebase --autostash` before claiming and before committing.

Plan: `docs/superpowers/plans/2026-07-14-community-first-shell.md`

## Active claims

| Agent | Unit | Files | Status | Claimed |
|---|---|---|---|---|
| coordinator | P1 — native core rebuild | `build/native/**`, `build/generated/**` (generated; not tracked) | **DONE** — all 5 artifacts rebuilt, `nm` confirms newswire symbols (were 0) | 2026-07-14 |
| agent-p2-inflight | P2 — land in-flight iOS/Android work + tests | 20 files across `apps/ios` + `apps/android` | **DONE** — committed `cb90b14`. iOS 198 / Android 110, 0 new failures | 2026-07-14 |
| agent-p3-coverage-2 | P3 — Rust coverage baseline | tests only | **DONE** — 94.60%→96.47% (`2040ccc`, `e43286c`). 121 lines remain, all unreachable; found 3 phantom guards | 2026-07-14 |
| agent-1a-newswire | **1A — complete the newswire projection surface** | newswire projection.rs + newswire_ffi.rs + contract + iOS/Android call sites | **DONE** — committed `d150e79`. 746 Rust tests green (isolated), bindings+staticlibs rebuilt, macOS RiotKit compiles. iOS/Android app compile pending | 2026-07-15 |
| agent-0a-catalog | **0A — canonical catalog & Apple artifacts** | AppModel.swift, both pbxproj, apps_starter.rs, StarterResourceTests.swift | **DONE** — committed `09413e6`. Rust 8/8, iOS StarterResourceTests 7/7, iOS app builds with all 8 pairs | 2026-07-15 |

## Known-red (do NOT report as working)

- **`TwoPeerNearbySyncTests` — 2 failures on `main`.** Bonjour two-peer sync: "Bob did not receive the item Alice added," 60s timeout. Reproduces identically at clean HEAD in an isolated worktree, so it predates `cb90b14`. Note `5d021de` is titled "fix(sync): resolve two-device double-start bug" — that fix did **not** make two-peer sync pass. Per the plan's physical-radio honesty rule: state proven vs assumed paths separately, and never claim two-peer sync works.
- **Rust coverage gate is RED** (94.60% vs required 100%). P3 is closing the 283-line baseline; the other 186 lines are deliberately deferred to the units that rewrite those files (0B, 1A).

## Note for other sessions

A pre-existing `stash@{0}` ("WIP on main: 3215689") is **not mine and has been left untouched**. I briefly popped it by accident and reverted the one file it touched (`apps/ios/Riot/Transport/LocalNetworkNearby.swift`) back to HEAD. The stash entry is preserved. If it's yours, it's still there.

| agent-0b-riverside | **0B — deterministic Riverside authority** | demo_fixture + fixture + drift + apps_contract + mobile_state.rs (1A list/inspect fix) + regression test + xtask coverage + RiversideMemberToolUITests | **DONE** — committed `1276024`. 75 Rust suites / 0 fail; native rebuilt; iOS RiversideMemberToolUITests SUCCEEDED; full iOS 205 (only 2 known-red). Also closed 1A's FFI list/inspect newswire bug | 2026-07-15 |

| agent-0c-containment | **0C — runtime containment & invalidation (SECURITY)** | `crates/riot-ffi/tests/apps_contract.rs`, `crates/riot-ffi/src/{apps_ffi,mobile_state}.rs`, `apps/ios/Riot/Apps/AppRuntimeView.swift`, `apps/ios/RiotTests/AppRuntimeHostTests.swift`, `scripts/apps/{miniapp-browser.spec.mjs,fixtures/hostile-egress.html,test/hostile-egress-fixture.test.mjs}`, `package.json` (one script line), **+ migration files (claim CLEARED 2026-07-15 — all clean at HEAD, 1B/1C/1E not dispatched): `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/Riot/Apps/AppBridgeController.swift`, `apps/android/.../MainActivity.kt`, `apps/android/.../RiotController.kt`, `apps/android/.../apps/{AppDataPort,RiotJsBridge,AppWebViewHost}.kt`, `apps/android/.../test/.../RiotJsBridgeTest.kt`** | **✅ COMPLETE — ALL PLATFORMS GREEN on the regenerated `invalidate()` binding; ready for team-lead re-verify + commit.** Rust apps_contract 44/44 (workspace 0-fail, clippy 0, fmt clean); **iOS RiotKit 209 tests, only the 2 known-red Bonjour**; **Android `testDebugUnitTest` 113 tests, 0 fail**; JS hostile fixture 5/5 + Playwright egress 2/2. Data path runs on the gated `AppExecutionSession` on BOTH platforms (bridges wired: iOS `AppBridgeController`/`ProfileRepository`, Android `UniffiAppDataPort`/`RiotJsBridge`); end-to-end revocation proven through the REAL bridge; CSP-stripped egress backstop (WKContentRuleList / blockNetworkLoads) proven; teardown cancels watches; §4.7 revoked→"Return to Tools" both platforms. FFI: only `AppExecutionSession` + `is_valid()` + `destroy()`→`invalidate()` rename (all regenerated). **Residual: iOS WebRTC not covered by the content-rule-list backstop — best-effort private-pref disable, threat-model it (plan Risk 9).** No new Swift files → both pbxproj untouched. **COMMITTED `d9699e8` — coordinator independently re-verified all platforms vs fresh native.** | 2026-07-15 |

## Do NOT claim (owned by later units)

- `crates/riot-core/src/newswire/**`, `crates/riot-ffi/src/newswire_ffi.rs` → **Unit 1A**
- `crates/riot-core/src/demo_fixture.rs`, `crates/xtask/src/{sign_conference_fixture,verify_conference_export}.rs` → **Unit 0B** (it rewrites them; covering them now is wasted work)
- `crates/riot-core/src/apps/starter.rs` → **Unit 0A**
