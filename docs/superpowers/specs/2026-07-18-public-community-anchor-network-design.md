# Public Community Anchor Network Design

Date: 2026-07-18
Status: Design review rounds 1-4 revised; pending round 5
Scope: Owner-rooted composite public sites, discovery, hosting, web mirroring,
and opportunistic internet sync

## Purpose

Riot already has signed local-first state, nearby exchange, public site
manifests, web rendering, and an iroh transport adapter. It lacks a reliable
on-ramp when the relevant phones and desktops are not simultaneously reachable
peer to peer.

This design adds plural **anchors**: always-online Riot peers that retain
admitted public site state, seed the same reconciliation protocol used by
ordinary peers, publish plural discovery feeds, and render safe public views at
ordinary web URLs.

An anchor improves reach. It never becomes the site's identity, root of trust,
or required home. Losing an anchor reduces availability but does not trigger
identity migration.

## Review Revisions

The first design-review round identified five implementation-blocking gaps.
This revision resolves them:

1. `riot/sync/1` is single-namespace and limited to 64 inventory IDs. Anchors
   therefore use a new, paginated `riot/sync/2` protocol with inbound namespace
   routing and immutable session snapshots.
2. Anchor operations do not fit the existing sync frames. A separate bounded
   `riot/anchor/1` control ALPN now owns hosting plans, idempotency, listing
   submission, and signed receipts.
3. Listing authority is now an exact Meadowcap boundary at
   `O:/directory/listing`, disjoint from editorial authority.
4. The capped mobile evidence repository is not reused as a server. A new
   SQLite-backed `AnchorRepository` provides server-scale transactions,
   reference accounting, crash recovery, and eviction.
5. The fixed gateway is not treated as a dynamic trusted renderer. The anchor
   produces immutable verified projections for an isolated, text-only static
   renderer.

This revision also adds the complete web-to-app journey, typed plural-source
outcomes, concrete global resource ceilings, bounded operator-selected gossip,
anchor key rotation, a deterministic test harness, per-slice TDD cycles, and
user-focused pilot thresholds.

Round two then closed the remaining executable-contract gaps: embedded plural
bootstrap and host management, directory-carried root tickets, exact HTTPS
handoff, separate operation/idempotency identities, one-way read versus staged
sync FSMs and digests, multi-ALPN/native transport boundaries, signed feed and
replica gossip, all-class persistence ceilings, authenticated work challenges,
safe hinted dialing, renderer sandbox/output validation, typed UX transitions,
and opt-in aggregate pilot measurement.

Round three closed boundary conditions exposed by those exact contracts:
scalable native site storage, multi-chunk sync responses, durable prepared
idempotency replay, stable anchor identity and paged key continuity, bounded
anchor proof profiles, root-safe delegated listing epochs, channel-bound peer
authentication, signed feed checkpoints/snapshots, immutable renderer ownership
transfer, operational-log quotas, self-contained host/consent states, strict
ticket/manifest transport matching, and exact pilot formulas.

Round four removed the last lifecycle overclaims: initiator and replica state
remain private through composite receipt, client storage uses the existing
core-owned RiotDatabase lifecycle, genesis/rotation envelopes are fully
verifiable, every control record is byte-bounded, cold handoff uses the signed
ticket as its bootstrap transport assertion, peers exchange rotated descriptor
chains before proof, feed removals can force bounded emergency checkpoints, and
pilot percentages require meaningful samples including install return.

## Product Decisions

1. **Riot-native always-on peers, not canonical homes.**
2. **Several removable, independently operated anchors from the first public
   pilot.**
3. **Client multi-homing plus optional anchor-to-anchor gossip.**
4. **Hosting, sharing, and listing are separate states.**
5. **Open hosting:** no account, payment, invitation, or editorial approval.
6. **Web mirror included:** directory, readable views, install handoff, and
   “Open in Riot.”
7. **Meadowcap enforced at every state ingress.**
8. **Public scope only:** no private-group rendezvous or read confidentiality.
9. **MVP site type is intentionally narrow:** only the approved owner-rooted
   composite site profile is listable and hostable. Its comments and open wire
   remain communal namespaces. Standalone legacy communal spaces remain
   supported by their existing local/share flows but do not enter the anchor
   network in this slice.
10. **Following is durable and ongoing:** acceptance creates a local copy and a
    schedule for opportunistic reconciliation with configured anchors. Manual
    refresh and nearby/file exchange remain available.
11. **MVP transport eligibility is explicit:** an anchor accepts only tickets
    whose signed transport floor is `require:none`. A ticket requiring Arti is
    refused as `unsupported_transport`; it is neither listed nor web-mirrored
    until an anchor implementation can satisfy that floor.

## Lessons Incorporated

- AT Protocol demonstrates the value of reliable hosted indexing and web
  views. Riot does not adopt a canonical PDS home.
- Nostr demonstrates plural client-chosen relay sets and replaceable location
  hints. Riot avoids making users guess divergent read/write sets by combining
  client multi-homing with optional anchor gossip.
- Briar, Secure Scuttlebutt, and Riot's existing research demonstrate that
  local and internet paths should use one reconciliation model.

References:

- <https://atproto.com/guides/the-at-stack>
- <https://github.com/nostr-protocol/nips/blob/master/65.md>
- <https://github.com/nostr-protocol/nips/blob/master/66.md>
- <https://github.com/nostr-protocol/nips/blob/master/77.md>
- `docs/research/2026-07-11-hybrid-gossip-backhaul-research.md`
- `docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`
- `docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md`

## Personas and Use Cases

| Person | Wants | So that | When |
| --- | --- | --- | --- |
| Organizer | Host a public site on several anchors and list it explicitly | the site remains discoverable while their device is offline | preparing or running a public publication/community |
| Web visitor | Read a safe public projection and preserve the exact destination through install | they can join without understanding Riot first | following a shared article or community URL |
| Riot reader | Search several anchors, verify a site, and retain it locally | one host outage does not strand them | discovering or revisiting a community |
| Contributor | Publish through a valid Meadowcap capability and sync through any available path | work can begin locally and propagate later | disconnected, nearby-only, or online |
| Anchor operator | Offer open hosting under explicit hard limits | public communities gain reach without granting the operator authority | running independent infrastructure |

## System Shape

```text
                       ordinary HTTPS
                            |
                    +-------v--------+
                    | static web     |
                    | mirror/search  |
                    +-------+--------+
                            | immutable verified snapshot
                    +-------v--------------------------+
                    | riot-anchor                      |
                    | control | sync/2 | repository    |
                    | feeds   | gossip | projection    |
                    +------+--------------------+------+
                           |                    |
                 same sync/2                    | same sync/2
                           |                    |
                   +-------v------+      +------v-------+
                   | Riot clients |      | other anchors|
                   +-------+------+      +------+-------+
                           |                    |
                           +-- nearby / files --+
```

Shared canonical frames, records, state enums, and codecs live in a new
dependency-neutral `riot-anchor-protocol` library crate. It depends on
`riot-core` with storage/default features disabled, and never on SQLite, HTTP,
iroh, Tokio, or server adapters.

The anchor daemon is a new workspace binary crate, `riot-anchor`, depending on
`riot-anchor-protocol`, `riot-core`, and `riot-transport`.

Native internet operations use a new mobile-compatible `riot-client-net` crate
that owns one iroh endpoint and Tokio runtime per application process. It
depends on `riot-anchor-protocol` and `riot-transport`; `riot-ffi` exposes its
cancellable async operations and typed event streams through UniFFI. Existing
native nearby carriers remain unchanged. The existing client-side `rusqlite`
feature remains in `riot-core`; anchor-server SQLite, HTTP-server, and renderer
dependencies never enter `riot-core`, `riot-anchor-protocol`,
`riot-client-net`, or native shells.

The native shell starts the client once after protected profile unlock and
closes it on profile lock/process teardown. Each FFI operation owns a
cancellation token; cancellation closes its streams, awaits task termination,
and emits a terminal state before releasing the handle. Background execution
uses the platform's bounded task window and persists retry intent, never an
unbounded Rust task. The existing native-core packaging script must
cross-compile `riot-client-net` and its iroh/Tokio dependencies for every
checked-in iOS and Android target before Slice 6 can pass.

## Community Authority Bootstrap

The anchor accepts only the owner-rooted composite public site profile approved
in the composite-site design.

The masthead namespace `O` is owned. The manifest lives at `O:/manifest` and is
admitted only when all of these independently hold:

1. the manifest capability is owned with zero delegations;
2. the capability receiver equals the `O` owner key;
3. `manifest.root` equals the `O` owner key;
4. the followed ticket root equals the `O` owner key;
5. the hosting namespace is exactly `O`;
6. every member namespace's intrinsic owned/communal rule matches its declared
   rule;
7. the manifest selects the supported read-open composite-site profile;
8. manifest version is at or above the durable per-site floor;
9. an already-seen version has the same digest.

A different digest at an already-seen manifest version is equivocation. The
anchor quarantines the site from listing, hosting commits, and web projection,
retains both conflicting proofs under a bounded incident record, and requires a
higher root-owned manifest version to recover.

The MVP composite contains:

- `O`: owned masthead/articles/moderation/directory;
- `C`: communal comments;
- `W`: communal open wire.

The anchor syncs `O` first, admits the manifest, then routes `C` and `W` only
when their full namespace IDs match the admitted member list.

## Listing Authority

### Reserved coordinate

The only listable coordinate is:

```text
O:/directory/listing
```

`/directory` is reserved and disjoint from `/manifest`, `/mod`, and
`/articles`. An editorial capability rooted under `/articles` can never
authorize a listing.

### Who may write it

A listing entry is valid when it is signed by either:

- the `O` owner using the owned zero-delegation capability; or
- a dedicated listing key whose complete Meadowcap chain begins at `O` and
  whose terminal area has:
  - receiver equal to the listing key's subspace;
  - path prefix exactly `/directory`;
  - a bounded time range containing the entry timestamp.

A delegated entry must also carry a canonical `ListingDelegateGrantV1` signed
directly by the `O` root under:

```text
"riot/listing-delegate-grant/v1" || canonical_cbor(grant_body)
```

The grant binds the root, delegate key, terminal capability digest, one exact
`listing_epoch`, issued time, and expiry. It cannot outlive the Meadowcap time
range. A Meadowcap chain alone does not let a delegate choose an epoch.

The Riot admission layer additionally requires the entry's path to equal
`/directory/listing`; authority over arbitrary `/directory` children is not
interpreted as a new record type.

### `CommunityListingV1`

The canonical CBOR payload binds:

- schema `riot/community-listing/1`;
- full root and `O`, `C`, and `W` namespace IDs;
- manifest digest and version;
- a canonical root-signed `PublicSiteTicketV2` core;
- `listing_epoch: u32`;
- `listing_revision: u32`;
- `listed: true | false`;
- title and summary;
- bounded topic tags and languages;
- optional coarse region;
- issued time and expiry.

