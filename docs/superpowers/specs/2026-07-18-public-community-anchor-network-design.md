# Public Community Anchor Network Design

Date: 2026-07-18
Status: Approved in brainstorming; pending metaswarm design review
Scope: Public communities, publications, discovery, web mirroring, and
opportunistic internet sync

## Purpose

Riot already supports local-first signed state, nearby exchange, deterministic
multi-node convergence, public web rendering, and an iroh transport adapter.
What it lacks is a reliable on-ramp when the relevant phones and desktops are
not simultaneously reachable peer to peer.

This design adds plural, interchangeable **anchors**: always-online Riot peers
that retain public community state, help users discover explicitly listed
communities, seed the same reconciliation protocol used by ordinary peers, and
render public communities at normal web URLs.

Anchors improve reach. They do not become community identity, authority, or a
required dependency. An anchor may disappear permanently without invalidating a
community or requiring a migration.

## Decisions

1. **Riot-native always-on peers, not canonical homes.** An anchor runs the same
   community reconciliation protocol as a client and stores the same signed
   public state. A community is identified by its root/namespace and manifest,
   never by its host.
2. **Plural anchors from the first release.** Clients ship with a removable
   default set, may add or remove anchors, and connect to several. No default
   anchor is canonical.
3. **Client multi-homing plus anchor gossip.** Clients reconcile independently
   with several anchors. Anchors may also reconcile with configured anchor
   peers. Neither path depends on the other.
4. **Hosting and discovery are separate.** A public community can be hosted and
   shared by ticket without being searchable. Search requires a separate,
   owner-authorized, expiring listing.
5. **Open hosting.** Any protocol-valid public community may request hosting
   without an account, invitation, payment, or editorial approval. Published
   resource limits still apply.
6. **Web mirror included.** Each anchor serves directory pages, readable
   community mirrors, QR/share tickets, and “Open in Riot” links alongside the
   native sync endpoint.
7. **Meadowcap is enforced at every ingress.** Anchors validate entry signatures
   and complete capability chains before retaining, indexing, serving, or
   gossiping state.
8. **Public scope only.** This design does not implement private-group
   rendezvous, encrypted mailboxes, MLS membership, or read confidentiality.

## Lessons Incorporated

The design combines three proven ideas without adopting any source system
wholesale:

- From AT Protocol: hosted infrastructure can make public data reliably
  available and provide useful search and web views, while signed repositories
  keep data verifiable. Riot does not adopt the PDS as a canonical community
  home.
- From Nostr: clients should be able to choose several interchangeable hosts
  and include replaceable location hints with stable signed identifiers. Riot
  does not rely on every client independently guessing compatible read/write
  relay sets.
- From Briar, Secure Scuttlebutt, and Riot's existing research: local sync and
  internet backhaul must use one transport-independent reconciliation
  primitive. There is no separate “server truth” merge path.

Primary references:

- <https://atproto.com/guides/the-at-stack>
- <https://github.com/nostr-protocol/nips/blob/master/65.md>
- <https://github.com/nostr-protocol/nips/blob/master/66.md>
- <https://github.com/nostr-protocol/nips/blob/master/77.md>
- `docs/research/2026-07-11-hybrid-gossip-backhaul-research.md`
- `docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`

## System Shape

```text
                                ordinary HTTPS
                                      |
                              +-------v-------+
                              | web + search  |
                              | derived views |
                              +-------+-------+
                                      |
                          +-----------v------------+
                          | Riot anchor             |
                          |                         |
                          | sync  store  directory  |
                          | mirror gossip limits    |
                          +-----+-------------+-----+
                                |             |
                      same sync |             | same sync
                                |             |
                   +------------v--+       +--v-------------+
                   | phones/desktops|       | other anchors  |
                   +-------+--------+       +--------+-------+
                           |                         |
                           +------ nearby/files -----+
```

The arrows all carry the same signed community state through the existing
reconciliation and preview/admission boundaries. Transport changes how bytes
move, not how state becomes valid.

## What an Anchor Is

An anchor is one deployable service with six isolated modules.

### Sync

Accepts the existing Riot reconciliation protocol over an internet-capable byte
channel. The native path uses `ByteSyncSession` over the existing
`riot/sync/1` iroh ALPN. The sync module does not interpret transport origin
when reconciling state.

### Store

