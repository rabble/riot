# Riot Public Store Release Kit

Status: exceptional revision 4 for design-review gate.

## Goal

Prepare Riot 1.0 as a free, worldwide public early-access release for:

- iPhone and iPad through the Apple App Store;
- macOS through the Mac App Store; and
- Android phones and tablets through Google Play.

The same candidate binaries must pass TestFlight and Google Play internal
testing before they are promoted to public distribution. The release kit must
generate every repository-owned artifact needed for submission, prepare and
validate explicit Console handoffs, and identify the legal, contractual,
hardware, signing, and authenticated Console actions that cannot be completed
from the repository.

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
   sign-off records for build, validation, Console handoff, hardware approval,
   and rollout.

The directory should be automation-ready without requiring Fastlane in this
slice. Canonical metadata uses JSON with a checked-in JSON Schema; human
worksheets use Markdown. Candidate manifests, visual provenance, build-number
allocations, and ledger events each have their own versioned JSON Schema.
Every durable schema has a fixed `$id`, explicit `schemaVersion`,
`additionalProperties: false`, canonical JSON serialization for digests,
globally unique operation IDs, and sequence/predecessor linkage; records are
validated before append and after readback.
Visual templates and their source captures produce deterministic output when
run with the pinned Node/npm and image-tool versions. One checked-in release
toolchain manifest centralizes every tool version and checksum. Signed Apple
archives and Android bundles are not byte-reproducible promises: each is built
once, treated as immutable, identified by its digest and signing identity, and
promoted without rebuilding.

Secrets, certificates, provisioning profiles, keystores, passwords, and
authenticated Console session data must remain outside git. Candidate-build
scripts may accept signing material from environment variables or explicitly
ignored local configuration. Signing commands verify restrictive file
permissions, never copy keys into persistent home-directory locations, never
place passwords in command arguments or logs, disable long-lived signing
daemons where applicable, clean unavoidable temporary files with traps, and
document revocation and rotation. Repository tooling never receives Apple or
Google store API credentials in this slice.

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
and is considered stale at handoff time: immediately before every Console
upload, the operator records a fresh readback from an authenticated Apple or
Google Console session. If the store maximum is at or above the reservation,
the candidate becomes `superseded` unless the existing store build can be
reconciled to the exact candidate under the human-Console evidence rules below.

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
OS/device class, orientation, locale, appearance, capability/journey claim ID,
source hash, synthetic fixture revision, and template version. Every generated
image is visually reviewed at its intended aspect ratio and at store-thumbnail
scale. The approval record names the provenance-manifest digest, reviewer,
timestamp, and both visual-review results.

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
allowed. Apple archive, export, and Console-handoff readiness all fail closed.
After approval, the selected value is injected during archive production and
verified in the archived plist for both Apple targets.

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
  later public review and release.
- Harden the existing release script so version, build, source state, archive,
  export, and Console-handoff intent are explicit. It stops after export and
  prints Xcode Organizer/App Store Connect handoff steps; it never uploads.
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
  signing key is unavailable, while refusing Console-handoff readiness.
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
5. Prepare and push a human-action intent to the authoritative release branch.
6. A named operator uploads iOS/iPadOS and macOS through Xcode Organizer/App
   Store Connect and uploads the Android App Bundle through Play Console.
   Repository tooling does not execute these mutations.
7. The operator and a second verifier record Console evidence and store-assigned
   identities in append-only events.
8. Run the platform and cross-device rehearsal against those exact store
   builds.
9. Append test and human approvals that reference the candidate-manifest
   digest.
10. A named operator submits each candidate for store review; a second verifier
    records submission and every asynchronous review state.
11. After the Console reports approved/ready for release, a named operator
    performs the explicit worldwide/full initial release; a second verifier
    records the public state.

Apple phased release and Google staged-percentage rollout are not used for
these first public versions: those controls apply to updates, and Google does
not offer percentage staging for an app's first production release. Riot 1.0
therefore uses a manual worldwide/full release on each platform after review
approval, followed by seven days of heightened observation. Later updates may
add a separately designed phased/staged contract.

