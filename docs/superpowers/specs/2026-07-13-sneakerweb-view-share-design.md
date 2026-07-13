# SneakerWeb view-and-share design

Status: Product design approved on 2026-07-13; pending metaswarm design review.

## Purpose

Riot will become a native viewer and carrier for standard SneakerWeb `.snk`
files. A person opens a file as an ordinary document, immediately browses the
sites it contains, keeps those sites in one local library, and can carry one
site, several sites, or a received collection onward. Riot does not create
SneakerWeb domains, hold SneakerWeb domain secrets, edit sites, or publish new
site entries in this slice.

This closes the explicit compatibility question in the dual-mode design. Riot
currently follows SneakerWeb-style path and rendering conventions but is not
wire-compatible: `willow25/drop_format` is disabled and `.riot-evidence` is an
intentionally different development codec. Interoperability is unusually
tractable because Riot and SneakerWeb 1.0.1 use the same protocol
implementation versions: `willow25 0.6.0-alpha.3` and `ufotofu 0.12.4`.

The product principle is: **content first, provenance on demand**. Normal UI
uses human titles and site previews. Complete keys, IDs, signatures, digests,
and source history are always inspectable, copyable, and never truncated, but
they do not dominate the browsing experience.

## User value and use cases

1. **Offline reader:** WHO receives a `.snk` through Files, AirDrop, nearby
   transfer, or a Riot space; WANTS it to open like a document without an
   import ceremony; SO THAT they can read useful sites immediately; WHEN live
   internet access is absent or unsafe.
2. **Community carrier:** WHO has collected useful sites; WANTS to select one,
   several, or a received collection and share it onward; SO THAT information
   keeps moving through people and devices; WHEN they meet someone, use the OS
   share sheet, or participate in a Riot space.
3. **Riot space member:** WHO sees a SneakerWeb collection shared in a space;
   WANTS a clear card and on-demand download; SO THAT large content does not
   silently consume storage; WHEN at least one nearby participant still holds
   the attachment.
4. **Careful verifier:** WHO needs to establish provenance; WANTS full public
   keys, namespace IDs, entry metadata, integrity results, and source history;
   SO THAT cryptographic integrity is inspectable without being confused with
   publisher trust; WHEN they open Details.
5. **Local moderator:** WHO encounters a domain they do not want; WANTS to
   block it once; SO THAT later files cannot silently restore it to normal
   browsing; WHEN viewing a site or its Details.

## Success and failure criteria

The first release succeeds when:

- a file exported by the official SneakerWeb 1.0.1 CLI opens and persists in
  Riot on both iOS and Android;
- an export containing Riot-selected complete sites imports successfully into
  that CLI with exact original Willow entries and payloads;
- a user reaches site content without choosing entries, approving keys, or
  understanding Willow terminology;
- 100 distinct files carrying overlapping sites converge into one durable
  library without duplicate site cards or loss across relaunch;
- a site, multi-site selection, and received collection can each be shared via
  the system share sheet, nearby transfer, and a Riot space carrier card;
- raw keys and IDs are absent from the default library and reader but fully
  visible and copyable through Details;
- invalid, cancelled, oversized, or interrupted files leave the authoritative
  collection byte-for-byte unchanged;
- a 100 MiB, 1,000-entry interoperability fixture opens in at most 10 seconds
  on a contemporary physical release-build phone, and a 1,000-site library
  becomes interactive within 300 ms after database open; measurements are
  taken across ten runs before release and reported as median and worst case.

The slice fails if it accepts forged content, partially commits a rejected
file, leaks a Riot bridge or external network access to site JavaScript, loses
full identifiers, requires per-site approval to read, silently downloads a
space attachment, or creates/re-signs a SneakerWeb site entry.

## Scope

### Included

- standard Willow Drop Format decoding and encoding for the SneakerWeb fixed
  communal namespace;
- transparent atomic open-and-merge from platform document entry points;
- one durable library with site and received-collection views;
- offline site browsing, `index.html` fallback, and `sneakerweb.html` previews;
- complete provenance and integrity Details;
- local persistent domain blocking;
- selection and streaming `.snk` export;
- system share sheet and nearby person transfer;
- content-addressed, on-demand `.snk` attachments referenced by Riot space
  cards;
- iOS and Android native surfaces over the shared Rust core.

### Excluded

