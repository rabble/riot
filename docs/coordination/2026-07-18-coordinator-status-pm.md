# Coordinator status — 2026-07-18 (PM)

Live snapshot as of main `3ede521` (through PR #56). Supersedes the AM status of the same
date — the app moved a lot in one day (10+ merges). This is the honest current state and the
ranked "what's needed" toward the goals.

## The goal, restated
Riot = indymedia for the phone era. **Open newswire** (communal wire + editorial desk),
publish-anywhere: post in the app → distributes device-to-device AND to a signed public web →
verifiable back in the app. Nearby sync for no-internet conditions. **Private groups (MLS)** =
the deliberately-deferred second mode. Composite indymedia sites (owned mastheads + communal
wire). Shared Rust core, native iOS/Android/macOS shells, stateless web gateway.

## Platform maturity (honest)

| Surface | State |
|---|---|
| **iOS** | **Mature.** Publish + editorial (Feature/Verify/Correct/Hide/Retract), **communal comments/replies** (#53), multi-community join-by-link/QR + share (#42), nearby sync, **notifications + what's-new/unread badges** (#50/#52), "Open in Riot" verify, onboarding, self-healing recovery (5 phases), coherent light-locked UI, moderation read (#54). On TestFlight (build 706). |
| **Web** | **Done + deployed.** Unified signed `riot-public-gateway-export/2` (proof bytes + independent reverification), display-name bylines, navigable site + app-drop, live at riot-newswire-dev. |
| **Android** | **Parity in progress.** Has profile store, persistence, community chooser, notifications engine (#55), newswire reading surface (#56). LAGS iOS: no comments, editorial, recovery, verify-loop, onboarding yet. |
| **macOS** | **Runnable + coherent.** The shell (pink sidebar, centered column, light-locked). Reuses iOS views; had a target-membership break (#39, fixed). |
| **Composite-site** (owned namespaces) | Units 0–4 landed (admission, manifest, moderation `/mod/`, composite resolver #45), moderation read on iOS (#54). Unit 6 (native UI) planned. |
| **Transport** | iroh FrameChannel + fail-closed ticket gate + runnable testnet seed/follow nodes (#31/#36/#38). |
| **Private groups (MLS)** | **Unbuilt.** Deferred by design. |

## What's needed — ranked toward the goals

1. **Android parity.** iOS is the reference; Android has the read + notify + persistence spine but
   lacks the write/editorial/comments/recovery/verify surfaces. This is the biggest gap to
   "cross-platform activist tool." Sequence: publish + editorial → comments → recovery → verify.
2. **On-device validation (the real test).** The whole thing has only just started running on
   real hardware (iOS TF 706). No two-device nearby-sync test on real phones yet — that's the
   load-bearing proof that publish-to-devices works in the field, not just on-Mac Bonjour.
3. **The identity-in-UI decision (design call, owner's).** Per-community identities are
   deliberately unlinkable; the byline shows "member · <hex>" until you name yourself *within a
   community*. Auto-carrying a name across communities would break pseudonymity. Needs an explicit
   product decision on how to make this friendly without linking identities. (See UX audit.)
4. **Public community anchor/discovery network.** New plan on main
   (`2026-07-18-public-community-anchor-network-implementation.md`) — how communities are found
   and anchored publicly. Planned, unbuilt. This is the "reach" half of discovery.
5. **Composite-site Unit 6** (native UI for owned sites) — the last planned composite unit.
6. **UX polish (from the persona audit + engagement gap map #48):** the masthead-header overlap
   on macOS, trust legibility, invite/share flow depth, discovery. Polish, not blockers.
7. **iOS/macOS app build in CI (nice-to-have).** CI is Linux-only; app-compile is only checked by
   local `scripts/green.sh` (which DOES build both app targets). Two app breaks slipped past green
   Linux CI this run (#33, #39). A `macos-latest` job would enforce green.sh automatically. Deferred
   — green.sh is the safety net if run before merging Swift.
8. **Deferred by design:** Private groups (MLS + encrypted drops); the whole-product human field trial.

## What shipped today (10+ merges, for the record)
Communal comments/replies (#53), notifications iOS+Android (#52/#55), what's-new/unread (#50),
composite moderation (#41) + resolver (#45) + iOS read (#54), the iOS surface 8-units (#42),
app-in-Tools (#44), macOS + shell coherence (#39/#49/#51), offline-branding removed (#40/#43),
self-healing recovery (#32/#34), Open-in-Riot verify (#28), onboarding (#29), web `/2` unification
(#26), Android newswire (#56), the engagement gap map (#48).

## Coordination notes (this multi-session run)
- **~13 sessions run concurrently; the board drains fast.** Everything currently merged, CI green,
  no open PRs at time of writing.
- **App-target breaks pass Linux CI.** Run `scripts/green.sh` (builds iOS + macOS app targets)
  before merging Swift — twice this run a break landed on green Linux CI and was caught at
  build-a-TestFlight time.
- **TF builds:** the coordinator cuts them (native core rebuild → xcodebuild archive → stage in
  Organizer; owner uploads via Organizer — no distribution cert/API key on the CLI). Build 706 is
  current. Bump the build number past the last upload each time.
- The per-community-unlinkable-identity model is load-bearing; do not "fix" the hex byline by
  carrying names across communities without an explicit owner decision — it would break the
  pseudonymity guarantee.

## Bottom line
The **open-newswire product is essentially built and shippable on iOS + web**, with a coherent UI,
real two-way interaction (comments), notifications, and a self-healing data layer — on TestFlight
now. The gap between here and "a real cross-platform activist tool people trust in the field" is
now **Android parity + on-device validation + the discovery/anchor network + one identity-UX
decision** — plus the deliberately-deferred private-groups mode. The hard protocol/trust/web work
is done; what remains is breadth (Android), proof (devices), reach (anchors), and polish.
