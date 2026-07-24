# Riot Public Store Release Kit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a repeatable, fail-closed release kit that produces validated Riot 1.0 store metadata, cinematic-overlay visual assets, signed-candidate handoffs, and evidence-backed public-release readiness for iPhone, iPad, macOS, Android phones, and Android tablets.

**Architecture:** Repository-owned Node ES modules validate deterministic source records, generate store artifacts, and maintain immutable candidate and Console-handoff ledgers without calling store APIs. Native platform projects provide release configuration, deterministic fixture/capture entry points, and exact-candidate rehearsal evidence; manual Xcode Organizer, App Store Connect, and Play Console actions remain authenticated human gates.

**Tech Stack:** Node 26 ESM with `node:test`, c8, Ajv 8.20.0, and Sharp 0.35.3; JSON Schema 2020-12 records; Rust 2021 workspace checks; Swift 6/SwiftUI/XCTest/XCUITest; Kotlin 2.2/Compose/JUnit/Android instrumentation; Xcode build/archive tooling; Gradle 9; Playwright; and CycloneDX npm 6.0.0.

---

**Source design:** `docs/superpowers/specs/2026-07-24-public-store-release-kit-design.md` at approved commit `5e039c2` plus action-precondition correction `42f7462`.

## Scope and decomposition rule

The design spans independently testable subsystems, so this master plan fixes
their interfaces and order. Each work unit gets a detailed TDD plan in
`docs/superpowers/plans/2026-07-24-public-store-release-kit-wuNNN-<slug>.md`
immediately before execution. Every detailed plan must pass the three-reviewer
plan gate before code changes begin. WU-000 through WU-009 are all required for
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

## Stable repository interfaces

### Command surface

`scripts/release/cli.mjs` exposes only:

