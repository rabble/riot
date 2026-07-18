# Public Community Anchor Network Design

Date: 2026-07-18
Status: Design review rounds 1-12 revised; pending round 13
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

Round five pinned complete descriptor verification floors, distinguished
destination commit from organizer receipt recovery, made checkpoint snapshots
current-key verifiable, sized exclusive emergency metadata/WAL reserves, added
site-generation compare-and-swap, specified the actual FFI-to-core database
ownership migration, closed local follow/configuration failure states, and made
pilot enrollment/export distinct, private, and replay-safe.

Round six bounded anonymous HTTPS work independently from control traffic,
added a single-use privacy-preserving pilot invitation contract, aligned the
native storage refactor with the real `LocalProfile`/`EvidenceStore` ownership,
and replaced historical-receipt verification during replication with a
current-key, connection-bound source attestation.

Round seven defined every cross-implementation digest, extended ingress bounds
through TCP/TLS/HTTP connection setup, made listing/removal idempotency and
emergency recovery atomic, bound replica preparation to a one-use destination
challenge and one immutable source snapshot, chose deterministic local mutation
contention and explicit runtime shutdown, and completed overload, enrollment,
export, withdrawal, and stale-source user lifecycles.

Round eight completed the remaining preimages, persisted checkpoint work before
filesystem publication, reserved removal capacity when listing becomes visible,
bounded the removal and HTTP parser queues, separated the process network
runtime from revocable profile/storage leases, terminalized prepared replicas
on peer-context loss, and made relisting, close recovery, source change, and all
pilot outcomes explicit.

Round nine made the CBOR grammar self-discriminating without undocumented
numeric tags, retained request digests across reserved owner-removal replay,
defined crash-orphan replica invalidation, corrected profile-scoped network
shutdown, fixed daemon feature ownership, and made multi-anchor publish,
relisting, and pilot cancellation/recovery states durably executable.

Round ten completed the remaining wire surface: named peer-transcript and proof
preimages, a canonical admission work stamp, exact success/refusal response
envelopes for every control operation, stable limit identifiers, pilot HMAC-key
retention, and reservation release for automatic listing lifecycle changes.

Round eleven closed structured response recovery by giving every refusal code
one canonical details payload and making `GetOperation` embed the original
operation kind and exact prepared payload or terminal outcome without
alternate nesting.

Round twelve fixed empty-feed and first-checkpoint sentinels, closed every
`sync/2` refusal variant, and specified the observable Prepare/Commit/restart/
expiry/unknown-operation lifecycle for `GetOperation` and checkpoint recovery.

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
`riot-anchor-protocol`, `riot-core` with `default-features = false`, and
`riot-transport`. It owns its server SQLite/repository dependencies directly;
the server never enables `riot-core/sqlite`.

Native internet operations use a new mobile-compatible `riot-client-net` crate
behind one process-singleton `RiotApplicationRuntime`, which owns one iroh
endpoint and Tokio runtime per application process. It
depends on `riot-anchor-protocol`, `riot-transport`, and a direct
no-default-features `riot-core` dependency for `SiteReplicaRepository`;
`riot-ffi` depends on `riot-client-net` and exposes its cancellable async
operations and typed event streams through UniFFI. No dependency edge runs
from `riot-core` or `riot-anchor-protocol` back to either adapter. Existing
native nearby carriers remain unchanged. The existing client-side `rusqlite`
feature remains in `riot-core`; anchor-server SQLite, HTTP-server, and renderer
dependencies never enter `riot-core`, `riot-anchor-protocol`,
`riot-client-net`, `riot-ffi`, or native shells. A feature-closure CI test
inspects every native target's resolved Cargo graph and fails if an HTTP server,
renderer, or `riot-anchor` daemon dependency appears.

`riot-transport` also changes its `riot-core` edge to
`default-features = false`; the native top-level `riot-ffi` build deliberately
uses `riot-core = { default-features = false, features = ["sqlite"] }` rather
than receiving SQLite through feature unification.

The native shell starts `RiotApplicationRuntime` once at process startup and
closes it only after every profile lease is closed at orderly process teardown.
Protected profile unlock obtains a revocable profile network lease; profile
lock releases that lease without closing the process endpoint. Each FFI
operation owns a
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

Title is at most 120 UTF-8 bytes, summary 512 bytes, topic tags eight entries
of 32 bytes, languages eight canonical BCP-47 tags of 35 bytes, and coarse
region 16 bytes. Unknown tags/languages are data, not new schema fields.

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

Clients persist the highest verified per-root transport epoch and reject older
tickets thereafter. Ticket expiry is capped at 90 days. A future move from the
public-only `require:none` profile to a confidential transport requires a new
reviewed bootstrap/revocation design; this MVP does not claim that replayed
pre-upgrade links hide metadata from a brand-new client.

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

All protocol records use deterministic positional CBOR arrays, definite-length
containers, minimal integer encodings, and sorted collections. There are no
implicit integer field keys or enum discriminants:

- a product type written `NameVn { a, b, c }` encodes exactly as
  `[n, a, b, c]`; a protocol-version-scoped `Name { a, b, c }` encodes as
  `[a, b, c]`, always in the displayed field order;
- an operation input/output list encodes by the same rule under its named
  versioned type;
- an optional field is `null` or its one typed value and never changes array
  length;
- a closed enum value is its exact lowercase ASCII `snake_case` name;
- a sum variant encodes as `[variant_name, ...variant_fields]`;
- a field whose schema name ends in `_bytes` is a CBOR byte string containing
  separately canonical bytes; every other nested typed field is embedded as a
  CBOR value, not double-encoded;
- unordered sets encode as arrays sorted by complete canonical element bytes;
  ordered lists retain their specified semantic order.

Every wire type in this document follows those rules unless an adjacent formula
gives an even more specific byte layout. Decoders reject unknown versions,
unknown enum/sum names, duplicate set elements, non-canonical encodings, and
trailing bytes. This grammar deliberately uses textual closed discriminants so
no implementation can invent numeric tags for control operations, sync frames,
states, reasons, phases, refusals, or modes.

Every anchor-owned signed body other than a descriptor binds the stable
`AnchorId`, current operator key ID, descriptor epoch, and descriptor digest.
Verification therefore identifies the exact still-chain-valid signing context
instead of treating every historical key as indefinitely current.

### Protocol identity digests

Every digest used as a signature coordinate, continuity link, cursor, receipt
field, or replay binding is defined either by the table below or by an adjacent
specialized formula such as the sync snapshot and directory-state digests. The
table uses this exact construction:

```text
digest_v1(label, canonical_bytes) =
  BLAKE3(
    u16be(byte_length(label)) ||
    label ||
    u64be(byte_length(canonical_bytes)) ||
    canonical_bytes
  )
```

Labels are the exact ASCII bytes below. “Envelope” means the complete canonical
signed envelope including its signature fields; “body” excludes the containing
signature:

```text
OperatorSignedEnvelopeV1<T> =
  canonical_cbor([1, T, exactly_64_signature_bytes])

AdmittedListingEnvelopeV1 =
  canonical_cbor([
    1,
    canonical_signed_listing_entry_bytes,
    canonical_capability_chain_bytes,
    null | canonical_delegate_grant_bytes
  ])

RootSignedTicketCoreEnvelopeV2 =
  canonical_cbor([2, PublicSiteTicketV2Core, exactly_64_root_signature_bytes])

ControlDigestBodyV1 =
  canonical_cbor([1, operation_kind, semantic_body])
```

`operation_kind` is exactly one of `describe`, `get_work_challenge`,
`prepare_host`, `commit_host`, `submit_listing`, `prepare_replica`,
`pull_directory_feed`, `pull_directory_snapshot`, or `get_operation`.
`ControlRequestV1` is `[1, operation_kind, idempotency_key, semantic_body]`;
`describe` uses `[1]` as its semantic body. Challenge retrieval uses
`[1, intended_operation_kind, intended_idempotency_key, community_root,
work_target_digest]` and does not claim a durable row. The remaining semantic
bodies are:

| Operation | Exact `semantic_body` |
| --- | --- |
| `prepare_host` | `[1, root_signed_ticket_core, ordered_namespace_snapshot_digests, optional_work_stamp]` |
| `commit_host` | `[1, operation_id, ordered_namespace_snapshot_digests]` |
| `submit_listing` | `[1, admitted_listing_envelope_bytes, optional_work_stamp]` |
| `prepare_replica` | `[1, replica_prepare_challenge, replica_source_attestation, root_signed_ticket_core, ordered_namespace_snapshot_digests]` |
| `pull_directory_feed` | `[1, after_sequence, limit]` |
| `pull_directory_snapshot` | `[1, checkpoint_digest, optional_snapshot_cursor]` |
| `get_operation` | `[1, operation_id]` |

Every bounded control reply is exactly
`ControlResponseV1 = [1, operation_kind, outcome]`, where `outcome` is either
`["success", success_payload]` or `["refused", ControlRefusal]`. There is no
third envelope shape; transport loss occurs outside a decoded response. Exact
success payloads are:

| Operation | Exact `success_payload` |
| --- | --- |
| `describe` | `[1, descriptor_envelope, anchor_limit_profile]` |
| `get_work_challenge` | `[1, work_challenge_envelope]` |
| `prepare_host` | `[1, operation_id, base_site_generation, ordered_namespace_host_plan, ordered_namespace_tokens, ordered_retained_snapshot_digests, sync_version, effective_operation_limits, operation_expiry]` |
| `commit_host` | `[1, hosting_receipt]` |
| `submit_listing` | `[1, listing_receipt]` |
| `prepare_replica` | `[1, operation_id, base_site_generation, ordered_namespace_host_plan, ordered_namespace_tokens, ordered_retained_snapshot_digests, sync_version, effective_operation_limits, operation_expiry]` |
| `pull_directory_feed` | `[1, ["page", inclusions, floor_sequence, head_sequence, head_digest, done]]` or `[1, ["checkpoint_required", checkpoint, snapshot_cursor]]` |
| `pull_directory_snapshot` | `[1, checkpoint, optional_snapshot_record, optional_next_cursor, done]` |
| `get_operation` | `[1, operation_id, originating_prepare_kind, ["prepared", operation_expiry, prepare_success_payload] \| ["terminal", terminal_operation_outcome]]` |

`effective_operation_limits` is a sorted array of `[limit_id,
effective_value]` drawn byte-identically from the described limit profile. It
contains all 69 IDs exactly once in strictly ascending order.
`ordered_namespace_host_plan`, tokens, retained digests, and feed inclusions
use their already specified semantic order. A retained idempotent result stores
and replays the complete canonical `ControlResponseV1`, not an
implementation-private projection.

For `get_operation`, `originating_prepare_kind` is exactly `prepare_host` or
`prepare_replica`. `prepare_success_payload` is the embedded canonical success
payload originally returned by that Prepare—not a CBOR byte string and not a
nested `ControlResponseV1`. `terminal_operation_outcome` is exactly one of
`["committed", hosting_receipt]` or `["refused", ControlRefusal]`; it is an
operation-lifecycle outcome, not a nested Commit response. The operation row
persists its originating kind plus the canonical prepared payload or terminal
outcome; replay constructs only the one wrapper shown above. Alternate
full-response nesting, Commit-outcome nesting, outcome omission, or byte-string
wrapping is rejected.

The observable lifecycle is fixed:

| Event | Operation row / `GetOperation` result |
| --- | --- |
| Prepare key claimed before operation creation | no public operation ID; `GetOperation` is impossible |
| Prepare transaction commits and returns an ID | `prepared` with the byte-identical embedded Prepare success payload |
| namespace sync is in progress or complete but Commit has not committed | unchanged `prepared` |
| Commit key is claimed but its transaction has not committed | unchanged `prepared`; Commit exact replay resolves its own idempotency row |
| Commit transaction succeeds | atomically `terminal ["committed", hosting_receipt]` |
| Commit or peer/session recovery terminally refuses the operation | atomically `terminal ["refused", ControlRefusal]` |
| process restarts | recovered persisted `prepared`, except peer-bound replicas become terminal `peer_context_changed` before readiness; terminal remains byte-identical |
| operation deadline passes before commit | terminal `operation_expired`; staging and tokens are invalidated in the same transaction |
| retained terminal/result window later expires | operation row is deleted; query returns `operation_not_found` |
| unknown random operation ID | `operation_not_found`, with no additional existence metadata |

Checkpoint/snapshot pull with an unknown, reclaimed, missing, or digest-invalid
checkpoint returns `checkpoint_unavailable` with the exact reason above. It
never returns an empty success page.

Namespace tuples always appear in `O`, `C`, `W` order. Every `operation_id`,
idempotency key, root, namespace ID, digest, nonce, key, signature, cursor, and
canonical `*_bytes` field is a CBOR byte string with the exact byte limit stated
elsewhere; times are unsigned Unix seconds. The protocol crate checks in a
machine-readable CDDL transcription and golden vectors generated from these
normative arrays. CDDL and code must agree with this document; neither may add
a tag, field, alternate map form, or numeric discriminant.

`AnchorLimitProfileV1` is exactly
`[1, profile_epoch, [[limit_id, effective_value, absolute_value]...]]`.
Entries contain every ID below exactly once in ascending order. A scalar value
is `u64`; a slash-compound value is `[first_u64, second_u64]`. Bytes are
expressed in bytes, durations in milliseconds, counts/rates in their printed
table unit, and CPU percentages in basis points. `fixed` and `unchanged`
resolve to the same numeric effective/absolute value; formulas such as `2 * L`
are resolved before signing. An operator may only lower an effective value;
changing any value increments `profile_epoch`.

