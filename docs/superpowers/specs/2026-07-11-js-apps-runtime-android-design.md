# JS apps runtime design (Android)

## Purpose and scope

The Android twin of `2026-07-11-js-apps-runtime-ios-design.md`: the runtime
layer for signed JS apps between the core platform
(`2026-07-11-signed-js-apps-design.md`, landed through `d2aae48`) and a
person on an Android phone checking items off a shared checklist inside
their space. The iOS design's decisions are settled and adopted here
unchanged wherever they are platform-neutral — trust model, review moment,
`window.riot` API shape, plain-language UI, coarse `watch` semantics. This
document only decides the Android-specific host mechanics and states
precisely what is buildable against the FFI surface that is landed *today*
versus what is gated on the app-directory and iOS-runtime plans.

**End state (definition of done, non-gated portion):** on an Android
emulator — create a space, install the checklist from signed bytes, review
it as the organizer ("Let everyone in this space use this?"), open it from
the space's Tools section, add and check items, with the whole flow plus
adversarial host behavior proven by an API 36 instrumented test.
Relaunch-persistence and the starter-catalog arrival are explicitly gated
(see below) — they are *not* in the non-gated definition of done, unlike
iOS, whose plan owns the FFI those need.

## Landed FFI surface (ground truth, `apps_ffi.rs`)

Everything non-gated builds on exactly this and nothing more:

- `MobileProfile.app_runtime() -> AppRuntimeSession`
- `install_app(manifest_bytes, bundle_bytes) -> InstalledAppRecord`
  (app_id hex, name, description, version, entry_point, permissions) —
  strict canonical CBOR decode + content-derived `app_id`; records the id;
  **does not retain retrievable bundle resources**
- `trust_app(app_id)` / `untrust_app(app_id)` -> `()`,
  `is_app_trusted(app_id) -> bool`
- `app_data_put(app_id, key, value)` -> `()`,
  `app_data_get -> Option<bytes>`, `app_data_list -> Vec<AppDataItem>`

Three consequences, designed around honestly rather than papered over:

1. **No `app_resource`.** That FFI method is owned by the iOS-runtime plan
   (its Task 5, itself gated on directory Task 6). Until it lands, the
   Android host keeps the decoded bundle it was given at install time in
   its own layer: install decodes `bundle_bytes` with a strict Kotlin
   mirror of `apps::bundle::decode_app_bundle` *after* `install_app` has
   accepted the same bytes (Rust remains the integrity/canonicality
   oracle; the Kotlin decode exists only to serve what Rust already
   verified, and any drift fails install loudly). When `app_resource`
   lands, the in-memory store is replaced by FFI lookups.
2. **No persistence returns.** `trust_app` and `app_data_put` return `()`
   today; the replay-persistence design (below) cannot be built yet. The
   non-gated runtime is per-process: state lives as long as the app
   process. Stated in the UI-visible sense: nothing promises persistence
   until the gated task lands.
3. **No listings.** `pending_apps`/`trusted_apps`/`directory_listings` are
   directory/iOS-plan additions. The Android host derives its own list
   from what it installed this session (`InstalledAppRecord`s it holds)
   plus `is_app_trusted` per id.

Hard contract inherited from the platform handoff (COLLABORATION.md,
platform claim, deferred item 3): **trust does not gate
`app_data_put/get/list` in Rust — launch gating is the native host's
job.** The Android host never constructs a WebView for an app unless
`is_app_trusted(app_id)` returned true at open time.

## Android WebView host

`AppWebViewHost` wraps one `android.webkit.WebView` per (app_id, space),
shown full-screen inside the existing single-activity shell (the app is
plain programmatic Views, one `MainActivity`, surfaces swapped into a
content container — the host view swaps in the same way, with a Close
button; no second Activity, no second `MobileProfile`).

**Serving.** Resources are served from the in-memory decoded bundle via
`WebViewClient.shouldInterceptRequest`, on a synthetic per-app origin:

```
https://<app_id_hex[0..32]>.<app_id_hex[32..64]>.riot-app.invalid/<path>
```