- creating a SneakerWeb domain or storing its secret;
- publishing, editing, deleting, or signing SneakerWeb site entries;
- treating cryptographic validity as identity, trust, accuracy, or safety;
- private or encrypted SneakerWeb content;
- automatic insertion of SneakerWeb pages into Riot spaces;
- automatic downloading of space attachments;
- arbitrary Willow Drop Format namespaces;
- incomplete/partial-payload drops. The MVP accepts complete payload-bearing
  `.snk` files produced by the SneakerWeb CLI. General slice-range persistence
  remains a later Willow Store-adapter feature;
- search, full-text indexing, bookmarks, or remote discovery;
- running the upstream CLI or its desktop filesystem store inside the app.

Creating a new `.snk` container from already-authorised entries is in scope and
is not publishing: Riot preserves the original entries, capabilities,
signatures, timestamps, and payloads exactly and adds no SneakerWeb signature.
Sharing into a Riot space does create a separately signed Riot carrier record;
that record says who recommended the package, not who authored its sites.

## User experience

### Open and browse

The OS registers Riot for `.snk`. Opening a file shows a single `Opening…`
state with determinate byte progress where the platform supplies a length.
There is no preview, key approval, domain checklist, or Import button. After
full validation and an atomic merge, Riot opens the received collection. The
same flow applies to a nearby transfer or a downloaded space attachment.

The SneakerWeb library has two presentations over the same store:

- **Sites** is the default grid/list of all unblocked domains, deduplicated by
  full subspace ID and ordered by most recent local receipt. Alternative sort
  controls are outside the MVP.
- **Received** lists source packages by human-safe filename, received time,
  file digest, and the domains that package contributed. Overlapping packages
  do not duplicate the underlying sites.

A site card uses `sneakerweb.html` when available. Otherwise Riot parses the
first HTML `<title>` from a complete `/index.html` without executing content,
decodes character references, removes control characters, collapses
whitespace, and limits display to 80 Unicode scalar values. An absent, empty,
malformed, or non-HTML title becomes `Untitled site`. The full domain key is
never substituted as the default title.

Opening a card loads `/index.html`. Relative navigation remains within the
site. A canonical link to another collected SneakerWeb domain opens that domain
in the same viewer. If the target is absent, the reader says `You don't have
this site yet` and offers to copy the complete target ID. External web links do
not load inside the SneakerWeb viewer; an explicit user gesture may hand the
fully displayed URL to the system browser after confirmation.

### Inspect provenance

Every site and received collection has a quiet `Details` action. Its first
level contains:

- the full, copyable 64-character lowercase-hex domain/subspace public key;
- the full, copyable SneakerWeb communal namespace ID;
- `Content is intact` or a typed integrity failure, never `Trusted` or
  `Verified publisher`;
- latest Willow content timestamp, clearly labelled as publisher-supplied;
- source package filename, digest, route, and local receipt time.

An Advanced disclosure lists each live entry's full path, timestamp, payload
length, WILLIAM3 digest, canonical entry identity, capability identity, and
signature result. Identifiers are never truncated in text, copy output,
accessibility values, logs intended for export, or share manifests.

### Share

Share is available on one site, a multi-selection, and a Received collection.
For a Received collection, Riot exports the **current locally known complete
versions** of the package's domain set; it does not claim byte identity with
the original file. The destination sheet offers:

- **Share file** through the platform share sheet;
- **Send nearby** to a directly selected person;
- **Share to a space** to create a carrier card in a selected Riot space.

The generated filename uses a sanitized human title plus `.snk`; the full file
digest remains in Details. Export is streamed to a protected temporary file and
deleted after the destination completes or cancels.

A Riot space carrier card reads, for example, `Street Medic Library · 4 sites
· 18 MB`, plus the sharing member's ordinary Riot identity and optional note.
The signed record contains a content digest, encoded length, complete domain
IDs, and non-authoritative display labels. It does not inline the `.snk`.
Recipients tap `Get collection`; Riot asks current nearby peers for the blob by
digest, shows transfer progress, verifies the digest and Drop Format, merges it,
and opens it. Any peer that holds the complete blob may carry it onward. A card
whose blob is absent says `Find someone nearby who has this collection`.

### Block