```text
 1 logical_retained_bytes_whole_anchor
 2 physical_retained_bytes
 3 ordinary_sqlite_database_including_wal
 4 non_payload_metadata_bytes
 5 sqlite_wal_bytes
 6 emergency_removal_metadata_reserve
 7 emergency_removal_wal_fsync_reserve
 8 staged_bytes
 9 live_staged_operations
10 idempotency_rows
11 idempotency_rows_per_source_per_24h
12 reserved_removal_idempotency_result_rows
13 incident_conflict_records
14 conflict_proofs_per_site_subject
15 hosted_sites
16 logical_bytes_per_site
17 live_entries_per_namespace
18 item_payload
19 bundle
20 concurrent_sync_control_sessions
21 sessions_per_source
22 sessions_per_site
23 tcp_listen_backlog
24 accepted_public_https_sockets
25 pending_tls_handshakes
26 tls_handshakes_per_source_per_minute
27 tls_handshakes_globally_per_second
28 tls_clienthello_total_handshake_bytes
29 tls_handshake_cpu_wall_time
30 active_public_https_connections
31 http_requests_per_keep_alive_connection
32 http_idle_absolute_connection_lifetime
33 http_decoded_header_fields_one_field_line
34 concurrent_public_https_handlers
35 queued_public_https_handlers
36 public_https_requests_per_source_per_minute
37 public_https_requests_globally_per_second
38 concurrent_public_http_database_snapshots
39 public_http_database_snapshots_per_source
40 public_http_query_cpu_wall_time
41 public_api_response_bytes
42 one_static_web_response
43 search_results_per_page
44 search_query_utf8_bytes
45 directory_listings
46 directory_feed_records
47 verification_queue_jobs
48 verification_cpu_per_request
49 aggregate_outstanding_verification_cpu_budget
50 reserved_owner_removal_verification_permits
51 queued_reserved_removal_jobs
52 queued_reserved_removal_canonical_bytes
53 reserved_valid_removal_database_writer_permits
54 emergency_checkpoint_worker
55 owner_removal_attempts_per_source_per_minute
56 owner_removal_attempts_globally_per_second
57 work_challenge_signatures_per_second
58 work_challenges_per_source_per_minute
59 static_projection_bytes
60 renderer_temporary_filesystem
61 renderer_temporary_files_inodes
62 concurrent_renderer_jobs
63 renderer_cpu_wall_time_per_generation
64 published_generations_per_site
65 local_operational_log_bytes_all_classes
66 diagnostic_log_bytes
67 rotated_local_log_files
68 concurrent_gossip_sessions_per_peer
69 gossip_transfer_per_peer_per_hour
```

Every operator-signed row below uses `OperatorSignedEnvelopeV1`; its signature
preimage remains that record type's stated domain plus canonical body bytes.
Canonical component byte strings must themselves pass their normal decoder and
byte-for-byte canonical re-encoding check before hashing.

| Name | Label | Canonical bytes hashed |
| --- | --- | --- |
| `descriptor_digest` | `riot/anchor-descriptor-envelope/v1` | complete `DescriptorEnvelopeV1` |
| `limit_profile_digest` | `riot/anchor-limit-profile/v1` | complete canonical `AnchorLimitProfileV1` body |
| `inclusion_digest` | `riot/directory-inclusion-envelope/v1` | `OperatorSignedEnvelopeV1<DirectoryInclusionBodyV1>` |
| `checkpoint_digest` | `riot/directory-checkpoint-envelope/v1` | `OperatorSignedEnvelopeV1<DirectoryCheckpointBodyV1>` |
| `snapshot_record_digest` | `riot/directory-snapshot-envelope/v1` | `OperatorSignedEnvelopeV1<DirectorySnapshotRecordBodyV1>` |
| `listing_digest` | `riot/admitted-listing-envelope/v1` | `AdmittedListingEnvelopeV1` |
| `root_signed_ticket_core_digest` | `riot/public-site-ticket-signed-core/v2` | `RootSignedTicketCoreEnvelopeV2`, excluding replaceable hints |
| `work_challenge_digest` | `riot/anchor-work-challenge-envelope/v1` | `OperatorSignedEnvelopeV1<WorkChallengeBodyV1>` |
| `control_request_digest` | `riot/anchor-control-request-body/v1` | `ControlDigestBodyV1` with every semantic field including `work_stamp`, excluding only outer idempotency key and transport/framing metadata |
| `work_target_digest` | `riot/anchor-work-target/v1` | `ControlDigestBodyV1` with the fixed optional `work_stamp` slot set to `null` and the same idempotency/framing exclusions |
| `page_digest` | `riot/sync-ids-page/v2` | complete canonical `IdsPage` frame |
| `peer_transcript_digest` | `riot/anchor-peer-transcript/v1` | complete canonical `PeerTranscriptV1` array |
| `replica_source_attestation_digest` | `riot/replica-source-attestation-envelope/v1` | `OperatorSignedEnvelopeV1<ReplicaSourceAttestationBodyV1>` |

An admitted manifest continues to use the existing manifest-version canonical
digest defined by `riot-core`; anchor records name both that digest and version.
`AnchorLimitProfileV1` is the canonical sorted array of every effective compiled or
operator-lowered limit in the resource table; `Describe` returns those exact
bytes and clients reject a descriptor/receipt whose digest differs.
No implementation may substitute a body digest for an envelope digest or hash
JSON/projection bytes. Work proofs use `work_challenge_digest` exactly.

Checked-in golden vectors contain, for every row, the exact canonical CBOR hex,
label bytes, length prefixes, expected digest, and one mutation that must
change it. Vectors cover minimum and maximum legal records, descriptor
transition linking, and cross-language Rust/Swift/Kotlin/TypeScript decoding;
`cargo xtask validate-contracts` rejects drift.

### `AnchorDescriptorV1`

Anchor operator signatures use Ed25519. The canonical verification key is:

```text
OperatorVerificationKeyV1 {
  algorithm: Ed25519,
  public_key: exactly 32 bytes
}

operator_key_id = BLAKE3(
  "riot/anchor-operator-key-id/v1" ||
  canonical_cbor(verification_key)
)
```

Every occurrence of `operator_key_id` must match the complete verification key
carried by its descriptor or pinned floor.

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

```text
AnchorDescriptorBodyV1 {
  anchor_id,
  genesis_operator_public_key,
  genesis_random_256_bits,
  current_operator_verification_key,
  current_operator_key_id,
  descriptor_epoch,
  previous_descriptor_digest: optional,
  current_iroh_endpoint_id,
  https_origin,
  operator_display_label,
  self_reported_failure_domain_label,
  supported_control_versions,
  supported_sync_versions,
  enabled_roles,
  limit_profile_digest,
  predecessor_operator_verification_key: optional,
  issued_at,
  expires_at
}
```

Epoch zero requires both predecessor optionals to be `null`; every greater
epoch requires both values, including same-key endpoint/origin/configuration
updates.

### `HostingReceiptV1`

The operator signs:

```text
"riot/hosting-receipt/v1" || canonical_cbor(receipt_body)
```

```text
HostingReceiptBodyV1 {
  anchor_id,
  operator_key_id,
  descriptor_epoch,
  descriptor_digest,
  hosting_operation_id,
  full_site_root,
  manifest_digest,
  manifest_version,
  base_site_generation,
  committed_site_generation,
  ordered_namespace_results,
  status,
  accepted_at,
  reported_retention_through,
  limit_profile_digest
}
```

Each namespace result is
`[namespace_id, snapshot_digest, entry_count]` in `O`, `C`, `W` order.

`reported_retention_through` is the anchor's signed operational claim, not a
cryptographic guarantee. A dishonest or failed anchor may break it.

### `ListingReceiptV1`

The canonical body is:

```text
ListingReceiptBodyV1 {
  anchor_id,
  operator_key_id,
  descriptor_epoch,
  descriptor_digest,
  listing_digest,
  full_site_root,
  accepted_listing_epoch,
  accepted_listing_revision,
  feed_coordinate,
  accepted_at,
  expires_at,
  request_idempotency_key
}
```

Its exact signature is:

```text
signature = Ed25519.Sign(
  operator_key,
  "riot/listing-receipt/v1" || canonical_cbor(receipt_body)
)

ListingReceiptV1 =
  OperatorSignedEnvelopeV1<ListingReceiptBodyV1>
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
stores a complete authenticated floor
`(AnchorId, epoch, descriptor_digest, OperatorVerificationKeyV1)` or the full
floor envelope. It fetches bounded pages of at most 16 descriptors and verifies
the first successor's previous digest and predecessor signature with that
pinned floor key, then every epoch increment, old-key signature, new-key
signature, stable `AnchorId`, and historical time overlap. It persists the
newest complete floor. The operator must retain this
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
it pins the complete descriptor floor tuple above, HTTPS origin, and roles. App
releases, not a live canonical service, update this fallback set.

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
Refuse { code, retryable, retry_after_seconds: optional, details }
```

`Refuse.code` is a closed lowercase textual enum, and `details` must match
exactly:

| Sync refusal code | Exact `details` |
| --- | --- |
| `unsupported_version` | `["version", supported_versions]` |
| `invalid_ticket` | `["ticket", reason]`, where reason is `signature \| root \| structure` |
| `expired_ticket` | `["ticket_expiry", expires_at, observed_at]` |
| `transport_mismatch` | `["transport", required_mode, observed_mode]` |
| `namespace_not_member` | `["namespace", namespace_id]` |
| `manifest_mismatch` | `["manifest", expected_digest, observed_digest]` |
| `invalid_mode` | `["mode", observed_mode]` |
| `operation_not_found` | `["operation", operation_id]` |
| `invalid_namespace_token` | `["namespace_token", namespace_id]` |
| `operation_expired` | `["operation_expiry", operation_id, expires_at, observed_at]` |
| `unexpected_frame` | `["frame", phase, expected_frame_names, observed_frame_name]` |
| `cursor_regression` | `["cursor", after_exclusive, observed_first_id]` |
| `page_mismatch` | `["page", expected_page_digest, observed_page_digest]` |
| `snapshot_mismatch` | `["snapshot", expected_snapshot_digest, observed_snapshot_digest]` |
| `request_mismatch` | `["request", request_id]` |
| `chunk_mismatch` | `["chunk", request_id, expected_index, observed_index]` |
| `frame_oversize` | `["encoded_size", observed_bytes, maximum_bytes]` |
| `admission_failed` | `["admission", subject]`, where subject is `authority \| bundle \| entry` |
| `quota_exceeded` | `["quota", limit_id, effective_value, observed_value]` |
| `busy` | `["capacity", limit_id]` |
| `peer_context_changed` | `["peer_context", side, prior_descriptor_digest, optional_latest_descriptor_digest, reason]` |

The transport and peer-context nested enums are the same closed values as
`ControlRefusal`; `phase`, frame names, and mode are the exact closed textual
discriminants declared by the `sync/2` FSM. Only `busy` is retryable and
requires `retry_after_seconds`; every structural, authority, cursor, digest,
token, expiry, or peer-context refusal is terminal for that session. Unknown
code/detail pairings close the session without applying staged state.

`page_digest` uses the protocol identity table above. For each direction the
inventory sender and receiver follow this exact FSM:

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
After all `O/C/W` stages complete, the destination anchor may commit during
`CommitHost`; that transaction also stores its signed receipt. The organizer's
local state remains private until it receives that receipt or recovers it
through `GetOperation`, then promotes its local stage in one transaction. A
replica source never has a mutable stage.

A disconnect before destination commit leaves every participant unchanged. A
disconnect after destination commit but before receipt delivery legitimately
leaves the destination committed and the organizer staged; `GetOperation`
resolves that state without rolling the destination back. A terminal refusal
or expired operation discards organizer staging.

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

Each site has a monotonically increasing committed `site_generation`.
`PrepareHost` and `PrepareReplica` capture `base_generation` and its three
snapshot digests. `CommitHost` serializes on the site row and performs a
compare-and-swap:

- if current generation equals the operation base, it promotes the staged
  union, increments generation once, and creates a receipt binding
  `base_generation`, `committed_generation`, and final snapshot digests;
- otherwise it makes no visible change and returns retryable
  `stale_base { current_generation, current_snapshot_digests }`.

Two concurrent operations from the same base therefore have one deterministic
transaction winner; the other cannot overwrite it. On `stale_base`, organizer
staging remains private only long enough to inspect the refusal, then is
discarded or used as local input to a newly prepared reconciliation. No stale
operation receives a receipt.

A new public Follow mirrors this shape locally: `O` is read and the manifest
admitted before exact `C` and `W` routing; all three committed anchor snapshots
land in local follow staging; one local transaction installs the followed site
and ongoing-sync schedule. Cancellation or failure before that transaction
leaves no durable follow mutation.

## `riot/anchor/1`: Control Plane

The control ALPN carries canonical CBOR frames no larger than 64 KiB. Every
request has a random 128-bit `idempotency_key`. The anchor stores
`control_request_digest` and an idempotency state:

```text
Claimed |
Prepared { operation_id, operation_expiry, canonical_prepare_response } |
Terminal { result }
```

The fixed-capacity global `IdempotencyKeyIndex` has one SQLite uniqueness
constraint across ordinary and reserved classes. An ordinary winning claim
points at its ordinary state row; `PrepareHost` or `PrepareReplica` creates the
operation and changes that row to `Prepared` in the same transaction.
Concurrent exact calls compare the retained request digest and replay it, while
another body is rejected without result disclosure. Namespace tokens are
deterministically derived as:

```text
HMAC-SHA256(
  anchor_operation_secret,
  u16be(23) || "riot/namespace-token/v1" ||
  u16be(byte_length(operation_id)) || operation_id ||
  u16be(byte_length(namespace_id)) || namespace_id ||
  u64be(operation_expiry_unix_seconds) ||
  u32be(token_secret_epoch)
)
```

