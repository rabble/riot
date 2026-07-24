# Riot Public Store Release Kit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a repeatable, fail-closed release kit for Riot 1.0 public early
access that produces validated free/worldwide store metadata,
cinematic-overlay visual assets, signed-candidate handoffs, and evidence-backed
public-release readiness for iPhone, iPad, macOS, Android phones, and Android
tablets.

**Architecture:** Repository-owned Node ES modules validate deterministic source records, generate store artifacts, and maintain immutable candidate and Console-handoff ledgers without calling store APIs. Native platform projects provide release configuration, deterministic fixture/capture entry points, and exact-candidate rehearsal evidence; manual Xcode Organizer, App Store Connect, and Play Console actions remain authenticated human gates.

**Tech Stack:** Node 26 ESM with `node:test`, c8, Ajv 8.20.0, and Sharp 0.35.3; JSON Schema 2020-12 records; Rust 2021 workspace checks; Swift 6/SwiftUI/XCTest/XCUITest; Kotlin 2.2/Compose/JUnit/Android instrumentation; Xcode build/archive tooling; Gradle 9; Playwright; and CycloneDX npm 6.0.0.

---

**Source design:** `docs/superpowers/specs/2026-07-24-public-store-release-kit-design.md` at approved commit `5e039c2` plus action-precondition correction `42f7462`.

## Scope and decomposition rule

The design spans independently testable subsystems, so this master plan fixes
their interfaces and order. Each work unit gets a detailed TDD plan in
`docs/superpowers/plans/2026-07-24-public-store-release-kit-wuNNN-<slug>.md`
immediately before execution. Every detailed plan must pass the three-reviewer
plan gate before code changes begin. WU-000 through WU-010 are all required for
the repository-owned release kit; Console mutations and legal/account answers
remain human-only completion gates.

## Verified baseline

- Apple bundle ID is already `net.protest.riot`; iOS and macOS marketing
  versions are `0.1`, and `scripts/testflight-release.sh` derives a build number
  from commit count and can copy/upload an API key. Both behaviors violate the
  approved design.
- Android namespace and application ID are `org.riot.evidence`, version name is
  `0.1`, version code is `1`, and no release signing or launcher resources are
  configured.
- iOS has a 1024-pixel app icon. macOS has no app-icon asset set. Android has no
  `res/` launcher-icon tree or Play store icon.
- `package.json` covers only `scripts/web/**/*.mjs`; release modules do not yet
  exist. `.coverage-thresholds.json` sets the JS tooling floor to 100 percent.
- `marketing/privacy/index.html` and `marketing/releases/index.html` exist,
  along with current iPhone-oriented captures under
  `marketing/assets/screenshots/`.
- The checkout contains unrelated user edits. Every work unit stages exact
  pathspecs and never uses `git add -A`, `git reset`, `git checkout --`, or
  destructive cleanup.

## Execution isolation

Implementation runs in a dedicated worktree created from the approved-plan
commit with `superpowers:using-git-worktrees`, on branch
`codex/riot-public-store-release-kit`. Before every work-unit commit, compare
`git diff --name-status <wu-start-commit>` and `git diff
<wu-start-commit> -- <owned paths>` to the detailed plan's declared scope.
Never stage a pre-existing modification or an undeclared path. Integration
back to the user's dirty checkout happens only after a clean three-dot diff
review; if an implementation path overlaps a user-modified path, preserve both
versions and stop for a conflict decision rather than overwriting either.

## Stable repository interfaces

### Command surface

`scripts/release/cli.mjs` exposes only:

```text
node scripts/release/cli.mjs status [--json]
node scripts/release/cli.mjs generate
node scripts/release/cli.mjs allocate --platform <ios|macos|android> --build <n> --store-max <n>
node scripts/release/cli.mjs build --platform <ios|macos|android> --candidate <id>
node scripts/release/cli.mjs bind-submission --candidate <id> --revision <n>
node scripts/release/cli.mjs record-approval --candidate <id> --kind <beta|device|accessibility|policy|hardware> --evidence <path>
node scripts/release/cli.mjs evaluate-beta --candidate <id>
node scripts/release/cli.mjs prepare-handoff --candidate <id> --action <upload|submit|release|withdraw>
node scripts/release/cli.mjs record-console-outcome --operation <uuid> --evidence <path>
node scripts/release/cli.mjs record-observation --candidate <id> --day <1..7> --evidence <path>
node scripts/release/cli.mjs inspect-artifact --platform <ios|macos|android> --path <path>
```

No command receives an App Store Connect or Play API credential, and no
production module contains a store-upload process adapter.

### Durable records

All records use canonical JSON, schema version `1`, SHA-256 content digests,
RFC 3339 UTC timestamps injected in tests, and full Git commit IDs:

```text
release/source/                         human-maintained canonical inputs
release/generated/                      deterministic generated metadata/assets
release/schemas/                        JSON Schema 2020-12 contracts
release/ledger/allocations.jsonl        append-only build reservations
release/ledger/events.jsonl             append-only candidate/action events
release/candidates/<candidate-id>/      immutable candidate manifests
release/submissions/<candidate-id>/     immutable submission-package revisions
release/evidence/<operation-uuid>/      redacted Console and rehearsal evidence
release/roles.json                      authorized release roles
```

Generated screenshots and signed build products are ignored when private or
machine-specific; canonical public metadata, schemas, templates, synthetic
fixtures, icons, generated store assets, and redacted evidence are tracked.

### Candidate states

The implementation uses exactly the platform-specific transition table from
the approved design. Apple uses `submission-ready → submission-human-action →
review-pending → approved-ready → release-human-action →
released-worldwide`. Google uses `submission-ready →
production-human-action → review-pending → released-worldwide`, with no
invented first-release `approved-ready` or second publication action.

## File ownership map

| Area | Files created or modified | Responsibility |
| --- | --- | --- |
| Release schemas/core | `scripts/release/{cli,canonical-json,schema,records,fs-atomic,status}.mjs`, `scripts/release/test/*.test.mjs`, `release/schemas/*.schema.json`, `release/roles.json`, `release/toolchains.json` | canonical validation, durable records, fail-closed status, pinned toolchain authority |
| Policy/metadata | `release/source/{product,apple,google,privacy,policy,accessibility,claims}.json`, `release/generated/{apple,google,worksheets}/**`, `marketing/{privacy,releases,support,accessibility}/index.html`, mirrored `marketing/public/**` | factual listing copy and explicit evidence-backed worksheets |
| Visuals | `scripts/release/{visual-model,visual-render,visual-validate}.mjs`, tests, `release/source/visuals.json`, `release/generated/visuals/**`, `release/generated/visual-provenance.json` | cinematic-overlay templates/icons and final candidate-bound six-frame/five-device set |
| Candidate engine | `scripts/release/{allocation,candidate,state-machine,ledger}.mjs`, tests, `release/ledger/**`, `release/candidates/**`, `release/submissions/**` | build allocation, immutable candidates/packages, legal transitions |
| Process/security | `scripts/release/{process-runner,credential-guard,supply-chain,export-compliance}.mjs`, tests | redaction, signing preflight, SBOM/advisory/export gates |
| Console handoff | `scripts/release/{handoff,evidence,approvals,observation}.mjs`, tests, `release/source/{console-statuses,observation}.json` | upload/review/release/withdraw intents, independent attestations, beta approvals, reconciliation, seven-day observation |
| Apple native | `apps/ios/**`, `apps/macos/**`, `scripts/testflight-release.sh`, `scripts/release-apple.sh` | 1.0 config, privacy/export injection, Mac signing/icon, deterministic capture |
| Android native | `apps/android/**`, `scripts/release-android.sh` | `net.protest.riot`, 1.0 config, signing guard, icons, bundle/capture |
| Verification/CI | `scripts/green.sh`, `scripts/release/verify-all.sh`, `package.json`, `package-lock.json`, `.github/workflows/ci.yml`, `.gitignore` | all-platform checks, zero-test detection, 100% release-tool coverage |

## Work-unit arc

| WU | Title | Hard dependencies | Completion artifact |
| --- | --- | --- | --- |
| WU-000 | Policy, privacy, export, schemas, toolchains, and CLI skeleton | approved design | validated canonical source/worksheet tree and truthful stop gate |
| WU-001 | Shared synthetic fixture and native fixture contracts | WU-000 | deterministic cross-platform release scenario and native loader tests |
| WU-002 | Metadata, configuration, and visual validators/generators | WU-000, WU-001 | listings, public pages, renderer/templates, icons, and draft visuals |
| WU-003 | Allocation, immutable candidates, submission packages, and status | WU-000 | crash-safe append-only candidate engine |
| WU-004 | Signing, export, and supply-chain guards | WU-003 | fail-closed credentialed-build preflight |
| WU-005 | iOS/iPadOS and macOS candidates plus final Apple visuals | WU-002, WU-004 | validation builds and, with credentials, immutable signed Apple candidates |
| WU-006 | Android candidate plus final Google visuals | WU-002, WU-004 | validation bundle and, with credentials, immutable signed `.aab` candidate |
| WU-007 | Manual Console upload handoff and evidence | WU-003, WU-005, WU-006 | uploaded store identities or explicit human-action blockers |
| WU-008 | Exact-store-build beta, accessibility, device, and hardware approvals | WU-007 | accepted store-build rehearsal evidence and `beta-accepted` transitions |
| WU-009 | Review/release/withdrawal handoffs and observation | WU-008 | platform-correct public lifecycle plus seven-day evidence contract |
| WU-010 | Composite verification, CI, and operator runbook | WU-000..WU-009 | one status command and human handoff package |