```text
node scripts/release/cli.mjs status [--json]
node scripts/release/cli.mjs generate
node scripts/release/cli.mjs allocate --platform <ios|macos|android> --build <n> --store-max <n>
node scripts/release/cli.mjs build --platform <ios|macos|android> --candidate <id>
node scripts/release/cli.mjs bind-submission --candidate <id> --revision <n>
node scripts/release/cli.mjs prepare-handoff --candidate <id> --action <upload|submit|release|withdraw>
node scripts/release/cli.mjs record-console-outcome --operation <uuid> --evidence <path>
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
| Release schemas/core | `scripts/release/{canonical-json,schema,records,fs-atomic,status}.mjs`, `scripts/release/test/*.test.mjs`, `release/schemas/*.schema.json`, `release/roles.json` | canonical validation, durable records, fail-closed status |
| Policy/metadata | `release/source/{product,apple,google,privacy,policy,accessibility,claims}.json`, `release/generated/{apple,google}/**`, `marketing/{privacy,releases,support,accessibility}/index.html`, mirrored `marketing/public/**` | factual listing copy and evidence-backed worksheets |
| Visuals | `scripts/release/{visual-model,visual-render,visual-validate}.mjs`, tests, `release/source/visuals.json`, `release/generated/visuals/**`, `release/generated/visual-provenance.json` | cinematic-overlay templates/icons and final candidate-bound six-frame/five-device set |
| Candidate engine | `scripts/release/{allocation,candidate,state-machine,ledger}.mjs`, tests, `release/ledger/**`, `release/candidates/**`, `release/submissions/**` | build allocation, immutable candidates/packages, legal transitions |
| Process/security | `scripts/release/{process-runner,credential-guard,supply-chain,export-compliance}.mjs`, tests | redaction, signing preflight, SBOM/advisory/export gates |
| Console handoff | `scripts/release/{handoff,evidence}.mjs`, tests, `release/source/console-statuses.json` | action-specific intents, independent attestations, reconciliation |
| Apple native | `apps/ios/**`, `apps/macos/**`, `scripts/testflight-release.sh`, `scripts/release-apple.sh` | 1.0 config, privacy/export injection, Mac signing/icon, deterministic capture |
| Android native | `apps/android/**`, `scripts/release-android.sh` | `net.protest.riot`, 1.0 config, signing guard, icons, bundle/capture |
| Verification/CI | `scripts/green.sh`, `scripts/release/verify-all.sh`, `package.json`, `package-lock.json`, `.github/workflows/ci.yml`, `.gitignore` | all-platform checks, zero-test detection, 100% release-tool coverage |

## Work-unit arc

| WU | Title | Hard dependencies | Completion artifact |
| --- | --- | --- | --- |
| WU-000 | Policy, privacy, accessibility, and schema foundation | approved design | validated canonical source tree and truthful blocking worksheet |
| WU-001 | Metadata generator and public support pages | WU-000 | complete Apple/Google English (U.S.) listing packages |
| WU-002 | Visual model, synthetic fixture, templates, and icons | WU-000, WU-001 | validated renderer/templates, icons, feature graphic, and draft visual fixtures |
| WU-003 | Allocation, immutable candidates, submission packages, and status | WU-000 | crash-safe append-only candidate engine |
| WU-004 | Signing, export, and supply-chain guards | WU-003 | fail-closed credentialed-build preflight |
| WU-005 | iOS/iPadOS and macOS release configuration | WU-001, WU-002, WU-004 | validation builds and credentialed archive handoff |
| WU-006 | Android release configuration | WU-001, WU-002, WU-004 | validation bundle and credentialed `.aab` handoff |
| WU-007 | Exact-candidate screenshots, accessibility, and device rehearsal | WU-002, WU-005, WU-006 | final validated 30-cell screenshot/provenance set and signed rehearsal records |
| WU-008 | Manual Console intent/evidence state machine | WU-003, WU-007 | upload/review/release/withdraw handoff and reconciliation |
| WU-009 | Composite verification, CI, and operator runbook | WU-000..WU-008 | one status command and human handoff package |

## WU-000: Policy, privacy, accessibility, and schema foundation

**Files:**

- Create: `release/source/product.json`
- Create: `release/source/privacy.json`
- Create: `release/source/policy.json`
- Create: `release/source/accessibility.json`
- Create: `release/source/claims.json`
- Create: `release/schemas/*.schema.json`
- Create: `scripts/release/canonical-json.mjs`
- Create: `scripts/release/schema.mjs`
- Create: `scripts/release/records.mjs`
- Create: `scripts/release/test/{canonical-json,schema,records,policy}.test.mjs`
- Modify: `package.json`
- Modify: `package-lock.json`

- [ ] Write RED tests for canonical key ordering, unknown/missing fields,
  malformed records, full-digest references, contradictory privacy claims,
  unresolved export classification, missing UGC controls, unsupported store
  claims, and incomplete per-device accessibility evidence.
- [ ] Run
  `node --test scripts/release/test/canonical-json.test.mjs scripts/release/test/schema.test.mjs scripts/release/test/records.test.mjs scripts/release/test/policy.test.mjs`
  and verify failures are caused by missing release modules.
- [ ] Implement pure modules with injected filesystem, clock, and hash
  dependencies; reject unknown schema properties and truncated identifiers.
- [ ] Populate evidence-backed source records from code/network inspection.
  Record legal/account fields as `HUMAN ACTION`; never invent an answer.
- [ ] Run `npm run test:release:coverage` and require 100 percent lines,
  branches, functions, and statements for `scripts/release/**/*.mjs`.
- [ ] Commit exact WU-000 paths with
  `git commit -m "feat(release): add policy and schema foundation"`.

## WU-001: Metadata generator and public support pages

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

- [ ] Write RED boundary tests for every Apple and Google field, including exact
  limit and limit-plus-one Unicode cases, required URLs, free/worldwide
  availability, 1.0 release notes, platform capability claims, and trust
  vocabulary.
- [ ] Run `node --test scripts/release/test/metadata.test.mjs` and verify the
  missing parser/generator causes failure.
- [ ] Implement deterministic generation from canonical JSON into
  platform-shaped text files plus a machine-readable field manifest.
- [ ] Add public support/report and accessibility pages, ensure the privacy and
  release pages use the same canonical facts, and mirror them under
  `marketing/public/`.
- [ ] Run `npm run release:generate`, `npm run test:release:coverage`, and the
  existing marketing page contract tests.
- [ ] Commit exact WU-001 paths with
  `git commit -m "feat(release): generate store metadata and support pages"`.

## WU-002: Visual model, synthetic fixture, templates, and icons

**Files:**

- Create: `release/source/visuals.json`
- Create: `fixtures/release/riot-1.0-synthetic.json`
- Create: `scripts/release/{visual-model,visual-render,visual-validate,image-inspector}.mjs`
- Create: `scripts/release/test/{visual-model,visual-render,visual-validate}.test.mjs`
- Create: `release/generated/visuals/draft/**`
- Create: `release/generated/visual-draft-provenance.json`
- Modify: Apple asset catalogs and Android `res/mipmap-*`, `res/drawable-*`, and `res/values/colors.xml`

- [ ] Write RED tests for the complete six-frame/five-device matrix,
  platform-specific orientation, Nearby-to-Join fallback, source-candidate
  field validation, private-data tokens, EXIF stripping, alpha/dimension rules,
  overlay geometry, contrast, headline limits, and 320-pixel thumbnail
  legibility. Exact candidate binding is added when WU-007 replaces draft
  sources with native candidate captures.
- [ ] Run the visual unit tests and verify they fail before the model,
  inspector, renderer, and source fixture exist.
- [ ] Implement a deterministic fixture and cinematic overlay renderer using
  checked-in Riot fonts/colors. Draft composition fixtures prove all 30
  geometries, but final store screenshots wait for WU-007 native candidate
  captures and never contain redrawn controls.
- [ ] Generate Android adaptive/legacy launcher icons, a 512×512 Play icon, a
  1024×500 feature graphic, the macOS app-icon set, and draft compositions for
  each required store geometry.
- [ ] Run mechanical validation, then use Playwright/image inspection to review
  every template/icon/draft at native aspect ratio and thumbnail scale. Record
  the draft-provenance digest and reviewer.
- [ ] Commit exact WU-002 source, tests, public generated assets, and provenance
  with `git commit -m "feat(release): add validated store visual system"`.

## WU-003: Allocation, immutable candidates, submissions, and status

**Files:**

- Create: `scripts/release/{fs-atomic,allocation,candidate,state-machine,ledger,status}.mjs`
- Create: `scripts/release/test/{fs-atomic,allocation,candidate,state-machine,ledger,status}.test.mjs`
- Create: `release/ledger/{allocations,events}.jsonl`
- Create: `release/candidates/.gitkeep`
- Create: `release/submissions/.gitkeep`
- Create: `release/roles.json`

- [ ] Write RED tests for local locking, fsync/rename recovery, explicit
  monotonic build numbers, store-maximum staleness, duplicate/cross-checkout
  reservations, immutable manifest hashes, immutable submission revisions, and
  every legal/illegal Apple/Google transition.
- [ ] Run the focused tests and verify the unimplemented modules fail.
- [ ] Implement pure state transitions and injected durable storage. Never
  derive a build number from Git count, never reuse a number, and never edit an
  existing candidate/package/event.
- [ ] Implement tri-state `PASS`/`BLOCKED`/`HUMAN ACTION` status output with
  exact failing JSON pointer, observed value, expected value, and recovery
  command; `--json` emits the same gate model.
- [ ] Run `npm run test:release:coverage` and concurrent temporary-directory
  integration tests.
- [ ] Commit exact WU-003 paths with
  `git commit -m "feat(release): add immutable candidate ledger"`.

## WU-004: Signing, export, and supply-chain guards

**Files:**

- Create: `scripts/release/{process-runner,credential-guard,export-compliance,supply-chain,sbom}.mjs`
- Create: `scripts/release/test/{process-runner,credential-guard,export-compliance,supply-chain,sbom}.test.mjs`
- Create: `release/source/export-compliance.json`
- Create: `release/source/security-exceptions.json`
- Modify/Create: `apps/android/gradle/verification-metadata.xml`
- Create: `apps/android/gradle.lockfile`
- Modify: `apps/android/gradle/wrapper/gradle-wrapper.properties`

- [ ] Write RED tests for nonzero/timeout/signal/zero-test processes, log
  redaction, missing/wrong-mode credentials, daemon/argument password exposure,
  archived export-value mismatch, missing dependency verification, changed
  tool/lock hashes, known-exploited or reachable-critical findings, and
  invalid/expired candidate-bound exceptions.
- [ ] Run focused tests and verify failures precede implementation.
- [ ] Implement guards with injected process/environment/filesystem adapters.
  Accept credential file paths only after permission checks; never copy
  credentials into the repository or persistent home directories.
- [ ] Pin and verify Gradle distribution/dependencies and emit deterministic
  candidate-bound CycloneDX SBOM records for Rust, Apple, and Android inputs.
- [ ] Run 100 percent release-tool coverage plus safe fake-credential
  integration fixtures.
- [ ] Commit exact WU-004 paths with
  `git commit -m "feat(release): enforce signing and supply-chain gates"`.

## WU-005: iOS/iPadOS and macOS release configuration

**Files:**

- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/ios/Riot/Info.plist`
- Modify: `apps/ios/Riot/Riot.entitlements`
- Create: `apps/ios/Riot/PrivacyInfo.xcprivacy`
- Modify: `apps/ios/ExportOptions.plist`
- Create: `apps/ios/RiotTests/ReleaseConfigurationTests.swift`
- Create: `apps/ios/RiotUITests/ReleaseCaptureUITests.swift`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot/Info.plist`
- Modify: `apps/macos/Riot/Riot.entitlements`
- Create: `apps/macos/Riot/PrivacyInfo.xcprivacy`
- Create: `apps/macos/Riot/Assets.xcassets/AppIcon.appiconset/**`
- Create: `apps/macos/RiotTests/ReleaseConfigurationTests.swift`
- Create: `apps/macos/RiotUITests/ReleaseCaptureUITests.swift`
- Rewrite: `scripts/testflight-release.sh`
- Create: `scripts/release-apple.sh`

- [ ] Write RED native/configuration tests for marketing version `1.0`, explicit
  build injection, `net.protest.riot`, iPhone/iPad families, Mac Apple-silicon
  and macOS 14 boundary, app icons, privacy manifests, sandbox/Keychain
  entitlements, export injection, candidate/capture arguments, and zero-test
  rejection.
- [ ] Run the focused XCTest/config validators and verify expected failures.
- [ ] Implement separate validation and credentialed candidate paths.
  `scripts/testflight-release.sh` becomes a compatibility wrapper that stops
  after archive/export and prints Xcode Organizer steps; remove API-key upload,
  home-directory copying, commit-count build numbers, and `ALLOW_DIRTY`.
- [ ] Build iOS simulator/device-validation and macOS validation artifacts; when
  signing is unavailable, verify candidate handoff remains `BLOCKED` rather
  than silently using ad-hoc output.
- [ ] Run iOS/macOS tests and config/archive inspectors.
- [ ] Commit exact WU-005 paths with
  `git commit -m "feat(release): configure Apple store candidates"`.

## WU-006: Android release configuration

**Files:**

- Modify: `apps/android/app/build.gradle.kts`
- Modify: `apps/android/settings.gradle.kts`
- Modify: `apps/android/app/src/main/AndroidManifest.xml`
- Create: `apps/android/app/src/main/res/mipmap-*/**`
- Create: `apps/android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml`
- Create: `apps/android/app/src/main/res/values/{colors,strings}.xml`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/ReleaseConfigurationTest.kt`
- Create: `scripts/release-android.sh`

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
- [ ] Run unit tests, lint, dependency verification, `bundleRelease` validation,
  and artifact inspection. A missing key must yield `HUMAN ACTION` or
  `BLOCKED`, never a debug-signed candidate.
