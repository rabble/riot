# SneakerWeb view-and-share design

Status: Product design and the human-directed post-escalation revision were
approved on 2026-07-13. The revision is pending a fresh Metaswarm design review
gate before planning or implementation. Fresh gate round 1 returned
NEEDS_REVISION, and round 2 returned NEEDS_REVISION on native picker handoff and
terminal UX details. This version resolves both rounds and is pending round 3.

## Purpose

Riot will become a native viewer and carrier for standard SneakerWeb `.snk`
files. A person opens a file as an ordinary document, immediately browses the
sites it contains, keeps those sites in one local library, and can carry one
site, several sites, or a received collection onward. Public Indymedia-style
communities can use a signed Sneaker Directory miniapp alongside Polls, Tasks,
and other social apps, or attach a collection to a normal Newswire post. Riot
does not create
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
4. **Community publisher:** WHO has selected one or many useful sites; WANTS to
   post the collection to a community Newswire or add it to that community's
   enabled Sneaker Directory app; SO THAT the same public material can take
   part in editorial publishing or a durable social directory; WHEN sharing
   within one or several Indymedia-style communities.
5. **Careful verifier:** WHO needs to establish provenance; WANTS full public
   keys, namespace IDs, entry metadata, integrity results, and source history;
   SO THAT cryptographic integrity is inspectable without being confused with
   publisher trust; WHEN they open Details.
6. **Local moderator:** WHO encounters a domain they do not want; WANTS to
   block it once; SO THAT later files cannot silently restore it to normal
   browsing; WHEN viewing a site or its Details.
7. **Storage-limited reader:** WHO has filled the local collection; WANTS to see
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
  the system share sheet, nearby transfer, a public-community Newswire post,
  and an enabled Sneaker Directory social app;
- one reviewed multi-site collection can be shared to several communities with
  per-destination success/retry results and no duplicate record on retry;
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
open. The 20 attempts contain five each through file/system share, direct
nearby, a Newswire attachment, and a Sneaker Directory listing; community
routes include one two-community partial-failure/retry exercise. Any lower
completion or delivery result blocks the field-readiness claim
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
- Newswire attachment references and host-authorized, app-bound Sneaker
  Directory listings in
  one or several public communities;
- one explicit `portable_public_collections` miniapp capability and a bounded
  host-mediated picker/open contract that exposes no `.snk` bytes to app code;
- iOS and Android native surfaces over the shared Rust core.

### Excluded

- creating a SneakerWeb domain or storing its secret;
- publishing, editing, deleting, or signing SneakerWeb site entries;
- treating cryptographic validity as identity, trust, accuracy, or safety;
- private or encrypted SneakerWeb content;
- automatic insertion of SneakerWeb pages into Riot spaces or social apps;
- automatic downloading of space attachments;
- arbitrary Willow Drop Format namespaces;
- incomplete/partial-payload drops. The MVP accepts complete payload-bearing
  `.snk` files produced by the SneakerWeb CLI. General slice-range persistence
  remains a later Willow Store-adapter feature;
- search, full-text indexing, bookmarks, or remote discovery;
- Directory-specific organizer hide/tombstone, ratings, comments, or deletion;
  this first app lists valid bindings chronologically and relies on local site
  block/removal plus ordinary app trust revocation. Newswire attachments retain
  the Newswire's separate editorial-action model;
- running the upstream CLI or its desktop filesystem store inside the app.

Creating a new `.snk` container from already-authorised entries is in scope and
is not publishing: Riot preserves the original entries, capabilities,
signatures, timestamps, and payloads exactly and adds no SneakerWeb signature.
Sharing into a Riot space does create a separately signed Riot carrier record;
that record says who recommended the package, not who authored its sites.
The carrier can be referenced by a signed Newswire post or by data belonging to
an enabled social miniapp. The miniapp supplies community meaning and
organization; the native core retains ownership of bytes, verification, and
viewing.

## User experience

### Navigation placement

The SneakerWeb library is global, not scoped to the selected Riot community.
It does not become a fifth community tab. On iPhone and compact Android, the
global profile/avatar menu contains a labelled `SneakerWeb library` row
alongside `Your profile`. iPad and large-screen Android use that same global
destination in the persistent profile/navigation sidebar rather than creating
a community tab. This preserves the approved Home, Tools, People, and Nearby
community shell. macOS is explicitly outside this mobile release; it must not
advertise `.snk` document handling or the library until a separate design adds
the complete macOS reader, sharing, and verification surface.

Opening an external `.snk` pushes the global Received collection route. When
Riot initiated the route, Back returns to the originating Riot community;
after an OS document launch, Back returns to Riot's library (Riot does not
claim it can navigate into the external originating app). A reader toolbar
provides Back, Forward, Library, Details, and Share.
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
- **Received** lists source packages by safe filename or collection title,
  receipt time, site count, and bounded human site titles as space permits.
  Complete file digest, domain IDs, and unchanged/older/blocked per-domain
  dispositions live only in Details. Overlap never duplicates a Site card;
  original package membership does not imply that every member changed live
  state.

The first-use Sites empty state says `Open a .snk file to collect offline
sites`, with Open File and receive guidance. Received says `Collections you
open will appear here`. Pagination shows a native loading row, Retry on failure,
and keeps the last stable page visible while retrying.

A site card uses `sneakerweb.html` as a decorative preview when available.
Independently, Riot runs a streaming, non-executing title extractor over at
most the first 256 KiB of a complete `/index.html`. It accepts at most 64 levels
of markup nesting, 100,000 tokenizer steps, and 8 KiB of accumulated title text;
crossing any limit aborts extraction rather than truncating parser state. It
decodes character references, removes control and bidi-control characters,
collapses whitespace, and limits display to 80 Unicode scalar values. An
absent, empty, malformed, limit-exhausted, or non-HTML title becomes `Untitled
site`; the full domain key is never the default title. Pathological HTML/title
corpora and fuzzing cover this native extractor as well as the renderer.

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
confusion. Every share requires a human collection title of 1-80 Unicode
scalars after safe-text normalization and accepts an optional 1,024-byte note.
The selected sites, title, note, total size, and public-content disclosure are
reviewed once before choosing destinations.

Destinations are:

- **Share file** through the platform share sheet;
- **Send to a person nearby** through bilateral confirmation, without requiring
  a shared community;
- **Post to Newswire** in one or more writable public communities; and
- **Add to Sneaker Directory** in one or more public communities where a
  trusted social app declaring the machine capability
  `portable_public_collections` is enabled.

