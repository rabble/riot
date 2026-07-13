# WebTiles-Inspired Microapp V2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adopt the strongest WebTiles ideas without weakening Riot's offline, community-governed model: cryptographically trustworthy app lineage, machine-enforced capabilities, richer inert metadata, a fast local preview loop, portable package adapters, and an eventual content-addressed resource graph.

**Architecture:** Deliver this as four independently shippable phases. Phase 1 introduces an additive, author-signed v2 release envelope while preserving every v1 app byte and app ID; only verified v2 releases participate in publisher lineage, and exact-version organizer approval remains mandatory. Phase 2 improves developer and directory experience using the verified package boundary. Phase 3 adds source adapters, beginning with read-only `.tile` import, while every imported app still passes Riot verification and trust. Phase 4 considers per-resource addressing and host-mediated intents only after measured prototypes and privacy review; neither may add runtime network access.

**Tech Stack:** Rust 2021, canonical `minicbor`, `willow25`, Ed25519 via the existing Willow subspace key, UniFFI 0.32, Swift/SwiftUI/WKWebView, Kotlin/Android WebView, plain HTML/CSS/JavaScript, CAR/MASL only behind an adapter boundary.

> **Plan-review status:** ESCALATED after the required maximum of three adversarial iterations (Feasibility/Completeness/Scope: `FAIL/FAIL/PASS` in each round). The post-gate revision incorporates the final findings—early FFI projection, all constructor sites, RED admission-path runs, iOS v2 restart coverage, explicit production activation owners, stable experimental CLI behavior, and complete vector projections—but that revision has not received a fourth review and is not gate-approved. User override or a manual continuation of review is required before execution.

---

## Product and security invariants

1. An app version remains immutable and content-derived. A new bundle always gets a new `app_id` and a new organizer approval.
2. Organizer approval is governance, not sandboxing. Approved code is still treated as hostile by the runtime.
3. A carrier may copy exact app bytes but cannot claim to be the original publisher or forge an official update.
4. V1 bundles, starter artifacts, installed state, and persisted trust continue to decode with their existing IDs. V1 apps are labelled legacy and never gain cryptographically verified publisher lineage.
5. Built-ins stay deterministic and key-free. They are verified by compiled-in provenance and exact bytes, not by committing a fixture private key.
6. No phase adds arbitrary network access to a running microapp. The bridge remains the only app I/O surface.
7. No phase gives one app direct access to another app's Willow prefix.
8. Full IDs and signatures are retained internally; no Nostr or Willow identifier is truncated.

## Delivery map

| Phase | Outcome | Depends on | Release gate |
|---|---|---|---|
| 1. Trustworthy v2 foundation | Author-signed releases, explicit lineage, typed capabilities, native enforcement, v1 compatibility | Current runtime | Required before any other phase |
| 2. Builder and directory experience | Rich inert cards and `riot-app dev` preview | Phase 1 verified package/capabilities | Can ship independently |
| 3. Portable sources | Source-neutral loader boundary and read-only WebTiles `.tile` import | Phase 1 only; Phase 2's preview harness is optional reuse | Must remain offline at runtime |
| 4. Resource graph and intents | Measured per-resource prototype and privacy-reviewed host intents | Phases 1–3 | Separate design reviews; no automatic rollout |

The detailed executable plan below covers Phase 1. The later phases have bounded charters and exit criteria at the end so they can be converted into separate implementation plans without reopening foundational decisions.

## Phase 1 file map

### New files

- `crates/riot-core/src/apps/capability.rs` — stable numeric capability IDs, host-owned labels, and operation mapping.
- `crates/riot-core/src/apps/release.rs` — canonical v2 release codec, author proof, lineage fields, and metadata validation.
- `crates/riot-core/src/apps/package.rs` — one verified package boundary normalizing v1 and v2 pairs.
- `crates/riot-core/tests/apps_release.rs` — v2 canonicality, signature, lineage, and hostile-input tests.
- `crates/riot-core/tests/apps_package.rs` — v1 compatibility and normalized package tests.
- `docs/specs/riot-microapps-v2.md` — public wire/runtime contract and WebTiles prior-art attribution.
- `fixtures/apps/v2-release-vector.json` — readable conformance projection with complete IDs.
- `fixtures/apps/v2-release-vector.cbor` — canonical signed release vector.
- `fixtures/apps/v2-release-vector.bundle.cbor` — exact canonical bundle bytes bound by the release vector.

### Existing files changed

- `Cargo.toml`, `crates/riot-core/Cargo.toml` — direct `signature = "=2.2.0"` dependency already present transitively in `Cargo.lock`.
- `crates/riot-core/src/willow/identity.rs` — narrow, infallible domain-specific release signing method; no secret accessor or new `WillowError` variant.
- `crates/riot-core/src/apps/mod.rs` — export the new modules and error variants.
- `crates/riot-core/src/apps/manifest.rs` — preserve the v1 codec unchanged and document it as legacy.
- `crates/riot-core/src/apps/index.rs` — verify pairs through `VerifiedAppPackage` and retain normalized descriptors.
- `crates/riot-core/src/import/bundle.rs` — admit strict v1/v2 manifest syntax while preserving pending-pair semantics.
- `crates/riot-core/src/apps/directory.rs` — replace author-name timestamp inference with verified family/predecessor edges.
- `crates/riot-core/src/apps/starter.rs` — normalize v1 built-ins without pretending they carry publisher proof.
- `crates/riot-core/tests/apps_{manifest,index_io,directory,starter}.rs`, `crates/riot-core/tests/core_import_app_index_entries.rs` — compatibility, import, partial-arrival, and lineage regression coverage.
- `crates/riot-app-cli/src/lib.rs`, `crates/riot-app-cli/src/main.rs` — v2 manifest parsing, family generation, signing, and inspection.
- `crates/riot-app-cli/tests/cli_pack.rs`, `crates/riot-app-cli/tests/fixtures/hello-app/riot-app.json` — v1/v2 CLI coverage.
- `crates/riot-ffi/src/apps_ffi.rs`, `crates/riot-ffi/src/mobile_state.rs` — expose normalized capability and release status fields.
- `crates/riot-ffi/tests/apps_contract.rs` — FFI compatibility and capability projection.
- `apps/ios/Riot/Apps/AppRuntimePolicy.swift`, `apps/ios/Riot/Apps/AppBridgeController.swift`, `apps/ios/Riot/Apps/AppRuntimeView.swift`, `apps/ios/Riot/Core/ProfileRepository.swift` — owned activation switch, restore behavior, and operation-level capability enforcement.
- `apps/ios/Riot/Apps/AppReviewSheet.swift`, `apps/ios/Riot/Directory/DirectoryModel.swift`, `apps/ios/Riot/Directory/DirectoryView.swift` — host-owned capability copy and verified/legacy release status.
- `apps/ios/RiotTests/{AppRuntimeHost,AppRepository,AppSyncReplication,DirectoryRepository,DirectoryStorefront}Tests.swift` — denied-operation, restart, direct-constructor, generated-record, and compatibility tests.
- `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`, `apps/android/app/src/main/kotlin/org/riot/evidence/apps/{AppRuntimePolicy,RiotJsBridge,RiotJsShim,RiotAppsController,DirectoryController}.kt` — own activation, pass policy at the actual bridge construction site, enforce operations, preserve `watch` semantics, restore safely, and render normalized listing fields.
- `apps/android/app/src/test/kotlin/org/riot/evidence/apps/{RiotJsBridgeTest,RiotAppsControllerTest,DirectoryControllerTest,InstalledAppsStoreTest}.kt` — JVM enforcement and generated-record tests; create `RiotAppsControllerTest.kt` and any other missing target only where it does not already exist.
- `apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppRuntimeEndToEndTest.kt` — compile and exercise the production bridge constructor with explicit runtime policy.
- `crates/riot-conformance/tests/microapp_v2_vectors.rs`, `crates/riot-conformance/Cargo.toml` — independent vector verification.
- `fixtures/manifest.json`, `crates/xtask/src/main.rs` — pin v2 vector hashes and validate their presence.
- `SERVICE-INVENTORY.md`, `COLLABORATION.md` — record the new package boundary and verified claims after tests pass.

