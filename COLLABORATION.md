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
| agent-1a-newswire | **1A — complete the newswire projection surface** | `crates/riot-core/src/newswire/{model,projection,store,entry}.rs`, `crates/riot-ffi/src/newswire_ffi.rs`, `crates/riot-ffi/tests/newswire_contract.rs`, `crates/riot-core/tests/newswire_*.rs`, `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/android/.../RiotController.kt` | IN PROGRESS | 2026-07-15 |
| agent-0a-catalog | **0A — canonical catalog & Apple artifacts** | `crates/riot-core/src/apps/starter.rs`, `crates/riot-core/tests/apps_starter.rs`, `apps/ios/Riot/AppModel.swift`, **both** `project.pbxproj`, `apps/ios/RiotTests/` (new `StarterResourceTests`) | IN PROGRESS | 2026-07-15 |

## Known-red (do NOT report as working)

- **`TwoPeerNearbySyncTests` — 2 failures on `main`.** Bonjour two-peer sync: "Bob did not receive the item Alice added," 60s timeout. Reproduces identically at clean HEAD in an isolated worktree, so it predates `cb90b14`. Note `5d021de` is titled "fix(sync): resolve two-device double-start bug" — that fix did **not** make two-peer sync pass. Per the plan's physical-radio honesty rule: state proven vs assumed paths separately, and never claim two-peer sync works.
- **Rust coverage gate is RED** (94.60% vs required 100%). P3 is closing the 283-line baseline; the other 186 lines are deliberately deferred to the units that rewrite those files (0B, 1A).

## Note for other sessions

A pre-existing `stash@{0}` ("WIP on main: 3215689") is **not mine and has been left untouched**. I briefly popped it by accident and reverted the one file it touched (`apps/ios/Riot/Transport/LocalNetworkNearby.swift`) back to HEAD. The stash entry is preserved. If it's yours, it's still there.

## Do NOT claim (owned by later units)

- `crates/riot-core/src/newswire/**`, `crates/riot-ffi/src/newswire_ffi.rs` → **Unit 1A**
- `crates/riot-core/src/demo_fixture.rs`, `crates/xtask/src/{sign_conference_fixture,verify_conference_export}.rs` → **Unit 0B** (it rewrites them; covering them now is wasted work)
- `crates/riot-core/src/apps/starter.rs` → **Unit 0A**
