# Writable public-newswire browser client design

Date: 2026-07-24
Status: User-approved; design-review revisions in progress

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
- Preview and accept same-namespace updates into an active community.
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
2. Rust verifies the full artifact without mutation.
3. The UI displays the community identity, authors, readable updates, and any
   expiry or AI-assisted markers.
4. The user explicitly accepts selected supported entries.
5. Rust atomically prepares the accepted canonical bundle and prospectively
   proves the resulting state remains replayable and exportable within all
   browser ceilings.
6. For an active profile, the host durably appends it in one IndexedDB
   transaction and acknowledges its exact digest.

On a clean browser, accepting a community creates a fresh local member identity
for the imported communal namespace. Imported authors are never impersonated.
The accepted descriptor and selected records, fresh member profile, and exact
bundle travel through the cross-store join transaction. Posting is disabled
until that transaction is complete.

With an active community, an import must authenticate the exact same namespace.
The entire acceptance fails on a namespace mismatch, unsupported record class,
invalid signature, invalid capability, malformed entry, or exceeded bound.

### Export

Rust constructs one consolidated canonical `.riot-evidence` artifact from
`riot-client`'s exact verified live proof ledger. The current descriptor and
every exported post retain their original canonical entry, capability,
signature, and payload component bytes; only the bounded canonical bundle
envelope is newly encoded. The browser downloads it without server involvement.
The UI distinguishes:

- **Saved on this browser**
- **Export prepared**
- **Exchanged**

Preparing a download never claims that another node received it.

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
- a home newswire with community name and offline state;
- a structured update composer;
- an immutable review screen;
- an import preview with selectable supported entries; and
- a community menu with export, full technical identifiers, and the
  prototype-key-storage warning.

The newswire displays headline, body, author label, freshness or expiry, source,
and AI-assisted status where supported by the current record model. Full
namespace, signer, entry, and bundle identifiers are available without
truncation behind technical details.

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
extra, wrong-sized, or wrong-digest asset aborts installation and leaves the
controlling release unchanged. Stable navigations are served from the
controlling release's cached `index.html`; subresources use their
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
- Cross-community and mixed-record imports fail atomically.
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