## V2 wire contract locked by this plan

`AppReleaseV2` is a definite-length canonical CBOR map with exactly 20 ascending integer keys:

| Key | Field | Encoding and bound |
|---:|---|---|
| 0 | `schema` | integer `2` |
| 1 | `name` | UTF-8, 1–80 bytes |
| 2 | `description` | UTF-8, 1–500 bytes |
| 3 | `version` | UTF-8, 1–32 bytes |
| 4 | `author_namespace_id` | 32 bytes |
| 5 | `author_subspace_id` | 32 bytes |
| 6 | `author_namespace_kind` | integer `0` (communal) in Phase 1; owned releases are rejected until Riot has an owned-space author/write-capability proof |
| 7 | `author_signing_key_id` | 32 bytes; must equal subspace ID for the current Willow identity |
| 8 | `family_id` | 32 random bytes generated once by `riot-app init`; never derived from name |
| 9 | `previous_app_id` | CBOR null or 32 bytes |
| 10 | `capabilities` | sorted, duplicate-free array of numeric IDs |
| 11 | `entry_point` | relative bundled path, 1–256 bytes |
| 12 | `icon_path` | null or exact bundled path, 1–256 bytes |
| 13 | `screenshot_path` | null or exact bundled path, 1–256 bytes |
| 14 | `categories` | at most 8 strings, each 1–32 bytes |
| 15 | `theme_rgb` | null or integer `0x000000..0xffffff` |
| 16 | `preferred_width` | null or integer `1..4096` |
| 17 | `preferred_height` | null or integer `1..4096` |
| 18 | `bundle_digest` | 32 bytes using existing `riot/app-bundle/v1` digest |
| 19 | `author_signature` | 64-byte Ed25519 signature |

The signature input is:

```text
SHA256(
  "riot/app-release-signature/v2" ||
  u32be(unsigned_fields_bytes.len()) ||
  unsigned_fields_bytes
)
```

where `unsigned_fields_bytes` is the canonical 19-field map containing keys `0..18`. The v2 app ID is:

```text
SHA256("riot/app-id/v2" || u32be(release_bytes.len()) || release_bytes)
```

Verification must reject any non-communal namespace kind in Phase 1, reject a namespace ID whose intrinsic bits are not communal, require `author_signing_key_id == author_subspace_id`, and require field 7 to decode as a valid Willow subspace public key. It then compares field 18 with the digest of the exact bundle bytes and verifies field 19 with field 7. A signature made by a key other than the embedded author, a signature over another bundle, noncanonical metadata, a changed predecessor, or changed capabilities is invalid. These checks are what permit `publisher_verified = true`; a valid carrier envelope alone never does. Owned namespace publication is deliberately deferred because the current owned module exposes a root namespace but no authorized owned writer.

Capability IDs are stable protocol values:

```rust
#[repr(u8)]
pub enum AppCapability {
    DataRead = 0,
    DataWrite = 1,
    MemberProfile = 2,
}
```

- `get`, `list`, and `watch` require `DataRead`.
- `put` requires `DataWrite`.
- `whoami` and `profile` require `MemberProfile`.
- V1 packages receive the existing bridge behavior through `CapabilityPolicy::LegacyV1`; this is compatibility, not inferred authority.
- V2 packages fail closed on unknown, duplicate, or unsorted capability IDs.
- Native UI labels come from the host registry, never from manifest prose.

## Task 0: Establish the blocking coverage baseline before feature work

**Files:**
- Read: `.coverage-thresholds.json`
- Read: existing Rust sources and tests reported by the coverage tools

- [ ] **Step 1: Run the source-of-truth command exactly as configured**

```bash
jq -r '.enforcement.command' .coverage-thresholds.json | sh
```

This is mandatory even though the current Tarpaulin command exposes only aggregate/line coverage and does not implement branch coverage. Record its exact output; never describe this command alone as proof of all four configured dimensions.

- [ ] **Step 2: Measure the other configured Rust dimensions without rounding**

```bash
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-features --branch --json --summary-only --output-path target/llvm-cov-summary.json
jq -e '.data | length == 1 and (.data[0].totals | (.lines.covered == .lines.count) and (.functions.covered == .functions.count) and (.regions.covered == .regions.count) and (.branches.covered == .branches.count))' target/llvm-cov-summary.json >/dev/null
```

For Rust, this plan explicitly maps the repository's `statements` dimension to LLVM code regions; lines, functions, regions/statements, and branches must each have exact covered/count equality. If either gate is RED because of pre-existing repository debt, stop before Task 1 and coordinate with the owner of the existing coverage-gate work. Do not lower thresholds, add exclusions, silently edit the shared `.coverage-thresholds.json`, or expand this plan into unrelated test debt.

## Task 1: Add a narrow author-release signing primitive

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/riot-core/Cargo.toml`
- Modify: `crates/riot-core/src/willow/identity.rs`
- Test: `crates/riot-core/src/willow/identity.rs`

- [ ] **Step 1: Write failing unit tests for domain-separated signing**

Add tests proving the existing author can sign one 32-byte release digest, its full public subspace verifies it, a changed digest fails, and the API returns only 64 public signature bytes:

```rust
#[test]
fn app_release_signature_is_domain_bound_and_publicly_verifiable() {
    let author = generate_communal_author().unwrap();
    let digest = [0x42; 32];
    let signature = author.sign_app_release_digest(&digest);
    assert_eq!(signature.len(), 64);
    assert!(verify_app_release_digest(
        author.identity().signing_key_id,
        digest,
        signature,
    ));
    assert!(!verify_app_release_digest(
        author.identity().signing_key_id,
        [0x43; 32],
        signature,
    ));
}
```

- [ ] **Step 2: Run the focused test and confirm RED**

Run: `cargo test -p riot-core app_release_signature_is_domain_bound_and_publicly_verifiable`

Expected: compile failure because `sign_app_release_digest` and `verify_app_release_digest` do not exist.

- [ ] **Step 3: Pin the already-resolved signature trait dependency**

Add `signature = "=2.2.0"` under `[workspace.dependencies]` and `signature.workspace = true` to `riot-core`. Do not update unrelated dependencies.

- [ ] **Step 4: Implement only the narrow signing and verification API**

Use `signature::{Signer, Verifier}` with Willow's existing `SubspaceSecret`/`SubspaceId`. Do not expose the secret, a generic signing oracle, or a raw-key constructor:

```rust
pub fn sign_app_release_digest(&self, digest: &[u8; 32]) -> [u8; 64] {
    let signed: willow25::prelude::SubspaceSignature = self.subspace_secret.sign(digest);
    let raw: &ed25519_dalek::Signature = (&signed).into();
    raw.to_bytes()
}