Retains bounded public Willow entries, capabilities, manifests, directory
records, and content-addressed payloads. Authoritative state is signed protocol
data; database tables and indices are disposable storage projections.

### Directory

Validates owner-authorized community listings, retains them under explicit
budgets, and builds a local browse/search index. It merges configured plural
directory feeds and deduplicates by full community identity.

### Mirror

Uses the existing gateway renderer to serve hosted public communities at
ordinary HTTPS URLs. Every page exposes the full community identity through an
“Open in Riot” link and QR/share ticket. The web mirror never handles private
group data.

### Gossip

Runs scheduled, bounded reconciliation against configured anchor peers.
Directory feeds are reconciled first. Community state is reconciled only for:

- communities the anchor already hosts;
- communities explicitly requested by a client;
- communities selected by a configured replication relationship; or
- a bounded, operator-configured subset of recent listings.

There is no mandatory global firehose and no requirement that every anchor
store every community.

### Limits

Applies resource budgets and backpressure before expensive parsing, allocation,
signature verification, payload retrieval, or gossip fanout.

## What an Anchor Is Not

An anchor is not:

- a PDS or canonical community home;
- a community root, capability issuer, signer, or key-recovery service;
- a trusted source of truth;
- a global moderation authority;
- a guarantee of permanent retention;
- an endorsement of listed communities;
- an iroh packet relay.

Iroh packet relays forward encrypted packets between endpoint IDs and assist
NAT traversal. They do not retain, index, reconcile, or render Riot application
state. The current `riot-transport` adapter deliberately uses
`N0DisableRelay`; a public anchor can be reached at a stable public address
without first deploying a custom iroh packet-relay network. Packet-relay policy
may evolve independently.

## Identities and Records

### Community identity

The stable identity is the community root/namespace plus its admitted manifest.
No hostname or anchor ID participates in community authority.

### `AnchorDescriptorV1`

An anchor-operator-signed descriptor advertises:

- schema and version;
- full anchor signing key ID;
- HTTPS origin;
- stable iroh endpoint ID and addressing information;
- supported sync and record versions;
- supported roles: hosting, directory, mirror, gossip;
- published hard-limit profile or its canonical digest;
- issued and expiry times;
- signature.

It is served from a well-known HTTPS route and may also circulate through
signed anchor-directory feeds. Expired descriptors are not used for new
connections.

### `CommunityListingV1`

An owner-authorized listing contains:

- schema and version;
- full community root and namespace IDs;
- manifest digest and monotonic manifest version;
- title and concise summary;
- bounded topic tags;
- supported languages;
- optional coarse region;
- listing revision;
- issued and expiry times;
- signer and capability proof through the normal Willow/Meadowcap envelope.

The listing is signed by the community root or by a key with an explicit,
time-bounded listing capability. It is not inferred from hosted content.

Each anchor includes accepted listing bytes in its own signed public directory
feed. Other anchors validate both the directory-feed entry and the inner
community listing. Search merges multiple feeds and selects the newest valid
listing revision per full community identity.

### `HostingReceiptV1`

An anchor-signed hosting receipt reports:

- full anchor ID;
- full community identity;
- status;
- accepted state frontier/heads;
- payload coverage state;
- acceptance time;
- promised retention horizon;
- applicable limit-profile digest;
- signature.

Statuses are a closed, versioned set:

- `accepted`;
- `partial`;
- `over_quota`;
- `expired`;
- `not_hosted`;
- `unsupported_version`;
- `invalid_authority`.

A receipt is evidence of what one anchor says it currently retains. It grants
no community authority and does not guarantee that the anchor will remain
online.

## Share Tickets and Location Hints

A share ticket carries:

- the signed community root and namespace;
- expected manifest digest/version;
- the root-signed transport floor, epoch, and expiry;
- a bounded list of replaceable anchor hints;
- an optional web-mirror hint.

Anchor and web hints are locations, not identity. They are intentionally
replaceable and are not used to authorize retrieved state. A client may try
them, but accepts data only after verifying the signed transport floor,
community identity, manifest, capabilities, entries, and payload digests.

The existing `riot://site/v1` ticket already treats `node` as an untrusted
seeding hint while signing the transport floor. Its compatible evolution may
carry repeated bounded hints; older clients may use a single hint while newer
clients combine all hints with their configured anchors.

An attacker-controlled hint can observe a connection or withhold data. It
cannot make another namespace satisfy the expected community root and digest.
The signed transport floor remains the pre-dial defense against stripping a
Tor-only requirement to force a clearnet connection.

