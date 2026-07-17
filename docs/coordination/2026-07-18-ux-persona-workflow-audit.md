# Riot — UX / Persona Workflow Audit (2026-07-18)

First audit of the product from a **user-value** angle rather than a protocol angle. Everything
to date has been plumbing-led (Willow, signing, sync, the `/2` export). This maps **who the
users are → what they're trying to do → the screen/page that serves it today → the gap.** It
drives two things: what to actually test in TestFlight, and the remaining roadmap ranked by user
value instead of by subsystem.

Context: Riot is indymedia-lineage activist publishing. **Open newswire** (communal wire +
editorial desk) is built; **private groups (MLS)** are deferred. "web = reach, app = truth."

## What exists today (ground truth)

**App screens (iOS):** `CommunityShell` (the tab shell), `CommunityChooser` (create / switch /
join-by-link), `PostUpdateView` (compose a post), `NewswireEditorial` (editorial actions),
`PeopleView` + `PeerProfileView` (contributors), `RadarPairingView` (nearby pairing),
`DirectoryView` (app directory), `ConferenceShellView`, `AppRuntimeView` (app-drop runtime).
**Editorial actions in core:** Feature, Verify, Correct, Hide, Tombstone, Retract.
**Web pages:** home (`render_newswire`), per-post (`render_post`), per-author (`render_author`),
`/publish/`, `/about/` — plus the launchable signed app-drop. (Unifying onto `/2` now.)

---

## Personas & their workflows

### P1 — The Reader (public web, no app)
**Wants:** read the newswire; judge whether a report is trustworthy; follow a story/author.
- **Has today:** navigable web site (home, per-post permalinks, per-author pages, about); signed
  content with a "signed by the collective" vs "open · unverified" distinction; display-name
  bylines (once `/2` unification lands).
- **Gaps:**
  - **Trust legibility** — the signed/unverified badge exists, but does a lay reader understand
    what it means? No "why you can trust this / what Riot is" moment beyond `/about/`.
  - **"Open in Riot" verify loop is incomplete** — the deep link exists (`riot://open?namespace=`,
    and per-post `&entry=` after unification) but there's no app-side landing that says "this exact
    post's signature checks out." The web can't *prove* itself to a reader without this.
  - **Discovery** — how does a reader find a community's site in the first place? No index/discovery.

### P2 — The Contributor (app; posts to the open wire)
**Wants:** post an update/report and have it seen; establish a recognizable byline.
- **Has today:** `PostUpdateView` to compose; posts land on the wire; nearby + (soon) web reach.
- **Gaps:**
  - **Identity/display-name UI** — display names are signed profile cards (`set_display_name`
    exists in FFI) but **is there a screen to set your own display name / desk name?** If not, every
    contributor is a hex key until an organizer names them. This is load-bearing for the byline
    decision. **VERIFY whether a profile/identity screen exists.**
  - **Post feedback** — after posting, is there confirmation it synced / who can see it? The
    pending-first-sync state exists for joins; is there a "your post is live/pending" signal?
  - **AI-assisted disclosure** — `ai_assisted` is a field; is it a compose-time toggle the
    contributor controls?

### P3 — The Editorial collective / organizer (app; runs the desk)
**Wants:** start a community/masthead; curate (feature the lede, verify a report); moderate
(hide/correct/retract a bad post); manage the roster.
- **Has today:** community creation (`CommunityChooser`), `NewswireEditorial` for actions,
  `PeopleView` for contributors. Core supports all six actions.
- **Gaps:**
  - **Which actions are actually exposed?** Core has Feature/Verify/Correct/Hide/Tombstone/Retract.
    **VERIFY `NewswireEditorial` surfaces all six** — Correct (issue a correction) and Retract are
    high-value editorial verbs; if only Feature/Verify/Hide are wired, Correct/Retract are a gap.
  - **Roster management UI** — adding/removing editors (the `editorial_roster`): is there a screen,
    or is the roster fixed at creation?
  - **Moderation legibility** — when a post is Hidden/Tombstoned, what does the collective (and the
    reader) see? Is there an audit trail view of editorial actions?

### P4 — The Follower (app; joins another community by link)
**Wants:** follow a community they trust; read its wire; get its updates.
- **Has today:** join-by-link (`CommunityChooser` paste sheet, recovered in v2); the joined
  community's Home reprojects after first sync (Risk 15/16); multi-community switching.