pub fn verify_app_release_digest(
    signing_key_id: [u8; 32],
    digest: [u8; 32],
    signature_bytes: [u8; 64],
) -> bool {
    let key = willow25::prelude::SubspaceId::from_bytes(&signing_key_id);
    let signature = willow25::prelude::SubspaceSignature::from(signature_bytes);
    key.verify(&digest, &signature).is_ok()
}
```

Use the infallible `Signer::sign` implementation already provided by Willow. Do not add a `WillowError` variant: doing so would unnecessarily change exhaustive FFI error mappings for an operation that cannot fail.

- [ ] **Step 5: Run unit tests and lint**

Run:

```bash
cargo test -p riot-core app_release_signature_
cargo clippy -p riot-core --all-features --all-targets -- -D warnings
```

Expected: tests pass and Clippy emits no warnings.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/riot-core/Cargo.toml crates/riot-core/src/willow/identity.rs Cargo.lock
git commit -m "feat(apps): add domain-bound release signing"
```

## Task 2: Implement typed capabilities and the canonical v2 release codec

**Files:**
- Create: `crates/riot-core/src/apps/capability.rs`
- Create: `crates/riot-core/src/apps/release.rs`
- Create: `crates/riot-core/tests/apps_release.rs`
- Modify: `crates/riot-core/src/apps/mod.rs`
- Modify: `crates/riot-core/Cargo.toml`

- [ ] **Step 1: Register the integration test target with conformance-only author construction**

Add:

```toml
[[test]]
name = "apps_release"
required-features = ["conformance"]
```

- [ ] **Step 2: Write RED tests for the exact wire contract**

Cover canonical round-trip, stable capability numbers, an author-signed communal release, altered bundle digest, altered family ID, altered predecessor, altered capabilities, signer-versus-embedded-author mismatch, signing-key-versus-subspace mismatch, communal-kind/owned-ID mismatch, rejection of every nonzero/owned namespace kind, unknown/duplicate/unsorted capabilities, wrong field order, indefinite maps/arrays, trailing bytes, oversized metadata, invalid metadata resource paths, and invalid Ed25519 public keys.

The central happy-path assertion must be:

```rust
let unsigned = sample_unsigned_release(bundle_digest);
let release = AppReleaseV2::sign(unsigned, &author).unwrap();
let encoded = encode_release_v2(&release).unwrap();
let decoded = decode_and_verify_release_v2(&encoded, &bundle_bytes).unwrap();
assert_eq!(decoded.release, release);
assert_eq!(decoded.app_id, app_id_for_v2(&encoded));
assert_eq!(decoded.capabilities, [AppCapability::DataRead, AppCapability::DataWrite]);
```

- [ ] **Step 3: Run and confirm RED**

Run: `cargo test -p riot-core --features conformance --test apps_release`

Expected: compile failure because the new modules and types do not exist.

- [ ] **Step 4: Implement the stable capability registry**

`AppCapability::from_wire`, `wire_id`, `developer_name`, and `review_label` must be exhaustive. Use these host-owned labels:

```rust
match self {
    Self::DataRead => "Read this tool's information in this space",
    Self::DataWrite => "Change this tool's information in this space",
    Self::MemberProfile => "Show the names people in this space have chosen",
}
```

Do not accept arbitrary strings as v2 capabilities.

- [ ] **Step 5: Implement the v2 codec exactly as locked above**

Follow the strict style in `apps/manifest.rs`: definite lengths, ascending numeric keys, allocation bounds before allocation, no unknown keys, no trailing bytes, and re-encode equality as the final canonicality proof.

Expose:

```rust
pub struct UnsignedAppReleaseV2 { /* keys 1..18, schema implied */ }
pub struct AppReleaseV2 {
    pub unsigned: UnsignedAppReleaseV2,
    pub author_signature: [u8; 64],
}
pub struct VerifiedReleaseV2 {
    pub release: AppReleaseV2,
    pub app_id: AppId,
    pub capabilities: Vec<AppCapability>,
}

pub fn encode_unsigned_release_v2(value: &UnsignedAppReleaseV2) -> Result<Vec<u8>, AppsError>;
pub fn encode_release_v2(value: &AppReleaseV2) -> Result<Vec<u8>, AppsError>;
pub fn decode_and_verify_release_v2(bytes: &[u8], bundle: &[u8]) -> Result<VerifiedReleaseV2, AppsError>;
pub fn app_id_for_v2(release_bytes: &[u8]) -> AppId;
```

`AppReleaseV2::sign` computes the unsigned digest, calls Task 1's narrow signer, and never handles raw secret bytes.

- [ ] **Step 6: Run focused and hostile tests**

Run:

```bash
cargo test -p riot-core --features conformance --test apps_release
cargo test -p riot-core --test apps_codec_hostile
cargo clippy -p riot-core --all-features --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/riot-core/src/apps/capability.rs crates/riot-core/src/apps/release.rs crates/riot-core/src/apps/mod.rs crates/riot-core/tests/apps_release.rs crates/riot-core/Cargo.toml
git commit -m "feat(apps): add signed v2 release codec"
```

## Task 3: Centralize v1/v2 package verification without changing v1 IDs

**Files:**
- Create: `crates/riot-core/src/apps/package.rs`
- Create: `crates/riot-core/tests/apps_package.rs`
- Modify: `crates/riot-core/src/apps/mod.rs`
- Modify: `crates/riot-core/src/apps/manifest.rs`
- Modify: `crates/riot-core/src/apps/index.rs`
- Modify: `crates/riot-core/src/apps/directory.rs`
- Modify: `crates/riot-core/src/apps/starter.rs`
- Modify: `crates/riot-core/src/import/bundle.rs`
- Modify: `crates/riot-core/tests/apps_{manifest,index_io,directory,starter}.rs`
- Modify: `crates/riot-core/tests/core_import_app_index_entries.rs`
- Modify: `crates/riot-core/Cargo.toml`
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/src/apps_ffi.rs`
- Modify: `crates/riot-ffi/tests/apps_contract.rs`
- Modify: `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- Modify: `apps/android/app/src/test/kotlin/org/riot/evidence/apps/{DirectoryControllerTest,InstalledAppsStoreTest}.kt`

- [ ] **Step 1: Write compatibility tests before changing production call sites**

Pin every current `STARTER_CATALOG` v1 app ID, prove `verify_app_pair` still returns those exact IDs, and prove the new detailed verifier reports `ManifestGeneration::LegacyV1` with no family or predecessor.

