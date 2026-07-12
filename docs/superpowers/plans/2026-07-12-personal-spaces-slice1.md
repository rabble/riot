# Personal Spaces — Slice 1 ("Make your public page") Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a person create a personal space (an *owned* Willow namespace they hold root over), author a page as a signed `kind: page` bundle, publish it, and have it render — publicly — on another nearby device, with every containment guarantee from the design's D3/D4 holding.

**Architecture:** Personal spaces are owned Willow namespaces (`willow25` ships `ReadCapability`/`WriteCapability::new_owned` and owned-namespace generation; Riot just never used them). The write path, today hardcoded to `new_communal`, is generalized to thread an owned write capability through `authorise_entry`/`commit_at`/`publish_app_index`. Willow paths gain a leading visibility segment (`pub`/`con`) enforced by one shared admission source of truth across local-write, the app-path classifiers, and the import gate. The root `NamespaceSecret` lives in platform secure storage and crosses FFI only as a sealed blob. Foreign `kind: page` bundles mount deny-closed (no bridge).

**Tech Stack:** Rust (`riot-core`, `riot-ffi`), `willow25 =0.6.0-alpha.3`, UniFFI, Swift/SwiftUI (`apps/ios`), WKWebView, XChaCha20-Poly1305 (sealed identity), SQLite (multi-space store — dependency, see below).

---

## Blocking dependency (read first)