## Meadowcap Admission Boundary

Every anchor independently performs all admission checks before data enters
retained storage or any derived view:

1. Parse under strict byte, depth, count, and path bounds.
2. Verify the namespace and admitted community manifest.
3. Verify that the manifest selects an explicitly supported public,
   read-open community profile. Unknown, gated-read, and private profiles fail
   closed before storage.
4. Verify the complete Meadowcap delegation chain.
5. Verify that the signer holds authority for the claimed subspace.
6. Verify that the entry path is within the delegated prefix.
7. Verify delegation, capability, entry, and listing time bounds.
8. Verify payload digest and entry signature.
9. Verify manifest and transport epoch floors against durable local floors.
10. Apply object-type and anchor resource limits.
11. Commit atomically only after all checks pass.

For an owned publication namespace, only valid delegated capability holders may
write. In a communal submissions namespace, each author remains restricted to
their own subspace. Invalid data is never stored as accepted state and is never
gossiped.

The anchor does not mint community capabilities, recover community root keys,
or decide community membership. It can carry signed delegation artifacts, but
authority originates with the community.

Meadowcap governs write authority. It is not read encryption. Content admitted
under this design is public and may be served to any reader. A future
read-restricted design requires encrypted private-group machinery and is
outside this scope.

## Lifecycle

### 1. Create and host

1. An organizer creates a community, manifest, and initial entries locally.
2. The client selects at least two configured anchors by default.
3. The client reconciles the community independently with each anchor.
4. Each anchor performs the complete admission boundary.
5. Each accepting anchor returns a signed hosting receipt.
6. The client retains receipts as availability observations, not authority.

Any peer may carry and offer valid public community state to an anchor.
Rehosting public state does not require the original author to remain online.

### 2. Opt into discovery

1. An authorized organizer creates `CommunityListingV1`.
2. The client submits the same signed listing to several anchors.
3. Each anchor validates listing authority and expiry.
4. Each accepting anchor includes the exact listing in its signed directory
   feed and local search index.
5. Directory feeds reconcile among configured peers under strict budgets.

Hosting without this step does not cause automatic listing.

### 3. Browse and follow

1. A reader searches the merged directory exposed by any configured anchor or
   opens a web mirror, link, or QR ticket.
2. The Riot client combines its configured anchors with bounded ticket hints.
3. It races or queries several sources and compares verified state frontiers.
4. It rejects identity, manifest, capability, and digest mismatches.
5. Retrieved state crosses the existing preview-first admission boundary.
6. The reader accepts and persists a durable local copy.

Directory appearance means only that a valid authority opted into listing. It
is not a trust, safety, accuracy, or endorsement signal.

### 4. Publish and converge

Valid writes may originate on phones or desktops and first travel through any
available path:

- nearby Bluetooth/local-network sync;
- an anchor;
- another ordinary peer;
- an exported file;
- a physical data mule.

When a node later reaches another source, the same reconciliation logic
exchanges the missing signed state. There is no canonical upload order and no
transport-specific conflict rule.

### 5. Recover from outage

When an anchor is unavailable, clients:

1. try other configured anchors;
2. try non-expired ticket hints;
3. preserve their durable local state;
4. continue nearby and file exchange;
5. reconcile the missing state when any anchor returns.

An outage is a reachability degradation, not an identity or migration event.

## Open Hosting and Resource Controls

Open hosting has no account or editorial admission gate. It still has strict,
published resource controls:

- maximum sync frame size;
- maximum entry and capability-chain size;
- maximum path length and depth;
- maximum individual payload size;
- per-community retained-byte and object-count budgets;
- bounded reconciliation transitions and wall-clock duration;
- bounded concurrent sessions;
- rate limits by connection source and community root;
- mandatory listing expiry;
- bounded record count and bytes per directory feed;
- content-addressed payload deduplication;
- lazy payload retrieval under explicit budgets;
- load shedding before allocation or expensive verification;
- bounded gossip fanout and retry backoff.

Root signatures do not prevent Sybil communities. Directory feeds therefore
remain plural and every client/anchor applies byte, count, age, and source-feed
budgets locally. No anchor automatically downloads an entire community merely
because a valid listing exists.

An anchor may evict locally under storage pressure, operational failure, or
legal necessity. It reports its local availability status when possible.
Eviction creates no protocol deletion and carries no instruction for other
anchors or peers to discard their copies.

