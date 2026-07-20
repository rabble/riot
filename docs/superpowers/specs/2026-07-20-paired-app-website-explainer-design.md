# Paired App + Website Explainer Design

**Date:** 2026-07-20  
**Status:** Approved in conversation; design review revision 1  
**Branch:** `feat/onboarding-find-and-explainer`

## Problem

A new Riot user can see individual product flows without first understanding
the system they are joining. The app and public website currently emphasize
different levels of the story: the app begins with community setup, while the
marketing site begins with a detailed six-step workflow. People need one compact
mental model that appears in both places and remains true when the network is
unavailable.

## Use Cases

1. **A first-time app user** wants to understand what makes Riot different so
   that they can decide whether to create or join a community before setup,
   including when the phone is offline.
2. **A prospective user on the public website** wants the same compact
   explanation so that arriving from a shared link does not produce a different
   product story from the app.
3. **A maintainer changing product copy** wants one build-time contract covering
   both surfaces so that headings and safety-critical supporting claims do not
   silently drift or contradict each other.

## Product Decision

Both surfaces tell the same five-beat story, in this exact order:

1. **No central account or publishing server**
2. **Publishing moves peer to peer**
3. **Many mirrors, not one site**
4. **Signed records, checked in the app**
5. **Web for reach; the app for provenance**

The headings are the cross-surface contract. Supporting copy may be adapted for
the reading context, but it must preserve the same claims:

- identity is a cryptographic key rather than a mandatory service login;
- signed posts move between participant devices and volunteer seeds, but this
  transport is not anonymous and infrastructure may observe connections;
- seeds, anchors, and mirrors may run on servers, but none is the canonical
  publishing authority or single required address;
- a hostile mirror can display altered text or false attribution in a browser,
  but it cannot produce an independently synced signed record the app accepts
  as the claimed author;
- app verification establishes cryptographic provenance and authorization of
  the independently held record, not factual truth, completeness, freshness,
  safety, or editorial endorsement;
- the web maximizes reach while the app lets the reader inspect the
  independently synced, admitted record.

This is deliberately a compact conceptual primer, not a protocol tutorial.

## App Experience

The first-run welcome screen presents three clear actions:

- **Get started** continues to community setup.
- **Join with a link or QR** continues to setup and directly presents the
  existing join sheet, making that intent meaningfully different from
  **Get started**. Setup also exposes the existing follow-by-site action.
- **How Riot works** opens an offline-native sheet containing the five-beat
  story and a clear **Done** action.

The explainer uses existing `RiotCard`, typography, color, header, and button
patterns. It performs no network request, stores no new state, and does not
block setup. Each beat remains a heading in the VoiceOver rotor; its supporting
paragraph follows in reading order. A standard sheet toolbar **Done** action
remains reachable at accessibility Dynamic Type sizes.

Nearby exchange is not an onboarding exit. The existing setup note continues
to explain that nearby exchange becomes available only after entering a
community.

The explainer remains first-run context in this slice. Adding a persistent
in-app help/settings destination is a separate product decision.

## Website Experience

The public marketing homepage at `marketing/index.html` adds the same
five-beat primer at the beginning of the existing **How it works** section.
The primer is an ordered list with one section heading and semantic headings for
each beat. The current six-step workflow remains immediately below it under a
distinct **What you do** heading. This separates:

- the stable mental model (why Riot works), from
- the concrete workflow (what a person does).

The primer follows the existing poster/card visual language, uses semantic
headings and ordinary HTML, and requires no JavaScript or external assets. It
must work at desktop and narrow mobile widths.

`marketing/public/index.html` remains byte-identical to the source homepage,
as required by the existing deployment contract.

The community-specific gateway `/about/` page keeps its contextual layout but
receives the same trust-boundary correction: it must no longer claim that an
untrusted mirror cannot alter what a browser displays or is inherently safe to
read. It instead directs readers to inspect the independently synced record in
Riot when provenance matters.

## Architecture and Data Flow

There is no runtime cross-surface dependency:

- `CommunityShell.swift`, which is shared by the iOS and macOS RiotKit targets,
  owns a small testable story definition and the initial setup intent.
- `ConferenceShellView.swift` owns the native presentation.
- Static HTML owns the marketing presentation.
- The existing marketing contract script owns one build-time expected-story
  array and inspects both the Swift definition and homepage. It pins the five
  ordered headings, critical hostile-mirror caveat, provenance limitation, and
  rejected unsafe phrases across both surfaces.

A shared JSON runtime asset was rejected because it would add parsing,
packaging, and failure modes to an explainer that must always work offline.
A web view was rejected because it would make first-run understanding depend on
connectivity and weaken native accessibility.

## Security and Privacy

The explainer accepts no user input and introduces no network, persistence,
authentication, or authorization boundary. The marketing page remains static
and its existing no-third-party-runtime posture is unchanged. The direct join
action only routes into the existing join sheet and does not change its
validation or admission boundary.

The copy must not overclaim private-group functionality, anonymity, or imply
that reading a hostile mirror is authoritative. It must explicitly state that:

- a mirror may display arbitrary or altered browser content;
- the app checks the independently synced record and its admission relationship;
- successful cryptographic verification establishes provenance, not truth,
  freshness, completeness, safety, or editorial approval.

