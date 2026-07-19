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
Canonical deterministic encoding (mirror `site/moderation.rs`, including its `prove_canonical` re-encode check so a non-canonical payload is rejected). `decode_article` takes the entry `path` and self-defends `is_under_articles(path)` (belt-and-suspenders like `read_moderation_record`). **Bounded fields, enforced on encode AND decode** (per security review — not just the 1 MiB bundle-level backstop, which is far too loose for per-field/DoS purposes): named constants `MAX_SECTION_BYTES = 64` (and non-empty), `MAX_HEADLINE_BYTES = 256`, `MAX_DEK_BYTES = 1024`, `MAX_BYLINE_BYTES = 128`, `MAX_BODY_BYTES = 65_536` (a "sane single-bundle article"; over-long → `ArticleRecordError::TooLong` → `InvalidInput`, a clear error, never silent truncation). Enforcing on decode too means a peer-supplied over-size article is rejected at admission/projection, not just at local author time.

### 4.2 Core signer — `crates/riot-core/src/site/article_entry.rs` (new)
```
pub struct SignedArticleRecord { signed: SignedWillowEntry, entry_id: EntryId }
pub fn create_signed_article(&OwnedMasthead, &OwnedArticleV1, ClockSnapshot) -> Result<SignedArticleRecord, ArticleSignError>
```
Mirror `moderation_entry.rs::create_signed_moderation_record`: canonical payload at collision-free `[ARTICLES_COMPONENT, section, <time+digest>]`, signed via `authorise_owner_entry` (owner cap = `Area::full()`, already proven to authorise article paths by the `delegated_editor_can_write_articles_but_not_manifest` test).

**Owner-only — precise statement (per security review).** This write path authors under the owner's masthead cap, so it *is* owner-only **as shipped**. But that is enforced by the **absence of a delegation-minting API in this path**, NOT by a cryptographic admission invariant: `admissible_capability` (`import/bundle.rs`) does *not* check `delegations().is_empty()` for owned namespaces — a validly-delegated cap under `/articles` already admits (proven by `composite_admission.rs::owned_editorial_under_delegated_cap_is_admitted_with_correct_followed_root`). What admission *does* guarantee is that any admitted `/articles` entry is rooted in *this site's* namespace secret with a valid Meadowcap chain (a forged/foreign cap is rejected — the load-bearing anti-impersonation property, adversarially proven). Task 6 (delegated editors) inherits this exact non-invariant and must not assume "owner-only" is cryptographic.

