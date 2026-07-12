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