Slice 1's cross-device demo depends on the **multi-space SQLite store**
(`docs/superpowers/plans/2026-07-12-multi-space-sqlite-store.md`), which is
**documentation-only — zero lines implemented** and exposes **no consumer trait
to pre-target**; it is a concrete store that *replaces* the current in-memory
one. Its API names are also **not yet reconciled** (the design spec says
`RiotDatabase`; the plan's Task 8 says `DatabaseSession`).

Consequence for this plan:

- **Phases A–D (Tasks 1–11) are store-independent** — `riot-core`/`riot-ffi`
  Rust plus iOS **and Android** containment work. They are fully TDD-able and may
  begin immediately; nothing here waits on SQLite.
- **Phase E (Tasks 12–17) is store-dependent.** Its tasks name the exact store
  API calls they will use, but their final step-level code is **finalized when
  the store's native-API task and iOS-cutover task land and the
  `RiotDatabase`/`DatabaseSession` naming is reconciled.** Do not implement
  Phase E against a guessed API surface.

Before cutting Phase E work units, confirm the store's `create` / `open_space` /
`list_spaces` / `AppSession.put_document` signatures are merged and stable.

---

## File structure

**Rust core — new files:**
- `crates/riot-core/src/willow/owned.rs` — owned-namespace generation, `NamespaceKind::Owned`, the owned root author, owned write-capability minting, owned sealed envelope. Kept separate from `identity.rs` so the communal sealed-identity invariants are not disturbed.
- `crates/riot-core/src/apps/visibility.rs` — the `Visibility` enum and the single `VISIBILITY_SEGMENTS` source of truth consumed by every admission gate.
- `crates/riot-core/src/apps/page.rs` — `kind: page` manifest recognition and page publication (`pub/page/current`).

**Rust core — modified files:**
- `crates/riot-core/src/willow/identity.rs` — add `NamespaceKind::Owned`; leave communal paths untouched.
- `crates/riot-core/src/willow/mod.rs` — `authorise_entry` accepts a supplied capability; export owned types.
- `crates/riot-core/src/apps/entry.rs` — `app_data_path`/`is_app_data_path` carry a visibility segment.
- `crates/riot-core/src/apps/index.rs` — `app_index_*`/`classify_app_index_path` carry a visibility segment.
- `crates/riot-core/src/apps/bridge.rs` — bridge writes are visibility-scoped.
- `crates/riot-core/src/import/bundle.rs` — `verify_frame` admits owned namespaces + the visibility segment (one source of truth with the classifiers).

**Rust FFI — modified:**
- `crates/riot-ffi/src/mobile_api.rs` — `PersonalSpace` record; `kind` field.
- `crates/riot-ffi/src/mobile_state.rs` — create-personal-space; owned root custody across FFI.

**iOS — new files:**
- `apps/ios/Riot/Onboarding/OnboardingGate.swift` — first-run name+space gate.
- `apps/ios/Riot/Pages/PageAuthoringView.swift` — template gallery + source editor.
- `apps/ios/Riot/Pages/PageTemplates.swift` — the built-in gaudy templates.
- `apps/ios/Riot/Apps/ForeignPageRuntime.swift` — deny-closed mount for foreign `kind: page` bundles.
- `apps/ios/Riot/Apps/AppNetworkBackstop.swift` — the iOS network backstop (sole-loader + covert-channel denial).

**iOS — modified:**
- `apps/ios/Riot/Apps/AppRuntimeView.swift` — route `kind: page` foreign bundles to the deny-closed runtime; install the backstop.
- `apps/ios/Riot/AppModel.swift`, `ConferenceShellView.swift`, `Core/ProfileRepository.swift` — personal-space creation, Spaces-tab card, distinct entry points, no privacy control.

---

## Phase A — Owned namespace primitives (store-independent)

### Task 1: `NamespaceKind::Owned` and owned-namespace generation

**Files:**
- Modify: `crates/riot-core/src/willow/identity.rs:23-26`
- Create: `crates/riot-core/src/willow/owned.rs`
- Modify: `crates/riot-core/src/willow/mod.rs:30-33` (module + exports)
- Test: `crates/riot-core/src/willow/owned.rs` (`#[cfg(test)]` module)

- [ ] **Step 1: Write the failing test** — `crates/riot-core/src/willow/owned.rs`:

```rust
//! Owned personal-space primitives. Kept out of `identity.rs` so the communal
//! sealed-identity invariants there are never accidentally loosened.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_owned_namespace_reports_owned() {
        let root = OwnedRoot::generate().expect("entropy");
        assert!(root.namespace_id().is_owned());
        assert!(!root.namespace_id().is_communal());
    }

    #[test]
    fn communal_and_owned_are_disjoint() {
        // A communal author's namespace is never owned, and vice versa.
        let communal = crate::willow::generate_space_organizer_author().expect("entropy");
        assert!(communal.identity().namespace_id.is_communal());
        let owned = OwnedRoot::generate().expect("entropy");
        assert!(owned.namespace_id().is_owned());
    }
}
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cargo test -p riot-core willow::owned -- --nocapture`
Expected: FAIL — `OwnedRoot` undefined.

- [ ] **Step 3: Add the `Owned` variant** — `crates/riot-core/src/willow/identity.rs:23-26`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespaceKind {
    Communal,
    Owned,
}
```

- [ ] **Step 4: Implement `OwnedRoot::generate`** — top of `crates/riot-core/src/willow/owned.rs`:

```rust
use willow25::prelude::*;
use zeroize::Zeroize;

use super::identity::os_fill;
use super::{NamespaceId, WillowError};

/// Custodian of a personal space: holds the owned namespace root secret. The
/// namespace ID is the root public key. Not `Clone`, not `Debug` — the root
/// secret must never be duplicated or printed.
pub struct OwnedRoot {
    namespace_id: NamespaceId,
    namespace_secret: NamespaceSecret,
}

impl OwnedRoot {
    /// Draws namespace candidates until the public key is owned (odd final
    /// byte). Each rejected draw is zeroized. Unlike communal generation, the
    /// secret is RETAINED — it is the space's root authority.
    pub fn generate() -> Result<Self, WillowError> {
        for _ in 0..128 {
            let mut secret_bytes = [0u8; 32];
            let result = os_fill(&mut secret_bytes);
            let secret = result.map(|()| NamespaceSecret::from_bytes(&secret_bytes));
            secret_bytes.zeroize();
            let secret = secret?;
            let namespace_id = secret.corresponding_namespace_id();
            if namespace_id.is_owned() {
                return Ok(Self { namespace_id, namespace_secret: secret });
            }
        }
        Err(WillowError::EntropyUnavailable)
    }

    pub fn namespace_id(&self) -> &NamespaceId {
        &self.namespace_id
    }
}
```

Notes (verified against the pinned crate):
- `os_fill` is currently private to `identity.rs` — change its declaration to `pub(crate) fn os_fill`.
- `NamespaceSecret::corresponding_namespace_id()` (`namespace_secret.rs:54`), `NamespaceSecret::from_bytes` (`:73`), and `NamespaceId::is_owned()` (`namespace_id.rs:104`, = odd final byte) all exist.
- willow25 **already ships `randomly_generate_owned_namespace()`** (`namespace_secret.rs:143`). Prefer calling it over re-rolling the loop above; the hand-rolled version is shown only to make the retained-secret contrast explicit. Whichever is used, the secret is retained (not zeroized).
- `AuthorIdentity::identity()` hardcodes `namespace_kind: NamespaceKind::Communal` (`identity.rs:74`). When Task 2 adds the owned author, update `identity()` so an owned author reports `NamespaceKind::Owned` — otherwise the kind is latent-wrong.

- [ ] **Step 5: Wire the module** — `crates/riot-core/src/willow/mod.rs` (after the existing `mod identity;`):

```rust
mod owned;
pub use owned::OwnedRoot;
```

- [ ] **Step 6: Run tests, verify pass**

Run: `cargo test -p riot-core willow::owned`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/riot-core/src/willow/owned.rs crates/riot-core/src/willow/identity.rs crates/riot-core/src/willow/mod.rs
git commit -m "feat(willow): NamespaceKind::Owned + OwnedRoot owned-namespace generation"
```

---

### Task 2: Owned write capability + owned root author

**Files:**
- Modify: `crates/riot-core/src/willow/owned.rs`
- Test: same file's test module

- [ ] **Step 1: Write the failing test** — add to `owned.rs` tests:

```rust
#[test]
fn owned_root_mints_write_cap_over_its_namespace() {
    let root = OwnedRoot::generate().expect("entropy");
    let author = root.author().expect("author");
    let cap = author.write_capability();
    // An owned write capability is authored over the owned namespace and is
    // NOT a communal capability.
    assert!(cap.granted_namespace().is_owned());
}

#[test]
fn owned_author_signs_a_committed_entry_in_its_own_subspace() {
    let root = OwnedRoot::generate().expect("entropy");
    let author = root.author().expect("author");
    assert_eq!(author.identity().namespace_id, *root.namespace_id());
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p riot-core willow::owned`
Expected: FAIL — `root.author()` / owned `write_capability` undefined.

- [ ] **Step 3: Implement `OwnedSpaceAuthor` + `OwnedRoot::author`** — append to `owned.rs`:

```rust
use willow25::authorisation::WriteCapability;

use super::identity::EvidenceAuthor;

/// The person's signing author *inside* their owned space. Carries the author
/// subspace secret and a delegated owned write capability so `authorise_entry`
/// can mint tokens exactly as it does for communal authors.
pub struct OwnedSpaceAuthor {
    inner: EvidenceAuthor,
    write_capability: WriteCapability,
}

impl OwnedRoot {
    /// Derive the author for this device's own subspace in the owned space and
    /// delegate it an owned write capability from the root.
    pub fn author(&self) -> Result<OwnedSpaceAuthor, WillowError> {
        let inner = EvidenceAuthor::generate_in_namespace(self.namespace_id.clone())?;
        // Root grants this subspace a write cap over the full namespace
        // (Area::full()); Slice 2 reads narrow by path prefix. `new_owned` takes
        // the keypair BY REFERENCE and returns `Self` — no Result, no `?`.
        // Signature (write_capability.rs:250):
        //   new_owned<K: Signer<NamespaceSignature> + Keypair<VerifyingKey=NamespaceId>>(
        //       keypair: &K, user_key: SubspaceId) -> Self
        // NamespaceSecret satisfies those bounds (namespace_secret.rs:155,163).
        let write_capability =
            WriteCapability::new_owned(&self.namespace_secret, inner.subspace_id());
        Ok(OwnedSpaceAuthor { inner, write_capability })
    }
}

impl OwnedSpaceAuthor {
    pub fn identity(&self) -> super::identity::AuthorIdentity {
        self.inner.identity()
    }
    pub fn write_capability(&self) -> WriteCapability {
        self.write_capability.clone()
    }
    /// The single accessor name used everywhere (Tasks 3 and 6). Do not
    /// introduce a second `_for_test` variant — one name avoids a compile break.
    pub(crate) fn evidence_author(&self) -> &EvidenceAuthor {
        &self.inner
    }
}
```

Note: `EvidenceAuthor::generate_in_namespace(NamespaceId)` is new — a sibling of
the existing `generate_communal_author_for_namespace` that does NOT require the
namespace to be communal. Add it to `identity.rs` next to that function, minting
only the subspace secret (the namespace ID is supplied). Do not route owned
namespaces through the communal factory, which rejects non-communal IDs.

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p riot-core willow::owned`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/willow/owned.rs crates/riot-core/src/willow/identity.rs
git commit -m "feat(willow): owned write-capability minting via new_owned + owned space author"
```

---

### Task 3: Capability-threading refactor (`authorise_entry` accepts a supplied capability)

This is the CTO's designated **first structural work unit** — land it before any
UX builds on the owned write path.

**Files:**
- Modify: `crates/riot-core/src/willow/mod.rs:102-113`
- Modify: `crates/riot-core/src/session.rs:771` (`commit_at` → `commit_at_with` + wrapper)
- Modify: `crates/riot-core/src/apps/index.rs:86` (`publish_app_index_with`)
- Test: `crates/riot-core/tests/owned_write_path.rs` (new integration test)

- [ ] **Step 1: Write the failing test** — `crates/riot-core/tests/owned_write_path.rs`:

```rust
use riot_core::willow::{authorise_entry_with, OwnedRoot, Entry};

#[test]
fn owned_author_authorises_entry_in_owned_namespace() {
    let root = OwnedRoot::generate().expect("entropy");
    let author = root.author().expect("author");
    let entry = Entry::builder()
        .namespace_id(root.namespace_id().clone())
        .subspace_id(author.identity().subspace_id)
        .path(riot_core::willow::Path::from_slices(&[b"pub", b"probe"]).unwrap())
        .timestamp(1)
        .payload(b"x")
        .build();
    let authorised = authorise_entry_with(
        author.evidence_author(),
        author.write_capability(),
        entry,
    );
    assert!(authorised.is_ok());
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p riot-core --test owned_write_path`
Expected: FAIL — `authorise_entry_with` undefined.

- [ ] **Step 3: Generalize `authorise_entry`** — `crates/riot-core/src/willow/mod.rs:102-113`. Keep the old function as a thin wrapper so communal callers are untouched:

```rust
/// Mints the authorisation token using a caller-supplied write capability.
/// The capability must include the entry (namespace + the author's subspace
/// area). Communal callers pass `author.write_capability()`; owned callers pass
/// their delegated owned capability.
pub fn authorise_entry_with(
    author: &EvidenceAuthor,
    capability: WriteCapability,
    entry: Entry,
) -> Result<AuthorisedEntry, WillowError> {
    entry
        .into_authorised_entry(&capability, author.subspace_secret())
        .map_err(|_| WillowError::DoesNotAuthorise)
}

/// Communal convenience wrapper — unchanged behaviour for existing callers.
pub fn authorise_entry(
    author: &EvidenceAuthor,
    entry: Entry,
) -> Result<AuthorisedEntry, WillowError> {
    authorise_entry_with(author, author.write_capability(), entry)
}
```

Add a `pub(crate)` `evidence_author_for_test`/`evidence_author` accessor on
`OwnedSpaceAuthor` and export `authorise_entry_with`.

- [ ] **Step 4: Thread through `commit_at` and `publish_app_index`** — **`commit_at` lives in `crates/riot-core/src/session.rs:771`** (`pub(crate)`), not `apps/index.rs`. Add `commit_at_with(store, author, capability, path, bytes, ts)` there and make the existing `commit_at` a wrapper passing `author.write_capability()` — this preserves every current caller (`profile/resolver.rs`, `apps/endorse.rs`, `apps/trust.rs`, `apps/index.rs::publish_app_index`). Then add `publish_app_index_with` in `apps/index.rs:86` alongside `publish_app_index`. Show the `commit_at_with` body:

```rust
pub fn commit_at_with(
    store: &EvidenceStore,
    author: &EvidenceAuthor,
    capability: WriteCapability,
    path: &Path,
    bytes: &[u8],
    willow_timestamp_micros: u64,
) -> Result<(), AppsError> {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path.clone())
        .timestamp(willow_timestamp_micros)
        .payload(bytes)
        .build();
    let authorised = authorise_entry_with(author, capability, entry).map_err(map_authorise_err)?;
    // ... identical inspect/plan/commit as existing commit_at from here ...
    commit_authorised(store, authorised, bytes)
}
```

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p riot-core --test owned_write_path`
Expected: PASS.

- [ ] **Step 6: Add the disjointness guard test** — same file:

```rust
#[test]
fn communal_capability_cannot_authorise_owned_entry() {
    let root = OwnedRoot::generate().unwrap();
    let owned_author = root.author().unwrap();
    let communal = riot_core::willow::generate_space_organizer_author().unwrap();
    let entry = Entry::builder()
        .namespace_id(root.namespace_id().clone())
        .subspace_id(owned_author.identity().subspace_id)
        .path(riot_core::willow::Path::from_slices(&[b"pub", b"p"]).unwrap())
        .timestamp(1).payload(b"x").build();
    // A communal capability names a different namespace/area — must not authorise.
    assert!(authorise_entry_with(&communal_evidence(&communal), communal.write_capability(), entry).is_err());
}
```

Run: `cargo test -p riot-core --test owned_write_path`
Expected: PASS (both tests).

- [ ] **Step 7: Commit**

```bash
git add crates/riot-core/src/willow/mod.rs crates/riot-core/src/apps/index.rs crates/riot-core/tests/owned_write_path.rs
git commit -m "refactor(willow): thread supplied write capability through authorise_entry/commit_at/publish_app_index"
```

---

### Task 4: Admission gate admits owned namespaces

**Files:**
- Modify: `crates/riot-core/src/import/bundle.rs:492-502`
- Test: `crates/riot-core/tests/owned_admission.rs`

- [ ] **Step 1: Write the failing test** — `crates/riot-core/tests/owned_admission.rs`: build a single-entry bundle authored by an `OwnedRoot::author()` at path `pub/probe`, import it, and assert it is accepted (today `verify_frame` rejects any `is_owned()` capability). (Use the existing bundle-encode test helpers in `apps/bridge.rs`/`import` tests as the template for constructing `SignedWillowEntry` → `encode_bundle`.)

```rust
#[test]
fn owned_namespace_bundle_is_admitted() {
    let (store, bundle) = owned_page_probe_bundle();
    let preview = store.inspect(&bundle, ImportContext::new("test")).unwrap();
    assert!(matches!(preview, InspectOutcome::Preview(_)));
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p riot-core --test owned_admission`
Expected: FAIL — `UnsupportedCapability` (owned capability rejected).

- [ ] **Step 3: Widen the gate** — `crates/riot-core/src/import/bundle.rs:492-502`:

```rust
    // Accept a communal namespace with a zero-delegation communal capability,
    // OR an owned namespace with an owned capability whose granted area covers
    // the entry. Delegations remain unsupported until Slice 2's read/grant flow.
    let namespace_ok = entry.namespace_id().is_communal() || entry.namespace_id().is_owned();
    // Delegations stay unsupported until Slice 2 for BOTH kinds. The owned
    // branch additionally requires the capability to cover the entry;
    // `includes` (write_capability.rs:334, NOT `includes_entry`) checks
    // namespace + subspace + path coverage. Entry impls Namespaced+Coordinatelike.
    let capability_shape_ok = capability.delegations().is_empty()
        && if capability.is_owned() {
            entry.namespace_id().is_owned() && capability.includes(entry)
        } else {
            !entry.namespace_id().is_owned()
        };
    if !namespace_ok || !capability_shape_ok {
        return Err(BundleDiagnostic {
            code: DiagnosticCode::UnsupportedCapability,
            component: ItemComponent::Authorization,
        });
    }
```

Note: `WriteCapability::includes_entry` (or `includes` over the entry's
namespace/subspace/path) is the willow25 predicate; confirm the exact method
name against the pinned crate and use it — do not hand-roll area math.

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p riot-core --test owned_admission`
Expected: PASS.

- [ ] **Step 5: Regression — communal path unchanged**

Run: `cargo test -p riot-core`
Expected: PASS (all existing import/bundle tests still green).

- [ ] **Step 6: Commit**

```bash
git add crates/riot-core/src/import/bundle.rs crates/riot-core/tests/owned_admission.rs
git commit -m "feat(import): admit owned-namespace bundles with an owned capability covering the entry"
```

---

## Phase B — Visibility path grammar

### Task 5: One `Visibility` source of truth + generalized paths

**Files:**
- Create: `crates/riot-core/src/apps/visibility.rs`
- Modify: `crates/riot-core/src/apps/entry.rs:12,44-99`, `crates/riot-core/src/apps/index.rs:26,529-621`
- Modify: `crates/riot-core/src/import/bundle.rs` (consume the shared constant)
- Test: `crates/riot-core/src/apps/visibility.rs` tests + `crates/riot-core/tests/visibility_admission.rs`

- [ ] **Step 1: Write the failing test** — `crates/riot-core/src/apps/visibility.rs`:

```rust
//! The leading path segment that fixes the public/protected boundary. This is
//! the SINGLE source of truth consumed by every admission gate — local write,
//! the app-path classifiers, and the import verifier — so they cannot drift.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pub_and_con_are_valid_segments() {
        assert_eq!(Visibility::from_segment(b"pub"), Some(Visibility::Public));
        assert_eq!(Visibility::from_segment(b"con"), Some(Visibility::Connections));
        assert_eq!(Visibility::from_segment(b"apps"), None);
        assert_eq!(Visibility::from_segment(b""), None);
    }
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p riot-core apps::visibility` → FAIL (undefined).

- [ ] **Step 3: Implement** — `crates/riot-core/src/apps/visibility.rs`:

```rust
pub const PUBLIC_SEGMENT: &[u8] = b"pub";
pub const CONNECTIONS_SEGMENT: &[u8] = b"con";
/// Every admission gate MUST validate the leading segment against this list.
pub const VISIBILITY_SEGMENTS: &[&[u8]] = &[PUBLIC_SEGMENT, CONNECTIONS_SEGMENT];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Connections,
}

impl Visibility {
    pub fn from_segment(seg: &[u8]) -> Option<Self> {
        match seg {
            PUBLIC_SEGMENT => Some(Self::Public),
            CONNECTIONS_SEGMENT => Some(Self::Connections),
            _ => None,
        }
    }
    pub fn segment(self) -> &'static [u8] {
        match self {
            Self::Public => PUBLIC_SEGMENT,
            Self::Connections => CONNECTIONS_SEGMENT,
        }
    }
}
```

Register the module in `apps/mod.rs`: `pub mod visibility;`.

- [ ] **Step 4: Generalize `app_data_path` + `is_app_data_path`** — `entry.rs`. `app_data_path` gains a `visibility: Visibility` first parameter and pushes `visibility.segment()` before `APPS_COMPONENT`. `is_app_data_path` first consumes and validates the visibility segment:

```rust
pub fn is_app_data_path(path: &Path) -> bool {
    let mut components = path.components();
    let Some(vis) = components.next() else { return false };
    if Visibility::from_segment(vis.as_ref()).is_none() {
        return false;
    }
    let Some(first) = components.next() else { return false };
    if first.as_ref() != APPS_COMPONENT { return false; }
    // ... unchanged app_id + key-segment validation from here ...
}
```

- [ ] **Step 5: Generalize `app_index_*` + `classify_app_index_path`** — `index.rs`. Each `app_index_*` path builder gains a leading `Visibility` param; `classify_app_index_path` consumes and validates the visibility segment first, then proceeds exactly as today. Return the visibility alongside the slot: change `AppIndexSlot` consumers to receive `(Visibility, AppIndexSlot)`, or add a `visibility` field to each variant. Keep `endorsements` plural (matches `app_index_endorsement_path`).

- [ ] **Step 6: Route ALL gates through the classifiers — do NOT add a blanket visibility prefix to `verify_frame`.** `verify_frame` (`bundle.rs` `schema_ok`) and the local-write binding gate `inspect_inner`'s `path_matches` (`session.rs:623-673` — the **fourth** gate; the plan's "three gates" undercounts) both legitimately admit **alert evidence** (`objects/alert/...`) and **profile cards** (`profile/<subspace>/card`), which carry NO visibility segment. A top-level "first component ∈ {pub,con}" check would reject those on both import and local write and break existing tests. The single source of truth therefore lives **inside `is_app_data_path` / `classify_app_index_path`** (Steps 4–5), which consume the `visibility` module; `schema_ok` and `path_matches` keep delegating to those classifiers for the app families and keep their existing arms for alert/profile. No new prefix logic is added to `verify_frame` itself.

- [ ] **Step 6b: Profile-card path stays unprefixed in Slice 1 (explicit decision).** The design places the calling card at `pub/profile/<subspace>/card`, but the calling card is only *used* in Slice 2 (the stranger-facing view). Slice 1 is public-only and does not move `profile/path.rs`; the ProfileCard write path is therefore intentionally NOT part of the `Visibility::Public` sweep. Record this so the grammar asymmetry (app/app-index carry `pub/`; profile/alert do not) is a decision, not a silent omission. `MAX_PATH_COMPONENTS = 64` (`bundle.rs:38`), so the extra leading segment is always within budget.

- [ ] **Step 7: Write the cross-gate regression** — `crates/riot-core/tests/visibility_admission.rs`:

```rust
#[test]
fn non_visibility_leading_segment_rejected_identically_by_all_gates() {
    // A path whose first component is neither pub nor con:
    let bad = Path::from_slices(&[b"xxx", b"apps", &[0u8;32], b"k"]).unwrap();
    assert!(!riot_core::apps::entry::is_app_data_path(&bad));
    assert!(riot_core::apps::index::classify_app_index_path(&bad).is_none());
    // And a bundle carrying such a path is refused by verify_frame:
    let bundle = single_entry_bundle_at(&bad);
    assert!(matches!(inspect(&bundle), InspectOutcome::Rejected(_)));
}
```

- [ ] **Step 8: Run, verify pass; fix every existing caller**

Run: `cargo test -p riot-core`
Expected: PASS. All existing callers of `app_data_path`/`app_index_*`/`AppDataBridge` now pass a `Visibility` — update them (the demo fixture and starter-app publication use `Visibility::Public`). This is the compile-driven sweep that proves "one source of truth."

- [ ] **Step 9: Commit**

```bash
git add crates/riot-core/src/apps/visibility.rs crates/riot-core/src/apps/entry.rs crates/riot-core/src/apps/index.rs crates/riot-core/src/apps/mod.rs crates/riot-core/src/import/bundle.rs crates/riot-core/tests/visibility_admission.rs
git commit -m "feat(apps): leading pub/con visibility segment as one admission source of truth"
```

---

### Task 6: `kind: page` manifest + page publication

**Files:**
- Create: `crates/riot-core/src/apps/page.rs`
- Modify: `crates/riot-core/src/apps/manifest.rs` (add `kind` field)
- Modify: `crates/riot-core/src/import/bundle.rs` (`schema_ok`: recognize the `pub/page/current` slot)
- Modify: `crates/riot-core/src/session.rs` (`inspect_inner`'s `path_matches`: recognize + owner-bind the `pub/page/current` slot)
- Test: `crates/riot-core/src/apps/page.rs` tests

> **Critical (plan-gate feasibility blocker):** `pub/page/current` is a NEW path
> family. Local writes pass through BOTH `verify_frame`'s `schema_ok` (`bundle.rs`)
> and `inspect_inner`'s `path_matches` (`session.rs:623-673`); each only
> recognizes app-data / app-index / profile / alert slots today. Without a new
> slot in **both**, `publish_page` is rejected (`schema_ok` → `UnsupportedSchema`)
> or silently dropped (`path_matches` fails to bind it to `verified`). Task 6
> MUST add a `page/current` slot classifier to both, binding it to the writing
> author's own subspace (last-write-wins, like the profile card). Add a
> `classify_page_pointer(path) -> Option<Visibility>` helper in `page.rs` and call
> it from both gates so this too is one source of truth.

- [ ] **Step 1: Write the failing test** — `page.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publishing_a_page_sets_pub_page_current_to_the_app_id() {
        let (store, author, cap) = owned_store();
        let manifest = page_manifest(b"index.html");
        let bundle = page_bundle(b"<h1>hi</h1>");
        let app_id = publish_page(&store, author.evidence_author(), cap, &manifest, &bundle, 1).unwrap();
        let current = read_page_current(&store).unwrap();
        assert_eq!(current, app_id);
    }
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p riot-core apps::page` → FAIL.

- [ ] **Step 3: Add `kind` to the manifest** — `manifest.rs`: add an optional `kind: ManifestKind` (`App` default, `Page`) to the parsed manifest struct + canonical CBOR round-trip. A `kind: page` manifest is otherwise a normal manifest.

- [ ] **Step 4: Add the `page/current` slot to both admission gates** — implement `classify_page_pointer` in `page.rs` (matches `<vis>/page/current`, returns the `Visibility`); add an arm to `schema_ok` (`bundle.rs`) and to `path_matches` (`session.rs`) that accepts it and binds it to the writing author's own subspace (last-write-wins). Mirror exactly how the profile-card slot is handled in both gates.

- [ ] **Step 5: Implement `publish_page`** — `page.rs`: calls `publish_app_index_with(store, author, capability, manifest, bundle, ts)` at `Visibility::Public`, then writes `page/current = app_id` via `commit_at_with` at the path built for `Visibility::Public` + `page/current`.

- [ ] **Step 6: Run, verify pass** — `cargo test -p riot-core apps::page` → PASS (publication now clears both gates).

- [ ] **Step 7: Commit**

```bash
git add crates/riot-core/src/apps/page.rs crates/riot-core/src/apps/manifest.rs crates/riot-core/src/import/bundle.rs crates/riot-core/src/session.rs
git commit -m "feat(apps): kind:page manifest + publish_page + page/current slot in both admission gates"
```

---

## Phase C — Root-key custody across FFI

### Task 7: Seal the owned root; store this-device-only, no-sync, no-backup

**Files:**
- Modify: `crates/riot-core/src/willow/owned.rs` (owned sealed envelope, distinct magic/AAD)
- Modify: `crates/riot-ffi/src/mobile_state.rs` (custody surface)
- Modify: `apps/ios/Riot/Core/KeychainWrappingKeyStore` (or equivalent) — accessibility class
- Test: `crates/riot-core/src/willow/owned.rs` tests + `crates/riot-ffi` custody test

- [ ] **Step 1: Write the failing test** — `owned.rs`:

```rust
#[test]
fn owned_root_seals_and_reopens_but_communal_opener_rejects_it() {
    let root = OwnedRoot::generate().unwrap();
    let key = [7u8; 32];
    let sealed = root.seal(&key).unwrap();
    let reopened = OwnedRoot::open_sealed(&key, &sealed).unwrap();
    assert_eq!(reopened.namespace_id(), root.namespace_id());
    // The communal opener MUST refuse an owned envelope (distinct magic/AAD).
    assert!(EvidenceAuthor::open_sealed_identity(&key, &sealed).is_err());
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p riot-core willow::owned` → FAIL.

- [ ] **Step 3: Implement owned seal/open** — `owned.rs`, mirroring `seal_identity` (XChaCha20-Poly1305) but with a **distinct** magic and AAD so the two envelope types are non-interchangeable, preserving the communal "reject non-communal" invariant at `identity.rs:142`:

```rust
const OWNED_ROOT_MAGIC: &[u8; 8] = b"RIOTOR\x01\0";
const OWNED_ROOT_AAD: &[u8] = b"riot/owned-root/sealed/v1";
// seal(): plaintext = namespace_id(32) || namespace_secret(32); zeroize plaintext.
// open_sealed(): validate magic, decrypt, require namespace_id.is_owned().
```

- [ ] **Step 4: FFI custody surface** — `mobile_state.rs`: the owned root secret is unsealed inside core, used to mint the author's owned capability, and **never returned across FFI in plaintext**. The FFI hands the host only the sealed blob (to persist) and the public `namespace_id`. Reuse the existing `WrappingKeyStore` pattern.

- [ ] **Step 5: iOS accessibility class** — the Keychain item storing the sealed owned root uses `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, `kSecAttrSynchronizable = false`, and is excluded from backup. Add a test asserting the attributes on the stored item.

- [ ] **Step 6: Write the custody assertion test** — `crates/riot-ffi` test: after creating a personal space, assert the plaintext 32-byte root secret bytes appear in **none** of: the FFI return values, the persisted profile JSON, the log buffer, or any committed Willow entry payload.

- [ ] **Step 7: Run, verify pass** — `cargo test -p riot-core willow::owned && cargo test -p riot-ffi custody` → PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/riot-core/src/willow/owned.rs crates/riot-ffi/src/mobile_state.rs apps/ios/Riot/Core/
git commit -m "feat(custody): seal owned root with distinct envelope; this-device-only no-sync Keychain storage"
```

---

## Phase D — iOS runtime containment (store-independent)

### Task 8: Foreign `kind: page` bundles mount deny-closed (the beacon test)

**Files:**
- Create: `apps/ios/Riot/Apps/ForeignPageRuntime.swift`
- Modify: `apps/ios/Riot/Apps/AppRuntimeView.swift:124-145`
- Test: `apps/ios/RiotTests/ForeignPageContainmentTests.swift`

- [ ] **Step 1: Write the failing test** — `ForeignPageContainmentTests.swift`:

```swift
func testForeignPageMountsWithNoBridge() throws {
    let runtime = ForeignPageRuntime(resolver: hostilePageResolver())
    let config = runtime.makeConfiguration()
    // No "riot" message handler is installed for a foreign page.
    XCTAssertFalse(config.userContentController.hasHandler(named: "riot"))
}

func testHostilePagePutWritesZeroEntries() throws {
    let store = InMemoryStoreSpy()
    let runtime = ForeignPageRuntime(resolver: hostilePageResolver(js: "window.webkit?.messageHandlers?.riot?.postMessage({id:1,op:'put',key:'v',value:'1'})"))
    runtime.load(into: store)
    runtime.waitForLoad()
    XCTAssertEqual(store.writeCount, 0, "a viewed page must never sign an entry as the visitor")
}
```

(`hasHandler(named:)` is a small test shim over `WKUserContentController`; `InMemoryStoreSpy` counts commit attempts.)

- [ ] **Step 2: Run, verify fail** — `xcodebuild test ... -only-testing:RiotTests/ForeignPageContainmentTests` → FAIL (`ForeignPageRuntime` undefined).

- [ ] **Step 3: Implement `ForeignPageRuntime`** — `ForeignPageRuntime.swift`: builds a `WKWebViewConfiguration` with the CSP scheme handler, the navigation lock, and the `window.open` denial from the existing runtime, but **injects no `riot` message handler, no `RiotJS`, and no bridge**. It is render-only:

```swift
/// Mounts a kind:page bundle from a namespace the viewer does NOT own. Slice 1
/// posture: no bridge at all — no put, no whoami, no profile. The page is inert
/// HTML/CSS/JS behind the sandbox. See design D3.
struct ForeignPageRuntime {
    let resolver: AppResourceResolver
    @MainActor func makeConfiguration() -> WKWebViewConfiguration {
        let c = WKWebViewConfiguration()
        c.websiteDataStore = .nonPersistent()
        // Deliberately NO addUserScript(RiotJS), NO add(bridge, name:"riot").
        c.setURLSchemeHandler(AppSchemeHandler(resolver: resolver), forURLScheme: AppSchemeHandler.scheme)
        AppNetworkBackstop.install(into: c)   // Task 9
        return c
    }
}
```

- [ ] **Step 4: Route foreign pages** — `AppRuntimeView.swift`: when the bundle's manifest `kind == .page` **and** the bundle's namespace is not the viewer's own owned namespace, construct `ForeignPageRuntime` instead of the bridged `AppRuntimeCoordinator`.

- [ ] **Step 5: Run, verify pass** — the two tests PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/ios/Riot/Apps/ForeignPageRuntime.swift apps/ios/Riot/Apps/AppRuntimeView.swift apps/ios/RiotTests/ForeignPageContainmentTests.swift
git commit -m "feat(ios): deny-closed foreign kind:page runtime (no bridge, no whoami) + beacon test"
```

---

### Task 9: iOS network backstop

**Files:**
- Create: `apps/ios/Riot/Apps/AppNetworkBackstop.swift`
- Test: `apps/ios/RiotTests/NetworkBackstopTests.swift`

- [ ] **Step 1: Write the failing test** — `NetworkBackstopTests.swift`: build a config via `AppNetworkBackstop.install`, load a page whose CSP has been stripped (test build) that attempts a subresource load to an external host, and assert the scheme handler is the sole loader — the external request is refused (recorded via a `URLProtocol` spy that must see zero requests).

```swift
func testCSPStrippedPageStillCannotReachNetwork() throws {
    let spy = OutboundRequestSpy.install()
    let runtime = ForeignPageRuntime(resolver: cspStrippedPage(js: "fetch('https://evil.example/x')"))
    runtime.waitForLoad()
    XCTAssertEqual(spy.externalRequestCount, 0, "backstop must block network even if CSP is absent")
}
```

- [ ] **Step 2: Run, verify fail** → FAIL (`AppNetworkBackstop` undefined).

- [ ] **Step 3: Implement via `WKContentRuleList`** (plan-gate feasibility fix) — a `WKNavigationDelegate` policy is the WRONG mechanism: `decidePolicyFor navigationAction` fires only for frame navigations, never for subresource loads (`fetch`/XHR/`img`/`script`/WebSocket), so a CSP-stripped `fetch('https://evil')` never reaches it. The CSP-independent tool that DOES gate subresource loads is **`WKContentRuleList`**. `AppNetworkBackstop.install` compiles and adds a rule list that blocks all loads and allows only the `riot-app` scheme:

```swift
enum AppNetworkBackstop {
    // Blocks every load, then makes an exception for the app's own scheme.
    // WKContentRuleList is evaluated by the network process for ALL resource
    // loads (unlike the navigation delegate), so it holds even if CSP is absent.
    static let ruleJSON = """
    [
      {"trigger":{"url-filter":".*"},"action":{"type":"block"}},
      {"trigger":{"url-filter":"^riot-app://"},"action":{"type":"ignore-previous-rules"}}
    ]
    """
    @MainActor static func install(into config: WKWebViewConfiguration) {
        WKContentRuleListStore.default().compileContentRuleList(
            forIdentifier: "riot-page-backstop", encodedContentRuleList: ruleJSON
        ) { list, _ in
            if let list { config.userContentController.add(list) }
        }
    }
}
```

Because compilation is async, `install` must complete before the page loads — compile once at runtime start and gate the first `load(URLRequest:)` on the rule list being installed (the test's `waitForLoad()` already serializes this). Document that the rule list — not the navigation delegate — is the subresource wall; the navigation lock remains for top-level navigation.

- [ ] **Step 4: Run, verify pass** → PASS.

- [ ] **Step 5: Extend the containment suite** — add the covert-channel cases from the design's [S1] list (`dns-prefetch`, WebRTC/STUN, form submission, `window.open`, `<link>` subresource, forged-origin storage read, secure-context APIs on the Android synthetic origin — the Android cases go in the Android test target). Each asserts refusal.

- [ ] **Step 6: Commit**

```bash
git add apps/ios/Riot/Apps/AppNetworkBackstop.swift apps/ios/RiotTests/NetworkBackstopTests.swift
git commit -m "feat(ios): network backstop independent of CSP + covert-channel containment suite"
```

---

### Task 10: Android — foreign `kind: page` bundles mount deny-closed

The spec (D3/D4) requires the deny-closed foreign-page posture and *provable*
containment parity on **both** platforms; `apps/android/.../AppWebViewHost.kt`
exists and today installs a bridge for trusted apps. This task is the Android
mirror of Task 8 and is store-independent.

**Files:**
- Modify: `apps/android/app/src/main/kotlin/org/riot/.../AppWebViewHost.kt`
- Create: `apps/android/.../ForeignPageWebViewHost.kt`
- Test: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/ForeignPageContainmentTest.kt`

- [ ] **Step 1: Write the failing instrumentation test** — a foreign `kind: page` bundle is hosted with **no `@JavascriptInterface` bridge object added** (no `put`, no `whoami`); a hostile page calling the bridge is a no-op and the store records zero visitor-signed writes.
- [ ] **Step 2: Run, verify fail.**
- [ ] **Step 3: Implement `ForeignPageWebViewHost`** — reuses the existing `blockNetworkLoads=true`, service-worker denial, Safe Browsing off, and DOM-storage disable (`AppWebViewHost.kt:52-84`) but calls **no `addJavascriptInterface`**. Route `kind: page` bundles from a namespace the viewer does not own to it.
- [ ] **Step 4: Run, verify pass. Commit.**

### Task 11: Android — secure-context API denial parity suite

Android's synthetic `https://` origin is a **secure context**, unlocking APIs the
iOS `riot-app://` non-secure origin never exposes (service workers, push,
background sync, secure-context crypto). Spec D4 requires each to be independently
denied and the denial proven.

**Files:**
- Test: `apps/android/.../SecureContextDenialTest.kt`
- Modify: `AppWebViewHost.kt` / `ForeignPageWebViewHost.kt` as needed to close any gap the tests expose.

- [ ] **Step 1: Write failing tests** — one assertion per API: `navigator.serviceWorker.register` rejects/absent; `PushManager` unavailable; background sync unavailable; `fetch`/XHR/WebSocket to any host blocked (network-load block); `<link rel=dns-prefetch>` and WebRTC/STUN produce no outbound connection (a request spy sees zero); form submission and `window.open` denied; a forged-origin storage read fails.
- [ ] **Step 2: Run, verify which fail.**
- [ ] **Step 3: Close any gaps** so every case is denied; document what each denial rests on.
- [ ] **Step 4: Run, verify all pass. Commit.**

## Phase E — Personal-space UX (BLOCKED on multi-space store)

> Do not begin until the store's native-API task and iOS-cutover task are
> merged. Each task below names the store call it depends on; the step-level
> Swift/Rust is finalized against the merged signatures — the `RiotDatabase` vs
> `DatabaseSession` naming must be reconciled first.

### Task 12: FFI — create a personal (owned) space and persist it

**Depends on store:** the owned-space creation entry point (the store spec's
`namespace_roots` table + `owned-root-custodian` role; the plan's Task 5 signer
persistence). Uses `OwnedRoot::generate` (Task 1) + `seal` (Task 7); persists the
sealed root via the store's signer table, and registers the space via the store's
create path (sibling of `create_communal_space`).

- [ ] Write the failing FFI test: `create_personal_space(title)` returns a
  `PersonalSpace { namespace_id, title, kind: Owned }`, the namespace reports
  `is_owned()`, and the sealed root is persisted (not the plaintext).
- [ ] Add `PersonalSpace` to `mobile_api.rs` and `create_personal_space` to
  `mobile_state.rs`, minting via `OwnedRoot`, sealing via Task 7, persisting via
  the store's signer/space tables.
- [ ] Run, verify pass. Commit.

### Task 13: First-run onboarding gate (name + space)

**Depends on store:** `list_spaces` (to detect "no spaces yet" → show onboarding)
and Task 14. There is no onboarding flag in the app today (confirmed).

- [ ] Failing UI test: on a store with zero spaces, `OnboardingGate` is presented
  before the tab shell; it collects a display name (first-ever caller of the FFI
  `set_display_name`) and creates a personal space; target ≤ 1 minute to name +
  space.
- [ ] Implement `OnboardingGate.swift`; present it from `RiotApp`/`ConferenceShellView`
  when `list_spaces(includeArchived: false)` is empty.
- [ ] Run, verify pass. Commit.

### Task 14: Template gallery + source editor authoring

**Depends on:** Task 6 (`publish_page`) via an FFI `publish_page` wrapper; store's
`open_space`/`AppSession` to write into the personal namespace.

- [ ] Failing test: selecting a template and publishing produces a signed
  `kind:page` bundle whose `pub/page/current` points at it; editing the source and
  republishing repoints `page/current` to a new app_id.
- [ ] **[S1] Offline-authoring test (explicit no-network precondition):** with the
  network unavailable and no local model, template selection, source editing, and
  publication all succeed. Assert the offline condition in the test setup (a
  request spy that must see zero outbound requests during the whole flow) — the
  criterion is "works in a blackout," so the test must actually exercise it.
- [ ] Implement `PageTemplates.swift` (the gaudy built-ins) and
  `PageAuthoringView.swift` (gallery → native fields → view-source editor).
  Rendered-preview-first is Slice 1.5; Slice 1 ships gallery + source editor.
- [ ] Defined states: no page yet (`page/current` unset), publish failure
  (missing/expired capability, signing failure, store-write failure).
- [ ] Run, verify pass. Commit.

### Task 15: Distinct creation entry points + Spaces-tab card + no privacy control

**Depends on:** Tasks 12–14.

- [ ] Failing UI test: the Spaces tab shows a personal-space card distinct from
  the group-space create control; "Make your page" (owned) and "Create group
  space" (communal) are separate, labeled entry points; **no** visibility/"connections
  only" control is present as a live switch (if shown at all it is disabled
  "coming soon").
- [ ] Implement in `ConferenceShellView.swift`/`AppModel.swift`, reusing the
  existing `SpacesView` structure; keep both space kinds visibly distinct.
- [ ] Run, verify pass. Commit.

### Task 16: Cross-device viewing (the demo)

**Depends on:** store's nearby-sync cutover (its Task 10) so a foreign owned
namespace can sync into the viewer's store; Task 8 (deny-closed runtime).

- [ ] Failing integration test (two headless nodes, per the existing
  `RIOT_SEED_SPACE` harness pattern): node A creates a personal space and
  publishes a page; node B syncs A's owned namespace and renders A's page via
  `ForeignPageRuntime` with no bridge; B writes zero visitor-signed entries.
- [ ] Implement the sync path for a foreign owned namespace + the "view a nearby
  person's page" surface.
- [ ] Run, verify pass. Commit.

### Task 17: Slice-1 acceptance sweep

- [ ] Verify every **[S1]**-tagged test in the design's Testing strategy is green:
  namespace-kind-intrinsic, root-key-custody, owned-cap-minting, visibility one
  source of truth, containment suite, beacon/foreign-page posture, offline
  authoring, iOS network backstop, **and Android parity — foreign-page bridge-less
  posture + secure-context denial suite (Tasks 10–11)**.
- [ ] Confirm acceptance criteria 1–7 (Slice 1) are each demonstrable on **both**
  iOS and Android (D4 requires provable parity).
- [ ] Run the Rust+Swift+Kotlin coverage command set against
  `.coverage-thresholds.json`; meet thresholds. Commit any test gaps closed.

---

## Self-review notes

- **Spec coverage:** owned namespace (T1–2), capability threading (T3), admission
  (T4), pub/con one-source-of-truth (T5), page=app + `kind:page` incl. the
  `page/current` slot in both gates (T6), custody incl. no-sync/no-backup +
  root-compromise storage decision (T7), deny-closed foreign runtime + beacon
  (T8), iOS backstop via WKContentRuleList + covert channels (T9), **Android
  deny-closed foreign runtime (T10) and secure-context denial parity (T11)**,
  onboarding/creation/authoring/no-privacy-control (T12–15), cross-device demo
  (T16), acceptance sweep (T17). LLM authoring and recovery export are Slice 1.5,
  correctly absent here.
- **Ordering:** the capability-threading refactor (T3) lands before any UX, per
  the CTO review. Phase E is fully gated on the store. iOS containment (T8–9)
  can precede Android containment (T10–11) so the primary demo path is unblocked
  first, but both land in Slice 1 — the spec requires *provable* parity.
- **willow25 method names** (`WriteCapability::new_owned` taking `&keypair` and
  returning `Self`; the coverage predicate `includes`, NOT `includes_entry`;
  `NamespaceSecret::corresponding_namespace_id`; `NamespaceId::is_owned`) are used
  as named in the pinned crate; confirm exact spelling at implementation time and
  adjust the call, never hand-roll the area math.
