# WU-001N — Durable catalog generation + Android codec preflight Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: metaswarm orchestrated-execution (4-phase loop). Steps use checkbox (`- [ ]`) syntax. Parent: `2026-07-22-riot-microapp-family-master-plan.md`. Spec: `docs/superpowers/specs/2026-07-22-riot-microapp-family-design.md` §"Content identity and upgrades". Builds on merged WU-001 and unblocks native WU-002c.

**Goal:** Persist the starter-catalog generation without growing legacy profiles, restore the exact generation into Rust on Apple and Android, and expose Android's exact full-profile encoded size as the preflight WU-002c will run before durable mutations.

**Architecture:** Rust keeps `Option<u8>` as the authority (`None` means generation 1; fresh profiles are `Some(2)`). Existing no-generation open APIs remain the fresh-profile APIs and keep selecting generation 2; new generation-aware local/database restore APIs plus the two sealed-identity restore APIs validate and retain the persisted marker. This distinction preserves identityless legacy snapshots instead of silently upgrading them. Apple's optional Codable field is absent for legacy snapshots and present as `2` on a fresh first save. Android writes legacy `null` profiles in the byte-identical v3 representation, writes generation-bearing profiles as v4, and uses one `encodedSize` function for both preflight and allocation so the prospective result cannot disagree with actual encoding.

**Tech Stack:** Rust 2021 + UniFFI, Swift 6/Foundation Codable, Kotlin 2.2/JVM binary codec, XCTest/JUnit.

**Scope boundary (do NOT exceed):**

- Rust: `crates/riot-ffi/src/mobile_api.rs`, `crates/riot-ffi/src/mobile_state.rs`, and only compile-fix/test call sites for the changed/new restore functions.
- Apple: `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/RiotTests/BindingSemanticsTests.swift`.
- Android: `apps/android/app/src/main/kotlin/org/riot/evidence/PersistedProfile.kt`, `RiotController.kt`, `apps/android/app/src/test/kotlin/org/riot/evidence/PersistedProfileCodecTest.kt`, and `apps/android/app/src/androidTest/kotlin/org/riot/evidence/BindingSemanticsTest.kt`.
- Bindings are regenerated and native builds/tests run, but generated bindings are not committed unless the repository's existing generator reports tracked output.
- Do not wire trust/app-data prepare/finalize, UI alerts, WebView teardown, or fault injection here; those are WU-002c.
- Do not edit any fixture bytes, starter catalog membership, theme files, or presentation UI.

---

## File responsibilities

| File | Responsibility |
| --- | --- |
| `crates/riot-ffi/src/mobile_state.rs` | Validate persisted `Option<u8>` and retain it on sealed and identityless restore paths; leave fresh opens at generation 2 |
| `crates/riot-ffi/src/mobile_api.rs` | Expose optional generation on sealed restores and explicit generation-aware identityless restore APIs |
| `apps/ios/Riot/Core/ProfileRepository.swift` | Optional Codable marker, fresh=2, legacy absence=nil, forward marker through both sealed and identityless restore |
| `apps/android/.../PersistedProfile.kt` | v3 null-marker preservation, v4 marker encoding, shared exact `encodedSize` preflight |
| `apps/android/.../RiotController.kt` | Truly fresh saves carry generation 2; existing-profile mutations preserve the marker; sealed and identityless restores forward it to FFI |
| Native/Rust tests | Pin fresh, legacy, explicit-generation, exact-size, and zero-growth behavior |

---

## Task 1: Thread the persisted generation through Rust restore

**Files:**

- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/src/mobile_api.rs`
- Test: inline `mobile_state.rs` tests
- Mechanical signature updates: `crates/riot-ffi/tests/{apps_contract,mobile_contract,mobile_refusal_surface,organizer_trust,persistence_contract}.rs`

- [ ] **Step 1: Write failing generation restore tests.** Add inline tests for both restore families:

  - sealed identity: restore with `None`, `Some(1)`, and `Some(2)` and assert the exact value is retained;
  - identityless local/database: restore with `None`, `Some(1)`, and `Some(2)` and assert the exact value is retained;
  - both families reject `Some(0)` and `Some(3)` with `MobileError::InvalidInput`.

Also pin the existing no-generation fresh local/database opens to `Some(2)` so the new restore seam cannot change fresh-profile semantics.

- [ ] **Step 2: Run the focused test and observe RED.**

Run: `cargo test -p riot-ffi mobile_state::tests::restore_uses_persisted_starter_catalog_generation`

Expected: compile failure because the sealed helper has no generation parameter and the identityless generation-aware restore APIs do not exist.

- [ ] **Step 3: Add one validator and generation-aware internal restore seams.** Use this contract:

```rust
fn validate_starter_catalog_generation(generation: Option<u8>) -> Result<Option<u8>, MobileError> {
    match generation {
        None | Some(1) | Some(2) => Ok(generation),
        Some(_) => Err(MobileError::InvalidInput),
    }
}