The ticket core is site-wide, not entry-specific. The `O` root key signs:

```text
Sign(
  "riot/public-site-ticket/v2" ||
  canonical_cbor(ticket_body)
)
```

The body binds schema/version, full root and `O`, `C`, and `W` IDs, manifest
digest/version, `min_sync_version: 2`,
`manifest_required_transport`, `transport_floor`, transport epoch, issued
time, and expiry. The ticket floor must be greater than or equal to the
manifest requirement under the current total ordering
`require:none < require:arti`. The MVP accepts
only the equality `require:none == require:none`; every other combination
fails before dialing as `unsupported_transport`. Its expiry must be at or
after the listing expiry. The listing entry and ticket core must name identical
roots, namespaces, and manifest coordinates or admission fails.

The core ticket builder reads the verified manifest and refuses a mismatched
transport pair. Clients compare `transport_floor >=
manifest_required_transport` and require both to be `require:none` before any
anchor or hinted iroh/HTTPS dial. For a cold handoff, the root-signed ticket
assertion is the authoritative bootstrap transport requirement; no manifest
fetch is needed before that first permitted `require:none` dial.

`PrepareHost` performs the same ticket check and compares against an
already-retained manifest when present; after a new operation stages `O`, it
compares the actual digest-matched manifest field before permitting `C`, `W`,
content admission, or commit. `SubmitListing` and every `ReadCommitted` open
compare all three values again against the admitted manifest. A mismatch is
`manifest_transport_mismatch`, quarantines the operation/listing, and never
downgrades to public iroh.

A dedicated listing delegate may update listing presentation and revision, but
cannot mint this root-signed ticket. The owner issues or refreshes the ticket
before delegating listing maintenance.

The Willow entry signature covers the payload digest, namespace, subspace,
path, timestamp, and capability. No second ad hoc signature scheme is added.

`listed: false` is an explicit unlisting tombstone. It stops future directory
display but does not delete hosted state or copies held by peers.

For a given root:

- a delegated entry is admitted only in the epoch named by its valid grant;
- only a root-owned zero-delegation entry or grant may establish the next epoch,
  and it may advance by exactly one;
- higher valid epoch wins;
- within an epoch, a root-owned zero-delegation listing unconditionally wins
  over every delegated listing, regardless of revision, and seals that epoch
  against later delegated changes;
- among records in the same authority class, higher revision wins;
- identical `(epoch, authority class, revision)` and digest deduplicate;
- identical coordinates with different digests are listing equivocation, so
  neither listing is shown;
- a higher-revision root-owned listing in the current epoch or any valid
  root-established next epoch clears equivocation and cannot be pinned by a
  delegate at `u32::MAX`;
- expiry is inclusive: the listing is invalid when `now >= expiry`.

Anchors persist the root-controlled epoch, sealed status, highest admitted
revision, grants, and conflict evidence so restart or cache eviction cannot
roll the listing backward.

## Canonical Anchor Records

All anchor-owned records use deterministic CBOR with integer map keys,
definite-length containers, minimal integer encodings, and sorted collections.
Decoders reject unknown required versions, duplicate fields, non-canonical
encodings, and trailing bytes.

Every anchor-owned signed body other than a descriptor binds the stable
`AnchorId`, current operator key ID, descriptor epoch, and descriptor digest.
Verification therefore identifies the exact still-chain-valid signing context
instead of treating every historical key as indefinitely current.

### `AnchorDescriptorV1`

`AnchorId` is stable for the life of one deployed anchor:

```text
AnchorId = BLAKE3(
  "riot/anchor-id/v1" ||
  genesis_operator_public_key ||
  genesis_random_256_bits
)
```

It does not change when operator signing or iroh endpoint keys rotate. Receipts,
feed coordinates, cursors, configuration, and deduplication use `AnchorId`;
signatures name the current operator key separately.

Every descriptor body contains both genesis inputs, and verifiers recompute
`AnchorId` before accepting it. The signed envelope is:

```text
DescriptorEnvelopeV1 {
  body,
  current_signature,
  predecessor_signature: optional
}

current_signature =
  Sign(
    current_operator_key,
    "riot/anchor-descriptor/v1" || canonical_cbor(body)
  )

predecessor_signature =
  Sign(
    predecessor_operator_key,
    "riot/anchor-descriptor-transition/v1" ||
    BLAKE3(canonical_cbor(body))
  )
```

The body contains:

- stable full `AnchorId`;
- genesis operator public key;
- genesis random 256-bit value;
- full anchor operator key ID;
- monotonically increasing descriptor epoch;
- previous descriptor digest when epoch is greater than zero;
- current iroh endpoint ID;
- HTTPS origin;
- bounded operator display label and self-reported failure-domain label;
- supported control and sync versions;
- enabled roles: host, directory, mirror, gossip;
- limit-profile digest;
- predecessor operator key, when rotating;
- issued and expiry times.

### `HostingReceiptV1`

The operator signs:

```text
"riot/hosting-receipt/v1" || canonical_cbor(receipt_body)
```

The body contains:

- anchor ID;
- operator key ID and descriptor epoch/digest;
- stable hosting operation ID;
- full site root and manifest digest/version;
- one ordered `(namespace_id, snapshot_digest, entry_count)` tuple per admitted
  member namespace;
- status;
- accepted time;
- `reported_retention_through`;
- limit-profile digest.

`reported_retention_through` is the anchor's signed operational claim, not a
cryptographic guarantee. A dishonest or failed anchor may break it.

### `ListingReceiptV1`

The operator signs anchor ID, operator key ID, descriptor epoch/digest, listing
digest, site root, accepted epoch/revision, directory feed coordinate,
acceptance time, expiry, and request idempotency key under:

```text
"riot/listing-receipt/v1"
```

Receipts never grant site authority.

## Anchor Keys

The anchor operator signing key is separate from:

- the iroh endpoint key;
- TLS keys;
- hosted community keys, which the anchor never possesses.

The operator key is loaded through an injectable `AnchorKeyStore`; production
uses a non-world-readable OS secret store or managed KMS. It is never stored in
the community database.

Epoch zero has no predecessor signature and must verify under the genesis key.
Every later epoch has both signatures above. The predecessor may be the same
key for an endpoint/origin/config update; in that case both fields are present
and verify under that key. Historical intermediates are validated against the
issued/expiry overlap that existed when the transition was signed. They may be
expired at retrieval time; only the resulting head must be currently valid.
After verifying continuity clients durably persist the operator key, highest
descriptor epoch, and descriptor digest. They reject any lower epoch, any
different digest at the same epoch, and every replayed predecessor descriptor.
Emergency revocation without the old key is an explicit user-visible
operational trust reset and is not hidden as normal rotation.

The well-known response contains the latest descriptor and a same-origin,
redirect-free descriptor-chain endpoint. A client pinned at any older digest
fetches bounded pages of at most 16 descriptors, verifies every previous digest,
epoch increment, old-key signature, new-key signature, stable `AnchorId`, and
time overlap, then persists the newest floor. The operator must retain this
chain for every still-supported app floor. A complete traversal is capped at
32 hops, 256 KiB canonical bytes, and five seconds; each page is at most
60 KiB. If the chain is unavailable, inconsistent, or exceeds a cap, the client
reports `descriptor_chain_unavailable` and requires an app update or explicit
trust reset; it never skips rotations.

Restoring one anchor backup into two live deployments is identity cloning, not
horizontal scaling. The database and KMS hold a matching
`deployment_instance_token`; startup must acquire the operator-configured
single-writer lease for that token before signing or readiness. A clone without
the lease fails closed and is reported as potential anchor equivocation.

## Transport Roles

Iroh packet relays and Riot anchors are distinct:

- an iroh relay forwards encrypted endpoint packets and assists NAT traversal;
- a Riot anchor is an application endpoint that validates and persists public
  site state.

The checked-in transport has:

- `bind` and `bind_seed` using `N0DisableRelay`;
- `bind_public` using the `N0` preset with relay plus Pkarr/DNS discovery.

The transport adapter is refactored around one stable iroh `Endpoint` and an
ALPN router. `bind_public` registers bounded handlers for `riot/sync/1`,
`riot/sync/2`, and `riot/anchor/1`; the accept loop reads the negotiated ALPN
and dispatches to the matching handler. The existing `sync_accept` API remains
as a compatibility wrapper for a router containing only `riot/sync/1`.
Unknown ALPNs close without allocating a protocol session.

The MVP anchor uses this routed `bind_public` with a stable iroh endpoint key.
The descriptor publishes the endpoint ID and all supported ALPN versions.
Anchor operator identity remains the separate signing key above.

## Anchor Bootstrap and Management

Each app release embeds an `AnchorBootstrapV1` resource covered by the app
package signature. The public-pilot resource contains at least three enabled
descriptors across at least two operators and failure domains. For each default
it pins the operator key, minimum descriptor epoch/digest, HTTPS origin, and
roles. App releases, not a live canonical service, update this fallback set.

On first run Riot fetches each pinned origin's
`/.well-known/riot-anchor.json`, using the safe dialing rules below, verifies
the descriptor signature and durable floor, and persists successful
descriptors. A default anchor is trusted only as the named routing/operator
identity; all community content and authority are still independently
verified. If every default is unavailable, cached Explore results remain
marked stale and direct links, QR, nearby, and files still work. Riot does not
invent a replacement directory.

Users can:

- enable or disable any default;
- add a host from its HTTPS origin or descriptor QR after verifying and
  explicitly accepting its operator identity and roles;
- remove a custom host locally;
- reset to the embedded defaults;
- inspect descriptor expiry, key continuity, roles, and last successful
  contact.

Removing a host changes only the client's routing configuration. It does not
unlist a community or remotely delete retained state. Organizer replacement is
an ordered flow: add/verify new host, host and obtain its receipt, optionally
list there, confirm the desired redundancy, disable the old client route, then
optionally submit a separately authorized unlisting to the old host.

Unsigned handoff hints may introduce a nonconfigured host, but never silently
persist it as a configured directory source. Riot first obtains its descriptor
and verifies the self-consistent operator signature. Before any hinted HTTPS
request it resolves all addresses, permits only public globally routable
addresses on port 443, rejects IP literals for private/loopback/link-local/
multicast/reserved ranges, disables redirects, pins the resolved address for
the TLS request, checks SNI/certificate hostname, bounds the response, and
rejects DNS rebinding. Native iroh hints must name the endpoint ID declared by
that verified descriptor. Shared hints never grant content authority.

## `riot/sync/2`: Routed Paginated Reconciliation

`riot/sync/1` remains supported for existing nearby and legacy tests. Anchors
do not silently downgrade to it.

