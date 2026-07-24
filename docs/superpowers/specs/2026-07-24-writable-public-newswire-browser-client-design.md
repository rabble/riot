# Writable public-newswire browser client design

Date: 2026-07-24
Status: User-approved; metaswarm design review passed

## Purpose

Build the smallest honest browser version of Riot: a Chromium-only static PWA
that is a real writable public-newswire client. It can create or open one public
community, prepare and locally sign newswire updates, preserve accepted state
across offline reloads, and import or export native-compatible
`.riot-evidence` files.

This slice is not a gateway-backed reader and does not make a server canonical.
The browser owns its signing identity and accepted records. Riot's Rust
implementation remains responsible for canonical encoding, Willow and
Meadowcap authority, signing, verification, preview-first admission, and
projection.

This design narrows and refreshes the broader, unimplemented
`2026-07-13-riot-local-first-pwa-design.md`. Its deliberate prototype
concessions are Chromium-only support and plaintext browser-local key storage.

## Product boundary

### In scope

- A framework-free static PWA under `apps/web`.
- Exactly one active public community.
- Create a new community as its organizer.
- Open the locally stored community after reload.
- Import a community into a clean browser with a fresh local member identity.
- Preview and accept updates bound to the active community's exact signed
  descriptor.
- Compose, immutably review, locally sign, and commit public newswire updates.
- Persist accepted bundles in IndexedDB and the signing profile in
  `localStorage`.
- Reload and read or write while offline after one successful application load.
- Export one canonical `.riot-evidence` artifact compatible with Riot's native
  admission path.
- File import/export as the only exchange transport.
- Chromium desktop and Android Chromium as the initial browser family.

### Out of scope

- Safari, Firefox, and cross-browser compatibility work.
- Multiple active communities or a community switcher.
- Private groups, encrypted group data, and private-to-public bridges.
- Live relay, gateway, WebSocket, WebRTC, WebTransport, iroh, BLE, Bonjour, or
  Tor synchronization.
- Accounts, remote signers, key recovery, identity backup, key rotation, or
  capability revocation.
- Sites, articles, governance, miniapps, arbitrary record classes, and native
  feature parity.
- Push, notifications, analytics, deployment, DNS, and production hosting.
- Claims that the prototype's key storage is hardened against a compromised
  origin.

## Chosen architecture

```text
apps/web
  |- UI and browser I/O
  |- localStorage: versioned signing profile and active-community metadata
  |- IndexedDB: append-only accepted bundle log and drafts
  `- service worker: immutable application assets
          |
          v
riot-web (thin wasm-bindgen adapter)
          |
          v
riot-client (browser-neutral workflow and state machine)
          |
          v
