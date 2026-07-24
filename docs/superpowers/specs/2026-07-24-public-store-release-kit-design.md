# Riot Public Store Release Kit

Status: approved for design review.

## Goal

Prepare Riot 1.0 as a free, worldwide public early-access release for:

- iPhone and iPad through the Apple App Store;
- macOS through the Mac App Store; and
- Android phones and tablets through Google Play.

The same candidate binaries must pass TestFlight and Google Play internal
testing before they are promoted to public distribution. The release kit must
generate every repository-owned artifact needed for submission, expose
credential-gated upload commands, and identify the small set of legal,
contractual, hardware, and console actions that cannot be completed from the
repository.

## Product position

The store listing will describe Riot as:

> Community-owned news and practical tools that keep working locally when
> networks are unreliable.

Version 1.0 is an early-access release, not a claim that every planned
resilient-network feature is field-proven. In particular, nearby radio sync
must be described as experimental until the physical-device release gate is
recorded against the exact candidate builds.

Riot is free, has no in-app purchases, and is intended for worldwide
availability. Store copy must avoid rankings, performance superlatives,
absolute privacy or anonymity claims, and capabilities not present in the
submitted build.

## Current-state findings

The repository already contains:

- an App Store Connect record for `net.protest.riot`;
- an iOS app icon and a TestFlight archive/export script;
- iOS and macOS Xcode projects sharing the SwiftUI application code;
- an Android application backed by the same Rust/UniFFI core;
- real iPhone screenshots and a seeded demo path;
- a public privacy page and release page; and
- broad unit, UI, integration, coverage, and packaging checks.

The release kit must resolve these verified gaps:

- iOS, macOS, and Android report pre-1.0 marketing versions in project files;
- Android's public application ID is `org.riot.evidence`, not
  `net.protest.riot`;
- Android has no release signing configuration, adaptive launcher assets, or
  Play-ready bundle workflow;
- macOS is configured for ad-hoc local signing rather than Mac App Store
  distribution and has no app icon;
- no canonical Apple/Google metadata or privacy-answer source exists;
- existing screenshots are not a complete, dimension-validated cross-platform
  store set;
- the current release script covers iOS/TestFlight only; and
- physical nearby exchange remains a documented hardware rehearsal gate.

Existing unrelated working-tree changes are outside this release design and
must not be overwritten, staged, or included in release-kit commits.

## Release package architecture

Add a canonical `release/` tree with four responsibilities:

1. **Metadata source** — shared positioning plus Apple- and Google-specific,
   localized field values.
2. **Policy evidence** — privacy, Data safety, permissions, content rating,
   review notes, encryption questions, and evidence links into the code.
3. **Visual source and output** — checked-in source captures/templates and
   reproducibly generated store assets.
4. **Candidate production** — scripts and manifests for build, validation,
   upload, hardware sign-off, and promotion.

The directory should be automation-ready without requiring Fastlane in this
slice. Metadata should use stable, parseable files where that helps validation
and plain Markdown for reviewer-facing worksheets. Generated outputs must be
deterministic for identical source inputs.

Secrets, certificates, provisioning profiles, keystores, API keys, passwords,
and authenticated session data must remain outside git. Scripts may accept
paths and credentials from environment variables or explicitly ignored local
configuration.

## Identifiers and versions

The public bundle/application identifier is `net.protest.riot` on iOS,
macOS, and Android.

Android's `applicationId` changes before its first Play upload. Its Kotlin
namespace and source package may remain `org.riot.evidence`; changing the
public application ID does not justify a source-wide rename.

All public targets use marketing version `1.0`. Store build numbers/version
codes must increase monotonically and be supplied by the release tooling.
Local debug workflows must remain usable without distribution credentials.

## Metadata deliverables

The English (U.S.) source listing will include at least:

- app name and platform-appropriate subtitle/short description;
- promotional text where supported;
- full description;
- keywords where supported;
- category recommendations;
- release notes;
- support, marketing, and privacy URLs;
- review contact/instructions worksheet;
- pricing and worldwide-availability checklist; and
- copyright and seller/developer fields that require account confirmation.

Character limits must be validated locally. Apple and Google variants may
differ where their policies or fields differ, but their factual claims must
remain consistent.

## Store visual system

The selected direction is **cinematic overlay**:

- real, current Riot UI occupies most of every image;
- a compact dark headline band establishes the benefit;
- overlay copy uses Riot's existing blunt poster voice;
- platform-specific crops preserve legibility; and
- screenshots never depict fake controls, fake notifications, unshipped
  features, rankings, prices, or unverifiable security claims.

The primary narrative is:

1. **Your community. Your newswire.** — Spaces/Home.
2. **Publish signed updates from the field.** — Compose.
3. **See what's verified, corrected, or still unconfirmed.** — Newswire.
4. **Carry useful tools with the community.** — apps/checklists.
5. **Exchange updates nearby.** — Nearby, marked experimental.
6. **Keep reading when the network doesn't cooperate.** — offline/local state.

Outputs must cover the current required Apple screenshot classes for iPhone,
iPad, and Mac, plus Google phone and tablet screenshots. Google also receives
a 512 by 512 store icon and a 1024 by 500 feature graphic. Each app bundle
receives correct platform-native launcher icons, including Android adaptive
icons and a macOS app-icon set.

Preview videos are out of scope for 1.0. They add production and review risk
without proving a user workflow that the screenshot sequence cannot show.

Asset validation must check dimensions, format, alpha requirements where
applicable, file size, ordering, and expected count. Every generated image must
be visually reviewed at its intended aspect ratio before completion.

## Privacy and policy evidence

The provisional, evidence-dependent privacy position is:

- no developer account is required;
- no advertising, tracking, or third-party analytics is integrated;
- the developer does not operate hidden collection of app activity; and
- people intentionally exchange signed content with communities, local files,
  public sites, or nearby peers.

This position must be verified against the complete submitted dependency graph
and network behavior. Store answers must not infer "no collection" merely from
the product thesis.

The release package will contain:

- Apple App Privacy answers and evidence;
- Google Data safety answers and evidence;
- permission justifications for camera, Bluetooth, local network,
  notifications, and Android's LAN-required `INTERNET` permission;
- an Apple privacy-manifest and required-reason-API audit;
- a privacy-policy consistency check;
- content-rating recommendations;
- user-generated-content policy analysis covering reporting, blocking,
  moderation, objectionable content, and public contact information; and
- review notes explaining first launch, demo content, no-login behavior,
  offline behavior, local permissions, and nearby testing.

If the audit finds a required safeguard missing, public promotion is blocked
until the safeguard is implemented and tested. Internal beta distribution may
continue when store policy allows it and the limitation is disclosed to
testers.

Apple encryption/export-compliance classification remains a human/legal
decision. Riot uses application cryptography, so the kit must present the
actual algorithms and distribution behavior and record the selected answer; it
must not preserve or change `ITSAppUsesNonExemptEncryption` by assumption.
Store agreements, tax, banking, trader-status, and similar account declarations
are also human-only gates.

## Platform candidate production

### iOS and iPadOS

- Keep Apple team signing external to source control.
- Produce a Release archive and App Store export suitable for TestFlight and
  later public promotion.
- Harden the existing release script so version, build, source state, archive,
  export, and upload intent are explicit.
- Preserve the current Keychain access-group behavior and device-only storage
  protections.

### macOS

- Preserve ad-hoc signing for local development.
- Add a distribution configuration/path for Apple team signing and Mac App
  Store export.
- Supply the sandbox entitlements, versioning, usage descriptions, app icon,
  and packaging metadata required by the store build.
- Verify that shared SwiftUI screens remain usable at Mac window sizes.

### Android