Sneaker Directory is an ordinary signed miniapp beside Polls, Tasks, Wiki, and
other community apps. Each space independently trusts/enables its bundle. The
app owns directory presentation, categories, descriptions, and future social
features in app-scoped storage. Its mutable annotations are separate from the
immutable carrier title/note. It never receives `.snk` bytes, filesystem
access, a database handle, block state, raw site IDs, blob digests, signing
keys, or access to the global library. Its explicit capability adds only three
classes of host-mediated operation: list/watch safe Directory summaries, open
a validated opaque listing handle in Riot's native card/viewer, or begin Riot's
native collection-picker/review/commit flow. JavaScript never completes or
signs a share itself. List/picker results expose only an opaque listing/result
handle, title, note, site count, encoded size, availability, and carrier display
attribution.

The capability's JavaScript surface is closed and promise-based:

```js
riot.collections.list(cursor)              // safe AppCollectionPage
riot.collections.watch(generation, cb)     // invalidation, not raw records
riot.collections.pick()                    // native picker/review/commit
riot.collections.open(listingHandle)       // native card after user gesture
```

No lower-level picker task, selection submission, carrier resolver, or blob API
is registered in `window.riot`.

Attachment authority is not stored in generic app data. The host atomically
commits a reserved signed `AppPublicCollectionBindingV1` beside the carrier;
ordinary `riot.put` rejects the reserved path family before signing. Every
receiving device can verify that binding independently. The app may store
categories, reactions, discussion, or other social annotations under its
normal prefix keyed by the opaque listing ID, but invalid/unbound IDs never
appear in the host listing projection and cannot open or download anything.
An app therefore cannot mint, retarget, enumerate, or open another app or
space's collection binding.
`list/watch` is a host projection with a change generation; `STALE_CURSOR` or a
missed generation requires replacing the app's safe summary snapshot, never
merging unverified app state. Open always presents the native collection card
first, and that card requires the separate Get collection action before any
download.

Post to Newswire creates an ordinary signed `NewsPostV1` whose attachment field
references the carrier entry. The editable post headline defaults to the
collection title; context/body is optional. It appears in the Open Wire and is
subject to ordinary transparent feature, correction, hide, tombstone, and
retraction actions. Those actions affect the Newswire projection, not the
SneakerWeb entries or an independent Directory listing.

The destination picker permits several community rows in one share. Each row
chooses Newswire or an enabled Sneaker Directory; selecting both for the same
community is allowed and both references share one carrier. A community without
the Directory app is not a Directory destination: a recognized organizer may
open its normal `Let everyone here use this` review, while other people see
`Ask an organizer to turn on Sneaker Directory`. Private/encrypted groups are
excluded because their public-content and metadata boundary requires the
separate reviewed group bridge.

The filename uses the sanitized required collection title. It never surfaces a
shortened key or digest. Existing destination files receive the platform's
normal collision suffix. Export streams to a protected temporary and deletes
it after completion, cancellation, lease expiry, or startup recovery.

System share shows the public-content disclosure, then the OS sheet. Nearby
requires the existing bilateral confirmation on both phones and is disabled
with guidance only when local permission or an eligible confirmed person is
absent; the `.snk` and direct-transfer envelope contain no Riot-community
membership, destination records, internal notes, or private metadata.
Before Accept/Reject, the recipient sees the confirmed sender handle plus the
bounded sender-supplied collection title, site count, encoded size, and `Public
SneakerWeb collection`; these preview fields are authenticated by the one-shot
confirmed-session request but are not treated as content integrity. The
envelope contains no note, raw ID, digest display, or community relationship.
Community rows list only public/open spaces where the active profile may create
the selected record. Review says `You are sharing this collection; you did not
author its sites` and shows Publish/Cancel. Permission denial, peer
disappearance, cancellation, and failure preserve the selection and offer
Retry or another destination.

Every destination exposes preparation as an explicit native state before
handoff: `Preparing collection` reports selecting/encoding byte and item
progress with Cancel. The task may continue in the background only while the
OS grants execution; suspension removes incomplete output and returns to
`Preparation interrupted` with Retry preserving the exact selection. System
share then becomes `Ready to share` and hands its lease to the OS sheet;
cancelling or returning from the sheet closes the lease without losing the
selection.

Direct nearby continues from preparation through `Waiting for confirmation`,
bilateral Accept/Reject, `Connecting`, sender and receiver `Transferring`
progress, Pause/Resume/Cancel, `Verifying`, and `Received/Open`. Peer loss is
`Interrupted`; Resume is available only while the same confirmed session is
alive and otherwise Retry starts fresh bilateral confirmation with a new
one-shot capability. A failed integrity check restarts at zero rather than from
an unverified checkpoint.

Each selected community is an independent idempotent child operation with
`Preparing attachment`, `Signing carrier`, `Signing destination`, `Committing`,
and `Shared`. The carrier and all selected Newswire/Directory references for
one community commit atomically, so no visible orphan is created. A native
batch coordinator reports each row as Shared, Needs retry, or No longer allowed;
one failure never rolls back another community. Retry uses the original
128-bit idempotency key and returns the existing receipt after a prior commit
rather than making duplicate posts/listings. Posting failure preserves the
prepared lease and review fields until its visible 15-minute idle expiry.
Cancel before one community's commit leaves no records there; after commit its
success is final. All flows use the accessibility progress announcement
cadence below, and all failure/cancellation paths restore focus to the
preserved review/selection.

A Newswire attachment and Directory listing both host the same native card. It
reads `Street Medic Library · 4 sites · 18 MB`, shows identity derived from the
outer signed carrier entry, and repeats that the member carried rather than
authored the sites. Its states are Available locally/Open; Not downloaded/Get
collection; Waiting for nearby holder/Retry; Transferring with progress and
Cancel; Interrupted/Resume; Verifying; Invalid/Remove reference locally;
Storage full/Manage storage; All domains blocked/View blocked sites; and
Ready/Open. Cards never auto-download. Any currently authorised nearby holder
of the public attachment may serve it after an explicit request.

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
count. `Remove received record` deletes that receipt/associations and also
deletes content for which it is the only retained source; content held by
another receipt remains. `Remove site from this device` deletes that domain's
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
need disambiguation. If title and phrase both collide, cards receive a stable
`item N of M` suffix ordered by the complete domain bytes; the ordinal is not an
identifier and Details remains the only raw-key surface. Decorative
`sneakerweb.html` never supplies the accessible
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
| Sneaker Directory is not enabled in a chosen community | Organizer: review/turn on app; member: `Ask an organizer to turn on Sneaker Directory.` |
| Some community destinations commit and others fail | Keep successful receipts; list failed/no-longer-allowed rows; Retry failed rows with original idempotency keys |
| Viewer process fails or exhausts its budget | `This site stopped working.` Retry/Close |
| A previously accepted local payload fails its digest on read | `This local copy is damaged.` Remove local copy/Open another file |
| Temporary export or destination fails | `This couldn't be shared.` Retry/Choose another |