Add a signed-v2 case that reports `ManifestGeneration::SignedV2`, verified publisher identity, family, predecessor, capabilities, and metadata. Add scan and import-admission cases for both arrival orders. A manifest-first v2 release may appear only in `pending_manifests`; until its exact bundle arrives it must never enter `apps`, directory listings, lineage, trust, install, share, or launch decisions. Prove `import/bundle.rs` accepts a canonical v2 manifest entry syntactically while retaining it as pending, rejects malformed/noncanonical v2 bytes, and promotes only after the exact bundle arrives.

- [ ] **Step 2: Write the carrier-forgery regression**

Construct a v2 release whose author fields claim Alice but whose signature is made by Mallory. Wrap it in valid Willow entries signed by a carrier. Assert the package verifier rejects it even though carrier signatures and bundle digest are valid.

- [ ] **Step 3: Add RED integration tests for every admission path**

Before production edits, add the import cases to `core_import_app_index_entries.rs` and the four FFI cases to `apps_contract.rs`: direct v2 `install_app`, `install_from_directory`, share/re-carriage followed by receiver admission, and restart-style re-admission of persisted exact bytes. Each asserts stable app ID, descriptor, publisher proof, and capability policy; mutating either package half must fail.

- [ ] **Step 4: Run every new test target and confirm RED**

Run:

```bash
cargo test -p riot-core --features conformance --test apps_package
cargo test -p riot-core --all-features --test apps_index_io
cargo test -p riot-core --all-features --test core_import_app_index_entries
cargo test -p riot-ffi --all-features --test apps_contract v2_
```

Expected: compile failures because `VerifiedAppPackage`, v2 import syntax, and normalized FFI fields do not exist. Do not proceed until all four targets have produced RED evidence.

- [ ] **Step 5: Implement normalized package types**

Use one descriptor downstream:

```rust
pub enum ManifestGeneration { LegacyV1, SignedV2 }

pub enum CapabilityPolicy {
    LegacyV1,
    Explicit(Vec<AppCapability>),
}

pub struct AppDescriptor {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: AuthorIdentity,
    pub entry_point: String,
    pub family_id: Option<[u8; 32]>,
    pub previous_app_id: Option<AppId>,
    pub capability_policy: CapabilityPolicy,
    pub icon_path: Option<String>,
    pub screenshot_path: Option<String>,
    pub categories: Vec<String>,
    pub theme_rgb: Option<u32>,
    pub preferred_size: Option<(u16, u16)>,
}

pub struct VerifiedAppPackage {
    pub app_id: AppId,
    pub generation: ManifestGeneration,
    pub publisher_verified: bool,
    pub descriptor: AppDescriptor,
}
```

`verify_app_package` first identifies v2 by the integer value at key 0; otherwise it invokes the byte-for-byte existing v1 decoder and ID function. Never attempt “v2 then fall back to v1” after a v2-shaped value fails verification.

Replace v1-only fields in `IndexedApp` with `descriptor: AppDescriptor` plus `generation` and `publisher_verified`. Change `PendingManifest` to retain the exact raw manifest/release bytes, claimed ID, carrier coordinates, timestamp, and a `PendingGeneration` discriminator. Pending v2 parsing may prove canonical syntax and the embedded publisher signature for display as “still arriving,” but it must be named `signature_verified`, not `publisher_verified`: the latter is set only by `verify_app_package` after field 18 matches the exact bundle. Do not project pending metadata into `AppListing`.

- [ ] **Step 6: Project normalized package facts through FFI**

Add these required fields to `InstalledAppRecord` in this task, before any FFI path assertions can pass:

```rust
pub manifest_generation: String,
pub capability_ids: Vec<String>,
pub capability_review_labels: Vec<String>,
pub legacy_capability_policy: bool,
pub publisher_verified: bool,
```

Update every current Swift/Kotlin hand-written `InstalledAppRecord` constructor with explicit legacy values. This is data projection only; native v2 install/persist/trust/launch remains disabled until Task 6.

- [ ] **Step 7: Route every pair admission path through the package verifier**

Make `index::verify_app_pair` a compatibility wrapper returning only `.app_id`. Update `import/bundle.rs` so the admission gate recognizes strict v1 or v2 manifest syntax instead of calling the v1-only decoder. Update publish, scan, starter verification, directory assembly, install, share, and FFI install paths to consume `VerifiedAppPackage` or its normalized `AppDescriptor` where they need metadata. Delete duplicate entry-point/digest checks only after the new boundary covers them. Preserve manifest-first publication as resumable partial arrival, but promote it from pending to indexed only when the exact pair verifies.

Make the RED assertions from Step 3 pass without weakening them. These tests are the release barrier's core-side evidence, not authorization to mount v2 in an unenforcing host.

- [ ] **Step 8: Run compatibility suites and refactor only while GREEN**

Run:

```bash
cargo test -p riot-core --all-features --test apps_manifest
cargo test -p riot-core --all-features --test apps_package
cargo test -p riot-core --all-features --test apps_index_io
cargo test -p riot-core --all-features --test apps_directory
cargo test -p riot-core --all-features --test apps_starter
cargo test -p riot-core --all-features --test core_import_app_index_entries
cargo test -p riot-ffi --all-features --test apps_contract
```

Expected: all v1 IDs remain unchanged; all tests pass. Once GREEN, deduplicate pair routing behind `verify_app_package`, then re-run the entire command block.

- [ ] **Step 9: Commit**

```bash
git add crates/riot-core/src/apps crates/riot-core/src/import/bundle.rs crates/riot-core/tests/apps_manifest.rs crates/riot-core/tests/apps_package.rs crates/riot-core/tests/apps_index_io.rs crates/riot-core/tests/apps_directory.rs crates/riot-core/tests/apps_starter.rs crates/riot-core/tests/core_import_app_index_entries.rs crates/riot-core/Cargo.toml crates/riot-ffi/src/apps_ffi.rs crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/apps_contract.rs apps/ios/RiotTests/DirectoryStorefrontTests.swift apps/android/app/src/test/kotlin/org/riot/evidence/apps/DirectoryControllerTest.kt apps/android/app/src/test/kotlin/org/riot/evidence/apps/InstalledAppsStoreTest.kt
git commit -m "refactor(apps): centralize verified package admission"
```

## Task 4: Replace heuristic updates with signed lineage edges

**Files:**
- Modify: `crates/riot-core/src/apps/directory.rs`
- Modify: `crates/riot-core/tests/apps_directory.rs`
- Modify: `crates/riot-ffi/src/apps_ffi.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/tests/apps_contract.rs`
- Modify: `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- Modify: `apps/android/app/src/test/kotlin/org/riot/evidence/apps/DirectoryControllerTest.kt`
- Modify: `apps/android/app/src/test/kotlin/org/riot/evidence/apps/InstalledAppsStoreTest.kt`

- [ ] **Step 1: Write RED directory tests for lineage semantics**

Cover:

- a signed v2 child with the same verified author and family and `previous_app_id == parent.app_id` creates one update edge;
- rename does not break lineage;
- same name does not create lineage;
- different author cannot extend the family;
- wrong predecessor does not supersede the local app;
- missing predecessor remains a normal independent listing until the predecessor arrives;
- two children of one predecessor are both surfaced as candidates, never silently ranked by timestamp;
- a cycle is rejected from lineage projection;
- trust remains attached only to exact `app_id`.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test -p riot-core --test apps_directory`

