# JS Apps Runtime (iOS) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Everything between the signed-JS-apps core platform and a person on an iOS simulator checking items off a shared checklist inside their space — checklist app fixture, packing, starter catalog, FFI listings/resource-serving, WKWebView host with `window.riot` bridge, Tools UI with organizer approval, XCUITest end-to-end.

**Architecture:** The checklist ships as plain HTML/JS packed into the existing canonical `apps::bundle`/`apps::manifest` CBOR codecs, embedded in `riot-core` via `include_bytes!` as a content-addressed starter catalog (no key material — integrity is canonical decode + re-derived `app_id`). New FFI methods list apps per space and serve bundle resources; iOS hosts them in a WKWebView behind a custom `riot-app://` scheme handler with a strict CSP and a `postMessage` bridge whose security boundary is the Rust `AppDataBridge`.

**Tech Stack:** Rust (riot-core, riot-ffi, xtask), UniFFI, Swift 6 / SwiftUI / WebKit, XCTest + XCUITest.

**Spec:** `docs/superpowers/specs/2026-07-11-js-apps-runtime-ios-design.md` (and its two neighbors: the platform spec `2026-07-11-signed-js-apps-design.md` and `2026-07-11-app-directory-design.md`).

---

## Before you start

1. Run `git status --short` and read `COLLABORATION.md`. This checkout is shared with other active agents. Claim the files of the task you are starting in `COLLABORATION.md` before editing.
2. **Dependency gate:** Tasks 1–4 are independent and can start now. Tasks 5–10 require the core platform plan (`docs/superpowers/plans/2026-07-11-signed-js-apps-core-platform.md`) Tasks 5–6 (`AppDataBridge`, `apps_ffi.rs`) to be **committed**. Check `git log --oneline` for `feat(apps): add AppDataBridge` and `feat(ffi): expose signed JS apps bridge` (titles may vary — look for `crates/riot-core/src/apps/bridge.rs` and `crates/riot-ffi/src/apps_ffi.rs` existing). If they haven't landed, do Tasks 1–4, then stop and hand off rather than duplicating the other agent's claimed work.
3. iOS tasks (6–10) need the native prerequisites from `apps/ios/README.md`: run `scripts/conference/build-native-core.sh` from the repo root after any FFI change, before building the Xcode project.

## File Structure

Rust:
- `fixtures/apps/checklist/` — Task 1: `index.html`, `app.js`, `style.css`, `riot-app.json` (checklist app source, no build step)
- `crates/riot-core/src/apps/pack.rs` — Task 2: `pack_app` (resources+fields → canonical manifest/bundle bytes + `app_id`), `content_type_for`
- `crates/xtask/src/main.rs` — Task 3: `pack-starter-apps` subcommand
- `fixtures/apps/checklist.manifest.cbor`, `fixtures/apps/checklist.bundle.cbor` — Task 3: committed packed artifacts (generated, then frozen)
- `crates/riot-core/src/apps/starter.rs` — Task 4: `include_bytes!` catalog, `decode_starter_app`, `starter_apps()`
- `crates/riot-ffi/src/apps_ffi.rs` + `mobile_state.rs` — Task 5: `AppListing` record, `list_space_apps`, `app_resource`, `app_display_name`, persistence-bundle returns

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
riot.whoami().then((who) => { me = who; });

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
        .catch(() => showError("Couldn't save that — try again"));
    });
    const label = document.createElement("label");
    label.textContent = row.value.text;
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
  riot.put("items/" + crypto.randomUUID(), { text, done: false, ...stamp() })
    .then(() => { input.value = ""; })
    .catch(() => showError("Couldn't save that — try again"));
});

