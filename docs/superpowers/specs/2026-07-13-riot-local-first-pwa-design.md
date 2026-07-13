# Riot local-first PWA vertical slice design

Date: 2026-07-13
Status: Revision 8 — censorship-resistance architecture revision, review pending

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

**Architectural invariant:** Communities are permanent; renderers are
disposable. A website is only one possible presentation of a community's
authenticated history. Community identity derives from signed records rather
than domain names or server ownership.

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
   recovery queue until downloaded, retried, or discarded with confirmation.

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
data import does not recover identity.

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
After acquiring the lock, the host snapshots the vault and complete bundle log,
then gives both to the controller's atomic restore transition. That transition
opens the sealed identity, creates a fresh in-memory store, and replays accepted
canonical bundles through the same inspect → plan → commit path used for new
imports before enabling mutation. No serialized internal Rust store is trusted.

### Rust/WASM adapter

A new `riot-client` ordinary Rust library owns the profile/store state machine,
prepared reviews, join-from-bundle, accepted-only bundles, and consolidated
export. Existing `riot-ffi` becomes the native UniFFI adapter over that
controller; a new `riot-web` `cdylib`/ordinary library becomes the thin
`wasm-bindgen` adapter over the same controller. `riot-web` does not depend on
UniFFI and neither binding implements controller state. All state, namespace
admission, entry projection, and error mapping therefore have one
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
- `open_replication_session() -> ReplicationSessionV1`
- `begin_replication(session_id) -> ReplicationOutcomeV1`
- `receive_replication_frame(session_id, frame_bytes) -> ReplicationOutcomeV1`
- `take_replication_frame(session_id) -> Uint8Array|null`
- `accept_replication_import(session_id) -> ReplicationOutcomeV1`
- `reject_replication_import(session_id, code) -> ReplicationOutcomeV1`
- `close_replication_session(session_id) -> Result<(), WebErrorV1>`
- `close() -> Result<(), WebErrorV1>`

`create_community` creates one organizer-shaped author with
`generate_space_organizer_author`; its namespace equals its signer/subspace.
`restore_community` is one atomic controller transition. Before it returns, no
mutation operation is available: it restores the sealed author, requires the
caller-supplied community and manifest namespaces to equal the namespace sealed
into that author, requires the caller-supplied signer ID to equal the sealed
signer, and replays every ordered record with its retained route. A zero-record
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

Riot already has the correct protocol primitive in `riot-core`:
`ReconcileSession` owns a bounded bidirectional exchange and explicitly owns no
transport; `ByteSyncSession` converts that state machine to opaque canonical
frames. The PWA must preserve and expose that separation rather than inventing a
gateway API.

`riot-client` owns a `ReplicationCoordinator` over those byte-only sessions. It
accepts a local authenticated inventory and exposes versioned operations to:

- open or close one bounded exchange for a namespace;
- begin reconciliation;
- receive one opaque bounded frame;
- take one outbound frame;
- surface a received canonical bundle through the ordinary preview → plan →
  commit boundary;
- acknowledge accepted or rejected imports only after their normal persistence
  transition; and
- report complete, rejected, paused, and failed session states without network
  terminology.

The controller never opens a socket, resolves a URL, selects a gateway, invokes
Web Bluetooth, or imports a Nym package. It knows canonical records, namespace
identity, bounded reconciliation frames, and admission results only.

The browser host implements this controller-owned port shape:

```text
TransportPortV1.kind() -> file | nearby | direct-community-node |
                          https-gateway | nym
TransportPortV1.status() -> unavailable | ready | connecting |
                            exchanging | paused | failed
TransportPortV1.receive() -> async TransportEventV1(frame_bytes | bundle_bytes)
TransportPortV1.send(frame_or_bundle_bytes) -> async TransportSendResultV1
TransportPortV1.close() -> async result
```

This is a behavioral contract, not an invitation to add five adapters now.
`file` is the only concrete MVP implementation. It receives a bounded canonical
bundle from a user-selected file and sends a bounded canonical bundle through a
user-initiated download; it uses ordinary preview/accept and export operations,
not a fake connected-peer handshake. The other four enum values exist so UI,
telemetry-free local receipts, and controller tests cannot collapse the
architecture back to `gateway`. Attempting to open an unimplemented kind returns
`TRANSPORT_UNAVAILABLE` without changing community state.

Transport kind, endpoint, first-seen time, delivery attempt, and availability
are local receipt facts. They never enter signed alert payloads, entry IDs,
community identity, or deterministic projection. The same authenticated record
received by file, nearby exchange, a direct node, an HTTPS gateway, or Nym must
produce the same admission decision and projection. Duplicate, reordered,
delayed, partially delivered, or replayed records remain safe under the existing
ID-based reconciliation and preview-first admission rules.

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

