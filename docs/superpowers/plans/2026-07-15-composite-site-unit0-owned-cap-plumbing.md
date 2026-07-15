# Composite Site — Unit 0: Owner-Side Owned-Capability Plumbing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the owner-side capability primitives for composite sites — mint an owned masthead identity, produce the owner write-capability, issue section-scoped time-boxed editor delegations (hard-refusing any area that escapes `/articles/`), and seal/persist the owned root secret — all in `riot-core`, headless-testable, with an FFI surface.

**Architecture:** Introduce an `OwnedMasthead` (the site owner's identity = an owned-namespace root secret + the owner's subspace signing secret) in `crates/riot-core/src/willow/`. It mints the owner `WriteCapability::new_owned(...)` and delegates section caps via `try_delegate`, enforcing the reserved-path rule at issuance. The root secret is sealed with the existing XChaCha20Poly1305 pattern (native-supplied wrapping key) and stored as a `local_state` BLOB — never plaintext, never crossing FFI unsealed. FFI follows the existing proc-macro UniFFI handle+arbiter pattern.

**Tech Stack:** Rust 2021, `willow25 0.6.0-alpha.3` (`WriteCapability`, `Area`, `Path`, `NamespaceSecret`, `SubspaceSecret`), `meadowcap 0.5.0`, XChaCha20Poly1305 sealing, proc-macro `uniffi`.

**Scope note:** This is Unit 0 of the composite-site design (`docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md`, §7). It builds the owner *write* side only. Admission/verification (Unit 1), manifest (Unit 2), moderation (Unit 3), render (Unit 4), transport (Unit 5), and native UI (Unit 6) are separate plans. This plan produces working, independently-testable software: an owner can be generated, mint their cap, delegate a scoped editor cap, and seal/restore the root.

**Design decisions locked here (from the spec + design-review gate):**
- The owner writes with `new_owned(&namespace_secret, owner_subspace_id)`; editors receive `try_delegate`'d caps.
- **Reserved-path rule (two-layer, Architect + Security round 1):** delegation issuance MUST hard-refuse any `Area` whose path escapes `/articles/` (the *belt*). The manifest/mod validators independently re-check at admission/validation (the *suspenders*, Units 1–2) — "admitted into O" never implies "honored as manifest/mod." This plan builds the belt.
- **Root secret at-rest (CTO round 2):** sealed under a native-provided wrapping key, stored as a `local_state` BLOB; the existing `open_sealed_identity` rejects owned namespaces, so a *new* owned-root envelope is required here.
- Reserved path constant: articles live under `/articles/<section>/…`; manifest at `/manifest`; moderation under `/mod/`. Unit 0 defines these path constants so later units share them.

---

## File Structure

- **Create** `crates/riot-core/src/willow/masthead.rs` — `OwnedMasthead` (owned root + owner subspace secret): generate, `owner_write_capability`, `delegate_section`, `authorise_owner_entry`, seal/open. One responsibility: owner-side owned-namespace identity + capability issuance + owner signing.
- **Create** `crates/riot-core/src/willow/site_paths.rs` — reserved path constants + helpers (`articles_prefix`, `manifest_path`, `mod_prefix`, `is_under_articles`). Shared by Units 0–4.
- **Modify** `crates/riot-core/src/willow/mod.rs` — declare the two new modules; re-export `OwnedMasthead` and the path helpers; add new `WillowError` variants.
- **Modify** `crates/riot-core/src/willow/owned.rs` — add a `pub(crate) fn into_parts` / accessor so `OwnedMasthead` can consume the root secret to mint caps (today `namespace_secret` is private and `#[allow(dead_code)]`).
- **Create** `crates/riot-ffi/src/site_ffi.rs` — FFI surface: create an owned site (returns a handle + sealed root), issue an editor delegation, restore from sealed root. Follows the `mobile_api`/`mobile_state` handle+arbiter pattern.
- **Modify** `crates/riot-ffi/src/lib.rs` — add `mod site_ffi;`.
- **Test** inline `#[cfg(test)]` in `masthead.rs` and `site_paths.rs`; integration test `crates/riot-core/tests/owned_masthead.rs`.

---

## Task 1: Reserved site-path constants

**Files:**
- Create: `crates/riot-core/src/willow/site_paths.rs`
- Modify: `crates/riot-core/src/willow/mod.rs` (add `mod site_paths; pub use site_paths::{...};`)

- [ ] **Step 1: Write the failing test**