- **Gaps:**
  - **Getting the link** — sharing a community produces a share-ref; is there a clean "invite /
    share this community" flow, and how does a follower receive it (QR? link? out-of-band)?
  - **Notifications** — does a follower learn a new post landed, or must they open + sync manually?
  - **Multi-community mental model** — per-community unlinkable identities is a strong privacy
    property; is it *legible* (does the user understand they're a different identity in each)?

### P5 — The Nearby peer / on-the-ground (app, offline)
**Wants:** at a protest with no/blocked internet, sync posts + alerts peer-to-peer.
- **Has today:** `RadarPairingView` nearby pairing; two-peer sync fixed (7242a0d); publishing
  traverses the nearby bridge (Risk 16).
- **Gaps:**
  - **Sync feedback** — is "I synced with N peers, got M new posts" visible and reassuring in a
    high-stress field context?
  - **Physical two-device validation** — NEVER tested on two real phones (no rig). This is the #1
    thing TF v2 unblocks. Until then nearby-sync-works is an on-Mac-Bonjour assumption.

### P6 — Private group member (deferred)
MLS + encrypted drops. Not built. Out of scope until the open newswire is validated.

---

## Cross-cutting UX gaps (not persona-specific)

1. **First-run / onboarding** — no named onboarding flow. How does a brand-new user go from
   install → identity → first community → first post? This is the single biggest TestFlight risk:
   a tester who can't get past first-run tests nothing.
2. **The verify loop ("Open in Riot")** — the strategic differentiator (web reach + app truth) is
   not closed. Groundwork is in (namespace+entry deep links); the app-side "this signature checks
   out" landing is the missing piece. **This is the highest-value remaining UX feature.**
3. **Identity/display-name management** — likely the highest-value small gap (see P2). Verify it.
4. **Trust legibility everywhere** — signed vs open, verified vs unverified, who-signed — is it
   consistently and understandably surfaced in both app and web?
5. **Discovery / sharing** — how communities are found and invitations flow.

---

## Ranked remaining work (user-value order)

1. **TestFlight v2 build + a first-run test script** (below) — validate the built workflows on
   device before building more. *In flight (native core rebuilding).*
2. **`/2` web unification** — one canonical rigorous web newswire. *In flight.*
3. **"Open in Riot" verify loop** — close web→app signature verification. The differentiator.
4. **Identity/display-name screen** (if absent) — small, unblocks real bylines for P2.
5. **Onboarding / first-run flow** — if first-run is rough, this outranks most things.
6. **Editorial completeness** — expose Correct/Retract; roster management; moderation audit view.
7. **Follower quality-of-life** — invite/share flow, new-post notification.
8. **Nearby field feedback** + the physical two-phone test.
9. Deploy the unified `/2` site to the CF mirror.

## TestFlight v2 — what to actually test (per persona)

- **P3 organizer:** install → create a community → post → Feature one, Verify another, Hide a
  third → confirm the Home reflects each. *(Does first-run work? Are all editorial actions there?)*
- **P2 contributor:** set a display name (if the screen exists) → post → see it on the wire.
- **P4 follower:** on a 2nd device/account, join by the shared link → confirm the joined Home
  shows the organizer's posts after sync.
- **P5 nearby:** two phones, airplane-mode + local — pair via Radar → confirm a post crosses.
- **P1 reader:** open the web mirror → read a post → tap "Open in Riot" → *(does anything happen?
  — this is the gap to confirm.)*

## Verify-me items — CHECKED (2026-07-18)

- **(a) Display-name handling EXISTS** — wired across `AppModel`, `ProfileRepository`, `PeopleView`,
  `ConferenceShellView`, `RadarPairingView`. So contributors aren't stuck as hex keys. Open
  question is *prominence/discoverability*, not existence — confirm there's an obvious "set your
  display/desk name" entry point, not just a buried field. Downgraded from gap to **present,
  verify prominence.**
- **(b) Editorial is nearly complete** — `NewswireEditorial` surfaces **Feature, Verify, Correct,
  Hide, Retract** (5 of 6). Correct and Retract ARE there (better than assumed). **Only Tombstone
  is unwired** — likely intentional (Tombstone is the heavy permanent-removal verb); confirm it's a
  deliberate omission, not a gap. Editorial completeness drops down the priority list accordingly.
- **(c) No onboarding / first-run flow** — CONFIRMED GAP. No onboarding-named view; first-run drops
  straight into the shell / community chooser. This stays the **#1 TestFlight risk** — a new tester
  with no guided path may not reach a first successful post. Highest-value UX add after the verify
  loop.

Net after checking: the app is **more built-out than a plumbing-first read suggests** (identity +
5/6 editorial actions already there). The two real UX gaps are **onboarding/first-run** and the
**"Open in Riot" verify loop**; everything else is polish/legibility.
