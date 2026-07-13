# SneakerWeb view-and-share design

Status: Product design approved on 2026-07-13. Metaswarm design review round 1
returned NEEDS_REVISION; this revision incorporates all blocking findings and
is pending round 2.

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

### Narrow safety exception to preview-before-ingest

The product brief's default remains `No automatic import of arbitrary packet
data` and `Preview before ingest`. A user deliberately opening a `.snk`
document is a narrow exception, not a repeal:

- the OS-level open, accepted nearby receive, or explicit `Get collection`
  action is the consent gesture;
- only the one fixed public SneakerWeb namespace is accepted;
- every entry and complete payload is verified before an atomic commit;
- content receives no Riot-space authority and runs only in the isolated
  renderer;
- a post-open summary makes the durable effect visible and offers `Undo`;
- Storage management permits later removal and precise space recovery.

Files discovered passively, synced carrier cards, and nearby advertisements
never ingest or download themselves. Any future namespace, private content, or
less constrained document type remains subject to preview-before-ingest.

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
6. **Storage-limited reader:** WHO has filled the local collection; WANTS to see
   reclaimable bytes, remove a receipt/site/blob, and retry; SO THAT automatic
   persistence remains reversible; WHEN Open or Get collection reports that
   space is insufficient.

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

Before release, at least ten people who did not build Riot complete a scripted
airplane-mode receive -> open -> find a named page -> share -> second-device
open journey using 10 MiB and 100 MiB fixtures. At least 9/10 must complete the
10 MiB journey without coaching within two minutes, at least 8/10 must complete
the 100 MiB journey within five minutes over local Wi-Fi/system sharing, and
19/20 attempted recipient deliveries across the cohort must arrive intact and
open. Any lower completion or delivery result blocks the field-readiness claim
and requires a recorded UX/transport revision; install counts are not evidence.

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

### Navigation placement

The SneakerWeb library is global, not scoped to the selected Riot community.
It does not become a fifth community tab. On iPhone and Android, the global
profile/avatar menu contains a labelled `SneakerWeb library` row alongside
`Your profile`; on macOS it is a global sidebar item above the community
destinations. This preserves the approved Home, Tools, People, and Nearby
community shell.

Opening an external `.snk` pushes the global Received collection route. Back
returns to the originating app/community when one exists, otherwise to the
library. A reader toolbar provides Back, Forward, Library, Details, and Share.
Back/Forward preserve site identity, path, and scroll position for the current
reader session. Library returns to the prior Sites/Received position and focus.

### Open and browse

The OS registers Riot for `.snk`. Opening a file shows `Opening…`, Cancel, and
determinate byte progress when length is known; otherwise it uses an
indeterminate indicator plus bytes received. Leaving the foreground may
continue an OS-granted document read only while the bounded task remains valid;
if the OS suspends it, Riot cancels staging and offers Retry on return. Cancel
deletes staging and restores focus to the invoking screen.

There is no key approval, domain checklist, or Import button. After full
validation and atomic merge, Riot opens the Received collection and announces
`N added, N updated, N unchanged, N blocked · X MB stored`, with Undo available
until another collection mutation begins or for at least 30 seconds. Undo
removes that receipt and content for which it is the only retained source;
overlapping content remains and the summary says so. An accepted nearby transfer
or downloaded attachment uses the same flow.

A valid file with no visible change has an explicit result: `Already up to
date`, `This collection only contains older versions`, `All sites in this
collection are blocked`, or `This collection is empty`. The Received receipt
remains inspectable unless removed. Retry appears for every retryable read,
transfer, storage, and temporary failure; cryptographic/wrong-namespace failure
is non-retryable unless the bytes change.

The SneakerWeb library has two presentations over the same store:

- **Sites** is the default grid/list of all unblocked domains, deduplicated by
  full subspace ID and ordered by most recent local receipt. Alternative sort
  controls are outside the MVP.
- **Received** lists source packages by safe filename, receipt time, file
  digest, and every domain named by the package, including unchanged, older,
  and blocked dispositions. Overlap never duplicates a Site card; original
  package membership does not imply that every member changed live state.

The first-use Sites empty state says `Open a .snk file to collect offline
sites`, with Open File and receive guidance. Received says `Collections you
open will appear here`. Pagination shows a native loading row, Retry on failure,
and keeps the last stable page visible while retrying.

A site card uses `sneakerweb.html` as a decorative preview when available.
Independently, Riot parses the first HTML `<title>` from a complete
`/index.html` without executing content, decodes character references, removes
control and bidi-control characters, collapses whitespace, and limits display to 80 Unicode
scalar values. An absent, empty, malformed, or non-HTML title becomes `Untitled
site`; the full domain key is never the default title.

Opening a card loads `/index.html`. Relative navigation remains within the
site. A canonical link to another collected domain opens it in the same reader.
An absent target says `You don't have this site yet` and offers to copy its
complete ID. A blocked target says `This site is blocked` and offers Details,
never implicit unblock. External links do not load inside the viewer; the
strict handoff contract is defined under Security.

