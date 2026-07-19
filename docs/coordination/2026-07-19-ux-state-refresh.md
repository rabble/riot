# Riot — UX State Refresh + TestFlight-v2 Test Script (2026-07-19)

**Supersedes** `docs/coordination/2026-07-18-ux-persona-workflow-audit.md` for the iOS-UX view.
That audit was written before today's iOS-surface build (PR #42/#47) and the `RiotDeepLink`
verify loop landed, so it lists several items as gaps that are now **shipped + tested**. This doc
is verified against `main = ae9ec47` (includes the #59 spaces/following feature and the iOS-surface
PRs). Every "shipped" claim below is backed by a code reference checked on this commit.

> **⚠️ Baseline drift (coordinator note, 2026-07-19):** this worktree is now 15+ commits past
> `ae9ec47` (HEAD `8ce2d84`). The "compact core flow" overnight run reworked the exact surfaces this
> table calls DONE — `a9e851c feat(onboarding): make setup compact and fail closed`,
> `bbfb0a0 feat(home): keep active alerts compact`, `3cc9c4d feat(shell): isolate communities and
> unify posting`. The claims' *substance* still holds (first-run path is still `.noCommunity`/
> `LaunchView`, now at `ConferenceShellView:205` not `:177`) but **line references are stale and the
> layouts changed** — re-verify a row against current HEAD before retiring it from the roadmap.

> Purpose: give the coordinator + owner an accurate roadmap and an actionable TestFlight-v2 test
> script, so no session re-builds a done thing and TF validates the right flows.

---

## What the 2026-07-18 audit listed as gaps that are ACTUALLY DONE

