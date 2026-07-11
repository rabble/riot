# Signed JS Apps — Core Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Rust core + FFI foundation for signed JS apps — manifest/bundle format, per-space trust list, and the namespace-scoped data bridge apps use to read/write their own Willow entries. This plan produces working, fully-tested software with no UI: everything is exercised through `cargo test`. A follow-up plan (native WebView runtime for iOS/Android + the checklist app itself) depends on this one landing first, mirroring how this repo already split Task 5 (nearby transport) into a core/FFI plan followed by separate iOS/Android work.

**Architecture:** A new `crates/riot-core/src/apps/` module, kept separate from `import/` (evidence-only). App data entries are ordinary Willow entries at a distinct top-level path (`apps/<app_id>/...`), signed and committed through the exact same `EvidenceStore::inspect → plan_all → commit` pipeline every other entry already uses — no parallel "trusted write" bypass. A small new UniFFI surface in `crates/riot-ffi/` exposes this to native code. Full design rationale: `docs/superpowers/specs/2026-07-11-signed-js-apps-design.md`.

**Tech Stack:** Rust (`riot-core`, `riot-ffi`), `minicbor` (manual canonical encode/decode, matching `model/mod.rs`'s existing style), `sha2` (domain-separated digests, matching `willow/digest.rs`), `willow25` (`Path`, `Entry`, `AuthorisedEntry`), UniFFI.

---

## Before you start

Run `git status --short` and read `COLLABORATION.md` — this is a shared checkout. The claim row "Active claim: signed JS apps platform" is already posted; update it with your progress as you land each task. None of the files this plan touches are currently claimed by anyone else, but re-check before each task in case that's changed.

## File Structure

- `crates/riot-core/src/apps/mod.rs` — module root, `AppsError`
- `crates/riot-core/src/apps/entry.rs` — Task 1: path construction, entry building
- `crates/riot-core/src/apps/manifest.rs` — Task 2: `AppManifest`, `app_id` derivation
- `crates/riot-core/src/apps/bundle.rs` — Task 2: minimal resource-pack format
- `crates/riot-core/src/apps/trust.rs` — Task 3: trust-list evaluation (pure function)
- `crates/riot-core/src/apps/bridge.rs` — Task 5: `AppDataBridge` (put/get/list)
- `crates/riot-core/src/import/join.rs` — Task 4: add `live_entries_with_prefix`
- `crates/riot-core/src/session.rs` — Task 4: add `EvidenceStore::entries_with_prefix`
- `crates/riot-core/src/lib.rs` — register `pub mod apps;`
- `crates/riot-ffi/src/apps_ffi.rs` — Task 6: UniFFI surface
- Integration test files under `crates/riot-core/tests/` and `crates/riot-ffi/tests/` (or wherever existing FFI contract tests live — check `crates/riot-ffi/` for the existing pattern before creating a new one)

---

### Task 1: App-data path construction and entry building

**Files:**
- Create: `crates/riot-core/src/apps/mod.rs`
- Create: `crates/riot-core/src/apps/entry.rs`
- Modify: `crates/riot-core/src/lib.rs` — add `pub mod apps;`
- Test: `crates/riot-core/tests/apps_entry_path.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_entry_path.rs
use riot_core::apps::entry::{app_data_path, build_app_data_entry, APPS_COMPONENT};
use riot_core::apps::AppsError;
use riot_core::willow::{generate_communal_author, Path};

#[test]
fn valid_key_builds_expected_path() {
    let app_id = [7u8; 32];
    let path = app_data_path(&app_id, "items/abc-123").expect("valid path");
    let expected =
        Path::from_slices(&[APPS_COMPONENT, &app_id, b"items", b"abc-123"]).expect("path");
    assert_eq!(path, expected);
}

#[test]
fn empty_key_is_rejected() {
    let app_id = [1u8; 32];
    assert_eq!(app_data_path(&app_id, ""), Err(AppsError::KeyEmpty));
}

#[test]
fn empty_segment_is_rejected() {
    let app_id = [1u8; 32];
    assert_eq!(
        app_data_path(&app_id, "items//x"),
        Err(AppsError::KeySegmentInvalid)
    );
}

#[test]
fn uppercase_or_traversal_like_segment_is_rejected() {
    let app_id = [1u8; 32];
    assert_eq!(
        app_data_path(&app_id, "../secret"),
        Err(AppsError::KeySegmentInvalid)
    );
    assert_eq!(
        app_data_path(&app_id, "Items/abc"),
        Err(AppsError::KeySegmentInvalid)
    );
}

#[test]
fn oversized_key_component_is_rejected() {
    let app_id = [1u8; 32];
    let long = "a".repeat(300);
    assert_eq!(
        app_data_path(&app_id, &long),
        Err(AppsError::PathComponentTooLong)
    );
}

#[test]
fn build_app_data_entry_signs_under_authors_own_namespace_and_subspace() {
    let author = generate_communal_author().expect("author");
    let app_id = [9u8; 32];
    let entry = build_app_data_entry(&author, &app_id, "items/x", 1, b"{}").expect("entry");
    assert_eq!(entry.namespace_id(), author.namespace_id());
    assert_eq!(entry.subspace_id(), author.subspace_id());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p riot-core --test apps_entry_path`
Expected: compile failure — `riot_core::apps` module does not exist yet.

- [ ] **Step 3: Write the module skeleton and error type**

```rust
// crates/riot-core/src/apps/mod.rs
//! Signed JS apps: manifest/bundle format, per-space trust list, and the
//! namespace-scoped data bridge apps use to read/write their own Willow
//! entries. Kept separate from `import/` (evidence-only).

pub mod bridge;
pub mod bundle;
pub mod entry;
pub mod manifest;
pub mod trust;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppsError {
    KeyEmpty,
    KeySegmentInvalid,
    TooManyPathComponents,
    PathComponentTooLong,
    PathTooLong,
    PathInvalid,
    ManifestFieldInvalid,
    BundleFieldInvalid,
    BundleTooLarge,
    Willow(crate::willow::WillowError),
}

impl std::fmt::Display for AppsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AppsError {}

impl From<crate::willow::WillowError> for AppsError {
    fn from(e: crate::willow::WillowError) -> Self {
        AppsError::Willow(e)
    }
}
```

Note: this references `bridge`, `bundle`, `manifest`, `trust` modules that don't exist until Tasks 2/3/5 — for this step, comment out those four `pub mod` lines (keep only `pub mod entry;`) so the crate compiles; uncomment each as its task adds the file.

```rust
// crates/riot-core/src/apps/mod.rs (Step 3, actual content for Task 1 only)
pub mod entry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppsError {
    KeyEmpty,
    KeySegmentInvalid,
    TooManyPathComponents,
    PathComponentTooLong,
    PathTooLong,
    PathInvalid,
    Willow(crate::willow::WillowError),
}

impl std::fmt::Display for AppsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AppsError {}

impl From<crate::willow::WillowError> for AppsError {
    fn from(e: crate::willow::WillowError) -> Self {
        AppsError::Willow(e)
    }
}
```

```rust
// crates/riot-core/src/apps/entry.rs
//! App-data path construction and entry building. A key like `"items/<id>"`
//! maps to Willow path segments `apps / <app_id> / items / <id>` — the same
//! communal namespace/subspace an alert entry would use, just under a
//! different top-level component so app data never collides with evidence.

use crate::import::bundle::{MAX_PATH_COMPONENTS, MAX_PATH_COMPONENT_BYTES, MAX_PATH_TOTAL_BYTES};
use crate::willow::identity::EvidenceAuthor;
use crate::willow::{Entry, Path};

use super::AppsError;

pub const APPS_COMPONENT: &[u8] = b"apps";
pub const APP_ID_BYTES: usize = 32;

/// A key segment is a non-empty sequence of lowercase ASCII letters,
/// digits, or hyphens — the same safe-path-segment rule already used for
/// conference-fixture routes. `crypto.randomUUID()` output (lowercase hex
/// and hyphens) satisfies this directly.
fn validate_segment(segment: &str) -> Result<(), AppsError> {
    if segment.is_empty() {
        return Err(AppsError::KeySegmentInvalid);
    }
    if !segment
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(AppsError::KeySegmentInvalid);
    }
    Ok(())
}

pub fn app_data_path(app_id: &[u8; APP_ID_BYTES], key: &str) -> Result<Path, AppsError> {
    if key.is_empty() {
        return Err(AppsError::KeyEmpty);
    }
    let segments: Vec<&str> = key.split('/').collect();
    for segment in &segments {
        validate_segment(segment)?;
    }

    let component_count = 2 + segments.len();
    if component_count > MAX_PATH_COMPONENTS {
        return Err(AppsError::TooManyPathComponents);
    }
    if app_id.len() > MAX_PATH_COMPONENT_BYTES {
        return Err(AppsError::PathComponentTooLong);
    }
    let mut total_bytes = APPS_COMPONENT.len() + app_id.len();
    for segment in &segments {
        if segment.len() > MAX_PATH_COMPONENT_BYTES {
            return Err(AppsError::PathComponentTooLong);
        }
        total_bytes += segment.len();
    }
    if total_bytes > MAX_PATH_TOTAL_BYTES {
        return Err(AppsError::PathTooLong);
    }

    let mut raw_segments: Vec<&[u8]> = Vec::with_capacity(component_count);
    raw_segments.push(APPS_COMPONENT);
    raw_segments.push(app_id);
    for segment in &segments {
        raw_segments.push(segment.as_bytes());
    }
    Path::from_slices(&raw_segments).map_err(|_| AppsError::PathInvalid)
}

pub fn build_app_data_entry(
    author: &EvidenceAuthor,
    app_id: &[u8; APP_ID_BYTES],
    key: &str,
    willow_timestamp_micros: u64,
    payload: &[u8],
) -> Result<Entry, AppsError> {
    let path = app_data_path(app_id, key)?;
    Ok(Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(willow_timestamp_micros)
        .payload(payload)
        .build())
}
```

```rust
// crates/riot-core/src/lib.rs — add this line alongside the existing `pub mod` lines
pub mod apps;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_entry_path`
Expected: 6 passed (the `mod.rs` in this step only declares `pub mod entry;` — leave the other four `pub mod` lines out until their own task adds the file, then add them at that point).

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/mod.rs crates/riot-core/src/apps/entry.rs crates/riot-core/src/lib.rs crates/riot-core/tests/apps_entry_path.rs
git commit -m "feat(apps): add app-data path construction and entry building"
```

---

### Task 2: App manifest and resource-bundle format

**Files:**
- Create: `crates/riot-core/src/apps/manifest.rs`
- Create: `crates/riot-core/src/apps/bundle.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — uncomment `pub mod manifest;` and `pub mod bundle;`, add `ManifestFieldInvalid`, `BundleFieldInvalid`, `BundleTooLarge` to `AppsError`
- Test: `crates/riot-core/tests/apps_manifest.rs`
- Test: `crates/riot-core/tests/apps_bundle.rs`

**Scope note:** the design doc names the [WICG Web Bundle](https://github.com/WICG/isolated-web-apps) format as the packaging inspiration. This task implements a minimal, self-contained resource-pack format — a fixed list of `(path, content_type, bytes)` resources plus a primary entry point, deterministically CBOR-encoded in the same manual style as `model/mod.rs::encode_alert`. It is *not* a byte-for-byte WICG-compliant `.wbn` file: nothing outside this crate's own decoder ever parses these bytes (no browser loads them directly — the native WebView host unpacks resources and serves them locally), so taking on full binary spec compliance would buy nothing. If a future need arises to interoperate with external `.wbn` tooling, that's a separate follow-up.

- [ ] **Step 1: Write the failing manifest tests**

```rust
// crates/riot-core/tests/apps_manifest.rs
use riot_core::apps::manifest::{
    app_id_for, decode_manifest, encode_manifest, AppManifest, MAX_APP_DESCRIPTION_BYTES,
};
use riot_core::apps::AppsError;
use riot_core::willow::generate_communal_author;

fn sample_manifest(author_identity: riot_core::willow::AuthorIdentity) -> AppManifest {
    AppManifest {
        name: "Checklist".to_string(),
        description: "Lets people add and check off shared to-dos.".to_string(),
        version: "1.0.0".to_string(),
        author: author_identity,
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    }
}

#[test]
fn manifest_round_trips_through_encode_decode() {
    let author = generate_communal_author().expect("author");
    let manifest = sample_manifest(author.identity());
    let bytes = encode_manifest(&manifest).expect("encode");
    let decoded = decode_manifest(&bytes).expect("decode");
    assert_eq!(decoded, manifest);
}

#[test]
fn oversized_description_is_rejected() {
    let author = generate_communal_author().expect("author");
    let mut manifest = sample_manifest(author.identity());
    manifest.description = "x".repeat(MAX_APP_DESCRIPTION_BYTES + 1);
    assert_eq!(
        encode_manifest(&manifest),
        Err(AppsError::ManifestFieldInvalid)
    );
}

#[test]
fn app_id_is_deterministic_and_bundle_sensitive() {
    let author = generate_communal_author().expect("author");
    let manifest = sample_manifest(author.identity());
    let bundle_digest_a = [1u8; 32];
    let bundle_digest_b = [2u8; 32];
    let id_a1 = app_id_for(&manifest, &bundle_digest_a).expect("id");
    let id_a2 = app_id_for(&manifest, &bundle_digest_a).expect("id");
    let id_b = app_id_for(&manifest, &bundle_digest_b).expect("id");
    assert_eq!(id_a1, id_a2);
    assert_ne!(id_a1, id_b);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --test apps_manifest`
Expected: compile failure — `riot_core::apps::manifest` does not exist.

- [ ] **Step 3: Implement the manifest module**

```rust
// crates/riot-core/src/apps/manifest.rs
//! App manifest: the plain-language description shown to a space organizer
//! before trusting an app, plus the fields needed to identify and locate
//! its bundle. Canonical encoding mirrors `model/mod.rs::encode_alert`'s
//! manual, strictly-ordered minicbor style.

use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};

use crate::willow::identity::{AuthorIdentity, NamespaceKind};

use super::AppsError;

pub const MAX_APP_NAME_BYTES: usize = 80;
pub const MAX_APP_DESCRIPTION_BYTES: usize = 500;
pub const MAX_APP_VERSION_BYTES: usize = 32;
pub const MAX_APP_ENTRY_POINT_BYTES: usize = 256;
pub const MAX_APP_PERMISSIONS: usize = 8;
pub const MAX_APP_PERMISSION_BYTES: usize = 64;
pub const MAX_MANIFEST_BYTES: usize = 4_096;

const APP_ID_DOMAIN: &[u8] = b"riot/app-id/v1";
pub type AppId = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: AuthorIdentity,
    pub permissions: Vec<String>,
    pub entry_point: String,
}

fn validate(manifest: &AppManifest) -> Result<(), AppsError> {
    let name_ok = !manifest.name.is_empty() && manifest.name.len() <= MAX_APP_NAME_BYTES;
    let description_ok = !manifest.description.is_empty()
        && manifest.description.len() <= MAX_APP_DESCRIPTION_BYTES;
    let version_ok = !manifest.version.is_empty() && manifest.version.len() <= MAX_APP_VERSION_BYTES;
    let entry_point_ok =
        !manifest.entry_point.is_empty() && manifest.entry_point.len() <= MAX_APP_ENTRY_POINT_BYTES;
    let permissions_ok = manifest.permissions.len() <= MAX_APP_PERMISSIONS
        && manifest
            .permissions
            .iter()
            .all(|p| !p.is_empty() && p.len() <= MAX_APP_PERMISSION_BYTES);
    if name_ok && description_ok && version_ok && entry_point_ok && permissions_ok {
        Ok(())
    } else {
        Err(AppsError::ManifestFieldInvalid)
    }
}

pub fn encode_manifest(manifest: &AppManifest) -> Result<Vec<u8>, AppsError> {
    validate(manifest)?;

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(9)?;
        e.u8(0)?.str(&manifest.name)?;
        e.u8(1)?.str(&manifest.description)?;
        e.u8(2)?.str(&manifest.version)?;
        e.u8(3)?.bytes(&manifest.author.namespace_id)?;
        e.u8(4)?.bytes(&manifest.author.subspace_id)?;
        e.u8(5)?.u8(match manifest.author.namespace_kind {
            NamespaceKind::Communal => 0,
        })?;
        e.u8(6)?.bytes(&manifest.author.signing_key_id)?;
        e.u8(7)?.array(manifest.permissions.len() as u64)?;
        for permission in &manifest.permissions {
            e.str(permission)?;
        }
        e.u8(8)?.str(&manifest.entry_point)?;
        Ok(())
    })();
    r.map_err(|_| AppsError::ManifestFieldInvalid)?;

    if buffer.len() > MAX_MANIFEST_BYTES {
        return Err(AppsError::ManifestFieldInvalid);
    }
    Ok(buffer)
}

pub fn decode_manifest(input: &[u8]) -> Result<AppManifest, AppsError> {
    if input.len() > MAX_MANIFEST_BYTES {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let mut d = Decoder::new(input);
    let err = |_| AppsError::ManifestFieldInvalid;

    if d.map().map_err(err)? != Some(9) {
        return Err(AppsError::ManifestFieldInvalid);
    }
    if d.u8().map_err(err)? != 0 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let name = d.str().map_err(err)?.to_string();
    if d.u8().map_err(err)? != 1 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let description = d.str().map_err(err)?.to_string();
    if d.u8().map_err(err)? != 2 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let version = d.str().map_err(err)?.to_string();
    if d.u8().map_err(err)? != 3 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let namespace_id: [u8; 32] = d
        .bytes()
        .map_err(err)?
        .try_into()
        .map_err(|_| AppsError::ManifestFieldInvalid)?;
    if d.u8().map_err(err)? != 4 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let subspace_id: [u8; 32] = d
        .bytes()
        .map_err(err)?
        .try_into()
        .map_err(|_| AppsError::ManifestFieldInvalid)?;
    if d.u8().map_err(err)? != 5 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let namespace_kind = match d.u8().map_err(err)? {
        0 => NamespaceKind::Communal,
        _ => return Err(AppsError::ManifestFieldInvalid),
    };
    if d.u8().map_err(err)? != 6 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let signing_key_id: [u8; 32] = d
        .bytes()
        .map_err(err)?
        .try_into()
        .map_err(|_| AppsError::ManifestFieldInvalid)?;
    if d.u8().map_err(err)? != 7 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let permission_count = d.array().map_err(err)?.ok_or(AppsError::ManifestFieldInvalid)?;
    if permission_count > MAX_APP_PERMISSIONS as u64 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let mut permissions = Vec::with_capacity(permission_count as usize);
    for _ in 0..permission_count {
        permissions.push(d.str().map_err(err)?.to_string());
    }
    if d.u8().map_err(err)? != 8 {
        return Err(AppsError::ManifestFieldInvalid);
    }
    let entry_point = d.str().map_err(err)?.to_string();

    if !d.input()[d.position()..].is_empty() {
        return Err(AppsError::ManifestFieldInvalid);
    }

    let manifest = AppManifest {
        name,
        description,
        version,
        author: AuthorIdentity {
            namespace_id,
            subspace_id,
            namespace_kind,
            signing_key_id,
        },
        permissions,
        entry_point,
    };
    validate(&manifest)?;
    Ok(manifest)
}

/// Content-derived app identity: `SHA256("riot/app-id/v1" || u32be(len) ||
/// manifest_bytes || bundle_digest)`, following the domain-separated digest
/// pattern in `willow/digest.rs`. Two different versions of the same app
/// (different bundle bytes) get different ids by design — a version bump is
/// a new app_id, re-trusted explicitly by each space's organizer.
pub fn app_id_for(manifest: &AppManifest, bundle_digest: &[u8; 32]) -> Result<AppId, AppsError> {
    let manifest_bytes = encode_manifest(manifest)?;
    let mut hasher = Sha256::new();
    hasher.update(APP_ID_DOMAIN);
    hasher.update((manifest_bytes.len() as u32).to_be_bytes());
    hasher.update(&manifest_bytes);
    hasher.update(bundle_digest);
    Ok(hasher.finalize().into())
}
```

Also add to `crates/riot-core/src/apps/mod.rs`:

```rust
pub mod manifest;
```

And add `ManifestFieldInvalid` to `AppsError` (already listed in Task 1's `mod.rs` if you included the full variant list up front; otherwise add it now).

- [ ] **Step 4: Run manifest tests to verify they pass**

Run: `cargo test -p riot-core --test apps_manifest`
Expected: 3 passed.

- [ ] **Step 5: Write the failing bundle tests**

```rust
// crates/riot-core/tests/apps_bundle.rs
use riot_core::apps::bundle::{decode_app_bundle, encode_app_bundle, AppBundle, AppResource, MAX_BUNDLE_TOTAL_BYTES};
use riot_core::apps::AppsError;

fn sample_bundle() -> AppBundle {
    AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![
            AppResource {
                path: "index.html".to_string(),
                content_type: "text/html".to_string(),
                bytes: b"<html></html>".to_vec(),
            },
            AppResource {
                path: "app.js".to_string(),
                content_type: "text/javascript".to_string(),
                bytes: b"console.log('hi')".to_vec(),
            },
        ],
    }
}

#[test]
fn bundle_round_trips_through_encode_decode() {
    let bundle = sample_bundle();
    let bytes = encode_app_bundle(&bundle).expect("encode");
    let decoded = decode_app_bundle(&bytes).expect("decode");
    assert_eq!(decoded, bundle);
}

#[test]
fn entry_point_not_among_resources_is_rejected() {
    let mut bundle = sample_bundle();
    bundle.entry_point = "missing.html".to_string();
    assert_eq!(
        encode_app_bundle(&bundle),
        Err(AppsError::BundleFieldInvalid)
    );
}

#[test]
fn oversized_bundle_is_rejected() {
    let mut bundle = sample_bundle();
    bundle.resources[0].bytes = vec![0u8; MAX_BUNDLE_TOTAL_BYTES + 1];
    assert_eq!(encode_app_bundle(&bundle), Err(AppsError::BundleTooLarge));
}
```

- [ ] **Step 6: Run to verify failure**

Run: `cargo test -p riot-core --test apps_bundle`
Expected: compile failure — `riot_core::apps::bundle` does not exist.

- [ ] **Step 7: Implement the bundle module**

```rust
// crates/riot-core/src/apps/bundle.rs
//! A minimal, self-contained resource pack: a fixed list of
//! `(path, content_type, bytes)` resources plus a primary entry point,
//! deterministically CBOR-encoded. See the "Scope note" in the Task 2
//! commit that introduced this file for why this isn't WICG-`.wbn`-compliant.

use minicbor::{Decoder, Encoder};

use super::AppsError;

pub const MAX_BUNDLE_RESOURCES: usize = 32;
pub const MAX_RESOURCE_PATH_BYTES: usize = 256;
pub const MAX_RESOURCE_CONTENT_TYPE_BYTES: usize = 64;
pub const MAX_BUNDLE_TOTAL_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppResource {
    pub path: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBundle {
    pub entry_point: String,
    pub resources: Vec<AppResource>,
}

fn validate(bundle: &AppBundle) -> Result<(), AppsError> {
    if bundle.resources.is_empty() || bundle.resources.len() > MAX_BUNDLE_RESOURCES {
        return Err(AppsError::BundleFieldInvalid);
    }
    let mut total_bytes = 0usize;
    let mut found_entry_point = false;
    for resource in &bundle.resources {
        if resource.path.is_empty() || resource.path.len() > MAX_RESOURCE_PATH_BYTES {
            return Err(AppsError::BundleFieldInvalid);
        }
        if resource.content_type.is_empty()
            || resource.content_type.len() > MAX_RESOURCE_CONTENT_TYPE_BYTES
        {
            return Err(AppsError::BundleFieldInvalid);
        }
        if resource.path == bundle.entry_point {
            found_entry_point = true;
        }
        total_bytes += resource.bytes.len();
    }
    if !found_entry_point {
        return Err(AppsError::BundleFieldInvalid);
    }
    if total_bytes > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }
    Ok(())
}

pub fn encode_app_bundle(bundle: &AppBundle) -> Result<Vec<u8>, AppsError> {
    validate(bundle)?;

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(2)?;
        e.u8(0)?.str(&bundle.entry_point)?;
        e.u8(1)?.array(bundle.resources.len() as u64)?;
        for resource in &bundle.resources {
            e.map(3)?;
            e.u8(0)?.str(&resource.path)?;
            e.u8(1)?.str(&resource.content_type)?;
            e.u8(2)?.bytes(&resource.bytes)?;
        }
        Ok(())
    })();
    r.map_err(|_| AppsError::BundleFieldInvalid)?;

    if buffer.len() > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }
    Ok(buffer)
}