`Block this site` is available from the site menu and Details, with a plain
confirmation naming the human title and showing the full ID on disclosure.
Blocking immediately removes the domain from normal browsing and selection,
deletes its render cache, and prevents later files or attachments from making
it visible. Accepted entry/provenance records and the block record remain for
inspection. Unblocking recomputes the current live site from retained entries;
it does not fetch or publish anything.

### Plain-language failures

| Condition | Default message |
| --- | --- |
| Malformed, truncated, forged, wrong-namespace, or incomplete file | `This file couldn't be opened.` |
| Storage or collection quota exceeded | `There isn't enough space to open this.` |
| File/nearby transfer interrupted | `The transfer stopped. Try again.` |
| Site has no complete `/index.html` | `This site doesn't have a home page.` |
| Linked SneakerWeb domain is absent | `You don't have this site yet.` |
| Space attachment has no reachable holder | `Find someone nearby who has this collection.` |
| Viewer process fails or exhausts its budget | `This site stopped working.` |
| Temporary export or destination fails | `This couldn't be shared. Try again.` |

Details exposes a stable typed error code and safe structural facts. It never
includes attacker-controlled payload text, secret material, or a truncated ID.

## Architecture

### Boundaries

SneakerWeb is a distinct public-content collection in the shared Rust core,
not a Riot mini-app and not an evidence-bundle variant:

```text
.snk / nearby blob / space attachment
                 |
        bounded Drop decoder
                 |
       staging + full verification
                 |
       atomic SneakerCollection merge
          /          |           \
     library      isolated       streaming Drop encoder
      queries       viewer        /       |        \
                               share    nearby    space blob
```

Rust owns decoding, canonical verification, namespace enforcement, Willow join
semantics, persistence, selection, Drop encoding, block policy, provenance,
and attachment digests. Swift and Kotlin own platform document entry points,
WebViews, OS sharing, and nearby lifecycle, calling versioned UniFFI types.
HTML/JavaScript never receives a database, filesystem, namespace-selection, or
native bridge API.

### Core components and contracts

`riot-core::sneakerweb` contains:

- `SneakerProtocol`: the fixed namespace bytes, domain encoding, canonical URL
  rules, `/index.html` fallback, and protocol version metadata;
- `SnkDecoder`: a streaming Willow Drop decoder that produces bounded staged
  authorised entries only after verifying canonical encodings, Meadowcap
  authority, entry signatures, payload length, WILLIAM3 digest, fixed
  namespace, path limits, and complete payload availability;
- `SneakerCollection`: namespace-scoped atomic join, source receipts, block
  policy, site queries, entry/resource resolution, and full Details models;
- `SnkEncoder`: streaming export of the current complete live entries for an
  explicit set of full subspace IDs, with no mutation or re-signing;
- `PortableBlobStore`: content-addressed `.snk` attachment storage, reference
  accounting, temporary export leases, chunk verification, and resumable
  reads;
- `SneakerCarrier`: canonical Riot carrier-card payload validation and mapping
  between a signed space entry and one portable blob digest.

The versioned FFI surface uses opaque handles and typed values. Native document
providers are never exposed as arbitrary paths to Rust: the host opens the
security-scoped URL/content URI, starts a bounded task, and streams chunks into
an app-protected core-owned staging file:

```text
begin_snk_open(display_name, route, expected_bytes) -> OpenSnkTask
OpenSnkTask.write_chunk(bytes) -> TransferProgress
OpenSnkTask.progress() -> TransferProgress
OpenSnkTask.finish() -> OpenSnkOutcome
OpenSnkTask.cancel() -> CancelOutcome
list_sneaker_sites(cursor, sort) -> SneakerSitePage
list_received_sneaks(cursor) -> ReceivedSneakPage
get_sneaker_site(domain_id) -> SneakerSite
get_sneaker_details(domain_id, cursor) -> SneakerDetailsPage
resolve_sneaker_resource(domain_id, path) -> ResourceLease
block_sneaker_domain(domain_id) -> BlockOutcome
unblock_sneaker_domain(domain_id) -> BlockOutcome
create_snk_export(domain_ids) -> SnkExportTask
create_space_sneaker_share(space_session, blob_digest, note) -> CarrierReceipt
request_portable_blob(blob_digest, peer) -> BlobTransferTask
```

Every domain parameter is exactly 32 bytes at the FFI boundary; string parsing
exists only for platform URLs and Details copy/paste. Pagination and tasks are
bounded, cancellable, and invalid after their owning database/session closes.
No API accepts a caller-supplied namespace for SneakerWeb operations.