In `crates/riot-core/src/willow/site_paths.rs`:
```rust
//! Reserved path regions for a composite-site owned masthead namespace `O`.
//!
//! `/manifest`         — the signed site manifest (Unit 2), never delegated.
//! `/articles/<sect>/` — editorial articles; the ONLY region delegated to editors.
//! `/mod/`             — moderation records (Unit 3), never delegated to editors.

use willow25::paths::Path;

/// First path component of the editorial region.
pub const ARTICLES_COMPONENT: &[u8] = b"articles";
/// First path component of the reserved manifest record.
pub const MANIFEST_COMPONENT: &[u8] = b"manifest";
/// First path component of the moderation region.
pub const MOD_COMPONENT: &[u8] = b"mod";

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: `Path::from_slices` returns `Result<Path, PathError>` in willow25
    // 0.6.0-alpha.3 — every call site `.expect(...)`s it.
    #[test]
    fn articles_path_is_under_articles_but_manifest_is_not() {
        let article = Path::from_slices(&[ARTICLES_COMPONENT, b"news", b"post-1"]).expect("path");
        let manifest = Path::from_slices(&[MANIFEST_COMPONENT]).expect("path");
        assert!(is_under_articles(&article), "article path must be under /articles");
        assert!(!is_under_articles(&manifest), "manifest path must NOT be under /articles");
    }

    #[test]
    fn empty_and_mod_paths_are_not_under_articles() {
        let empty = Path::from_slices(&[]).expect("path");
        let moderation = Path::from_slices(&[MOD_COMPONENT, b"revoke-1"]).expect("path");
        assert!(!is_under_articles(&empty));
        assert!(!is_under_articles(&moderation));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core willow::site_paths -- --nocapture`
Expected: FAIL — `cannot find function is_under_articles in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add above the `#[cfg(test)]` block in `site_paths.rs`:
```rust
/// True iff `path`'s first component is exactly `articles` (the delegatable region).
/// A delegated editor cap's granted area path MUST satisfy this; `/manifest` and
/// `/mod/` (and the empty/root path) must not, so they can never be delegated.
pub fn is_under_articles(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|first| first.as_ref() == ARTICLES_COMPONENT)
}
```

In `crates/riot-core/src/willow/mod.rs`, add after the existing `mod owned;` line:
```rust
mod site_paths;
pub use site_paths::{is_under_articles, ARTICLES_COMPONENT, MANIFEST_COMPONENT, MOD_COMPONENT};
```