| Audit gap (2026-07-18) | Reality on `ae9ec47` | Evidence |
|---|---|---|
| "Open in Riot" verify loop **incomplete** — *the audit's #1 differentiator* | **DONE, both sides, tested.** Web emits `riot://open?namespace=&entry=` per-post links + QR; app parses → resolves → landing sheet with honest outcomes and anti-forgery | app: `RiotApp.onOpenURL` → `AppModel.handleDeepLink`/`openFromDeepLink` → `RiotDeepLinkResolver.resolveOpen` → `openOutcome` → `.sheet` (`ConferenceShellView:57`); outcomes `RiotOpenOutcome{.verified/.postNotHeld/.notFollowing/.openedHome}`; `riot` scheme in `Info.plist`. web: `apps/gateway/tests/test_newswire.py:135`. tests: `RiotTests/DeepLinkTests.swift` (11 tests incl. `testVerifiesAGenuineSignedPostAndRefusesAForgedEntryID`) |
| Editorial "only Feature/Verify/Hide wired; Correct/Retract/Tombstone a gap" | **All 6 wired** (Feature, Verify, Correct, Hide, Retract, Tombstone) | `NewswireEditorial.swift` — `case tombstone`, "Safety tombstone", closed-field rules cover hide/tombstone/retract (15 tombstone refs) |
| Getting the invite link / share flow (P4) | **DONE** — `ShareCommunitySheet` (riot:// link via `ShareLink` + local QR) in Community settings | PR #42 Unit 2 |
| Join-by-link only a paste sheet (P4) | **DONE** — `JoinByReferenceSheet` (paste **+ camera QR scan**, honest no-title preview, duplicate→switch) | PR #42 Unit 1 |
| Read alerts / "Signed alerts" a dead number (P1/P3) | **DONE** — Home Alerts card → list → `AlertDetailSheet` (per-community, core-verified signer) | PR #42 Unit 3 |
| Editorial gated to session-created communities | **DONE** — un-gated via `newswire_is_editor` FFI predicate (display == admission authority) | PR #42 Unit 4a/4b |
| Add-a-tool absent on iOS (P3) | **DONE** — `.fileImporter` → install → untrusted → existing `AppReviewSheet` (no auto-trust) | PR #42 Unit 5 |
| Only "post an update" (P2) | **DONE** — compose Update/**Alert**/**Request** + operational fields (no dead-disabled Post) | PR #42 Unit 6 |
| Dead-ends: no-op Open-wire button, offlineStale loop, chooser no-ops, Tools empty state | **DONE** — all fixed with reachable next actions | PR #42 Units 1/7 |
| Display-name / identity screen (P2) | **Present** — set at first-run (`LaunchView` "Save name", skippable) and editable in `YourProfileSheet` (avatar→profile). Prominence is fine (first-run + profile). | `ConferenceShellView:273,1359-1383` |
| First-run / onboarding (cross-cutting #1) | **Present but being REDESIGNED — do NOT retire this item.** `LaunchView` is the `.noCommunity` guided path (name-skippable + create + join-by-link/QR + nearby), and `a9e851c` already made setup compact/fail-closed. Meanwhile `design/interaction-frame` (spec `docs/superpowers/specs/2026-07-18-interaction-frame-design.md`, tip `c020277`) is an active redesign of exactly this surface — a first-run resilience hero + in-context teaching. Two sessions own first-run; coordinate, don't declare done. | `ConferenceShellView:205`, `AppModel.launchState` |

**Net: the iOS UX layer is ~complete against the persona audit.** The audit's ranked items #3
(verify loop), #4 (display name), #6 (editorial completeness), #7 (follower share/join) are done.

---

## The REAL remaining gaps (verified), ranked by value × availability

Most are contended by other sessions or blocked on the owner — flagged so overnight/uncontended
sessions don't collide.

1. **Physical two-phone nearby test (P5)** — *blocked: owner hardware.* Nearby sync is proven on
   on-Mac Bonjour + logic; never on two real phones. #1 thing TF-v2 unblocks. (Backlog, owner.)
2. **`/2` web unification + deploy to CF mirror** — *contended:* an active gateway session owns
   `apps/gateway/newswire.py`. Don't touch (cross-session PR collisions bit the composite-site
   Unit 1). Deploy is also an owner/guardrail action.
3. **Owner ratifications** — 1A `CurrentEntryV2` (Risk 2), per-community sealed identity (Risk 12).
   *Blocked: owner decision.*
4. **Follower new-post notification (P4)** — a follower must open+sync to learn of a new post;
   no push/badge. Real gap, but background-delivery is a large feature (APNs/background sync) —
   not a safe overnight change; needs a design decision.
5. **Discovery / community index (P1)** — no way to *find* a community's site to follow beyond an
   out-of-band link. Real, but a product/architecture decision (a directory), not a quick fix.
6. **Android is a generation behind** — new-model core present but the community-first shell is
   unmounted (still the old debug surfaces). Large; deferred until iOS/newswire validated.
7. **Trust legibility polish (cross-cutting)** — the signed/verified/open distinction exists in
   several surfaces (wire treatment, verify-landing outcome, web badge); a consistency pass
   (unified vocabulary + a one-line "what this means") is a genuine small UX win, in-lane and
   uncontested — the best candidate for incremental iOS polish if one is wanted.

---

## TestFlight-v2 test script (per persona) — reflects the SHIPPED UX

Run on device after the native core is archived from clean `main`. Each step cites the surface it
exercises so a failure localizes.

**P3 Organizer (first-run + editorial):**
1. Install → `LaunchView`: optionally set a display name (skippable) → "Create a community", name it. *(first-run works?)*
2. Post an update (`PostUpdateView`). Post an Alert (pick Alert mode → fill source + expiry + coarse location → Post). *(compose modes + operational fields — Post enables once fields set?)*
3. On the wire, run each editorial action on a post: **Feature, Verify, Correct, Hide, Retract, Tombstone**. *(all 6 present + effect visible?)*
4. Community settings → **"Share this community"** → confirm a riot:// link + QR appear (`ShareCommunitySheet`).

**P2 Contributor (identity + post):**
1. Avatar → **Your profile** → set display name (`YourProfileSheet`). *(prominent enough?)*
2. Post → confirm it appears on the wire; note the pending-first-sync signal if offline.

**P4 Follower (join by link/QR):**
1. 2nd device/account → chooser → **"Join with a link or QR"** → paste the organizer's link OR **scan the QR**. *(honest "name arrives on first sync" preview, no fabricated title?)*
2. After sync, the joined Home shows the organizer's posts. Re-pasting the same link → switches, no duplicate.

**P5 Nearby (offline field):**
1. Two phones, airplane-mode + local Wi-Fi/BLE → Radar pair → confirm a post crosses. *(the untested-on-hardware case — the real TF unblock.)*

**P1 Reader → verify loop (the differentiator):**
1. Open the web mirror → read a post → tap **"Open in Riot"** (or scan the post QR).
2. In the app, confirm the landing sheet: **if you follow that community and hold that post → "verified"** (signature checks out); **if you follow but don't hold it → "post not held"**; **if you don't follow → "not following, offer to join"**. *(A forged entry id from a hostile mirror must land on "not held" — never a fake ✓. `DeepLinkTests.testVerifiesAGenuineSignedPostAndRefusesAForgedEntryID` proves this in-app.)*

---

## Recommended next steps (uncontested, safe first)

- **Owner:** archive TF-v2 from clean `main`; run the script above; ratify the two pending decisions.
- **Coordinator:** treat the audit's iOS-UX items as DONE **except first-run/onboarding** — that is
  under active redesign in TWO places (`compact-flow` in this worktree + `design/interaction-frame`).
  Do NOT re-point the roadmap away from it; instead **reconcile those two sessions** so first-run
  isn't rebuilt twice with divergent premises. Re-point at the genuinely open gaps (notifications,
  discovery, Android parity) after re-verifying each against current HEAD, not `ae9ec47`.
- **Uncontested overnight/incremental work:** trust-legibility consistency pass (gap #7) is the one
  safe in-lane iOS code task — **but scope it against the interaction-frame's "in-context teaching"
  remit first** (the frame owns first-encounter verify/signed cues; gap #7 owns the wire-treatment /
  verify-landing / web-badge *vocabulary consistency*). Assign it to exactly one session so the
  "signed/verified/open" language isn't unified twice, divergently.
