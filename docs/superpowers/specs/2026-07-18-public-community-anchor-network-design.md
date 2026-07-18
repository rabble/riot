# Public Community Anchor Network Design

Date: 2026-07-18
Status: Design review round 1 revised; pending round 2
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

## Round 1 Review Revisions

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

The anchor daemon is a new workspace binary crate, `riot-anchor`, depending on
`riot-core` and `riot-transport`. Server dependencies never flow into
`riot-core`, `riot-ffi`, or native shells.

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
- monotonically increasing listing revision;
- `listed: true | false`;
- title and summary;
- bounded topic tags and languages;
- optional coarse region;
- issued time and expiry.

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
- idempotency request ID;
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
feed coordinate, acceptance time, expiry, and request ID under:

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

Rotation uses a descriptor containing the predecessor key and two signatures:
one by the old key over the new descriptor digest and one by the new key over
the descriptor. Clients accept overlap only before the old descriptor expires
and persist the new key after verifying continuity. Emergency revocation
without the old key is an operational trust reset and is not hidden as normal
rotation.

## Transport Roles

Iroh packet relays and Riot anchors are distinct:

- an iroh relay forwards encrypted endpoint packets and assists NAT traversal;
- a Riot anchor is an application endpoint that validates and persists public
  site state.

The checked-in transport has:

- `bind` and `bind_seed` using `N0DisableRelay`;
- `bind_public` using the `N0` preset with relay plus Pkarr/DNS discovery.

The MVP anchor uses `bind_public` with a stable iroh endpoint key. The
descriptor publishes the resulting endpoint ID. Anchor operator identity
remains the separate signing key above.

## `riot/sync/2`: Routed Paginated Reconciliation

`riot/sync/1` remains supported for existing nearby and legacy tests. Anchors
do not silently downgrade to it.

New composite-site tickets bind:

- `min_sync_version: 2`;
- manifest digest/version;
- signed transport floor/epoch/expiry;
- site root and member namespace IDs.

The minimum sync version is inside the root-signed ticket payload. A client
refuses an anchor that cannot satisfy it.

### Inbound routing

The ALPN is `riot/sync/2`. Unlike `sync/1`, the responder does not construct a
namespace session before reading. It first reads a bounded `OpenNamespace`
frame:

```text
OpenNamespace {
  protocol_version,
  request_id,
  community_root,
  manifest_digest,
  namespace_id,
  direction
}
```

The responder verifies the active host/follow operation, routes to that exact
namespace, opens an immutable repository snapshot, and returns either
`SnapshotStart` or a structured refusal.

### Snapshot inventory

Each direction reconciles a stable, lexicographically sorted entry-ID snapshot:

```text
SnapshotStart { snapshot_digest, entry_count }
IdsPage {
  snapshot_digest,
  after_exclusive: optional EntryId,
  entry_ids: at most 256,
  done
}
NeedEntries { entry_ids: at most 64 }
Entries { canonical bundle: at most 64 entries and 8 MiB }
DirectionComplete { snapshot_digest }
SessionComplete
Refuse { code, subject, retryable, retry_after_seconds }
```

Rules:

- a page is strictly sorted and starts after its cursor;
- page overlap, duplicate IDs, cursor regression, or digest changes fail the
  session;
- each side uses one immutable snapshot per direction;
- writes arriving mid-session appear in the next session;
- entries still pass the existing preview/admission decoder;
- bundles retain the current 64-entry, 8 MiB, 1 MiB-per-item ceilings;
- state is committed only after admission;
- the second direction runs after the first, preserving bidirectional sync.

This is bounded paginated inventory exchange, not the final bandwidth-optimal
algorithm. A future Negentropy/range-summary version may reduce ID transfer
without changing anchor authority or storage semantics.

### Composite transaction

A hosting operation stages `O`, then `C`, then `W` under one request ID.
Nothing becomes directory-visible or mirror-visible until:

1. all three namespace sessions finish;
2. all entries pass admission;
3. the declared snapshot digests match staged state; and
4. one SQLite transaction promotes the complete staged site.