## Web Experience

Each anchor provides:

- a directory landing page;
- bounded search and filters over valid, unexpired listings;
- readable routes for hosted communities;
- full community identity and freshness information;
- “Open in Riot” links;
- QR/share tickets with multiple anchor hints;
- an anchor-information page generated from `AnchorDescriptorV1`.

The renderer remains read-only. It has no signing key, authoring flow, or
private-group access.

Web output is a projection. A malicious mirror can omit or misrepresent content
to a casual browser reader. Riot verifies the underlying signed state before
import and displays verification outside community-controlled content.

## Native Client Experience

### Explore

The client queries configured anchors, merges valid unexpired listings, and
deduplicates them by full community identity. Cards may show observed
availability such as “available from three anchors,” but availability is not
trust.

### Follow

The client contacts multiple sources, verifies the community, shows the
existing import preview, and persists a local copy after acceptance.

### Publish

Community maintainers see hosting state separately from listing state:

- hosted on which anchors;
- last verified frontier and receipt horizon;
- listed or unlisted;
- listing expiry and refresh action.

The default action hosts on several configured anchors. Listing remains an
explicit, separate choice.

### Anchor settings

Users can:

- inspect the full signed descriptor;
- add an anchor from a URL or QR code;
- remove any default anchor;
- see supported features and published limits;
- see recent reachability without treating it as trust;
- choose which anchors receive their public communities.

## Failure and Threat Model

### A malicious anchor can

- observe a client's IP address, requested communities, timing, and approximate
  transfer volume;
- withhold new or historical state;
- omit a listing from its local search;
- return stale but previously valid state;
- lie in an unsigned web projection;
- disappear without notice;
- issue a dishonest hosting receipt.

### A malicious anchor cannot

- forge a valid community root or signer;
- forge a valid Meadowcap delegation;
- write outside an admitted capability;
- substitute a different namespace for an expected signed ticket;
- alter an entry or payload without failing digest/signature verification;
- delete copies already held by peers or other anchors.

### Metadata privacy

Iroh encrypts the connection in transit, but the Riot anchor is an application
endpoint and sees public community state plus connection metadata. This design
does not claim anonymity or follow-graph privacy. A root-signed transport floor
continues to allow a future community to require a privacy transport and fail
closed when that transport is unavailable.

Anchor metrics are aggregate by default. Logs never contain ticket secrets,
private keys, capability secrets, or payload bodies. Public identifiers are
printed in full only when a specific diagnostic requires them; they are never
truncated.

### Trust and moderation

Search inclusion means “owner-authorized and structurally valid,” not
“verified,” “safe,” or “endorsed.” Trust and curation remain reader-selected
lenses over signed public data. This design adds no shared moderation authority
and no network-wide deletion mechanism.

## Error Semantics

Errors are structured and stable across native and web surfaces:

- `invalid_authority`: signature or Meadowcap admission failed;
- `unsupported_version`: record or sync version is unsupported;
- `over_quota`: a published resource budget was exceeded;
- `expired`: listing, ticket, delegation, or requested retention expired;
- `not_hosted`: this anchor has no retained state for the community;
- `stale`: the anchor has valid state but is behind a verified peer frontier;
- `unreachable`: connection failed without an authority judgment;
- `partial`: metadata or entries are retained but some payloads are absent.

Clients distinguish refusal from network failure. They never convert one
anchor's refusal into a global community status. A failure on one source causes
the client to try other sources within bounded retry and concurrency budgets.

## Deterministic MVP Proof

The first end-to-end proof uses three independent anchors and several clients:

1. An organizer creates a public community locally.
2. The organizer hosts it on anchors A and B.
3. The organizer signs and submits an opt-in listing.
4. Directory and configured community replication reach anchor C.
5. A new user discovers the community through anchor C's web directory.
6. The organizer goes offline.
7. The new user opens the community in Riot, verifies it, and saves a local
   copy.
8. Anchor A is stopped; discovery and sync still succeed through B and C.
9. A writer creates a valid update while connected only to a nearby peer.
10. Later connectivity carries that update to one anchor.
11. Anchor gossip and client multi-homing converge all reachable nodes.
12. A final reconciliation round transfers no new accepted state.

The proof must show that the same full entry IDs, signer IDs, community IDs,
manifest digest, and application-level reads converge everywhere.

## Testing