so exact prepared replay returns the same tokens after restart without storing
them in plaintext. The operation stores the token-secret epoch; key-store
rotation retains prior secrets until every operation using them expires.
The Prepare idempotency row normally remains `Prepared` and replays its original
response after operation completion; a caller then uses `GetOperation`. The
security exception is an uncommitted peer-bound replica whose session closes:
its Prepare mapping atomically becomes Terminal `peer_context_changed`, so
invalid tokens are never replayed as usable. Every `CommitHost` has its own
idempotency row. The final
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
| `HostingReceiptV1`, `ListingReceiptV1`, or `ReplicaSourceAttestationV1` | 4 KiB |
| `WorkChallengeV1`, `WorkStampV1`, `ReplicaPrepareChallengeV1`, refusal, peer hello, or peer proof | 4 KiB |
| `PrepareHost` request / response | 32 KiB / 8 KiB |
| `CommitHost` request / response | 4 KiB / 8 KiB |
| `SubmitListing` request / response | 32 KiB / 8 KiB |
| `PrepareReplica` request / response | 60 KiB / 8 KiB |
| `GetOperation` response | 16 KiB |
| Directory feed or snapshot frame | 60 KiB |
| Public directory JSON response | 4 MiB |

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
and `work_target_digest` computed with the fixed optional `work_stamp` slot set
to `null`.
Output is a signed `WorkChallengeV1` as defined under Admission work stamp.
Challenge retrieval is rate-limited but does not create a durable idempotency
row.

### `PrepareHost`

Input:

- root-signed composite ticket;
- client-observed namespace snapshot digests;
- optional valid admission work stamp.

Output:

- stable operation ID;
- captured base site generation;
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

`SubmitListing` has an atomic idempotency boundary. For ordinary listing or
refresh, one SQLite transaction claims the request, changes the listing/search
state, appends exactly one inclusion, stores the current-state record,
invalidates projection generation, creates the signed receipt, and stores the
Terminal result. Crash/retry returns that exact result and never appends a
second inclusion.

Valid owner unlisting uses the separately reserved removal table and this
recoverable state:

```text
RemovalCommitted {
  idempotency_key,
  control_request_digest,
  listing_digest,
  inclusion_digest,
  receipt,
  checkpoint_work_id: optional
} |
RemovalTerminal { idempotency_key, control_request_digest, exact_result }

RemovalSlot =
  Free |
  ReservedForListedRoot { root } |
  Committed {
    root,
    removal_operation_id,
    idempotency_key,
    control_request_digest
  } |
  Terminal {
    root,
    idempotency_key,
    control_request_digest,
    expires_at,
    exact_result
  }
```

The table preprovisions exactly `2 * L` physical slots. The transaction that
makes `listed: true` visible must atomically change one `Free` slot to
`ReservedForListedRoot`; every currently listed root therefore owns removal
capacity that another root cannot consume. A root owns at most two slots total,
including its current reservation and retained Terminal results. If no global
slot is free, or that root already owns two retained Terminal slots, listing
remains hosted but invisible and returns `removal_replay_window` with required
`retry_after_seconds` equal to the earliest blocking Terminal expiry. This
per-root rule is why a third list/remove cycle cannot consume the capacity
needed to remove a newly visible listing.

After structural bounds and authority verification, the unlisting transaction
serializes on the root, changes its reserved slot directly to `Terminal` or
operation-linked `RemovalCommitted`, changes listing/search/current-state
visibility, appends one `Removed` inclusion, invalidates the projection, and
creates the receipt. It never uses ordinary idempotency capacity. Concurrent
different keys cannot both mutate it; the loser observes `already_unlisted`
without retaining a claim. While already unlisted, another key receives that
bounded result; receipt recovery uses the original key. Relisting acquires a
new `Free` slot. After two list/remove cycles inside the 24-hour retention
window, relisting—not a subsequent removal—waits for the earlier Terminal.

Reservation lifecycle follows visibility intent. Hosting eviction or a
reversible security suspension keeps `ReservedForListedRoot`, because the
listing may become visible again and its owner must still be able to remove it.
Listing expiry, manifest-invalidating terminal suspension, or an operator
terminal administrative removal atomically appends the appropriate signed
`Suspended`/`Removed` inclusion, clears current visibility/restore intent, and
changes `ReservedForListedRoot` to `Free` in the same transaction. A later
owner tombstone observes the durable `already_unlisted` floor and needs no
removal slot. Startup cleanup performs the same transition idempotently, so
abandoned reservations cannot accumulate.

One fixed-capacity `IdempotencyKeyIndex`, sized for the ordinary ceiling plus
`2 * L` exclusively reserved entries, makes a key unique across ordinary and
reserved records. Ordinary claims can never consume the reserved partition.
Before any
reserved lookup the anchor performs the same bounded decode, canonical
re-encoding, and `control_request_digest` computation as an ordinary request.
It then looks up the key: equal digest returns only that exact stored state or
result; unequal digest returns `idempotency_conflict` without revealing the
stored result; an absent key proceeds to authority verification. The
unlisting transaction atomically claims the index entry and records the digest
in `RemovalCommitted`/`RemovalTerminal`. No later phase may drop or recompute
it. This lookup precedence applies even when a key was first used by an
ordinary operation.

Emergency checkpoint generation uses a durable work record:

```text
CheckpointWorkV1 {
  work_id,
  phase: Planned | Signed | FilesPublished | FloorAdvanced | Reclaimed,
  frozen_state_generation,
  covered_head_sequence,
  covered_head_inclusion_digest,
  previous_checkpoint_digest,
  created_at,
  canonical_checkpoint_body,
  checkpoint_envelope: optional,
  checkpoint_digest: optional,
  snapshot_generation_id,
  ordered_members: [(root, snapshot_record_digest)],
  covered_removal_operation_ids,
  temp_name,
  final_name
}
```

One planning transaction freezes immutable versioned snapshot-member rows,
their order/digests, head, state generation, previous checkpoint, creation
time, canonical checkpoint body, output identities, and every unassigned
`RemovalCommitted` operation it covers. Later site/listing changes create new
versions and cannot alter this work. The operator signs the persisted body and
stores the exact envelope/digest as `Signed`.

Filesystem publication is fixed:

1. write the signed checkpoint and frozen member bytes under `temp_name`;
2. hash/validate them, fsync every file, and fsync the temporary directory;
3. atomically rename to `final_name` on the same filesystem and fsync its
   parent directory;
4. after step 3, persist `FilesPublished` with the validated final hashes;
5. in one database transaction verify the published hashes, switch the
   checkpoint/snapshot pointer, advance the logical feed floor, and change
   every covered removal slot/operation to `RemovalTerminal` with its original
   idempotency key, request digest, and exact receipt; that same commit persists
   `FloorAdvanced`;
6. only after that transaction may each waiting caller receive success.

Recovery inspects the persisted phase and exact names/hashes, safely removes an
unpublished temp tree or resumes the next step, and never invents a new
timestamp, membership set, body, digest, inclusion, receipt, or result. One
checkpoint can terminalize many covered removals atomically; each remains
recoverable through its own original key.

Physical feed-row deletion, index reclamation, old-generation deletion, and WAL
truncation are **not** on the acknowledgement path. After `FloorAdvanced`, a
bounded maintenance worker deletes at most 1,024 covered rows per transaction,
checkpoints within ordinary WAL limits, retains the newest two published
generations, and persists `Reclaimed` only after all named physical work
completes. Ordinary listing and
relisting admission stays paused until reserve readiness is restored, but
logical floor advancement and every already-reserved owner removal remain
independent of worst-case physical compaction amplification.

### `PrepareReplica`

This operation is accepted only on a mutually authenticated, configured
anchor-peer connection whose rule permits the named site. It does not ask a
peer pinned at the current descriptor to validate an old hosting receipt.
After the peer handshake, the source instead creates:

```text
ReplicaPrepareChallengeV1 {
  destination_anchor_id,
  random_256_bit_nonce,
  prepare_idempotency_key,
  full_site_root,
  issued_at,
  expires_at
}

ReplicaSourceAttestationBodyV1 {
  source_anchor_id,
  source_current_operator_key_id,
  source_current_descriptor_epoch,
  source_current_descriptor_digest,
  destination_anchor_id,
  peer_transcript_digest,
  destination_prepare_nonce,
  prepare_idempotency_key,
  full_site_root,
  manifest_digest_and_version,
  root_signed_ticket_core_digest,
  source_site_generation,
  ordered_namespace_snapshot_digests,
  issued_at,
  expires_at
}
```

The destination sends the challenge on the authenticated peer connection. It
retains at most 32 live challenges per peer for one minute; expiry is at most
one minute. The source refuses a challenge whose destination, site, or
idempotency coordinate does not match the intended replication.

The source's current operator key signs
`"riot/replica-source-attestation/v1" || canonical_cbor(body)`. The expiry is
at most five minutes after issuance. The descriptor coordinates must equal the
source head authenticated by the current peer handshake; destination ID and
the table-defined `peer_transcript_digest` bind it to this connection and peer.
The source issues it only from one immutable currently hosted-site snapshot.
`ReplicaSourceAttestationV1` is
`OperatorSignedEnvelopeV1<ReplicaSourceAttestationBodyV1>`.

`PrepareReplica` input uses the challenge's idempotency key and contains this
attestation, the same complete root-signed ticket core, and desired snapshot
digests. One destination transaction atomically consumes the unique challenge
nonce and table-defined `replica_source_attestation_digest`, claims that
idempotency key, and creates the operation.
Exact request replay returns the prepared result; a different request or second
claim receives `attestation_consumed`.

The destination verifies the current-key signature, connection/peer binding,
time, ticket/manifest/site coordinates, and exact snapshot tuple before
creating tokens and applying peer-specific byte/site budgets. To begin
`ReplicaIntoStaged`, the source first opens one immutable repository read
transaction, then reads and compares generation plus all namespace digests
inside that transaction, and streams only from that same snapshot. A mismatch
fails `stale_source`; no compare-then-open sequence is legal. Changes committed
after the immutable snapshot opens appear in the next replication.

`stale_source` immediately discards destination staging. The source obtains a
fresh destination challenge and attestation and may retry at most three times
with 1/2/4-second backoff inside peer budgets. Publish progress exposes
`RetryingStaleSource`; background gossip exposes the same attempts under
Technical details, then a typed `source_changed` terminal outcome without
changing existing hosted copies. No historical operator key is accepted merely
because it signed an old receipt. `CommitHost` produces a destination-signed
hosting receipt. Replication never copies listing status implicitly.

Publishing a new local descriptor head atomically stops peer-operation
admission and closes every live authenticated peer session before the new key
or coordinates serve operations. Peers must complete a new descriptor-chain
exchange and handshake; an attestation or challenge from the old session is
invalid even if its wall-clock expiry has not elapsed.

Every prepared replica operation persists source and destination descriptor
epoch/digest, `peer_transcript_digest`, and the connection's monotonically
allocated `peer_session_generation`. The generation comes from a persisted
anchor-wide `u64` counter incremented before a session becomes authenticated;
exhaustion fails closed. It is valid only while that exact authenticated
session is live. Any session close—including descriptor/config rotation,
transport loss, or orderly shutdown—atomically:

- terminalizes every uncommitted replica operation for that session as
  `peer_context_changed {
  side, prior_descriptor, latest_known_descriptor, reason }`;
- invalidates its namespace-token acceptance and deletes destination staging;
- changes its Prepare idempotency mapping to that Terminal result.

Exact same-key/body replay then returns `peer_context_changed`, never the old
tokens. Same key/different body always returns `idempotency_conflict` before
attestation handling. A fresh key that reuses a consumed challenge/attestation
returns `attestation_consumed`. Recovery requires a new descriptor exchange,
peer handshake, challenge, attestation, and prepared operation; already
committed destination receipts/state are unaffected.

A process crash cannot preserve an authenticated QUIC session. Before control,
sync, or idempotency-replay readiness on every startup, one recovery
transaction selects every uncommitted replica preparation regardless of its
persisted session generation, records Terminal `peer_context_changed {
side: destination, reason: process_restart }`, invalidates namespace-token
acceptance, deletes staging, and replaces its Prepare idempotency mapping with
that exact result. Only then may the persisted session-generation counter
allocate a higher value or any peer operation become ready. No generation is
reused, and a crash-orphaned prepared response can never become live again.

### `PullDirectoryFeed`

This peer operation is paginated by the source anchor's monotonically
increasing feed sequence:

```text
PullDirectoryFeed { after_sequence, limit: at most 32 }
DirectoryFeedPage { inclusions, floor_sequence, head_sequence, head_digest, done }
CheckpointRequired { checkpoint, snapshot_cursor }
```

`head_digest` is the table-defined `inclusion_digest` at `head_sequence`.
Each inclusion is at most 48 KiB; the server includes no more than the
requested count that fits one 60 KiB page, preserving the control-frame
ceiling. The operation is read-only and available only to authenticated
configured peers.

### `PullDirectorySnapshot`

Input names a verified checkpoint digest and optional opaque snapshot cursor.
Output contains the immutable checkpoint plus the next full-root-ordered
`DirectorySnapshotRecordV1`, next cursor, and `done`. At most one snapshot
record and 60 KiB is returned per frame. On the final frame the receiver must
recompute the checkpoint state digest before advancing its feed cursor.

### `GetOperation`

Returns the current state or retained terminal result for an operation ID,
allowing recovery after a disconnect between commit and receipt delivery.

### Refusals

```text
ControlRefusal {
  code,
  subject:
    ticket | manifest | listing | namespace | capacity |
    version | transport | operation | work | peer,
  retryable,
  retry_after_seconds: optional,
  details
}
```

`code` is a closed lowercase textual enum. `details` must have the exact
matching sum shape below; no generic map or extra diagnostic fields are
allowed:

| Code | Exact `details` |
| --- | --- |
| `invalid_authority`, `not_hosted`, `idempotency_conflict` | `["none"]` |
| `unsupported_version` | `["versions", supported_versions]` |
| `over_quota` | `["quota", limit_id, effective_value, observed_value]` |
| `unsupported_transport` | `["transport", required_mode, observed_mode]` |
| `manifest_transport_mismatch`, `manifest_mismatch` | `["digests", expected_digest, observed_digest]` |
| `expired` | `["expiry", expires_at, observed_at]` |
| `equivocation` | `["equivocation", first_digest, second_digest]` |
| `anchor_profile_oversize` | `["encoded_size", observed_bytes, maximum_bytes]` |
| `site_too_large` | `["storage", required_class, advertised_bytes, local_limit_bytes]` |
| `work_required` | `["work", policy_epoch, difficulty]` |
| `stale_base` | `["site_state", current_generation, ordered_namespace_snapshot_digests]` |
| `stale_source` | `["source_state", observed_generation, current_generation, ordered_namespace_snapshot_digests]` |
| `attestation_consumed` | `["attestation", replica_source_attestation_digest]` |
| `already_unlisted` | `["listing_state", "already_unlisted"]` |
| `removal_replay_window` | `["relist_window", earliest_retry_at]` |
| `operation_not_found` | `["operation", operation_id]` |
| `operation_expired` | `["operation_expiry", operation_id, expires_at]` |
| `checkpoint_unavailable` | `["checkpoint", checkpoint_digest, reason]` |
| `peer_context_changed` | `["peer_context", side, prior_descriptor_digest, optional_latest_descriptor_digest, reason]` |
| `busy` | `["capacity", limit_id]` |
| `peer_auth_failed` | `["peer_auth", stage]` |

