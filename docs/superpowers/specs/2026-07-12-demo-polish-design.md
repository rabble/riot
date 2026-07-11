# Demo polish design — seeded demo mode, motion kit, macOS-clean

## Purpose and scope

Make Riot easy to demo live and make the demo *feel* like the thing it is: a
tool that works with no internet in the room. Two deliverables, both mostly
new files, plus a thin integration pass:

1. **Demo mode** — a seeded, believable space loaded through the real import
   pipeline, so an organizer's phone looks lived-in the second the app opens.
2. **A motion kit** — reusable animation/haptic components that make P2P sync
   *visible*, applied to the five screens the demo script touches.

Both are shared RiotKit code, so the macOS app (which compiles those sources
by reference) gets them for free — subject to the platform rules below.

The demo script itself (`docs/product/demo-script.md`) is a committed artifact
written **first**: it is the spec that decides which integrations matter. Any
polish not reachable from the script is out of scope.

Not in scope: navigation rework, custom tab transitions, Android polish
(Android stays functional-but-plain), and any change to the sync protocol.

## Prerequisite discovered during design: minimal display names

**Three of the four demo beats currently render hex gibberish.** There is no
display-name plumbing anywhere: `app_display_name` returns `"member-" + 8 hex`
(`mobile_state.rs:1374`), `AlertPayload` has no author-name field
(`model/mod.rs:73`), and endorsements carry endorser *subspace ids* with no id→name
mapping — so the board shows `member-a3f9c2b1`, the finale's emotional peak reads
*"checked by member-3f9a…"*, and "Endorsed by KC Mutual Aid" has no name source at
all. Seeding names into fixture content does not fix this: the *live* checklist
check and the *live* sync arrival are exactly the moments that matter, and they
would still show hex.

So a minimal display-name layer is **in scope as this project's first phase**, not
a follow-up. It is small because it reuses machinery the app-directory work already
built, and it follows the recommendations already researched in
`docs/research/2026-07-11-user-profiles-willow-research.md`:

- **A profile entry in your own subspace** at a new `profile/` path family —
  an ordinary signed Willow entry, canonically CBOR-encoded (same manual style as
  `apps/endorse.rs`), payload `{ display_name: String }`, size-capped.
- **Two-gate import admission** for the new path family — `verify_frame`'s schema
  gate plus `inspect`'s binding gate (entry subspace must equal the path's subspace
  component, so nobody writes a profile into someone else's slot). This is the same
  single-sourced-classifier pattern as `is_app_data_path` / `classify_app_index_path`;
  every new Willow path family in this codebase needs it.
- **A resolver**: scan profile entries → `subspace_id → display name`.
- **The Earthstar display rule (non-negotiable, from the research):** never render a
  self-claimed name without its key-derived component. The UI shows **`Ana · a3f9`**,
  never a bare "Ana". Same-name-different-key never merges — identical to the
  impersonation rule the directory already enforces for apps.
- **FFI**: `set_display_name(name)`, and display names surfaced on entries,
  endorsements, and `riot.whoami()` (so the checklist writes a real name).

Because a profile is just another signed entry, it **syncs like everything else** —
which upgrades the finale rather than complicating it: phone B learns Ana's name
because her profile entry arrives over the same nearby transport, live on stage.