## WU-000: Policy, privacy, export, schemas, toolchains, and CLI skeleton

**Files:**

- Create: `release/source/product.json`
- Create: `release/source/privacy.json`
- Create: `release/source/policy.json`
- Create: `release/source/accessibility.json`
- Create: `release/source/claims.json`
- Create: `release/source/export-compliance.json`
- Create: `release/source/account-gates.json`
- Create: `release/source/network-matrix.json`
- Create: `release/source/review-instructions.json`
- Create: `release/schemas/*.schema.json`
- Create: `release/toolchains.json`
- Create: `scripts/release/canonical-json.mjs`
- Create: `scripts/release/schema.mjs`
- Create: `scripts/release/records.mjs`
- Create: `scripts/release/policy.mjs`
- Create: `scripts/release/cli.mjs`
- Create: `scripts/release/test/{canonical-json,schema,records,policy,cli}.test.mjs`
- Generate: `release/generated/worksheets/{app-privacy,data-safety,permissions,required-reason-apis,content-rating,review-instructions,outbound-network,ugc-operations,account-gates,export-compliance,accessibility}.md`
- Modify: `package.json`
- Modify: `package-lock.json`

- [ ] Write RED tests for canonical key ordering, unknown/missing fields,
  malformed records, full-digest references, contradictory privacy claims,
  unresolved export classification, missing UGC controls, unsupported store
  claims, incomplete per-device accessibility evidence, missing tool
  version/checksum pins, and every required worksheet.
- [ ] Run
  `node --test scripts/release/test/canonical-json.test.mjs scripts/release/test/schema.test.mjs scripts/release/test/records.test.mjs scripts/release/test/policy.test.mjs scripts/release/test/cli.test.mjs`
  and verify failures are caused by missing release modules.
- [ ] Implement pure modules with injected filesystem, clock, and hash
  dependencies; reject unknown schema properties and truncated identifiers.
- [ ] Populate evidence-backed source records and explicit Markdown worksheets
  for Apple App Privacy, Google Data Safety, permission justifications,
  required-reason APIs/privacy manifests, content ratings, review
  instructions, the outbound-network matrix, UGC controls and 24/72-hour
  operations, account/trader/tax/banking agreements, export compliance, and
  device-specific accessibility answers. Record legal/account fields as
  `HUMAN ACTION`; never invent an answer.
- [ ] Create a tested CLI/package-script skeleton supporting `status`,
  `status --json`, and `generate`; later work units register commands through
  explicit imports without changing these diagnostics.
- [ ] Enforce the policy stop gate: if filtering, in-app content/author
  reporting, local blocking, moderator/tombstone handling, response ownership,
  or public contact is missing, status is `BLOCKED` and WU-003 candidate
  production cannot start. Product remediation requires a separate
  brainstormed, design-reviewed, TDD plan before its files enter scope.
- [ ] Run `npm run test:release:coverage` and require 100 percent lines,
  branches, functions, and statements for `scripts/release/**/*.mjs`.
- [ ] Commit exact WU-000 paths with
  `git commit -m "feat(release): add policy and schema foundation"`.

## WU-001: Shared synthetic fixture and native fixture contracts

**Files:**