pub fn decode_app_bundle(input: &[u8]) -> Result<AppBundle, AppsError> {
    if input.len() > MAX_BUNDLE_TOTAL_BYTES {
        return Err(AppsError::BundleTooLarge);
    }
    let mut d = Decoder::new(input);
    let err = |_| AppsError::BundleFieldInvalid;

    if d.map().map_err(err)? != Some(2) {
        return Err(AppsError::BundleFieldInvalid);
    }
    if d.u8().map_err(err)? != 0 {
        return Err(AppsError::BundleFieldInvalid);
    }
    let entry_point = d.str().map_err(err)?.to_string();
    if d.u8().map_err(err)? != 1 {
        return Err(AppsError::BundleFieldInvalid);
    }
    let resource_count = d.array().map_err(err)?.ok_or(AppsError::BundleFieldInvalid)?;
    if resource_count == 0 || resource_count > MAX_BUNDLE_RESOURCES as u64 {
        return Err(AppsError::BundleFieldInvalid);
    }
    let mut resources = Vec::with_capacity(resource_count as usize);
    for _ in 0..resource_count {
        if d.map().map_err(err)? != Some(3) {
            return Err(AppsError::BundleFieldInvalid);
        }
        if d.u8().map_err(err)? != 0 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let path = d.str().map_err(err)?.to_string();
        if d.u8().map_err(err)? != 1 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let content_type = d.str().map_err(err)?.to_string();
        if d.u8().map_err(err)? != 2 {
            return Err(AppsError::BundleFieldInvalid);
        }
        let bytes = d.bytes().map_err(err)?.to_vec();
        resources.push(AppResource { path, content_type, bytes });
    }

    if !d.input()[d.position()..].is_empty() {
        return Err(AppsError::BundleFieldInvalid);
    }

    let bundle = AppBundle { entry_point, resources };
    validate(&bundle)?;
    Ok(bundle)
}
```

Also add `pub mod bundle;` to `crates/riot-core/src/apps/mod.rs`.

- [ ] **Step 8: Run all Task 2 tests, clippy, commit**

Run: `cargo test -p riot-core --test apps_manifest --test apps_bundle`
Expected: 6 passed.

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/manifest.rs crates/riot-core/src/apps/bundle.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_manifest.rs crates/riot-core/tests/apps_bundle.rs
git commit -m "feat(apps): add manifest and resource-bundle codecs"
```