### Manual Console operation contract

No repository command uploads, submits for review, releases, halts, withdraws,
or otherwise changes a store build in this slice. Those are explicit
`HUMAN ACTION` gates:

| Platform | Artifact handoff | Authenticated readback/evidence |
| --- | --- | --- |
| iOS/iPadOS | Xcode Organizer → App Store Connect/TestFlight | App Store Connect version/build, processed state, submission/build ID, timestamp, operator, and redacted screenshot/export |
| macOS | Xcode Organizer → App Store Connect | App Store Connect version/build, processed state, submission/build ID, timestamp, operator, and redacted screenshot/export |
| Android | signed `.aab` → Google Play Console internal testing | package/version name/version code, track/release ID, app-signing certificate fingerprint, state, timestamp, operator, and redacted screenshot/export |

The authoritative cross-checkout coordination domain is the fast-forward-only
remote branch `release/riot-1.0`. Before any Console mutation, the release
coordinator commits and pushes exactly one active intent event for the
candidate/platform/action. A non-fast-forward push fails; the actor fetches and
re-evaluates. No second intent is allowed until the first has a recorded
outcome. The Console itself is authoritative for what remote state exists; the
immutable local candidate manifest is authoritative for which artifact was
approved.

The checked-in release-role file names the release coordinator, Console
operators, evidence verifiers, policy approver, and hardware approver. The
Console operator and evidence verifier for one action must be different named
people. The protected release branch and Git-host authentication are the
authority for intent/outcome events; promotion evidence records the actor,
verifier, predecessor digest, and candidate-manifest digest.

The human handoff protocol is:

1. `prepare-handoff` validates the candidate and fresh Console maximum
   attestation, writes an operation UUID and expected remote identity, and
   requires that intent commit to land on `release/riot-1.0`.
2. The tool stops with `HUMAN ACTION` and prints the exact Console steps and
   evidence fields. It has no store credentials and performs no mutation.
3. Before touching the Console, the operator may cancel. The same operator and
   a distinct verifier must attest that no Console mutation was attempted;
   `cancelled-before-action` then returns to the prior stable state. This
   transition is forbidden once any Console interaction may have begun.
4. Otherwise, the operator performs the action once in the authenticated
   Console.
5. `record-console-outcome` validates operator-supplied evidence against the
   candidate; a second named verifier attests to the Console readback before
   the outcome commit lands on the release branch.
6. If the operator session is interrupted or the remote result is unclear, the
   state becomes `<action>-indeterminate`. No second action is permitted. The
   operator and verifier inspect the Console until it supplies authoritative
   positive evidence or a terminal negative outcome such as an explicit
   rejected/invalid/withdrawn record. An eventually consistent “not found” or
   absent row is never proof that the action did not occur.
7. Positive evidence appends `reconciled-success`. Terminal negative evidence
   appends `reconciled-failed`; an upload then requires a new build number,
   while submission or release actions may be retried only from the explicit
   state table.
   Ambiguous evidence remains `HUMAN ACTION` indefinitely.

### Candidate, review, and release state table

All transitions append schema-validated events; no manifest is edited.