riot.watch("items", render);
```

- [ ] **Step 4: Write `riot-app.json`**

The pack-time manifest source. The author identity is a fixed committed **public** identity (conference-fixture precedent; placeholder values, replaced by a real project identity in the directory round). `signing_key_id` is always taken equal to `subspace_id` at pack time (matching `identity.rs`'s invariant); `namespace_kind` is always communal.

```json
{
  "name": "Checklist",
  "description": "A shared checklist for this space. Anyone here can add items and check them off.",
  "version": "1.0.0",
  "entry_point": "index.html",
  "permissions": [
    "Keep its own notes in this space. Nothing else — no internet, no photos."
  ],
  "author": {
    "namespace_id_hex": "27cd7747ceecf672b65a998f1606162fc1e39793dd61a442a0af65ba4f92951e",
    "subspace_id_hex": "99069a7b075d21e0dc7e4b7c7daf311f8e1d308001763d9d78ef60e9b9857157"
  }
}
```

- [ ] **Step 5: Commit**

```bash
git add fixtures/apps/checklist/
git commit -m "feat(apps): add checklist app source fixture"
```

---

### Task 2: `pack_app` in riot-core

**Files:**
- Create: `crates/riot-core/src/apps/pack.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — add `pub mod pack;`
- Test: `crates/riot-core/tests/apps_pack.rs`

Pure function: resources + manifest fields in, canonical `(manifest_bytes, bundle_bytes, app_id)` out. No filesystem access in riot-core — directory walking lives in xtask (Task 3); the drift test (Task 4) re-derives the same bytes from `std::fs` reads at test time. Resource ordering is pinned (sort by path) so packing is deterministic regardless of input order.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_pack.rs
use riot_core::apps::bundle::AppResource;
use riot_core::apps::pack::{content_type_for, pack_app, AppPackInput};
use riot_core::willow::identity::{AuthorIdentity, NamespaceKind};

fn author() -> AuthorIdentity {
    AuthorIdentity {
        namespace_id: [0x11; 32],
        subspace_id: [0x22; 32],
        namespace_kind: NamespaceKind::Communal,
        signing_key_id: [0x22; 32],
    }
}

fn input() -> AppPackInput {
    AppPackInput {
        name: "Checklist".into(),
        description: "A shared checklist.".into(),
        version: "1.0.0".into(),
        author: author(),
        permissions: vec!["Keep its own notes in this space".into()],
        entry_point: "index.html".into(),
        resources: vec![
            AppResource {
                path: "index.html".into(),
                content_type: "text/html".into(),
                bytes: b"<!doctype html>".to_vec(),
            },
            AppResource {
                path: "app.js".into(),
                content_type: "text/javascript".into(),
                bytes: b"console.log(1)".to_vec(),
            },
        ],
    }
}

#[test]
fn pack_is_deterministic_and_order_independent() {
    let a = pack_app(input()).expect("pack");
    let mut reversed = input();
    reversed.resources.reverse();
    let b = pack_app(reversed).expect("pack");
    assert_eq!(a.manifest_bytes, b.manifest_bytes);
    assert_eq!(a.bundle_bytes, b.bundle_bytes);
    assert_eq!(a.app_id, b.app_id);
}

#[test]
fn packed_bytes_round_trip_through_the_standard_decoders() {
    let packed = pack_app(input()).expect("pack");
    let manifest = riot_core::apps::manifest::decode_manifest(&packed.manifest_bytes).expect("manifest");
    let bundle = riot_core::apps::bundle::decode_app_bundle(&packed.bundle_bytes).expect("bundle");
    assert_eq!(manifest.name, "Checklist");
    assert_eq!(bundle.entry_point, "index.html");
    assert_eq!(bundle.resources.len(), 2);
}

#[test]
fn changing_a_resource_changes_the_app_id() {
    let a = pack_app(input()).expect("pack");
    let mut changed = input();
    changed.resources[1].bytes = b"console.log(2)".to_vec();
    let b = pack_app(changed).expect("pack");
    assert_ne!(a.app_id, b.app_id);
}

#[test]
fn missing_entry_point_is_rejected() {
    let mut bad = input();
    bad.entry_point = "missing.html".into();
    assert!(pack_app(bad).is_err());
}

