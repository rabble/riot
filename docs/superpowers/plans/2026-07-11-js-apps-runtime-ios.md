# JS Apps Runtime (iOS) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Everything between the signed-JS-apps core platform and a person on an iOS simulator checking items off a shared checklist inside their space — checklist app fixture, packing, starter catalog, FFI listings/resource-serving, WKWebView host with `window.riot` bridge, Tools UI with organizer approval, XCUITest end-to-end.

**Architecture:** The checklist ships as plain HTML/JS packed into the existing canonical `apps::bundle`/`apps::manifest` CBOR codecs, embedded in `riot-core` via `include_bytes!` as a content-addressed starter catalog (no key material — integrity is canonical decode + re-derived `app_id`). New FFI methods list apps per space and serve bundle resources; iOS hosts them in a WKWebView behind a custom `riot-app://` scheme handler with a strict CSP and a `postMessage` bridge whose security boundary is the Rust `AppDataBridge`.

**Tech Stack:** Rust (riot-core, riot-ffi, xtask), UniFFI, Swift 6 / SwiftUI / WebKit, XCTest + XCUITest.

**Spec:** `docs/superpowers/specs/2026-07-11-js-apps-runtime-ios-design.md` (and its two neighbors: the platform spec `2026-07-11-signed-js-apps-design.md` and `2026-07-11-app-directory-design.md`).

---

## Before you start

1. Run `git status --short` and read `COLLABORATION.md`. This checkout is shared with other active agents. Claim the files of the task you are starting in `COLLABORATION.md` before editing.
2. **Reconciliation (2026-07-11, after the app-directory plan started executing):** the core platform plan is fully landed (all 6 tasks, through `cfc888d`), and the app-directory plan (`docs/superpowers/plans/2026-07-11-app-directory.md`) is executing concurrently in this same checkout. Three of its tasks deliberately interlock with this plan — **do not duplicate them**:
   - Directory **Task 5** creates `crates/riot-core/src/apps/starter.rs` (`verify_starter_catalog`, `STARTER_CATALOG` — empty). This plan's Task 3 *fills* that catalog; it does not create the module.
   - Directory **Task 7** creates the `riot-app` CLI whose `pack` walks an app dir, validates content types, and emits canonical manifest+bundle bytes. This plan's Task 2 *uses* that CLI; the earlier draft's own `pack_app`/xtask packer is superseded (Task 4 below is a tombstone).
   - Directory **Task 6** adds `directory_listings` to `apps_ffi.rs`. This plan's Task 5 re-checks that surface first and adds only what's missing.
