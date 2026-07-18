# Public Community Anchor Network Design

Date: 2026-07-18
Status: Design review rounds 1-2 revised; pending round 3
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
`riot-core`, but never on SQLite, HTTP, iroh, Tokio, or server adapters.

The anchor daemon is a new workspace binary crate, `riot-anchor`, depending on
`riot-anchor-protocol`, `riot-core`, and `riot-transport`.

Native internet operations use a new mobile-compatible `riot-client-net` crate
that owns one iroh endpoint and Tokio runtime per application process. It
depends on `riot-anchor-protocol` and `riot-transport`; `riot-ffi` exposes its
cancellable async operations and typed event streams through UniFFI. Existing
native nearby carriers remain unchanged. SQLite, HTTP-server, and renderer
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

The Riot admission layer additionally requires the entry's path to equal
`/directory/listing`; authority over arbitrary `/directory` children is not
interpreted as a new record type.

### `CommunityListingV1`

The canonical CBOR payload binds:

- schema `riot/community-listing/1`;
- full root and `O`, `C`, and `W` namespace IDs;
- manifest digest and version;
- a canonical root-signed `PublicSiteTicketV2` core;
- monotonically increasing listing revision;
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
digest/version, `min_sync_version: 2`, `transport_floor: require:none`,
transport epoch, issued time, and expiry. Its expiry must be at or after the
listing expiry. The listing entry and ticket core must name identical roots,
namespaces, and manifest coordinates or admission fails.

A dedicated listing delegate may update listing presentation and revision, but
cannot mint this root-signed ticket. The owner issues or refreshes the ticket
before delegating listing maintenance.

The Willow entry signature covers the payload digest, namespace, subspace,
path, timestamp, and capability. No second ad hoc signature scheme is added.

`listed: false` is an explicit unlisting tombstone. It stops future directory
display but does not delete hosted state or copies held by peers.

For a given root:

- higher valid revision wins;
- identical revision and digest deduplicate;
- identical revision with different digests is listing equivocation, so neither
  listing is shown;
- a higher **root-owned zero-delegation** listing clears listing equivocation;
- expiry is inclusive: the listing is invalid when `now >= expiry`.

Anchors persist the highest admitted revision and conflict evidence so restart
or cache eviction cannot roll the listing backward.

## Canonical Anchor Records

All anchor-owned records use deterministic CBOR with integer map keys,
definite-length containers, minimal integer encodings, and sorted collections.
Decoders reject unknown required versions, duplicate fields, non-canonical
encodings, and trailing bytes.

### `AnchorDescriptorV1`

The operator signs:

```text
"riot/anchor-descriptor/v1" || canonical_cbor(descriptor_body)
```

The body contains:

- full anchor operator key ID;
- monotonically increasing descriptor epoch;
- previous descriptor digest when epoch is greater than zero;
- current iroh endpoint ID;
- HTTPS origin;
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

The operator signs the listing digest, site root, accepted revision, directory
feed coordinate, acceptance time, expiry, and request idempotency key under:

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