- [ ] Commit exact WU-006 paths with
  `git commit -m "feat(release): configure Android Play candidate"`.

## WU-007: Exact-candidate screenshots, accessibility, and rehearsal

**Files:**

- Create: `release/source/journeys.json`
- Create: `release/source/devices.json`
- Create: `scripts/release/{journey,accessibility,rehearsal}.mjs`
- Create: `scripts/release/test/{journey,accessibility,rehearsal}.test.mjs`
- Modify/Create: WU-005 Apple fixture/capture/rehearsal sources
- Modify/Create: WU-006 Android fixture/capture/rehearsal sources
- Generate: `release/generated/visuals/{iphone,ipad,mac,android-phone,android-tablet}/**`
- Generate: `release/generated/visual-provenance.json`
- Generate: `release/evidence/rehearsals/**`

- [ ] Write RED tests for missing/skipped/zero-test journeys, wrong candidate,
  fixture drift, invalid join/no-peer dead ends, restart/offline failures,
  unadvertised capability claims, semantic labels/roles/states, reading/focus
  order, focus traps, contrast, target size, text scaling, and missing
  VoiceOver/TalkBack/Mac-keyboard human records.
- [ ] Run focused Node and native tests and verify failures before entry points
  and schemas exist.
- [ ] Implement deterministic capture/rehearsal launch arguments that are
  compiled for release capture but cannot expose fixture mode in ordinary
  production launch.