- Create: `fixtures/release/riot-1.0-synthetic.json`
- Create: `apps/ios/Riot/Release/ReleaseFixture.swift`
- Create: `apps/ios/RiotTests/ReleaseFixtureTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`
- Create: `apps/macos/RiotTests/ReleaseFixtureTests.swift`
- Modify: `apps/android/app/build.gradle.kts`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/ReleaseFixture.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/ReleaseFixtureTest.kt`

- [ ] Write RED Swift and Kotlin tests that load the same fixed fixture and
  assert its schema version, fixed clock, full identifiers, six narrative
  states, no private/person/location/notification data, and byte-identical
  canonical fixture digest.
- [ ] Run the focused Apple and Android loader tests after
  `sh scripts/conference/build-native-core.sh`; verify failure is caused by the
  missing fixture/loaders.
- [ ] Implement minimal native read-only fixture decoders. Production launch
  cannot select the fixture; capture-only entry points arrive in WU-005/WU-006.
- [ ] Run the focused tests again and require the same digest on iOS, macOS,
  and Android.
- [ ] Commit exact WU-001 paths with
  `git commit -m "test(release): add shared synthetic fixture contract"`.

## WU-002: Metadata, configuration, and visual validators/generators

**Files:**

- Create: `release/source/apple/en-US.json`
- Create: `release/source/google/en-US.json`
- Create: `scripts/release/metadata.mjs`
- Create: `scripts/release/test/metadata.test.mjs`
- Generate: `release/generated/apple/en-US/*.txt`
- Generate: `release/generated/google/en-US/*.txt`
- Create: `marketing/support/index.html`
- Create: `marketing/accessibility/index.html`
- Create: `marketing/public/support/index.html`
- Create: `marketing/public/accessibility/index.html`
- Modify: `marketing/privacy/index.html`
- Modify: `marketing/public/privacy/index.html`
- Modify: `marketing/releases/index.html`
- Modify: `marketing/public/releases/index.html`
- Modify: `scripts/release/cli.mjs`
- Modify: `scripts/release/test/cli.test.mjs`

- [ ] Write RED boundary tests for every Apple and Google field, including exact
  limit and limit-plus-one Unicode cases, required URLs, free/worldwide
  availability, explicit public early-access positioning, 1.0 release notes,
  platform capability claims, and trust vocabulary.
- [ ] Run `node --test scripts/release/test/metadata.test.mjs` and verify the
  missing parser/generator causes failure.
- [ ] Implement deterministic generation from canonical JSON into
  platform-shaped text files plus a machine-readable field manifest.
- [ ] Add public support/report and accessibility pages, ensure the privacy and
  release pages use the same canonical facts, and mirror them under
  `marketing/public/`.
- [ ] Run `npm run release:generate`, `npm run test:release:coverage`, and the
  existing marketing page contract tests.

### WU-002 visual/configuration subtask

**Files:**

- Create: `release/source/visuals.json`
- Create: `scripts/release/configuration.mjs`
- Create: `scripts/release/{visual-model,visual-render,visual-validate,image-inspector}.mjs`
- Create: `scripts/release/test/{configuration,visual-model,visual-render,visual-validate}.test.mjs`
- Create: `release/generated/visuals/draft/**`
- Create: `release/generated/visual-draft-provenance.json`
- Create: `release/generated/icons/{macos,android}/**`
- Create: `release/generated/visuals/google/{play-icon-512.png,feature-graphic-1024x500.png}`

- [ ] Write RED tests for the complete six-frame/five-device matrix,
  platform-specific orientation, Nearby-to-Join fallback, source-candidate
  field validation, private-data tokens, EXIF stripping, alpha/dimension rules,
  overlay geometry, contrast, headline limits, and 320-pixel thumbnail
  legibility. Exact candidate binding is added when WU-005 and WU-006 replace
  draft sources with native candidate captures.
- [ ] Add configuration-validator fixtures proving the current `0.1`,
  Android `org.riot.evidence` application ID, missing Mac/Android icons, and
  absent privacy/release configuration report `BLOCKED` without making the
  WU-002 test suite red.
- [ ] Run the visual unit tests and verify they fail before the model,
  inspector, renderer, and source fixture exist.
- [ ] Implement a deterministic fixture and cinematic overlay renderer using
  checked-in Riot fonts/colors. Draft composition fixtures prove all 30
  geometries, but final store screenshots wait for WU-005/WU-006 native
  candidate captures and never contain redrawn controls.
- [ ] Generate Android adaptive/legacy launcher icons, a 512×512 Play icon, a
  1024×500 feature graphic, the macOS app-icon set, and draft compositions for
  each required store geometry.
- [ ] Run mechanical validation, then use Playwright/image inspection to review
  every template/icon/draft at native aspect ratio and thumbnail scale. Record
  the draft-provenance digest and reviewer.
- [ ] Commit exact WU-002 metadata, source, tests, public generated assets, and
  provenance with
  `git commit -m "feat(release): generate store metadata and visual system"`.

## WU-003: Allocation, immutable candidates, submissions, and status

**Files:**

- Create: `scripts/release/{fs-atomic,allocation,candidate,state-machine,ledger,status}.mjs`
- Create: `scripts/release/test/{fs-atomic,allocation,candidate,state-machine,ledger,status}.test.mjs`
- Create: `release/ledger/{allocations,events}.jsonl`
- Create: `release/candidates/.gitkeep`
- Create: `release/submissions/.gitkeep`
- Create: `release/roles.json`
- Modify: `scripts/release/cli.mjs`
- Modify: `scripts/release/test/cli.test.mjs`

- [ ] Write RED tests for local locking, fsync/rename recovery, explicit
  monotonic build numbers, store-maximum staleness, duplicate/cross-checkout
  reservations, manifest finalization only after signed artifact and
  candidate-bound visual hashes exist, immutable manifest hashes, immutable
  submission revisions, and every legal/illegal Apple/Google transition.
- [ ] Run the focused tests and verify the unimplemented modules fail.
- [ ] Implement pure state transitions and injected durable storage. Never
  derive a build number from Git count, never reuse a number, and never edit an
  existing candidate/package/event. A platform build remains `draft` until its
  artifact/signing identity, native icon hashes, final pre-upload screenshot
  hashes, and toolchain digest are present; only then does one atomic
  finalization write the immutable candidate manifest and enter `built`.
- [ ] Implement tri-state `PASS`/`BLOCKED`/`HUMAN ACTION` status output with
  exact failing JSON pointer, observed value, expected value, and recovery
  command; `--json` emits the same gate model.
- [ ] Run `npm run test:release:coverage` and concurrent temporary-directory
  integration tests.
- [ ] Commit exact WU-003 paths with
  `git commit -m "feat(release): add immutable candidate ledger"`.

## WU-004: Signing, export, and supply-chain guards

**Files:**

- Create: `scripts/release/{process-runner,credential-guard,export-compliance,supply-chain,sbom,artifact-inspection}.mjs`
- Create: `scripts/release/test/{process-runner,credential-guard,export-compliance,supply-chain,sbom,artifact-inspection}.test.mjs`
- Create: `scripts/release/test/fixtures/artifacts/{ios-archive,macos-archive,android-aab}/**`
- Modify: `release/source/export-compliance.json`
- Create: `release/source/security-exceptions.json`
- Modify: `release/toolchains.json`
- Create: `apps/android/gradle/verification-metadata.xml`
- Create: `apps/android/app/gradle.lockfile`
- Modify: `apps/android/gradle/wrapper/gradle-wrapper.properties`
- Modify: `apps/android/build.gradle.kts`
- Modify: `scripts/release/cli.mjs`
- Modify: `scripts/release/test/cli.test.mjs`
- Modify: `package.json`

- [ ] Write RED tests for nonzero/timeout/signal/zero-test processes, log
  redaction, missing/wrong-mode credentials, daemon/argument password exposure,
  archived export-value mismatch, missing dependency verification, changed
  tool/lock hashes, known-exploited or reachable-critical findings, and
  invalid/expired candidate-bound exceptions. Drift tests cover every pinned
  Rust/Swift/Xcode/Java/Android SDK/NDK/Gradle/Node/npm/Sharp/Ajv/CycloneDX
  version and checksum in the single checked-in toolchain manifest.
- [ ] Add archive/bundle fixture inspection cases for identifier, marketing and
  build version, signing certificate fingerprint, entitlements, privacy/export
  values, packaged icons, ABI/architecture, and candidate artifact digest.
- [ ] Run focused tests and verify failures precede implementation.
- [ ] Implement guards with injected process/environment/filesystem adapters.
  Accept credential file paths only after permission checks; never copy
  credentials into the repository or persistent home directories.
- [ ] Pin and verify Gradle distribution/dependencies and emit deterministic
  candidate-bound CycloneDX SBOM records for Rust, Apple, and Android inputs.
  Enable `lockAllConfigurations()` in the root Android build, bootstrap
  `gradle.lockfile` once with `--write-locks` under review, then prove normal
  and final verification use the checked-in locks and strict verification
  metadata without either write flag.
- [ ] Run 100 percent release-tool coverage plus safe fake-credential
  integration fixtures.
- [ ] Commit exact WU-004 paths with
  `git commit -m "feat(release): enforce signing and supply-chain gates"`.

## WU-005: iOS/iPadOS and macOS candidates plus final Apple visuals

**Files:**

- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/ios/Riot/Info.plist`
- Modify: `apps/ios/Riot/Riot.entitlements`
- Modify: `apps/ios/Riot/RiotApp.swift`
- Create: `apps/ios/Riot/PrivacyInfo.xcprivacy`
- Create: `apps/ios/Riot/Release/ReleaseCaptureMode.swift`
- Modify: `apps/ios/ExportOptions.plist`
- Create: `apps/ios/RiotTests/ReleaseConfigurationTests.swift`
- Create: `apps/ios/RiotUITests/ReleaseCaptureUITests.swift`
- Create: `apps/ios/RiotUITests/ReleaseJourneyUITests.swift`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot.xcodeproj/xcshareddata/xcschemes/Riot-macOS.xcscheme`
- Modify: `apps/macos/Riot/Info.plist`
- Modify: `apps/macos/Riot/Riot.entitlements`
- Modify: `apps/macos/Riot/RiotMacApp.swift`
- Create: `apps/macos/Riot/PrivacyInfo.xcprivacy`
- Create: `apps/macos/Riot/Assets.xcassets/AppIcon.appiconset/**`
- Create: `apps/macos/RiotTests/ReleaseConfigurationTests.swift`
- Create: `apps/macos/RiotUITests/ReleaseCaptureUITests.swift`
- Create: `apps/macos/RiotUITests/ReleaseJourneyUITests.swift`
- Generate: `release/generated/visuals/{iphone,ipad,mac}/**`
- Create: `release/generated/visual-provenance-apple.json`
- Rewrite: `scripts/testflight-release.sh`
- Create: `scripts/release-apple.sh`

- [ ] Write RED native/configuration tests for marketing version `1.0`, explicit
  build injection, `net.protest.riot`, iPhone/iPad families, Mac Apple-silicon
  and macOS 14 boundary, app icons, privacy manifests, sandbox/Keychain
  entitlements, export injection, candidate/capture arguments, iOS UI-test
  membership, a macOS UI-test target included by the shared `Riot-macOS`
  scheme, and zero-test rejection.
- [ ] Run the focused XCTest/config validators and verify expected failures.
- [ ] Implement separate validation and credentialed candidate paths.
  `scripts/testflight-release.sh` becomes a compatibility wrapper that stops
  after archive/export and prints Xcode Organizer steps; remove API-key upload,
  home-directory copying, commit-count build numbers, and `ALLOW_DIRTY`.
- [ ] Add deterministic capture-only launch arguments backed by the WU-001
  fixture. In one sealed candidate transaction, produce fixture-enabled capture
  builds and the distribution archives from the same clean commit, native-core
  inputs, version/build, configuration, and generated sources. Capture the
  six-frame iPhone, iPad, and Mac sequences, compose/validate the overlays, and
  bind both capture-build and visual hashes beside the signed archive hash in
  each candidate manifest. Ordinary production launch ignores/rejects capture
  mode; WU-008 later confirms the processed store build identity.
- [ ] Wire capture launch handling through `RiotApp.swift` and
  `RiotMacApp.swift`; add every declared UI-test file to its target and make the
  shared `Riot` and `Riot-macOS` schemes execute those targets.
- [ ] Build iOS simulator/device-validation and macOS validation artifacts; when
  signing is unavailable, verify candidate handoff remains `BLOCKED` rather
  than silently using ad-hoc output.
- [ ] Run iOS/macOS tests and config/archive inspectors.
- [ ] Commit exact WU-005 paths with
  `git commit -m "feat(release): configure Apple store candidates"`.

## WU-006: Android candidate plus final Google visuals

**Files:**

- Modify: `apps/android/app/build.gradle.kts`
- Modify: `apps/android/settings.gradle.kts`
- Modify: `apps/android/app/src/main/AndroidManifest.xml`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Create: `apps/android/app/src/main/res/mipmap-*/**`
- Create: `apps/android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml`
- Create: `apps/android/app/src/main/res/values/{colors,strings}.xml`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/ReleaseCaptureMode.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/ReleaseConfigurationTest.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/ReleaseCaptureTest.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/ReleaseJourneyTest.kt`
- Create: `scripts/release-android.sh`
- Generate: `release/generated/visuals/{android-phone,android-tablet}/**`
- Create: `release/generated/visual-provenance-android.json`