Expected: failures because the current code groups by `(author signing key, name)` and timestamp.

- [ ] **Step 3: Change the listing contract additively**

Retain `superseded_by` for one release as a deprecated field set to `None`. Add:

```rust
pub family_id: Option<[u8; 32]>,
pub previous_app_id: Option<AppId>,
pub update_candidates: Vec<AppId>,
pub publisher_verified: bool,
pub legacy_package: bool,
```

Build edges only when child and parent are signed v2, share the same full author signing key and family ID, and the child explicitly names the parent's exact ID. Sort candidate IDs lexicographically for deterministic output.

- [ ] **Step 4: Project the fields through UniFFI**

Add raw full-byte vectors to `DirectoryListing`; never convert them to shortened display IDs. Keep existing fields for source compatibility.

Because UniFFI record initializers require every field, update the hand-written Swift/Kotlin test fixtures that construct `DirectoryListing` or `InstalledAppRecord` directly. Give legacy fixtures explicit `publisher_verified = false`, empty family/predecessor/capability collections, and `legacy_package = true`; do not let generated defaults conceal compatibility behavior.

- [ ] **Step 5: Run core and FFI tests**

Run:

```bash
cargo test -p riot-core --test apps_directory
cargo test -p riot-ffi --all-features --test apps_contract
cargo xtask generate-bindings
cargo xtask validate-contracts
scripts/conference/build-native-core.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/DirectoryStorefrontTests -derivedDataPath build/ios-derived
cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew :app:testDebugUnitTest --tests '*DirectoryControllerTest' --tests '*InstalledAppsStoreTest'
```

Expected: tests pass; generated Swift/Kotlin compile inputs include the additive fields; contract validation passes.

- [ ] **Step 6: Commit**

```bash
git add crates/riot-core/src/apps/directory.rs crates/riot-core/tests/apps_directory.rs crates/riot-ffi/src/apps_ffi.rs crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/tests/apps_contract.rs apps/ios/RiotTests/DirectoryStorefrontTests.swift apps/android/app/src/test/kotlin/org/riot/evidence/apps/DirectoryControllerTest.kt apps/android/app/src/test/kotlin/org/riot/evidence/apps/InstalledAppsStoreTest.kt
git commit -m "feat(apps): derive updates from signed lineage"
```

## Task 5: Make `riot-app` create, pack, and inspect v2 releases

**Files:**
- Modify: `crates/riot-app-cli/src/lib.rs`
- Modify: `crates/riot-app-cli/src/main.rs`
- Modify: `crates/riot-app-cli/tests/cli_pack.rs`
- Modify: `crates/riot-app-cli/tests/fixtures/hello-app/riot-app.json`
- Test fixtures: create temporary v1/v2 manifests inside `cli_pack.rs`; do not commit private keys.

- [ ] **Step 1: Write RED command tests**

Add tests for:

```text
riot-app init <empty-dir> --name "Mutual Aid"
  --capability data-read --capability data-write
riot-app pack <v2-dir> --key-dir <key-dir> --experimental-v2 --out app.riot
riot-app inspect app.riot
```

`--capability <name>` is repeatable, accepts only `data-read`, `data-write`, and `member-profile`, sorts/deduplicates them before writing, and defaults to the empty set (no bridge operation allowed). Assert `init` creates a new `riot-app.json` atomically, refuses an existing manifest, writes a full 64-character random `family_id`, uses `schema_version: 2`, and emits only the explicitly requested capability names. Test empty default, one flag, repeated flags, all flags, and an unknown flag.

Assert repeated packing with the same source/key produces the same release bytes except for the Willow carrier timestamp; changing code changes `app_id` but not `family_id`; setting `previous_app_id` signs the exact predecessor; tampering fails inspect.

- [ ] **Step 2: Preserve the legacy input contract explicitly**

An input with no `schema_version` continues to pack v1 exactly as today and prints:

```text
warning: legacy v1 package: publisher lineage and enforced capabilities are unavailable
```

`schema_version: 2` requires `family_id`, rejects unknown capability strings, remote icon/screenshot URLs, missing metadata resources, malformed predecessor IDs, and unknown JSON keys. Throughout Phase 1, `pack` requires `--experimental-v2` and always prints `experimental wire format: hosts must implement Riot microapps v2 before launch`. Task 6 does not remove or change the flag/warning; removing it is a separate post-Phase-1 product decision after the public spec is pinned.

- [ ] **Step 3: Run and confirm RED**

Run: `cargo test -p riot-app-cli --test cli_pack`

Expected: failures for missing `init`, v2 fields, and inspection output.

- [ ] **Step 4: Implement strict v2 JSON input**

Use this developer-facing shape:

```json
{
  "schema_version": 2,
  "family_id": "<64 lowercase hex characters>",
  "previous_app_id": null,
  "name": "Mutual Aid",
  "description": "Match needs and offers inside this space.",
  "version": "1.0.0",
  "entry_point": "index.html",
  "capabilities": ["data-read", "data-write", "member-profile"],
  "icon": "icon.png",
  "screenshot": null,
  "categories": ["community"],
  "theme_rgb": "2f7d68",
  "preferred_size": { "width": 390, "height": 720 }
}
```

Continue using the existing duplicate-key-rejecting bounded JSON visitor. Add exact decoders for 32-byte lowercase hex and six-digit RGB; never echo hostile raw fields unescaped.

- [ ] **Step 5: Sign and inspect through the production verifier**

`pack` builds the bundle first, embeds its digest in the unsigned v2 release, signs it through `EvidenceAuthor`, encodes it, and calls `verify_app_package` before writing anything. `inspect` prints generation, full app ID, full family ID, full predecessor ID when present, verified publisher status, capabilities, and resource paths.

- [ ] **Step 6: Run CLI tests and backward-compatibility fixtures**

Run:

```bash
cargo test -p riot-app-cli --test cli_pack
cargo test -p riot-core --test apps_starter
cargo run -p riot-core --example pack_starter
git diff --exit-code -- fixtures/apps/*.manifest.cbor fixtures/apps/*.bundle.cbor
```

Expected: CLI tests pass and the deterministic v1 starter artifacts do not change.

- [ ] **Step 7: Commit**

```bash
git add crates/riot-app-cli/src crates/riot-app-cli/tests
git commit -m "feat(cli): create and pack signed v2 apps"
```

## Task 6: Activate and enforce typed capabilities in both native hosts