- [ ] Run the common-task matrix on iPhone, iPad, Mac, Android phone, and
  Android tablet candidates; run required physical Nearby pairings before
  selecting frame 5. Record exact candidate/device/OS/evidence digests.
- [ ] Regenerate WU-002 captures from the accepted candidates and verify the
  complete 30-cell matrix, Nearby/Join selection, dimensions, orientations,
  native and thumbnail reviews, and visual provenance manifest bind the same
  candidate IDs.
- [ ] Commit exact WU-007 source and redacted evidence with
  `git commit -m "test(release): record candidate rehearsal evidence"`.

## WU-008: Manual Console handoff and reconciliation

**Files:**

- Create: `release/source/console-statuses.json`
- Create: `scripts/release/{handoff,evidence}.mjs`
- Create: `scripts/release/test/{handoff,evidence}.test.mjs`
- Create: `release/evidence/.gitkeep`
- Modify: `scripts/release/cli.mjs`

- [ ] Write RED tests for missing pushed intent, non-fast-forward predecessor,
  upload preconditions, post-upload submission-package/store-identity binding,
  same-identity attestations, self-authorizing role changes, unsafe
  cancellation, Apple/Google status discrimination, direct negative outcomes,
  interruption, eventual-consistency absence, indeterminate replay, and
  candidate/package substitution.
