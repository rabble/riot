# Riot Public Store Release Kit

Status: revision 2 for design-review gate.

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

> Community-owned news and practical tools designed to stay useful locally
> when networks are unreliable.

Version 1.0 is an early-access release, not a claim that every planned
resilient-network feature is field-proven. In particular, nearby radio sync
must be described as experimental until the applicable cross-device release
gates are recorded against the exact candidate builds. A valid signature
proves source and integrity, not truth. Editorial labels are community signals,
not independent factual verification. Store text and screenshots must use
those distinctions explicitly.

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
4. **Candidate production** — scripts, immutable manifests, and append-only
   sign-off records for build, validation, upload, hardware approval, and
   promotion.

The directory should be automation-ready without requiring Fastlane in this
slice. Canonical metadata uses JSON with a checked-in JSON Schema; human
worksheets use Markdown. Candidate manifests, visual provenance, build-number
allocations, and ledger events each have their own versioned JSON Schema.
Visual templates and their source captures produce deterministic output when
run with the pinned Node/npm and image-tool versions. One checked-in release
toolchain manifest centralizes every tool version and checksum. Signed Apple
archives and Android bundles are not byte-reproducible promises: each is built
once, treated as immutable, identified by its digest and signing identity, and
promoted without rebuilding.

Secrets, certificates, provisioning profiles, keystores, API keys, passwords,
and authenticated session data must remain outside git. Scripts may accept
paths and credentials from environment variables or explicitly ignored local
configuration. Credentialed commands verify restrictive file permissions, use
least-privilege store roles, never copy keys into persistent home-directory
locations, never place passwords in command arguments or logs, disable
long-lived signing daemons where applicable, clean unavoidable temporary files
with traps, and document revocation and rotation.

## Identifiers and versions

The public bundle/application identifier is `net.protest.riot` on iOS,
macOS, and Android.

Android's `applicationId` changes before its first Play upload. Its Kotlin
namespace and source package may remain `org.riot.evidence`; changing the
public application ID does not justify a source-wide rename.

All public targets use marketing version `1.0`. Build numbers are explicit
operator inputs: `RIOT_IOS_BUILD`, `RIOT_MAC_BUILD`, and
`RIOT_ANDROID_VERSION_CODE`. The tool never derives them from git commit count.
Before a candidate can be built, the operator records the current store-side
maximum for that platform. The validator requires the requested number to
exceed both that maximum and every number in a checked-in append-only release
ledger.

Each immutable candidate ID is
`1.0-<ios|macos|android>-<build>-<12-character-commit>`. Creating a candidate
uses an exclusive local lock and atomic temporary-file rename; reusing an ID,
build number, or artifact path fails.

The cross-checkout lock domain is the designated release branch's allocation
ledger. One release coordinator reserves numbers by committing and pushing an
allocation event before candidate production. Every build starts from a commit
containing that landed reservation. Fetch/rebase detects duplicate allocations;
both colliding candidates are blocked and the later reservation must allocate a
new number and rebuild. The recorded store maximum includes actor and timestamp
and is considered stale at upload time: immediately before every upload, the
operator records a fresh authenticated store readback. If the store maximum is
at or above the reservation, the candidate becomes `superseded` unless the
existing store build can be reconciled to the exact candidate under the remote
mutation rules below.

Local debug workflows remain usable without distribution credentials.

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

## Platform capability and claim contract

Every store claim must be backed by both a code path and a passing candidate
journey. The initial contract is:

| Persona/journey | iPhone/iPad | macOS 14, Apple silicon | Android phone/tablet | Store-claim rule |
| --- | --- | --- | --- | --- |
| Reader: open a local community and read its newswire | required | required | required | universal claim after all three candidates pass |
| Organizer: create/switch a community | required | required | required | universal claim after all three candidates pass |
| Contributor: compose, review, sign, and persist an update | required | required | required | universal claim after all three candidates pass |
| Follower: join by a valid reference and recover from invalid input | required | required | required | platform claim only where the submitted UI passes |
| Community member: open bundled community tools | required | only tools actually compiled into the Mac candidate | required | screenshots and copy are platform-specific |
| Nearby peer: exchange on physical devices | iOS-to-iOS and iOS-to-Android required before advertising | not advertised in 1.0 | Android-to-Android and Android-to-iOS required before advertising | experimental claim appears only after the named pair passes |

