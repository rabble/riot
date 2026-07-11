# JS apps runtime design (iOS first)

## Purpose and scope

This is the runtime layer for signed JS apps: everything between the core
platform (`2026-07-11-signed-js-apps-design.md` and its implementation plan
`docs/superpowers/plans/2026-07-11-signed-js-apps-core-platform.md`) and a
person on an iPhone checking items off a shared checklist inside their
space. The app-directory design
(`2026-07-11-app-directory-design.md`) covers discovery, sharing,
endorsement, and the full storefront; this design covers *running* — the
WebView host, the `window.riot` bridge, live updates, and the minimal
launch/approve surface an end-to-end demo needs.

**Platform scope:** iOS only this round. Android follows in a later round
against the same FFI surface.

**End state (definition of done):** on an iOS simulator — create a space,
see the checklist in the space's pending tools, approve it as the
organizer ("Let everyone in this space use this?"), open it from the Tools
row, add and check items, relaunch the app, and the items are still there.
The whole flow is proven by XCUITest, plus adversarial host tests showing
a hostile page cannot reach the network or another app's data.

**Sequencing:** core platform plan Tasks 2–6 land first (all `cargo test`
verifiable; Task 1 landed as `4c07956`, Task 2 is in progress in this
checkout). This design's work builds strictly on that FFI surface.

## Boundaries with the two neighboring specs

Adopted *from* the app-directory design (pulled forward, not reinvented,
so nothing built here is thrown away when the storefront lands):

- **Starter catalog arrival** (`apps/starter.rs`): the checklist ships
  embedded in the binary via `include_bytes!` as a manifest+bundle pair
  signed by a fixed Riot project author identity — same
  fixed-public-author precedent as the conference fixture. Built-ins run
  through the exact same decode/verify path as synced apps; "Built into
  Riot" is a provenance label, not a trust shortcut. There is no install
  step and no dev import button.
- **Tools row** per space: daily-use launch surface, checked against the
  trust list at open time.
- **Review moment**: this round builds a minimal version of the
  directory's review page — name, author, plain-language description,
  "THIS APP CAN" permissions box, and the organizer action "Let everyone
  in this space use this?". The storefront round later expands this same
  screen; it does not replace it.

Left *to* the app-directory round: storefront browsing, endorsements,
share-to-a-space, provenance ranking, the `riot-app` CLI, and Android UI.

Left *to* the core platform plan (prerequisites used as-is): manifest and
bundle codecs, `app_id` derivation, trust-list evaluation with the
known-organizer subspace rule, prefix queries, `AppDataBridge` scoping,
and the base UniFFI surface.

## Checklist app: source and packing

The app lives at `fixtures/apps/checklist/` as plain files — `index.html`,
`app.js`, `style.css`, and a `riot-app.json` manifest source (name,
description, version, entry point) — no build step, no framework, exactly
the shape the directory design's future CLI packs.

Packing for the starter catalog: a small `pack_app_dir` function in
`riot-core` (new `apps/pack.rs`) reads a directory, validates
size caps and the entry point, and emits the signed manifest+bundle bytes.
It is called from a `cargo xtask pack-starter-apps` step whose output is
committed and embedded via `include_bytes!` (checked in a test that
re-derives the bytes and compares, so drift between source and embedded
pack fails CI rather than shipping stale). The future `riot-app pack` CLI
wraps this same function.

**Correction (2026-07-11, planning-time ground truth):** an earlier draft
of this section had the starter pack signed with a committed dev-only
keypair. Investigation while writing the implementation plan found there
is no manifest/bundle signature mechanism anywhere in the codebase to
reuse: app integrity is *content-addressed* — strict canonical CBOR
decoding plus the content-derived `app_id` (`manifest.rs::app_id_for`) —
and entry-level Willow signatures only exist where entries cross devices,
where the app-directory design already assigns signing to the *carrier*
at share time, not the author at pack time. So the starter catalog
commits no key material at all: the embedded manifest+bundle bytes are
verified through the standard `decode_manifest`/`decode_app_bundle`
canonical path with the `app_id` re-derived on load, the manifest's
`author` field carries a fixed committed *public* identity (the
conference fixture's fixed-public-author precedent; placeholder values
pinned in the implementation plan, replaced by a real project identity in
the directory round), and tampering with embedded bytes fails decode or
changes the `app_id` — either way the tampered app is silently excluded
or arrives untrusted. Launch authorization remains entirely with the
space organizer's trust marker.

## Organizer approval flow

When a space is created locally, the creator's subspace_id is recorded as
that space's known organizer — the concrete local instance of the platform
spec's fixed-known-organizer mechanism. Spaces defined by fixtures keep
the fixed organizer identifiers their fixture declares (conference-demo
precedent); no new joining mechanism is introduced here.

The Spaces tab's space view gains a **Tools** section:

- Trusted apps: tappable rows (name only in v1), opening the app
  full-screen.
