# Composite Owner `/articles` Write + Manifest Sections + Reader — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a composite-site owner publish a signed manifest declaring named sections, author rich editorial articles (headline/dek/body/byline/section) validated against those sections, and read them back through a section-grouped, moderation-honest feed — with a native reader + owner compose surface.

**Architecture:** Mirror the proven `/mod` sibling chain end to end — pure canonical-CBOR record + codec (`riot-core`), owner signer via `authorise_owner_entry` (`riot-core`), store-coupled projection + admission (`riot-ffi`, beside `resolve_composite_site_from_store`), UniFFI surface, native shells (no business logic). No new transport, no new family plumbing (the `/articles` classifier, offer export, and bundle import already exist).

**Tech Stack:** Rust 2021 (`riot-core`, `riot-ffi`), minicbor canonical codec, willow25 meadowcap (`OwnedMasthead`), UniFFI, Swift 6 / SwiftUI (iOS + macOS), Kotlin (Android host-logic parity).

**Design:** `docs/superpowers/specs/2026-07-19-owner-articles-write-design.md` (rev-8, design-review gate PASSED). Read it — §7 lists every acceptance test, §6 the invariants, §9 the risks.

**Baseline / prerequisites:**
- Branch `feat/composite-owner-articles-write` (off `origin/main`). Green baseline: `cargo test -p riot-core -p riot-ffi` passes before starting.
- Sibling references to mirror (READ these first): `crates/riot-core/src/site/moderation.rs` (codec + `MAX_*_BYTES` + `prove_canonical`), `crates/riot-core/src/site/moderation_entry.rs` (`create_signed_moderation_record`), `crates/riot-ffi/src/site_ffi.rs` (`create_site_moderation_action`/`author_moderation` ~652-700, `import_owned_mod` ~797-821, `resolve_composite_site_from_store` ~474-589), `crates/riot-core/src/site/manifest.rs` (codec: `encode_site_manifest`/`decode_site_manifest`, `decode_members` bound-before-allocate idiom), `crates/riot-core/src/site/version_floor.rs` (`admit_manifest_version`, `manifest_identity`).
- **Cargo.lock contract:** this plan adds NO new crate dependencies, so the `validate-contracts` sha256 pin is unaffected. If a task ever adds a dep, update `fixtures/manifest.json` per the printed `actual`.
- **UniFFI coupling:** Units 0(b) and 4 add `#[uniffi::export]` methods/records. Their binding regen (`cargo run -p xtask -- generate-bindings`) + native staticlib rebuild (`sh scripts/conference/build-native-core.sh` — Apple slices at minimum) MUST land together (checksum-abort otherwise). Units 0(a),1,2,3 are pure Rust / non-exported FFI helpers — no regen.

---

## File structure

| File | New/Mod | Responsibility |
|---|---|---|
| `crates/riot-core/src/site/manifest.rs` | Mod | `sections` field (CBOR key 7, omit-when-empty, bound-before-allocate), `section_is_declared`, `MAX_SECTION_BYTES`/`MAX_SECTIONS` |
| `crates/riot-core/src/site/article.rs` | New | `OwnedArticleV1` record + canonical `encode_article`/`decode_article(path,payload)` + `MAX_*_BYTES` + `ArticleRecordError` |
| `crates/riot-core/src/site/article_entry.rs` | New | `create_signed_article` (owner-signed at `/articles/<section>/<time+digest>`) + `SignedArticleRecord` + `ArticleSignError` |
| `crates/riot-core/src/site/article_render.rs` | New | pure `article_feed_render(&CompositeDegradation) -> ArticleFeedRender` + `ResolvedArticleV1` value type + redaction |
| `crates/riot-core/src/site/mod.rs` | Mod | re-export the new modules/types |
| `crates/riot-ffi/src/site_ffi.rs` | Mod | `resolve_site_degradation` (extracted), `resolve_article_feed_from_store`, `import_owned_article`, `publish_site_manifest`, `create_site_article`, `resolve_site_articles`, FFI records/enums |
| `crates/riot-core/tests/composite_admission.rs` | Mod | forged/foreign-cap-refused-admission test for `/articles` (Task 3.6) |
| `apps/ios/Riot/CompositeArticleReader.swift` | New | pure reader mapping (feed→rows) + `CompositeArticleReaderView` |
| `apps/ios/Riot/OwnedArticleCompose.swift` | New | owner compose (publish-manifest/section + author + pre-publish preview), seizure-gated |
| `apps/android/app/src/main/kotlin/org/riot/evidence/CompositeArticle.kt` | New | pure Kotlin twin (reader mapping + compose-gate logic) |
| `apps/ios/RiotTests/…`, `apps/android/…/test/…` | New | native unit tests |
| both `*.xcodeproj/project.pbxproj` | Mod | register new Swift files (iOS app + macOS) |

Sequential dependency: **0 → 1 → 2 → 3 → 4 → {5a, 5b}**. 5a (reader) needs only Unit 4; 5b (compose) needs Unit 4 **and PR #68** (the seizure gate `SiteSeizureDisclosure`/`OwnedSiteCreationGate`).

---

## Unit 0 — Manifest sections + authoring

### Task 0.1: `sections` field + `MAX_SECTION_BYTES`/`MAX_SECTIONS` constants (schema)

**Files:** Modify `crates/riot-core/src/site/manifest.rs`; Test: same file's `#[cfg(test)]`.

**PRE-REQ (Feasibility): `manifest.rs` has NO `#[cfg(test)]` module today.** Task 0.1 builds it: add `#[cfg(test)] mod tests { use super::*; fn sample_manifest() -> SiteManifestV1 { SiteManifestV1 { root: [7u8;32], members: vec![/* one Masthead member, mirror the existing manifest literals grepped elsewhere */], moderation_path: vec![b"mod".to_vec()], transport_policy: TransportPolicyV1{ allow: vec![], require: RequireTransport::None }, version: 1, sections: vec![] } } }`. All new tests live here.

