# Signed JS apps design

## Purpose and scope

This adds a way for people to share small, signed JavaScript tools through
Riot — apps that read and write their own slice of Willow data and sync
between phones over the same transport already built for evidence content.
The first app is a shared checklist. This is new territory beyond the Phase
0A evidence-sprint scope (`riot-core`'s own doc comment: "Phase 0A evidence
scope only"; time-ledger budget: 16h, 10.35h charged) — it is not charged
against that ledger.

This is a deliberate, narrow exception to the conference-demo fixture's rule
that packages carry no executable JavaScript
(`docs/superpowers/specs/2026-07-11-riot-conference-native-demo-design.md`).
That rule stands for fixed incident-board packages rendered by a built-in
profile. Signed JS apps are a separate, explicitly-trusted mechanism — apps
never run without a space organizer adding them to that space's trust list —
and live in their own namespace, never mixed into incident-board content.

## Prior art adopted

- **[WICG Isolated Web Apps / Web Bundles](https://github.com/WICG/isolated-web-apps)**
  — pack an app's HTML/CSS/JS into one `.wbn` Web Bundle; derive app identity
  from a content hash rather than an author-chosen name.
- **[Holepunch Pear/Hypercore](https://docs.pears.com/)** — validates
  distributing signed apps as ordinary P2P data over an existing replication
  mechanism, rather than a separate app-store channel.
- **[Figma plugins](https://developers.figma.com/docs/plugins/how-plugins-run/)**
  — two-context split: a sandboxed context has data access but no browser
  APIs, an iframe/webview has browser APIs but no data access, bridged by
  message passing with explicit named permissions in the manifest.
- **[VS Code webviews](https://code.visualstudio.com/api/extension-guides/webview)**
  — strict CSP (no remote script or network access, nonce'd scripts),
  `postMessage` bridge, host-side enforcement independent of the sandbox.

We adopt the Web Bundle *packaging* format only — not IWA's separate
Integrity Block signature layer, since Willow's existing entry signature
(reusing `EvidenceAuthor`/`AuthorIdentity` from `willow/identity.rs`) already
covers authorship and integrity, and adding a second signature scheme would
be pure duplication.

## Architecture

A new `crates/riot-core/src/apps/` module, alongside `import/`, `willow/`,
`sync/`, holding: bundle/manifest parsing and signature verification, the
per-space trust list, and `AppDataBridge` (the namespace-scoped read/write
API). It is intentionally kept separate from `import/` (evidence-only) rather
than extending it.

An app bundle is two Willow entries:
- **Manifest** — small CBOR/JSON: `app_id` (hash of manifest core fields +
  bundle digest — content-derived, not author-chosen), `name`, `description`
  (plain language — this is the entire trust surface shown to an approving
  organizer), `version`, `author` (`AuthorIdentity`), `permissions: [...]`
  (named scopes; v1 has exactly one, implicit access to the app's own
  namespace), `entry_point` (path within the bundle).
- **Bundle payload** — the app's HTML/CSS/JS packed as a `.wbn` Web Bundle,
  stored as one payload-bearing entry, reusing the existing payload/entry
  size ceilings from `import/bundle.rs` rather than inventing new ones.

Both entries are signed by the author's existing `EvidenceAuthor` key — no
new cryptography.

Distribution reuses the existing sync/transport stack unchanged: a bundle is
just another signed Willow entry, so it rides `ReconcileSession` /
`ByteSyncSession` and the BLE + local-TCP nearby transport from Task 5.
Receiving a bundle only stores it (inert data); it does not become launchable
by itself.

## Space-scoped trust

**Planning-time correction (2026-07-11):** the paragraph below originally said
trust-list writes use "the space's existing admin `WriteCapability`
(Meadowcap delegation)." Ground-truth investigation while writing the
implementation plan found riot-core never actually uses capability
delegation anywhere — `willow25::WriteCapability::delegate` exists in the
underlying library, but every `EvidenceAuthor` in this codebase only ever
mints a zero-delegation, own-subspace-only communal capability
(`identity.rs::write_capability`). There is no "space admin" capability
concept today. Adding real delegation would be a separate, larger change
(subspace-signature delegation semantics, a new capability-minting/verifying
path) out of scope for a checklist-first MVP. Instead: trust-list authority
uses a **fixed, known organizer subspace_id per space** — the same precedent
already used for the conference fixture's fixed public author identifiers
(`docs/superpowers/specs/2026-07-11-riot-conference-native-demo-design.md`).
A client only honors a trust-list entry if it was authored under a
subspace_id on that space's known-organizer list; entries from any other
subspace at the trust-list path are ignored. This keeps the *intent* of the
original paragraph (reuse an existing authority mechanism, invent no new
permission system) while naming the mechanism that actually exists.

Each space (namespace) has a trust list: a small Willow-stored set of
`app_id`s, written by a recognized organizer subspace (see correction above).
An app is launchable in a
space only once its `app_id` is on that space's trust list. Adding an app to
the list is the one moment a human reads its plain-language `description` and
confirms; after that, it's available to everyone in the space automatically
(trust state syncs like everything else — no per-person approval). Removing
an app from the list stops it from being launched for new sessions; it does
not retroactively hide or delete already-synced app data, consistent with
Willow's append-only model and how the rest of Riot treats revocation.

A bundle whose signature fails to verify is silently excluded from the list
an organizer can choose to trust — same "ineligible items just aren't shown"
pattern the evidence-import path already uses for cryptographically invalid
items.

## Runtime: WebView + bridge API

The app's JS never talks to Willow directly. It calls a small injected
library (`window.riot`), which round-trips through the native host over
`postMessage` (`WKScriptMessageHandler` on iOS, `@JavascriptInterface` on
Android):

```js
riot.get(key)              // -> Promise<value | null>
riot.put(key, value)       // -> Promise<void>, value must be JSON-serializable, size-capped
riot.list(prefix)          // -> Promise<{key, value}[]>
riot.watch(prefix, cb)     // fires cb() as matching entries sync in from other phones
riot.whoami()              // -> { displayName }  (never a raw key)
```

`key`/`prefix` are relative. The native bridge (`AppDataBridge`, exposed via
a small UniFFI surface alongside `mobile_api.rs`) prepends
`apps/<app_id>/<space_id>/` itself and rejects anything that would escape that
prefix (including adversarial inputs like `../`-style keys) — this is
enforced at the host boundary, not left to app-code discipline, matching the
VS Code guidance that the sandbox alone is not sufficient.

Writes are signed with the *using person's* own Riot identity, not the app
author's — same as any other Willow entry — so "who checked this off" is
always a real person. Willow's existing last-write-wins-per-path
reconciliation handles merges; no new CRDT logic is needed.

The loaded HTML carries a strict Content-Security-Policy blocking all
network/remote-script loading — the bridge is the app's only I/O, matching
the nearby-transport principle of never falling back to the internet.

## First app: shared checklist

Built entirely on the bridge above — no checklist-specific Rust code:

```
key: items/<item_id>              (item_id = crypto.randomUUID(), generated
                                    client-side in the app's own JS — no
                                    bridge call needed to create an id)
value: {
  text: string,
  done: boolean,
  updated_by: string,   // display name, from riot.whoami()
  updated_at: number    // ms epoch; UI display order only —
                         // the actual merge is Willow's own per-path
                         // last-write-wins, not this field
}
```

- **Add**: `riot.put("items/" + newId, { text, done: false, updated_by, updated_at })`
- **Check/uncheck**: read-modify-write the same key.
- **List/live update**: `riot.watch("items/", render)` on load; re-renders as
  items sync in from other phones.
- **Delete**: out of scope for v1 (Willow is append-only; a tombstone
  convention isn't needed for a first app). Items can be unchecked instead.

## Error handling and plain-language UI

No "namespace", "bundle", "signature", "sync", or "capability" in anything a
person sees:

| Situation | What the person sees |
|---|---|
| New app arrives, not yet trusted | Doesn't appear in the space's tool list |
| Organizer reviewing an app to trust | App name + plain-language description + "Let everyone in this space use this?" |
| Bad/corrupt bundle (signature fails) | Silently excluded from the list; never a scary error dialog |
| App trusted, opening it | Normal loading, then the app's own UI |
| Bridge write fails (e.g. storage full) | "Couldn't save that — try again" inline in the app, not a crash |
| Data arrives from another phone | List just updates — no "synced!" toast |

## Testing strategy

TDD throughout, mirroring existing `riot-core` conventions:

- **`riot-core`**: unit tests for manifest parsing/validation, signature
  verification (valid/invalid/tampered bundle), trust-list add/remove/check,
  and `AppDataBridge` prefix-scoping, including adversarial key inputs.
- **`riot-ffi`**: contract tests for the new UniFFI surface (install bundle,
  list trusted apps, bridge get/put/list/watch), matching existing
  `mobile_api.rs` test patterns.
- **Native shells**: iOS/Android WebView host tests proving the CSP is
  actually applied and that a malicious test page attempting a network fetch
  or an out-of-scope key is rejected — an explicit adversarial test, not just
  happy-path.
- **Checklist app**: plain JS with no build step; exercised via the same
  bridge stub used in host tests (add/check/uncheck/watch).
