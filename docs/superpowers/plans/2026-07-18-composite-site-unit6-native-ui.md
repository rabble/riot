# Composite Site — Unit 6: Native UI (iOS + Android) — Implementation Plan

**Date:** 2026-07-18
**Design:** `docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md` §6 (view model it renders), §8 Unit 6, §9.3 (mandatory seizure disclosure), §3 (editor-invite handshake), §5.3 (fail-closed transport copy).
**Depends on:** Unit 4 (resolved composite view model — NOT built) and Unit 5 (iroh transport / ticket — landed #31). **Do not start until Unit 4 is on `main`.**
**Grounded against HEAD** (2026-07-18 recon).

---

## 1. Scope

Unit 6 is the **native rendering + interaction** for composite sites on iOS + Android, drawing Unit 4's resolved view model with **no business logic** (shared-core rule). It also adds the interaction surfaces that only exist natively: following a site, the editor-invite handshake, QR, and the mandatory safety disclosures.

**In scope:**
- **Follow-site view (iOS)** — close the Android-only share-join asymmetry. Android has `joinAdditionalCommunity`/`decodeShareReference` (`RiotController.kt:91/109`); iOS has `joinAdditionalCommunity` (`AppModel.swift:649`) + `CommunityJoinSheet` (`CommunityChooser.swift:277`) but the composite *site-follow* entry point is new. Build the iOS follow/join view for a composite site ticket.
- **Composite site surface** — render Unit 4's `ResolvedCompositeSite`: editorial front page, comments, open-wire column, per-item **trust-tier styling** (editorial vs open-wire vs comment must be visually unambiguous — the anti-impersonation UI).
- **Degradation / transport states** — designed copy + next-step for each Unit 4 state (`moderation-loading`, `editorial-only`, `transport-blocked`, `manifest-invalid`, `manifest-rollback-alarm`, `member-unverified`), following the existing honest-degradation convention (`ShellRecoveryView`/`RecoveryNoticeBanner`/`CatalogFailureView` at `ConferenceShellView.swift:383–493`; Android `pendingFirstSync` at `CommunityChooser.kt:63`).
- **Editor-invite two-way handshake** — invitee presents key (QR/paste) → owner mints section cap (Unit 0 `delegate_section`) → returns cap (QR/paste). Signing/verification exists; the *handshake protocol + UI* is new.
- **QR generation + camera scan (iOS + Android)** — none exists today (recon: zero AVCapture/ZXing/camera refs). New on both platforms.
- **Writer expired-cap warning** — render Unit 4's expired-cap view-model datum at compose ("your editorial access expired on <date>"); a peer-rejected write shows `failed/pending`, never "published."
- **Mandatory seizure disclosure (§9.3)** — a required in-app string at site creation: device seizure = full site takeover (captor can impersonate the site and revoke real editors), not merely "key loss." Acceptance item — an activist must make an informed decision before minting a masthead on a phone.
- **Compose-time "unfollowable (require:arti)" notice** — a site created with `require:arti` is declarable but unfollowable in v1; the composer must say so (fail-closed UX, §5.3).

**Out of scope:** any business logic (all in Unit 4 core); the resolver itself; transport dialing internals (Unit 5); async/remote editor invite (design §10 — co-presence assumed for v1).

## 2. Load-bearing invariants (design + shared-core rule)

1. **No business logic in the shells.** Trust tier, treatment, degradation, expired-cap state are all resolved by Unit 4; Unit 6 renders exactly what core produced. If a decision is tempting in Swift/Kotlin, it belongs in Unit 4.
2. **Trust-tier styling must be unambiguous.** The whole point of core tagging W ≠ editorial is defeated if the UI styles them alike. Editorial, open-wire, and comment must be visually distinct — this is a security-relevant UI requirement, not cosmetic.
3. **Seizure disclosure is mandatory and blocking at creation** (§9.3). Not a dismissible footnote — a required acknowledgement before a masthead is minted on-device.
4. **Fail-closed transport copy is honest** (§5.3) — a `require:arti` / `transport-blocked` state says plainly "this site requires Tor, unavailable in this version," never a false "connecting…".
5. **Accountable degradation** — moderated/held/unverified states render as honest placeholders with a next-step, matching the existing `ShellRecoveryView` convention; never a blank screen or infinite spinner.

## 3. Surface (verified against HEAD 2026-07-18)

| Area | iOS (CURRENT) | Android (CURRENT) | Unit 6 action |
|---|---|---|---|
| Shell / routes | `CommunityShell.swift`, `ConferenceShellView.swift:504` `CommunityShellView`, `AppModel.swift:11` `RiotDestination` (home/tools/people/nearby) | `MainActivity.kt`, `ConferenceSurface` enum | Add composite-site surface within the shell |
| Join / follow | `AppModel.swift:649` `joinAdditionalCommunity`, `CommunityChooser.swift:277` `CommunityJoinSheet`, deep-link `RiotDeepLink.swift` | `RiotController.kt:91` `joinAdditionalCommunity`, `:109` `decodeShareReference` | NEW iOS composite-site follow view (close asymmetry) |
| Newswire surface | `NewswireEditorial.swift:480` `NewswireSurfaceModel` | `NewswireEditorial.kt` | Extend/compose into the composite site surface |
| Degradation UI | `ConferenceShellView.swift:383` `RecoveryNoticeBanner`, `:421` `ShellRecoveryView`, `:460` `CatalogFailureView` | `CommunityChooser.kt:63` `pendingFirstSync` | Reuse the convention for Unit 4's degradation enum |
| FFI wrapper | `Core/ProfileRepository.swift:253` `RiotProfileRepository` | `RiotController.kt:33` | Add composite-site view-model wrappers |
| QR | NONE (one docstring mention) | NONE | NEW both platforms |
| Camera scan | NONE | NONE | NEW both platforms |
| Editor-invite handshake | signing exists; handshake NO | signing exists; handshake NO | NEW both |
| Seizure disclosure | NONE | NONE | NEW both, blocking at creation |
| pbxproj | `apps/ios/Riot.xcodeproj/project.pbxproj` + `apps/macos/Riot.xcodeproj/project.pbxproj` (hand-edited) | n/a | Any NEW Swift file → BOTH pbxproj, serialized |

## 4. Tasks (TDD — RED first; native tests per platform)

Split by platform; iOS and Android are largely parallel. Each task: failing test first (XCTest / JUnit), then implement.

- **Task 1 — composite-site surface (render the view model).** iOS + Android views that draw Unit 4's `ResolvedCompositeSite`. RED: given a resolved model with editorial + comment + open-wire items, each renders in its tier; a `Hidden`/`Tombstoned` item renders as an accountable placeholder (not absent).
- **Task 2 — trust-tier styling (anti-impersonation).** RED (security-UI): an open-wire item is visually distinct from an editorial item; a test asserts the open-wire item does NOT carry editorial styling/badge. Mutation: if the view read the tier wrong, the test fails.
- **Task 3 — degradation / transport states.** RED: each Unit 4 state (`moderation-loading`, `editorial-only`, `transport-blocked`, `manifest-invalid`, `manifest-rollback-alarm`, `member-unverified`) renders its designed copy + next-step; `transport-blocked` for `require:arti` says the honest fail-closed string; no infinite spinner (loading-timeout fallback surfaces).
- **Task 4 — iOS follow-site view (close asymmetry).** RED: an iOS user can follow a composite site from a ticket/share-ref (parity with Android's `joinAdditionalCommunity`). Reuse `RiotDeepLinkResolver` outcome pattern.
- **Task 5 — QR generation + camera scan (both platforms).** RED: a ticket/editor-key round-trips through QR gen → scan → decode on iOS and Android. New camera-permission handling (fail to Settings, matching the Nearby permission-recovery pattern). **Guardrail (shared-core):** QR decode is **byte transport only** — the scan path must NOT re-parse the ticket's `require`/floor or make its own fail-closed dial decision; that verification stays in Unit 5/Unit 4 core (route the outcome through Unit 4's `transport-blocked` state). A shell that decides "safe to dial" from a scanned ticket migrates the §5.1 pre-dial decision into the UI — forbidden.
- **Task 6 — editor-invite two-way handshake.** RED: invitee key → owner mints `/articles/<section>` cap (Unit 0 `delegate_section`) → invitee holds a working editor cap; the handshake UI walks both roles. Co-presence assumed (design §10). **FFI PREREQUISITE (blocked-pending, verify before starting):** `delegate_section` exists only in `crates/riot-core/src/willow/masthead.rs:70` and is **NOT exposed in `riot-ffi`** today — minting a section cap from native needs a new FFI binding (+ native staticlib rebuild). This is a write/action path neither Unit 4 (read-path view model) nor Unit 6 (render, no-FFI) cleanly owns. Confirm the mint + cap-exchange FFI is present (add it to Unit 4's FFI surface, or a small Unit-3/4 FFI addition) BEFORE Task 6; otherwise Task 6 is blocked. The read-path tasks (1,2,3,7) and follow (Task 4, reuses `joinAdditionalCommunity`) need no new FFI.
- **Task 7 — writer expired-cap warning + rejected-write state.** RED: render Unit 4's expired-cap datum at compose ("expired on <date>"); a peer-rejected write shows `failed`, never "published." **Guardrail (shared-core):** both the expired-cap datum AND the `failed/pending/published` write-status are **core-reported fields** (Unit 4's §6 writer-side contract) — the shell renders them, it must NOT infer publish-success from local state. Name the write-status field explicitly when Unit 4's surface is verified at HEAD.
- **Task 8 — site-creation safety gates: seizure disclosure (§9.3) + `require:arti` unfollowable notice (§5.3).** Both are creation-side, both platforms. (a) RED: minting a masthead is BLOCKED until the seizure disclosure is acknowledged; the required string names impersonation + editor-revocation, not just "key loss." (b) RED: choosing a `require:arti` transport policy at creation shows the compose-time "declarable but unfollowable in v1" notice (distinct from Task 3's *follower-side* `transport-blocked` state — this is the *creator-side* notice). Both iOS and Android.

## 5. Acceptance / RED cases (§8.1 Unit 6 + §9.3)

1. Editor-invite two-way handshake round-trips (both platforms).
2. QR round-trip both platforms.
3. Writer expired-cap warning appears at compose; rejected write is `failed`, not "published."
4. Seizure disclosure present + blocking at creation, with the correct threat wording.
5. Trust-tier visual separation (open-wire never styled editorial).
6. `require:arti` compose-time unfollowable notice present; `transport-blocked` honest copy.
7. Every degradation state has copy + next-step; no blank screen, no infinite spinner.

## 6. File scope (claim before editing — pbxproj is the serialization hazard)

iOS: NEW composite-site view + follow view + QR + handshake + seizure-disclosure Swift files under `apps/ios/Riot/…`; NEW `apps/ios/RiotTests/…` tests; `apps/ios/Riot/Core/ProfileRepository.swift` wrappers; **BOTH `apps/ios/Riot.xcodeproj/project.pbxproj` + `apps/macos/Riot.xcodeproj/project.pbxproj`** for every new Swift file. Android: NEW composite-site views + QR + handshake + disclosure under `apps/android/.../evidence/…`; `RiotController.kt` wrappers; NEW `…Test.kt`.
**pbxproj is the shared-file serialization trap (COLLABORATION rule 5): no unit starts while either pbxproj is claimed/dirty; if a sibling has an uncommitted file ref, committing the working-tree pbxproj puts a dangling ref into HEAD and breaks the build for everyone.** Add Swift-source commits + pbxproj entries carefully (temp-index technique or COLLABORATION note if blocked). NO new FFI expected — Unit 6 consumes Unit 4's view model; if a wrapper needs a new FFI call, that is a Unit 4 gap, not a Unit 6 addition.

## 7. Verification gates

- iOS: `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'`; Riot.app + macOS Riot-macOS BUILD SUCCEEDED; both pbxproj `plutil -lint` OK. Known-red baseline is the 2 Bonjour tests only.
- Android: `cd apps/android && ./gradlew :app:testDebugUnitTest`.
- Camera/QR: at minimum a decode/encode round-trip unit test; device camera scan is state-proven-vs-assumed (no device rig ⇒ mark honestly, like the BLE caveat).
- No business logic in shells — reviewer confirms every decision traces to a Unit 4 view-model field.

## 8. Sequencing & hazards

1. **Blocked on Unit 4** — renders its view model. Re-verify Unit 4's FFI record shapes at HEAD before starting (they will be new).
2. **Native-only surfaces (QR, camera, handshake) are the new build** — the render tasks are mechanical once the view model exists; QR/camera/handshake are the real work and carry device-capability caveats (prove logic, mark physical-device as assumed).
3. **pbxproj serialization** — the top collision risk; see §6. Coordinate; do not start while either pbxproj is dirty.
4. **Seizure disclosure is a safety requirement, not a nicety** (§9.3) — an activist mints a masthead on a phone believing "key loss" is the risk when the real risk is impersonation + editor-revocation. Treat Task 8 as a hard acceptance gate.
5. **iOS/Android parity** — keep the two platforms feature-symmetric (the asymmetry Unit 6 exists partly to close); a feature landing on one platform only must be flagged, not silent.
6. **Shared-checkout** — many native sessions run concurrently (recon shows ~16 worktrees). `gh pr list` before + during; claim the exact files incl. pbxproj (the Unit-1 duplication lesson + the pbxproj-dangling-ref lesson).
