# iOS — Surface Built-but-Stranded Capabilities & Kill Dead-Ends — Design

**Date:** 2026-07-18
**Status:** Design — pending design-review gate
**Scope:** iOS only. Build the missing UI for capabilities that already work end-to-end via UniFFI, and remove existing UI dead-ends. Organized by user job. Integrates into the existing 4-tab community shell — no new tab.

---

## 1. Problem & scoping

Two audits (FFI capability surface + native UI map) found a gap between what the Rust core exposes and what a user can actually reach:

- **Built + works end-to-end + no user-reachable screen** (surface these): join-by-share-reference, reading signed alerts, editorial actions for editors of *joined* communities, iOS tool import, operational alert/request compose mode.
- **Built at FFI but NOT end-to-end** (do NOT build UI — would create new dead-ends): the whole composite-site / owned-masthead permission layer (`create_owned_site`/`restore_owned_site`, editor-cap delegation, moderation revoke/tombstone). Owned editorial can't round-trip through FFI yet (followed-root wiring + store `/articles/` retention unbuilt). **Explicitly out of scope** — it depends on composite-site Units 2–5 landing first.
- **Platform gap** (out of scope this slice): Android is a generation behind (new-model core present but unmounted). iOS-only here.

**This design surfaces the working-but-unscreened capabilities and kills the dead-ends. It does NOT build the permission model UI.**

### Locked decisions
- **Platform:** iOS only.
- **Journeys:** Reader, Editor, Organizer, Contributor (all four).
- **Join input:** link + QR — generate both; consume via paste-a-link AND camera scan.
- **Dead-end bug-fixes:** baked in regardless.
- **Nav:** integrate into the existing Home/Tools/People/Nearby shell + chooser; **no new tab**.

### Anti-dead-end invariants (apply to every screen)
1. Every join/follow entry point routes to the **same** `JoinByReferenceSheet` — no divergent half-built join paths.
2. Every terminal/empty/error state offers a real next action (or an honest "waiting" state) — never a blank, never a silent retry loop.
3. No fake data: a digest-bound share ref carries only namespace+digest, so pre-sync UI says so ("name arrives on first sync"), never invents a title.

---

## 2. Navigation integration (the map)

```
Launch (no community)
  ├─ Create a community            (exists)
  ├─ Find one nearby → Nearby      (exists)
  └─ + Join with a link/QR ──────► JoinByReferenceSheet (NEW)

Community shell (Home/Tools/People/Nearby)
  ├─ community name → Chooser
  │     └─ + Join another (link/QR) ─► JoinByReferenceSheet (same sheet)
  ├─ HOME
  │     ├─ Newswire surface  (fix no-op "Open wire" + offlineStale loop)
  │     ├─ + Alerts card → AlertsListView → AlertDetail   (read alerts)
  │     ├─ Post composer + mode picker (Update / Alert / Request)  (contributor)
  │     └─ Editorial controls shown for ANY real editor (roster-derived)  (editor)
  ├─ TOOLS
  │     ├─ + "Add a tool" (doc picker → install)   (organizer)
  │     └─ empty state → "Add a tool" action
  ├─ community settings sheet
  │     └─ + "Share this community" (link + QR)     (generate side of join)
  └─ PEOPLE / NEARBY  (exist, unchanged)
```

---

## 3. Reader — join-by-reference + read alerts

**`JoinByReferenceSheet`** (reused from Launch + Chooser)
- Segmented input: **Paste link** / **Scan QR**.
  - Paste: text field accepts `riot://newswire/join/…` → `newswire_decode_share_reference`.
  - Scan: camera via `AVCaptureMetadataOutput` (`NSCameraUsageDescription`) reads the same `riot://` string → decode.
