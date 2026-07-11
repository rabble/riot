# User profiles & multiple identities in Willow-adjacent protocols

Research input for Riot profile design (2026-07-11). Question: how do
Willow apps and close relatives model user profiles (name/avatar) and
multiple identities per person (per-group personas)?

## Earthstar (Willow's direct predecessor)

- Profiles are **ordinary documents at conventional paths**, not protocol
  objects: `/about/1.0/~@author/displayName`, `/about/1.0/~@author/avatar.jpg`.
  The `~@author` path segment is a write-lock: only that keypair can write
  there, so profile docs are self-authenticating.
  (earthstar-project.org/community/application-formats; Standard-paths wiki)
- Identity addresses are `@` + immutable 4-char shortname + `.` + base32
  ed25519 pubkey (e.g. `@suzy.bo5so…`). Shortname exists purely to blunt
  impersonation. Hard rule: **two addresses differing in any way are
  different authors, even with identical pubkeys.**
  (earthstar-project.org/specs/data-spec-es5)
- Identities are global keypairs independent of shares; per-share persona
  variation happens via per-share `/about` documents, not per-share keys.

## Willow / willow-rs / Earthstar 11

- Willow specifies **no profile convention** — app-layer concern. Meadowcap
  communal namespaces use **user pubkeys directly as SubspaceIds**;
  ownership is proven by signature. Docs explicitly anticipate **multiple
  subspaces per user**. (willowprotocol.org/specs/meadowcap)
- Ecosystem is pre-consumer: Earthstar 11 is beta on JSR; no shipping
  Willow consumer apps found. `earthstar-project/willow-rs` on GitHub is
  archived (2025-10-23); live development is at Codeberg
  `worm-blossom/willow_rs` (matches our willow-implementation audit).

## p2panda

- Per-operation ed25519 signatures; the notable design is **Persona**:
  one keypair per device, bound into a single persona via mutual
  declaration operations (many keys → one identity). Multiple personas per
  user and shared/group personas are explicit open design goals.
  (p2panda handbook issue #15)

## Adjacent art for per-context personas

- **Matrix**: global profile + **per-room displayname/avatar overrides in
  `m.room.member`** — the closest analog to "different profile per group"
  on one identity. Chronic bug class: global-name changes clobbering
  per-room overrides (matrix-spec #103). Lesson: define precedence
  (space > global) and never let global edits overwrite overrides.
- **Nostr**: one key = one identity; kind-0 replaceable profile event;
  impersonation patched externally via NIP-05 DNS identifiers.
- **Scuttlebutt**: `about` messages; one key = one identity = one device;
  multi-device via Fusion Identities (parallel to p2panda persona).

## Privacy: one global key vs per-space keys

Key reuse is a durable correlation handle: literature recommends a fresh
key per context to prevent cross-context linking (e.g. arxiv 2511.17260,
1912.05861). For an activist app, one global key across spaces
deanonymizes a user careful in one space but exposed in another. Since
Willow's subspace = pubkey and multiple subspaces per user are explicitly
supported, **per-space keypairs are the naturally aligned
privacy-default**, at the cost of key management and portable reputation.

## Design-pattern synthesis

| Pattern | Examples | Impersonation | Correlation privacy | Key mgmt |
|---|---|---|---|---|
| Global identity + profile doc | Nostr kind-0, SSB about, Earthstar author+/about | weak (needs shortname/NIP-05) | worst | simplest |
| Per-context override, one identity | Matrix m.room.member, per-share /about | stable id, varied presentation | poor | simple |
| Per-context keypairs | Willow multi-subspace, fresh-key-per-instance | isolated per space | best | hard |
| Many keys → one identity (persona/petnames) | p2panda Persona, SSB Fusion | local naming resists | tunable | medium |

## Directly adoptable recommendations

1. Profiles as ordinary self-authenticating signed entries at an
   own-subspace profile path (our write capability already enforces the
   Earthstar `~@author` lock semantics).
2. Earthstar's display rule: never render a self-claimed name without its
   key-derived component; same-name-different-key never merges (we already
   follow this for apps in the directory design).
3. Per-space keypairs as the privacy default (Willow-aligned; core already
   has `generate_communal_author_for_namespace`); a persona/petname layer
   can later link personas across spaces opt-in.
