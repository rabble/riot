# Writable public-newswire browser client design

Date: 2026-07-24
Status: User-approved; pending metaswarm design review

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
its filesystem-backed `fjall`/`lsm-tree` store unconditionally. A verified,
minimal vendor patch will make those dependencies optional behind a
`persistent-storage` feature and gate only `PersistentStore`. The Wasm graph
uses `MemoryStore`; native behavior remains unchanged.

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
- an ordered, append-only accepted-bundle log;
- durable drafts;
- pending import/join metadata; and
- schema and application-release versions.

The log, rather than a serialized Rust heap or projection, is durable truth. On
startup the host loads the profile and bounded ordered log, then
`PublicNewswireClient` reconstructs state by replaying every bundle through the
ordinary verification and admission path. A corrupt or unverifiable record
opens recovery mode and is never silently skipped.

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
3. The host saves the returned versioned profile in `localStorage`.
4. The host creates the active-community IndexedDB record.
5. Only after both steps succeed does the UI report **Saved on this browser**.

An organizer with an empty accepted-bundle log is valid. If profile storage
succeeds but IndexedDB initialization is interrupted, startup can reconstruct
the empty community from the versioned profile rather than replacing its key.

### Open after reload

1. The service worker supplies one coherent version of the static shell and
   Wasm assets.
2. The host obtains the writer lock.
3. It loads the profile and accepted-bundle log.
4. Rust validates the profile and replays the log.
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
   bundle and entering pending-persistence state.
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
5. Rust atomically prepares the accepted canonical bundle.
6. The host durably appends it and acknowledges its exact digest.

On a clean browser, accepting a community creates a fresh local member identity
for the imported communal namespace. Imported authors are never impersonated.
Pending join metadata in IndexedDB makes interruption recoverable across the
separate IndexedDB and `localStorage` boundaries.

With an active community, an import must authenticate the exact same namespace.
The entire acceptance fails on a namespace mismatch, unsupported record class,
invalid signature, invalid capability, malformed entry, or exceeded bound.

### Export

Rust constructs one consolidated canonical `.riot-evidence` artifact from the
verified active state. The browser downloads it without server involvement.
The UI distinguishes:

- **Saved on this browser**
- **Export prepared**
- **Exchanged**

Preparing a download never claims that another node received it.

## Interface shape

Exact Rust names may change during planning, but the boundary must support these
operations without moving workflow state into JavaScript:

```text
create_community(title) -> pending profile
confirm_profile_saved(profile_id) -> community
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
- A restrictive CSP permits only same-origin authored scripts, styles, workers,
  images, and Wasm.
- The application loads no third-party runtime JavaScript.
- User text is rendered as text, never interpolated through `innerHTML`.
- Service-worker releases are content-versioned and activate only on a clean
  reload so JS and Wasm versions cannot mix.

The key stored in `localStorage` is accessible to any successful same-origin
script execution. CSP and dependency minimization reduce exposure but do not
turn this into hardened key custody. That limitation is visible in the product
and acceptance documentation.

## Testing

Implementation follows TDD. Tests are written and observed failing before the
corresponding production behavior.

### Rust and graph contracts

- The Wasm dependency graph contains no `fjall`, `lsm-tree`, SQLite, UniFFI,
  iroh, Tokio transport, BLE, Bonjour, or Arti dependencies.
- The pinned Willow vendor patch has checked upstream provenance, retained
  licenses, and per-file integrity manifests.
- Native `riot-core` and `riot-ffi` builds retain their existing features and
  panic behavior.
- Community creation yields valid organizer authority.
- Prepared review bytes are exactly the bytes later signed.
- Reviews are immutable, state-bound, and single-use.
- Imports cannot mutate before acceptance.
- Cross-community and mixed-record imports fail atomically.
- Replay returns the same projection and signer relationship.
- Exported bundles re-enter through the native-compatible admission path.

### Browser contracts in real Chromium

- Create -> post -> reload displays the same signed update.
- After the first successful load, reload succeeds with the static server
  unavailable.
- Export -> clean browser context -> import restores public records with a new
  member identity.
- Storage failure never creates a phantom successful post.
- Drafts survive interrupted review or failed persistence.
- A second tab is read-only.
- Clearing the signing profile removes writing authority without erasing or
  reassigning public authorship.
- Invalid, oversized, cross-namespace, and unsupported imports commit nothing.
- Service-worker updates keep authored JS and generated Wasm coherent.
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