3. **Dependency gates:** Task 1 is unblocked now. Task 2 gates on directory Task 7; Task 3 on directory Task 5 + our Task 2; Task 5 on directory Task 6. iOS tasks 6–10 follow Task 5. If a gate hasn't landed, stop and hand off (or wait) rather than editing files the directory session has claimed (`apps/starter.rs`, `apps_ffi.rs`, `mobile_state.rs`, `riot-app-cli/`).
4. iOS tasks (6–10) need the native prerequisites from `apps/ios/README.md`: run `scripts/conference/build-native-core.sh` from the repo root after any FFI change, before building the Xcode project.
5. **Reconciliation №2 (execution-time, "don't wait" directive):** the landed platform FFI (`apps_ffi.rs`: `install_app → InstalledAppRecord`, trust calls, `app_data_*`) plus the Android twin's landed precedent (client-side `AppBundleCodec` decoding bundle bytes only after Rust's `install_app` accepted them — `apps/android/.../apps/AppBundleCodec.kt`) unblock most of this plan without waiting on the directory session:
   - **Task 2** runs now via `crates/riot-core/examples/pack_checklist.rs` (deterministic, key-free, fixed committed public author identity) instead of the CLI. **Decision after the CLI landed (`e938592`):** the generator stays the starter-catalog packer permanently — `riot-app pack` signs with a real key, so its output is not deterministic across repacks (fresh key → new `app_id`), which is wrong for frozen committed artifacts with a drift guard; the CLI is the third-party developer path. Landed as `175b964`, reviewed APPROVED. Canonical checklist `app_id` after the digest-domain reconciliation (`index::app_bundle_digest` now re-exports `bundle::app_bundle_digest`, domain `riot/app-bundle/v1`): `aa9633796890899d5b6f958e8f34db1aaa9d621eedc88f17234cb6b36dd1b910`.
   - **Task 7** is decoupled from the repository layer: new `AppBundleCodec.swift` + `AppResourceResolver.swift` (Swift mirrors of the Android pair), `AppSchemeHandler` backed by a resolver, `AppBridgeController` backed by an `AppDataBridging` protocol whose concrete adapter wraps the generated `AppRuntimeSession` directly. Zero Rust/FFI edits.
   - **Task 6** (repository) shrinks to: install-from-starter-catalog on open, trust persistence (persist trusted app ids + installed pairs in the profile JSON; re-install + re-trust on open — the landed FFI trust state is profile-local in-memory), resolver construction, and listing from remembered `InstalledAppRecord`s.
   - **Task 5** (FFI additions) narrows to app-data replay-persistence returns + display name, and stays gated on the directory session releasing `apps_ffi.rs`/`mobile_state.rs`; item-persistence across relaunch (part of Task 10's definition of done) depends on it.

## File Structure

Rust:
- `fixtures/apps/checklist/` — Task 1: `index.html`, `app.js`, `style.css`, `riot-app.json` (checklist app source, no build step; `riot-app.json` uses the `riot-app` CLI's schema — no author field, identity comes from the pack-time key)
- `fixtures/apps/checklist.manifest.cbor`, `fixtures/apps/checklist.bundle.cbor` — Task 2: artifacts packed once via `riot-app pack` with an ephemeral key (never committed), then frozen
- `scripts/apps/repack-starter.sh` — Task 2: documented repack procedure
- `crates/riot-core/src/apps/starter.rs` — Task 3: fill `STARTER_CATALOG` with `include_bytes!` pairs (module and `verify_starter_catalog` come from directory Task 5)
- `crates/riot-core/tests/apps_starter.rs` — Task 3: add drift-guard tests to the directory plan's file
- `crates/riot-ffi/src/apps_ffi.rs` + `mobile_state.rs` — Task 5: `app_resource`, `app_display_name`, replay-persistence returns, plus whatever listing gap remains after directory Task 6's `directory_listings`

iOS (all new reusable code in the **RiotKit** static-lib target; screens in the app target):
- `apps/ios/Riot/Core/ProfileRepository.swift` — Task 6: app methods + replay persistence
- `apps/ios/Riot/Apps/RiotJS.swift` — Task 7: injected `window.riot` user script
- `apps/ios/Riot/Apps/AppSchemeHandler.swift` — Task 7: `riot-app://` WKURLSchemeHandler + CSP
- `apps/ios/Riot/Apps/AppBridgeController.swift` — Task 7: WKScriptMessageHandler → repository
- `apps/ios/Riot/Apps/AppRuntimeView.swift` — Task 8: UIViewRepresentable WKWebView host
- `apps/ios/Riot/AppModel.swift` + `apps/ios/Riot/ConferenceShellView.swift` — Task 9: Tools section, review sheet, full-screen cover
- `apps/ios/RiotTests/AppRuntimeHostTests.swift` — Tasks 7–8 unit tests
- `apps/ios/RiotUITests/ChecklistFlowUITests.swift` — Task 10: end-to-end XCUITest

**Xcode project note (applies to every iOS task):** `Riot.xcodeproj/project.pbxproj` is hand-managed classic format — no file-system-synchronized groups. Adding a file means four edits: a `PBXFileReference`, a `PBXBuildFile`, appending the build-file ID to the right `PBXSourcesBuildPhase` (RiotKit sources phase for `Riot/Apps/*` and `Core/*`; app target phase for `ConferenceShellView.swift`-adjacent code; `B…` UI-test phase for UI tests), and appending the file-ref ID to the owning `PBXGroup` children. Follow the existing sequential hex UUID convention (`A00000000000000000000xxx` app-side, `B00000000000000000000xxx` UI-test side) — grep for the current highest and continue the sequence. WebKit needs no new link entry (`import WebKit` is enough).

---

### Task 1: Checklist app source files

**Files:**
- Create: `fixtures/apps/checklist/index.html`
- Create: `fixtures/apps/checklist/style.css`
- Create: `fixtures/apps/checklist/app.js`
- Create: `fixtures/apps/checklist/riot-app.json`

These are static fixtures — no test of their own here; Task 2's pack tests and Task 7's host tests exercise them. Keep every file exactly as written (they are inputs to a frozen content hash from Task 3 onward).

- [ ] **Step 1: Write `index.html`**

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Checklist</title>
<link rel="stylesheet" href="style.css">
</head>
<body>
<main>
  <h1>Checklist</h1>
  <p id="error" role="alert" hidden></p>
  <form id="add-form">
    <input id="new-item" type="text" aria-label="New item" placeholder="Add something to do" autocomplete="off">
    <button id="add" type="submit">Add</button>
  </form>
  <ul id="items" aria-label="Checklist items"></ul>
  <p id="empty">Nothing here yet. Add the first item.</p>
</main>
<script src="app.js"></script>
</body>
</html>
```

- [ ] **Step 2: Write `style.css`**

```css
:root { color-scheme: light dark; }
body { font-family: -apple-system, sans-serif; margin: 0; padding: 16px; }
main { max-width: 480px; margin: 0 auto; }
h1 { font-size: 1.4rem; text-transform: uppercase; letter-spacing: 0.05em; }
#add-form { display: flex; gap: 8px; margin-bottom: 16px; }
#new-item { flex: 1; padding: 10px; border: 2px solid currentColor; font-size: 1rem; }
#add { padding: 10px 16px; border: 2px solid currentColor; background: none; font-size: 1rem; }
#items { list-style: none; padding: 0; margin: 0; }
#items li { display: flex; align-items: center; gap: 10px; padding: 10px 0; border-bottom: 1px solid currentColor; }
#items li.done label { text-decoration: line-through; opacity: 0.6; }
#items input[type="checkbox"] { width: 22px; height: 22px; }
#items .meta { margin-left: auto; font-size: 0.75rem; opacity: 0.6; }
#error { border: 2px solid currentColor; padding: 8px; }
#empty { opacity: 0.6; }
```

- [ ] **Step 3: Write `app.js`**

Built entirely on `window.riot` (injected by the native host — Task 7). Data model per the platform spec: `items/<uuid>` → `{ text, done, updated_by, updated_at }`.

```js
"use strict";

const list = document.getElementById("items");
const empty = document.getElementById("empty");
const form = document.getElementById("add-form");
const input = document.getElementById("new-item");
const error = document.getElementById("error");

let me = { displayName: "" };
riot.whoami().then((who) => { me = who; }).catch(() => {});

function newID() {
  if (crypto.randomUUID) { return crypto.randomUUID(); }
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
}

function showError(message) {
  error.textContent = message;
  error.hidden = false;
}

function stamp() {
  return { updated_by: me.displayName, updated_at: Date.now() };
}

function render(rows) {
  error.hidden = true;
  rows.sort((a, b) => (a.value.updated_at || 0) - (b.value.updated_at || 0));
  empty.hidden = rows.length > 0;
  list.replaceChildren(...rows.map((row) => {
    const li = document.createElement("li");
    if (row.value.done) { li.className = "done"; }
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = Boolean(row.value.done);
    box.setAttribute("aria-label", row.value.text);
    box.addEventListener("change", () => {
      riot.put(row.key, { ...row.value, done: box.checked, ...stamp() })
        .catch(() => { box.checked = !box.checked; showError("Couldn't save that — try again"); });
    });
    const label = document.createElement("label");
    label.textContent = row.value.text;
    box.id = "box-" + row.key.replaceAll("/", "-");
    label.htmlFor = box.id;
    const meta = document.createElement("span");
    meta.className = "meta";
    meta.textContent = row.value.updated_by || "";
    li.append(box, label, meta);
    return li;
  }));
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = input.value.trim();
  if (!text) { return; }
  input.value = "";
  riot.put("items/" + newID(), { text, done: false, ...stamp() })
    .catch(() => { input.value = text; showError("Couldn't save that — try again"); });
});

riot.watch("items", render);
```

- [ ] **Step 4: Write `riot-app.json`**

The pack-time manifest source, in the `riot-app` CLI's exact schema (directory plan Task 7): no author field — the author identity comes from the pack-time key, never from the JSON. Note the permission string must stay ≤ 64 bytes (`MAX_APP_PERMISSION_BYTES`); the plain-language "nothing else" framing lives in the description, which allows 500.

```json
{
  "name": "Checklist",
  "description": "A shared checklist for this space. Anyone here can add items and check them off. It keeps its notes in this space and nothing else — no internet, no photos.",
  "version": "1.0.0",
  "entry_point": "index.html",
  "permissions": [
    "Keep its own notes in this space"
  ]
}
```

- [ ] **Step 5: Commit**

```bash
git add fixtures/apps/checklist/
git commit -m "feat(apps): add checklist app source fixture"
```

---

### Task 2: Pack and commit the starter artifacts (gated on directory Task 7)

**Files:**
- Create: `scripts/apps/repack-starter.sh`
- Create (generated, then committed frozen): `fixtures/apps/checklist.manifest.cbor`, `fixtures/apps/checklist.bundle.cbor`

**Gate:** the `riot-app` CLI (directory plan Task 7) must be landed — check for `crates/riot-app-cli/` with a working `pack`. Re-read its actual command-line surface first (`riot-app pack` argument names below are from the directory plan and may have shifted in landing).

**Key handling:** pack with an **ephemeral** author key — generate, pack, discard. No key material is committed (spec correction `afae443`): the committed manifest carries only the derived *public* identity, integrity is content-addressing, and a repack under a fresh key changes the `app_id`, which is correct behavior (new bytes = new trust decision). The drift guard (Task 3) re-derives the *bundle* bytes (key-free) and compares manifest fields to `riot-app.json`, so it never needs the key.

- [ ] **Step 1: Write `scripts/apps/repack-starter.sh`**

```bash
#!/bin/sh
# Repack the starter checklist app into the committed catalog artifacts.
# Run from the repo root after editing fixtures/apps/checklist/.
#
# Uses a fresh ephemeral key each time (nothing is committed but the two
# artifact files): repacking therefore changes the app_id, and every space
# organizer re-approves the new version — that is the trust model working,
# not a bug. See docs/superpowers/specs/2026-07-11-js-apps-runtime-ios-design.md.
set -eu

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

cargo run -p riot-app-cli --bin riot-app -- keygen --out-dir "$workdir"
cargo run -p riot-app-cli --bin riot-app -- pack fixtures/apps/checklist \
  --key-dir "$workdir" \
  --out-manifest fixtures/apps/checklist.manifest.cbor \
  --out-bundle fixtures/apps/checklist.bundle.cbor

echo "Packed. Commit the two fixtures/apps/checklist.*.cbor files."
```

Adjust the flag names to the CLI's real surface (read `crates/riot-app-cli/src/main.rs` — the directory plan sketches `keygen`/`pack`/`inspect` but the landed argument spelling wins). If the landed `pack` emits only a combined import bundle and not the two raw artifact files, add `--out-manifest`/`--out-bundle` style outputs to the CLI as a small additive change — coordinate via `COLLABORATION.md` since `riot-app-cli/` is the directory session's file (it may already be released by then; check the claim table).

`chmod +x scripts/apps/repack-starter.sh`.

- [ ] **Step 2: Run it and sanity-check the artifacts**

Run: `sh scripts/apps/repack-starter.sh`
Expected: both `.cbor` files exist and are non-empty. Sanity-check: `cargo run -p riot-app-cli --bin riot-app -- inspect fixtures/apps/checklist.manifest.cbor` (or whatever the landed inspect takes) shows name "Checklist".

- [ ] **Step 3: Commit (artifacts are frozen inputs from here on)**

```bash
git add scripts/apps/repack-starter.sh fixtures/apps/checklist.manifest.cbor fixtures/apps/checklist.bundle.cbor
git commit -m "feat(apps): pack starter checklist into committed catalog artifacts"
```

---

### Task 3: Fill the starter catalog + drift guard (gated on directory Task 5 and our Task 2)

**Files:**
- Modify: `crates/riot-core/src/apps/starter.rs` (created by directory Task 5 — `verify_starter_catalog` + empty `STARTER_CATALOG`)
- Modify: `crates/riot-core/tests/apps_starter.rs` (same task's test file — add tests, keep theirs)

**Gate:** directory Task 5 landed (module exists) and Task 2 above committed the artifacts. `apps/starter.rs` belongs to the directory session's claim until their plan's row reads Done/released for that task — check `COLLABORATION.md` and coordinate there before editing; this change is the one their Task 5 doc comment explicitly expects ("the checklist app's pair arrives with that follow-up").

- [ ] **Step 1: Write the failing tests (added to the existing `apps_starter.rs`)**

```rust
#[test]
fn shipped_catalog_contains_the_checklist() {
    let apps = riot_core::apps::starter::verify_starter_catalog(
        riot_core::apps::starter::STARTER_CATALOG,
    );
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].manifest.name, "Checklist");
    assert_eq!(apps[0].manifest.entry_point, "index.html");
}