A failed operation deletes or expires its staged rows. Existing admitted site
state remains unchanged.

## `riot/anchor/1`: Control Plane

The control ALPN carries canonical CBOR frames no larger than 64 KiB. Every
request has a random 128-bit request ID. An anchor stores idempotent results for
24 hours; repeating the same request ID and body returns the same result, while
reusing an ID with a different body is rejected.

Operations:

### `Describe`

Returns the current signed `AnchorDescriptorV1` and limit profile.

### `PrepareHost`

Input:

- root-signed composite ticket;
- desired namespace snapshot digests;
- optional valid admission work stamp.

Output:

- ordered namespace host plan;
- current retained snapshot digests;
- sync version and limits;
- operation expiry.

The client then opens `sync/2` for each required namespace.

### `CommitHost`

Input:

- request ID;
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

### `GetOperation`

Returns an idempotently retained result for a request ID, allowing recovery
after a disconnect between commit and receipt delivery.

### Refusals

```text
ControlRefusal {
  code,
  subject: ticket | manifest | listing | namespace | capacity | version,
  retryable,
  retry_after_seconds: optional
}
```

Codes include `invalid_authority`, `unsupported_version`, `over_quota`,
`expired`, `not_hosted`, `equivocation`, `work_required`, and `busy`.
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
  source_coverage: Complete | Partial { succeeded, failed },
  next_cursor
}