- [ ] Write RED tests for public application ID `net.protest.riot`, retained
  internal namespace `org.riot.evidence`, version name `1.0`, explicit positive
  version code, adaptive/legacy/store icons,
  release signing identity, external secret delivery, Play upload/app-signing
  fingerprints, unsigned validation mode, and signed handoff refusal.
- [ ] Run Gradle unit/configuration tests and verify expected failures.
- [ ] Change only the public Android `applicationId` to `net.protest.riot`;
  retain the internal Kotlin/Gradle namespace to avoid a source-wide rename.
  Configure external upload-key signing with non-argument secrets and
  `--no-daemon`, and generate a release `.aab`. Keep the debug/validation build
  available without signing material.
- [ ] In one sealed candidate transaction, produce fixture-enabled capture
  builds and the upload-signed App Bundle from the same clean commit,
  native-core inputs, version code/name, configuration, and generated sources.
  Capture six-frame phone and tablet sequences, compose/validate overlays, and
  bind capture-build, visual, and `.aab` hashes in the candidate manifest.
  Capture mode is accepted only by the instrumentation/candidate harness, never
  an ordinary production launch; WU-008 later confirms the Play-processed
  app-signing identity.
- [ ] Run unit tests, lint, dependency verification, `bundleRelease` validation,
  and artifact inspection. A missing key must yield `HUMAN ACTION` or
  `BLOCKED`, never a debug-signed candidate.
