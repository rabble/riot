# Spaces-First — Rung 4: "Your space" personal home — Implementation Plan (DRAFT — blocked on a reconciliation decision)

**Status:** DRAFT, overnight 2026-07-18. **BLOCKED on a user decision** (below) — not gate-ready. Documented per the overnight guardrail "if a task needs a decision only the user can make, log it and move on."

**Goal (spaces-first spec §2.1, bounded):** surface the `Personal` tier in the two-pane shell — a pinned personal home (profile, drafts, your posts, settings) — and let the user enter it.

---

## 🚧 BLOCKING RECONCILIATION — decision for the user

There is a **prior gate-passed design `docs/superpowers/specs/2026-07-12-personal-spaces-and-pages-design.md`** (+ `plans/2026-07-12-personal-spaces-slice1.md`) that already defines a **"personal space"**: an *owned* Willow namespace where one person holds root and publishes a **page** (HTML/CSS bundle, GeoCities/MySpace style), public or connections-only, with a stranger-facing calling card. It is a **substantial feature** with **hard prerequisites**: the multi-space SQLite store design, full-meadowcap-management, and signed-JS-apps/app-directory. Code exists (`crates/riot-core/src/willow/owned.rs` `OwnedRoot`; `identity.rs` personal-space variant) but **Slice 1 looks unfinished** (unchecked steps; depends on the multi-space store).

**The spaces-first `Personal` tier and this personal-spaces feature are the same object seen at two scopes.** Before Rung 4 can be planned/executed:

1. **Is spaces-first "your space" == the personal-spaces-and-pages feature?** (My working assumption: YES — "your space" is the *navigation entry* to the personal space; the *contents* (page, profile, drafts) are the personal-spaces feature.) Confirm or correct.
2. **How much of personal-spaces is actually built?** (owned.rs primitives exist; the page/store/render slices appear incomplete.) Rung 4's scope depends entirely on this — if personal-spaces is unbuilt, "your space" is a large feature, not a bounded tier.
3. **Bounded vs full:** the spaces-first spec §11 Decision B bounded "your space" to a pinned home (no personal CMS). The personal-spaces design is the full CMS (pages, HTML editor, AI-assisted authoring). **These conflict.** Which wins? (Doc-wins rule says the more specific personal-spaces design likely governs the CONTENTS; the spaces-first bound governs what the v1 NAV surfaces.)

**Until (1)–(3) are answered, executing Rung 4 risks either reinventing personal-spaces or contradicting its gate-passed design.** Skipped for autonomous execution; the skeleton below is the navigation-only part that holds regardless.

---

## Rung 4 skeleton (navigation-only — holds under either reconciliation)

Regardless of the contents decision, the two-pane shell needs the `Personal` tier surfaced. This part is safe to plan:

- **4.1 — Personal-tier assignment (core, pure-Rust, cargo-verifiable).** Rung 1 added the `Personal` relationship as a tag but nothing ASSIGNS it. Decide which space is the personal home: the profile's own organizer namespace, OR (if personal-spaces is built) the owned personal namespace from `owned.rs`. A core method `mark_personal_space(root)` / or derive it, so exactly one row is `Personal`. RED: exactly one space resolves as `Personal`; it appears in the Your-space tier, pinned first. **NOTE:** which namespace is "personal" depends on reconciliation (2).
- **4.2 — iOS/macOS `YourSpaceDetailView`** replacing Rung 2's `YourSpaceDetailPlaceholder`. **Bounded (spec §11 B):** shows the person's profile/identity, their drafts, their posts, and app/account settings — links OUT to the personal-spaces page feature if/when built, rather than embedding a CMS. New Swift file → BOTH pbxproj. Planning-only overnight.
- **4.3 — §6.4 exposure boundary (core).** The personal-home view model must NEVER surface the owned-root/owner secret (keystore-backed per personal-spaces design + composite §8 Unit 0). Contract test: the personal view model carries only public ids/content, no key material. Pure-Rust, cargo-verifiable.
- **4.4 — Android** mirror skeleton.

## Deferred / dependent
- The actual personal PAGE (HTML/CSS bundle, editor, public/private read gate) is the **personal-spaces-and-pages feature**, not this rung — Rung 4 links to it, does not rebuild it.
- Real personal-space CREATION flow (mint owned namespace) overlaps personal-spaces Slice 1 + the seizure disclosure (Rung 5 §4.4).

## Self-review
- §2.1 bounded personal home tier → 4.1 (assignment) + 4.2 (bounded detail). ◐ blocked on reconciliation.
- §6.4 exposure boundary → 4.3. ✓ (speccable now)
- Reconciliation with prior personal-spaces design → this whole doc; **the decision is the gate.**
