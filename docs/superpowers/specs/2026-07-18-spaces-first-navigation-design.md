# Spaces-First Navigation — Design

**Date:** 2026-07-18
**Status:** Design — v2, revised after design-review gate (5/5 reviewers: PM, Architect, Designer, Security, CTO). All mechanical findings folded in; two strategic findings (sequencing, "your space" scope) were escalated to the maintainer, who accepted the risk — documented as deliberate decisions in §11. Next: re-review → writing-plans.
**Scope:** Reshape Riot's shell information architecture so **spaces are the anchor** — the user's own space, the community spaces they belong to, and the indymedia sites they follow — with tools demoted to a per-space detail. This becomes the **spine of composite-site Unit 6** (native UI); the Unit 4 composite render is the followed-site detail. iOS + Android + macOS shells over the shared Rust core (no business logic in shells).

---

## 1. Problem

Today the shell is **single-community**. The nav is four routes — Home / Tools / People / Nearby — that all live *inside one active community*; switching communities is a modal (`openCommunityChooser` / `isCommunityChooserPresented`, `apps/ios/Riot/AppModel.swift`). A comment at `AppModel.swift:4-11` records that the "single-community shell" redesign deliberately **removed** an older top-level "Spaces" surface (the old Spaces/Apps/Board/Post/Connect debug surfaces).

Two things make that inversion wrong now:

1. **Hierarchy is upside down.** "Tools" is signed-JS apps that run *inside* one space — the narrowest concept in the system — yet it occupies a top-level nav slot. The spaces a person belongs to are the widest, most meaningful organizing concept, and they're demoted to a modal.
2. **Composite-site work multiplies the need.** Units 0–5 landed "follow indymedia sites." A person will follow *many* sites and belong to *several* communities — browsing, comparing, and moving between them is the primary activity. A modal chooser is the wrong tool for "I follow twelve sites."

**Principle (locked with the maintainer):** the spaces you own + the community spaces you belong to + the sites you follow are the anchor; tools are just things that run inside a space.

## 2. The three kinds of space

The core models a per-space `CommunityRelationship`, **derived from the held author, never caller-asserted** (`crates/riot-ffi/src/mobile_api.rs:16`). This design recognizes **three tiers**:

| Tier | Relationship | Read/write | Exists today? |
|---|---|---|---|
| **Your space** | personal home — your profile, drafts, your posts | fully yours | **NEW** — see §2.1 |
| **Communities** | organizer or member of a communal space | read + write | yes (`CommunityRelationship`: organizer / member) |
| **Following** | an indymedia site you follow (composite site) | mostly read | **partial** — composite render is Unit 4 (PR #46, unmerged); no follow/list-followed state exists yet (see §6) |

### 2.1 "Your space" is new core work (accepted scope)

There is **no `isPersonal`/`ownSpace` field today**; the only "own space" signal is the implicit `relationship == organizer`. A distinct personal tier is therefore net-new core work — it trips the UniFFI gate (binding regen + native staticlib rebuild) **and** the durable registry format (§6.3). The maintainer has chosen to keep it in v1 (§11 Decision B). To bound the risk, v1 "your space" is a **pinned personal home** — your profile, your drafts, your posts, and app/account settings — and nothing more (no personal CMS, no cross-space aggregation).

### 2.2 A site you OWN vs a site you FOLLOW (security-load-bearing)

A composite site **you own** (masthead root secret on this device — the exact object the seizure threat §9.3 of the composite design warns about) is **not** the same as a site you follow. An owned masthead lives under **Communities** (you are its organizer/owner — read+write, and the root secret is keystore-backed), NOT under Following (which is read-mostly, untrusted content). This distinction drives the seizure-disclosure placement in §4.4.

## 3. The anchor: two-pane, adaptive

A **two-pane layout** — a tiered space list on the left, the selected space's detail on the right — is the anchor.

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
│ ＋ Join  ⌕Follow                                       │
└──────────────┴───────────────────────────────────────┘
```

- **macOS / iPad:** both panes visible. **Verified reuse:** `ConferenceShellView.swift:640` `macShell` is already a `NavigationSplitView` with a `List(RiotDestination.phoneTabs, selection:)` sidebar + detail — the space list replaces the sidebar contents.
- **Phone:** collapses to a drill-in — the space list is the root; tapping a space pushes its detail; back returns to the list.
- **Launch behaviour (required):** on phone, launch **restores the last-active space's detail** (not the bare list), so a single-community user pays no extra tap. Reuse the existing decision primitive `CommunityReturnOutcome.decide(active:all:)` (`CommunityChooser.swift:165`), extended to the new tiers. Desktop default selection = last-active space, else the top row of Your space.

### 3.1 Left-list rows — one honest state vocabulary

Each row is grouped by tier and carries a **single defined state vocabulary**, icon **and** text (never colour alone), covering: `available` · `syncing` · `pendingFirstSync` · `quarantined` · `degraded/transport-blocked`. A `require:arti` followed site shows "requires Tor — unavailable" **at the row** (fail-closed honesty without drilling in — Security S1). A `quarantined` space stays visible and selectable; its detail shows the recovery affordance (never vanished — the "accountable degradation" convention extends to spaces, not just moderated items).

**Core-vs-shell honesty note:** most row state is core-provided (`CommunityRow`: `relationship`, `recent_activity`, `sync_freshness`, `quarantined`, `available` — `mobile_api.rs:32-51`), but `pendingFirstSync` is currently **UI-derived in Swift** (`CommunityChooser.swift:70,114`). The implementation plan must decide whether to promote it to a core field or keep it a documented shell-derived display state; either way it is display-only, never a policy input.

### 3.2 Scale, search, ordering

Following is expected to grow. The list specifies: **collapsible tier groups**, a **search/filter** field pinned above the list, and a default ordering (activity-desc within tier, with unread surfaced). The Join / Follow affordances are **pinned** (do not scroll away on a long Following list).

### 3.3 Accessibility (the list IS the nav — load-bearing)

Because the list replaces the entire tab bar, it must be fully navigable non-visually: VoiceOver/TalkBack rows announce **tier + name + state** ("Richmond, community, 3 new, syncing"); all state indicators are icon+text (colour-independent, colourblind/grayscale safe); Dynamic Type / large-text reflow; defined focus and selection order. This is in scope for v1, not deferred.

## 4. The detail: fits the kind of space

The right pane adapts to what the space is. A **shared chrome contract** across all three kinds keeps it one app: a header with `title + tier badge + primary relationship control` (Follow/Unfollow · Leave/Archive · Settings) in a consistent position.

- **Your space** → your posts, profile, drafts, and app/account settings (§2.1 bound).
- **Community you belong to** → today's routes, now *inside the selected space*: **Home** (feed) · **People** · **Nearby** · **Tools**. Tools is a screen here, not a top-level tab. `CommunityShellView` (`ConferenceShellView.swift:499`) is already parameterized by `community: CommunityContext`, so relocating these under a selected-space detail is mechanical. **Leaving** a community uses the existing `archive_community` / `restore_community` (`mobile_api.rs:383/388`), surfaced from the detail header and/or a row swipe.
- **Followed indymedia site** → the **Unit 4 composite render**: **Editorial** (front page) · **Comments** · **Wire**, + **Follow/Unfollow**. Renders `ResolvedCompositeSite` / `SiteTrustTier` / `SiteItemTreatment` / `SiteDegradation` (**PR #46 — unmerged; see §6.1 prerequisite**). Moderated items → accountable placeholders (never vanish); degradation states (moderation-loading / editorial-only / transport-blocked / manifest alarms) rendered per `SiteDegradation`.

### 4.1 Trust-tier styling — concrete, non-spoofable (SECURITY-UI)

Editorial vs open-wire vs comment must be **unambiguous, non-spoofable, and context-independent**:
- The tier is core-resolved (`SiteTrustTier`); the **shell paints trust chrome AROUND content** and never lets content supply its own tier styling (an open-wire item cannot render itself to look editorial).
- The treatment combines **icon + shape + label**, not colour alone — legible in grayscale and under colourblindness, and it survives a **screenshot/reshare out of context** (a bare colour or a lone text word does not; the composed badge does).
- Tiers are visually distinct at the row/section level too (Security S3): Following (untrusted content) is never mistakable for a space you own or organize.

### 4.2 Add / Join / Follow — three distinct actions, not one button

The single "＋ Add/follow" is split by intent (Security B1 + Designer #5 + PM #4):
- **Join a community** → existing `join_public_space` / `join_newswire_community`.
- **Follow a site** → paste/scan a **ticket/QR** (new follow FFI, §6.2). v1 has **no discovery directory** (conscious non-goal): add = paste or scan a ticket only.
- **Create** → §4.3/§4.4.

### 4.3 Create a community vs 4.4 Mint a masthead — and the seizure gate

- **Create a community** (communal space) → existing organizer-author creation.
- **Mint a masthead** (own a composite site — root secret lands on this device) → this is the high-stakes flow. The **mandatory seizure disclosure** (§9.3 of the composite design: "device seizure = full site takeover — the captor can impersonate the site and revoke the real editors") fires **here, blocking, at mint time** — NOT on follow, NOT on join. An owned masthead then appears under **Communities** (§2.2). This pins the disclosure that the prior spec left as vague "somewhere in a detail/creation flow."

## 5. What this replaces / reuses (verified against HEAD)

- **Reuses (verified):** macOS `NavigationSplitView` (`ConferenceShellView.swift:640`); `CommunityShellView` community-parameterized detail (`:499`); `CommunityRow`/`CommunityChooserRow` state; the community list `List` (`CommunityChooser.swift:216`); return-decision primitive (`:165`); leave/archive (`mobile_api.rs:383/388`).
- **Depends on (unmerged):** Unit 4 composite render FFI (`ResolvedCompositeSite` etc.) — **only on `feat/composite-unit4-ffi` / PR #46**, absent from main. Hard prerequisite for the followed-site detail (§6.1).
- **Rewrites:** `RiotDestination` (top-level routes → the space list is root; Home/People/Nearby/Tools become *per-community* sub-routes); the modal `CommunityChooser` → persistent left pane; `AppModel`/`ConferenceShellView` navigation.
- **Removes from top level:** the Tools tab (relocated to per-community detail).

## 6. Core / FFI work (shared Rust; the real cost, not "just additive")

### 6.1 Prerequisite: Unit 4 render FFI (PR #46) must land first
The followed-site detail renders types that exist only on PR #46. **Sequencing gate:** the followed-site-detail work unit cannot start until #46 is on main.

### 6.2 Followed-site state is new, not a field-add
Followed composite sites have **no registry presence today**: `community_row()` (`mobile_state.rs:2132`) builds from an author-bearing `CommunityRecord` (`available` = "author loadable"), while a followed site (owned root, resolved by the stateless `resolve_site_manifest` / Unit 4 resolver) has **no author and no newswire descriptor**. There is **no follow/unfollow FFI and no list-followed FFI at all**. Required, as an explicit dependency chain: `follow_site(ticket)` / `unfollow_site` / `list_followed_sites()` state, feeding the left pane. **Decision for the plan:** a parallel `list_followed_sites()` + distinct row type, **vs** a genuine tagged union over `CommunityRow` — do NOT shoehorn a followed site into the author-derived `CommunityRow`.

### 6.3 Durable registry format
The relationship enum is **durably wire-encoded**: `community_registry.rs` `Relationship` `to_wire`/`from_wire` under `REGISTRY_VERSION = 1` / `RECORD_FIELDS = 9`. A new `Following` variant and/or a personal-space flag (§2.1) is a **persisted-format version bump / migration** decision, on top of the UniFFI regen. The plan must make the version/migration call explicit.

### 6.4 Personal-space contents + exposure boundary (Security S2)
"Your space" surfaces personal contents (profile, drafts). Its view model **must never surface root/owner secrets** (keystore-backed, composite §8 Unit 0) and must not co-mingle/aggregate personal material with untrusted Following data. State the exposure boundary in the FFI.

### 6.5 Shell picks detail by core-assigned tier only
Which tier a space is, and every trust/treatment/degradation decision, is **core-resolved**; the shell only routes to a detail surface by the core-assigned tier. No policy logic in Swift/Kotlin.

## 7. Relationship to Unit 6 (maintainer decision: reshape Unit 6 around this)

**This design is the spine of Unit 6** (§11 Decision A — maintainer chose to reshape Unit 6 rather than ship the render on the current shell first). The gate-passed Unit 6 plan is re-planned spaces-first. To contain the big-bang risk the CTO/PM flagged, the reshape is executed as an **increment ladder**, each rung independently landable + testable behind the existing Shell/Tab navigation test suites:

1. **Core** — `following` + personal relationship on the list surface (§6.2/6.3), UniFFI-gated, one commit incl. the durable-format decision.
2. **Shell skeleton** — two-pane space list as root with the existing community detail routes carried over **verbatim** (relocate, don't redesign); launch-restore; a11y; row state vocabulary. Tools relocated to per-community detail.
3. **Followed-site detail** — wire the Unit 4 `ResolvedCompositeSite` render (gated on §6.1 / PR #46) + trust-tier chrome (§4.1).
4. **"Your space" tier** — the personal home detail (§2.1), last, so the net-new concept doesn't block the rest.
5. **Unit 6 obligations** — editor-invite handshake, QR gen + camera scan, writer expired-cap warning, mandatory seizure disclosure (§4.4), compose-time `require:arti` notice — hosted in the appropriate space/creation flow.

## 8. Non-goals (v1)

- No change to the composite-site core/protocol (Units 0–5 stand).
- No tools/apps runtime redesign — Tools is *relocated*, not redesigned.
- No cross-space aggregate feed (unified "all my spaces" timeline) — per-space entry only. Future slice.
- No community-internal route redesign (Home/People/Nearby) beyond relocating them.
- **No site-discovery directory** — add = paste/scan a ticket (conscious v1 scope, not an omission).

## 9. Missing-flow coverage (added per gate)

- **Empty / first-run:** a brand-new user has an (optionally empty) Your space and no communities/follows. The root shows an empty state with copy + a primary "Join a community / Follow a site" call to action; the detail pane shows a neutral welcome, never a blank or a fabricated feed.
- **Leave / archive** a community (§4) and **unfollow** a site (§4.2) are both wired from the new list + detail.
- **Launch restore** (§3) and **quarantined-space recovery** (§3.1).

## 10. Risks

1. **3-platform shell rewrite** touching landed code (`AppModel`, `ConferenceShellView`, `CommunityShell`, `CommunityChooser` × iOS/Android/macOS + both `project.pbxproj`). Mitigation: the §7 increment ladder (each rung landable + bisectable); reuse the split view; relocate-don't-redesign; land behind existing nav test suites (rewritten to the new IA).
2. **pbxproj serialization hazard** — new Swift files need entries in both iOS + macOS `project.pbxproj`; coordinate (shared-checkout rule); temp-index technique or a COLLABORATION note if a sibling holds the file.
3. **UniFFI + durable-format gate** — §6 changes need binding regen + native staticlib rebuild **and** a registry version/migration, in coordinated commits.
4. **"Your space" scope creep** — bounded to a pinned personal home (§2.1); resist a personal CMS in v1.
5. **Composite value ships behind the rewrite** (CTO/PM concern; accepted per §11 Decision A) — mitigated by ordering the increment ladder so the followed-site detail (rung 3) lands as early as its #46 prerequisite allows.

## 11. Decision record

- **Decision A — Sequencing (2026-07-18):** the gate (CTO + PM) recommended shipping Unit 6's nav-agnostic composite render on the current shell first, then the IA as its own unit. **The maintainer chose to reshape Unit 6 around spaces-first** (one coherent unit). Risk accepted; mitigated by the §7 increment ladder.
- **Decision B — "Your space" tier (2026-07-18):** the gate recommended cutting or hard-constraining the personal tier. **The maintainer chose to keep it in v1**, bounded to a pinned personal home (§2.1). Net-new core cost (UniFFI + durable format) accepted.
- **Decision C — Reversing Tools-in-tab (#44):** the "open tools inside the Tools tab" work merged 2026-07-18 (#44) is intentionally superseded — Tools leaves the top level and becomes a per-community detail. This is a deliberate IA turn (composite-follow multiplies the multi-space need), recorded here so the same-day reversal is a decision, not thrash.
