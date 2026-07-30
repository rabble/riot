# WU-001 Shared Synthetic Fixture and Native Contracts Plan

> **Status (2026-07-27): COMPLETE.** Plan approved by all three reviewers
> (607ffe55). Implemented in 966d020c; code-quality review requested changes,
> fixes landed in 2683dd14 (`.gitattributes` LF guard — user-approved scope
> expansion — plus Kotlin/Swift seam parity: exact `schemaVersion`, trailing
> content rejection, `identifiers` error precedence). Final verdicts:
> spec-compliance PASS, code-quality APPROVE. Post-commit verification: iOS
> 4/4, macOS 3/3, Android 4/4 focused tests; fixture SHA-256
> `930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469` identical
> in both loaders; fixture present in test bundles, absent from all
> Debug/Release app products and Android APK/AAB; WU-000 regressions green.
> Known unrelated environment issue: rabble/riot#151
> (`lintVitalReportRelease` AGP path-variable failure, reproduces on parent
> commit; must be resolved before WU-006 release builds).

**Goal:** Add one deterministic, obviously synthetic Riot 1.0 release scenario
that iOS, iPadOS, macOS, Android phone, and Android tablet capture work can
consume later, while proving today that Swift and Kotlin load the exact same
bytes and that no production launch path can select the fixture.

**Architecture:** A checked-in JSON file is the only fixture source of truth.
The fixture contains a fixed UTC clock, six ordered store-story states, and
full 64-character identifiers derived from documented synthetic labels. Native
loaders accept caller-supplied bytes only, verify a pinned SHA-256 before
decoding, reject malformed or semantically invalid data, and expose immutable
read-only models. The JSON is bundled only into unit-test targets in this work
unit. WU-005 and WU-006 will add separately guarded capture-only entry points;
normal production launch receives no bundle lookup, environment switch, launch
argument, or fixture selector here.

**Tech stack:** JSON; Swift 6, Foundation, CryptoKit, XCTest; Kotlin/JVM,
`java.security.MessageDigest`, Android `org.json`, pinned host-test runtime
`org.json:json:20240303`, JUnit 4; Xcode project resources; Gradle test
resources.

**Allowed file scope:**