**Files:**
- Read/verify: `crates/riot-ffi/tests/apps_contract.rs`
- Create: `apps/ios/Riot/Apps/AppRuntimePolicy.swift`
- Modify: `apps/ios/Riot/Apps/AppBridgeController.swift`
- Modify: `apps/ios/Riot/Apps/AppRuntimeView.swift`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Read/verify unchanged delegation: `apps/ios/Riot/Apps/RiotJS.swift`
- Modify: `apps/ios/RiotTests/{AppRuntimeHost,AppRepository,AppSyncReplication}Tests.swift`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/apps/AppRuntimePolicy.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/apps/RiotJsBridge.kt`
- Read/verify unchanged delegation: `apps/android/app/src/main/kotlin/org/riot/evidence/apps/RiotJsShim.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/apps/RiotAppsController.kt`
- Test: `apps/android/app/src/test/kotlin/org/riot/evidence/apps/{RiotJsBridgeTest,InstalledAppsStoreTest}.kt`
- Create test: `apps/android/app/src/test/kotlin/org/riot/evidence/apps/RiotAppsControllerTest.kt`
- Modify: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/apps/AppRuntimeEndToEndTest.kt`

**Atomic release barrier:** Tasks 1–5 may be committed for review, but no release may advertise, persist as installed, trust, or launch signed-v2 apps. Before this task, v2 is limited to strict decoding, directory arrival, inspection, and explicitly experimental CLI output. Task 6 becomes releasable only when both hosts enforce the same policy matrix and the full native gates pass in one release candidate.

- [ ] **Step 1: Verify the Task 3 projection prerequisite remains GREEN**

Run `cargo test -p riot-ffi --all-features --test apps_contract v2_`. It must already prove a v1 install reports `legacy_capability_policy == true`, a v2 install reports exact stable IDs/labels/generation, and unknown wire capabilities never cross FFI. Task 6 consumes this contract; it does not add a second late set of required UniFFI fields.

- [ ] **Step 2: Define the activation owner and safe defaults in RED tests**

The only production owners are `AppRuntimePolicy.swift` and `AppRuntimePolicy.kt`. Each exposes `signedV2CapabilitiesSupported`; test builds can inject true/false, while the checked-in production value starts `false`. Write iOS and Android tests proving that when false, v2 may be listed/inspected but cannot be installed into the native serving store, persisted, trusted, or launched; v1 behavior is unchanged. The install refusal must occur before persistence callbacks.

Run the focused iOS/Android test commands and confirm RED because the policy owners and guards do not exist.

- [ ] **Step 3: Project the existing normalized policy into both hosts**

Wire the Task 3/4 record fields into immutable native launch policies. IDs and review labels come only from `AppCapability`; never forward v2 manifest prose. Keep the production activation constants false through all RED/GREEN implementation steps.

- [ ] **Step 4: Write RED iOS tests for every denied operation**

Construct bridges with each capability subset. Assert denied `get/list`, `put`, and `whoami/profile` requests return the existing promise error envelope without calling the mocked data/profile port. Assert v1 legacy policy retains current behavior. For `watch`, preserve the existing JavaScript contract: registration is synchronous and returns `undefined`; its initial and notification refreshes delegate to `list`, so without `DataRead` the callback is never invoked and the mocked data port is never touched. Do not invent a native `watch` operation or change it into a Promise. Add a launch-barrier assertion: signed v2 cannot construct `AppRuntimeLaunch` while the injected policy is false, even if organizer-trusted.

In `AppRepositoryTests`, persist a signed v2 pack with capabilities beside a valid v1 pack, reopen the repository under policy true and verify exact generation/capabilities/trust, then reopen under policy false and verify v2 is skipped/quarantined while v1 still restores. A corrupt v2 pack must not prevent either valid pack from restoring. `AppSyncReplicationTests` must update direct `AppRuntimeLaunch` construction to supply explicit policy and cover a true-policy v2 refresh.

- [ ] **Step 5: Run the focused iOS test and confirm RED**

After building native core, run the RiotKit test command with `-only-testing:RiotTests/AppRuntimeHostTests -only-testing:RiotTests/AppRepositoryTests -only-testing:RiotTests/AppSyncReplicationTests`. Expected: policy, capability, restore, or direct-constructor assertions fail under the current unconditional dispatch.

- [ ] **Step 6: Enforce at the iOS bridge boundary and make the focused test GREEN**

`AppRuntimeLaunch` carries an immutable `Set<AppCapabilityID>`. `AppBridgeController` checks the required capability before dispatching each operation. Use one host-owned error string, `"This tool is not allowed to do that"`; do not reveal storage or identity internals.

Re-run the focused iOS command. Re-read the bridge dispatch once GREEN and collapse repeated checks into one `requiredCapability(for:)` mapping without changing behavior.

- [ ] **Step 7: Write RED Android JVM tests for the same matrix**

Use `RiotJsBridge` with fake `AppDataPort`/profile ports and assert denied calls return `{ "ok": false }` envelopes and never touch the fake port. Add a shim assertion matching iOS: `watch` delegates through `riotList`, swallows the denial as it does today, and never invokes its callback. In `RiotAppsControllerTest`, prove a trusted v2 app cannot pass `requireTrusted` while `supportsSignedV2Capabilities` is false, and can pass only when it is true. Prove `restore` handles packages independently: an unsupported or corrupt package is quarantined/skipped without crash-looping startup or preventing valid v1/v2 packages from restoring.

- [ ] **Step 8: Run the focused Android tests and confirm RED**

Run from `apps/android`: `JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew :app:testDebugUnitTest --tests '*RiotJsBridgeTest' --tests '*RiotAppsControllerTest'`.

Expected: constructor/policy compile failures or denial assertions fail under current unconditional dispatch.

- [ ] **Step 9: Enforce at the Android bridge boundary and make the focused tests GREEN**

Pass the immutable normalized policy from `RiotAppsController` through the actual `RiotJsBridge(...)` construction in `MainActivity.kt`. Check it before every data/profile port call. Add the explicit host-support install/persist/trust/launch checks and make Android restore best-effort per package, matching iOS's existing `try? installPack` behavior. Update `AppRuntimeEndToEndTest` for the new bridge constructor and compile it even when no emulator is available. Keep `blockNetworkLoads`, Safe Browsing disablement, service-worker denial, and navigation locks unchanged. Re-run the focused tests, then centralize the operation-to-capability mapping without altering the envelopes.

- [ ] **Step 10: Flip production activation in one reviewable change**

Only after all focused policy and bridge tests pass on both hosts, change the two production policy constants from false to true in the same commit. Add assertions against the production factories (not only injected test values) so a release cannot accidentally advertise v2 on one platform and disable it on the other. The supported rollback is the exact inverse one-line change on both platforms while retaining decoding and restore hardening.

- [ ] **Step 11: Regenerate bindings and run native suites**

Run:

```bash
cargo xtask generate-bindings
cargo test -p riot-ffi --all-features --test apps_contract
scripts/conference/build-native-core.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-derived
(cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew :app:testDebugUnitTest :app:assembleDebug)
(cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew :app:compileDebugAndroidTestKotlin)
```

Expected: all commands pass. If the named simulator is unavailable, choose an installed arm64 iPhone simulator and record the exact replacement in `COLLABORATION.md`.

- [ ] **Step 12: Commit**

```bash
git add crates/riot-ffi apps/ios/Riot apps/ios/RiotTests apps/android/app/src
git commit -m "feat(runtime): enforce microapp capabilities"
```

