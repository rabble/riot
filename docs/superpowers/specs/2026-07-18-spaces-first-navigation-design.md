# Spaces-First Navigation — Design

**Date:** 2026-07-18
**Status:** Design — brainstormed + approved (visual companion). Next: design-review gate → writing-plans.
**Scope:** Reshape Riot's shell information architecture so **spaces are the anchor** — the user's own space, the community spaces they belong to, and the indymedia sites they follow — with tools demoted to a per-space detail. This becomes the **spine of composite-site Unit 6** (native UI); the Unit 4 composite render is the followed-site detail. iOS + Android + macOS shells over the shared Rust core (no business logic in shells).

---

## 1. Problem

Today the shell is **single-community**. The nav is four routes — Home / Tools / People / Nearby — that all live *inside one active community*; switching communities is a modal (`openCommunityChooser` / `isCommunityChooserPresented`, `apps/ios/Riot/AppModel.swift`). A comment at `AppModel.swift:6` records that the "single-community shell" redesign deliberately **removed** an older top-level "Spaces" surface (the old Spaces/Apps/Board/Post/Connect debug surfaces).

Two things make that inversion wrong now:

1. **Hierarchy is upside down.** "Tools" is signed-JS apps that run *inside* one space — the narrowest concept in the system — yet it occupies a top-level nav slot. The spaces a person belongs to are the widest, most meaningful organizing concept, and they're demoted to a modal.
2. **Composite-site work multiplies the need.** Units 0–5 landed "follow indymedia sites." A person will follow *many* sites and belong to *several* communities — browsing, comparing, and moving between them is the primary activity. A modal chooser is the wrong tool for "I follow twelve sites."

**Principle (locked with the maintainer):** the spaces you own + the community spaces you belong to + the sites you follow are the anchor; tools are just things that run inside a space.

## 2. The three kinds of space

The core already models a per-space `CommunityRelationship` (`crates/riot-ffi/src/mobile_api.rs` `CommunityRow`). This design recognizes **three tiers**, two of which exist today plus one new:

| Tier | Relationship | Read/write | Exists today? |
|---|---|---|---|
| **Your space** | personal home — your profile, drafts, your posts | fully yours | **NEW concept** — distinct from an organized community |
| **Communities** | organizer or member of a communal space | read + write | yes (`CommunityRelationship`) |
| **Following** | an indymedia site you follow (composite site) | mostly read | yes (composite-site Units 0–5; render = Unit 4 `ResolvedCompositeSite`) |

"Your space" is a **first-class personal tier**, pinned at the top — not merely "a community you organize." This needs a small amount of core work to distinguish a personal home space from an organized community (see §6).

## 3. The anchor: two-pane, adaptive

A **two-pane layout** — a tiered space list on the left, the selected space's detail on the right — is the anchor:

```
┌──────────────┬───────────────────────────────────────┐
│ YOUR SPACE   │  indymedia.org            [Following ✓]│
│  ★ You—home  │  Editorial · Comments · Wire           │
│ COMMUNITIES  │  ┌───────────────────────────────────┐ │
│  Richmond ·3 │  │ EDITORIAL — Report from the port… │ │
│  Portland MA │  ├───────────────────────────────────┤ │
│  Bay (sync…) │  │ OPEN-WIRE tip: medics needed…     │ │
│ FOLLOWING    │  ├───────────────────────────────────┤ │
│  indymedia●  │  │ [hidden — moderated] placeholder  │ │
│  crimethinc  │  └───────────────────────────────────┘ │
│ ＋ Add/follow │                                        │
└──────────────┴───────────────────────────────────────┘
```

- **macOS / iPad:** both panes visible (the macOS shell is already a `NavigationSplitView` — this reuses it).
- **Phone:** collapses to a drill-in — the space list is the root screen; tapping a space pushes its detail; back returns to the list.
- One design, adapts per device. The list is **always the way home** — no modal chooser.

The left list is **grouped by tier** (Your space / Communities / Following), each row carrying the honest state the core already provides (recent activity, sync freshness, `pendingFirstSync`, `quarantined`, `available`). "Add / follow a space" lives at the bottom of the list.

## 4. The detail: fits the kind of space

The right pane **adapts to what the space is** (not a uniform set of routes):