Why this and not the alternatives: `file://` loading is disabled outright
(`allowFileAccess = false` — nothing is ever unpacked to disk, same as
iOS); a custom scheme (`riot-app://`) is second-class in Android WebView
(opaque origins, no secure context, flaky `fetch`/module semantics), so
the standard Android approach — the same one `WebViewAssetLoader`
implements — is interception on a synthetic **https** host, which gives a
real secure context and honest same-origin behavior. `WebViewAssetLoader`
itself is not used because it serves from assets/resources/disk paths and
our bundles exist only in memory; we implement the identical pattern with
an exact-match in-memory resolver. The hex id is split into two 32-char
DNS labels (labels cap at 63 chars), giving every app a distinct origin
derived from its content-addressed id, so browser-side state can never
cross apps. The `.invalid` TLD (RFC 2606) is guaranteed unresolvable, and
`settings.blockNetworkLoads = true` blocks WebView network loads outright
— even a missed interception cannot reach the network. Requests for any
other host, and any path that is not an exact bundle-resource match
(exact string match after stripping one leading `/` — no path
interpretation at all, so `../escape` simply matches nothing), get an
empty 404 response. Cleartext is irrelevant (nothing leaves the
interceptor) but blocked anyway by the https-only origin plus
`MIXED_CONTENT_NEVER_ALLOW`.

**Hardening.** JavaScript is enabled only inside `AppWebViewHost` — no
other WebView exists in the app. `domStorageEnabled = false` (bridge data
is the only storage), `allowFileAccess = false`,
`allowContentAccess = false`, `setSupportMultipleWindows(false)`,
`javaScriptCanOpenWindowsAutomatically = false`, no geolocation.
`shouldOverrideUrlLoading` refuses every navigation that is not this
app's own origin.

**CSP.** Every served response carries the same header as iOS:
`Content-Security-Policy: default-src 'none'; script-src 'self';
style-src 'self'; img-src 'self' data:` — defense in depth on top of
`blockNetworkLoads`.