---

### Task 3: Trust-list evaluation

**Files:**
- Create: `crates/riot-core/src/apps/trust.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — uncomment `pub mod trust;`
- Test: `crates/riot-core/tests/apps_trust.rs`

**Design note:** per the spec's planning-time correction, trust authority is a fixed, known list of organizer `SubspaceId`s for a space (not capability delegation, which doesn't exist in this codebase — see the spec's correction). This task implements pure evaluation logic over a slice of trust-marker entries (each already known to be a validly-signed live Willow entry — signature/liveness checking happens where entries enter the store, same as everything else). Keeping this a pure function (no `EvidenceStore` dependency) makes it trivial to test exhaustively.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_trust.rs
use riot_core::apps::trust::{is_trusted, TrustMarker, TrustMarkerKind};
use riot_core::willow::identity::EvidenceAuthor;

fn subspace_of(author: &EvidenceAuthor) -> [u8; 32] {
    *author.subspace_id().as_bytes()
}

#[test]
fn no_markers_means_not_trusted() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    assert!(!is_trusted(&app_id, &[], &[subspace_of(&organizer)]));
}

#[test]
fn organizer_trust_marker_grants_trust() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![TrustMarker {
        app_id,
        author_subspace_id: subspace_of(&organizer),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 10,
    }];
    assert!(is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}

#[test]
fn non_organizer_trust_marker_is_ignored() {
    let non_organizer = riot_core::willow::generate_communal_author().expect("author");
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![TrustMarker {
        app_id,
        author_subspace_id: subspace_of(&non_organizer),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 10,
    }];
    assert!(!is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}

#[test]
fn newer_revoke_overrides_older_trust_from_same_organizer() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Trust,
            timestamp_micros: 10,
        },
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Revoke,
            timestamp_micros: 20,
        },
    ];
    assert!(!is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}

#[test]
fn older_revoke_does_not_override_newer_trust() {
    let organizer = riot_core::willow::generate_communal_author().expect("author");
    let app_id = [1u8; 32];
    let markers = vec![
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Revoke,
            timestamp_micros: 10,
        },
        TrustMarker {
            app_id,
            author_subspace_id: subspace_of(&organizer),
            kind: TrustMarkerKind::Trust,
            timestamp_micros: 20,
        },
    ];
    assert!(is_trusted(&app_id, &markers, &[subspace_of(&organizer)]));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --test apps_trust`