The Android implementation currently exposes Spaces, Newswire, Compose,
Import, apps/tools, persistence, followed sites, and Nearby in
`apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`; the
candidate rehearsal, not that inventory alone, authorizes the store claim.

Permission denial, invalid join data, no nearby peers, and unavailable remote
content must leave the core local reading/creation path useful and show an
actionable recovery step. A dead end or a claim that exists only on another
platform blocks that platform's screenshot and public listing.

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
2. **Publish signed updates from the field.** — Compose, with supporting copy
   stating that signatures prove source and integrity rather than truth.
3. **Read signatures and community editorial labels.** — Newswire, showing the
   exact labels present in the captured candidate.
4. **Carry useful tools with the community.** — apps/checklists.
5. **Exchange updates nearby.** — Nearby, marked experimental and used only on
   platforms whose named hardware pairs pass.
6. **Keep a local copy available offline.** — an evidenced local/offline state,
   without claiming universal network resilience.

Outputs must cover the current required Apple screenshot classes for iPhone,
iPad, and Mac, plus Google phone and tablet screenshots. Google also receives
a 512 by 512 store icon and a 1024 by 500 feature graphic. Each app bundle
receives correct platform-native launcher icons, including Android adaptive
icons and a macOS app-icon set.

Preview videos are out of scope for 1.0. They add production and review risk
without proving a user workflow that the screenshot sequence cannot show.

The composition contract uses the implemented Riot identity:

- Anton for overlay headlines, Space Mono for structural labels, and Work Sans
  only when body copy is necessary;
- `ink` `#17160f` and `paper` `#eae6da` as the default band pair, with existing
  pink/blue tokens reserved for accents;
- an opaque headline band occupying at most 24 percent of a portrait image and
  28 percent of a landscape image;
- at least 5 percent safe inset for overlay text and protected UI focal content
  on every edge; the opaque band itself may bleed to the image edge;
- no more than three headline lines and no more than 42 headline characters;
- at least 4.5:1 text contrast;
- headline cap height of at least 56 source pixels on phone output and 72
  source pixels on tablet/Mac output; and
- a 320-pixel-wide thumbnail review in which the headline and the screenshot's
  key state or action remain recognizable.

All essential screenshot claims must also appear in accessible listing text;
color or image text is never the only carrier of meaning.

Source captures come only from deterministic synthetic fixtures built into the
candidate. Production/private communities, real notifications, personal names,
locations, and operational identifiers are prohibited. The asset pipeline
strips image metadata and scans visible fixture text and identifiers before
commit.

Asset validation checks dimensions, format, alpha requirements where
applicable, file size, ordering, expected count, metadata absence, fixture
provenance, safe areas, text geometry, and contrast. A screenshot provenance
manifest records candidate ID, full commit, candidate build, platform,
OS/device class, locale, appearance, source hash, synthetic fixture revision,
and template version. Every generated image is visually reviewed at its
intended aspect ratio and at store-thumbnail scale. The approval record names
the provenance-manifest digest, reviewer, timestamp, and both visual-review
results.

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
- a user-generated-content compliance record covering Terms/user-policy
  acceptance, prohibited-content rules, filtering, in-app content/author
  reporting, local author blocking, moderator response/tombstone handling,
  response ownership/service level, and public contact information; and
- review notes explaining first launch, demo content, no-login behavior,
  offline behavior, local permissions, and nearby testing.

The policy audit is the first implementation work unit. Public promotion
requires tested UGC safeguards, a published response process, and privacy
answers for any transmitted report. If any safeguard is missing, this
release-kit work records a blocking dependency and starts a separately
brainstormed, design-reviewed, TDD implementation workstream before candidate
production. Internal beta distribution may continue only when store policy
allows it and the limitation is disclosed to testers. Release-kit completion
does not waive this public-readiness dependency.

