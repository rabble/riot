# Coordinator status — 2026-07-18

Live snapshot. Supersedes `2026-07-16-coordinator-status.md`. main = `59499c7`.

## The two goals, where they stand

**1. Publish to devices (nearby sync) — SHIPPED + guarded.**
Create a community, publish, and it distributes to a second device that follows by share-ref.
- v2 distributed publishing merged (PR #12): Risk 15 (join carries the descriptor) + Risk 16
  (`track_committed_entry` puts newswire into the sync inventory so publishing traverses the
  nearby bridge) + recovered iOS 3D join UI. Isolation independently audited (HOLDS).
- Cross-community isolation regression guard merged (PR #13): a newswire post in A is never
  offered to B after a switch. 17/17 persistence_contract.
- **TestFlight v2 is ready to archive from clean main** (owner-side Xcode Organizer step).
- Deferred: physical two-phone BLE test (needs the owner's hardware).

**2. Publish to the public web (the indymedia reach layer) — 90% landed, one corrective PR left.**
A post published in the app renders on the public web as real signed content, verifiably.
The pipeline is `app/xtask signs → gateway renders → web mirrors`; the gateway is a stateless
renderer of a signed export.

## Web newswire — the export chain (all MERGED, rigorous)

- **WS1 (PR #15):** `cargo xtask export-newswire` + `verify-newswire-export` produce a signed
  `riot-public-gateway-export/2` (`fixtures/newswire/gateway-space/public-export-v1.json`) with
  proof bytes + independent Ed25519 reverification. Mirrors the proven conference-board pipeline.
- **WS1-b (PR #17):** `create_signed_profile_card` core helper; export mints organizer + editor
  profile cards and emits a top-level `contributors[]` block with real **display-name bylines**
  ("RIOT Editorial Desk", "Harbor Desk") from signed, synced profile-card records.

## Web newswire — the render layer (the open item)

Two web tracks collided on `apps/gateway/newswire.py`:
- **PR #19 (`web/newswire-arc`, MERGED):** a rich, deployed, navigable site (`/`, `/publish/`,
  `/about/`, per-post + per-author permalinks) + a launchable signed **AppBundle "drop"** — but
  renders from the interim **`/1`** projected export (`newswire-export-v1.json`, no proof bytes,
  no independent reverification). It merged before the coordination HOLD took effect.
- **WS2 (PR #20, DRAFT):** the **`/2`** consumption layer — `newswire_view_from_export` +
  `render_post`/`render_author` on a typed seam, display-name bylines. Now the **graft seed**,
  not a standalone merge (it can't merge onto post-#19 main without reverting the richer site).

**Owner decision (2026-07-16): unify onto `/2`.** Display names + per-author pages on the public
web; retire `/1`. (See memory `riot-web-author-identity`; verified honest — display names are
signed, synced profile-card records.)

**Corrective unification (IN FLIGHT, agent-3-registry):** a graft ON TOP of merged #19 — convert
main's `newswire.py` + `build.py` to render the rigorous `/2` export, remapping the field access
(WS2 is the reference), **keeping #19's richer pages + app-drop**, and **retiring `/1`**
(`newswire-export-v1.json` + `generate_newswire_export.rs`). Make-or-break scope question being
answered first: is the signed app-drop bundle coupled to `/1` data (→ needs bundle regen) or
independent (→ web render moves to `/2`, bundle stays)? Plan → coordinator gate → execute → PR.

## What's left on the web loop after unification

- **Verify "in Riot":** #19's app-drop already gives a launchable signed bundle (web reach, app
  truth). The remaining piece is the explicit web→app "Open in Riot" verify deep link
  (`riot://open?namespace=` exists; a per-entry verify path binds the mirror copy to the signed
  record). Scope after unification lands; part may already be covered by the app-drop.
- **Deploy the `/2` site** to the CF mirror (`riot-newswire-dev.protestnet.workers.dev`) once
  unified.

## Adjacent live track (not the web loop)

- **Composite-site / owned namespaces** (design/composite-site session): Unit 1 admission merged
  (PR #14); Units 0/2 in plans. This is the owned-masthead + signed-site-manifest track — related
  but separate from the communal newswire. Its newswire render work is what became #19.

## Coordination lessons this run (for any session)

- **Cross-session PRs merge faster than comment-gating can hold them.** PR #19 merged despite a
  HOLD comment. A coordinator can comment on but not block another session's PR. If you own a web
  PR touching `apps/gateway/newswire.py`, route it through the coordinator before merge, or expect
  a corrective rebase after.
- **The `newswire.py` schema is `/2` (`riot-public-gateway-export/2`).** Do NOT reintroduce
  `riot.newswire.export/1`. One gateway, one signed schema.
- **Verify-before-ratify held every merge:** each web/Rust PR was re-run on a clean checkout
  (gates + golden inspection) before the coordinator ratified. Keep doing this.
- **Recover, don't restart, a session that dies on infra:** WS2's agent dropped on a network error
  mid-task; its green uncommitted work was recovered (it resumed and finished). Check liveness
  before touching a dropped agent's worktree.

## Ranked backlog

1. **`/2` unification graft** (in flight) — the last step to one canonical rigorous web newswire.
2. **Deploy the unified `/2` site** to the CF mirror.
3. **"Open in Riot" verify deep link** — scope after unification (app-drop may cover part).
4. **TF v2 physical two-phone test** — owner hardware.
5. **Owner ratifications (decisions):** 1A `CurrentEntryV2` (Risk 2); per-community sealed identity
   (Risk 12).
6. **Composite-site Units 0/2** — the design session's track.
7. **Hygiene:** prune stale worktrees (many live: ws1/ws1b/ws2/pr12/docs/unit*); reconcile
   `COLLABORATION.md` drift.