Details exposes a stable typed code and safe structural facts. It never includes
payload text, secret material, raw OS URI/path, or a truncated identifier.

## Architecture

### Boundaries

SneakerWeb is a distinct public-content collection in the shared Rust core,
not an evidence-bundle variant. The protocol viewer is native; a community may
organize carrier references through an ordinary social miniapp:

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
      queries       viewer        /       |         \
                               file    nearby   portable blob
                                                  |
                                     signed public carrier
                                       /               \
                                Newswire post    social-app listing
```

Rust owns decoding, canonical verification, namespace enforcement, Willow join
semantics, persistence, selection, Drop encoding, block policy, provenance,
and attachment digests. Swift and Kotlin own platform document entry points,
WebViews, OS sharing, and nearby lifecycle, calling versioned UniFFI types.
Site HTML/JavaScript never receives a database, filesystem,
namespace-selection, or native bridge API. A trusted social miniapp remains in
the existing isolated app host and sees only verified safe binding summaries,
its own signed social-annotation records, and the three bounded
`portable_public_collections` operation classes described above.

The fixed SneakerWeb namespace is permanently reserved with collection kind
`sneakerweb-public`. `RiotDatabase` rejects creating/joining it as a Riot space,
and `SpaceSession`/FFI factories reject it with `RESERVED_NAMESPACE`.
Repository constructors and ordinary space sync reject and disconnect the
record; rebuildable app/document/trust/change-feed projectors ignore an already
quarantined row and emit a diagnostic counter but never expose it. Conversely, the
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
  between a signed space entry and one portable blob digest;
- `SocialCollectionHost`: capability, active-space/app binding, opaque
  listing handles, closed host-binding codec, and native collection-card
  handoff. It never enters the site renderer or exposes the SneakerCollection
  query/resource API to miniapp JavaScript;
- `SpaceSneakerShare`: atomic carrier plus Newswire/app-reference creation,
  per-destination idempotency receipts, and prepared-blob lease ownership.

#### Social-app capability activation

Current `AppManifest` permissions are author-supplied human-readable strings;
they are not machine-enforced scopes. This design must not compare security
behavior to mutable display copy. The capability therefore activates through a
versioned manifest contract before the Directory bundle ships:

- legacy manifest/app-ID v1 remains byte-for-byte accepted and receives no new
  host capability;
- manifest v2 adds canonical key `9`, a definite, lexicographically sorted,
  duplicate-free array of closed machine capability tokens, and derives its
  app ID under domain `riot/app-id/v2`; key `9` is mandatory even when the array
  is empty;
- the only new token in this slice is exact ASCII
  `portable_public_collections`; any unknown token rejects the entire v2
  manifest rather than loading it with a partial capability set;
- the organizer review page renders host-owned copy for the token:
  `Open and share public SneakerWeb collections you choose. It cannot read other
  files, sites, or apps.` This copy is not supplied by the bundle author;
- v1 apps, untrusted v2 apps, and a trusted app without the token receive no
  picker/open handlers at all; and
- manifest canonical vectors, old-client rejection/new-client acceptance, app
  ID separation, trust revocation, and native bridge dispatch are blocking
  tests. The starter Sneaker Directory is packaged and trusted through the same
  ordinary app-index flow as Polls and Tasks, never as a privileged built-in.

The codec uses upstream `willow25::drop_format::{DropDecoder, DropEncoder}`
directly with Riot-owned bounded producers/consumers. It does **not** call
upstream `import_drop` against authoritative tables or `export_drop` with an
unbounded elastic queue. Riot independently stages and verifies every decoded
entry/payload, and independently queries selected complete entries for the
encoder. A pre-implementation conformance work unit enables the exact pinned
feature and proves official CLI vectors before application code proceeds.

All FFI DTOs carry `schema_version = 1`; identifiers are fixed-width bytes and
errors are closed enums with `retryable`, `recovery_action`, and safe detail
fields. FFI entry calls never execute work on the native UI thread. A short-lived
state actor linearizes commands and submits only bounded SQLite transactions to
Riot's serialized database worker. Streaming I/O, Drop parsing/encoding,
hashing, and verification run on a bounded two-worker executor under per-task
cancellation tokens; they publish immutable progress snapshots back to the
actor at chunk boundaries. Consequently `progress`, `cancel`, block, and owner
close remain serviceable while CPU/I/O work is active. Page cursors contain an
opaque authenticated `(database_generation, collection_generation,
last_sort_key)` snapshot; mutation makes them `STALE_CURSOR`, after which
native restarts from page one while retaining the visible page.

Native document providers are never exposed as arbitrary paths to Rust. The
host opens the security-scoped URL/content URI and streams chunks into a
core-owned, app-protected, backup-excluded staging handle:

```text
begin_snk_open(display_name, route, expected_bytes) -> OpenSnkTask
begin_snk_open_from_blob(portable_blob_lease, display_name, route) -> OpenSnkTask
OpenSnkTask.write_chunk(bytes) -> TransferProgress
OpenSnkTask.progress() -> TransferProgress
OpenSnkTask.finish() -> OpenSnkOutcome
OpenSnkTask.cancel() -> CancelOutcome
undo_snk_open(receipt_id, undo_generation) -> RemovalOutcome
list_sneaker_sites(cursor) -> SneakerSitePage
list_received_sneaks(cursor) -> ReceivedSneakPage
list_blocked_sneaker_sites(cursor) -> SneakerSitePage
list_sneaker_storage(cursor) -> SneakerStoragePage
get_sneaker_site(domain_id) -> SneakerSite
get_sneaker_details(domain_id, cursor) -> SneakerDetailsPage
get_received_sneak_details(source_id, cursor) -> ReceivedSneakDetailsPage
resolve_sneaker_resource(domain_id, path) -> ResourceLease
block_sneaker_domain(domain_id) -> BlockOutcome
unblock_sneaker_domain(domain_id) -> BlockOutcome
remove_sneaker_source(source_id) -> RemovalOutcome
remove_sneaker_site(domain_id) -> RemovalOutcome
remove_portable_blob(blob_id) -> RemovalOutcome
create_snk_export(domain_ids) -> SnkExportTask
begin_space_sneaker_share(public_space_session, prepared_blob_lease,
  title, note, destinations, idempotency_key) -> SpaceSneakerShareTask