The minimum operational contract is: reports receive an acknowledgment within
24 hours; credible imminent-harm or illegal-content reports receive a
moderator decision within 24 hours; other objectionable-content reports receive
a decision within 72 hours. The public support/report URL identifies the
responsible operator and escalation path. Local blocking works immediately
without waiting for a moderator or disclosing the block list.

Apple encryption/export-compliance classification remains a human/legal
decision. Riot uses application cryptography, so the kit must present the
actual algorithms and distribution behavior and record the selected answer; it
must not preserve or change `ITSAppUsesNonExemptEncryption` by assumption. The
current hard-coded value is removed from candidate configuration. While the
classification is unresolved, only unsigned/local validation builds are
allowed. Apple archive, export, and upload all fail closed. After approval, the
selected value is injected during archive production and verified in the
archived plist for both Apple targets.

Store agreements, tax, banking, trader-status, and similar account declarations
remain human-only gates and must be resolved before public promotion when
required by the store.

The audit also produces an outbound-network matrix for first launch,
denied/granted permissions, nearby sync, and followed-site refresh. It records
the initiator, destination class, transmitted fields, redirect handling, and
retention assumption so developer collection is not confused with
user-directed disclosure.

## Platform candidate production

### iOS and iPadOS

- Keep Apple team signing external to source control.
- Produce a Release archive and App Store export suitable for TestFlight and
  later public promotion.
- Harden the existing release script so version, build, source state, archive,
  export, and upload intent are explicit.
- Read an App Store Connect API key from its original external path after
  checking ownership/mode; do not copy it to
  `~/.appstoreconnect/private_keys`.
- Preserve the current Keychain access-group behavior and device-only storage
  protections.

### macOS

- Preserve ad-hoc signing for local development.
- Add a distribution configuration/path for Apple team signing and Mac App
  Store export.
- Supply the sandbox entitlements, versioning, usage descriptions, app icon,
  and packaging metadata required by the store build.
- Verify that shared SwiftUI screens remain usable at Mac window sizes.
- Ship macOS 14 on Apple silicon only in 1.0 and disclose that support boundary
  in metadata and the hardware matrix. Intel/universal packaging is a later
  release.

### Android

- Change `applicationId` to `net.protest.riot`.
- Add adaptive and legacy launcher icons plus the Play store icon.
- Add release signing driven only by an external, permission-checked Play
  upload key and passwords delivered without command-line exposure. Use Play
  App Signing, record both upload and app-signing certificate fingerprints,
  and run credentialed Gradle commands with `--no-daemon`.
- Produce a signed release Android App Bundle (`.aab`).
- Keep release builds possible in a validation-only unsigned mode when the
  signing key is unavailable, while refusing any upload/promotion command.
- Preserve API-level policy compliance and the current local-network address
  restrictions.

## Release supply-chain contract

Candidate production pins and records the Rust, Swift/Xcode, Java, Android
SDK/NDK, Gradle, Node, npm, and image-tool versions. It requires:

- Cargo release commands with `--locked` (or `--frozen` where the environment
  is already provisioned);
- `npm ci` against the checked-in lockfile;
- a Gradle wrapper distribution SHA-256, wrapper-JAR validation, strict
  dependency verification metadata, and dependency locks;
- advisory/vulnerability scans with an explicit disposition for every finding;
- an SBOM for each native candidate; and
- hashes of lockfiles, tool binaries or version output, and generated UniFFI
  inputs in each candidate manifest.

Dependency or tool changes after candidate creation invalidate that candidate.

## Promotion flow

There are three distinct immutable candidates: iOS/iPadOS, macOS, and Android.
Each owns its build number, signed artifact, store identity, beta result,
hardware result, and promotion gate.

1. Complete build-affecting policy work: UGC dependency, privacy manifest, and
   Apple export classification.
2. Select a clean, committed source revision and explicit unused build numbers.
3. Run portable repository, metadata, asset, supply-chain, and release
   validations.
4. Build each candidate once and write its immutable candidate manifest.
5. Upload iOS/iPadOS to TestFlight, macOS to its App Store Connect beta/review
   path, and Android to Play internal testing. Record store receipts and
   store-assigned build identities in append-only events.
6. Run the platform and cross-device rehearsal against those exact store
   builds.