| From | Event | To | Rule |
| --- | --- | --- | --- |
| `draft` | build succeeded | `built` | immutable manifest written |
| `draft` or `built` | discard before Console upload | `discarded` | terminal; number never reused |
| `built` | pushed upload intent | `upload-human-action` | one active remote-branch intent |
| `upload-human-action` | two-person `cancelled-before-action` | `built` | only when no Console interaction was attempted |
| `upload-human-action` | verified Console success | `uploaded` | store identity matches candidate |
| `upload-human-action` | terminal negative Console outcome | `rejected` | terminal; new build required |
| `upload-human-action` | interrupted/unclear | `upload-indeterminate` | no retry |
| `upload-indeterminate` | verified positive readback | `uploaded` | `reconciled-success` |
| `upload-indeterminate` | terminal negative readback | `rejected` | terminal; new build required |
| `built` | store maximum/collision invalidates number | `superseded` | allowed only before upload intent; terminal |
| `uploaded` | beta/device/policy gates pass | `beta-accepted` | exact store build tested |
| `uploaded` | store rejects candidate | `rejected` | terminal |
| `beta-accepted` or metadata-only `review-rejected` | pushed submit-for-review intent | `submission-human-action` | one Console action; binary unchanged |
| `submission-human-action` | two-person `cancelled-before-action` | prior stable state | only when no Console interaction was attempted |
| `submission-human-action` | verified submission accepted | `review-pending` | record platform remote status |
| `submission-human-action` | terminal negative Console outcome | `review-rejected` | classify metadata-only versus binary-affecting |
| `submission-human-action` | interrupted/unclear | `submission-indeterminate` | no retry |
| `submission-indeterminate` | verified positive readback | evidenced review state | `review-pending`, `approved-ready`, or `review-rejected` |
| `submission-indeterminate` | terminal negative readback | prior stable state | fresh intent allowed |
| `review-pending` | Console status remains waiting/in-review | `review-pending` | append status observation |
| `review-pending` | verified approval/ready status | `approved-ready` | eligible for initial release intent |
| `review-pending` | verified review rejection | `review-rejected` | record whether metadata-only or binary-affecting |
| `review-rejected` | binary/configuration change required | `rejected` | terminal; new build required |
| `approved-ready` | pushed worldwide/full release intent | `release-human-action` | initial release only |
| `release-human-action` | two-person `cancelled-before-action` | `approved-ready` | only when no Console interaction was attempted |
| `release-human-action` | verified worldwide/full release | `released-worldwide` | successful public state |
| `release-human-action` | terminal negative Console outcome | `approved-ready` | append failure; fresh intent allowed |
| `release-human-action` | interrupted/unclear | `release-indeterminate` | no retry |
| `release-indeterminate` | verified public readback | `released-worldwide` | `reconciled-success` |
| `release-indeterminate` | terminal negative readback | `approved-ready` | fresh release intent allowed |
| `released-worldwide` | pushed halt/withdraw intent | `withdraw-human-action` | emergency/manual action |
| `withdraw-human-action` | two-person `cancelled-before-action` | `released-worldwide` | only when no Console interaction was attempted |
| `withdraw-human-action` | verified unavailable/withdrawn | `withdrawn` | terminal for this candidate |
| `withdraw-human-action` | terminal negative Console outcome | `released-worldwide` | append failure; fresh intent allowed |
| `withdraw-human-action` | interrupted/unclear | `withdraw-indeterminate` | no retry until reconciled |
| `withdraw-indeterminate` | verified readback | evidenced public or withdrawn state | append actual state |

Apple evidence schemas enumerate `Waiting for Review`, `In Review`,
`Pending Developer Release`/approved-ready, rejected, ready/distributed, and
removed-from-sale equivalents. Google evidence schemas enumerate internal-test
availability, changes-in-review/review-pending, approved/ready, production
available, rejected, and unpublished equivalents. Each
platform/action-specific schema records the enumerated remote status and the
SHA-256 of a redacted Console screenshot or export.

A rejected or superseded build receives a new build number. Partial builds may
be discarded before Console upload but allocated numbers are never reused. A
stale manifest, changed artifact hash, different signing identity, missing
prior state, or invalid transition fails. Ledger appends use an exclusive
local lock, fsync, and atomic rename in addition to the remote-branch
fast-forward rule; interrupted writes recover the last complete event and
report the incomplete transition.

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
results, hardware results, human approvals, Console submission/release/
withdrawal receipts, remaining gates, approver/verifier identities, and the
SHA-256 of the complete immutable candidate manifest. Console-handoff readiness
verifies the store build identity, artifact signing certificate, and recorded
candidate-manifest digest before printing human steps.

The command surface stays deliberately separate:

- `status` is read-only and reports `PASS`, `BLOCKED`, or `HUMAN ACTION` for
  every gate with the failing file/value, expected value, and recovery command;