The nested enums are also closed: `side` is `source | destination`; peer-context
`reason` is `descriptor_rotation | configuration_rotation | transport_loss |
orderly_shutdown | process_restart`; transport mode is `require_none |
require_arti | unsupported_other`; storage `required_class` is
`profile_total | site_logical_bytes | entries_per_namespace | item_payload |
bundle`; and peer-auth `stage` is `descriptor_exchange | hello_validation |
channel_binding | initiator_proof | responder_proof | configured_rule`.
Checkpoint-unavailable `reason` is `unknown | reclaimed | snapshot_missing |
digest_mismatch`.
Unknown code/detail or enum pairings fail decoding. Error text and
implementation diagnostics stay outside the wire result.
`removal_replay_window`, `busy`, and every overload result require
`retry_after_seconds`; for relisting it is the exact earliest retained
removal-slot expiry.
A refusal is a protocol result, not a transport failure and not a signed
hosting receipt.

## Typed Client Operation Results

Native boundaries expose source-specific envelopes:

```text
AnchorAttempt<T> {
  anchor_id,
  result:
    Verified(T) |
    Refused(ControlRefusal) |
    Overloaded {
      surface: Https | Control,
      retry_after_seconds,
      http_status: optional 429 | 503
    } |
    TransportFailure(kind),
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

FollowFailure =
  LocalStorage {
    required_class,
    advertised,
    local_limit,
    recovery: ManageFollowedSiteStorage | Cancel
  } |
  SourceAttempts { attempts } |
  InvalidReference { subject } |
  AuthorityConflict { subject, evidence } |
  CancelledAfterConsent |
  Internal

FollowState =
  Validating |
  Resolving { attempts } |
  AwaitingConsent { verified_site, ticket_core, destination_status } |
  WaitingForLocalStorage { retry_context, consent_retained, retry_after } |
  WaitingForSourceCapacity { attempts, next_retry_at } |
  Syncing { per_anchor_progress } |
  CommittingLocal |
  RecoveringLocalCommit { local_operation_id } |
  ProfileClosing { retry_context } |
  ProfileClosed { retry_after_reopen } |
  CloseIncompleteRecoverable { reason, retry_context } |
  Saved { site_root, destination_status, attempts } |
  AlreadyFollowed { site_root, destination_status } |
  Cancelled { mutation: None } |
  Failed { phase, failure: FollowFailure, retry_context, local_mutation: None } |
  Quarantined { subject, evidence }

ReplacementPhase =
  VerifyingNew |
  HostingNew |
  ListingNew |
  ConfirmingRedundancy |
  DisablingOldRoute |
  OptionallyUnlistingOld |
  Complete

DestinationPublishOutcome =
  Pending { phase } |
  RetryingStaleSource {
    source_anchor,
    attempt,
    next_retry_at
  } |
  SourceChanged {
    source_anchor,
    attempts,
    existing_hosted_copy: Unchanged,
    actions: RefreshSource | ChooseDifferentSource | RetryLater | CancelDestination
  } |
  Hosted { receipt } |
  Listed { hosting_receipt, listing_receipt } |
  WaitingToRelist {
    signed_listing_intent,
    retry_at,
    prior_removal_receipts_retained: true,
    hosted_state: Unchanged,
    auto_retry: PendingUnlessCancelled,
    actions: CancelDestination | CheckAgain
  } |
  Refused(ControlRefusal) |
  Overloaded { retry_at, preserved_work } |
  Unreachable { retry_context } |
  Cancelled {
    hosted_state: Unchanged,
    retained_hosting_receipt: optional,
    retained_listing_receipt: optional
  }

PublishState =
  SelectingHosts |
  Preparing { per_destination: Map<AnchorId, DestinationPublishOutcome> } |
  Syncing { per_destination: Map<AnchorId, DestinationPublishOutcome>, namespace } |
  WaitingForLocalStorage {
    blocked_by_operation_id,
    retry_context,
    per_destination: Map<AnchorId, DestinationPublishOutcome>
  } |
  PersistingLocalResults {
    local_operation_id,
    per_destination: Map<AnchorId, DestinationPublishOutcome>
  } |
  RecoveringLocalPersistence {
    local_operation_id,
    per_destination: Map<AnchorId, DestinationPublishOutcome>
  } |
  RecoveringRemoteReceipt {
    per_destination: Map<AnchorId, DestinationPublishOutcome>
  } |
  WaitingToRelist {
    per_destination: Map<AnchorId, DestinationPublishOutcome>,
    pending_anchor_ids: [AnchorId],
    completed_anchor_ids: [AnchorId],
    earliest_retry_at,
    actions: CancelAllWaiting | ContinueIndependentRetries
  } |
  Overloaded { per_destination, earliest_retry_at, preserved_work } |
  ProfileClosing { per_destination, preserved_work } |
  ProfileClosed { retry_after_reopen, per_destination, preserved_work } |
  CloseIncompleteRecoverable { reason, per_destination, preserved_work } |
  Hosted { receipts, redundancy } |
  Listing { per_destination: Map<AnchorId, DestinationPublishOutcome> } |
  Listed { listing_receipts, host_receipts } |
  Unlisting { per_destination: Map<AnchorId, DestinationPublishOutcome> } |
  Refreshing { per_destination: Map<AnchorId, DestinationPublishOutcome> } |
  Replacing { old_anchor, new_anchor, phase: ReplacementPhase } |
  Partial { per_destination: Map<AnchorId, DestinationPublishOutcome> } |
  Failed { phase, per_destination: Map<AnchorId, DestinationPublishOutcome> }

ConfigurationChange =
  Single { anchor_id, kind: Added | Enabled | Disabled | Removed } |
  Batch {
    added: [AnchorId],
    enabled: [AnchorId],
    disabled: [AnchorId],
    removed: [AnchorId]
  }

HostConfigurationState =
  Idle { configured_hosts } |
  FetchingDescriptor { origin } |
  VerifyingDescriptor { origin, descriptor } |
  AwaitingHostConsent { descriptor, continuity, roles } |
  PersistingHost { intent_id, descriptor, selected_roles } |
  Enabling { intent_id, anchor_id } |
  Disabling { intent_id, anchor_id } |
  RemovingLocalRoute { intent_id, anchor_id } |
  ResettingEmbeddedDefaults { intent_id } |
  WaitingForLocalStorage {
    blocked_by_operation_id,
    requested_change,
    retry_after
  } |
  RecoveringConfiguration { intent_id } |
  ProfileClosing { requested_change_or_intent } |
  ProfileClosed { retry_after_reopen, requested_change_or_intent } |
  CloseIncompleteRecoverable { reason, requested_change_or_intent } |
  Ready { configured_hosts, change: optional ConfigurationChange } |
  Cancelled { mutation: None } |
  Failed { phase, reason, observed_change: None }
```

Every `Map<AnchorId, T>` above is a bounded canonical array of
`[full_anchor_id, T]` pairs sorted by full anchor-ID bytes, not a CBOR map.
There is exactly one entry for every selected destination. A destination
transition replaces only its own entry, so simultaneous `SourceChanged`,
success, refusal, overload, unreachable, and relist-wait outcomes remain
visible together. Aggregate `Listed` requires every non-cancelled selected
destination to reach its requested terminal success; otherwise the operation
ends `Partial` with the complete map.

Every local mutation exposes:

```text
LocalMutationOutcome =
  Busy { active_operation_id, retry_after_seconds } |
  Running { operation_id, cancellable_before_start } |
  Recovering { operation_id } |
  Committed { operation_id, exact_result } |
  NotCommitted { operation_id, retry_context } |
  Closed
```

`Busy` is not queued and is never collapsed into `Internal`. Follow preserves
the verified preview and accepted consent for the bounded ticket/preview
lifetime; explicit Retry resubmits after the active mutation finishes. Publish
preserves verified remote receipts in `remote_results` while local persistence
is busy. `RecoveringRemoteReceipt` uses anchor `GetOperation`;
`RecoveringLocalPersistence` and `RecoveringLocalCommit` use the core storage
port's local operation/intent lookup—never the anchor. Host configuration
preserves the requested single/batch change but creates no intent until retry
is admitted. Once a local command begins, cancellation cannot claim no
mutation: `Recovering` reads its durable operation/intent and ends in exactly
`Committed` or `NotCommitted`.

The UI mapping is exhaustive:

| Durable boundary | Follow | Publish | Host configuration |
| --- | --- | --- | --- |
| command not admitted because another local write owns the slot | `WaitingForLocalStorage` | `WaitingForLocalStorage` | `WaitingForLocalStorage` |
| admitted and durably identified, result not yet known | `CommittingLocal` | `PersistingLocalResults` | the requested `Persisting`/`Enabling`/`Disabling`/`RemovingLocalRoute`/`ResettingEmbeddedDefaults` state |
| admitted, callback/response lost | `RecoveringLocalCommit` | `RecoveringLocalPersistence` | `RecoveringConfiguration` |
| remote receipt verified, local receipt write not admitted or recovering | not applicable | `WaitingForLocalStorage` or `RecoveringLocalPersistence`, with `remote_results` retained | not applicable |
| close begins before admission | `ProfileClosing` | `ProfileClosing` | `ProfileClosing` |
| profile is closed and no same-database work remains | `ProfileClosed` | `ProfileClosed` | `ProfileClosed` |
| close cannot yet prove the durable result or release the database | `CloseIncompleteRecoverable` | `CloseIncompleteRecoverable` | `CloseIncompleteRecoverable` |

No `Busy` outcome creates an operation or may be shown as recovery. No
`Running`/`Recovering` outcome may be shown as no mutation, failure, or safe
profile switch until its durable lookup resolves.

Profile close transitions preserve only bounded verified previews, signed
listing intents, remote receipt envelopes, and retry IDs in protected native
handoff state. `ProfileClosed` says “Profile locked—reopen to continue.”
`CloseIncompleteRecoverable` says the profile is still finishing a local write,
prevents switching to the same database, and offers Retry close; no operation
is labeled failed or uncommitted until recovery proves it.

`expired` always names the expired subject. `over_quota` always names the
anchor and quota class. A verified receipt, protocol refusal, and local network
failure can never collapse into the same enum case.

Directory merging owns a local opaque cursor containing the normalized query
hash, merged snapshot ID, and one `(anchor_id, source_snapshot_id,
source_cursor)` tuple per configured source. A cursor is invalid if used with a
different query or source set. Per-source exclusions remain available in
Technical details even when a record is absent from visible results.

Client configuration contains at most 32 hosts; every `ConfigurationChange`
array is deduplicated, sorted by full `AnchorId`, and bounded by that same
count.

Legal mutation boundaries are normative:

- Explore never mutates follow or hosting state.
- HTTP 429/503 with the bounded typed body code `overloaded` maps to
  `AnchorAttempt::Overloaded` with the validated `Retry-After`; another 503
  remains a source-specific transport/service failure. Anchors emit decimal
  delta-seconds; clients accept `1..300`,
  use five seconds when absent/invalid, and never retry past the operation
  deadline. The client immediately tries the next configured source,
  never reports overloaded sources as zero results or invalid references, and
  schedules a same-source retry only within the operation timeout.
- Follow mutates no durable state before `Accept` from
  `AwaitingConsent`; `Cancel` or app termination there discards the retained
  verified payload with no mutation. `Retry` from a retryable failure consumes
  the explicit `retry_context`, never hidden side state.
- Accepted data is staged locally; `Saved` is emitted only after one atomic
  local follow commit. Cancellation or failure before it emits
  `local_mutation: None`.
- Local storage is rechecked before every stage append and final promotion.
  If availability changes after consent, the whole private follow stage rolls
  back and emits `FollowFailure::LocalStorage`; retry after storage management
  starts from the retained verified envelope and never exposes a partial site.
- Hosting, listing, unlisting, and refresh are independent per-anchor
  operations. Aggregate `Partial` never rewrites a verified per-anchor result.
- Replacing a host first reaches `Hosted` on the new anchor. Local removal of
  the old host then stops future client sync but does not claim remote deletion.
  Remote unlisting is a separate signed operation, and retained hosted state
  expires under the old receipt's policy.
- Host configuration persists nothing before `AwaitingHostConsent` acceptance.
  A change refused as local `Busy` stores no intent. Every admitted
  add/enable/disable/remove/reset first stores an intent ID and atomically
  commits its single or batch `ConfigurationChange`. Reset replaces only the
  routing configuration with package-signed embedded defaults; it never
  changes follows, listings, or remote state. After crash or terminal-delivery
  loss in any mutation state, recovery rereads the intent/result and emits
  `Ready` with the exact single/batch change if committed, or `Failed {
  observed_change: None }` only when no configuration mutation exists.

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
default-on `rusqlite`, `RiotDatabase`, migration runner, and profile database.

This requires an explicit refactor of today's ownership:

1. Today `MobileProfile` contains only `Arc<Mutex<ProfileState>>`; it does not
   own the database field. The storage ownership change is inside the actual
   `LocalProfile`: remove both its `store: EvidenceStore` and
   `db: Option<RiotDatabase>` fields and add one revocable
   `storage_port: ProfileStoragePortLease`. The lifecycle section adds the
   only strong storage owner beside `inner`, never a raw database.
