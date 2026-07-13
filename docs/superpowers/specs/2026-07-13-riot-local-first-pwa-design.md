# Riot local-first PWA vertical slice design

Date: 2026-07-13
Status: Revision 12 — final-round blocker corrections, review pending

## Purpose

Ship the smallest honest browser version of Riot: a single-public-community PWA
that can be opened from an ordinary URL, installed, reloaded offline, create a
locally signed update, preserve that update across reloads, and export/import
the same bounded `.riot-evidence` artifact used by the native core.

This is a vertical slice, not feature parity. It proves that the browser can be
a real local-first Riot client rather than a thin view onto a canonical server.
The static host distributes application code; the browser owns its identity and
accepted state. A future remote signer may recover the root identity and enroll
scoped local device keys through OAuth, but no remote signer is required or
implemented in this slice.

The browser is also the primary Riot node, not a thin client for a gateway. It
signs and commits locally before any synchronization attempt. Community identity
and history derive from authenticated records, never a domain name, gateway,
database, or renderer. Servers may improve discovery, retention, validation,
rendering, or availability, but no server is authoritative.

**Architectural invariant:** Community identity and authenticated validity are
permanent while at least one copy of the signed records survives; renderers are
disposable. A website is only one possible presentation of a community's
authenticated history. Community identity derives from signed records rather
than domain names or server ownership. Cryptography does not guarantee that a
copy remains available, that a renderer is complete, or that a renderer shows
verified records honestly.

## User and value

Primary user: a community organizer or participant who can reach Riot in a
browser before connectivity degrades and needs a usable local copy afterward.

Use cases:

1. **A participant wants to open and install Riot before an event so that they
   can still read their accepted public/community information when the network
   disappears.** When the application has loaded once, the shell and committed
   community state remain usable in an offline browser session.
2. **An organizer wants to publish a time-bounded update from a laptop so that
   nearby participants can carry a signed update without installing a native
   build.** After explicit review and **Post update**, the browser signs the
   exact canonical Willow entry that was reviewed and commits it through Riot's
   existing preview-first admission path.
3. **A participant wants to move information by file so that exchange still
   works when no compatible online peer is reachable.** Export downloads a
   single `.riot-evidence` public-data bundle. On a clean browser, import first
   previews one community's readable updates; acceptance creates a fresh member
   identity inside that communal namespace and makes it the selected community.
   In an existing community, imports must match its namespace.
4. **A returning participant wants the same local identity and accepted entries
   after a normal reload so that the PWA behaves like an application, not a
   disposable demo.** Browser-local encrypted identity material and an
   append-only accepted-bundle log rehydrate the Rust core on startup.
5. **A participant whose browser storage was cleared wants an honest recovery
   state so that Riot never implies missing local information is still safe.**
   The empty state explains that public-data import can restore carried updates
   but cannot restore the prior signer or organizer authority. It never silently
   creates a replacement identity and presents it as the prior one.
6. **A participant interrupted during review, signing, persistence, or an app
   update wants a deterministic return path so that they do not post twice or
   mistake volatile state for durable state.** Drafts survive reload; reviews
   are invalidated safely; unpersisted canonical bundles remain in a blocked
   recovery queue until retried or discarded with confirmation.

The slice succeeds when at least four of five representative first-time users
on each supported browser can complete open → create community → post update →
reload offline → see the same update → export in under three minutes without
coaching or a server-side account. The evaluation occurs before calling the
slice product-ready. It fails if signing or reload requires the network, reload
changes the signing identity, import bypasses preview/accept, any participant
mistakes public-data export for identity recovery, or two of five users abandon
the same step. A second timed checkpoint requires four of five first-time users
to import a supplied bundle, understand that they are joining with a new member
identity, select updates, and reach Home in under three minutes without
coaching. Results are recorded separately for Chromium and WebKit with elapsed
time, abandonment step, and whether each participant can explain that public
data import does not recover identity. The same evaluation asks each participant
to distinguish **Saved on this browser**, **Export prepared**, and **Exchanged**;
the slice fails if more than one of five describes a prepared download as
delivery to another node.

## Chosen approach

Use a framework-free PWA host plus a small `wasm-bindgen` adapter around a
shared ordinary-Rust `riot-client` controller and the `riot-core` protocol
implementation. `riot-client` additionally owns the versioned record-exchange
contract and transport-independent reconciliation coordinator. Browser-side
adapters perform actual I/O; neither the controller nor protocol code knows
whether records arrived through a file, a nearby peer, a direct community node,
an HTTPS gateway, or a metadata-resistant transport.

Alternatives rejected for this slice:

- Extending the Python public gateway would produce an installable reader but
  would not prove browser-owned state or signing.
- A conventional web API holding state and keys would be faster to scaffold but
  would make the host canonical and undermine Riot's shutdown-resistant model.
- Reimplementing Willow, Meadowcap, or bundle encoding in JavaScript would
  create a second protocol implementation without conformance evidence.
- A JavaScript-only transport contract would make each renderer define its own
  replication semantics. Riot instead keeps reconciliation and exchanged-record
  validation in shared Rust while leaving browser-specific I/O outside WASM.
- An async Rust transport trait called directly across `wasm-bindgen` would
  prematurely couple the MVP to browser networking and executor choices. The
  controller exposes bounded exchange operations; host adapters drive them.

## Scope

### In scope

- Framework-free responsive PWA shell at `apps/web/`.
- Exactly one local profile and one selected public communal community. A person
  either creates it as organizer or imports it and joins with a fresh member
  subspace; communal members may post under their own signer identity.
- The seeded Riverside demo is a test fixture and optional first-run `Try the
  demo` action. It uses the identical import-as-member path and never mixes with
  an existing community.
- Readable current-update listing with headline, body, author label, source,
  freshness, expiry, and AI-assisted status. Full IDs and cryptographic facts
  live behind a deliberate **Technical details** disclosure.
- Structured update creation, immutable exact-byte review, local Ed25519
  signing, and commit through a **Post update** action.
- Preview-first `.riot-evidence` file import and explicit acceptance.
- Export of one consolidated canonical bundle for the current community.
- Browser-local encrypted identity persistence and accepted-bundle persistence.
- Draft persistence, single-writer multi-tab behavior, and version-coherent PWA
  updates.
- Offline reload after one successful online load.
- A signer boundary whose only implementation is local in this slice but whose
  request/result contract can later support a remote recovery authority.
- A versioned, transport-neutral record-exchange contract owned by
  `riot-client`, with file import/export as the only concrete MVP transport.
- Explicit separation of protocol, durable storage, synchronization, and
  rendering so independent websites can later render the same authenticated
  community history.

### Out of scope

- Private groups, MLS, encrypted group drops, group rendezvous, or the explicit
  private↔public bridge.
- Nearby BLE, Bonjour, WebRTC, WebTransport, WTP, or live multi-peer sync.
- Direct community-node, HTTPS synchronization, Nym mixnet, automatic relay,
  discovery-directory, or background replication implementations.
- Remote OAuth, remote signing, remote identity-recovery UI, device enrollment,
  or capability revocation.
- Hosted user accounts, server-side canonical state, analytics, notifications,
  background sync, or push.
- Arbitrary third-party miniapp installation. Existing bundled miniapps remain
  separate from this proof.
- Deployment, DNS, TLS, and production hosting configuration.
- Viewer/publisher/replicator mode selection in the UI; the MVP acts as one
  local publisher/reader and does not retain history for other nodes.
- Identity backup or recovery. `.riot-evidence` export contains public community
  data only; remote recovery is explicitly future work.

## Architecture

```text
static host
  index.html + app.js + styles.css + manifest + service worker
                              |
                              v
browser UI ---- typed JS host/controller ---- riot-web WASM adapter
   |                         |                     |
   |                         |                     v
   |                         |            riot-client shared controller
   |                         |        state + replication coordinator
   |                         |                     |
   |                         |                     v
   |                         |                 riot-core
   |                         |        Willow / Meadowcap / bundles /
   |                         |        preview-plan-commit admission
   |                         |
   |                         +---- BrowserVault (IndexedDB + WebCrypto)
   |                         +---- BundleLog (IndexedDB, append-only)
   |                         +---- TransportPortV1 (browser I/O only)
   |                                   |
   |                                   +---- FileExchangeTransport (MVP)
   |                                   +---- Nearby (deferred)
   |                                   +---- Direct community node (deferred)
   |                                   +---- HTTPS gateway (deferred)
   |                                   +---- Nym mixnet (deferred)
   |
   +---- service worker cache (one content-versioned release only)
   +---- Web Lock (`riot-profile-writer`) for single-writer tabs
```

The architecture has four replaceable layers:

1. **Protocol** authenticates canonical records and authority in `riot-core`.
2. **Storage** retains browser-owned identity, drafts, accepted records, and
   projections locally.
3. **Synchronization** reconciles and exchanges signed records through a
   transport adapter after local commit.
4. **Rendering** projects verified local state into one replaceable website or
   application surface.

A domain is not part of a community identifier. Two independent static sites
may load the same signed records and render the same authenticated community
without sharing a database, account system, or operator. A renderer may be
stateful locally, but it is disposable from the community's perspective.

The intended service worker caches no mutable user data and contains no signer
API. That is code organization, not a security boundary: any compromised
same-origin page or worker can access IndexedDB and invoke WebCrypto. This
public-data prototype accepts that host-compromise limitation explicitly.

IndexedDB is the persistence boundary. On startup, the JS host acquires an
exclusive `riot-profile-writer` Web Lock for the page lifetime. If another tab
holds it, this tab opens a read-only view labeled **Riot is open in another
tab**; it cannot create a profile, post, accept an import, update storage, or
activate a new release. Browsers without Web Locks show `UNSUPPORTED_BROWSER`.
After acquiring the lock, the host cursor-validates and boundedly stages the
vault and complete bundle log, then gives both to the controller's atomic restore transition. That transition
opens the sealed identity, creates a fresh in-memory store, and replays accepted
canonical bundles through the same inspect → plan → commit path used for new
imports before enabling mutation. No serialized internal Rust store is trusted.

### Rust/WASM adapter

A new `riot-client` ordinary Rust library separates signer-independent community
state from publisher authority:

- `CommunityReplica` owns the selected namespace, verified accepted store,
  deterministic projection, replay, import/export, and bounded reconciliation.
  It has no signer and can power a future Viewer or Replicator.