Expected: compile failure — `riot_core::apps::trust` does not exist.

- [ ] **Step 3: Implement**

```rust
// crates/riot-core/src/apps/trust.rs
//! Per-space app trust evaluation. Trust authority is a fixed, known list
//! of organizer `SubspaceId`s for the space (see the design spec's
//! planning-time correction — this codebase has no capability-delegation
//! concept to reuse). A marker from any other subspace at the trust-list
//! path is ignored. Among markers from a *recognized* organizer for the
//! same app, the most recent timestamp wins — ordinary last-write-wins,
//! same as any other Willow path.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustMarkerKind {
    Trust,
    Revoke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustMarker {
    pub app_id: [u8; 32],
    pub author_subspace_id: [u8; 32],
    pub kind: TrustMarkerKind,
    pub timestamp_micros: u64,
}

pub fn is_trusted(
    app_id: &[u8; 32],
    markers: &[TrustMarker],
    organizer_subspace_ids: &[[u8; 32]],
) -> bool {
    let latest = markers
        .iter()
        .filter(|m| &m.app_id == app_id)
        .filter(|m| organizer_subspace_ids.contains(&m.author_subspace_id))
        .max_by_key(|m| m.timestamp_micros);

    matches!(latest, Some(m) if m.kind == TrustMarkerKind::Trust)
}
```