- Change `applicationId` to `net.protest.riot`.
- Add adaptive and legacy launcher icons plus the Play store icon.
- Add release signing driven only by external credentials.
- Produce a signed release Android App Bundle (`.aab`).
- Keep release builds possible in a validation-only unsigned mode when the
  signing key is unavailable, while refusing any upload/promotion command.
- Preserve API-level policy compliance and the current local-network address
  restrictions.

## Promotion flow

1. Select a clean, committed source revision.
2. Run all blocking repository and release validations.
3. Generate metadata, assets, candidates, hashes, and a release manifest.
4. Upload the Apple candidate to TestFlight and Android candidate to Play
   internal testing.
5. Run the device and policy rehearsal against those exact candidates.
6. Record results and human approvals in the release manifest/checklist.
7. Promote the exact accepted candidates to public early access without
   rebuilding.

The manifest records:

- full git commit;
- dirty-tree status;
- marketing versions and platform build numbers;
- public identifiers;
- binary/archive and visual-asset SHA-256 hashes;
- validation commands and results;
- privacy/policy worksheet revision;
- hardware matrix and results; and
- remaining human gates.

## Blocking verification

Automated release checks include:

- `cargo test --workspace --all-features`;
- strict Rust formatting, check, and Clippy;
- the coverage floors from `.coverage-thresholds.json`;
- iOS and macOS unit/build checks;
- Android unit, instrumentation/device, lint, and release-bundle checks;
- zero-test detection;
- first-launch and core create/join/post/read/persist workflows;
- identifier and version consistency;
- metadata limits and required fields;
- screenshot/icon/feature-graphic validation;
- privacy-answer and public-policy consistency; and
- repository secret scanning.

The device rehearsal covers:

- first launch without an account;
- creating and joining a community;
- publishing, reading, and persistence after restart;
- permissions denied and later recovered;
- offline/local behavior;
- accessibility and large text;
- representative iPhone, iPad, Mac, Android phone, and Android tablet layouts;
  and
- physical nearby exchange, including at least two real devices and the
  advertised fallback behavior.

No public-readiness claim is allowed when a blocking check is absent, skipped,
or reports zero executed tests.

## Failure behavior

Release commands fail closed with actionable diagnostics for:

- dirty or mismatched source state;
- missing generated Rust/UniFFI artifacts;
- wrong identifiers, versions, or build numbers;
- absent signing credentials for a signed/upload action;
- invalid or incomplete metadata;
- missing or invalid visual assets;
- inconsistent privacy declarations;
- failed or empty test runs;
- candidate-hash mismatch; or
- missing required human/hardware approvals.

Tooling must never:

- copy credentials or signing material into the repository;
- silently upload;
- overwrite an accepted candidate;
- rebuild between beta acceptance and public promotion;
- mark a legal answer on the user's behalf; or
- report readiness with a blocking gate unresolved.

## Definition of done

The release kit is complete when:

1. Apple and Google metadata passes local validation and is ready for manual
   entry or later API automation.
2. All required store visuals exist, pass mechanical validation, and pass
   visual review.
3. iOS/iPadOS, macOS, and Android candidate artifacts can be produced by
   documented commands.
4. Public identifiers and versioning are consistent.
5. Privacy, policy, permissions, content-rating, review, and encryption
   worksheets are complete and evidence-backed.
6. All repository quality and coverage gates pass.
7. A candidate manifest records artifact hashes and all automated results.
8. The TestFlight, Play internal-test, device-rehearsal, and public-promotion
   steps are documented without requiring a rebuild.
9. The only unresolved tasks are explicitly authenticated console actions,
   legal/account declarations, or physical-device observations that cannot be
   performed in the repository.

## Explicit non-goals

- No Fastlane or store-API integration in this slice.
- No paid distribution, subscriptions, or in-app purchases.
- No localization beyond English (U.S.) for the initial release.
- No app-preview video.
- No source-wide Android package/namespace rename.
- No implementation of deferred private-group/MLS features.
- No claim that nearby sync is field-proven before the hardware gate passes.