### Persistence

This feature depends on the approved multi-space SQLite design. SneakerWeb uses
the existing canonical `accepted_entries`, `live_entries`, path-prefix,
payload, and receipt model under the protocol's exact fixed namespace. It does
not use the Phase 0 in-memory `EvidenceStore`, its closed schema decoder, or its
1 MiB per-item limit.

The SQLite design gains:

| Table | Purpose |
| --- | --- |
| `sneaker_sources` | File digest, safe display name, encoded size, route, receipt time, outcome, and complete domain count. |
| `sneaker_source_domains` | Exact domain membership of one accepted source package. |
| `sneaker_blocks` | Full domain ID, local block state, time, and optional local note. |
| `portable_blobs` | SHA-256 digest, length, protected local path/blob handle, completeness, reference count, and last access. |
| `space_blob_refs` | Space namespace, signed carrier entry, portable blob digest, state, and local availability. |
| `blob_chunks` | Resumable chunk index, length, chunk digest, and completion state for in-flight nearby transfers. |

The authoritative entry tables retain full canonical entry, capability,
signature, payload, coordinate, and source receipt references. `live_entries`
contains only Willow winners. Source rows survive overlap and pruning so
Details can explain where data came from. Blocking is a local read/share policy
and never rewrites Willow history.

An open operation decodes into a protected temporary staging database/file
outside the authoritative tables. After the decoder reaches a canonical end,
the core computes the complete join and quota delta against one database
snapshot. One serialized SQLite transaction revalidates the snapshot, inserts
accepted/live/provenance rows, applies pruning, records the source, and updates
blob references. Any error or cancellation deletes staging and commits
nothing. Notifications publish only after commit.

### Resource budgets

The first release has explicit acceptance ceilings, enforced while streaming
before allocations or integer conversions:

- encoded `.snk` or portable blob: 1 GiB;
- decoded payload bytes per open/export: 1 GiB;
- domains per file: 1,024;
- entries per file: 50,000;
- one resource payload: 256 MiB;
- one path: 4,096 bytes, at most 256 components, at most 1,024 bytes per
  component;
- total retained SneakerWeb payloads and blobs: 2 GiB by default, with at least
  512 MiB of device free space left untouched;
- Details page: 100 entries; library page: 100 sites;
- nearby blob chunk: 256 KiB with a SHA-256 digest and a 32 MiB checkpoint.

The core accepts a lower platform storage ceiling but never a higher one than
the release constants. Exceeding a budget is a typed non-mutating rejection.
Export refuses a selection whose complete current payloads are unavailable or
whose encoded result would exceed the same 1 GiB ceiling.

### Interoperability and merge semantics

The exact SneakerWeb namespace is the 32 bytes published by the protocol. Each
domain is one Willow subspace. Paths map UTF-8 URL components to Willow path
components; percent-decoding occurs exactly once and invalid UTF-8 or ambiguous
normalisation is rejected. A trailing slash resolves to `index.html`; an
extensionless path is tried exactly before the optional `/index.html`
fallback, matching current SneakerWeb behavior.

Accepted entries use Willow's existing order-independent pruning and recency
rules. A source may contain older data and still produce a successful receipt;
it simply does not replace newer live resources. Publisher-supplied timestamps
are never displayed as local receipt time or used as a trust signal. Riot
exports the complete current live entries for every selected domain in
canonical order. Blocked domains are neither resolved nor exportable through
normal APIs.

Round-trip conformance compares canonical entry bytes, capability bytes,
signatures, payloads, paths, timestamps, and full IDs—not merely rendered HTML
or site counts. The official SneakerWeb 1.0.1 CLI is pinned as an external
test oracle; a checked-in, license-compatible fixture prevents network access
from being required during ordinary tests.

### Isolated renderer

SneakerWeb content never enters `AppWebViewHost`, `AppSchemeHandler`, or the
`window.riot` bridge. A separate native `SneakerWebViewHost` talks only to a
read-only resource resolver.

To preserve canonical `http://<domain>.localhost:1312/...` links, a small
read-only server binds loopback only while a reader is active. It accepts only
`GET` and `HEAD`, requires an exact `sneakerweb.localhost:1312` or
`<64-hex-domain>.localhost:1312` Host, rejects request bodies and excess
headers, percent-decodes once, resolves through the Rust store, and never maps
paths to the filesystem. Failure to bind port 1312 fails closed with a typed
`VIEWER_UNAVAILABLE`; Riot does not silently start a remotely reachable
listener or rewrite site bytes.

