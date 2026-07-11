# App Directory Implementation Plan (core + FFI + CLI)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the data layer, FFI surface, and publishing CLI for the app directory — app-index paths, endorsement markers, pure directory assembly, publish/share/endorse store I/O, starter-catalog verification, and the `riot-app` packing tool. Everything here is exercised through `cargo test`; the native storefront UI (iOS/Android screens) is a separate follow-up plan, mirroring how the signed-JS-apps work split core/FFI from native.

**Architecture:** New files in the existing `crates/riot-core/src/apps/` module (`index.rs`, `endorse.rs`, `directory.rs`, `starter.rs`), small additions to the apps FFI surface, and a new `crates/riot-app-cli/` workspace member. All writes go through the same `EvidenceStore::inspect → plan_all → commit` pipeline as everything else; the directory itself is a pure function over decoded entries. Design rationale: `docs/superpowers/specs/2026-07-11-app-directory-design.md`.

**Tech Stack:** Rust (`riot-core`, `riot-ffi`, new `riot-app-cli`), `minicbor` (manual canonical style, matching `apps/manifest.rs`), `sha2` (domain-separated digests, matching `willow/digest.rs`), `serde_json` (CLI manifest input only — already a workspace pin), UniFFI.

---

## Before you start

1. Run `git status --short` and read `COLLABORATION.md`. This is a shared checkout with an **active session executing the signed-JS-apps core platform plan** (`docs/superpowers/plans/2026-07-11-signed-js-apps-core-platform.md`, claim row "signed JS apps platform"). **This plan is blocked until that claim row reads Done** — it consumes that plan's `AppManifest`/`AppBundle` codecs, `app_id_for`, `TrustMarker`/`is_trusted`, `entries_with_prefix`, the `AppDataBridge` payload-retrieval mechanism, and the `apps_ffi.rs` surface. Do not start while it's still Executing.
2. Post a claim row for this plan: new files `crates/riot-core/src/apps/{index,endorse,directory,starter}.rs`, their tests, `crates/riot-ffi/src/apps_ffi.rs` (additive), `crates/riot-app-cli/` (new crate), root `Cargo.toml` (one member line).
3. Verify baseline: `cargo test --workspace --all-features` green before Task 1.
4. **Payload retrieval reality check:** core-plan Task 5 had a flagged gap — committed `Entry` values carry a payload *digest*, not payload bytes, so the core plan required adding some payload-retrieval mechanism (their Step 3 correction note). Find what actually landed (grep `payload` in `session.rs`, `import/join.rs`, `apps/bridge.rs`) and note the real API name. Tasks 4–6 below call it `store.payload_for(entry_id)` — substitute the landed name/shape throughout; the behavior required is exactly "give me back the payload bytes for a live entry I hold an `EntryId` (or `Entry`) for."

## File Structure

- `crates/riot-core/src/apps/index.rs` — Task 1: app-index path builders + `app_bundle_digest`; Task 4: publish/scan store I/O
- `crates/riot-core/src/apps/endorse.rs` — Task 2: endorsement marker codec; Task 4: `write_endorsement`
- `crates/riot-core/src/apps/directory.rs` — Task 3: pure listing assembly
- `crates/riot-core/src/apps/starter.rs` — Task 5: starter-catalog verification + (empty for now) embedded catalog
- `crates/riot-core/src/apps/mod.rs` — register new modules, add `IndexFieldInvalid`, `EndorsementFieldInvalid`, `IndexEntryMismatch` to `AppsError`
- `crates/riot-ffi/src/apps_ffi.rs` — Task 6: `directory_listings`, `share_app`, `endorse_app`
- `crates/riot-app-cli/` — Task 7: new workspace member (`keygen`, `pack`, `inspect`)
- Tests: `crates/riot-core/tests/apps_index_paths.rs`, `apps_endorse.rs`, `apps_directory.rs`, `apps_index_io.rs`, `apps_starter.rs`; FFI + CLI tests per their crates' existing conventions

---

### Task 1: App-index paths and the app-bundle digest

**Files:**
- Create: `crates/riot-core/src/apps/index.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — add `pub mod index;` and new `AppsError` variants
- Test: `crates/riot-core/tests/apps_index_paths.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_index_paths.rs
use riot_core::apps::index::{
    app_bundle_digest, app_index_bundle_path, app_index_endorsement_path,
    app_index_manifest_path, app_index_prefix_for, APP_INDEX_COMPONENT,
};
use riot_core::willow::Path;

#[test]
fn manifest_and_bundle_paths_have_expected_shape() {
    let app_id = [7u8; 32];
    let manifest = app_index_manifest_path(&app_id).expect("path");
    let bundle = app_index_bundle_path(&app_id).expect("path");
    assert_eq!(
        manifest,
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"manifest"]).expect("path")
    );
    assert_eq!(
        bundle,
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"bundle"]).expect("path")
    );
}

#[test]
fn endorsement_path_embeds_endorser_subspace() {
    let app_id = [7u8; 32];
    let endorser = [9u8; 32];
    let path = app_index_endorsement_path(&app_id, &endorser).expect("path");
    assert_eq!(
        path,
        Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"endorsements", &endorser])
            .expect("path")
    );
}

#[test]
fn per_app_prefix_is_a_prefix_of_all_three() {
    let app_id = [7u8; 32];
    let prefix = app_index_prefix_for(&app_id).expect("prefix");
    assert!(prefix.is_prefix_of(&app_index_manifest_path(&app_id).expect("p")));
    assert!(prefix.is_prefix_of(&app_index_bundle_path(&app_id).expect("p")));
    assert!(prefix.is_prefix_of(
        &app_index_endorsement_path(&app_id, &[9u8; 32]).expect("p")
    ));
}

#[test]
fn app_bundle_digest_is_deterministic_and_length_bound() {
    let a = app_bundle_digest(b"bytes");
    let b = app_bundle_digest(b"bytes");
    let c = app_bundle_digest(b"other");
    assert_eq!(a, b);
    assert_ne!(a, c);
    // Domain separation: not a bare SHA-256 of the input.
    use sha2::{Digest, Sha256};
    let bare: [u8; 32] = Sha256::digest(b"bytes").into();
    assert_ne!(a, bare);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p riot-core --test apps_index_paths`
Expected: compile failure — `riot_core::apps::index` does not exist.

- [ ] **Step 3: Implement**

```rust
// crates/riot-core/src/apps/index.rs
//! App-index paths: where a distributable app lives as Willow entries.
//! `app-index/<app_id>/manifest`, `app-index/<app_id>/bundle`, and
//! `app-index/<app_id>/endorsements/<endorser-subspace>`. Deliberately a
//! different top-level component from `apps/<app_id>/...` (runtime data,
//! `entry.rs`) so an app writing a data key named "manifest" can never
//! collide with its own distribution entries.

use sha2::{Digest, Sha256};

use crate::willow::Path;

use super::entry::APP_ID_BYTES;
use super::AppsError;

pub const APP_INDEX_COMPONENT: &[u8] = b"app-index";

const APP_BUNDLE_DIGEST_DOMAIN: &[u8] = b"riot/app-bundle-digest/v1";

/// Domain-separated digest of the encoded `AppBundle` bytes — the
/// `bundle_digest` input to `manifest::app_id_for`. Pinned here (not in
/// `willow/digest.rs`) because it is app-platform identity, not Willow
/// entry identity.
pub fn app_bundle_digest(bundle_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(APP_BUNDLE_DIGEST_DOMAIN);
    hasher.update((bundle_bytes.len() as u32).to_be_bytes());
    hasher.update(bundle_bytes);
    hasher.finalize().into()
}

pub fn app_index_prefix_for(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id]).map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_manifest_path(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id, b"manifest"])
        .map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_bundle_path(app_id: &[u8; APP_ID_BYTES]) -> Result<Path, AppsError> {
    Path::from_slices(&[APP_INDEX_COMPONENT, app_id, b"bundle"])
        .map_err(|_| AppsError::PathInvalid)
}

