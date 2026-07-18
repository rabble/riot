# Riot Compact Core Flow Design

**Date:** 2026-07-19  
**Status:** Design review candidate  
**Scope:** iOS and shared macOS SwiftUI surfaces only

## Problem

Riot’s core loop is present but not reliably usable. First-run Nearby changes an
invisible route, the empty-wire post action has no handler, newswire rows omit
the report body, and a successful composer cannot start another post. The Home
screen also gives equal visual weight to tools, two content systems, and a
permanent composer, obscuring the primary job: understand what is happening and
contribute an update.

The work must improve comprehension without weakening Riot’s guarantees:
signatures prove authorship rather than truth, identities remain key-tagged,
editorial and open-wire content remain visibly distinct, and local success is
never described as global delivery.

## Users and outcomes

1. **A first-time contributor** wants to join or create a community so that they
   can reach a useful Home screen without guessing which of five equal actions
   matters.
2. **A reader** wants to open a report so that they can read its headline, body,
   byline, provenance, and treatment.
3. **A contributor** wants to post repeatedly so that one successful local write
   does not strand the composer.
4. **A nearby participant** wants to understand the current connection step so
   that they can review and accept a concrete number of updates without seeing
   renderer diagnostics.
5. **An organizer or member** wants to scan Tools and People so that trust detail
   remains available without dominating directory browsing.

## Chosen approach

Use the existing community-first shell and models, adding presentation state and
small typed view-model helpers. Do not introduce a new navigation framework,
database shape, FFI contract, dependency, or policy layer.

Rejected alternatives:

- A shell rewrite: unnecessary and likely to regress retained tab state,
  community switching, and macOS keyboard behavior.
- A new unified Rust feed type: the immediate confusion is native composition,
  and alerts/newswire already have distinct verified models.
- Removing trust detail: compactness must move detail behind explicit review or
  disclosure, not erase it.

## Target flow

```text
Welcome
  → Join a community (QR/link)
  → Create one
  → Find nearby
  → Try demo

Community Home
  → visible community chooser
  → Post update
  → chronological/editorial newswire
  → active alerts only
  → compact tool shortcuts

Post update
  → compose
  → immutable review
  → local signed commit
  → “Saved on this device — sync to share”
  → Done | Post another

Read update
  → headline + body
  → author with key-derived tag
  → correction/verification/AI disclosure
  → Reply or editorial action where authorized

Nearby
  → Looking
  → device selected
  → connected
  → “Add N updates”
  → synced result
```

## Screen design

### First run

Welcome copy becomes three short promises: read local updates, publish signed
reports, and exchange nearby without internet. “Works without a server” replaces
the absolute “No servers” claim.

Setup has one filled action at a time. “Join a community” is primary; create,
Nearby, and demo remain available but secondary. A non-empty display-name draft
is committed automatically before create, join, or Nearby. A dedicated Save
button is removed.

First-run Nearby owns a temporary `NearbyTransportController` and presents the
existing Nearby surface with the open profile as host. A successful adoption
refreshes the store; launch state then naturally changes from no-community to
community. Cancel returns to setup.

### Home and shell

The phone header always shows `Community name ▾`, plus profile and settings.
Long names truncate rather than displacing 44-point controls.

Home begins with one `Post update` action and the newswire. The composer is a
sheet backed by the existing retained `PostUpdateViewModel`, so drafts still
survive route changes. Empty or active alerts do not create a second empty feed:
the Alerts card renders only when scoped alerts exist. Tool shortcuts move below
content.

Front page and Open wire remain separate, explicit sections. This work does not
blend them or introduce ranking.

### Reading and posting

`NewswirePostRow` carries the core-projected body in addition to the headline.
Ordinary rows expose `Read update`; a detail sheet renders the complete report
and existing trust annotations. Hidden and tombstoned rows never reveal payload
content and remain accountable placeholders.

After a successful local post, the composer says it is saved locally and will
spread through exchange. `Done` dismisses the sheet. `Post another` clears all
draft fields, error state, and posted state, then returns to editing. It never
reuses an operational expiry or AI-assistance choice accidentally.

Notification permission is not requested merely because a community opened.
The first successful post is the contextual request point. Denial does not block
posting or local use.

### Tools, People, Nearby

Tools cards show name, purpose, trust/status badges, and one availability action.
Permissions and recommendation/share controls move under `More details` or the
existing review sheet. The terms “Tools” and “tool” are used consistently in
directory copy.

People uses Riot typography and chrome rather than an unrelated system-large
navigation title. Empty `Post the first update` opens the composer directly.
Full identifiers remain behind Technical details.

Nearby retains automatic discovery because the current demo and transport
contracts expect it. The screen removes `Renderer: incident-board/1`, shortens
the repeated transport explanation, uses “Nearby devices” and “Recently synced,”
and labels acceptance with the offered count: `Add N updates`. Permission,
failure, different-community, and treatment states remain explicit.

## State and error handling

- No raw persistence, transport, or hostile payload error reaches display copy.
- First-run Nearby can be cancelled without creating or mutating a community.
- A failed display-name save does not falsely claim success; existing fixed error
  handling remains visible and community creation does not proceed under a
  different claimed name.
- A post detail sheet never renders a hidden/tombstoned body.
- Composer dismissal persists a non-empty draft; successful posts clear persisted
  drafts.
- Permission denial leaves read, post, and file exchange available.
- Notification authorization is optional and requested at most once through the
  existing notifier.

## Accessibility

- Every new action has a stable accessibility identifier and a minimum 44×44
  target.
- Dynamic Type may wrap copy; community names truncate to preserve controls.
- Status is expressed in text, never color alone.
- Sheets have explicit Done/Cancel actions and restore focus to their trigger.
- Full IDs remain selectable under Technical details and are never truncated.
- Reduced-motion behavior and existing Riot theme tokens remain unchanged.

## TDD contract

1. A shell/onboarding test proves first-run Nearby presents a real surface
   instead of only mutating an invisible route.
2. A Home contract test proves every empty-wire and People post action opens the
   retained composer.
3. Newswire tests prove ordinary rows carry bodies while treated rows do not
   reveal them, and detail presentation copy remains author/key-tagged.
4. Composer tests prove posted → post-another resets every field and returns to
   editing, while Done does not create another write.
5. Alerts tests prove an empty scoped alert list requests no Home card.
6. Directory/People/Nearby pure presentation tests pin compact labels, disclosure
   placement, and `Add N updates`.
7. Focused suites run RED then GREEN; shared macOS tests, iOS simulator build,
   project-file lint, and the repository coverage gate run before completion.

## Definition of done

- All five functional dead ends from the audit are fixed.
- Home’s primary action and readable feed appear before secondary tools.
- Empty alerts and the permanent embedded composer no longer consume Home.
- Current community is visible on every phone route.
- Tools, People, and Nearby remove the audited redundant/technical clutter while
  preserving recovery and trust detail.
- Existing four-tab order, editorial/open-wire separation, per-community draft
  safety, identity tags, and local-first wording remain intact.
- No new dependency, schema, FFI surface, or core policy is introduced.
- Android is explicitly reported as not audited or changed by this iOS/macOS
  slice.