- Pending apps (valid, verified, not yet trusted — in v1 this is the
  starter catalog's checklist): a row with a "Review" affordance opening
  the review sheet.
- The review sheet shows: app name, author display name, the
  plain-language description from the manifest (the entire trust surface,
  per the platform spec), a "THIS APP CAN" box rendered from the
  manifest's permission list ("Keep its own notes in this space. Nothing
  else — no internet, no photos"), and the button **"Let everyone in this
  space use this?"**. Approving writes the trust marker through the core
  plan's trust FFI; the row moves to the trusted list.
- Non-organizers see pending apps with "Ask an organizer to turn this on"
  (matching the directory design) — not exercised in the v1 demo, where
  the local user is the organizer of their own space.

Revocation UI is out of scope this round (the core trust FFI supports it;
the storefront round surfaces it). Launch checks trust at open time, so a
revoked app quietly disappears from the Tools row rather than erroring.

## iOS WebView host

New FFI additions (small, alongside the core plan's `apps_ffi.rs`
surface): `pending_apps(space)` / `trusted_apps(space)` listing summaries
(app_id, name, description, permissions), and
`app_resource(app_id, path) -> bytes` serving a single file out of a
stored, signature-verified bundle. Resource lookup rejects path traversal
in Rust — the host never touches bundle bytes directly.

`AppRuntimeView` wraps a `WKWebView`:

- **Serving**: a custom `WKURLSchemeHandler` for
  `riot-app://<app_id>/<path>` backed by `app_resource`. Nothing is
  unpacked to disk. Unknown paths get a 404-equivalent failure.
- **CSP**: every response carries
  `Content-Security-Policy: default-src 'none'; script-src 'self';
  style-src 'self'; img-src 'self' data:` — the page cannot load remote
  script or reach the network. Navigation policy additionally refuses any
  non-`riot-app://` navigation, and `WKWebsiteDataStore.nonPersistent()`
  keeps no cross-app browser state.
- **Bridge**: a `riot.js` `WKUserScript` injected at document start
  defines `window.riot` (`get`, `put`, `list`, `watch`, `whoami`) over
  `window.webkit.messageHandlers.riot.postMessage`, with correlation ids
  resolving promises. The native `AppBridgeController`
  (`WKScriptMessageHandler`) validates message shape and forwards to the
  FFI bridge, which enforces prefix scoping and size caps in Rust — host
  discipline is not the security boundary, matching the platform spec.
  `whoami` returns the profile display name only, never key material.
- One `AppRuntimeView` per (app_id, space); presented as a full-screen
  cover with a close button from the Tools row.

## Live updates (`watch`)

No polling loop. `riot.watch(prefix, cb)` registers the callback in
`riot.js`; the native host fires a `riot-data-changed` event into the page
(`evaluateJavaScript`) after (a) a successful `put` from that page, (b) a
sync session completing, (c) the app returning to foreground. On the
event, `riot.js` re-runs `list(prefix)` and invokes matching callbacks
with fresh rows. This is deliberately coarse — the checklist re-renders
its whole list, which is fine at v1 sizes (bridge values are size-capped;
lists are small).

Because app data rides the existing sync stack, two-phone live sync works
whenever nearby sync runs; the two-device demo itself stays out of scope
this round, consistent with physical-radio testing being deferred in the
transport work.

## Error handling and plain-language UI

Extends the platform design's table; still no "bundle", "signature",
"namespace", "sandbox", or "sync" in anything a person sees:

| Situation | What the person sees |
|---|---|
| Starter app not yet trusted | Listed under "New" in Tools with "Review" — never launchable |
| Corrupt/tampered embedded pack | Silently excluded (verify path failed); Tools section just doesn't list it |
| Organizer approves | Row moves to the space's tools; everyone in the space gets it |
| Non-organizer taps a pending app | "Ask an organizer to turn this on" |
| Bridge write fails (storage full) | "Couldn't save that — try again" inline in the app |
| Page attempts network fetch | Fails inside the page (CSP); no user-facing dialog |
| App revoked while open | Current session finishes; next open, row is gone |
| Data arrives from another phone | List just updates — no toast |

## Testing strategy

TDD throughout, matching existing conventions:

- **`riot-core`**: `pack_app_dir` golden test (stable `app_id` for
  identical input; oversize, missing entry point, and traversal-named
  resources rejected); starter-catalog test with a deliberately corrupted
  embedded pack proving silent exclusion; embedded-bytes drift test.
- **`riot-ffi`**: contract tests for `pending_apps`/`trusted_apps`
  (starter app appears pending, flips to trusted after the trust call)
  and `app_resource` (serves entry point, rejects traversal and unknown
  app_id) — same in-process harness as existing FFI tests.
- **RiotTests (host)**: scheme handler serves bundle bytes with the exact
  CSP header and refuses out-of-bundle paths; bridge controller rejects
  malformed and oversized messages; navigation policy refuses
  non-`riot-app://` loads. An explicit adversarial page attempts a network
  `fetch` and an out-of-scope key through the bridge — both must fail.
- **RiotUITests (XCUITest)**: the definition-of-done flow — create space →
  review → approve → open checklist → add item → check it → relaunch →
  still there.
- **Checklist JS**: exercised through the real WebView in host tests (its
  behavior is add/check/uncheck/watch against the bridge; no separate JS
  test runner in v1).
