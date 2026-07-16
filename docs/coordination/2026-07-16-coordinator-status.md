# Coordinator status — 2026-07-16

A snapshot of where the Riot build stands, what shipped, what's in flight, and the
ranked backlog. Supersedes `2026-07-15-multi-session-state.md` for the live picture.

## Shipped

- **TestFlight v1 (build 621, `net.protest.riot`)** uploaded to App Store Connect (Xcode Organizer, team GZCZBKH7MY). Archived from clean `main`. Tests the on-device community + publishing UI: create/switch communities, Home/Tools/People/Nearby, post updates, editorial actions, People. **Two known limits (both fixed in v2):** publishing is single-device (Risk 16), and iOS can't join-by-link yet (a dropped commit, recovering).
- **Community-first shell (plan units P1→3) — on `main`, green.** Multi-community SQLite registry with per-community unlinkable sealed identity, isolated fail-closed switch, newswire Home/front-page/open-wire/editorial, offline merge & share.
- **Two-peer nearby sync — FIXED** (`7242a0d`): was a same-community sync-role deadlock, not a radio limit. Proven over on-Mac Bonjour; physical BLE between two phones still assumed (no device rig — testable via v2 on two TestFlight phones).
- **Security residuals — on `main`** (PR #5): Risk 9 (WebRTC bundle-scan, deny-closed, at `verify_app_pair`) + Risk 13 (seal-inline-on-join). Both independently re-verified.
- **Coverage gate is now REAL** (PR #3/#4): honest per-tool ratchet floors that the scripts + CI read; CI enforces the line floor via `cargo llvm-cov --fail-under-lines` (tarpaulin's ptrace engine hangs in CI). Resolves the old fiction / Risk 6.

## In flight — v2 (the distributed-publishing build)

Branch `feat/join-descriptor` (worktree `riot-wt-3e`, agent-3-registry). ONE PR delivers:
- **Recover the dropped iOS 3D** — the iOS manual-join UI (`d0194b8`) was silently dropped from `main` during a history rewrite (Android half survived, iOS didn't). Cherry-picking `d0194b8` + `ac7c81e` back.
- **Risk 15 (descriptor carry)** — `join_newswire_community` registers the share-ref's descriptor id so a *joined* community's Home can reproject (not a dead follow). Rust done (`bef0c48`), persistence_contract 15/15.
- **Risk 16 (newswire enters the sync inventory)** — `import_signed_newswire` now tracks its committed entry in `sync_inventory` (like the alert/app-data paths), so a newswire profile can open a sync session and **publishing traverses the nearby bridge**. Rust done (`4bc4d09`). Guardrail: the `inventory == active-namespace-live-ids` isolation invariant must still hold (namespace-scoped) — coordinator runs an adversarial isolation pass.
- **End-to-end proof:** create A → publish → follower joins by share-ref → sync → follower's Home shows the post.

This is what makes "community + PUBLISHING" actually distribute to a second device. When it merges, cut TF v2.

## Ranked backlog / what needs doing

1. **Land v2** (above) — critical path; makes publishing distribute + restores iOS join. In flight.
2. **Physical two-device test** — validate nearby sync + publishing between two real iPhones via TF v2. Needs hardware (the owner's).
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