#[test]
fn content_types_are_inferred_for_known_extensions_only() {
    assert_eq!(content_type_for("index.html"), Some("text/html"));
    assert_eq!(content_type_for("app.js"), Some("text/javascript"));
    assert_eq!(content_type_for("style.css"), Some("text/css"));
    assert_eq!(content_type_for("logo.svg"), Some("image/svg+xml"));
    assert_eq!(content_type_for("photo.png"), Some("image/png"));
    assert_eq!(content_type_for("evil.wasm"), None);
    assert_eq!(content_type_for("noextension"), None);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --test apps_pack`
Expected: compile failure — `riot_core::apps::pack` does not exist.

- [ ] **Step 3: Implement `pack.rs`**

```rust
// crates/riot-core/src/apps/pack.rs
//! Pack an app's resources + manifest fields into the canonical committed
//! byte artifacts. Pure — no filesystem access; callers (xtask, tests)
//! read files themselves. Determinism matters: `decode_app_bundle`
//! enforces canonicality, and the starter-catalog drift test re-derives
//! these bytes in CI, so resources are always sorted by path before
//! encoding.

use sha2::{Digest, Sha256};

use crate::willow::identity::AuthorIdentity;

use super::bundle::{encode_app_bundle, AppBundle, AppResource};
use super::manifest::{app_id_for, encode_manifest, AppId, AppManifest};
use super::AppsError;

#[derive(Debug, Clone)]
pub struct AppPackInput {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: AuthorIdentity,
    pub permissions: Vec<String>,
    pub entry_point: String,
    pub resources: Vec<AppResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedApp {
    pub manifest_bytes: Vec<u8>,
    pub bundle_bytes: Vec<u8>,
    pub app_id: AppId,
}

pub fn pack_app(input: AppPackInput) -> Result<PackedApp, AppsError> {
    let mut resources = input.resources;
    resources.sort_by(|a, b| a.path.cmp(&b.path));

    let bundle = AppBundle {
        entry_point: input.entry_point.clone(),
        resources,
    };
    let bundle_bytes = encode_app_bundle(&bundle)?;
    let bundle_digest: [u8; 32] = Sha256::digest(&bundle_bytes).into();

    let manifest = AppManifest {
        name: input.name,
        description: input.description,
        version: input.version,
        author: input.author,
        permissions: input.permissions,
        entry_point: input.entry_point,
    };
    let manifest_bytes = encode_manifest(&manifest)?;
    let app_id = app_id_for(&manifest, &bundle_digest)?;

    Ok(PackedApp {
        manifest_bytes,
        bundle_bytes,
        app_id,
    })
}

/// Known-safe web resource types only; anything else is refused at pack
/// time rather than guessed.
pub fn content_type_for(path: &str) -> Option<&'static str> {
    let extension = path.rsplit_once('.')?.1;
    match extension {
        "html" => Some("text/html"),
        "js" => Some("text/javascript"),
        "css" => Some("text/css"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        _ => None,
    }
}
```

Add to `crates/riot-core/src/apps/mod.rs` alongside the existing module list: `pub mod pack;`

Note: `encode_app_bundle`'s own validation already rejects a missing entry point (`entry_point_found` check in `bundle.rs`), so `pack_app` needs no duplicate check — the test passes through that path.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_pack`
Expected: 5 passed.

- [ ] **Step 5: Full check and commit**

Run: `cargo test -p riot-core --all-features` and `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: all green, clean.

```bash
git add crates/riot-core/src/apps/pack.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_pack.rs
git commit -m "feat(apps): add deterministic app packing"
```

---

### Task 3: `cargo xtask pack-starter-apps` + committed artifacts

**Files:**
- Modify: `crates/xtask/Cargo.toml` — add `riot-core = { path = "../riot-core" }`
- Modify: `crates/xtask/src/main.rs` — new subcommand
- Create (generated, then committed): `fixtures/apps/checklist.manifest.cbor`, `fixtures/apps/checklist.bundle.cbor`

- [ ] **Step 1: Add the riot-core dependency**

In `crates/xtask/Cargo.toml` under `[dependencies]`: `riot-core = { path = "../riot-core" }`. Then run `cargo xtask validate-contracts` — expected PASS (the validator checks riot-core's *release* feature graph; a default-features path dep from xtask must not enable riot-core's test-only feature — if validate-contracts fails on the feature graph, use `riot-core = { path = "../riot-core", default-features = false }` and re-check).

- [ ] **Step 2: Write the failing xtask test**

In `crates/xtask/src/main.rs`'s existing `#[cfg(test)] mod tests`, following the `temp_dir(name)` scaffold pattern already there:

```rust
#[test]
fn pack_starter_app_dir_produces_decodable_artifacts() {
    let dir = temp_dir("pack-starter");
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("index.html"), b"<!doctype html>").expect("write html");
    fs::write(dir.join("app.js"), b"console.log(1)").expect("write js");
    fs::write(
        dir.join("riot-app.json"),
        br#"{
  "name": "Sample",
  "description": "A sample app.",
  "version": "1.0.0",
  "entry_point": "index.html",
  "permissions": ["Keep its own notes in this space"],
  "author": {
    "namespace_id_hex": "1111111111111111111111111111111111111111111111111111111111111111",
    "subspace_id_hex": "2222222222222222222222222222222222222222222222222222222222222222"
  }
}"#,
    )
    .expect("write manifest source");

    let packed = pack_starter_app_dir(&dir).expect("pack");
    riot_core::apps::manifest::decode_manifest(&packed.manifest_bytes).expect("manifest decodes");
    riot_core::apps::bundle::decode_app_bundle(&packed.bundle_bytes).expect("bundle decodes");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn pack_starter_app_dir_rejects_unknown_resource_types() {
    let dir = temp_dir("pack-starter-bad");
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("index.html"), b"<!doctype html>").expect("write html");
    fs::write(dir.join("evil.wasm"), b"\0asm").expect("write wasm");
    fs::write(
        dir.join("riot-app.json"),
        br#"{
  "name": "Sample",
  "description": "A sample app.",
  "version": "1.0.0",
  "entry_point": "index.html",
  "permissions": ["Keep its own notes in this space"],
  "author": {
    "namespace_id_hex": "1111111111111111111111111111111111111111111111111111111111111111",
    "subspace_id_hex": "2222222222222222222222222222222222222222222222222222222222222222"
  }
}"#,
    )
    .expect("write manifest source");

    let result = pack_starter_app_dir(&dir);
    assert!(result.is_err(), "unknown resource types must fail packing, got {result:?}");

    fs::remove_dir_all(&dir).ok();
}
```

Run: `cargo test -p xtask`
Expected: compile failure — `pack_starter_app_dir` does not exist.

- [ ] **Step 3: Implement the subcommand**

Add to `crates/xtask/src/main.rs` (module level, near the other command fns):

```rust
struct PackedStarterApp {
    manifest_bytes: Vec<u8>,
    bundle_bytes: Vec<u8>,
    app_id_hex: String,
}

fn hex32(value: &serde_json::Value, key: &str) -> Result<[u8; 32], String> {
    let text = value
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("riot-app.json: missing author.{key}"))?;
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("riot-app.json: author.{key} must be 64 hex chars"));
    }
    let mut out = [0u8; 32];
    for (i, chunk) in text.as_bytes().chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16).unwrap() as u8;
        let lo = (chunk[1] as char).to_digit(16).unwrap() as u8;
        out[i] = hi << 4 | lo;
    }
    Ok(out)
}

fn pack_starter_app_dir(dir: &std::path::Path) -> Result<PackedStarterApp, String> {
    use riot_core::apps::bundle::AppResource;
    use riot_core::apps::pack::{content_type_for, pack_app, AppPackInput};
    use riot_core::willow::identity::{AuthorIdentity, NamespaceKind};

    let source = std::fs::read_to_string(dir.join("riot-app.json"))
        .map_err(|e| format!("read riot-app.json: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&source).map_err(|e| format!("parse riot-app.json: {e}"))?;
    let text = |key: &str| -> Result<String, String> {
        json.get(key)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or_else(|| format!("riot-app.json: missing {key}"))
    };
    let author_json = json
        .get("author")
        .ok_or("riot-app.json: missing author")?;
    let subspace_id = hex32(author_json, "subspace_id_hex")?;
    let author = AuthorIdentity {
        namespace_id: hex32(author_json, "namespace_id_hex")?,
        subspace_id,
        namespace_kind: NamespaceKind::Communal,
        signing_key_id: subspace_id,
    };
    let permissions = json
        .get("permissions")
        .and_then(|v| v.as_array())
        .ok_or("riot-app.json: missing permissions")?
        .iter()
        .map(|p| p.as_str().map(str::to_owned).ok_or("riot-app.json: permission must be a string".to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    let mut resources = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read dir: {e}"))? {
        let entry = entry.map_err(|e| format!("read dir entry: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "riot-app.json" {
            continue;
        }
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            return Err(format!("unexpected non-file in app dir: {name}"));
        }
        let content_type = content_type_for(&name)
            .ok_or_else(|| format!("unsupported resource type: {name}"))?;
        resources.push(AppResource {
            path: name.clone(),
            content_type: content_type.to_string(),
            bytes: std::fs::read(entry.path()).map_err(|e| format!("read {name}: {e}"))?,
        });
    }

    let packed = pack_app(AppPackInput {
        name: text("name")?,
        description: text("description")?,
        version: text("version")?,
        author,
        permissions,
        entry_point: text("entry_point")?,
        resources,
    })
    .map_err(|e| format!("pack: {e}"))?;

    Ok(PackedStarterApp {
        app_id_hex: packed.app_id.iter().map(|b| format!("{b:02x}")).collect(),
        manifest_bytes: packed.manifest_bytes,
        bundle_bytes: packed.bundle_bytes,
    })
}

fn run_pack_starter_apps() -> Result<String, String> {
    let root = workspace_root()?;
    let dir = root.join("fixtures/apps/checklist");
    let packed = pack_starter_app_dir(&dir)?;
    std::fs::write(root.join("fixtures/apps/checklist.manifest.cbor"), &packed.manifest_bytes)
        .map_err(|e| format!("write manifest artifact: {e}"))?;
    std::fs::write(root.join("fixtures/apps/checklist.bundle.cbor"), &packed.bundle_bytes)
        .map_err(|e| format!("write bundle artifact: {e}"))?;
    Ok(format!("packed checklist app_id={}", packed.app_id_hex))
}
```

Register it: add a `Some("pack-starter-apps")` arm to `main`'s command `match` mapping `run_pack_starter_apps()` to PASS/FAIL prints + `ExitCode` exactly like `generate-bindings`, and append `"pack-starter-apps"` to `available_commands()`. Adjust the snippets above to the actual signatures in the file (e.g. `workspace_root()`'s return type) — read the surrounding code first and match it; the exact error-string plumbing there wins over this sketch.

- [ ] **Step 4: Run tests, then generate the artifacts**

Run: `cargo test -p xtask`
Expected: all pass (existing + 2 new).

Run: `cargo xtask pack-starter-apps`
Expected: PASS line with the checklist `app_id` hex. `fixtures/apps/checklist.manifest.cbor` and `fixtures/apps/checklist.bundle.cbor` now exist.

- [ ] **Step 5: Commit (including generated artifacts — they are frozen inputs from here on)**

```bash
git add crates/xtask/ fixtures/apps/checklist.manifest.cbor fixtures/apps/checklist.bundle.cbor Cargo.lock
git commit -m "feat(xtask): pack starter apps into committed canonical artifacts"
```

---

### Task 4: Starter catalog in riot-core

**Files:**
- Create: `crates/riot-core/src/apps/starter.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — add `pub mod starter;`
- Test: `crates/riot-core/tests/apps_starter.rs`

`include_bytes!` is a first in this workspace — the committed `.cbor` artifacts compile into the library so the mobile binary carries them. Everything is verified at load through the standard canonical decoders; a corrupted embed is silently excluded (never a panic). The drift test re-packs from `fixtures/apps/checklist/` at test time and must produce byte-identical artifacts, so editing app source without re-running `cargo xtask pack-starter-apps` fails CI.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_starter.rs
use riot_core::apps::starter::{decode_starter_app, starter_apps};

#[test]
fn starter_catalog_contains_the_checklist() {
    let apps = starter_apps();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].manifest.name, "Checklist");
    assert_eq!(apps[0].bundle.entry_point, "index.html");
    assert!(apps[0]
        .bundle
        .resources
        .iter()
        .any(|r| r.path == "app.js" && r.content_type == "text/javascript"));
}

#[test]
fn corrupted_manifest_bytes_are_silently_excluded() {
    let apps = starter_apps();
    let manifest_bytes = riot_core::apps::manifest::encode_manifest(&apps[0].manifest).expect("encode");
    let bundle_bytes = riot_core::apps::bundle::encode_app_bundle(&apps[0].bundle).expect("encode");

    let mut tampered = manifest_bytes.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0xff;
    assert!(decode_starter_app(&tampered, &bundle_bytes).is_none());

    let mut tampered_bundle = bundle_bytes.clone();
    let last = tampered_bundle.len() - 1;
    tampered_bundle[last] ^= 0xff;
    assert!(decode_starter_app(&manifest_bytes, &tampered_bundle).is_none());
}

#[test]
fn entry_point_mismatch_between_manifest_and_bundle_is_excluded() {
    let apps = starter_apps();
    let mut manifest = apps[0].manifest.clone();
    manifest.entry_point = "style.css".into(); // valid resource, but not the bundle's entry point
    let manifest_bytes = riot_core::apps::manifest::encode_manifest(&manifest).expect("encode");
    let bundle_bytes = riot_core::apps::bundle::encode_app_bundle(&apps[0].bundle).expect("encode");
    assert!(decode_starter_app(&manifest_bytes, &bundle_bytes).is_none());
}

/// Drift guard: the committed artifacts must equal a fresh pack of the
/// committed source directory. Editing fixtures/apps/checklist/* without
/// re-running `cargo xtask pack-starter-apps` fails here.
#[test]
fn embedded_artifacts_match_a_fresh_pack_of_the_source_directory() {
    use riot_core::apps::bundle::AppResource;
    use riot_core::apps::pack::{content_type_for, pack_app, AppPackInput};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/apps");
    let dir = root.join("checklist");

    let source: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("riot-app.json")).expect("read riot-app.json"),
    )
    .expect("parse riot-app.json");

    let apps = starter_apps();
    let manifest = &apps[0].manifest;
    // riot-app.json is the source of truth the xtask packed from; the
    // decoded embedded manifest must agree with it field-for-field.
    assert_eq!(manifest.name, source["name"].as_str().unwrap());
    assert_eq!(manifest.description, source["description"].as_str().unwrap());
    assert_eq!(manifest.version, source["version"].as_str().unwrap());
    assert_eq!(manifest.entry_point, source["entry_point"].as_str().unwrap());

    let mut resources = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read dir") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "riot-app.json" {
            continue;
        }
        resources.push(AppResource {
            path: name.clone(),
            content_type: content_type_for(&name).expect("known type").to_string(),
            bytes: std::fs::read(entry.path()).expect("read resource"),
        });
    }

    let packed = pack_app(AppPackInput {
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        version: manifest.version.clone(),
        author: manifest.author.clone(),
        permissions: manifest.permissions.clone(),
        entry_point: manifest.entry_point.clone(),
        resources,
    })
    .expect("re-pack");

    let embedded_manifest = std::fs::read(root.join("checklist.manifest.cbor")).expect("artifact");
    let embedded_bundle = std::fs::read(root.join("checklist.bundle.cbor")).expect("artifact");
    assert_eq!(packed.manifest_bytes, embedded_manifest, "manifest drift — re-run cargo xtask pack-starter-apps");
    assert_eq!(packed.bundle_bytes, embedded_bundle, "bundle drift — re-run cargo xtask pack-starter-apps");
    assert_eq!(packed.app_id, apps[0].app_id);
}
```

If `AuthorIdentity` is not `Clone`, take its fields individually (it's a plain struct of arrays — construct a new one). If `serde_json` is not already a riot-core dev-dependency, add it under `[dev-dependencies]` (it is already a workspace dependency).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --test apps_starter`
Expected: compile failure — `riot_core::apps::starter` does not exist.

- [ ] **Step 3: Implement `starter.rs`**

```rust
// crates/riot-core/src/apps/starter.rs
//! Built-in starter apps, embedded at compile time and verified through
//! the exact same canonical decoders as any synced app — "Built into
//! Riot" is a provenance label, not a trust shortcut. No key material:
//! integrity is canonical decoding plus the content-derived app_id; a
//! tampered embed fails decode (or changes identity) and is silently
//! excluded, matching the import path's treatment of invalid items.
//! Launch still requires the space organizer's trust marker.

use sha2::{Digest, Sha256};

use super::bundle::{decode_app_bundle, AppBundle};
use super::manifest::{app_id_for, decode_manifest, AppId, AppManifest};

const CHECKLIST_MANIFEST: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/apps/checklist.manifest.cbor"));
const CHECKLIST_BUNDLE: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/apps/checklist.bundle.cbor"));

#[derive(Debug, Clone)]
pub struct StarterApp {
    pub app_id: AppId,
    pub manifest: AppManifest,
    pub bundle: AppBundle,
}

/// Standard decode path for a manifest+bundle pair. Returns `None` for
/// anything invalid — non-canonical bytes, entry-point mismatch, size
/// violations — with no distinction callers could turn into UI errors.
pub fn decode_starter_app(manifest_bytes: &[u8], bundle_bytes: &[u8]) -> Option<StarterApp> {
    let manifest = decode_manifest(manifest_bytes).ok()?;
    let bundle = decode_app_bundle(bundle_bytes).ok()?;
    if manifest.entry_point != bundle.entry_point {
        return None;
    }
    let bundle_digest: [u8; 32] = Sha256::digest(bundle_bytes).into();
    let app_id = app_id_for(&manifest, &bundle_digest).ok()?;
    Some(StarterApp {
        app_id,
        manifest,
        bundle,
    })
}

pub fn starter_apps() -> Vec<StarterApp> {
    [(CHECKLIST_MANIFEST, CHECKLIST_BUNDLE)]
        .into_iter()
        .filter_map(|(m, b)| decode_starter_app(m, b))
        .collect()
}
```

Add `pub mod starter;` to `apps/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_starter`
Expected: 4 passed.

- [ ] **Step 5: Full check and commit**

Run: `cargo test -p riot-core --all-features`, `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`, `cargo xtask validate-contracts`
Expected: all green/clean/PASS.

```bash
git add crates/riot-core/src/apps/starter.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_starter.rs crates/riot-core/Cargo.toml
git commit -m "feat(apps): embed content-addressed starter catalog with drift guard"
```

---

### Task 5: FFI — app listings, resource serving, display name

**Files:**
- Modify: `crates/riot-ffi/src/apps_ffi.rs` (created by core plan Task 6 — see gate below)
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Test: `crates/riot-ffi/tests/mobile_contract.rs` (or the apps contract test file Task 6 created — match it)

**Gate:** requires core plan Tasks 5–6 committed (`apps/bridge.rs` + `apps_ffi.rs`). **Step 1 is mandatory re-reading**, exactly as the core plan itself prescribes for Task 6: the other agent's landed surface is authoritative over every snippet below. Adapt names/shapes to what exists; do not create a parallel surface.

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
          return call("list", { prefix: prefix }).then(function (rows) {
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