Also add `pub mod trust;` to `crates/riot-core/src/apps/mod.rs`.

- [ ] **Step 4: Run tests, clippy, commit**

Run: `cargo test -p riot-core --test apps_trust`
Expected: 5 passed.

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/trust.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_trust.rs
git commit -m "feat(apps): add per-space trust-list evaluation"
```

---

### Task 4: Query stored entries by path prefix

**Files:**
- Modify: `crates/riot-core/src/import/join.rs`
- Modify: `crates/riot-core/src/session.rs`
- Test: `crates/riot-core/tests/core_entries_with_prefix.rs`

**Why this is needed:** `EvidenceStore` today only exposes `live_entry_ids()` (the full set) and `provenance(entry_id)` (no path). The app data bridge (Task 5) needs to list/watch everything under `apps/<app_id>/...` without scanning and re-decoding every entry in the store by hand. `willow25::paths::Path::is_prefix_of` already exists and does exactly the comparison needed — this task just wires it through the same `JoinState` → `EvidenceStore` path `live_entry_ids()` already uses.

- [ ] **Step 1: Write the failing test**

```rust
// crates/riot-core/tests/core_import_transaction.rs already has helpers for
// building signed test bundles (`signed()`/`signed_distinct()`); this new
// test file follows the same setup pattern. Check that file first for the
// exact current helper signatures before writing the setup below, since
// they may have evolved — the shape here is what they should produce.