7. Append test and human approvals that reference the candidate-manifest
   digest.
8. Promote each independently accepted candidate to a staged worldwide public
   early-access rollout without rebuilding.

Candidate states are:

`draft → built → upload-pending → uploaded → beta-accepted →
promotion-pending → promoted`.

`discarded`, `rejected`, and `superseded` are terminal. A remote call whose
result cannot be proven enters `upload-indeterminate` or
`promotion-indeterminate`; neither state permits another mutation.

Every remote store mutation uses a durable intent/outcome protocol:

1. append and fsync an intent event containing candidate ID, operation UUID,
   intended store action, current authenticated store readback, artifact and
   manifest digests, and expected remote identity;
2. invoke the store exactly once;
3. capture the command receipt without secrets;
4. perform an authenticated store readback for public identifier, version,
   build number, processed/upload status, signing certificate identity where
   exposed, and store-assigned submission/build ID; and
5. append and fsync a success or failure event with that readback.

Crash and retry behavior is exact:

- before the intent is durable, no remote call is allowed;
- after durable intent but before a provable outcome, restart enters the
  indeterminate state and performs read-only reconciliation first;
- if authenticated readback proves the intended candidate exists or the
  promotion is active, append a `reconciled-success` outcome and continue;
- if readback proves the mutation never occurred, append
  `reconciled-not-applied`, then a fresh intent may retry the same immutable
  candidate;
- if the store identity conflicts or readback cannot decide, report
  `HUMAN ACTION`; never retry automatically; and
- after a durable outcome, duplicate invocation is a read-only no-op that
  reports the existing receipt.

The store's authenticated readback is authoritative for whether a remote
mutation happened; the immutable local manifest remains authoritative for
which artifact was approved. Reconciliation succeeds only when both identities
agree.

Transitions append events to a ledger; the immutable candidate manifest is
never edited. A rejected build receives a new build number. Partial builds may
be discarded before upload but allocated numbers are never reused. A stale
manifest, changed artifact hash, different signing identity, missing prior
state, or invalid transition fails. Ledger appends use an exclusive lock,
fsync, and atomic rename; interrupted writes recover the last complete event
and report the incomplete transition.

Each candidate manifest records:

- full git commit;
- dirty-tree status;
- marketing version, platform build number, and candidate ID;
- public identifiers;
- signed artifact and visual-asset SHA-256 hashes;
- code-signing identity and certificate fingerprint;
- effective entitlements and archived privacy/export values;
- lockfile, generated-binding, SBOM, and toolchain hashes;
- validation commands and results;
- privacy/policy worksheet revision.

Append-only sign-off events record store upload receipts/identifiers, beta
results, hardware results, human approvals, promotion receipts, remaining
gates, and the SHA-256 of the complete immutable candidate manifest. Promotion
verifies the store build identity, artifact signing certificate, and recorded
candidate-manifest digest before allowing the operator to continue.

The command surface stays deliberately separate:

- `status` is read-only and reports `PASS`, `BLOCKED`, or `HUMAN ACTION` for
  every gate with the failing file/value, expected value, and recovery command;
- `generate` writes deterministic metadata/visual output;
- `build` creates one named candidate;
- `upload` requires explicit credentials and confirmation; and
- `promote` accepts only a beta-accepted candidate.

## Tooling implementation and TDD contract

Release orchestration is implemented as small pure Node ES modules under
`scripts/release/`, with thin executable adapters. Modules receive injected
filesystem, process runner, environment, clock, hash, image inspector, and
console adapters. Tests use `node:test`, checked-in fixtures, fake runners, and
temporary directories; they never invoke a live store, signing identity, or
real credential.

`package.json`, the JS test command, `c8` include patterns, and CI are expanded
so all `scripts/release/**/*.mjs` production modules are subject to the existing
100 percent JS tooling line, branch, function, and statement floors in
`.coverage-thresholds.json`. Platform shell scripts contain only argument
normalization and `exec` into covered Node modules or native build tools.

Every work unit follows RED → GREEN → REFACTOR. The initial test design is:

| Component | RED test written first | Smallest GREEN boundary | Fixtures/helpers and required cases |
| --- | --- | --- | --- |
| Metadata parser/schema | valid source currently has no parser; overlong/missing fields must fail | parse one locale and report JSON-pointer diagnostics | valid Apple/Google files; exact limit, limit+1, missing required, unknown field, malformed JSON |
| Claim/capability validator | platform claim with no passing journey must fail | compare listing claims to capability matrix | universal, platform-only, experimental-unpassed, exact trust vocabulary |
| Visual validator | wrong dimensions/alpha/metadata/unsafe capture must fail | inspect one PNG and provenance entry | every boundary size; alpha rules; EXIF present; production token; missing fixture hash; 320-pixel thumbnail |
| Visual generator | one synthetic capture must render to a golden geometry record | compose one headline band from pinned tokens | phone/tablet/landscape; 1/3/4 lines; contrast; safe-area and band-height boundaries |
| Build-number allocator | duplicate, stale, rebased, cross-checkout, and store-max collisions must fail | validate explicit number and append allocation | empty ledger; duplicate ID; pushed reservation; merge collision; stale maximum; final readback advanced; interrupted append; concurrent local lock; mandatory reallocation |
| Candidate manifest | changed artifact or signing identity must fail verification | create/verify immutable manifest | iOS, Mac, Android; unsigned validation-only Android; missing artifact; stale tool hash; malformed schema |
| State machine/ledger | invalid skip, duplicate transition, accepted-ID reuse, and mutation from indeterminate state must fail | append one legal transition atomically | every legal/illegal edge and terminal state; partial write recovery; rejected/discarded/superseded semantics; rejected retry with new number |
| Process runner | dirty tree, nonzero command, timeout, and zero tests must fail | run one fake validation and normalize result | Cargo/XCTest/Gradle zero-test logs; signal; timeout; redacted stderr |
| Credential guard | persistent copy, broad mode, logged secret, and absent credential must fail | authorize one fake upload using external read-only path | `0600`/wrong modes; ASC key; Android key/password; cleanup; redaction; `--no-daemon` |
| Export-compliance guard | unresolved or archived-value mismatch must block Apple archive/export/upload | inject and verify one recorded decision | unresolved; exempt/non-exempt; iOS/Mac archived plist mismatch |
| Supply-chain guard | missing Gradle checksum/locks or changed lock/tool hash must fail | verify one pinned input set | Cargo/npm/Gradle/toolchain fixtures; advisory finding with/without disposition |
| Status reporter | mixed gates must never summarize as ready | render tri-state result and recovery action | PASS/BLOCKED/HUMAN ACTION; missing evidence; exact file/value diagnostics |
| Upload/promote adapter | implicit upload, rebuilt hash, wrong store ID, unaccepted candidate, and blind retry after a crash must fail | persist intent, call a fake store runner, read back, then persist outcome | crash before intent; after intent/before call; after remote success/before outcome; after durable outcome; reconciliation present/absent/conflicting/unknown; cancellation; receipt capture; final store-max change; per-platform identity; promotion without rebuild |
| Privacy/policy consistency | store answer contradicting code/network/policy evidence must fail | compare one answer to evidence matrix | no collection; user-directed fetch; report transmission; missing UGC control; stale policy URL |

Golden image tests assert geometry/tokens and are supplemented by Playwright
captures plus human visual review; they do not substitute brittle byte-for-byte
PNG snapshots for semantic checks.

Owned implementation scope includes `release/**`, `scripts/release/**`,
`scripts/testflight-release.sh`, `scripts/green.sh`, `package.json` and its
lockfile, `.github/workflows/ci.yml`, `.coverage-thresholds.json` only if its
include/enforcement description must change (floors never decrease), both
Xcode projects and platform plists/entitlements/assets, shared iOS/macOS
SwiftUI and UI-test sources needed for deterministic seed/capture/rehearsal,
Android Gradle/wrapper/resource files plus the relevant `src/main`, `src/test`,
and `src/androidTest` fixture/capture/rehearsal sources, shared `fixtures/**`
synthetic release data, and the marketing privacy/support/release pages. Any
UGC product remediation gets its own approved file scope before edits.

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
- repository secret scanning;
- locked and verified dependency/tool inputs, SBOM generation, and advisory
  disposition; and
