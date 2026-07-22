# riot-anchor deployment

Run one public Riot community anchor: the always-on daemon that serves the
`riot/anchor/1` control plane (Describe, GetWorkChallenge, PrepareHost,
CommitHost, GetOperation, ...) and the `riot/sync/2` data path (host push into
staging, committed pull for followers) over iroh QUIC, backed by a single
SQLite database.

Configuration authority: `crates/riot-anchor/src/config.rs` (module docs list
every variable). The database path is the **`--db` CLI flag**, not an
environment variable.

## 1. Generate keys

Two independent 32-byte secrets, 64 hex chars each:

```sh
openssl rand -hex 32   # -> RIOT_ANCHOR_OPERATOR_KEY_HEX (required)
openssl rand -hex 32   # -> RIOT_ANCHOR_ENDPOINT_KEY_HEX (recommended)
```

- **Operator key** — signs the anchor descriptor and every receipt; on first
  boot it derives the proposals for the database-durable secrets and the
  deployment token binding the database to this deployment.
- **Endpoint key** — the public endpoint's stable iroh NodeId. Without it the
  daemon warns (`no RIOT_ANCHOR_ENDPOINT_KEY set; using an EPHEMERAL endpoint
  identity`) and clients lose the address on every restart.

Keep both out of the shell history and the repo; back the operator key up.

## 2. First boot

```sh
cd deploy/riot-anchor
cp riot-anchor.example.env riot-anchor.env   # fill in the two keys
docker compose up -d --build
docker compose logs -f riot-anchor
```

On a fresh database the daemon persists (migration 4, `anchor_secrets` table,
first-write-wins): the **genesis random** (fixes the anchor id) and the
**namespace-token secret** (outstanding PrepareHost operations re-derive their
sync tokens from it). It also persists the **epoch-0 descriptor bytes** on
first serve. Together these make identity database-bound — see below.

Without docker, the same daemon runs directly:

```sh
cargo build --release -p riot-anchor --features daemon --locked
RIOT_ANCHOR_OPERATOR_KEY_HEX=... RIOT_ANCHOR_ENDPOINT_KEY_HEX=... \
  ./target/release/riot-anchor --db /var/lib/riot-anchor/anchor.sqlite3
```

## 3. Restart-identity guarantee

Anchor identity survives restarts **because it lives in the database, not in
the config**:

- The persisted descriptor is reused verbatim at the same epoch — a restart
  with changed metadata (e.g. a new display label) serves the
  already-persisted epoch-0 descriptor rather than equivocating with a second
  digest for the same epoch.
- The database-bound secrets (migration 4) win over anything re-derived from
  the environment: an operator-key edit cannot silently change the anchor id
  or orphan outstanding namespace tokens. Durable admission floors (ticket
  transport epoch, manifest rollback floors) survive restart the same way.

Corollary: **the SQLite database file IS the anchor's identity.** Losing it
means a new anchor; restoring one backup into two live processes is the clone
scenario the deployment lease exists to refuse.

## 4. Backup

The repository is a single-writer SQLite database in **WAL journal mode**
(`repository.rs`: `journal_mode = WAL`, `synchronous = FULL`). Committed data
can live in the write-ahead log (`anchor.sqlite3-wal`) as well as the main
`anchor.sqlite3` file, so a backup that copies only the main file can silently
drop the most recent commits. The daemon does **not** issue an explicit
`wal_checkpoint` on shutdown — it relies on SQLite's implicit last-connection
checkpoint — so do not assume the WAL is empty even after a clean stop.

Back up **while the daemon is stopped** (copying under a live writer can capture
a torn snapshot) and copy all three files together — the main database, the WAL,
and the shared-memory index — so the snapshot is self-consistent:

```sh
docker compose stop riot-anchor
docker run --rm -v riot-anchor_anchor-db:/db -v "$PWD":/out debian:bookworm-slim \
  sh -c 'cp /db/anchor.sqlite3 /out/ 2>/dev/null; \
         cp /db/anchor.sqlite3-wal /out/ 2>/dev/null; \
         cp /db/anchor.sqlite3-shm /out/ 2>/dev/null; true'
docker compose start riot-anchor
```

(`-wal`/`-shm` may be absent after a clean stop that checkpointed and removed
them — the `cp` for a missing file is harmless. Copying all three whenever they
exist is the simplest guidance that is correct in every case.) Restore the same
three files together into the volume before starting the daemon.

Back up `riot-anchor.env` (the operator key) alongside it: the database binds
to the deployment token derived from the operator key, so a restore under a
*different* operator key is refused (`LeaseTokenMismatch`).

Never run two daemons against one restored database — see below.

## 5. Restarts, crashes, and the deployment lease

The daemon holds a single-writer **deployment lease** in the database
(TTL = `LEASE_TTL_SECS` in `crates/riot-anchor/src/config.rs`, currently
**300 s**, renewed every TTL/3 by the same watchdog tick that proves the
writer thread is alive):

- **Clean shutdown** (SIGINT or SIGTERM) relinquishes the lease in place — an
  immediate restart takes it without waiting. The daemon catches **both**
  SIGINT (Ctrl-C) and SIGTERM, and SIGTERM is what `docker compose stop` and
  `docker compose restart` send, so those commands exit cleanly (code 0/143,
  not the 137 of a SIGKILL) and the very next start takes the lease with no
  `LeaseHeld` wait. `compose.yaml` sets `stop_grace_period: 30s` so the
  relinquish + clean SQLite close has time to run before Docker would escalate
  to SIGKILL.
- **Crash / SIGKILL** leaves the lease standing. A restart within the TTL is
  refused `LeaseHeld` and exits; `restart: unless-stopped` keeps retrying, so
  the outage after a hard crash is **bounded by the lease TTL (≤ 300 s)**,
  after which the expired lease is taken normally.
- **Same-config double-start** (two daemons, one database — e.g. a second
  `docker compose up` elsewhere on a shared volume, or a restored backup
  started while the original still runs): the second process presents the
  same deployment token but a **different per-process holder id**, is refused
  `LeaseHeld`, and **exits fatally instead of forking the database**. This is
  deliberate; do not "fix" it by sharing state files.

## 6. Smoke test

`scripts/anchor/demo-cross-city.sh` drives the full hosting lifecycle
(PrepareHost → sync/2 push → CommitHost → ReadCommitted pull) against a
running anchor — `ANCHOR_NODE_ID=<endpoint key's public id>` for a deployed
anchor, or `--local` for a self-contained one-machine run.