- `status --json` emits the same gate model for CI;
- `generate` writes deterministic metadata/visual output;
- `build` creates one named candidate;
- `prepare-handoff` persists a Console intent and prints human steps;
- `record-console-outcome` validates operator/verifier evidence; and
- no command mutates Apple or Google store state.

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
| State machine/ledger | invalid skip, duplicate transition, accepted-ID reuse, mutation from indeterminate state, and cancellation after possible Console action must fail | append one legal transition atomically | every legal/illegal edge and terminal state; cancellation-before-action with two people; upload/submission/review/release/withdraw states; partial write recovery; metadata-only versus binary rejection; rejected/discarded/superseded semantics |
| Process runner | dirty tree, nonzero command, timeout, and zero tests must fail | run one fake validation and normalize result | Cargo/XCTest/Gradle zero-test logs; signal; timeout; redacted stderr |
| Signing-credential guard | persistent copy, broad mode, logged secret, and absent credential must fail candidate signing | authorize one fake signed build using external material | `0600`/wrong modes; Apple signing identity; Android key/password; cleanup; redaction; `--no-daemon`; store API credential rejected |
| Export-compliance guard | unresolved or archived-value mismatch must block Apple archive/export/Console handoff | inject and verify one recorded decision | unresolved; exempt/non-exempt; iOS/Mac archived plist mismatch |
| Supply-chain guard | missing Gradle checksum/locks or changed lock/tool hash must fail | verify one pinned input set | Cargo/npm/Gradle/toolchain fixtures; advisory finding with/without disposition |
| Status reporter | mixed gates must never summarize as ready | render tri-state result and recovery action | PASS/BLOCKED/HUMAN ACTION; missing evidence; exact file/value diagnostics |
| Console handoff/evidence | any attempted store process invocation, missing pushed intent, wrong store ID, unaccepted candidate, unsafe cancellation, or blind retry after ambiguity must fail | persist intent, print steps, validate discriminated operator/verifier evidence | no store credential/process adapter; remote-ref CAS failure; distinct operator/verifier; cancellation before action; Apple/Google upload statuses; submit/review/approve/reject lifecycle; full initial release; withdraw; interruption; positive/terminal-negative/absent/ambiguous readback; eventual-consistency absence remains indeterminate |
| Privacy/policy consistency | store answer contradicting code/network/policy evidence must fail | compare one answer to evidence matrix | no collection; user-directed fetch; report transmission; missing UGC control; stale policy URL |
| iOS/iPadOS release configuration | wrong ID/version/icon, missing privacy manifest, unsafe export value, signing or entitlement mismatch must fail | validate one archive/config fixture | Debug versus Release; iPhone/iPad families; app icon; archived plist; Keychain group; unresolved export decision |
| macOS release configuration | ad-hoc candidate, wrong ID/version/icon, Intel claim, sandbox/entitlement or export mismatch must fail | validate one Mac archive/config fixture | local ad-hoc stays valid; App Store distribution; Apple-silicon/macOS 14 boundary; app icon; archived plist |
| Android release configuration | wrong application ID/version, missing icons, debug key, daemon/password exposure, or unsigned handoff must fail | validate one bundle/config fixture | `net.protest.riot`; adaptive/legacy/store icons; validation-only unsigned versus signed candidate; upload/app-signing fingerprints |
| Synthetic fixture contract | production identifier, non-deterministic value, or fixture unavailable on one platform must fail | load one shared synthetic release scenario | iOS/iPad/Mac/Android parity; fixed clock/IDs/content; no personal/location/notification data; production build exclusion outside capture mode |
| Native capture entry points | capture without candidate/fixture provenance or wrong device/orientation must fail | launch one platform capture test and emit provenance | iPhone portrait; iPad; Mac landscape; Android phone/tablet; locale/appearance/orientation; metadata stripping |
| Candidate journey rehearsal | missing, skipped, zero-test, dead-end recovery, or platform-only claim must fail | execute one scripted first-install-to-first-read journey | create/join/publish/sign/restart/offline; denied permission recovery; invalid join; no peers; all named device classes; advertised hardware pairs |