- `PublisherAuthority` owns the sealed author, organizer/member relationship,
  immutable prepared reviews, and local signing. It attaches to a replica but is
  not required to verify or render one.
- `RiotClientController` composes one replica with `PublisherAuthority|null` and
  owns pending-profile, pending-bundle, and replication-session transitions.
  The MVP always attaches publisher authority after create or join; a mode
  selector and signer-free viewer UI remain deferred.

Existing `riot-ffi` becomes the native UniFFI adapter over those components; a
new `riot-web` `cdylib`/ordinary library becomes the thin `wasm-bindgen` adapter
over the same controller. `riot-web` does not depend on UniFFI and neither
binding implements controller state. All state, namespace admission,
signer-independent projection, and error mapping therefore have one
implementation.

Public operations:

- `create_community(title) -> CommunityCreatedV1`
- `restore_community(wrapping_key_bytes, sealed_identity_bytes, community, restore_log) -> CommunityV1`
- `preview_new_community(bundle_bytes) -> CommunityImportReviewV1`
- `join_reviewed_community(review_id, selected_entry_ids, local_title) -> CommunityJoinedV1`
- `preview_import(bundle_bytes) -> ImportReviewV1`
- `accept_import(review_id, selected_entry_ids) -> PendingBundleV1`
- `prepare_update(UpdateDraftV1) -> UpdateReviewV1`
- `post_review(review_id) -> PendingBundleV1`
- `list_updates() -> UpdateV1[]`
- `export_community() -> BundleArtifactV1`
- `confirm_profile_persisted(pending_profile_id) -> CommunityV1`
- `abort_pending_profile(pending_profile_id) -> Result<(), WebErrorV1>`
- `acknowledge_bundle_persisted(sha256) -> Result<(), WebErrorV1>`
- `open_replication_session(role: ReplicationRoleV1) -> ReplicationSessionV1`
- `begin_replication(session_id) -> ReplicationOutcomeV1`
- `receive_replication_frame(session_id, frame_bytes) -> ReplicationOutcomeV1`
- `take_replication_frame(session_id) -> Uint8Array|null`
- `accept_replication_import(session_id, review_id) -> PendingBundleV1`
- `acknowledge_replication_bundle_persisted(session_id, sha256) -> ReplicationOutcomeV1`
- `reject_replication_import(session_id, code) -> ReplicationOutcomeV1`
- `close_replication_session(session_id) -> Result<(), WebErrorV1>`
- `close() -> Result<(), WebErrorV1>`

The ordinary-Rust `CommunityReplica::restore_from_log(community, restore_log)`
and `CommunityReplica::project()` APIs are signer-free and receive direct host
tests in this slice. The web adapter does not expose a new Viewer onboarding
flow yet; later Viewer UI is an adapter/product addition, not a rewrite of
community verification or projection.

`create_community` creates one organizer-shaped author with
`generate_space_organizer_author`; its namespace equals its signer/subspace.
`restore_community` is one atomic publisher-controller transition. Before it returns, no
mutation operation is available: it restores the sealed author, requires the
caller-supplied community and manifest namespaces to equal the namespace sealed
into that author, requires the caller-supplied signer ID to equal the sealed
signer, and replays every ordered record with its fixed admission route. A zero-record
organizer log is valid. A member profile must have at least its atomic join
bundle, so a zero-record member log is `REPLAY_FAILED`; every record present
must authenticate the same namespace. Any failure closes the partial session.
Only `local_title` is editable. It verifies organizer
equality before exposing organizer behavior. A restored member has
`namespace != signer/subspace`; relationship is always derived from that fact,
never trusted from the editable local community record. The record's title is a
local label, not signed authority. `join_reviewed_community` first validates a
single communal namespace and every displayed entry without mutation, then
generates a fresh member author with `generate_communal_author_for_namespace`.
Members may publish under their own subspace but never receive organizer-only
authority. An active profile rejects imports from every other namespace.

Create and first-run join return a `pending_profile_id` and leave the controller
in a state where only confirm, abort, and close are callable. The host confirms
only after the complete profile transaction commits. Post and existing-community
import similarly block further mutation until the host acknowledges the exact
bundle hash after its IndexedDB transaction; acknowledgement before persistence
is forbidden by the host state machine. This makes the durable boundary
explicit instead of relying on UI timing.

The Riverside fixture goes through `preview_new_community` and
`join_reviewed_community` with the local title `Riverside Tenants Union`. There
is no privileged demo admission path in the PWA.

### Transport-independent replication boundary

Riot already has the seed of the boundary in `riot-core`: `ReconcileSession`
owns no transport, and `ByteSyncSession` converts its state machine to opaque
canonical frames. It is not yet an unbounded history-replication protocol. The
MVP coordinator deliberately preserves its current window: one selected
namespace, at most `MAX_SYNC_IDS = 64` current live entries, and at most one
`MAX_BUNDLE_BYTES = 8 MiB` entries bundle. If the current live inventory exceeds
that window, opening returns `REPLICATION_WINDOW_EXCEEDED` with zero state
change. Pagination, tombstone/history partitioning, and Replicator retention are
future protocol work that must not change signed record formats.

`riot-client` owns a `ReplicationCoordinator` around one byte-only session. The
MVP permits exactly one active replication session, one bounded outbound frame,
one bounded inbound frame, and one pending import. Its inventory contains only
the current live, verified Riot alert/update entries supported by this PWA.
Profile, app-data, and app-index entries are excluded before ID summary and
cannot appear in an accepted entries frame; a received entries bundle containing
any undisclosed/non-alert frame fails the whole exchange. This keeps every
replicated entry human-readable in `ReplicationImportReviewV1`. Extending sync
to additional record classes requires a later design with class-specific review
and projection; no opaque record is silently committed. The coordinator does
not exchange every historical bundle in the append-only browser replay log. The
accepted log remains the durable source from which live alerts are rebuilt; it
is not advertised as a complete Replicator feed. Names remain renderer-local
decoration outside `CommunityProjectionV1`; sync does not invent or trust them.

This alert-only view is a new, explicit exchange session policy,
`org.riot.alert-live-set/1`, selected and matched before either side passes a
frame to `ByteSyncSession`. Its inner frames deliberately retain the existing
`org.riot.conference-sync/1` codec and are therefore syntactically wire-compatible
with legacy complete-inventory sync, but the inventory semantics are not
interoperable. Every future duplex adapter must negotiate the outer exchange
policy before routing any inner `Hello`; it must refuse a legacy/unnegotiated
peer rather than letting syntactic compatibility bypass the policy. A missing or
different policy fails before `Hello`; entry IDs never infer compatibility. No
live adapter or cross-native interoperability claim ships in this MVP. Future
native/browser transports either adopt the alert policy or negotiate another
versioned policy with its own complete review/projection rules; transport choice
still does not alter signed record formats.

Each coordinator session has only the fixed capability `duplex-frames`, the
fixed `org.riot.alert-live-set/1` exchange profile, and a protocol role,
`initiator|responder`; it never receives a concrete transport kind. The trusted
browser host binds its returned session ID exactly once to a
locally selected immutable port/kind ID. The remote peer and received
bytes cannot set or alter that host binding. There is no silent fallback:
failure of `nym` or any future metadata-resistant adapter may not transmit
through direct-node or HTTPS without closing the session and making a new
explicit local user/policy decision. Transport policy and record authenticity
are separate concerns.

The one replication state enum and legal transitions are:

```text
idle(initiator) --begin--> frame-ready --take-frame--> awaiting-frame
idle(responder) --receive Hello--> frame-ready --take-frame--> awaiting-frame
awaiting-frame --receive summary/request--> frame-ready
awaiting-frame --receive entries--> import-review-required
awaiting-frame --receive Complete--> complete
any frame-accepting state --receive Reject--> rejected
import-review-required --reject--> final-frame-ready
import-review-required --accept all reviewed entries--> awaiting-persistence
awaiting-persistence --ack exact hash--> frame-ready | final-frame-ready | complete
final-frame-ready --take-frame--> complete
any nonterminal except awaiting-persistence --close/cancel--> closed
malformed/limit/internal failure--> failed
```

`frame-ready` forbids another begin, receive, accept, or reject until the caller
drains the exact outbound frame. `final-frame-ready` permits only take or close;
it becomes complete after its exact final frame is drained. `rejected`,
`complete`, `closed`, and `failed` are terminal. Only an initiator
may call `begin`; only a responder may accept `Hello` from `idle`. Simultaneous
initiation is a deterministic `REPLICATION_UNEXPECTED_FRAME`, not an implicit
role change. A future duplex adapter assigns roles from its explicit
connection/session setup (for example outbound is initiator and accepted inbound
is responder); role negotiation is not inferred from unauthenticated bytes.
Transport cancellation before core commit closes the session with no community
mutation. The browser keeps no background/idle session in this release; a future
network adapter must add its own bounded timeout and retry policy before use.
`close_replication_session` and session cancellation are forbidden in
`awaiting-persistence` and return `PERSISTENCE_PENDING` with zero state change.
Only exact-hash durability acknowledgement or confirmed `close()` of the entire
controller can leave that state. Controller close discards the quarantined
in-memory store and requires rebuild from the prior durable log; a session-only
close can never expose or retain the core-committed pending entries.

An incoming entries frame follows this exact durability sequence:

1. `ByteSyncSession` verifies the namespace, canonical frame, exact requested
   IDs, signatures, capabilities, and bundle ceilings, but retains its pending
   entries without calling `import_accepted`.
2. `ReplicationCoordinator` performs ordinary inspect and creates a
   `ReplicationImportReviewV1` only if decoded count, exact requested/pending ID
   count, valid row count, and eligible row count are equal; there are zero
   rejected, unsupported, duplicate, or non-alert rows; and prospective
   inventory validation proves the durable live set plus every pending entry
   remains within 64 IDs and 8 MiB. The prospective plan's entry IDs must equal
   the pending IDs exactly. Any failed equality rejects the whole session before
   an accept action exists. Duplex sync is all-or-reject: unlike file import, it
   does not permit a selective subset that would disagree with the requested
   frame or with `ByteSyncSession::pending_entries`.
