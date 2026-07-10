# Phase 0A — WU0 Report: Preflight, Contracts, Pins

- **Status:** PLATFORM PASS / G0 REVISE (the 2026-07-10 WU0R implementation resolved the graph but did not close the executable evidence gate)
- **Owning work unit:** WU0
- **Date:** 2026-07-10
- **Elapsed agent-hours:** ~1.0 charged for WU0; combined WU0R+WU1 time is accounted separately in the ledger

## What was proven

1. **Frozen environment achieved and recorded.** Every pin in `fixtures/manifest.json` reflects an actually installed, verified version: Rust 1.95.0 (toolchain-pinned), Xcode 26.2/Swift 6.2.3, JDK 17.0.19, AGP 9.0.1, Gradle 9.1.0 (wrapper committed), Build-Tools 36.0.0, NDK 28.2.13676358, Platform-Tools 37.0.0, Emulator 36.6.11, cmdline-tools zip 14742923 (sha256 recorded), system image `android-36;google_apis;arm64-v8a` revision 7 (`source.properties` sha256 recorded).
2. **The pinned arm64 AVD (`riot-phase0a`) boots** headless and reports `sys.boot_completed`.
3. **A blank instrumentation test passes on it**: `BlankInstrumentationTest.instrumentationContextTargetsEvidencePackage`, run via `gradle :app:connectedDebugAndroidTest` — 1 test, BUILD SUCCESSFUL.
4. **Empty `riot-ffi` compiles in release for all four runtime targets**: `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android`, `x86_64-linux-android` (NDK linkers wired in `.cargo/config.toml`).
5. **The full pinned dependency graph resolves and compiles**: 230 packages locked; `willow25 =0.5.0`, `uniffi =0.32.0`, `minicbor =2.2.2`, `cddl-cat =0.7.1`, `sha2 =0.10.9`, `ed25519-dalek =2.2.0` (restricted features), `rand_core =0.6.4`. `Cargo.lock` committed, sha256 in the manifest.
6. **Contract validator follows TDD**: RED run enumerated the absent contracts (`schemas/alert.cddl`, `fixtures/manifest.json`); GREEN run passes with all pins, ceilings, fixture-ownership, and report-field requirements present. Command: `cargo xtask validate-contracts`.

## Post-gate Willow correction

The platform/toolchain evidence above remains valid. The dependency claim does not: follow-up inspection of the archived GitHub repository, canonical Codeberg repository, crates, and changelogs found that `willow25 =0.5.0` resolves `bab_rs 0.6.x`, while upstream states that every `bab_rs` version before 0.7 computes incorrect WILLIAM3 digests and that 0.8 is the corrected construction.

Before Willow implementation, WU0R must pin `willow25 =0.6.0-alpha.3` with default features disabled and `std` enabled, force `bab_rs =0.8.1`, regenerate the lock/hash, add corrected WILLIAM3 vectors, and rerun the five-target compile/feature checks. See `docs/research/2026-07-10-willow-implementation-audit.md` and Revision 5 of the evidence-sprint design.

## Exact commands (as run)

```
rustup toolchain install 1.95.0
rustup target add --toolchain 1.95.0 aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android x86_64-linux-android
brew install openjdk@17
sdkmanager --install "platform-tools" "emulator" "build-tools;36.0.0" "system-images;android-36;google_apis;arm64-v8a"
avdmanager create avd -n riot-phase0a -k "system-images;android-36;google_apis;arm64-v8a" --device pixel_7
emulator -avd riot-phase0a -no-window -no-audio -no-boot-anim -no-snapshot
cargo xtask validate-contracts        # RED then GREEN
cargo build --workspace
cargo build -p riot-ffi --release --target <each of 4 targets>
gradle :app:connectedDebugAndroidTest # in apps/android, JAVA_HOME=JDK17
gradle wrapper --gradle-version 9.1.0
```

## Evidence paths and hashes

- `fixtures/manifest.json` — frozen pins, ceilings, fixture ownership, report fields.
- `Cargo.lock` — sha256 `5d5ea10add923766d2ec0a6021b958540ed1053a963e7ee450c7348b992200a5`.
- cmdline-tools zip — sha256 `ed304c5ede3718541e4f978e4ae870a4d853db74af6c16d920588d48523b9dee`.
- system image `source.properties` — sha256 `f53d5fbbb89420d911b9e5ed9243aff60caad24132a76e649efc8dfd96295731`.
- `apps/android/` — host app + passing blank instrumentation test.

## Deviations and notes

- The host already had an Android SDK with the exact pinned NDK; missing components were installed rather than assumed absent (the design allowed either).
- Gradle dependency locking is deferred to WU3 with the real Android host app, as recorded in the manifest (`gradle_locks_sha256: pending`).
- Deterministic-provider and forbidden-feature closure scans run in WU4 as designed; nothing in the current graph includes OpenMLS or group code.

## WU0R implementation and review reopening (2026-07-10)

The implementation updated the dependency graph and produced useful compile evidence:

1. Workspace pins updated: `willow25 = "=0.6.0-alpha.3"` (default-features off, `std` only, `drop_format` excluded — verified feature-gated in upstream Cargo.toml) and direct `bab_rs = "=0.8.1"` (default-features off, `william3`). Stable 0.5.0 rejected because it resolves `bab_rs 0.6.x`, which upstream's changelog documents as computing incorrect WILLIAM3 digests.
2. `Cargo.lock` regenerated; new sha256 `8513394ad473c639030d58a85f7dd88571700ba8b38adfae7bc3a5b0061e822d` recorded in the manifest. Verified in-graph: `willow25 0.6.0-alpha.3`, `bab_rs 0.8.1`.
3. WILLIAM3 outputs frozen at `fixtures/willow/william3-vectors.txt` (empty, short 4-byte, 700-byte partial-block, 5000-byte multi-block), guarded by `public_william3_golden_vectors`.
4. Five-target compile probe rerun (host dev + 4 release cross-targets, all pass). `cargo tree -p riot-ffi -e features` recorded at `fixtures/feature-closure.txt`; contains no `openmls` and no `willow25/drop_format`.
5. This report updated with the 0.5.0 rejection rationale (step 1 above).

Deviation recorded: `pollster =0.4.0` and `ufotofu =0.12.4` added as direct workspace pins. Both were already in the locked transitive graph via willow25; the direct pins let riot-core drive the async ufotofu codec traits synchronously and change no resolved versions.

The Revision 5 review invalidated the G0 PASS claim without discarding the useful compile results:

- `cargo xtask validate-contracts` still accepts the old version/lock and does not verify the vector hash, independent vector provenance, Drop feature exclusion, or release panic strategy;
- the frozen vector file records only outputs generated by the same dependency under test;
- `Cargo.toml` still uses `panic = "abort"`, which makes the required FFI panic catch/quarantine behavior impossible;
- the manifest does not yet freeze the namespace-view, digest-reference, or comprehensive retained-store charge limits;
- WU0R and WU1 time was reported as one combined approximately 2.0-hour estimate, so the ledger charges that combined duration once and charges all future repair time separately.

G0 remains REVISE until Task 0 of `docs/superpowers/plans/2026-07-10-riot-phase0a-public-kernel.md` passes. WU2 must not continue on the strength of this report.

## Next action

Repair and rerun G0, then repair and rerun G1. Preserve the currently green fixtures as implementation evidence, but do not claim GO or continue WU2 until both reopened gates PASS.