pub(crate) fn open_profile_from_sealed_identity(
    mut wrapping_key: Vec<u8>,
    sealed_identity: Vec<u8>,
    starter_catalog_generation: Option<u8>,
) -> Result<Arc<MobileProfile>, MobileError>
```

Validate before constructing the profile and pass the returned value to `profile_with_author`. Apply the same final parameter and validation to `open_profile_from_sealed_identity_with_database` before `profile_with_author_and_db`.

Add explicit identityless restore helpers equivalent to:

```rust
pub(crate) fn open_local_profile_for_starter_catalog_generation(
    starter_catalog_generation: Option<u8>,
) -> Result<Arc<MobileProfile>, MobileError>

pub(crate) fn open_local_profile_with_database_for_starter_catalog_generation(
    db_path: String,
    starter_catalog_generation: Option<u8>,
) -> Result<Arc<MobileProfile>, MobileError>
```

Both use the same validator. Keep the existing `open_local_profile` and `open_local_profile_with_database` as fresh-profile functions with generation `Some(2)`; do not redefine their no-argument meaning.

- [ ] **Step 4: Extend/export the four UniFFI restore seams.** Append `starter_catalog_generation: Option<u8>` to the two sealed restore signatures, preserving existing parameter order. Export the two explicitly named generation-aware identityless restore functions and forward unchanged to `mobile_state`. Keep the existing fresh functions exported and unchanged.

- [ ] **Step 5: Update existing Rust sealed-restore call sites mechanically.** Every pre-WU-001N sealed test supplies `None`; only new generation tests use `Some(1)`/`Some(2)`. Existing fresh-open call sites remain unchanged. Do not change what any existing test is proving.

- [ ] **Step 6: Run GREEN and workspace compile.**

Run:

```text
cargo test -p riot-ffi mobile_state::tests::restore_uses_persisted_starter_catalog_generation
cargo check --workspace --all-features
```

Expected: PASS.

- [ ] **Step 7: Regenerate the ABI and build/install native libraries before any host test.** The Rust signature changes make the checked-in/generated host bindings stale, and Gradle does not build Rust. Run the repository's combined native build now:

```text
ANDROID_HOME=$HOME/Library/Android/sdk scripts/conference/build-native-core.sh
```

Expected: fresh Swift/Kotlin bindings plus iOS simulator, macOS, and Android arm64/x86_64 libraries under `build/native`. Repeat this command after any later Rust ABI change; do not compile host calls against the prior bindings.

---

## Task 2: Persist generation 2 on Apple and preserve legacy absence

**Files:**

- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Test: `apps/ios/RiotTests/BindingSemanticsTests.swift`

- [ ] **Step 1: Add RED persistence tests.** Add helpers that deserialize the snapshot JSON and tests proving:

1. a fresh `RiotProfileRepository.open` first save contains `"starterCatalogGeneration": 2` together with the sealed identity;
2. deleting that key to emulate a legacy snapshot, reopening, and performing a permitted save leaves the key absent rather than materializing generation 1;
3. explicit generation 1 reopens successfully and remains durably encoded as `1` (Task 1 proves the internal `Some(1)` retention);
4. a sealed legacy snapshot reopens with the same signer; an identityless legacy snapshot necessarily mints and seals a signer on its first reopen, then a second reopen preserves that newly sealed signer. Both paths keep the generation key absent in subsequent durable JSON rather than materializing generation 2.

Run the focused XCTest target and confirm the fresh-marker assertion fails.

```text
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/BindingSemanticsTests
```

- [ ] **Step 2: Add the optional Codable field.** Extend `PersistedProfile` with:

```swift
var starterCatalogGeneration: UInt8?
```

Set `.empty` to `2`, accept it in the private initializer, and decode with:

```swift
starterCatalogGeneration = try container.decodeIfPresent(
    UInt8.self, forKey: .starterCatalogGeneration
)
```

Synthesized encoding must remain in use so `nil` omits the key. Do not convert `nil` to `1` in storage; absence itself is generation 1.

- [ ] **Step 3: Forward the marker on every restore path.** Append `starterCatalogGeneration: persisted.starterCatalogGeneration` to both `openProfileFromSealedIdentity` variants in `openCore`. When persisted state exists but has no sealed identity, call the new generation-aware local/database restore API with the optional marker; do not fall through to the fresh no-argument API. Only absence of persisted state calls the existing fresh FFI and receives Rust generation 2. Host tests prove that all representations are accepted and remain durable without upgrade; Task 1's Rust white-box tests are the proof of the exact retained internal `Option<u8>` because the value has no public getter.

- [ ] **Step 4: Run focused Apple tests GREEN.** Re-run iOS `BindingSemanticsTests`; then run the shared macOS test scheme because it compiles and exercises the same repository source:

```text
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'
```

Expected: both test targets pass and the legacy JSON key remains absent.

---

## Task 3: Add Android v4 marker without growing v3 profiles

**Files:**

- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/PersistedProfile.kt`
- Test: `apps/android/app/src/test/kotlin/org/riot/evidence/PersistedProfileCodecTest.kt`