> Note: confirm the `Path` component iterator method name against `willow25 0.6.0-alpha.3` (`path.components()` yielding items whose `.as_ref()` is `&[u8]`). If the accessor differs (e.g. `iter()` / `as_slices()`), adjust `is_under_articles` accordingly — the test pins the behavior.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core willow::site_paths -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/willow/site_paths.rs crates/riot-core/src/willow/mod.rs
git commit -m "feat(willow): reserved site-path constants + is_under_articles gate"
```

---

## Task 2: Expose the owned root secret to same-crate minting

**Files:**
- Modify: `crates/riot-core/src/willow/owned.rs` (add a `pub(crate)` accessor to the retained `namespace_secret`)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `owned.rs`:
```rust
#[test]
fn owned_root_exposes_secret_ref_for_minting() {
    let root = OwnedRoot::generate().expect("owned root");
    // The secret's corresponding namespace id must match the root's namespace id.
    let secret = root.namespace_secret_ref();
    assert_eq!(secret.corresponding_namespace_id(), *root.namespace_id());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core willow::owned -- --nocapture`
Expected: FAIL — `no method named namespace_secret_ref`.

- [ ] **Step 3: Write minimal implementation**

In `owned.rs`, remove the `#[allow(dead_code)]` on `namespace_secret` and add (inside `impl OwnedRoot`):
```rust
/// Borrow the retained owned-namespace root secret for capability minting.
/// `pub(crate)` — the secret never leaves the crate and never crosses FFI.
pub(crate) fn namespace_secret_ref(&self) -> &NamespaceSecret {
    &self.namespace_secret
}
```
Ensure `NamespaceSecret` is imported in `owned.rs` (it already is, via the generation code) and that `.corresponding_namespace_id()` returns `NamespaceId` comparable with `*root.namespace_id()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core willow::owned -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/willow/owned.rs
git commit -m "feat(willow): pub(crate) accessor for owned root secret (minting)"
```

---

## Task 3: `OwnedMasthead::generate` — owner identity (owned root + owner subspace)

**Files:**
- Create: `crates/riot-core/src/willow/masthead.rs`
- Modify: `crates/riot-core/src/willow/mod.rs` (declare + re-export)

- [ ] **Step 1: Write the failing test**

Create `crates/riot-core/src/willow/masthead.rs`:
```rust
//! `OwnedMasthead` — the composite-site owner identity.
//!
//! Combines the owned-namespace root secret (authority to mint the owner write
//! capability and to delegate) with the owner's own subspace signing secret
//! (the author key the owner writes entries as, and the signer for delegations).
//! Unit 0 scope: generation, owner capability minting, section delegation
//! issuance (reserved-path enforced), and sealed persistence.

use crate::willow::identity::{os_fill, SubspaceSecretExt as _};
use crate::willow::owned::OwnedRoot;
use crate::willow::WillowError;
use willow25::authorisation::WriteCapability;
use willow25::keys::SubspaceSecret;
use willow25::subspace::SubspaceId;

pub struct OwnedMasthead {
    root: OwnedRoot,
    owner_subspace_secret: SubspaceSecret,
}

impl OwnedMasthead {
    /// The owned namespace id (site root of trust).
    pub fn namespace_id(&self) -> &willow25::namespace::NamespaceId {
        self.root.namespace_id()
    }

    /// The owner's subspace id (the receiver of the owner write capability).
    pub fn owner_subspace_id(&self) -> SubspaceId {
        self.owner_subspace_secret.corresponding_subspace_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_masthead_has_owned_namespace_and_owner_subspace() {
        let m = OwnedMasthead::generate().expect("masthead");
        assert!(m.namespace_id().is_owned(), "masthead namespace must be owned");
        // owner subspace id is stable across calls
        assert_eq!(m.owner_subspace_id(), m.owner_subspace_id());
    }
}
```

> The exact import paths for `SubspaceSecret` / `SubspaceId` / `NamespaceId` and the `corresponding_subspace_id()` accessor must match how `identity.rs` imports them (it uses `willow25::...` prelude re-exports — mirror those exact `use` paths rather than guessing module names). `SubspaceSecretExt` is a placeholder for whatever trait exposes `corresponding_subspace_id()`; if it's an inherent method, drop the trait import.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core willow::masthead -- --nocapture`
Expected: FAIL — `no function generate` / unresolved imports.

- [ ] **Step 3: Write minimal implementation**

Add to `masthead.rs` (inside `impl OwnedMasthead`), mirroring `OwnedRoot::generate`'s entropy+zeroize style and `generate_communal_author`'s subspace-secret creation:
```rust
/// Generate a fresh masthead: a new owned namespace root + a fresh owner subspace secret.
pub fn generate() -> Result<Self, WillowError> {
    let root = OwnedRoot::generate()?;
    let mut seed = [0u8; 32];
    os_fill(&mut seed).map_err(|_| WillowError::EntropyUnavailable)?;
    let owner_subspace_secret = SubspaceSecret::from_bytes(&seed); // takes &[u8;32]
    seed.iter_mut().for_each(|b| *b = 0);
    Ok(Self { root, owner_subspace_secret })
}
```
In `mod.rs` add:
```rust
mod masthead;
pub use masthead::OwnedMasthead;
```

> Match `SubspaceSecret::from_bytes` to the actual constructor used in `identity.rs`'s `generate_communal_author` (it draws a subspace secret the same way — copy that exact call). If it uses a different helper (e.g. `generate_subspace_secret_with(entropy)`), reuse that.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core willow::masthead -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/willow/masthead.rs crates/riot-core/src/willow/mod.rs
git commit -m "feat(willow): OwnedMasthead::generate (owned root + owner subspace)"
```

---

## Task 4: `owner_write_capability` — mint the owner cap

**Files:**
- Modify: `crates/riot-core/src/willow/masthead.rs`

- [ ] **Step 1: Write the failing test**

Add to `masthead.rs` tests:
```rust
#[test]
fn owner_capability_is_owned_full_area_zero_delegation() {
    let m = OwnedMasthead::generate().unwrap();
    let cap = m.owner_write_capability();
    assert!(cap.is_owned(), "owner cap must be owned-rooted");
    assert!(cap.delegations().is_empty(), "owner cap must have zero delegations");
    assert_eq!(cap.granted_namespace(), m.namespace_id(), "cap namespace must be the site root");
    assert_eq!(cap.receiver(), &m.owner_subspace_id(), "cap receiver must be the owner subspace");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core willow::masthead::tests::owner_capability -- --nocapture`
Expected: FAIL — `no method owner_write_capability`.

- [ ] **Step 3: Write minimal implementation**

Add to `impl OwnedMasthead`:
```rust
/// Mint the owner's owned write capability (grants `Area::full()` over the site namespace).
pub fn owner_write_capability(&self) -> WriteCapability {
    WriteCapability::new_owned(self.root.namespace_secret_ref(), self.owner_subspace_id())
}
```

> `new_owned(&NamespaceSecret, SubspaceId)` type-checks because `NamespaceSecret: Signer<NamespaceSignature> + Keypair<VerifyingKey = NamespaceId>` (verified in willow25 `namespace_secret.rs`). `granted_namespace()` returns `&NamespaceId`; compare with `m.namespace_id()` which is `&NamespaceId` — adjust deref if the compiler wants `cap.granted_namespace() == m.namespace_id()` vs a `*` on either side.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core willow::masthead::tests::owner_capability -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/willow/masthead.rs
git commit -m "feat(willow): OwnedMasthead::owner_write_capability (new_owned)"
```

---

## Task 5: `delegate_section` — issue a section-scoped, time-boxed editor cap (reserved-path enforced)

**Files:**
- Modify: `crates/riot-core/src/willow/masthead.rs`
- Modify: `crates/riot-core/src/willow/mod.rs` (add `WillowError::DelegationAreaEscapesArticles`)

- [ ] **Step 1: Write the failing test**

Add to `masthead.rs` tests:
```rust
use willow25::groupings::area::Area;
use willow25::prelude::TimeRange; // TimeRange = WillowRange<Timestamp>, flat-exported; verify path
use willow25::paths::Path;
use crate::willow::site_paths::ARTICLES_COMPONENT;

fn a_time_range() -> TimeRange {
    // TimeRange::new(start: Timestamp, end: Option<Timestamp>) — None would be open-ended.
    TimeRange::new(0u64.into(), Some(u64::MAX.into())) // [now, expiry] stand-in
}

#[test]
fn delegate_section_under_articles_succeeds_and_scopes_receiver() {
    let m = OwnedMasthead::generate().unwrap();
    let editor = SubspaceSecret::from_bytes(&[7u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let area = Area::new(
        Some(editor_id),
        Path::from_slices(&[ARTICLES_COMPONENT, b"news"]).expect("path"),
        a_time_range(),
    );
    let cap = m.delegate_section(editor_id, area).expect("delegation under /articles must succeed");
    assert!(!cap.delegations().is_empty(), "delegated cap must carry a delegation link");
    assert_eq!(cap.receiver(), &editor_id, "final receiver must be the editor");
    assert_eq!(cap.granted_namespace(), m.namespace_id());
}

#[test]
fn delegate_escaping_articles_is_refused() {
    let m = OwnedMasthead::generate().unwrap();
    let editor = SubspaceSecret::from_bytes(&[9u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    // path targets /manifest — MUST be refused at issuance (belt).
    let bad_area = Area::new(
        Some(editor_id),
        Path::from_slices(&[crate::willow::site_paths::MANIFEST_COMPONENT]).expect("path"),
        a_time_range(),
    );
    assert!(
        matches!(
            m.delegate_section(editor_id, bad_area),
            Err(WillowError::DelegationAreaEscapesArticles)
        ),
        "a delegation whose area escapes /articles must be refused"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core willow::masthead::tests::delegate -- --nocapture`
Expected: FAIL — `no method delegate_section` and `no variant DelegationAreaEscapesArticles`.

- [ ] **Step 3: Write minimal implementation**

In `mod.rs`, add to the `WillowError` enum:
```rust
/// A section delegation was requested for an area whose path escapes `/articles/`.
DelegationAreaEscapesArticles,
```
In `masthead.rs`, add to `impl OwnedMasthead`:
```rust
/// Delegate a section-scoped, time-boxed write capability to an editor.
/// REFUSES (belt) any `new_area` whose path is not under `/articles/` — the owner
/// must never mint a cap that could reach `/manifest` or `/mod/`. The manifest/mod
/// validators re-check independently (suspenders, Units 1–2).
pub fn delegate_section(
    &self,
    editor_subspace_id: SubspaceId,
    new_area: Area,
) -> Result<WriteCapability, WillowError> {
    if !crate::willow::site_paths::is_under_articles(new_area.path()) {
        return Err(WillowError::DelegationAreaEscapesArticles);
    }
    let mut cap = self.owner_write_capability();
    cap.try_delegate(&self.owner_subspace_secret, new_area, editor_subspace_id)
        .map_err(|_| WillowError::DoesNotAuthorise)?;
    Ok(cap)
}
```

> `try_delegate(&mut self, keypair, new_area, new_receiver)` signs with the CURRENT receiver's keypair — here the owner's `SubspaceSecret` (the owner is the receiver of `owner_write_capability`). `new_area.path()` returns `&Path`. Confirm `Area::path()` accessor name against willow25 `area.rs:143`. Map `InvalidCapability` to a suitable `WillowError` (reuse `DoesNotAuthorise`, or add `DelegationRejected` if a distinct variant reads better — keep the test's `matches!` in sync).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core willow::masthead::tests::delegate -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/willow/masthead.rs crates/riot-core/src/willow/mod.rs
git commit -m "feat(willow): OwnedMasthead::delegate_section with reserved-path belt"
```

---

## Task 6: Sign/verify round-trip — owner authorises; delegated cap is scope-restricted

This is the spec §8.1 Unit 0 RED case "sign/verify round-trip" and §7 bullet 3 ("sign records with the owned root or a delegated cap"). It proves the minted caps actually *authorise* — not just that they have the right shape — and that a delegated `/articles` editor cap **cryptographically cannot** author a `/manifest` entry (the reserved-path model enforced at the cap level, complementing the issuance belt in Task 5). Uses the raw willow25 `entry.into_authorised_entry(&cap, &subspace_secret)` path (the repo's `authorise_entry` helper hard-codes a *communal* cap, so it can't be reused here) and the reusable `verify_entry` verifier (`mod.rs:122`).

**Files:**
- Modify: `crates/riot-core/src/willow/masthead.rs` (add `authorise_owner_entry` + tests)

- [ ] **Step 1: Write the failing test**

Add to `masthead.rs` tests (imports at top of the test module):
```rust
use crate::willow::{verify_entry, MANIFEST_COMPONENT};
use willow25::entry::Entry;

fn entry_in(namespace: &willow25::namespace::NamespaceId, subspace: SubspaceId, path: &[&[u8]]) -> Entry {
    Entry::builder()
        .namespace_id(namespace.clone())
        .subspace_id(subspace)
        .path(Path::from_slices(path).expect("path"))
        .timestamp(1_000u64)
        .payload(b"payload-bytes")
        .build()
}

#[test]
fn owner_capability_authorises_and_verifies() {
    let m = OwnedMasthead::generate().unwrap();
    // owner may write anywhere, including /manifest (owner cap area == full)
    let entry = entry_in(m.namespace_id(), m.owner_subspace_id(), &[MANIFEST_COMPONENT]);
    let authorised = m.authorise_owner_entry(entry.clone()).expect("owner authorises");
    assert!(verify_entry(&entry, authorised.authorisation_token()), "owner-signed entry must verify");
}

#[test]
fn delegated_editor_can_write_articles_but_not_manifest() {
    let m = OwnedMasthead::generate().unwrap();
    let editor = SubspaceSecret::from_bytes(&[11u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let area = Area::new(
        Some(editor_id),
        Path::from_slices(&[ARTICLES_COMPONENT, b"news"]).expect("path"),
        a_time_range(),
    );
    let editor_cap = m.delegate_section(editor_id, area).expect("delegate");

    // POSITIVE: an entry under /articles/news, signed by the editor, authorises.
    let good = entry_in(m.namespace_id(), editor_id, &[ARTICLES_COMPONENT, b"news", b"post-1"]);
    let authorised = good.clone()
        .into_authorised_entry(&editor_cap, &editor)
        .expect("editor authorises under /articles");
    assert!(verify_entry(&good, authorised.authorisation_token()));

    // NEGATIVE: the SAME editor cap cannot author a /manifest entry (outside granted area).
    let bad = entry_in(m.namespace_id(), editor_id, &[MANIFEST_COMPONENT]);
    assert!(
        bad.into_authorised_entry(&editor_cap, &editor).is_err(),
        "a delegated /articles cap must NOT authorise a /manifest write"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core willow::masthead::tests -- --nocapture`
Expected: FAIL — `no method authorise_owner_entry`.

- [ ] **Step 3: Write minimal implementation**

Add to `impl OwnedMasthead` (and `use willow25::authorisation::AuthorisedEntry;` + `use willow25::entry::Entry;` at the top of `masthead.rs`):
```rust
/// Sign an entry as the site owner (owner cap, granted `Area::full()`).
/// The signer is the owner's SubspaceSecret — the namespace secret is only used
/// when *minting* the cap, never when authorising an entry.
pub fn authorise_owner_entry(&self, entry: Entry) -> Result<AuthorisedEntry, WillowError> {
    entry
        .into_authorised_entry(&self.owner_write_capability(), &self.owner_subspace_secret)
        .map_err(|_| WillowError::DoesNotAuthorise)
}
```

> Confirm the `AuthorisedEntry` / `Entry` import paths against how `mod.rs` re-exports them (`pub use willow25::...`), and that `authorised.authorisation_token()` is the accessor `verify_entry` expects (`mod.rs:122` + `entry.rs:94`). The negative case relies on `into_authorised_entry` returning `Err(DoesNotAuthorise)` when the path is outside the cap's granted area — confirmed against willow25 `entry.rs:190` + `does_authorise` (`authorisation_token.rs:231`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core willow::masthead::tests -- --nocapture`
Expected: PASS (both new tests).

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/willow/masthead.rs
git commit -m "feat(willow): owner entry signing + sign/verify round-trip (editor scope-restricted)"
```

---

## Task 7: Seal / open the owned masthead root secret

**Files:**
- Modify: `crates/riot-core/src/willow/masthead.rs`
- Modify: `crates/riot-core/src/willow/mod.rs` (add `WillowError::SealedMastheadInvalid` if a distinct variant is wanted)

- [ ] **Step 1: Write the failing test**

Add to `masthead.rs` tests:
```rust
#[test]
fn sealed_masthead_roundtrips_and_hides_secrets() {
    let m = OwnedMasthead::generate().unwrap();
    let ns = *m.namespace_id();
    let owner = m.owner_subspace_id();
    let key = [0x5a; 32];

    let sealed = m.seal(&key).expect("seal");
    // secrets must not appear in cleartext in the sealed blob
    assert!(sealed.windows(32).all(|w| w != m.root.namespace_secret_ref().as_bytes()));

    let restored = OwnedMasthead::open_sealed(&key, &sealed).expect("open");
    assert_eq!(*restored.namespace_id(), ns, "namespace id must survive seal roundtrip");
    assert_eq!(restored.owner_subspace_id(), owner, "owner subspace must survive roundtrip");
}

#[test]
fn open_sealed_masthead_rejects_wrong_key() {
    let m = OwnedMasthead::generate().unwrap();
    let sealed = m.seal(&[0x01; 32]).unwrap();
    assert!(
        matches!(
            OwnedMasthead::open_sealed(&[0x02; 32], &sealed),
            Err(WillowError::SealedMastheadInvalid)
        ),
        "opening with the wrong wrapping key must fail closed"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core willow::masthead::tests -- --nocapture`
Expected: FAIL — `no method seal` / `open_sealed` / `no variant SealedMastheadInvalid`.

- [ ] **Step 3: Write minimal implementation**

Mirror `EvidenceAuthor::seal_identity` / `open_sealed_identity` (`identity.rs:86,114`) — XChaCha20Poly1305, a distinct magic + AAD, plaintext = `namespace_secret (32) ‖ owner_subspace_secret (32)`. Unlike `open_sealed_identity`, this envelope is for an OWNED namespace, so it does NOT reject owned ids.
```rust
const MASTHEAD_MAGIC: &[u8] = b"RIOTMH\x01\0";
const MASTHEAD_AAD: &[u8] = b"riot/owned-masthead/sealed/v1";
const SEALED_MASTHEAD_PLAINTEXT: usize = 64;

impl OwnedMasthead {
    pub fn seal(&self, wrapping_key: &[u8; 32]) -> Result<Vec<u8>, WillowError> {
        let mut plaintext = [0u8; SEALED_MASTHEAD_PLAINTEXT];
        plaintext[..32].copy_from_slice(self.root.namespace_secret_ref().as_bytes());
        plaintext[32..].copy_from_slice(self.owner_subspace_secret.as_bytes());
        // ... same XChaCha20Poly1305 seal as identity.rs::seal_identity, with
        //     MASTHEAD_MAGIC prefix and MASTHEAD_AAD; zeroize plaintext after.
        // Return magic ‖ nonce ‖ ciphertext+tag.
        todo_seal(MASTHEAD_MAGIC, MASTHEAD_AAD, wrapping_key, &plaintext)
    }

    pub fn open_sealed(wrapping_key: &[u8; 32], sealed: &[u8]) -> Result<Self, WillowError> {
        // verify MASTHEAD_MAGIC; XChaCha20Poly1305 open with MASTHEAD_AAD;
        // split 64-byte plaintext into namespace_secret ‖ owner_subspace_secret;
        // reconstruct OwnedRoot from the namespace secret and OwnedMasthead from both.
        // On any failure return Err(WillowError::SealedMastheadInvalid).
    }
}
```

> The `todo_seal`/prose lines above are the ONE place this plan defers to the concrete crypto in `identity.rs` — the implementer copies `seal_identity`/`open_sealed_identity` byte-for-byte, swapping magic/AAD/plaintext-layout. Add `OwnedRoot::from_namespace_secret(secret) -> Result<Self, WillowError>` in `owned.rs` (re-deriving `namespace_id` via `secret.corresponding_namespace_id()`, asserting `is_owned()`) so `open_sealed` can rebuild the root. Add `WillowError::SealedMastheadInvalid`. **Do not leave `todo_seal` in the committed code** — it must be the real implementation before Step 4.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core willow::masthead::tests -- --nocapture`
Expected: PASS (all masthead tests).

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/src/willow/masthead.rs crates/riot-core/src/willow/owned.rs crates/riot-core/src/willow/mod.rs
git commit -m "feat(willow): seal/open OwnedMasthead root secret (owned envelope)"
```

---

## Task 8: Integration test — full owner lifecycle

**Files:**
- Create: `crates/riot-core/tests/owned_masthead.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Integration: an owner generates a masthead, mints its cap, delegates a section
//! editor cap under /articles, and seals/restores the root.
use riot_core::willow::{is_under_articles, OwnedMasthead, ARTICLES_COMPONENT, MANIFEST_COMPONENT};
use willow25::entry::Entry;
use willow25::groupings::area::Area;
use willow25::prelude::TimeRange;
use willow25::keys::SubspaceSecret;
use willow25::paths::Path;

#[test]
fn owner_lifecycle_mint_delegate_seal_restore() {
    let m = OwnedMasthead::generate().expect("masthead");
    assert!(m.namespace_id().is_owned());

    // owner cap
    let owner_cap = m.owner_write_capability();
    assert!(owner_cap.is_owned() && owner_cap.delegations().is_empty());

    // delegate a Culture-section editor
    let editor = SubspaceSecret::from_bytes(&[3u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let area = Area::new(
        Some(editor_id),
        Path::from_slices(&[ARTICLES_COMPONENT, b"culture"]).expect("path"),
        TimeRange::new(0u64.into(), Some(u64::MAX.into())),
    );
    assert!(is_under_articles(area.path()));
    let editor_cap = m.delegate_section(editor_id, area).expect("delegate");
    assert_eq!(editor_cap.receiver(), &editor_id);

    // sign/verify: owner authorises a manifest write; editor cap does not
    let owner_entry = Entry::builder()
        .namespace_id(m.namespace_id().clone())
        .subspace_id(m.owner_subspace_id())
        .path(Path::from_slices(&[MANIFEST_COMPONENT]).expect("path"))
        .timestamp(1u64).payload(b"manifest-bytes").build();
    assert!(m.authorise_owner_entry(owner_entry).is_ok(), "owner authorises /manifest");

    // seal + restore
    let key = [0x77; 32];
    let sealed = m.seal(&key).unwrap();
    let restored = OwnedMasthead::open_sealed(&key, &sealed).unwrap();
    assert_eq!(*restored.namespace_id(), *m.namespace_id());
    assert_eq!(restored.owner_subspace_id(), m.owner_subspace_id());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-core --test owned_masthead -- --nocapture`
Expected: FAIL until all prior tasks are merged (should PASS once Tasks 1–7 are in; this test is the guard that they compose).

- [ ] **Step 3: (no new impl)** — this task is a composition guard; if it fails, the defect is in Tasks 1–7, fix there.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-core --test owned_masthead -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/riot-core/tests/owned_masthead.rs
git commit -m "test(willow): owner lifecycle integration (mint/delegate/seal/restore)"
```

---

## Task 9: FFI surface — create site, restore

**Files:**
- Create: `crates/riot-ffi/src/site_ffi.rs`
- Modify: `crates/riot-ffi/src/lib.rs` (add `mod site_ffi;`)

- [ ] **Step 1: Write the failing test**

In `site_ffi.rs`, an in-memory FFI test mirroring `mobile_state` test style:
```rust
//! FFI: owner-side site creation + delegation issuance. Secrets are sealed with a
//! native-provided wrapping key and never cross the boundary unsealed.

#[derive(uniffi::Record)]
pub struct CreatedSite {
    /// Owned namespace id (site root), hex.
    pub namespace_id: String,
    /// Owner subspace id, hex.
    pub owner_subspace_id: String,
    /// Sealed masthead root — opaque; persist via the durable profile.
    pub sealed_root: Vec<u8>,
}

#[derive(uniffi::Record)]
pub struct EditorDelegation {
    /// Canonical-encoded delegated WriteCapability for the editor.
    pub encoded_capability: Vec<u8>,
    /// Editor subspace id the cap was scoped to, hex.
    pub editor_subspace_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_site_returns_owned_namespace_and_sealed_root() {
        let key = vec![0x22u8; 32];
        let site = create_owned_site(key.clone()).expect("create site");
        assert_eq!(site.namespace_id.len(), 64, "hex-encoded 32-byte id");
        assert!(!site.sealed_root.is_empty(), "sealed root must be present");
        // restoring with the same key yields the same namespace id
        let restored_ns = restore_owned_site(key, site.sealed_root.clone())
            .expect("restore")
            .namespace_id;
        assert_eq!(restored_ns, site.namespace_id);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p riot-ffi site_ffi -- --nocapture`
Expected: FAIL — `create_owned_site` / `restore_owned_site` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `site_ffi.rs` (free `#[uniffi::export]` fns, hex helpers as in `mobile_state`):
```rust
use riot_core::willow::OwnedMasthead;
use crate::mobile_api::MobileError;

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }

fn exact_key(k: &[u8]) -> Result<[u8; 32], MobileError> {
    <[u8; 32]>::try_from(k).map_err(|_| MobileError::InvalidInput)
}

#[uniffi::export]
pub fn create_owned_site(wrapping_key: Vec<u8>) -> Result<CreatedSite, MobileError> {
    let key = exact_key(&wrapping_key)?;
    let m = OwnedMasthead::generate().map_err(|_| MobileError::InvalidInput)?;
    let sealed = m.seal(&key).map_err(|_| MobileError::InvalidInput)?;
    Ok(CreatedSite {
        namespace_id: hex(m.namespace_id().as_bytes()),
        owner_subspace_id: hex(&subspace_id_bytes(&m.owner_subspace_id())),
        sealed_root: sealed,
    })
}

#[uniffi::export]
pub fn restore_owned_site(wrapping_key: Vec<u8>, sealed_root: Vec<u8>) -> Result<CreatedSite, MobileError> {
    let key = exact_key(&wrapping_key)?;
    let m = OwnedMasthead::open_sealed(&key, &sealed_root).map_err(|_| MobileError::InvalidInput)?;
    Ok(CreatedSite {
        namespace_id: hex(m.namespace_id().as_bytes()),
        owner_subspace_id: hex(&subspace_id_bytes(&m.owner_subspace_id())),
        sealed_root, // echo back; caller already holds it
    })
}
```
Add a `subspace_id_bytes(&SubspaceId) -> [u8;32]` helper (mirror how `identity.rs` turns a subspace id into `[u8;32]` for `AuthorIdentity.subspace_id`). In `lib.rs` add `mod site_ffi;`. Zeroize `key` after use (follow the `wrapping_key.zeroize()` pattern in `mobile_state.rs:331`).

> **Delegation-issuance FFI** (`issue_editor_delegation`) is deferred to the start of Unit 6 (it needs the editor's presented subspace id from the invite handshake + a section string → path mapping, which is UI-driven). Unit 0's FFI ships site creation + restore; that is enough to unblock Units 1–2 (which consume `OwnedMasthead` directly in-crate, not via FFI).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p riot-ffi site_ffi -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit + regenerate bindings**

Per the UniFFI record-change coupling hazard (new `uniffi::Record` types), regenerate bindings and rebuild the native staticlib in the SAME commit — a mismatch is a runtime checksum abort, not a compile error.
```bash
# regenerate bindings + rebuild staticlib per repo's generate-bindings task, then:
git add crates/riot-ffi/src/site_ffi.rs crates/riot-ffi/src/lib.rs <regenerated bindings>
git commit -m "feat(ffi): create/restore owned site (sealed root, no secrets across FFI)"
```

---

## Task 10: Workspace gates

- [ ] **Step 1: fmt + clippy + full test**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
```
Expected: all clean/green.

- [ ] **Step 2: Coverage floor**

Run: `cargo tarpaulin --workspace --all-features --fail-under $(jq -r '.tarpaulin.lines' .coverage-thresholds.json)`
Expected: at/above the ratchet floor (Unit 0 is pure Rust with dense unit tests; it should raise, not lower, coverage). If it lowers a floor, add tests — never lower a floor without committed justification (CLAUDE.md).

- [ ] **Step 3: Commit any test top-ups**

```bash
git add -A && git commit -m "test(willow): coverage top-up for owned masthead"
```

---

## Self-Review

**Spec coverage (§7 Owner-side capability plumbing + §8.1 Unit 0 RED cases):** mint owned write cap ✅ Task 4; issue section-scoped delegations ✅ Task 5; hard-refuse areas escaping `/articles/` (issuance belt) ✅ Task 5; **sign/verify round-trip** ✅ **Task 6** (`authorise_owner_entry` signs with the owner cap and `verify_entry` confirms it authorises; a delegated `/articles` editor cap is proven to authorise a `/articles` write and to be *rejected* for a `/manifest` write — the reserved-path model enforced cryptographically, the suspenders to Task 5's belt); FFI create/restore ✅ Task 9 (delegation-issuance FFI deferred to Unit 6 with rationale: needs the UI-driven invite handshake); root secret at-rest sealed (keystore-backed via native wrapping key), never plaintext, never crosses FFI unsealed ✅ Task 7 + Task 9.

**Placeholder scan:** one intentional deferral — the `todo_seal` prose in Task 7 Step 3 points at the concrete `identity.rs` crypto to copy; flagged "do not leave in committed code." All other steps carry real code.

**Type consistency:** `OwnedMasthead` methods (`namespace_id`, `owner_subspace_id`, `owner_write_capability`, `delegate_section`, `authorise_owner_entry`, `seal`, `open_sealed`) are used consistently across Tasks 3–9 and the integration test. `is_under_articles(&Path)` (Task 1) is the single gate reused in Task 5 and the integration test. New `WillowError` variants (`DelegationAreaEscapesArticles`, `SealedMastheadInvalid`) are defined before use. `authorise_owner_entry` (Task 6) is exercised again in the Task 8 integration test.

**Open verification points for the implementer (willow25 accessor names — pinned by tests, adjust impl if the crate differs):** `Path::from_slices` returns `Result` (→ `.expect(...)`, all test call sites); `SubspaceSecret::from_bytes(&[u8;32])` + `corresponding_subspace_id()` (Tasks 3,5,6); `Path::components()`/`.as_ref()` (Task 1); `Area::new` / `Area::path()` + `TimeRange::new` (Tasks 5,6); `granted_namespace()`/`receiver()` deref shapes (Task 4); `Entry::builder()` chain + `into_authorised_entry(&cap, &SubspaceSecret)` + `authorisation_token()` (Task 6, verified against the entry-fixture flow in `crates/riot-core/tests/public_willow.rs` and `public_bundle.rs`). If an accessor name differs, the test stays and the impl line adjusts.