- candidate-manifest/signing/store-identity verification.

The device rehearsal covers:

- first launch without an account;
- creating and joining a community;
- publishing, reading, and persistence after restart;
- permissions denied and later recovered;
- offline/local behavior;
- accessibility and large text;
- representative iPhone, iPad, Mac, Android phone, and Android tablet layouts;
  and
- physical iOS-to-iOS, Android-to-Android, and iOS-to-Android nearby exchange
  for any listing that advertises Nearby, using the exact beta candidates and
  advertised fallback behavior. Mac Nearby is not advertised in 1.0.

No public-readiness claim is allowed when a blocking check is absent, skipped,
or reports zero executed tests.

Before rollout, every supported platform must pass 100 percent of the scripted
first-install-to-first-read, create, publish/sign, restart/persist, denied
permission/recovery, and offline-local journeys on every named device class.
There must be zero critical crashes, hangs, data-loss events, privacy/policy
failures, or failed advertised hardware pairs.

Public availability is worldwide but staged over seven days. Apple phased
release and Google staged rollout are used where available. Store diagnostics
and the published support/report channel are reviewed daily. Rollout pauses if
crash-free sessions fall below 99.5 percent on any platform, any critical
data-loss/security/privacy failure is confirmed, an advertised core journey is
unavailable, or moderation reports cannot meet the published response target.
After seven stable days at full rollout, v1 exits heightened observation;
ordinary support and store-diagnostic review continue. A paused candidate is
fixed under a new build number rather than overwritten or rebuilt in place.

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
- candidate, signing-identity, toolchain, or store-build mismatch;
- invalid candidate-state transition or interrupted ledger append;
- unresolved export classification for an Apple archive/export/upload;
- missing UGC safeguard or operational response owner; or
- missing required human/hardware approvals.

Tooling must never:

- copy credentials or signing material into the repository;
- copy store keys into persistent credential-discovery directories;
- expose credentials in arguments, logs, crash output, or long-lived daemons;
- silently upload;
- overwrite an accepted candidate;
- rebuild between beta acceptance and public promotion;
- mark a legal answer on the user's behalf; or
- report readiness with a blocking gate unresolved.

## Definition of done

### Release-kit implementation complete

The repository-owned kit is complete when:

1. Apple and Google metadata passes local validation and is ready for manual
   entry or later API automation.
2. All required store visuals exist, pass mechanical validation, and pass
   visual review.
3. Credential-free validation builds can be produced by documented commands,
   and credentialed candidate commands fail safely when credentials or human
   decisions are absent.
4. Public identifiers and versioning are consistent.
5. Privacy, policy, permissions, content-rating, review, and encryption
   worksheets are complete and evidence-backed.
6. All repository quality and coverage gates pass.
7. Candidate-manifest, ledger, status, upload, and promotion contracts pass
   their complete unit and integration fixture suites.
8. The TestFlight, Mac App Store beta/review, Play internal-test,
   device-rehearsal, and public-promotion steps are documented without
   requiring a rebuild.
9. The policy audit has either confirmed the required UGC safeguards or linked
   a separately approved blocking remediation workstream.

### Public release ready

Riot 1.0 is ready for staged public promotion only when:

1. Three separately identified signed candidates exist for iOS/iPadOS, macOS,
   and Android with immutable manifests and store upload identities.
2. Export compliance, store agreements, privacy answers, content ratings, UGC
   controls/operations, and public support contact are approved.
3. All beta, device, accessibility, offline, persistence, and advertised
   hardware-pair journeys pass against those exact candidates.
4. Every candidate is in `beta-accepted` state with no blocking or unknown
   gate in the read-only status report.
5. The staged rollout, daily observation owner, halt thresholds, and withdrawal
   procedure are recorded.

## Explicit non-goals

- No Fastlane or store-API integration in this slice.
- No paid distribution, subscriptions, or in-app purchases.
- No localization beyond English (U.S.) for the initial release.
- No app-preview video.
- No source-wide Android package/namespace rename.
- No implementation of deferred private-group/MLS features.
- No claim that nearby sync is field-proven before the hardware gate passes.