Official Nym materials describe its TypeScript SDK as supporting browser-based
mixnet applications and its browser client as WebAssembly running in a worker.
That makes Nym a plausible future host adapter, not a reason to add Nym types or
dependencies to `riot-core`, `riot-client`, or this release. The future design
must separately threat-model message-size leakage, timing, cover traffic,
recipient addressing, SDK supply chain, browser worker policy, availability,
and the fact that metadata resistance does not make received claims true.

### Deployment roles and disposable renderers

Future deployments may choose one of three local capability profiles without
changing community identity or record formats:

- **Viewer:** receives, verifies, projects, and renders authenticated records;
  it need not sign or retain records for strangers.
- **Publisher:** additionally owns a signer, reviews exact bytes, commits
  locally, and offers its records to transports.
- **Replicator:** intentionally retains bounded community history and assists
  reconciliation; this is an explicit operator/user choice, never a hidden duty
  imposed on every viewer.

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
| `CommunityV1` | `version`, `namespace_id`, `local_title`, `relationship` (`organizer|member`), `signer_id` |
| `ProposedCommunityV1` | `version`, `namespace_id`, `relationship` (always `member`) |
| `CommunityCreatedV1` | `version`, `pending_profile_id`, `community`, `wrapping_key_bytes`, `sealed_identity_bytes` |
| `CommunityImportReviewV1` | `version`, `review: ImportReviewV1`, `proposed_community: ProposedCommunityV1`, `suggested_local_title` |
| `CommunityJoinedV1` | `version`, `pending_profile_id`, `community: CommunityV1`, `wrapping_key_bytes`, `sealed_identity_bytes`, `accepted_bundle: PendingBundleV1` |
| `RestoreRecordV1` | `version`, `bundle_bytes: Uint8Array`, `route` (`web-local-post|web-file-import`), `sha256` |
| `RestoreLogV1` | `version`, `namespace_id`, `total_bytes`, ordered `records: RestoreRecordV1[]` |
| `UpdateDraftV1` | `version`, `headline`, `description`, `language`, `urgency`, `severity`, `certainty`, `valid_from: decimal-string|null`, `expires_at`, `affected_area: string|null`, ordered `source_claims: string[]`, `ai_assisted` |
| `UpdateV1` | draft fields plus `entry_id`, `namespace_id`, `signer_id`, rendered `author_label`, `created_at`, `signature_valid`, `capability_valid`, `durability` |
| `ReviewedUpdateV1` | draft fields plus allocated `object_id`, `revision_id`, `entry_id`, `namespace_id`, `signer_id`, `created_at` |
| `UpdateReviewV1` | `version`, `review_id`, `update: ReviewedUpdateV1`, `community: CommunityV1`, `acting_author_label`, `canonical_digest` |
| `TechnicalDetailsV1` | `version`, complete `entry_id`, `namespace_id`, `signer_id`, `payload_sha256`, `signature_valid`, `capability_valid` |
| `ImportRowV1` | `version`, `headline`, `description`, deterministic `author_label`, ordered `source_claims: string[]`, `created_at`, `expires_at`, `ai_assisted`, `selectable`, fixed `rejection_code|null`, `technical: TechnicalDetailsV1` |
| `ImportReviewV1` | `version`, `review_id`, `namespace_id`, `byte_count`, ordered `valid_rows: ImportRowV1[]`, ordered `rejected_rows: ImportRowV1[]`, `selected_entry_ids: string[]` |
| `PendingBundleV1` | `version`, exact accepted-only `bundle_bytes: Uint8Array`, closed `route` (local/file plus reserved future transport values below), complete `entry_ids: string[]`, `sha256` |
| `BundleArtifactV1` | `version`, canonical `bundle_bytes: Uint8Array`, complete `entry_ids: string[]`, `sha256`, deterministic `filename` |
| `ReplicationSessionV1` | `version`, opaque `session_id`, complete `namespace_id`, `state` (`idle|frame-ready|awaiting-frame|import-review-required|rejected|complete|closed`) |
| `ReplicationOutcomeV1` | `version`, `session_id`, same closed `state`, `bundle_bytes: Uint8Array|null`, `rejection_code: string|null` |
| `TransportEventV1` | `version`, `kind` (`frame|bundle|closed|failed`), `bytes: Uint8Array|null`, `message_key: string|null` |
| `TransportSendResultV1` | `version`, `status` (`sent|cancelled|unavailable|failed`), `message_key: string|null` |
| `WebErrorV1` | `version`, stable `code`, `field: string|null`, `message_key`; never raw parser/debug text |

