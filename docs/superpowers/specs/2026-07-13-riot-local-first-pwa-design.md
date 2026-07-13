# Riot local-first PWA vertical slice design

Date: 2026-07-13
Status: User-approved direction; pending metaswarm design review

## Purpose

Ship the smallest honest browser version of Riot: a public/community-space PWA
that can be opened from an ordinary URL, installed, reloaded offline, create a
locally signed alert, preserve that alert across reloads, and export/import the
same bounded `.riot-evidence` artifact used by the native core.

This is a vertical slice, not feature parity. It proves that the browser can be
a real local-first Riot client rather than a thin view onto a canonical server.
The static host distributes application code; the browser owns its identity and
accepted state. A future remote signer may recover the root identity and enroll
scoped local device keys through OAuth, but no remote signer is required or
implemented in this slice.

## User and value

Primary user: a community organizer or participant who can reach Riot in a
browser before connectivity degrades and needs a usable local copy afterward.

Use cases:

1. **A participant wants to open and install Riot before an event so that they
   can still read their accepted public/community information when the network
   disappears.** When the application has loaded once, the shell and committed
   space state remain usable in an offline browser session.
2. **An organizer wants to publish a time-bounded alert from a laptop so that
   nearby participants can carry a signed update without installing a native
   build.** After explicit review and Sign, the browser produces a canonical
   Willow entry and commits it through Riot's existing preview-first admission
   path.
3. **A participant wants to move information by file so that exchange still
   works when no compatible online peer is reachable.** Export downloads a
   `.riot-evidence` bundle; import previews and explicitly accepts eligible
   entries before changing the local store.
4. **A returning participant wants the same local identity and accepted entries
   after a normal reload so that the PWA behaves like an application, not a
   disposable demo.** Browser-local encrypted identity material and an
   append-only accepted-bundle log rehydrate the Rust core on startup.
5. **A participant whose browser storage was cleared wants an honest recovery
   state so that Riot never implies missing local information is still safe.**
   The empty state explains that this browser has no local profile or spaces and
   offers Import; it never silently creates a replacement identity and presents
   it as the prior one.

The slice succeeds when a first-time user can complete open → create → sign →
reload offline → see the same alert → export in under three minutes without a
server-side account. It fails as a product proof if signing or reload requires
the network, if reload changes the signing identity, or if imported bytes bypass
preview/accept.

## Chosen approach

Use a framework-free PWA host plus a small `wasm-bindgen` adapter around
`riot-core`.

Alternatives rejected for this slice:

- Extending the Python public gateway would produce an installable reader but
  would not prove browser-owned state or signing.
- A conventional web API holding state and keys would be faster to scaffold but
  would make the host canonical and undermine Riot's shutdown-resistant model.
- Reimplementing Willow, Meadowcap, or bundle encoding in JavaScript would
  create a second protocol implementation without conformance evidence.

## Scope

### In scope

- Framework-free responsive PWA shell at `apps/web/`.
- One local profile and one organizer-created public/community space.
- Seeded demo-space import through the real `riot-core` decoder and verifier.
- Current alert listing with full signer ID, freshness, expiry, and AI-assisted
  status outside any miniapp content.
- Structured alert creation, explicit review, local Ed25519 signing, and commit.
- Preview-first `.riot-evidence` file import and explicit acceptance.
- Export of the browser's accepted canonical bundles.
- Browser-local encrypted identity persistence and accepted-bundle persistence.
- Offline reload after one successful online load.
- A signer boundary whose only implementation is local in this slice but whose
  request/result contract can later support a remote recovery authority.

### Out of scope

- Private groups, MLS, encrypted group drops, group rendezvous, or the explicit
  private↔public bridge.
- Nearby BLE, Bonjour, WebRTC, WebTransport, WTP, or live multi-peer sync.
- Remote OAuth, remote signing, recovery UI, device enrollment, or capability
  revocation.
- Hosted user accounts, server-side canonical state, analytics, notifications,
  background sync, or push.
- Arbitrary third-party miniapp installation. Existing bundled miniapps remain
  separate from this proof.
- Deployment, DNS, TLS, and production hosting configuration.

## Architecture

```text
static host
  index.html + app.js + styles.css + manifest + service worker
                              |
                              v
browser UI ---- typed JS host/controller ---- riot-web WASM adapter
   |                         |                     |
   |                         |                     v
   |                         |                 riot-core
   |                         |        Willow / Meadowcap / bundles /
   |                         |        preview-plan-commit admission
   |                         |
   |                         +---- BrowserVault (IndexedDB + WebCrypto)
   |                         +---- BundleLog (IndexedDB, append-only)
   |                         +---- File import/export
   |
   +---- service worker cache (immutable application assets only)
```