**Bridge.** A `riot-shim.js` script injected at document start via
androidx.webkit `WebViewCompat.addDocumentStartJavaScript` (the mirror of
iOS's `WKUserScript` at-document-start; scoped by allowlist to the app's
own origin) defines the identical `window.riot` API — `get`, `put`,
`list`, `watch`, `whoami` — so the checklist's `app.js` from
`fixtures/apps/checklist/` runs unmodified on both platforms. Underneath,
the shim calls a `@JavascriptInterface` object (`RiotNative`) whose
methods are synchronous string-in/JSON-envelope-out; the shim wraps them
in Promises to keep the API contract. If
`WebViewFeature.DOCUMENT_START_SCRIPT` is unavailable (ancient WebView),
the host **fails closed**: the app does not launch ("This tool can't run
on this phone yet"). The native object validates message shape and size
(262,144-byte budget, mirroring iOS) and forwards to
`AppRuntimeSession.app_data_*`, which enforces prefix scoping and value
caps in Rust — host discipline is not the security boundary, same as the
platform spec. `whoami` returns `"member-"` + first 8 hex chars of the
profile's signing key id, computed host-side from the existing
`identity()` binding (never key material, never the full id; converges
with the gated `app_display_name` FFI when it lands).

## Trust gating and organizer review

Same flow as iOS, in the existing view idiom:

- The Spaces surface gains a **Tools** section once a space exists:
  trusted apps as tappable rows (name, "Open"), installed-but-untrusted
  apps as rows with a "New" marker and a "Review" action.
- Install-from-bytes: a two-step document picker (manifest `.cbor`, then
  bundle `.cbor`) feeding `install_app` — the same picker pattern the
  Import surface already uses. This is the v1 arrival path only because
  the starter catalog is gated; it is not a dev backdoor around trust
  (installing never trusts).
- The review view is the trust-decision moment, plain language only: app
  name, description (the entire trust surface), a "THIS APP CAN" box from
  the manifest permissions, and **"Let everyone in this space use
  this?"**. Approving calls `trust_app`; the row flips to trusted.
- Launch checks `is_app_trusted` at open time, every time. Untrusted or
  revoked → no WebView is ever constructed.

## Live updates (`watch`)

Identical coarse semantics to iOS: no polling. The shim re-runs
`list(prefix)` and invokes watchers when (a) the page's own `put`
succeeds (fired shim-side), (c) the activity resumes
(`webView.evaluateJavascript("window.__riotDataChanged && …")` from
`onResume`). Trigger (b), sync completion, is wired to the existing
`SyncCoordinator` completion callback when app data actually rides sync
on this device pair — app-data sync admission landed in core (`b4abd93`)
but FFI sync review is still alert-only (platform handoff, deferred item
1), so (b) is noted as a follow-up alongside the directory session's
lifting of that restriction.

## Persistence (gated)

The decided approach mirrors the iOS spec exactly: persist the signed
bundle bytes and replay them through the existing
`inspectBytes → createPlan → accept` path on next open — the same
mechanism `RiotController.restore()` already uses for alert bundles, so
Android consumes it with a `PersistedProfile` extension, not a new
mechanism. This **requires `trust_app` and `app_data_put` to return the
committed bundle bytes**, an FFI addition owned by the iOS-runtime plan
(its Task 5) — Android consumes it when landed, and deliberately does
*not* fake persistence in the meantime by re-calling `trust_app`/
`app_data_put` on restore, which would mint fresh signed entries with new
timestamps instead of restoring the originals and diverge across synced
devices. Installed-app records and decoded bundles are likewise
per-process until then.

## Error handling and plain-language UI

Extends the platform table; no "bundle", "signature", "namespace",
"sandbox", or "sync" in anything a person sees:

| Situation | What the person sees |
|---|---|
| Picked files fail `install_app` (corrupt/tampered/mismatched) | "That file isn't a Riot tool" in the status line; nothing installed |
| Installed, not yet trusted | Row under Tools marked "New" with "Review" — never launchable |
| Organizer approves | Row flips to "Open" |
| Untrusted/revoked app opened | Row simply isn't openable / is gone next visit |
| Bridge write fails (storage full) | "Couldn't save that — try again" inline in the app |
| Page attempts network fetch | Fails inside the page (CSP + blocked network); no dialog |
| WebView too old for document-start scripts | "This tool can't run on this phone yet" |
| App closed and process restarted (pre-gated-persistence) | Tools list is empty again — nothing pretends otherwise |

## Buildable today vs gated

**Today, against the landed `AppRuntimeSession` only (no Rust/FFI
changes):** the Kotlin bundle codec + in-memory resource store, the
hardened WebView host + interception + CSP, the `window.riot` bridge, the
Tools/install/review/trust UI, and the end-to-end instrumented test that
packs the committed `fixtures/apps/checklist/` sources into canonical
CBOR in the test, installs, trusts, launches, and round-trips data
through the real WebView.

**Gated (Android consumes, never builds):**

- `app_resource` + listings FFI → iOS-runtime plan Task 5 / directory
  plan Task 6 (then delete the in-memory bundle store).
- Starter-catalog checklist arrival → directory Task 5 + iOS-runtime
  Tasks 2–3 (then the install picker stops being the checklist's path).
- Persistence returns → iOS-runtime plan Task 5 (then the replay
  extension above).
- Committed `checklist.*.cbor` artifacts / `riot-app` CLI → directory
  Task 7 + iOS-runtime Task 2 (then the test-side Kotlin packer is
  retired for the checklist and kept only for adversarial inputs).

## Testing strategy

TDD throughout, matching the existing Android split (JVM unit tests +
API 36 instrumented tests, JDK 17):

- **JVM unit tests:** strict bundle-codec decode (canonical re-encode
  check, trailing bytes, misordered keys, bounds — mirroring
  `apps_codec_hostile.rs` in spirit), exact-match resource resolution
  (traversal-shaped paths resolve to nothing), per-app origin derivation,
  bridge input validation (malformed shape, oversized values, empty keys
  rejected with error envelopes before any FFI call — bridge tested
  against a fake port, no Android runtime).
- **Instrumented (API 36 emulator):** the definition-of-done flow —
  pack the committed checklist sources, install, review, trust, open,
  add/check an item through the real WebView, `app_data_get` sees it;
  an adversarial page proving `fetch` fails and a traversal-shaped key
  is rejected in Rust; and a negative test that an untrusted app never
  gets a WebView.
- **Checklist JS:** the committed iOS fixture is reused byte-for-byte
  (it is a frozen content-hash input); exercised through the real
  WebView, no separate JS runner.