3. `accept_replication_import(session_id, review_id)` rechecks session/review
   liveness, commits through plan-all, and returns an exact normalized
   `PendingBundleV1` while entering `awaiting-persistence`. It does not advance
   reverse inventory or emit peer acknowledgement.
4. One IndexedDB transaction appends that bundle with the fixed admission route
   `web-record-exchange`, updates the manifest, and records an optional local
   `TransportReceiptV1` outside protocol admission.
5. Only `acknowledge_replication_bundle_persisted(session_id, exact_sha256)`
   calls `ByteSyncSession::import_accepted`, installs the new live inventory,
   and produces the next frame or completion.

Persistence failure leaves the controller in `awaiting-persistence`, blocks all
other mutation, and uses the same Retry/Close-without-saving recovery
queue as a local post. Retry persists and acknowledges the same bytes/hash.
Close-without-saving closes the controller so startup rebuilds only the prior
durable log; no acceptance frame was sent, so a peer may safely retry. A crash
has the same effect. Rejecting before commit calls `import_rejected`; accepting
or rejecting a stale/wrong review or wrong hash changes nothing.

The controller never opens a socket, resolves a URL, selects a gateway, invokes
Web Bluetooth, or imports a Nym package. `riot-client` is the authoritative owner
of exchange-profile, envelope, state-transition, limit, and error semantics.
The JS browser host implements only asynchronous registry/port I/O against that
contract and may not redefine it per renderer.

Two immutable transport capabilities avoid conflating files with peer sessions:

- `bundle-carrier` moves one canonical `.riot-evidence` bundle with no session
  ID or handshake. `file` is the sole MVP implementation and always uses the
  existing selective preview/import or consolidated export paths.
- `duplex-frames` moves `TransportEnvelopeV1 { version, session_id,
  exchange_profile, payload_kind: sync-frame, payload_bytes }`. Nearby, direct-community-node,
  HTTPS-gateway, and Nym are reserved kinds with no implementation or UI in the
  MVP; deterministic in-memory tests drive this capability only.

Every adapter checks a file/declared message/stream length before aggregation
where the browser API permits it, rejects bundles above `MAX_BUNDLE_BYTES`,
rejects frames above `MAX_SYNC_FRAME_BYTES`, and avoids a second unbounded copy
before WASM. APIs such as WebSocket that allocate a complete message before the
handler cannot provide preallocation protection; a future adapter must document
that residual risk, close immediately on oversize, and add rate/concurrency
policy. The MVP allocates no network adapter and enforces one session/buffer.

`TransportPortV1` has one operation state enum:

```text
unavailable (no registered implementation)
ready -> opening -> exchanging -> ready
ready|opening|exchanging --cancel--> ready
ready|opening|exchanging --close--> closed
adapter/protocol failure --> failed
```

The browser-owned interface is deliberately small and transport-neutral:

```text
TransportRegistryV1.describe(kind_id) -> { capability, availability }
TransportRegistryV1.open(kind_id, local_config) -> async TransportPortV1
TransportRegistryV1.bind(session_id, port) -> immutable local binding
TransportPortV1.kind_id() -> bounded string
TransportPortV1.capability() -> TransportCapabilityV1
TransportPortV1.state() -> TransportPortStateV1
TransportPortV1.receive() -> async TransportEventV1
TransportPortV1.send(envelope: TransportEnvelopeV1) -> async TransportSendResultV1
TransportPortV1.cancel() -> async TransportSendResultV1
TransportPortV1.close() -> async TransportSendResultV1
```

`closed` and `failed` are terminal port instances; retry constructs a new port.
Each `TransportEventV1` and `TransportSendResultV1` carries a closed
`TransportFailureCodeV1|null`; `message_key` is localization-only and UI logic
never branches on it. One port operation is in flight at a time. A
`bundle-carrier` accepts only `bundle` envelopes with `session_id = null`; a
`duplex-frames` port accepts only `sync-frame` envelopes with its bound session
ID. Mode mismatch fails before bytes cross into the controller. A `file` port
cannot bind a replication session and returns `TRANSPORT_MODE_MISMATCH` in the
host; file import/export never masquerades as a live replication session.

`TransportEnvelopeV1.session_id` and `exchange_profile` are host-local routing
metadata, not peer wire fields. On send, the port verifies the locally bound
session/profile and strips both before its concrete adapter transmits the opaque
canonical `payload_bytes`; adapter-specific setup must have already negotiated
the same exchange profile. On receive, that established port wraps peer payload
bytes with its own local bound session ID/profile before returning the event.
Independent peers never compare or share controller session IDs. A future
network adapter must separately specify its connection/handshake identifier and
profile-negotiation bytes; this MVP defines neither because it ships no duplex
adapter.

Transport kind, endpoint, first-seen time, delivery attempt, and availability
are optional local receipt facts. They never enter signed payloads, entry IDs,
community identity, core admission budgeting, authority, trust scoring,
capability decisions, projection ordering, or completeness claims. Core import
uses the single fixed `web-record-exchange` admission route for every
external adapter; actual provenance lives only in the browser manifest's local
receipt. Therefore identical received bytes have identical store-charge and
admission outcomes across file, nearby, direct node, HTTPS, and Nym. First-receipt
retention is informational and attacker-influenceable.

Browser-manifest `kind_id` is 1–64 lowercase ASCII bytes matching
`[a-z0-9][a-z0-9.-]*`. `endpoint_label`, when a future adapter supplies one, is
at most 128 UTF-8 bytes after removing user-info, query strings, fragments, and
control characters; bearer tokens, cookies, OAuth codes, full Nym addresses,
and other credentials are never retained. Receipt parse/size failure drops the
receipt, not the already verified record.

Only registered implementations may produce UI. Nearby, direct-node, HTTPS, and
Nym do not appear as disabled controls or failed connections in this release.
File UI distinguishes three facts: **Saved on this browser** after durable local
commit; **Export prepared** after the browser accepts download initiation; and
**Exchanged** only after a future peer transport completes exchange of
authenticated records. File export never claims another node received the data. With file as
the only adapter, Home shows offline availability and that exchange is manual,
not a meaningless online/offline transport indicator.

Publishing therefore has a strict order:

```text
review exact canonical bytes
        ↓
sign
        ↓
commit and persist locally
        ↓
report local durable success
        ↓
synchronize later through any available transport
```

Network failure can delay dissemination but cannot roll back publication to the
local community history or make the signature invalid. The accepted bundle log
is the source for later synchronization; the MVP does not need a second
gateway-shaped upload queue.

Planned transport roles are:

- **Nearby peer exchange:** short-range browser/native exchange where platform
  support permits it.
- **Direct community node:** a deliberately chosen node that retains and serves
  signed community records.
- **HTTPS gateway:** a disposable cache, validator, renderer, discovery
  directory, or synchronization endpoint; never the owner of a community.
- **Nym mixnet:** a metadata-resistant browser adapter using Nym's browser
  TypeScript/WASM SDK; it carries the same opaque bounded exchange bytes and is
  neither mandatory nor part of the Riot protocol.
- **File exchange:** human-carried canonical bundles, implemented in this MVP.