use riot_core::apps::entry::{app_data_path, build_app_data_entry};
use riot_core::import::bundle::encode_bundle;
use riot_core::willow::entry::SignedWillowEntry;
use riot_core::willow::{authorise_entry, encode_capability, encode_entry, generate_communal_author};
use riot_core::session::{CommitOutcome, ImportContext, InspectOutcome, RiotSession};

fn commit_app_entry(store: &riot_core::session::EvidenceStore, author: &riot_core::willow::identity::EvidenceAuthor, app_id: &[u8; 32], key: &str, payload: &[u8], timestamp: u64) {
    let entry = build_app_data_entry(author, app_id, key, timestamp, payload).expect("entry");
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let signed = SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    };
    let bundle_bytes = encode_bundle(std::slice::from_ref(&signed)).expect("encode bundle");
    let preview = match store.inspect(&bundle_bytes, ImportContext::new("test")).expect("inspect") {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("rejected: {r:?}"),
    };
    let plan = preview.plan_all().expect("plan_all");
    match plan.commit().expect("commit") {
        CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => {}
    }
}

#[test]
fn entries_with_prefix_returns_only_matching_live_entries() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    commit_app_entry(&store, &author, &[7u8; 32], "items/a", b"{}", 1);
    commit_app_entry(&store, &author, &[7u8; 32], "items/b", b"{}", 2);
    commit_app_entry(&store, &author, &[9u8; 32], "items/c", b"{}", 3);

    let prefix = app_data_path(&[7u8; 32], "items").expect("prefix");
    let matches = store.entries_with_prefix(&prefix).expect("query");

    assert_eq!(matches.len(), 2);
}

#[test]
fn entries_with_prefix_is_empty_when_nothing_matches() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let prefix = app_data_path(&[1u8; 32], "items").expect("prefix");

    let matches = store.entries_with_prefix(&prefix).expect("query");

    assert!(matches.is_empty());
}
```

Note: `app_data_path(&[7u8; 32], "items")` builds a *shorter* path than the full item path (`apps/<app_id>/items`, 3 components) — since `Path::is_prefix_of` compares component-by-component, this shorter path is a genuine prefix of `apps/<app_id>/items/a`. Confirm this compiles as-is; `app_data_path`'s own segment validation only requires non-empty lowercase/digit/hyphen segments, which `"items"` satisfies on its own.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --features conformance --test core_entries_with_prefix`
Expected: compile failure — `entries_with_prefix` does not exist on `EvidenceStore`.

- [ ] **Step 3: Add `live_entries_with_prefix` to `JoinState`**

In `crates/riot-core/src/import/join.rs`, add to the existing `impl JoinState` block (the one with `has_seen`/`is_live_id`/`live_ids`):

```rust
    /// Live entries whose path is prefixed by `prefix`.
    pub fn live_entries_with_prefix(&self, prefix: &crate::willow::Path) -> Vec<(EntryId, crate::willow::Entry)> {
        self.live
            .iter()
            .filter(|s| prefix.is_prefix_of(s.entry.path()))
            .map(|s| (s.id, s.entry.clone()))
            .collect()
    }
```

- [ ] **Step 4: Add `entries_with_prefix` to `EvidenceStore`**

In `crates/riot-core/src/session.rs`, add near the existing `live_entry_ids`:

```rust
    pub fn entries_with_prefix(
        &self,
        prefix: &crate::willow::Path,
    ) -> Result<Vec<(EntryId, crate::willow::Entry)>, SessionError> {
        let st = self.inner.lock().map_err(|_| SessionError::Internal)?;
        st.require_store(self.store_id)?;
        Ok(st
            .store
            .as_ref()
            .unwrap()
            .join
            .live_entries_with_prefix(prefix))
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p riot-core --features conformance --test core_entries_with_prefix`
Expected: 2 passed.

If the helper setup in Step 1 doesn't compile against the *actual current* signatures of `RiotSession::open`/`create_store`/`SignedWillowEntry` (they may have shifted since this plan was written — re-check `crates/riot-core/tests/core_import_transaction.rs` for the current pattern before debugging blind), adjust the test's setup code to match reality; the assertions (`entries_with_prefix` returns exactly the matching live entries) are what matters, not this exact helper shape.

- [ ] **Step 6: Full workspace check and commit**

Run: `cargo test --workspace --all-features`
Expected: all green, no regressions in existing `import`/`session` tests.

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/import/join.rs crates/riot-core/src/session.rs crates/riot-core/tests/core_entries_with_prefix.rs
git commit -m "feat(core): add path-prefix query over live entries"
```

Update `COLLABORATION.md`'s claim row: `session.rs`/`import/join.rs` are free again after this commit, same as every prior session that's touched them.

---

### Task 5: `AppDataBridge` — the namespace-scoped read/write API

**Files:**
- Create: `crates/riot-core/src/apps/bridge.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — uncomment `pub mod bridge;`
- Test: `crates/riot-core/tests/apps_bridge.rs`

**Design note:** `put` mirrors `riot-ffi`'s existing `sign_draft` exactly (`crates/riot-ffi/src/mobile_state.rs:243`) — build an entry, sign it, wrap it as a one-item bundle via `encode_bundle`, then commit through the same `inspect → plan_all → commit` pipeline every other entry uses. This is a local, self-contained call (`EvidenceStore::inspect` returns its own owned `ImportPreview`/`ImportPlan`, not a shared session-wide slot), so it's safe to call concurrently with an unrelated in-progress sync-import preview elsewhere in the same store — Step 1 includes a test proving that.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_bridge.rs
use riot_core::apps::bridge::AppDataBridge;
use riot_core::session::{ImportContext, InspectOutcome, RiotSession};
use riot_core::willow::generate_communal_author;