Every resource response includes `X-Content-Type-Options: nosniff`, a
content-type derived only from the resolved path, and this minimum policy
(platform syntax may add stricter equivalent directives):

```text
default-src 'self' data: blob:;
script-src 'self' 'unsafe-inline' blob:;
style-src 'self' 'unsafe-inline';
connect-src 'none'; object-src 'none'; frame-src 'none';
worker-src 'none'; base-uri 'self'; form-action 'none'
```

This permits same-site static resources and inline site script/style
compatibility while denying external connections, workers, forms, objects, and
frames. `unsafe-eval` is not allowed. Native WebView request interception is a
second independent network deny. JavaScript is allowed, but WebView file and
content access, geolocation, media capture, notifications, downloads, popups,
new windows, password storage, Safe Browsing URL reporting, service workers,
and native message handlers are disabled. The data store is non-persistent.
Top-level canonical cross-domain links are mediated by native navigation;
cross-domain subresource reads are blocked.

`sneakerweb.html` cards render in a sandboxed, scriptless, opaque-origin frame
with fixed size and execution/time limits. Site rendering runs in a disposable
WebView process. A crash, memory termination, or repeated main-thread stall
closes that reader without affecting the store or other Riot views.

### Space attachments and nearby transport

A carrier record is a closed, canonical CBOR Riot object containing schema ID,
SHA-256 blob digest, encoded length, complete full domain IDs, site count,
sanitized display labels, optional bounded note, and sharing member attribution.
The existing space signer signs that wrapper. The wrapper cannot claim or
replace the embedded sites' authorship and never embeds a SneakerWeb secret.

The `.snk` bytes live in `PortableBlobStore`, deduplicated by SHA-256. Space
entry reconciliation carries only the small record. Blob retrieval is an
explicit second protocol over the established nearby connection: peers
advertise held digests, a recipient requests one digest, and fixed chunks are
written to staging with per-chunk and final-file verification. Local TCP is
preferred; BLE remains a resumable fallback with progress and cancellation.
The Drop decoder still verifies the completed blob before collection commit,
so blob hashing is not substituted for Willow authorisation.

System sharing exports the same standard bytes without a carrier wrapper.
Direct nearby sharing may transfer an export without first posting a space
record. Space deletion or hiding removes its reference, not a blob still
referenced by another space, source, or active export lease. Unreferenced blobs
are garbage-collected only after a grace period and never while a transfer or
reader holds a lease.

## Security model

### Assets and adversaries

Protected assets are Riot identities and private-space data, integrity of the
SneakerWeb collection, availability/storage, accurate provenance display,
local block policy, and the guarantee that carrying content does not impersonate
its publisher. Adversaries include a malicious `.snk` author, a mutating or
truncating carrier, hostile HTML/JavaScript, oversized or algorithmically
pathological encodings, a peer lying about held blobs, a forged space card,
and a site that attempts to probe other sites, Riot data, the LAN, or internet.

### Controls and residual risks

- Canonical Willow/Meadowcap/WILLIAM3 verification precedes acceptance; SHA-256
  addresses transport blobs but does not replace protocol verification.
- Fixed namespace enforcement prevents a `.snk` from smuggling Riot-space
  entries into the public collection.
- File-wide staging and one transaction prevent partial acceptance.
- Streaming budgets, checked arithmetic, bounded queues, cancellation, and
  disk reservation limit resource exhaustion.
- The viewer is read-only, network-denied, non-persistent, bridge-free, and
  separated from the mini-app runtime.
- Full IDs and separate `intact`, `publisher-supplied time`, `received`, and
  `shared by` labels prevent trust/provenance conflation.
- Space carrier records are accepted only through ordinary verified space
  entry rules; blob digest, length, and domain set are rechecked after transfer.
- Block state is checked at commit, query, render, export, and attachment-open
  boundaries.
- Logs contain route, lengths, stable error codes, and full IDs only in an
  explicit user-exported diagnostic; ordinary logs use internal correlation
  IDs and never payload text.

