# Anchor daemon — runnability gap & runbook (2026-07-20; updated 2026-07-22)

> **STATUS UPDATE (2026-07-22, `feat/anchor-sync2-serving`):** the gap this doc
> was written about is CLOSED. The daemon exists and runs: `riot-anchor --db
> <path>` (WU-019, `--features daemon`) serves the `riot/anchor/1` control
> plane (Describe / GetWorkChallenge / PrepareHost / **CommitHost** /
> GetOperation) and the `riot/sync/2` data path (**HostReconcileStaged** push
> into staging, **ReadCommitted** committed pull) over real iroh, on a durable
> `AnchorRepository` with database-bound secrets, persisted descriptor, and a
> single-writer deployment lease + watchdog. The original text below is kept
> for the record with its stale claims struck; the "what exists / what's
> missing" analysis it made is exactly what got built.
>
> - **Run one:** `deploy/riot-anchor/` (Dockerfile, compose.yaml, operator env
>   template, backup/restart runbook).
> - **Prove it:** `scripts/anchor/demo-cross-city.sh` — PrepareHost → sync/2
>   push → CommitHost → ReadCommitted pull, printing each stage; `--local`
>   runs a one-machine smoke pass, `--host`/`--follow <ticket>` split the two
>   cities. The demo roles are `crates/riot-anchor/examples/{demo_anchor,
>   demo_host,demo_follow}.rs`, reusing the e2e test's own fixtures/FSMs.
> - **Still true:** the mobile wiring gap (last table row) — `riot-client-net`
>   is not wired into `riot-ffi`; that is issue #107 Phase 3, blocked on the
>   Checkpoint A human demo.

**Question this answers:** what do we need to actually *run* an anchor so two users in
different cities, phones not both online, can sync? ~~**Short answer: the anchor's protocol
and state logic are done, but the entire server-process layer is unwritten.** You cannot
run an anchor today — there is no binary.~~ *(2026-07-22: superseded — the binary exists;
see the status update above.)* This doc records the exact gap, the one real
architectural decision it forces, and the smallest path to a first live cross-city demo.

Verified against `origin/main` (`c10db72`) and the active branch
`origin/overnight/2026-07-20-anchor-m2`. Companion to
`docs/coordination/2026-07-19-anchor-build-state.md` (M1/M2 build state) and the plan
`docs/superpowers/plans/2026-07-18-public-community-anchor-network-implementation.md`.

## TL;DR *(updated 2026-07-22)*

| Layer | State | Where |
| --- | --- | --- |
| M1 protocol (canonical wire, `sync/2`, `anchor/1` control, listing authority, ALPN router) | ✅ done | `riot-anchor-protocol`, `riot-transport` |
| M2 hosting core (SQLite `AnchorRepository`, control/hosting/listing/removal/checkpoint, `sync/2` adapter) | ✅ core done | `riot-anchor` |
| Daemon: assemble handlers + open an iroh endpoint + serve `sync/2`+`anchor/1` in a loop | ✅ **done (2026-07-22)** | `crates/riot-anchor/src/{main,config,daemon,admission}.rs`, feature `daemon` |
| Deploy manifests + operator runbook | ✅ **done (2026-07-22)** | `deploy/riot-anchor/`, `scripts/anchor/demo-cross-city.sh` |
| Mobile client can reach an anchor over the internet | ❌ not wired | `riot-client-net` exists, not wired into `riot-ffi` — issue #107 Phase 3 |

~~**The daemon is a planned work unit (WU-019), sequenced in Slice 7 / the M3+ deployment
tail — not an accidental gap.** The active M2 session has NOT reached it. It builds
additive new files (`daemon.rs`, `main.rs`, `admission.rs`), so it does not overlap the
M2 hosting source — but it IS on the anchor roadmap, so **it must be claimed/coordinated,
not built by a second session in parallel** (the recurring duplication trap).~~
*(2026-07-22: WU-019 was claimed and built on `feat/anchor-sync2-serving`, with the
single-writer actor design recommended below — option A.)*