- [ ] **Step 1: Write RED codec tests.** Cover all of these independently:

- `starterCatalogGeneration = 2` round-trips and the encoded header version is 4;
- `starterCatalogGeneration = null` encodes with header version 3;
- decoding then re-encoding a v3 profile is byte-for-byte identical;
- invalid generation 0 or 3 rejects before stream allocation;
- explicit generation 1 round-trips as v4;
- a malformed v4 stream containing generation 0 or 3 is rejected during decode;
- `encodedSize(profile) == encode(profile).size` for both null/v3 and generation-2/v4 profiles.
- a valid v3 profile whose exact encoded size is 4,194,240 bytes succeeds, while a prospective v3 profile of 4,194,241 bytes is rejected by `encodedSize` before allocation;
- the same exact-limit/+1 behavior for v4, accounting for its four-byte marker.

Run:

```text
cd apps/android
./gradlew :app:testDebugUnitTest --tests org.riot.evidence.PersistedProfileCodecTest
```

Expected: compile failure because the model field and internal preflight do not exist.

- [ ] **Step 2: Extend the model and version constants.** Add a trailing defaulted field so positional legacy constructors remain source-compatible:

```kotlin
val starterCatalogGeneration: Int? = null,
```

Use named constants:

```kotlin
private const val VERSION = 4
private const val VERSION_WITH_APPS = 3
private const val VERSION_WITH_STARTER_CATALOG_GENERATION = 4
private const val LEGACY_WRITABLE_VERSION = 3
```

- [ ] **Step 3: Make the wire version representation-dependent.** `encodeInternal` chooses v3 when the marker is null and v4 otherwise, writes the chosen version in the header, and appends one 32-bit generation only for v4. The decoder reads the field only for v4 and requires it to be 1 or 2. A v1-v3 decode produces `null`.

```kotlin
val wireVersion = if (profile.starterCatalogGeneration == null) {
    LEGACY_WRITABLE_VERSION
} else {
    VERSION_WITH_STARTER_CATALOG_GENERATION
}
output.writeInt(wireVersion)
// existing fields
profile.starterCatalogGeneration?.let(output::writeInt)
```

- [ ] **Step 4: Promote `encodedSize` to the production preflight seam.** Change it from `private` to `internal`, keep all existing validation before allocation, validate marker membership, and add `Int.SIZE_BYTES` only when the marker is present. Remove `encodedSizeForTest`; tests call the same `encodedSize` WU-002c will consume. `encodeInternal` must continue to call `encodedSize(profile)` before allocating.

- [ ] **Step 5: Run codec tests GREEN.** The byte-identical v3 assertion and both exact-limit/+1 pairs are blocking; do not accept merely equal decoded models or an allocation-time failure. Construct boundary profiles from the shared `encodedSize` base and two legal app-data bundles so no individual length-prefixed field exceeds its existing 2 MiB limit.

---

## Task 4: Persist and restore the Android generation

**Files:**

- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/RiotController.kt`
- Test: `apps/android/app/src/test/kotlin/org/riot/evidence/PersistedProfileCodecTest.kt`
- Test: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/BindingSemanticsTest.kt`

- [ ] **Step 1: Write RED controller integration tests on the real bindings.** Extend `BindingSemanticsTest` to prove the host-observable contract:

  1. fresh `createSpace` and first-time `joinSpace` persist generation 2;
  2. sealed snapshots carrying `null`, explicit 1, and 2 all reopen successfully and keep the same marker in their next durable snapshot;
  3. an identityless legacy snapshot reopens successfully and remains `null`/v3 after its next permitted persist, proving it did not take the fresh marker-materializing path;
  4. `joinAdditionalCommunity` preserves the loaded snapshot's marker (`null`, 1, or 2) and sentinel alerts, installed apps, and app data rather than reconstructing a partial profile.

The Task 1 Rust white-box tests—not these host tests—prove the exact internal retained `Option<u8>`; no test-only production getter is added.