2. Add `CoreProfileStorage` in `riot-core::store`. Its durable constructor takes
   the single opened `RiotDatabase` and internally creates the existing
   `EvidenceRepository`/`EvidenceStore`, the new `ClientSiteRepository`,
   registry persistence, transaction gate, migrations, backup/restore, and one
   bounded serialized command worker. Move the current
   `riot-ffi::community_registry` durable record/codec and quarantine keys into
   `riot-core::profile`; FFI retains only projections and UI conversion. The
   memory constructor provides the same command port without SQLite.
3. Replace the FFI calls to `RiotSession::open_sqlite(database)` and retained
   database cloning with `CoreProfileStorage::open_sqlite(database)`. The
   storage worker exclusively owns `RiotSession`, `EvidenceStore`, every live
   core `ImportPreview`/`ImportPlan`, and maps them to opaque generation-scoped
   IDs. `StoredPreview`, `StoredPlan`, `MobileImportPreview`,
   `MobileImportPlan`, sync handles, and other FFI state retain only those IDs,
   immutable display projections, and a revocable profile lease. No FFI type
   owns a raw database handle, repository, transaction, callable core session
   handle, or strong storage `Arc`; all existing evidence and registry access
   as well as new client-site access crosses `ProfileStorageCommand`.
4. Refactor every current `with_active` path that calls `profile.store`,
   `persist_registry`, `load_registry`, backup/restore, or another
   SQLite-capable helper into three phases. Under `ProfileState`, it prepares
   immutable command inputs, captures the relevant community/app generation,
   installs one unique pending profile-operation ID for a mutation, and clones
   the revocable storage-port lease. It then releases `ProfileState` and
   executes the command.
   Finally it reacquires `ProfileState` and applies the returned projection only
   when the pending ID and captured generation still match.
5. Exactly one mutable profile command may be pending. A second mutation is
   never queued: it returns `LocalMutationOutcome::Busy` before capturing inputs
   or creating a durable intent. Read commands may queue with a captured
   generation and discard their result if it changes. A failed mutation clears
   the pending marker without changing memory. If the process closes after
   durable commit but before in-memory apply, reopen loads the committed
   registry/evidence/site state and rebuilds projections.

`ProfileStorageCommand` includes the operations now reached through
`EvidenceStore`—inspect/load/query/commit—and registry load/replace, in addition
to client-site stage/promote, configuration, floors, receipts, backup, restore,
and migration. This is not a client-site-only worker. Its compiled queue holds
at most 64 commands, but at most one queued/running command may mutate; remaining
slots are generation-tagged reads. A full queue returns typed `busy`. Commands
are leaf operations: they never call back into, lock, or await `ProfileState`.
“Mutation” includes any existing `EvidenceStore` inspect/plan/commit operation
that changes session handles or repository state, plus registry, site,
configuration, receipt, migration, backup, or restore changes; only proven
side-effect-free queries use read slots.
Cancellation marks a queued mutation; the worker skips it and returns
`NotCommitted`. Once execution begins, cancellation only detaches the caller
and the durable result is reported as `Recovering`.
Synchronous UniFFI calls may wait for their result only after releasing
`ProfileState`;
`riot-client-net` awaits the same bounded command port and never executes
`rusqlite` on an iroh/Tokio network executor.

The normative lock rule is testable: acquiring `ProfileState` while a storage
command is already running is allowed, but invoking or awaiting any
`ProfileStorageCommand`, `EvidenceStore`, registry database method, or raw
`RiotDatabase` operation while holding `ProfileState` fails the lock-order test.
Cancellation closes the network operation but lets an already-started atomic
storage command finish and report its recovered outcome.

### Native profile lifecycle

`MobileProfile` gains an `Arc<MobileProfileRuntime>` beside its existing
`inner`. `MobileProfileRuntime` is the sole strong owner of that profile's
`CoreProfileStorage`, cancellable operations, one
`ProfileNetworkLease` borrowed from the process `RiotApplicationRuntime`, and
all revocable storage-port leases. It does not own or close the process iroh
endpoint/Tokio runtime.

```text
MobileProfileRuntimeState =
  Open |
  Closing { close_id, deadline } |
  CloseIncompleteRecoverable { close_id, reason } |
  Closed { close_id, storage_outcome }

ProfileState =
  Active(LocalProfile { storage_port: ProfileStoragePortLease, ... }) |
  Closing { close_id, recovery_operation_ids } |
  Closed { close_id } |
  Failed
```

UniFFI exports idempotent async `MobileProfile.close()`. Native profile lock,
profile switch, and orderly app teardown call it explicitly; generated wrapper
disposal is not treated as an awaited close. Exact close retry replays `Closed`
or resumes `CloseIncompleteRecoverable`. Close performs this fixed order:

1. atomically enter `Closing` and close new FFI/network operation admission;
2. cancel all network operations and await their terminal events within ten
   seconds, while their existing storage leases remain valid only for terminal
   recovery;
3. release the profile's `ProfileNetworkLease`; this cancels that profile's
   remaining network tasks but leaves the process endpoint/runtime serving
   other profiles;
4. revoke every cloned storage-port lease so retained profile or child-handle
   `Arc`s cannot submit more work;
5. under `ProfileState`, replace `Active` with `Closing`, retaining only
   recovery operation IDs, then release `ProfileState`;
6. close storage admission; terminate every queued read waiter with `Closed`;
   cancel a queued not-yet-started mutation as `NotCommitted`;
7. allow the one running atomic command its compiled ten-second deadline,
   recover its durable result, checkpoint WAL, join the worker, and release
   `RiotDatabase`;
8. publish the replayable runtime and `ProfileState::Closed` result.

At the network deadline, the profile runtime closes only connections and
streams registered to its `ProfileNetworkLease` and aborts only that profile's
remaining network tasks. It never closes the process iroh endpoint or Tokio
runtime while any application-runtime lease exists. A database command a
profile task already submitted is
owned solely by the storage worker and still resolves through durable recovery.
Storage commands use SQLite interruption/busy deadlines so normal close is
bounded. If underlying I/O cannot return, close reports typed
`CloseIncompleteRecoverable`, keeps the database path unavailable to a new
profile, and relies on WAL/intent recovery after process termination rather
than falsely publishing `Closed`.

No close/join/database release occurs while holding `ProfileState`. A normal
profile switch cannot open another profile at the same database path until
close reaches `Closed`. If the OS kills the process before the bounded drain
finishes, the next open performs WAL and durable-intent recovery before
readiness; it never assumes cancellation meant no commit.

`RiotApplicationRuntime` is acquired through one process-global constructor;
a second constructor returns the existing instance rather than starting another
endpoint/runtime. Its idempotent async close first requires every
`MobileProfileRuntime` lease to be `Closed`, rejects new leases, joins the
process network tasks, and then closes the endpoint/Tokio runtime. A profile
close can therefore neither duplicate nor prematurely terminate process-shared
networking.

New versioned tables store verified manifests, `O/C/W` entries, payloads,
follow/host staging, anchor configuration/floors, receipts, and retry intent.
Core backup/restore includes these tables and validates their floors before
reopening. One core-owned transaction promotes all three namespaces plus the
ongoing-sync schedule after consent. `riot-ffi` only constructs the retained
owner, injects ports, phase-splits in-memory state transitions, and projects
typed results.

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
| `DirectoryInclusionV1` or `DirectorySnapshotRecordV1` | 48 KiB |
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

The following rows correspond one-for-one, in order, to stable limit IDs
`1..=69` in the canonical registry above; slash-compound rows use the
two-element value form.

| Resource | Default | Absolute ceiling |
| --- | ---: | ---: |
| Logical retained bytes, whole anchor | 20 GiB | 100 GiB |
| Physical retained bytes | 20 GiB | 100 GiB |
| Ordinary SQLite database including WAL | 24 GiB | 110 GiB |
| Non-payload metadata bytes | 2 GiB | 8 GiB |
| SQLite WAL bytes | 256 MiB | 1 GiB |
| Emergency removal metadata reserve | 768 MiB | 3 GiB |
| Emergency removal WAL/fsync reserve | 768 MiB | 3 GiB |
| Staged bytes | 256 MiB | 1 GiB |
| Live staged operations | 10,000 | 50,000 |
| Idempotency rows | 100,000 | 500,000 |
| Idempotency rows per source per 24 h | 2,000 | 10,000 |
| Reserved removal idempotency/result rows | `2 * L` | `2 * L` |
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
| TCP listen backlog | 256 | 1,024 |
| Accepted public HTTPS sockets | 512 | 2,048 |
| Pending TLS handshakes | 64 | 256 |
| TLS handshakes per source per minute | 30 | 120 |
| TLS handshakes globally per second | 200 | 800 |
| TLS ClientHello / total handshake bytes | 16 KiB / 64 KiB | fixed |
| TLS handshake CPU / wall time | 100 ms / 5 s | 500 ms / 10 s |
| Active public HTTPS connections | 256 | 1,024 |
| HTTP requests per keep-alive connection | 100 | 1,000 |
| HTTP idle / absolute connection lifetime | 15 s / 5 min | 60 s / 30 min |
| HTTP decoded header fields / one field line | 64 / 8 KiB | fixed |
| Concurrent public HTTPS handlers | 128 | 512 |
| Queued public HTTPS handlers | 128 | 512 |
| Public HTTPS requests per source per minute | 120 | 600 |
| Public HTTPS requests globally per second | 500 | 2,000 |
| Concurrent public HTTP database snapshots | 32 | 128 |
| Public HTTP database snapshots per source | 2 | 8 |
| Public HTTP query CPU / wall time | 250 ms / 2 s | 1 s / 5 s |
| Public API response bytes | 1 MiB | 4 MiB |
| One static web response | 2 MiB | 8 MiB |
| Search results per page | 50 | 100 |
| Search query UTF-8 bytes | 128 | 256 |
| Directory listings | 10,000 | 50,000 |
| Directory-feed records | 100,000 | 500,000 |
| Verification queue jobs | 512 | 2,048 |
| Verification CPU per request | 500 ms | 2 s |
| Aggregate outstanding verification CPU budget | 16 s | 64 s |
| Reserved owner-removal verification permits | 4 | fixed |
| Queued reserved-removal jobs | 256 | 1,024 |
| Queued reserved-removal canonical bytes | 4 MiB | 16 MiB |
| Reserved valid-removal database writer permits | 2 | fixed |
| Emergency checkpoint worker | 1 | fixed |
| Owner-removal attempts per source per minute | 10 | 40 |
| Owner-removal attempts globally per second | 100 | 400 |
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

The three fixed reservation rows are mandatory partitions, not operator-tunable
ordinary capacity.

Every rate row is a token bucket with no hidden burst: global per-second
buckets hold one second of published tokens, and per-source per-minute buckets
hold one minute. Permit acquisition is nonblocking; failure rejects before
allocating the protected resource.

The emergency reserves are exclusive headroom, not ordinary capacity. For the
configured listing ceiling `L`, readiness computes:

```text
required_emergency_bytes =
  L * (
    MAX_DIRECTORY_SNAPSHOT_RECORD_BYTES + 128 +
    2 * (MAX_REMOVAL_RESULT_BYTES + 128)
  ) +
  64 MiB
```

Both metadata and WAL/fsync reserves must be at least that value, and the
filesystem quota must provision ordinary database/WAL ceilings plus both
reserves. Default `L = 10,000` fits 768 MiB; absolute `L = 50,000` fits 3 GiB.
`MAX_REMOVAL_RESULT_BYTES` is 4 KiB and the separate removal table has exactly
`2 * L` preprovisioned slots.
Ordinary admissions are charged only against their advertised ordinary
ceilings and stop before consuming either separately provisioned reserve. Only
owner removal, expiry, equivocation/security suspension, checkpoint publication,
and the later bounded checkpoint-reclamation/WAL maintenance may consume them.
Logical checkpoint publication and removal acknowledgement never wait for
physical row deletion, index reclamation, or WAL truncation. Readiness fails if
byte, page, free-space, or WAL accounting cannot still publish one worst-case
emergency checkpoint and retain its post-acknowledgement maintenance headroom.

### Public HTTPS admission

Every public HTTP route, including descriptors, descriptor chains, feed and
snapshot pages, search/detail JSON, handoff pages, and static community views,
passes a compiled admission layer independent of `riot/anchor/1` and
`riot/sync/2`.

The bound begins at `accept`, not at routing. The kernel listen backlog is
configured to the table ceiling. An accepted socket must acquire the global
socket permit and transient per-source handshake token before any application
TLS allocation. Pending handshakes have their own semaphore, CPU/wall deadline,
and ClientHello/total-byte counters; resumed handshakes count identically and
TLS 0-RTT is disabled. MVP accepts TLS 1.3 with ALPN `http/1.1` only. HTTP/2,
HTTP/3, pipelining, and protocol upgrade are rejected, so no compressed-header
table, multiplexed stream, or flow-control allocation exists.

After handshake, one connection has at most one in-flight request, the fixed
idle/absolute lifetime, and the fixed request-count ceiling. Before allocating
a handler or opening a SQLite snapshot, the parser enforces the
global/per-source request token bucket, 8 KiB request target, 16 KiB decoded
total headers, 64 decoded fields, 8 KiB per field line, five-second header
deadline, and zero request body. Only `GET` and `HEAD` are accepted. Transfer
encoding, duplicate/ambiguous content length, nonzero content length, every
Range request, dynamic response compression, and unsupported methods receive a
bounded error and close.

The request then acquires the bounded handler queue/permit. Dynamic routes
additionally acquire global and per-source database-snapshot permits, execute
through SQLite progress interruption (or an equivalently killable bounded
query worker) under the published CPU/wall deadline, and serialize no more than
the endpoint and global response-byte ceiling. Static routes use no database
snapshot but retain the same handler, deadline, and response-byte envelope.
Response writes have a 30-second deadline. A limit returns bounded HTTP 429
with `Retry-After` or 503 and creates no durable row, cursor, detached query,
task, or unbounded buffered response.