pub fn app_index_endorsement_path(
    app_id: &[u8; APP_ID_BYTES],
    endorser_subspace_id: &[u8; 32],
) -> Result<Path, AppsError> {
    Path::from_slices(&[
        APP_INDEX_COMPONENT,
        app_id,
        b"endorsements",
        endorser_subspace_id,
    ])
    .map_err(|_| AppsError::PathInvalid)
}
```

In `crates/riot-core/src/apps/mod.rs`, add `pub mod index;` alongside the existing `pub mod` lines, and add these variants to `AppsError` (they're used by Tasks 2 and 4):

```rust
    IndexFieldInvalid,
    EndorsementFieldInvalid,
    IndexEntryMismatch,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_index_paths`
Expected: 4 passed.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/index.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_index_paths.rs
git commit -m "feat(apps): add app-index paths and app-bundle digest"
```

---

### Task 2: Endorsement marker codec

**Files:**
- Create: `crates/riot-core/src/apps/endorse.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — add `pub mod endorse;`
- Test: `crates/riot-core/tests/apps_endorse.rs`

An endorsement is one small signed entry per (app, endorser) at the Task 1 endorsement path — last-write-wins per path means an endorser updates or retracts by overwriting their own marker. The payload repeats `app_id` to bind the marker bytes to the app even if the bytes are ever seen out of path context.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_endorse.rs
use riot_core::apps::endorse::{
    decode_endorsement, encode_endorsement, EndorsementMarker, MAX_ENDORSEMENT_NOTE_BYTES,
};
use riot_core::apps::AppsError;

fn sample() -> EndorsementMarker {
    EndorsementMarker {
        app_id: [7u8; 32],
        note: "we ran jail support with this".to_string(),
        retracted: false,
    }
}

#[test]
fn endorsement_round_trips() {
    let marker = sample();
    let bytes = encode_endorsement(&marker).expect("encode");
    assert_eq!(decode_endorsement(&bytes).expect("decode"), marker);
}

#[test]
fn empty_note_is_allowed() {
    let marker = EndorsementMarker { note: String::new(), ..sample() };
    let bytes = encode_endorsement(&marker).expect("encode");
    assert_eq!(decode_endorsement(&bytes).expect("decode"), marker);
}

#[test]
fn retracted_round_trips() {
    let marker = EndorsementMarker { retracted: true, ..sample() };
    let bytes = encode_endorsement(&marker).expect("encode");
    assert!(decode_endorsement(&bytes).expect("decode").retracted);
}

#[test]
fn oversized_note_is_rejected() {
    let marker = EndorsementMarker {
        note: "x".repeat(MAX_ENDORSEMENT_NOTE_BYTES + 1),
        ..sample()
    };
    assert_eq!(
        encode_endorsement(&marker),
        Err(AppsError::EndorsementFieldInvalid)
    );
}

#[test]
fn tampered_bytes_are_rejected() {
    let mut bytes = encode_endorsement(&sample()).expect("encode");
    // Truncation and trailing garbage must both fail the canonical decoder.
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(decode_endorsement(&truncated).is_err());
    bytes.push(0x00);
    assert!(decode_endorsement(&bytes).is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p riot-core --test apps_endorse`
Expected: compile failure — `riot_core::apps::endorse` does not exist.

- [ ] **Step 3: Implement**

Follow `apps/manifest.rs`'s canonical style exactly: definite lengths, ascending integer keys, no trailing bytes, and a final re-encode equality check.

```rust
// crates/riot-core/src/apps/endorse.rs
//! Endorsement marker: a signed "we use this" from an organizer, stored one
//! per (app, endorser subspace) at `app_index_endorsement_path`. Overwrite
//! to update; set `retracted` to withdraw. Canonical minicbor, same rules
//! as `manifest.rs`.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

use super::manifest::AppId;
use super::AppsError;

pub const MAX_ENDORSEMENT_NOTE_BYTES: usize = 200;
pub const MAX_ENDORSEMENT_BYTES: usize = 512;

const FIELD_COUNT: u64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorsementMarker {
    pub app_id: AppId,
    /// Optional short plain-language note ("we ran jail support with
    /// this"); empty string means no note.
    pub note: String,
    pub retracted: bool,
}

fn validate(marker: &EndorsementMarker) -> Result<(), AppsError> {
    if marker.note.len() > MAX_ENDORSEMENT_NOTE_BYTES {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    Ok(())
}

pub fn encode_endorsement(marker: &EndorsementMarker) -> Result<Vec<u8>, AppsError> {
    validate(marker)?;

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(FIELD_COUNT)?;
        e.u8(0)?.bytes(&marker.app_id)?;
        e.u8(1)?.str(&marker.note)?;
        e.u8(2)?.u8(u8::from(marker.retracted))?;
        Ok(())
    })();
    r.map_err(|_| AppsError::EndorsementFieldInvalid)?;

    if buffer.len() > MAX_ENDORSEMENT_BYTES {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    Ok(buffer)
}

pub fn decode_endorsement(input: &[u8]) -> Result<EndorsementMarker, AppsError> {
    if input.len() > MAX_ENDORSEMENT_BYTES {
        return Err(AppsError::EndorsementFieldInvalid);
    }

    let mut d = Decoder::new(input);
    let err = |_| AppsError::EndorsementFieldInvalid;

    if d.map().map_err(err)? != Some(FIELD_COUNT) {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    if d.u8().map_err(err)? != 0 {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    let app_id: AppId = d
        .bytes()
        .map_err(err)?
        .try_into()
        .map_err(|_| AppsError::EndorsementFieldInvalid)?;
    if d.u8().map_err(err)? != 1 {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    if d.datatype().map_err(err)? != Type::String {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    let note = d.str().map_err(err)?.to_string();
    if d.u8().map_err(err)? != 2 {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    let retracted = match d.u8().map_err(err)? {
        0 => false,
        1 => true,
        _ => return Err(AppsError::EndorsementFieldInvalid),
    };

    if d.position() != input.len() {
        return Err(AppsError::EndorsementFieldInvalid);
    }

    let marker = EndorsementMarker { app_id, note, retracted };
    validate(&marker)?;

    // Canonicality proof, same as decode_manifest.
    if encode_endorsement(&marker)? != input {
        return Err(AppsError::EndorsementFieldInvalid);
    }
    Ok(marker)
}
```

Add `pub mod endorse;` to `crates/riot-core/src/apps/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_endorse`
Expected: 5 passed.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/endorse.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_endorse.rs
git commit -m "feat(apps): add endorsement marker codec"
```

---

### Task 3: Pure directory assembly

**Files:**
- Create: `crates/riot-core/src/apps/directory.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — add `pub mod directory;`
- Test: `crates/riot-core/tests/apps_directory.rs`