`PublicSiteTicketV2` is the root-signed core defined by the listing record. A
client refuses an anchor that cannot satisfy its minimum sync version or signed
transport floor.

### Inbound routing

The ALPN is `riot/sync/2`. Unlike `sync/1`, the responder does not construct a
namespace session before reading. It first reads a bounded `OpenNamespace`
frame:

```text
OpenNamespace {
  protocol_version,
  session_id,
  ticket_core,
  namespace_id,
  mode:
    ReadCommitted |
    HostReconcileStaged { operation_id, namespace_token } |
    ReplicaIntoStaged { operation_id, namespace_token }
}
```

`ReadCommitted` is the public follow/read path. It requires no control-plane
operation. The responder verifies the root signature, ticket expiry, transport
floor, exact namespace membership, and equality with its currently committed
manifest, then exposes a one-way committed snapshot.

Both staged modes verify the active operation and its unguessable,
per-namespace 256-bit token, then route writes only to that operation's staged
namespace. A stage is initialized from the destination anchor's currently
committed namespace when `PrepareHost` or `PrepareReplica` creates the
operation. `HostReconcileStaged` is bidirectional for an organizer client.
`ReplicaIntoStaged` is one-way from a source anchor into a destination and can
never mutate the source.

### Snapshot inventory

An immutable inventory sorts full canonical entry-ID bytes
lexicographically. Its digest is exactly:

```text
BLAKE3(
  "riot/sync-snapshot/v2" ||
  u32be(len(namespace_id)) || namespace_id ||
  u64be(entry_count) ||
  u64be(logical_bytes) ||
  for each sorted ID: u32be(len(entry_id)) || entry_id
)
```

The frame set is:

```text
SnapshotStart {
  phase,
  namespace_id,
  snapshot_digest,
  entry_count,
  logical_bytes
}
IdsPage {
  phase,
  snapshot_digest,
  after_exclusive: optional EntryId,
  entry_ids: at most 256,
  done
}
NeedEntries { phase, page_digest, request_id, entry_ids: at most 64 }
PageNeedsComplete { phase, page_digest }
EntriesChunk {
  phase,
  page_digest,
  request_id,
  chunk_index,
  done,
  canonical bundle: at most 64 entries and 8 MiB
}
PageComplete { phase, page_digest }
DirectionComplete { phase, sender_snapshot_digest }
NamespaceComplete { mode, final_snapshot_digest }
Refuse { code, subject, retryable, retry_after_seconds }
```

`page_digest` is the BLAKE3 digest of the canonical `IdsPage` frame. For each
direction the inventory sender and receiver follow this exact FSM:

1. Sender opens one immutable snapshot and sends `SnapshotStart`.
2. Sender sends exactly one strictly sorted `IdsPage`.
3. Receiver sends zero to four `NeedEntries` frames with distinct request IDs.
   Every requested ID must occur in that page and may occur once. Receiver ends
   requests with exactly one `PageNeedsComplete`.
4. For each request in order, sender deterministically partitions the requested
   entries, preserving requested-ID order, into one or more legal
   `EntriesChunk` bundles. Chunk indices start at zero without gaps; only the
   last has `done: true`. Because every admitted anchor-profile item is at most
   2 MiB, at least one item always fits. Sender then sends `PageComplete`.
5. Only after `PageComplete` may sender send the next page. Its first ID must be
   greater than `after_exclusive`.
6. After the page with `done: true`, receiver first verifies the advertised
   inventory digest from the received ID stream, then admits and commits all
   received bundles for that direction, and sends `DirectionComplete`.

Page overlap, duplicate IDs, cursor regression, digest change, out-of-page
need, missing/duplicate request or chunk index, oversized chunk, unexpected
frame, admission failure, or premature EOF terminates the session with no
visibility outside the relevant local/staged transaction.
Bundles retain the current 64-entry, 8 MiB, and 1 MiB payload-per-item
ceilings; the anchor-profile encoded item including proofs is at most 2 MiB.
Writes arriving after a sender opens its snapshot appear in the next session.

Each received chunk is admitted into direction-private staging in a short
transaction. `DirectionComplete` verifies completeness and atomically promotes
that direction's stage into its parent local-follow or anchor-host operation.
An abort deletes or expires the direction stage; already committed application
state remains unchanged. This reconciles bounded per-chunk writes with
end-of-direction atomic visibility.

`logical_bytes` is the exact sum of canonical encoded bundle-item bytes in the
sender snapshot. It is digest-bound and supports conservative quota preflight;
the receiver still enforces actual bytes while staging.

`ReadCommitted` executes one phase, `AnchorToClient`. After
`DirectionComplete`, the anchor sends `NamespaceComplete` with its committed
snapshot digest. The client may retain additional valid local entries; read
sync does not upload them.

`HostReconcileStaged` executes two ordered phases:

1. `AnchorToClient` uses the stage's committed-base snapshot. The client admits
   received entries into its private host-operation stage, not its committed
   application namespace.
2. Only then, the client opens an immutable view of its local committed
   namespace union that private stage and becomes inventory sender for
   `ClientToAnchor`. The anchor admits missing entries into its own stage.

After phase two, the anchor computes the stage digest and sends
`NamespaceComplete`. It must equal the client's phase-two snapshot digest.

`ReplicaIntoStaged` executes only `SourceToDestination`: the source anchor
sends one immutable committed snapshot; the destination admits missing entries
into its operation stage, computes the union digest, and returns
`NamespaceComplete`. No frame offers destination-only entries to the source.

Direction completion promotes only into the parent private operation stage.
No organizer-client or anchor application state becomes committed until all
`O/C/W` namespace stages complete, destination `CommitHost` commits, and any
lost receipt is recovered through `GetOperation`. The organizer then promotes
its local private stage in one transaction; a replica source discards its
read-only operation view. Any error before this sequence leaves every
participant's prior committed site unchanged.

This is bounded paginated inventory exchange, not the final bandwidth-optimal
algorithm. A future Negentropy/range-summary version may reduce ID transfer
without changing anchor authority or storage semantics.

### Composite transaction

A hosting or replica operation stages `O`, then `C`, then `W` under one stable
operation ID.
Nothing becomes directory-visible or mirror-visible until:

1. all three namespace sessions finish;
2. all entries pass admission;
3. the declared snapshot digests match staged state; and
4. one SQLite transaction promotes the complete staged site.

A failed operation deletes or expires its staged rows. Each admitted bundle is
written in a short staging transaction; the final promotion below is a separate
atomic SQLite transaction. Existing admitted site state remains unchanged.

A new public Follow mirrors this shape locally: `O` is read and the manifest
admitted before exact `C` and `W` routing; all three committed anchor snapshots
land in local follow staging; one local transaction installs the followed site
and ongoing-sync schedule. Cancellation or failure before that transaction
leaves no durable follow mutation.

## `riot/anchor/1`: Control Plane

The control ALPN carries canonical CBOR frames no larger than 64 KiB. Every
request has a random 128-bit `idempotency_key`. The anchor stores the canonical
request-body digest and an idempotency state:

```text
Claimed |
Prepared { operation_id, operation_expiry, canonical_prepare_response } |
Terminal { result }
```

One SQLite unique constraint atomically claims a new key. The winning
`PrepareHost` or `PrepareReplica` transaction creates the operation and changes
the same row to `Prepared`; concurrent exact calls read and replay it, while
another body is rejected. Namespace tokens are deterministically derived as:

```text
HMAC-SHA256(
  anchor_operation_secret,
  "riot/namespace-token/v1" ||
  operation_id || namespace_id || operation_expiry
)
```

so exact prepared replay returns the same tokens after restart without storing
them in plaintext. The operation stores the token-secret epoch; key-store
rotation retains prior secrets until every operation using them expires.
The Prepare idempotency row remains `Prepared` and always replays its original
response, even after the operation completes; a caller then uses
`GetOperation`. Every `CommitHost` has its own idempotency row. The final
promotion transaction atomically marks the operation committed, creates the
receipt, and changes that Commit row to `Terminal`. A refused Prepare or any
ordinary single-call request stores `Terminal` directly.

A prepared mapping is retained through `operation_expiry + 24 hours`; a
terminal or ordinary single-call result is retained for 24 hours. The
prepared-operation lifetime is at most one hour, so its mapping cannot
disappear while live. Namespace tokens are accepted only while the operation
status is actively staged, so replaying a completed Prepare cannot reopen it.
A `Claimed` row has a 30-second lease; startup or retry may delete it only when
no operation/result exists and the lease expired. Prepare operation creation
and transition to `Prepared` commit atomically.

### Encoded control-record profile

Every limit below is over complete canonical encoded bytes, including
signatures and envelopes:

| Record/frame | Maximum |
| --- | ---: |
| `DescriptorEnvelopeV1` | 8 KiB |
| Well-known descriptor response | 16 KiB |
| Descriptor-chain page | 60 KiB / 16 envelopes |
| Limit profile | 8 KiB |
| `HostingReceiptV1` or `ListingReceiptV1` | 4 KiB |
| `WorkChallengeV1`, refusal, peer hello, or peer proof | 4 KiB |
| `PrepareHost` request / response | 32 KiB / 8 KiB |
| `CommitHost` request / response | 4 KiB / 8 KiB |
| `SubmitListing` request / response | 32 KiB / 8 KiB |
| `PrepareReplica` request / response | 60 KiB / 8 KiB |
| `GetOperation` response | 16 KiB |
| Directory feed or snapshot frame | 60 KiB |
| Public directory JSON response | 1 MiB |

Descriptor HTTPS origins are at most 255 UTF-8 bytes; endpoint hints are at
most 512 bytes; supported control/sync version arrays have at most 16 distinct
entries each; roles have at most the four defined values; and headers total at
most 16 KiB. Operator/failure-domain labels are at most 64 UTF-8 bytes each.
Namespace tuples are exactly the three `O/C/W` members. All other collections
carry explicit count and byte limits in their schemas.

Descriptor/config construction runs before readiness. An operator
configuration that cannot produce legal descriptor, limit-profile, receipt, or
peer frames fails readiness instead of truncating a signed value.

Long-running work has a separate random 256-bit `operation_id`, created by the
anchor and returned from `PrepareHost` or `PrepareReplica`. Every later call
uses a new idempotency key plus the stable operation ID. `GetOperation` looks
up the operation ID, never an idempotency key.

Operations:

### `Describe`

Returns the current signed `AnchorDescriptorV1` and limit profile.

### `GetWorkChallenge`

Input contains the intended operation kind, idempotency key, community root,
and canonical request-body digest computed with the optional `work_stamp` field
absent. Output is a signed `WorkChallengeV1` as defined under Admission work
stamp. Challenge retrieval is rate-limited but does not create a durable
idempotency row.

