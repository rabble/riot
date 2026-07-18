# Engagement gap map (2026-07-18)

What's built vs. missing for UX/engagement, grounded in `origin/main`. Riot is
**local-first P2P — no server** — which bounds what "engagement" can mean
(no server push; notifications are local + sync-triggered; "what's new" is a
locally-tracked seen-cursor, not a server feed).

> Note: a shared local checkout can lag `origin/main` by dozens of commits.
> Verify against `origin/main` (`git show origin/main:<path>`), not the working tree.

## Built (do NOT rebuild) — mostly from PR #42, on main

| Area | Where | State |
|---|---|---|
| Invite loop: share link + **QR generate** + **QR scan** + join-by-link | `ShareCommunitySheet/Model`, `QRImageRenderer` (CoreImage), `QRScannerView` (AVFoundation), `JoinByReferenceSheet/Model` | DONE, tested |
| Read alerts | `AlertsListView` + `AlertDetailSheet` | DONE |
| Editor-for-joined | FFI `newswire_is_editor` (`newswire_ffi.rs:459`), `canOfferEditorialControls` | DONE |
| Add-a-tool (iOS) | `ProfileRepository.installApp`, `DirectoryView` gated button | DONE |
| Compose modes (Update/Alert/Request) | `PostUpdateView` segmented picker + operational card | DONE |
| Newswire **post-moderation** (feature/hide/tombstone/correct) | core + FFI `create_newswire_editorial_action` + `NewswireEditorial.swift` | DONE |
| Tools open in-tab + activity strip + single header | #44/#40 | DONE |

## Missing — the net-new engagement work, ranked

### 1. What's-new / unread — **Swift-only, first fix**
The projection already exposes what's needed: `NewswireProjectedPost` carries
`entry_id: String` and `tai_j2000_micros: u64` (the Willow order key, newest-first).
Seen-state is a per-DEVICE UI concern → persist a per-community last-seen cursor
in **UserDefaults** (not the Willow store), diff the projection → unread count.
- **FFI: none.** Swift-only. No native rebuild.
- UI: Home tab **badge**, a **"N new"** front-page delta, per-item **new dot**;
  advance the cursor when the user views Home.
- Effort **S/M**. Highest ROI (the reason to reopen the app).

### 2. Local notifications (P2P-honest)
On nearby sync / background refresh, if unread > 0, fire a local
`UNUserNotification` ("N new in <community>"); a foreground new-content banner.
No server push exists and none is possible — this is local only.
- FFI: none (uses #1's cursor + a sync/scenePhase hook).
- Depends on #1. Effort **S/M**.

### 3. Threaded replies — flat comments (minimal real)
Read-side scaffolding already exists in the Unit 4 resolver: `TrustTier::Comment`
(`resolve.rs:28`), `resolve_soft_link(parent_ref, held_article_ids)`
(`resolve.rs:175`), a "comments loading" degradation state. Nothing **produces** a
comment. `SiteRole::Comments` (`manifest.rs:62`) is only a manifest pointer.
- **Needed:** a comment record schema in core (`{parent_entry_id, body, timestamp}`
  with a `parent_ref:[u8;32]`) + codec + communal writer (fresh subspace per
  comment for the design's unlinkable property); FFI `create_site_comment` +
  `list_comments_for_post`; iOS reply-compose + grouped read under a post.
- **Data-model constraint:** flat comments fit Willow cleanly (distinct entry per
  comment, tree built client-side at render). Unlinkable authors ⇒ **no per-person
  comment history and `/mod/` Revoke cannot ban a commenter — moderate per-content
  (tombstone) only.** Nested reply-to-reply is representable but low value — defer.
- Effort **M**. The participation loop.

### 4. Composite-site `/mod/` moderation — surfacing job
Core is fully built (Unit 3 `moderation.rs`: Revoke-a-person, Tombstone-content,
`ModEpoch` freshness heartbeat, 24h window; Unit 4 resolver overlay). Import/read
of received `/mod/` records is wired at the admission gate. But there is **no
create FFI, no read-state FFI, and no iOS**. (A `feat/composite-unit4-ffi` branch
is WIP.)
- **Needed:** `create_moderation_record(Revoke|Tombstone|Endorse, target)`
  owner-signed → import; `read_moderation_state()` exposing revoked subspaces +
  tombstoned ids + freshness verdict; an **owner-only** iOS moderation sheet +
  a "moderation N hours stale" banner (core `evaluate_freshness` computes it).
- **Constraint:** `/mod/` is owner-signed only (never delegated); Revoke targets a
  subspace, so it can't ban unlinkable commenters (see #3). Freshness is a real UX
  requirement — a stale mod set must degrade visibly or revokes silently roll back.
- Effort **M/L** (core done; purely surfacing). Owned sites have no app UI yet, so
  this pairs with the broader owned-site UI (composite-site Unit 6).

### 5. Android parity
#42's surfaces (join/share/alerts/add-a-tool/compose-modes) are iOS/macOS only.
Android is a generation behind (new-model core present but the app is still the
old debug shell). Big separate track; net-new for every surface above.

## Sequencing
1 → 2 (retention loop, Swift-only, no FFI) → 3 (comments, participation, one FFI
unit) → 4 (`/mod/` surfacing) → 5 (Android). Each: TDD, `scripts/ios-check.sh`
fast-loop verify, worktree off `origin/main` + pathspec, plan-review gate, and any
new `uniffi::Record` lands with the native staticlib rebuild in the same commit.