Pure function, no store dependency — like `trust.rs`. Inputs are already-decoded records; Task 4 produces them from a real store.

Deliberate v1 choice: endorsement *notes* are stored by Task 2's codec but not carried into `EndorsementRecord`/listings — v1 surfaces only who endorses. The native detail page can read notes later without any format change.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_directory.rs
use riot_core::apps::directory::{
    assemble_directory, AppProvenance, DirectoryInputs, EndorsementRecord, IndexedApp,
    SpaceTrust,
};
use riot_core::apps::manifest::AppManifest;
use riot_core::apps::trust::{TrustMarker, TrustMarkerKind};
use riot_core::willow::identity::{AuthorIdentity, NamespaceKind};

fn identity(seed: u8) -> AuthorIdentity {
    AuthorIdentity {
        namespace_id: [seed; 32],
        subspace_id: [seed; 32],
        namespace_kind: NamespaceKind::Communal,
        signing_key_id: [seed; 32],
    }
}

fn manifest(name: &str, author_seed: u8, version: &str) -> AppManifest {
    AppManifest {
        name: name.to_string(),
        description: "Does a thing for your group.".to_string(),
        version: version.to_string(),
        author: identity(author_seed),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    }
}

fn indexed(app_id: [u8; 32], m: AppManifest, carrier: [u8; 32], ts: u64) -> IndexedApp {
    IndexedApp {
        app_id,
        manifest: m,
        bundle_present: true,
        provenance: AppProvenance::Carried { carrier_subspace_id: carrier },
        manifest_timestamp_micros: ts,
    }
}

fn empty_inputs() -> DirectoryInputs {
    DirectoryInputs {
        apps: vec![],
        endorsements: vec![],
        spaces: vec![],
        met_subspace_ids: vec![],
    }
}

#[test]
fn same_app_id_from_two_carriers_lists_once() {
    let m = manifest("Checklist", 1, "1.0.0");
    let mut inputs = empty_inputs();
    inputs.apps = vec![
        indexed([7u8; 32], m.clone(), [2u8; 32], 10),
        indexed([7u8; 32], m, [3u8; 32], 20),
    ];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings.len(), 1);
    assert_eq!(listings[0].app_id, [7u8; 32]);
}

#[test]
fn built_in_provenance_wins_over_carried_for_same_app_id() {
    let m = manifest("Checklist", 1, "1.0.0");
    let mut built_in = indexed([7u8; 32], m.clone(), [0u8; 32], 0);
    built_in.provenance = AppProvenance::BuiltIn;
    let mut inputs = empty_inputs();
    inputs.apps = vec![indexed([7u8; 32], m, [3u8; 32], 20), built_in];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings.len(), 1);
    assert_eq!(listings[0].provenance, AppProvenance::BuiltIn);
}

#[test]
fn same_name_different_author_never_merges() {
    let mut inputs = empty_inputs();
    inputs.apps = vec![
        indexed([1u8; 32], manifest("Shift Signup", 1, "1.0.0"), [9u8; 32], 10),
        indexed([2u8; 32], manifest("Shift Signup", 2, "1.0.0"), [9u8; 32], 10),
    ];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings.len(), 2);
    assert!(listings.iter().all(|l| l.superseded_by.is_none()));
}

#[test]
fn newer_manifest_from_same_author_and_name_supersedes_older() {
    let mut inputs = empty_inputs();
    inputs.apps = vec![
        indexed([1u8; 32], manifest("Checklist", 1, "1.0.0"), [9u8; 32], 10),
        indexed([2u8; 32], manifest("Checklist", 1, "1.1.0"), [9u8; 32], 20),
    ];
    let listings = assemble_directory(&inputs);
    let old = listings.iter().find(|l| l.app_id == [1u8; 32]).expect("old");
    let new = listings.iter().find(|l| l.app_id == [2u8; 32]).expect("new");
    assert_eq!(old.superseded_by, Some([2u8; 32]));
    assert_eq!(new.superseded_by, None);
}

#[test]
fn endorsements_dedup_by_subspace_skip_retracted_and_split_met_unmet() {
    let mut inputs = empty_inputs();
    inputs.apps = vec![indexed([7u8; 32], manifest("Checklist", 1, "1.0.0"), [9u8; 32], 10)];
    inputs.endorsements = vec![
        EndorsementRecord { app_id: [7u8; 32], endorser_subspace_id: [4u8; 32], retracted: false },
        // Same endorser twice: counts once.
        EndorsementRecord { app_id: [7u8; 32], endorser_subspace_id: [4u8; 32], retracted: false },
        // Retracted: does not count.
        EndorsementRecord { app_id: [7u8; 32], endorser_subspace_id: [5u8; 32], retracted: true },
        // Unmet endorser.
        EndorsementRecord { app_id: [7u8; 32], endorser_subspace_id: [6u8; 32], retracted: false },
    ];
    inputs.met_subspace_ids = vec![[4u8; 32]];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings[0].endorsements.met_subspace_ids, vec![[4u8; 32]]);
    assert_eq!(listings[0].endorsements.unmet_count, 1);
}

#[test]
fn trusted_in_reflects_per_space_trust_evaluation() {
    let organizer = [8u8; 32];
    let mut inputs = empty_inputs();
    inputs.apps = vec![indexed([7u8; 32], manifest("Checklist", 1, "1.0.0"), [9u8; 32], 10)];
    inputs.spaces = vec![
        SpaceTrust {
            space_namespace_id: [10u8; 32],
            markers: vec![TrustMarker {
                app_id: [7u8; 32],
                author_subspace_id: organizer,
                kind: TrustMarkerKind::Trust,
                timestamp_micros: 5,
            }],
            organizer_subspace_ids: vec![organizer],
        },
        SpaceTrust {
            space_namespace_id: [11u8; 32],
            markers: vec![],
            organizer_subspace_ids: vec![organizer],
        },
    ];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings[0].trusted_in_spaces, vec![[10u8; 32]]);
}

#[test]
fn listings_sort_by_met_endorsements_then_name() {
    let mut inputs = empty_inputs();
    inputs.apps = vec![
        indexed([1u8; 32], manifest("Zebra Notes", 1, "1.0.0"), [9u8; 32], 10),
        indexed([2u8; 32], manifest("Alpha Notes", 2, "1.0.0"), [9u8; 32], 10),
        indexed([3u8; 32], manifest("Ride Board", 3, "1.0.0"), [9u8; 32], 10),
    ];
    inputs.endorsements = vec![EndorsementRecord {
        app_id: [3u8; 32],
        endorser_subspace_id: [4u8; 32],
        retracted: false,
    }];
    inputs.met_subspace_ids = vec![[4u8; 32]];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings[0].app_id, [3u8; 32]); // endorsed first
    assert_eq!(listings[1].name, "Alpha Notes"); // then name order
    assert_eq!(listings[2].name, "Zebra Notes");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p riot-core --test apps_directory`
Expected: compile failure — `riot_core::apps::directory` does not exist.

- [ ] **Step 3: Implement**

```rust
// crates/riot-core/src/apps/directory.rs
//! Pure directory assembly: decoded index records in, sorted listings out.
//! No store dependency — Task 4's scan produces the inputs. The directory
//! is computed, never stored (see the design spec).