Residual risks are explicit. A correctly authorised publisher may distribute
lies, disturbing material, or hostile-but-sandboxed code. Signatures do not
establish a human identity. A carrier may withhold content. Publisher clocks
may be inaccurate. OS WebView defects and a fully compromised device remain
outside the app boundary. Public `.snk` bytes are not confidential from a
carrier or space member who downloads them.

## TDD and verification

`.coverage-thresholds.json` remains the source of truth: 100% lines, branches,
functions, and statements, enforced by `cargo tarpaulin --fail-under 100`.
Implementation follows small RED-GREEN-REFACTOR slices:

1. **Protocol constants and decoder:** failing official-fixture and hostile
   corpus tests; then fixed namespace, complete-payload decoder, canonical
   verification, and bounded typed errors.
2. **Atomic collection:** failing merge, overlap, rollback, cancellation,
   quota, block, and restart tests; then staged SQLite commit and queries.
3. **Encoder interoperability:** failing selected-domain and CLI round-trip
   tests; then streaming export with exact-byte assertions and cleanup.
4. **FFI contracts:** failing lifecycle, pagination, cancellation, full-ID,
   stale-handle, and panic-containment tests; then opaque tasks and typed DTOs.
5. **Renderer:** failing iOS and Android tests for canonical paths, MIME/CSP,
   absent home pages, external requests, forms, popups, permissions, service
   workers, cross-site reads, process failure, and absence of every Riot bridge;
   then the isolated hosts and loopback resolver.
6. **Library UX:** failing UI/accessibility tests for zero-ceremony opening,
   Sites/Received deduplication, hidden-by-default identifiers, complete Details,
   loading, empty, block, missing-link, and failure states; then native views.
7. **Sharing:** failing exact selection, temporary-file cleanup, carrier schema,
   blob deduplication, unavailable-holder, chunk corruption, resume, cancel,
   garbage-collection lease, and multi-peer handoff tests; then each route.
8. **End to end:** official CLI -> Riot -> selected export -> official CLI;
   iOS -> Android and Android -> iOS document handoff; two-space carrier-card
   isolation; process-death recovery; offline rendering; physical Files,
   share-sheet, local TCP, and BLE rehearsal.

Test helpers include a pinned official `.snk`, deterministic domain authors,
malformed/truncated/mutated corpus generators, a temporary SQLite database,
fault-injecting staged storage, a fake clock and storage quota, an in-memory
portable-blob peer, loopback HTTP requests, native WebView test pages, and a
CLI harness. Fuzz/property tests cover Drop decoding, URL/path conversion,
carrier CBOR, selection isolation, and arbitrary merge order. No test fixture
contains a production or reusable private key.

Blocking verification before completion:

```text
cargo test --workspace --all-features
cargo tarpaulin --fail-under 100
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
platform iOS/macOS test command from scripts/green.sh
./gradlew test connectedCheck
official SneakerWeb 1.0.1 cross-import harness
physical two-device rehearsal checklist
```

## Delivery slices and dependencies

1. Land or rebase onto the approved multi-space SQLite production store; this
   feature must not deepen the replayed `profile.json` model.
2. Add Drop Format conformance fixtures, bounded decoder/encoder, collection
   schema, and atomic Rust APIs.
3. Add FFI and platform document/library surfaces.
4. Add the isolated renderer and adversarial native tests.
5. Add standard file sharing and direct nearby transfer.
6. Add portable blob transfer and signed Riot space carrier cards.
7. Run cross-client, coverage, performance, platform, and physical-device gates.

Slices 2-4 produce useful open/browse capability. Slice 5 completes person-to-
person carrying. Slice 6 completes space sharing. No slice may claim `.snk`
support until both import and export pass the official CLI round trip.

## Definition of done

- All included user journeys work without publishing tools or raw-ID-first UI.
- The official CLI round trip and exact-byte assertions pass.
- Invalid input, cancellation, quota failure, and crash injection prove atomic
  non-mutation.
- The isolated renderer passes every network/bridge/permission adversarial test.
- Full provenance is inspectable and identifiers are never truncated.
- System, nearby, and Riot-space sharing preserve original site authority.
- Blocking persists and is enforced at every ingress/egress/read boundary.
- iOS and Android persist the same collection semantics across relaunch.
- The repository's full test, lint, formatting, platform, coverage, performance,
  and physical-device gates pass.
- User-facing docs explain `Content is intact` versus publisher trust and the
  public nature of shared `.snk` content.
