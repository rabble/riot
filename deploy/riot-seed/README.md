# riot-seed — an always-on Riot testnet node

A seed holds a namespace's signed entries and reseeds them to any follower over
iroh (real QUIC, NAT-traversing). v1 is a **read-mostly origin/mirror seed**:
it serves what it holds. Ingesting follower publishes (decode + verify + append)
is the next slice. State persists, so the site identity + follow ticket stay
stable across restarts.

## Build

```bash
cargo build --release -p riot-transport --bin riot-seed --bin riot-follow
# → target/release/riot-seed, target/release/riot-follow
```

## Run locally (prove it)

```bash
# terminal 1 — the seed
RIOT_SEED_DIR=/tmp/seed RIOT_SEED_DEMO=3 ./target/release/riot-seed
# prints: node id, namespace, and a root-signed ticket (riot://site/v1/...)

# terminal 2 — a follower, using the ticket from the seed's output
./target/release/riot-follow 'riot://site/v1/…&node=…&sig=…'
# → synced — 3 entries now live in the local store
```

The follower **verifies the ticket signature before dialing** (fail-closed): a
`require:arti` ticket, or a tampered one, never opens a connection.

## Deploy on the Mac mini (always-on)

iroh does its own NAT traversal (relay + discovery) — no port-forwarding or
Tailscale required, though tailnet peers also get a direct address for free.

```bash
mkdir -p ~/riot-seed
cp target/release/riot-seed ~/riot-seed/riot-seed
cp deploy/riot-seed/com.rabble.riot-seed.plist ~/Library/LaunchAgents/
# edit the /Users/rabble/... paths in the plist if your home dir differs
launchctl load -w ~/Library/LaunchAgents/com.rabble.riot-seed.plist
```

It runs at boot and restarts on crash. Get the follow ticket:

```bash
grep 'ticket' ~/riot-seed/seed.out.log
```

Hand that ticket to any follower (another machine, phone, the web-mirror
ingest) and they sync this namespace from the mini over the internet.

To stop: `launchctl unload ~/Library/LaunchAgents/com.rabble.riot-seed.plist`

## A network (≥2 seeds)

Run `riot-seed` on the mini **and** a second host (e.g. a fly.io VM) with the
same site state (`state/site.bin` copied over), or point them at each other once
seed-to-seed peering lands. Multiple seeds holding the same namespace = the
testnet; a follower syncs from whichever it can reach, and (with Willow
set-reconcile) any follower who holds data can reseed others.

## Ports / firewall

iroh binds a UDP socket and uses n0's relays for hole-punching. Outbound UDP is
enough for most home networks; no inbound port-forward needed.
