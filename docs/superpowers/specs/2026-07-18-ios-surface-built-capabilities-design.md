# iOS — Surface Built-but-Stranded Capabilities & Kill Dead-Ends — Design

**Date:** 2026-07-18
**Status:** Design — **gate PASSED 5/5 (2026-07-18)**. PM, Security, Designer, Architect, CTO all APPROVED (round 2 for the three that flagged Unit 4 / composer / chooser). Ready for planning.

**Revision note (gate r1):** Unit 4 (editor un-gate) can't be pure-Swift — a joined community's roster isn't readable across FFI; split into **4a (new `newswire_is_editor` FFI predicate, one binding regen + staticlib rebuild) + 4b (Swift consumer)**, §8 blanket "no new FFI" corrected. Unit 6 mode picker would strand users (Alert/Request need source/expiry/location fields the composer lacks) → it now ships those operational inputs + inline validation. Unit 1 also fixes the chooser's existing dead Create/Find-nearby no-ops. `AlertDetail` is a value struct not a view → Unit 3 builds a detail sheet. Alerts are global not per-community → scoped to the active community. QR hardened (riot://-only, length-bound, teardown, actionable errors); pending-sync no longer leads with the red Nearby CTA; duplicate-join handled; alert signer renders the core-verified value.
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
  - Scan: camera via `AVCaptureMetadataOutput` (`NSCameraUsageDescription`). **Security (gate r1):** accept **only** the `riot://` scheme, length-bound the payload before decode, hand the raw string to `newswire_decode_share_reference`, and never open/auto-follow any other URL. **Tear down `AVCaptureSession` on dismiss/backgrounding.** A non-`riot://` or malformed code → the **same actionable error** as paste (no crash, no silent no-op).
- **Honest preview:** "Join community `<short-ns>`? Its name and posts arrive on first sync." Share ref (`NewswireShareReferenceV1`) = namespace + descriptor-entry + digest, **no title** — UI never fabricates one (anti-spoof). Render the short namespace in the monospace `IdentifierRow` treatment.
- **Duplicate join:** if the decoded namespace is already in `list_communities()`, switch to that community instead of erroring / creating a second entry.
- Confirm → import/join (`inspect_bytes` → `MobileImportPreview` → `create_plan` → `accept()`, or `join_public_space`) → member community in **"pending first sync"** state.
- **Pending-first-sync (corrected):** lead with the honest explanation ("Posts arrive once a peer or seed connects"), offer a verified-working path (re-open with a link / wait). **Do NOT lead with the Nearby CTA** (two-peer nearby sync is red on main) — keep Nearby secondary, never the headline action.
- Camera denied → "Open Settings" using Nearby's recovery *pattern* but **camera-appropriate copy**, gated on `AVCaptureDevice.authorizationStatus`. Paste-a-link is always the no-camera fallback.

**Generate side — "Share this community"** (Community settings sheet)
- `newswire_share_reference(active id)` → `riot://` string → locally-rendered **QR image** + iOS **Share sheet** for the link.

**Read alerts** (Home)
- **`AlertDetailSheet` (NEW view):** `AlertDetail` at `AppModel.swift:984` is a *value model struct* (`init(entry:)`, summary/technical split) — **not** a SwiftUI view. This unit **builds** the detail sheet rendering it (reuse the `DisclosureGroup` technical-details pattern from `CatalogFailureView`/`CommunitySettingsSheet`).
- **`AlertsListView`** rows: headline + **core-verified signer** + `AlertFreshness`, organizer-first. **Anti-spoof (gate r1):** signer + organizer-first ordering come from the core's verified signature / organizer-marker classification via `list_current_entries` — never a self-claimed author field. Headline/name render as plain `Text` (no markdown/AttributedString auto-link).
- **Entry point + scoping (corrected):** ONE **Alerts card on Home** (per-community) — not the Nearby "On this device" `LabeledContent` (avoid two divergent entry points). `model.entries` is currently **global** (`list_current_entries`/`currentEntries()` isn't community-scoped; `reproject_active` reprojects for the active community) — the card MUST show only the **active community's** alerts; add a filter if the set is genuinely cross-community, to preserve per-community honesty.
- The dead `LabeledContent("Signed alerts", value: count)` → tappable entry into the per-community list.
- Empty = benign "No alerts yet".

---

## 4. Editor — un-gate joined communities (requires ONE FFI addition)

**Gate-round-1 correction:** a joined community's `editorial_roster` is **not** reachable across FFI today — it exists only as CREATE input (`NewswireSpaceInput`) and inside the in-store `SpaceDescriptorV1`; `NewswireProjectionView` and `CommunityRow` carry no roster, and Swift's `CommunityContext.editorialRoster` is a create-time display-only value (`CommunityShell.swift:34/45/51/412`, consumed at `ConferenceShellView.swift:311`) — the session-only hack this unit replaces. (Note: `ConferenceShellView.swift:98` is the *founding roster fed into core* at create — that MUST be kept; it seeds the stored descriptor roster the predicate reads back.) So this unit is **not** pure-Swift.

- **New FFI predicate (keeps roster logic in Rust — shared-core):** `newswire_is_editor(descriptor_entry_id, subject_id) -> bool`, computed from the descriptor's authenticated roster (post-verified-sync). Prefer this over widening `NewswireProjectionView` — it avoids threading raw roster bytes through Swift and returns a descriptor-authenticated answer, not a locally-asserted one. This is a **new `#[uniffi::export]` free fn** → **one binding regen + coordinated native staticlib rebuild** (record-change / checksum-coupling discipline; coordinator-centralized). No new `uniffi::Record`.
- Swift: show `EditorialActionSheet` controls iff `newswire_is_editor(activeDescriptorId, whoami.id)`.
- **Honest ordering + not-yet-synced:** before a joined community's descriptor has synced, the predicate is false → no controls, shown with a one-line note "Editorial controls appear after this community's first sync," not a bare empty view.
- **Defense in depth (display ≠ authority):** the UI predicate is a *display* gate only. The core still independently rejects an editorial action from a non-roster author at signing/admission — so even if controls were forced visible, the action fails at core. A test asserts this.
- Note: the newswire `editorial_roster` is the editorial mechanism in *this* app. The cryptographic cap layer (composite-site Unit 1) is the separate, deferred owned-site track — not wired to this UI.

**4a implementation requirements (gate r1, from Architect/CTO — must hold or the predicate is wrong):**
- Reuse the **existing** authenticated read: `load_space_descriptor` + the same `roster.contains(&signer_id)` membership check used at admission (`entry.rs:223`, `projection.rs:237`) — do NOT introduce a parallel roster lookup, so the display gate and the authority gate stay provably identical.
- **Empty-roster semantic = whatever admission does (CORRECTED, plan-gate r1).** The invariant is **display == authority**. Verified: the core admission gate `require_action_authority` (`entry.rs:212`) has **no founder special-case** — it requires `roster.contains(signer)`, so a founder who set an *empty* roster is **rejected** at admission. Therefore the display predicate must also return **false** for founder + empty roster (else it shows controls the core rejects — the exact divergence this unit prevents). ⇒ **member ⇒ true; non-member ⇒ false; founder + empty roster ⇒ false** (matches admission). The current Swift `isRecognizedEditor` treats empty-roster as founder-true — that Swift behavior *diverges from the core* and is replaced. **Separate product question (out of scope):** if a founder *should* always be an editor, that's a change to `require_action_authority` (admission) — do it there so display inherits it, not a display-only special-case. The extracted `is_editorial_authority` is the single place both would change together.
- **Unknown / not-yet-synced descriptor ⇒ `false`, no error** (drives 4b's "controls appear after first sync" note off a defined false).
- **4b deletes the dead DISPLAY-only path** (`EditorialAuthority.isRecognizedEditor` `NewswireEditorial.swift:206`, `CommunityContext.editorialRoster` `CommunityShell.swift:34/45/51/412` consumed at `ConferenceShellView.swift:311`) so two roster-authority sources never coexist. **Do NOT touch `ConferenceShellView.swift:98`** — that is the founding roster fed into core at create, load-bearing.
- Rust tests: roster member ⇒ true; non-member ⇒ false; joined member + empty roster ⇒ false; **founder + empty roster ⇒ false (matches admission)**; unknown descriptor ⇒ false; non-descriptor entry id ⇒ false.

---

## 5. Organizer — add-a-tool

- **"Add a tool"** button (Tools route header + the empty-state action) → document picker → `install_app(manifest, bundle)` (or `install_from_directory`) → tool appears in Tools → existing `AppReviewSheet` handles the organizer trust decision.
- Mirrors Android's existing import flow. Tools empty state stops being terminal.

---

## 6. Contributor — alert/request compose mode (+ the operational fields)

- `PostUpdateView` has `ComposerMode` in the model but never draws the picker. Add a segmented **Update / Alert / Request** control at the composer top.
- **Gate-round-1 correction (avoid a stranded state):** the model's validation already requires **source claim + expiry + coarse location** when mode ≠ update (`PostUpdateViewModel` validation), but the view renders no inputs for them — so selecting Alert/Request today would leave Post permanently disabled with nothing to satisfy. This unit therefore **also adds the operational inputs** — a source-claim field, an expiry picker, and a coarse-location field — shown **only** when Alert/Request is selected, plus inline `model.validation` messaging ("add a source and expiry to post an alert"). Update mode is unchanged (no extra fields).
- Alert vs Request must be **user-visibly distinct** (Alert = incident/warning; Request = an ask/needs). If they resolve to the same operational post kind at the core, collapse to one control rather than ship two identical-feeling modes. Confirm the distinct outcome during implementation.
- Alert/Request route through `create_newswire_post` with the operational overlay built from those fields.

---

## 7. Dead-end fixes (baked in)

- **No-op "Open wire" button** (`NewswireEditorial.swift:652`, empty closure) → wire to focus/scroll the open-wire section (a real action); the wire already renders below, so this is an anchor, not new surface.
- **`offlineStale` "Try again" loop** → re-derive the descriptor id via `listCommunities`; if still absent (nearby-joined, no `descriptorEntryId`), offer **"Sync with a peer" (→ Nearby)** or **"Rejoin with a link"** — a path forward, not a silent re-loop.
- **Tools empty state** → "Add a tool" action (§5).
- **Post-join "pending first sync"** → honest waiting state; verified-working path first, Nearby secondary (§3).
- **Chooser's dead no-op buttons (gate r1):** opened from the shell, `CommunityChooserView`'s "Create a community" / "Find one nearby" fall through to empty `{}` default closures (`CommunityChooser.swift:183-190`) — dead no-ops in the exact sheet Unit 1 edits. Wire them to the real create / Nearby actions while adding "+ Join another" (all in Unit 1).

---

## 8. Work units (decomposition)

Shared-core rule holds: no business logic in the app; all new views consume existing FFI + resolved view models. New Swift files require **both** `apps/ios/Riot.xcodeproj` + `apps/macos/Riot.xcodeproj` `project.pbxproj` registration (hand-edited, serializes across sessions — claim in COLLABORATION.md).

| # | Unit | Files (iOS) | New FFI? |
|---|---|---|---|
| **1** | `JoinByReferenceSheet` (paste + scan QR + preview + duplicate-join + commit) + entry points (Launch, Chooser) **+ fix chooser's dead Create/Find-nearby no-ops** | NEW `JoinByReferenceSheet.swift`, `QRScannerView.swift` (AVFoundation), edits to `ConferenceShellView`/`CommunityChooser`; `Info.plist` camera key; both pbxproj | No (`newswire_decode_share_reference`, `inspect_bytes`/import) |
| **2** | "Share this community" (link + QR generate) | NEW `ShareCommunitySheet.swift` (+ QR render), edit `CommunitySettingsSheet`; both pbxproj | No (`newswire_share_reference`) |
| **3** | Read alerts — `AlertsListView` + NEW `AlertDetailSheet` (renders the `AlertDetail` value model), per-community scoped | NEW `AlertsListView.swift` + `AlertDetailSheet.swift`, edit Home + `AppModel`; both pbxproj | No (`list_current_entries`) |
| **4a** | **Rust/FFI:** `newswire_is_editor(descriptor_entry_id, subject_id) -> bool` predicate (descriptor-authenticated roster). **One binding regen + coordinated native staticlib rebuild.** | `crates/riot-ffi/src/newswire_ffi.rs`, `crates/riot-core/src/newswire/*`, `crates/riot-ffi/tests/*`, regenerated bindings | **YES — new `#[uniffi::export]` fn (no new Record)** |
| **4b** | **Swift:** editor un-gate — show `EditorialActionSheet` iff `newswire_is_editor(...)`; "controls appear after first sync" note; defense-in-depth (core still rejects non-editor) | edit `CommunityShell`/`NewswireEditorial`/`AppModel`, `Core/ProfileRepository.swift` wrapper | No (consumes 4a) |
| **5** | Add-a-tool (doc picker → `install_app`, routed through `AppReviewSheet` trust gate — no auto-trust) + Tools empty-state action | edit `DirectoryView`/`AppModel`, doc picker | No (`install_app`/`install_from_directory`) |
| **6** | Composer mode picker (Update/Alert/Request) **+ operational fields (source/expiry/coarse-location) + inline validation** | edit `PostUpdateView` | No (`create_newswire_post` overlay) |
| **7** | Dead-end fixes (Open-wire anchor, offlineStale path, pending-sync state) | edit `NewswireEditorial`/`AppModel` | No |

**FFI/rebuild (corrected, gate r1):** Units 1,2,3,5,6,7 are **pure-Swift** (no new FFI, no binding regen). **Unit 4a is the ONE Rust/FFI addition** — a new `#[uniffi::export]` predicate (no new `uniffi::Record`), requiring a coordinated binding regen + native staticlib rebuild (record-change / checksum-coupling discipline, coordinator-centralized), and must land before the Unit 4b Swift consumer.

---

## 9. Testing

- Per unit: `apps/ios/RiotTests/*` view-model + surface tests (mirror `NewswireSurfaceTests`, `PeopleSurfaceTests`, `CommunityChooserTests`), executed on iPhone 17 Pro sim (OS 26.2). macOS `RiotKit-macOS` build + the subset that isn't iOS-only.
- **Anti-dead-end assertions (the point of the slice):** every new terminal/empty/error state asserts a reachable next action — e.g. Tools empty state exposes "Add a tool"; `offlineStale` exposes a forward path; invalid-join-link shows an actionable error; post-join renders the pending-sync state, not blank.
- Join round-trip: generate a share ref → decode it → assert the honest pre-sync preview (namespace present, no fabricated title). **Duplicate-join** → switches to existing community. **Malformed / non-`riot://` QR payload** → actionable error state (not crash/silent). **Camera denied** → recovery state + paste fallback still available.
- Editor gating (Unit 4a Rust test + 4b Swift test): `newswire_is_editor` returns true for a roster member of a *joined* descriptor, false for a non-member and pre-sync (RED-then-green). **Defense-in-depth:** the core still rejects an editorial action from a non-roster author even with UI controls forced visible.
- Alerts: the Alerts card shows only the **active community's** alerts (per-community scoping), not a global cross-community set; signer renders the core-verified value.
- Composer: selecting Alert/Request reveals the operational fields; Post stays disabled with inline guidance until source+expiry are supplied, then enables (no permanent dead-disable). Alert vs Request produce distinct user-visible outcomes.
- Both pbxproj `plutil -lint` clean; iOS app + macOS app BUILD SUCCEEDED. Unit 4a: `cargo test -p riot-ffi` + regenerated bindings load on-device.

---

## 10. Risks / open questions

1. **Camera/QR is real new native work** (AVFoundation + permission) — the one genuinely new capability; the rest is wiring existing views/FFI. Scoped to Unit 1; paste-a-link works without it (graceful if camera denied).
2. **Two-peer nearby sync is red on main** — the join-by-link path deliberately does NOT depend on nearby; but a joined community's *content* still needs a first sync from a peer/seed, so "pending first sync" may persist if no provider is reachable. Honest state, not a bug to hide.
3. **Editor un-gate needs FFI (RESOLVED, gate r1):** confirmed the roster is not readable across FFI for a joined community → Unit 4 split into **4a (Rust/FFI `newswire_is_editor` predicate, one rebuild)** + **4b (Swift consumer)**. This is the only non-Swift unit; sequence 4a before 4b with record-change discipline.
4. **Content first-sync path:** with two-peer nearby sync red on main, how does a link-joined community actually receive its first content — seed/relay or nearby? If no working provider exists in this slice, join delivers a joined-but-empty community shown honestly as "pending first sync." Acceptable as v1 surfacing; flag that a working seed/relay follow path is the real completion (composite-site transport track).
5. **Alert vs Request** must have a distinct user-visible outcome or collapse to one control — resolve during Unit 6.
6. **Android parity** deliberately deferred — widens the iOS/Android gap; call out for the coordinator.
7. Shared-checkout: both `project.pbxproj` files serialize all Swift-file additions across sessions — claim explicitly, land file-adding units carefully. Unit 4a's binding regen + staticlib rebuild is coordinator-centralized (checksum-coupling).