The source key is a transient connection address or configured trusted-proxy
address prefix used only in in-memory expiring buckets; it is never written to
application or pilot logs. Deployments behind a proxy fail readiness unless
the trusted hop and forwarded-address policy are explicit. A TLS-terminating
proxy must enforce equal or tighter backlog, handshake, protocol, connection,
header, method, and rate ceilings at the outermost listener; checked deployment
configuration and a live enforcement probe are readiness requirements.
Unknown or untrusted forwarded headers are ignored. Readiness also verifies
file-descriptor/process limits can provision the compiled socket pools plus
reserved control/database capacity.

HTTP readers have a separate read-only connection pool and CPU/handler
semaphores. They cannot acquire the control-plane verification permits, the two
database writer permits reserved for valid owner unlisting/security removal,
or the emergency checkpoint/WAL lane. The configured public-snapshot ceiling
is validated low enough that those writers and the emergency reserves remain
available. Structurally invalid or unauthorized removal-shaped control
requests stay in ordinary per-source control budgets. A structurally bounded
tombstone naming an existing durable listing floor may use the separate
per-source removal bucket. The reserved verifier accepts at most one candidate
per `(root, source)`, eight candidates per root, and schedules full roots in
deficit round-robin order; one root may occupy at most one of the four permits.
Its preallocated queue enforces both the global job and canonical-byte ceilings;
it never evicts an admitted candidate. A new candidate beyond either ceiling
receives typed `busy` with retry timing, while already admitted roots continue
deterministic rounds. A failed signature/capability check releases its slot and
cannot create a claim row. Once authority verifies, the job leaves the
untrusted queue and cannot be starved or evicted by later candidates. Only that
canonical tombstone may consume its root's reserved removal slot, acquire one
of the two reserved database writers, and enter the emergency checkpoint lane.

For an established HTTP request, 429/503 serves a prebuilt, at most 8 KiB,
keyboard/screen-reader-accessible error page from a fixed reserved buffer. It
does not redirect, so the canonical directory, community, or handoff URL stays
in the address bar. It states the retry delay, offers a reload action, and
never renders overload as “not found,” an invalid invitation, or zero results.
API routes return the equivalent bounded JSON `{ code: "overloaded",
retry_after_seconds }`; neither representation is generated from database
state.
If the socket or TLS limit prevents connection establishment, native clients
record source-specific overload/transport attempts and continue plural-source
fallback; browsers retain the requested URL for ordinary reload.

Before creating any durable row for a new request, the anchor applies, in
order: frame/body bounds; in-memory connection/source limits; bounded canonical
decode with unknown/duplicate/trailing-field rejection; byte-for-byte canonical
re-encoding; `control_request_digest` computation; then existing idempotency
lookup and constant-time digest comparison. Only an exact digest may read/replay
the prior result; the same key with another body is rejected without revealing
that result. A novel key then passes global metadata/row headroom, work
verification when required, and full authority/admission verification. Exact
replay does not repeat expensive work. A novel key that exceeds any source or
global ceiling is refused without persistence.

For a structurally bounded owner-unlisting candidate, bounds, canonicalization,
digest computation, and global key-index comparison remain first and
unchanged. A novel key then bypasses ordinary idempotency-row headroom and
admission work, enters the fair reserved verifier, and only after authority
succeeds atomically claims the preprovisioned removal slot plus global key
index.

Challenge signing has separate in-memory per-source and global token buckets
before any KMS call. Concurrent final requests for one idempotency key race on
the global index plus their class's durable state transition; only the winner
can consume capacity, and all exact losers replay its state.

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
WorkChallengeBodyV1 {
  anchor_id,
  operator_key_id,
  descriptor_epoch,
  descriptor_digest,
  operation_kind,
  idempotency_key,
  work_target_digest,
  community_root,
  random_challenge,
  policy_epoch,
  difficulty,
  issued_at,
  expires_at
}

operator_signature =
  Sign("riot/anchor-work-challenge/v1" || canonical_cbor(challenge))

WorkChallengeV1 =
  OperatorSignedEnvelopeV1<WorkChallengeBodyV1>

proof =
  BLAKE3(
    "riot/anchor-work-proof/v1" ||
    work_challenge_digest ||
    u64be(counter)
  )

WorkStampV1 {
  challenge_envelope_bytes,
  counter,
  proof_bytes
}
```

`challenge_envelope_bytes` is the complete canonical
`OperatorSignedEnvelopeV1<WorkChallengeBodyV1>` as a CBOR byte string,
`counter` is a `u64`, and `proof_bytes` is exactly the 32-byte BLAKE3 output
above. The optional work-stamp slot in a control semantic body is therefore
either `null` or canonical `[1, challenge_envelope_bytes, counter,
proof_bytes]`.

The proof must have the challenge's number of leading zero bits. The anchor
first canonically decodes the stamp and nested envelope, recomputes
`work_challenge_digest`, verifies its own challenge signature, verifies
`proof_bytes`, and checks every binding: anchor/key/descriptor, intended
operation kind, outer idempotency key, `work_target_digest` of this request
with the work-stamp slot reset to `null`, community root, policy epoch, and time
window. Only then may it perform admission work or claim the request. An exact
request replay compares the stored `control_request_digest` and returns its
existing state without re-consuming work; the same key with a changed stamp,
counter, or body is `idempotency_conflict`.
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

`after_descriptor_digest` and `verified_head_digest` are table-defined
`descriptor_digest` values.
Peer configuration pins the complete authenticated descriptor floor tuple,
normally the provisioned current envelope. The initiator sends
`PeerHello(Initiator)`. If its head is
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

PeerTranscriptV1 {
  control_protocol_version,
  negotiated_alpn_bytes,
  initiator_hello,
  responder_hello,
  channel_binding
}

transcript =
  canonical_cbor([
    1,
    1,
    h'72696f742f616e63686f722f31',
    initiator_hello,
    responder_hello,
    channel_binding
  ])

peer_proof_signature_preimage(role, peer_transcript_digest) =
  u16be(25) || "riot/anchor-peer-proof/v1" ||
  u16be(byte_length(role)) || role ||
  peer_transcript_digest

signature =
  Sign(
    operator_key,
    peer_proof_signature_preimage(role, peer_transcript_digest)
  )
```

The only legal control protocol version is integer `1`; the only legal
`negotiated_alpn_bytes` is the exact 13-byte ASCII value `riot/anchor/1` shown
in hex above. `role` in the signature preimage is the exact lowercase ASCII
`initiator` or `responder`, length-prefixed as shown; no role byte or numeric
alias exists. `peer_transcript_digest` hashes the complete canonical
`PeerTranscriptV1` array under its table label.

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
ZERO_DIGEST_32 =
  h'0000000000000000000000000000000000000000000000000000000000000000'