- [ ] Run focused tests and verify failure before handoff modules exist.
- [ ] Implement credential-free action-specific instructions and immutable
  evidence validation. Upload binds expected ID/version/build, artifact,
  signing identity, and fresh store maximum; later actions bind store identity,
  candidate digest, and active submission-package digest.
- [ ] Implement separately authenticated operator/verifier attestations,
  `cancelled-before-action`, and fail-closed reconciliation. No test or
  production adapter may execute `xcodebuild -upload`, `altool`, Transporter,
  Play Publisher API, or browser automation against a store.
- [ ] Run the complete state/evidence suite at 100 percent JS coverage.
- [ ] Commit exact WU-008 paths with
  `git commit -m "feat(release): add manual Console evidence handoff"`.

## WU-009: Composite verification, CI, and operator runbook

**Files:**

- Create: `scripts/release/cli.mjs`
- Create: `scripts/release/verify-all.sh`
- Create: `release/README.md`
- Create: `release/OPERATOR-RUNBOOK.md`
- Create: `release/generated/release-checklist.md`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `scripts/green.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `.gitignore`

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
- [ ] Run the complete verification matrix below and record exact command
  results in the generated checklist.
- [ ] Commit exact WU-009 paths with
  `git commit -m "ci(release): enforce public store readiness"`.

## Final verification matrix

Run from a clean commit containing all reservations and generated outputs:

```bash
npm ci
npm run release:generate
git diff --exit-code -- release/generated
npm run test:release:coverage
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
sh scripts/web/coverage.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 16 Pro'
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
(cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 \
  ./gradlew --no-daemon --write-verification-metadata sha256 \
  testDebugUnitTest lint bundleRelease)
sh scripts/release/verify-all.sh
node scripts/release/cli.mjs status
```

Expected repository result: all credential-free checks pass; missing signing
materials, Apple export decision, account agreements, physical-device records,
and Console actions appear as `HUMAN ACTION` or `BLOCKED` with recovery
instructions. They never appear as `PASS`.

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
