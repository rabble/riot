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

## Design finding, 2026-08-02: "peer" means two different things

`SyncOutcome` carries no peer identity — only `kind`, `entries`,
`rejection_code`, `terminal`, `import_bundle_bytes`. The core does not have a
concept of "who I just exchanged with", and that is not an oversight:

- **People** are already remembered. Every signed entry carries its author, the
  store keeps them, and the People screen is built from exactly that. Riot does
  NOT need a new record to remember humans.
- **Devices** are what is forgotten. How to reach a phone again — a NodeId, a
  BLE identifier, a last-known local address — is TRANSPORT state. It lives in
  `AndroidNearbyController` / `NearbyTransportController`, above the core, and
  it is discarded when the session ends.

So the thing to build is a **reachability registry**, not a people registry, and
the naming in the rest of this plan should be read that way. It answers "how do
I get back to that device", and it is keyed by the transport identity, with the
Willow authors we learned from that exchange recorded alongside so a person sees
a name rather than a hex string.

**Where it belongs is a real decision, not a detail:**

- *In the core*, as a persisted record: survives with the profile, works the
  same on both platforms, participates in emergency wipe automatically. But the
  core currently has no transport concepts at all, and pushing NodeIds into it
  couples Willow storage to iroh/BLE addressing.
- *In the native layer*, beside the profile: keeps the core transport-free, but
  must be written twice (Swift and Kotlin), and must be wired into wipe by hand
  on both.

Recommendation: **core**, as an opaque `reachability_hint: Vec<u8>` the core
never interprets. The core stores and returns bytes; the transport layer is the
only thing that understands them. That keeps Willow free of iroh while giving
the registry durability, cross-platform parity, and wipe for free.

## Resolution, 2026-08-02: peering is the consent act

Correcting the two revisions above. You do not remember an ADDRESS — iroh
resolves a stable NodeId to a path through discovery and relays, exactly as Tor
resolves an onion address. There is no routing state to keep. What you remember
is a 32-byte identity.

But `iroh.rs` makes followers **deliberately ephemeral** — "EPHEMERAL NodeId
(§5.4), reducing cross-session linkability" — so remembering a phone means
opting out of a privacy property that was chosen on purpose. Automatic peer
memory would quietly build the correlation graph Riot exists to avoid.

**Peering resolves it.** Two people explicitly become peers; that act is when
NodeIds are exchanged, and it is what makes remembering legitimate rather than
surveillance. It also answers the question left open by the 2026-07-23 transport
decision ("peer-to-peer for known peers" — but how do known peers swap NodeIds).

**Use a stable NodeId PER PEER, not one global stable NodeId.** A single stable
identity is trackable by everyone who ever sees it; a fully ephemeral one is
unreachable. A distinct stable key per friendship gives reachability to that
friend and correlation to nobody. This is the same shape Riot already uses for
records — per-community authors that are random and unlinkable by design —
applied to transport instead.

What this makes buildable, in order:

1. **Relays and seeds first.** They have stable NodeIds by design, remembering
   them carries no privacy cost, and today the relay is a compiled-in constant
   with no way to add or persist another. This is the whole of the current gap
   for "sync with a server again".
2. **Peering second**, as an explicit flow: exchange, name each other, store the
   per-peer key. Nothing is remembered without it.

Open questions this does NOT settle, and they are design work, not coding:

- how a peering act is performed (QR in person, over an existing community,
  out of band)
- whether a peer relationship is per-device or per-person across their devices
- what unpeering means for records already exchanged (nothing can be recalled)
- how iroh reaches a backgrounded phone at all — still open from 2026-07-23

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