The service worker never caches mutable user data and never owns signer
material. IndexedDB is the persistence boundary. On startup, the JS host opens
the vault, restores the sealed local identity, creates a fresh in-memory core
store, and replays accepted canonical bundles through the same inspect → plan →
commit path used for new imports. No serialized internal Rust store is trusted.

### Rust/WASM adapter

A new `riot-web` workspace crate is a `cdylib` and ordinary Rust library. It
exports one opaque `WebRiot` object and JSON-shaped DTOs through `wasm-bindgen`.
The adapter owns the non-`Clone`, non-`Debug` `EvidenceAuthor`, one
`EvidenceStore`, pending draft state, and a bounded list of canonical bundles
needed for export/persistence.

Public operations:

- `create_profile() -> PersistedIdentity`
- `open_profile(wrapped_key_bytes, sealed_identity_bytes) -> IdentityView`
- `create_public_space(title) -> SpaceView`
- `import_preview(bundle_bytes, route) -> ImportPreviewView`
- `accept_import(preview_id, selected_entry_ids) -> CommitView`
- `create_alert(draft) -> ReviewView`
- `sign_review(review_id) -> CommitView`
- `list_alerts() -> AlertView[]`
- `export_bundles() -> Uint8Array[]`
- `close()`

`PersistedIdentity` contains only the core's sealed identity blob plus a random
32-byte wrapping key that must immediately be protected by `BrowserVault`; the
adapter zeroizes its temporary copy. No API returns a Willow subspace secret or
owned namespace root.

Every opaque preview/review ID is single-use, session-bound, and rejected after
replacement, commit, or close. The adapter maps internal errors to a closed
enumeration of stable browser error codes and never exposes debug strings that
could contain input bytes.

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
the sealed Riot identity in WASM. This protects storage at rest from casual
IndexedDB inspection; it does not claim protection from malicious same-origin
code while Riot is unlocked. Clearing browser site data destroys this local
profile unless the user exported data. Remote recovery is future work.

### Accepted bundle log

The first slice stores accepted-only canonical bundle bytes in an append-only
IndexedDB object store keyed by SHA-256. After a selective import, the Rust
adapter re-encodes exactly the selected, verified frames into a normalized
bundle; rejected or unselected frames never enter persistence or export. A
bounded manifest records insertion order and total retained bytes. Startup
replay is deterministic and stops with an actionable recovery state if any
stored bundle no longer passes core verification. Duplicate hashes do not grow
storage.

The retained browser log uses the core's existing 16 MiB store budget as a hard
upper bound. Browser code rejects a write before IndexedDB mutation if the
manifest would exceed that bound. A transaction writes the bundle and manifest
atomically. Export returns the exact accepted bundles; it does not invent a new
container format.

### Future signer boundary

The UI/controller treats signing as an asynchronous operation even though the
only implementation in this slice is local. Its conceptual contract is:

```text
SignerBackend.public_identity() -> public identity
SignerBackend.sign(canonical_bytes, signing_context) -> signature
SignerBackend.status() -> local | remote-online | remote-offline | locked
```

A future remote backend may use OAuth Authorization Code + PKCE to authorize a
Keycast-like Willow signer. That service must sign domain-separated canonical
Willow/Meadowcap bytes and must not reinterpret drafts. NIP-46 is a useful
interaction model but is not a drop-in protocol because Riot uses Ed25519
Willow signatures rather than Nostr secp256k1 events. The intended future model
is remote root/recovery authority plus scoped, expiring local device
capabilities, so ordinary signing continues offline.

This slice does not refactor `riot-core` into a fully pluggable signer. It keeps
the adapter boundary narrow so the later signer design can be separately
specified and reviewed instead of smuggling network authority into the proof.

## User interface

The PWA has three top-level views:

1. **Spaces** — identity strip, offline/online indicator, current public space,
   alert timeline, and Import/Export actions.
2. **Create alert** — structured fields matching `AlertDraft`, followed by a
   non-editable review step showing exact headline, description, expiry,
   sources, affected area, and AI-assisted status. Only the review step has the
   Sign button.
3. **Import review** — filename/route, byte size, eligible count, full entry
   IDs/signers, invalid count, and explicit Accept/Cancel. No automatic commit.

Persistent chrome outside content shows the complete local signer ID, current
space namespace, offline status, and whether the displayed entry has valid
signature/capability. IDs are never truncated.