### Inspect provenance

Every Site and Received collection has a quiet `Details` action. Site Details
first shows:

- full, copyable 64-character lowercase-hex domain/subspace public key;
- full, copyable SneakerWeb communal namespace ID;
- `Content is intact`, never `Trusted` or `Verified publisher`;
- latest publisher-supplied Willow timestamp;
- every source package filename, digest, route, and local receipt time.

Advanced Site Details lists each accepted/live entry's full path, timestamp,
payload length, WILLIAM3 digest, canonical entry identity, capability identity,
signature result, every associated source receipt, and its duplicate/older/
winner/pruned disposition. Received Details lists exact original membership,
every source entry and receipt-time disposition, current-live contribution,
file-level result, and whether a current export differs. `Updated since
received` appears when current entries differ from the received versions.
Identifiers are never truncated in visual text, copy output, accessibility
values, diagnostic exports, or share manifests.

### Share

Share is available on one Site, a multi-selection, and a Received collection.
Received exports the current complete versions of its currently unblocked
domain set, not original file bytes. If blocked domains were present, review
says `N blocked sites won't be included`; if none remain, Share is disabled
with a path to Blocked sites. `Updated since received` prevents byte-identity
confusion. Destinations are:

- **Share file** through the platform share sheet;
- **Send nearby** through the current public community's Nearby surface;
- **Share to a space** through a selected public/open Riot space.

The filename uses a sanitized title, or
`sneakerweb-collection-YYYYMMDD-HHMMSS.snk` when no safe title remains; it never
surfaces a shortened key or digest. Existing destination files receive the
platform's normal collision suffix. Export streams to a protected temporary and deletes it
after completion, cancellation, lease expiry, or startup recovery.

System share shows the public-content disclosure, then the OS sheet. Send
nearby requires the existing bilateral confirmation on both phones and is
disabled with guidance if permission, a selected public community, or an
eligible confirmed person is absent. Share to a space lists only public/open
spaces where the profile may write, then reviews title, site count, size,
optional note, `You are sharing this collection; you did not author its sites`,
and Publish/Cancel. Private/encrypted spaces are excluded from the MVP.
Permission denial, peer disappearance, cancellation, and failure preserve the
selection and offer Retry or another destination.

A space card reads `Street Medic Library · 4 sites · 18 MB`, shows identity
derived from the outer signed entry, and repeats that the member carried rather
than authored the sites. Its states are Available locally/Open; Not downloaded/
Get collection; Waiting for nearby holder/Retry; Transferring with progress and
Cancel; Interrupted/Resume; Verifying; Invalid/Remove card locally; Storage
full/Manage storage; All domains blocked/View blocked sites; and Ready/Open.
Cards never auto-download. Any currently authorised nearby holder of the public
space attachment may serve it after an explicit request.

### Block, undo, and storage recovery

`Block this site` is available from the site menu and Details. Confirmation
uses the human title with full ID on disclosure. Blocking removes the domain
from browsing/share, closes its active reader, cancels exports/transfers that
contain it, deletes render cache, and prevents later packages from restoring
visibility. Accepted provenance and the Block record remain inspectable.

The library menu contains `Blocked sites` and `Storage`. Blocked sites uses
human titles plus a key-derived accessibility disambiguator and exposes retained
Details, Unblock, and Remove local data. Unblock recomputes current live content
without fetching or publishing.

Storage lists Sites, Received receipts, and portable blobs by reclaimable byte
count. `Remove received record` deletes that receipt/associations; content held
by another receipt remains. `Remove site from this device` deletes that domain's
payloads, accepted/live rows, caches, and source-entry associations while
keeping a compact removal audit row and any independent Block; a future open may
collect it again unless blocked. `Remove downloaded attachment` deletes an
unleased blob while its carrier card becomes Not downloaded. Confirmations state
exact reclaimable bytes and overlap that remains. Removal is one transaction,
releases quota immediately, and returns to the failed task's Retry path.

### Accessibility contract