### `PrepareHost`

Input:

- root-signed composite ticket;
- client-observed namespace snapshot digests;
- optional valid admission work stamp.

Output:

- stable operation ID;
- ordered namespace host plan;
- one unguessable namespace token per required namespace;
- current retained snapshot digests;
- sync version and limits;
- operation expiry.

The client then opens `sync/2` for each required namespace.

### `CommitHost`

Input:

- operation ID;
- final snapshot digests for `O`, `C`, and `W`.

Output:

- signed `HostingReceiptV1`; or
- typed refusal.

### `SubmitListing`

Input:

- complete canonical signed Willow entry for `O:/directory/listing`;
- optional valid admission work stamp.

Output:

- signed `ListingReceiptV1`; or
- typed refusal.

For `listed: true`, the anchor atomically verifies that the listing's root,
`O`, `C`, `W`, manifest digest/version, and ticket core exactly equal its
currently committed composite manifest and root ticket. The site must currently
be hosted on that anchor. Otherwise it returns `not_hosted` or
`manifest_mismatch` and creates no directory inclusion.

For `listed: false`, the anchor may instead match the tombstone against its
durable prior admitted listing/manifest/ticket floors. This permits explicit
unlisting after hosting was evicted, without making stale state visible again.

If a valid listing arrives before hosting, it is rejected rather than queued.
If hosted state is later evicted, its listing record remains durably known but
is suspended from local directory visibility and web projection. A matching
host refresh restores it before listing expiry. Unlisting hides the site but
never removes hosted state.

### `PrepareReplica`

This operation is accepted only on a mutually authenticated, configured
anchor-peer connection whose rule permits the named site. Input contains the
source descriptor, source hosting receipt, the same root-signed ticket core,
and desired snapshot digests. The destination validates all three, creates a
normal staged operation with tokens, and applies peer-specific byte/site
budgets. The source then acts as the sync initiator using
`ReplicaIntoStaged`. `CommitHost` produces a destination-signed receipt.
Replication never copies listing status implicitly.

### `PullDirectoryFeed`

This peer operation is paginated by the source anchor's monotonically
increasing feed sequence:

```text
PullDirectoryFeed { after_sequence, limit: at most 32 }
DirectoryFeedPage { inclusions, floor_sequence, head_sequence, head_digest, done }
CheckpointRequired { checkpoint, snapshot_cursor }
```

Each inclusion is at most 48 KiB; the server includes no more than the
requested count that fits one 60 KiB page, preserving the control-frame
ceiling. The operation is read-only and available only to authenticated
configured peers.

### `PullDirectorySnapshot`

Input names a verified checkpoint digest and optional opaque snapshot cursor.
Output contains the immutable checkpoint plus the next full-root-ordered
current-state record, next cursor, and `done`. At most one inclusion and 60 KiB
is returned per frame. On the final frame the receiver must recompute the
checkpoint state digest before advancing its feed cursor.

### `GetOperation`

Returns the current state or retained terminal result for an operation ID,
allowing recovery after a disconnect between commit and receipt delivery.

### Refusals

```text
ControlRefusal {
  code,
  subject:
    ticket | manifest | listing | namespace | capacity |
    version | transport | operation | work,
  retryable,
  retry_after_seconds: optional
}
```

Codes include `invalid_authority`, `unsupported_version`, `over_quota`,
`unsupported_transport`, `manifest_transport_mismatch`, `expired`,
`not_hosted`, `manifest_mismatch`,
`equivocation`, `anchor_profile_oversize`, `site_too_large`, `work_required`,
and `busy`.
A refusal is a protocol result, not a transport failure and not a signed
hosting receipt.

## Typed Client Operation Results

Native boundaries expose source-specific envelopes:

```text
AnchorAttempt<T> {
  anchor_id,
  result: Verified(T) | Refused(ControlRefusal) | TransportFailure(kind),
  observed_at
}

DirectoryPage {
  items,
  per_source: [AnchorAttempt<SourceDirectoryPage>],
  coverage: Complete | Partial | CachedOnly | Unavailable,
  cache: Live | Cached { captured_at, expires_at },
  merged_snapshot_id,
  exclusions: { expired, unlisted, conflicted, invalid },
  next_cursor
}

ExploreState =
  Idle |
  Loading { sources, prior_page } |
  Ready(DirectoryPage) |
  Failed { per_source, cached_page: optional DirectoryPage }

FollowState =
  Validating |
  Resolving { attempts } |
  AwaitingConsent { verified_site, ticket_core, destination_status } |
  Syncing { per_anchor_progress } |
  CommittingLocal |
  Saved { site_root, destination_status, attempts } |
  AlreadyFollowed { site_root, destination_status } |
  Cancelled { mutation: None } |
  Failed { phase, attempts, retry_context, local_mutation: None } |
  Quarantined { subject, evidence }

ReplacementPhase =
  VerifyingNew |
  HostingNew |
  ListingNew |
  ConfirmingRedundancy |
  DisablingOldRoute |
  OptionallyUnlistingOld |
  Complete

PublishState =
  SelectingHosts |
  Preparing { per_anchor } |
  Syncing { per_anchor, namespace } |
  RecoveringReceipt { per_anchor } |
  Hosted { receipts, redundancy } |
  Listing { per_anchor } |
  Listed { listing_receipts, host_receipts } |
  Unlisting { per_anchor } |
  Refreshing { per_anchor } |
  Replacing { old_anchor, new_anchor, phase: ReplacementPhase } |
  Partial { completed, refused, unreachable } |
  Failed { phase, per_anchor }

HostConfigurationState =
  Idle { configured_hosts } |
  FetchingDescriptor { origin } |
  VerifyingDescriptor { origin, descriptor } |
  AwaitingHostConsent { descriptor, continuity, roles } |
  PersistingHost { intent_id, descriptor, selected_roles } |
  RecoveringConfiguration { intent_id } |
  Enabling { anchor_id } |
  Disabling { anchor_id } |
  RemovingLocalRoute { anchor_id } |
  ResettingEmbeddedDefaults |
  Ready { configured_hosts, changed_anchor } |
  Cancelled { mutation: None } |
  Failed { phase, reason, mutation: None }
```

`expired` always names the expired subject. `over_quota` always names the
anchor and quota class. A verified receipt, protocol refusal, and local network
failure can never collapse into the same enum case.

Directory merging owns a local opaque cursor containing the normalized query
hash, merged snapshot ID, and one `(anchor_id, source_snapshot_id,
source_cursor)` tuple per configured source. A cursor is invalid if used with a
different query or source set. Per-source exclusions remain available in
Technical details even when a record is absent from visible results.

Legal mutation boundaries are normative:

- Explore never mutates follow or hosting state.
- Follow mutates no durable state before `Accept` from
  `AwaitingConsent`; `Cancel` or app termination there discards the retained
  verified payload with no mutation. `Retry` from a retryable failure consumes
  the explicit `retry_context`, never hidden side state.
- Accepted data is staged locally; `Saved` is emitted only after one atomic
  local follow commit. Cancellation or failure before it emits
  `local_mutation: None`.
- Hosting, listing, unlisting, and refresh are independent per-anchor
  operations. Aggregate `Partial` never rewrites a verified per-anchor result.
- Replacing a host first reaches `Hosted` on the new anchor. Local removal of
  the old host then stops future client sync but does not claim remote deletion.
  Remote unlisting is a separate signed operation, and retained hosted state
  expires under the old receipt's policy.
- Host configuration persists nothing before `AwaitingHostConsent` acceptance.
  Enabling/disabling/removing changes one local routing record atomically.
  Reset replaces only the routing configuration with the package-signed
  embedded defaults; it never changes follows, listings, or remote state.
  After crash in `PersistingHost`, recovery rereads the intent and committed
  routing record: it emits `Ready` if committed or `Failed { mutation: None }`
  only when no record exists.

Pure injectable ports mirror existing repository patterns:

- `AnchorDirectoryPort`;
- `AnchorHostingPort`;
- `AnchorSyncPort`;
- `AnchorConfigurationPort`;
- `Clock`;
- `RetryScheduler`.

## Native Client Site Repository

The current capped `EvidenceRepository` and its 1,024-entry/16 MiB store and
2 MiB preview budgets are not used for followed composite sites.

A new `ClientSiteRepository` and dependency-neutral `SiteReplicaRepository`
trait live in `riot-core::store`. The concrete repository uses the existing
default-on `rusqlite`, `RiotDatabase`, migration runner, profile database, and
`RiotSession` lifecycle. New versioned tables store verified manifests,
`O/C/W` entries, payloads, follow/host staging, anchor configuration/floors,
receipts, and retry intent. `RiotSession` retains the profile lock and
transaction ownership; backup/restore includes these tables and validates their
floors before reopening. One core-owned transaction promotes all three
namespaces plus the ongoing-sync schedule after consent.

`riot-client-net` receives a thread-safe command port to this repository. All
synchronous `rusqlite` work runs on the existing dedicated serialized database
worker or `spawn_blocking`, never an iroh/Tokio network executor.
`riot-ffi` constructs the session, injects the port, and projects typed results;
it does not own migrations, connections, transactions, or durable business
state.

Default device limits are 1 GiB total followed-site logical bytes, 64 MiB per
site, 4,096 live entries per namespace, and the anchor-profile per-item bounds.
Users may configure lower total storage. Before accepting a preview, Riot
compares the ticket/manifest, anchor limit descriptor, advertised snapshot
counts, and remaining local budget. A site that cannot fit yields typed
`site_too_large { required_class, advertised, local_limit }`; it does not fall
back to the smaller evidence store or partially commit. SQLite WAL, crash
recovery, per-site staging, payload reference accounting, and eviction of
explicitly unfollowed/expired staging follow the same atomicity rules as the
anchor repository.

## Anchor Repository

`riot-anchor` owns a new SQLite-backed `AnchorRepository`. It reuses canonical
Riot codecs and admission functions but does not extend the capped
`EvidenceRepository` or demo `SiteState`.

Logical tables:

- `communities`;
- `manifests` and durable version/digest floors;
- `public_site_tickets`;
- `namespaces`;
- `entries`;
- `payloads`;
- `community_payload_refs`;
- `listings` and listing conflict floors;
- `directory_inclusions`;
- `directory_feed_heads`;
- `hosting_receipts`;
- `staged_operations`;
- `idempotency_results`;
- `anchor_peers`;
- `operator_state` and descriptor floors.

### Transactions

- SQLite WAL mode and foreign keys are mandatory.
- Each incoming bundle is fully admitted and appended to operation-private
  staging rows in a short transaction. Staging is never query-visible.
- Final promotion, committed reference updates, search visibility, directory
  visibility, projection generation request, and receipt creation share one
  atomic commit transaction.