FollowResult {
  site_root,
  local_state: Saved | AlreadyFollowed | Cancelled,
  anchor_attempts,
  destination_entry: optional full EntryId
}
```

`expired` always names the expired subject. `over_quota` always names the
anchor and quota class. A verified receipt, protocol refusal, and local network
failure can never collapse into the same enum case.

Pure injectable ports mirror existing repository patterns:

- `AnchorDirectoryPort`;
- `AnchorHostingPort`;
- `AnchorSyncPort`;
- `Clock`;
- `RetryScheduler`.

## Anchor Repository

`riot-anchor` owns a new SQLite-backed `AnchorRepository`. It reuses canonical
Riot codecs and admission functions but does not extend the capped
`EvidenceRepository` or demo `SiteState`.

Logical tables:

- `communities`;
- `manifests` and durable version/digest floors;
- `namespaces`;
- `entries`;
- `payloads`;
- `community_payload_refs`;
- `listings` and listing conflict floors;
- `directory_inclusions`;
- `hosting_receipts`;
- `staged_operations`;
- `idempotency_results`;
- `anchor_peers`;
- `operator_state`.

### Transactions

- SQLite WAL mode and foreign keys are mandatory.
- Staging, full admission, promotion, search-index visibility, and receipt
  creation share one transaction boundary at commit.
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
| Staged bytes | 256 MiB | 1 GiB |
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
| Concurrent gossip sessions per peer | 2 | 4 |
| Gossip transfer per peer per hour | 256 MiB | 1 GiB |

The anchor also bounds:

- request headers and control bodies;
- queued signature/capability verification jobs;
- cumulative verification CPU time per source and globally;
- response bytes;
- retry count and exponential backoff;
- directory-feed bytes and records;
- cache lifetime to signed expiry.

### Admission work stamp

Global ceilings protect the process but do not make anonymous admission fair.
For a previously unseen root or a new listing, an anchor uses a stateless work
challenge:

```text
BLAKE3(
  "riot/anchor-work/v1" ||
  anchor_id || operation || community_root ||
  random_challenge || expires || counter
)
```

The digest must have the descriptor-advertised number of leading zero bits.
Difficulty is `0..24`, the challenge expires after five minutes, and the work
stamp is bound to one anchor and operation. Difficulty zero means no work.

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
- the relationship names directory-only or explicit site replication rules;
- per-peer work and byte budgets permit it.

Clients achieve baseline redundancy by hosting independently on several
anchors. Gossip improves reach but is not required.

## Directory Feeds and Search

Each anchor owns a signed public directory feed containing exact accepted
listing envelopes plus inclusion metadata. Other anchors can reconcile feeds
under operator-configured directory peering.

The HTTPS API:

```text
GET /.well-known/riot-anchor.json
GET /api/v1/directory?q=&cursor=&limit=
GET /api/v1/directory/<full-community-root>
GET /c/<full-community-root>/
GET /c/<full-community-root>/e/<full-entry-id>
```

Search responses are lossless JSON projections containing:

- the canonical listing envelope for client verification;
- directory source anchor IDs;
- source coverage (`complete` or `partial`);
- a stable opaque cursor;
- no claim of endorsement.

Inputs are normalized and bounded. Database queries are parameterized. Cursors
bind the normalized query and snapshot generation so they cannot be reused
across queries.

Open hosting does not imply automatic listing. A valid listing submitted under
budget appears in that anchor's raw directory feed. The default Explore view
merges configured feeds, deduplicates one visible record per root, excludes
expired/conflicted/unlisted records, and exposes topic/language/region filters.
It does not claim Sybil-proof ranking.

## Safe Web Projection

The existing fixed gateway remains intact. The anchor adds a new isolated
snapshot pipeline:

1. Rust reads one admitted SQLite snapshot.
2. Rust emits `AnchorWebSnapshotV1`, containing only typed, bounded public text
   records and canonical full identifiers.
3. A renderer worker with no database credentials and no network access reads
   the snapshot and writes a complete static tree.
4. The anchor validates the output manifest and atomically swaps the served
   directory.

The Rust daemon and renderer worker ship as one deployable container, but the
worker is a separate process and trust boundary. It may reuse extracted pure
templates from `apps/gateway`; it does not weaken the existing pinned-fixture
constructor or tests.

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

Riot-native sync still carries admitted payloads. The web MVP does not attempt
to serve arbitrary media safely.

Projection cache lifetime never exceeds listing or record expiry. Admission,
eviction, unlisting, or manifest change invalidates the affected generation.

## Canonical Web-to-App Handoff

Anchor-generated pages use a versioned composite handoff envelope:

```text
riot://site/v2/<canonical-base64url-envelope>
```

The canonical envelope has two layers. Its `signed_core` is the root-signed
ticket and includes:

- full site root and `O`, `C`, `W` IDs;
- manifest digest/version;
- minimum sync version;
- transport floor/epoch/expiry;
- optional destination full entry ID.

Its `anchor_hints` field is unsigned, replaceable routing metadata outside the
signed core. A client verifies the signed core first, caps the envelope at
eight hints of 512 bytes each, and treats hints only as candidate routes. A
hint can never alter the site, namespaces, manifest, transport floor, expiry,
or destination entry. Reissuing the same signed core with different hints does
not change community identity or authority.

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
ticket and destination remain in the page URL/QR, so returning preserves the
journey.

### Install unavailable

The visitor can continue reading the safe web projection and copy/save the
normal HTTPS link or QR. The page does not claim offline availability.

### Expired, malformed, or failed import

The page stays readable. Riot identifies whether the ticket, manifest,
capability, or anchor attempt failed; preserves the web URL; and offers retry
with configured anchors, rescan/copy of a fresh ticket, or cancel. It never
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
- retryable expired ticket with preserved web destination.

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
- cap hints at eight and each hint at 512 bytes;
- allow only defined iroh and HTTPS hint schemes;
- verify the signed transport floor before every native dial;
- never cause an anchor to fetch a client-supplied web URL or private-network
  address.

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
- at least two infrastructure failure domains or regions;
- seven consecutive pilot days.

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

## Deterministic Test Harness

New support lives under `crates/riot-anchor/tests/support/` and uses production
interfaces:

- `TestAnchor` with isolated SQLite database and keys;
- `TestClient` using the public control/sync protocol types;
- `TestAnchorNetwork` with deterministic partitions and ordered schedules;
- `FakeClock`;
- in-memory duplex `AnchorTransport`;
- `DeterministicGossipScheduler`;
- `FailpointRepository` that can fail before/after SQLite transaction stages;
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
- duplicate ticket fields and v1/v2 downgrade are not rejected.

GREEN:

- canonical records, reserved listing admission, floors, tombstones, v2 ticket,
  and parser bounds pass focused tests.

REFACTOR:

- share canonical decoders and authority predicates without changing existing
  v1 behavior.

### Slice 2: `sync/2`

RED:

- responder cannot route a namespace before constructing a session;
- 257+ IDs cannot reconcile in bounded pages;
- cursor overlap/digest change is not rejected.

GREEN:

- immutable paginated bidirectional reconciliation converges over in-memory
  duplex for `O`, `C`, and `W`.

REFACTOR:

- isolate common bundle admission shared with `sync/1`.

### Slice 3: Anchor control and repository

RED:

- crash between staging and commit leaks partial visibility;
- repeated request IDs duplicate work or receipts;
- logical quota is bypassed by cross-community dedup.

GREEN:

- Prepare/Sync/Commit, SQLite atomic promotion, receipt recovery, quotas,
  reference accounting, and restart pass.

REFACTOR:

- separate protocol types from daemon adapters.

### Slice 4: Plural directory and bounded gossip

RED:

- hosting implies listing;
- conflicting listing revisions produce divergent search;
- client-triggered requests create background fanout.

GREEN:

- signed feeds, listing/unlisting, typed coverage, configured gossip, work
  challenge, and three-anchor quiescence pass.

REFACTOR:

- extract deterministic scheduler policies.

### Slice 5: Static web projection and handoff

RED:

- hostile HTML/SVG/attribute strings execute or escape context;
- app-absent journey loses the ticket/destination;
- old deep links regress.

GREEN:

- immutable text-only rendering, security headers, v2 handoff, install-return
  journey, and v1 compatibility pass.

REFACTOR:

- extract pure templates while preserving pinned gateway fixtures.

### Slice 6: Native plural-source UX

RED:

- zero results collapse with partial source failure;
- receipt/refusal/transport errors collapse;
- one-anchor outage blocks follow.

GREEN:

- injected ports drive every Explore, Follow, Publish, refresh, cancel, and
  recovery state on iOS and Android.

REFACTOR:

- share Rust result semantics through UniFFI while preserving native state
  presentation.

### Slice 7: Operational closure

RED:

- global capacity, verification queue, logging, eviction, or restart contracts
  fail under adversarial load.

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
- concurrent uploads for one site;
- crash before and after admission commit;
- eviction during a read snapshot;
- sync disconnect between commit and receipt;
- `GetOperation` after restart;
- gossip loops and final quiescence;
- all anchors unavailable;
- one or two anchors unavailable;
- stale cached directory with partial source coverage;
- malformed Unicode/control characters in listings and search;
- hostile strings in every HTML context;
- cancellation before preview and before commit.

## Quality Gates

Before completion:

- `cargo test --workspace --all-features`;
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-features -- -D warnings`;
- gateway Python tests;
- blocking local iOS and Android tests plus regenerated UniFFI bindings until
  native CI exists;
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
- Tor/Arti implementation beyond preserving the signed transport floor.
- Bandwidth-optimal range reconciliation in the first `sync/2`.

## Definition of Done

The design is implemented when:

- composite public sites host on several independent anchors without accounts;
- hosting never implies listing;
- exact listing authority and equivocation rules are enforced;
- clients discover through plural feeds and ordinary web pages;
- web/app handoff preserves site and entry destination across install;
- clients persist a verified local copy and schedule ongoing reconciliation;
- anchors enforce manifest and Meadowcap authority before propagation;
- `sync/2` routes and paginates all composite namespaces without the 64-ID
  inventory ceiling;
- control operations are canonical, idempotent, recoverable, and typed;
- the anchor repository survives crash/restart and enforces logical/global
  quotas;
- open hosting cannot exceed compiled process ceilings or trigger unbounded
  gossip;
- the text-only web renderer preserves the hostile-content boundary;
- deterministic three-anchor tests converge and survive anchor loss;
- the public pilot meets the user-focused thresholds;
- all quality and coverage gates pass.