Rotation uses a descriptor containing the predecessor key and digest and two
signatures: one by the old key over the new descriptor digest and one by the
new key over the descriptor. Clients accept overlap only before the old
descriptor expires. After verifying continuity they durably persist the
operator key, highest descriptor epoch, and descriptor digest. They reject any
lower epoch, any different digest at the same epoch, and every replayed
predecessor descriptor. Emergency revocation without the old key is an
explicit user-visible operational trust reset and is not hidden as normal
rotation.

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
    ReconcileStaged { operation_id, namespace_token }
}
```

`ReadCommitted` is the public follow/read path. It requires no control-plane
operation. The responder verifies the root signature, ticket expiry, transport
floor, exact namespace membership, and equality with its currently committed
manifest, then exposes a one-way committed snapshot.

`ReconcileStaged` is the host/replica path. The responder additionally verifies
the active operation and its unguessable, per-namespace 256-bit token, then
routes writes only to that operation's staged namespace. A stage is initialized
from the currently committed namespace when `PrepareHost` or `PrepareReplica`
creates the operation.

### Snapshot inventory

An immutable inventory sorts full canonical entry-ID bytes
lexicographically. Its digest is exactly:

```text
BLAKE3(
  "riot/sync-snapshot/v2" ||
  u32be(len(namespace_id)) || namespace_id ||
  u64be(entry_count) ||
  for each sorted ID: u32be(len(entry_id)) || entry_id
)
```

The frame set is:

```text
SnapshotStart { phase, namespace_id, snapshot_digest, entry_count }
IdsPage {
  phase,
  snapshot_digest,
  after_exclusive: optional EntryId,
  entry_ids: at most 256,
  done
}
NeedEntries { phase, page_digest, entry_ids: at most 64 }
PageNeedsComplete { phase, page_digest }
Entries { phase, page_digest, canonical bundle: at most 64 entries and 8 MiB }
PageComplete { phase, page_digest }
DirectionComplete { phase, sender_snapshot_digest }
NamespaceComplete { mode, final_snapshot_digest }
Refuse { code, subject, retryable, retry_after_seconds }
```

`page_digest` is the BLAKE3 digest of the canonical `IdsPage` frame. For each
direction the inventory sender and receiver follow this exact FSM:

1. Sender opens one immutable snapshot and sends `SnapshotStart`.
2. Sender sends exactly one strictly sorted `IdsPage`.
3. Receiver sends zero to four `NeedEntries` frames. Every requested ID must
   occur in that page and may occur once. Receiver ends requests with exactly
   one `PageNeedsComplete`.
4. Sender returns one `Entries` frame per request, in request order, then
   `PageComplete`.
5. Only after `PageComplete` may sender send the next page. Its first ID must be
   greater than `after_exclusive`.
6. After the page with `done: true`, receiver first verifies the advertised
   inventory digest from the received ID stream, then admits and commits all
   received bundles for that direction, and sends `DirectionComplete`.

Page overlap, duplicate IDs, cursor regression, digest change, out-of-page
need, unexpected frame, admission failure, or premature EOF terminates the
session with no visibility outside the relevant local/staged transaction.
Bundles retain the current 64-entry, 8 MiB, and 1 MiB-per-item ceilings.
Writes arriving after a sender opens its snapshot appear in the next session.

`ReadCommitted` executes one phase, `AnchorToClient`. After
`DirectionComplete`, the anchor sends `NamespaceComplete` with its committed
snapshot digest. The client may retain additional valid local entries; read
sync does not upload them.

`ReconcileStaged` executes two ordered phases:

1. `AnchorToClient` uses the stage's committed-base snapshot. The client admits
   and commits received entries locally.
2. Only then, the client opens a new immutable snapshot of its resulting local
   namespace and becomes inventory sender for `ClientToAnchor`. The anchor
   admits missing entries into the stage.

After phase two, the anchor computes the stage digest and sends
`NamespaceComplete`. It must equal the client's phase-two snapshot digest.
Any error leaves the prior committed site unchanged and marks the operation
retryable or failed according to the refusal.

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
request-body digest and terminal result for 24 hours. Repeating the same key
and body returns that result; reusing a key with another body is rejected.

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
`ReconcileStaged`. `CommitHost` produces a destination-signed receipt.
Replication never copies listing status implicitly.

### `PullDirectoryFeed`

This peer operation is paginated by the source anchor's monotonically
increasing feed sequence:

```text
PullDirectoryFeed { after_sequence, limit: at most 32 }
DirectoryFeedPage { inclusions, head_sequence, head_digest, done }
```

Each inclusion is at most 16 KiB and each page at most 60 KiB, preserving the
control-frame ceiling. The operation is read-only and available only to
authenticated configured peers.

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
`unsupported_transport`, `expired`, `not_hosted`, `manifest_mismatch`,
`equivocation`, `work_required`, and `busy`.
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
  Preview { verified_site, destination_status } |
  AwaitingConsent |
  Syncing { per_anchor_progress } |
  CommittingLocal |
  Saved { site_root, destination_status, attempts } |
  AlreadyFollowed { site_root, destination_status } |
  Cancelled { mutation: None } |
  Failed { phase, attempts, local_mutation: None } |
  Quarantined { subject, evidence }

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
  Replacing { old_anchor, new_anchor, phase } |
  Partial { completed, refused, unreachable } |
  Failed { phase, per_anchor }
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
- Follow mutates no durable state before explicit preview acceptance.
- Accepted data is staged locally; `Saved` is emitted only after one atomic
  local follow commit. Cancellation or failure before it emits
  `local_mutation: None`.
- Hosting, listing, unlisting, and refresh are independent per-anchor
  operations. Aggregate `Partial` never rewrites a verified per-anchor result.
- Replacing a host first reaches `Hosted` on the new anchor. Local removal of
  the old host then stops future client sync but does not claim remote deletion.
  Remote unlisting is a separate signed operation, and retained hosted state
  expires under the old receipt's policy.

Pure injectable ports mirror existing repository patterns:

- `AnchorDirectoryPort`;
- `AnchorHostingPort`;
- `AnchorSyncPort`;
- `AnchorConfigurationPort`;
- `Clock`;
- `RetryScheduler`.

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
| Static projection bytes | 5 GiB | 20 GiB |
| Published generations per site | 2 | 2 |
| Concurrent gossip sessions per peer | 2 | 4 |
| Gossip transfer per peer per hour | 256 MiB | 1 GiB |

Before creating any durable row for a new request, the anchor applies, in
order: frame/body bounds, in-memory connection and source rate limits, existing
idempotency lookup, global metadata/row headroom, work verification when
required, then canonical/admission verification. Exact idempotent replay reads
the prior row without repeating work. A novel key that exceeds any source or
global ceiling is refused without persistence.

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

### Gossip amplification boundary

An arbitrary client request never enqueues cross-anchor replication.

Anchor gossip occurs only when:

- both operators configured a peer relationship;
- the iroh peer endpoint equals the endpoint in a currently valid descriptor
  signed by the configured operator key;
- both sides complete nonce-based proof of possession of their operator keys
  over the connection transcript before receiving peer budgets;
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
  sequence,
  previous_inclusion_digest,
  state: Included | Suspended | Removed,
  reason,
  listing_entry_and_capability,
  public_site_ticket_core,
  admitted_manifest_entry_and_capability,
  accepted_manifest_digest_and_version,
  observed_at
}
```

