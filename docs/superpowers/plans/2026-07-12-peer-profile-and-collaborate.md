# Peer profile & collaborate — plan / coordination

CLAIMED BY: transport/discovery session (the one that landed local-network
nearby + reliability). Touching: a NEW `apps/ios/Riot/Peers/PeerProfileView.swift`
and ONE tap-handler line in `ConferenceShellView.swift`'s `ConnectionStatusView`.
If you own the Connection screen, coordinate here before rewriting that tap.

## What the user asked for

From a discovered/connected peer (e.g. "PATIENT BROOM" on the Connect screen):
tap them → see **their profile** and **their collections** → **collaborate**:
invite them to my spaces, endorse/adopt what they carry.

## The data we already have (map to existing FFI — do not reinvent)

- **Peer identity**: pairing yields a peer with a rendered name. Every id this
  device can name is in `RiotProfileRepository.displayNames()` →
  `[subspaceHexId: renderedName]` (already run through `render_display_name`, so
  it is `"Ana · a3f91122"`, never a bare key). The stable thing is the subspace
  id; the name is a claim that can change (see `profile_ffi.rs`).
- **Their profile**: Rust `profile_for(id)` resolves a peer's profile card
  (Earthstar model: profile is a document at a conventional path, self-
  authenticating via the `~@author` write-lock). Surface it through the repo.
- **Their collections**: the app directory already attributes apps to authors.
  `RiotDirectoryRow.author` is "who carries/authored this," and `.endorsement`
  is "recommended by …". A peer's collection = the directory listings whose
  author is that peer's subspace id. No new store needed — filter what
  `directoryListings()` already returns.
- **Space membership**: `joinSpace` exists, and a phone with no space auto-joins
  its peer's space on pair (commit 8dfbbe0).

## What is MISSING (flag for the core/FFI owner)

1. **Explicit "invite peer P to space S"**. Today membership spreads by
   auto-join-on-pair; there is no addressed invite. Options, cheapest first:
   a. Reuse auto-join: "invite" = share the space so the peer's device joins on
      next sync (no new FFI; weakest — it is really "share", not "invite").
   b. A signed invite record in the space namespace naming the peer's subspace
      id, which their device honors on sync. Needs a small core addition.
   Recommend (a) for the demo, (b) as the real design.
2. **Per-peer profile fetch** surfaced to Swift: wrap `profile_for(id)` on
   `RiotProfileRepository` returning `{displayName, tag, joinedSpaces?}`.

## UI slice (this session is building)

`PeerProfileView` — a sheet presented when a peer row is tapped on the Connect
screen (after/independent of pairing):

- Header: rendered display name + tag (monospace, Riot style).
- "Their collections": the directory rows authored by this peer, each with the
  existing review/endorse affordances (reuse `DirectoryView` card semantics).
- "Collaborate": a button per one of my spaces — "Invite to {space}" — wired to
  the invite path chosen above (starting with share/auto-join).
- Plain language only (per app rule): no "subspace", "namespace", "key".

## Open questions for the group

- Does "tap a peer" mean *before* pairing (browse-time preview) or *after*
  (connected)? Pre-pair we only know the friendly name; the real profile needs a
  synced profile card, i.e. post-connect. Proposing: tap → connect → profile.
- Multi-identity/per-space personas (see
  `docs/research/2026-07-11-user-profiles-willow-research.md`): a peer may present
  a different name per space (Matrix-style override). The view should show the
  name for the *current* space context, not a global one, when that lands.

## Two-node headless test findings (transport/space owner: please pick up)

Verified with the new hooks (`5714970`): `RIOT_SEED_SPACE=1` on a host + a fresh
joiner, both `RIOT_AUTO_DISCOVER=1 RIOT_AUTO_CONFIRM=1`, launched via `open -n`.

1. **TCC Local Network crash (environmental, not code):** first two-node run can
   SIGABRT one instance with `__TCC_CRASHING_DUE_TO_PRIVACY_VIOLATION__` (the
   Local Network prompt cannot be answered headlessly). Once the permission is
   granted for the app, both instances survive. Real demo: grant it once.
2. **Joiner does not join (open bug):** even with both instances alive and
   discovering each other (dns-sd shows 2 services), a fresh joiner never lands
   in the host's seeded space — `joiner: space=NONE` after 18s. The break is in
   auto-connect -> SpacePairing -> confirmJoinSpace, NOT in the peer-profile UI
   or these test hooks (host-with-seed-space is stable solo; transport pairing is
   20/20 in the standalone harness). Someone who owns NearbyTransportController /
   SpacePairing should trace why the join does not complete on a real socket.

Until (2) is fixed, the peer-profile People list / collections cannot populate
between two strangers, because nothing syncs without a shared space.

## Root-cause dig into joiner-never-joins (traced 2026-07-12)

Instrumented the auto-connect -> pair -> join chain with file traces. Findings:

**BUG 1 (FIXED, c8b3299): auto-connect never fired.** `autoConnectToFirstPeer()`
guarded `case .idle = state`, but `findNearby` leaves state at `.looking` and a
peer is only discovered while looking — so the guard was always false. No device
ever auto-connected. Fixed to allow `.idle` OR `.looking`. Traces now show
`autoConnect -> requestConnection`, `startLocalSession`, `beginSpaceHandshake`
on both sides — the transport connects.

**BUG 2 (OPEN, transport/startup owner): host advertises before its space
exists.** At `beginSpaceHandshake`, `mySpace=nil` on the HOST even though it has
a space moments later. `findNearby` (ConnectionStatusView.onAppear) races
`bootstrap` (RiotMacApp `.task`), so a phone can start advertising/pairing before
its profile/space is ready. A phone with no space announces nil, so the peer has
nothing to adopt -> the joiner stays spaceless. Real fix: do not start
discovery/advertising until the repository is open (and, for a host, gate on
`currentSpace != nil` or re-announce when the space arrives). NOTE: RIOT_SEED_SPACE
exacerbates this (it seeds in bootstrap); the real demo taps "Create space" first,
so a real host usually has a space — but the race is genuine.

**BUG 3 (OPEN): space handshake ends in `.failed` with no decision.** After
`beginSpaceHandshake`, no `settle(...)` fires and state goes `.failed` — SpacePairing's
`onFailure` path, not a `.nothingToShare`/`.adopt` decision. Needs a trace inside
SpacePairing.receive/fail over the local-network channel to see whether the peer's
SpaceAnnounce arrives malformed or the connection drops during handoff. Owner of
SpacePairing / the LocalNetwork channel handoff should take this.

Repro (both bugs 2+3 visible): the two `open -n ... RIOT_SEED_SPACE/AUTO_*` lines
above; read /tmp/riot-trace-<id>.log with the file-trace instrumentation.