### 4.3 Store-coupled projection — `crates/riot-ffi/src/site_ffi.rs` (extend) + shared freshness helper
```
pub struct ResolvedArticleV1 {   // core value type in site/article.rs (pure)
    entry_id: [u8;32], author_subspace: [u8;32], section: Vec<u8>,
    headline: Option<String>, dek: Option<String>, body: Option<String>, byline: Option<String>,
    treatment: PostTreatment,   // ordinary / hidden / tombstoned
}
// the store scan lives beside resolve_composite_site_from_store, NOT in resolve.rs.
// Returns the site DEGRADATION alongside the articles (B1) — never a bare list:
struct ResolvedArticleFeedV1 { degradation: CompositeDegradation, articles: Vec<ResolvedArticleV1> }
fn resolve_article_feed_from_store(store, root, freshness: &ModerationFreshness) -> ResolvedArticleFeedV1
```
- **B1 — the moderation-loading hold is carried, not lost (security).** `item_treatment` returns `Ordinary` during `ModerationFreshness::Loading` *by design* — in `resolve_composite_site` the hold is a **separate site-level `CompositeDegradation` field** ("open content held, never rendered un-moderated"), NOT a per-item treatment. A flat `Vec<ResolvedArticle>` would therefore render a freshly-synced, not-yet-moderated article as clean `Ordinary` with no hold signal — pushing "is this safe to treat as vetted?" onto the shell. So the article projection returns a **feed carrying the degradation** (same `resolve_site_freshness` verdict as the trust surface); a `ModerationLoading`/held feed structurally tells the reader to hold all article content (reuse PR #68's `CompositeContentHold` discipline). Core-tested: a `Loading` verdict blocks rendering of a fresh `Ordinary`-treated article end-to-end.
- **Placement (per CTO review):** `crates/riot-core/src/site/resolve.rs` is deliberately **store-free** (every fn — `resolve_trust_tier`, `item_treatment`, … — takes no store). The store-coupled "decode admitted entries and project them" logic already has a precedent in the FFI crate: `resolve_composite_site_from_store` (`site_ffi.rs:474`). `resolve_articles_from_store` lives **beside it in `site_ffi.rs`** (the FFI crate, not a native shell — the shared-core rule targets Swift/Kotlin, not `riot-ffi`), so `resolve.rs` keeps its purity. The pure per-entry helpers (`decode_article`, redaction) stay in `riot-core`; only the store scan is FFI-side. The `ResolvedArticleV1` value type itself is a pure core type in `site/article.rs`.
- **Freshness is a passed-in input, not recomputed.** Extract the held/protected/`evaluate_freshness` scan currently inlined in `resolve_composite_site_from_store` into a shared helper (in `site_ffi.rs`, e.g. `fn resolve_site_freshness(store, root, now) -> ModerationFreshness`); both `resolve_composite_site` and `resolve_articles_from_store` call it. This gives correct time-window treatment **and** guarantees the article reader and the trust surface see one consistent freshness verdict (no mid-sync divergence), and avoids two drifting copies of the scan. Unit 3 regression-locks `resolve_composite_site` against its prior inline behavior before adding the article path.
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
Mirror `create_site_moderation_action`: `OwnedMasthead::open_sealed(key, sealed_root)` → build `OwnedArticleV1` (bounds enforced in `encode`) → `create_signed_article` → `import_owned_article(profile, root, &signed)` (new sibling of `import_owned_mod`, lives in `site_ffi.rs`). **`import_owned_article` MUST route through the same `inspect_core_with_root(Some(root))` admission path a follower uses** (as `import_owned_mod` does) — so a locally-authored article can never diverge from what a follower would actually admit (verified symmetry, per security review). Zeroize the wrapping key. **No manifest-section validation** (none exists — cut in rev-2): any non-empty, in-bounds `section` string is accepted; there is no declared-section registry to bypass. (Security's B2 — "section validation bypassable in one FFI wrapper" — is moot here because v1 has no such validation; the forward concern about loosely-scoped delegated `/articles` caps is a **Task 6** requirement, recorded in §9.)

### 4.5 FFI read — `crates/riot-ffi/src/site_ffi.rs` (extend)
```
pub struct ResolvedArticle { entry_id, author_subspace, section, headline: Option<String>, dek: Option<String>, body: Option<String>, byline: Option<String>, treatment: SiteItemTreatment }  // reuses the EXISTING SiteItemTreatment enum
pub struct ResolvedArticleFeed { degradation: SiteDegradation, articles: Vec<ResolvedArticle> }  // B1: hold carried alongside
impl MobileProfile { pub fn resolve_site_articles(...) -> Result<ResolvedArticleFeed, MobileError> }
```
Same argument shape as `resolve_composite_site` (entry/cap/sig/payload/root/now) → compute freshness once via the shared helper → `resolve_article_feed_from_store` → map to the FFI record, degradation included. The reader honors the degradation hold before rendering any article (B1). Requires binding regen + native staticlib rebuild (UniFFI record coupling).

### 4.6 Native reader + compose (iOS + Android)
- **Reader** (iOS `CompositeArticleReaderView`; Android per §"parity" below): honor the feed **degradation hold first** (a `ModerationLoading`/held feed shows the hold, not clean articles — B1); then render `headline/dek/byline/body`; a `nil` field under `hidden`/`tombstoned` treatment renders the accountable placeholder (reuse `NewswirePostDisplay` discipline). **Byline is unauthenticated self-declared text** (the crypto proves "the masthead published this," never "this named person wrote it") — the reader labels it as *published-by-the-site* (e.g. an attribution style tied to the site identity), never as an independently-verified author identity. The `author_subspace` (the real crypto identity) is carried in the view model for a future owner-vs-delegate distinction (Task 6) but v1 renders no delegation UI. Pure-logic mapping unit-tested; thin view.
- **Owner compose** (owner-only): a sheet (headline/dek/body/byline + a **free-text section field / recent-sections suggestion**, since no declared list exists) calling `create_site_article`. Gated behind the seizure disclosure. **Dependency:** the seizure gate (`SiteSeizureDisclosure`/`OwnedSiteCreationGate`) lives in **PR #68**, not on `main` — this unit's compose surface lands after PR #68 merges (or rebases on it); listed as a blocking external dependency in §9. "Edit" is author-new + tombstone-old (§2), never in-place.
- **Android:** the app has no Compose (plain Views). Consistent with PR #68's resolution, Android ships **logic-only parity** in this unit — the tested pure-Kotlin value/twin layer (article→display-rows mapping, redaction honesty), with the view/compose surface explicitly deferred (no Compose infra; orphan until follow-init anyway). Committed here, not left open to plan-time churn.

## 5. Data flow

owner composes → `create_site_article` (sign under masthead + import) → owner's store holds the signed `/articles` entry, immediately visible via `resolve_site_articles` (the reachable v1 loop) → `build_followed_site_offer` exports the site's **entire live signed-entry set verbatim** (manifest + mod + articles together — the offer does no `/articles`-specific filtering) → follower `import_followed_site_bundle` admits it, **family-gated on the import side** to owned `/mod` + `/articles` (already, Following-gated) → follower's `resolve_site_articles` projects → reader renders (unreachable until follow-init).

## 6. Error handling / invariants

- **Anti-impersonation admission (load-bearing, adversarially proven).** Any admitted `/articles` entry is rooted in *this site's* namespace secret with a valid Meadowcap chain; a forged/foreign/communal cap naming the owned namespace is rejected (`composite_admission.rs`). This is the real security guarantee.
- **Owner-only write = API-surface fact, not a crypto invariant.** This path is owner-only because it exposes no delegation-minting API — admission itself would accept a valid delegated `/articles` cap. Stated honestly so Task 6 doesn't inherit a false assumption.
- **Section is free text (v1).** Any non-empty, in-bounds section string is valid; no manifest registry to validate against (deferred v2 schema change). No "unknown section" concept exists to enforce or bypass.
- **Moderation honored + redacted in core.** A hidden/tombstoned article resolves with `treatment` set and body fields `None`; core never emits a moderated body.
- **Moderation-loading holds the whole feed (B1).** The article projection carries the site `degradation`; under `ModerationLoading` (moderation not yet synced) the reader holds all article content — a fresh, not-yet-moderated article is never rendered as clean. The trust decision lives in core+the feed shape, not the shell.
- **Byline is unauthenticated.** Rendered as site-published attribution, never as verified author identity.
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
- **Core:** `encode/decode` round-trip + `prove_canonical` rejection of non-canonical bytes; per-field bounds rejection on **both encode and decode**; `create_signed_article` owner-authorised; **a forged/foreign cap is refused admission** (mirror `composite_admission.rs`); sign → `import_owned_article` → `resolve_article_feed_from_store` returns fields intact; a moderated article resolves to `None` bodies + treatment; **a `ModerationLoading` verdict holds a fresh `Ordinary` article end-to-end (B1)**; shared freshness helper returns identical verdict to `resolve_composite_site`'s prior inline path (regression-locks the refactor); `import_owned_article` uses the same `inspect_core_with_root(Some(root))` path a follower would.
- **FFI:** `create_site_article` → `resolve_site_articles` returns it; `build_followed_site_offer` includes it (durable profile); wrapping-key zeroized.
- **Native:** reader maps `[ResolvedArticle]` → rows incl. redacted-placeholder (iOS XCTest + Android JUnit, pure logic); compose gated behind seizure disclosure.
- **Coverage:** meets `.coverage-thresholds.json` floors.

## 8. Work decomposition (for the plan)

1. **Core record** `article.rs` — encode/`decode(path,payload)` + per-field bounds. Pure Rust. RED-first.
2. **Core signer** `article_entry.rs` — `create_signed_article`, owner-only, non-owner refused. Pure Rust. RED-first.
3. **Store-coupled projection + shared freshness + import** — extract `resolve_site_freshness` shared helper in `site_ffi.rs` (refactor `resolve_composite_site_from_store` to use it, regression-locked against prior behavior), add `resolve_article_feed_from_store` **beside it in `site_ffi.rs`** (redacting, freshness-fed, degradation-carrying per B1 — NOT in the store-free `resolve.rs`), add `import_owned_article` (same admission path as a follower). Pure per-entry helpers (`decode_article`, redaction) + the `ResolvedArticleV1` value type stay in `riot-core::site::article`. **Touches `riot-core` AND `riot-ffi`, no UniFFI regen yet.** RED-first, end-to-end write→resolve; the test asserts a new record family that is decoded-but-not-dispatched would fail (guards the projection-registration trap at the trust-tier/path-prefix match site, `site_ffi.rs:534-541`).
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
- **Offer whole-namespace edge (plan-stage, pre-existing):** `build_followed_site_offer` walks the *entire* owned namespace and re-verifies on `encode_bundle`; a stray `/manifest` entry fails the whole offer as a confusing `SessionLimit`. This unit populates more owned content, making the edge likelier — the plan should add an explicit family filter + clearer diagnostic (predates this design).
- **Task 6 forward requirement (recorded, not built here):** when delegated-editor write lands, `delegate_section` only refuses areas *outside* `/articles/` — it does not pin a delegated cap to a single declared section; Task 6 must specify section-scoping + an owner-vs-delegate reader distinction (the `author_subspace` field is already carried for this).
- **Doc-debt (plan-stage):** `composite_moderation_admission.rs` / `site_ffi.rs:591-604` reference a `delegate_moderation` test that doesn't exist in the repo; clean up the dangling reference the "/mod ~ /articles owner-only" comparison leans on.
