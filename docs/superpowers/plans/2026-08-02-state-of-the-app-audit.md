# State of the app: what is built, what is reachable, what is missing

**Date:** 2026-08-02
**Companion to:** `2026-08-02-usable-by-normal-people-plan.md`

## How to read this

Riot's recurring failure mode is not missing code. It is **code that exists and
cannot be reached** — a capability lands in `riot-core`, gets an FFI wrapper, and
never gets a screen; or it gets a screen that is exercised only by a test
substituting the part that breaks. So each item below is graded on three
separate axes, because "done" has meant three different things:

- **Core** — the logic exists in `riot-core` / `riot-ffi`.
- **Reachable** — a person can actually get to it in the app.
- **Proven** — a test walks it on the REAL substrate (durable database, real
  keychain, real sync boundary), not a double.

Evidence level is noted per row. Where I have not verified something in this
pass it says so rather than guessing.

## 1. Built, reachable, and proven

| capability | notes |
|---|---|
| create a community | durable journey test, 2026-08-01 |
| post an update | durable journey test |
| reply to a post | durable journey test; also reader-side, in-memory and durable |
| react to a post | durable journey test; toggle + retract covered |
| project the wire | front page / open wire / comments / reaction tallies |
| join by share reference | covered in FFI contract tests |

**Caveat that undercuts this whole table:** the durable journeys use a *stable
test-double wrapping key*. On a real device the identity does not survive a
relaunch, so these pass while the same actions fail for a person. Proven means
proven against the harness, not against the product.

## 2. Built in the core, NOT reachable by a person

This is the largest and least visible category — work that is done and earns
nothing until it has a surface.

| capability | evidence |
|---|---|
| editorial actions: Feature, Verify, Correct, Hide, **Tombstone, Retract** | `NewswireEditorialActionKind` in `newswire_ffi.rs:103` |
| follow another site | `follow_site` / `list_followed_sites` in `mobile_api.rs:558` |
| composite sites (owned masthead + communal wire) | design gate-passed 2026-07-15; `/mod/` moderation logic done and tested |
| owned-site articles / write path | deferred to "Rung 5"; render surface orphaned on both platforms |
| meadowcap capabilities (read caps, owned namespaces, delegation) | present in the pinned `willow25`; Riot does not use them |
| personal spaces / pages | design gate-passed 2026-07-12 |
| anchor relay control plane | landed #90 |

**"Retract" matters most here.** Updating and correcting the record is in the
one-sentence goal for this app, the core supports it, and there is no way to do
it. It is also editor-gated rather than author-gated — worth deciding whether an
author should be able to correct their own post.

## 3. Genuinely unbuilt

| capability | status |
|---|---|
| **internet sync between phones** | issue #107. `riot-ffi` carries no iroh; mobile is BLE + local network only. The anchor relay is a one-way pull, not a path between two people. |
| encrypted private groups (MLS) | deliberately last; p2panda abandoning MLS |
| destructive editing / true deletion | not possible once others hold a copy; needs an honest story, not a feature |
| archiving + restoring communities in the UI | supported in core, no surface |
| app-to-app distribution (Android) | designed 2026-07-29, not implemented |
| Android universal APK (armeabi-v7a) | 32-bit phones cannot install today |

## 4. Broken

| defect | severity |
|---|---|
| **identity does not survive a relaunch on a real device** | fatal — permanent data-loss loop, proven on a real machine 2026-08-01 |
| writes into a joined community never verified end to end | unknown until WU-2 |
| "Not synced yet" vs "Synced just now" on one screen | confusing |
| stale "Reactions aren't available" after a reaction succeeds | confusing |
| Android Play release blocked | issue #151 (`lintVitalReportRelease`) |
| 6 pre-existing red Swift tests on main | AnchorProtocolVector, CompactReactionBar, NewswireSurface, ShellNavigation |

## 5. Distribution

- **iOS:** TestFlight only. App Store package builds; the release kit reports
  `BLOCKED` — twelve `policy.*` gates plus account and legal items.
- **macOS:** notarized DMG on GitHub (works today); App Store `.pkg` builds.
- **Android:** 46MB sideloaded APK, arm64 + x86_64 only. No Play listing.

## The road to "a normal person can use this"

Four phases. Each ends in something demonstrable, not just merged.

### Phase A — the loop closes on one device

The relaunch column becomes true. Identity survives, existing broken profiles
recover, and one full row is walked by hand on real iOS and Android hardware:
install → join → post → reply → react → quit → relaunch → it is all still there.

*Ends when:* a person can use Riot for a week without losing anything.

### Phase B — the loop closes between two people in a room

Nearby exchange proven on two physical devices for every action, not just
posts — replies, reactions, tools. Journeys on the real substrate for each.
The two-phone rehearsal exists; it needs to cover the whole row.

*Ends when:* two people at the same table can hold a conversation.

### Phase C — the loop closes between people who are apart

Issue #107. This is the product. Until it exists Riot is a same-room tool and
the marketing should say so. Design questions are open: how the relay reaches a
phone, how known peers exchange node ids.

*Ends when:* two people in different cities can hold a conversation.

### Phase D — a person can get it and correct it

Surface the built-but-unreachable work, starting with retract/correct, follow-a-
site, and archive/restore. Clear the store gates or publish an honest signed
download page. Ship the universal APK so cheap phones are not excluded.

*Ends when:* someone can install Riot without a laptop and fix their own typo.

## What I would not do yet

- Private groups / MLS. Already last on the list, and correctly so.
- New feature surfaces before Phase A. Every one adds a cell to a grid whose
  first column is still false.
- More 0.1.x releases until Phase A lands. Each one costs existing users their
  community.