Run the focused connected test and confirm RED before changing the controller:

```text
cd apps/android
JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
ANDROID_HOME=$HOME/Library/Android/sdk \
./gradlew :app:connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.BindingSemanticsTest
```

If no emulator/device is available, record the exact `adb devices` evidence, continue every runnable gate, and treat the instrumentation test as a CI-required blocker rather than silently replacing it with a compile-only assertion.

- [ ] **Step 2: Mark only truly fresh base profiles as generation 2 and make existing-profile mutation lossless.** `createSpace` and first-time `joinSpace` construct `PersistedProfile(..., starterCatalogGeneration = 2)` before the first persist. `joinAdditionalCommunity` is an existing-profile mutation: replace direct reconstruction/persist with `mutatePersisted { snapshot -> snapshot.copy(space = PersistedSpace(...)) }`, using the controller's `persistLock`-serialized read-modify-write seam. This must preserve `alerts`, `identityState`, `installedApps`, `appData`, and `starterCatalogGeneration`; never materialize a marker on a grandfathered `null` profile.

- [ ] **Step 3: Forward the restored marker through UniFFI.** In `openProfile(snapshot)`, keep three distinct paths:

  - `snapshot == null`: existing fresh `openLocalProfileWithDatabase` API (generation 2);
  - `snapshot.identityState != null`: sealed restore with `snapshot.starterCatalogGeneration?.toUByte()`;
  - `snapshot.identityState == null`: new generation-aware local/database restore with `snapshot.starterCatalogGeneration?.toUByte()`.

Append this nullable conversion to the sealed call:

```kotlin
snapshot.starterCatalogGeneration?.toUByte()
```

to `openProfileFromSealedIdentityWithDatabase`. Do not merge the two `identityState == null` cases; an identityless persisted profile is not fresh.

- [ ] **Step 4: Confirm generated bindings are current, run host tests, then run controller integration GREEN.** Task 1 Step 7 is the required ABI/native build. If any Rust ABI changed after it, rerun the full script before this step. Otherwise a binding-only regeneration may be used as a drift check:

Run: `cargo run --locked -p xtask -- generate-bindings`

Then run:

```text
cd apps/android
JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
ANDROID_HOME=$HOME/Library/Android/sdk \
./gradlew :app:testDebugUnitTest
```

Expected: BUILD SUCCESSFUL. If UniFFI maps optional `u8` as nullable `UByte`, keep the conversion above; follow the generated signature rather than casting through a signed byte.

Then rerun the focused `connectedDebugAndroidTest` command from Step 1 and require the new controller behaviors to pass.

---

## Task 5: Full quality and compatibility gate

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-features`
- [ ] `cargo clippy --workspace --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features`
- [ ] `ANDROID_HOME=$HOME/Library/Android/sdk scripts/conference/build-native-core.sh` (binding generation plus all native libraries)
- [ ] Android full host-JVM unit suite with JDK 17 and `ANDROID_HOME`
- [ ] Focused iOS `BindingSemanticsTests`
- [ ] Focused Android `BindingSemanticsTest` on an emulator/device
- [ ] macOS `RiotKit-macOS` tests
- [ ] Coverage using the source of truth:

```text
scripts/web/coverage.sh
```

- [ ] Audit `git diff --check`, `git status --short`, and changed paths against the scope boundary.

---

## Definition of Done

- Fresh Rust, Apple, and Android profiles durably use generation 2.
- A missing marker remains the zero-byte durable representation of generation 1 on both native hosts.
- Both sealed-identity and identityless restore families receive the persisted optional generation; Rust white-box tests prove `None`, explicit 1, and 2 are retained exactly, while unknown generations fail closed.
- Android null-marker profiles remain wire v3 and decode→encode byte-identically; generation-bearing profiles use v4.
- Android `PersistedProfileCodec.encodedSize` is the exact production preflight, equals the actual byte count for v3 and v4, accepts exactly 4,194,240 bytes, and rejects +1 before allocation.
- Android controller integration tests prove fresh markers, sealed and identityless representation acceptance without durable upgrade, and lossless marker/alerts/apps/app-data preservation during `joinAdditionalCommunity`.
- No trust/app-data transaction or UI behavior is changed in this WU.
- Rust, bindings, Android, Apple, strict Clippy, and the coverage ratchet all pass.

## Execution note for WU-002c

WU-002c must call `PersistedProfileCodec.encodedSize(prospective)` while holding the shared authority/persistence lock before durable trust, app-data, or install growth. It must not call `encode` merely to discover size, duplicate the size formula, or materialize a generation marker on a grandfathered null/v3 profile.