- A receipt is reconstructable from committed state after restart.
- A crash before commit leaves only expirable staging rows.
- A crash after commit returns the same receipt through `GetOperation`.
- Reads use immutable SQLite snapshots.

### Payload accounting

Physical payload bytes deduplicate by digest. Quotas charge every community the
full logical payload size before deduplication. Per-community reference rows
prevent one community's eviction from deleting a payload still retained for
another.

### Eviction

New admissions never evict data within a signed
`reported_retention_through`. If capacity cannot honor the horizon, the anchor
rejects the new operation.

After horizons expire, eviction order is deterministic:

1. expired/unlisted directory projections;
2. incomplete abandoned staging;
3. unlisted sites by oldest last successful host refresh;
4. listed sites by oldest last successful host refresh.

Promotion, reference decrements, search removal, projection invalidation, and
new receipt state occur atomically.

## Open Hosting Resource Contract

Operators may configure lower values, but not exceed the compiled absolute
ceilings. The signed descriptor publishes effective values.

### Anchor public-profile bounds

Anchor eligibility is intentionally stricter than the general Riot/Meadowcap
codec maxima. `AnchorPublicProfileV1` requires:

| Canonical object | Maximum |
| --- | ---: |
| Capability chain attached to one anchor-profile entry | 12 KiB |
| Manifest entry plus capability | 16 KiB |
| Listing entry plus capability and optional delegate grant | 16 KiB |
| `PublicSiteTicketV2` core | 768 bytes |
| `DirectoryInclusionV1` | 48 KiB |
| One sync bundle item including payload/proofs | 2 MiB |
| Listing expiry from admission | 30 days |
| Ticket expiry from admission | 90 days |
| Prepared operation lifetime | 1 hour |

Bounds are checked before host staging, listing persistence, or feed
construction. A general protocol-valid record that exceeds them receives the
typed `anchor_profile_oversize` refusal and remains valid for nearby/file/P2P
exchange. Thus every valid anchor-profile listing and inclusion fits the 64 KiB
control-frame limit.

MVP defaults and absolute ceilings:

| Resource | Default | Absolute ceiling |
| --- | ---: | ---: |
| Logical retained bytes, whole anchor | 20 GiB | 100 GiB |
| Physical retained bytes | 20 GiB | 100 GiB |
| Whole SQLite database including WAL | 24 GiB | 110 GiB |
| Non-payload metadata bytes | 2 GiB | 8 GiB |
| SQLite WAL bytes | 256 MiB | 1 GiB |
| Staged bytes | 256 MiB | 1 GiB |
| Live staged operations | 10,000 | 50,000 |
| Idempotency rows | 100,000 | 500,000 |
| Idempotency rows per source per 24 h | 2,000 | 10,000 |
| Incident/conflict records | 10,000 | 50,000 |
| Conflict proofs per site/subject | 2 | 4 |
| Hosted sites | 10,000 | 50,000 |
| Logical bytes per site | 64 MiB | 256 MiB |
| Live entries per namespace | 4,096 | 16,384 |
| Item payload | 1 MiB | 1 MiB |
| Bundle | 8 MiB / 64 entries | unchanged |
| Concurrent sync/control sessions | 128 | 512 |
| Sessions per source | 4 | 16 |
| Sessions per site | 8 | 32 |
| Search results per page | 50 | 100 |
| Search query UTF-8 bytes | 128 | 256 |
| Directory listings | 10,000 | 50,000 |
| Directory-feed records | 100,000 | 500,000 |
| Verification queue jobs | 512 | 2,048 |
| Verification CPU per request | 500 ms | 2 s |
| Aggregate outstanding verification CPU budget | 16 s | 64 s |
| Work-challenge signatures per second | 100 | 500 |
| Work challenges per source per minute | 30 | 120 |
| Static projection bytes | 5 GiB | 20 GiB |
| Renderer temporary filesystem | 1 GiB | 4 GiB |
| Renderer temporary files/inodes | 10,000 | 50,000 |
| Concurrent renderer jobs | 4 | 16 |
| Renderer CPU/wall time per generation | 30 s | 120 s |
| Published generations per site | 2 | 2 |
| Local operational log bytes, all classes | 512 MiB | 2 GiB |
| Diagnostic log bytes | 128 MiB | 512 MiB |
| Rotated local log files | 128 | 512 |
| Concurrent gossip sessions per peer | 2 | 4 |
| Gossip transfer per peer per hour | 256 MiB | 1 GiB |

Before creating any durable row for a new request, the anchor applies, in
order: frame/body bounds, in-memory connection and source rate limits, existing
idempotency lookup, global metadata/row headroom, work verification when
required, then canonical/admission verification. Exact idempotent replay reads
the prior row without repeating work. A novel key that exceeds any source or
global ceiling is refused without persistence.

Challenge signing has separate in-memory per-source and global token buckets
before any KMS call. Concurrent final requests for one idempotency key race on
the single durable `Claimed` row; only the winner can consume capacity, and all
exact losers replay its state.

Idempotency results and terminal refusals expire after 24 hours; abandoned
staging expires at its operation deadline; expired directory-feed history is
compacted behind a signed head checkpoint; conflict evidence retains only the
bounded proofs above. Startup recovery performs the same cleanup before
readiness. Database, WAL, metadata, static-tree, and verification limits are
independent hard checks, so payload deduplication cannot hide other disk or CPU
growth.

### Admission work stamp

Global ceilings protect the process but do not make anonymous admission fair.
For a previously unseen root or a new listing, an anchor uses a stateless work
challenge:

```text
WorkChallengeV1 {
  anchor_id,
  operator_key_id,
  descriptor_epoch,
  descriptor_digest,
  operation_kind,
  idempotency_key,
  canonical_request_body_digest,
  community_root,
  random_challenge,
  policy_epoch,
  difficulty,
  issued_at,
  expires_at
}

operator_signature =
  Sign("riot/anchor-work-challenge/v1" || canonical_cbor(challenge))

proof =
  BLAKE3("riot/anchor-work-proof/v1" || digest(challenge) || counter)
```

The proof must have the challenge's number of leading zero bits. The anchor
first verifies its own challenge signature, all bindings, and time window.
Difficulty is `0..24`, the challenge expires after five minutes, and the work
stamp is therefore bound to one anchor, request key, exact request body, root,
operation kind, and pressure-policy epoch. A proof cannot authorize a different
request. Replaying the exact request can only retrieve its idempotent result.
Difficulty zero means no work.

The descriptor publishes a deterministic pressure policy. The default policy
uses difficulty zero while the maximum of logical-storage, staged-storage,
site-count, listing-count, and verification-queue utilization is below 75%.
At or above 75%, a valid nonzero work stamp is mandatory for every unseen root
and new listing. Difficulty increases monotonically with the published
utilization bands and never exceeds 24. Existing admitted roots may refresh
without work while within their quotas. At any hard ceiling, new admissions
are refused regardless of work.

This remains open hosting: no identity, approval, payment, or invitation is
introduced. It gives operators a bounded pressure valve rather than pretending
root signatures alone resist Sybil floods.

Accepted sites within their reported horizon are not displaced by new Sybil
roots. At hard capacity the anchor refuses new roots.

### Configured-peer authentication

Peer-only control operations begin with this exact `riot/anchor/1` handshake;
all frames are canonical CBOR. Hello/proof/request/ack frames are at most
4 KiB; descriptor pages follow the 60 KiB chain-page limit:

```text
PeerHello {
  role: Initiator | Responder,
  nonce: 32 random bytes,
  anchor_id,
  operator_key_id,
  descriptor_epoch,
  descriptor_digest,
  iroh_endpoint_id,
  issued_at
}

PeerProof {
  role,
  signature
}

PeerDescriptorRequest { anchor_id, after_descriptor_digest }
PeerDescriptorPage { envelopes: at most 16, done }
PeerDescriptorAck { anchor_id, verified_head_digest }
```

Peer configuration pins stable `AnchorId`, genesis operator key, and a minimum
descriptor digest. The initiator sends `PeerHello(Initiator)`. If its head is
not already provisioned, the responder sends `PeerDescriptorRequest` from its
pinned floor; the initiator returns byte-bounded chain pages and the responder
verifies through the Hello head before sending `PeerDescriptorAck`. The
responder then sends `PeerHello(Responder)` and the initiator performs the same
request/page/ack sequence when needed.

Only after both heads are verified, fresh within five minutes, and bound to the
configured anchors/roles/endpoints does the responder send its proof. The
initiator verifies it and sends its proof. Nonces must differ. The unauthenticated
descriptor exchange permits at most 32 hops, 256 KiB, and five seconds total,
uses the normal verification CPU budgets, and exposes no peer operation or
replication budget. An exact current descriptor may instead be provisioned out
of band, in which case each side immediately acknowledges it.

The transport adapter exposes the live QUIC TLS exporter:

```text
channel_binding =
  TLS-Exporter("EXPORTER-Riot-Anchor-Peer-v1", 32 bytes)

transcript = canonical_cbor(
  protocol_version,
  negotiated_alpn,
  initiator_hello,
  responder_hello,
  channel_binding
)

signature =
  Sign(
    operator_key,
    "riot/anchor-peer-proof/v1" ||
    role_byte ||
    BLAKE3(transcript)
  )
```

Both proofs bind roles, nonces, operator keys, stable anchor IDs, descriptor
epochs/digests, iroh endpoint IDs, ALPN, and this live connection. A reflected,
replayed, cross-role, cross-endpoint, or cross-connection proof fails.
`PrepareReplica`, `PullDirectoryFeed`, and `PullDirectorySnapshot` remain
unavailable until both proofs verify and the configured-rule lookup succeeds.
Failure returns one bounded `peer_auth_failed { stage }` refusal and closes the
connection.

### Gossip amplification boundary

An arbitrary client request never enqueues cross-anchor replication.

Anchor gossip occurs only when:

- both operators configured a peer relationship;
- the configured-peer handshake above succeeds;
- the relationship names directory-only or explicit site replication rules;
- per-peer work and byte budgets permit it.

Clients achieve baseline redundancy by hosting independently on several
anchors. Gossip improves reach but is not required.

## Directory Feeds and Search

Each anchor owns an append-only signed public directory feed. Its canonical
record is:

```text
DirectoryInclusionV1 {
  anchor_id,
  operator_key_id,
  descriptor_epoch,
  descriptor_digest,
  sequence,
  previous_inclusion_digest,
  state: Included | Suspended | Removed,
  reason,
  listing_entry_and_capability,
  optional_listing_delegate_grant,
  public_site_ticket_core,
  admitted_manifest_entry_and_capability,
  accepted_manifest_digest_and_version,
  observed_at
}
```

The operator signs:

```text
Sign(
  operator_key,
  "riot/directory-inclusion/v1" ||
  canonical_cbor(inclusion_body)
)
```