Sites, Received, Blocked sites, Details, and Storage use semantic screen titles
and headings; platform segments expose selected state. A card's native
accessibility label combines human title, freshness, availability, and a
non-secret two-word disambiguator deterministically derived from the full domain
key (for example, `amber river`; it is not a shortened identifier and carries
no trust meaning). The same phrase appears visually only when duplicate titles
need disambiguation. Decorative `sneakerweb.html` never supplies the accessible
name; Details always exposes the complete raw key.

Selection announces changes and total count. Progress announces start, every
10% or 10 seconds (whichever is less frequent), and terminal result without
stealing focus. Closing Details/share/errors/readers restores invoking focus.
Dynamic Type/font scaling, reflow, 44x44-point targets, keyboard navigation,
visible focus, high contrast, non-color status, and Reduce Motion are required.
Site-authored content may be inaccessible, but Riot chrome, provenance,
fallback title, sharing/blocking, and failure recovery always remain accessible.

### Plain-language failures

| Condition | Default message and action |
| --- | --- |
| Malformed, truncated, forged, wrong-namespace, or incomplete file | `This file couldn't be opened.` Details/Close |
| Storage or collection quota exceeded | `There isn't enough space to open this.` Manage storage/Retry |
| File/nearby transfer interrupted | `The transfer stopped.` Resume or Retry/Cancel |
| Site has no complete `/index.html` | `This site doesn't have a home page.` Details/Back |
| Linked domain absent | `You don't have this site yet.` Copy full ID/Back |
| Space attachment has no reachable authorised holder | `Find someone nearby who has this collection.` Retry |
| Viewer process fails or exhausts its budget | `This site stopped working.` Retry/Close |
| A previously accepted local payload fails its digest on read | `This local copy is damaged.` Remove local copy/Open another file |
| Temporary export or destination fails | `This couldn't be shared.` Retry/Choose another |

Details exposes a stable typed code and safe structural facts. It never includes
payload text, secret material, raw OS URI/path, or a truncated identifier.

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

The fixed SneakerWeb namespace is permanently reserved with collection kind
`sneakerweb-public`. `RiotDatabase` rejects creating/joining it as a Riot space,
and `SpaceSession`, app/document projections, ordinary space sync, trust lists,
and space change feeds reject or ignore it by invariant. Conversely, the
SneakerCollection route accepts no other namespace. Database CHECK constraints,
repository constructors, migration tests, and FFI factories enforce this split;
path text such as `apps/...` inside SneakerWeb can never enter Riot app
projections.

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

The codec uses upstream `willow25::drop_format::{DropDecoder, DropEncoder}`
directly with Riot-owned bounded producers/consumers. It does **not** call
upstream `import_drop` against authoritative tables or `export_drop` with an
unbounded elastic queue. Riot independently stages and verifies every decoded
entry/payload, and independently queries selected complete entries for the
encoder. A pre-implementation conformance work unit enables the exact pinned
feature and proves official CLI vectors before application code proceeds.

All FFI DTOs carry `schema_version = 1`; identifiers are fixed-width bytes and
errors are closed enums with `retryable`, `recovery_action`, and safe detail
fields. Calls execute on Riot's serialized database worker and never the native
UI thread. Page cursors contain an opaque signed `(database_generation,
collection_generation, last_sort_key)` snapshot; mutation makes them
`STALE_CURSOR`, after which native restarts from page one while retaining the
visible page.

Native document providers are never exposed as arbitrary paths to Rust. The
host opens the security-scoped URL/content URI and streams chunks into a
core-owned, app-protected, backup-excluded staging handle:

```text
begin_snk_open(display_name, route, expected_bytes) -> OpenSnkTask
OpenSnkTask.write_chunk(bytes) -> TransferProgress
OpenSnkTask.progress() -> TransferProgress
OpenSnkTask.finish() -> OpenSnkOutcome
OpenSnkTask.cancel() -> CancelOutcome
list_sneaker_sites(cursor) -> SneakerSitePage
list_received_sneaks(cursor) -> ReceivedSneakPage
get_sneaker_site(domain_id) -> SneakerSite
get_sneaker_details(domain_id, cursor) -> SneakerDetailsPage
get_received_sneak_details(source_id, cursor) -> ReceivedSneakDetailsPage
resolve_sneaker_resource(domain_id, path) -> ResourceLease
block_sneaker_domain(domain_id) -> BlockOutcome
unblock_sneaker_domain(domain_id) -> BlockOutcome
remove_sneaker_source(source_id) -> RemovalOutcome
remove_sneaker_site(domain_id) -> RemovalOutcome
create_snk_export(domain_ids) -> SnkExportTask
create_space_sneaker_share(public_space_session, export_lease, note) -> CarrierReceipt
request_space_blob(public_space_session, carrier_entry_id, peer) -> BlobTransferTask
begin_direct_snk_send(confirmed_nearby_session, export_lease) -> BlobTransferTask
```

