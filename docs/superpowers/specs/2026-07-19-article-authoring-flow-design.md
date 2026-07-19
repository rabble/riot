# Article Authoring Flow — Design

**Goal:** let an owner/editor **write and publish a long-form article** to a community's owned
masthead (`/articles/`) from iOS. Today this is an end-to-end gap: the `/articles/` region + editor
capabilities exist in core, but there is **no callable write path at any layer** (no
`create_article` FFI; article writes exist only inside Rust masthead tests) and iOS composite-site
surfaces are **read + moderate only**.

## The identity model (read first — it differs from newswire)
Owned-masthead editorial is **NOT** the communal unlinkable newswire author. Articles are authored
under the **owned-site capability model**: the owner subspace, or an editor the owner delegated a
write-capability to for `/articles/` (the masthead editorial cap / editor delegation already in
`site_paths.rs` / `masthead.rs`). So:
- Only the owner or a delegated editor may author an article; the write is capability-gated in core,
  and the UI is gated behind an `is_editor(owned_site)` predicate (mirror the existing
  `newswire_is_editor` pattern, but for the OWNED site, not the communal wire).
- This is the "composite-site vs newswire editorial" distinction — do NOT conflate `/articles/` (owned,
  caps, long-form) with the communal `newswire/v1` wire (communal, unlinkable, short posts).

## Distribution reality (be honest in the UI)
Owned-namespace content does **not auto-propagate to followers** today: publishing an article signs it
into the owned namespace, but followers/web receive it only via the existing composite/follow
hand-off (the owner passes signed bytes onward; auto owned-ns propagation is unbuilt). The composer
must be honest: "Published to your site. Followers see it the next time they sync your site" — never
imply instant fan-out. (If/when auto owned-ns propagation lands, this copy relaxes.)

## Architecture (the missing write path, core → FFI → iOS)

### Core
- An **article record** (`OwnedArticleV1`: title, body, optional summary/cover-ref, language,
  timestamps) written to `/articles/<...>` under the owned namespace, signed through the owned-site
  write-capability. Goes through the same preview → plan → commit atomic import boundary as every
  other write (copy-on-write; a fault before swap leaves state unchanged).
- Reuse the existing `/articles/` path predicates + editor-delegation authority (already present).
- Editor authority is re-checked in core at sign time — UI gating is not the security boundary.

### FFI
- `create_owned_article(site, title, body, summary?, language) -> SignedRecord` — capability-gated;
  refuses if the profile is neither owner nor a delegated `/articles/` editor (a typed refusal, like
  the app-organizer refusals).
- (Later) `update_owned_article` / `retract_owned_article` via a correction/tombstone, mirroring
  editorial semantics.
- **Read side already exists:** `resolve_composite_site` / `resolve_site_manifest` project `/articles/`
  for the read view — the composer just needs to publish into what that already renders.
- An `owned_site_is_editor(site, subject)` predicate for UI gating.

### iOS
- An **Article composer** — distinct from `PostUpdateView` (that's the communal wire). Long-form:
  Title, Body (multi-line, generous), optional Summary + cover. Gated behind `owned_site_is_editor`;
  only surfaced on a community whose owned site this profile can edit.
- Entry point: from the composite-site read view (`CompositeSiteReadView`) — a "Write article" action
  visible only to editors; published articles then appear in that same read view.
- Honest post-publish state (see Distribution reality above).

## Units
1. Core `OwnedArticleV1` record + `/articles/` write through the owned-site write-cap (TDD; atomic
   preview→commit; editor re-check).
2. FFI `create_owned_article` + `owned_site_is_editor` (+ typed non-editor refusal).
3. iOS Article composer gated behind the editor predicate; entry from `CompositeSiteReadView`;
   honest post-publish copy.
4. (Follow-up) `update`/`retract` article + surfacing in the read view.

## Out of scope
- Auto propagation of owned-namespace articles to followers/web (separate, currently unbuilt).
- Rich media beyond a cover reference; embedded apps in articles.
- Android composer (parity later).

## Testing
Core: article write admits under a valid owned/editor cap, is REFUSED for a non-editor and for the
communal-newswire author, round-trips, and appears in the composite-site projection. FFI: the
capability-gated create + `owned_site_is_editor` contract (editor vs member vs organizer-shaped).
iOS: composer only appears for editors; publish → article shows in the read view. Coverage per
`.coverage-thresholds.json`.