/// Drift guard, key-free: the committed bundle artifact must equal a fresh
/// canonical encode of the committed source files, and the committed
/// manifest's fields must equal riot-app.json. Editing
/// fixtures/apps/checklist/* without re-running
/// scripts/apps/repack-starter.sh fails here.
#[test]
fn committed_artifacts_match_the_committed_source() {
    use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
    use riot_core::apps::manifest::decode_manifest;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/apps");
    let dir = root.join("checklist");

    // Bundle: pure function of the source files, no key involved.
    let mut resources = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read dir") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "riot-app.json" {
            continue;
        }
        let content_type = match name.rsplit_once('.').map(|(_, e)| e) {
            Some("html") => "text/html",
            Some("js") => "text/javascript",
            Some("css") => "text/css",
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            other => panic!("unsupported starter resource type: {name} ({other:?})"),
        };
        resources.push(AppResource {
            path: name,
            content_type: content_type.to_string(),
            bytes: std::fs::read(entry.path()).expect("read resource"),
        });
    }
    resources.sort_by(|a, b| a.path.cmp(&b.path));

    let source: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("riot-app.json")).expect("read riot-app.json"),
    )
    .expect("parse riot-app.json");

    let rebuilt = encode_app_bundle(&AppBundle {
        entry_point: source["entry_point"].as_str().unwrap().to_string(),
        resources,
    })
    .expect("re-encode bundle");
    let committed_bundle = std::fs::read(root.join("checklist.bundle.cbor")).expect("artifact");
    assert_eq!(rebuilt, committed_bundle, "bundle drift — re-run scripts/apps/repack-starter.sh");

    // Manifest: field-for-field against riot-app.json (author is pack-time
    // ephemeral and deliberately not re-derivable — not compared).
    let committed_manifest = std::fs::read(root.join("checklist.manifest.cbor")).expect("artifact");
    let manifest = decode_manifest(&committed_manifest).expect("decode manifest");
    assert_eq!(manifest.name, source["name"].as_str().unwrap());
    assert_eq!(manifest.description, source["description"].as_str().unwrap());
    assert_eq!(manifest.version, source["version"].as_str().unwrap());
    assert_eq!(manifest.entry_point, source["entry_point"].as_str().unwrap());
    let permissions: Vec<&str> = source["permissions"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
    assert_eq!(manifest.permissions, permissions);
}
```

Match the landed `verify_starter_catalog` return shape (the directory plan's `IndexedApp` exposes `manifest`; adjust field access if the landed struct differs). The resource ordering + content-type mapping in the drift test must mirror the landed CLI `pack` behavior — read `riot-app-cli`'s pack implementation and copy its exact ordering rule (the directory plan sorts by path; verify).

If `serde_json` is not already a riot-core dev-dependency, add it under `[dev-dependencies]` (already a workspace pin).

Run: `cargo test -p riot-core --test apps_starter` — the two new tests fail (empty catalog / missing artifacts read).

- [ ] **Step 2: Fill `STARTER_CATALOG`**

In `crates/riot-core/src/apps/starter.rs`, replace the empty slice (and stale "empty until the WebView runtime lands" comment) with:

```rust
const CHECKLIST_MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/checklist.manifest.cbor"
));
const CHECKLIST_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/apps/checklist.bundle.cbor"
));

/// (manifest_bytes, bundle_bytes) pairs embedded at compile time.
pub const STARTER_CATALOG: &[(&[u8], &[u8])] = &[(CHECKLIST_MANIFEST, CHECKLIST_BUNDLE)];
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_starter`
Expected: all pass — the directory plan's original three (including `the_shipped_catalog_verifies_completely`, which now verifies a 1-entry catalog) plus the two new ones.

- [ ] **Step 4: Full check and commit**

Run: `cargo test -p riot-core --all-features`, `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`, `cargo xtask validate-contracts`
Expected: green/clean/PASS.

```bash
git add crates/riot-core/src/apps/starter.rs crates/riot-core/tests/apps_starter.rs crates/riot-core/Cargo.toml
git commit -m "feat(apps): embed checklist in the starter catalog with drift guard"
```

---

### Task 4: (superseded)

The earlier draft's `pack_app`-in-riot-core + `cargo xtask pack-starter-apps` tasks are superseded by the app-directory plan's `riot-app` CLI (its Task 7) — packing logic lives in one place. See Tasks 2–3 above; nothing to do here.

---

### Task 5: FFI — app listings, resource serving, display name

**Files:**
- Modify: `crates/riot-ffi/src/apps_ffi.rs` (created by core plan Task 6 — see gate below)
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Test: `crates/riot-ffi/tests/mobile_contract.rs` (or the apps contract test file Task 6 created — match it)

**Gate:** the core platform plan is fully landed (all 6 tasks); additionally wait for directory plan Task 6 (`directory_listings`/`share_app`/`endorse_app` in `apps_ffi.rs`) so this task doesn't collide with the directory session's claim on the same files — check `COLLABORATION.md`. **Step 1 is mandatory re-reading**: the landed surface is authoritative over every snippet below. In particular, if the landed `directory_listings` already returns name/description/permissions/per-space trust state (it is designed to), **do not add a parallel `list_space_apps`** — reuse it from iOS and add only what's genuinely missing (`app_resource`, `app_display_name`, replay-persistence byte returns). Adapt the test/impl snippets below accordingly; never create a parallel surface.

- [ ] **Step 1: Re-read the landed app FFI surface**

Read `crates/riot-ffi/src/apps_ffi.rs`, `mobile_api.rs`, `mobile_state.rs` in full. Record: the object type wrapping app calls (plan named it `AppRuntimeSession`), how `trust_app`/`untrust_app`/`is_app_trusted` persist markers, how `app_data_put` returns (and whether it exposes the committed bundle bytes), the `AppsError → MobileError` mapping, and the hex-encoding helper `mobile_state.rs` already uses for entry ids. Two things this task **must** guarantee, adding them if Task 6 didn't:

1. **Replay persistence:** iOS persistence is "save the signed bundle bytes, replay through `inspect → plan → accept` on next open" (`ProfileRepository.open`). For checklist items and trust decisions to survive relaunch, `app_data_put` and `trust_app` must each return the committed bundle bytes to the caller (same shape as `sign_draft`'s `SignedAlert.bundle_bytes`). If Task 6's methods return `()`, extend them additively.
2. **Trust markers must be readable back** for `list_space_apps` below (via `is_app_trusted` or a marker listing).

- [ ] **Step 2: Write failing contract tests**

In the FFI contract-test file (match the existing in-process harness — `open_local_profile()` + `create_public_space`, no fixtures):

```rust
#[test]
fn starter_checklist_is_listed_untrusted_then_flips_after_trust() {
    let profile = open_local_profile().expect("profile");
    profile.create_public_space("Berlin Mutual Aid".into()).expect("space");

    let listings = profile.list_space_apps().expect("list");
    assert_eq!(listings.len(), 1);
    let checklist = &listings[0];
    assert_eq!(checklist.name, "Checklist");
    assert!(!checklist.description.is_empty());
    assert!(!checklist.permissions.is_empty());
    assert!(!checklist.trusted);
    assert_eq!(checklist.app_id_hex.len(), 64);

    let receipt = profile.trust_app(checklist.app_id_hex.clone()).expect("trust");
    assert!(!receipt.bundle_bytes.is_empty(), "trust must return replayable bundle bytes");

    let after = profile.list_space_apps().expect("list");
    assert!(after[0].trusted);
}