Every domain parameter is exactly 32 bytes at the FFI boundary; string parsing
exists only for canonical URLs and Details copy/paste. No SneakerWeb API accepts
a caller-supplied namespace.

Opaque objects have complete state machines:

- `OpenSnkTask`: `Receiving -> Validating -> Committing -> Completed`, with
  terminal `Cancelled | Failed`. `write_chunk(max 256 KiB)` is legal only while
  Receiving; `finish` checks exact expected length and canonical termination;
  `cancel` is idempotent. Repeated `finish` returns the same terminal outcome.
- `SnkExportTask`: `Selecting -> Encoding -> Ready(ExportLease)`, with terminal
  `Cancelled | Failed`. It exposes progress/cancel/finish. `finish` yields an
  `ExportLease` rather than bytes or a filesystem path.
- `ExportLease`: metadata plus `read_range(offset, max <= 1 MiB)`,
  `retain_as_portable_blob()`, and idempotent `close`. Retain fsyncs and promotes
  the exact export into CAS; system share/native nearby may stream without
  promotion. Expiry or close makes reads `LEASE_CLOSED`.
- `ResourceLease`: immutable MIME/length/digest/block-generation metadata,
  bounded `read_range`, and idempotent `close`; every read rechecks its block
  generation.
- `BlobTransferTask`: `Negotiating -> Transferring -> Verifying -> Ready`, with
  `Paused | Cancelled | Failed`; it exposes progress, pause/resume, cancel, and
  finish. Ready yields a content lease or invokes the ordinary Open task; it
  never directly mutates SneakerCollection.

All tasks are owned by a database generation and, when relevant, a public
`SpaceSession`/nearby-session generation. After a terminal result, cancellation
returns `AlreadyTerminal`, writes/reads return the terminal typed error, and
close/drop only cleans resources. A database/session close before terminal
wins and returns `OWNER_CLOSED`. Finish-versus-cancel and block-versus-read are
linearized by the serialized worker: the first accepted command wins, except a
newer block generation always prevents publication of an export/carrier. A
process death has no callable outcome; startup reconciliation performs the
same cleanup before new tasks start.

The minimum DTO contracts are:

| DTO | Required fields |
| --- | --- |
| `SneakerSite` | version, full 32-byte domain, native title, complete/index/preview flags, unblocked state/generation, current bytes/entry count, latest publisher timestamp, latest local receipt, source count |
| `ReceivedSneak` | version, opaque source ID, safe filename, full SHA-256, encoded bytes, receipt route/time, exact domain counts by disposition, current-export-differs flag |
| `SneakerDetailsPage` | version, full domain/namespace, integrity vocabulary, publisher timestamp, source-history rows, live/accepted entry rows, next cursor |
| `ReceivedSneakDetailsPage` | version, source facts, exact original domain/entry membership, receipt dispositions, current contribution/difference, next cursor |
| `TransferProgress` | version, phase, completed bytes/items, optional bounded total, resumable/cancellable booleans |
| `OpenSnkOutcome` | version, receipt ID, added/updated/unchanged/blocked/older counts, retained/reclaimable bytes, undo generation, zero-change reason |

Stable core errors are `INVALID_DROP`, `WRONG_NAMESPACE`,
`INCOMPLETE_PAYLOAD`, `LIMIT_EXCEEDED`, `STORAGE_FULL`, `STALE_CURSOR`,
`OWNER_CLOSED`, `BLOCKED`, `LEASE_CLOSED`, `INVALID_STATE`, `CANCELLED`,
`TEMPORARY_IO`, `VIEWER_UNAVAILABLE`, `NO_AUTHORISED_HOLDER`, and
`SECURITY_POLICY`. Raw parser/SQL/OS text never crosses FFI. Native maps only
these enums and typed fields to the user-state matrix above.

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
| `sneaker_source_entries` | Many-to-many source/entry association with source-local order and duplicate/older/winner/pruned disposition. |
| `sneaker_sites` | Rebuildable domain projection: safe title, index/preview presence, latest receipt, complete bytes, block generation, and current entry count. |
| `sneaker_blocks` | Full domain ID, local block state, time, and optional local note. |
| `sneaker_removals` | Compact local removal audit sufficient for Undo/recovery messaging without retaining removed payloads. |
| `storage_reservations` | Task owner, reserved staging/database/blob bytes, expiry, and recovery state. |
| `portable_blobs` | SHA-256 digest, length, protected local path/blob handle, completeness, reference count, and last access. |
| `space_blob_refs` | Space namespace, signed carrier entry, portable blob digest, state, and local availability. |
| `blob_chunks` | Resumable chunk index, length, chunk digest, and completion state for in-flight nearby transfers. |