#[test]
fn put_then_get_round_trips() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");
    let app_id = [7u8; 32];

    AppDataBridge::put(&store, &author, &app_id, "items/a", 1, b"{\"done\":false}")
        .expect("put");

    let value = AppDataBridge::get(&store, &app_id, "items/a").expect("get");
    assert_eq!(value, Some(b"{\"done\":false}".to_vec()));
}

#[test]
fn get_on_missing_key_returns_none() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let app_id = [7u8; 32];

    let value = AppDataBridge::get(&store, &app_id, "items/missing").expect("get");
    assert_eq!(value, None);
}

#[test]
fn list_only_returns_entries_for_the_requesting_app() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    AppDataBridge::put(&store, &author, &[7u8; 32], "items/a", 1, b"1").expect("put");
    AppDataBridge::put(&store, &author, &[9u8; 32], "items/b", 2, b"2").expect("put");

    let listed = AppDataBridge::list(&store, &[7u8; 32], "items").expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, "items/a");
    assert_eq!(listed[0].1, b"1");
}

#[test]
fn put_rejects_traversal_like_key_before_touching_willow() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    let result = AppDataBridge::put(&store, &author, &[7u8; 32], "../escape", 1, b"x");
    assert!(result.is_err());

    let listed = AppDataBridge::list(&store, &[7u8; 32], "").unwrap_or_default();
    assert!(listed.is_empty());
}

#[test]
fn put_does_not_disturb_an_unrelated_pending_import_preview() {
    // Proves AppDataBridge::put's inspect/plan/commit call is self-contained
    // and doesn't interfere with a preview obtained separately from the
    // same store, since EvidenceStore::inspect returns its own owned
    // ImportPreview rather than mutating shared session state.
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    // An empty/no-op bundle inspect just to obtain an independent preview
    // handle representing "something else is mid-review" — using the same
    // one-item-bundle shape as a real import, authored by a second author
    // so it doesn't collide with the app-data path below.
    let other_author = generate_communal_author().expect("author");
    AppDataBridge::put(&store, &other_author, &[1u8; 32], "unrelated", 1, b"x")
        .expect("unrelated put");
    let unrelated_preview = match store
        .inspect(&[], ImportContext::new("test"))
        .unwrap_or(InspectOutcome::Rejected(Default::default()))
    {
        InspectOutcome::Preview(p) => Some(p),
        InspectOutcome::Rejected(_) => None,
    };

    let put_result = AppDataBridge::put(&store, &author, &[7u8; 32], "items/a", 2, b"y");
    assert!(put_result.is_ok());
    drop(unrelated_preview);
}
```

The last test's exact setup (deliberately feeding `&[]` to `inspect` to get *some* independent handle) is awkward — its point is only "an unrelated call into `store.inspect` around the same time doesn't make `AppDataBridge::put` fail." If `InspectOutcome::Rejected` isn't `Default`, simplify this test to just: call `AppDataBridge::put` twice in a row for two different app_ids without any intervening state reset, and assert both succeed. Keep whichever version compiles cleanly against the real `InspectOutcome`/rejection types — the property being tested (independent, non-interfering local commits) is what matters.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --test apps_bridge`
Expected: compile failure — `riot_core::apps::bridge` does not exist.

- [ ] **Step 3: Implement `AppDataBridge`**

```rust
// crates/riot-core/src/apps/bridge.rs
//! The namespace-scoped read/write API apps use. `key`/`prefix` are always
//! relative to `apps/<app_id>/` — callers (the native WebView bridge) never
//! see or set a full Willow path. Writes are signed with the *calling
//! person's* own identity, exactly like a self-authored alert
//! (`riot-ffi::mobile_state::sign_draft`), and land through the identical
//! inspect/plan/commit pipeline — no separate trusted-write path.

use ed25519_dalek::Signature;

use crate::import::bundle::encode_bundle;
use crate::session::{CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, SessionError};
use crate::willow::entry::SignedWillowEntry;
use crate::willow::identity::EvidenceAuthor;
use crate::willow::{authorise_entry, encode_capability, encode_entry};

use super::entry::{app_data_path, build_app_data_entry, APP_ID_BYTES};
use super::AppsError;

pub struct AppDataBridge;

impl AppDataBridge {
    pub fn put(
        store: &EvidenceStore,
        author: &EvidenceAuthor,
        app_id: &[u8; APP_ID_BYTES],
        key: &str,
        willow_timestamp_micros: u64,
        value: &[u8],
    ) -> Result<(), AppsError> {
        let entry = build_app_data_entry(author, app_id, key, willow_timestamp_micros, value)?;
        let authorised = authorise_entry(author, entry)?;
        let token = authorised.authorisation_token();
        let signature: Signature = token.signature().clone().into();
        let signed = SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes: value.to_vec(),
        };
        let bundle_bytes =
            encode_bundle(std::slice::from_ref(&signed)).map_err(|_| AppsError::BundleFieldInvalid)?;

        let preview = match store
            .inspect(&bundle_bytes, ImportContext::new("app-write"))
            .map_err(session_err)?
        {
            InspectOutcome::Preview(p) => p,
            InspectOutcome::Rejected(_) => return Err(AppsError::BundleFieldInvalid),
        };
        let plan = preview.plan_all().map_err(session_err)?;
        match plan.commit().map_err(session_err)? {
            CommitOutcome::Committed(_) | CommitOutcome::NoChanges(_) => Ok(()),
        }
    }

    pub fn get(
        store: &EvidenceStore,
        app_id: &[u8; APP_ID_BYTES],
        key: &str,
    ) -> Result<Option<Vec<u8>>, AppsError> {
        let path = app_data_path(app_id, key)?;
        let matches = store.entries_with_prefix(&path).map_err(session_err)?;
        Ok(matches
            .into_iter()
            .find(|(_, entry)| entry.path() == &path)
            .map(|(_, entry)| entry.payload_digest_bytes().to_vec()))
    }

    pub fn list(
        store: &EvidenceStore,
        app_id: &[u8; APP_ID_BYTES],
        prefix: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, AppsError> {
        let path = app_data_path(app_id, prefix)?;
        let matches = store.entries_with_prefix(&path).map_err(session_err)?;
        let _ = matches;
        todo!("see Step 3 correction note below")
    }
}

fn session_err(_: SessionError) -> AppsError {
    AppsError::BundleFieldInvalid
}
```