Every public call returns `Result<T, WebErrorV1>`. `route` is a closed enum, not
free caller-controlled text: local posts use `web-local-post`, file/demo imports
use `web-file-import`, and the reserved future values are `web-nearby`,
`web-direct-community-node`, `web-https-gateway`, and `web-nym`. Startup replay
accepts only and uses the exact enum stored with that log record. Reserved values
may be replayed but cannot be created by an unavailable adapter. Every opaque
import, review, replication, or pending-profile ID is single-use, session-bound,
and rejected after replacement, commit/abort, or close.

Every nested DTO also carries `version: 1`; arrays are always present, even
when empty, and nullable fields are JSON `null`, never omitted. `byte_count` and
`total_bytes` are safe non-negative JavaScript numbers bounded below 16 MiB.
`durability` is the closed enum
`durable|not-saved`; `rejection_code` is null for selectable rows and otherwise
one of `invalid-signature|invalid-capability|wrong-community|malformed-update|expired|unsupported-entry-type`.
The adapter derives the mandatory key tag as the first four signer/subspace
bytes rendered as eight lowercase hex characters. It uses the shared profile
resolver's verified name when present, otherwise `You` for the local signer and
`Community member` for another signer; `author_label` is always
`<rendered name> · <key tag>`. Role is shown separately. Complete signer IDs
remain in Technical details. Creating or editing display names is not part of
this slice. `suggested_local_title` is the literal `Imported community`; it has
no authority meaning and may be edited before join.

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

A versioned manifest records its single `namespace_id`, ordered
`{sha256, byte_length, route}` records, and total retained bytes. On
every open, one read-only IndexedDB transaction snapshots the manifest and all
bundle objects before validation. The host recomputes each object hash and size,
validates the manifest namespace/totals/order/version, and rejects missing,
duplicate, or unexpected unreferenced objects as `REPLAY_FAILED`. Unsupported
but internally consistent schema versions enter read-only recovery and preserve
all raw records for export. Duplicate bundle hashes retain the first admission
route and do not grow storage.

The retained browser log uses the core's existing 16 MiB store budget as a hard
upper bound. Browser code rejects a write before IndexedDB mutation if the
manifest would exceed that bound. One transaction writes the bundle and
manifest atomically, together with the associated draft deletion or first-join
profile metadata when that operation requires it. Replay uses each record's
original normalized route so provenance and route-byte accounting are stable.

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
local community name, online/offline and durability status, readable updates,
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
4. Success copy: **Saved on this browser. Export or exchange it to share.** The
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

States:

- First run: the three community actions above; identity creation stays an
  implementation detail of create/join.
- Loading/unlocking: disabled actions and a plain progress label.
- Ready online/offline: identical core actions; only network-dependent copy
  changes.
- Empty community: Post an update and Import updates actions.
- No retained profile: use the first-run warning above because a clean browser
  and complete site-data clearing are indistinguishable.
- Corrupt retained storage: no silent reset; explain what is present but cannot
  open, preserve recoverable raw public bundles if possible, state that the
  prior identity cannot be recovered in this slice, and offer raw export.
- Unsupported but valid storage schema: read-only recovery with raw export; no
  automatic migration or reset.
- Storage full: refuse the mutation and offer Export community data.
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

Every export action and success state says: **This file contains public
community updates. It does not back up your identity or organizer authority.**

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

If persistence fails after core commit, the controller keeps the generated
bundle in a recovery queue, blocks all further mutations, and opens a visible
**Not saved to this browser** recovery screen with Retry and immediate
exact-bundle download. It warns that closing or reloading can lose the in-memory
bundle and that a downloaded copy may circulate even if local persistence is
abandoned. **Close without saving** requires confirmation, closes the controller,
and reloads only the previously persisted state. Riot never claims durable
success. While the queue exists the original draft cannot be posted again. A
successful Retry atomically persists the bundle/manifest and clears that draft.

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
   only canonical bundle; browser appends it with route `web-file-import` in one
   IndexedDB transaction.
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

File import/export is the first `TransportPortV1` adapter. The file picker and
download APIs live in the browser host; canonical bundle verification,
selection, admission, and export remain in the controller/core. Cancelling a
picker or download is a transport cancellation, not a protocol failure and not
a community mutation.

### Startup and offline reload

1. Load one coherent content-versioned application release from network or the
   matching service-worker cache.
2. Acquire the exclusive writer lock, then snapshot and validate the vault,
   manifest, and all bundle records in one read-only IndexedDB transaction.