```

The empty feed is exactly `floor_sequence = 0`, `head_sequence = 0`, and
`head_digest = ZERO_DIGEST_32`. The first inclusion has `sequence = 1` and
`previous_inclusion_digest = ZERO_DIGEST_32`. The first checkpoint and every
`CheckpointWorkV1` planning it use
`previous_checkpoint_digest = ZERO_DIGEST_32`. No real BLAKE3 digest is treated
as a sentinel, and an all-zero value is illegal at later links.

```text
DirectoryInclusionBodyV1 {
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

DirectoryInclusionV1 =
  OperatorSignedEnvelopeV1<DirectoryInclusionBodyV1>
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

Checkpoint state uses a current-key reissuance, not unverifiable historical
signatures:

```text
DirectorySnapshotRecordBodyV1 {
  anchor_id,
  current_operator_key_id,
  current_descriptor_epoch,
  current_descriptor_digest,
  root,
  state,
  reason,
  listing_entry_and_capability,
  optional_listing_delegate_grant,
  public_site_ticket_core,
  admitted_manifest_entry_and_capability,
  accepted_manifest_digest_and_version,
  source_inclusion_digest
}
```

The current operator signs
`"riot/directory-snapshot-record/v1" || canonical_cbor(record_body)`.
`DirectorySnapshotRecordV1` is
`OperatorSignedEnvelopeV1<DirectorySnapshotRecordBodyV1>`.
`source_inclusion_digest` is provenance only. Verification depends on the
current descriptor signature plus fresh independent listing/manifest
admission, so a cold peer need not recover a retired feed-signing key.

Before compacting, the anchor creates:

```text
DirectoryCheckpointBodyV1 {
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
`DirectoryCheckpointV1` is
`OperatorSignedEnvelopeV1<DirectoryCheckpointBodyV1>`.
`state_digest` is:

```text
BLAKE3(
  "riot/directory-state/v1" ||
  for each current root sorted by full root bytes:
    u32be(len(root)) || root ||
    u32be(len(snapshot_record_digest)) || snapshot_record_digest
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
`PullDirectorySnapshot` then pages current `DirectorySnapshotRecordV1` records in
full-root order under that immutable checkpoint, at most one 48 KiB record and
60 KiB per frame. The peer verifies every record, recomputes `state_digest`,
sets its cursor to `head_sequence/head_inclusion_digest`, then resumes the
incremental feed. A checkpoint mismatch or unavailable snapshot is a typed
fail-closed error, never an empty feed.

Ten percent of the feed-record ceiling is reserved for unlisting, expiry,
equivocation, and security suspension, alongside the exclusive byte/WAL
reserves in the resource contract. At 90% utilization the anchor refuses new
listings/refreshes and creates an emergency checkpoint. A removal arriving
while the reserve is under pressure follows the `RemovalCommitted` through
`RemovalTerminal` state machine: its first transaction atomically replaces that
root's bounded current state with a signed `Removed` inclusion; recovery then
publishes the frozen checkpoint snapshot and advances the logical floor before
acknowledging the stored receipt. Bounded physical compaction/WAL maintenance
follows independently. Under a healthy deployment that passed reserve
readiness, this constant-per-root acknowledgement path may shorten incremental
history below the 30-day target but never exceeds ordinary caps or blocks an
owner removal; offline peers recover through the checkpoint snapshot.

Operator-key rotation pauses directory writes, publishes the verified new
descriptor, reissues all bounded current snapshot records under that descriptor,
creates a checkpoint, advances the feed floor to its head, and compacts
older-key incremental rows before directory/gossip readiness returns. Thus a
peer at a newer descriptor floor sees current-key snapshot records; an older
peer first verifies the forward descriptor chain.

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
If the store auto-launches Riot without the community, first-run guidance says
to return to the preserved browser page and tap “Open in Riot”; the app does
not guess the missing destination.

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
| One or more overloaded sources | Continue other sources; label delayed hosts and show the shortest verified retry time |
| All sources unreachable, cached results | Cached results marked “Saved results — may be out of date” |
| All sources overloaded, no cache | “Public hosts are busy” with countdown/retry; never “No communities matched” |
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
- local storage busy, preserving consent/preview and offering retry without
  silently queuing a mutation;
- source overloaded, trying another host and then showing its retry time;
- recovering a local commit whose cancellation/terminal delivery was lost;
- all sources unreachable;
- manifest/listing equivocation quarantined;
- expired ticket with preserved readable web destination and organizer-refresh
  explanation.

Before `Accept`, the verified preview states that this is a public community,
the complete followed site will be stored locally and receive ongoing updates,
and contacted public hosts can observe connection metadata. Storage required
versus available and the selected destination are shown without exposing
protocol jargon.

### Publish and listing

Maintainers see per-anchor progress:

- preparing;
- syncing `O`, `C`, `W`;
- hosted through reported date;
- refused with named quota/subject;
- overloaded with verified retry time and preserved work;
- unreachable;
- receipt recovery;
- local receipt persistence busy/recovery with verified remote success
  preserved;
- source changed during anchor replication, with at most three visible bounded
  retries before a typed per-destination failure;
- listing submitted;
- listing expired/refresh due;
- unlisted.

Hosting and listing are separate controls. A failed listing refresh never
removes hosted state.

Each destination `WaitingToRelist` row says: “This host is keeping earlier
remove receipts replayable until <time>. Your community is still hosted here
but is not in this host’s directory.” It preserves that anchor's signed intent,
offers `Check again`, and automatically retries once at its own exact expiry
unless that destination or all waiting destinations are cancelled. `Check
again` refreshes readiness but cannot bypass the server window; it is disabled
while an attempt is already in flight. Other anchors continue independently,
and completed receipts remain visible while rows count down at different
times. Cancelling one row preserves all other successes and pending retries;
`CancelAllWaiting` cancels only waiting rows. The state is distinct from quota,
overload, invalid authority, and terminal listing failure. A later owner
removal remains available because no relisting transition became visible
without its reserved removal slot.

`SourceChanged` says: “The source changed before this host finished copying.
Copies already hosted elsewhere were not changed.” The destination row names
the source and host, shows the three-attempt history under Technical details,
and offers Refresh source, Choose another source, Retry later, or Cancel this
host without discarding other destination results.

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
Countdowns announce entry into a wait, the final minute, readiness, and failure;
they do not announce every second. Recovery/profile-close transitions preserve
focus and expose operation IDs only under Technical details.

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
or product-account identifier.

To audit distinct participants and prevent duplicate exports, explicit pilot
enrollment creates an unrelated window-scoped Ed25519 keypair. Before the
window, the collector generates shuffled, high-entropy single-use scratch
credentials:

```text
pilot_signature_preimage(label, body) =
  u16be(byte_length(label)) || label ||
  u64be(byte_length(canonical_cbor(body))) || canonical_cbor(body)

PilotInvitationBodyV1 {
  pilot_window,
  random_256_bit_secret,
  fixed_role_flags,
  collector_key_id,
  issued_at,
  expiry
}

PilotInvitationV1 {
  body,
  collector_signature = Ed25519.Sign(
    collector_key,
    pilot_signature_preimage("riot/pilot-invitation/v1", body)
  )
}

PilotEnrollmentRequestBodyV1 {
  invitation,
  window_scoped_public_key,
  random_128_bit_idempotency_key,
  requested_at
}

PilotEnrollmentRequestV1 {
  body,
  client_signature = Ed25519.Sign(
    window_scoped_key,
    pilot_signature_preimage("riot/pilot-enrollment-request/v1", body)
  )
}
```

The app's signed pilot configuration pins the pilot window, safe-dialed HTTPS
collector origin, collector Ed25519 verification key, and:

```text
collector_key_id = digest_v1(
  "riot/pilot-collector-key/v1",
  canonical_cbor({ algorithm: Ed25519, public_key: exactly 32 bytes })
)
```

One authoritative recruitment ledger serves every coordinator.
The coordinator gives exactly one credential, shuffled within its fixed-role
batch, to each consenting participant and atomically records only the human
consent contact, permitted role flags, and `credential_issued: true`; it does
not record the credential, scratch-card order, client key, or later pseudonym.
A person may decline without a credential being marked issued. Roles are fixed
for the window; an attempt to alter them is rejected, and a legitimate role
change is deferred to a later pilot window rather than minting a second token.
“Distinct participant” therefore means a distinct single-use credential under
this trusted recruitment procedure, not cryptographic proof that two
credentials could never reach one human.

The collector invitation lookup and deletion commitment are exactly:

```text
pilot_invitation_lookup = HMAC-SHA256(
  collector_invitation_key,
  u16be(31) || "riot/pilot-invitation-lookup/v1" ||
  u64be(byte_length(canonical_cbor(PilotInvitationV1))) ||
  canonical_cbor(PilotInvitationV1)
)

keyed_token_commitment = HMAC-SHA256(
  collector_deletion_key,
  u16be(30) || "riot/pilot-token-commitment/v1" ||
  u64be(byte_length(canonical_cbor(PilotEnrollmentTokenV1))) ||
  canonical_cbor(PilotEnrollmentTokenV1)
)
```

The collector provisions distinct invitation/deletion HMAC keys per pilot
window before issuing credentials. It never rotates either key during that
window or its 30-day audit period; encrypted backup/restore includes them, and
readiness fails closed if either is unavailable. Enrollment replay, withdrawal,
and deletion-receipt replay use the original window keys. Only after every
invitation/token/receipt row for the window has passed retention may the keys be
cryptographically erased.

The collector stores only `pilot_invitation_lookup` and atomically transitions:

```text
Unspent |
Spent {
  enrollment_request_digest,
  canonical_enrollment_response
} |
Revoked
```

`invitation_digest` and `enrollment_request_digest` are respectively
`digest_v1("riot/pilot-invitation-envelope/v1",
canonical_cbor(PilotInvitationV1))` and
`digest_v1("riot/pilot-enrollment-request-envelope/v1",
canonical_cbor(PilotEnrollmentRequestV1))`.

An unspent valid credential creates one random participant pseudonym and signed
`PilotEnrollmentTokenV1`. Its canonical body binds only collector key ID, pilot
window, random pseudonym, window-scoped public key, the invitation's fixed role
flags, issuance, and expiry; the collector signs
`pilot_signature_preimage("riot/pilot-enrollment-token/v1", token_body)`.
Exact request replay returns the byte-identical token. Reuse with a different
key, idempotency key, or body returns `invitation_spent`; it cannot create
another pseudonym. The token contains no Riot identity or device identifier.

Exports are cumulative:

```text
PilotMetricsExportBodyV1 {
  enrollment_token,
  export_sequence: u64 in 1..=u64::MAX-1,
  random_128_bit_idempotency_key,
  cumulative_metrics
}

PilotMetricsExportV1 {
  body,
  client_signature = Ed25519.Sign(
    window_scoped_key,
    pilot_signature_preimage("riot/pilot-metrics-export/v1", body)
  )
}

PilotExportReceiptBodyV1 {
  pilot_window,
  keyed_token_commitment,
  export_sequence: u64,
  cumulative_metrics_digest,
  accepted_at
}

PilotExportReceiptV1 {
  body,
  collector_signature = Ed25519.Sign(
    collector_key,
    pilot_signature_preimage("riot/pilot-export-receipt/v1", body)
  )
}

PilotWithdrawalBodyV1 {
  enrollment_token,
  random_128_bit_idempotency_key,
  withdrawal_sequence: exactly 1,
  requested_at
}

PilotWithdrawalRequestV1 {
  body,
  client_signature = Ed25519.Sign(
    window_scoped_key,
    pilot_signature_preimage("riot/pilot-withdrawal/v1", body)
  )
}

PilotWithdrawalReceiptBodyV1 {
  pilot_window,
  keyed_token_commitment,
  withdrawal_request_digest,
  deleted_at,
  published_aggregate_limit
}

PilotWithdrawalReceiptV1 {
  body,
  collector_signature = Ed25519.Sign(
    collector_key,
    pilot_signature_preimage("riot/pilot-withdrawal-receipt/v1", body)
  )
}
```

`withdrawal_request_digest` is
`digest_v1("riot/pilot-withdrawal-request-envelope/v1",
canonical_cbor(PilotWithdrawalRequestV1))`; `keyed_token_commitment` is the
HMAC-SHA256 value above, not a public stable identifier.
Checked-in pilot vectors fix every canonical body, signature preimage, key ID,
request digest, valid signature, and one-bit-invalid signature.

The collector verifies token/signature/window and keeps only the highest
sequence for each pseudonym, replacing rather than summing prior cumulative
exports. Every counter in a higher sequence must be component-wise greater than
or equal to the prior accepted cumulative value; a decrease is rejected.
Distinct valid tokens count participants by role; replay, lower
sequence, conflicting same-sequence export, unknown fields, counter overflow,
or a value outside `u64` is rejected. An exact same-sequence/body/idempotency
replay returns the byte-identical signed `PilotExportReceiptV1` without
recounting. `cumulative_metrics_digest` uses
`digest_v1("riot/pilot-cumulative-metrics/v1", canonical_cbor(metrics))`.

Withdrawal uses its independent one-shot sequence value `1`; it does not need
to exceed the export counter, so export at its maximum cannot prevent deletion.
One transaction claims the
withdrawal idempotency key, verifies the active token/pseudonym, deletes the
highest export and denominator contribution, revokes the token and invitation,
and stores the canonical signed deletion receipt. Exact retry returns that
receipt; a different body/key is rejected. Export after withdrawal cannot
recreate the row. The collector retains only keyed digests and that bounded
receipt needed to reject reenrollment and replay, never the metrics, pseudonym,
token body, or public key.

Participants see the aggregate and explicitly export it. Anchor logs are not
joined to client metrics. Invitation/token/export/withdrawal request bodies,
source addresses, and transport metadata are never written to collector
application logs or joined to recruitment records. The collector applies
the anchor's accepted-socket, TLS 1.3/http1-only, handshake, 64-header/8-KiB
field-line, 8-KiB request-target, 16-KiB aggregate decoded-header,
five-second header-read, idle, and absolute-lifetime bounds, but has this
separate request
state machine: exactly one `POST application/cbor` request per connection on
enrollment, export, or withdrawal; exactly one decimal `Content-Length` in
`1..=65,536`; no transfer encoding, `Expect`, content/transfer compression,
duplicate length, or trailing bytes; and a ten-second body-read deadline.
It reads exactly the declared bytes into the preallocated body budget, closes
after its bounded response, and applies compiled handler/queue,
per-source/global rate, signature-CPU, and database-row ceilings before
verification. Tokens, spent/revoked invitation digests, and collector rows are
deleted after the published pilot report and its 30-day audit window.

Participant state is explicit and stored in protected local profile storage:

```text
PilotParticipantRecordV1 {
  pilot_window,
  collector_origin_and_key,
  state: PilotParticipationState,
  invitation: optional PilotInvitationV1,
  window_private_key: optional SecretKey,
  token: optional PilotEnrollmentTokenV1,
  roles: optional,
  cumulative_metrics: optional,
  highest_export_sequence: optional,
  pending_enrollment: optional {
    exact_signed_request,
    idempotency_key,
    send_state: NotSent | MayHaveBeenSent
  },
  pending_export: optional {
    exact_signed_request,
    sequence,
    idempotency_key,
    send_state: NotSent | MayHaveBeenSent
  },
  pending_withdrawal: optional {
    exact_signed_request,
    idempotency_key,
    send_state: NotSent | MayHaveBeenSent
  },
  last_export_receipt: optional PilotExportReceiptV1,
  withdrawal_receipt: optional PilotWithdrawalReceiptV1
}

PilotParticipationState =
  NotEnrolled |
  ImportingInvitation |
  AwaitingPilotConsent { roles, privacy_summary, expiry } |
  Declined { invitation_deleted: true, network_mutation: None } |
  PersistingEnrollment { invitation_digest, key_id, idempotency_key } |
  RecoveringEnrollment { signed_request, idempotency_key } |
  RecoverEnrollmentThenWithdraw { signed_request, idempotency_key } |
  EnrollmentRetryable {
    reason: Offline | Overloaded { retry_after } |
      LocalStorageBusy { blocked_by_operation_id },
    signed_request,
    idempotency_key
  } |
  EnrollmentTerminal {
    reason: Invalid | Expired | SpentByDifferentRequest |
      Revoked | CollectorRejected,
    local_invitation_and_key: Deleted
  } |
  Enrolled { token, roles, highest_export_sequence } |
  PreviewingAggregate { cumulative_metrics } |
  Exporting { sequence, cumulative_metrics_digest, idempotency_key } |
  ExportConfirmed { sequence, accepted_at } |
  ExportRetryable {
    reason: Offline | Overloaded { retry_after } |
      LocalStorageBusy { blocked_by_operation_id } | ResponseLost,
    signed_export
  } |
  CancelExportByWithdrawal { pending_export, signed_withdrawal } |
  ExportTerminal {
    reason: InvalidToken | TokenExpired | Revoked |
      SequenceExhausted | CollectorRejected,
    withdrawal_material: Retained
  } |
  Withdrawing { withdrawal_sequence, idempotency_key } |
  WithdrawalRetryable {
    reason: Offline | Overloaded { retry_after } |
      LocalStorageBusy { blocked_by_operation_id } | ResponseLost,
    signed_withdrawal
  } |
  WithdrawalTerminal {
    reason: InvalidSignature | CollectorRejected,
    local_material: Retained
  } |
  Withdrawn {
    receipt,
    confirmed_at,
    published_aggregate_limit,
    key_token_metrics: Deleted
  } |
  RetentionComplete { local_key_token_metrics: Deleted }
```

The record, not an individual enum variant, is the durable source of all
material required for its next transition. Every state validates a fixed
presence invariant: enrollment recovery has invitation/key/pending enrollment;
every enrolled/export state has key/token/roles/metrics/highest sequence;
export recovery additionally has the exact pending export; and every
withdrawal state has key/token plus the exact pending withdrawal. A transition
atomically updates state and material. No UI projection may own the only copy
of a key, token, signed request, metric aggregate, sequence, or receipt.

Before first enrollment I/O, Riot atomically stores the invitation, generated
window key, and idempotency key. Crash recovery repeats the exact signed
request. Response loss enters `RecoveringEnrollment`, never `Spent`; exact
replay recovers the token. Declining consent performs no network request and
deletes the imported invitation before returning to `NotEnrolled`.

Before export or withdrawal Riot stores the next checked sequence, exact signed
body, and idempotency key; retry is byte-identical. Retryable states retain that
material and expose the phase-specific accessible controls below. Enrollment terminal
states delete an unusable invitation/key. Export terminal states retain the
token/key/metrics so authenticated withdrawal remains possible. A confirmed
withdrawal deletes key/token/metrics and retains only its signed accessible
receipt. After a pilot report is published it cannot retract an already
published anonymous aggregate; the confirmation explains that limit. After the
audit window, `RetentionComplete` confirms the collector holds no participant
row and deletes all remaining local pilot secrets. Overload is always
retryable with verified timing, never a terminal pilot failure.

Cancellation is phase-specific and never destroys ambiguous credentials:

- before enrollment send, Cancel atomically deletes invitation/key/pending
  request and enters `Declined`;
- after enrollment may have been sent, Cancel enters
  `RecoverEnrollmentThenWithdraw`; Riot byte-identically recovers the token,
  persists it, signs/submits withdrawal, and reaches `Withdrawn`. Until that
  succeeds the key and exact enrollment request remain protected and the UI
  offers Retry or “Keep for automatic cleanup,” not destructive discard;
- after an export may have been sent, Cancel export enters
  `CancelExportByWithdrawal` and uses authenticated withdrawal to remove either
  possible collector contribution. It never sends a same-sequence export merely
  to discover whether cancellation is safe;
- after withdrawal may have been sent there is no destructive Cancel. Dismiss
  hides the sheet while protected automatic exact retry continues; reopening
  shows `WithdrawalRetryable` until the signed receipt is recovered.

An accepted export atomically stores its receipt, advances
`highest_export_sequence`, clears `pending_export`, and moves through
`ExportConfirmed` back to `Enrolled` after the participant acknowledges the
confirmation. A rejected definitely-not-accepted export clears its pending
request only for a terminal reason that proves no contribution exists.

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
  `first_launch_handoff` is explicitly a post-return proxy: the coordinated
  pilot procedure establishes that the participant began on the no-app page;
  Riot does not inspect browser history. It measures resumed handoff completion,
  not store conversion or pre-return abandonment, and needs no browser
  fingerprint or server-side correlation.

The collector derives totals from the highest accepted cumulative export per
token and publishes the denominator beside every percentage. A confirmed
`PilotWithdrawalRequestV1` performs the deletion/revocation transaction above;
Riot then deletes local metrics and retains only the accessible confirmation.

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

Renderer readiness runs an enforcement probe that confirms a distinct empty
network namespace, read-only root, quota-limited temporary mount, no published
tree mount, expected unprivileged UID/seccomp policy, and daemon ownership of
the serving tree. A declarative container setting without a passing probe does
not satisfy readiness.

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
  `after_removal_commit`, `after_checkpoint_work_planned`,
  `after_checkpoint_signed`, `after_checkpoint_temp_files_fsync`,
  `after_checkpoint_rename_before_parent_fsync`,
  `after_checkpoint_parent_fsync`,
  `after_checkpoint_pointer_floor_advance`,
  `after_covered_removals_terminalized`,
  `before_removal_terminal_delivery`,
  `during_incremental_checkpoint_reclaim`,
  `before_maintenance_wal_checkpoint`, `after_maintenance_wal_checkpoint`,
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
- accepted-socket/TLS/HTTP admission limiter;
- gossip scheduler;
- projection renderer;
- work-challenge verifier;
- profile storage worker/runtime lifecycle;
- pilot collector key/state store.

## TDD Delivery Slices

### Slice 1: Authority records and tickets

RED:

- same-version manifest equivocation is not quarantined;
- editorial capability incorrectly authorizes `/directory/listing`;
- max-revision delegate prevents root recovery;
- listing/ticket/manifest coordinates can disagree;
- ticket transport summary differs from the manifest;
- duplicate ticket fields, oversize anchor proofs, unsupported Arti floor, and
  v1/v2 downgrade are not rejected;
- a descriptor's operator key ID differs from its complete current verification
  key, or two pinned floors claim the same epoch with different digests/keys;
- Rust/Swift/Kotlin/TypeScript disagree on any domain label, canonical bytes,
  length prefix, body/envelope choice, or expected protocol identity digest.
- any implementation disagrees on `limit_profile_digest`, listing-receipt
  signature bytes, namespace-token HMAC input, or the distinction between the
  control-request and work-target digest bodies;
- any encoder uses a CBOR map, undocumented numeric field/enum tag, alternate
  operation discriminant, embedded-versus-byte-string substitution, or field
  order other than the normative positional grammar;
- either peer role signs a different transcript/ALPN/version representation,
  or any control success/refusal uses an alternate response envelope;

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
- a `sync/2` refusal uses an unknown code/detail pair or disagrees on
  retryability;
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
- canonical decode, re-encoding, and body digest comparison occur after an
  idempotency lookup or reveal a stored result to a same-key/different-body
  request;
- a reserved removal drops its request digest, or one key can claim both an
  ordinary and reserved result;
- crash at any SubmitListing/removal failpoint duplicates an inclusion, loses a
  receipt/result, or makes exact idempotent recovery impossible;
- listing becomes visible without atomically owning a reserved removal slot, or
  another root can consume that slot;
- recovery changes a frozen checkpoint timestamp, member order, head,
  signature, output name, digest, or covered-removal set;
- a caller receives removal success before the checkpoint pointer, logical
  floor, and all covered removal results advance atomically;
- concurrent same-base commits both win or the loser overwrites newer state;
- work proof replays against another body;
- a work stamp changes its nested challenge bytes, counter, proof, request key,
  root, operation kind, policy epoch, expiry, or null-stamp target without
  rejection;
- logical quota is bypassed by cross-community dedup;
- metadata-only requests exceed a persistent ceiling;
- client O/C/W state overflows the legacy evidence repository;
- `LocalProfile` retains a raw `EvidenceStore`/database path, an evidence or
  registry database command runs under `ProfileState`, or profile
  migration/backup/restore loses client-site stages or floors.

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
  exhaustion blocks an owner tombstone;
- a post-rotation cold peer cannot verify current-key snapshot records;
- a current-floor destination accepts an old hosting receipt without a
  current-key, connection-bound replica-source attestation, or accepts that
  attestation after the source generation changes;
- one challenge/attestation prepares twice, survives descriptor rotation, or
  compares source generation outside the immutable snapshot it streams.
- a prepared replica replays usable tokens after its peer session closes or
  either descriptor rotates, rather than terminalizing as
  `peer_context_changed`;
- startup reaches readiness with any crash-orphaned uncommitted replica
  preparation, token acceptance, staging row, or reusable peer-session
  generation;

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
- post-consent storage exhaustion or batch configuration recovery reports a
  false no-mutation result;
- a second local mutation queues ambiguously, local busy/commit recovery
  collapses into Internal, or a waiter survives profile close without a
  terminal result;
- an FFI `ImportPreview`/`ImportPlan` handle strongly owns core session or
  evidence storage, or a profile close tears down the process-global network
  runtime while another profile lease remains;
- 429/503 appears as zero results/invalid reference or loses the preserved web
  URL and accessible retry;
- `stale_source` retries invisibly/unboundedly or collapses into generic
  publish failure;
- one destination's source change, relist window, success, refusal, overload,
  or cancellation overwrites another destination's outcome;
- `removal_replay_window` appears as quota/failure, loses the signed listing
  intent, omits its exact retry time, or hides that hosting remains intact;
- a native resolved Cargo graph contains the anchor daemon, HTTP server, or
  renderer dependency closure;
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
- maximum-record removal fails after ordinary row/byte/WAL exhaustion;
- ordinary idempotency exhaustion blocks a valid removal or forged requests
  monopolize one root/all reserved verification permits;
- TLS handshake churn, slow ClientHello/header, keep-alive, unsupported method,
  Range, or connection lifetime bypasses a compiled ingress ceiling;
- anonymous HTTP search/feed/static floods exceed connection, queue, CPU,
  snapshot, rate, or response-byte ceilings or starve the reserved removal
  lane;
- 65 decoded header fields, a field line over 8 KiB, an over-cap reserved
  removal queue, or over-cap reserved canonical bytes are admitted;
- one pilot invitation creates two pseudonyms, changes role, or returns a
  different token on exact enrollment replay;
- collector `GET`, missing/duplicate/non-decimal/out-of-range content length,
  transfer encoding, compressed or trailing bytes, or a body that misses its
  ten-second deadline is admitted;
- export accepts sequence `u64::MAX`, or an export at `u64::MAX - 1` prevents
  the independent sequence-1 authenticated withdrawal;
- forged/replayed/overflow pilot signatures or unauthenticated withdrawal
  deletes, restores, or recounts a participant row;
- a replayed pilot export inflates participants/denominators or a
  subminimum/zero denominator is reported as pass.
- a higher cumulative export decreases a counter, post-I/O Cancel destroys the
  only enrollment/export/withdrawal recovery material, or an acknowledged
  export fails to advance durable participant sequence before returning;

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
- work-stamp golden vectors for exact nested challenge envelope, null-slot
  target digest, counter/proof, expiry, request key/root/kind/policy bindings;
- peer-proof reflection, replay, role swap, exporter mismatch, and stale hello;
- pre-auth peer descriptor refresh across same-key and changed-key rotations;
- forged predecessor verification key after a non-genesis pinned floor;
- ambiguous same-epoch descriptor floors with different digests or keys;
- every protocol identity golden vector plus body-versus-envelope mutation;
- exact listing-receipt signature, namespace HMAC, invitation-lookup HMAC,
  token-commitment HMAC, limit-profile digest, and work/control digest vectors;
- prepared idempotency replay before/after restart and expiry;
- normative CBOR grammar fixtures for every control operation, sync frame,
  signed body, sum variant, refusal, phase, and mode, with alternate maps,
  numeric tags, field orders, and nested-byte forms rejected;
- every control response success/refusal and `GetOperation` nested terminal
  outcome, plus both peer roles and the exact ALPN transcript/preimage;
- every structured refusal code/details pair and all three `GetOperation`
  states, with alternate full-response, outcome-only, payload-only, and
  byte-string nesting rejected;
- 64 requested maximum-size items split across ordered chunks;
- concurrent uploads for one site;
- two same-base site commits with one CAS winner and one `stale_base`;
- crash before and after admission commit;
- eviction during a read snapshot;
- sync disconnect between commit and receipt;
- `GetOperation` after restart;
- `GetOperation` during Prepare, sync, Commit claim, committed/refused
  terminal, expiry, retained-result deletion, and random unknown ID, with every
  alternate nesting rejected;
- directory feed fork/reorder and descriptor rollback;
- empty-feed, first-inclusion, and first-checkpoint encodings differ from the
  exact zero-digest sentinel contract;
- every closed `sync/2` refusal code/details shape, retry rule, and unknown
  pairing rejection;
- feed cursor before compaction floor and snapshot digest mismatch;
- cold checkpoint verification after several operator-key rotations;
- removals beyond reserved feed capacity using emergency checkpoint coalescing;
- maximum-size removal at ordinary metadata/database/WAL ceilings;
- listing-slot reservation races, two retained removal results, relist
  countdown/retry, and a later guaranteed owner removal;
- same removal key with exact/different digests across reserved and ordinary
  classes, proving stored results remain confidential;
- restart at every `CheckpointWorkV1` and publication failpoint, including
  rename before parent fsync, atomic floor/result advancement, terminal
  delivery loss, bounded reclamation, and maintenance WAL checkpoint;
- client pinned across several descriptor-key rotations;
- replica source attestation replayed on another connection/destination or
  after source generation changes;
- crash/restart with live prepared replica operations before every persisted
  phase, proving pre-readiness terminalization and session-generation advance;
- concurrent source commit between replica authorization and immutable
  snapshot acquisition;
- listing before hosting and hosting eviction after listing;
- gossip loops and final quiescence;
- unsupported `require:arti` ticket;
- all anchors unavailable;
- all embedded defaults disabled, removed, reset, or unreachable;
- zero and subminimum pilot denominators plus install-return minimums;
- replayed/lower/conflicting pilot export sequences and distinct role counts;
- replayed/altered pilot enrollment invitations and fixed-role enforcement;
- concurrent coordinator issuance against the one authoritative recruitment
  ledger;
- pilot crash recovery during enrollment/export/withdrawal plus unauthorized,
  exact-replay, offline, post-report, and post-retention withdrawal;
- pilot consent decline, local-storage busy, overload with retry timing,
  response loss, every enrollment/export/withdrawal terminal reason, retained
  withdrawal material, confirmed deletion, and retention completion;
- post-send enrollment Cancel-to-recover-and-withdraw, export
  Cancel-by-withdrawal, withdrawal sheet dismissal with exact automatic retry,
  and component-wise cumulative monotonicity;
- HTTP slow-header, rate, queue, database-snapshot, query-time, and oversized
  response exhaustion while a valid owner removal completes;
- TLS slow-handshake/resumption churn, keep-alive exhaustion, rejected
  HTTP/2/upgrade/method/Range/compression, and connection-level starvation;
- every removal failpoint from committed tombstone through frozen checkpoint,
  filesystem publication, atomic checkpoint/floor/Terminal result, lost
  delivery, exact retry, and separately bounded physical reclamation;
- local queue-full, busy retry, cancel-before-start, cancel-after-start,
  shutdown with queued/running work, and reopen-after-commit recovery;
- two simultaneous source-changed destinations, several independent relist
  windows mixed with completed/refused/unreachable anchors, per-row cancel, and
  cancel-all-waiting without receipt loss;
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
  repository through the single core-owned profile database lifecycle and
  schedule ongoing reconciliation;
- mobile internet operations share the process-singleton Rust-owned
  iroh/Tokio endpoint through revocable profile leases and cancellable UniFFI
  operations;
- `LocalProfile` reaches existing evidence, registry, and new site persistence
  only through the bounded core storage command port, with no database command
  or await under `ProfileState`;
- exactly one local mutation is admitted, every busy/cancel/recovery waiter has
  a typed terminal result, and explicit profile close revokes network/storage
  leases before bounded worker/database shutdown;
- anchors enforce manifest and Meadowcap authority before propagation;
- the root-signed ticket bootstrap requirement is checked before a cold public
  dial, and the digest-matched manifest requirement is checked before content
  admission or composite commit;
- `sync/2` routes and paginates all composite namespaces without the 64-ID
  inventory ceiling or 8 MiB response dead end;
- control operations are canonical, idempotent, recoverable, and typed;
- one positional-CBOR grammar and closed textual discriminants define every
  control/sync/signed-body encoding without implementation-chosen tags;
- exact work-stamp and peer-proof preimages plus versioned control
  success/refusal envelopes interoperate across independent implementations;
- empty feed/checkpoint sentinels, every `sync/2` refusal, and the complete
  Prepare-to-GetOperation lifecycle have one canonical encoding and recovery
  result;
- all signed coordinates and continuity/cursor identities match the checked-in
  domain-separated body/envelope digest vectors across implementations;
- host reconciliation keeps organizer state private until a destination
  receipt is delivered or recovered, while same-base destination commits use
  one site-generation CAS winner and return `stale_base` to every loser;
- operation IDs, idempotency keys, and authenticated work proofs cannot be
  replayed across bodies;
- stable anchor IDs, complete authenticated descriptor floors, current
  verification keys, current-key checkpoint snapshots, and prepared namespace
  tokens survive key rotation and restart;
- startup terminalizes every crash-orphaned peer-session-bound replica before
  readiness, and profile close never closes the process-shared endpoint;
- the anchor repository survives crash/restart and enforces logical/global
  quotas;
- per-listed-root reservation plus exclusive emergency metadata and WAL/fsync
  reserves permit a maximum-size owner removal and logical checkpoint/floor
  advance after ordinary capacity and idempotency rows are exhausted, with one
  crash-safe listing/feed/receipt/result lifecycle; physical reclamation and
  WAL truncation remain bounded post-acknowledgement maintenance;
- open hosting cannot exceed compiled process ceilings or trigger unbounded
  gossip;
- authenticated directory and replica gossip has a complete bounded wire
  lifecycle, channel-bound peer authentication, current-key connection-bound
  single-use source attestations, same-snapshot generation checks, and
  checkpoint recovery;
- public HTTPS remains within compiled accepted-socket, TLS handshake,
  connection-lifetime, request rate/queue, CPU, database-snapshot, query-time,
  and response-byte ceilings without consuming the reserved
  owner-removal/checkpoint lane;
- the text-only web renderer uses immutable ownership transfer and preserves
  the hostile-content boundary;
- deterministic three-anchor tests converge and survive anchor loss;
- typed post-consent local-storage failure and crash-safe single/batch host
  configuration recovery never report a false no-mutation outcome;
- plural publish and relist states preserve one durable outcome and retry clock
  per selected anchor without losing completed receipts;
- opt-in, pseudonymous, replay-safe cumulative exports count distinct pilot
  credentials from single-use idempotent enrollment, expose crash-safe
  participant UX and safe post-I/O cancellation, honor authenticated idempotent
  withdrawal, and prove the user-focused thresholds without correlated server
  logs;
- all quality and coverage gates pass.