## Task 7: Render verified status and host-owned capability copy

**Files:**
- Modify: `apps/ios/Riot/Apps/AppReviewSheet.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryModel.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryView.swift`
- Modify: `apps/ios/RiotTests/DirectoryRepositoryTests.swift`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/apps/DirectoryController.kt`
- Test: `apps/android/app/src/test/kotlin/org/riot/evidence/apps/DirectoryControllerTest.kt`

- [ ] **Step 1: Write model tests before UI changes**

Assert v2 rows display `Verified publisher` plus host-owned capability labels, while v1 rows display `Legacy package` and the existing plain-language warning. Neither UI may imply that organizer approval makes code technically safe. On Android, put this copy in a pure `DirectoryPresentation` value produced by `DirectoryController`, assert it in `DirectoryControllerTest`, and make `MainActivity` render that value; the test must fail if the activity continues rendering raw legacy `permissions`.

- [ ] **Step 2: Run focused tests and confirm RED**

Run the iOS `DirectoryRepositoryTests` and Android `DirectoryControllerTest`; expect missing fields/copy failures.

- [ ] **Step 3: Update review presentation without adding executable previews**

The review screen remains entirely native. Update both SwiftUI and the Android `MainActivity.kt` directory/review builders to show:

```text
Verified publisher
This exact release was signed by its publisher.

This tool can
• Read this tool's information in this space
• Change this tool's information in this space
```

For v1:

```text
Legacy package
Publisher history and fine-grained access were not recorded for this release.
```

Do not show shortened author/app IDs and do not execute app JS to render its card.

- [ ] **Step 4: Run native tests**

Run the same iOS RiotKit and Android JVM commands from Task 6. Expected: all pass, including existing storefront and checklist regressions.

- [ ] **Step 5: Commit**

```bash
git add apps/ios/Riot/Apps/AppReviewSheet.swift apps/ios/Riot/Directory apps/ios/RiotTests/DirectoryRepositoryTests.swift apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt apps/android/app/src/main/kotlin/org/riot/evidence/apps/DirectoryController.kt apps/android/app/src/test/kotlin/org/riot/evidence/apps/DirectoryControllerTest.kt
git commit -m "feat(directory): show verified release capabilities"
```

## Task 8: Publish the contract and independent conformance vector

**Files:**
- Create: `docs/specs/riot-microapps-v2.md`
- Create: `fixtures/apps/v2-release-vector.json`
- Create: `fixtures/apps/v2-release-vector.cbor`
- Create: `fixtures/apps/v2-release-vector.bundle.cbor`
- Create: `crates/riot-conformance/tests/microapp_v2_vectors.rs`
- Modify: `crates/riot-conformance/Cargo.toml`
- Modify: `fixtures/manifest.json`
- Modify: `crates/xtask/src/main.rs`
- Modify: `SERVICE-INVENTORY.md`

- [ ] **Step 1: Write RED conformance and structural tests**

The independent test reads the JSON projection, signed release CBOR, and exact bundle CBOR; requires expected JSON values for all 20 release fields (including nulls, bounds-sensitive metadata, full author/family/predecessor IDs, capability IDs, digest, and signature); recomputes the bundle digest from those bundle bytes; verifies the pair through the production boundary; independently compares every decoded field and app ID; then mutates every signed field and each package half one at a time and expects rejection.

Extend xtask tests so missing or stale `microapp_v2_release_vector_sha256` or `microapp_v2_bundle_vector_sha256` fails `validate-contracts`.

- [ ] **Step 2: Run and confirm RED**

Run:

```bash
cargo test -p riot-conformance --test microapp_v2_vectors
cargo test -p xtask
```

Expected: missing vector/hash failures.

- [ ] **Step 3: Generate one deterministic vector without committing a private key**

Generate in a temporary directory using the conformance-only seeded author helper, write only public JSON, signed release CBOR, and the deterministic canonical bundle CBOR, then delete the temporary secret material. The JSON projection includes full IDs, the bundle SHA-256, and the expected 64-byte signature as lowercase hex because they are public verification fixtures.

- [ ] **Step 4: Write the public v2 specification**

Document the exact field table, signature/app-ID formulas, capability registry, v1 compatibility, carrier versus publisher roles, organizer governance, runtime CSP/bridge rules, and test vectors. Attribute WebTiles/DASL directly for MASL resource manifests, `.tile`/CAR packaging, inert metadata/cards, `prev` lineage, network isolation, loader adapters, host mediation, and future intents. Label signed Willow lineage, random family IDs, exact-version organizer approval, and numeric native-bridge capabilities as Riot-specific adaptations rather than WebTiles interoperability claims.

- [ ] **Step 5: Pin vector hashes and update the service inventory**

Record separate SHA-256 values for the exact release and bundle CBOR vectors in `fixtures/manifest.json`; make xtask recompute both. Add `apps::release`, `apps::package`, and `apps::capability` ownership rows to `SERVICE-INVENTORY.md`.

- [ ] **Step 6: Run conformance gates**

Run:

```bash
cargo test -p riot-conformance --test microapp_v2_vectors
cargo test -p xtask
cargo xtask validate-contracts
```

Expected: all pass and `validate-contracts: PASS`.

- [ ] **Step 7: Commit**

```bash
git add docs/specs/riot-microapps-v2.md fixtures/apps/v2-release-vector.json fixtures/apps/v2-release-vector.cbor fixtures/apps/v2-release-vector.bundle.cbor fixtures/manifest.json crates/riot-conformance crates/xtask/src/main.rs SERVICE-INVENTORY.md
git commit -m "docs(apps): publish microapp v2 contract"
```

## Task 9: Run full quality, compatibility, and coverage gates

**Files:**
- Modify only if evidence changed: `COLLABORATION.md`
- Read: `.coverage-thresholds.json`

- [ ] **Step 1: Verify formatting and the complete Rust graph**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
cargo xtask validate-contracts
```

Expected: every command exits 0.

- [ ] **Step 2: Run the blocking source-of-truth command exactly**

Run exactly the command currently stored in `.coverage-thresholds.json`:

```bash
jq -r '.enforcement.command' .coverage-thresholds.json | sh
```

Expected: exit 0. With the current Tarpaulin-only configuration this establishes its supported aggregate/line threshold, not branch/function/statement coverage. Do not claim otherwise, and do not claim native coverage from it; native suites remain separate gates.

- [ ] **Step 3: Prove all four configured Rust dimensions exactly**

```bash
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-features --branch --json --summary-only --output-path target/llvm-cov-summary.json
jq -e '.data | length == 1 and (.data[0].totals | (.lines.covered == .lines.count) and (.functions.covered == .functions.count) and (.regions.covered == .regions.count) and (.branches.covered == .branches.count))' target/llvm-cov-summary.json >/dev/null
```

Expected: exit 0 with covered/count equality for lines, functions, LLVM regions (the explicit Rust statements mapping), and branches. Save the JSON artifact path in the evidence record.

- [ ] **Step 4: Verify all native targets**

