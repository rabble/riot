# Phase 0A — WU0 Report: Preflight, Contracts, Pins

- **Status:** PASS — G0 review findings closed 2026-07-10 (see "Second-review repair" at end); validator now enforces exact ceiling values and inspects the resolved feature graph
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

## Task 0 closure (2026-07-10)

Every reopening finding is individually closed:

1. **Validator now rejects the obsolete graph structurally.** `crates/xtask/src/main.rs` gained `validate_contents(root)` which parses TOML/JSON (no substring trust): requires `willow25 =0.6.0-alpha.3` (default-features off, `std` only, `drop_format` forbidden), `bab_rs =0.8.1` (default-features off, `william3`), direct `hifitime =4.3.0`, `panic = "unwind"`, the lockfile resolving exactly one willow25/bab_rs version with no `openmls`, the manifest's `cargo_lock_sha256` matching the actual lockfile bytes, a non-empty matching `william3_vectors_sha256`, the full ceilings table including the new store-charge/namespace/plan limits, fixture ownership, and report fields. Seven unit tests feed regressions independently (old willow pin, old bab_rs lock, stale lock hash, `panic = "abort"`, `drop_format` feature, missing vector hash/ceilings) and assert each specific failure line. `cargo test -p xtask`: 7/7. `cargo xtask validate-contracts` on the repo: PASS.
2. **Vectors now carry independent provenance.** `fixtures/willow/william3-vectors.json` (replacing the self-attested `.txt`) records input recipes, digests, bab_rs version, and per-vector provenance. Five vectors are cross-checked verbatim against the independently implemented `Deln0r/willow-go` corrected-WILLIAM3 commit `9d848ee` (fetched raw patch, not a summarizer transcription): `empty`, `single_byte_zero`, `hello world`, `exactly_1023_bytes`, `exactly_1025_bytes_two_chunks` — plus `nonzero_pattern_5000_bytes`, whose input (`00..fa` repeating) is byte-identical to Riot's `multi-block` recipe (`i mod 251`) and whose digest matches Riot's independently generated value exactly. Chunk-boundary coverage sits at 1023/1024±1 both sides. `crates/riot-conformance/tests/william3_vectors.rs` computes every digest through `bab_rs` directly (declared dev-dependency) and requires ≥1 cross-checked vector, sub-chunk and multi-chunk inputs, and byte-identity of the alert-golden payload with the codec fixture. A sixth willow25-path test (`public_william3_matches_frozen_vector_fixture`) proves willow25's `PayloadDigest` agrees with the same frozen vectors, closing the loop between the digest dependency and entry construction. One summarizer-fabricated hex value was caught by this process and discarded — the test disagreeing with a fake vector while agreeing with all real ones is itself evidence the executable check works.
3. **Corrected pins finished.** Direct `hifitime = "=4.3.0"` (matches the already-resolved transitive version). `panic = "unwind"` in the release profile (abort made the FFI catch/quarantine contract impossible). Lock regenerated; `cargo_lock_sha256` `8bb2eb1b112fcbdc83d6db046ce44d5e0e8476cd3477d9a35050ec605c367791`; `william3_vectors_sha256` `980192eb0ace7bea5e0fbebfe7351cd661aec4498a449e13d18780afb2ec88d2`. New xtask-only parser pins `toml =0.9.8`, `serde_json =1.0.145` (dev/validator layer, not the riot-core release graph).
4. **Five-target proof rerun with `--locked`**: workspace all-targets check plus `riot-core` on `aarch64-apple-ios-sim`, `aarch64-apple-ios`, `aarch64-linux-android`, `x86_64-linux-android` — all pass. Feature closure recorded at `build/evidence/wu0r-feature-tree.txt` (sha256 `0139028399715954cfdf0ce759c0557320df75638750bb5e310534408896fb2b`); negative search for `willow25 feature "drop_format"` and `bab_rs v0.[0-7].` finds nothing.
5. **This report** now separates platform PASS from corrected-dependency PASS and records the 0.5.0 rejection rationale above.

