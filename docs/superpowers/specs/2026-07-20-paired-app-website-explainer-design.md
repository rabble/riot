# Paired App + Website Explainer Design

**Date:** 2026-07-20  
**Status:** Approved in conversation; pending metaswarm design review  
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
3. **A maintainer changing product copy** wants an explicit test contract so
   that the app and website do not silently drift into contradictory claims.

## Product Decision

Both surfaces tell the same five-beat story, in this order:

1. **No servers, no accounts**
2. **Publishing is peer-to-peer**
3. **Many mirrors, not one site**
4. **Signed, not trusted**
5. **Web is reach; the app is proof**

The headings are the cross-surface contract. Supporting copy may be adapted for
the reading context, but it must preserve the same claims:

- identity is a cryptographic key rather than a service login;
- signed posts move between participant devices and volunteer seeds;
- websites are replaceable mirrors rather than the canonical authority;
- signatures prevent a mirror from forging or altering authorship;
- the web maximizes reach while the app performs authoritative verification.

This is deliberately a compact conceptual primer, not a protocol tutorial.

## App Experience

The first-run welcome screen presents three clear actions:

- **Get started** continues to community setup.
- **Join or find a community** continues to the same setup surface, where the
  existing join-by-link/QR, follow-by-site, and nearby paths are available.
- **How Riot works** opens an offline-native sheet containing the five-beat
  story and a clear **Done** action.

The explainer uses existing `RiotCard`, typography, color, header, and button
patterns. It performs no network request, stores no new state, and does not
block setup. VoiceOver reads each heading and its supporting paragraph as one
logical element.

The explainer remains first-run context in this slice. Adding a persistent
in-app help/settings destination is a separate product decision.

## Website Experience

The public marketing homepage at `marketing/index.html` adds the same
five-beat primer at the beginning of the existing **How it works** section.
The current six-step workflow remains immediately below it under a distinct
**What you do** label. This separates:

- the stable mental model (why Riot works), from
- the concrete workflow (what a person does).

The primer follows the existing poster/card visual language, uses semantic
headings and ordinary HTML, and requires no JavaScript or external assets. It
must work at desktop and narrow mobile widths.

`marketing/public/index.html` remains byte-identical to the source homepage,
as required by the existing deployment contract.

The community-specific gateway `/about/` page remains unchanged. It already
tells a contextual version of the same censorship-resistance story for an
individual newswire; this slice pairs the native app with the general public
marketing site.

## Architecture and Data Flow

There is no runtime cross-surface dependency:

- Swift owns the native presentation and a small testable story definition.
- Static HTML owns the marketing presentation.
- Contract tests pin the five ordered headings on both surfaces.

A shared JSON runtime asset was rejected because it would add parsing,
packaging, and failure modes to an explainer that must always work offline.
A web view was rejected because it would make first-run understanding depend on
connectivity and weaken native accessibility.

## Security and Privacy

The feature accepts no user input and introduces no network, persistence,
authentication, or authorization boundary. The marketing page remains static
and its existing no-third-party-runtime posture is unchanged.

The copy must not overclaim private-group functionality or imply that reading a
hostile mirror is itself authoritative. The final beat explicitly preserves the
trust boundary: web for reach, app verification for proof.

## Test-Driven Implementation

### Native RED → GREEN

1. Add a Swift unit test that requires the ordered five-heading
   `OnboardingExplainerStory` contract; observe the test target fail because
   that testable story definition does not yet exist.
2. Extract the existing explainer points into the smallest internal story type
   and render the sheet from it; rerun the targeted tests to green.
3. Add/extend an iOS UI test covering welcome → **How Riot works** → **Done**
   and the independent **Join or find a community** action.

### Website RED → GREEN

1. Extend the marketing contract test to require the ordered five headings,
   the **What you do** workflow label, semantic structure, and source/public
   mirror equality; observe failure against the current homepage.
2. Add the primer and minimal responsive styling to the source homepage.
3. Copy the completed source homepage to its byte-identical public mirror and
   rerun the contract test to green.

### Verification

- Run the focused Swift tests and iOS UI flow on an installed simulator.
- Run `node scripts/marketing/protocol-page-contracts.mjs`.
- Run the relevant broader iOS test/build gate required by the repository.
- Render the marketing homepage at desktop and narrow mobile widths and inspect
  screenshots for hierarchy, wrapping, contrast, and overflow.
- Confirm `marketing/index.html` and `marketing/public/index.html` are
  byte-identical.

## Acceptance Criteria

- The app explainer is reachable in one tap from the first-run welcome screen
  and dismisses without altering profile or community state.
- Join/find is a first-class welcome action rather than hidden behind
  create-first wording.
- The app and marketing homepage present all five contract headings in the
  approved order.
- The website distinguishes the five-beat mental model from its existing
  six-step workflow.
- The app explainer works with no connectivity.
- The marketing site adds no dependency, runtime fetch, or JavaScript.
- Native, marketing-contract, mirror-drift, and visual checks pass.

## Failure Criteria

The slice is not complete if either surface omits or reorders a contract
heading, if the marketing deployment mirror drifts, if the app explainer needs
network access, if the mobile layout overflows, or if the website implies that
an unverified mirror is authoritative.

## File Scope

Expected implementation scope:

- `apps/ios/Riot/ConferenceShellView.swift`
- `apps/ios/RiotTests/ShellNavigationTests.swift` or a focused adjacent test
- one existing `apps/ios/RiotUITests/*.swift` flow test
- `marketing/index.html`
- `marketing/public/index.html`
- `scripts/marketing/protocol-page-contracts.mjs`

No Rust core, FFI, database, gateway, protocol, or deployment configuration
changes are required.