- [ ] Commit exact WU-006 paths with
  `git commit -m "feat(release): configure Android Play candidate"`.

## WU-007: Manual Console upload handoff and evidence

**Files:**

- Create: `release/source/console-statuses.json`
- Create: `scripts/release/handoff.mjs`
- Create: `scripts/release/evidence.mjs`
- Create: `scripts/release/test/handoff.test.mjs`
- Create: `scripts/release/test/evidence.test.mjs`
- Create: `release/evidence/.gitkeep`
- Modify: `scripts/release/cli.mjs`

- [ ] Write RED tests for missing pushed upload intent, non-fast-forward
  predecessor, stale store maximum, expected ID/version/build mismatch,
  artifact/signing/candidate digest mismatch, same-identity attestations,
  self-authorizing role changes, unsafe cancellation, direct negative outcome,
  interruption, eventual-consistency absence, and indeterminate upload replay.
- [ ] Run focused tests and verify failure before handoff/evidence modules
  exist.
- [ ] Implement credential-free upload instructions and immutable evidence
  validation. Upload binds expected ID/version/build, artifact, signing
  identity, candidate digest, and fresh authenticated store maximum, then
  records the store-assigned identity after two independently authenticated
  attestations.
- [ ] Implement `cancelled-before-action` and fail-closed reconciliation. No
  test or production adapter may execute `xcodebuild -upload`, `altool`,
  Transporter, Play Publisher API, or browser automation against a store.