#[test]
fn app_resource_serves_bundle_files_and_rejects_everything_else() {
    let profile = open_local_profile().expect("profile");
    profile.create_public_space("Berlin Mutual Aid".into()).expect("space");
    let app_id = profile.list_space_apps().expect("list")[0].app_id_hex.clone();

    let index = profile.app_resource(app_id.clone(), "index.html".into()).expect("index");
    assert_eq!(index.content_type, "text/html");
    assert!(!index.bytes.is_empty());

    assert!(profile.app_resource(app_id.clone(), "../escape".into()).is_err());
    assert!(profile.app_resource(app_id.clone(), "missing.js".into()).is_err());
    assert!(profile.app_resource("00".repeat(32), "index.html".into()).is_err());
}

#[test]
fn app_display_name_is_stable_and_never_a_raw_key() {
    let profile = open_local_profile().expect("profile");
    let name = profile.app_display_name().expect("name");
    assert!(!name.is_empty());
    assert!(name.len() < 24, "display name must be short, not key material: {name}");
    assert_eq!(name, profile.app_display_name().expect("name"), "stable across calls");
}
```

Run: `cargo test -p riot-ffi` — expected: compile failure on the new methods.

- [ ] **Step 3: Implement in `mobile_state.rs` + `apps_ffi.rs`**

New records (in `apps_ffi.rs`, following the `uniffi::Record` convention):

```rust
#[derive(Debug, Clone, uniffi::Record)]
pub struct AppListing {
    pub app_id_hex: String,
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub trusted: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppResourceBytes {
    pub content_type: String,
    pub bytes: Vec<u8>,
}
```

`mobile_state.rs` free functions, each a `with_active` delegator (adapt to the profile's actual space/trust state fields from Step 1):

- `list_space_apps(inner)` — requires a joined space (else `MobileError::InvalidInput`); maps `riot_core::apps::starter::starter_apps()` to `AppListing`s, computing `trusted` through the landed trust path (the organizer set for a locally-created space is the profile's own subspace id, per the core plan's known-organizer model). Hex-encode `app_id` with the existing helper.
- `app_resource(inner, app_id_hex, path)` — decode hex (64 chars → `[u8; 32]`, else `InvalidInput`); find the starter app by id (else `InvalidInput`); find the resource by exact `path` match against `bundle.resources` (else `InvalidInput`). Exact-match lookup is the traversal defense — no path interpretation happens at all; `"../escape"` simply matches no resource. Return `AppResourceBytes { content_type, bytes }`.
- `app_display_name(inner)` — `"member-"` + first 8 hex chars of the profile identity's subspace id (v1 placeholder until profiles carry names; never the full key).

Expose each as a thin `#[uniffi::export]` method on `MobileProfile` (or on the Task 6 app object if that's where app methods live — match the landed convention).

- [ ] **Step 4: Tests, bindings, leak check, commit**

Run: `cargo test -p riot-ffi --all-features` — all green.
Run: `cargo xtask generate-bindings` — non-empty Swift/Kotlin, no errors.
Run: `cargo clippy -p riot-ffi --all-features --all-targets -- -D warnings` — clean.
Check the existing `include_str!` surface-leak tests still pass (no forbidden core type names in the FFI declarations — new types must be plain records).

```bash
git add crates/riot-ffi/
git commit -m "feat(ffi): expose app listings, resource serving, display name"
```

---

### Task 6: iOS repository layer

**Files:**
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Test: `apps/ios/RiotTests/AppRepositoryTests.swift` (new — add to RiotTests target in pbxproj)

Run `scripts/conference/build-native-core.sh` first so the regenerated `riot_ffi.swift` (with Task 5's methods) is in `build/generated/riot-ffi/`.

- [ ] **Step 1: Write the failing tests**

Follow `BindingSemanticsTests.swift`'s pattern exactly: real FFI, `TestWrappingKeyStore` (fixed 32-byte key), `FileManager.default.temporaryDirectory` storage.

```swift
import XCTest
@testable import RiotKit

final class AppRepositoryTests: XCTestCase {
    private func makeRepository() throws -> RiotProfileRepository {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-repo-\(UUID().uuidString).json")
        let storage = try ProtectedProfileStorage(fileURL: url)
        return try RiotProfileRepository.open(storage: storage, keyStore: TestWrappingKeyStore())
    }

    func testStarterChecklistListsPendingThenTrusts() throws {
        let repository = try makeRepository()
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")

        let before = try repository.spaceApps()
        XCTAssertEqual(before.count, 1)
        XCTAssertEqual(before[0].name, "Checklist")
        XCTAssertFalse(before[0].trusted)

        try repository.trustApp(appID: before[0].appIDHex)
        let after = try repository.spaceApps()
        XCTAssertTrue(after[0].trusted)
    }

    func testAppDataAndTrustSurviveReopen() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-repo-reopen-\(UUID().uuidString).json")
        let storage = try ProtectedProfileStorage(fileURL: url)
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(storage: storage, keyStore: keyStore)
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try first.spaceApps()[0].appIDHex
        try first.trustApp(appID: appID)
        try first.appDataPut(appID: appID, key: "items/one", valueJSON: #"{"text":"water","done":false}"#)

        let second = try RiotProfileRepository.open(storage: storage, keyStore: keyStore)
        XCTAssertTrue(try second.spaceApps()[0].trusted)
        XCTAssertEqual(
            try second.appDataGet(appID: appID, key: "items/one"),
            #"{"text":"water","done":false}"#
        )
    }

    func testAppResourceServesEntryPointAndRefusesEscapes() throws {
        let repository = try makeRepository()
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex

        let index = try repository.appResource(appID: appID, path: "index.html")
        XCTAssertEqual(index.contentType, "text/html")
        XCTAssertFalse(index.bytes.isEmpty)
        XCTAssertThrowsError(try repository.appResource(appID: appID, path: "../escape"))
    }
}
```

(`TestWrappingKeyStore` is currently `private` in `BindingSemanticsTests.swift` — duplicate the 10-line helper into this file, matching that convention rather than widening its access.)

Add the file to the RiotTests sources phase in `project.pbxproj` (see the Xcode project note at the top). Run the RiotKit test scheme — expected: compile failure on the new repository methods.

- [ ] **Step 2: Implement the repository additions**

In `ProfileRepository.swift`:

```swift
public struct RiotSpaceApp: Equatable, Sendable {
    public let appIDHex: String
    public let name: String
    public let description: String
    public let permissions: [String]
    public let trusted: Bool
}

public struct RiotAppResource: Equatable, Sendable {
    public let contentType: String
    public let bytes: Data
}
```

Methods on `RiotProfileRepository`, each mapping straight onto the generated FFI methods and the existing persisted-replay pattern:

- `spaceApps() throws -> [RiotSpaceApp]` — `profile.listSpaceApps()` mapped to DTOs.
- `trustApp(appID: String) throws` — calls the FFI trust method; **appends the returned bundle bytes to the persisted replay list** (same list the alert bundles use, or a sibling array field if the persisted struct separates them — extend `PersistedProfile` accordingly) and saves.
- `appDataPut(appID: String, key: String, valueJSON: String) throws` — FFI put; append returned bundle bytes to the replay list; save.
- `appDataGet(appID: String, key: String) throws -> String?` and `appDataList(appID: String, prefix: String) throws -> [(key: String, valueJSON: String)]` — thin FFI pass-throughs (bytes ↔ UTF-8 strings at this boundary; the bridge stores JSON text).
- `appResource(appID: String, path: String) throws -> RiotAppResource`.
- `appDisplayName() throws -> String`.

In `static open`, replay the new persisted bundle list through the same `inspectBytes → eligibleEntries → createPlan.accept` path the alert bundles already use (app-data admission landed in core — commit `b4abd93`). Keep replay order: space join first, then all bundles oldest-first.

- [ ] **Step 3: Run the unit tests**

Run (from repo root; both commands verbatim from `apps/ios/README.md`):

```sh
scripts/conference/build-native-core.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-derived
```
Expected: all green including the 3 new tests.

- [ ] **Step 4: Commit**

```bash
git add apps/ios/Riot/Core/ProfileRepository.swift apps/ios/RiotTests/AppRepositoryTests.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): repository surface for space apps, trust, app data, resources"
```

---

### Task 7: iOS WebView plumbing — scheme handler, riot.js, bridge

**Files:**
- Create: `apps/ios/Riot/Apps/RiotJS.swift`
- Create: `apps/ios/Riot/Apps/AppSchemeHandler.swift`
- Create: `apps/ios/Riot/Apps/AppBridgeController.swift`
- Test: `apps/ios/RiotTests/AppRuntimeHostTests.swift`

All three files go in the **RiotKit** target (new `Apps` group under the `Riot` group in pbxproj). This task is where the spec's security tests live: exact CSP on every response, out-of-bundle refusal, malformed/oversized bridge messages rejected, and an adversarial page proving `fetch` and out-of-scope keys fail.

- [ ] **Step 1: Write the failing tests**

```swift
import WebKit
import XCTest
@testable import RiotKit

@MainActor
final class AppRuntimeHostTests: XCTestCase {
    private func makeRepositoryWithTrustedChecklist() throws -> (RiotProfileRepository, String) {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-host-\(UUID().uuidString).json")
        let storage = try ProtectedProfileStorage(fileURL: url)
        let repository = try RiotProfileRepository.open(storage: storage, keyStore: TestWrappingKeyStore())
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex
        try repository.trustApp(appID: appID)
        return (repository, appID)
    }

    private func makeWebView(repository: RiotProfileRepository, appID: String) -> (WKWebView, AppBridgeController) {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        let bridge = AppBridgeController(repository: repository, appIDHex: appID)
        configuration.userContentController.addUserScript(
            WKUserScript(source: RiotJS.source, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        )
        configuration.userContentController.add(bridge, name: "riot")
        configuration.setURLSchemeHandler(AppSchemeHandler(repository: repository), forURLScheme: AppSchemeHandler.scheme)
        let webView = WKWebView(frame: .zero, configuration: configuration)
        bridge.webView = webView
        return (webView, bridge)
    }

    private func loadEntryPoint(_ webView: WKWebView, appID: String) {
        let url = URL(string: "\(AppSchemeHandler.scheme)://\(appID)/index.html")!
        webView.load(URLRequest(url: url))
    }

    private func waitForJS(_ webView: WKWebView, _ script: String, timeout: TimeInterval = 10) throws -> Any? {
        var result: Any?
        let expectation = expectation(description: "js")
        // Poll because page load + bridge round-trips are async.
        Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { timer in
            webView.evaluateJavaScript(script) { value, _ in
                if let value, !(value is NSNull) {
                    result = value
                    timer.invalidate()
                    expectation.fulfill()
                }
            }
        }
        wait(for: [expectation], timeout: timeout)
        return result
    }

    func testSchemeHandlerServesEntryPointWithStrictCSP() throws {
        let (repository, appID) = try makeRepositoryWithTrustedChecklist()
        let handler = AppSchemeHandler(repository: repository)
        let response = try handler.response(for: URL(string: "riot-app://\(appID)/index.html")!)
        XCTAssertEqual(response.response.statusCode, 200)
        XCTAssertEqual(
            response.response.value(forHTTPHeaderField: "Content-Security-Policy"),
            AppSchemeHandler.csp
        )
        XCTAssertEqual(response.response.value(forHTTPHeaderField: "Content-Type"), "text/html")
        XCTAssertFalse(response.bytes.isEmpty)
    }

    func testSchemeHandlerRefusesUnknownPathsAndForeignApps() throws {
        let (repository, appID) = try makeRepositoryWithTrustedChecklist()
        let handler = AppSchemeHandler(repository: repository)
        XCTAssertThrowsError(try handler.response(for: URL(string: "riot-app://\(appID)/../escape")!))
        XCTAssertThrowsError(try handler.response(for: URL(string: "riot-app://\(appID)/missing.js")!))
        let foreign = String(repeating: "0", count: 64)
        XCTAssertThrowsError(try handler.response(for: URL(string: "riot-app://\(foreign)/index.html")!))
    }

    func testChecklistPageBootsAndRoundTripsAnItemThroughTheBridge() throws {
        let (repository, appID) = try makeRepositoryWithTrustedChecklist()
        let (webView, _) = makeWebView(repository: repository, appID: appID)
        loadEntryPoint(webView, appID: appID)

        _ = try waitForJS(webView, "window.riot ? 'ready' : null")
        _ = try waitForJS(webView, """
            window.riot.put('items/test-item', {text: 'water', done: false, updated_by: '', updated_at: 1})
              .then(() => 'stored')
        """)
        let stored = try repository.appDataGet(appID: appID, key: "items/test-item")
        XCTAssertNotNil(stored)
        XCTAssertTrue(stored!.contains("water"))
    }

    func testHostileFetchAndOutOfScopeKeysFail() throws {
        let (repository, appID) = try makeRepositoryWithTrustedChecklist()
        let (webView, _) = makeWebView(repository: repository, appID: appID)
        loadEntryPoint(webView, appID: appID)
        _ = try waitForJS(webView, "window.riot ? 'ready' : null")

        // CSP: network fetch must be blocked inside the page.
        let fetchResult = try waitForJS(webView, """
            fetch('https://example.com').then(() => 'FETCHED').catch(() => 'blocked')
        """)
        XCTAssertEqual(fetchResult as? String, "blocked")

        // Rust-side scoping: a traversal-shaped key must reject, not write.
        let putResult = try waitForJS(webView, """
            window.riot.put('../escape', {x: 1}).then(() => 'WROTE').catch(() => 'rejected')
        """)
        XCTAssertEqual(putResult as? String, "rejected")
    }

    func testBridgeRejectsMalformedAndOversizedMessages() throws {
        let (repository, appID) = try makeRepositoryWithTrustedChecklist()
        let bridge = AppBridgeController(repository: repository, appIDHex: appID)
        XCTAssertFalse(bridge.handleForTesting(body: "not a dictionary"))
        XCTAssertFalse(bridge.handleForTesting(body: ["op": "get"])) // missing id
        XCTAssertFalse(bridge.handleForTesting(body: [
            "id": 1, "op": "put", "key": "items/x",
            "value": String(repeating: "a", count: 300_000),
        ])) // oversized
    }
}
```

(Reuse the same private `TestWrappingKeyStore` helper pattern as Task 6.) Add to RiotTests sources phase. Run the RiotKit scheme — expected: compile failures for the three new types.

- [ ] **Step 2: Implement `RiotJS.swift`**

```swift
public enum RiotJS {
    /// Injected at document start. Defines window.riot over the
    /// webkit message handler with promise-correlation ids. The host
    /// resolves calls via window.__riotResolve and pushes change events
    /// via window.__riotDataChanged.
    public static let source = """
    (function () {
      const pending = new Map();
      let nextId = 1;
      const watchers = [];
      function call(op, params) {
        return new Promise((resolve, reject) => {
          const id = nextId++;
          pending.set(id, { resolve, reject });
          window.webkit.messageHandlers.riot.postMessage(Object.assign({ id, op }, params));
        });
      }
      window.__riotResolve = function (id, ok, payload) {
        const entry = pending.get(id);
        if (!entry) { return; }
        pending.delete(id);
        if (ok) { entry.resolve(payload); } else { entry.reject(new Error(String(payload))); }
      };
      window.__riotDataChanged = function () {
        for (const watcher of watchers) {
          window.riot.list(watcher.prefix).then(watcher.cb).catch(function () {});
        }
      };
      window.riot = {
        get: function (key) {
          return call("get", { key: key }).then(function (v) { return v == null ? null : JSON.parse(v); });
        },
        put: function (key, value) {
          return call("put", { key: key, value: JSON.stringify(value) }).then(function () { return undefined; });
        },
        list: function (prefix) {
          // Prefixes are segment-based; a trailing "/" would produce an
          // empty segment the core rejects, so normalize it away here.
          var clean = prefix.replace(/\\/+$/, "");
          return call("list", { prefix: clean }).then(function (rows) {
            return rows.map(function (r) { return { key: r.key, value: JSON.parse(r.value) }; });
          });
        },
        watch: function (prefix, cb) {
          watchers.push({ prefix: prefix, cb: cb });
          window.riot.list(prefix).then(cb).catch(function () {});
        },
        whoami: function () { return call("whoami", {}); },
      };
    })();
    """
}
```

- [ ] **Step 3: Implement `AppSchemeHandler.swift`**

The testable core is a synchronous `response(for:)`; the WKURLSchemeHandler conformance is a thin wrapper. Resource lookup is an exact string match against the verified bundle in Rust (Task 5) — no path interpretation in Swift.

```swift
import WebKit

public final class AppSchemeHandler: NSObject, WKURLSchemeHandler {
    public static let scheme = "riot-app"
    public static let csp =
        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:"

    public struct Response {
        public let response: HTTPURLResponse
        public let bytes: Data
    }

    public enum SchemeError: Error { case badURL, notFound }

    private let repository: RiotProfileRepository

    public init(repository: RiotProfileRepository) {
        self.repository = repository
    }

    public func response(for url: URL) throws -> Response {
        guard url.scheme == Self.scheme, let appID = url.host else {
            throw SchemeError.badURL
        }
        let path = String(url.path.drop(while: { $0 == "/" }))
        guard !path.isEmpty else { throw SchemeError.badURL }
        let resource: RiotAppResource
        do {
            resource = try repository.appResource(appID: appID, path: path)
        } catch {
            throw SchemeError.notFound
        }
        guard let httpResponse = HTTPURLResponse(
            url: url,
            statusCode: 200,
            httpVersion: "HTTP/1.1",
            headerFields: [
                "Content-Type": resource.contentType,
                "Content-Security-Policy": Self.csp,
                "Content-Length": String(resource.bytes.count),
            ]
        ) else { throw SchemeError.badURL }
        return Response(response: httpResponse, bytes: resource.bytes)
    }

    public func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url else {
            urlSchemeTask.didFailWithError(SchemeError.badURL)
            return
        }
        do {
            let served = try response(for: url)
            urlSchemeTask.didReceive(served.response)
            urlSchemeTask.didReceive(served.bytes)
            urlSchemeTask.didFinish()
        } catch {
            urlSchemeTask.didFailWithError(error)
        }
    }

    public func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {}
}
```

Note: `URL(string: "riot-app://<id>/../escape")` may normalize before reaching the handler — that's fine; the refusal test passes either way because normalization can only produce a path that isn't an exact bundle-resource match, and the Rust layer is the boundary regardless.

- [ ] **Step 4: Implement `AppBridgeController.swift`**

```swift
import WebKit

@MainActor
public final class AppBridgeController: NSObject, WKScriptMessageHandler {
    /// Total message budget; individual values are further capped in Rust.
    public static let maxMessageBytes = 262_144

    private let repository: RiotProfileRepository
    private let appIDHex: String
    public weak var webView: WKWebView?
    /// Called after a successful put from this page (Task 8 hooks
    /// persistence-side notifications here too).
    public var onLocalWrite: (() -> Void)?

    public init(repository: RiotProfileRepository, appIDHex: String) {
        self.repository = repository
        self.appIDHex = appIDHex
    }

    public func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        _ = handleForTesting(body: message.body)
    }

    /// Returns false when the message is rejected before dispatch —
    /// malformed shape, unknown op, or over the size budget.
    @discardableResult
    public func handleForTesting(body: Any) -> Bool {
        guard let dict = body as? [String: Any],
              let id = dict["id"] as? Int,
              let op = dict["op"] as? String
        else { return false }
        let approximateSize = (dict["key"] as? String ?? "").utf8.count
            + (dict["value"] as? String ?? "").utf8.count
            + (dict["prefix"] as? String ?? "").utf8.count
        guard approximateSize <= Self.maxMessageBytes else {
            reply(id: id, ok: false, payloadJSON: jsonString("Couldn't save that — try again"))
            return false
        }

        switch op {
        case "get":
            guard let key = dict["key"] as? String else { return false }
            do {
                let value = try repository.appDataGet(appID: appIDHex, key: key)
                reply(id: id, ok: true, payloadJSON: value.map(jsonString) ?? "null")
            } catch {
                reply(id: id, ok: false, payloadJSON: jsonString("Couldn't load that"))
            }
        case "put":
            guard let key = dict["key"] as? String, let value = dict["value"] as? String else { return false }
            do {
                try repository.appDataPut(appID: appIDHex, key: key, valueJSON: value)
                reply(id: id, ok: true, payloadJSON: "null")
                onLocalWrite?()
                notifyDataChanged()
            } catch {
                reply(id: id, ok: false, payloadJSON: jsonString("Couldn't save that — try again"))
            }
        case "list":
            guard let prefix = dict["prefix"] as? String else { return false }
            do {
                let rows = try repository.appDataList(appID: appIDHex, prefix: prefix)
                let encoded = rows.map { #"{"key":\#(jsonString($0.key)),"value":\#(jsonString($0.valueJSON))}"# }
                reply(id: id, ok: true, payloadJSON: "[\(encoded.joined(separator: ","))]")
            } catch {
                reply(id: id, ok: false, payloadJSON: jsonString("Couldn't load that"))
            }
        case "whoami":
            let name = (try? repository.appDisplayName()) ?? "member"
            reply(id: id, ok: true, payloadJSON: #"{"displayName":\#(jsonString(name))}"#)
        default:
            reply(id: id, ok: false, payloadJSON: jsonString("Unsupported"))
            return false
        }
        return true
    }

    public func notifyDataChanged() {
        webView?.evaluateJavaScript("window.__riotDataChanged && window.__riotDataChanged()")
    }

    private func reply(id: Int, ok: Bool, payloadJSON: String) {
        webView?.evaluateJavaScript("window.__riotResolve(\(id), \(ok), \(payloadJSON))")
    }

    private func jsonString(_ value: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: [value])
        let array = data.flatMap { String(data: $0, encoding: .utf8) } ?? "[\"\"]"
        return String(array.dropFirst().dropLast())
    }
}
```

- [ ] **Step 5: Run the tests**

```sh
scripts/conference/build-native-core.sh   # only if FFI changed since Task 6
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-derived
```
Expected: all green, including the WebView-driven adversarial tests (they run the real checklist page in-process; WKWebView works in simulator unit tests).

- [ ] **Step 6: Commit**

```bash
git add apps/ios/Riot/Apps/ apps/ios/RiotTests/AppRuntimeHostTests.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): app WebView plumbing — scheme handler, riot.js bridge, CSP"
```

---

### Task 8: `AppRuntimeView` + change notifications

**Hard contract (platform review, deferred item 3):** Rust deliberately does NOT trust-gate `app_data_put/get/list` — the WebView host is the enforcement point. On iOS that means: an `AppRuntimeView`/`AppBridgeController` may only ever be constructed for an app that is trusted in the current space, checked at open time (the Tools row already only offers "Open" for trusted apps; additionally `guard` on the trust state in the launch action so a stale UI can't open a just-revoked app). The Android runtime plan states the same gate (`is_app_trusted` host-side) — keep the two hosts consistent.

**Files:**
- Create: `apps/ios/Riot/Apps/AppRuntimeView.swift` (RiotKit target)
- Modify: `apps/ios/Riot/Apps/AppBridgeController.swift` (only if a hook is missing)

- [ ] **Step 1: Implement `AppRuntimeView`**

```swift
import SwiftUI
import WebKit

/// Full-screen host for one trusted app in one space. Every navigation
/// other than riot-app:// is refused; browser state is non-persistent;
/// the page's only I/O is the riot bridge.
public struct AppRuntimeView: View {
    public static let dataChangedNotification = Notification.Name("RiotAppDataChanged")

    private let repository: RiotProfileRepository
    private let appIDHex: String
    private let appName: String
    private let onClose: () -> Void

    public init(
        repository: RiotProfileRepository,
        appIDHex: String,
        appName: String,
        onClose: @escaping () -> Void
    ) {
        self.repository = repository
        self.appIDHex = appIDHex
        self.appName = appName
        self.onClose = onClose
    }

    public var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(appName)
                    .font(.riot(.mono, size: 14))
                    .textCase(.uppercase)
                Spacer()
                Button("Close", action: onClose)
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("app-close")
            }
            .padding(12)
            AppWebView(repository: repository, appIDHex: appIDHex)
        }
    }
}

private struct AppWebView: UIViewRepresentable {
    let repository: RiotProfileRepository
    let appIDHex: String

    func makeCoordinator() -> Coordinator {
        Coordinator(repository: repository, appIDHex: appIDHex)
    }

    func makeUIView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.addUserScript(
            WKUserScript(source: RiotJS.source, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        )
        configuration.userContentController.add(context.coordinator.bridge, name: "riot")
        configuration.setURLSchemeHandler(
            AppSchemeHandler(repository: repository),
            forURLScheme: AppSchemeHandler.scheme
        )
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = context.coordinator
        context.coordinator.bridge.webView = webView
        context.coordinator.observeDataChanges()
        if let url = URL(string: "\(AppSchemeHandler.scheme)://\(appIDHex)/index.html") {
            webView.load(URLRequest(url: url))
        }
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {}

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate {
        let bridge: AppBridgeController
        private var observer: NSObjectProtocol?

        init(repository: RiotProfileRepository, appIDHex: String) {
            self.bridge = AppBridgeController(repository: repository, appIDHex: appIDHex)
        }

        func observeDataChanges() {
            observer = NotificationCenter.default.addObserver(
                forName: AppRuntimeView.dataChangedNotification,
                object: nil,
                queue: .main
            ) { [weak bridge] _ in
                MainActor.assumeIsolated { bridge?.notifyDataChanged() }
            }
        }

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
        ) {
            let allowed = navigationAction.request.url?.scheme == AppSchemeHandler.scheme
            decisionHandler(allowed ? .allow : .cancel)
        }

        deinit {
            if let observer { NotificationCenter.default.removeObserver(observer) }
        }
    }
}
```

Post `AppRuntimeView.dataChangedNotification` from the refresh sources outside the page itself, matching the spec's three `watch` triggers: (a) the page's own puts — already direct via the bridge's `notifyDataChanged()`; (b) sync completion — post from `RiotAppModel` wherever sync refreshes `entries` (locate the sync-completion path in `AppModel`/`SyncCoordinator`; if none is cleanly identifiable, skip and note the deviation in the commit message and the COLLABORATION.md row); (c) app returning to foreground — add `.onChange(of: scenePhase)` in `AppRuntimeView` (`@Environment(\.scenePhase)`) posting the notification when the phase becomes `.active`.

- [ ] **Step 2: Build check + commit**

Add the file to the RiotKit sources phase. Run the same `xcodebuild test -scheme RiotKit` command — expected: everything still green (this view gets exercised by Task 10's UI test; unit coverage of the navigation policy came from Task 7's bridge/handler tests).

```bash
git add apps/ios/Riot/Apps/AppRuntimeView.swift apps/ios/Riot/AppModel.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): AppRuntimeView WebView host with change notifications"
```

---

### Task 9: Tools UI — space section, review sheet, launch

**Files:**
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Test: `apps/ios/RiotTests/ToolsSectionTests.swift`

- [ ] **Step 1: Write the failing model tests**

```swift
import XCTest
@testable import RiotKit

@MainActor
final class ToolsSectionTests: XCTestCase {
    func testAppsRefreshAfterSpaceCreationAndTrustFlipsListing() throws {
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: FileManager.default.temporaryDirectory
            .appendingPathComponent("tools-\(UUID().uuidString)"))
        model.createSpace(title: "Berlin Mutual Aid")

        XCTAssertEqual(model.apps.count, 1)
        XCTAssertEqual(model.apps[0].name, "Checklist")
        XCTAssertFalse(model.apps[0].trusted)

        model.trustApp(appID: model.apps[0].appIDHex)
        XCTAssertTrue(model.apps[0].trusted)
        XCTAssertNil(model.errorMessage)
    }
}
```

Adapt the `bootstrap` call to its real signature (read `AppModel.swift` — if `bootstrap()` takes no directory parameter, add an optional one defaulting to the current Application Support path so tests can isolate storage; `RiotAppModel` already has a test-only `init(testError:)` precedent for testability affordances). Run RiotKit scheme — expected failure: no `apps` property.

- [ ] **Step 2: Extend `RiotAppModel`**

Add `@Published public private(set) var apps: [RiotSpaceApp] = []`, refreshed in `bootstrap()` and after `createSpace` via a private `refreshApps()` (`apps = (try? repository?.spaceApps()) ?? []`). Add:

```swift
public func trustApp(appID: String) {
    perform {
        try repository?.trustApp(appID: appID)
        refreshApps()
    }
}
```

(Match `perform`'s actual shape — it wraps mutations and routes errors to `errorMessage`.)

- [ ] **Step 3: Add the Tools section + review sheet + launch cover to `SpacesView`**

In `ConferenceShellView.swift`, extend `SpacesView` (state: `@State private var reviewing: RiotSpaceApp?`, `@State private var running: RiotSpaceApp?`). Below the existing space card, when `model.space != nil`:

```swift
RiotCard {
    VStack(alignment: .leading, spacing: 12) {
        Text("Tools")
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        if model.apps.isEmpty {
            Text("No tools yet.")
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
        ForEach(model.apps, id: \.appIDHex) { app in
            HStack {
                Text(app.name)
                    .font(.riot(.body, size: 17, relativeTo: .body))
                Spacer()
                if app.trusted {
                    Button("Open") { running = app }
                        .buttonStyle(.riotPrimary)
                        .accessibilityIdentifier("open-\(app.name)")
                } else {
                    RiotBadge("New")
                    Button("Review") { reviewing = app }
                        .buttonStyle(.riotSecondary)
                        .accessibilityIdentifier("review-\(app.name)")
                }
            }
        }
    }
}
```

Review sheet (`.sheet(item: $reviewing)` — make `RiotSpaceApp` `Identifiable` via `appIDHex`), the trust-decision moment, plain language only:

```swift
private struct AppReviewSheet: View {
    @Environment(\.colorScheme) private var colorScheme
    let app: RiotSpaceApp
    let onApprove: () -> Void
    let onCancel: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(app.name)
                    .font(.riot(.poster, size: 32, relativeTo: .largeTitle))
                Text(app.description)
                    .font(.riot(.body, size: 17, relativeTo: .body))
                RiotCard {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("This app can")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        ForEach(app.permissions, id: \.self) { permission in
                            Text(permission)
                                .font(.riot(.body, size: 15, relativeTo: .body))
                        }
                    }
                }
                Button("Let everyone in this space use this") { onApprove() }
                    .buttonStyle(.riotPrimary)
                    .accessibilityIdentifier("approve-app")
                Button("Not now") { onCancel() }
                    .buttonStyle(.riotSecondary)
            }
            .padding(20)
        }
    }
}
```

Launch: `.fullScreenCover(item: $running)` presenting `AppRuntimeView(repository:appIDHex:appName:onClose: { running = nil })` — `SpacesView` needs repository access; expose it via a computed accessor on `RiotAppModel` (`public var profileRepository: RiotProfileRepository? { repository }`) rather than widening the stored property.

- [ ] **Step 4: Run unit tests + build the app**

Run the `RiotKit` test scheme and the `Riot` app build (both commands from the README, as in Task 6). Expected: green tests, clean build.

- [ ] **Step 5: Commit**

```bash
git add apps/ios/Riot/AppModel.swift apps/ios/Riot/ConferenceShellView.swift apps/ios/RiotTests/ToolsSectionTests.swift apps/ios/Riot.xcodeproj/project.pbxproj
git commit -m "feat(ios): Tools section with organizer review sheet and app launch"
```

---

### Task 10: XCUITest end-to-end + final verification

**Files:**
- Create: `apps/ios/RiotUITests/ChecklistFlowUITests.swift` (UI-test target, `B…` UUID sequence)
- Modify: `COLLABORATION.md`

- [ ] **Step 1: Write the UI test (the definition of done)**

XCUITest reaches web content through `app.webViews` element queries (the checklist's inputs carry accessibility labels from Task 1's HTML). Existing convention: find by label/visible text, dismiss the startup alert first, screenshots via `XCTAttachment`.

```swift
import XCTest

final class ChecklistFlowUITests: XCTestCase {
    func testCreateSpaceApproveChecklistAddItemAndSurviveRelaunch() {
        let app = XCUIApplication()
        app.launch()
        if app.alerts.firstMatch.waitForExistence(timeout: 2) {
            app.alerts.firstMatch.buttons.firstMatch.tap()
        }

        // Create the space if this run starts fresh.
        let createButton = app.buttons["Create public space"]
        if createButton.waitForExistence(timeout: 3) {
            createButton.tap()
        }

        // Review and approve the checklist as the organizer.
        let review = app.buttons["review-Checklist"]
        XCTAssertTrue(review.waitForExistence(timeout: 5))
        review.tap()
        let approve = app.buttons["approve-app"]
        XCTAssertTrue(approve.waitForExistence(timeout: 5))
        approve.tap()

        // Open it and add an item inside the WebView.
        let open = app.buttons["open-Checklist"]
        XCTAssertTrue(open.waitForExistence(timeout: 5))
        open.tap()
        let webView = app.webViews.firstMatch
        let field = webView.textFields["New item"]
        XCTAssertTrue(field.waitForExistence(timeout: 10), "checklist page must load")
        field.tap()
        field.typeText("Bring water")
        webView.buttons["Add"].tap()
        XCTAssertTrue(webView.staticTexts["Bring water"].waitForExistence(timeout: 10))

        // Check it off.
        let checkbox = webView.checkBoxes["Bring water"]
        if checkbox.waitForExistence(timeout: 5) {
            checkbox.tap()
        } else {
            webView.switches["Bring water"].tap() // WebKit may expose <input type=checkbox> as a switch
        }

        // Relaunch: trust and the item must survive.
        app.terminate()
        app.launch()
        if app.alerts.firstMatch.waitForExistence(timeout: 2) {
            app.alerts.firstMatch.buttons.firstMatch.tap()
        }
        let reopen = app.buttons["open-Checklist"]
        XCTAssertTrue(reopen.waitForExistence(timeout: 5), "trust must persist across relaunch")
        reopen.tap()
        XCTAssertTrue(app.webViews.firstMatch.staticTexts["Bring water"].waitForExistence(timeout: 10),
                      "items must persist across relaunch")

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
```

Add to the RiotUITests sources phase. Note the COLLABORATION.md caveat: if installing the app manually for debugging, use `xcrun simctl install`, not `xcodebuild install`.

- [ ] **Step 2: Run the UI test**

```sh
scripts/conference/build-native-core.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-app-derived \
  -only-testing:RiotUITests/ChecklistFlowUITests
```
Expected: PASS. (If the `Riot` scheme doesn't include the UI-test target, enable it in the shared scheme's Test action — `RiotTabNavigationUITests` already runs somehow; match whatever wiring it uses.)

- [ ] **Step 3: Full verification sweep**

Run, all from repo root:
- `cargo test --workspace --all-features` — all green
- `cargo clippy --workspace --all-features --all-targets -- -D warnings` — clean
- `cargo xtask validate-contracts` and `cargo xtask generate-bindings` — PASS
- The RiotKit unit-test suite and the full `Riot` scheme test run (both xcodebuild commands) — all green

- [ ] **Step 4: Update COLLABORATION.md + commit**

Update this workstream's claim row to **Done, released** with the commit list and the verification evidence above (exact commands + results, per the file's ground rules).

```bash
git add apps/ios/RiotUITests/ChecklistFlowUITests.swift apps/ios/Riot.xcodeproj/project.pbxproj COLLABORATION.md
git commit -m "test(ios): checklist end-to-end XCUITest — approve, use, relaunch"
```

---

## After this plan lands

The on-device end-to-end goal is met. Known follow-ups deliberately left out (tracked in the specs, not silently dropped): Android runtime parity (same FFI surface), the full app-directory storefront round (`2026-07-11-app-directory-design.md` — share/endorse/CLI/catalog spaces), trust revocation UI, sync-completion-triggered `watch` refresh if Task 8 fell back to foreground-only, and replacing the placeholder starter author identity with a real project identity.
