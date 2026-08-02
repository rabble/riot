# Riot remembers who it talks to

**Date:** 2026-08-02
**Rule:** red/green. No fix without a test that failed first.

## The gap

Riot has no memory of anyone it has ever talked to.

- The relay is a **compiled-in constant** — `AnchorRelayDefaults.relayNodeId`,
  one baked NodeId and one baked ticket.
- The persisted profile stores space, alerts, sealed identity, trusted apps,
  endorsements, app data, carried apps, and catalog generation. **No peers. No
  relays.**

So "sync with them again later" is not a missing feature, it is impossible by
construction: you cannot re-reach what was never stored. Every exchange is a
first meeting. A community that met once has no way back to each other.

This also reframes issue #107. Internet sync is two problems, and only the big
one was written down:

1. **Remember who and where** — a durable registry of peers and relays.
2. **Reach them again** — the transport.

(2) has nothing to dial without (1). And (1) needs no new crypto and no protocol
design: a persisted record, an FFI surface, a screen.

## What "remembered" has to mean

For a peer:

- a stable identity (the subspace/author key already exchanged during sync)
- how to reach them (NodeId for iroh; last-known local address for LAN)
- when we last exchanged, and what community it was about
- a name a person recognises, derived from records already synced

For a relay:

- NodeId and any dial hints
- which communities it serves, and the ticket that admits us
- when it last answered

Both must survive a relaunch, and neither may become a tracking dossier: this is
a list of people a person chose to exchange with, and it must be inspectable and
deletable by them. Emergency wipe must take it.

## Work units

### WU-1 — A durable peer registry in the core

**Red:** sync with a peer, drop the profile, reopen it, and the peer is still
known with the same identity and last-exchange time.

Persist alongside the profile, in the same SQLite database, so it inherits the
durability the journey harness already exercises. Reuse the existing sync path
to populate it — a peer is recorded when an exchange COMMITS, never on mere
discovery, so a passer-by is not remembered.

### WU-2 — A relay registry, replacing the constant

**Red:** a relay added at runtime survives a relaunch and is used for the next
pull; the compiled-in default becomes a seed, not the only possibility.

`AnchorRelayDefaults` stays as first-run seed data. Everything else reads the
registry. This is what makes "my community's own relay" expressible at all.

### WU-3 — Sync again with someone remembered

**Red:** a journey that exchanges, relaunches, and exchanges again with a
remembered peer WITHOUT rediscovery — the second exchange carries only the delta.

This is the unit that makes the whole thing worth building; WU-1 and WU-2 are
storage until this works.

### WU-4 — A person can see and forget

Nearby lists people you have exchanged with, and when. Settings lists relays.
Both offer forget. Emergency wipe clears both.

**Red:** forget removes the peer and it is gone after a relaunch; wipe clears
the registry.

## Order

WU-1 → WU-3 → WU-2 → WU-4. WU-3 immediately after WU-1 because storage nobody
re-reads is not worth having, and WU-3 proves the read path.

## Definition of done

Two devices exchange, both are closed, both reopen, and they exchange again
without either person re-scanning anything — proven by a journey test on the
durable substrate and by hand on two physical devices.