riot-core (Willow, Meadowcap, signing, verification, admission, projection)
```

### `riot-core`

`riot-core` remains the protocol and authorization implementation. Browser code
must not recreate canonical records, signatures, capabilities, namespace
checks, import verification, or projections in JavaScript.

The existing optional `sqlite` feature remains enabled for native builds and
disabled in the Wasm graph. The pinned `willow25` package currently compiles
its filesystem-backed `fjall`/`lsm-tree` store and `async-fs` dependency
unconditionally. A verified, minimal vendor patch will make `fjall` and
`async-fs` optional behind a `persistent-storage` feature and gate only the
`persistent_store` module and `PersistentStore` re-export on
`all(feature = "std", feature = "persistent-storage")`. Native Riot enables
that feature by default; `riot-core`'s Wasm dependency disables it.

Riot does not use Willow's `MemoryStore` as its evidence authority. The Wasm
client uses Riot's existing `MemoryEvidenceStore` while the patched graph
excludes Willow's unused filesystem implementation. Native behavior remains
unchanged.

### `riot-client`

A new ordinary Rust library owns one `PublicNewswireClient`. It is independent
of UniFFI, wasm-bindgen, browser APIs, databases, and transports. It owns:

- creation and restoration of one public-community profile;
- signer-free verified community state;
- organizer or member publisher authority;
- immutable prepared-update reviews;
- preview-first import reviews;
- post, accept, and replay state transitions;
- canonical public-newswire projection;
- native-compatible consolidated export; and
- pending persistence acknowledgements.

The controller returns opaque profile or bundle bytes plus bounded,
versioned DTOs. It never reads or writes browser storage. Mutation that produces
a new canonical bundle enters a pending-persistence state; further mutation is
blocked until the host acknowledges that exact bundle after durable storage.

The in-memory Riot evidence repository intentionally drops capability and
signature bytes after admission, so it cannot be the source of a proof-safe
export. `riot-client` therefore owns a separate exact-proof ledger. For every
accepted entry it retains the verified canonical entry, capability, signature,
and payload component bytes copied from the decoded bundle, plus the accepted
selection and live/pruned result. Export selects the exact live component bytes
and passes them through `riot-core`'s validating bundle encoder, which
re-verifies them and creates only a new canonical outer envelope. It never
reconstructs a signed entry from decoded fields.

### `riot-web`

A new `wasm-bindgen` adapter mechanically converts bounded JavaScript inputs,
byte arrays, Rust DTOs, and stable error codes. It contains no independent
client state and does not depend on `riot-ffi` or UniFFI.

Wasm runs on the page's main thread for this bounded first slice. Moving the
adapter to a dedicated worker is deferred until measurements show that bounded
imports or projections harm responsiveness.

### `apps/web`

The host uses static HTML, CSS, and ES modules without a runtime framework. It
owns:

- DOM rendering and accessible status announcements;
- IndexedDB transactions;
- the versioned `localStorage` profile record;
- Chromium Web Locks;
- file selection and downloads;
- service-worker registration and release coherence; and
- calling the Wasm adapter in the required order.

JavaScript treats signing profiles and canonical bundles as opaque bytes.

## Storage and restoration

### Signing profile

The first slice stores the opaque signing profile, including private signing
material, as base64 in one versioned `localStorage` record. This is an explicit
prototype concession. The UI states that clearing site data loses posting
authority and that export preserves public community data, not identity.

No production-hardening claim is made. A future design must replace this with a
non-extractable or externally controlled signer and a real recovery model.

### Accepted bundle log

IndexedDB contains:

- one active-community record;
- an ordered, append-only accepted-bundle log whose records contain sequence,
  exact bundle bytes, actual byte length, SHA-256 digest, and accepted entry
  IDs;
- a manifest containing the ordered record digests and aggregate counts;
- durable drafts;
- pending create/join operations; and
- schema and application-release versions.

IndexedDB manifest, operation, and active-community metadata are bounded
canonical-CBOR `Blob`s rather than unconstrained structured clones. Operation
metadata and the manifest are each limited to 64 KiB; profile and bundle bytes
are separate Blobs with their own limits. The host checks `Blob.size` before
parsing. Transactions request Chromium's `durability: "strict"`; the product
still calls the result committed browser storage, not power-loss-proof fsync.

The log, rather than a serialized Rust heap or projection, is durable truth. On
startup the host loads the profile and bounded ordered log, then
`PublicNewswireClient` reconstructs state by replaying every bundle through the
ordinary verification and admission path. A corrupt or unverifiable record
opens recovery mode and is never silently skipped.

Browser-local ceilings are fixed for this slice:

```text
profile record                         256 KiB
one canonical bundle                    8 MiB / 64 entries
accepted-bundle log                    256 records / 32 MiB aggregate
accepted entries across replay       1,024
live entries eligible for export        64
one consolidated export                 8 MiB
```

Before any post or import is offered for durable commit, `riot-client`
prospectively applies it to a clone of the current state and proves that the
resulting exact live proof set still encodes as one valid canonical export
within the 64-entry and 8-MiB native limits. It also checks the projected log
record, aggregate byte, and accepted-entry ceilings. Capacity failure changes
nothing and returns `BROWSER_CAPACITY_EXCEEDED`.

The signed descriptor consumes one of the 64 live export entries. Every live
news post consumes another, whether or not its display expiry has passed;
expiry does not prune signed history or free capacity. The UI always shows
remaining entry capacity, shows projected entry and byte use on post/import
review, and warns at five remaining entries or 10% remaining byte capacity.
When full, compose and import confirmation are disabled with: **This prototype
community is full. Export its current records before starting a new local
community.** This slice has no deletion, rollover, or multi-file continuation.

Startup opens only the fixed database, object stores, and keys for the current
schema. It walks the bundle store with a cursor capped at 257 records, checks
each actual `Blob.size` before reading it, stops before exceeding 32 MiB,
recomputes every digest, and compares sequence and digest order with the
manifest. It never sizes a read or allocation from attacker-controlled manifest
counts. Only after those checks does it stage bytes for bounded Rust replay.

### Cross-store create and join transaction

Creation and clean-browser join span IndexedDB and `localStorage`, which cannot
share a browser transaction. Both use the same idempotent two-phase protocol.
Rust first returns one immutable pending result containing:

```text
operation kind and operation ID
profile ID and exact profile bytes
namespace and signer IDs
descriptor entry ID
exact canonical bundle bytes and digest
selected entry IDs (join only)
```

The host then performs these ordered durable steps:

1. Write an IndexedDB operation in `prepared` phase containing that exact
   result, including the pending profile bytes needed after a crash.
2. Write the matching versioned `localStorage` profile as `pending`.
3. In one IndexedDB transaction, append the bundle if its digest is absent,
   update the manifest and active-community record, and mark the operation
   `committed`.
4. Replace the matching `localStorage` record with `active`.
5. Delete the matching IndexedDB operation.
6. Acknowledge the exact profile ID and bundle digest to the live Rust
   controller.

Every step is compare-and-set on operation ID, profile ID, namespace,
descriptor ID, and bundle digest. Repeating a completed step is harmless;
different values fail closed.

Startup resumes only the exact recorded operation:

- `prepared` with no profile or the matching pending profile resumes at step 2
  using the already-generated profile bytes; it never mints a new identity.
- `committed` with the matching pending or active profile resumes at step 4.
- An active profile with matching active-community metadata and no pending
  operation performs ordinary replay.
- Any mismatched profile, namespace, descriptor, digest, phase, bundle, or
  active-community record opens read-only recovery.
- An unrecognized profile without its matching operation and active record
  opens read-only recovery; it is never interpreted as an empty new community.

Pending profile bytes are removed from IndexedDB when the operation is
finalized. Their temporary duplication is part of the acknowledged prototype
key-storage risk.

### Single writer

The page holds an exclusive Chromium Web Lock for the lifetime of a writable
session. If another tab owns the lock, the new tab opens read-only and clearly
reports that Riot is already writable elsewhere. It cannot create, post,
accept, import, change storage, or activate an application update.

## User flows

### First run

The empty state offers:

- **Create community**
- **Import community file**

There is no server login, seed account, or automatic identity replacement.

### Create community

1. The user supplies a bounded local title.
2. Rust generates an organizer signing identity, namespace, and authority.
3. Rust creates and signs the canonical `SpaceDescriptorV1` establishing the
   community identity and founding editorial roster.
4. Rust returns one pending profile plus the exact descriptor bundle,
   descriptor entry ID, and bundle digest.
5. The host completes the cross-store create transaction.
6. Rust becomes writable only after the host acknowledges both the profile ID
   and exact descriptor bundle digest.
7. Only then does the UI report **Saved on this browser**.

The signed descriptor is always bundle zero in the accepted log. An empty log
is not a created community. Replay, post authorization, and export all require
that exact descriptor.

### Open after reload

1. The service worker supplies one coherent version of the static shell and
   Wasm assets.
2. The host obtains the writer lock.
3. It loads the profile and accepted-bundle log.
4. Rust validates the profile, requires bundle zero to contain the matching
   signed descriptor, and replays the complete log.
5. The UI becomes writable only after replay completes successfully.

If public records remain but the signing profile is missing or invalid, Riot
displays those verified records read-only. It must not invent replacement
authority or attribute a new identity to prior posts.

### Compose and post

1. The browser autosaves the bounded draft in IndexedDB.
2. `riot-client` canonicalizes it and returns an immutable human-readable
   review tied to the exact canonical bytes.
3. The user explicitly chooses **Post update**.
4. Rust signs and commits exactly the reviewed bytes, producing one canonical
   bundle, prospectively proves the resulting state remains exportable within
   all browser ceilings, and enters pending-persistence state.
5. One IndexedDB transaction appends that bundle and clears the draft.
6. The host acknowledges the exact bundle digest to Rust.
7. Only then does the UI report the update as saved.

A storage failure keeps the draft and never produces a success claim.

### Import

1. The host rejects files above the byte ceiling before copying them into Wasm.
2. Rust verifies the full artifact without mutation. Any invalid or unsupported
   item rejects the whole file before a selectable preview; valid siblings are
   not silently substituted for the file the user chose.
3. A clean-browser import must contain exactly one valid
   `SpaceDescriptorV1`. Rust pins its namespace and descriptor entry ID and
   requires every selectable record to bind to that exact descriptor. A file
   with zero descriptors, multiple descriptors, or a record bound to another
   descriptor is rejected before confirmation.
4. For an active community, Rust requires the pinned descriptor entry ID as
   well as the namespace to match. The file may contain an identical duplicate
   of the pinned descriptor; a different valid descriptor in the same namespace
   is a different community and is rejected. Descriptor migration and
   governance are out of scope.
5. The UI displays the community identity, authors, readable updates, explicit
   **Expires…** or **Expired…** labels, AI-assisted markers, selected count,
   and projected capacity. All selectable entries are verified and supported.
   On clean import, the single descriptor is shown as required and is not
   deselectable; news posts are selectable, and the required descriptor counts
   toward the selected total. Confirmation is disabled when the selected total
   is zero or the result would exceed capacity.
6. On a clean browser, the review says: **This file restores public community
   records. It does not restore an author or organizer. Riot will create a new
   author stored only in this browser.** The final action is **Add community and
   create my author**.
7. The user explicitly accepts selected supported entries.
8. Rust atomically prepares the accepted canonical bundle and prospectively
   proves the resulting state remains replayable and exportable within all
   browser ceilings.
9. For an active profile, the host durably appends it in one IndexedDB
   transaction and acknowledges its exact digest.

On a clean browser, accepting a community creates a fresh local member identity
for the imported communal namespace. Imported authors are never impersonated.
The accepted descriptor and selected records, fresh member profile, and exact
bundle travel through the cross-store join transaction. Posting is disabled
until that transaction is complete.

With an active community, an import must authenticate the exact same namespace.
It must also bind to the exact pinned descriptor. The entire file is rejected
before selection on a namespace or descriptor mismatch, unsupported record
class, invalid signature, invalid capability, malformed entry, or exceeded
input bound.

### Export

Rust constructs one consolidated canonical `.riot-evidence` artifact from
`riot-client`'s exact verified live proof ledger. The current descriptor and
every exported post retain their original canonical entry, capability,
signature, and payload component bytes; only the bounded canonical bundle
envelope is newly encoded. The browser downloads it without server involvement.
The UI uses only these achieved states:

- **Saved on this browser**
- **Export prepared**

Preparing a download never claims that another node received it, and this slice
never reports **Exchanged**.

## Interface shape

Exact Rust names may change during planning, but the boundary must support these
operations without moving workflow state into JavaScript:

```text
create_community(title) -> pending profile + signed descriptor bundle
confirm_profile_and_bundle_saved(profile_id, bundle_digest) -> community
resume_pending_profile(pending_operation) -> pending profile/bundle state
restore(profile_bytes, community_record, ordered_bundles) -> community
prepare_update(draft) -> immutable update review
post_review(review_id) -> pending canonical bundle
preview_import(bundle_bytes) -> immutable import review
join_reviewed_community(review_id, selected_entry_ids) -> pending profile/bundle
accept_import(review_id, selected_entry_ids) -> pending canonical bundle
acknowledge_bundle_saved(bundle_digest) -> updated community
list_updates() -> projected update list
export_community() -> canonical artifact
close()
```

Every DTO and error crossing Wasm is versioned and bounded. Review identifiers
are single-use and invalidated by relevant state changes.

## UI

The initial surface contains:

- first-run create/import choices;
- a home newswire with community name, remaining capacity, and offline/file-only
  exchange state;
- a structured update composer;
- an immutable review screen;
- an import preview with selectable supported entries; and
- a community menu with export, full technical identifiers, and the
  prototype-key-storage warning.

The newswire displays headline, body, author label, explicit **Expires…** or
**Expired…** state, source, and AI-assisted status where supported by the
current record model. Full
namespace, signer, entry, and bundle identifiers are available without
truncation behind technical details.

After clean-browser import, Home and community settings identify **Your author
on this browser** with its full handle separately from imported authors and the
organizer. They never describe the new member as a recovered identity.

Online or offline, Home says **Nothing syncs automatically. Share updates by
exporting a file for someone else to import.** Offline adds **You can keep
writing; posts save on this browser.** Post success says **Saved on this
browser. Export a file to share it.** Export success says **Export prepared**
and explains that Riot cannot know whether another person received or imported
the file. **Exchanged** is explanatory future vocabulary only and never appears
as an achieved state in this slice.

All actions use labeled native controls. Import choices are a `fieldset` with a
`legend`; selection count and persistence messages use polite live regions.
Opening compose, review, import preview, or a recovery panel moves focus to its
heading. Back/cancel restores focus to the invoking control. Validation errors
are associated with their inputs, preserve entered values, and receive focus as
a summary before the invalid fields. Adding a saved post does not steal focus
from the post-success message or the next deliberate action.

## Recovery interactions

Recovery never deletes or replaces identity automatically.

- **Missing or invalid signing profile, verified log:** Home opens read-only
  with **Posting key missing on this browser**. Verified posts and normal
  `.riot-evidence` export remain available. **Retry storage check** rereads the
  profile; **Start over on this browser** follows the destructive flow below.
- **Corrupt, mismatched, or unverifiable log:** Normal Home and normal export
  remain unavailable because Riot cannot claim the stored history is complete.
  The recovery panel identifies the first failing sequence and sanitized error,
  offers **Retry verification**, permits download of each bounded raw stored
  bundle with **Unverified recovery data — not a Riot export**, and offers the
  destructive start-over flow. A verified prefix may be summarized as
  incomplete but is never presented as the current community or exported as a
  complete artifact.
- **Interrupted create or clean join with an exact matching operation:** The
  page shows **Finishing community setup** and resumes the recorded idempotent
  steps without minting a key. A storage failure exposes **Retry setup**. Any
  mismatch moves to the corrupt-state recovery panel.
- **Terminal Wasm failure:** Mutation stops immediately. The panel offers
  **Reload and retry**, bounded raw bundle downloads labeled unverified, and
  start over. Normal feed and export are unavailable because the failed core
  cannot establish their validity.

**Start over on this browser** first offers export when normal verified export
is available, states that the posting key and all locally stored records will
be removed, and requires a second explicit confirmation containing the full
community name. It then deletes only the fixed Riot storage keys and database.
Cancellation changes nothing. Starting over creates or imports a new local
community; it does not continue a full community under a new descriptor.

## Failure and security model

- Invalid signatures, capabilities, namespaces, paths, timestamps, or bundles
  commit nothing.
- File bytes, entry counts, field lengths, replay-log size, and projected list
  size have explicit ceilings before allocation or rendering.
- Unsupported record classes fail rather than being committed invisibly.
- A failed durable write leaves Rust in a quarantined pending state and leaves
  the draft recoverable.
- Replay failure identifies the failing log position and opens recovery mode.
- Wasm panics become a terminal client error; the page does not continue with
  uncertain state.
- The application loads no third-party runtime JavaScript.
- User text is rendered as text, never interpolated through `innerHTML`.

The PWA requires a dedicated HTTPS origin containing no unrelated application
or user-authored pages. Every response carries these exact baseline headers:

```text
Content-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; worker-src 'self'; manifest-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'
Permissions-Policy: accelerometer=(), ambient-light-sensor=(), autoplay=(), bluetooth=(), camera=(), display-capture=(), encrypted-media=(), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), publickey-credentials-create=(), publickey-credentials-get=(), serial=(), usb=(), xr-spatial-tracking=()
Referrer-Policy: no-referrer
X-Content-Type-Options: nosniff
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
```

There are no inline scripts or styles and no `eval` exception.
`wasm-unsafe-eval` is the sole compilation exception.

Each build emits a content-hashed release manifest containing the release ID,
every authored/generated asset URL, byte length, and SHA-256 digest. During
installation the worker fetches every listed response with `cache: "reload"`,
verifies its actual bytes, and populates a release-named cache. Any missing,
wrong-sized, or wrong-digest listed asset aborts installation and leaves the
controlling release unchanged. A build-time static-root contract fails on any
authored/generated file absent from the manifest; the worker does not pretend
it can enumerate arbitrary extra origin files. Stable navigations are served
from the controlling release's cached `index.html`; subresources use their
content-addressed URLs.

The worker never calls `skipWaiting` or `clients.claim`. A waiting release does
not activate while any page from the controlling release remains open, which
also means it cannot replace code while a writer lock or pending Rust operation
exists. The UI reports **Update ready — close all Riot tabs and reopen**.
Activation and old-cache deletion occur only after all old-release clients are
gone. The next navigation is controlled by one coherent release.

The key stored in `localStorage` is accessible to any successful same-origin
script execution. Compromise of the static host, any same-origin page or
service worker, a browser extension with site access, or the browser profile
permits key theft and lasting impersonation. CSP, a dedicated origin, and
dependency minimization reduce exposure but do not turn this into hardened key
custody. That limitation is visible in onboarding, community settings, and
acceptance documentation.

## Testing

Implementation follows TDD. Tests are written and observed failing before the
corresponding production behavior.

### Rust and graph contracts

- The Wasm dependency graph contains no `fjall`, `lsm-tree`, `async-fs`,
  SQLite, UniFFI, iroh, Tokio transport, BLE, Bonjour, or Arti dependencies.
- The pinned Willow vendor patch has checked upstream provenance, retained
  licenses, and per-file integrity manifests.
- Native `riot-core` and `riot-ffi` builds retain their existing features and
  panic behavior.
- Community creation yields valid organizer authority and a signed descriptor
  bundle that must be acknowledged before posting.
- Prepared review bytes are exactly the bytes later signed.
- Preparation captures the canonical payload and timestamp; posting consumes
  those retained bytes instead of calling the current sign-immediately
  convenience path with reconstructed UI fields.
- Reviews are immutable, state-bound, and single-use.
- Imports cannot mutate before acceptance.
- Invalid/unsupported sibling items reject the whole file before selection.
- Cross-community, mixed-record, same-namespace/different-descriptor, and
  multiple-descriptor clean imports fail atomically.
- Replay returns the same projection and signer relationship.
- The exact-proof ledger preserves accepted component bytes and live/pruned
  selection across replay.
- Prospective post/import admission refuses states that exceed any log, replay,
  or consolidated-export ceiling.
- Exported bundles include the descriptor and re-enter through the
  native-compatible admission path.

### Browser contracts in real Chromium

- Create -> post -> reload displays the same signed update.
- After the first successful load, reload succeeds with the static server
  unavailable.
- Create and clean-browser join recover idempotently from interruption before
  and after every `prepared`, profile-write, `committed`, activation, cleanup,
  and Rust-acknowledgement step.
- Any cross-store identifier, namespace, descriptor, digest, phase, or byte
  mismatch opens read-only recovery and never mints a replacement identity.
- Export -> clean browser context -> import creates a new member identity;
  that member posts offline, reloads, exports, and a third clean browser imports
  the artifact and verifies the post under the member's correct authorship.
- Storage failure never creates a phantom successful post.
- Drafts survive interrupted review or failed persistence.
- A second tab is read-only.
- Clearing the signing profile removes writing authority without erasing or
  reassigning public authorship.
- Invalid, oversized, cross-namespace, and unsupported imports commit nothing.
- Startup rejects wrong Blob sizes/digests, manifest disagreement, record 257,
  aggregate byte 32 MiB + 1, and accepted entry 1,025 before unbounded staging.
- Profile 256 KiB + 1, one bundle 8 MiB + 1, and metadata 64 KiB + 1 fail
  before parsing; a reused digest paired with different bytes fails rather than
  being treated as a duplicate.
- Capacity remaining, near-full, full, projected post/import use, and the
  no-pruning-on-expiry rule are visible before confirmation.
- Missing-key, corrupt-log, interrupted-operation, and terminal-Wasm recovery
  expose exactly their specified read/export/retry/raw-download/start-over
  actions without automatic deletion.
- Failed or partial precache never installs; a waiting worker never activates
  with an old-release client; reopening uses one coherent release.
- Keyboard operation, focus movement, form labels, and status announcements
  work for every primary flow.
- Static security headers and CSP match their exact contracts.

Pure JavaScript helpers receive Node unit coverage. Storage, Web Locks,
service-worker, localStorage, IndexedDB, and reload behavior are tested in
actual Chromium rather than mocked as proof of browser behavior.

## Definition of done

One reproducible static build:

1. opens and installs in Chromium;
2. creates or imports one public community;
3. posts a locally signed public-newswire update without a network;
4. survives a full offline reload with the same signing identity and verified
   projection;
5. exports and imports native-compatible `.riot-evidence` artifacts; and
6. passes workspace tests, strict Clippy, formatting, the locked Wasm build,
   Chromium browser tests, and the coverage floors in
   `.coverage-thresholds.json`.

No server is required for identity, signing, accepted state, rendering, or file
exchange after the application assets have loaded once.
