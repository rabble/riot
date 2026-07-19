# Composite Site — Owner `/articles` Write + Article Reader — Design

**Date:** 2026-07-19
**Status:** Design (pre design-review gate)
**Parent design:** `docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md`
**Relation to Unit 6:** unblocks the composite-site owner-produce path that the render surface (PR #68, WU-006 Tasks 1–3/8a) and the followed-site sync/offer track (`build_followed_site_offer`, `import_followed_site_bundle`, `feat/composite-followed-site-sync`) both already assume but nothing produces.

---

## 1. Problem

A composite site's owner can create a masthead (`create_owned_site`) and author moderation records (`create_site_moderation_action`), but **cannot author an editorial article**. The follower-side machinery to carry and render owned content already exists — `build_followed_site_offer` exports owned `/articles` entries verbatim, `import_followed_site_bundle` admits them (Following-gated), `resolve_composite_site` counts masthead-namespace items as `Editorial`, and `is_owned_editorial_entry` already classifies `/articles/<section>/<id>` paths. The single missing link is the **owner authoring path** (write an article) and the **content reader** (render an article's body — the current view model carries only trust-tier + moderation treatment, no article content).

Verified unclaimed: no `create_site_article`/`write_article`/`author_article` function exists on `main` or any remote branch (2026-07-19 sweep).

## 2. Goals / Non-goals

**Goals**
- Owner authors a rich editorial article (`headline`, `dek`, `body`, `byline`, `section`) signed under the masthead capability at `/articles/<section>/<collision-free-id>`.
- The article is admitted, exported by the offer, and projected back for reading — all through the existing owned-editorial family plumbing.
- A native reader (iOS + Android) renders the article on the composite front page, honoring moderation treatment (a hidden/tombstoned article is an accountable placeholder, never a silent disappearance).
- Owner-only compose UI, gated behind the existing §9.3 seizure disclosure.

**Non-goals (explicitly deferred)**
- **Delegated-editor** article write (needs the `delegate_section` FFI wrapper — Task 6). Owner-only here.
- **Follow initiation** / live transport sync (owned by the `feat/composite-followed-site-sync` / `-transport` track).
- Article editing/versioning/deletion beyond what moderation (hide/tombstone) already provides.
- Media/attachments in the body (text only for v1).

## 3. Architecture

One core+FFI **write** path and one core+FFI **read projection**, each mirroring a proven sibling, plus a native reader. No new transport, and no new family *plumbing* — the `/articles` path classifier (`is_owned_editorial_entry`), bundle import, and offer export already handle owned editorial entries.

**Chosen approach (A):** a dedicated `OwnedArticleV1` record with its **own** read projection (`ResolvedArticleV1`), kept separate from the trust/moderation row (`ResolvedSiteItem` = tier + treatment). Two view-model shapes for two concerns: the anti-impersonation trust surface stays untouched; article content is its own projection. (Rejected: B — folding a body onto `ResolvedSiteItem`, which muddies a security-relevant type; C — shells decoding raw entries, which violates no-business-logic-in-shells.)

## 4. Components

### 4.1 Core record — `crates/riot-core/src/site/article.rs` (new)
```
pub struct OwnedArticleV1 {
    pub section: Vec<u8>,     // a manifest-declared section component, e.g. b"news"
    pub headline: String,
    pub dek: String,          // summary / standfirst (may be empty)
    pub body: String,
    pub byline: String,       // display byline (may be empty)
}
pub fn encode_article(&OwnedArticleV1) -> Vec<u8>
pub fn decode_article(&[u8]) -> Result<OwnedArticleV1, ArticleRecordError>
```
Canonical, deterministic encoding (mirror `site/moderation.rs::encode_moderation_record`). Bounded field lengths (reject absurd sizes) to keep a single article a sane bundle unit.

### 4.2 Core signer — `crates/riot-core/src/site/article_entry.rs` (new)
```
pub struct SignedArticleRecord { pub signed: SignedWillowEntry, pub entry_id: EntryId }
pub fn create_signed_article(&OwnedMasthead, &OwnedArticleV1, ClockSnapshot)
    -> Result<SignedArticleRecord, ArticleSignError>
```
Mirror `moderation_entry.rs::create_signed_moderation_record`: build the canonical payload, place it at a collision-free path `[ARTICLES_COMPONENT, section, <time+digest>]` (same time+digest scheme as `newswire_path`/`/mod`), sign under the owner's `OwnedMasthead` via `authorise_owner_entry`, return the wire entry + id. **Only the owner** can sign (requires the masthead secret) — a non-owner-authored article is refused, exactly like `/mod`.

### 4.3 Core projection — `crates/riot-core/src/site/resolve.rs` (extend)
```
pub struct ResolvedArticleV1 {
    pub entry_id: [u8;32], pub author_subspace: [u8;32],
    pub section: Vec<u8>, pub headline: String, pub dek: String,
    pub body: String, pub byline: String,
    pub treatment: PostTreatment,   // ordinary / hidden / tombstoned
}
pub fn resolve_articles(store, root) -> Vec<ResolvedArticleV1>
```
Decode admitted `/articles` entries under `root`, apply the same moderation treatment resolution the composite surface uses, order deterministically (section order per manifest, then signed-time). A hidden/tombstoned article is returned with its treatment set (the reader renders a placeholder), never dropped. **This decode is the non-compiler-forced projection site** (the record-family trap): the plan wires it explicitly and tests it end-to-end (write → resolve returns it).