Explicitly NOT in scope: avatars, per-space keypairs / persona linking (the
research's privacy default — a real identity change, deserving its own spec),
profile editing beyond a single name field.

## The demo script (the driver)

Roughly four minutes, two iPhones, both in airplane mode with Bluetooth on:

1. **Open** — the app is already in the seeded *Riverside Tenants Union*
   space: a board of six believable alerts (courthouse support ask, supply
   drop, know-your-rights note, …), real member display names, recent-reading
   timestamps.
2. **Discover** — the App Directory storefront: the checklist ("Built into
   Riot", already on) plus *Shift Signup*, endorsed by two named groups,
   sitting under Available.
3. **Trust** — open its review page (author, provenance, endorsements,
   plain-language permissions) → "Let everyone here use this" → stamp-slam +
   haptic thunk. It appears in Tools.
4. **Sync finale** — phone B: radar pairing screen finds phone A; entries
   visibly *arrive* on B's board with stamp animations; an item checked on A
   ripples into B's checklist as "checked by Ana". Closing beat: the
   no-internet banner.

## Demo mode: a seeded space that is not fake

**The seed is a real, signed import bundle.** `fixtures/demo/riverside/` holds
a committed, deterministically-built RIOTE1 bundle (alerts + app-index entries
for the endorsed Shift Signup app + endorsement markers + a half-done
checklist's app-data entries), authored by a **fixed public demo identity** —
the same fixed-public-author precedent as the conference fixture, and, like the
starter catalog, **no key material is committed**: integrity is content-derived
and the bundle is only ever *imported*, never re-signed at runtime.

Loading it goes through the ordinary `inspect → plan_all → commit` pipeline
that every other bundle uses. Consequences that make this worth doing:

- Every seeded entry is a genuine signed Willow entry, so **seeded content
  syncs for real on stage** — the finale isn't a special case.
- Demo mode exercises the real code paths, so it is also a de-facto
  integration test rather than a parallel fake-data path that can rot.

**Entry point:** a hidden toggle — long-press the version string in Settings —
reveals "Load demo space" / "Hide demo space". Demo content lives in its own
namespace and is **additive**: it never touches or overwrites real spaces or
the person's identity.

**On removal — be precise, because Willow is append-only.** There is no delete
primitive and this design does not invent one. "Hide demo space" makes the
profile stop listing the demo namespace (the entries remain in the local store,
inert and unreachable from the UI, exactly as any un-listed namespace would
be). Genuinely reclaiming the bytes is a profile reset — the same escape hatch
that already exists. The implementation plan must confirm how the profile's
space list is stored before wiring this, and if hiding turns out to need a new
persisted "hidden namespaces" concept, that is a small, explicit addition to
call out rather than smuggle in.

**Drift guard:** a test asserts the committed fixture bytes equal a fresh
deterministic rebuild from the source content, mirroring the checklist
fixture's guard. Editing the seed content without repacking fails CI.

## Motion kit (`apps/ios/Riot/Design/Motion/`, shared RiotKit)

Five components, each a small SwiftUI view or view-modifier, each usable from
any screen and each safe to compile on macOS:

| Component | What it does | Used by |
|---|---|---|
| `StampSlam` | Rubber-stamp landing: scale overshoot + slight rotation, in the existing pink stamp color | Alert arrival on the board; the trust confirmation |
| `SyncRipple` | A ring pulses out from a newly-arrived item; attribution ("checked by Ana") fades in | Board arrivals; checklist item changes |
| `RadarPairingView` | Concentric sweep during nearby pairing; a discovered peer pops in as a labeled dot | The Connection/pairing screen |
| `Haptics` | Three named moments — trust thunk (heavy), sync complete (success), arrival (light) | Trust action; sync completion; entry arrival |
| `FinaleBanner` | Dismissible "No internet. No servers. Just these phones." | Demo mode, closing beat |

**One animation, two payoffs:** `StampSlam` is deliberately reused for both the
trust confirmation and entry arrival — the app's signature gesture, not two
unrelated effects.

**Platform rules (these are what make macOS free):**
- `Haptics` wraps `UIImpactFeedbackGenerator` in `#if os(iOS)` and compiles to
  a **no-op stub on macOS**, so call sites are identical on both platforms.
- No `UIKit` import may leak out of a Motion component; everything else is
  pure SwiftUI.
- `RadarPairingView` renders the peers the transport layer reports. Where
  macOS nearby transport differs from iOS BLE, it simply shows the paths that
  platform actually has (local network) — the view has no platform branching
  of its own.

Everything else — the seeded space, the storefront, the board, the checklist —
is shared code and behaves identically on macOS with no extra work.

## Integration pass (the only shared-file edits)

The demo script touches five screens: Board, Directory/storefront, App review
sheet, Tools, Connection/pairing. Wiring the motion kit into them requires
small edits to `ConferenceShellView.swift` and `AppModel.swift`, which the
**iOS runtime session currently claims**. This is the one coordination point:
claim a short integration window in `COLLABORATION.md`, land the new-file
modules first (zero conflict), then do the integration edits in one focused
commit.

## Error handling and plain language

| Situation | What the person sees |
|---|---|
| Demo space fails to load (corrupt fixture) | "Couldn't load the demo space" — the app stays in its real state, nothing half-imported (the import pipeline is transactional) |
| Demo mode on, real data present | Both exist side by side; demo content is visibly a separate space, never merged |
| Pairing finds no peer | The radar keeps sweeping with "Looking for people nearby…" — never an error dialog |
| Sync arrives while a screen is open | Content animates in; no modal, no "synced!" toast |
| Haptics unavailable (macOS, or iOS setting off) | Silently nothing — never a fallback alert |

## Testing strategy

- **Demo loader:** a real test — toggle on → seeded space present via the
  actual import pipeline → toggle off → gone, and a pre-existing real space is
  bit-for-bit untouched throughout.
- **Fixture drift guard:** committed bytes == fresh deterministic rebuild.
- **Motion components:** existence/render tests (each component renders in
  light and dark, on both platforms) — animations themselves are not
  unit-tested.
- **macOS compile gate:** the macOS target builds with the Motion kit included;
  a test asserts the `Haptics` stub is what's compiled off-iOS.
- **XCUITest:** walks the entire demo script end-to-end on iOS (open → seeded
  board → directory → trust → Tools), which is also the regression test for
  the integration pass. The two-phone sync finale is verified by a manual
  simulator/device pass — it needs two devices and real radios.

## Sequencing

1. Demo script (`docs/product/demo-script.md`) — written and committed first;
   it decides everything below.
2. **Minimal display names** (the prerequisite above): profile codec + `profile/`
   path family + two-gate admission + resolver + FFI + the `Ana · a3f9` display
   rule. Entirely `cargo test`-verifiable; no UI.
3. Seeded fixture + deterministic builder + drift guard (core/fixtures; no UI).
   Depends on step 2 — seeded members carry real profile entries.
4. Demo-mode loader + hidden toggle (shared RiotKit; testable without UI).
5. Motion kit components (all new files; zero shared-file conflict).
6. Integration pass into the five screens (the one coordinated window).
7. macOS verification pass (build + render tests; no new UI code).
8. XCUITest of the full script.

Steps 2–5 can proceed while the iOS runtime session still holds the shell
files; only step 6 needs the coordination window.

**Note on the sync interaction:** profile entries are a new path family, so —
like app entries — they will not cross the sync surface until the sync-inclusion
work (app-directory Task 5b, currently owned by another session) lands and is
generalized past app paths. The demo's live "phone B learns Ana's name" beat
therefore *depends on that task*. Until it lands, seeded profiles still render
correctly on each device from local fixtures; only the live cross-device name
learning waits. The implementation plan must verify 5b's landed shape and, if it
hard-codes app paths, generalize the participating-entry predicate to include
`profile/` rather than adding a second parallel mechanism.