The operator signs the domain-separated canonical record. Sequence starts at
one and increases by one; the previous digest makes truncation or reordering
detectable relative to a signed head. `ListingReceiptV1.feed_coordinate` is
exactly `(anchor_id, sequence, inclusion_digest)`.

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

The HTTPS API:

```text
GET /.well-known/riot-anchor.json
GET /api/v1/feed?after=&limit=
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

Production ships the daemon and renderer as one deployment unit with separate
OCI containers. The renderer sidecar is unprivileged, has a read-only root
filesystem, no network namespace interface, a restrictive seccomp profile, and
write access only to one generation-temporary volume. The daemon alone can
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
  the admitted listing;
- `destination_entry`: optional full entry ID outside the ticket core;
- `anchor_hints`: unsigned replaceable routing metadata.

The envelope is at most 1,800 canonical bytes before base64url encoding and
contains at most four hints of 192 bytes. This keeps the complete HTTPS URL
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

Public IDs are printed in full when a specific diagnostic requires them; they
are never truncated.

Ticket and descriptor parsers:

- reject duplicate authoritative fields and ambiguous encodings;
- cap v2 handoff envelopes at 1,800 canonical bytes and hints at four entries
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

- A follow journey starts when a user opens a verified Explore result or v2
  handoff and enters `AwaitingConsent`. It enters the duration denominator only
  after acceptance, ends at `Saved`, and records explicit cancellation,
  invalid ticket, and app termination as separate outcomes rather than silently
  excluding them.
- A host-plus-list journey starts at organizer confirmation with at least two
  selected hosts and ends when two hosting receipts and at least one listing
  receipt are verified. Refusal, cancellation, or timeout is a failed attempt.
- An intentional failover trial starts when the harness marks one configured
  anchor unavailable immediately before the first dial and ends at `Saved`
  through another anchor or at 30 seconds.
- Destination preservation compares the input full root/entry bytes with the
  locally verified saved/opened result. A legitimately absent destination is
  reported separately and is not counted as opening a different entry.

The collector sums exported aggregates and publishes the denominator beside
every percentage. Consent withdrawal deletes unsubmitted local metrics.

User-focused pilot thresholds:

- at least 10 organizers, 30 readers, and 50 follow attempts;
- 90% of installed-app discovery-to-saved journeys complete within two minutes;
- 80% of organizer host-plus-list journeys complete within three minutes;
- 95% of follow attempts succeed through another anchor within 30 seconds when
  one configured anchor is intentionally unavailable;
- 100% of accepted follows preserve the requested full site and destination
  entry IDs;
- zero invalid capability or manifest admissions.

Stop/revise criteria:

- a valid site becomes unreachable solely because one anchor is lost;
- a web-to-app handoff opens a different site or loses its requested entry;
- any invalid authority reaches retained state or web output;
- open admission can exceed a compiled global ceiling;
- fewer than 80% of installed-app follow journeys complete in the pilot.

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
- listing/ticket/manifest coordinates can disagree;
- duplicate ticket fields, unsupported Arti floor, and v1/v2 downgrade are not
  rejected.

GREEN:

- canonical records, reserved listing admission, exact manifest match, floors,
  tombstones, v2 ticket, and parser bounds pass focused tests.

REFACTOR:

- share canonical decoders and authority predicates without changing existing
  v1 behavior.

### Slice 2: `sync/2`

RED:

- responder cannot route a namespace before constructing a session;
- 257+ IDs cannot reconcile in bounded pages;
- cursor overlap/digest change is not rejected;
- read sync can mutate anchor state or staged host sync can leak before commit;
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
- work proof replays against another body;
- logical quota is bypassed by cross-community dedup;
- metadata-only requests exceed a persistent ceiling.

GREEN:

- routed multi-ALPN Describe/Prepare/Sync/Commit, SQLite atomic promotion,
  receipt recovery, authenticated work, all-class quotas, reference accounting,
  and restart pass.

REFACTOR:

- separate protocol types from daemon adapters.

### Slice 4: Plural directory and bounded gossip

RED:

- hosting implies listing;
- conflicting listing revisions produce divergent search;
- client-triggered requests create background fanout;
- feed forks, unauthenticated peers, and listing-before-hosting become visible.

GREEN:

- signed chained feeds, listing/unlisting/suspension, typed coverage,
  authenticated configured replica gossip, and three-anchor quiescence pass.

REFACTOR:

- extract deterministic scheduler policies.

### Slice 5: Static web projection and handoff

RED:

- hostile HTML/SVG/attribute strings execute or escape context;
- symlink/path/MIME output escapes renderer validation;
- app-absent journey loses the ticket/destination;
- old deep links regress.

GREEN:

- sandboxed immutable text-only rendering, validated output manifests, security
  headers, bounded byte-identical HTTPS/custom v2 handoff, install-return
  journey, and v1 compatibility pass.

REFACTOR:

- extract pure templates while preserving pinned gateway fixtures.

### Slice 6: Native plural-source UX

RED:

- zero results collapse with partial source failure;
- receipt/refusal/transport errors collapse;
- one-anchor outage blocks follow;
- clean install has no verified defaults or reset path;
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
  aggregate-metric, or rollback contracts fail under adversarial load.

GREEN:

- compiled ceilings, safe load shedding, deterministic eviction, privacy
  configuration, deployment packaging, and the pilot rehearsal pass.

REFACTOR:

- remove test-only duplication and update `SERVICE-INVENTORY.md`.

## Edge-Case Matrix

Tests explicitly cover:

- `now == expiry`;
- manifest and listing revision ties/equivocation;
- listing tombstone followed by stale replay;
- manifest upgrade during sync;
- duplicate/ambiguous anchor hints;
- loopback/private/DNS-rebinding hints and descriptor endpoint mismatch;
- work proof replay with another key or body;
- concurrent uploads for one site;
- crash before and after admission commit;
- eviction during a read snapshot;
- sync disconnect between commit and receipt;
- `GetOperation` after restart;
- directory feed fork/reorder and descriptor rollback;
- listing before hosting and hosting eviction after listing;
- gossip loops and final quiescence;
- unsupported `require:arti` ticket;
- all anchors unavailable;
- all embedded defaults disabled, removed, reset, or unreachable;
- one or two anchors unavailable;
- stale cached directory with partial source coverage;
- malformed Unicode/control characters in listings and search;
- hostile strings in every HTML context;
- cancellation before preview and before commit.

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
- exact listing authority and equivocation rules are enforced;
- clients discover through plural feeds and ordinary web pages;
- web/app handoff preserves site and entry destination across install;
- directory results and web pages carry the same root-signed public-site ticket
  core without giving anchors minting authority;
- clients persist a verified local copy and schedule ongoing reconciliation;
- mobile internet operations run through the cancellable Rust-owned
  iroh/Tokio/UniFFI boundary;
- anchors enforce manifest and Meadowcap authority before propagation;
- `sync/2` routes and paginates all composite namespaces without the 64-ID
  inventory ceiling;
- control operations are canonical, idempotent, recoverable, and typed;
- operation IDs, idempotency keys, and authenticated work proofs cannot be
  replayed across bodies;
- the anchor repository survives crash/restart and enforces logical/global
  quotas;
- open hosting cannot exceed compiled process ceilings or trigger unbounded
  gossip;
- authenticated directory and replica gossip has a complete bounded wire
  lifecycle and independently verified feed history;
- the text-only web renderer preserves the hostile-content boundary;
- deterministic three-anchor tests converge and survive anchor loss;
- opt-in aggregate metrics prove the public pilot meets the user-focused
  thresholds without correlated server logs;
- all quality and coverage gates pass.
