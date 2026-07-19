# Cross-Space Activity Log + Notifications — Design

**Goal:** one place a person sees "what's happened across ALL my communities" — and gets notified —
instead of having to open each community and scan its wire. A unified activity inbox that spans
spaces, plus cross-space unread/notification counts.

## THE load-bearing constraint (read first)
**Per-community identities are deliberately unlinkable.** Each community is a fresh, unlinkable
author; carrying a name/identity across communities breaks the pseudonymity guarantee. Therefore:

> The cross-space activity log is a **LOCAL, device-side aggregation only.** It is a projection the
> device can build *because it holds all of your per-community identities and all synced records*. It
> MUST NEVER create, sign, sync, or publish any record that links your activity in community A to
> community B. No "global activity" record family. No cross-community identity. A peer/organizer of
> community A can never learn that your A-persona and your B-persona are the same device.

Every design choice below preserves this: aggregation happens after each community has already synced
its own records locally; the union is computed on-device and never leaves it.

## What the log aggregates (v1 — existing signed record families, no new protocol)
The audit confirms the only "what happened" surfaces today are the newswire wire + editorial history
(newswire-scoped) and a per-app presence caption (one app, presence not actions). v1 builds a real
timeline by projecting the signed record families that ALREADY exist, per community, into one local
stream:
- newswire **posts** (`create_newswire_post`)
- **comments/replies** (`NewsCommentV1`)
- **reactions** (`NewsReactionV1`, if the reactions branch lands)
- **editorial actions** (feature/verify/correct/hide/tombstone/retract)
- **site moderation** (owned `/mod/` actions)
- shell-level **membership events** (you joined/created a community; an app was trusted/added)

Each item already carries a signer + timestamp within its community. The activity engine maps each to
an `ActivityEntry { space_id, actor_rendered, kind, target, tai_micros, deep_link }` and unions across
all joined communities, newest first.

## What it does NOT cover in v1 (and why)
**"What people did inside the carried apps"** (checklist edits, map pins, etc.) is NOT in v1. Carried
apps today only store local bridge rows (presence), they do not EMIT signed activity records to the
space. A true per-app action log requires apps to write a signed **app-activity record family** (a new
protocol record + a bridge API for apps to emit it) — that is a **v2 extension**, specced separately.
v1 is honest about this: it logs community-level signed activity + app *lifecycle* events (trusted /
added / opened), not in-app actions.

## Identity rendering (the trap)
Each row renders the actor as their **per-community display name within that space** (the same
sanctioned render path the People/wire surfaces use) — never a global identity. Your OWN actions
across spaces are shown to you locally, but rendered per-persona; the UI never implies one identity
spans spaces. Rows are grouped/badged by space so the separation is visible, not collapsed.

## Notifications (extend, don't rebuild)
A notifications engine already exists (per-community unread/what's-new badges). Extend it to compute
**cross-space** unread: the activity engine tracks a per-community last-seen cursor (it already does
for the wire) and the app surfaces a single aggregate "N new across your communities" plus per-space
breakdown. Local notifications (and later APNs/background — a separate large feature) fire from the
same on-device projection; nothing about the notification payload crosses community boundaries.

## Architecture
- **Core / FFI (mostly reuse):** each community already projects its records. Add an FFI
  `project_activity(space)` that returns the community's `ActivityEntry` list (a thin re-projection
  over the existing per-family projections), plus the existing unread cursor. NO new record family in
  v1. The cross-community union is done in the native shell, not core (so no core ever sees two
  communities' identities together — keeps the unlinkability boundary in the shell's local memory).
- **iOS:** a new top-level **Activity** surface, a peer of the community chooser (it is the one
  screen that legitimately spans spaces, because it is local-only). Sections: "All" + per-space filter.
  Each row: space badge · per-space actor · action · time; tap deep-links into that space's relevant
  surface (wire post, comment thread, app). A cross-space unread pill on the app icon / chooser.
- **Android:** parity later.

## Units
1. `project_activity` FFI over existing per-family projections (+ unread cursor) — Rust, testable.
2. Native `ActivityEntry` model + the cross-space union + per-space last-seen tracking (shell-local).
3. iOS **Activity** surface (list, per-space filter, deep-links) + cross-space unread pill.
4. Cross-space notification counts wired into the existing notify engine.

## Out of scope
- **App-action activity (v2):** a signed `app-activity` record family + a bridge emit API so carried
  apps contribute in-app actions to the log. Own spec.
- Background push delivery (APNs / Android background) — large, separate.
- Any cross-community linkage, ever.

## Testing
Unit-test the activity projection (each family → entry), the cross-space union ordering, and the
per-space cursor. A property test: no `ActivityEntry` or notification payload contains more than one
community's identity; the union is only ever assembled in shell-local memory.