States:

- First run: explanation plus `Create local profile` and `Import`.
- Loading/unlocking: disabled actions and a plain progress label.
- Ready online/offline: identical core actions; only network-dependent copy
  changes.
- Empty space: create-alert CTA and import CTA.
- Storage cleared/corrupt: no silent reset; explain what is missing, preserve
  recoverable raw bundles if possible, and offer export/import.
- Storage full: refuse the mutation and offer Export before cleanup.
- Unsupported browser: explain required WebAssembly, IndexedDB, WebCrypto, and
  service-worker capabilities; keep the public gateway link available.

The interface follows Riot's existing field-document visual language rather
than diVine brand rules: restrained paper/dark surfaces, high-contrast status
colors, monospace provenance, no gradients, 44px minimum targets, reduced
motion support, and responsive operation down to 320 CSS pixels.

## Data flows

### First run

1. Register service worker after the page is interactive.
2. Feature-detect WASM, IndexedDB, WebCrypto, and service workers.
3. User chooses Create local profile.
4. WASM creates and seals the identity; BrowserVault protects the wrapping key.
5. WASM creates the organizer-bound communal space.
6. The space identity and empty log manifest commit atomically in IndexedDB.
7. UI shows the complete signer and namespace IDs.

### Create and sign

1. UI validates field presence, byte ceilings, expiry, and source claims for
   immediate feedback.
2. WASM repeats authoritative validation and creates a review object without
   signing.
3. UI renders the immutable review.
4. Human presses Sign.
5. WASM freezes IDs/time, signs canonical entry bytes locally, encodes one
   canonical bundle, and commits it through inspect → plan → commit.
6. Browser persists the exact bundle before reporting durable success.
7. UI refreshes from the core's live view.

If persistence fails after core commit, the controller keeps the generated
bundle in memory, presents `Not saved to this browser`, and offers immediate
download. It never claims durable success.

### Import

1. Browser rejects files above the existing 2 MiB preview ceiling before
   allocation into WASM.
2. WASM decodes and verifies without mutation.
3. UI shows eligible and rejected counts and complete identities.
4. User selects entries and presses Accept.
5. WASM commits atomically and returns a normalized canonical bundle containing
   exactly the selected accepted entries; browser appends that bundle in one
   IndexedDB transaction.
6. If browser persistence fails, the same non-durable recovery behavior applies.

### Startup and offline reload

1. Load application assets from network or service-worker cache.
2. Open vault and bundle log.
3. Restore identity into WASM.
4. Replay every stored bundle through normal verification/admission.
5. Render only after replay completes; progress shows bundle count.
6. A corrupt bundle stops replay and enters recovery instead of being skipped.

## Security model

### Trusted

- The currently loaded, versioned Riot application release.
- Browser WebCrypto implementation and same-origin isolation.
- `riot-core` canonical encoding, signature verification, capability checking,
  limits, and preview/plan/commit semantics.

### Untrusted

- Every imported file and its filename/MIME type.
- Every accepted public entry's claims; valid signatures do not assert truth.
- The static host as a source of availability. A compromised future host
  release is capable of same-origin abuse, so production deployment requires a
  separate signed-release/update policy before private groups are considered.
- IndexedDB bytes on startup until replayed and verified.

### Controls

- Strict CSP: self-hosted scripts/styles only; `object-src 'none'`,
  `base-uri 'none'`, `frame-ancestors 'none'`, `connect-src 'self'` in this
  no-sync slice, and no inline script.
- No third-party runtime dependencies, analytics, fonts, or remote assets.
- No `innerHTML` for imported/user content; render with text nodes.
- Existing core byte/count/path/store ceilings remain authoritative.
- File input is size-checked before `arrayBuffer()` and rechecked in Rust.
- Full IDs in UI and logs; secret/capability/signature bytes never logged.
- Service worker caches only an explicit versioned allowlist and deletes old
  application caches on activation. It cannot intercept IndexedDB.
- Browser vault schema and AAD are versioned; authentication failure never
  falls back to a new identity.
- Sign is always a human action on a stable review; no miniapp or imported page
  receives signing access.
- Private groups are excluded because a hosted origin has a stronger active
  code-update threat than signed public data can tolerate without further work.

Known limitation: while the PWA is unlocked, malicious same-origin code could
invoke local signing or extract decrypted material from application memory.
The vertical slice makes no hardware-backed or host-compromise resistance
claim. The future remote authority reduces durable root exposure but does not
remove the need for signer-side policy and human-readable approval.