request_space_blob(public_space_session, carrier_entry_id, peer) -> SpaceBlobReceiveTask
begin_direct_snk_send(confirmed_nearby_session, export_lease) -> DirectSnkSendTask
accept_direct_snk_receive(confirmed_nearby_session, request_id) -> DirectSnkReceiveTask
reject_direct_snk_receive(confirmed_nearby_session, request_id) -> RejectOutcome
begin_app_collection_pick(active_app_session, genuine_gesture_token)
  -> AppCollectionPickerTask
AppCollectionPickerTask.take_native_event() -> AppCollectionPickerEvent
AppCollectionPickerTask.submit_native_selection(domain_ids) -> AppCollectionReview
AppCollectionPickerTask.submit_native_review(title, note) -> TransferProgress
AppCollectionPickerTask.progress() -> TransferProgress
AppCollectionPickerTask.finish() -> AppCollectionCommitResult
AppCollectionPickerTask.retry() -> TransferProgress
AppCollectionPickerTask.cancel() -> CancelOutcome
list_app_collection_bindings(active_app_session, cursor) -> AppCollectionPage
watch_app_collection_bindings(active_app_session, generation) -> AppCollectionChange
open_app_collection_binding(active_app_session, listing_handle,
  genuine_gesture_token)
  -> NativeCarrierCard
