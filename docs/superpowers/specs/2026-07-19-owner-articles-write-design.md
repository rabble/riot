# Composite Site — Owner `/articles` Write + Article Reader — Design

**Date:** 2026-07-19 (rev. 2 — post design-review round 1)
**Status:** Design (re-review pending)
**Parent design:** `docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md`
**Relation to Unit 6:** unblocks the composite-site owner-produce path that the render surface (PR #68, WU-006 Tasks 1–3/8a) and the followed-site sync/offer track (`build_followed_site_offer`, `import_followed_site_bundle`, `feat/composite-followed-site-sync`) both already assume but nothing produces.

---

## 1. Problem & use cases

A composite site's owner can create a masthead (`create_owned_site`) and author moderation records (`create_site_moderation_action`), but **cannot author an editorial article**. Moderation is the only thing an owner can currently sign under the masthead — there is no way to publish a signed editorial statement.

**Use cases (WHO / WANTS / SO THAT):**
- **An activist running a rapid-response indymedia-style site** wants to publish a signed editorial account of an unfolding action under the masthead, SO THAT readers get an accountable, owner-signed narrative — not just moderation actions on other people's posts. (Reachable today: owner drafts + previews on their own device.)
- **A community organizer** wants to draft and preview their site's front page before anyone follows it, SO THAT the site has real content the moment followers arrive. (Reachable today.)
- **A follower** wants to read the site's editorial articles once they follow it, SO THAT they see the masthead's own voice. (Wired + tested here, but **not reachable by any user until follow-initiation ships** — see §2.)

The follower-side machinery to carry and render owned content already exists — `build_followed_site_offer` exports owned `/articles` entries verbatim, `import_followed_site_bundle` admits them (Following-gated), and `is_owned_editorial_entry` (`site_paths.rs`) + `is_followed_site_family = is_owned_moderation_entry || is_owned_editorial_entry` (`site/follow.rs`) already classify `/articles/<section>/<id>` paths. The missing links are the **owner authoring path** and the **content reader** (the current view model carries only trust-tier + treatment, no article body).

Verified unclaimed: no `create_site_article`/`OwnedArticleV1`/`ResolvedArticleV1`/`resolve_articles` on `main` or any of ~60 remote branches (2026-07-19 sweep).

## 2. Goals / Non-goals & the honest reachability picture

**Reachable-today value (the real v1 benefit):** an owner **drafts and previews their own signed editorial front page** on their own device — `create_site_article` → `resolve_site_articles` on the owner's own store, no follower required. This is the analogue of `create_site_moderation_action` → immediately visible via `resolve_composite_site`.

**Wired-but-unreachable-until-follow-init:** follower visibility. The offer exports and import admits the article, and the reader renders it — but no user can *follow* a composite site yet (`follow_site(ticket)` is a **test-only stub**; production follow is planned "Rung 5", not started; PR #68's sibling render surface is likewise orphan). This unit builds the produce+read path *ready and tested*; it does not claim end-to-end user reach. Connecting the orphaned surfaces is tracked as a follow-up once follow-initiation lands (see §9).

**Goals**
- Owner authors a rich editorial article (`headline`, `dek`, `body`, `byline`, `section`) signed under the masthead capability at `/articles/<section>/<collision-free-id>`.
- The article is admitted, exported by the offer, and projected back for reading, honoring moderation treatment.
- A native reader (iOS + Android) renders the article, and a native owner-only compose surface authors it.

**Non-goals (explicitly deferred)**
- **Manifest-declared sections.** `SiteManifestV1` has no section registry (only a single-variant `SiteLayout::SiteDefault`); adding one is a frozen-v1-manifest schema change out of scope here. v1 accepts any owner-chosen non-empty `section` string (bounded); articles order by signed-time. (Deferred to a manifest-schema v2.)
- **Delegated-editor write** (needs `delegate_section` FFI wrapper). Owner-only.
- **Follow initiation / live transport sync** (another track).
- **In-place edit / versioning / deletion.** Articles are immutable time+digest-path entries; "edit" = author a new article + tombstone the old one via the existing `/mod` path (no new mechanism).
- Media/attachments in the body (text only for v1).

## 3. Architecture

One core+FFI **write** path and one core+FFI **read projection**, each mirroring a proven sibling, plus a native reader + compose surface. No new transport, and no new family *plumbing* — the `/articles` classifier, bundle import, and offer export already handle owned editorial entries.

**Chosen approach (A):** a dedicated `OwnedArticleV1` record with its **own** read projection (`ResolvedArticleV1`), separate from the trust/moderation row (`ResolvedSiteItem` = tier + treatment). This matches the established precedent — `ProjectedPost`/`ProjectedComment` (`newswire/projection.rs`) already keep content separate from the trust surface. (Rejected: B — folding a body onto `ResolvedSiteItem`, muddying a security-relevant type; C — shells decoding raw entries, violating no-business-logic-in-shells.)

## 4. Components

### 4.1 Core record — `crates/riot-core/src/site/article.rs` (new)
```
pub struct OwnedArticleV1 { section: Vec<u8>, headline: String, dek: String, body: String, byline: String }
pub fn encode_article(&OwnedArticleV1) -> Vec<u8>
pub fn decode_article(path: &Path, payload: &[u8]) -> Result<OwnedArticleV1, ArticleRecordError>
```
Canonical deterministic encoding (mirror `site/moderation.rs`). `decode_article` takes the entry `path` and self-defends `is_under_articles(path)` (belt-and-suspenders like `read_moderation_record`). **Bounded fields** (reject over → `ArticleRecordError::TooLong`, surfaced as `InvalidInput`): `section ≤ 64` bytes and non-empty; `headline ≤ 256`; `dek ≤ 1024`; `byline ≤ 128`; `body ≤ 65_536` bytes (a "sane single-bundle article" — the user-facing rationale for the cap; an over-long article is rejected with a clear error, not silently truncated). (The `/mod` `MAX_MODERATION_RECORD_BYTES = 512` is far too small for a body, hence per-field ceilings.)

### 4.2 Core signer — `crates/riot-core/src/site/article_entry.rs` (new)
```
pub struct SignedArticleRecord { signed: SignedWillowEntry, entry_id: EntryId }
pub fn create_signed_article(&OwnedMasthead, &OwnedArticleV1, ClockSnapshot) -> Result<SignedArticleRecord, ArticleSignError>
```
Mirror `moderation_entry.rs::create_signed_moderation_record`: canonical payload at collision-free `[ARTICLES_COMPONENT, section, <time+digest>]`, signed via `authorise_owner_entry` (owner cap = `Area::full()`, already proven to authorise article paths by the `delegated_editor_can_write_articles_but_not_manifest` test). **Only the masthead secret can sign** — a non-owner cap cannot produce an admissible `/articles` entry (same guarantee as `/mod`).

### 4.3 Core projection — `crates/riot-core/src/site/resolve.rs` (extend) + shared freshness helper
```
pub struct ResolvedArticleV1 {
    entry_id: [u8;32], author_subspace: [u8;32], section: Vec<u8>,
    headline: Option<String>, dek: Option<String>, body: Option<String>, byline: Option<String>,
    treatment: PostTreatment,   // ordinary / hidden / tombstoned
}
pub fn resolve_articles(store, root, freshness: &ModerationFreshness) -> Vec<ResolvedArticleV1>
```
- **Freshness is a passed-in input, not recomputed.** Extract the held/protected/`evaluate_freshness` scan currently inlined in `resolve_composite_site_from_store` into a shared core helper (e.g. `site::moderation::resolve_site_freshness(store, root, now) -> ModerationFreshness`); both `resolve_composite_site` and `resolve_articles` call it. This gives correct time-window treatment **and** guarantees the article reader and the trust surface see one consistent freshness verdict (no mid-sync divergence), and avoids two drifting copies of the scan.
- **Redaction is enforced at the core boundary (security).** When an article's resolved `treatment` is `Hidden` or `Tombstoned`, `resolve_articles` returns `headline/dek/body/byline = None` — core never hands a shell a moderated body. The `Option` types make "no leaked content on hold" a **type-level** guarantee (matching `ProjectedPost`'s `None`/empty redaction discipline), not reader convention.
- Order by signed-time (no manifest section order exists).

### 4.4 FFI write — `crates/riot-ffi/src/site_ffi.rs` (extend)
```
pub struct SiteArticleOutcome { entry_id: String, section: String }   // thin: articles have no epoch/freshness concept, unlike SiteModerationOutcome
impl MobileProfile {
  pub fn create_site_article(&self, sealed_root: Vec<u8>, wrapping_key: Vec<u8>,
      section: String, headline: String, dek: String, body: String, byline: String)
      -> Result<SiteArticleOutcome, MobileError>
}
```
Mirror `create_site_moderation_action`: `OwnedMasthead::open_sealed(key, sealed_root)` → build `OwnedArticleV1` (bounds enforced in `encode`) → `create_signed_article` → `import_owned_article(profile, root, &signed)` (new sibling of `import_owned_mod`, lives in `site_ffi.rs`). Zeroize the wrapping key. **No manifest-section validation** (none exists) — any non-empty, in-bounds `section` string is accepted.

### 4.5 FFI read — `crates/riot-ffi/src/site_ffi.rs` (extend)
```
pub struct ResolvedArticle { entry_id, author_subspace, section, headline: Option<String>, dek: Option<String>, body: Option<String>, byline: Option<String>, treatment: SiteItemTreatment }  // reuses the EXISTING SiteItemTreatment enum
impl MobileProfile { pub fn resolve_site_articles(...) -> Result<Vec<ResolvedArticle>, MobileError> }
```
Same argument shape as `resolve_composite_site` (entry/cap/sig/payload/root/now) → compute freshness once via the shared helper → `resolve_articles` → map to the FFI record. Requires binding regen + native staticlib rebuild (UniFFI record coupling).

### 4.6 Native reader + compose (iOS + Android)
- **Reader** (iOS `CompositeArticleReaderView`; Android per §"parity" below): render `headline/dek/byline/body`; a `nil` field under `hidden`/`tombstoned` treatment renders the accountable placeholder (reuse `NewswirePostDisplay` discipline). Pure-logic mapping unit-tested; thin view.
- **Owner compose** (owner-only): a sheet (headline/dek/body/byline + a **free-text section field / recent-sections suggestion**, since no declared list exists) calling `create_site_article`. Gated behind the seizure disclosure. **Dependency:** the seizure gate (`SiteSeizureDisclosure`/`OwnedSiteCreationGate`) lives in **PR #68**, not on `main` — this unit's compose surface lands after PR #68 merges (or rebases on it); listed as a blocking external dependency in §9. "Edit" is author-new + tombstone-old (§2), never in-place.
- **Android:** the app has no Compose (plain Views), so — consistent with PR #68's resolution — Android ships the tested pure-logic value/twin layer; a plain-View reader/compose only if it can be reachable, else logic-parity with the view gap flagged. Decided at plan time exactly as PR #68 did (not re-litigated here).

## 5. Data flow

owner composes → `create_site_article` (sign under masthead + import) → owner's store holds the signed `/articles` entry, immediately visible via `resolve_site_articles` (the reachable v1 loop) → `build_followed_site_offer` exports it verbatim (already) → follower `import_followed_site_bundle` admits it (already, Following-gated) → follower's `resolve_site_articles` projects → reader renders (unreachable until follow-init).

## 6. Error handling / invariants

- **Owner-only write.** Signing requires the masthead secret; a non-owner cap cannot authorise/admit an `/articles` entry (same guarantee as `/mod`; admission via `is_owned_editorial_entry` + owner-signature verification).
- **Section is free text (v1).** Any non-empty, in-bounds section string is valid; no manifest registry to validate against (that's a deferred v2 schema change). No `InvalidInput` for "unknown section" — that concept doesn't exist yet.
- **Moderation honored + redacted in core.** A hidden/tombstoned article resolves with `treatment` set and body fields `None`; the reader shows the accountable placeholder. Core never emits a moderated body.
- **Consistent freshness.** Article reader and trust surface share one `ModerationFreshness` (shared helper) — no divergent verdict mid-sync.
- **Durable-only offer.** The export path reconstructs signed entries from the durable store and no-ops in-memory (signed-entries-durable-only constraint); write + local resolve work in-memory, but any offer/round-trip-to-follower test uses a **durable** profile.
- **No business logic in shells.** The reader renders `ResolvedArticle` verbatim; treatment, redaction, ordering, freshness are all core-resolved.
- **UniFFI coupling.** New records + methods → binding regen **and** native staticlib rebuild together.

## 7. Success criteria

**User-facing (the ship signal):**
- An owner can compose an article (headline/dek/body/byline/section) and see it on their own front-page preview via `resolve_site_articles` — with no follower and no network — in one sitting. This is the v1 "done."
- A moderated (hidden/tombstoned) article shows an accountable placeholder to a reader, never its body.
- (Deferred-reach, verified-not-shipped-to-users:) a followed article round-trips owner→offer→follower→reader with fields intact.

**Technical (verification):**
- **Core:** `encode/decode` round-trip; bounds rejection per field; `create_signed_article` owner-authorised, non-owner refused; sign → `import_owned_article` → `resolve_articles` returns fields intact; a moderated article resolves to `None` bodies + treatment; shared freshness helper returns identical verdict to `resolve_composite_site`'s prior inline path (regression-locks the refactor).
- **FFI:** `create_site_article` → `resolve_site_articles` returns it; `build_followed_site_offer` includes it (durable profile); wrapping-key zeroized.
- **Native:** reader maps `[ResolvedArticle]` → rows incl. redacted-placeholder (iOS XCTest + Android JUnit, pure logic); compose gated behind seizure disclosure.
- **Coverage:** meets `.coverage-thresholds.json` floors.

## 8. Work decomposition (for the plan)

1. **Core record** `article.rs` — encode/`decode(path,payload)` + per-field bounds. Pure Rust. RED-first.
2. **Core signer** `article_entry.rs` — `create_signed_article`, owner-only, non-owner refused. Pure Rust. RED-first.
3. **Core projection + shared freshness + import** — extract `resolve_site_freshness` shared helper (refactor `resolve_composite_site_from_store` to use it, regression-locked), add `resolve_articles` (redacting, freshness-fed), add `import_owned_article`. **Touches `riot-core` AND `riot-ffi`** (import + the FFI-side resolve wiring), **no UniFFI regen yet**. RED-first, end-to-end write→resolve.
4. **FFI surface** — `create_site_article` + `resolve_site_articles` + records (`SiteArticleOutcome`, `ResolvedArticle` with `Option` fields, reuse `SiteItemTreatment`); binding regen + native staticlib rebuild.
5. **Native reader + owner compose** (iOS + Android), compose seizure-gated (**after PR #68**), reader redaction-honest.

Strictly sequential 1→2→3→4→5, no forward/circular deps.

## 9. Risks & dependencies

- **Projection-registration trap:** decoding the new family for the reader is not compiler-forced — Unit 3 tests write→resolve end-to-end.
- **Freshness-refactor regression:** extracting the shared helper touches the live `resolve_composite_site` path — Unit 3 regression-locks it against the prior inline behavior before adding `resolve_articles`.
- **Redaction is security-load-bearing:** `Option` fields + core redaction are the mechanism; a fresh security review must confirm no moderated-body leak path.
- **Dependency — seizure gate (PR #68):** the compose surface (Unit 5) needs `SiteSeizureDisclosure`/`OwnedSiteCreationGate`, which are on the unmerged PR #68, not `main`. Unit 5's compose lands after PR #68 merges or rebases onto it; the reader (Unit 5a) has no such dependency and can land first.
- **Cross-track coordination:** follow-initiation + sync + offer internals are another session's track; this unit stays strictly on owner-produce + reader, only *consuming* the existing offer/import (never modifying `follow_site`/transport/offer).
- **Reachability:** reader + compose are orphan until follow-initiation lands (same status as PR #68). Built ready + tested, wired to no dead entry point; connection tracked as a follow-up when follow-init ships.