Official Nym materials describe its
[TypeScript SDK](https://www.nym.com/glossary/nym-typescript-sdk) as supporting
browser-based mixnet applications and its
[browser client](https://nym.com/blog/introducing-the-nym-sdk-powerful-privacy-served-directly-to-your-browser)
as WebAssembly running in a worker.
That makes Nym a plausible future host adapter, not a reason to add Nym types or
dependencies to `riot-core`, `riot-client`, or this release. The future design
must separately threat-model message-size leakage, timing, cover traffic,
recipient addressing, SDK supply chain, browser worker policy, availability,
and the fact that metadata resistance does not make received claims true.

### Deployment roles and disposable renderers

Future deployments may choose one of three local capability profiles without
changing community identity or record formats:

- **Viewer:** receives, persists a bounded local copy for offline use, verifies,
  projects, and renders authenticated records without a signer. It does not
  serve that copy to strangers merely because it viewed it.
- **Publisher:** a Viewer with attached `PublisherAuthority`; it reviews exact
  bytes, signs, commits locally, and can offer its records to transports.
- **Replicator:** a Viewer or Publisher whose operator explicitly enables a
  separately bounded retention-and-serving policy. This operational capability
  is independent of organizer/member signing authority and is never imposed on
  every viewer.

The MVP is a publisher that reads its own selected community and exchanges by
file. It does not implement a mode selector or serve other peers. The separation
is architectural so a later static deployment containing only HTML, JS, WASM,
and an optional transport adapter can render authenticated history without a
central database. Sites such as `location.indymedia.org`,
`berlin.indymedia.org`, `berlin.protest.net`, an archive, or a friend's host
could independently present the same records; none of those domains becomes the
community's root of trust.

### Immutable update review

`riot-core` gains a `PreparedAlert` split underneath the existing
`create_signed_alert` convenience function:

1. `prepare_alert` allocates object/revision IDs, captures the clock snapshot,
   validates and canonically encodes the payload, builds the complete entry and
   capability, and retains the entry object plus exact canonical entry,
   capability, and payload bytes without signing.
2. The shared controller binds the prepared object to a single-use review ID,
   current namespace, full signer ID, store generation, and
   `SHA256("riot/update-review/v1" || u32be(entry length) || entry bytes || u32be(capability length) || capability bytes || u32be(payload length) || payload bytes)`.
3. `UpdateReviewV1` renders exactly those retained semantic fields, including
   destination community, acting identity, created time, expiry, source claims,
   and digest in Technical details.
4. `post_review` rechecks review liveness, namespace, signer, store generation,
   and expiry. Ed25519 receives exactly the retained canonical Willow entry
   bytes as its sole message, matching Meadowcap
   `AuthorisationToken::new_for_entry`; capability and payload bytes are
   digest-bound review evidence but are not concatenated into the signature
   message. The controller builds the token from the retained capability object
   and returned signature, verifies it against the retained entry and expected
   signer, and rejects any mismatch. It never allocates new IDs, captures a new
   creation timestamp, or re-encodes editable fields. Any mismatch returns
   `REVIEW_STALE` and requires a new review.
5. The one-entry bundle uses the exact reviewed entry, capability, and payload
   byte arrays plus the verified signature; none is reconstructed from UI data.
   It commits through ordinary inspect → plan → commit. Existing
   `create_signed_alert` becomes
   prepare-then-sign internally so native behavior and test vectors do not fork.

This removes the review/sign time-of-check/time-of-use gap and provides the
exact-byte boundary a future remote signer will need.

### Versioned browser DTOs

Bytes cross as `Uint8Array`. Unsigned 64-bit times cross as decimal strings in
Unix seconds; entry IDs, namespace IDs, signer IDs, and digests are complete
lowercase hex strings. Closed enums use the existing lowercase names
`immediate|expected|future|past|unknown`,
`extreme|severe|moderate|minor|unknown`, and
`observed|likely|possible|unlikely|unknown`.

| DTO | Required fields |
| --- | --- |
| `CommunityV1` | `version`, `namespace_id`, `local_title`, `publisher: PublisherIdentityV1|null` |
| `PublisherIdentityV1` | `version`, `relationship` (`organizer|member`), complete `signer_id` |
| `ProposedCommunityV1` | `version`, `namespace_id`, `relationship` (always `member`) |
| `CommunityCreatedV1` | `version`, `pending_profile_id`, `community`, `wrapping_key_bytes`, `sealed_identity_bytes` |
| `CommunityImportReviewV1` | `version`, `review: ImportReviewV1`, `proposed_community: ProposedCommunityV1`, `suggested_local_title` |
| `CommunityJoinedV1` | `version`, `pending_profile_id`, `community: CommunityV1`, `wrapping_key_bytes`, `sealed_identity_bytes`, `accepted_bundle: PendingBundleV1` |
| `TransportReceiptV1` | browser-manifest-only: `version`, bounded `kind_id`, bounded credential-free `endpoint_label: string|null`, `first_seen_at`, `attempt: decimal-string|null` |
| `RestoreRecordV1` | `version`, `bundle_bytes: Uint8Array`, `admission_route: AdmissionRouteV1`, `transport_receipt: TransportReceiptV1|null`, `sha256` |
| `RestoreLogV1` | `version`, `namespace_id`, `total_bytes`, ordered `records: RestoreRecordV1[]` |
| `UpdateDraftV1` | `version`, `headline`, `description`, `language`, `urgency`, `severity`, `certainty`, `valid_from: decimal-string|null`, `expires_at`, `affected_area: string|null`, ordered `source_claims: string[]`, `ai_assisted` |
| `ProjectedUpdateV1` | signed semantic fields plus complete `entry_id`, `namespace_id`, `signer_id`, `key_tag`, `created_at`, `signature_valid`, `capability_valid`; no resolved name, receipt, durability, local-person pronoun, or renderer state |
| `CommunityProjectionV1` | `version`, complete `namespace_id`, deterministic ordered `updates: ProjectedUpdateV1[]` |
| `UpdateV1` | `version`, `projected: ProjectedUpdateV1`, rendered local `author_label`, `durability`, `transport_receipt: TransportReceiptV1|null` |
| `ReviewedUpdateV1` | draft fields plus allocated `object_id`, `revision_id`, `entry_id`, `namespace_id`, `signer_id`, `created_at` |
| `UpdateReviewV1` | `version`, `review_id`, `update: ReviewedUpdateV1`, `community: CommunityV1`, `acting_identity: PublisherIdentityV1`, `acting_author_label`, `canonical_digest` |
| `TechnicalDetailsV1` | `version`, complete `entry_id`, `namespace_id`, `signer_id`, `payload_sha256`, `signature_valid`, `capability_valid` |
| `ImportRowV1` | `version`, `headline`, `description`, deterministic `author_label`, ordered `source_claims: string[]`, `created_at`, `expires_at`, `ai_assisted`, `selectable`, fixed `rejection_code|null`, `technical: TechnicalDetailsV1` |
| `ImportReviewV1` | `version`, `review_id`, `namespace_id`, `byte_count`, ordered `valid_rows: ImportRowV1[]`, ordered `rejected_rows: ImportRowV1[]`, `selected_entry_ids: string[]` |
| `ReplicationImportReviewV1` | `version`, `session_id`, `review: ImportReviewV1`; all eligible rows are selected and selection cannot be edited |
| `PendingBundleV1` | `version`, exact accepted-only `bundle_bytes: Uint8Array`, `admission_route: AdmissionRouteV1`, complete `entry_ids: string[]`, `sha256` |
| `BundleArtifactV1` | `version`, canonical `bundle_bytes: Uint8Array`, complete `entry_ids: string[]`, `sha256`, deterministic `filename` |
| `TransportEnvelopeV1` | `version`, `capability: TransportCapabilityV1`, `session_id: string|null`, `exchange_profile: string|null`, `payload_kind` (`bundle|sync-frame`), bounded `payload_bytes` |
| `ReplicationSessionV1` | `version`, opaque `session_id`, complete `namespace_id`, `exchange_profile` (fixed `org.riot.alert-live-set/1`), `role: ReplicationRoleV1`, `state: ReplicationStateV1` |
| `ReplicationOutcomeV1` | `version`, `session_id`, `state: ReplicationStateV1`, `review: ReplicationImportReviewV1|null`, `rejection_code: string|null` |
| `TransportEventV1` | `version`, `kind` (`envelope|closed|failed`), `envelope: TransportEnvelopeV1|null`, `failure_code: TransportFailureCodeV1|null`, `message_key: string|null` |
| `TransportSendResultV1` | `version`, `status` (`sent|cancelled|unavailable|failed`), `failure_code: TransportFailureCodeV1|null`, `message_key: string|null` |
| `WebErrorV1` | `version`, stable `code`, `field: string|null`, `message_key`; never raw parser/debug text |

Every public call returns `Result<T, WebErrorV1>`. The named closed enums are:

- `AdmissionRouteV1 = web-local-post|web-record-exchange`. It is fixed by the
  operation, never supplied by an adapter; every external file or peer record
  uses `web-record-exchange`.
- `TransportCapabilityV1 = bundle-carrier|duplex-frames`.
- `ReplicationRoleV1 = initiator|responder`.
- `ReplicationStateV1 = idle|frame-ready|final-frame-ready|awaiting-frame|import-review-required|awaiting-persistence|rejected|complete|closed|failed`.
- `TransportPortStateV1 = unavailable|ready|opening|exchanging|closed|failed`.
- `TransportFailureCodeV1 = cancelled|unavailable|mode-mismatch|oversize|io|timeout|protocol`.

Startup replay accepts only the two fixed admission values. Transport kind IDs
exist only in the browser registry/manifest as local provenance and session
policy, never in `riot-client` or core admission. The initial registered IDs are
`file`, `nearby`, `direct-community-node`, `https-gateway`, and `nym`; a future
adapter can add a bounded ID without changing protocol/controller types.
Every opaque import, review, replication, or pending-profile ID is single-use,
session-bound, and rejected after replacement, commit/abort, or close.

Every nested DTO also carries `version: 1`; arrays are always present, even
when empty, and nullable fields are JSON `null`, never omitted. `byte_count` and
`total_bytes` are safe non-negative JavaScript numbers bounded below 16 MiB.
`durability` is the closed enum
`durable|not-saved`; `rejection_code` is null for selectable rows and otherwise
one of `invalid-signature|invalid-capability|wrong-community|malformed-update|expired|unsupported-entry-type`.
`CommunityReplica::project()` is a pure, signer-free projection over verified
accepted records. It deterministically sorts updates and returns structured
`ProjectedUpdateV1` data; it contains no resolved profile name, domain,
endpoint, transport receipt, durability, current-user marker, or localized
phrase. The adapter derives the
mandatory key tag as the first four signer/subspace bytes rendered as eight
lowercase hex characters. The browser renderer may label the attached local
publisher as `You` and otherwise use `Community member`;
`author_label` is `<rendered name> · <key tag>`. Role is shown separately.
Complete signer IDs remain in Technical details. Creating or editing display
names is not part of this slice. `suggested_local_title` is the literal
`Imported community`; it has no authority meaning and may be edited before join.

Projection order is `created_at` descending, then complete `entry_id` bytes
ascending as the total tie-breaker. `CommunityReplica::project_canonical_bytes()`
encodes the same structure with the versioned
`org.riot.community-projection/1` canonical CBOR codec: definite-length maps and
arrays, unsigned integer field keys in ascending order, UTF-8 text, byte strings
for complete binary IDs/digests, and no floating-point or optional omitted
fields (`null` is explicit). Its authoritative schema is:

```text
CommunityProjectionV1 = {
  0: 1,                         # unsigned version
  1: bytes32 namespace_id,
  2: [ProjectedUpdateV1 ...]    # total order defined above
}

ProjectedUpdateV1 = {
  0: 1,                         # unsigned version
  1: bytes32 entry_id,
  2: bytes32 namespace_id,
  3: bytes32 signer_id,
  4: text headline,
  5: text description,
  6: text language,
  7: uint urgency,
  8: uint severity,
  9: uint certainty,
  10: null | uint valid_from_unix_seconds,
  11: uint expires_at_unix_seconds,
  12: null | text affected_area,
  13: [text source_claim ...],   # signed order retained
  14: bool ai_assisted,
  15: uint created_at_unix_seconds,
  16: text key_tag,              # exactly eight lowercase hex characters
  17: true signature_valid,
  18: true capability_valid
}
```

Urgency codes are `immediate=0, expected=1, future=2, past=3, unknown=4`;
severity codes are `extreme=0, severe=1, moderate=2, minor=3, unknown=4`;
certainty codes are `observed=0, likely=1, possible=2, unlikely=3, unknown=4`.
Every text/count ceiling is the corresponding validated `riot-core` ceiling;
indefinite values, tags, duplicate keys, unknown keys, non-minimal integers,
invalid UTF-8, and trailing bytes are rejected. The checked-in schema module and
golden vectors are authoritative for Rust, WASM, and renderer conformance tests.
“Byte-identical projection” means these canonical bytes from the same signed
live alert set and projection-code version, not identical DOM, localized labels,
or CSS.

The workspace pins `wasm-bindgen = 0.2.126`. `scripts/web/build.sh` verifies
`wasm-bindgen-cli 0.2.126`, runs
`cargo build --locked --release -p riot-web --target wasm32-unknown-unknown`,
then invokes that exact CLI with `--target web` to emit glue into a generated
directory and creates the content-hashed service-worker asset manifest. A
version mismatch is a hard build failure.
Generated glue/build output is not hand-edited. `package-lock.json` similarly
pins Node test/build dependencies.

Profile creation returns the core's sealed identity blob plus a random 32-byte
wrapping key that must immediately be protected by `BrowserVault`; temporary
Rust buffers are zeroized and JS `Uint8Array`s are overwritten on a best-effort
basis. Garbage-collected or generated binding copies are not claimed to be
fully zeroizable. No API returns a Willow subspace secret or owned namespace
root.

The adapter maps internal errors to a closed enumeration of stable browser
error codes and never exposes debug strings that could contain input bytes.

### Browser-compilable Willow dependency

The crates.io `willow25 = 0.6.0-alpha.3` package unconditionally depends on
`fjall`/`lsm-tree`; `lsm-tree` deliberately fails to compile for
`wasm32-unknown-unknown`. Riot therefore vendors that exact released source at
`vendor/willow25-browser/` and selects it through `[patch.crates-io]`. The fork
keeps the package name/version and protocol code unchanged. Its allowlisted
patch is only:

- mark `fjall` and `async-fs` optional;
- add a `persistent-storage` feature containing those dependencies; and
- gate `storage::persistent_store` and its re-export on
  `all(feature = "std", feature = "persistent-storage")`.

`MemoryStore`, entry/capability codecs, Meadowcap, and cryptographic parameters
remain the upstream code. Riot enables `std` but not `persistent-storage` for
both `riot-client` and `riot-web`; no Riot crate currently uses
`PersistentStore`. The vendor directory includes the crates.io archive checksum,
license files, package URL, an upstream per-file hash manifest, and one
reviewable patch file. `xtask verify-willow-vendor` checks unchanged files
against that manifest and patched files against their committed post-patch
hashes; it also fails if `fjall`, `async-fs`, or `lsm-tree` appears in the
`riot-web` target feature graph. Replacing the fork requires an upstream release
with the same optional-storage boundary plus the full conformance suite.
The workspace explicitly lists `vendor/willow25-browser` in `exclude`, and both
coverage tools ignore that exact vendored path while continuing to measure all
authored workspace Rust.

The browser graph is exactly
`riot-web -> riot-client -> riot-core -> willow25(MemoryStore)`. Target-specific
configuration enables `getrandom 0.2`'s `js` backend. Native release builds keep
the workspace's required `panic = "unwind"`; `.cargo/config.toml` sets
`-C panic=abort` only for `wasm32-unknown-unknown`, where no UniFFI panic-catching
contract exists. The feasibility gate builds this complete release graph and
the native UniFFI graph before browser feature work continues.

### Browser vault

On first profile creation:

1. Generate a non-extractable AES-GCM `CryptoKey` with WebCrypto and store the
   structured-clone key in IndexedDB.
2. Ask WASM to create the Riot identity and sealed identity envelope under a
   fresh random wrapping key.
3. Encrypt that wrapping key with the non-extractable AES key using a fresh
   nonce and fixed, versioned additional authenticated data.
4. Store only `{version, encryptedWrappingKey, nonce, sealedIdentity}`.
5. Zero/finalize all reachable temporary byte buffers on both sides.

On normal startup, WebCrypto decrypts the wrapping key only long enough to open
the sealed Riot identity in WASM. The envelope avoids storing the wrapping key
as plaintext application bytes and `extractable: false` prevents ordinary JS
export, but protection of a structured-cloned CryptoKey at rest is
browser-dependent. Riot does not claim that it survives copying a browser
profile or raw IndexedDB/key backing files. At most it protects a logical copy
of the ciphertext records that excludes the stored CryptoKey, and it does not
protect against the origin itself. A malicious
same-origin page or service worker can use the stored non-extractable CryptoKey,
invoke signing, or read decrypted application memory whenever it executes; the
prototype makes no stronger claim. Clearing browser site data destroys the
local signer permanently in this slice. Public-data export can restore carried
updates under a new member identity, but cannot recover the former identity or
organizer authority. Remote recovery is future work.

### Accepted bundle log

The first slice stores accepted-only canonical bundle bytes in an append-only
IndexedDB object store keyed by SHA-256. After a selective import, the shared
controller re-encodes exactly the selected, admitted frames in ascending entry-ID
order into a normalized bundle; rejected, unselected, foreign-namespace, and
non-alert frames never enter persistence or export.

A versioned manifest is split into a fixed header
`{version, generation, namespace_id, record_count, total_bytes}` and ordered descriptor rows
`{ordinal, sha256, byte_length, admission_route, transport_receipt|null}`.
`MAX_LOG_RECORDS = 4096`, the header is at most 4 KiB, each descriptor is at most
512 bytes, all descriptors total at most 1 MiB, each bundle is at most 8 MiB,
and staged canonical bundle bytes total at most 16 MiB.

The header and every descriptor value are stored as canonical CBOR `Blob`s, not
structured-clone object graphs. Store keys are fixed: literal `header`, unsigned
ordinal integers, and 64-character lowercase SHA-256 hex respectively. Restore
uses phased read-only transactions under the page-lifetime exclusive writer
lock, and never awaits `Blob.arrayBuffer()`, CBOR parsing, or hashing while an
IndexedDB transaction must remain active:

1. A short transaction calls `count()` on the header store, fetches only literal
   `header`, requires a `Blob`, and returns its handle. After transaction
   completion, the host checks `Blob.size <= 4 KiB`, buffers, and parses it.
2. A second short transaction re-fetches the header Blob handle, calls `count()`
   on descriptors, and returns only expected ordinal Blob handles
   `0..record_count-1`; request handlers reject non-Blob or over-512-byte values.
   After completion, the host buffers/re-parses the header and requires the same
   exact bytes/generation, then sequentially buffers/decodes descriptors within
   the 1 MiB aggregate ceiling.
3. A third short transaction again fetches the exact header Blob, calls
   `count()` on bundles, and returns Blob handles only for expected unique
   64-character hashes. Request handlers reject non-Blob values and any
   per-bundle/declared aggregate size mismatch. After completion, the host
   revalidates the same header bytes/generation, then sequentially buffers,
   hashes, validates, and stages the immutable bundle snapshots within the 16
   MiB ceiling.

Thus asynchronous decoding cannot make a needed transaction inactive, and an
attacker-sized unexpected key is never enumerated or materialized: it can only
cause a count/missing-expected-key rejection. `generation` changes atomically
with every manifest write. The Web Lock and repeated exact-header checks prevent
application-writer races; hostile same-origin code outside Riot can still race
or exhaust browser resources and is already outside the trusted-release claim.
Missing, duplicate, out-of-order, non-`Blob`, oversized, unexpected, or
noncanonical metadata/objects are `REPLAY_FAILED`. Browser/IDB may deserialize a
malicious non-Blob value returned at an exact expected key before JavaScript can
reject its type; the design makes no claim against that pre-JS browser
allocation. Unsupported but internally consistent schema versions enter
read-only recovery and preserve bounded raw records for recovery export.
Duplicate bundle hashes retain the first local receipt and do not grow storage;
that receipt is informational and never changes record trust or projection.

The retained browser log uses the core's existing 16 MiB store budget as a hard
upper bound. Browser code rejects a write before IndexedDB mutation if the
manifest would exceed that bound. One transaction writes the bundle and
manifest atomically, together with the associated draft deletion or first-join
profile metadata when that operation requires it. Replay uses only the record's
fixed `AdmissionRouteV1`; optional transport receipts never cross into core
admission, budgeting, authorization, or deterministic projection.

The log detects corruption and internal omission relative to its manifest. It
cannot detect same-origin deletion of both a manifest suffix and its objects,
replacement or reordering by an older/different self-consistent database, or
complete site-data clearing without an external anchor. Home therefore reports
last local change time without claiming the browser holds a complete global
history.

**Export community data** asks the shared controller to encode the current live
single-namespace inventory as one canonical bundle, sorted by entry ID and
bounded by the native 8 MiB `MAX_BUNDLE_BYTES`. The deterministic filename is
`riot-<full-namespace-id>.riot-evidence`. One click produces
one download or a clear failure; there is no partial multi-download state and
no new outer container format.

### Future signer boundary

The UI/controller treats signing as an asynchronous operation even though the
only implementation in this slice is local. Its conceptual contract is:

```text
SignerBackend.public_identity() -> public identity
SignerBackend.sign(canonical_entry_bytes, signing_context) -> signature
SignerBackend.status() -> local | remote-online | remote-offline | locked
```

A future remote backend may use OAuth Authorization Code + PKCE to authorize a
Keycast-like Willow signer. That service must sign the exact retained canonical
Willow entry bytes unchanged, just as the local Meadowcap implementation does,
and must not reinterpret drafts. The signing context and domain-separated
review digest inform signer-side policy but never enter the Ed25519 message.
NIP-46 is a useful
interaction model but is not a drop-in protocol because Riot uses Ed25519
Willow signatures rather than Nostr secp256k1 events. The intended future model
is remote root/recovery authority plus scoped, expiring local device
capabilities, so ordinary signing continues offline.

This slice does not refactor `riot-core` into a fully pluggable signer. It keeps
the adapter boundary narrow so the later signer design can be separately
specified and reviewed instead of smuggling network authority into the proof.

## User interface

This slice follows the approved community-first vocabulary. Willow spaces remain
technical containers and never name an ordinary top-level screen.

With no retained community, first use offers:

- **Create a community** — ask for a local community name, then create the
  organizer-bound identity and community atomically.
- **Import community data** — choose one public-data bundle, preview its single
  namespace and readable updates, provide or accept a local-only community
  label, then **Add this community**. Acceptance creates a fresh member signer
  within that namespace. Import is not identity recovery.
- **Try the Riverside demo** — the same import-as-member flow with a fixed local
  label and committed fixture bytes.

A browser with no retained profile cannot distinguish a genuinely clean first
run from complete site-data clearing. The same visible warning therefore
appears on every no-profile screen: **If you used Riot in this browser before,
that signer and any organizer authority are gone. Import restores public
updates only and creates a new member identity.** No copy claims to have
detected a previous installation.

With one retained community, launch opens **Home** directly. Home shows the
local community name, local durability and offline-availability status, that
exchange is manual in this release, readable updates,
**Post an update**, **Import updates**, and **Export community data**. There is
no community list or switcher. Organizer/member relationship and complete
namespace/signer IDs appear in **Community settings → Technical details**, and
entry IDs/signature/capability facts appear in each update's **Technical
details** disclosure. IDs are always complete when shown.

The post flow is:

1. **Post an update** form with headline, what people need to know, required
   **Where this came from**, visible/editable expiry, affected area, and closed
   urgency/severity/certainty choices. Local assistance is off by default.
2. Exact review showing all fields, destination community, acting author label,
   created/expiry times, and Technical details digest.
3. Final **Post update** action. While signing/persisting it is disabled and
   labeled **Posting…**; cancellation is unavailable after signing begins.
4. Success copy: **Saved on this browser. Export it to share.** The
   new update is inserted by signed creation time without moving focus.

Import review shows one selectable row per valid update: headline, body, author
label, ordered sources, age, expiry, and AI-assisted status. Rejected rows are
not selectable and show one fixed plain-language reason (`invalid signature`,
`invalid capability`, `wrong community`, `malformed update`, `expired`, or
`unsupported entry type`). Complete IDs and parser-safe codes are in Technical
details. On first run the single atomic CTA is **Add this community**: it creates
the member identity and accepts the selected rows together. With an existing
community the CTA is **Add selected updates**. Either CTA is disabled when
nothing is selected, and Cancel has no mutation. The label field says **This
name exists only on this browser.**
Selectable rows use native labeled checkboxes inside a `fieldset`/`legend`;
selection changes announce the selected count in a polite live region without
re-reading the whole row. Rejected rows contain no disabled fake checkbox.

States:

- First run: the three community actions above; identity creation stays an
  implementation detail of create/join.
- Loading/unlocking: disabled actions and a plain progress label.
- Ready: identical core actions regardless of network reachability; locally
  durable records and manual file exchange remain available.
- Empty community: Post an update and Import updates actions.
- No retained profile: use the first-run warning above because a clean browser
  and complete site-data clearing are indistinguishable.
- Corrupt retained storage: no silent reset; explain what is present but cannot
  open, preserve recoverable raw public bundles if possible, state that the
  prior identity cannot be recovered in this slice, and offer the distinctly
  labeled unverified recovery artifact.
- Unsupported but valid storage schema: read-only recovery with the distinctly
  labeled unverified recovery artifact; no
  automatic migration or reset.
- Storage full before core commit: refuse the mutation with `STORE_FULL`, retain
  the editable draft/selection, and keep **Export community data** available for
  the previously durable history.
- Persistence failure after core commit: show only the blocked `not-saved`
  recovery queue; every export/transport is disabled. After successful Retry,
  normal export includes the new durable record. After confirmed Close without
  saving, the controller reloads the prior durable state and normal export
  becomes available again without the discarded record.
- Another writer tab: read-only Home with **Riot is open in another tab**.
- Another writer tab before a community exists: a read-only first-run page with
  the same explanation and no create/import controls; it automatically offers
  the normal actions only after the writer lock becomes available.
- Install available: an in-app **Install Riot** action appears only while a
  captured `beforeinstallprompt` is usable. Dismissal leaves a quiet install
  action for a later visit; an interrupted or unknown prompt is recomputed on
  next launch. `appinstalled` or standalone display mode hides the action.
  Browsers without the event show concise browser-menu instructions and never
  claim the app was installed.
- Update available: preserve drafts, finish or recover any durable write, then
  offer **Reload to update**; never activate silently under an open page.
- Unsupported browser: explain required WebAssembly, IndexedDB, WebCrypto, and
  service-worker/Web-Locks capabilities; keep the public gateway link available.

The interface follows Riot's existing field-document visual language rather
than diVine brand rules: restrained paper/dark surfaces, high-contrast status
colors, monospace provenance, no gradients, 44px minimum targets, reduced
motion support, and responsive operation down to 320 CSS pixels. It uses one
`main` landmark, ordered headings, labeled forms, visible `:focus-visible`
outlines, text/icon/shape rather than color-only state, 200% zoom reflow without
horizontal scrolling, focus restoration to the invoking control after dialogs,
and `aria-live` announcements that never steal focus. Drafts persist on every
field change and survive navigation/reload. A failed form submission associates
each error with its input through `aria-describedby`, focuses the first invalid
field, and preserves every value. Successful post clears only the posted draft.
Home orders non-expired updates by signed creation time descending and puts
expired updates in a collapsed **Earlier** section. Newly inserted updates do
not move focus.

Home's manual-exchange panel always explains: **Saved on this browser means the
record is durable here. Export prepared means Riot created a file for you to
carry; it cannot confirm anyone received it. Exchanged is reserved for a
completed exchange of authenticated records, which this version does not
provide.**
The vocabulary is therefore visible before the comprehension check even though
the MVP never displays an Exchanged status badge.

Every normal canonical export action and success state says: **This file contains public
community updates. It does not back up your identity or organizer authority.**
After the browser accepts download initiation, success says **Export prepared**
and explicitly avoids **sent**, **delivered**, or **exchanged**. Picker/download
cancellation leaves community state unchanged and shows no success claim.

Corrupt/unsupported-storage recovery is a different artifact and never uses the
normal export copy, success style, `.riot-evidence` extension, or normal import
picker. It downloads `riot-unverified-recovery-<timestamp>.bin` with the visible
warning: **Unverified recovery bytes. Riot could not verify these records; this
file may be corrupt and may not import. It does not contain an identity backup.**
Success says **Unverified recovery file prepared**. Raw recovery preserves bytes
for expert/manual salvage only and makes no community-update or authenticity
claim.

## Data flows

### Create a community

1. Register service worker after the page is interactive.
2. Feature-detect WASM, IndexedDB, WebCrypto, service workers, and Web Locks.
3. User chooses Create a community and supplies a local community name.
4. Shared controller creates the organizer-bound author/community; BrowserVault
   protects the returned wrapping key and sealed identity.
5. Vault, community record, and empty log manifest commit in one IndexedDB
   transaction before success is shown. The host then calls
   `confirm_profile_persisted`; only that transition enables mutation. Failure
   calls `abort_pending_profile`, closes the new controller, zeroes reachable
   returned key buffers, and leaves the no-profile screen; no organizer identity
   is presented as created.
6. Home opens with organizer relationship in Community settings only after
   confirmation.

### Prepare and post

1. UI validates field presence, byte ceilings, expiry, and source claims for
   immediate feedback.
2. Shared controller repeats authoritative validation, freezes the complete
   signable request, and returns `UpdateReviewV1` without signing.
3. UI renders only that immutable review.
4. Human presses Post update.
5. Controller revalidates review/session bindings, signs only the retained
   canonical entry bytes, encodes one canonical bundle from every retained
   component, and commits through
   inspect → plan → commit.
6. One IndexedDB transaction writes the exact bundle, updates the manifest, and
   deletes the posted draft (or replaces it with a durable posted marker) before
   reporting durable success. Reload immediately before the transaction sees
   the draft and no durable entry; reload immediately after sees the durable
   entry and no postable draft. The retry path uses this same transaction.
7. The host acknowledges the exact bundle hash only after commit; the controller
   then permits another mutation and the UI refreshes from its live view.
8. Local durable success is final for publication. The MVP performs no automatic
   upload. The accepted log immediately makes the record available to file
   export and to future replication sessions without changing or re-signing it.

Between steps 5 and 7, the controller quarantines the committed bundle as
`not-saved`: the recovery UI may describe it, but normal projection,
consolidated/file export, and replication inventory remain pinned to the last
durability acknowledgement. Only the exact-hash acknowledgement publishes the
new record to those read/synchronization surfaces. Thus no transport can observe
a locally posted record before its durable browser commit.

The same quarantine invariant applies to every mutation path: local post,
existing-community file import, first-run join, and duplex replication. Pending
records may appear only in their dedicated recovery/review surface; normal
projection, consolidated export, and any replication inventory advance only
after the operation's exact durability acknowledgement.

If persistence fails after core commit, the controller keeps the generated
bundle in a recovery queue, blocks all further mutations, and opens a visible
**Not saved to this browser** recovery screen with Retry and a warning that
closing or reloading loses the in-memory signed bundle. File export and every
peer transport remain disabled until persistence succeeds. **Close without
saving** requires confirmation, closes the controller, and reloads only the
previously persisted state. Riot never claims durable success. While the queue
exists the original draft cannot be posted again. A successful Retry atomically
persists the bundle/manifest and clears that draft.
On transition to recovery, focus moves to the recovery heading, the heading is
announced once with the non-durable warning, and Retry is the next focusable
control; focus never returns to the disabled Post action. Close-without-saving
uses a labeled confirmation dialog and restores focus to its invoking control on
cancel.

### Import or join

1. Browser rejects files above the native 8 MiB `MAX_BUNDLE_BYTES` ceiling
   before `arrayBuffer()` and Rust rechecks the same ceiling.
2. Shared controller decodes and verifies without mutation, requires exactly one
   public communal namespace, and produces readable rows.
3. The user selects valid rows. On first run, they provide a local-only
   community label and press **Add this community**; the controller atomically
   creates a member author in that namespace and accepts the selection. With an
   active community, the namespace must match or the whole import is rejected,
   and the CTA is **Add selected updates**.
4. Either cancellation or an empty selection leaves identity and store
   unchanged.
5. Controller commits exactly the selection and returns a normalized accepted-
   only canonical bundle; browser appends it with admission route
   `web-record-exchange` and an optional local `file` receipt in one IndexedDB
   transaction.
   On first-run join, the protected vault, community record, accepted bundle,
   and manifest commit together in that transaction before success is shown.
   The host then calls `confirm_profile_persisted`; an existing-community import
   instead calls `acknowledge_bundle_persisted` with the exact hash.
6. If an existing-community import fails persistence, the ordinary bundle
   recovery queue applies. If the first-run join transaction fails, no mutable
   joined handle or Retry claim escapes: the host calls
   `abort_pending_profile`, closes the new controller, zeroes reachable new
   identity buffers, returns to the still-readable import preview, and offers
   download of the accepted public bundle plus a fresh
   restart. A later join attempt intentionally creates a new member signer.

File import/export is the first `bundle-carrier` transport adapter. The file picker and
download APIs live in the browser host; canonical bundle verification,
selection, admission, and export remain in the controller/core. Cancelling a
picker or download is a transport cancellation, not a protocol failure and not
a community mutation.

### Startup and offline reload

1. Load one coherent content-versioned application release from network or the
   matching service-worker cache.
2. Acquire the exclusive writer lock, use store `count()` plus only the exact
   fixed header/ordinal/hash keys to validate size-checkable metadata Blobs and
   immutable bundle Blob handles in three short read-only transactions. Decode
   and hash only after each transaction completes; revalidate the exact header
   generation in every phase. Reject count, per-record, manifest, generation,
   or aggregate ceiling violations before adding bytes to the bounded staging
   log; never enumerate unknown keys.
3. Pass the protected identity, community, and complete `RestoreLogV1` to the
   controller's atomic `restore_community` transition. It permits an empty
   organizer log and otherwise replays every bundle with its fixed admission route
   through normal verification/admission before enabling mutations.
4. Reject any community/manifest/author namespace or signer mismatch.
5. Render only after replay completes; progress shows bundle count.
6. A corrupt bundle stops replay and enters recovery instead of being skipped.

### Release update

The worker precaches the complete content-hashed release into a new cache; a
failed fetch fails installation and the incomplete cache is deleted. HTML names
only that release's hashed JS, WASM, CSS, and manifest assets. The waiting worker
notifies every controlled client but does not activate itself. **Reload to
update** is enabled only after every open client acknowledges that drafts are
persisted and no mutation/recovery queue is active; an unresponsive client makes
the UI require that other Riot tabs be closed. The writer then sends the waiting
worker `ACTIVATE_RELEASE`, which calls `skipWaiting`. On `controllerchange`, all
acknowledged pages reload into the new release. The new worker calls
`clients.claim()` but retains old release caches until `clients.matchAll()` and
client release acknowledgements prove that no old-release page remains; fetches
for old hashed assets continue to resolve from their matching old cache. It then
deletes only unreferenced release caches. Data opens only when the vault/log
schema version is supported; otherwise the new release enters read-only
recovery. Tests prove old or new assets are served coherently, never a mixture.
Navigation requests for stable `index.html` are always answered from the
controlling worker's own release cache, so a deployment cannot inject new HTML
into an old controlled page before activation.

## Security model

### Trusted

- The currently loaded, versioned Riot application release.
- Browser WebCrypto implementation and the browser's origin boundary against
  other origins.
- `riot-core` canonical encoding, signature verification, capability checking,
  limits, and preview/plan/commit semantics.

### Untrusted

- Every imported file and its filename/MIME type.
- Every transport, peer, gateway, community node, Nym endpoint, discovery
  result, delivery receipt, and renderer domain.
- Every accepted public entry's claims; valid signatures do not assert truth.
- Every renderer's completeness and presentation choices. Two honest compatible
  renderers can reproduce the same deterministic projection; a malicious or
  obsolete renderer can omit or misrepresent it and is not made trustworthy by
  serving valid signed records.
- The static host as a source of availability. A compromised future host
  release is capable of same-origin abuse, so production deployment requires a
  separate signed-release/update policy before private groups are considered.
- IndexedDB bytes on startup until replayed and verified.

### Controls

- The origin must send this CSP as an HTTP response header (a meta tag does not
  satisfy the contract):
  `default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; worker-src 'self'; manifest-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'`.
  `wasm-unsafe-eval` is the narrow documented exception required for browser
  WebAssembly compilation; JavaScript `unsafe-eval`, `unsafe-inline`, blob
  workers, and data/network assets remain forbidden.
- Response headers also set `Referrer-Policy: no-referrer`,
  `X-Content-Type-Options: nosniff`, and exactly
  `Permissions-Policy: accelerometer=(), ambient-light-sensor=(), autoplay=(), bluetooth=(), camera=(), display-capture=(), encrypted-media=(), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), publickey-credentials-create=(), publickey-credentials-get=(), serial=(), usb=(), xr-spatial-tracking=()`.
- No third-party runtime dependencies, analytics, fonts, or remote assets.
- Adding any future network/Nym adapter requires a separate reviewed deployment
  policy that extends `connect-src`, `worker-src`, and Permissions Policy only
  with the adapter's minimum explicit origins/capabilities; adapters may not
  weaken the default CSP globally or introduce wildcard connectivity.
- No `innerHTML` for imported/user content; render with text nodes.
- Existing core byte/count/path/store ceilings remain authoritative.
- Every transport carries only opaque bounded canonical frames or bundles into
  the same verification/admission boundary. A transport can drop, delay,
  duplicate, reorder, replay, censor, or observe traffic but cannot grant
  authority, alter community identity, or bypass review/verification.
- File input is size-checked before `arrayBuffer()` and rechecked in Rust.
- Full IDs in UI and logs; secret/capability/signature bytes never logged.
- Intended service-worker code caches only the coherent versioned release and
  never opens IndexedDB. Same-origin enforcement cannot prevent a compromised
  worker from opening IndexedDB; that accepted limitation is tested/documented,
  not presented as isolation.
- Browser vault schema and AAD are versioned; authentication failure never
  falls back to a new identity.
- Posting is always a human action on an immutable exact-byte review; no miniapp
  or imported page receives signing access.
- Private groups are excluded because a hosted origin has a stronger active
  code-update threat than signed public data can tolerate without further work.

Known limitation: malicious same-origin code, including a worker running without
an open UI, could use the stored WebCrypto key, invoke local signing, or extract
decrypted material. The vertical slice makes no hardware-backed,
host-compromise, rollback-detection, or identity-recovery claim. The future
remote authority reduces durable root exposure but does not remove the need for
signer-side policy and human-readable approval.

Metadata-resistant delivery is not anonymity by declaration. A future Nym
adapter may reduce network metadata exposure, but Riot must not claim sender,
recipient, timing, volume, browser, endpoint, or content privacy until that
adapter's separate threat model and measurements support the claim.

## Errors and recovery

Stable user-facing categories:

- `UNSUPPORTED_BROWSER` — required platform API missing; link to public gateway.
- `IDENTITY_LOCKED` / `IDENTITY_CORRUPT` — do not create a replacement; offer
  the distinctly labeled unverified recovery artifact if bytes are available
  and explain that identity recovery is not present.
- `ENTROPY_UNAVAILABLE` / `CLOCK_UNAVAILABLE` — no signing/profile mutation;
  retain the draft and offer retry.
- `IMPORT_TOO_LARGE`, `IMPORT_REJECTED`, `WRONG_COMMUNITY`,
  `NO_ELIGIBLE_ENTRIES` — no mutation.
- `REVIEW_STALE` / `PREVIEW_STALE` — retain the draft/selection and require a
  fresh review.
- `INVALID_DRAFT` / `EXPIRED_DRAFT` — return to editable form without posting.
- `STORE_FULL` — no mutation; offer export.
- `ANOTHER_WRITER` — read-only view until the other tab closes.
- `UNSUPPORTED_SCHEMA` — read-only recovery and distinctly labeled unverified
  recovery artifact.
- `PERSISTENCE_FAILED` — core may hold an in-memory commit; show non-durable
  state, block every transport, and offer Retry or confirmed Close without
  saving.
- `PERSISTENCE_PENDING` — session close/cancel is forbidden while a
  core-committed bundle awaits durability acknowledgement; zero state change,
  with Retry or confirmed whole-controller Close without saving as the only
  exits.
- `REPLAY_FAILED` — stop at the first corrupt record and offer the distinctly
  labeled unverified recovery artifact; never silently skip.
- `REPLICATION_WINDOW_EXCEEDED` — current live inventory exceeds the MVP's 64
  entry/8 MiB reconciliation and single-bundle export window; no session opens.
  Previously retained per-operation bundles remain durable locally, but this
  MVP cannot produce one complete live-set export. The UI states that limitation
  and points to future paginated export rather than promising a false fallback.
- `REPLICATION_UNEXPECTED_FRAME` — wrong session/state/frame ordering or mode;
  no mutation and no peer acceptance acknowledgement.
- `INTERNAL` — close the WASM session, retain persisted bytes, and offer reload.
- `TRANSPORT_CANCELLED` / `TRANSPORT_UNAVAILABLE` /
  `TRANSPORT_MODE_MISMATCH` / `TRANSPORT_OVERSIZE` / `TRANSPORT_FAILED` — no
  rollback of locally durable records, no silent fallback, and no authority
  implication; file exchange remains available when its adapter is usable.

All errors are actionable, contain no raw imported bytes or secrets, and are
rendered in an `aria-live` region.

## Testing and TDD

`.coverage-thresholds.json` remains the source of truth: 100% lines, branches,
functions, and statements. Before production implementation begins, its
enforcement command changes to `scripts/web/coverage.sh`. A non-interactive
`scripts/web/bootstrap.sh` installs `nightly-2026-07-01` with
`llvm-tools-preview` using
`rustup toolchain install nightly-2026-07-01 --profile minimal --component llvm-tools-preview --no-self-update`.
It also runs and verifies
`rustup target add wasm32-unknown-unknown --toolchain 1.95.0` for the workspace's
pinned stable compiler.
If the installed versions differ, it runs locked installs for
`cargo-llvm-cov = 0.8.7`, `cargo-tarpaulin = 0.37.0`, and
`wasm-bindgen-cli = 0.2.126`; the coverage/build scripts reject any other
versions. The coverage script runs
the existing `cargo tarpaulin --workspace --all-features --fail-under 100` line
gate, runs `cargo +nightly-2026-07-01 llvm-cov clean --workspace`, and generates
`cargo +nightly-2026-07-01 llvm-cov --workspace --all-features --branch --json --output-path target/llvm-cov/riot.json`.
A checked-in
validator fails unless LLVM totals report 100% lines, functions, regions, and
branches; LLVM regions are the Rust executable-statement metric recorded under
the repository's `statements` threshold. The script then runs pinned `c8 --100`
Node tests over every authored production JS/controller/service-worker module,
including 100% JS statements. Generated
`wasm-bindgen` glue is excluded from JS coverage because it is generated from
Rust; the authored Rust implementation remains covered on the host and the glue
is exercised in both browser engines. Playwright behavior tests remain blocking
in addition to, not instead of, line/branch/function/statement coverage.

`package.json` and `package-lock.json` pin `c8` and `@playwright/test`; CI and
local verification use `npm ci`, never floating `npx` resolution. Browser ports
(IndexedDB, WebCrypto, CacheStorage, Web Locks, downloads) are injected behind
small authored modules so all branches run under Node fakes for coverage and
again against real browser implementations for behavioral proof.
`playwright.config.js` has named `chromium` and `webkit` projects. CI installs
the package-lock-pinned browser revisions with
`./node_modules/.bin/playwright install --with-deps chromium webkit`.

TDD work proceeds in independently green slices:

1. **Coverage baseline remediation (existing RED):** the retained Tarpaulin
   artifact records 4,177/5,010 traced lines (about 83.37%), so the repository is
   not assumed green. First wire `.coverage-thresholds.json` to the composite
   command and add `scripts/web/bootstrap.sh`, the pinned nightly LLVM report,
   and its totals validator. Then add behavior-focused tests and only minimal
   test seams for existing authored production Rust under `crates/*/src` until
   Tarpaulin lines and LLVM lines/functions/regions/branches all reach 100%.
   Generated UniFFI/wasm glue, `target/`, and vendored upstream sources are the
   only exclusions; every exclusion is an exact checked-in path. Split this
   bounded remediation into per-crate commits, with no product behavior or web
   feature changes. Fixture omissions separately prove each metric fails. This
   gate must pass before the WASM milestone.
2. **WASM build contract (RED first):** a target check fails on the current
   `getrandom` configuration and, after a temporary entropy fix, at
   `lsm-tree`'s unsupported-platform guard. Vendor the exact allowlisted
   `willow25` optional-storage patch, add target-scoped entropy/panic settings,
   and extract the ordinary `riot-client` controller from its UniFFI adapter.
   `xtask verify-willow-vendor`, `cargo tree`, native release checks, and the
   full `riot-web` release build must prove that the browser graph uses
   `MemoryStore`, contains no `fjall`/`async-fs`/`lsm-tree`/UniFFI, and preserves
   the native unwind contract. This is a hard gate; no browser feature code
   proceeds until it passes.
3. **Prepared alert core:** host Rust tests first require frozen IDs/times/bytes,
   domain-separated digest, signature over the exact retained entry bytes,
   expiry/stale failure, zero mutation on failure, and compatibility output from
   `create_signed_alert`; then split prepare/sign in `riot-core`.
4. **Signer-free replica and shared publisher controller:** `riot-client`
   contract tests first cover `CommunityReplica` restore/projection with no
   signer; golden canonical-CBOR projection vectors, total order/tie cases, and
   byte-identical `CommunityProjectionV1` under two renderer hostnames;
   organizer create/empty-log restore; import-as-member; atomic restore
   finalization; stored namespace/signer mismatch rejection; single-namespace
   rejection; readable preview rows; exact selective normalized bundle; fixed
   admission routes; optional receipt exclusion from trust/projection; identical
   store-charge outcomes for very short and maximum-length endpoint labels;
   consolidated deterministic export; immutable review IDs; member posting;
   pending-profile confirm/abort; bundle-persistence acknowledgement and
   mutation blocking; `PERSISTENCE_PENDING` on session close/cancel with zero
   state change; duplicate import; and every stable error mapping. The same
   tests wrap the existing byte-only session in `ReplicationCoordinator` and
   prove exactly one active session/frame buffer/pending import, the 64-ID and
   8-MiB live-window failures, immutable local transport binding, no silent
   fallback, initiator and responder starts, simultaneous-initiation rejection,
   terminal Complete/Reject receive paths, alert-only inventory filtering,
   exchange-profile mismatch before Hello, explicit incompatibility with native
   complete-inventory sync, non-alert entries-frame rejection, equality of
   requested/pending/decoded/valid/eligible/planned ID sets, prospective live-set
   validation, frame-drain ordering, all-or-reject review, exact
   commit→persist→hash-ack transition, wrong/stale hash safety, crash-before-ack
   retry safety, oversize/mode-mismatch rejection, and zero controller I/O.
   Only then add the ordinary Rust surface; existing `riot-ffi` tests must prove
   the native adapter preserves its contract.
5. **WASM DTO lifecycle:** host Rust tests cover every DTO field/enum/time/byte
   conversion and `WebErrorV1` branch before `wasm-bindgen` exposure. Browser
   smoke tests then call create, prepare, post, import, list, export,
   open/begin/receive/accept/reject/close replication, and close through
   generated glue.
6. **BrowserVault, BundleLog, writer lock, and transport port:** tests begin with missing
   implementations and cover first creation, reopen, authenticated-decryption
   failure, duplicate bundle/first receipt, manifest hash/size/order/total/version,
   store `count()` plus exact fixed-key lookup without key enumeration, 4-KiB
   header/512-byte descriptor/1-MiB manifest-row ceilings, non-Blob values,
   hostile oversized/unexpected keys never materialized, hostile oversized
   header/descriptor/bundle values rejected by Blob size before `arrayBuffer()`,
   transaction auto-close/liveness, cross-phase generation drift,
   count/object/aggregate rejection before staging,
   missing/extra/rollback-limitation copy, atomic transaction abort, 16 MiB
   boundary, storage clear, corrupt replay, unsupported schema, exclusive writer,
   second-tab/no-community read-only behavior, post transaction interruption
   including hard page termination immediately before/after
   bundle+manifest+draft commit, first-join transaction
   failure/identity discard, recovery-queue mutation blocking, the exact
   `TransportPortV1` state contract, closed failure mapping, cancel/concurrency
   behavior, registry-authoritative kind binding, adapter stripping/rebinding of
   distinct peer-local session IDs, an in-memory duplex fake proving bounded opaque frame exchange,
   unavailable reserved adapters with zero mutation and no UI controls, and the
   file picker/download bundle-carrier as the only concrete implementation. Real
   IndexedDB/WebCrypto/Web Locks tests use unique browser contexts; no in-memory
   mock certifies persistence.
7. **UI flows:** Playwright tests cover both first-run paths and demo, community
   naming, organizer/member action visibility, draft persistence,
   prepare→review→post, interruption at every durable boundary, offline restart,
   readable selective import, wrong-community rejection, deterministic export
   filename/download failure, install states, update-available multi-client
   activation and controlling-cache navigation, indistinguishable clean/cleared
   no-profile warning, storage recovery, 320px and 200% reflow, keyboard-only
   focus restoration, field-error association, non-color states, live-region
   behavior, reduced motion, and no horizontal overflow. Posting succeeds while
   every transport is unavailable; UI copy distinguishes **saved here** from
   **exchanged**, and no gateway/domain is presented as community ownership.
8. **Security/static contracts:** tests assert exact response headers, coherent
   release cache installation/activation, zero third-party requests, no
   `innerHTML`, no remote URLs/assets, no secret logging tokens, no write without
   immutable review, no service-worker vault call in intended code, no
   private-group surface, no protocol/controller imports of browser networking
   or Nym packages, no transport kind/endpoint in replication controller or
   signed identity/projection, and no implemented transport other than file
   exchange.

Required verification:

```text
cargo test --workspace --all-features
cargo check --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
scripts/web/bootstrap.sh
npm ci
./node_modules/.bin/playwright install --with-deps chromium webkit
scripts/web/build.sh
scripts/web/coverage.sh
npm run test:web:e2e
```

The browser suite runs Chromium and WebKit at minimum because storage,
service-worker, and WebCrypto behavior—not just DOM rendering—is in scope.

## Acceptance criteria

1. A clean browser can create one named public community; its organizer
   relationship and complete signer/namespace IDs remain stable across reload,
   with IDs available in Technical details rather than persistent chrome.
2. A human can create, review exact frozen bytes, post, and durably persist a
   valid update without any network request after the initial application
   load.
3. The same update, remaining unposted drafts, community, and identity appear
   after browser restart in offline mode; a successfully posted draft never
   reappears as postable.
4. One exported canonical public-data bundle imports through readable
   preview/accept into a second clean browser context. Imported entries preserve
   their complete original signer/namespace identities; the receiving browser
   receives a new member signer and never claims identity recovery.
5. Invalid, oversized, corrupt, stale, expired, duplicate, and storage-full
   paths have deterministic tested outcomes and never silently mutate state.
6. A second tab is read-only, a release update never mixes asset versions, and
   interruptions never result in a false durable-success claim or expose a
   successfully posted draft as postable again.
7. The PWA makes zero third-party runtime requests and remains usable with the
   origin offline after first load.
8. No server stores user state or signing material, and no remote signer is
   needed for this slice.
9. `riot-client` owns a tested transport-independent replication coordinator;
   protocol and controller code import no browser networking, gateway, domain,
   or Nym dependency.
10. File exchange is the only concrete transport, yet reserved adapters can be
    unavailable without blocking local publication and never appear as fake
    MVP controls. The same bytes have the same authentication, admission cost,
    and projection regardless of optional local transport receipt.
11. Community identity and deterministic projection contain no renderer domain
    or endpoint. A test fixture can be rendered under two unrelated hostnames
    with byte-identical authenticated history and `CommunityProjectionV1`
    bytes, while renderer-local labels may differ.
12. All repository quality gates and the composite 100% coverage enforcement
    command pass.
13. A signer-free `CommunityReplica` host test restores, verifies, persists a
    bounded local copy, and projects a community without constructing
    `PublisherAuthority`.
14. Four of five participants correctly distinguish saved-local, prepared-file,
    and completed-exchange states; file export never claims peer delivery.

## Future work

- Specify the OAuth recovery authority and scoped Meadowcap device enrollment
  as a separate design with signer-side policy, revocation, audit, and offline
  expiry behavior.
- Specify and implement Nearby, direct community-node, disposable HTTPS
  gateway, and Nym browser adapters independently through `TransportPortV1`;
  do not create a canonical web database or change signed record formats.
- Specify discovery and replicator retention policy separately from transport.
  A viewer must never become a replicator implicitly.
- Specify history/tombstone pagination, partitioned reconciliation, and
  retention windows before extending the current 64-ID/8-MiB live inventory;
  do not redefine the signed record formats to obtain scalability.
- Build stateless/disposable renderer deployments that rebuild projections from
  signed replicated records and prove domain-independent community identity.
- Threat-model private groups, signed PWA releases, unlock UX, plaintext
  lifetime, backups, and storage eviction before exposing encrypted group state
  to the hosted origin.
- Add bundled miniapps only after their existing containment guarantees are
  reproduced in the browser host without granting signing authority.