The authoritative entry tables retain full canonical entry, capability,
signature, payload, and coordinate. `sneaker_source_entries` preserves every
source association after overlap/pruning; it is not reduced to a first receipt.
`live_entries` contains only Willow winners. `sneaker_sites` avoids payload scans
for the 1,000-site launch budget and can be rebuilt transactionally from live
rows. Blocking is a local read/share policy and never rewrites Willow history.

An open operation decodes into a protected temporary staging database/file
outside the authoritative tables. After the decoder reaches a canonical end,
the core computes the complete join and quota delta against one database
snapshot. One serialized SQLite transaction revalidates the snapshot, inserts
accepted/live/provenance rows, applies pruning, records the source, and updates
blob references. Any error or cancellation deletes staging and commits
nothing. Notifications publish only after commit.

Original document bytes are discarded after a successful ordinary open; source
digest, exact source-entry/domain membership, dispositions, and receipt metadata
remain. A Received share is therefore explicitly reconstructed from current
complete entries. Space/direct-nearby exports become CAS blobs only when an
`ExportLease` is retained for that purpose.

CAS finalization is crash-consistent: reserve quota in SQLite; create a
no-follow random file in the app-protected, backup-excluded CAS staging
directory; stream while checking length/chunks; fsync the file; verify final
SHA-256 and `.snk` length; atomically rename to the digest path; fsync the
directory; then transactionally publish/attach the `portable_blobs` row and
release the reservation. Startup runs before database service: delete expired
staging/chunks/share temporaries, reconcile final files without rows and rows
without files, expire leases/reservations, recompute references, and finish or
roll back pending removals. A complete unreferenced file becomes a bounded
orphan eligible for deletion, never an implicit accepted collection.

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
- accepted SneakerWeb entries: 250,000; source receipts: 10,000;
  source-entry associations: 1,000,000; database/CAS combined: 3 GiB;
- concurrent tasks per device: one Open, one Export, two Blob transfers;
  aggregate staging/chunk reservations: 1 GiB;
- Details page: 100 entries; library page: 100 sites;
- nearby blob chunk: 256 KiB with a SHA-256 digest and a 32 MiB checkpoint.

The core accepts a lower platform storage ceiling but never a higher one than
the release constants. Exceeding a budget is a typed non-mutating rejection.
Export refuses a selection whose complete current payloads are unavailable or
whose encoded result would exceed the same 1 GiB ceiling.

Every task reserves its worst remaining disk growth before writing and updates
that reservation monotonically. Decoder/verification CPU work is cancellable
and has a 60-second active-CPU ceiling per 100 MiB (excluding time waiting for
input); connection, header, and queue limits are separate below. Reservations
and metadata—not only payload bytes—count toward denial-of-service limits.

### Interoperability and merge semantics

The exact SneakerWeb namespace is
`9fc4cc86cad94d11025afcf75e0dab24bc3c6c91f0cd92fbe0ca574d469c681e`
(the 32 bytes published by the protocol). Each domain is one Willow subspace.
URL domain hex accepts either case and normalizes to full lowercase for display;
keys never normalize through a shortened form. Paths map UTF-8 URL components to Willow path
components; percent-decoding occurs exactly once and invalid UTF-8 or ambiguous
normalisation is rejected. A trailing slash resolves to `index.html`.
SneakerWeb 1.0.1 resolves a non-empty extensionless path exactly; Riot does not
add a `<path>/index.html` fallback because that would be a non-standard
extension.

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

Host validation is not authorization. Each reader creates an unguessable
256-bit capability and installs it as a per-domain, HttpOnly, SameSite=Strict,
non-persistent cookie through the native WebView cookie store before the first
load. The server binds `(capability, reader, domain, block_generation)` and
returns the same indistinguishable 404 for absent/invalid cookie, Host, domain,
or resource. JavaScript cannot read the cookie; closing/blocking the reader
revokes it. Canonical cross-domain navigation asks native to mint/install a new
domain binding before load. Ordinary browsers/local apps without the capability
cannot enumerate membership or read content. Tests must prove the cookie
behavior on each supported WebView; if either platform cannot preserve it for
subresources, port-1312 serving is blocked rather than weakened.

The server binds IPv4 and IPv6 loopback only, rejects absolute-form targets,
duplicate Host/Cookie, folded/control headers, request lines over 8 KiB, more
than 32 headers/16 KiB total, bodies, nonzero Content-Length, and transfer
encoding. It permits at most four connections and eight requests per second per
reader, 16 total in-flight requests, correct bodyless HEAD, and one bounded
single byte range for media. Idle connections close after five seconds.