- **Your space** → your posts, profile, drafts, settings.
- **Community you belong to** → today's routes, but now *inside the selected space*: **Home** (feed) · **People** · **Nearby** · **Tools**. Tools is a screen here, not a top-level tab.
- **Followed indymedia site** → the **Unit 4 composite render**: **Editorial** (front page, trust-tier styled) · **Comments** · **Wire**, plus a **Follow / Unfollow** control. Mostly read. Moderated items render as accountable placeholders (Unit 3/4), never vanish. Degradation states (moderation-loading, editorial-only, transport-blocked, manifest alarms) render per Unit 4's `SiteDegradation`.

This is the load-bearing consequence: **Tools leaves the top level entirely** and becomes a per-community detail; a followed site never shows Nearby/People (they have no meaning for a site you follow).

## 5. What this replaces / reuses in the shells

- **Reuses:** the macOS `NavigationSplitView`; `CommunityRow`/`CommunityChooserRow` data (title, relationship, activity, freshness, honest states); the composite render FFI (`ResolvedCompositeSite`, `SiteTrustTier`, `SiteDegradation`, `SiteItemTreatment` from Unit 4 Task 7).
- **Rewrites:** `RiotDestination` (four top-level routes → the space list is the root; Home/People/Nearby/Tools become *per-community* sub-routes); `CommunityShell` / `ConferenceShellView` / `AppModel` navigation; the modal `CommunityChooser` becomes the persistent left pane.
- **Removes from top level:** the Tools tab.

## 6. Core / FFI work (shared Rust, no logic in shells)

Most of this is a shell reshape, but three core-side items:

1. **Personal "your space" concept.** Distinguish a personal home space from an organized community in `CommunityRelationship` (or a sibling flag), so the shells can pin it. Small, additive; may touch `mobile_api.rs` / `mobile_state.rs` (a `uniffi` surface change ⇒ binding regen + native staticlib rebuild in the same commit, per the UniFFI gate — `scripts/conference/build-native-core.sh`).
2. **Followed-site relationship + list.** Surface followed composite sites in the same "list my spaces" call (or a sibling) with a `following` relationship, so the left pane draws all three tiers from core. Composite sites are addressed by their owned root; the render is Unit 4's resolver.
3. **Per-space detail routing is shell-only** — core exposes the data (community projection, `ResolvedCompositeSite`, personal-space contents); the shell picks the detail surface by tier. No business logic in the shell (which tier a space is, and every trust/treatment/degradation decision, is core-resolved).

## 7. Relationship to Unit 6 (composite-site native UI)

**This design becomes the spine of Unit 6.** The gate-passed Unit 6 plan (`docs/superpowers/plans/2026-07-18-composite-site-unit6-native-ui.md`) assumed today's single-community shell; it is **re-planned spaces-first**. The Unit 6 deliverables slot into the new IA:

- follow-site view → the "Follow / Add a space" action + the Following tier.
- composite surface + trust-tier styling → the **followed-site detail** (right pane), rendering `ResolvedCompositeSite`.
- degradation / transport states → rendered in the followed-site detail.
- editor-invite handshake, QR gen + camera scan, writer expired-cap warning, mandatory seizure disclosure, compose-time `require:arti` notice → unchanged obligations, now hosted inside the appropriate space detail / creation flow.

Sequencing: the spaces-first shell reshape is the **first** work of the re-planned Unit 6; the composite render (already built in core + FFI) is wired in as the followed-site detail.

## 8. Non-goals (v1)

- No change to the composite-site core/protocol (Units 0–5 stand).
- No new tools/apps runtime — Tools is *relocated*, not redesigned.
- No cross-space aggregation/feed (a unified "all my spaces" timeline) — each space is entered individually. A future slice could add an aggregate Home for "Your space".
- No redesign of the community internal routes (Home/People/Nearby) beyond relocating them inside the selected space.

## 9. Risks

1. **Large shell rewrite across 3 platforms** touching landed, working single-community code (`AppModel`, `CommunityShell`, `ConferenceShellView`, both `project.pbxproj`). Mitigate: reuse the macOS split view; keep the per-community routes intact (relocate, don't redesign); land behind the existing test suites (Shell/Tab navigation tests get rewritten to the new IA).
2. **pbxproj serialization hazard** — new Swift files need entries in both iOS + macOS `project.pbxproj`; coordinate (shared-checkout rule).
3. **"Your space" scope creep** — keep it minimal (a pinned personal home); resist building a full personal CMS in v1.
4. **UniFFI gate** — any `mobile_api` surface change for the personal-space/following relationship requires binding regen + native staticlib rebuild in the same commit.