```

Every domain parameter is exactly 32 bytes at the FFI boundary; string parsing
exists only for canonical URLs and Details copy/paste. No SneakerWeb API accepts
a caller-supplied namespace.

`destinations` is a closed non-empty set for one space: at most one
`NewswireAttachment { headline, body }` and at most one
`AppPublicCollectionBinding { trusted_app_id, listing_id }`. The core verifies the
NewsPost contract or exact trusted app/capability/active-space binding before
signing. A native multi-community coordinator creates one task per selected
space with independently generated 128-bit idempotency keys; there is no
cross-space atomic transaction.

Only native picker completion supplies domain IDs. `AppCollectionPickerTask`
owns the entire native picker -> collection review -> prepared blob ->
`SpaceSneakerShareTask(AppPublicCollectionBinding)` flow and returns a committed
`AppCollectionCommitResult`; miniapp JavaScript never receives a draft that it
can retarget or commit. Its gesture token is single-use, bound to the active
trusted app/space/profile session, and expires after 30 seconds if no native
picker is shown. The generated per-space idempotency key remains attached to
the task across retry. Trust, capability, session generation, and active profile
are rechecked before showing review, before the atomic commit, and before
returning the result to JavaScript.

The bridge-facing `riot.collections.pick()` dispatch creates the task but none
of `take_native_event`, `submit_native_selection`, or `submit_native_review` is
registered in the JavaScript message dispatcher. Swift/Kotlin retains the task
handle and receives `PresentPicker { max_sites: 1,024 }` from
`take_native_event`. The platform picker returns either Cancel or a bounded,
ordered-unique array of complete 32-byte domain IDs. Cancel invokes task cancel;
selection submission is legal only in `AwaitingNativePicker`, rechecks the
active app/space/profile/gesture generation and every domain's availability/
block generation, then returns `AppCollectionReview` containing only native
safe titles, count, encoded estimate, default collection title, and disclosure
copy.

Swift/Kotlin presents that native review and either cancels or calls
`submit_native_review` with the edited title and note. Core performs the exact
safe-text normalization and size validation, then generates the random 16-byte
listing ID and 16-byte child idempotency key itself and moves to Preparing.
Review edits are allowed until the first accepted submit; afterward Retry uses
the frozen normalized fields and same child. Changing them requires Cancel and
a new picker flow. An identical duplicate selection/review call returns the
original accepted review/progress; a different duplicate returns
`INVALID_STATE`/`IDEMPOTENCY_CONFLICT` without mutation. Stale generation,
unavailable domain, invalid text, or a call in the wrong state is a typed
non-mutating failure. `take_native_event` is single-consumer and repeated after
delivery returns `NO_EVENT` until the next state transition.

The nearby multiplexer emits a bounded `IncomingPortableBlobRequest` DTO before
either receive factory is legal; it contains request ID, safe peer handle,
digest, length, and channel kind, never a filesystem path. For a direct request
it additionally requires safe collection title (`1..=80` Unicode scalars) and
site count (`1..=1,024`), authenticated inside the confirmed one-shot request;
for a space request those display fields are derived from the verified carrier
rather than peer input. Space receipt is
accepted only through `request_space_blob`; direct receipt is accepted only
through `accept_direct_snk_receive` after native bilateral confirmation.

Opaque objects have complete state machines and single terminal results:

- `OpenSnkTask`: `Receiving -> Validating -> Committing -> Completed`, with
  terminal `Cancelled | Failed`. `write_chunk(max 256 KiB)` is legal only while
  Receiving; `finish` checks exact expected length and canonical termination;
  `cancel` is idempotent. Repeated `finish` returns the same terminal outcome.
- `SnkExportTask`: `Selecting -> Encoding -> Ready(ExportLease)`, with terminal
  `Cancelled | Failed`. It exposes progress/cancel/finish. `finish` yields an
  `ExportLease` rather than bytes or a filesystem path. The first `finish`
  starts/awaits the asynchronous job; repeated calls return the identical lease
  identity or terminal error. The lease has a 15-minute idle expiry refreshed
  by a successful read or retain call and is owned by the database generation.
- `ExportLease`: metadata plus `read_range(offset, max <= 1 MiB)`,
  `retain_as_portable_blob()`, and idempotent `close`. Retain fsyncs and promotes
  the exact export into CAS and returns a `PreparedBlobLease`; system
  share/native nearby may stream without promotion. Every read and retain
  rechecks every selected domain's block generation. Expiry or close makes
  reads `LEASE_CLOSED`; native owner teardown closes all of its leases.
- `PreparedBlobLease`: immutable digest/length/selected-domain block generations,
  15-minute idle expiry, bounded retain/release, and no filesystem path. A
  successful space task converts its temporary hold to durable carrier
  references in the commit transaction. When the last task/lease expires before
  commit, ordinary blob grace-period collection applies.
- `ResourceLease`: immutable MIME/length/digest/block-generation metadata,
  bounded `read_range`, and idempotent `close`; every read rechecks its block
  generation.
- `SpaceSneakerShareTask`: `Preparing -> SigningCarrier ->
  SigningDestinations -> Committing -> Shared(CarrierReceipt)`, with terminal
  `Cancelled | Failed | OwnerClosed`. It exposes monotonic progress, cancel,
  and asynchronous finish; repeated finish returns the same receipt or terminal
  error. Carrier and every selected destination record for
  that one space commit atomically. Recreating a task with the same
  `(space, signer, idempotency_key)` returns/continues the recorded operation;
  after commit it returns the identical receipt and never signs a duplicate.
  Reusing that key with different title, note, blob, or destinations returns
  `IDEMPOTENCY_CONFLICT` without mutation.
  A retryable pre-commit failure preserves its prepared lease/review fields
  until the visible idle expiry; cancellation or expiry releases them.
- `AppCollectionPickerTask`: `AwaitingNativePicker -> Reviewing -> Preparing ->
  Delegating(SpaceSneakerShareTask) -> Committed(AppCollectionCommitResult)`,
  with recoverable `Retryable` and terminal `UserCancelled |
  NonRetryableFailed | OwnerClosed`. It exposes progress, idempotent cancel,
  retry while its 15-minute reviewed-selection lease remains valid, and
  asynchronous finish; repeated finish returns the same committed result or
  current typed error. Retry resumes the original child/idempotency key rather
  than signing a new operation. `submit_native_selection` is the only
  `AwaitingNativePicker -> Reviewing` transition and
  `submit_native_review` is the only `Reviewing -> Preparing` transition.
  Losing trust/capability/app-space-profile binding before commit cancels the
  child and commits nothing. Revocation after commit leaves signed public
  records intact but makes list/watch/open handlers unavailable until the app
  is trusted again. Final handle drop cancels both parent and child before
  commit.

The native host retains the invoking miniapp route and focus token. Picker or
review Cancel before commit returns a safe `cancelled` callback to the still-
trusted invoking app, restores the invoking control's focus, and leaves no
record. Retryable child failure remains in the native review with selection,
frozen fields, idempotency key, Retry, and Cancel. Pre-commit trust/capability/
session revocation commits nothing, sends no callback into the app, and routes
to the host-owned community Tools surface with `Sneaker Directory is no longer
available here` and organizer/member recovery guidance.

The atomic carrier/binding commit is the final linearization point and wins
over a later revocation or app closure. With trust intact, the host returns the
safe `AppCollectionCommitResult` JavaScript projection, forces list/watch to a
fresh host snapshot, returns to the miniapp, and focuses/announces the new
listing. If trust/session/app availability disappears after commit but before
callback, native announces `Shared to Sneaker Directory`, delivers no data to
the app, and routes to the host-owned unavailable/Tools surface; it never
reports the committed share as failed. Relaunch/retrust later projects the
signed binding normally.
- `DirectSnkSendTask`: `Negotiating -> Sending -> WaitingForVerifiedAck ->
  Sent(SendReceipt)`, with `Paused | Cancelled | Failed | OwnerClosed`. Sent is
  legal only after the receiver acknowledges exact length/digest verification;
  the first `finish` starts/awaits delivery and repeated calls return the same
  receipt or terminal error. It never yields or promotes a local blob.
  Pause/Resume is legal only while
  the confirmed session remains alive. Retry after session loss requires new
  bilateral confirmation and a new one-shot capability.
- `DirectSnkReceiveTask` and `SpaceBlobReceiveTask`: `Negotiating -> Receiving
  -> Verifying -> Ready(PortableBlobLease)`, with `Paused | Cancelled | Failed |
  OwnerClosed`. The first `finish` starts/awaits the transfer and repeated calls
  return the same lease identity or terminal error. `PortableBlobLease` is
  read-only, has the same 15-minute idle expiry, and must be passed explicitly
  to `begin_snk_open_from_blob`; receiving never mutates SneakerCollection.
  Closing/expiry releases the active lease but retains a promoted blob only
  while a carrier or explicit retain reference owns it.

All tasks are owned by a database generation and, when relevant, a public
`SpaceSession`/nearby-session generation. After a terminal result, cancellation
returns `AlreadyTerminal`, writes/reads return the terminal typed error, and
close/drop only cleans resources. A database/session close accepted by the
actor before commit returns `OWNER_CLOSED`; after atomic commit the completed
result wins. For finish versus cancel, the first actor-accepted command wins.
A newer block generation always overrides selection/encoding success and
prevents export-lease, carrier, resource, or chunk publication. Cancellation
sets an atomic token before queuing actor cleanup, so the worker observes it at
the next 256 KiB/10,000-step checkpoint even if the actor queue is busy.
Progress snapshots are monotonic and may lag by at most one checkpoint.
Deterministic barrier tests cover finish/cancel, block/read, block/retain,
owner-close/commit, and task-drop during validation, encoding, and verification.
A final native handle drop for any nonterminal task is an idempotent cancel:
it sets the token, releases reservations/temporary leases, and may not continue
detached in the background.
A process death has no callable outcome; startup reconciliation performs the
same cleanup before new tasks start.

The minimum DTO contracts are:

| DTO | Required fields |
| --- | --- |
| `SneakerSite` | version, full 32-byte domain, native title, complete/index/preview flags, unblocked state/generation, current bytes/entry count, latest publisher timestamp, latest local receipt, source count |
| `ReceivedSneak` | version, opaque source ID, safe filename/collection title, encoded bytes, receipt route/time, site/disposition counts, bounded safe human titles, current-export-differs flag; no digest/domain ID |
| `SneakerDetailsPage` | version, full domain/namespace, integrity vocabulary, publisher timestamp, source-history rows, live/accepted entry rows, next cursor |
| `ReceivedSneakDetailsPage` | version, source facts including full SHA-256, exact original full domain/entry membership, receipt dispositions, current contribution/difference, next cursor |
| `TransferProgress` | version, phase, completed bytes/items, optional bounded total, resumable/cancellable booleans |
| `OpenSnkOutcome` | version, receipt ID, added/updated/unchanged/blocked/older counts, retained/reclaimable bytes, undo generation, zero-change reason |
| `CarrierReceipt` | version, full space ID, idempotency key, full carrier entry ID, ordered destination kind/full entry ID pairs, blob digest/length, committed time |
| `CommunityShareBatchResult` | version, preserved selection/review ID, ordered per-space idempotency key and `shared/retryable/no-longer-allowed/cancelled` outcome, optional CarrierReceipt/stable error |
| `SendReceipt` | version, transfer request ID, safe peer handle, blob digest/length, verified-ack time |
| `IncomingPortableBlobRequest` | version, request/channel IDs, safe peer handle, digest/length, channel kind; direct only: bounded safe title/site count; no note/community/raw site IDs |
| `AppCollectionPickerEvent` | version, event generation, closed `PresentPicker/PresentReview/Return` variant and safe native-only fields |
| `AppCollectionReview` | version, selected count/encoded estimate, bounded native site titles, normalized default title, disclosure, app/space/profile generation; no JavaScript projection |
| `AppCollectionSummary` | version, app/space/session-bound opaque listing handle, safe title/note/site count/encoded size/availability/carrier attribution, change generation; no site IDs/digest/bytes |
| `AppCollectionCommitResult` | version, opaque listing handle, full space/app IDs for native audit only, idempotency key, CarrierReceipt, committed time; JavaScript projection omits raw IDs/receipt and returns handle plus safe summary |

Stable core errors are `INVALID_DROP`, `WRONG_NAMESPACE`,
`INCOMPLETE_PAYLOAD`, `LIMIT_EXCEEDED`, `STORAGE_FULL`, `STALE_CURSOR`,
`OWNER_CLOSED`, `BLOCKED`, `LEASE_CLOSED`, `INVALID_STATE`, `CANCELLED`,
`TEMPORARY_IO`, `VIEWER_UNAVAILABLE`, `NO_AUTHORISED_HOLDER`, and
`SECURITY_POLICY`, plus destination errors `APP_NOT_TRUSTED`,
`APP_CAPABILITY_DENIED`, `DESTINATION_NOT_WRITABLE`, and
`IDEMPOTENCY_CONFLICT`, and native-flow errors `NO_EVENT` and
`INVALID_SELECTION`. Raw parser/SQL/OS text never crosses FFI. Native maps only
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
| `space_sneaker_share_ops` | Full space/signer/idempotency key, task state, prepared-lease expiry, exact destination request, terminal carrier/destination IDs, and cleanup state. |
| `app_public_collection_bindings` | Rebuildable verified space/app/listing-to-carrier projection, full signer IDs, live trust/capability availability, and change generation for paged list/watch. |
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
directory; stream while checking length/chunks; fsync the file; and verify final
SHA-256 and `.snk` length. A per-full-digest process-local finalization lock
serializes in-process contenders; the platform
`install_no_replace(staging, digest_path)` primitive is the cross-process
arbiter and
must atomically succeed only when the destination did not exist (for example,
`renameat2(RENAME_NOREPLACE)` or same-filesystem link/create-exclusive
semantics). Ordinary replacing rename is forbidden.

The digest lock is held through destination verification, directory fsync,
`portable_blobs` publication/attachment, and reservation release. The winner
installs the file, fsyncs the directory, transactionally publishes/attaches the
row, and releases its reservation before unlocking. A
loser opens the winner without following links, verifies regular-file type,
exact length, and digest, fsyncs the containing directory to establish
durability in its process, deletes its staged duplicate, transactionally
inserts-or-ignores then attaches the exact digest/length row, and releases its
reservation. The insert-or-ignore covers a valid file left by a process death
before row publication; conflicting row facts fail as corruption. A mismatching existing
path is quarantined as corruption and fails closed; it is never overwritten
while a lease may exist. Deterministic same-digest contenders are tested with
barriers before install, after install, before/after row publication, and across
process-death reconciliation. Startup runs before database service: delete expired
staging/chunks/share temporaries; verify any digest-named final file without a
row; insert/attach it only when a durable carrier/share operation/reservation
references the exact digest and length; otherwise mark it as a bounded orphan;
reconcile rows without files; expire leases/reservations; recompute references;
and finish or roll back pending removals. A complete unreferenced file becomes a bounded
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
- community destinations per reviewed batch: 32, each with at most one Newswire
  and one Directory reference; at most four child space-share tasks active at
  once, with the remainder queued and cancellable;
- Sneaker Directory listing values remain within the existing miniapp value,
  entry-count, and per-space storage budgets; the new capability raises none of
  those ceilings;
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
non-persistent host-only cookie with a fixed name, omitted `Domain`, and
`Path=/` through the native WebView cookie store before the first load. The
server binds `(capability, reader, domain, block_generation)` and
returns the same indistinguishable 404 for absent/invalid cookie, Host, domain,
or resource. JavaScript cannot read the cookie; closing/blocking the reader
revokes it synchronously in server state and the WebView cookie store.
Canonical cross-domain navigation asks native to mint/install a new
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
worker-src 'none'; base-uri 'self'; form-action 'none';
frame-ancestors 'none';
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
fallback to a network-loaded resource. Tests parse the emitted CSP rather than
string-match it and assert every directive independently, especially
`form-action` and `frame-ancestors`. Packet-level platform tests also prove DNS
prefetch, preconnect, speculative navigation, and renderer-initiated sockets
never leave loopback.

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
Unicode hostname, ASCII/punycode hostname, explicit port, path, query, and
fragment before the system browser receives it. Site JavaScript cannot
synthesize the confirmation or call an app URL handler.

### Space attachments and nearby transport

A carrier is a closed RFC 8949 deterministic-CBOR map. Definite lengths,
shortest integer/length forms, and numeric-key canonical order are mandatory;
indefinite forms, tags, floats, duplicate/unknown keys, and trailing bytes are
rejected. The complete version-1 schema is:

| Key | Name | CBOR type | Required validation |
| --- | --- | --- | --- |
| `0` | schema | text | Required; exact ASCII `org.riot.sneakerweb-carrier/1`. |
| `1` | blob digest | byte string | Required; exactly 32 SHA-256 bytes. |
| `2` | encoded length | unsigned integer | Required; `1..=1,073,741,824`, equal to the blob length. |
| `3` | collection title | text | Required; valid UTF-8, no control/bidi-control scalar, `1..=80` Unicode scalars after whitespace collapse. |
| `4` | domains | array of byte strings | Required; `1..=1,024` elements, each exactly 32 bytes, lexicographically increasing and therefore unique. |
| `5` | labels | array of text | Required; exactly as many elements as key `4`; label at index `i` describes domain at index `i`; each is valid UTF-8, contains no control/bidi-control scalar, and is at most 80 Unicode scalars after whitespace collapse. |
| `6` | site count | unsigned integer | Required; exactly the array length at keys `4` and `5`. |
| `7` | note | text | Optional; at most 1,024 UTF-8 bytes after the same control/bidi rejection. |

The top-level map therefore has exactly seven pairs without a note or eight with
one. It has no author/member/attribution field. The existing space signer
creates an ordinary authorised Willow entry; its verified outer entry subspace
and signer are the sole sharing attribution.

The Willow path is exactly four byte components: ASCII `objects`, ASCII
`sneakerweb-carrier`, the lowercase hexadecimal encoding of a 16-byte share ID,
and the lowercase hexadecimal encoding of a 16-byte revision ID. Both IDs are
independent 128-bit values from the platform CSPRNG and each encoded component
is exactly 32 ASCII bytes; raw ID bytes, uppercase hex, other component counts,
and other encodings are rejected. A version-1 share creates one immutable
revision and never reuses either ID. Validation rejects an owned/private
namespace, unsupported schema/path, duplicate/misordered domain, label/domain
misalignment, size/count mismatch, invalid display text, capability outside
the exact namespace/subspace/path, or any unknown payload field.

Each share ID is immutable in the MVP; the revision component prevents prefix
collision with other shares and no edit/delete UI exists. The closed importer
adds this path family explicitly and projects valid live entries into carrier
cards keyed by full `(space namespace, entry_id)`. The projection derives the
member profile from the entry signer, treats title/labels/note as inert native text,
and checks current space authority on display/request. It never grants authority
to or modifies embedded sites.

A Newswire attachment extends the pending `NewsPostV1` contract with union member
`PublicAttachmentRefV1 { media_type:
"application/vnd.riot.sneakerweb-carrier+cbor", carrier_entry_id }`. The full
entry ID must resolve to a valid carrier in the same public community; clients
never follow a cross-space attachment reference. The NewsPost and carrier outer
entries must have the same verified signer.

An app-hosted directory reference is a reserved host-owned record, not generic
app data. Its Willow path has exactly four byte components: ASCII `objects`,
ASCII `app-public-collections`, exactly 64 lowercase ASCII hex bytes encoding
the full 32-byte app ID, and exactly 32 lowercase ASCII hex bytes encoding a
random 16-byte listing ID.
`AppDataBridge` rejects this path family before any generic app
write is signed. Only `SpaceSneakerShareTask` may construct it.

Its payload is a closed deterministic-CBOR map with the same definite-length,
shortest-form, sorted-key, no-tags/floats/unknowns/trailing-bytes rules as the
carrier:

| Key | Name | CBOR type | Required validation |
| --- | --- | --- | --- |
| `0` | schema | text | Exact ASCII `org.riot.app-public-collection-binding/1`. |
| `1` | space ID | byte string | Exactly 32 bytes and equal to the containing public namespace. |
| `2` | app ID | byte string | Exactly 32 bytes and equal to the path app component. |
| `3` | listing ID | byte string | Exactly 16 bytes and equal to the path listing component. |
| `4` | carrier entry ID | byte string | Complete canonical `RiotEntryIdV1` bytes, `1..=4,096`, resolving in key `1`. |
| `5` | signer subspace ID | byte string | Exactly 32 bytes and equal to both outer entry signers. |
| `6` | created time | unsigned integer | Exact share-operation time in microseconds within the existing signed-time bounds. |

The binding outer signer must equal the carrier outer signer, and the carrier
must be committed in the same SQLite/Willow transaction. Current app trust and
the declared capability are required at creation and at list/watch/open time;
they are not embedded as permanent authority. A receiving device verifies path,
CBOR, space, app, signer, carrier, and live trust without local share-operation
history.

The host derives a stable opaque `listing_handle` as base64url of the full
SHA-256 of the canonical binding entry. App code receives that handle and safe
summary only, never key `4`; the handle is a lookup key, not authority, and is
resolved solely within the active app/space. Categories, reactions, or
discussion are ordinary app-owned records keyed by the handle and cannot
create or retarget a binding. Unknown/malformed bindings and annotations for an
unbound handle remain inert and never reach the attachment resolver.

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
32 MiB persist verified offset/chunk digests. Direct `Request` includes the
bounded title/site-count preview inside its one-shot capability MAC/transcript;
space `Request` omits them and resolves display from the signed carrier. The
mux keeps the confirmed
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
- Newswire and app references resolve only same-space carriers with the same
  verified signer. App references additionally require current app trust, the
  exact declared capability, active app/space binding, a genuine user gesture,
  and a short-lived opaque handle; revocation makes synced listing data inert.
- The social-app bridge exposes bounded safe card metadata and native
  picker/open gestures only. It cannot enumerate the global library, resolve
  arbitrary carrier IDs, read resources, receive raw site IDs/digests/bytes, or
  cause a download without the native Get collection action.
- Block state has a monotonic generation checked at commit, query, every
  ResourceLease read, render, export finish/publication, carrier creation, and
  every nearby chunk; generation change revokes active work.
- Loopback capability cookies, rate/shape limits, same-origin headers, and the
  independent WebView deny prevent local-app/browser and site network probing.
- Blob requests disclose no global inventory and are scoped to a verified
  public carrier/SpaceSession or one-shot bilateral direct-share capability.
- Carrier attribution comes only from the verified outer Willow entry signer;
  payload labels/notes are bounded inert, bidi-isolated native text.
- CAS staging, quota reservation, atomic no-replace installation, fsync
  ordering, backup exclusion, and startup reconciliation bound crash leftovers
  and filesystem races.
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
3. Keep the workspace `willow25 = 0.6.0-alpha.3` declaration at default
   features off with `std`. Add `features = ["drop_format"]` on the normal
   `crates/riot-core -> willow25.workspace` dependency edge—the sole requesting
   edge—because `riot-core::sneakerweb` owns the codec. Cargo feature unification
   means every Willow use in a graph containing `riot-core` sees that feature;
   this is requester-edge control, not an impossible claim of per-crate compile
   isolation. Refresh `Cargo.lock` and record its checksum.
4. Version `fixtures/manifest.json` and the architecture contract from the
   Phase 0A-only closure to the public-kernel SneakerWeb closure. Update xtask
   so `drop_format` is requested only by that production core edge, is absent
   from graphs that do not contain it, and continues to reject
   OpenMLS, conformance injection, version drift, or other default features.
5. Refresh the checked feature-closure fixture and add validator tests for
   missing, correctly scoped, and illicitly widened Drop Format configurations.
6. Build and link the exact resolved graph for `aarch64-apple-ios`,
   `aarch64-apple-ios-sim`, `aarch64-linux-android`, and
   `x86_64-linux-android`; run one official decode/encode fixture on each native
   runtime before schema/UI work.
7. Drive `DropDecoder`/`DropEncoder` through bounded test producers, prove
   canonical termination, complete payload validation, hostile-input limits,
   and CLI cross-import. The offline evidence records the exact installed
   SneakerWeb executable SHA-256 and full CLI invocations in addition to crate
   checksum and fixture digests. Any failure stops the feature and preserves
   the old release contract.

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
elsewhere. The checked wrapper documents the Rust mapping
`statements -> LLVM regions`, requires the configured statements and regions
thresholds to agree, and reports both names in its gate output; it does not
quietly treat line coverage as statement coverage.
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
   stale-handle, send-versus-receive terminal result, per-space idempotency,
   concurrent cancellation/progress, and panic-containment tests; then opaque
   tasks and typed DTOs.
5. **Renderer:** failing iOS and Android tests for canonical paths, MIME/CSP,
   absent home pages, external requests, forms, popups, permissions, service
   workers, cross-site reads, process failure, and absence of every Riot bridge;
   then the isolated hosts and loopback resolver.
6. **Library UX:** failing UI/accessibility tests for zero-ceremony opening,
   Sites/Received deduplication, hidden-by-default identifiers, complete Details,
   Received-summary absence of digest/domain IDs, loading, empty, block,
   missing-link, and failure states; then native views.
7. **Sharing:** failing exact selection, required title, temporary-file cleanup,
   carrier/Newswire/host-binding schemas, same-space/signer checks, trusted-app
   capability, opaque host gestures, host-only picker selection/review state
   transitions, app-picker commit success/cancel/retry/duplicate submission and
   suppression/pre/post-commit trust revocation, terminal route/focus/
   announcement behavior, cross-app/cross-space misuse and
   isolation, generic-write rejection for the reserved path, remote binding
   verification without local operation history, multi-community
   partial success and duplicate-free retry, atomic same-space destinations,
   no-replace same-digest contention, direct-request title/site-count preview,
   unavailable-holder, chunk corruption,
   separate send/receive completion, resume, cancel, garbage-collection lease,
   and multi-peer handoff tests; then each route.
8. **End to end:** official CLI -> Riot -> selected export -> official CLI;
   iOS -> Android and Android -> iOS document handoff; Newswire and Sneaker
   Directory sharing across two communities; explicit recipient download;
   cross-app/cross-space isolation; process-death recovery; offline rendering;
   physical Files, share-sheet, local TCP, and BLE rehearsal.

Test helpers include a pinned official `.snk`, deterministic domain authors,
malformed/truncated/mutated corpus generators, a temporary SQLite database,
fault-injecting staged storage, a fake clock and storage quota, an in-memory
portable-blob peer, loopback HTTP requests, native WebView test pages, and a
CLI harness. Fuzz/property tests cover Drop decoding, URL/path conversion,
bounded native HTML title extraction, carrier CBOR, app listing/reference
validation, selection isolation, arbitrary merge order, pathological
capability depth, integer boundaries, block/read/export races, chunk resume,
cross-space inventory leakage, absolute-form HTTP, malformed Host/cookies,
synthetic external navigation, and port teardown/rebind. Deterministic barriers
control finish/cancel/block/database-close races; fault injection covers every
CAS fsync/no-replace-install/transaction crash point, including two contenders
for one digest. No fixture contains a production or
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
scripts/verify-xcresult-tests.sh build/snk-riotkit.xcresult --require-tests \
  --require-suite SneakerWebCoreTests --require-suite SneakerShareTaskTests
scripts/verify-xcresult-tests.sh build/snk-riot.xcresult --require-tests \
  --require-suite SneakerDocumentUITests --require-suite SneakerDirectoryPermissionUITests
(cd apps/android && ../../scripts/android-sneakerweb-test-gate.sh)
scripts/sneakerweb-interop.sh --offline --version 1.0.1
scripts/sneakerweb-physical-rehearsal.sh --require-recorded-results
```

