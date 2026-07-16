# Coordinator status — 2026-07-16

A snapshot of where the Riot build stands, what shipped, what's in flight, and the
ranked backlog. Supersedes `2026-07-15-multi-session-state.md` for the live picture.

## Shipped

- **TestFlight v1 (build 621, `net.protest.riot`)** uploaded to App Store Connect (Xcode Organizer, team GZCZBKH7MY). Archived from clean `main`. Tests the on-device community + publishing UI: create/switch communities, Home/Tools/People/Nearby, post updates, editorial actions, People. **Two known limits (both fixed in v2):** publishing is single-device (Risk 16), and iOS can't join-by-link yet (a dropped commit, recovering).
- **Community-first shell (plan units P1→3) — on `main`, green.** Multi-community SQLite registry with per-community unlinkable sealed identity, isolated fail-closed switch, newswire Home/front-page/open-wire/editorial, offline merge & share.
- **Two-peer nearby sync — FIXED** (`7242a0d`): was a same-community sync-role deadlock, not a radio limit. Proven over on-Mac Bonjour; physical BLE between two phones still assumed (no device rig — testable via v2 on two TestFlight phones).
- **Security residuals — on `main`** (PR #5): Risk 9 (WebRTC bundle-scan, deny-closed, at `verify_app_pair`) + Risk 13 (seal-inline-on-join). Both independently re-verified.
- **Coverage gate is now REAL** (PR #3/#4): honest per-tool ratchet floors that the scripts + CI read; CI enforces the line floor via `cargo llvm-cov --fail-under-lines` (tarpaulin's ptrace engine hangs in CI). Resolves the old fiction / Risk 6.
- **v2 distributed publishing — MERGED** (PR #12, `feat/newswire-sync`): follow multiple communities + SEE published newswire on a second device. Risk 15 (`join_newswire_community` carries the descriptor, no dead follow) + Risk 16 (`track_committed_entry` puts newswire into the sync inventory so publishing traverses the nearby bridge) + the recovered iOS 3D join UI (`d0194b8`, cleanly reconciled onto a moved main — took main's keyed `switchToCommunity`, layered the join UI). **Isolation independently ratified:** adversarial audit returned HOLDS — `install_sync_inventory`'s total `retain` + `inventory_ids != live_ids → Err` re-check (`mobile_state.rs:1713,1724`) fails closed on any foreign-namespace entry; 16/16 persistence_contract incl. isolation + write-race + newswire e2e; CI 4/4. **TF v2 ready to archive from clean main.** Fast-follow requested: a direct newswire cross-community isolation regression test (mechanism proven; just wants a durable guard).

## In flight — WS1 (the web missing link)

The distributed-publishing half is done (v1 + v2 shipped). The remaining mission half is the public web loop — see the program roadmap `docs/superpowers/plans/2026-07-16-newswire-web-integration.md`. WS1 (signed newswire gateway export, Rust/xtask) has a detailed TDD plan (`2026-07-16-newswire-gateway-export.md`) through the plan-review gate (Feasibility + Scope PASS; Completeness FAIL→revised→re-review in flight). **Branch reality:** origin/main newswire is still demo (`sample_view`); a live sibling (`design/composite-site-manifest`) already renders real records on an interim schema (`riot.newswire.export/1`, no proof bytes/reverification). WS1 unifies onto the board's `riot-public-gateway-export/2` with proof bytes + independent Ed25519 reverification, writing NEW file paths (non-conflicting); the renderer rewire (WS1-b) is coordinated with the newswire.py owner.

## Ranked backlog / what needs doing

1. **Physical two-device test** — validate nearby sync + publishing between two real iPhones via TF v2. Needs hardware (the owner's).
3. **Owner ratifications (decisions, not builds):** 1A `CurrentEntryV2` deviation (Risk 2); per-community sealed-identity model (Risk 12).
4. **Web/gateway half** — the public read/publish-to-the-world surface (the indymedia mission half). **Now planned:** see `docs/superpowers/plans/2026-07-16-newswire-web-integration.md` (program roadmap WS1–WS4). The true missing link is WS1 (signed newswire gateway export in Rust/xtask) — the board already proves the pattern; the newswire home is the only surface still on demo `sample_view()`. WS1 → WS2 (gateway renders real) → WS3 ("Open in Riot" verify).
5. **Composite-site / owned namespaces + personal pages** — a separate live session's track (Unit 1 landing on `main`).
6. **Residuals:** Risk 10 (Android digest instrumentation test); Risk 9 obfuscation (documented, bar-raising); Risk 14 (nearby-adopt a SECOND community — reopens 2B, needs its own security pass).
7. **Deferred by owner:** MLS / private groups; the §8.3 whole-product human trial.
8. **Hygiene:** prune stale worktrees; `COLLABORATION.md` has drifted to a stale revision (shows old known-red / coverage-RED) — reconcile.

## Incidents / lessons (this multi-session run)

- **Pushed-to-main ≠ safe on a contested main.** iOS 3D was pushed then dropped by another session's rewrite; half a cross-platform feature vanished silently. Fix: verify `git merge-base --is-ancestor` after main moves; land via PR-merge, not direct push.
- **Agent-to-agent routing is unreliable here** (names don't route across sessions). Coordinate through the shared ledger / docs, not point-to-point relays.
- **The main checkout's branch wanders** — never assume it's on `main`; work in named worktrees.
- **tarpaulin hangs under ptrace in CI** — use `cargo llvm-cov` for the CI coverage gate.

## Branch / worktree map

| Branch | Owner | State |
|---|---|---|
| `main` | shared | green; shell + security + coverage gate + composite-site Unit 1 |
| `feat/join-descriptor` | coordinator | v2 in flight (iOS recovery + Risk 15 + Risk 16) |
| `feat/composite-site-unit1` / `unit1-admission` | composite-site session | owned-namespace admission |
| `test/raise-reachable-coverage` | coverage session | merged (PR #3); residual ledger local |
| `design/composite-site-manifest` | design session | (the main checkout usually sits here) |