## What exists (the pieces a daemon would assemble)

- **Repository open:** `AnchorRepository::open(path)` / `open_with_ceilings(path, AccountingCeilings)`
  / `open_in_memory()` — `crates/riot-anchor/src/repository.rs`. One WAL `rusqlite::Connection`,
  **single-writer, no pool.** Startup primitives: `acquire_deployment_lease`, `verify_deployment_lease`,
  `recover_readiness`.
- **Control handler (`riot/anchor/1`):** `AnchorControlService::new(ctx, policy, signer, token_ring)`
  then `handle(&self, repo: &mut AnchorRepository, request_bytes, now, entropy) -> ControlHandling`
  — `crates/riot-anchor/src/control.rs`. Repo is passed per-call.
- **Composite hosting commit:** `CommitHostService::new(ctx, authority, signer).commit(...)`
  — `crates/riot-anchor/src/hosting.rs`.
- **sync/2 adapter:** `AnchorSyncRepository::new(repo: SharedRepo, token_ring, now)` implementing
  `Sync2Repository` — `crates/riot-anchor/src/sync_service.rs`. Note `SharedRepo = Rc<RefCell<AnchorRepository>>`.
- **Transport:** `crates/riot-transport/src/iroh.rs` `bind_public(secret: [u8;32])` (public relay preset)
  + `accept_with_router(endpoint, router)` (documented "the general accept primitive a public anchor
  runs in a loop"); `crates/riot-transport/src/router.rs` `AlpnRouter::{new, register, dispatch}`;
  ALPN constants `ALPN_SYNC_V2 = b"riot/sync/2"`, `ALPN_ANCHOR_V1 = b"riot/anchor/1"` in
  `crates/riot-transport/src/lib.rs`.
- **Closest runnable scaffold:** `crates/riot-transport/src/bin/riot-seed.rs` + `seed.rs::run_seed`
  show the exact `bind_public(secret) → advertise addr → loop { accept }` bootstrap. But seed serves
  `riot/sync/1` willow byte-sync with **no `AnchorRepository`, no control plane, no sync/2** — a
  pattern to copy, not an anchor to extend.

## What's missing (WU-019, in build order) *(2026-07-22: ALL of 1–5 below now exist as described)*

1. **Assembler + CLI** — new `crates/riot-anchor/src/daemon.rs` + `main.rs`: parse config
   (`--db <path>`, identity, listen/relay), open `AnchorRepository`, `acquire_deployment_lease`,
   construct control + sync/2 services, run readiness/liveness + graceful shutdown.
2. **Anchor listen loop** — register `riot/sync/2` and `riot/anchor/1` handlers on an `AlpnRouter`
   (today **only `riot/sync/1` is ever registered anywhere**), then drive `accept_with_router` in a
   loop over a `bind_public(secret)` endpoint.
3. **Admission/ingress bounds** — new `admission.rs` + `tests/ingress_limits.rs` (WU-019 scope):
   per-partition load shedding, bounded logging.
4. **Config/identity loader** — the operator secrets the design lists (anchor operator key, **iroh
   endpoint key**, namespace-token HMAC epochs, cursor HMAC epochs, TLS key/cert, deployment
   lease credentials) loaded by path/fd. None of this is consumed by code today; `riot-anchor` has
   **no iroh/tokio dependency at all** yet.
5. **Deploy surface (WU-026)** — `deploy/riot-anchor/{compose.yaml, riot-anchor.example.toml}`,
   `scripts/anchor/deployment-contract.sh`.

## The one real architectural decision (flag before coding WU-019)

**A `Send` boundary must be bridged.** `AlpnRouter`'s `Handler` is `Arc<… Send + Sync …>` and
`dispatch` runs handlers concurrently under a semaphore. But the anchor's `sync_service::SharedRepo`
is `Rc<RefCell<AnchorRepository>>` — `!Send` — over a single non-pooled SQLite connection. The
handlers cannot be dropped onto the router as-is. Options:

- **(A) Single-writer actor task.** One task owns the `AnchorRepository`; router handlers send
  request bytes over an `mpsc` channel and await a oneshot reply. Preserves the single-writer SQLite
  model, keeps `!Send` state off the router. **Recommended** — matches the current repo design and
  the "single-writer WAL" reality; also the natural place for the deployment lease + fairness/ingress
  bounds (WU-019) to live.
- **(B) `Arc<Mutex<AnchorRepository>>`.** Simpler glue, but serializes every handler on one mutex
  (no better than the actor for a single connection) and changes `sync_service`'s ownership type,
  which is **the active M2 session's file** — avoid touching it.

Either way the decision belongs with whoever owns WU-019; option A touches only new daemon files
plus a thin adapter, so it does not edit the contended M2 sources.

**Also load-bearing:** WU-019 must add `iroh` + `tokio` to `crates/riot-anchor/Cargo.toml`. That is
a new dependency on a crate that currently has none → the `Cargo.lock` sha in `fixtures/manifest.json`
changes → `xtask validate-contracts` fails until refreshed ([[riot-cargo-lock-contract]]). Keep
iroh/tokio confined to `daemon.rs`/`main.rs`; the protocol crate `riot-anchor-protocol` forbids them
by test (`tests/dependency_boundary.rs`) and `riot-anchor`'s library core should stay transport-free.

## Smallest path to a first live cross-city demo *(2026-07-22: steps 1–3 are built and scripted — `scripts/anchor/demo-cross-city.sh`; step 4 = issue #107 Phase 3, still open)*

Goal: one anchor process + two clients, where client B pulls what client A published while A is
offline. Ordered, each independently checkpointable:

1. **WU-019 daemon (server side).** Assemble the actor + listen loop above; `riot/anchor/1` +
   `riot/sync/2` served over `bind_public`. Milestone: the daemon accepts a `sync/2` session and a
   `PrepareHost`→`CommitHost` control round-trip against a real on-disk `AnchorRepository`. Testable
   headless with a Rust client harness (no mobile needed).
2. **Host a community on it.** Drive the control plane to admit a `community_root` + its `O/C/W`
   namespaces (the `PrepareHost`/`CommitHost` flow already exists in `hosting.rs`).
3. **Client reach.** `riot-client-net` already owns a client iroh endpoint + `safe_dial`; a headless
   client harness can dial the anchor's `sync/2` and reconcile. This proves store-and-forward
   (A → anchor retains → B later) **before** any mobile work.
4. **(Later) Mobile.** Wire `riot-client-net` into `riot-ffi` + cross-compile iroh/tokio for
   iOS/Android — this is the plan's **Human Checkpoint A** and the genuinely hard/unproven bit
   ([[riot-mobile-transport-local-only]]: mobile is BLE + LAN only today, `riot-ffi` has zero iroh).
   Not required for the headless cross-city proof in steps 1–3.

Steps 1–3 give a demonstrable internet cross-city sync with **no mobile changes** — the fastest
honest proof the anchor works. Step 4 is what puts it in users' hands.

## Recommendation / coordination *(2026-07-22: resolved — WU-019 built with option A, the single-writer actor; iroh/tokio stayed confined behind the `daemon` feature; the headless demo harness exists)*

- The daemon (WU-019) is the blocker and it is **unclaimed but on the anchor roadmap**. Before anyone
  builds it: **claim it in `COLLABORATION.md` and confirm the active `overnight/2026-07-20-anchor-m2`
  session isn't about to pick it up** — it builds new files so it won't merge-conflict, but two
  sessions writing the same daemon is the exact duplication this project keeps hitting.
- Confirm the **actor-vs-mutex** decision (A recommended) and the **iroh/tokio-in-riot-anchor**
  dependency addition with the anchor lead/owner before coding — both are architectural, not glue.
- A headless Rust demo harness (steps 1–3) is the cheapest way to prove the cross-city case and
  should precede any mobile transport work.