- [ ] Run the upload state/evidence suite at 100 percent JS coverage. The real
  candidate remains `HUMAN ACTION` until an operator uploads it and a distinct
  verifier records the Console readback.
- [ ] Commit exact WU-007 paths with
  `git commit -m "feat(release): add manual Console upload handoff"`.

## WU-008: Exact-store-build beta, accessibility, device, and hardware approvals

**Files:**

- Create: `release/source/journeys.json`
- Create: `release/source/devices.json`
- Create: `scripts/release/{journey,accessibility,rehearsal,approvals}.mjs`
- Create: `scripts/release/test/{journey,accessibility,rehearsal,approvals}.test.mjs`
- Modify: `apps/ios/Riot/Release/ReleaseCaptureMode.swift`
- Modify: `apps/ios/RiotUITests/ReleaseCaptureUITests.swift`
- Modify: `apps/ios/RiotUITests/ReleaseJourneyUITests.swift`
- Create: `apps/ios/RiotUITests/ReleaseAccessibilityUITests.swift`
- Modify: `apps/macos/RiotUITests/ReleaseCaptureUITests.swift`
- Modify: `apps/macos/RiotUITests/ReleaseJourneyUITests.swift`
- Create: `apps/macos/RiotUITests/ReleaseAccessibilityUITests.swift`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/ReleaseCaptureMode.kt`
- Modify: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/ReleaseCaptureTest.kt`
- Modify: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/ReleaseJourneyTest.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/ReleaseAccessibilityTest.kt`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`
- Generate: `release/evidence/rehearsals/**`
- Modify: `scripts/release/cli.mjs`
- Modify: `scripts/release/test/cli.test.mjs`

- [ ] Write RED tests for missing/skipped/zero-test journeys, wrong candidate,
  fixture drift, invalid join/no-peer dead ends, restart/offline failures,
  unadvertised capability claims, semantic labels/roles/states, reading/focus
  order, focus traps, contrast, target size, text scaling, and missing
  VoiceOver/TalkBack/Mac-keyboard human records. Approval tests require
  separately identified beta, device, accessibility, policy, and hardware
  evidence before `uploaded → beta-accepted`.
- [ ] Run focused Node and native tests and verify failures before entry points
  and schemas exist.
- [ ] Install/download the Console-assigned beta builds and run the common-task
  matrix on iPhone, iPad, Mac, Android phone, and Android tablet. Run required
  physical Nearby pairings before retaining a frame-5 Nearby claim. Record
  exact store identity, candidate/device/OS, test counts, and evidence digests.
- [ ] Verify the installed store build's ID/version/build/signing identity
  matches the immutable candidate whose pre-upload screenshots and visual
  provenance are already hashed. A mismatch rejects the candidate; screenshots
  are never silently regenerated from a different build.
- [ ] Implement `record-approval` and `evaluate-beta`; append
  `beta-accepted` only when all required independently attributable records
  pass. Missing physical devices or Console builds remain `HUMAN ACTION`.
- [ ] Commit exact WU-008 source and redacted evidence with
  `git commit -m "test(release): record candidate rehearsal evidence"`.

## WU-009: Review/release/withdrawal handoffs and observation

**Files:**

- Create: `release/source/observation.json`
- Create: `scripts/release/observation.mjs`
- Create: `scripts/release/test/observation.test.mjs`
- Modify: `scripts/release/handoff.mjs`
- Modify: `scripts/release/evidence.mjs`
- Modify: `scripts/release/test/handoff.test.mjs`
- Modify: `scripts/release/test/evidence.test.mjs`
- Modify: `scripts/release/cli.mjs`
- Generate: `release/generated/observation-checklist.md`

- [ ] Write RED tests for immutable initial/remediated submission packages,
  post-upload package/store-identity binding, Apple submit/review/approve/manual
  release, Google first-production/review/automatic publication, withdrawal or
  unpublish, cancellation, direct negative outcomes, interruption,
  eventual-consistency absence, indeterminate replay, and candidate/package
  substitution.
- [ ] Write RED observation tests for a named owner, exactly seven dated daily
  diagnostic/support reviews, crash-free sessions below 99.5 percent, any
  critical data-loss/security/privacy event, unavailable advertised journey,
  missed moderation SLA, and mandatory withdrawal/unpublish preparation.
- [ ] Run focused tests and verify failure before observation and the extended
  post-upload transition handlers exist.