**Correction needed before this compiles/passes — read before implementing:** `get`/`list` above assume `entry.payload_digest_bytes()` returns the *payload bytes themselves*. It does not — per `willow/mod.rs`'s `EntryFacts` trait, it returns the WILLIAM3 *digest* of the payload (32 bytes), not the payload. `Entry` (from `willow25`) carries a payload digest and length for integrity, not the payload bytes inline — the actual bytes live in the *bundle* (`SignedWillowEntry.payload_bytes` at write time), not in the committed `Entry` you get back from `entries_with_prefix`.

Before writing this step for real, check whether `EvidenceStore`/`JoinState` retains payload bytes anywhere accessible per entry (search `import/join.rs`'s `Stored` struct — the "Files and Code Sections" history for this repo notes `Stored` was deliberately changed to hold only `entry: Entry`, *not* the full authorised entry or payload, to stop retaining capability/token material). If payload bytes are genuinely not retained after commit, `get`/`list` need one of:
1. A small **payload store** added alongside `JoinState` (keyed by `EntryId` or by path, capped by the same kind of byte-budget accounting `session.rs` already does for everything else) — the most likely correct fix, since apps fundamentally need their data readable back, not just its digest.
2. Or: confirm there's an existing payload-retrieval path elsewhere in the FFI layer (check how `mobile_api.rs` lets a caller read back a *previously signed* alert's content — if such a method exists, mirror it).

This is a real design gap this plan's research didn't fully close — resolve it with a fresh, focused investigation (grep `payload_bytes`, `payload_digest`, and any `retrieve`/`read_payload`-shaped method across `riot-core`/`riot-ffi`) as the first thing done in this step, before writing `get`/`list`'s real bodies. Do not paper over it by inventing a method that doesn't exist. Write this step's RED test first regardless (Step 1 above), then let the real fix — whichever of the two options above turns out right — be what makes it GREEN.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_bridge`
Expected: 5 passed, once Step 3's payload-retrieval gap is resolved.

- [ ] **Step 5: Full workspace check and commit**

Run: `cargo test --workspace --all-features`
Expected: all green.

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/bridge.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_bridge.rs
git commit -m "feat(apps): add AppDataBridge put/get/list"
```

If Step 3's gap required a new payload-retention mechanism, that's very likely touching `import/join.rs`/`session.rs` again — note the additional files actually touched in this commit's message and in the `COLLABORATION.md` claim row; don't silently under-report scope.

---

### Task 6: UniFFI surface

**Files:**
- Create: `crates/riot-ffi/src/apps_ffi.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs` — add app-bridge methods to `LocalProfile`/`ProfileState`, following the exact `with_active` pattern used throughout
- Modify: `crates/riot-ffi/src/lib.rs` (or wherever `mobile_api`/`mobile_state` are registered — check the existing module list) to register `apps_ffi`
- Test: mirror whatever the existing FFI contract-test convention is — locate it first (check `crates/riot-ffi/` for a `tests/` directory or inline `#[cfg(test)]` modules in `mobile_api.rs`) and match it exactly rather than inventing a new pattern

**Scope:** this task exposes `put`/`get`/`list` (Task 5) plus manifest install and trust-list check/set (Tasks 2/3) as UniFFI methods, following the `MobileSyncSession`-style wrapper shown in `mobile_api.rs:166-259` — a `#[derive(uniffi::Object)]` struct wrapping the same `Arc<Mutex<ProfileState>>`, each method a thin synchronous delegator into a `mobile_state.rs` function that goes through `with_active`. Because the exact current shape of `ProfileState`/`LocalProfile`/`MobileError` may have shifted since this plan's research pass, **the first step of this task must be re-reading `crates/riot-ffi/src/mobile_api.rs` and `mobile_state.rs` in full** to confirm the pattern still matches before writing new code against it — do not assume the snippets quoted in Task 5's design note are still verbatim-current.

- [ ] **Step 1: Re-confirm the current FFI wrapper pattern**

Read `crates/riot-ffi/src/mobile_api.rs` and `crates/riot-ffi/src/mobile_state.rs` in full. Confirm: the `#[derive(uniffi::Object)]` + `Arc<Mutex<ProfileState>>` shape, the `with_active` helper's exact current signature, and `MobileError`'s current variant list (a new variant will likely be needed, e.g. `AppRejected` or similar, mapped from `AppsError`).

- [ ] **Step 2: Write failing FFI contract tests**

Write tests exercising: installing a manifest+bundle pair, trusting an app_id, then `app_data_put`/`app_data_get`/`app_data_list` round-tripping through the FFI layer end-to-end (not just the core layer Task 5 already covered) — match whatever test harness pattern the existing `mobile_api.rs` tests use (in-process, no simulator/emulator needed, same as every other FFI contract test in this crate).

- [ ] **Step 3: Implement the FFI methods**

Add to `mobile_state.rs`: `app_data_put`, `app_data_get`, `app_data_list`, `trust_app`, `untrust_app`, `is_app_trusted` — each a thin `with_active` delegator calling into `riot_core::apps::bridge::AppDataBridge` / `riot_core::apps::trust`. Add corresponding `#[uniffi::export]` methods to a new `AppRuntimeSession` object in `apps_ffi.rs`, matching `MobileSyncSession`'s exact shape (own `sync_id`-style handle field, synchronous methods, `Result<_, MobileError>`).

- [ ] **Step 4: Run tests, generate bindings, clippy, commit**

Run: `cargo test -p riot-ffi --all-features`
Expected: all green.

Run: `cargo xtask generate-bindings`
Expected: non-empty Swift/Kotlin output, no errors (this crate's existing contract-validation convention — check `cargo xtask validate-contracts` too).

Run: `cargo clippy -p riot-ffi --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-ffi/src/apps_ffi.rs crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/src/lib.rs
git commit -m "feat(ffi): expose signed JS apps bridge over UniFFI"
```

---

## After this plan lands

Update `COLLABORATION.md`'s "signed JS apps platform" claim row to **Done, released** with the final commit list and `cargo test --workspace --all-features` result. Everything in this plan is testable via `cargo test` alone — no native shell, simulator, or emulator needed, so it can be verified and landed by a single agent session without device access.

The follow-up plan (not written yet) covers: the checklist app's static HTML/CSS/JS, and the iOS/Android WebView runtime hosts (`AppRuntimeView`/CSP/`postMessage` bridge on iOS, `AppRuntimeScreen`/`@JavascriptInterface` bridge on Android) that call the `AppRuntimeSession` UniFFI surface this plan builds. Write that plan fresh once this one is verified green — native build/test evidence (`xcodebuild test`, `./gradlew testDebugUnitTest`) will matter there in a way it doesn't here.