The physical evidence matrix is iPhone 17 Pro/iOS 26.2 and Pixel 9-class/API
36, release builds, airplane mode with Bluetooth/local Wi-Fi selectively
enabled. Ten cold-cache and ten warm-cache runs use fixed 10 MiB and 100 MiB
fixtures; each records wall time, peak RSS (100 MiB open must remain at or below
512 MiB), encoded/retained bytes, transport, completion, and final digests.
Recorded median/worst results and the novice cohort results are committed as a
release evidence artifact. `android-sneakerweb-test-gate.sh` deletes prior unit
and connected-test result directories, records the run start time, executes
`testDebugUnitTest connectedDebugAndroidTest lintDebug`, parses only fresh unit
JUnit XML and connected-test result artifacts, requires positive executed-test
counts in both categories, requires unit suites
`SneakerLibraryViewModelTest`, `SneakerShareCoordinatorTest`, and
`SneakerDirectoryPermissionTest`, requires
connected suites `SneakerDocumentOpenTest` and
`SneakerWebViewIsolationTest` plus `SneakerCommunityShareTest`, and fails on
missing, stale, skipped-only, or malformed results.
Zero executed native tests on either platform is a gate failure.

## Delivery slices and dependencies

1. Complete the approved multi-space SQLite plan through its database lifecycle,
   authoritative store, native API, iOS cutover, restart/isolation tests, and
   release gate. Release construction must instantiate `RiotDatabase`; the
   current replayed `profile.json`/in-memory production path must be absent.
   Until those executable conditions pass, SneakerWeb schema work is blocked.
   The Newswire destination additionally waits for the approved
   multi-community open-newswire record/attachment contract; the Directory
   destination reuses the landed signed-app trust/runtime contract and cannot
   bypass it.