- [ ] Extend credential-free action-specific instructions and evidence
  validation. Every post-upload intent, both attestations, evidence record, and
  outcome bind the assigned store identity, candidate digest, and active
  submission-package revision/digest.
- [ ] Implement separately authenticated operator/verifier attestations,
  platform-discriminated reconciliation, and the durable seven-day observation
  ledger/checklist. No command mutates a store.
- [ ] Run the complete state/evidence suite at 100 percent JS coverage.
- [ ] Commit exact WU-009 paths with
  `git commit -m "feat(release): add review and observation handoffs"`.

## WU-010: Composite verification, CI, and operator runbook

**Files:**

- Create: `scripts/release/verify-all.sh`
- Create: `release/README.md`
- Create: `release/OPERATOR-RUNBOOK.md`
- Create: `release/generated/release-checklist.md`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `scripts/green.sh`
- Modify: `scripts/ios-check.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `.gitignore`
- Modify: `scripts/release/cli.mjs`

- [ ] Write RED CLI/integration tests for all commands, structured diagnostics,
  dirty-tree scope, generated-file drift, zero tests, missing platform tools,
  absent signing/human decisions, and a complete synthetic green fixture.
- [ ] Run CLI tests and verify missing orchestration fails.
- [ ] Wire the stable command surface, deterministic generation checks,
  platform validation lanes, coverage enforcement from
  `.coverage-thresholds.json`, secret scanning, SBOM/advisory checks, and
  visual/candidate/evidence status into one read-only report.
- [ ] Add CI jobs that can validate credential-free outputs and fail on
  generated drift without requiring Apple/Google credentials or physical
  hardware. Native signed archives, hardware rehearsals, legal answers, and
  Console actions remain explicit `HUMAN ACTION`.
- [ ] Extend `scripts/ios-check.sh` with deterministic
  available-simulator-ID resolution for both iPhone and iPad. The composite
  runner executes the `Riot` UI-test scheme on both IDs and the `Riot-macOS`
  UI-test target in addition to portable unit schemes, rejecting zero-test
  logs.
- [ ] Run the complete verification matrix below and record exact command
  results in the generated checklist.
- [ ] Commit exact WU-010 paths with
  `git commit -m "ci(release): enforce public store readiness"`.

## Final verification matrix

Run from a clean commit containing all reservations and generated outputs:

```bash
npm ci
npm run release:generate
git diff --exit-code -- release/generated
npm run test:release:coverage
cargo fmt --all -- --check
cargo check --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
sh scripts/web/coverage.sh
sh scripts/conference/build-native-core.sh
RIOT_IOS_SIMULATOR_ID="$(sh scripts/ios-check.sh simulator-id)"
RIOT_IPAD_SIMULATOR_ID="$(sh scripts/ios-check.sh ipad-simulator-id)"
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination "platform=iOS Simulator,id=${RIOT_IOS_SIMULATOR_ID}"
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination "platform=iOS Simulator,id=${RIOT_IOS_SIMULATOR_ID}"
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination "platform=iOS Simulator,id=${RIOT_IPAD_SIMULATOR_ID}"
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS'
(cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 \
  ./gradlew --no-daemon --dependency-verification strict \
  :app:testDebugUnitTest :app:lint :app:bundleRelease \
  :app:assembleDebugAndroidTest :app:connectedDebugAndroidTest)
npm run release:inspect-artifact-fixtures
sh scripts/release/verify-all.sh
node scripts/release/cli.mjs status
```

Expected repository result: all credential-free checks pass; missing signing
materials, Apple export decision, account agreements, physical-device records,
and Console actions appear as `HUMAN ACTION` or `BLOCKED` with recovery
instructions. They never appear as `PASS`.

For each real signed candidate, the operator also runs the explicit inspector
before upload:

```bash
node scripts/release/cli.mjs inspect-artifact --platform ios --path "$RIOT_IOS_ARCHIVE"
node scripts/release/cli.mjs inspect-artifact --platform macos --path "$RIOT_MACOS_ARCHIVE"
node scripts/release/cli.mjs inspect-artifact --platform android --path "$RIOT_ANDROID_AAB"
```

The Android connected test command requires a running API 36 emulator/device.
Physical Nearby, VoiceOver/TalkBack, signed archive inspection, and Console
steps are recorded as human evidence and cannot be replaced by CI.

## Final review and handoff

- Run `superpowers:requesting-code-review` over the complete diff.
- Run the repository coverage gate from `.coverage-thresholds.json`; never
  lower a floor to make the change pass.
- Visually review every generated store asset at native and thumbnail size.
- Verify no secrets, production/private fixture data, build products, or
  unredacted Console captures are tracked.
- Prepare the manual Xcode Organizer, App Store Connect, and Play Console
  handoff from immutable candidates. Do not claim the apps were uploaded,
  reviewed, or published until authenticated evidence is recorded.