Sequence starts at one and increases by one; the previous digest makes
truncation or reordering detectable relative to a signed head.
`ListingReceiptV1.feed_coordinate` is exactly
`(anchor_id, sequence, inclusion_digest)`.

`Included` is emitted only after the local atomic hosted-manifest/listing match
defined by `SubmitListing`. Unlisting, expiry, manifest mismatch, hosting
eviction, or equivocation emits `Suspended` or `Removed` with a typed reason.
The signed manifest proof lets a peer independently validate structural
admission; the source receipt is evidence about the source, never authority for
the receiving anchor.

Configured peers pull this log through `PullDirectoryFeed`. A peer's verified
record may appear in its merged search with the source anchor named even when
the peer does not host that site. It never enters the peer's local raw feed or
web mirror unless the peer separately completes `PrepareReplica`, commit, and
local listing admission. Peer cursors persist `(operator key, descriptor epoch,
sequence, inclusion digest)` and reject forked histories.

### Feed checkpoint and bounded recovery

Before compacting, the anchor creates:

```text
DirectoryCheckpointV1 {
  anchor_id,
  operator_key_id,
  descriptor_epoch,
  descriptor_digest,
  floor_sequence,
  head_sequence,
  head_inclusion_digest,
  state_digest,
  previous_checkpoint_digest,
  created_at
}
```

It signs
`"riot/directory-checkpoint/v1" || canonical_cbor(checkpoint_body)`.
`state_digest` is:

```text
BLAKE3(
  "riot/directory-state/v1" ||
  for each current root sorted by full root bytes:
    u32be(len(root)) || root ||
    u32be(len(current_inclusion_digest)) || current_inclusion_digest
)
```

Thirty days is the normal incremental-history retention target, not a promise
that can override the compiled cap. Every compacted entry must first be covered
by a signed checkpoint. The newest two checkpoints and their complete
current-state snapshots are retained.

`floor_sequence` is the lowest accepted incremental cursor: a request with
`after_sequence == floor_sequence` receives the next retained sequence, while
for `after_sequence < floor_sequence`, `PullDirectoryFeed` returns
`CheckpointRequired { checkpoint, snapshot_cursor }`.
`PullDirectorySnapshot` then pages current `DirectoryInclusionV1` records in
full-root order under that immutable checkpoint, at most one 48 KiB record and
60 KiB per frame. The peer verifies every record, recomputes `state_digest`,
sets its cursor to `head_sequence/head_inclusion_digest`, then resumes the
incremental feed. A checkpoint mismatch or unavailable snapshot is a typed
fail-closed error, never an empty feed.

Ten percent of the feed-record ceiling is reserved for unlisting, expiry,
equivocation, and security suspension. At 90% utilization the anchor refuses
new listings/refreshes and creates an emergency checkpoint. A removal arriving
while the reserve is under pressure atomically replaces that root's bounded
current-state record with a signed `Removed` inclusion, builds and fsyncs a new
checkpoint/snapshot, advances the floor past the coalesced event, then compacts
covered incremental rows before acknowledging its removal receipt. This
constant-per-root emergency path may shorten incremental history below the
30-day target, but never exceeds the cap or blocks an owner removal; offline
peers recover through the checkpoint snapshot.

The HTTPS API:

```text
GET /.well-known/riot-anchor.json
GET /.well-known/riot-anchor-chain/v1?after=<descriptor-digest>&cursor=
GET /api/v1/feed?after=&limit=
GET /api/v1/feed/snapshot?checkpoint=&cursor=
GET /api/v1/directory?q=&cursor=&limit=
GET /api/v1/directory/<full-community-root>
GET /open/v2/<canonical-base64url-envelope>
GET /c/<full-community-root>/
GET /c/<full-community-root>/e/<full-entry-id>
```

Search responses are lossless JSON projections containing:

- the canonical listing envelope for client verification;
- the canonical root-signed public-site ticket core;
- directory source anchor IDs;
- source coverage (`complete` or `partial`);
- a stable opaque cursor;
- no claim of endorsement.

For each item, the API also returns an HTTPS handoff URL constructed from that
exact retained ticket core, an optional requested destination, and current
bounded source hints. The anchor can replace hints but cannot change or mint
the root-signed core. Native Explore consumes the same core directly, so
directory discovery and web sharing enter one Follow state machine.

For a peer-learned item the URL origin must be a verified source descriptor
whose latest inclusion says `Included` and whose roles contain both `host` and
`mirror`. Sources are tried in the user's configured priority, then full
`AnchorId` order. If none qualifies, the item exposes a native handoff and
“Web view unavailable”; the querying anchor never emits a local mirror URL for
content it does not host.

The public feed endpoint is a lossless JSON envelope carrying base64url
canonical signed inclusion/checkpoint bytes plus `head`, `floor`, and opaque
cursor. An old cursor returns HTTP 409 with typed `checkpoint_required` and the
signed checkpoint/snapshot cursor. Its snapshot endpoint is the HTTPS
projection of the same immutable `PullDirectorySnapshot` protocol; cursors
bind anchor ID, checkpoint digest, and last full root.

Inputs are normalized and bounded. Database queries are parameterized. Cursors
bind the normalized query and snapshot generation so they cannot be reused
across queries.

Open hosting does not imply automatic listing. A valid listing submitted under
budget for a currently hosted site appears in that anchor's raw directory feed.
The default Explore view merges local and configured peer feeds, deduplicates
one visible record per root, excludes expired/conflicted/unlisted records, and
exposes topic/language/region filters. It does not claim Sybil-proof ranking.

## Safe Web Projection

The existing fixed gateway remains intact. The anchor adds a new isolated
snapshot pipeline:

1. Rust reads one admitted SQLite snapshot.
2. Rust emits `AnchorWebSnapshotV1`, containing only typed, bounded public text
   records and canonical full identifiers.
3. A renderer worker with no database credentials reads the snapshot and writes
   a complete static tree.
4. The anchor validates the output manifest and atomically swaps the served
   directory.

Production ships the daemon and renderer as one orchestrated deployment unit
with separate OCI containers, not a same-network-namespace pod. The renderer
job uses `network_mode: none` (or an equivalent dedicated empty network
namespace), is unprivileged, has a read-only root filesystem, a restrictive
seccomp profile, and write access only to one quota-limited
generation-temporary volume. The daemon alone can
atomically publish a validated generation. A local-development subprocess is
allowed but is not described as a security boundary. The renderer may reuse
extracted pure templates from `apps/gateway`; it does not weaken the existing
pinned-fixture constructor or tests.

MVP mirror rules:

- typed text records only;
- context-aware escaping for text and attributes;
- routes derived only from canonical validated full IDs;
- no owner-authored HTML, CSS, JavaScript, SVG, iframe, or executable MIME;
- no inline arbitrary attachments;
- unknown payload profiles are omitted with a plain explanation;
- success and error pages carry CSP as an HTTP header;
- `default-src 'none'`;
- `base-uri 'none'` and `form-action 'none'`;
- only the exact hashed static stylesheet is permitted;
- `frame-ancestors 'none'`, `nosniff`, HSTS, and restrictive
  `Permissions-Policy`;
- no cookies and no third-party requests.

The output manifest lists every relative path, byte length, BLAKE3 digest, and
MIME type. Validation walks the generation using no-follow file APIs and
rejects symlinks, hard links, devices, sockets, non-regular files, path escapes,
duplicate or case-colliding routes, files absent from the manifest, unexpected
MIME types, size/count overruns, and changed digests. Security headers are
applied by the daemon to every success and error response, not by templates.

Publication is an ownership transfer, not validation followed by reuse of a
writable tree:

1. The renderer exits and its temporary mount is detached.
2. The daemon opens every source and destination with no-follow descriptors,
   copies into a fresh daemon-owned generation, and hashes bytes during copy.
3. It rechecks the manifest, fsyncs files and directories, makes the tree
   read-only to the daemon's serving identity, and confirms the renderer
   container has no mount or descriptor into it.
4. Only then does it atomically rename the generation into service.

Renderer temporary output is on its own filesystem quota and cannot consume
database, log, or published-generation headroom.

Riot-native sync still carries admitted payloads. The web MVP does not attempt
to serve arbitrary media safely.

Directory-card cache lifetime never exceeds listing expiry. Direct content
projection follows currently hosted admitted records and can remain readable
after unlisting or ticket expiry. Hosting eviction removes the direct
projection; unlisting removes only directory/search generations. Record
change, manifest change, moderation change, or hosting eviction invalidates the
affected content generation.

## Canonical Web-to-App Handoff

Anchor-generated pages use one canonical CBOR `HandoffEnvelopeV2` and encode
the same bytes in both schemes:

```text
riot://site/v2/<canonical-base64url-envelope>
https://<anchor-origin>/open/v2/<canonical-base64url-envelope>
```

The envelope contains:

- `signed_core`: the site-wide root-signed `PublicSiteTicketV2` retained from
  admitted hosting; a listing, when present, carries the exact same core;
- `destination_entry`: optional full entry ID outside the ticket core;
- `anchor_hints`: unsigned replaceable routing metadata.

The envelope is at most 1,800 canonical bytes before base64url encoding and
contains at most three hints of 192 bytes. This keeps the complete HTTPS URL
within 2,500 characters for QR and platform handoff compatibility. Larger
configured anchor sets remain local and are not copied into the envelope.

A client verifies the signed core before using the destination or dialing. The
destination must name an entry in one of the ticket's exact namespaces and,
after sync, must verify as admitted content. If unavailable, Riot opens the
verified site home and reports that the requested item was unavailable; it
never substitutes another entry. Hints are parsed as a separate field and can
never overwrite the site, namespace, manifest, transport, expiry, or
destination fields. Reissuing the same signed core and destination with
different hints does not change community identity or authority.

The HTTPS handoff route serves the safe projection directly with no redirect,
fingerprinting token, cookie, or query string. Its “Open in Riot” link is the
custom-scheme form containing the byte-identical envelope. The QR code encodes
the ordinary HTTPS form so a device without Riot still reaches a readable
page. Direct `/c/...` and `/e/...` routes remain readable canonical content
routes, but their share/Open controls emit this handoff URL.

Existing links remain supported:

- `riot://newswire/join/v1/...` remains the legacy newswire join reference;
- `riot://open?namespace=...&entry=...` continues to verify/open content already
  held locally;
- neither is emitted as the new composite anchor handoff.

### App installed

“Open in Riot” launches the v2 reference. Riot:

1. validates the ticket before dialing;
2. resolves configured anchors plus bounded hints;
3. shows the unknown-community preview;
4. follows after explicit acceptance;
5. opens the destination entry after commit, or the site home when no
   destination is present.

### App not installed

The HTTPS page remains the durable continuation. It shows platform install
links, the QR code, and:

> Install Riot, return to this page, then tap Open in Riot.

No fingerprinting or server-side deferred-link token is used. The complete
ticket, destination, and bounded hints remain in the HTTPS path and QR, so
returning preserves the journey even after browser or app-store round trips.

### Install unavailable

The visitor can continue reading the safe web projection and copy/save the
normal HTTPS link or QR. The page does not claim offline availability.

### Expired, malformed, or failed import

The page stays readable. Riot identifies whether the ticket, manifest,
capability, or anchor attempt failed; preserves the web URL; and offers retry
with configured anchors or cancel. An anchor cannot refresh an expired
root-signed ticket; the page says the organizer must publish a refreshed
listing while preserving the still-readable web projection. Riot never
silently opens a different community.

## Native UX State Model

User-facing language says “public hosts” or “Find communities online.”
“Anchor,” descriptor digests, and full protocol details live under Technical
details.

### Explore placement

Explore is available:

- from the first-run/community chooser as “Find communities online”;
- from the existing community switcher;
- without requiring the user to leave a currently selected community.

States:

| Technical state | User-facing result |
| --- | --- |
| Loading | “Looking across public hosts…” with cancel |
| Complete, zero results | “No communities matched” with filters/reset |
| Partial source success | Results plus “Some public hosts didn’t respond” and retry |
| All sources unreachable, cached results | Cached results marked “Saved results — may be out of date” |
| All sources unreachable, no cache | “Couldn’t reach public hosts” with retry and link/QR option |
| Listing conflict/expiry | Omitted from results; technical details explain why |

### Follow

States:

- validating ticket;
- contacting named public hosts;
- preview ready;
- already followed;
- saving;
- saved and ongoing updates enabled;
- cancelled with no mutation;
- source-specific refusal;
- local `site_too_large`, showing the required storage class and local limit,
  with actions to manage followed-site storage or cancel; no partial follow;
- all sources unreachable;
- manifest/listing equivocation quarantined;
- expired ticket with preserved readable web destination and organizer-refresh
  explanation.

### Publish and listing

Maintainers see per-anchor progress:

- preparing;
- syncing `O`, `C`, `W`;
- hosted through reported date;
- refused with named quota/subject;
- unreachable;
- receipt recovery;
- listing submitted;
- listing expired/refresh due;
- unlisted.

Hosting and listing are separate controls. A failed listing refresh never
removes hosted state.

The host-management screen starts with the configured defaults selected and a
recommended target of three independent hosts. It explains each signed
retention date as “reported through,” never guaranteed. Adding redundancy,
replacing a failed host, disabling a route, refreshing hosting, listing,
unlisting, and resetting default hosts use the typed transitions above. Local
route removal is labeled “Stop using this host”; remote listing removal is
“Remove from this host’s directory.” Neither is presented as deleting copies.

“Independent” in the client means different stable operator identities and
different signed self-reported failure-domain labels. The UI marks the latter
as operator-reported, not independently verified. Pilot operators and failure
domains are verified out of band before the observation window.

### Accessibility

All web and native host flows are keyboard and screen-reader operable. QR is
always paired with a copyable HTTPS URL and native share action. Multi-anchor
progress changes use polite live announcements, preserve focus on retry/install
return, expose text labels in addition to color or icons, and do not reorder
focused rows. Partial, stale, refused, expired, and unreachable states have
distinct accessible names. Reduced-motion settings disable nonessential
progress animation.

## Privacy and Logging

A Riot anchor is a public application endpoint. It sees public content and can
observe client IP, requested community, timing, query, and transfer volume.
Iroh encryption does not hide these facts from the endpoint.

Default application, reverse-proxy, CDN, and load-balancer configuration:

- does not log query strings, tickets, capability material, payload bodies, or
  per-IP/community pairs;
- truncates operational access logs to seven days;
- restricts access to operators;
- emits aggregate counts by default;
- requires an explicit, time-bounded diagnostic mode for detailed request
  correlation.

Application, proxy, audit, crash, CDN-export spool, and diagnostic logs share
the compiled byte/file ceilings above. Size rotation runs before the seven-day
time limit and deletes oldest closed files first. Diagnostic mode has its own
smaller subquota and shuts off when full or expired. External proxy/CDN sinks
must declare equivalent retention and storage caps in deployment
configuration; an unbounded sink fails readiness validation. Log saturation
drops new non-authoritative log events and increments a bounded in-memory
counter; it never consumes database/static headroom or blocks protocol
readiness. Logs are not authority or recovery state.

Public IDs are printed in full when a specific diagnostic requires them; they
are never truncated.

Ticket and descriptor parsers:

- reject duplicate authoritative fields and ambiguous encodings;
- cap v2 handoff envelopes at 1,800 canonical bytes and hints at three entries
  of 192 bytes;
- allow only defined iroh and HTTPS hint schemes;
- verify the signed transport floor before every native dial;
- enforce the client and anchor safe-dial rules before any HTTPS fetch;
- never permit a client-supplied URL to trigger server-side fetching.

## Failure and Threat Model

A malicious anchor can:

- observe connection metadata;
- withhold or omit state;
- serve stale but previously valid state;
- omit a listing locally;
- lie in an unsigned web projection;
- issue a dishonest receipt;
- disappear.

It cannot:

- forge site roots, manifests, listings, entries, or Meadowcap chains;
- write outside an admitted capability;
- replace the signed site identity through an anchor hint;
- force rollback below durable manifest/listing/transport floors;
- delete copies held by clients or other anchors.

Directory inclusion means “owner-authorized and structurally admitted,” not
“safe,” “true,” or “endorsed.” Trust and moderation remain reader-selected
lenses.

## Performance and Pilot Contract

Engineering targets under the default limit profile:

- warm directory query p95 below 500 ms at 10,000 listings;
- anchor descriptor p95 below 200 ms;
- restart recovery and receipt reconstruction below 30 seconds at 20 GiB;
- transparent failover begins within 5 seconds and completes within 30 seconds
  when another anchor is reachable;
- no unbounded allocation or queue growth under invalid input.

Public pilot prerequisites:

- at least three anchors;
- at least two independent operators;
- at least two infrastructure failure domains or regions.

The observation window is seven consecutive days.

### Privacy-compatible pilot measurement

Pilot measurement is client-owned and opt-in. Riot records a local
`PilotMetricsV1` aggregate using a monotonic clock. It contains only counters,
success/failure categories, and coarse duration buckets; it contains no
community, entry, anchor, operator, query, IP, capability, or stable device
identifier. Participants see the aggregate and explicitly export it to the
pilot collector. Anchor logs are not joined to client metrics.

Event boundaries and denominators are fixed:

- A reported follow attempt begins at `Validating`; the “at least 50” pilot
  count includes valid, invalid, cancelled, accepted, and failed attempts.
- A follow journey starts when a user opens a verified Explore result or v2
  handoff and enters `AwaitingConsent`; that monotonic instant is its duration
  start. Let `A` be every such journey that later emits `Accept`. `A` includes
  post-accept cancellation, app termination, timeout, refusal, and transport
  failure. Pre-accept cancellation and inputs that never verify into
  `AwaitingConsent` are reported separately and are not in `A`.
- Let `S120` be members of `A` that reach `Saved` within 120 seconds of their
  duration start, and `S` be members that reach `Saved` at any time before the
  five-minute follow-attempt timeout. The 90% threshold is exactly `S120 / A`.
  The 80% stop/revise completion criterion is exactly `S / A`.
- A host-plus-list journey starts at organizer confirmation with at least two
  selected hosts and ends when two hosting receipts and at least one listing
  receipt are verified. Its three-minute percentage is successful journeys
  completed within 180 seconds divided by all confirmed journeys; refusal,
  cancellation, app termination, or the ten-minute organizer-attempt timeout
  remains in the denominator.
- An intentional failover trial starts when the harness marks one configured
  anchor unavailable immediately before the first dial and ends at `Saved`
  through another anchor or at 30 seconds. Its percentage is successful
  alternate-anchor saves within 30 seconds divided by all intentional trials.
- Destination preservation compares the input full root/entry bytes with the
  locally verified saved/opened result. A legitimately absent destination is
  reported separately and is not counted as opening a different entry.
- Let `I` be first-ever app launches that carry a valid v2 envelope after the
  participant began on the no-app HTTPS page, and `I5` be those reaching
  `Saved` within five minutes of that first app handling. This local
  `first_launch_handoff` classification needs no browser fingerprint or
  server-side correlation.

The collector sums exported aggregates and publishes the denominator beside
every percentage. Consent withdrawal deletes unsubmitted local metrics.

A pilot is valid only with:

- at least 50 reported follow attempts;
- `A >= 30` accepted follow journeys;
- at least 10 confirmed host-plus-list journeys;
- at least 20 intentional failover trials;
- at least 20 accepted destination-bearing follows;
- `I >= 10` no-app install-return journeys.

A zero or subminimum denominator is **inconclusive**, never a pass. The
observation window must be extended or the pilot rerun.

User-focused pilot thresholds:

- at least 10 organizers and 30 readers, plus every valid denominator above;
- `S120 / A >= 90%` for installed-app discovery-to-saved journeys;
- 80% of organizer host-plus-list journeys complete within three minutes;
- 95% of follow attempts succeed through another anchor within 30 seconds when
  one configured anchor is intentionally unavailable;
- 100% of the at least 20 destination-bearing accepted follows preserve the
  requested full site and destination entry IDs;
- `I5 / I >= 80%`, with 100% of successful install returns preserving root and
  requested destination bytes;
- zero invalid capability or manifest admissions.

Stop/revise criteria:

- a valid site becomes unreachable solely because one anchor is lost;
- a web-to-app handoff opens a different site or loses its requested entry;
- any invalid authority reaches retained state or web output;
- open admission can exceed a compiled global ceiling;
- `S / A < 80%` for installed-app follow journeys;
- any required pilot denominator is below its minimum.

## Deployment and Recovery Contract

The anchor deployment includes a persistent SQLite volume, versioned
forward-only schema migrations, the daemon, renderer sidecar, static
generation volume, and explicit limit/peer/bootstrap configuration. Startup
runs migration and bounded recovery before readiness. Liveness reports process
health only; readiness requires a writable database, loaded operator key,
valid descriptor, recovered operation table, and one validated published or
empty web generation.

Graceful shutdown stops accepts, gives active commits a bounded drain window,
checkpoints WAL, and leaves unfinished operations recoverable. Rollback is
allowed only to a binary declaring compatibility with the current schema and
record versions; otherwise deployment fails closed. Backups contain the
database and operator-state metadata but never export community signing keys,
which the anchor does not own.

## Deterministic Test Harness

New support lives under `crates/riot-anchor/tests/support/` and uses production
interfaces:

- `TestAnchor` with isolated SQLite database and keys;
- `TestClient` using the public control/sync protocol types;
- `TestAnchorNetwork` with deterministic partitions and ordered schedules;
- `FakeClock`;
- in-memory duplex `AnchorTransport`;
- `DeterministicGossipScheduler`;
- `FailpointRepository` with named production boundaries:
  `after_stage_bundle_commit`, `before_promotion_transaction`,
  `before_promotion_commit`, `after_promotion_before_receipt_delivery`,
  `before_projection_publish`, and `after_projection_publish_before_cleanup`;
- `FakeRenderer` recording immutable snapshot generations;
- bounded operation reports containing metadata only.

Real iroh and HTTP integration tests are separate from the deterministic
network. No test reaches into repository internals to manufacture accepted
state.

Required injected production seams:

- clock;
- operator key store;
- repository transaction/failpoint boundary;
- control transport;
- sync transport;
- gossip scheduler;
- projection renderer;
- work-challenge verifier.

## TDD Delivery Slices

### Slice 1: Authority records and tickets

RED:

- same-version manifest equivocation is not quarantined;
- editorial capability incorrectly authorizes `/directory/listing`;
- max-revision delegate prevents root recovery;
- listing/ticket/manifest coordinates can disagree;
- ticket transport summary differs from the manifest;
- duplicate ticket fields, oversize anchor proofs, unsupported Arti floor, and
  v1/v2 downgrade are not rejected.

GREEN:

- canonical records, root-controlled listing epochs/grants, exact manifest and
  transport match, floors, tombstones, v2 ticket, and profile bounds pass
  focused tests.

REFACTOR:

- share canonical decoders and authority predicates without changing existing
  v1 behavior.

### Slice 2: `sync/2`

RED:

- responder cannot route a namespace before constructing a session;
- 257+ IDs cannot reconcile in bounded pages;
- one 64-ID request exceeds 8 MiB or chunk indexes fork;
- cursor overlap/digest change is not rejected;
- read sync can mutate anchor state or staged host sync can leak before commit;
- replica sync mutates the source or organizer state commits before receipt;
- independent implementations disagree on frame order or snapshot digest.

GREEN:

- golden snapshot-digest vectors and exact FSM traces pass;
- one-way public read and two-phase staged reconciliation converge over
  in-memory duplex for `O`, `C`, and `W`.

REFACTOR:

- isolate common bundle admission shared with `sync/1`.

### Slice 3: Anchor control and repository

RED:

- crash between staging and commit leaks partial visibility;
- operation IDs collide with idempotency keys or duplicate work/receipts;
- prepared replay after restart changes operation ID or namespace token;
- work proof replays against another body;
- logical quota is bypassed by cross-community dedup;
- metadata-only requests exceed a persistent ceiling;
- client O/C/W state overflows the legacy evidence repository;
- existing profile migration/backup/restore loses client-site stages or floors.

GREEN:

- routed multi-ALPN Describe/Prepare/Sync/Commit, durable prepared replay,
  server and scalable client SQLite atomic promotion, receipt recovery,
  authenticated work, all-class quotas, reference accounting, and restart pass.

REFACTOR:

- separate protocol types from daemon adapters.

### Slice 4: Plural directory and bounded gossip

RED:

- hosting implies listing;
- conflicting listing revisions produce divergent search;
- client-triggered requests create background fanout;
- feed forks, stale pre-checkpoint cursors, unauthenticated/reflected peer
  proofs, and listing-before-hosting become visible;
- unseen peer descriptor rotation cannot authenticate or removal reserve
  exhaustion blocks an owner tombstone.

GREEN:

- signed chained feeds/checkpoint snapshots, listing/unlisting/suspension,
  channel-bound configured replica gossip, typed coverage, and three-anchor
  quiescence pass.

REFACTOR:

- extract deterministic scheduler policies.

### Slice 5: Static web projection and handoff

RED:

- hostile HTML/SVG/attribute strings execute or escape context;
- symlink/path/MIME output escapes renderer validation;
- renderer mutates a file after validation or fills its temporary filesystem;
- app-absent journey loses the ticket/destination;
- old deep links regress.

GREEN:

- sandboxed immutable ownership-transfer rendering, validated output manifests,
  security headers, bounded byte-identical HTTPS/custom v2 handoff,
  install-return journey, and v1 compatibility pass.

REFACTOR:

- extract pure templates while preserving pinned gateway fixtures.

### Slice 6: Native plural-source UX

RED:

- zero results collapse with partial source failure;
- receipt/refusal/transport errors collapse;
- one-anchor outage blocks follow;
- clean install has no verified defaults or reset path;
- host configuration or follow consent depends on undocumented side state;
- cancellation leaves native networking or staged local mutation alive.

GREEN:

- `riot-client-net` iroh/Tokio lifecycle and UniFFI cancellation tests pass;
- injected ports drive every Explore, Follow, Publish, host-management,
  refresh, cancel, accessibility, and recovery state on iOS and Android.

REFACTOR:

- share Rust result semantics through UniFFI while preserving native state
  presentation.

### Slice 7: Operational closure

RED:

- global capacity, verification queue, logging, migration, eviction, restart,
  aggregate-metric formula, renderer quota, or rollback contracts fail under
  adversarial load;
- a subminimum/zero pilot denominator is reported as pass.

GREEN:

- compiled ceilings, safe load shedding, deterministic eviction, privacy
  configuration, deployment packaging, and the pilot rehearsal pass.

REFACTOR:

- remove test-only duplication and update `SERVICE-INVENTORY.md`.

## Edge-Case Matrix

Tests explicitly cover:

- `now == expiry`;
- manifest and listing revision ties/equivocation;
- delegate at `u32::MAX` followed by root recovery;
- listing tombstone followed by stale replay;
- manifest upgrade during sync;
- duplicate/ambiguous anchor hints;
- loopback/private/DNS-rebinding hints and descriptor endpoint mismatch;
- work proof replay with another key or body;
- peer-proof reflection, replay, role swap, exporter mismatch, and stale hello;
- pre-auth peer descriptor refresh across same-key and changed-key rotations;
- prepared idempotency replay before/after restart and expiry;
- 64 requested maximum-size items split across ordered chunks;
- concurrent uploads for one site;
- crash before and after admission commit;
- eviction during a read snapshot;
- sync disconnect between commit and receipt;
- `GetOperation` after restart;
- directory feed fork/reorder and descriptor rollback;
- feed cursor before compaction floor and snapshot digest mismatch;
- removals beyond reserved feed capacity using emergency checkpoint coalescing;
- client pinned across several descriptor-key rotations;
- listing before hosting and hosting eviction after listing;
- gossip loops and final quiescence;
- unsupported `require:arti` ticket;
- all anchors unavailable;
- all embedded defaults disabled, removed, reset, or unreachable;
- zero and subminimum pilot denominators plus install-return minimums;
- one or two anchors unavailable;
- stale cached directory with partial source coverage;
- malformed Unicode/control characters in listings and search;
- hostile strings in every HTML context;
- post-validation renderer mutation and log/temp-filesystem exhaustion;
- cancellation before consent acceptance and before commit.

## Quality Gates

Before completion:

- `cargo test --workspace --all-features`;
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-features --all-targets -- -D warnings`;
- `cargo xtask validate-contracts`;
- `(cd apps/gateway && python3 -m unittest discover -s tests)` and
  `sh scripts/conference/gateway-smoke.sh`;
- `sh scripts/conference/build-native-core.sh`, which regenerates UniFFI
  bindings and rebuilds the iOS/Android libraries in the same gate;
- `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'
  -derivedDataPath build/ios-derived`;
- `xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'
  -derivedDataPath build/ios-app-derived`;
- from `apps/android`, with the checked-in JDK/SDK prerequisites:
  `./gradlew :app:testDebugUnitTest :app:assembleDebug
  :app:assembleDebugAndroidTest :app:connectedDebugAndroidTest`;
- `scripts/web/coverage.sh`;
- thresholds from `.coverage-thresholds.json`:
  - Tarpaulin lines: 97;
  - LLVM lines/functions/regions/branches: 95/95/92/83;
  - JS tooling lines/branches/functions/statements: 100.

The absent generic metaswarm guide paths are not treated as evidence. Planning
uses checked-in CI docs, `SERVICE-INVENTORY.md`, this spec, existing tests, and
the headless-network design as the baseline.

## Non-Goals

- Private groups, MLS, encrypted drops, or private rendezvous.
- Read authorization.
- Legacy standalone communal-space hosting/listing.
- Accounts, passwords, PDS homes, or account migration.
- A global firehose or requirement that every anchor store everything.
- Canonical search ranking or network-wide moderation.
- Permanent retention guarantees.
- Payment or invitation gates.
- Arbitrary web HTML/media rendering.
- A custom iroh packet-relay network.
- Tor/Arti hosting; tickets requiring it are explicitly ineligible for the MVP.
- Bandwidth-optimal range reconciliation in the first `sync/2`.

## Definition of Done

The design is implemented when:

- composite public sites host on several independent anchors without accounts;
- clean installs verify a removable plural default set and users can
  add/remove/replace/reset hosts without changing community identity;
- hosting never implies listing;
- exact listing authority, owner recovery, and equivocation rules are enforced;
- clients discover through plural feeds and ordinary web pages;
- web/app handoff preserves site and entry destination across install;
- directory results and web pages carry the same root-signed public-site ticket
  core without giving anchors minting authority;
- clients atomically persist verified `O/C/W` state in the scalable site
  repository and schedule ongoing reconciliation;
- mobile internet operations run through the cancellable Rust-owned
  iroh/Tokio/UniFFI boundary;
- anchors enforce manifest and Meadowcap authority before propagation;
- the root-signed ticket bootstrap requirement is checked before a cold public
  dial, and the digest-matched manifest requirement is checked before content
  admission or composite commit;
- `sync/2` routes and paginates all composite namespaces without the 64-ID
  inventory ceiling or 8 MiB response dead end;
- control operations are canonical, idempotent, recoverable, and typed;
- operation IDs, idempotency keys, and authenticated work proofs cannot be
  replayed across bodies;
- stable anchor IDs, descriptor chains, and prepared namespace tokens survive
  key rotation and restart;
- the anchor repository survives crash/restart and enforces logical/global
  quotas;
- open hosting cannot exceed compiled process ceilings or trigger unbounded
  gossip;
- authenticated directory and replica gossip has a complete bounded wire
  lifecycle, channel-bound peer authentication, and checkpoint recovery;
- the text-only web renderer uses immutable ownership transfer and preserves
  the hostile-content boundary;
- deterministic three-anchor tests converge and survive anchor loss;
- opt-in aggregate metrics prove the public pilot meets the user-focused
  thresholds without correlated server logs;
- all quality and coverage gates pass.
