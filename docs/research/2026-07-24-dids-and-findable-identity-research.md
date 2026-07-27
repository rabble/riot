# DIDs & Findable Identity for Riot (Willow)

**Date:** 2026-07-24
**Status:** Decision made + implemented (Earthstar handle model adopted)
**Scope:** Research → decision → implementation of self-certifying, human-readable
identity handles for spaces, communities, and authors in Riot.

---

## TL;DR

Riot adopted **Earthstar's identity/naming model** as an app-layer convention:
self-certifying handles of the form `@<shortname>.<52-char base32 key>` for
authors and `<sigil><shortname>.<52-char base32 id>` for spaces (`+` communal /
`-` owned). The suffix encodes the **full 32-byte key**, so the handle is
verifiable from the string alone — no DID resolver, no network, no registry. The
shortname is decorative. This augments (does not replace) Riot's existing 8-hex
display tag.

This is **not** a dependency on Earthstar software (JS, LGPL, stalled beta) and
makes no wire-level interop claim (different ciphersuite). It borrows the
*format and model*, which is exactly the layer Willow's spec leaves to the app.

---

## 1. Where Riot started

Riot's identity is purely cryptographic: 32-byte Ed25519 keys via `willow25
0.6.0-alpha.3`. No DIDs, no `PeerId`, no petnames anywhere.

- **Spaces = Willow namespaces**, identified by `NamespaceId` (`[u8;32]`). Two
  flavors: `Communal` (privilege-less) and `Owned` (namespace id IS the root
  pubkey). The kind is encoded in the **final byte of the id** (even = communal,
  odd = owned) — load-bearing, checked everywhere.
- **Users/authors = subspaces**, identified by `SubspaceId`. Per-community key
  isolation is a **deliberate privacy default** (fresh author per community to
  prevent cross-community linkability).
- **Transport = Iroh** (`1.0.2`); nodes also have an Iroh `NodeId` (Ed25519).

Existing findability mechanisms (none a stable human-readable ID):
1. `riot://newswire/join/v1/<ns>/<entry>/<digest>` — community join share.
2. `riot://site/v1/<namespace>?root=...&url=...&node=...&sig=...` — site ticket.
3. Anchor-network `CommunityListingV1` — public directory of sites.