- [ ] **Step 1 — RED test: a current 7-key manifest still decodes, absent key 7 → empty sections.**
```rust
#[test]
fn manifest_without_sections_decodes_as_empty_and_round_trips() {
    let m = sample_manifest();
    let bytes = encode_site_manifest(&m).unwrap();
    let decoded = decode_site_manifest(&bytes).unwrap();
    assert!(decoded.sections.is_empty());
    assert_eq!(encode_site_manifest(&decoded).unwrap(), bytes); // byte-identical
}
```
- [ ] **Step 2 — Run, expect FAIL** (`sections` field doesn't exist): `cargo test -p riot-core manifest_without_sections`. Expected: compile error `no field sections`.
- [ ] **Step 3 — Add the field + constants.** In `SiteManifestV1` add `pub sections: Vec<Vec<u8>>,`. Add near `MAX_*` consts: `pub const MAX_SECTION_BYTES: usize = 64;` and `pub const MAX_SECTIONS: usize = 64;`. Update every existing `SiteManifestV1 { .. }` literal in the file/tests to include `sections: vec![]` (grep `SiteManifestV1 {`).
- [ ] **Step 4 — Encode: dynamic map length, omit-when-empty.** In `encode_site_manifest`, change `e.map(7)?` to a computed length: `let pairs = if manifest.sections.is_empty() { 7 } else { 8 }; e.map(pairs)?;`. After the key-6 (`layout`) emit, add (only when non-empty):
```rust
if !manifest.sections.is_empty() {
    e.u8(7)?.array(manifest.sections.len() as u64)?;
    for s in &manifest.sections { e.bytes(s)?; }
}
```
- [ ] **Step 5 — Decode: default before loop, bound-before-allocate, add key-7 arm.** In `decode_site_manifest`, before the key loop initialize `let mut sections: Vec<Vec<u8>> = Vec::new();` (default empty — NOT an `Option` with `MissingKey`). Bump the pairs guard `if pairs > 7` → `if pairs > 8`. Add a match arm mirroring `decode_members`' bound-before-allocate idiom:
**Use the REAL `SiteManifestError` variants (Feasibility): `{InputTooLarge, TooManyEntries(&'static str), FieldTooLarge(&'static str), UnknownKey(u64), DuplicateOrMisorderedKey(u64), MissingKey(u64), WrongSchema, InvalidEnum(&'static str), NonCanonical, TrailingBytes, Malformed}`** — there is NO `IndefiniteLength`/`TooLong`/`NotCanonical`. Mirror `decode_members`' exact idiom:
```rust
7 => {
    let len = d.array()?.ok_or(SiteManifestError::Malformed)?;      // indefinite length → Malformed (mirror decode_members)
    if len as usize > MAX_SECTIONS { return Err(SiteManifestError::TooManyEntries("sections")); }
    let mut v = Vec::with_capacity(len as usize);
    for _ in 0..len {
        let s = d.bytes()?.to_vec();
        if s.is_empty() || s.len() > MAX_SECTION_BYTES { return Err(SiteManifestError::FieldTooLarge("section")); }
        v.push(s);
    }
    sections = v;
}
```
Assign `sections` into the constructed `SiteManifestV1` (no `.ok_or(MissingKey)` — it defaults empty). Keep the trailing `prove_canonical(input, encode_site_manifest(&manifest)?)?;` unchanged — it now rejects a present-but-empty key 7 for free (encode omits it, so re-encode differs → `NonCanonical`). (Confirm `decode_members`' actual variant choices when implementing and match them.)
- [ ] **Step 6 — Run, expect PASS**: `cargo test -p riot-core manifest_without_sections`.
- [ ] **Step 7 — Commit**: `git add crates/riot-core/src/site/manifest.rs && git commit -m "feat(site): manifest sections field (CBOR key 7, omit-when-empty)"`

### Task 0.2: Canonicity + bound tests (non-empty round-trip, reject present-empty, identity, DoS)

- [ ] **Step 1 — RED tests:**
```rust
#[test]
fn non_empty_sections_round_trip() {
    let mut m = sample_manifest();
    m.sections = vec![b"news".to_vec(), b"analysis".to_vec()];
    let bytes = encode_site_manifest(&m).unwrap();
    assert_eq!(decode_site_manifest(&bytes).unwrap().sections, m.sections);
    assert_eq!(encode_site_manifest(&decode_site_manifest(&bytes).unwrap()).unwrap(), bytes);
}
#[test]
fn key7_present_but_empty_array_is_rejected_non_canonical() {
    // hand-encode a manifest that emits key 7 with a 0-length array
    let bytes = encode_manifest_with_forced_empty_sections_key(); // test helper: map(8) + empty key-7 array
    assert!(matches!(decode_site_manifest(&bytes), Err(SiteManifestError::NonCanonical)));
}
#[test]
fn oversize_section_count_rejected_before_alloc() {
    let bytes = encode_manifest_claiming_section_count(MAX_SECTIONS as u64 + 1); // header claims too many
    assert!(matches!(decode_site_manifest(&bytes), Err(SiteManifestError::TooManyEntries("sections"))));
}
```
NOTE (Feasibility): `manifest_identity` is **private** (`version_floor.rs:106`), takes `&SiteManifestV1` (not bytes), and has an unbound generic — it CANNOT be called from `manifest.rs`'s test module. The "identity differs iff sections differ" property is therefore verified **behaviorally** at the floor level in Task 0.4's `same_version_conflicting_content_is_equivocation_alarm` test (two same-version manifests differing only in `sections` → `EquivocationAlarm`), not by a direct call here.
- [ ] **Step 2 — Run, expect FAIL** (helpers/reject-path missing).
- [ ] **Step 3 — Implement** the two tiny test helpers (`encode_manifest_with_forced_empty_sections_key`, `encode_manifest_claiming_section_count`) in the test module using a raw `minicbor::Encoder`. The reject paths are already implemented in Task 0.1 (bound-before-alloc → `TooManyEntries`) and by `prove_canonical` (present-empty → `NonCanonical`).
- [ ] **Step 4 — Run, expect PASS**. **Step 5 — Commit**: `feat(site): manifest sections canonicity + DoS-bound tests`.

### Task 0.3: `section_is_declared` shared validator

**Files:** Modify `crates/riot-core/src/site/manifest.rs`.
- [ ] **Step 1 — RED test:**
```rust
#[test]
fn section_is_declared_accepts_declared_rejects_undeclared() {
    let mut m = sample_manifest(); m.sections = vec![b"news".to_vec()];
    assert!(section_is_declared(&m, b"news"));
    assert!(!section_is_declared(&m, b"sports"));
    assert!(!section_is_declared(&m, b"")); // empty never declared
}
```
- [ ] **Step 2 — Run FAIL. Step 3 — Implement:** `pub fn section_is_declared(m: &SiteManifestV1, section: &[u8]) -> bool { !section.is_empty() && m.sections.iter().any(|s| s == section) }`.
- [ ] **Step 4 — PASS. Step 5 — Commit**: `feat(site): section_is_declared shared validator`.

### Task 0.4: `publish_site_manifest` FFI (owner-sign → version-floor-gated commit)

**Files:** Modify `crates/riot-ffi/src/site_ffi.rs` (+ `ResolvedSiteManifest.sections`). Mirror `author_moderation` (open_sealed → build → sign → import) but gate on the floor.

- [ ] **Step 1 — RED tests** (in `site_ffi.rs` `#[cfg(test)]`, DURABLE profile — the floor needs `LocalProfile.db`; mirror the existing `durable_profile()` helper at `site_ffi.rs:1731`). **Store-unchanged is asserted via `resolve_site_articles`/`resolve_site_manifest` observable state, NOT a nonexistent `debug_store_bytes` — assert the manifest the resolver returns is still the pre-rejection one (same version/sections):**
```rust
fn declared(feed_or_manifest) -> (u64, Vec<String>) { /* pull (version, sections) from resolve_site_manifest */ }

#[test]
fn publish_then_higher_version_admits_lower_rolls_back() {
    let (profile, key, sealed) = durable_owned_site();  // wraps durable_profile() + create_owned_site
    let out1 = profile.publish_site_manifest(sealed.clone(), key.clone(), members(), vec![b"news".to_vec()], transport(), 1).unwrap();
    assert_eq!(out1.sections, vec!["news".to_string()]);
    profile.publish_site_manifest(sealed.clone(), key.clone(), members(), vec![b"news".to_vec(), b"ops".to_vec()], transport(), 2).unwrap();  // higher admits
    let before = declared(resolve_manifest(&profile));  // (2, [news, ops])
    assert!(matches!(profile.publish_site_manifest(sealed.clone(), key.clone(), members(), vec![b"news".to_vec()], transport(), 1), Err(MobileError::ManifestRollback)));
    assert_eq!(declared(resolve_manifest(&profile)), before);  // unchanged: still v2
}
#[test]
fn require_downgrade_rejected_leaves_manifest_unchanged() {
    // publish v1 with require: Arti (strict), then attempt a higher version that lowers require to None
    let (profile, key, sealed) = durable_owned_site();
    profile.publish_site_manifest(sealed.clone(), key.clone(), members(), vec![b"news".to_vec()], transport_arti(), 1).unwrap();
    let before = declared(resolve_manifest(&profile));
    assert!(matches!(profile.publish_site_manifest(sealed.clone(), key.clone(), members(), vec![b"news".to_vec()], transport_none(), 2), Err(MobileError::ManifestRequireDowngrade)));
    assert_eq!(declared(resolve_manifest(&profile)), before);  // unchanged
}
#[test]
fn same_version_conflicting_content_is_equivocation_alarm() {
    let (profile, key, sealed) = durable_owned_site();
    profile.publish_site_manifest(sealed.clone(), key.clone(), members(), vec![b"news".to_vec()], transport(), 1).unwrap();
    let before = declared(resolve_manifest(&profile));
    // SAME version 1, different sections → conflicting content at the same version
    assert!(matches!(profile.publish_site_manifest(sealed.clone(), key.clone(), members(), vec![b"news".to_vec(), b"ops".to_vec()], transport(), 1), Err(MobileError::ManifestEquivocation)));
    assert_eq!(declared(resolve_manifest(&profile)), before);  // unchanged; alarm surfaced distinctly (own variant)
}
#[test]
fn publish_leaves_sync_inventory_unchanged() {
    // mirror importing_a_mod_bundle_leaves_the_sync_inventory_unchanged: snapshot sync_inventory, publish, assert equal
    let (profile, key, sealed) = durable_owned_site();
    let before = with_active(&profile.inner, |p| Ok(p.sync_inventory.len())).unwrap();  // inline like importing_a_mod_bundle_leaves_the_sync_inventory_unchanged (no named accessor exists)
    profile.publish_site_manifest(sealed, key, members(), vec![b"news".to_vec()], transport(), 1).unwrap();
    assert_eq!(with_active(&profile.inner, |p| Ok(p.sync_inventory.len())).unwrap(), before);
}
#[test]
fn publish_on_in_memory_profile_fails_closed() {
    // durable-only: LocalProfile.db is None for an in-memory profile → publish returns an error, never a silent no-op
    let (profile, key, sealed) = in_memory_owned_site();
    assert!(profile.publish_site_manifest(sealed, key, members(), vec![b"news".to_vec()], transport(), 1).is_err());
}
```
(`durable_owned_site`/`in_memory_owned_site` are thin test helpers wrapping the existing `durable_profile()`/`open_local_profile()` + `create_owned_site`; the sync_inventory snapshot uses the inline `with_active(&profile.inner, |p| Ok(p.sync_inventory.len()))` pattern from the sibling test — no named accessor exists.)
- [ ] **Step 2 — Run FAIL** (`publish_site_manifest` undefined).
- [ ] **Step 3 — Implement.** Add the `#[uniffi::export]` method on `MobileProfile` per design §4.0. Skeleton (fill from `author_moderation` + `admit_manifest_version`):
```rust
pub fn publish_site_manifest(&self, sealed_root: Vec<u8>, mut wrapping_key: Vec<u8>,
    members: Vec<SiteMemberInput>, sections: Vec<Vec<u8>>, transport: TransportPolicyInput, version: u64)
    -> Result<SiteManifestOutcome, MobileError> {
    let mut key = exact_key(&wrapping_key)?; wrapping_key.iter_mut().for_each(|b| *b = 0);
    let masthead = OwnedMasthead::open_sealed(&key, &sealed_root).map_err(|_| MobileError::InvalidInput)?;
    key.iter_mut().for_each(|b| *b = 0);
    let root = *masthead.namespace_id().as_bytes();
    let manifest = SiteManifestV1 { root, members: members.into_core()?, moderation_path: vec![b"mod".to_vec()],
        transport_policy: transport.into_core()?, sections, version };
    let payload = encode_site_manifest(&manifest).map_err(|_| MobileError::InvalidInput)?;
    let signed = /* authorise_owner_entry at [MANIFEST_COMPONENT] over payload — mirror author_moderation's signing */;
    with_active(&self.inner, |profile| {
        // Durable-only: the floor lives on LocalProfile.db (RiotDatabase impls VersionFloorStore, version_floor.rs:176),
        // NOT profile.store (EvidenceStore). In-memory profiles (db == None) fail closed.
        let db = profile.db.as_ref().ok_or(MobileError::InvalidInput /* or a DurableRequired variant */)?;
        // GATE on the floor BEFORE any commit (confirm admit_manifest_version's exact arg shape when implementing):
        match admit_manifest_version(db, &root, &manifest)?  // match admit_manifest_version(db, root, &manifest)?root: match admit_manifest_version(db, root, &manifest)?[u8;32] {
            VersionFloorOutcome::Accepted => {}
            VersionFloorOutcome::RollbackRejected => return Err(MobileError::ManifestRollback),
            VersionFloorOutcome::RequireDowngradeRejected => return Err(MobileError::ManifestRequireDowngrade),
            VersionFloorOutcome::EquivocationAlarm => return Err(MobileError::ManifestEquivocation),
        }
        import_owned_manifest(profile, root, &signed)?; // sibling of import_owned_mod; MUST NOT touch sync_inventory
        Ok(SiteManifestOutcome { root: hex(&root), version, sections: manifest.sections.iter().map(|s| String::from_utf8_lossy(s).into_owned()).collect() })
    })
}
```
Add the new `MobileError` variants (`ManifestRollback`, `ManifestRequireDowngrade`, `ManifestEquivocation`) and the `SiteManifestOutcome` `uniffi::Record`. Add `sections: Vec<String>` to `ResolvedSiteManifest` and populate it in `resolve_site_manifest`. **Do NOT** copy `import_owned_mod`'s unconditional commit — the match above must return before `import_owned_manifest` on any non-`Accepted` outcome.
- [ ] **Step 4 — Run PASS. Step 5 — regen+rebuild** (batches with Unit 4; see Task 4.5) — for now just `cargo test -p riot-ffi publish_`. **Step 6 — Commit**: `feat(ffi): publish_site_manifest — owner-signed, version-floor-gated, sections`.

---

## Unit 1 — `OwnedArticleV1` record + codec (`crates/riot-core/src/site/article.rs`)

Mirror `moderation.rs` exactly (schema tag, `Encoder`/`Decoder`, `prove_canonical`, `MAX_*_BYTES`, closed `ArticleRecordError`).

### Task 1.1: struct + bounds constants + error enum
- [ ] **Step 1 — RED test** (`article.rs` `#[cfg(test)]`):
```rust
#[test]
fn article_round_trips_and_rejects_oversize() {
    let a = OwnedArticleV1 { section: b"news".to_vec(), headline: "H".into(), dek: "D".into(), body: "B".into(), byline: "by".into() };
    let bytes = encode_article(&a);
    let path = Path::from_slices(&[ARTICLES_COMPONENT, b"news", b"id"]).unwrap();
    assert_eq!(decode_article(&path, &bytes).unwrap(), a);
    let mut big = a.clone(); big.body = "x".repeat(MAX_BODY_BYTES + 1);
    assert!(matches!(encode_article(&big), Err(ArticleRecordError::FieldTooLarge("body")))); // enforced on ENCODE
}
```
- [ ] **Step 2 — FAIL** (module absent). **Step 3 — Create `article.rs`:** module doc mirroring `moderation.rs`'s; `pub const MAX_SECTION_BYTES` (re-export from manifest or redefine 64), `MAX_HEADLINE_BYTES=256`, `MAX_DEK_BYTES=1024`, `MAX_BYLINE_BYTES=128`, `MAX_BODY_BYTES=65_536`; `pub const ARTICLE_RECORD_SCHEMA: &str = "org.riot.site.article/1";`; `pub struct OwnedArticleV1 { section, headline, dek, body, byline }`; **`ArticleRecordError` mirrors the REAL `ModerationRecordError` taxonomy (`moderation.rs:151-165`) plus per-field bound + path variants:** `pub enum ArticleRecordError { InputTooLarge, FieldTooLarge(&'static str), UnknownKey(u64), DuplicateOrMisorderedKey(u64), MissingKey(u64), WrongSchema, InvalidEnum(&'static str), NonCanonical, TrailingBytes, Malformed, NotUnderArticles }`. `encode_article` returns `Result<Vec<u8>, ArticleRecordError>` and checks every field bound BEFORE encoding (return `FieldTooLarge("<field>")`); it emits a tagged canonical map (schema key 0, then section/headline/dek/body/byline as ordered integer keys) — mirror `encode_moderation_record`'s key-by-key shape.
- [ ] **Step 3b — `decode_article(path, payload)`:** first `if !is_under_articles(path) { return Err(NotUnderArticles); }` (belt-and-suspenders, mirrors `read_moderation_record`'s `is_under_mod`); then strict canonical decode with per-field bound checks (reject `TooLong` on decode too); finish with `prove_canonical`-style re-encode-compare (mirror `decode_moderation_record`).
- [ ] **Step 4 — PASS. Step 5 — Register** in `site/mod.rs` (`mod article; pub use article::*;`). **Step 6 — Commit**: `feat(site): OwnedArticleV1 record + canonical codec`.

### Task 1.2: canonicity + per-field decode-bound tests
- [ ] RED: a non-canonical byte string is rejected; an oversize field on DECODE is rejected; a wrong-path decode returns `NotUnderArticles`. Implement is already present (Task 1.1). PASS. Commit `test(site): article codec canonicity + bounds`.

---

## Unit 2 — signer (`crates/riot-core/src/site/article_entry.rs`)

Mirror `moderation_entry.rs::create_signed_moderation_record` exactly.

### Task 2.1: `create_signed_article`
- [ ] **Step 1 — RED test:**
```rust
#[test]
fn owner_signs_article_at_articles_path_and_returns_entry_id() {
    let masthead = OwnedMasthead::generate().unwrap();
    let a = OwnedArticleV1 { section: b"news".to_vec(), headline: "H".into(), dek: "".into(), body: "B".into(), byline: "".into() };
    let signed = create_signed_article(&masthead, &a, snapshot(1_000)).unwrap();
    // path is [articles, section, <time+digest>]; re-decoding the payload yields the same article
    assert!(is_under_articles(signed.signed.entry().path()));
    // admissible under the owner's own namespace (sanity)
}
```
NOTE per design §4.2/§8: there is NO "non-owner refused" test here — a pure signer takes a valid `OwnedMasthead` by construction. Forged/foreign-cap refusal is proven in Unit 3 (admission).
- [ ] **Step 2 — FAIL. Step 3 — Implement:** `create_signed_article(masthead, article, snapshot) -> Result<SignedArticleRecord, ArticleSignError>` mirroring `create_signed_moderation_record`: `encode_article` the payload, build the collision-free path `[ARTICLES_COMPONENT, &article.section, &time_plus_digest(snapshot, &payload)]` (reuse the same time+digest helper `/mod` uses — grep `moderation_entry.rs` for it), `masthead.authorise_owner_entry(entry)`, return `{ signed, entry_id }`. `pub struct SignedArticleRecord { pub signed: SignedWillowEntry, pub entry_id: EntryId }`; `pub enum ArticleSignError { PathInvalid, NotAuthorised, Encode(ArticleRecordError) }`.
- [ ] **Step 4 — PASS. Step 5 — Register** in `site/mod.rs`. **Commit**: `feat(site): create_signed_article owner signer`.

---

## Unit 3 — projection + shared degradation + import (`crates/riot-ffi/src/site_ffi.rs`)

### Task 3.1: characterize-then-refactor — extract `resolve_site_degradation` (its own GREEN commit)
- [ ] **Step 1 — Characterization test** (capture CURRENT behavior, GREEN against unmodified code):
```rust
#[test]
fn resolve_composite_site_degradation_is_stable_for_fixtures() {
    for fx in degradation_fixtures() { // moderation-loading, manifest-invalid, clean, etc.
        assert_eq!(resolve_composite_site_from_store(&fx.store, &fx.signed_manifest, &fx.root, fx.now).degradation, fx.expected_degradation);
    }
}
```
- [ ] **Step 2 — Run PASS** (documents current behavior). **Step 3 — Extract:** pull the degradation-producing body of `resolve_composite_site_from_store` (manifest validation via `validate_site_manifest`, the held/protected `/mod` scan, the per-member emptiness scan, the `resolve_degradation(&DegradationInputs{..})` fold) into `fn resolve_site_degradation(store, signed_manifest, root, now) -> CompositeDegradation`. Refactor `resolve_composite_site_from_store` to call it. **Step 4 — Run PASS** (characterization test still green). **Step 5 — Commit (standalone)**: `refactor(ffi): extract resolve_site_degradation (behavior-preserving)`.

### Task 3.2: `article_feed_render` pure classifier (`crates/riot-core/src/site/article_render.rs`)
- [ ] **Step 1 — RED test** (pure, exhaustive over the 8 variants):
```rust
#[test]
fn every_degradation_maps_to_render_warn_or_hold() {
    use CompositeDegradation::*;
    for d in [ModerationLoading, ManifestInvalid, ManifestRollbackAlarm, EquivocationAlarm, TransportBlocked] {
        assert!(matches!(article_feed_render(&d), ArticleFeedRender::Hold(_)));
    }
    for d in [EditorialOnly, MemberUnverified] { assert!(matches!(article_feed_render(&d), ArticleFeedRender::Warn(_))); }
    assert!(matches!(article_feed_render(&CompositeDegradation::None), ArticleFeedRender::Render));
}
```
- [ ] **Step 2 — FAIL. Step 3 — Implement** `article_render.rs`: `pub enum ArticleFeedRender { Render, Warn(CompositeDegradation), Hold(CompositeDegradation) }`; the mapping above (identical to iOS `CompositeContentHold.holdFor`); plus `pub struct ResolvedArticleV1 { entry_id:[u8;32], author_subspace:[u8;32], section:Vec<u8>, headline:Option<String>, dek:Option<String>, body:Option<String>, byline:Option<String>, treatment: PostTreatment }`. Register in `site/mod.rs`. **Step 4 — PASS. Commit**: `feat(site): article_feed_render verdict + ResolvedArticleV1`.

### Task 3.3: `resolve_article_feed_from_store` (hold-nulls-content, decode-skip, section-grouped)
- [ ] **Step 1 — RED tests:**
```rust
#[test]
fn hold_nulls_all_article_content() {
    let fx = fixture_with_degradation(CompositeDegradation::ModerationLoading, /*one ordinary article*/);
    let feed = resolve_article_feed_from_store(&fx.store, &fx.signed_manifest, &fx.root, fx.now);
    assert!(matches!(feed.render, ArticleFeedRender::Hold(_)));
    assert!(feed.articles.iter().all(|a| a.headline.is_none() && a.body.is_none()));
}
#[test]
fn warn_degradation_preserves_article_content() {
    // §7: mild variants yield warn WITH content — resolve must NOT null under Warn (only Hold nulls)
    let fx = fixture_with_degradation(CompositeDegradation::EditorialOnly, /*one ordinary article H/D/B/by*/);
    let feed = resolve_article_feed_from_store(&fx.store, &fx.signed_manifest, &fx.root, fx.now);
    assert!(matches!(feed.render, ArticleFeedRender::Warn(_)));
    assert!(feed.articles.iter().all(|a| a.headline.is_some() && a.body.is_some())); // content preserved
}
#[test]
fn hidden_article_is_placeholder_ordinary_is_content() { /* treatment set, body None for hidden; Some for ordinary render */ }
#[test]
fn one_malformed_article_does_not_blank_the_feed() { /* two good + one undecodable → 2 articles returned */ }
#[test]
fn feed_groups_by_declared_section_then_unsectioned() { /* declared [news, ops]; an article in "ghost" trails */ }
```
- [ ] **Step 2 — FAIL. Step 3 — Implement** beside `resolve_composite_site_from_store`: validate manifest (reuse), compute `let deg = resolve_site_degradation(store, signed_manifest, root, now);` (the SHARED value), scan `/articles/*` entries — for each: `let Ok(article) = decode_article(entry.path(), payload) else { continue };` (skip-and-continue), compute per-item `treatment` via the same moderation resolution the composite surface uses; build `ResolvedArticleV1` with content `None` when `matches!(article_feed_render(&deg), ArticleFeedRender::Hold(_))` OR treatment is Hidden/Tombstoned, else `Some`; group by `manifest.sections` order then a trailing unsectioned group (never drop). Return `ResolvedArticleFeedV1 { render: article_feed_render(&deg), articles }`.
- [ ] **Step 4 — PASS. Commit**: `feat(ffi): resolve_article_feed_from_store (hold-null, decode-skip, section-grouped)`.

### Task 3.4: `import_owned_article` (follower admission path, no sync_inventory)
- [ ] **Step 1 — RED tests:**
```rust
#[test]
fn author_then_resolve_returns_the_article() { /* create_signed_article → import_owned_article → resolve_article_feed shows it */ }
#[test]
fn import_owned_article_leaves_sync_inventory_unchanged() { /* mirror importing_a_mod_bundle_leaves_the_sync_inventory_unchanged */ }
```
- [ ] **Step 2 — FAIL. Step 3 — Implement** `import_owned_article(profile, root, &signed) -> Result<(), MobileError>` mirroring `import_owned_mod` line-for-line: same `encode_bundle` → `inspect_core_with_root(Some(root))` → `plan_all` → `commit`, and it MUST NOT call `install_sync_inventory`. **Step 4 — PASS. Commit**: `feat(ffi): import_owned_article (admission path, sync-isolated)`.

### Task 3.5: cross-surface degradation consistency test
- [ ] RED: for every degradation fixture, `article_feed_render(resolve_site_degradation(..))`'s hold-ness equals `resolve_composite_site_from_store(..).degradation`'s hold-ness (both derive from the same `resolve_site_degradation`). PASS (shared computation). Commit `test(ffi): article/trust degradation agree per variant`.

### Task 3.6: forged/foreign-cap refused admission (the load-bearing security test)
- [ ] **Files:** Modify `crates/riot-core/tests/composite_admission.rs`. RED: an `/articles` entry authored under a DIFFERENT namespace's masthead (or a communal cap naming the owned namespace) is REJECTED by `import_owned_article`'s admission (mirror `owned_editorial_under_delegated_cap_is_admitted_with_correct_followed_root` + `marker_bit_forgery_communal_cap_naming_owned_namespace_is_rejected`). No new admission code — this proves the reused chokepoint holds for the write path. PASS. Commit `test(admission): forged/foreign cap cannot land an /articles entry`.

---

## Unit 4 — FFI surface (`create_site_article`, `resolve_site_articles`) + regen

### Task 4.1: `create_site_article` (manifest-wire-validated section check)
- [ ] **Step 1 — RED tests:**
```rust
#[test]
fn create_article_with_declared_section_succeeds_undeclared_rejected() {
    let (p, key, sealed) = durable_owned_site();
    p.publish_site_manifest(sealed.clone(), key.clone(), members(), vec![b"news".to_vec()], transport(), 1).unwrap();
    // The signed manifest wire (entry/cap/sig/payload) is read back from the store's O:/manifest entry via a test
    // helper `published_manifest_wire(&p, root) -> (Vec<u8>,Vec<u8>,Vec<u8>,Vec<u8>)` (build it in Task 4.1 Step 3 by
    // scanning the store for the MANIFEST_COMPONENT entry and returning its four wire fields — the same shape the app
    // obtains after publishing). NOT a nonexistent `last_manifest_wire`.
    let (me, mc, ms, mp) = published_manifest_wire(&p, root);
    assert!(p.create_site_article(sealed.clone(), key.clone(), me, mc, ms, mp,
        "news".into(), "H".into(), "".into(), "B".into(), "".into()).is_ok());
    assert!(matches!(p.create_site_article(/* same but */ "sports".into(), ..), Err(MobileError::InvalidInput)));
}
#[test]
fn create_article_rejects_a_foreign_signed_manifest_wire() { /* a manifest wire signed by a different masthead → Err */ }
```
- [ ] **Step 2 — FAIL. Step 3 — Implement** the `#[uniffi::export]` method per design §4.4 signature (takes the 4-field manifest wire): open masthead → `root`; `validate_site_manifest(&signed_manifest_wire, &root)` (reject foreign/bad-sig — do NOT scan the store); `if !section_is_declared(&validated.manifest, section.as_bytes()) { return Err(InvalidInput); }`; build `OwnedArticleV1` → `create_signed_article` → `import_owned_article`; zeroize key; return `SiteArticleOutcome { entry_id, section }`.
- [ ] **Step 4 — PASS (cargo test). Commit**: `feat(ffi): create_site_article (manifest-validated section)`.

### Task 4.2: `resolve_site_articles` + FFI records/enums
- [ ] **Step 1 — RED test:** `resolve_site_articles(entry/cap/sig/payload/root/now)` returns a `ResolvedArticleFeed { render, site_display, articles }`; computes freshness/degradation once via `resolve_site_degradation` then `resolve_article_feed_from_store`. Assert a published+authored article appears; a hold fixture returns `.hold` with `None` content.
- [ ] **Step 2 — FAIL. Step 3 — Implement:** the `#[uniffi::export]` method + the records/enums: `pub enum SiteArticleFeedRender { Render, Warn(SiteDegradation), Hold(SiteDegradation) }` (PascalCase — clippy `-D warnings`); `pub struct ResolvedArticle { entry_id, author_subspace, section, headline: Option<String>, dek, body, byline, treatment: SiteItemTreatment }`; `pub struct ResolvedArticleFeed { render: SiteArticleFeedRender, site_display: String, articles: Vec<ResolvedArticle> }`; a `From<ArticleFeedRender> for SiteArticleFeedRender` mapping. `site_display` from the resolved manifest/site context.
- [ ] **Step 4 — PASS. Commit**: `feat(ffi): resolve_site_articles + article FFI records`.

### Task 4.3: offer includes the authored article + full follower round-trip (durable)
- [ ] **Step 1 — RED (durable):** after `create_site_article`, `build_followed_site_offer(root)` includes the `/articles` entry (regression lock).
- [ ] **Step 2 — RED (durable, §7 deferred-reach criterion — cross-profile):**
```rust
#[test]
fn followed_article_round_trips_owner_to_follower_with_fields_intact() {
    // OWNER: publish manifest (section "news"), author an article
    let (owner, okey, osealed) = durable_owned_site();
    owner.publish_site_manifest(osealed.clone(), okey.clone(), members(), vec![b"news".to_vec()], transport(), 1).unwrap();
    let (me,mc,ms,mp) = published_manifest_wire(&owner, root_of(&osealed));
    owner.create_site_article(osealed.clone(), okey.clone(), me.clone(), mc.clone(), ms.clone(), mp.clone(),
        "news".into(), "Headline".into(), "Dek".into(), "Body text".into(), "by".into()).unwrap();
    let offer = build_followed_site_offer(&owner, &root_of(&osealed)).unwrap();
    // FOLLOWER: a SECOND, distinct durable profile that FOLLOWS the site, imports the bundle, resolves
    let follower = durable_profile();
    follower.follow_site_for_test(root_of(&osealed).to_vec()).unwrap();      // existing test seam (Following record)
    follower.import_followed_site_bundle(offer, root_of(&osealed).to_vec()).unwrap();
    let feed = follower.resolve_site_articles(me, mc, ms, mp, hex(&root_of(&osealed)), NOW).unwrap();
    let a = feed.articles.iter().find(|a| a.section == "news").unwrap();
    assert_eq!((a.headline.as_deref(), a.dek.as_deref(), a.body.as_deref(), a.byline.as_deref()),
               (Some("Headline"), Some("Dek"), Some("Body text"), Some("by")));  // fields survive the real bundle round-trip
}
```
- [ ] **Step 3** — both PASS (offer export + import admission are pre-existing; this proves the NEW article codec survives a real cross-profile bundle round-trip, not just same-store resolve). **Commit**: `test(ffi): article offer→follower round-trip, fields intact`.

### Task 4.4: wrapping-key zeroization
- [ ] RED: `create_site_article` and `publish_site_manifest` zero the passed key buffer before return (mirror the existing zeroization assertion pattern). PASS. Commit `test(ffi): article/manifest FFI zeroize wrapping key`.

### Task 4.5: binding regen + native staticlib rebuild (batched)
- [ ] **Step 1** — enumerate every new/changed `#[uniffi::export]` type in this plan: `SiteManifestOutcome`, `SiteArticleOutcome`, `SiteArticleFeedRender`, `ResolvedArticle`, `ResolvedArticleFeed`, `ResolvedSiteManifest.sections`, methods `publish_site_manifest`/`create_site_article`/`resolve_site_articles`, `MobileError::{ManifestRollback,ManifestRequireDowngrade,ManifestEquivocation}`.
- [ ] **Step 2** — `cargo run --locked -p xtask -- generate-bindings`. **Step 3** — `sh scripts/conference/build-native-core.sh` (or at minimum the three Apple slices per the overnight rebuild recipe). **Step 4** — `cargo test --workspace --all-features` green; `cargo clippy --workspace --all-features -- -D warnings` clean; `cargo tarpaulin --workspace --all-features --fail-under <thresholds.tarpaulin.lines>` meets the floor. **Step 5 — Commit** (regen + staticlib together): `chore(ffi): regen bindings + native core for article/manifest surface`.

### Task 4.6: offer family-filter + clearer diagnostic (design §9 — explicit plan instruction)

**Files:** Modify `crates/riot-ffi/src/mobile_state.rs` (`build_followed_site_offer` ~2091-2131). Design §9 directs the plan to add this because publishing manifests + articles makes the stray-`/manifest` whole-offer-failure edge likelier.

- [ ] **Step 1 — RED tests** (durable profile):
```rust
#[test]
fn offer_includes_only_owned_mod_and_articles_family() {
    // author a /mod record + an /articles record + publish a /manifest; the offer bundles /mod + /articles,
    // and a stray non-family entry in the namespace does NOT fail the whole offer.
    let offer = build_followed_site_offer(profile, &root).unwrap();
    assert!(offer_contains_family(&offer, /*mod+articles*/));
}
#[test]
fn offer_failure_has_a_specific_diagnostic_not_generic_session_limit() {
    // force an encode_bundle failure; assert the error is a differentiated variant, not the confusing MobileError::SessionLimit
    assert!(!matches!(build_followed_site_offer(bad_profile, &root), Err(MobileError::SessionLimit)));
}
```
- [ ] **Step 2 — FAIL. Step 3 — Implement:** in `build_followed_site_offer`, filter the walked live-entry set to the owned-site family (`is_owned_moderation_entry(e) || is_owned_editorial_entry(e)`, plus the `/manifest` entry the follower needs — decide per design: the follower's family gate excludes `/manifest`, so exclude it from the offer too and document that followers get the manifest via the caller-supplied wire, matching the read path). Map an `encode_bundle` failure to a differentiated `MobileError` variant with a clear message instead of the generic `SessionLimit`.
- [ ] **Step 4 — PASS. Commit**: `fix(ffi): offer family-filter + clearer diagnostic (design §9)`.

---

## Unit 5a — Native reader (iOS view + Android logic parity)

### Task 5a.1: iOS pure reader mapping (`apps/ios/Riot/CompositeArticleReader.swift`)
- [ ] **Step 1 — RED test** (`apps/ios/RiotTests/CompositeArticleReaderTests.swift`):
```swift
func testHoldShowsNoContentWarnShowsBannerRenderShowsClean() {
    XCTAssertEqual(CompositeArticleReaderModel.from(feed(.hold(.moderationLoading), articles: [ordinary()])).rows, [.held])
    XCTAssertTrue(CompositeArticleReaderModel.from(feed(.warn(.editorialOnly), articles: [ordinary()])).showsBanner)
    XCTAssertEqual(CompositeArticleReaderModel.from(feed(.render, articles: [])).rows, [.empty]) // render + empty
}
func testWarnEmptyIsDistinctFromRenderEmpty() {  // G4
    let m = CompositeArticleReaderModel.from(feed(.warn(.editorialOnly), articles: []))
    XCTAssertTrue(m.showsBanner)              // warn banner present
    XCTAssertEqual(m.rows, [.empty])          // AND the empty state, composed (verdict outer, empty inner)
}
func testBylineNeverRendersAsStandaloneAuthor() {
    let rows = CompositeArticleReaderModel.from(feed(.render, articles: [ordinary(byline: "Jane, Reuters")], site: "The Wire")).rows
    XCTAssertTrue(rows.contains { $0.attribution == "Jane, Reuters — on The Wire" })   // subordinated to site_display
    XCTAssertFalse(rows.contains { $0.attribution == "By Jane, Reuters" })             // never a standalone author
}
func testAuthorSubspaceRevealAffordanceIsPresent() {  // G2 — §4.6/§7 HARD requirement
    // every rendered article row carries a reveal action exposing the crypto signing identity (author_subspace),
    // distinct from the display byline. Pure-logic: the mapped row has a non-nil `revealIdentity` == the article's author_subspace hex.
    let row = CompositeArticleReaderModel.from(feed(.render, articles: [ordinary(authorSubspace: "ab12…")], site: "The Wire")).rows.first!
    XCTAssertEqual(row.revealIdentity, "ab12…")
}
func testArticlesAreGroupedUnderDeclaredSectionHeaders() {  // G3 — §8 5a
    // feed carries section-ordered articles; the model emits a section-header row before each section's articles.
    let m = CompositeArticleReaderModel.from(feed(.render, sectionsInOrder: ["news","ops"], articles: [ordinary(section: "ops"), ordinary(section: "news")]))
    XCTAssertEqual(m.sectionHeaders, ["news", "ops"])  // declared order, header per non-empty section
}
func testHiddenVsTombstonedDistinctPlaceholders() {
    let hidden = CompositeArticleReaderModel.from(feed(.render, articles: [moderated(.hidden)])).rows.first!
    let tomb = CompositeArticleReaderModel.from(feed(.render, articles: [moderated(.tombstoned)])).rows.first!
    XCTAssertNotEqual(hidden.placeholder, tomb.placeholder)  // two distinct strings
}
```
- [ ] **Step 2 — FAIL. Step 3 — Implement** the pure `CompositeArticleReaderModel.from(_ feed: ResolvedArticleFeed)`: verdict→rows (hold→no content, warn→banner+content, render→clean); **section-header grouping** in declared order; byline subordinated to `feed.site_display`; a per-row **`revealIdentity`** carrying the article's `author_subspace` (the hard-requirement reveal affordance — a tap surfaces the crypto identity, distinct from byline); empty/held/warn states (warn+empty composes); distinct hidden/tombstoned placeholders. Thin `CompositeArticleReaderView` renders it (section headers, and the reveal as a tappable disclosure). Register both files in BOTH pbxproj (RiotKit target in ios + macos), per the PR #68 pattern. **Step 4 — Run** `xcodebuild test -scheme RiotKit …`; `plutil -lint` both pbxproj. **Commit**: `feat(ios): composite article reader (verdict/byline/section-headers/identity-reveal)`.

### Task 5a.2: Android logic parity (`apps/android/…/CompositeArticle.kt`)
- [ ] **Step 1** — `cargo run -p xtask -- generate-bindings` (Kotlin twin of the new records). **Step 2 — RED** JUnit (`CompositeArticleTest.kt`): the pure Kotlin `CompositeArticleReaderModel.from(...)` mirrors the FULL iOS mapping — hold→no content, warn→banner (incl. warn+empty), byline subordinated to `site_display` (never standalone), **`revealIdentity` == author_subspace present** (§4.6 hard requirement), **section-header grouping in declared order**, distinct hidden/tombstoned placeholders, empty state. **Step 3 — Implement** the pure Kotlin (no Compose — logic parity only, per PR #68). **Step 4 — Run** `cd apps/android && ./gradlew :app:testDebugUnitTest`. **Commit**: `feat(android): composite article reader logic parity`.

---

## Unit 5b — Native owner compose (depends on Unit 4 AND PR #68)

**Gate:** do NOT start until PR #68 (`SiteSeizureDisclosure`/`OwnedSiteCreationGate`) is merged/rebased into this branch.

### Task 5b.1: iOS compose (`apps/ios/Riot/OwnedArticleCompose.swift`)
- [ ] **Step 1 — RED tests:** the compose gate refuses to sign until the seizure disclosure is acknowledged (reuse `OwnedSiteCreationGate`); a **pre-publish preview** renders the built `OwnedArticleV1` locally BEFORE `create_site_article`; a live byte-counter validates per-field bounds inline; the section field offers declared sections (from the resolved manifest). Pure-logic where possible (the preview mapping reuses 5a's model).
- [ ] **Step 2 — FAIL. Step 3 — Implement** the compose sheet (publish-manifest/declare-section via `publish_site_manifest`; author via `create_site_article`; preview via the 5a reader model), seizure-gated. Register in both pbxproj. **Step 4 — Run** RiotKit tests + iOS/macOS builds + `plutil -lint`. **Commit**: `feat(ios): owner article compose (preview + seizure-gated)`.

### Task 5b.2: Android compose-gate logic parity
- [ ] RED JUnit: the compose-gate + preview mapping logic (pure Kotlin, no Compose UI). Implement. Run gradle. Commit `feat(android): owner compose logic parity`.

---

## Final verification (before PR)
- [ ] `cargo test --workspace --all-features` green; `cargo clippy --workspace --all-features -- -D warnings` clean; `cargo fmt --all -- --check`.
- [ ] `cargo tarpaulin --workspace --all-features --fail-under <thresholds.tarpaulin.lines>` meets `.coverage-thresholds.json` (new pure files `article.rs`/`article_entry.rs`/`article_render.rs` should be ~100%).
- [ ] `xcodebuild test -scheme RiotKit …` green; Riot.app + Riot-macOS BUILD SUCCEEDED; both pbxproj `plutil -lint` OK.
- [ ] `cd apps/android && ./gradlew :app:testDebugUnitTest` green.
- [ ] `cargo run -p xtask -- validate-contracts` passes (Cargo.lock sha unchanged — no new deps).
- [ ] Run `/self-reflect` to capture learnings; commit the KB updates BEFORE the PR (metaswarm pre-PR knowledge capture).

## Coverage / test-spec cross-check (design §7)
Every §7 acceptance criterion maps to a task: manifest sections/canonicity/floor/sync-isolation → 0.1–0.4; article codec/bounds → 1.1–1.2; owner signer → 2.1; hold-null per variant + decode-skip + cross-surface + forged-cap → 3.2–3.6; section validation + zeroize + offer → 4.1–4.4; reader verdict/byline/empty/placeholders → 5a; compose preview/seizure-gate → 5b.