New ceilings frozen in the manifest: `retained_store_budget_bytes` 16 MiB, `namespace_views` 64, `store_charge_entry_bytes` 512, `store_charge_namespace_bytes` 256, `store_charge_receipt_bytes` 256, `store_charge_digest_reference_bytes` 32, `entry_reference_cap` 1024, `plan_tombstone_bytes` 256, `plans_per_preview` 64.

## Independent gate review (2026-07-10)

The required commands were rerun from `main` at reviewed HEAD `deb847a5741d6fbbeb598e6a10e8e99c67f1daa6`: `cargo xtask validate-contracts` PASS; `cargo test -p xtask` 7/7; `cargo test -p riot-conformance william3_` 2/2; `cargo test -p riot-core public_` 39/39; workspace clippy PASS with warnings denied.

G0 is nevertheless **REOPENED** because the validator is not regression-proof:

1. In an isolated worktree, changing `ceilings.artifact_bytes` from 8 MiB to 1 byte still produced `validate-contracts: PASS`. The validator checks ceiling presence/type, not the exact Revision 5 values. The real manifest also records `transaction_snapshot_bytes = 8 MiB` and omits the dedicated 4 KiB Entry / 64-byte signature ceilings while the design requires a 16 MiB next-snapshot charge and those component limits.
2. In the same isolated worktree, enabling `willow25/drop_format` in the `riot-core` dependency, regenerating `Cargo.lock`, and refreshing its recorded hash still produced `validate-contracts: PASS --locked`. The validator checks only the root workspace declaration, not the resolved workspace feature graph. The current real feature graph is clean, but the claimed regression gate does not preserve that fact.

Exact repair required before WU2:

- validate every frozen ceiling against its exact numeric/string value, including the missing component ceilings and 16 MiB transaction snapshot charge;
- inspect the resolved `cargo metadata`/feature graph (or an equivalent locked structural source) and reject `willow25/drop_format` regardless of which workspace member enables it;
- add regression tests for both accepted mutations above, then rerun the five mandatory commands and refresh the report/lock hashes if any frozen file changes.

## Next action

Keep platform preflight PASS, repair G0, and do not start Task 4/WU2 until an independent rerun records G0 PASS.

## Second-review repair (2026-07-10) — G0 findings closed

The independent review demonstrated two accepted regressions; both are now closed and guarded by tests.

1. **Exact ceiling values, not presence.** `EXPECTED_CEILINGS` in `crates/xtask/src/main.rs` pins all 34 ceilings to their exact Revision 5 values; a mutated value (the review's `artifact_bytes: 8 MiB → 1`) now fails with `must be exactly 8388608`. Regression test: `rejects_mutated_ceiling_value`. The manifest gained the three missing frozen ceilings: `entry_bytes` 4096, `signature_bytes` 64, and `transaction_snapshot_bytes` corrected to 16 MiB.
2. **Resolved feature-graph inspection.** `check_resolved_feature_graph` runs `cargo tree -p riot-ffi -e features --locked` and rejects `willow25 feature "drop_format"`, `openmls`, `riot-core feature "conformance"`, or any `bab_rs v0.x != 0.8.1` in the release closure — catching the review's "enable drop_format + refresh lock hash" regression that structural manifest checks alone missed. `check_crate_manifests` additionally scans every crate manifest for a crate-level `drop_format` enablement or a version override that escapes the workspace pin. Regression tests: `rejects_crate_level_drop_format_enablement`, `rejects_crate_level_version_override`. Verified the guard has teeth: the current riot-ffi closure shows only `riot-core feature "default"`, and a conformance leak renders as the exact guarded string.

Validator suite is now 10/10; `cargo xtask validate-contracts` prints "PASS (structural + resolved feature graph)". Lock hash refreshed after the `zeroize` and `conformance`-feature additions.

Note carried forward (PASS-WITH-NOTES from review, non-blocking): WILLIAM3 provenance is recognized by a string prefix rather than by re-validating the source commit; the frozen file is genuine (cross-checked against the raw willow-go patch), so this does not reopen G0.

Corrected next action: G0 PASS. Tasks 1–3 complete (see WU1 report); WU2/Task 4 may begin.