```bash
scripts/conference/build-native-core.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-derived
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'
cd apps/android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 ./gradlew :app:testDebugUnitTest :app:assembleDebug
```

Expected: all exit 0. Run `connectedDebugAndroidTest` as an additional gate when an API 36 emulator is available; do not report it as passing if no emulator ran it.

- [ ] **Step 5: Prove v1 artifacts and IDs did not drift**

```bash
cargo run -p riot-core --example pack_starter
git diff --exit-code -- fixtures/apps/*.manifest.cbor fixtures/apps/*.bundle.cbor
```

Expected: no diff.

- [ ] **Step 6: Record evidence and commit only the report**

Update `COLLABORATION.md` with exact commands, outcomes, environment substitutions, and commit IDs. Do not overwrite unrelated concurrent notes.

```bash
git add COLLABORATION.md
git commit -m "docs: record microapp v2 verification"
```

## Phase 2 charter: Builder and directory experience

Create a separate implementation plan after Phase 1 lands. It must deliver both items below as independently committable work units.

### 2A. Rich inert cards

- Use only v2 `icon_path`, `screenshot_path`, categories, theme color, and preferred size.
- Add an FFI `app_card_asset(app_id, kind)` returning verified local bytes plus content type; never accept a URL.
- Render metadata-only cards in native directory UI without mounting a WebView.
- Reject missing paths, SVG with active external references, oversized decoded images, and unsupported MIME types.
- Verify empty/no-image fallback, corrupt asset fallback, narrow phone layout, and macOS layout.

**Exit criterion:** hundreds of directory cards can be listed without executing app JavaScript, and activating a card still performs an exact-version trust check before launch.

### 2B. `riot-app dev`

- Add `riot-app dev <dir> --state <optional-json>` using a loopback-only preview server.
- Serve only canonicalized files beneath the app root, with the production CSP and no outbound proxying.
- Inject a clearly marked development bridge implementing the exact v2 capability policy and `get/put/list/watch/whoami/profile` shapes.
- Provide deterministic controls for empty, seeded, write-failure, and remote-change states at 390×844 and 1280×800.
- Reuse `verify_app_package` before preview; a source change invalidates the displayed app ID.
- Add path traversal, symlink, oversized request, network-attempt, capability-denial, and clean-shutdown tests.

**Exit criterion:** a new developer can run the tutorial app, exercise bridge states, and pack the same directory without editing source or installing Riot.

## Phase 3 charter: Source-neutral loading and WebTiles import

Create a separate design and implementation plan after Phase 1. Phase 2's preview harness may be reused if it has landed, but WebTiles import must not wait on directory-card or preview UX work.

0. Validate demand first: identify at least two real `.tile` apps or community/developer workflows that benefit from import, and compare that value against improving Riot-native onboarding/templates. If evidence does not clear that gate, stop after the source-neutral loader refactor or defer Phase 3 entirely.
1. Introduce a small `AppPackageSource` interface returning exact release and bundle bytes; implementations: built-in, installed, Willow index, memory/dev, and imported file.
2. Refactor the existing three-source resolver in `riot-ffi/mobile_state.rs` behind that interface without changing precedence or IDs.
3. Add a read-only `.tile`/CAR adapter that maps MASL paths and content types into a temporary Riot bundle, then emits a normal Riot v2 release signed by the local importer/converter identity.
4. Keep two claims separate: Phase 1 `publisher_verified` means the resulting Riot release signer is verified; a new Phase 3 `UpstreamProvenance::UnverifiedWebTiles` descriptor means Riot has not authenticated the upstream WebTiles publisher. UI must never relabel the local converter as the upstream author.
5. Require explicit organizer approval and assign a new Riot `app_id`; do not pretend a converted package preserves upstream trust. This extends source provenance without adding a third wire generation or weakening the Phase 1 verifier.
6. Never invoke WebTiles' loading server, service worker, arbitrary HTTP loader, forms, popups, or network capabilities in Riot.
7. Publish export only after import semantics and license/provenance presentation are proven.

**Exit criterion:** a self-contained `.tile` file can be inspected, converted, reviewed, approved, and run completely offline under Riot's CSP and bridge, while malformed CAR/MASL inputs fail closed.

## Phase 4 charter: Resource graph and host-mediated intents

These remain separate, optional design-review tracks.

### 4A. Content-addressed resource graph

- Measure bundle sizes, duplicate bytes, cold launch, nearby transfer, and partial-arrival behavior first.
- Prototype an atomic required shell plus optional CID-addressed assets; no resource may be fetched from arbitrary network locations.
- Define completeness receipts so an app cannot launch with a missing required script.
- Compare the prototype against the current 1 MiB/32-resource bundle using real miniapps and interrupted sync.
- Adopt only if it materially improves deduplication or large offline assets without weakening atomic carriage.

### 4B. Host-mediated intents

- Begin with the deferred Activity Feed contract, not general cross-app reads.
- Define typed, versioned intent names with bounded payload schemas, explicit user gestures, host confirmation, rate limits, and auditable source app IDs.
- Apps publish minimal privacy-reviewed summaries to a host-owned namespace; they never read another app's storage.
- Unknown intents fail closed and cannot open network URLs or secondary WebViews.

**Exit criterion:** each track receives its own five-role design review. No prototype becomes a production protocol merely because it works in a demo.

## Rollback strategy

- V2 wire support is additive, but Tasks 1–5 are not independent production release points. The first v2-capable release is the atomic Task 6 barrier after both native enforcement matrices pass.
- A supported rollback build retains the v2 decoder, normalized package records, and per-package best-effort restore, sets `supportsSignedV2Capabilities = false`, and removes native v2 install/trust/launch affordances. The CLI remains explicitly experimental. That leaves v1 operational and already persisted v2 bytes quarantined/inert instead of misinterpreted.
- Do not claim that installing a binary predating all v2 decoding is a safe rollback after v2 packages have been persisted. Such a binary downgrade requires first removing/quarantining v2 installed records with the supported rollback build; document this operational limitation in release notes.
- Do not rewrite persisted v1 manifests, app IDs, trust markers, or app-data prefixes.
- Codec, import, and lineage commits can be reverted before activation. After activation, capability enforcement and the v2 launch barrier are security-coupled and must never be reverted independently; UI copy may be reverted without changing enforcement.
- The v2 decoder must reject unknown schema versions rather than route them through v1.
- If native capability enforcement cannot land on both iOS and Android, keep v2 inspection/import code unreleased and `schema_version: 2` packing behind `--experimental-v2`; do not persist, trust, or launch it in a release build.

## Definition of done

- Existing v1 starter artifacts and IDs are byte-identical.
- A carried v2 release retains verifiable original-publisher proof after any number of carriers.
- Only an exact signed predecessor edge creates an update relationship; names and timestamps do not.
- Trust never carries automatically to a new app ID.
- V2 capabilities are numeric, canonical, host-labelled, and enforced before native port access on iOS and Android.
- The public spec and independent vector agree with production code.
- Workspace tests, strict Clippy, contract validation, native tests, and the repository's 100% coverage gate pass.
- Later phases remain bounded by their charters and cannot add runtime networking or cross-app storage access.