use std::collections::{BTreeMap, BTreeSet};

use crate::willow::identity::AuthorIdentity;

use super::manifest::{AppId, AppManifest};
use super::trust::{is_trusted, TrustMarker};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppProvenance {
    BuiltIn,
    Carried { carrier_subspace_id: [u8; 32] },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedApp {
    pub app_id: AppId,
    pub manifest: AppManifest,
    pub bundle_present: bool,
    pub provenance: AppProvenance,
    /// Willow timestamp of the manifest entry; 0 for built-ins.
    pub manifest_timestamp_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorsementRecord {
    pub app_id: AppId,
    pub endorser_subspace_id: [u8; 32],
    pub retracted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceTrust {
    pub space_namespace_id: [u8; 32],
    pub markers: Vec<TrustMarker>,
    pub organizer_subspace_ids: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryInputs {
    pub apps: Vec<IndexedApp>,
    pub endorsements: Vec<EndorsementRecord>,
    pub spaces: Vec<SpaceTrust>,
    /// Subspaces this phone has actually synced with — endorsers on this
    /// list are named groups; others only bump an anonymous count.
    pub met_subspace_ids: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EndorsementSummary {
    pub met_subspace_ids: Vec<[u8; 32]>,
    pub unmet_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppListing {
    pub app_id: AppId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: AuthorIdentity,
    pub permissions: Vec<String>,
    pub bundle_present: bool,
    pub provenance: AppProvenance,
    pub trusted_in_spaces: Vec<[u8; 32]>,
    pub endorsements: EndorsementSummary,
    /// Set when a manifest with the same (author signing key, name) and a
    /// newer manifest timestamp exists. Never set across different authors —
    /// impersonators don't get to "supersede" anyone.
    pub superseded_by: Option<AppId>,
}

pub fn assemble_directory(inputs: &DirectoryInputs) -> Vec<AppListing> {
    // Dedup by app_id; BuiltIn provenance wins, otherwise first seen wins.
    let mut by_id: BTreeMap<AppId, IndexedApp> = BTreeMap::new();
    for app in &inputs.apps {
        match by_id.get(&app.app_id) {
            Some(existing) if existing.provenance == AppProvenance::BuiltIn => {}
            Some(_) if app.provenance == AppProvenance::BuiltIn => {
                by_id.insert(app.app_id, app.clone());
            }
            Some(_) => {}
            None => {
                by_id.insert(app.app_id, app.clone());
            }
        }
    }

    // Supersession: within (author signing key, name), the newest manifest
    // timestamp wins; every other member points at it.
    let mut newest: BTreeMap<([u8; 32], String), (AppId, u64)> = BTreeMap::new();
    for app in by_id.values() {
        let key = (app.manifest.author.signing_key_id, app.manifest.name.clone());
        let candidate = (app.app_id, app.manifest_timestamp_micros);
        match newest.get(&key) {
            Some((_, ts)) if *ts >= candidate.1 => {}
            _ => {
                newest.insert(key, candidate);
            }
        }
    }

    let met: BTreeSet<[u8; 32]> = inputs.met_subspace_ids.iter().copied().collect();

    let mut listings: Vec<AppListing> = by_id
        .values()
        .map(|app| {
            let key = (app.manifest.author.signing_key_id, app.manifest.name.clone());
            let superseded_by = match newest.get(&key) {
                Some((winner, _)) if *winner != app.app_id => Some(*winner),
                _ => None,
            };

            let mut met_endorsers: BTreeSet<[u8; 32]> = BTreeSet::new();
            let mut unmet_endorsers: BTreeSet<[u8; 32]> = BTreeSet::new();
            for e in &inputs.endorsements {
                if e.app_id != app.app_id || e.retracted {
                    continue;
                }
                if met.contains(&e.endorser_subspace_id) {
                    met_endorsers.insert(e.endorser_subspace_id);
                } else {
                    unmet_endorsers.insert(e.endorser_subspace_id);
                }
            }

            let trusted_in_spaces: Vec<[u8; 32]> = inputs
                .spaces
                .iter()
                .filter(|s| is_trusted(&app.app_id, &s.markers, &s.organizer_subspace_ids))
                .map(|s| s.space_namespace_id)
                .collect();

            AppListing {
                app_id: app.app_id,
                name: app.manifest.name.clone(),
                description: app.manifest.description.clone(),
                version: app.manifest.version.clone(),
                author: app.manifest.author.clone(),
                permissions: app.manifest.permissions.clone(),
                bundle_present: app.bundle_present,
                provenance: app.provenance.clone(),
                trusted_in_spaces,
                endorsements: EndorsementSummary {
                    met_subspace_ids: met_endorsers.into_iter().collect(),
                    unmet_count: unmet_endorsers.len(),
                },
                superseded_by,
            }
        })
        .collect();

    listings.sort_by(|a, b| {
        b.endorsements
            .met_subspace_ids
            .len()
            .cmp(&a.endorsements.met_subspace_ids.len())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.app_id.cmp(&b.app_id))
    });
    listings
}
```

Add `pub mod directory;` to `crates/riot-core/src/apps/mod.rs`.

Note: if `AuthorIdentity` doesn't derive `Clone`/`PartialEq` (check `willow/identity.rs`), adjust — as of this plan's research it derives both.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_directory`
Expected: 7 passed.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/directory.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_directory.rs
git commit -m "feat(apps): add pure directory assembly"
```

---

### Task 4: Index store I/O — publish, endorse, scan

**Files:**
- Modify: `crates/riot-core/src/apps/index.rs` — add `publish_app_index`, `scan_app_index`
- Modify: `crates/riot-core/src/apps/endorse.rs` — add `write_endorsement`
- Test: `crates/riot-core/tests/apps_index_io.rs`

**Before writing any code in this task:** re-run the "payload retrieval reality check" from the preamble and substitute the real API for `payload_for` below. Also mirror the exact signed-entry commit helper the landed `apps/bridge.rs` uses (`AppDataBridge::put`'s body) — extract it into a shared private helper rather than copying it a third time if that's cheap (`apps/mod.rs`-level `fn commit_signed_entry(store, author, entry, payload)`), but do not refactor `bridge.rs` beyond that extraction; it belongs to the core-platform claim.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_index_io.rs
// Setup helpers: follow the committed pattern in crates/riot-core/tests/apps_bridge.rs
// (the core-platform plan's Task 5 test) for opening a session/store and
// generating authors — re-check that file for current signatures first.
use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::directory::AppProvenance;
use riot_core::apps::endorse::{write_endorsement, EndorsementMarker};
use riot_core::apps::index::{app_bundle_digest, publish_app_index, scan_app_index};
use riot_core::apps::manifest::{app_id_for, encode_manifest, AppManifest};
use riot_core::session::RiotSession;
use riot_core::willow::generate_communal_author;

fn sample_pair(author_identity: riot_core::willow::identity::AuthorIdentity) -> (Vec<u8>, Vec<u8>) {
    let bundle = AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<html></html>".to_vec(),
        }],
    };
    let bundle_bytes = encode_app_bundle(&bundle).expect("bundle");
    let manifest = AppManifest {
        name: "Checklist".to_string(),
        description: "Shared to-dos for your group.".to_string(),
        version: "1.0.0".to_string(),
        author: author_identity,
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    };
    let manifest_bytes = encode_manifest(&manifest).expect("manifest");
    (manifest_bytes, bundle_bytes)
}

#[test]
fn publish_then_scan_round_trips_one_app() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("author");
    let dev = generate_communal_author().expect("author");
    let (manifest_bytes, bundle_bytes) = sample_pair(dev.identity());

    let app_id =
        publish_app_index(&store, &carrier, &manifest_bytes, &bundle_bytes, 100).expect("publish");

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.apps.len(), 1);
    assert_eq!(scanned.apps[0].app_id, app_id);
    assert!(scanned.apps[0].bundle_present);
    assert_eq!(
        scanned.apps[0].provenance,
        AppProvenance::Carried {
            carrier_subspace_id: *carrier.subspace_id().as_bytes()
        }
    );
}

#[test]
fn publish_rejects_mismatched_manifest_and_bundle_entry_points() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("author");
    let dev = generate_communal_author().expect("author");
    let (manifest_bytes, _) = sample_pair(dev.identity());
    let other_bundle = encode_app_bundle(&AppBundle {
        entry_point: "main.html".to_string(),
        resources: vec![AppResource {
            path: "main.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<html></html>".to_vec(),
        }],
    })
    .expect("bundle");

    assert!(publish_app_index(&store, &carrier, &manifest_bytes, &other_bundle, 100).is_err());
}

#[test]
fn scan_skips_index_entries_whose_app_id_does_not_match_content() {
    // An adversarial carrier publishing someone's manifest under the wrong
    // app_id path must be invisible, not an error.
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("author");
    let dev = generate_communal_author().expect("author");
    let (manifest_bytes, bundle_bytes) = sample_pair(dev.identity());

    publish_app_index(&store, &carrier, &manifest_bytes, &bundle_bytes, 100).expect("publish");

    // Write a manifest entry at a WRONG app_id path by hand, through the
    // same commit pipeline (mirror the helper publish_app_index uses).
    riot_core::apps::index::publish_manifest_at_for_tests(
        &store,
        &carrier,
        &[0xEE; 32],
        &manifest_bytes,
        101,
    )
    .expect("hand publish");

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.apps.len(), 1, "mismatched entry must be skipped");
}

#[test]
fn endorse_then_scan_surfaces_the_marker_once_per_endorser() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let carrier = generate_communal_author().expect("author");
    let dev = generate_communal_author().expect("author");
    let endorser = generate_communal_author().expect("author");
    let (manifest_bytes, bundle_bytes) = sample_pair(dev.identity());
    let app_id =
        publish_app_index(&store, &carrier, &manifest_bytes, &bundle_bytes, 100).expect("publish");

    let marker = EndorsementMarker { app_id, note: "works great".to_string(), retracted: false };
    write_endorsement(&store, &endorser, &marker, 200).expect("endorse");
    // Overwrite with a retraction: LWW per path means the retraction wins.
    let retract = EndorsementMarker { app_id, note: String::new(), retracted: true };
    write_endorsement(&store, &endorser, &retract, 300).expect("retract");

    let scanned = scan_app_index(&store).expect("scan");
    let records: Vec<_> = scanned
        .endorsements
        .iter()
        .filter(|e| e.app_id == app_id)
        .collect();
    assert_eq!(records.len(), 1);
    assert!(records[0].retracted);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p riot-core --test apps_index_io`
Expected: compile failure — `publish_app_index` etc. do not exist.

- [ ] **Step 3: Implement**

Add to `crates/riot-core/src/apps/index.rs` (signatures; bodies follow the landed `AppDataBridge::put` commit pattern exactly — build entry at the explicit path, authorise, wrap as one-item import bundle, `inspect → plan_all → commit`):

```rust
pub struct ScannedIndex {
    pub apps: Vec<super::directory::IndexedApp>,
    pub endorsements: Vec<super::directory::EndorsementRecord>,
}

/// Validates the manifest/bundle pair (both decode; entry points match;
/// app_id recomputes), then writes both index entries signed by `carrier`.
/// Returns the content-derived app_id.
pub fn publish_app_index(
    store: &EvidenceStore,
    carrier: &EvidenceAuthor,
    manifest_bytes: &[u8],
    bundle_bytes: &[u8],
    willow_timestamp_micros: u64,
) -> Result<AppId, AppsError> {
    let manifest = decode_manifest(manifest_bytes)?;
    let bundle = decode_app_bundle(bundle_bytes)?;
    if manifest.entry_point != bundle.entry_point {
        return Err(AppsError::IndexEntryMismatch);
    }
    let app_id = app_id_for(&manifest, &app_bundle_digest(bundle_bytes))?;
    commit_at(store, carrier, &app_index_manifest_path(&app_id)?, manifest_bytes, willow_timestamp_micros)?;
    commit_at(store, carrier, &app_index_bundle_path(&app_id)?, bundle_bytes, willow_timestamp_micros)?;
    Ok(app_id)
}

/// Test-only hook for writing a manifest at an arbitrary (wrong) app_id
/// path — lets tests prove scan's integrity check without a second
/// commit-pipeline implementation. Not part of the public app API.
pub fn publish_manifest_at_for_tests(
    store: &EvidenceStore,
    carrier: &EvidenceAuthor,
    app_id: &[u8; 32],
    manifest_bytes: &[u8],
    willow_timestamp_micros: u64,
) -> Result<(), AppsError> { /* commit_at(...) with the given path */ }

/// Reads every live `app-index/...` entry, decodes, and integrity-checks.
/// Anything invalid — undecodable payload, app_id mismatch, endorsement at
/// a path whose subspace doesn't match the entry's author — is silently
/// skipped, mirroring the import path's treatment of invalid items.
pub fn scan_app_index(store: &EvidenceStore) -> Result<ScannedIndex, AppsError> {
    // entries_with_prefix(Path::from_slices(&[APP_INDEX_COMPONENT]))
    // For each entry: parse path shape:
    //   [app-index, <32b app_id>, manifest]      -> manifest candidate
    //   [app-index, <32b app_id>, bundle]        -> bundle candidate
    //   [app-index, <32b app_id>, endorsements, <32b subspace>] -> endorsement
    // Fetch payload bytes via the landed payload-retrieval API.
    // Manifest candidates: decode_manifest; hold until pairing with bundle.
    // Bundle candidates: decode_app_bundle (existence + validity only).
    // Pair by path app_id; verify app_id_for(manifest, app_bundle_digest(bundle_bytes)) == path app_id;
    //   IndexedApp { provenance: Carried { carrier_subspace_id: entry.subspace_id } ,
    //                manifest_timestamp_micros: entry.timestamp(), bundle_present }
    //   A manifest with NO bundle entry yet is still listed (bundle_present: false —
    //   the "Still arriving from your group…" state) but only if its app_id can't be
    //   verified without the bundle — so: manifest-only entries are listed unverified?
    //   NO: without bundle bytes the app_id cannot be recomputed. List manifest-only
    //   candidates ONLY when the path app_id will be verified later at launch; for v1
    //   directory purposes include them with bundle_present=false and skip verification,
    //   but NEVER mark them launchable. Trust decisions still key on app_id which the
    //   bundle will be checked against at launch time by the runtime host.
    // Endorsements: decode_endorsement; skip when marker.app_id != path app_id
    //   OR entry author subspace != path subspace component (spoofed slot).
}
```

And in `crates/riot-core/src/apps/endorse.rs`:

```rust
/// Writes the endorser's marker at their own endorsement slot for the app.
pub fn write_endorsement(
    store: &EvidenceStore,
    endorser: &EvidenceAuthor,
    marker: &EndorsementMarker,
    willow_timestamp_micros: u64,
) -> Result<(), AppsError> {
    let payload = encode_endorsement(marker)?;
    let path = super::index::app_index_endorsement_path(
        &marker.app_id,
        endorser.subspace_id().as_bytes(),
    )?;
    // commit_at(store, endorser, &path, &payload, willow_timestamp_micros)
}
```

The commented `scan_app_index` sketch above contains one deliberate policy decision spelled out inline: **manifest-only apps (bundle not yet synced) are listed with `bundle_present: false` and skip app-id verification, but are never launchable.** Full verification happens whenever the bundle is present; the runtime host re-verifies at launch regardless. Implement exactly that.

`subspace_id().as_bytes()` — confirm the real accessor for raw subspace bytes on `SubspaceId` (check how `apps/trust.rs` tests extract it: `*author.subspace_id().as_bytes()`); adjust if the landed helper differs.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_index_io`
Expected: 4 passed.

- [ ] **Step 5: Full workspace check, clippy, commit**

Run: `cargo test --workspace --all-features`
Expected: all green, no regressions.

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/index.rs crates/riot-core/src/apps/endorse.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_index_io.rs
git commit -m "feat(apps): add app-index publish/endorse/scan store I/O"
```

---

### Task 5: Starter catalog

**Files:**
- Create: `crates/riot-core/src/apps/starter.rs`
- Modify: `crates/riot-core/src/apps/mod.rs` — add `pub mod starter;`
- Test: `crates/riot-core/tests/apps_starter.rs`

The mechanism ships now; the real embedded checklist ships with the WebView-runtime follow-up plan (there's nothing to launch it with yet). `STARTER_CATALOG` is therefore an **empty slice in this task** — the tests exercise the verification function with generated pairs, including a corrupted one.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/apps_starter.rs
use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::directory::AppProvenance;
use riot_core::apps::manifest::{encode_manifest, AppManifest};
use riot_core::apps::starter::{verify_starter_catalog, STARTER_CATALOG};
use riot_core::willow::generate_communal_author;

fn pair(name: &str) -> (Vec<u8>, Vec<u8>) {
    let author = generate_communal_author().expect("author");
    let bundle_bytes = encode_app_bundle(&AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<html></html>".to_vec(),
        }],
    })
    .expect("bundle");
    let manifest_bytes = encode_manifest(&AppManifest {
        name: name.to_string(),
        description: "Built-in tool.".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    })
    .expect("manifest");
    (manifest_bytes, bundle_bytes)
}

#[test]
fn valid_pairs_verify_with_built_in_provenance_and_zero_timestamp() {
    let (m, b) = pair("Checklist");
    let apps = verify_starter_catalog(&[(&m, &b)]);
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].provenance, AppProvenance::BuiltIn);
    assert_eq!(apps[0].manifest_timestamp_micros, 0);
    assert!(apps[0].bundle_present);
}

#[test]
fn corrupted_built_in_is_silently_excluded() {
    let (m, b) = pair("Checklist");
    let mut corrupt = b.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0xFF;
    let apps = verify_starter_catalog(&[(&m, &corrupt)]);
    assert!(apps.is_empty(), "corrupt bundle must be excluded, not an error");
}

#[test]
fn the_shipped_catalog_verifies_completely() {
    // Guards the embedded catalog forever: every shipped pair must verify.
    let apps = verify_starter_catalog(STARTER_CATALOG);
    assert_eq!(apps.len(), STARTER_CATALOG.len());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p riot-core --test apps_starter`
Expected: compile failure — `riot_core::apps::starter` does not exist.

- [ ] **Step 3: Implement**

```rust
// crates/riot-core/src/apps/starter.rs
//! Built-in starter catalog. Built-ins are ordinary manifest+bundle pairs
//! run through the exact same decode/verify path as synced apps — "Built
//! into Riot" is a provenance label, not a trust shortcut, and a built-in
//! still needs an organizer's per-space trust decision to launch.
//!
//! The catalog is empty until the WebView runtime lands (nothing could
//! launch a built-in yet); the checklist app's pair arrives with that
//! follow-up, generated by `riot-app pack` and committed under
//! `fixtures/apps/`.

use super::bundle::decode_app_bundle;
use super::directory::{AppProvenance, IndexedApp};
use super::index::app_bundle_digest;
use super::manifest::{app_id_for, decode_manifest};

/// (manifest_bytes, bundle_bytes) pairs embedded at compile time.
pub const STARTER_CATALOG: &[(&[u8], &[u8])] = &[];

/// Decodes and integrity-checks every pair; invalid pairs are silently
/// excluded, mirroring the import path's treatment of invalid items.
pub fn verify_starter_catalog(pairs: &[(&[u8], &[u8])]) -> Vec<IndexedApp> {
    pairs
        .iter()
        .filter_map(|(manifest_bytes, bundle_bytes)| {
            let manifest = decode_manifest(manifest_bytes).ok()?;
            let bundle = decode_app_bundle(bundle_bytes).ok()?;
            if manifest.entry_point != bundle.entry_point {
                return None;
            }
            let app_id = app_id_for(&manifest, &app_bundle_digest(bundle_bytes)).ok()?;
            Some(IndexedApp {
                app_id,
                manifest,
                bundle_present: true,
                provenance: AppProvenance::BuiltIn,
                manifest_timestamp_micros: 0,
            })
        })
        .collect()
}
```

Add `pub mod starter;` to `crates/riot-core/src/apps/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p riot-core --test apps_starter`
Expected: 3 passed.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-core/src/apps/starter.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_starter.rs
git commit -m "feat(apps): add starter-catalog verification"
```

---

### Task 6: FFI surface — directory listings, share, endorse

**Files:**
- Modify: `crates/riot-ffi/src/apps_ffi.rs` (created by core-platform Task 6)
- Modify: `crates/riot-ffi/src/mobile_state.rs` — thin `with_active` delegators
- Test: match the FFI contract-test convention the core-platform Task 6 established (locate its tests first; add alongside)

- [ ] **Step 1: Re-confirm the landed FFI shape**

Read `crates/riot-ffi/src/apps_ffi.rs` and the core-platform Task 6 additions to `mobile_state.rs` in full. Confirm: the session-object pattern (`AppRuntimeSession` or whatever actually landed), `MobileError`'s app-related variant, how spaces are identified across the FFI boundary (`PublicSpace` in `mobile_api.rs`), and how `mobile_state` resolves a space to a store + author. Everything below adapts to those names.

- [ ] **Step 2: Write failing FFI contract tests**

Exercise end-to-end through the FFI layer (not core — Task 4 covered that): publish an app pair (via the same install path core Task 6 exposed), then `directory_listings()` returns one listing with the right name/provenance/trust state; `endorse_app` bumps the listing's endorsement summary; `share_app` into a second profile's context makes it appear in that profile's listings after a sync round (reuse whatever two-profile sync harness the existing FFI sync tests use — check `crates/riot-ffi/` tests for the `MobileSyncSession` pattern before inventing one).

- [ ] **Step 3: Implement the FFI records and methods**

UniFFI records mirror `AppListing` flattened to FFI-friendly types (match existing record conventions in `mobile_api.rs` — `Vec<u8>` for 32-byte ids, `String` for text):

```rust
#[derive(uniffi::Record)]
pub struct DirectoryListing {
    pub app_id: Vec<u8>,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author_signing_key_id: Vec<u8>,
    pub permissions: Vec<String>,
    pub bundle_present: bool,
    pub built_in: bool,
    pub carrier_subspace_id: Option<Vec<u8>>,
    pub trusted_in_spaces: Vec<Vec<u8>>,
    pub endorsing_met_subspaces: Vec<Vec<u8>>,
    pub endorsing_unmet_count: u32,
    pub superseded_by: Option<Vec<u8>>,
}
```

Methods (each a thin delegator into `mobile_state.rs`, which calls `scan_app_index` + `verify_starter_catalog` + `assemble_directory` / `publish_app_index` / `write_endorsement`):
- `directory_listings() -> Result<Vec<DirectoryListing>, MobileError>`
- `share_app(app_id: Vec<u8>, space: PublicSpace) -> Result<(), MobileError>` — looks up the app's manifest/bundle bytes from the local store, re-publishes them via `publish_app_index` with the caller as carrier in that space's context.
- `endorse_app(app_id: Vec<u8>, note: String, retract: bool) -> Result<(), MobileError>`

`met_subspace_ids` for the assembly call: the set of subspaces present among the store's live entries (derivable from `entries_with_prefix` on the empty path or an existing session accessor — check what `mobile_state.rs` already tracks for peers/authors before adding anything new).

- [ ] **Step 4: Run tests, bindings, clippy, commit**

Run: `cargo test -p riot-ffi --all-features`
Expected: all green.

Run: `cargo xtask generate-bindings` and `cargo xtask validate-contracts`
Expected: non-empty output, PASS.

Run: `cargo clippy -p riot-ffi --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-ffi/src/apps_ffi.rs crates/riot-ffi/src/mobile_state.rs
git commit -m "feat(ffi): expose app directory listings, share, endorse"
```

---

### Task 7: `riot-app` CLI — keygen, pack, inspect

**Files:**
- Create: `crates/riot-app-cli/Cargo.toml`
- Create: `crates/riot-app-cli/src/main.rs`
- Create: `crates/riot-app-cli/src/lib.rs` (logic lives here so tests don't shell out)
- Create: `crates/riot-app-cli/tests/cli_pack.rs`
- Create: `crates/riot-app-cli/tests/fixtures/hello-app/` (riot-app.json, index.html, app.js)
- Modify: root `Cargo.toml` — add `"crates/riot-app-cli"` to workspace members

No new dependency pins: argument parsing is hand-rolled `std::env::args` matching `crates/xtask`'s style; JSON input uses the existing `serde_json` workspace pin; everything else comes from `riot-core`.

**Key handling:** `keygen` writes two files with a loud plain-language warning: `author.wrapkey` (32 random bytes, hex, mode 0600) and `author.sealed` (`EvidenceAuthor::seal_identity` output). `pack` reads both via `open_sealed_identity`. This reuses the existing sealed-identity format rather than inventing key serialization.

**`riot-app.json` format** (the developer-facing manifest source; author identity comes from the key, never from the JSON):

```json
{
  "name": "Hello App",
  "description": "Says hello to your group.",
  "version": "1.0.0",
  "entry_point": "index.html",
  "permissions": ["own-app-data"]
}
```

- [ ] **Step 1: Create the crate skeleton and register it**

`crates/riot-app-cli/Cargo.toml`:

```toml
[package]
name = "riot-app-cli"
version = "0.1.0"
edition = "2021"
publish = false

[[bin]]
name = "riot-app"
path = "src/main.rs"

[dependencies]
riot-core = { path = "../riot-core" }
serde_json = { workspace = true }
rand_core = { workspace = true }
ed25519-dalek = { workspace = true }

[dev-dependencies]
tempfile = "=3.14.0"
```

(Check whether `tempfile` is already pinned anywhere in the workspace before adding; if another crate's dev-dependencies pin a different version, match it.)

Add `"crates/riot-app-cli"` to the root `Cargo.toml` members array.

Run: `cargo check -p riot-app-cli`
Expected: compiles (empty main).

- [ ] **Step 2: Write the failing pack tests**

```rust
// crates/riot-app-cli/tests/cli_pack.rs
use riot_app_cli::{content_type_for, pack, PackError, PackInput};
use riot_core::apps::bundle::decode_app_bundle;
use riot_core::apps::index::app_bundle_digest;
use riot_core::apps::manifest::{app_id_for, decode_manifest};
use riot_core::willow::generate_communal_author;

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app")
}

#[test]
fn pack_produces_verifiable_manifest_and_bundle() {
    let author = generate_communal_author().expect("author");
    let output = pack(PackInput {
        app_dir: fixture_dir(),
        author: &author,
        timestamp_micros: 1_000,
    })
    .expect("pack");

    let manifest = decode_manifest(&output.manifest_bytes).expect("manifest decodes");
    let bundle = decode_app_bundle(&output.bundle_bytes).expect("bundle decodes");
    assert_eq!(manifest.name, "Hello App");
    assert_eq!(bundle.entry_point, "index.html");
    assert_eq!(
        output.app_id,
        app_id_for(&manifest, &app_bundle_digest(&output.bundle_bytes)).expect("id")
    );
    // The emitted import bundle is the standard RIOTE1 format.
    assert_eq!(&output.import_bundle_bytes[..6], b"RIOTE1");
}

#[test]
fn app_id_is_stable_for_identical_input_and_key() {
    let author = generate_communal_author().expect("author");
    let a = pack(PackInput { app_dir: fixture_dir(), author: &author, timestamp_micros: 1_000 })
        .expect("pack");
    let b = pack(PackInput { app_dir: fixture_dir(), author: &author, timestamp_micros: 2_000 })
        .expect("pack");
    // Timestamps affect entries, never identity.
    assert_eq!(a.app_id, b.app_id);
}

#[test]
fn unknown_extension_is_rejected_with_the_file_named() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("riot-app.json"), include_str!("fixtures/hello-app/riot-app.json")).unwrap();
    std::fs::write(dir.path().join("index.html"), "<html></html>").unwrap();
    std::fs::write(dir.path().join("movie.mp4"), [0u8; 4]).unwrap();
    let author = generate_communal_author().expect("author");
    let err = pack(PackInput { app_dir: dir.path().to_path_buf(), author: &author, timestamp_micros: 0 })
        .expect_err("must reject");
    match err {
        PackError::UnsupportedFile(name) => assert!(name.contains("movie.mp4")),
        other => panic!("wrong error: {other:?}"),
    }
}

#[test]
fn oversized_app_reports_actual_and_limit_bytes() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("riot-app.json"), include_str!("fixtures/hello-app/riot-app.json")).unwrap();
    std::fs::write(dir.path().join("index.html"), vec![b'x'; 1_100_000]).unwrap();
    let author = generate_communal_author().expect("author");
    let err = pack(PackInput { app_dir: dir.path().to_path_buf(), author: &author, timestamp_micros: 0 })
        .expect_err("must reject");
    match err {
        PackError::TooLarge { actual, limit } => {
            assert!(actual > limit);
            assert_eq!(limit, riot_core::apps::bundle::MAX_BUNDLE_TOTAL_BYTES);
        }
        other => panic!("wrong error: {other:?}"),
    }
}

#[test]
fn content_types_cover_the_v1_set() {
    assert_eq!(content_type_for("index.html"), Some("text/html"));
    assert_eq!(content_type_for("app.js"), Some("text/javascript"));
    assert_eq!(content_type_for("style.css"), Some("text/css"));
    assert_eq!(content_type_for("data.json"), Some("application/json"));
    assert_eq!(content_type_for("icon.png"), Some("image/png"));
    assert_eq!(content_type_for("logo.svg"), Some("image/svg+xml"));
    assert_eq!(content_type_for("movie.mp4"), None);
}
```

Fixture files:

```json
// crates/riot-app-cli/tests/fixtures/hello-app/riot-app.json
{
  "name": "Hello App",
  "description": "Says hello to your group.",
  "version": "1.0.0",
  "entry_point": "index.html",
  "permissions": ["own-app-data"]
}
```

```html
<!-- crates/riot-app-cli/tests/fixtures/hello-app/index.html -->
<html><body><h1>Hello</h1><script src="app.js"></script></body></html>
```

```js
// crates/riot-app-cli/tests/fixtures/hello-app/app.js
console.log("hello");
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p riot-app-cli`
Expected: compile failure — `riot_app_cli::pack` does not exist.

- [ ] **Step 4: Implement the library**

```rust
// crates/riot-app-cli/src/lib.rs
//! Packs a folder of HTML/CSS/JS into a signed, distributable Riot app:
//! validates riot-app.json + resources, encodes the AppBundle, derives the
//! app_id, builds the two app-index entries signed by the developer's
//! author, and emits them as one standard RIOTE1 import bundle — the same
//! format every other import path already accepts.

use std::path::PathBuf;

use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource, MAX_BUNDLE_TOTAL_BYTES};
use riot_core::apps::index::{
    app_bundle_digest, app_index_bundle_path, app_index_manifest_path,
};
use riot_core::apps::manifest::{app_id_for, encode_manifest, AppManifest};
use riot_core::willow::identity::EvidenceAuthor;

pub struct PackInput<'a> {
    pub app_dir: PathBuf,
    pub author: &'a EvidenceAuthor,
    pub timestamp_micros: u64,
}

pub struct PackOutput {
    pub app_id: [u8; 32],
    pub manifest_bytes: Vec<u8>,
    pub bundle_bytes: Vec<u8>,
    /// RIOTE1 import bundle holding the two signed app-index entries.
    pub import_bundle_bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum PackError {
    MissingManifest,
    ManifestJsonInvalid(String),
    UnsupportedFile(String),
    TooLarge { actual: usize, limit: usize },
    Core(riot_core::apps::AppsError),
    Io(std::io::Error),
}

pub fn content_type_for(file_name: &str) -> Option<&'static str> {
    let ext = file_name.rsplit('.').next()?;
    match ext {
        "html" => Some("text/html"),
        "js" => Some("text/javascript"),
        "css" => Some("text/css"),
        "json" => Some("application/json"),
        "png" => Some("image/png"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

pub fn pack(input: PackInput<'_>) -> Result<PackOutput, PackError> {
    // 1. Read + parse riot-app.json (serde_json::Value; reject unknown
    //    top-level keys, require the five known ones; author identity
    //    comes from input.author.identity(), never from JSON).
    // 2. Walk the directory (flat + subdirs, sorted for determinism;
    //    skip riot-app.json itself). For each file: content_type_for or
    //    PackError::UnsupportedFile; accumulate size, early TooLarge
    //    check against MAX_BUNDLE_TOTAL_BYTES before encoding.
    // 3. Build AppBundle (resource paths relative, '/'-separated),
    //    encode_app_bundle, app_bundle_digest, AppManifest, encode_manifest,
    //    app_id_for.
    // 4. Build the two entries at app_index_manifest_path/bundle_path with
    //    Entry::builder() (namespace/subspace from input.author, exactly the
    //    pattern in apps/entry.rs::build_app_data_entry), authorise_entry,
    //    wrap both as SignedWillowEntry, encode_bundle -> import_bundle_bytes.
    //    (This mirrors the commit-side wrapping in apps/bridge.rs::put; the
    //    CLI just stops before inspect/commit and writes the bytes to disk.)
}
```

`main.rs` is a thin shell: subcommand dispatch (`keygen`, `pack`, `inspect`), file I/O, and plain-language error printing (`"bundle is 1.4 MB; limit is 1 MB"` — format bytes as MB with one decimal). `inspect <file>` decodes an emitted import bundle and prints name, version, author key id hex, app_id hex, resource list. `keygen --out <dir>` as described in the task header. No tests for `main.rs` beyond `cargo build`; all logic that matters is in the library.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p riot-app-cli`
Expected: 6 passed.

- [ ] **Step 6: Prove the CLI output imports cleanly (integration)**

Add to `crates/riot-app-cli/tests/cli_pack.rs`:

```rust
#[test]
fn packed_output_commits_through_the_real_import_pipeline_and_scans_back() {
    use riot_core::session::{ImportContext, InspectOutcome, RiotSession};

    let author = generate_communal_author().expect("author");
    let output = pack(PackInput { app_dir: fixture_dir(), author: &author, timestamp_micros: 1_000 })
        .expect("pack");

    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let preview = match store
        .inspect(&output.import_bundle_bytes, ImportContext::new("riot-app-pack"))
        .expect("inspect")
    {
        InspectOutcome::Preview(p) => p,
        InspectOutcome::Rejected(r) => panic!("rejected: {r:?}"),
    };
    preview.plan_all().expect("plan").commit().expect("commit");

    let scanned = riot_core::apps::index::scan_app_index(&store).expect("scan");
    assert_eq!(scanned.apps.len(), 1);
    assert_eq!(scanned.apps[0].app_id, output.app_id);
}
```

(Adjust `ImportContext`/`InspectOutcome` names to the landed session API — same correction rule as Task 4.)

Run: `cargo test -p riot-app-cli`
Expected: 7 passed. This single test is the plan's proof that "publishing invents no new transport."

- [ ] **Step 7: Full workspace check, clippy, commit**

Run: `cargo test --workspace --all-features`
Expected: all green.

Run: `cargo clippy -p riot-app-cli --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/riot-app-cli/ Cargo.toml
git commit -m "feat(cli): add riot-app pack/keygen/inspect publishing tool"
```

---

## After this plan lands

1. Update this plan's `COLLABORATION.md` claim row to **Done** with the final commit list and `cargo test --workspace --all-features` result.
2. Everything here is `cargo test`-verifiable — no simulator/emulator needed.
3. Follow-up plans (write fresh, in order):
   - **Native storefront UI** (iOS `apps/ios/Riot/Directory/`, Android equivalent): storefront screen, review/detail page, space picker, Tools row — consuming Task 6's FFI surface. Blocked on nothing else in this plan, but launching apps also needs:
   - **WebView runtime + checklist app** (already named as the core-platform plan's follow-up). Once the checklist exists, generate its pair with `riot-app pack` using the fixed Riot project author key, commit under `fixtures/apps/checklist/`, and flip `STARTER_CATALOG` from `&[]` to include it — Task 5's `the_shipped_catalog_verifies_completely` test guards it from then on.