2. Update the product brief's preview-before-ingest rule with a link to this
   fixed-public-namespace, user-opened, reversible exception; update
   `.coverage-thresholds.json` to the executable combined coverage wrapper; and
   pass the Dependency and contract activation gate above.
3. Add bounded decoder/encoder, collection
   schema, and atomic Rust APIs.
4. Add FFI and platform document/library/storage-management surfaces.
5. Add the isolated renderer and adversarial native tests.
6. Add standard file sharing and direct nearby transfer/multiplexer.
7. Add portable blob transfer, type-complete per-space share/direct-transfer
   tasks, and signed public-space carrier cards.
8. Add Newswire attachment integration, the `portable_public_collections`
   capability/host, and the signed Sneaker Directory miniapp with
   multi-community native orchestration.
9. Run cross-client, coverage, performance, accessibility, platform, cohort,
   and physical-device gates.

Slices 2-5 produce useful open/browse capability. Slice 6 completes person-to-
person carrying. Slices 7-8 complete public-community sharing. No slice may claim `.snk`
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
- System, nearby, Newswire, and Sneaker Directory sharing preserve original
  site authority; multi-community retry is duplicate-free and reports each
  destination independently.
- The Directory remains an ordinary trusted social app, and adversarial app
  code cannot enumerate the library, obtain site/blob bytes, or cross app/space
  boundaries through the collection host.
- Blocking persists and is enforced at every ingress/egress/read boundary.
- iOS and Android persist the same collection semantics across relaunch.
- The repository's full test, lint, formatting, platform, coverage, performance,
  and physical-device gates pass.
- User-facing docs explain `Content is intact` versus publisher trust and the
  public nature of shared `.snk` content.