Every resource response includes `X-Content-Type-Options: nosniff`, a
content-type derived only from the resolved path, and this minimum policy
(platform syntax may add stricter equivalent directives):

```text
default-src 'self' data: blob:;
script-src 'self' 'unsafe-inline' blob:;
style-src 'self' 'unsafe-inline';
connect-src 'none'; object-src 'none'; frame-src 'none';
worker-src 'none'; base-uri 'self'; form-action 'none'
frame-ancestors 'none'
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

Responses also set `Cross-Origin-Resource-Policy: same-origin`,
`Referrer-Policy: no-referrer`, `Cache-Control: no-store`, and a restrictive
Permissions-Policy disabling camera, microphone, geolocation, payment, USB,
Bluetooth, sensors, and clipboard access. Android explicitly permits only this
cleartext loopback origin and blocks it from global service-worker interception;
iOS uses an ephemeral `WKWebsiteDataStore`, app-bound navigation/content rules,
and ATS limited to loopback. Platform tests prove no non-loopback bind and no
fallback to a network-loaded resource.

`sneakerweb.html` is rendered offscreen in a sandboxed, scriptless,
opaque-origin preview host with a reader capability, fixed viewport, and time/
memory limit, then flattened to a native image; the library never embeds a live
site iframe. Site rendering runs in a disposable WebView process. A crash,
memory termination, or repeated main-thread stall closes that reader without
affecting the store or other Riot views.

External handoff is allowed only for a main-frame navigation caused by a native-
confirmed genuine user gesture and a natively parsed `http` or `https` URL.
Riot rejects userinfo, controls/bidi controls, URLs over 2,048 bytes, ambiguous
IP/host syntax, and every custom/file/content/data/javascript/intent scheme.
The confirmation renders inert, bidi-isolated text with normalized scheme,
Unicode hostname, ASCII/punycode hostname, explicit port, and path before the
system browser receives it. Site JavaScript cannot synthesize the confirmation
or call an app URL handler.

### Space attachments and nearby transport

A carrier is a closed canonical-CBOR system object
`org.riot.sneakerweb-carrier/1` stored at
`objects/sneakerweb-carrier/<16-byte-share-id>/<16-byte-revision-id>` in the
selected public Riot namespace. It contains SHA-256 blob digest, encoded length,
ordered unique full domain IDs, site count, sanitized display labels (80 Unicode
scalars each), and an optional 1,024-byte UTF-8 note. It contains no member ID.
The existing space signer creates an ordinary authorised Willow entry; its
verified entry subspace is the sole sharing attribution. Validation rejects an
owned/private namespace, unsupported schema/path, duplicate domain, size/count
mismatch, invalid UTF-8/control/bidi display text, capability outside the exact
namespace/subspace/path, or payload attribution field.

Each share ID is immutable in the MVP; the revision component prevents prefix
collision with other shares and no edit/delete UI exists. The closed importer
adds this path family explicitly and projects valid live entries into carrier
cards keyed by full `(space namespace, entry_id)`. The projection derives the
member profile from the entry signer, treats labels/note as inert native text,
and checks current space authority on display/request. It never grants authority
to or modifies embedded sites.

The `.snk` bytes live in `PortableBlobStore`, deduplicated by SHA-256. Space
entry reconciliation carries only the small record. Devices never advertise a
global digest inventory. For a public-space request, both peers must hold the
same verified carrier entry in an active public `SpaceSession`; the request is
scoped to `(space namespace, carrier entry_id, digest, expected length)` and the
sender rechecks it before each chunk. For direct nearby send, bilateral UI
confirmation creates a random 256-bit one-shot transfer capability bound to the
confirmed nearby session, digest, length, sender, and recipient friendly handle.
Capabilities expire after 10 minutes, one completion, cancellation, or session
close. Private spaces and their metadata never enter this protocol.

Current native transports have one receive owner, so a versioned
`RiotNearbyEnvelopeV2` multiplexer owns the connection. After bilateral protocol
negotiation it routes bounded channels `space_sync` and `portable_blob` by
channel/request ID; the existing SyncCoordinator becomes a channel consumer
rather than the receive owner. V1 peers fail with `This version can't receive
collections` and ordinary sync remains unchanged. Blob frames are
`Request/Accept/Reject/Chunk/Ack/Pause/Resume/Complete/Cancel`, length-delimited
and capped at 256 KiB. At most two chunks are unacknowledged; checkpoints every
32 MiB persist verified offset/chunk digests. The mux keeps the confirmed
connection open until all channels finish or 60 seconds idle, then closes once.
Local TCP is preferred; BLE is a resumable public-content fallback with honest
time/progress UI, never a confidentiality claim.

Chunks write only through the reserved CAS staging handle and follow the atomic
finalization/startup-recovery protocol above. The completed SHA-256/length and
the carrier's ordered domain set are checked before the ordinary Drop decoder
verifies and opens the collection; blob hashing never substitutes for Willow
authorisation.

System sharing exports the same standard bytes without a carrier wrapper.
Direct nearby sharing may transfer an export without first posting a space
record. Space deletion or hiding removes its reference, not a blob still
referenced by another space, source, or active export lease. Unreferenced blobs
are garbage-collected only after a grace period and never while a transfer or
reader holds a lease.

All blob references and leases change transactionally. A block generation
increment cancels any reader/export/direct transfer containing that domain and
prevents a carrier from being published; an already-published public carrier
may remain in its space but opens in the All domains blocked state locally.

## Security model

### Assets and adversaries

Protected assets are Riot identities and private-space data, integrity of the
SneakerWeb collection, its locally held domain/blob inventory as sensitive
device metadata, availability/storage, accurate provenance display,
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
- Block state has a monotonic generation checked at commit, query, every
  ResourceLease read, render, export finish/publication, carrier creation, and
  every nearby chunk; generation change revokes active work.
- Loopback capability cookies, rate/shape limits, same-origin headers, and the
  independent WebView deny prevent local-app/browser and site network probing.
- Blob requests disclose no global inventory and are scoped to a verified
  public carrier/SpaceSession or one-shot bilateral direct-share capability.
- Carrier attribution comes only from the verified outer Willow entry signer;
  payload labels/notes are bounded inert, bidi-isolated native text.
- CAS staging, quota reservation, fsync/rename ordering, backup exclusion, and
  startup reconciliation bound crash leftovers and filesystem races.
- Logs contain route, lengths, stable error codes, and full IDs only in an
  explicit user-exported diagnostic; ordinary logs use internal correlation
  IDs and never payload text.

Diagnostic export requires a confirmation naming its metadata exposure,
contains no private-space identifiers or raw OS paths/URIs by default, and is
written through the same protected temporary/expiry cleanup as `.snk` exports.

Residual risks are explicit. A correctly authorised publisher may distribute
lies, disturbing material, or hostile-but-sandboxed code. Signatures do not
establish a human identity. A carrier may withhold content. Publisher clocks
may be inaccurate. OS WebView defects and a fully compromised device remain
outside the app boundary. Public `.snk` bytes are not confidential from a
carrier or space member who downloads them.

## Dependency and contract activation gate

Drop Format is currently forbidden by the Phase 0A frozen contract. The first
work unit is therefore a deliberate contract-version change, not a hidden Cargo
feature toggle:

1. Add failing tests showing the official SneakerWeb 1.0.1 fixture cannot be
   decoded and the existing validator rejects `drop_format`.
2. Pin the crates.io `sneakerweb 1.0.1` oracle by checksum
   `dc3d20ffadb278e7a8c8e5a06890e10d21c5bcd6d08d8f5811877f6bc9d797c8`
   (MIT OR Apache-2.0), with an offline fixture manifest and reproducible
   generation command in `fixtures/sneakerweb/`.
3. Keep `willow25 = 0.6.0-alpha.3`, default features off, and add exactly
   `drop_format` beside `std`. Refresh `Cargo.lock` and record its checksum.
4. Version `fixtures/manifest.json` and the architecture contract from the
   Phase 0A-only closure to the public-kernel SneakerWeb closure. Update xtask
   so `drop_format` is required only through the dedicated codec/core path,
   remains absent from unrelated test-only graphs, and continues to reject
   OpenMLS, conformance injection, version drift, or other default features.
5. Refresh the checked feature-closure fixture and add validator tests for
   missing, correctly scoped, and illicitly widened Drop Format configurations.
6. Build and link the exact resolved graph for `aarch64-apple-ios`,
   `aarch64-apple-ios-sim`, `aarch64-linux-android`, and
   `x86_64-linux-android`; run one official decode/encode fixture on each native
   runtime before schema/UI work.
7. Drive `DropDecoder`/`DropEncoder` through bounded test producers, prove
   canonical termination, complete payload validation, hostile-input limits,
   and CLI cross-import. Any failure stops the feature and preserves the old
   release contract.

This gate supersedes the older research condition only with executable
evidence: pinned upstream payload import, hosted/mobile builds, authoritative
official CLI vectors, and Riot hostile-input tests. The design makes no claim
that generic partial-slice Drop storage is complete.

## TDD and verification

`.coverage-thresholds.json` remains the source of truth: 100% lines, branches,
functions, and statements. Tarpaulin remains required for the configured gate;
because it does not implement branch coverage, `cargo llvm-cov` separately
blocks on branches, regions, lines, and functions.
Before implementation, its enforcement command is versioned to
`scripts/coverage-gate.sh`; that checked script runs both commands below and
fails if either tool is missing or any configured dimension is below 100. This
updates the source of truth rather than silently enforcing a second threshold
elsewhere.
Implementation follows small RED-GREEN-REFACTOR slices:

1. **Protocol constants and decoder:** failing official-fixture and hostile
   corpus tests; then fixed namespace, complete-payload decoder, canonical
   verification, and bounded typed errors.
2. **Atomic collection:** failing merge, overlap, rollback, cancellation,
   quota, block, and restart tests; then staged SQLite commit and queries.
3. **Encoder interoperability:** failing selected-domain and CLI round-trip
   tests; then streaming export with exact canonical-component preservation and
   cleanup.
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
carrier CBOR, selection isolation, arbitrary merge order, pathological
capability depth, integer boundaries, block/read/export races, chunk resume,
cross-space inventory leakage, absolute-form HTTP, malformed Host/cookies,
synthetic external navigation, and port teardown/rebind. Deterministic barriers
control finish/cancel/block/database-close races; fault injection covers every
CAS fsync/rename/transaction crash point. No fixture contains a production or
reusable private key.

Blocking verification before completion:

```sh
cargo xtask validate-contracts
cargo test --workspace --all-features
cargo tarpaulin --fail-under 100
cargo llvm-cov --workspace --all-features --branch \
  --fail-under-lines 100 --fail-under-functions 100 \
  --fail-under-regions 100 --fail-under-branches 100
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -enableCodeCoverage YES -resultBundlePath build/snk-riotkit.xcresult
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -enableCodeCoverage YES -resultBundlePath build/snk-riot.xcresult
scripts/verify-xcresult-tests.sh build/snk-riotkit.xcresult --require-tests
scripts/verify-xcresult-tests.sh build/snk-riot.xcresult --require-tests
(cd apps/android && ./gradlew testDebugUnitTest connectedDebugAndroidTest lintDebug)
scripts/sneakerweb-interop.sh --offline --version 1.0.1
scripts/sneakerweb-physical-rehearsal.sh --require-recorded-results
```

The physical evidence matrix is iPhone 17 Pro/iOS 26.2 and Pixel 9-class/API
36, release builds, airplane mode with Bluetooth/local Wi-Fi selectively
enabled. Ten cold-cache and ten warm-cache runs use fixed 10 MiB and 100 MiB
fixtures; each records wall time, peak RSS (100 MiB open must remain at or below
512 MiB), encoded/retained bytes, transport, completion, and final digests.
Recorded median/worst results and the novice cohort results are committed as a
release evidence artifact. Zero executed native tests is a gate failure.

## Delivery slices and dependencies

1. Complete the approved multi-space SQLite plan through its database lifecycle,
   authoritative store, native API, iOS cutover, restart/isolation tests, and
   release gate. Release construction must instantiate `RiotDatabase`; the
   current replayed `profile.json`/in-memory production path must be absent.
   Until those executable conditions pass, SneakerWeb schema work is blocked.
2. Update the product brief's preview-before-ingest rule with a link to this
   fixed-public-namespace, user-opened, reversible exception; update
   `.coverage-thresholds.json` to the executable combined coverage wrapper; and
   pass the Dependency and contract activation gate above.
3. Add bounded decoder/encoder, collection
   schema, and atomic Rust APIs.
4. Add FFI and platform document/library/storage-management surfaces.
5. Add the isolated renderer and adversarial native tests.
6. Add standard file sharing and direct nearby transfer/multiplexer.
7. Add portable blob transfer and signed public-space carrier cards.
8. Run cross-client, coverage, performance, accessibility, platform, cohort,
   and physical-device gates.

Slices 2-5 produce useful open/browse capability. Slice 6 completes person-to-
person carrying. Slice 7 completes public-space sharing. No slice may claim `.snk`
support until both import and export pass the official CLI round trip.

## Definition of done

- All included user journeys work without publishing tools or raw-ID-first UI.
- The official CLI round trip and exact canonical entry/capability/signature/
  payload preservation assertions pass. Container byte equality is not claimed
  because valid Drop ordering/encodings can differ.
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