## Errors and recovery

Stable user-facing categories:

- `UNSUPPORTED_BROWSER` — required platform API missing; link to public gateway.
- `IDENTITY_LOCKED` / `IDENTITY_CORRUPT` — do not create a replacement; offer
  import/recovery.
- `IMPORT_TOO_LARGE`, `IMPORT_REJECTED`, `NO_ELIGIBLE_ENTRIES` — no mutation.
- `PREVIEW_STALE` — ask user to review again.
- `INVALID_DRAFT` / `EXPIRED_DRAFT` — return to editable form without signing.
- `STORE_FULL` — no mutation; offer export.
- `PERSISTENCE_FAILED` — core may hold an in-memory commit; show non-durable
  state and offer exact bundle download.
- `REPLAY_FAILED` — stop at the first corrupt record and offer raw-log export;
  never silently skip.
- `INTERNAL` — close the WASM session, retain persisted bytes, and offer reload.

All errors are actionable, contain no raw imported bytes or secrets, and are
rendered in an `aria-live` region.

## Testing and TDD

`.coverage-thresholds.json` remains the source of truth: 100% lines, branches,
functions, and statements; `cargo tarpaulin --fail-under 100` is blocking. New
Rust production branches require host-runnable tests even when browser
integration tests also cover them.

TDD work proceeds in independently green slices:

1. **WASM build contract (RED first):** a target check fails on the current
   `getrandom` browser configuration. Add only target-scoped dependency features
   needed for `wasm32-unknown-unknown`; do not enable conformance or filesystem
   storage in the release graph. Tests assert the release feature closure and
   successful WASM compilation.
2. **Adapter lifecycle:** host Rust tests cover create/open/close, complete IDs,
   invalid sealed identity, single-use review IDs, stale preview, invalid draft,
   expiry, sign/commit, duplicate import, export exactness, and error mapping.
   Tests are written against the ordinary Rust library surface before the
   `wasm-bindgen` wrappers are added.
3. **BrowserVault and BundleLog:** Playwright browser tests begin with missing
   implementations and cover first creation, reopen, authenticated-decryption
   failure, duplicate bundle, atomic transaction abort, byte budget boundary,
   storage clear, and corrupt replay. IndexedDB is real, isolated by a unique
   origin/database per test; no in-memory mock certifies persistence.
4. **UI flows:** Playwright tests cover first run, create→review→sign, cancel,
   offline reload, import preview→accept, rejected import, export download,
   storage-full recovery, 320px layout, keyboard-only operation, reduced
   motion, and no horizontal overflow.
5. **Security/static contracts:** tests assert the CSP, zero external requests,
   service-worker allowlist, no `innerHTML`, no remote URLs/assets, no secret
   logging tokens, no write without review, and no private-group surface.

Required verification:

```text
cargo test --workspace --all-features
cargo check --workspace --all-features
cargo check -p riot-web --target wasm32-unknown-unknown
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo tarpaulin --fail-under 100
node --test <web contract tests>
npx playwright test <web PWA tests>
```

The browser suite runs Chromium and WebKit at minimum because storage,
service-worker, and WebCrypto behavior—not just DOM rendering—is in scope.

## Acceptance criteria

1. A clean browser can create one local profile and public space; the complete
   signer and namespace IDs are visible and stable across reload.
2. A human can create, review, sign, and durably persist a valid alert without
   any network request after the initial application load.
3. The same alert and identity appear after browser restart in offline mode.
4. Exported canonical bytes import through preview/accept into a second clean
   browser context and preserve complete signer/namespace identity.
5. Invalid, oversized, corrupt, stale, expired, duplicate, and storage-full
   paths have deterministic tested outcomes and never silently mutate state.
6. The PWA makes zero third-party runtime requests and remains usable with the
   origin offline after first load.
7. No server stores user state or signing material, and no remote signer is
   needed for this slice.
8. All repository quality gates and the 100% coverage enforcement command pass.

## Future work

- Specify the OAuth recovery authority and scoped Meadowcap device enrollment
  as a separate design with signer-side policy, revocation, audit, and offline
  expiry behavior.
- Add gateway/relay synchronization through the same reconciliation primitive
  as other transports; do not create a canonical web database.
- Threat-model private groups, signed PWA releases, unlock UX, plaintext
  lifetime, backups, and storage eviction before exposing encrypted group state
  to the hosted origin.
- Add bundled miniapps only after their existing containment guarantees are
  reproduced in the browser host without granting signing authority.