Display today: `"<name> · <8-hex-tag>"` — the tag is the first 4 bytes (32 bits)
of the subspace id, explicitly **not** self-certifying ("cheap to grind, must
not carry an authorization decision").

---

## 2. The protocol landscape

### Willow (the spec Riot is built on)

Willow's data model is **parameterised**: `NamespaceId`, `SubspaceId`,
`AuthorisationToken` are generic types. The spec explicitly refuses to choose:
*"Should namespaces be identified via human-readable strings, or via public keys?
That depends entirely on the use-case. To sidestep such questions, the Willow
data model is generic over certain choices of parameters."* You can use "strings,
256-bit integers, urls, or iris scans."

**Discovery is entirely the app's job.** Willow's Confidential Sync assumes a
byte-stream connection already exists; it does private interest-overlap
detection *after* connection. There is no DHT, no relay protocol, no
namespace-address scheme. Finding where to connect for a given namespace/user is
100% out of scope for the protocol.

### p2panda

Identity = bare Ed25519 pubkey (`Author = VerifyingKey`; `NodeId = VerifyingKey`
too). No DIDs, no names, no key derivation. Every interesting id is a BLAKE3
hash in hex (self-certifying but not human-findable). **No public discovery
path** — discovery is deliberately confidential-by-default (Private Set
Intersection over secret 32-byte Topics, side-channel sharing). The 2024–2026
roadmap moves *away* from public findability (Tor, confidential network-id
exchange, node allow/deny lists). Confirms "findable public identity" is an
app-layer decision.

### Earthstar (Willow's predecessor)

Earthstar used self-describing addresses: `+gardening.<hash>` for shares and
`@suzy.<hash>` for authors — a **human-readable prefix + cryptographic suffix in
one string**, self-certifying. This is the closest direct precedent to what Riot
wants.

### The fundamental tension

**No single method is both self-certifying *and* a native public URL.** Every
successful system (Nostr, Iroh, Earthstar) stacks two layers: *the key is the
identity; the DNS/web/relay name is an optional, revocable, cacheable hint.*
This is the right stance for Riot and is consistent with its privacy stance.

---

## 3. DID methods (evaluated, mostly rejected for now)

| Method | Self-certifying? | Network? | Human-readable? | Public URL? | Key rotation? |
|---|---|---|---|---|---|
| Raw Willow key (have it) | Yes | No | No | No | Via Meadowcap |
| **did:key** | Yes | No | No | No | **No** |
| **did:web** | No (DNS+HTTPS) | Yes | Yes | **Yes** | Yes |
| **did:webvh** | Partial (SCID) | Yes | Yes | **Yes** | Yes (+pre-rotation) |
| did:ion (Bitcoin) | Partial | Yes | No | No | Full lifecycle |
| did:pkh (wallet) | Yes | Yes | No | No | Via wallet |
| Nostr NIP-05 | No (DNS hint) | Yes | Yes | **Yes** | N/A (re-points) |

- **did:key** is strictly weaker than the raw key Riot already has (no rotation).
  Only useful as a base under another layer.
- **did:web / NIP-05** are the natural fit for an *optional public URL* layer
  (deferred — see §6).
- **Blockchain naming** (did:ion/pkh, ENS, Namecoin, Handshake) adds heavy
  dependencies solving a problem DNS already handles. Skipped.

---

## 4. Why did Willow move away from Earthstar's naming?

Grounded in primary sources (Gwil's Meadowcap intro; the Willow data-model spec;
the Earthstar v10 changelog):

1. **Access control was tangled with naming (explicit).** In old Earthstar, the
   share address *was* the public key: knowing it granted irrevocable read; the
   private key granted irrevocable write. Gwil: *"This made the leaking of a
   share's public and private key a daunting and somewhat inevitable prospect. I
   would very much like to put this model behind us."* When Meadowcap replaced
   it with capability tokens, the human-readable prefix dropped as a *side
   effect* (v10 changelog: "Share addresses are now the public key of a share
   keypair").

2. **Generality / parameterisation (explicit in the spec).** Willow sidesteps
   the human-readable-vs-key choice by leaving `NamespaceId`/`SubspaceId` as
   parameters. Earthstar's "sin" was baking one naming convention *into the
   protocol*; Willow leaves it to the app.

**What is NOT documented as a motivation** (despite being true): the collision
problem, the cosmetic-name issue, metadata leakage. The Earthstar specs concede
shortnames are "non-unique" and "somewhat memorable" — so the problems were
*known* — but no Willow-era source cites them as the reason for the redesign.

**Implication for Riot:** Willow didn't drop the naming because it was wrong —
it dropped it because it shouldn't be a *protocol* concern. Reintroducing an
Earthstar-style scheme at Riot's **application layer** is using Willow as
designed: filling in the parameter the spec leaves to you.

---

## 5. Webfinger vs NIP-05 (relevant for the deferred public-URL layer)

- **NIP-05 is a parallel re-invention of Webfinger**, not a specialization. The
  NIP-05 spec never references RFC 7033; it defines its own
  `.well-known/nostr.json` returning a flat `{names, relays}` object. Simpler,
  static-hostable, but no interop.
- **Webfinger (RFC 7033)** is IETF standards-track, deployed across
  Mastodon/ActivityPub, OpenID Connect, XMPP. It natively carries keys (in
  `properties`, values are free-form strings) and multiple service endpoints
  (repeated `links`).
- For Riot's deferred public-URL layer, **Webfinger is the more "Willow-like"
  choice** — standards-based, compositional, federates with the fediverse. The
  Earthstar-style handle stays the canonical self-certifying form; Webfinger is
  the optional public face.

---

## 6. The decision: adopt Earthstar's identity model

**Adopt Earthstar's identity/DID/username model** — the `@author.suffix` /
`+share.suffix` / `-share.suffix` self-certifying human-readable addressing — as
a Riot app-layer convention. Knowing Earthstar's software is the older,
abandoned version of what became Willow.

### Why this is the right call

- Riot already encodes communal/owned as the final byte of the namespace id.
  Earthstar's `+`/`-` sigil is a free human-readable rendering of that bit,
  cross-checked on decode. Riot already has the byte-level distinction; it just
  lacked the human-readable encoding.
- Riot's display convention is already "name is decorative; key is identity."
  Earthstar's model matches exactly.
- Riot is **already a peer implementation** of the Earthstar/Willow app-layer
  concept in Rust (communal/owned, path-scoped delegated caps, share refs, sync
  servers). It's not adopting from scratch — it's converging on a shared
  convention, with a *stronger* privacy model (per-community isolation that
  Earthstar lacks).
- It's **dead easy**: self-certifying, pasteable, QR-able, works offline.

### What was implemented

New module `riot_core::identity` with:
- `AuthorHandle`: `@<shortname>.<52-char base32 subspace key>`
- `SpaceHandle`: `<sigil><shortname>.<52-char base32 namespace id>`, sigil `+`
  (communal) / `-` (owned) derived from the id's final byte.
- Suffix = full 32-byte key → 52 lowercase base32 chars (self-certifying).
- Shortname = 3..=32 chars of `[a-z0-9-]` (interior hyphens only), decorative,
  non-unique (equality is by key only).
- Case-insensitive decode, lowercase encode.
- Sigil/parity cross-check: a `+` on an odd-final-byte id is rejected.

FFI augmentations (non-breaking): `my_author_handle`, `mint_space_handle`,
`parse_author_handle`, `parse_space_handle`. The existing 8-hex tag and
share-ref/ticket URIs are **unchanged** — the handle augments for out-of-band
sharing.

### What was NOT done (explicitly deferred)

- Webfinger/did:web public-URL layer (separate decision).
- Findable *user* identity spanning communities (conflicts with per-community
  isolation — the persona layer).
- Replacing the 8-hex tag (augment only).
- Any crypto/willow25/sealed-identity change.
- Wire-level interop with Earthstar v11 (cinn25519 vs ed25519 — format only).

---

## 7. Note on pre-existing working-tree state

During implementation, the working tree contained unrelated uncommitted work
(site-ticket `onion` field, transport changes, docs) that predates this task.
That pre-existing change to `site/ticket.rs` (`canonical()` gaining an 8th arg)
trips clippy's `too_many_arguments` lint. It is **not** from this identity work
and was left untouched.

---

## Sources

- Willow Data Model spec — https://willowprotocol.org/specs/data-model/index.html
- Meadowcap spec — https://willowprotocol.org/specs/meadowcap/index.html
- Willow Confidential Sync — https://willowprotocol.org/specs/confidential-sync/index.html
- Meadowcap intro (Gwil) — https://gwil.garden/posts/meadowcap-intro.html
- Earthstar how-it-works — https://earthstar-project.org/docs/how-it-works
- Earthstar future (capabilities) — https://earthstar-project.org/docs/future
- Earthstar v11 beta (JSR) — https://jsr.io/@earthstar/earthstar
- willow_rs (active Rust substrate, Codeberg) — https://codeberg.org/worm-blossom/willow_rs
- p2panda access control — https://p2panda.org/2025/07/28/access-control.html
- W3C DID Core v1.0 — https://www.w3.org/TR/did-core/
- did:key spec — https://w3c-ccg.github.io/did-key-spec/
- did:web spec — https://w3c-ccg.github.io/did-method-web/
- did:webvh v1.0 — https://identity.foundation/didwebvh/
- Nostr NIP-05 — https://nips.nostr.com/5
- Nostr NIP-65 — https://github.com/nostr-protocol/nips/blob/master/65.md
- Iroh global node discovery — https://www.iroh.computer/blog/iroh-global-node-discovery
- Webfinger (RFC 7033) — https://datatracker.ietf.org/doc/html/rfc7033