## Test-Driven Implementation

### Native RED → GREEN

1. Add a Swift unit test that requires the ordered five-heading
   `OnboardingExplainerStory` contract and distinct `.general` / `.join`
   `OnboardingSetupIntent` values; observe the test target fail because those
   RiotKit definitions do not yet exist.
2. Add the smallest public story/intent value types to `CommunityShell.swift`,
   render the existing sheet from the story, and route `.join` into the existing
   join sheet; rerun the targeted tests to green.
3. Extend the isolated `RiotTabNavigationUITests` flow to cover welcome →
   **How Riot works** → **Done**, assert the unchanged welcome returns, then
   exercise **Join with a link or QR** independently while retaining the
   assertion that nearby is absent.

The explainer view already exists in the branch before this continuation. The
native RED proves the missing testable contract and distinct navigation intent;
GREEN is the extraction/refactor and truthful routing, not a claim that the
pre-existing view was written test-first.

### Website RED → GREEN

1. Extend the marketing contract test to require the ordered five headings,
   safety-critical body phrases, absence of rejected unsafe phrases, the
   **What you do** workflow label, semantic structure, and source/public mirror
   equality; observe failure against the current homepage and Swift definition.
2. Add the primer and minimal responsive styling to the source homepage.
3. Copy the completed source homepage to its byte-identical public mirror and
   correct the gateway `/about/` trust-boundary copy; rerun the contract and
   focused gateway tests to green.

### Verification

- Build the required native core first with
  `sh scripts/conference/build-native-core.sh`.
- Resolve the simulator once with
  `SIMULATOR_ID="$(sh scripts/ios-check.sh simulator-id)"`.
- Run the focused RiotKit unit test with:
  `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination "platform=iOS Simulator,id=$SIMULATOR_ID" -derivedDataPath build/ios-derived -only-testing:RiotTests/ShellNavigationTests`.
- Run the focused UI flow with:
  `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot -destination "platform=iOS Simulator,id=$SIMULATOR_ID" -derivedDataPath build/ios-ui-derived -only-testing:RiotUITests/RiotTabNavigationUITests/testCompactCoreFlowFromFirstRunThroughReadingAReport`.
- Run `node scripts/marketing/protocol-page-contracts.mjs`.
- Run
  `python3 -m unittest apps.gateway.tests.test_newswire apps.gateway.tests.test_build_newswire`.
- Run `cmp marketing/index.html marketing/public/index.html`.
- Run the shared-source macOS gate with `sh scripts/ios-check.sh test`.
- Run the final device target-membership gate with
  `sh scripts/ios-check.sh ios`.
- Run the mandatory coverage gate sourced from `.coverage-thresholds.json` with
  `sh scripts/web/coverage.sh`.
- Render the marketing homepage at desktop and narrow mobile widths and inspect
  screenshots, including 320 CSS pixels, for heading hierarchy, ordered-story
  clarity, wrapping, contrast, and overflow.
- Inspect the app at an accessibility Dynamic Type size and confirm the toolbar
  **Done** action and every beat remain reachable and correctly ordered for
  VoiceOver.

## Acceptance Criteria

- The app explainer is reachable in one tap from the first-run welcome screen
  and dismisses without altering profile or community state.
- Join/find is a first-class welcome action rather than hidden behind
  create-first wording, routes directly to the existing link/QR join sheet, and
  does not advertise nearby as an onboarding exit.
- The app and marketing homepage present all five contract headings in the
  approved order.
- The website distinguishes the five-beat mental model from its existing
  six-step workflow.
- Both surfaces state that hostile browser mirrors may alter display, and that
  app verification establishes record provenance/admission rather than truth.
- The app explainer works with no connectivity.
- The marketing site adds no dependency, runtime fetch, or JavaScript.
- Native, marketing-contract, mirror-drift, and visual checks pass.

Before release, run a privacy-preserving moderated comprehension check with at
least five representative first-time readers, including at least two community
organizers and two prospective members. After one read on either surface, at
least four of five must correctly explain:

1. Riot has no mandatory central account or single publishing authority, while
   seeds, anchors, and mirrors may still exist;
2. a browser mirror can display altered material;
3. app verification establishes provenance of the independently synced record,
   not factual truth.

No analytics or tracking are added. Missing the threshold means revising the
copy before release and repeating the check.

## Failure Criteria

The slice is not complete if either surface omits or reorders a contract
heading or critical trust claim, if the marketing deployment mirror drifts, if
the app explainer needs network access, if the mobile layout overflows, if the
direct join action does not open the join flow, or if any surface implies that
an unverified mirror is authoritative or that signatures establish factual
truth.

## File Scope

Expected implementation scope:

- `apps/ios/Riot/ConferenceShellView.swift`
- `apps/ios/Riot/CommunityShell.swift`
- `apps/ios/RiotTests/ShellNavigationTests.swift`
- `apps/ios/RiotUITests/RiotTabNavigationUITests.swift`
- `marketing/index.html`
- `marketing/public/index.html`
- `scripts/marketing/protocol-page-contracts.mjs`
- `apps/gateway/newswire.py`
- `apps/gateway/tests/test_newswire.py`

No Rust core, FFI, database, protocol, project-file, or deployment
configuration changes are required.