### Protocol tests

- Canonical encoding and signatures for all three records.
- Descriptor and listing expiry.
- Listing authority through root and delegated capabilities.
- Anchor hints never participate in community identity.
- Root-signed transport floor rejects stripping, replay, and rollback.
- Unknown versions and transport floors fail closed.

### Admission tests

- Valid owned-publication delegation is accepted.
- Out-of-prefix, expired, forged, or wrong-subspace capabilities are rejected.
- Communal authors cannot write another author's subspace.
- Unknown, gated-read, and private community profiles fail before storage.
- Invalid state is absent from retained storage, search, web output, and gossip.
- Admission is atomic under interruption and parser failure.

### Anchor contract tests

- Hosting requires no account.
- Hosting does not imply listing.
- Valid owner-authorized listing appears in search.
- Expired or forged listing disappears or is rejected.
- Web mirror renders only admitted public state.
- Receipts accurately describe retained heads and payload coverage.
- Deduplication and eviction preserve protocol validity.

### Resource and adversarial tests

- Oversized frames, entries, paths, payloads, and capability chains fail before
  unbounded allocation.
- Per-community and directory-feed budgets are enforced.
- Duplicate delivery is idempotent.
- Interrupted sync releases resources and can be retried.
- Stale and dishonest anchors cannot downgrade accepted local state.
- Gossip loops do not amplify identical entries or payloads.

### Network tests

- Three-anchor deterministic lifecycle described above.
- Client multi-homing works when anchor gossip is disabled.
- Anchor gossip works when the original publishing client is offline.
- Any one anchor can disappear without blocking discovery and sync.
- Nearby-only state later converges through anchors.
- Final quiescent rounds accept no new state.

### Native and web tests

- Explore merges and deduplicates listings from several anchors.
- Follow preserves preview-before-accept.
- Hosting and listing are visibly separate actions.
- Removing all default anchors leaves nearby/file operation available.
- Web pages contain full-identity share tickets and retain existing gateway
  security headers.

### Quality gates

Implementation follows TDD. Before completion it runs:

- `cargo test --workspace --all-features`;
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-features -- -D warnings`;
- native platform tests for changed client surfaces;
- gateway tests for changed web surfaces;
- the coverage enforcement commands and floors in
  `.coverage-thresholds.json`.

## Delivery Slices

This design should be implemented as independently reviewable slices:

1. **Records and pure admission:** canonical records, signatures, Meadowcap
   verification, expiry, limits, and ticket-hint evolution.
2. **Headless anchor peer:** bounded store, stable iroh endpoint, sync acceptor,
   hosting receipts, and deterministic two-anchor tests.
3. **Plural directory and gossip:** anchor descriptors, directory feeds,
   listing validation, configured peer reconciliation, and three-anchor tests.
4. **Web mirror and discovery:** directory/search pages, community rendering,
   QR/share tickets, and anchor information.
5. **Native multi-anchor experience:** configurable anchor set, Explore,
   Follow, multi-home publishing, hosting receipts, and listing controls.
6. **Adversarial and operational closure:** quotas, load shedding, eviction,
   observability privacy, outage tests, packaging, and deployment runbook.

The implementation plan will assign exact file scope and dependencies after
the design review gate.

## Non-Goals

- Private groups, MLS, encrypted group drops, or private rendezvous.
- Read authorization or gated plaintext delivery.
- Accounts, passwords, canonical homes, or PDS migration.
- A global firehose or requirement that every anchor store everything.
- A canonical directory or global search ranking.
- Network-wide moderation or deletion.
- Permanent hosting guarantees.
- Payment, proof-of-work, or invitation gates.
- A custom iroh packet-relay network.
- Tor/Arti integration beyond preserving the existing signed transport-floor
  contract.

## Definition of Done

The feature is complete when:

- public communities can be hosted on several independently configured anchors
  without accounts;
- hosting does not cause automatic listing;
- owner-authorized listings are discoverable through plural anchor feeds and
  ordinary web pages;
- clients follow from web, search, link, or QR and persist verified local state;
- anchors enforce signatures and Meadowcap capabilities before propagation;
- clients and anchors reconcile through the same transport-independent state
  machine;
- the deterministic three-anchor proof converges and survives anchor loss;
- open-hosting resource controls and structured failure semantics are tested;
- private-group data cannot enter any anchor or web-mirror path;
- all repository quality and coverage gates pass.