- Create `fixtures/release/riot-1.0-synthetic.json`
- Create `apps/ios/Riot/Release/ReleaseFixture.swift`
- Create `apps/ios/RiotTests/ReleaseFixtureTests.swift`
- Modify `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify `apps/macos/Riot.xcodeproj/project.pbxproj`
- Create `apps/macos/RiotTests/ReleaseFixtureTests.swift`
- Modify `apps/android/app/build.gradle.kts`
- Create
  `apps/android/app/src/main/kotlin/org/riot/evidence/ReleaseFixture.kt`
- Create
  `apps/android/app/src/test/kotlin/org/riot/evidence/ReleaseFixtureTest.kt`

No app entry point, shipping resource phase, identifier/version setting,
capture path, or WU-002+ release tooling changes are in scope.

## Canonical fixture contract

The JSON root is an exact allowlisted object:

- `schemaVersion`: integer `1`
- `fixtureRevision`: exact string `riot-1.0-synthetic-v1`
- `fixtureKind`: exact string `synthetic`
- `fixedClock`: exact RFC 3339 millisecond UTC value
  `2026-07-24T12:00:00.000Z`
- `identifiers`: exact object containing `communityId`, `contributorId`, and
  `stateIds`
- `narrativeStates`: exactly six entries in the order below

Every identifier is a full lowercase 64-character SHA-256 hex digest derived
from the UTF-8 label `riot-release-fixture:v1:<role>`. Tests independently
recompute each expected value; the fixture never contains a production,
truncated, random, device-derived, or user-derived identifier.

The six state IDs and claims are:

1. `community-newswire` — Your community. Your newswire.
2. `signed-publishing` — Publish signed updates from the field.
3. `labels-and-signatures` — Read signatures and community editorial labels.
4. `community-tools` — Carry useful tools with the community.
5. `nearby-exchange` — Exchange updates nearby; explicitly experimental.
6. `offline-copy` — Keep a local copy available offline.

Each state contains only `id`, `surface`, `headline`, `supportingCopy`,
`communityId`, `contributorId`, and `entryId`. The content uses neutral
synthetic nouns and contains no person/name, email, phone, location,
coordinates, address, notification, device token, APNs/FCM token, private
community, production hostname, or operational identifier.

The exact fixture file bytes, including final newline, are SHA-256 pinned in
both native loaders. The loaders validate digest first and semantic invariants
second. Unknown JSON fields are rejected by explicit key-set validation before
typed decoding. Each implementation also exposes an `internal` pure semantic
validation seam to its unit-test target. That seam accepts caller-supplied
bytes without the public digest guard so mutations can reach their intended
semantic diagnostic; it performs no I/O and is not a public or launch-facing
fixture selector. Public decoding remains permanently digest-pinned.

The fixture is byte-complete except for the SHA-256 calculated after writing
these exact UTF-8 bytes. It uses two-space indentation, the property order
shown below, and one final LF:

```json
{
  "schemaVersion": 1,
  "fixtureRevision": "riot-1.0-synthetic-v1",
  "fixtureKind": "synthetic",
  "fixedClock": "2026-07-24T12:00:00.000Z",
  "identifiers": {
    "communityId": "594f46efab3be7a37c73bc24210ff74db1bde77cfd7d38ec45fba1e7fc6683c2",
    "contributorId": "5f0aa3963f6529c1b6bdfec174cc6ca9ff0571dd852e646b3d85c0595e6bad17",
    "stateIds": {
      "community-newswire": "965a23af58731ecbb8725fbcacb4db4a4dd1366dfa51bd7dbdb54a4bbe673029",
      "signed-publishing": "3393b18dda2eb674db90b223d7974d431a95c8edbf7a255aaab31aff2dfcb7af",
      "labels-and-signatures": "f0dacbe550055e95e36da64af1a345ce854a396c36d0a34cc202d4d5acfce0c9",
      "community-tools": "7b1ebb0e0e36d98dfc46a0cd22d8e7e9a045c035f6df3ce4a0667870919e401d",
      "nearby-exchange": "7c77f58a435f92f469740e60cd7fa7ae8271bc483b32813c31d9b36ce9dd4c9c",
      "offline-copy": "419d963210324a56be33c0fa1ee46b7234545f27a48aa7c837e38d324ea6ef40"
    }
  },
  "narrativeStates": [
    {
      "id": "community-newswire",
      "surface": "spaces-home",
      "headline": "Your community. Your newswire.",
      "supportingCopy": "Follow updates from a community you choose.",
      "communityId": "594f46efab3be7a37c73bc24210ff74db1bde77cfd7d38ec45fba1e7fc6683c2",
      "contributorId": "5f0aa3963f6529c1b6bdfec174cc6ca9ff0571dd852e646b3d85c0595e6bad17",
      "entryId": "965a23af58731ecbb8725fbcacb4db4a4dd1366dfa51bd7dbdb54a4bbe673029"
    },
    {
      "id": "signed-publishing",
      "surface": "compose",
      "headline": "Publish signed updates from the field.",
      "supportingCopy": "Signatures show source and integrity, not whether a claim is true.",
      "communityId": "594f46efab3be7a37c73bc24210ff74db1bde77cfd7d38ec45fba1e7fc6683c2",
      "contributorId": "5f0aa3963f6529c1b6bdfec174cc6ca9ff0571dd852e646b3d85c0595e6bad17",
      "entryId": "3393b18dda2eb674db90b223d7974d431a95c8edbf7a255aaab31aff2dfcb7af"
    },
    {
      "id": "labels-and-signatures",
      "surface": "newswire",
      "headline": "Read signatures and community editorial labels.",
      "supportingCopy": "See signed source details. Community editorial labels are community signals, not independent factual verification.",
      "communityId": "594f46efab3be7a37c73bc24210ff74db1bde77cfd7d38ec45fba1e7fc6683c2",
      "contributorId": "5f0aa3963f6529c1b6bdfec174cc6ca9ff0571dd852e646b3d85c0595e6bad17",
      "entryId": "f0dacbe550055e95e36da64af1a345ce854a396c36d0a34cc202d4d5acfce0c9"
    },
    {
      "id": "community-tools",
      "surface": "apps-checklists",
      "headline": "Carry useful tools with the community.",
      "supportingCopy": "Open a shared checklist alongside community updates.",
      "communityId": "594f46efab3be7a37c73bc24210ff74db1bde77cfd7d38ec45fba1e7fc6683c2",
      "contributorId": "5f0aa3963f6529c1b6bdfec174cc6ca9ff0571dd852e646b3d85c0595e6bad17",
      "entryId": "7b1ebb0e0e36d98dfc46a0cd22d8e7e9a045c035f6df3ce4a0667870919e401d"
    },
    {
      "id": "nearby-exchange",
      "surface": "nearby",
      "headline": "Exchange updates nearby.",
      "supportingCopy": "Experimental: exchange updates directly with a nearby device.",
      "communityId": "594f46efab3be7a37c73bc24210ff74db1bde77cfd7d38ec45fba1e7fc6683c2",
      "contributorId": "5f0aa3963f6529c1b6bdfec174cc6ca9ff0571dd852e646b3d85c0595e6bad17",
      "entryId": "7c77f58a435f92f469740e60cd7fa7ae8271bc483b32813c31d9b36ce9dd4c9c"
    },
    {
      "id": "offline-copy",
      "surface": "offline-copy",
      "headline": "Keep a local copy available offline.",
      "supportingCopy": "Keep a local copy ready to read without a connection.",
      "communityId": "594f46efab3be7a37c73bc24210ff74db1bde77cfd7d38ec45fba1e7fc6683c2",
      "contributorId": "5f0aa3963f6529c1b6bdfec174cc6ca9ff0571dd852e646b3d85c0595e6bad17",
      "entryId": "419d963210324a56be33c0fa1ee46b7234545f27a48aa7c837e38d324ea6ef40"
    }
  ]
}
```

The identifier labels are exact:

| Fixture field | SHA-256 UTF-8 label |
| --- | --- |
| `identifiers.communityId` | `riot-release-fixture:v1:community` |
| `identifiers.contributorId` | `riot-release-fixture:v1:contributor` |
| `stateIds["community-newswire"]` | `riot-release-fixture:v1:community-newswire` |
| `stateIds["signed-publishing"]` | `riot-release-fixture:v1:signed-publishing` |
| `stateIds["labels-and-signatures"]` | `riot-release-fixture:v1:labels-and-signatures` |
| `stateIds["community-tools"]` | `riot-release-fixture:v1:community-tools` |
| `stateIds["nearby-exchange"]` | `riot-release-fixture:v1:nearby-exchange` |
| `stateIds["offline-copy"]` | `riot-release-fixture:v1:offline-copy` |

## Task 0: Prove WU-000 and native prerequisites

**Files:** No changes.

1. Run the accepted foundation before starting RED work:

   ```sh
   npm run test:release:unit
   ./node_modules/.bin/c8 --100 --all \
     --include='scripts/release/**/*.mjs' \
     --exclude='scripts/release/test/**' \
     --temp-directory=build/wu001-c8 \
     --reports-dir=build/wu001-coverage \
     node --test scripts/release/test/*.test.mjs
   npm run release:generate
   git diff --exit-code -- release/generated/worksheets
   set +e
   npm run release:status -- --json
   status_rc=$?
   set -e
   test "$status_rc" -eq 1
   ```

2. Build the required generated UniFFI bindings and native libraries:

   ```sh
   sh scripts/conference/build-native-core.sh
   ```

3. If the foundation, generation drift, or native build fails, stop. The
   expected release status is `BLOCKED`/exit 1 because later release evidence
   is not complete; any other status result is a prerequisite failure.

## Task 1: Write the shared contract tests RED

**Files:**

- Create `apps/ios/RiotTests/ReleaseFixtureTests.swift`
- Create `apps/macos/RiotTests/ReleaseFixtureTests.swift`
- Create
  `apps/android/app/src/test/kotlin/org/riot/evidence/ReleaseFixtureTest.kt`
- Modify both Xcode projects and Android `build.gradle.kts` only enough to make
  the missing implementation/fixture failures observable

1. Add the planned test file references to the iOS `RiotTests` sources and
   macOS `RiotKitTests-macOS` sources.
2. Add a resources phase to the macOS unit-test target if absent. Add the same
   `fixtures/release/riot-1.0-synthetic.json` file reference to the iOS and
   macOS unit-test resource phases only. Do not add it to either app target.
3. Extend Android's `test` source set with
   `rootProject.file("../../fixtures/release")`; do not add it to `main` or
   `androidTest`. Add exactly
   `testImplementation("org.json:json:20240303")` so host-JVM unit tests use a
   real parser while the app continues to use Android's platform `org.json`.
4. Write equivalent XCTest/JUnit cases that assert:
   - the named fixture exists in each unit-test bundle/classpath;
   - the exact fixture-byte digest equals one shared expected literal;
   - schema version, revision, kind, and fixed clock are exact;
   - the six state IDs, order, surfaces, headlines, and experimental marker are
     exact;
   - every full identifier equals the deterministic digest of its documented
     synthetic label and is 64 lowercase hex characters;
   - root, identifier, and state key sets are exact;
   - recursive key/value scanning finds no private/person/location/
     notification/production data;
   - mutated digest fails through the public pinned loader;
   - through the internal semantic-validation seam, unknown fields, bad fixed
     clock, missing/duplicate/reordered states, mismatched IDs, truncated IDs,
     and a non-synthetic kind reach and assert their specific semantic errors;
   - table-driven mutations cover prohibited keys and values for every promised
     class: person/name; email/phone; location/address/coordinates;
     notification/device/APNs/FCM tokens; private data; production URL,
     hostname, and IP address; and operational `npub1`, `nsec1`, and `note1`
     identifiers;
   - the public loader requires explicit bytes and has no bundle, environment,
     process-argument, or default-fixture API.

   The prohibited-data mutation table is exact. Create one independent
   semantic-seam case for every key listed and one independent case for every
   value listed. Inject each key into `narrativeStates[0]` with the value
   `"synthetic-test-value"`. Inject each prohibited value by replacing
   `narrativeStates[0].supportingCopy`, leaving all other semantics valid:

   | Class | Injected keys | Injected values |
   | --- | --- | --- |
   | person/name | `person`, `personName`, `name` | `Person: Ana` |
   | email/phone | `email`, `phone` | `ana@example.com`, `+64 21 555 0100` |
   | location/address/coordinates | `location`, `address`, `latitude`, `longitude`, `coordinates` | `123 Main Street`, `37.7749,-122.4194` |
   | notification/device tokens | `notification`, `deviceToken`, `apnsToken`, `fcmToken` | `ExponentPushToken[fixture]` |
   | private data | `private`, `privateCommunity` | `private community` |
   | production network data | `url`, `hostname`, `ipAddress` | `https://riot.protest.net`, `riot.protest.net`, `203.0.113.1` |
   | operational Nostr IDs | `npub`, `nsec`, `note` | `npub1t985dmat80n6xlrnhsjzzrlhfkcmmemul47n3mz9lws70lrxs0pqwzdyaw`, `nsec1tu92893lv55urd4almqhfnrv48ls2uwas5hxg6eashq9jhnt45ts9en3zd`, `note1m99r7nwc0wdrkzldrqan96gklg5usqspq7z9696j6unf0ljnpxjspqfw99` |
5. Run the focused tests and capture the expected compile/resource failures:

   ```sh
   xcodebuild test \
     -project apps/ios/Riot.xcodeproj \
     -scheme RiotKit \
     -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
     -derivedDataPath build/wu001-ios-derived \
     -only-testing:RiotTests/ReleaseFixtureTests

   xcodebuild test \
     -project apps/macos/Riot.xcodeproj \
     -scheme RiotKit-macOS \
     -destination 'platform=macOS' \
     -derivedDataPath build/wu001-macos-derived \
     -only-testing:RiotKitTests-macOS/ReleaseFixtureTests

   (
     cd apps/android
     RIOT_ANDROID_SDK="${ANDROID_HOME:-/Users/rabble/Library/Android/sdk}"
     JAVA_HOME=/opt/homebrew/opt/openjdk@17 \
       ANDROID_HOME="$RIOT_ANDROID_SDK" \
       ANDROID_SDK_ROOT="$RIOT_ANDROID_SDK" \
       ./gradlew :app:testDebugUnitTest \
       --tests org.riot.evidence.ReleaseFixtureTest
   )
   ```

Expected RED: missing fixture and missing native `ReleaseFixture` APIs. If any
focused command executes zero tests, the task is RED even when the runner exits
zero.

## Task 2: Create the immutable synthetic fixture

**Files:**

- Create `fixtures/release/riot-1.0-synthetic.json`

1. Write the exact allowlisted JSON with the fixed clock, deterministic
   label-derived identifiers, and six ordered states.
2. Keep presentation text truthful to the approved visual narrative:
   signatures prove source/integrity rather than truth, Nearby is
   experimental, offline copy is local rather than a universal resilience
   claim, and community editorial labels are community signals rather than
   independent factual verification.
3. End the file with one LF and calculate its SHA-256 using both available
   platform commands:

   ```sh
   shasum -a 256 fixtures/release/riot-1.0-synthetic.json
   openssl dgst -sha256 fixtures/release/riot-1.0-synthetic.json
   ```

4. Put that exact digest into all three test contracts. Re-run tests; they must
   remain RED because native loaders do not yet exist.

## Task 3: Implement the Swift read-only decoder

**Files:**

- Create `apps/ios/Riot/Release/ReleaseFixture.swift`
- Modify `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify `apps/macos/Riot.xcodeproj/project.pbxproj`

1. Add immutable `Sendable`, `Equatable`, `Decodable` fixture, identifier, and
   state models plus a small typed `ReleaseFixtureError`.
2. Implement `ReleaseFixture.decode(bytes:)` only:
   - SHA-256 exact-byte verification with the pinned digest;
   - JSON object/key-set validation before `JSONDecoder`;
   - exact revision/kind/fixed-clock/state inventory validation;
   - strict 64-lowercase-hex checks and deterministic synthetic-label digest
     checks;
   - per-state community/contributor equality and state-specific entry ID;
   - exact `surface`, `headline`, and `supportingCopy` for all six states,
     including the experimental Nearby and editorial-label qualifications;
   - recursive prohibited-key/value checks.
3. Add an `internal` semantic-validation seam used by `@testable` unit tests.
   It may bypass only the byte-digest comparison; it executes every other
   production validation and cannot read or select fixture data.
4. Do not add a `Bundle` lookup, singleton, environment reader, process
   argument reader, default bytes, app-state mutation, or capture-mode switch.
5. Add the source to `RiotKit` in iOS and `RiotKit-macOS` in macOS. It may link
   into shipping code as a pure decoder, but the fixture JSON remains absent
   from both shipping resource phases and no launch path calls it.
6. Run iOS and macOS focused tests and require nonzero execution and PASS.

## Task 4: Implement the Kotlin read-only decoder

**Files:**

- Create
  `apps/android/app/src/main/kotlin/org/riot/evidence/ReleaseFixture.kt`

1. Add immutable Kotlin data classes, a typed
   `ReleaseFixtureContractException`, and
   `ReleaseFixture.decode(bytes: ByteArray)`.
2. Match Swift exactly for byte digest, allowlisted key sets, fixed values,
   state inventory/order, every exact surface/headline/supporting-copy value,
   full synthetic identifiers, prohibited data, and fail-closed diagnostics.
3. Add an `internal` semantic-validation seam with the same constraints as the
   Swift seam.
4. Use Android `org.json` in production, the pinned JVM-safe test runtime from
   Task 1, and `MessageDigest`; add no production dependency or resource
   accessor.
5. Run the focused Android test with JDK 17 and require nonzero execution and
   PASS.

## Task 5: Cross-platform parity and target-membership verification

**Files:** No new scope.

1. Run all three focused suites and extract their executed-test counts.
2. Build normal Debug and unsigned Release iOS/macOS apps in dedicated
   DerivedData trees before scanning actual products:

   ```sh
   RIOT_DERIVED_DATA=build/wu001-shared-dd sh scripts/ios-check.sh sim
   RIOT_DERIVED_DATA=build/wu001-shared-dd sh scripts/ios-check.sh fast

   xcodebuild build \
     -project apps/ios/Riot.xcodeproj \
     -scheme Riot \
     -configuration Release \
     -destination 'generic/platform=iOS Simulator' \
     -derivedDataPath build/wu001-ios-release-derived \
     CODE_SIGNING_ALLOWED=NO

   xcodebuild build \
     -project apps/macos/Riot.xcodeproj \
     -scheme Riot-macOS \
     -configuration Release \
     -destination 'platform=macOS' \
     -derivedDataPath build/wu001-macos-release-derived \
     CODE_SIGNING_ALLOWED=NO
   ```

3. Assert the fixture exists in test products but not Debug or Release app
   products. Positive searches must produce a path:

   ```sh
   test -n "$(find build/wu001-ios-derived -path '*RiotTests.xctest*' \
     -name 'riot-1.0-synthetic.json' -print -quit)"
   test -z "$(find build/wu001-shared-dd -path '*Riot.app*' \
     -name 'riot-1.0-synthetic.json' -print -quit)"
   test -z "$(find build/wu001-ios-release-derived -path '*Riot.app*' \
     -name 'riot-1.0-synthetic.json' -print -quit)"

   test -n "$(find build/wu001-macos-derived \
     -path '*RiotKitTests-macOS.xctest*' \
     -name 'riot-1.0-synthetic.json' -print -quit)"
   test -z "$(find build/wu001-shared-dd -path '*Riot.app*' \
     -name 'riot-1.0-synthetic.json' -print -quit)"
   test -z "$(find build/wu001-macos-release-derived -path '*Riot.app*' \
     -name 'riot-1.0-synthetic.json' -print -quit)"

   rg -n 'riot-1\\.0-synthetic\\.json' \
     apps/ios/Riot.xcodeproj/project.pbxproj \
     apps/macos/Riot.xcodeproj/project.pbxproj
   ```

   Inspect those project references and require that they occur in `RiotTests`
   and `RiotKitTests-macOS` resource phases only, never the `Riot` or
   `Riot-macOS` app resource phases.

4. Validate both project files:

   ```sh
   plutil -lint apps/ios/Riot.xcodeproj/project.pbxproj
   plutil -lint apps/macos/Riot.xcodeproj/project.pbxproj
   xcodebuild -list -project apps/ios/Riot.xcodeproj
   xcodebuild -list -project apps/macos/Riot.xcodeproj
   ```

5. Build Android Debug plus unsigned Release APK/AAB artifacts, run the complete
   unit suite, assert fixture classpath presence in the unit test, and inspect
   all actual archives for production-resource absence:

   ```sh
   (
     cd apps/android
     RIOT_ANDROID_SDK="${ANDROID_HOME:-/Users/rabble/Library/Android/sdk}"
     JAVA_HOME=/opt/homebrew/opt/openjdk@17 \
       ANDROID_HOME="$RIOT_ANDROID_SDK" \
       ANDROID_SDK_ROOT="$RIOT_ANDROID_SDK" \
       ./gradlew :app:testDebugUnitTest :app:assembleDebug \
       :app:assembleRelease :app:bundleRelease
     ! unzip -Z1 app/build/outputs/apk/debug/app-debug.apk |
       rg -q '(^|/)riot-1\\.0-synthetic\\.json$'
     ! unzip -Z1 app/build/outputs/apk/release/app-release-unsigned.apk |
       rg -q '(^|/)riot-1\\.0-synthetic\\.json$'
     ! unzip -Z1 app/build/outputs/bundle/release/app-release.aab |
       rg -q '(^|/)riot-1\\.0-synthetic\\.json$'
   )
   ```

6. Run repository fixture scans with explicit no-match semantics:

   ```sh
   ! rg -n -i \
     'person|name|email|phone|address|street|city|country|latitude|longitude|coordinates|notification|device.?token|apns|fcm|private|https?://|riot\\.protest\\.net|([0-9]{1,3}\\.){3}[0-9]{1,3}|\\b(npub|nsec|note)1' \
     fixtures/release/riot-1.0-synthetic.json
   ! rg -n \
     'ReleaseFixture\\.(default|shared|load)|ProcessInfo|Bundle\\.main|System\\.getenv|System\\.getProperty|intent' \
     apps/ios/Riot/Release/ReleaseFixture.swift \
     apps/android/app/src/main/kotlin/org/riot/evidence/ReleaseFixture.kt
   ```

   Expected result: no unsafe match. The words needed in negative test
   fixtures stay in test source, not the canonical fixture.

## Task 6: WU-001 final quality gates and commit

1. Run WU-000 regressions:

   ```sh
   npm run test:release:unit
   ./node_modules/.bin/c8 --100 --all \
     --include='scripts/release/**/*.mjs' \
     --exclude='scripts/release/test/**' \
     --temp-directory=build/wu001-c8 \
     --reports-dir=build/wu001-coverage \
     node --test scripts/release/test/*.test.mjs
   npm run release:generate
   git diff --exit-code -- release/generated/worksheets
   set +e
   npm run release:status -- --json
   status_rc=$?
   set -e
   test "$status_rc" -eq 1
   ```

   Release status must remain truthfully `BLOCKED` with exit 1; capture that
   expected exit rather than treating it as readiness.
2. Run:

   ```sh
   git diff --check
   git status --short
   ```

3. Stage exact paths and inspect the cached name/status and diff. Require that
   every staged path is in the declared WU-001 scope:

   ```sh
   git add \
     fixtures/release/riot-1.0-synthetic.json \
     apps/ios/Riot/Release/ReleaseFixture.swift \
     apps/ios/RiotTests/ReleaseFixtureTests.swift \
     apps/ios/Riot.xcodeproj/project.pbxproj \
     apps/macos/Riot.xcodeproj/project.pbxproj \
     apps/macos/RiotTests/ReleaseFixtureTests.swift \
     apps/android/app/build.gradle.kts \
     apps/android/app/src/main/kotlin/org/riot/evidence/ReleaseFixture.kt \
     apps/android/app/src/test/kotlin/org/riot/evidence/ReleaseFixtureTest.kt
   git diff --cached --name-status
   git diff --cached --check
   ```

4. Commit only those paths:

   ```sh
   git commit -m "test(release): add shared synthetic fixture contract"
   ```

5. Run the post-commit focused suites, digest checks, WU-000 release tests and
   coverage, generated-output drift check, and clean-worktree check again.
6. Send the commit through independent spec-compliance and code-quality review.
   Do not begin WU-002 until both reviewers approve.

## Acceptance criteria

- One checked-in JSON byte sequence is consumed by Swift on iOS/macOS and
  Kotlin on Android with the same pinned SHA-256.
- All platforms prove exact schema/revision/fixed clock, six ordered narrative
  states, and full deterministic synthetic identifiers.
- Mutation, unknown fields, unsafe content, wrong state order, and
  non-synthetic identifiers fail closed.
- The fixture is available to native unit tests but absent from normal iOS,
  macOS, and Android production resources.
- No production launch selector, environment switch, argument, or default
  fixture accessor exists.
- Focused suites execute nonzero tests and pass; normal app builds and the full
  Android unit suite pass.
- WU-000 remains 100% covered, deterministic, and truthfully blocked.
- Only declared WU-001 paths are committed.