- **Honest preview:** "Join community `<short-ns>`? Its name and posts arrive on first sync." (Share ref = namespace + digest only.)
- Confirm → import/join (`inspect_bytes` → `MobileImportPreview` → `create_plan` → `accept()`, or `join_public_space`) → member community in **"pending first sync"** state → route into shell showing that honest state.
- Errors with actions: invalid link → clear message; camera denied → "Open Settings" (reuse Nearby's deep-link pattern).

**Generate side — "Share this community"** (Community settings sheet)
- `newswire_share_reference(active id)` → `riot://` string → locally-rendered **QR image** + iOS **Share sheet** for the link.

**Read alerts** (Home)
- Alerts card (when `list_current_entries()` non-empty) → **`AlertsListView`** (rows: headline + signer via `whoami`/`profile_for` + `AlertFreshness`, organizer-first) → tap → **`AlertDetail`** (existing view `AppModel.swift:984`, currently unrendered — wire it).
- The dead `LabeledContent("Signed alerts", value: count)` becomes the tappable entry into the list.
- Empty = benign "No alerts yet".

---

## 4. Editor — un-gate joined communities

- Derive editor status from the **active** community's descriptor `editorial_roster` (not session-only). Populate `CommunityContext.editorialRoster` for any active community; show `EditorialActionSheet` controls iff `roster.contains(whoami.id)`.
- Honest ordering: joined community's roster arrives on first sync → before sync, no controls; after, a real editor sees them. No fake authority.
- Note: the newswire `editorial_roster` is the editorial mechanism in *this* app. The cryptographic cap layer (Unit 1) is the separate, deferred owned-site track — not wired to this UI.

---

## 5. Organizer — add-a-tool

- **"Add a tool"** button (Tools route header + the empty-state action) → document picker → `install_app(manifest, bundle)` (or `install_from_directory`) → tool appears in Tools → existing `AppReviewSheet` handles the organizer trust decision.
- Mirrors Android's existing import flow. Tools empty state stops being terminal.

---

## 6. Contributor — alert/request compose mode

- `PostUpdateView` already has `ComposerMode` in the model but never draws the picker. Add a segmented **Update / Alert / Request** control at the composer top; Alert/Request route through `create_newswire_post` with the alert/request overlay.

---

## 7. Dead-end fixes (baked in)

- **No-op "Open wire" button** (`NewswireEditorial.swift:652`, empty closure) → wire to focus/scroll the open-wire section (a real action); the wire already renders below, so this is an anchor, not new surface.
- **`offlineStale` "Try again" loop** → re-derive the descriptor id via `listCommunities`; if still absent (nearby-joined, no `descriptorEntryId`), offer **"Sync with a peer" (→ Nearby)** or **"Rejoin with a link"** — a path forward, not a silent re-loop.
- **Tools empty state** → "Add a tool" action (§5).
- **Post-join "pending first sync"** → honest waiting state with Nearby/retry affordance.

---

## 8. Work units (decomposition)

Shared-core rule holds: no business logic in the app; all new views consume existing FFI + resolved view models. New Swift files require **both** `apps/ios/Riot.xcodeproj` + `apps/macos/Riot.xcodeproj` `project.pbxproj` registration (hand-edited, serializes across sessions — claim in COLLABORATION.md).

| # | Unit | Files (iOS) | New FFI? |
|---|---|---|---|
| **1** | `JoinByReferenceSheet` (paste + scan QR + preview + commit) + entry points (Launch, Chooser) | NEW `JoinByReferenceSheet.swift`, `QRScannerView.swift` (AVFoundation), edits to `ConferenceShellView`/`CommunityChooser`; `Info.plist` camera key; both pbxproj | No (uses `newswire_decode_share_reference`, `inspect_bytes`/import) |
| **2** | "Share this community" (link + QR generate) | NEW `ShareCommunitySheet.swift` (+ QR render), edit `CommunitySettingsSheet`; both pbxproj | No (`newswire_share_reference`) |
| **3** | Read alerts — `AlertsListView` + wire `AlertDetail` | NEW `AlertsListView.swift`, edit Home + `AppModel` (surface entries), render existing `AlertDetail`; both pbxproj | No (`list_current_entries`) |
| **4** | Editor un-gate — roster-derived editorial authority for active community | edit `CommunityShell`/`NewswireEditorial`/`AppModel` (`editorialRoster` from active descriptor) | No |
| **5** | Add-a-tool (doc picker → install) + Tools empty-state action | edit `DirectoryView`/`AppModel`, doc picker | No (`install_app`/`install_from_directory`) |
| **6** | Composer mode picker (Update/Alert/Request) | edit `PostUpdateView` | No (`create_newswire_post` overlay) |
| **7** | Dead-end fixes (Open-wire anchor, offlineStale path, pending-sync state) | edit `NewswireEditorial`/`AppModel` | No |

**No new `uniffi::Record`** in any unit → no binding regen / native staticlib rebuild. Every unit consumes an existing, already-surfaced FFI method (except join, which uses the already-wrapped decode/import path).

---

## 9. Testing

- Per unit: `apps/ios/RiotTests/*` view-model + surface tests (mirror `NewswireSurfaceTests`, `PeopleSurfaceTests`, `CommunityChooserTests`), executed on iPhone 17 Pro sim (OS 26.2). macOS `RiotKit-macOS` build + the subset that isn't iOS-only.
- **Anti-dead-end assertions (the point of the slice):** every new terminal/empty/error state asserts a reachable next action — e.g. Tools empty state exposes "Add a tool"; `offlineStale` exposes a forward path; invalid-join-link shows an actionable error; post-join renders the pending-sync state, not blank.
- Join round-trip: generate a share ref → decode it → assert the honest pre-sync preview (namespace present, no fabricated title).
- Editor gating: a roster member of a *joined* community sees editorial controls; a non-member does not (RED-then-green on the un-gate).
- Both pbxproj `plutil -lint` clean; iOS app + macOS app BUILD SUCCEEDED.

---

## 10. Risks / open questions

1. **Camera/QR is real new native work** (AVFoundation + permission) — the one genuinely new capability; the rest is wiring existing views/FFI. Scoped to Unit 1; paste-a-link works without it (graceful if camera denied).
2. **Two-peer nearby sync is red on main** — the join-by-link path deliberately does NOT depend on nearby; but a joined community's *content* still needs a first sync from a peer/seed, so "pending first sync" may persist if no provider is reachable. Honest state, not a bug to hide.
3. **Editor un-gate depends on the descriptor roster being reachable for a joined community** — confirm `project_newswire_space`/the descriptor surfaces the roster post-sync; if not, small FFI addition (flag if so — would change the "no new FFI" claim for Unit 4).
4. **Android parity** is deliberately deferred — this widens the iOS/Android gap further; call it out for the coordinator.
5. Shared-checkout: both `project.pbxproj` files serialize all Swift-file additions across sessions — claim explicitly, land units that add files carefully.