### 4.4 FFI write — `crates/riot-ffi/src/site_ffi.rs` (extend)
```
pub struct SiteArticleOutcome { pub entry_id: String, pub section: String }
impl MobileProfile {
  pub fn create_site_article(&self, sealed_root: Vec<u8>, wrapping_key: Vec<u8>,
      section: String, headline: String, dek: String, body: String, byline: String)
      -> Result<SiteArticleOutcome, MobileError>
}
```
Mirror `create_site_moderation_action`: `OwnedMasthead::open_sealed(key, sealed_root)` → validate `section` against the resolved manifest's declared sections (unknown section → `InvalidInput`) → `create_signed_article` → `import_owned_article(profile, root, &signed)` (new import sibling of `import_owned_mod`). Zeroize the wrapping key.

### 4.5 FFI read — `crates/riot-ffi/src/site_ffi.rs` (extend)
```
pub struct ResolvedArticle { entry_id, author_subspace, section, headline, dek, body, byline, treatment: SiteItemTreatment }
impl MobileProfile { pub fn resolve_site_articles(...) -> Result<Vec<ResolvedArticle>, MobileError> }
```
Same argument shape as `resolve_composite_site` (entry/cap/sig/payload/root/now) → `resolve_articles` → map to the FFI record. Requires binding regen + native staticlib rebuild (UniFFI record coupling).

### 4.6 Native reader (iOS + Android)
- **iOS** `apps/ios/Riot/…`: a `CompositeArticleReaderView` rendering `[ResolvedArticle]` — headline, dek, byline, body — with a treatment placeholder for hidden/tombstoned (reuse the existing `NewswirePostDisplay` placeholder discipline). Pure-logic mapping (article → display rows) unit-tested (XCTest); thin view.
- **Android** `apps/android/…`: the pure-Kotlin twin (mirror the WU-006 parity pattern; the app has no Compose, so the tested value/logic layer + a plain-View reader if reachable, else logic-only parity with the gap flagged — decided at plan time consistent with PR #68).
- **Owner compose UI**: an author sheet (headline/dek/body/byline/section picker) calling `create_site_article`, gated behind the existing seizure-disclosure (a created owned site). Owner-only.

## 5. Data flow

owner composes → `create_site_article` (sign under masthead + import) → store holds the signed `/articles` entry → `build_followed_site_offer` exports it verbatim (already) → follower `import_followed_site_bundle` admits it (already, Following-gated) → `resolve_site_articles` projects → reader renders. Owner sees their own article via the same `resolve_site_articles` on their own store.

## 6. Error handling / invariants

- **Owner-only write.** Signing requires the masthead secret; a non-owner cap cannot authorise an `/articles` entry (same guarantee as `/mod`). Enforced in core, not the shell.
- **Section validity.** `section` must be a manifest-declared section; unknown → `InvalidInput` (no orphan sections).
- **Moderation honored end to end.** A hidden/tombstoned article resolves to a placeholder, never dropped — the reader shows the accountable state (consistent with the composite trust surface).
- **Durable-only.** The offer/export path reconstructs signed entries from the durable store and no-ops in-memory (per the signed-entries-durable-only constraint); article write + resolve on the local store work in-memory, but any test asserting offer/round-trip-to-follower uses a **durable** profile.
- **No business logic in shells.** The reader renders `ResolvedArticle` fields verbatim; section validity, treatment, and ordering are all core-resolved.
- **UniFFI coupling.** New records (`SiteArticleOutcome`, `ResolvedArticle`) + new methods require binding regen **and** native staticlib rebuild together (checksum-abort otherwise).

## 7. Testing

- **Core:** `encode/decode` round-trip; `create_signed_article` → owner-authorised, non-owner refused; sign → `import_owned_article` → `resolve_articles` returns it with fields intact; a moderated (hidden/tombstoned) article resolves to a placeholder; bounded-field rejection.
- **FFI:** `create_site_article` → `resolve_site_articles` returns the article; unknown section → `InvalidInput`; `build_followed_site_offer` includes the authored article (durable profile); wrapping-key zeroized.
- **Native:** reader maps `[ResolvedArticle]` → display rows incl. treatment placeholder (iOS XCTest + Android JUnit, pure logic); compose sheet gated behind the seizure disclosure.
- **Coverage:** meets `.coverage-thresholds.json` (Rust line/branch floors).

## 8. Work decomposition (for the plan)

1. Core record `article.rs` (encode/decode + bounds) — RED-first.
2. Core signer `article_entry.rs` (`create_signed_article`, owner-only) — RED-first.
3. Core projection `resolve_articles` + `import_owned_article` (the projection-registration site) — RED-first, end-to-end write→resolve.
4. FFI `create_site_article` + `resolve_site_articles` + records; binding regen + staticlib rebuild.
5. Native reader (iOS + Android) + owner compose sheet (seizure-gated).

Units 1–3 are pure Rust (no binding). Unit 4 is the single FFI/coupling step. Unit 5 is native, per-platform.

## 9. Risks

- **Projection-registration trap:** decoding the new family for the reader is not compiler-forced — Unit 3 tests write→resolve end-to-end to catch a silently-non-projecting family.
- **Offer/durable coupling:** round-trip-to-follower tests must use a durable profile or silently no-op.
- **Cross-track coordination:** the follow-initiation + sync side is another session's track; this unit stays strictly on owner-produce + reader and does not touch `follow_site`/transport/offer internals (only consumes the existing offer/import).
- **Native reader reachability:** the reader is orphan until follow-initiation lands (same status as PR #68's surface); it is built ready + tested, not wired to a dead entry point.