3. Pass the protected identity, community, and complete `RestoreLogV1` to the
   controller's atomic `restore_community` transition. It permits an empty
   organizer log and otherwise replays every bundle with its recorded route
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
  raw public-data export if available and explain that identity recovery is not
  present.
- `ENTROPY_UNAVAILABLE` / `CLOCK_UNAVAILABLE` — no signing/profile mutation;
  retain the draft and offer retry.
- `IMPORT_TOO_LARGE`, `IMPORT_REJECTED`, `WRONG_COMMUNITY`,
  `NO_ELIGIBLE_ENTRIES` — no mutation.
- `REVIEW_STALE` / `PREVIEW_STALE` — retain the draft/selection and require a
  fresh review.
- `INVALID_DRAFT` / `EXPIRED_DRAFT` — return to editable form without posting.
- `STORE_FULL` — no mutation; offer export.
- `ANOTHER_WRITER` — read-only view until the other tab closes.
- `UNSUPPORTED_SCHEMA` — read-only recovery and raw public-data export.
- `PERSISTENCE_FAILED` — core may hold an in-memory commit; show non-durable
  state and offer exact bundle download.
- `REPLAY_FAILED` — stop at the first corrupt record and offer raw-log export;
  never silently skip.
- `INTERNAL` — close the WASM session, retain persisted bytes, and offer reload.
- `TRANSPORT_UNAVAILABLE` / `TRANSPORT_FAILED` — no rollback of locally durable
  records and no authority implication; file exchange remains available.

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
4. **Shared controller extension:** `riot-client` contract tests first cover
   organizer create/empty-log restore, import-as-member, atomic restore
   finalization, stored namespace/signer mismatch rejection,
   single-namespace rejection,
   readable preview rows, exact selective normalized bundle, fixed routes,
   consolidated deterministic export, immutable review IDs, member posting,
   pending-profile confirm/abort, bundle-persistence acknowledgement and mutation
   blocking, duplicate import, and every stable error mapping. The same tests
   wrap the existing byte-only reconciliation session in the new
   `ReplicationCoordinator`, prove the controller performs no I/O, and prove
   identical records have identical admission/projection across every reserved
   transport route. Only then add the new ordinary Rust surface; existing
   `riot-ffi` tests must then prove the native adapter preserves its contract.
5. **WASM DTO lifecycle:** host Rust tests cover every DTO field/enum/time/byte
   conversion and `WebErrorV1` branch before `wasm-bindgen` exposure. Browser
   smoke tests then call create, prepare, post, import, list, export,
   open/begin/receive/accept/reject/close replication, and close through
   generated glue.
6. **BrowserVault, BundleLog, writer lock, and transport port:** tests begin with missing
   implementations and cover first creation, reopen, authenticated-decryption
   failure, duplicate bundle/first route, manifest hash/size/order/total/version,
   missing/extra/rollback-limitation copy, atomic transaction abort, 16 MiB
   boundary, storage clear, corrupt replay, unsupported schema, exclusive writer,
   second-tab/no-community read-only behavior, post transaction interruption
   including hard page termination immediately before/after
   bundle+manifest+draft commit, first-join transaction
   failure/identity discard, recovery-queue mutation blocking, the exact
   `TransportPortV1` state contract, an in-memory fake proving bounded opaque
   frame exchange, unavailable reserved adapters with zero mutation, and the
   file picker/download adapter as the only concrete implementation. Real
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
   or Nym packages, no domain/endpoint in signed identity/projection, and no
   implemented transport other than file exchange.

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
    unavailable without blocking local publication. The same record has the
    same authentication, admission, and projection regardless of its local
    transport route.
11. Community identity and deterministic projection contain no renderer domain
    or endpoint. A test fixture can be rendered under two unrelated hostnames
    with byte-identical authenticated history and projection output.
12. All repository quality gates and the composite 100% coverage enforcement
   command pass.

## Future work

- Specify the OAuth recovery authority and scoped Meadowcap device enrollment
  as a separate design with signer-side policy, revocation, audit, and offline
  expiry behavior.
- Specify and implement Nearby, direct community-node, disposable HTTPS
  gateway, and Nym browser adapters independently through `TransportPortV1`;
  do not create a canonical web database or change signed record formats.
- Specify discovery and replicator retention policy separately from transport.
  A viewer must never become a replicator implicitly.
- Build stateless/disposable renderer deployments that rebuild projections from
  signed replicated records and prove domain-independent community identity.
- Threat-model private groups, signed PWA releases, unlock UX, plaintext
  lifetime, backups, and storage eviction before exposing encrypted group state
  to the hosted origin.
- Add bundled miniapps only after their existing containment guarantees are
  reproduced in the browser host without granting signing authority.