Golden image tests assert geometry/tokens and are supplemented by Playwright
captures plus human visual review; they do not substitute brittle byte-for-byte
PNG snapshots for semantic checks.

Implementation dependency order is fixed:

1. policy/privacy/export audit and versioned durable-record schemas;
2. shared synthetic fixtures and native configuration tests;
3. metadata/visual/config validators and generators;
4. build-number allocation, ledger, manifest, and status engine;
5. signing, export, and supply-chain guards;
6. credential-free then signed candidate builds;
7. native capture and journey rehearsal;
8. Console handoff/evidence reconciliation; and
9. beta acceptance, review submission, approval, and worldwide/full initial
   release handoffs.

No later work unit begins while an earlier blocking contract is red.

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

Before review submission and public release, every supported platform must
pass 100 percent of the scripted
first-install-to-first-read, create, publish/sign, restart/persist, denied
permission/recovery, and offline-local journeys on every named device class.
There must be zero critical crashes, hangs, data-loss events, privacy/policy
failures, or failed advertised hardware pairs.

Each approved 1.0 candidate is released worldwide/full because first-version
percentage phasing is unavailable. For seven days after each public release,
store diagnostics and the published support/report channel are reviewed daily.
The operator prepares a withdraw/halt `HUMAN ACTION` if crash-free sessions fall
below 99.5 percent on any platform, any critical data-loss/security/privacy
failure is confirmed, an advertised core journey is unavailable, or moderation
reports cannot meet the published response target. After seven stable days,
v1 exits heightened observation; ordinary support and store-diagnostic review
continue. A withdrawn candidate is fixed under a new build number rather than
overwritten or rebuilt in place.

## Failure behavior

Release commands fail closed with actionable diagnostics for:

- dirty or mismatched source state;
- missing generated Rust/UniFFI artifacts;
- wrong identifiers, versions, or build numbers;
- absent signing credentials for a signed candidate;
- invalid or incomplete metadata;
- missing or invalid visual assets;
- inconsistent privacy declarations;
- failed or empty test runs;
- candidate, signing-identity, toolchain, or store-build mismatch;
- invalid candidate-state transition or interrupted ledger append;
- unresolved export classification for an Apple archive/export/Console
  handoff;
- missing UGC safeguard or operational response owner; or
- missing required human/hardware approvals.

Tooling must never:

- copy credentials or signing material into the repository;
- accept or copy store API keys in this slice;
- expose credentials in arguments, logs, crash output, or long-lived daemons;
- silently upload;
- overwrite an accepted candidate;
- rebuild between beta acceptance and public release;
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
7. Candidate-manifest, ledger, status, Console-handoff, evidence, and
   submission/review/release/withdrawal state contracts pass their complete
   unit and integration fixture suites.
8. The TestFlight, Mac App Store beta/review, Play internal-test,
   device-rehearsal, review-submission, and worldwide/full public-release steps
   are documented without requiring a rebuild.
9. The policy audit has either confirmed the required UGC safeguards or linked
   a separately approved blocking remediation workstream.

### Public release ready

Riot 1.0 is ready for public review and release only when:

1. Three separately identified signed candidates exist for iOS/iPadOS, macOS,
   and Android with immutable manifests and store upload identities.
2. Export compliance, store agreements, privacy answers, content ratings, UGC
   controls/operations, and public support contact are approved.
3. All beta, device, accessibility, offline, persistence, and advertised
   hardware-pair journeys pass against those exact candidates.
4. Every candidate is at least `beta-accepted` before review submission and is
   in `approved-ready` before worldwide/full release, with no blocking or
   unknown gate in the read-only status report.
5. The worldwide/full initial-release action, seven-day observation owner,
   halt thresholds, and withdrawal procedure are recorded.

## Explicit non-goals

- No Fastlane or store-API integration in this slice.
- No paid distribution, subscriptions, or in-app purchases.
- No localization beyond English (U.S.) for the initial release.
- No app-preview video.
- No source-wide Android package/namespace rename.
- No implementation of deferred private-group/MLS features.
- No claim that nearby sync is field-proven before the hardware gate passes.
