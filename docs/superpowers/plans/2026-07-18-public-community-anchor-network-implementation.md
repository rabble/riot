# Public Community Anchor Network Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `metaswarm:orchestrated-execution` to implement this plan work-unit by work-unit. Steps use checkbox (`- [ ]`) syntax for tracking. Every work unit follows IMPLEMENT → VALIDATE → fresh ADVERSARIAL REVIEW → COMMIT; every implementation starts with the named failing tests.

**Goal:** Deliver the approved plural public-anchor network through production-ready hosting, discovery/gossip, safe web mirrors and handoff, and native plural-host UX. The privacy-preserving pilot is explicitly outside this active plan and requires its own future spec→plan review when a live pilot is scheduled.

**Architecture:** `riot-anchor-protocol` is the dependency-light canonical wire layer. `riot-core` owns all client profile persistence through a serialized storage command port; `riot-client-net` owns the process-singleton internet runtime; `riot-anchor` owns server persistence, ingress, hosting, directory, gossip, and HTTP; a separate renderer binary consumes immutable typed snapshots. Native shells consume typed UniFFI state and never implement authority or retry semantics.

**Tech Stack:** Rust 2021, minicbor/CDDL, Willow/Meadowcap, Ed25519, BLAKE3/HMAC-SHA256, rusqlite/WAL, iroh/QUIC, Tokio, rustls/hyper HTTP/1.1, UniFFI, Swift 6/SwiftUI, Kotlin/JVM Android, Python renderer fixtures, OCI/Compose.

**Approved design:** `docs/superpowers/specs/2026-07-18-public-community-anchor-network-design.md` at design-gate commit `30563cb` (reviewed candidate `2779c8c`).

---

## Delivery Rules

- `.coverage-thresholds.json` is the only coverage source of truth: Tarpaulin lines 97; LLVM lines/functions/regions/branches 95/95/92/83; JS tooling 100/100/100/100.
- The existing user changes to `package.json`, `package-lock.json`, and the Xcode workspace user-state file are not part of this plan and must remain untouched.
- `riot/sync/1`, the fixed gateway, legacy deep links, nearby carriers, and standalone communal-space flows remain compatible.
- Each RED step must be run and observed failing for the reason named before implementation.
- Each GREEN step runs the focused test plus `cargo test --workspace --all-features`.
- Each work unit records its inventory impact in its DoD/test evidence; WU-028 applies the consolidated `SERVICE-INVENTORY.md` update after the functional interfaces are stable.
- No work unit may lower a compiled ceiling, coverage floor, transport floor, or authority check to make a test pass.
- New dependency versions are pinned in the workspace manifest. Versions already resolved in `Cargo.lock` are preferred: `blake3 1.8.5`, `base64 0.22.1`, `hyper 1.10.1`, `http-body-util 0.1.4`, `rustls 0.23.42`, `tokio-rustls 0.26.4`, `tower 0.5.3`, and `tower-http 0.6.11`.

## Dependency Graph

```text
WU-001 ─→ WU-002 ─→ WU-003A ─→ WU-003B ─→ WU-004 ─→ WU-005 ─→ WU-006A ─→ WU-006B
                                                   │         │                    │
                                                   └─────────┴──────────────→ WU-007

WU-001 ─→ WU-008 ─→ WU-009 ─→ WU-010A ─→ WU-010B ─┐
WU-003B ─→ WU-011A ─→ WU-011B ─→ WU-011C ─→ WU-012A ─→ WU-012B ─→ WU-012C
WU-005 ────────────────────────────────→ WU-011C
WU-009 ─────────────────────────────────────────────→ WU-012A
                                                      │
                                                      ├→ WU-022A/WU-023A
WU-004 ─→ WU-013A ─→ WU-013B ─→ WU-014 ─→ WU-015 ─→ WU-015B ─→ WU-016
WU-005 ───────────────────────────────────────────→ WU-015
                                                      │
                                                      ├───────────────┐
WU-011C/WU-013B/WU-015B/WU-016 ────────────────────→ WU-017B
WU-011C ────────────────────────────────────────────→ WU-015B
WU-007 ────────────────────────────────→ WU-018A ─→ WU-018B
WU-007/WU-013B/WU-015/WU-018A ─────────────────────→ WU-018B
WU-007/WU-014/WU-015/WU-015B/WU-016 ───────────────┐
WU-017B/WU-018B ────────────────────────────────────┴→ WU-019

WU-017B ─→ WU-020P
WU-015/WU-016/WU-017B/WU-020P ─→ WU-020A ─→ WU-020B ─→ WU-021A ─→ WU-021B
WU-020P ─────────────────────────────────→ WU-020B
WU-019/WU-017B ─────────────────────────────────────────────→ WU-021B
WU-012C/WU-021B ─→ WU-022A ─→ WU-022B ─→ WU-022C ─→ WU-022C2 ─→ WU-022D
WU-012C/WU-021B ─→ WU-023A ─→ WU-023B ─→ WU-023C
WU-019/WU-020B/WU-021B ─→ WU-026A ─→ WU-026B
WU-016/WU-018B/WU-019/WU-020B/WU-021B/WU-022D/WU-023C/WU-026B ─→ WU-027
WU-027 ─→ WU-028
```

Parallel execution is allowed only where this graph and non-overlapping file scopes permit it. Commits remain sequential.

## Requirement Traceability

| Approved design requirement | Implemented and verified by |
| --- | --- |
| Slice 1 — authority records, positional grammar, tickets, descriptors, receipts, identity digests, cross-language agreement | WU-001–WU-004, WU-006A–WU-006B |
| Slice 2 — routed/paginated sync, immutable snapshots, exact refusals, one-way/staged directions | WU-005, WU-007, WU-015 |
| Slice 3 — scalable client storage, profile worker/lifecycle, anchor repository, idempotent control, CAS/receipt recovery, atomic ordinary listing, emergency removal | WU-008–WU-010B, WU-013A–WU-016 |
| Slice 4 — signed plural feeds, authenticated cursors, descriptor rotation, configured-peer attestation, bounded gossip | WU-011C, WU-017B–WU-018B |
| Slice 5 — immutable text projection, hostile-output validation, ordinary HTTPS, byte-identical v2 handoff and legacy compatibility | WU-020P–WU-021B |
| Slice 6 — process runtime, cancellable UniFFI states, durable background reconciliation, verified runtime bootstrap packaging, defaults/configuration, Explore/Follow/Publish/relist/source-change on both native platforms, and shared-Swift macOS compatibility | WU-009, WU-011A–WU-012C, WU-022A–WU-023C |
| Slice 7 — all compiled ceilings, ingress/load shedding, deployment/recovery, deterministic system/load proof | WU-013A–WU-013B, WU-016, WU-019, WU-026A–WU-028 |
| Definition of Done — plural independent accountless hosting and failure survival | WU-012A–WU-012B, WU-015, WU-018A–WU-018B, WU-022A–WU-023C, WU-027 |
| Definition of Done — no hosting/listing coupling; atomic listing/replay, owner recovery, and exact Meadowcap authority | WU-003A–WU-003B, WU-015–WU-017B |
| Definition of Done — root-signed cold bootstrap, durable floors, exact O/C/W local/anchor atomicity | WU-003B, WU-008, WU-011A–WU-012C, WU-013A–WU-015 |
| Definition of Done — recoverable control, removal, checkpoint, rotation, replica-source and startup lifecycles | WU-004, WU-014–WU-018B |
| Definition of Done — bounded public iroh/HTTPS, renderer, and operational resource use | WU-007, WU-016, WU-019–WU-021B, WU-026A–WU-027 |
| Definition of Done — typed post-consent storage/recovery and durable per-destination outcomes | WU-008–WU-012B, WU-022A–WU-023B |
| Definition of Done — every quality/coverage gate | WU-028 |

### Explicitly deferred scope

The pilot recruitment, enrollment, aggregate export, withdrawal, retention, and collector-side
validity/threshold-reporting requirements are not part of this active implementation plan. Historical
WU numbers 024–025 are reserved and intentionally absent below. They may be planned only after M2–M4
are real and a live pilot has approved human coordinators, a contact-handling policy, fixed-role
credential batches, exact native measurement-event boundaries, collector-side aggregate validity
rules, and signed production fixtures. That future work requires a separate design update,
implementation plan, design review gate, and plan review gate; no worker may infer pilot behavior
from the archived design text or implement it under WU-026–WU-028.

## Work Unit Decomposition

### WU-001: Workspace and protocol crate boundary

**Spec:** System Shape; TDD Slice 1; feature-closure requirement.

**Files:**

- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/riot-anchor-protocol/Cargo.toml`
- Create: `crates/riot-anchor-protocol/src/lib.rs`
- Create: `crates/riot-anchor-protocol/tests/dependency_boundary.rs`

**DoD:**

- `riot-anchor-protocol` depends on `riot-core` with `default-features = false` and has no SQLite, HTTP, iroh, Tokio, renderer, FFI, or platform dependency.
- Workspace pins canonical crypto/server dependencies and includes future crates without enabling them in native graphs.
- A structural test rejects forbidden protocol dependencies and accidental `riot-core/sqlite`.

- [ ] **RED:** Add `dependency_boundary.rs` asserting parsed `cargo metadata` has no forbidden package reachable from `riot-anchor-protocol`; run `cargo test -p riot-anchor-protocol --test dependency_boundary` and observe the missing package/manifest failure.
- [ ] **GREEN:** Add the crate with crate-level safety/documentation attributes only. Each later protocol work unit adds its own `pub mod` declaration in `lib.rs` in the same commit that creates the module, so the crate remains buildable at every commit.
- [ ] Run `cargo check -p riot-anchor-protocol --no-default-features` and
  `cargo test -p riot-anchor-protocol --test dependency_boundary`.
- [ ] Commit as `build(anchor): establish protocol dependency boundary`.

### WU-002: Positional CBOR codec and protocol identity digests

**Spec:** Canonical Anchor Records; Protocol identity digests; Encoded control-record profile.

**Files:**

- Create: `crates/riot-anchor-protocol/src/codec.rs`
- Create: `crates/riot-anchor-protocol/src/digest.rs`
- Modify: `crates/riot-anchor-protocol/src/lib.rs`
- Create: `crates/riot-anchor-protocol/tests/canonical_codec.rs`
- Create: `crates/riot-anchor-protocol/schema/riot-anchor-v1.cddl`

**DoD:**

- Definite positional arrays, minimal integers, exact textual discriminants, embedded typed values versus `*_bytes`, sorted-set validation, trailing-byte rejection, and byte-identical re-encoding are shared primitives.
- `digest_v1`, every table-defined label, control/work body distinction, length prefix, namespace HMAC input, listing-receipt preimage, peer transcript, and limit-profile digest match the design exactly.
- Bounded decode reports a closed `CodecError` without allocating beyond the caller-provided record limit.

**Interface:**

```rust
pub fn digest_v1(label: &'static [u8], canonical: &[u8]) -> [u8; 32];
pub fn decode_canonical<T: CanonicalRecord>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, CodecError>;
pub trait CanonicalRecord: Sized {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError>;
    fn decode_fields(decoder: &mut minicbor::Decoder<'_>) -> Result<Self, CodecError>;
}
```

- [ ] **RED:** Add hostile fixtures for maps, indefinite containers, numeric enum tags, swapped fields, duplicate set members, non-minimal integers, nested-byte substitution, trailing bytes, and every identity digest; run `cargo test -p riot-anchor-protocol --test canonical_codec` and observe missing codec/digest symbols.
- [ ] **GREEN:** Implement the bounded positional codec helpers, domain-separated digest helpers, and CDDL transcription.
- [ ] Run `cargo test -p riot-anchor-protocol --test canonical_codec`.
- [ ] Commit as `feat(anchor): add canonical protocol codec and digests`.

### WU-003A: Core listing authority boundary

**Spec:** Listing Authority reserved coordinate and delegated writer boundary.

**Files:**

- Modify: `crates/riot-core/src/willow/site_paths.rs`
- Modify: `crates/riot-core/src/willow/masthead.rs`
- Create: `crates/riot-core/tests/listing_authority_boundary.rs`

**DoD:**

- Core defines the exact `/directory/listing` record path and `/directory` delegation prefix separately from `/articles`.
- Owner zero-delegation capability and time-boxed listing delegation constructors cannot authorize `/manifest`, `/mod`, `/articles`, or arbitrary directory record families.
- Existing editorial delegation and v1 admission behavior remains byte-for-byte compatible.

- [ ] **RED:** Add path/delegation tests proving editorial misuse, arbitrary-directory misuse, receiver mismatch, time escape, and owner root recovery; observe missing listing helpers.
- [ ] **GREEN:** Add focused path predicates and listing delegation construction without widening the generic bundle admission predicate.
- [ ] Run `cargo test -p riot-core --test listing_authority_boundary` and existing masthead/site tests.
- [ ] Commit as `feat(core): define listing authority boundary`.

### WU-003B: Tickets, listings, and manifest floors

**Spec:** Community Authority Bootstrap; Listing Authority; `CommunityListingV1`; Product Decisions 7 and 10.

**Files:**

- Create: `crates/riot-anchor-protocol/src/authority.rs`
- Create: `crates/riot-anchor-protocol/src/records.rs`
- Modify: `crates/riot-anchor-protocol/src/lib.rs`
- Create: `crates/riot-anchor-protocol/tests/authority_records.rs`
- Modify: `crates/riot-anchor-protocol/schema/riot-anchor-v1.cddl`

**DoD:**

- Exact owner-rooted `O/C/W` profile and `O:/directory/listing` coordinate are enforced.
- Root zero-delegation and exact root/manifest/ticket/namespace matching are independent checks.
- Dedicated listing delegates require a root-signed epoch grant, exact `/directory` terminal area/time, and cannot pin root recovery at `u32::MAX`.
- Same-version manifest/listing equivocation quarantines; a higher valid root record recovers.
- `PublicSiteTicketV2` rejects duplicate/oversize fields, v1/v2 downgrade, `require:arti`, mismatched transport, expiry, and rollback; its root signature is checked before any dial.

**Interface:**

```rust
pub fn admit_public_site_ticket(
    envelope: &RootSignedTicketCoreEnvelopeV2,
    manifest: Option<&ValidatedManifest>,
    floor: &TransportFloor,
    now: u64,
) -> Result<AdmittedTicket, AuthorityError>;
pub fn resolve_listing(
    durable: &ListingFloor,
    candidate: &AdmittedListingEnvelopeV1,
    now: u64,
) -> Result<ListingTransition, AuthorityError>;
```

- [ ] **RED:** Add the Slice 1 authority failures, including editorial capability misuse, max-revision delegate/root recovery, coordinate disagreement, expiry equality, and unsupported Arti; run `cargo test -p riot-anchor-protocol --test authority_records` and observe failures.
- [ ] **GREEN:** Implement canonical records plus pure admission/resolution using WU-003A's core authority predicates without changing v1 tickets.
- [ ] Run `cargo test -p riot-anchor-protocol --test authority_records` and `cargo test -p riot-core site`.
- [ ] Commit as `feat(anchor): enforce ticket and listing authority`.

### WU-004: Anchor identity, descriptors, receipts, limits, and control envelopes

**Spec:** Canonical Anchor Records; `AnchorDescriptorV1`; Hosting/Listing receipts; Anchor Keys; `riot/anchor/1`; Refusals; Admission work stamp.

**Files:**

- Create: `crates/riot-anchor-protocol/src/control.rs`
- Modify: `crates/riot-anchor-protocol/src/records.rs`
- Modify: `crates/riot-anchor-protocol/src/lib.rs`
- Create: `crates/riot-anchor-protocol/tests/control_vectors.rs`
- Modify: `crates/riot-anchor-protocol/schema/riot-anchor-v1.cddl`

**DoD:**

- Stable anchor ID, complete descriptor floor, genesis and every rotation/update envelope, `AnchorBootstrapV1`, 16-item/32-hop/256-KiB continuity caps, receipts, all 82 limit IDs, work challenge/stamp, operation/idempotency identities, namespace tokens, and exact request/success/refusal/GetOperation envelopes are canonical.
- Every refusal tuple, retry delay, and retry scope is the closed design matrix; unknown cross-pairings fail decode.
- `Describe`, `GetWorkChallenge`, `PrepareHost`, `CommitHost`, `SubmitListing`, `PrepareReplica`, feed/snapshot pulls, and `GetOperation` have exact operation bodies and response payloads.

**Interface:**

```rust
pub enum ControlOperation {
    Describe(DescribeV1),
    GetWorkChallenge(GetWorkChallengeV1),
    PrepareHost(PrepareHostV1),
    CommitHost(CommitHostV1),
    SubmitListing(SubmitListingV1),
    PrepareReplica(PrepareReplicaV1),
    PullDirectoryFeed(PullDirectoryFeedV1),
    PullDirectorySnapshot(PullDirectorySnapshotV1),
    GetOperation(GetOperationV1),
}
pub fn verify_descriptor_chain(
    floor: DescriptorFloor,
    pages: impl Iterator<Item = DescriptorEnvelopeV1>,
    now: u64,
) -> Result<DescriptorFloor, DescriptorError>;
```

- [ ] **RED:** Add golden/hostile tests in `control_vectors.rs` for all operation bodies, response nesting, 82 limits, every refusal row, work preimages, receipt signatures, bootstrap bounds, and descriptor genesis/rotation/chain failures.
- [ ] **GREEN:** Implement the control record family and exact validators from the normative tables.
- [ ] Run `cargo test -p riot-anchor-protocol --test control_vectors --test authority_records --test canonical_codec`.
- [ ] Commit as `feat(anchor): define descriptors receipts and control protocol`.

### WU-005: Routed paginated `riot/sync/2`

**Spec:** `riot/sync/2`; Composite transaction; TDD Slice 2.

**Files:**

- Create: `crates/riot-anchor-protocol/src/sync2.rs`
- Modify: `crates/riot-anchor-protocol/src/lib.rs`
- Create: `crates/riot-anchor-protocol/tests/sync2_fsm.rs`
- Create: `crates/riot-anchor-protocol/tests/sync2_duplex.rs`
- Modify: `crates/riot-anchor-protocol/schema/riot-anchor-v1.cddl`

**DoD:**

- Responder routes after bounded `OpenNamespace`, with exact ReadCommitted, HostReconcileStaged, and ReplicaIntoStaged modes.
- Immutable snapshot digest, 256-ID pages, up to four 64-ID needs, multi-chunk 8-MiB bundles, ordered chunk indexes, direction-private staging, and exact terminal digest verification are implemented.
- All refusal code/detail/retry tuples and frame/phase order are closed.
- One-way read cannot mutate anchor state; staged host/replica directions remain private until their parent transaction promotes.

**Interface:**

```rust
pub trait Sync2Repository {
    type Snapshot: Sync2Snapshot;
    type DirectionStage: Sync2DirectionStage;
    fn open_namespace(&self, request: &OpenNamespace) -> Result<OpenedNamespace<Self>, Sync2Refusal>;
}
pub struct Sync2Session<R: Sync2Repository> { /* transport independent */ }
pub enum Sync2Action { Send(Sync2Frame), Admit(EntriesChunk), PromoteDirection, Complete }
```

- [ ] **RED:** Add exact codec/hostile/FSM traces in `sync2_fsm.rs` and 257+/maximum-item duplex tests for O/C/W, cursor overlap, request/chunk mismatch, early EOF, admission rollback, stale source, and one-way mutation.
- [ ] **GREEN:** Implement the canonical frames, snapshot/page digests, transport-independent FSM, staging callbacks, and duplex harness.
- [ ] Run `cargo test -p riot-anchor-protocol --test sync2_fsm --test sync2_duplex` and
  `cargo test -p riot-transport --test iroh_sync`.
- [ ] Commit as `feat(anchor): add routed paginated sync v2`.

### WU-006A: Rust and TypeScript conformance vectors

**Spec:** TDD Slices 1 and 6; round-18 implementation notes.

**Files:**

- Create: `fixtures/anchor/protocol-v1-vectors.json`
- Create: `fixtures/anchor/bootstrap-development-v1.cbor`
- Create: `crates/riot-anchor-protocol/tests/golden_vectors.rs`
- Create: `scripts/web/anchor-protocol-vectors.ts`
- Create: `scripts/web/test/anchor-protocol-vectors.test.mjs`

**DoD:**

- Rust emits and consumes one deterministic checked-in vector fixture.
- A real TypeScript implementation independently verifies canonical bytes, digest/preimage distinctions, signatures, HMAC inputs, response nesting, peer roles, and all sentinels; the existing Node test runner imports it through Node 26's built-in type stripping.
- One-bit and alternate-grammar mutations fail without calling Rust for expected values.
- A deterministic three-descriptor development bootstrap exercises package-resource parsing;
  release validation refuses to call it a production default set.

- [ ] **RED:** Add Rust/Node consumers and observe failure because the fixture is absent.
- [ ] **GREEN:** Generate deterministic protocol/bootstrap fixtures from protocol constructors and review them into source control; the existing `scripts/web/test/*.test.mjs` test glob discovers the TypeScript-backed Node test without changing `package.json` or `package-lock.json`.
- [ ] Run `cargo test -p riot-anchor-protocol --test golden_vectors` and `npm run test:web:unit`.
- [ ] Commit as `test(anchor): pin Rust and web protocol vectors`.

### WU-006B: Native conformance vectors and feature closure

**Spec:** TDD Slices 1 and 6; round-18 native cross-compilation note.

**Files:**

- Create: `apps/ios/RiotTests/AnchorProtocolVectorTests.swift`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/AnchorProtocolVectorTest.kt`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/android/app/build.gradle.kts`

**DoD:**

- Swift and Kotlin independently verify the same canonical bytes, digest/preimage distinctions, signatures, HMAC inputs, response nesting, peer roles, and sentinels as Rust/TypeScript.
- One-bit and alternate-grammar mutations fail on every platform.
- No platform redefines protocol labels or truncates identifiers.
- Both native targets embed and validate the development bootstrap resource; a release build accepts
  only a separately supplied, package-signed production resource with at least three live
  descriptors across two operators/failure domains.

- [ ] **RED:** Add native consumers/resource wiring and observe missing vector assertions or target resources.
- [ ] **GREEN:** Make each platform consume both checked-in fixtures without asking Rust for expected values; all expected native cases are already fixed by WU-006A.
- [ ] Run
  `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/AnchorProtocolVectorTests`
  and `(cd apps/android && ./gradlew :app:testDebugUnitTest --tests org.riot.evidence.AnchorProtocolVectorTest)`;
  this unit does not claim iroh/Tokio native cross-compilation, which becomes executable only in WU-012B.
- [ ] Commit as `test(anchor): pin native protocol vectors`.

### WU-007: Multi-ALPN iroh router and bounded stream lifecycle

**Spec:** Transport Roles; public iroh admission; configured-peer channel binding.

**Files:**

- Modify: `crates/riot-transport/Cargo.toml`
- Modify: `crates/riot-transport/src/lib.rs`
- Modify: `crates/riot-transport/src/iroh.rs`
- Create: `crates/riot-transport/src/router.rs`
- Create: `crates/riot-transport/tests/alpn_router.rs`

**DoD:**

- `riot-transport` uses `riot-core` with `default-features = false`.
- One endpoint routes `riot/sync/1`, `riot/sync/2`, and `riot/anchor/1`; unknown ALPNs and second/unidirectional streams close without session allocation.
- Handshake, first frame, progress, frame read/write, idle, and absolute lifetime permits are enforced and cancellation releases all resources.
- Live TLS exporter bytes are exposed only to the authenticated peer handshake.
- Existing `sync_accept` remains a sync/1 compatibility wrapper.

- [ ] **RED:** Add router tests covering every ALPN, unknown ALPN, stream-count violations, trickle/stall deadlines, permit release, and deterministic exporter context.
- [ ] **GREEN:** Refactor endpoint binding and accept into `AlpnRouter`, retaining the sync/1 wrapper.
- [ ] Run `cargo test -p riot-transport --all-features` and
  `cargo check -p riot-transport --all-targets --all-features`; WU-012B performs iOS/Android
  iroh/Tokio cross-compilation once `riot-ffi` actually reaches `riot-client-net`.
- [ ] Commit as `feat(transport): route bounded anchor protocols`.

### WU-008: Scalable client site repository and migrations

**Spec:** Native Client Site Repository; TDD Slice 3.

**Files:**

- Modify: `crates/riot-core/src/store/schema.rs`
- Modify: `crates/riot-core/src/store/mod.rs`
- Create: `crates/riot-core/src/store/site_replica.rs`
- Create: `crates/riot-core/tests/client_site_repository.rs`
- Modify: `crates/riot-core/tests/sqlite_backup_restore.rs`

**DoD:**

- Versioned tables store verified manifests, O/C/W items/payload refs, follow/host stages, schedules, anchor floors/configuration, receipts, retry intents, and local operation outcomes in the existing `RiotDatabase`.
- `SiteReplicaRepository` is dependency-neutral; `ClientSiteRepository` implements 1-GiB total, 64-MiB/site, 4096 entries/namespace, exact logical charging, staging, CAS/promotion, eviction, floors, and backup/restore.
- One atomic follow commit installs all namespaces and ongoing reconciliation; cancellation/failure exposes none.

**Interface:**

```rust
pub trait SiteReplicaRepository: Send + Sync {
    type Snapshot;
    fn begin_follow(&self, plan: FollowPlan) -> Result<LocalOperationId, SiteStoreError>;
    fn append_chunk(&self, operation: LocalOperationId, chunk: AdmittedChunk) -> Result<(), SiteStoreError>;
    fn promote_follow(&self, operation: LocalOperationId) -> Result<FollowCommit, SiteStoreError>;
    fn recover(&self, operation: LocalOperationId) -> Result<LocalMutationOutcome, SiteStoreError>;
}
```

- [ ] **RED:** Add migration, quota, overflow, cancellation, crash/reopen, backup/restore, payload-reference, CAS, and atomic O/C/W promotion tests.
- [ ] **GREEN:** Add schema migration and repository implementation through `RiotDatabase` transactions.
- [ ] Run `cargo test -p riot-core --test client_site_repository --test sqlite_backup_restore --test sqlite_lifecycle --test sqlite_integrity_fail_closed --all-features`.
- [ ] Commit as `feat(core): add scalable client site repository`.

### WU-009: Core-owned profile storage command port

**Spec:** Native profile storage ownership steps 1–5; lock rule; local mutation outcomes.

**Files:**

- Create: `crates/riot-core/src/store/profile_storage.rs`
- Create: `crates/riot-core/src/profile/registry.rs`
- Modify: `crates/riot-core/src/store/mod.rs`
- Modify: `crates/riot-core/src/profile/mod.rs`
- Create: `crates/riot-core/tests/profile_storage_worker.rs`

**DoD:**

- `CoreProfileStorage` solely owns `RiotSession`, `EvidenceStore`, site repository, registry, preview/plan handles, database, and a 64-command worker.
- Exactly one mutable command is admitted; a second returns typed Busy and is never queued.
- Queued cancellation returns NotCommitted; started cancellation detaches and recovers exact durable outcome.
- Registry codec/quarantine keys move from FFI to core.
- No command calls back into or awaits FFI `ProfileState`.

**Interface:**

```rust
pub enum ProfileStorageCommand { Inspect(InspectInput), Plan(PlanInput), Commit(CommitInput),
    RegistryLoad, RegistryReplace(RegistryRecord), Site(SiteCommand),
    Configuration(ConfigurationCommand), Backup(BackupInput), Restore(RestoreInput), Close }
pub struct ProfileStoragePortLease { /* revocable, no raw database access */ }
pub enum LocalMutationOutcome { Busy { active_operation_id: [u8; 32], retry_after_seconds: u64 },
    Running { operation_id: [u8; 32], cancellable_before_start: bool },
    Recovering { operation_id: [u8; 32] }, Committed { operation_id: [u8; 32], exact_result: Vec<u8> },
    NotCommitted { operation_id: [u8; 32], retry_context: Vec<u8> }, Closed }
```

- [ ] **RED:** Add queue-full, second mutation, cancel-before/after-start, lock-order, generation discard, shutdown, reopen-after-commit, and public module-export tests in `profile_storage_worker.rs`.
- [ ] **GREEN:** Implement memory/SQLite constructors, opaque handle generations, worker ownership, mutation gate, leases, and registry move; register/export the module through `store/mod.rs`.
- [ ] Run `cargo test -p riot-core --test profile_storage_worker --all-features` and
  `cargo test -p riot-core --all-features`.
- [ ] Commit as `refactor(core): centralize profile storage ownership`.

### WU-010A: FFI storage ownership migration

**Spec:** Native profile storage ownership steps 1–4 and lock-order clauses.

**Files:**

- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/src/mobile_api.rs`
- Modify: `crates/riot-ffi/src/community_registry.rs`
- Modify: `crates/riot-ffi/src/newswire_ffi.rs`
- Create: `crates/riot-ffi/tests/profile_storage_ownership.rs`

**DoD:**

- `LocalProfile` has only a revocable storage-port lease, not `EvidenceStore` or `RiotDatabase`.
- Import/sync child handles retain opaque IDs and weak/revocable leases, not strong core session/storage ownership.
- Every SQLite-capable call phase-splits around `ProfileState`; no database command or await occurs under its mutex.
- Durable registry code is removed from FFI; FFI retains projections/conversion only.

- [ ] **RED:** Add raw-owner, child strong-owner, lock-order, generation-race, and database-under-ProfileState tests.
- [ ] **GREEN:** Route existing evidence/registry flows through `ProfileStorageCommand`, delete durable FFI registry ownership, and retain only opaque handle IDs/projections.
- [ ] Run `cargo test -p riot-ffi --test profile_storage_ownership --all-features` and
  `cargo test -p riot-ffi --all-features`.
- [ ] Commit as `refactor(ffi): adopt core storage command port`.

### WU-010B: Explicit profile runtime and close lifecycle

**Spec:** Native profile lifecycle exact state/order; process endpoint separation.

**Files:**

- Create: `crates/riot-ffi/src/profile_runtime.rs`
- Modify: `crates/riot-ffi/src/lib.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/src/mobile_api.rs`
- Create: `crates/riot-ffi/tests/profile_close_lifecycle.rs`

**DoD:**

- Async idempotent close follows the exact eight-step order, has ten-second network/storage drains, preserves recoverable IDs, blocks same-path reopen until Closed, and never closes the process endpoint.
- New FFI/network admission closes before cancellation; storage leases remain only for terminal recovery and are revoked before worker shutdown.
- CloseIncompleteRecoverable never claims no mutation or releases the database path.

- [ ] **RED:** Add concurrent mutation, queued/running close, child-handle-after-close, same-path reopen, endpoint-survives-profile-close, timeout recovery, and exact close replay tests.
- [ ] **GREEN:** Add `MobileProfileRuntime`, async close export, runtime states, cancellable-operation registry, and strict drain/revoke/join order over WU-010A's leases.
- [ ] Run `cargo test -p riot-ffi --all-features`.
- [ ] **Human checkpoint B:** Present migration/backup proof, lock-order instrumentation, close traces, and existing-feature regression results.
- [ ] Commit as `refactor(ffi): add explicit profile close lifecycle`.

### WU-011A: Process-singleton client internet runtime

**Spec:** System Shape; native network lifecycle.

**Files:**

- Create: `crates/riot-client-net/Cargo.toml`
- Create: `crates/riot-client-net/src/lib.rs`
- Create: `crates/riot-client-net/src/runtime.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**DoD:**

- One process `RiotApplicationRuntime` owns one Tokio runtime and iroh endpoint; constructors reuse it.
- Profile leases cancel only their streams/tasks; application close requires all profile leases closed.
- Runtime task registration, per-profile cancellation, and endpoint shutdown are bounded and independently testable with injected endpoint/task factories.

- [ ] **RED:** Add unit tests inside `runtime.rs` for duplicate construction, cross-profile close, lease cancellation, task drain, and application close ordering; observe missing runtime types.
- [ ] **GREEN:** Create the workspace crate and implement singleton runtime, profile leases, cancellation registry, and idempotent close.
- [ ] Run `cargo test -p riot-client-net runtime`.
- [ ] Commit as `feat(client-net): add shared application runtime`.

### WU-011B: Verified bootstrap and safe dialing

**Spec:** Anchor Bootstrap and Management; ticket-first safe-dial rules.

**Files:**

- Create: `crates/riot-client-net/src/safe_dial.rs`
- Modify: `crates/riot-client-net/src/lib.rs`
- Create: `crates/riot-client-net/tests/runtime_and_dial.rs`

**DoD:**

- HTTPS origins use public port-443 addresses, redirect-free pinned resolution, SNI/cert validation, response bounds, and rebinding protection.
- Root ticket verification and `require:none` equality happen before iroh or HTTPS dialing; descriptor endpoint hints cannot override verified identity.

- [ ] **RED:** Add private/loopback/link-local/multicast/reserved/DNS-rebinding, redirect, endpoint mismatch, expired/downgraded/Arti ticket tests with a fake resolver/connector.
- [ ] **GREEN:** Implement the safe resolver/connector and ticket/descriptor verify-before-dial entry points over WU-011A's runtime lease.
- [ ] Run `cargo test -p riot-client-net`.
- [ ] Commit as `feat(client-net): verify bootstrap before safe dialing`.

### WU-011C: Signed directory feed and cursor protocol

**Spec:** Directory Feeds and Search wire records; feed checkpoint/cursor recovery.

**Files:**

- Create: `crates/riot-anchor-protocol/src/directory.rs`
- Modify: `crates/riot-anchor-protocol/src/lib.rs`
- Modify: `crates/riot-anchor-protocol/schema/riot-anchor-v1.cddl`
- Create: `crates/riot-anchor-protocol/tests/directory_vectors.rs`

**DoD:**

- Empty/first sentinels, signed inclusions, receipts, checkpoints, current-key snapshot records,
  authenticated feed/snapshot cursors, compaction floors, and every cursor failure are canonical.
- Rotation coordinates and current-key reissuance records are independently verifiable.
- Client and server work units consume these canonical types directly; neither defines a private
  directory grammar or treats JSON projections as authority.

- [ ] **RED:** Add golden/hostile tests for sentinel/link order, feed forks, every cursor reason,
  snapshot state digest, checkpoint chain, inclusion/receipt coordinate, and current-key reissuance.
- [ ] **GREEN:** Implement directory record/cursor codecs, signing preimages, verification, and CDDL.
- [ ] Run `cargo test -p riot-anchor-protocol --test directory_vectors`.
- [ ] Commit as `feat(anchor): define signed directory protocol`.

### WU-012A: Typed plural client operations

**Spec:** Typed Client Operation Results; legal mutation boundaries.

**Files:**

- Create: `crates/riot-client-net/src/operations.rs`
- Create: `crates/riot-client-net/src/directory.rs`
- Create: `crates/riot-client-net/src/hosting.rs`
- Create: `crates/riot-client-net/src/reconcile.rs`
- Modify: `crates/riot-client-net/src/lib.rs`

**DoD:**

- Directory, follow, publish, host configuration, refresh, replace, unlist, cancel, and receipt recovery expose the exact typed state/result families and one durable row per destination.
- 429/503 overload is source-specific with validated retry timing; other sources continue.
- Explore never mutates; Follow mutates only after consent and one atomic local commit; hosting/listing/unlisting remain independent.
- Local Busy, Running, Recovering, Committed, NotCommitted, and Closed remain distinct.
- All operations own cancellation tokens, publish one terminal event, and preserve verified receipts/intents through local busy/recovery.
- A deadline-injected reconciliation runner leases the process runtime, atomically claims due persisted schedules through WU-009, reconciles every enabled configured anchor without coupling hosting/listing, persists per-source retry intent before release, and resumes after restart.
- Foreground and platform-bounded background callers use the same runner; expiration/cancellation stops network work at the supplied deadline while retaining exact next-due/recovery state.
- Configuration admits at most 32 distinct normalized hosts; configuration-change arrays are deduplicated and sorted before persistence or emission.
- A merged directory cursor is bound to the normalized query and exact source set that created it; reuse with a different query or source set is rejected before any source request.
- The production `DirectorySourceAdapter` uses WU-011C's signed feed/snapshot records and
  authenticated cursors over WU-011B's verified safe-dial transport. It verifies source signatures,
  checkpoint continuity, cursor binding, expiry/floors, and result bounds before yielding typed
  cards. Fake ports remain test-only and cannot be selected by production constructors.

- [ ] **RED:** Add module unit tests with fake byte transports for every design transition, plural
  partial outcomes, signature/checkpoint/floor/cursor failures, stale-source three-attempt retry,
  relist windows, per-row cancellation, local busy/recovery, profile close, due/not-due claims,
  restart resumption, partial-source scheduled runs, bounded-window expiry, persisted retry,
  33rd-host rejection, normalized-host deduplication, sorted `ConfigurationChange` output, and
  merged-cursor rejection for both changed query and changed source set. Add a production-constructor
  test that proves the adapter emits and consumes WU-011C canonical bytes rather than a fake schema.
- [ ] **GREEN:** Implement the real protocol-backed `DirectorySourceAdapter`, injected byte-transport
  test seam, foreground state machines, and `reconcile.rs`'s deadline-bounded durable scheduler over
  WU-011's runtime and WU-009's core storage command port.
- [ ] Run `cargo test -p riot-client-net` and
  `cargo test -p riot-anchor-protocol --test directory_vectors`.
- [ ] Commit as `feat(client-net): add typed plural operations`.

### WU-012B: UniFFI anchor operations and event streams

**Spec:** Typed Client Operation Results at the native boundary; cancellation/event delivery.

**Files:**

- Modify: `crates/riot-ffi/Cargo.toml`
- Create: `crates/riot-ffi/src/anchor_ffi.rs`
- Modify: `crates/riot-ffi/src/lib.rs`
- Create: `crates/riot-ffi/tests/anchor_operation_contract.rs`
- Modify: `scripts/conference/build-native-core.sh`

**DoD:**

- Local Busy, Running, Recovering, Committed, NotCommitted, and Closed cross UniFFI without collapsing to Internal.
- Every WU-012A state, source attempt, per-destination outcome, consent/retry context, configuration intent, and cancellation terminal is losslessly exported.
- Operation handles expose cancellable async calls/typed callbacks or streams and never strongly own profile storage.
- UniFFI exposes process-runtime acquire/final-close and a deadline-bounded `run_due_reconciliation` entry point whose completion reports exact claimed/completed/deferred work without transferring scheduler policy to native code.

- [ ] **RED:** Add FFI contract tests for every enum variant, sorted map encoding, callback terminality, consent/retry/cancel, weak lease, process-runtime lifetime, bounded background-run deadline, and close interaction.
- [ ] **GREEN:** Export WU-012A records, enums, callbacks/streams, and operation handles through `anchor_ffi.rs`; extend the native build script's dependency-closure assertion to require `riot-client-net`/iroh/Tokio and reject server-only crates.
- [ ] Run `cargo test -p riot-ffi --test anchor_operation_contract --all-features`,
  `cargo test -p riot-ffi --all-features`, and `sh scripts/conference/build-native-core.sh`.
- [ ] **Human checkpoint A:** Present protocol vectors, Cargo feature graphs, ALPN traces, native
  iOS/Android iroh/Tokio cross-compilation, and acceptance of the operator-supplied production
  bootstrap resource.
- [ ] Commit as `feat(ffi): expose typed plural anchor operations`.

### WU-012C: Production bootstrap verification and native resource injection

**Spec:** Anchor Bootstrap and Management; release resource fail-closed contract.

**Files:**

- Modify: `crates/riot-ffi/src/anchor_ffi.rs`
- Create: `crates/riot-ffi/tests/anchor_bootstrap_contract.rs`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/android/app/build.gradle.kts`
- Create: `scripts/anchor/bootstrap-resource-contract.sh`

**DoD:**

- UniFFI exposes the canonical bootstrap verifier/parser from WU-004/WU-006 without transferring
  signature, descriptor-chain, diversity-floor, or development-resource policy to Swift/Kotlin.
- Debug/test builds may embed the visibly development-only fixture. Release builds require
  `RIOT_ANCHOR_BOOTSTRAP_FILE`, copy it as the single runtime bootstrap resource, verify it during the
  build, and fail before packaging if it is absent, development-marked, unsigned, expired,
  below three descriptors/two operators/two failure domains, or mismatched after copy.
- The iOS application resources phase and Android application assets—not only RiotTests/host-JVM
  tests—contain the injected runtime resource. Neither build file references an operator-specific
  source path or commits production bytes.
- WU-022A and WU-023A load this packaged resource through the generated verifier before constructing
  any directory/host client; no hard-coded host list exists in native code.

- [ ] **RED:** Add `anchor_bootstrap_contract.rs` and `bootstrap-resource-contract.sh`; run both
  against Debug and Release packaging and observe that application targets lack a verified runtime
  bootstrap and Release does not fail closed.
- [ ] **GREEN:** Export the verifier and add deterministic resource-copy/verification build phases
  for iOS and Android. The contract creates an ephemeral fixed-test signed input, proves it reaches
  both application packages byte-identically, proves the development fixture is rejected in Release,
  and proves absence/tampering fails before packaging.
- [ ] Run `cargo test -p riot-ffi --test anchor_bootstrap_contract`,
  `sh scripts/anchor/bootstrap-resource-contract.sh`,
  `xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-app-derived`,
  and `(cd apps/android && ./gradlew :app:assembleDebug)`.
- [ ] Commit as `build(native): inject verified anchor bootstrap resources`.

### WU-013A: Anchor crate and forward-only schema

**Spec:** Anchor Repository; Open Hosting Resource Contract; deployment migration/recovery.

**Files:**

- Create: `crates/riot-anchor/Cargo.toml`
- Create: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/src/schema.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**DoD:**

- Forward-only SQLite schema covers every logical table in the design, including preprovisioned removal slots and emergency reserves.
- Schema compatibility is explicit and rollback to a binary that does not declare the current version fails closed.

- [ ] **RED:** Add schema migration unit tests in `schema.rs` for every table/index/constraint, forward migration, newer-version refusal, and preprovisioned slot/reserve rows; observe missing crate/schema.
- [ ] **GREEN:** Add the workspace crate and forward-only schema module.
- [ ] Run `cargo test -p riot-anchor --lib schema::tests` and assert the runner reports the named schema cases rather than zero filtered matches.
- [ ] Commit as `feat(anchor): add forward-only server schema`.

### WU-013B: Anchor repository quotas and recovery

**Spec:** Anchor Repository transactions/accounting/eviction; deployment lease and recovery.

**Files:**

- Create: `crates/riot-anchor/src/repository.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/repository_foundation.rs`

**DoD:**

- WAL/foreign keys/full durability, logical/physical/metadata/WAL/staging/idempotency/conflict/log/static independent accounting, immutable read snapshots, deployment token/lease, and readiness recovery are enforced.
- Payload dedup never reduces per-community logical charge.
- Expired staging and deterministic eviction obey signed retention horizons.

- [ ] **RED:** Add deployment-lease clone, all-class ceiling, cross-community dedup, immutable snapshot, eviction-order, and crash-recovery tests over WU-013A's migrated schema.
- [ ] **GREEN:** Implement the repository service layer; handlers in later units may not access raw connections.
- [ ] Run `cargo test -p riot-anchor --test repository_foundation`.
- [ ] Commit as `feat(anchor): add durable repository foundation`.

### WU-014: Control admission, idempotency, work, and Prepare

**Spec:** `riot/anchor/1` admission ordering; PrepareHost; GetWorkChallenge; GetOperation.

**Files:**

- Create: `crates/riot-anchor/src/control.rs`
- Create: `crates/riot-anchor/src/idempotency.rs`
- Create: `crates/riot-anchor/src/work.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/control_prepare.rs`

**DoD:**

- Bounds → canonical decode/re-encode → request digest → constant-time lookup precedes expensive checks and durable claims.
- Pre-claim busy/quota/work refusals create no row and same-key retry can succeed; claimed changed-body replay is conflict without disclosure.
- Prepare atomically stores operation, base generation, staged O/C/W, deterministic namespace tokens, exact Prepared response, expiry, and originating kind.
- GetOperation exposes exact prepared/terminal lifecycle across restart/expiry/unknown IDs.
- Work challenges/stamps bind every required coordinate and pressure policy.

- [ ] **RED:** Add ordering spies, collision/replay, restart, pre-claim retry-through-success, changed stamp, token derivation/rotation, pressure-band, and GetOperation lifecycle tests in `control_prepare.rs`.
- [ ] **GREEN:** Implement control admission service over `AnchorRepository`, idempotency index, work verifier, Describe/challenge/Prepare/GetOperation handlers.
- [ ] Run `cargo test -p riot-anchor --test control_prepare --test repository_foundation`.
- [ ] Commit as `feat(anchor): implement recoverable control prepare`.

### WU-015: Staged sync admission, composite Commit, and receipt recovery

**Spec:** Composite transaction; Commit matrix; manifest/Meadowcap ingress; TDD Slice 3.

**Files:**

- Create: `crates/riot-anchor/src/sync_service.rs`
- Create: `crates/riot-anchor/src/hosting.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/hosting_commit.rs`
- Create: `crates/riot-anchor/tests/hosting_failpoints.rs`

**DoD:**

- O is admitted first; digest-matched manifest authorizes exact C/W routing; every entry passes Meadowcap and anchor-profile bounds.
- Direction chunks remain private; final O/C/W promotion, references, generation CAS, receipt, token invalidation, and terminal operation/Commit result are one transaction.
- Every Commit refusal has the exact reusable/terminal cleanup disposition.
- Crash/lost delivery reconstructs byte-identical receipt through GetOperation; two same-base commits have one winner.

- [ ] **RED:** Add every named stage/promotion failpoint, all Commit refusal rows, same-base races, manifest equivocation/transport mismatch, stale source/base, operation expiry, and lost-receipt tests.
- [ ] **GREEN:** Adapt `Sync2Repository` to anchor staging and implement final validation/CAS/promotion/receipt recovery.
- [ ] Run `cargo test -p riot-anchor --test hosting_commit --test hosting_failpoints` and
  `cargo test -p riot-anchor-protocol --test sync2_duplex`.
- [ ] Commit as `feat(anchor): commit composite hosting atomically`.

### WU-015B: Atomic ordinary listing submission and replay

**Spec:** Ordinary `SubmitListing` lifecycle; inclusion/receipt atomicity; idempotent retry and recovery.

**Files:**

- Create: `crates/riot-anchor/src/listing.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/listing_submit.rs`
- Create: `crates/riot-anchor/tests/listing_failpoints.rs`

**DoD:**

- The ordinary listing/refresh handler applies WU-014 admission ordering, verifies current hosted generation and listing authority, and rejects listing-before-hosting without creating durable state.
- One repository transaction claims the global idempotency key, appends exactly one signed inclusion, updates current listing state, invalidates the affected directory/search projection generation, creates the signed listing receipt, and stores the byte-identical terminal operation result.
- Same-key/same-body retry returns the stored terminal bytes without a second inclusion; same-key/changed-body retry conflicts without result disclosure.
- Crash at every transaction failpoint is wholly absent or wholly committed. Lost delivery is recoverable through `GetOperation`; refresh replaces current state while retaining the signed feed history required by the design.
- The Willow-owning admission adapter verifies the listing entry signature, root-signed delegate grant,
  complete Meadowcap capability chain (`is_valid`/`does_authorise`), exact root/listing coordinate,
  grant epoch/time, and embedded ticket/root signature before constructing a
  non-publicly-constructible `VerifiedListingAuthority` proof token. The transaction service accepts
  that token rather than unverified listing bytes; `resolve_listing` is unreachable from a caller
  that has not completed all signature/capability checks.

- [ ] **RED:** Add ordinary submit, refresh, listing-before-hosting, stale generation, changed-body
  replay, lost-delivery, and same-key retry tests; add hostile mutations for entry/grant/cap/root
  signatures, receiver/area/time/root/epoch/ticket mismatches, and a compile-fail/private-constructor
  assertion for `VerifiedListingAuthority`; inject a failpoint at every durable mutation and assert
  zero partial state, one inclusion, one projection-generation invalidation, and byte-identical
  receipt/terminal replay.
- [ ] **GREEN:** Implement the verified listing-authority adapter and ordinary `SubmitListing`
  service exclusively through `AnchorRepository`'s transaction boundary and WU-014's shared
  admission/idempotency machinery.
- [ ] Run `cargo test -p riot-anchor --test listing_submit --test listing_failpoints --test control_prepare --test hosting_commit --test hosting_failpoints` and `cargo test -p riot-anchor-protocol --test directory_vectors`.
- [ ] Commit as `feat(anchor): submit ordinary listings atomically`.

### WU-016: Reserved owner removal and crash-safe checkpoints

**Spec:** SubmitListing removal lifecycle; removal lanes; CheckpointWorkV1; TDD Slices 3 and 7.

**Files:**

- Create: `crates/riot-anchor/src/removal.rs`
- Create: `crates/riot-anchor/src/checkpoint.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/removal_reserve.rs`
- Create: `crates/riot-anchor/tests/checkpoint_recovery.rs`

**DoD:**

- Visibility atomically reserves one of `2*L` slots; per-root two-slot/relist window and expiry release rules are exact.
- One global idempotency index spans ordinary/reserved classes without result disclosure.
- Direct-root protected quarter, aggregate 51-job/52-MiB caps, delegated lane, fair two-round scheduler, reserved verifier/writer, and pre-claim overload are instrumented.
- Checkpoint planning freezes all bytes/names/timestamps/members/removals; filesystem publication and atomic floor/result advancement recover at every failpoint.
- Removal acknowledgement does not wait for physical compaction; maximum-size removal survives ordinary row/byte/WAL exhaustion.

- [ ] **RED:** Add reservation races, two-cycle relist, cross-class keys, invalid-candidate saturation, aggregate cap, maximum-record exhaustion, every checkpoint publication/reclaim failpoint, and lost terminal delivery tests.
- [ ] **GREEN:** Implement removal scheduler, slots, work records, immutable files, recovery, and bounded maintenance.
- [ ] Run `cargo test -p riot-anchor --test removal_reserve --test checkpoint_recovery`.
- [ ] **Human checkpoint C:** Present storage failpoint matrix, reserved-lane fairness metrics, and maximum-size removal evidence.
- [ ] Commit as `feat(anchor): guarantee recoverable owner removal`.

### WU-017B: Directory persistence, search, and visibility

**Spec:** Directory Feeds and Search behavior; listing/hosting separation; plural search.

**Depends on:** WU-011C canonical directory records, WU-013B repository services, WU-015B ordinary
listing transactions, and WU-016 removal/checkpoint recovery. This unit cannot begin before all four
are committed.

**Files:**

- Create: `crates/riot-anchor/src/directory.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/directory_feed.rs`

**DoD:**

- Hosting never implies listing; listing-before-hosting is rejected; eviction suspends without losing owner removal; refresh restores only while valid.
- Search merge is deterministic by root, exposes source/coverage/exclusions, and does not claim endorsement.
- Rotation reissues current snapshot state before readiness.
- Directory-card cache lifetime never exceeds listing expiry; unlisting invalidates only directory/search generations and never removes a still-hosted direct projection.

- [ ] **RED:** Add feed fork/reorder, sentinel, stale/after-head/malformed/wrong checkpoint/generation/regressed/expired cursor, conflicting listing, listing-before-hosting, rotation, suspension/restore, cache-at-expiry, unlist-directory-only, emergency compaction, and deterministic search-merge cases in `directory_feed.rs`.
- [ ] **GREEN:** Implement repository-backed inclusion/checkpoint/snapshot/search/visibility services in one cohesive directory module.
- [ ] Run `cargo test -p riot-anchor --test directory_feed --test listing_submit --test removal_reserve --test checkpoint_recovery` and `cargo test -p riot-anchor-protocol --test directory_vectors`.
- [ ] Commit as `feat(anchor): publish signed plural directory feeds`.

### WU-018A: Configured-peer protocol and channel binding

**Spec:** Configured-peer authentication wire contract and `PrepareReplica` attestation records.

**Files:**

- Create: `crates/riot-anchor-protocol/src/peer.rs`
- Modify: `crates/riot-anchor-protocol/src/lib.rs`
- Modify: `crates/riot-anchor-protocol/schema/riot-anchor-v1.cddl`
- Create: `crates/riot-anchor-protocol/tests/peer_vectors.rs`

**DoD:**

- Descriptor exchange, exact hello/proof transcript, TLS exporter context, role/nonces/endpoints, challenge, source attestation, and every digest/preimage are canonical and bounded.
- Replay, reflection, role swap, endpoint/connection substitution, stale descriptor, and second-use cases fail deterministically.

- [ ] **RED:** Add golden/hostile vectors for both roles, omitted versus present-empty exporter context, reflection/replay, descriptor pages, challenge/attestation binding, expiry, and second use.
- [ ] **GREEN:** Implement peer records, canonical transcript/preimages, verification, and CDDL.
- [ ] Run `cargo test -p riot-anchor-protocol --test peer_vectors`.
- [ ] Commit as `feat(anchor): define authenticated peer protocol`.

### WU-018B: Replica transfer and bounded gossip service

**Spec:** PrepareReplica lifecycle; Gossip amplification boundary; TDD Slice 4.

**Depends on:** WU-007 ALPN router, WU-013B repository services, WU-015 staging/commit adapter, and
WU-018A authenticated peer records. This unit cannot begin before all four are committed.

**Files:**

- Create: `crates/riot-anchor/src/peer.rs`
- Create: `crates/riot-anchor/src/gossip.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/peer_auth.rs`

**DoD:**

- Descriptor exchange, mutual proof, and configured-rule lookup gate every peer-only operation.
- One-use challenge/attestation binds current source/destination descriptors, peer session generation, immutable source generation/snapshots, key, root, and connection.
- Source change emits authenticated `stale_source`; destination atomically terminalizes staging/tokens/Prepare mapping. Session close/rotation/restart yields `peer_context_changed`.
- Client requests never trigger background fanout; scheduler respects peer/session/hour budgets and reaches deterministic three-anchor quiescence.

- [ ] **RED:** Add reflection/role/exporter/replay/stale hello, descriptor refresh, challenge double-use, source mutation, connection loss, rotation, crash-orphan, gossip loop, and three-anchor convergence tests in `peer_auth.rs`.
- [ ] **GREEN:** Implement peer service, startup invalidation, replica adapter, and deterministic scheduler over WU-018A types.
- [ ] Run `cargo test -p riot-anchor --test peer_auth --test repository_foundation --test hosting_commit` and `cargo test -p riot-anchor-protocol --test peer_vectors`.
- [ ] Commit as `feat(anchor): add authenticated bounded gossip`.

### WU-019: Bounded daemon ingress and control/sync serving

**Spec:** Public iroh and HTTPS admission; encoded bounds; Privacy and Logging.

**Depends on:** WU-007 router, WU-014 control admission, WU-015 hosting sync, WU-015B listing,
WU-016 reserved removal, WU-017B directory services, and WU-018B peer/gossip services. The daemon
must route the real service implementations; stub handlers are not an acceptable intermediate.

**Files:**

- Create: `crates/riot-anchor/src/admission.rs`
- Create: `crates/riot-anchor/src/daemon.rs`
- Create: `crates/riot-anchor/src/main.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/ingress_limits.rs`

**DoD:**

- Configuration resolves all 82 effective/absolute values, secrets, deployment lease, peers, origins, log sinks, and descriptor before readiness.
- Iroh and TCP/TLS/HTTP limits begin before expensive allocation and release permits across every error/timeout path.
- HTTP is TLS 1.3 + HTTP/1.1 only; methods, Range, upgrade, compression, header count/line/aggregate, keep-alive count, idle/absolute lifetime, queues, rates, DB snapshots, CPU/wall, and response bytes are bounded.
- Logs omit forbidden material, share byte/file caps, rotate/delete safely, and cannot consume authoritative headroom.

- [ ] **RED:** Add handshake churn, slow ClientHello/header/frame/trickle/write, stream abuse, 65 headers, long field, method/Range/upgrade/compression, queue/rate/snapshot/response exhaustion, permit leaks, and log-saturation tests while a reserved removal completes.
- [ ] **GREEN:** Define validated daemon configuration beside its lifecycle in `daemon.rs`, then wire admission partitions, ALPN handlers, graceful shutdown, readiness/liveness, and bounded logging.
- [ ] Run `cargo test -p riot-anchor --test ingress_limits --test control_prepare --test hosting_commit --test hosting_failpoints --test listing_submit --test listing_failpoints --test removal_reserve --test checkpoint_recovery --test directory_feed --test peer_auth`.
- [ ] Commit as `feat(anchor): serve bounded public ingress`.

### WU-020P: Dependency-neutral web snapshot contract

**Spec:** Safe Web Projection typed daemon→renderer boundary.

**Files:**

- Create: `crates/riot-anchor-protocol/src/web_snapshot.rs`
- Modify: `crates/riot-anchor-protocol/src/lib.rs`
- Modify: `crates/riot-anchor-protocol/schema/riot-anchor-v1.cddl`
- Create: `crates/riot-anchor-protocol/tests/web_snapshot_vectors.rs`

**DoD:**

- `AnchorWebSnapshotV1` and its bounded nested text/route/record types live in the dependency-light
  protocol crate, have canonical positional CBOR and hostile/golden vectors, and contain no database,
  network, filesystem, credentials, owner HTML, or executable content.
- Both daemon and renderer depend on this one type/codec. The renderer never depends on
  `riot-anchor`; the daemon and renderer cannot define parallel snapshot grammars.

- [ ] **RED:** Add size/count/text/path/profile/unknown-field/canonicality vectors and a dependency
  test proving the snapshot module does not pull SQLite/network/server crates; observe the missing
  shared type.
- [ ] **GREEN:** Implement the bounded snapshot codec/CDDL and export it from
  `riot-anchor-protocol`.
- [ ] Run `cargo test -p riot-anchor-protocol --test web_snapshot_vectors --test dependency_boundary`.
- [ ] Commit as `feat(anchor): define shared web snapshot contract`.

### WU-020A: Typed web snapshot and daemon publication service

**Spec:** Safe Web Projection steps 1 and 4; immutable daemon ownership transfer.

**Files:**

- Create: `crates/riot-anchor/src/projection.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/projection_publish.rs`

**DoD:**

- Rust reads one admitted immutable repository snapshot and emits WU-020P's bounded
  `riot_anchor_protocol::AnchorWebSnapshotV1`.
- The daemon validates output manifests and performs no-follow copy/hash/fsync/read-only ownership transfer before atomic generation publication.
- Output validation rejects links/devices/sockets/path escapes/case collisions/extras/MIME/size/count/digest changes and post-validation mutation.
- Direct projections follow currently hosted admitted records: unlisting or ticket expiry leaves them readable, hosting eviction removes them, and record/manifest/moderation/hosting changes invalidate exactly the affected content generation.

- [ ] **RED:** Add path/symlink/hardlink/MIME/collision/temp-quota/post-validation-mutation, publish failpoint, unlist, ticket-expiry, hosting-eviction, record-change, manifest-change, moderation-change, unaffected-generation, and restart tests against a fake renderer.
- [ ] **GREEN:** Implement snapshot extraction, manifest validation, immutable ownership transfer, dependency-indexed invalidation, and generation retention behind a renderer port.
- [ ] Run `cargo test -p riot-anchor --test projection_publish`.
- [ ] Commit as `feat(anchor): publish validated immutable generations`.

### WU-020B: Isolated text-only renderer

**Spec:** Safe Web Projection steps 2–3 and renderer deployment boundary; TDD Slice 5.

**Files:**

- Create: `crates/riot-anchor-renderer/Cargo.toml`
- Create: `crates/riot-anchor-renderer/src/main.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**DoD:**

- Renderer depends on `riot-anchor-protocol` only, has no database/network/server code or credentials,
  and reads only WU-020P's `AnchorWebSnapshotV1`.
- Text/attribute escaping, canonical-ID routes, exact hashed stylesheet, no executable owner content, and unknown-profile omission are enforced.
- Output manifest lists every path/length/BLAKE3/MIME and stays within file/byte/inode limits.
- Existing gateway fixtures remain byte-pinned and unchanged.

- [ ] **RED:** Add unit tests in `main.rs` for hostile strings in every context, HTML/SVG/attribute injection, unknown profiles, canonical routes, CSP stylesheet hash, file-count, and byte limits.
- [ ] **GREEN:** Add the workspace renderer binary and pure text templates/output manifest.
- [ ] Run `cargo test -p riot-anchor-renderer`,
  `cargo test -p riot-anchor --test projection_publish`,
  `(cd apps/gateway && python3 -m unittest discover -s tests)`, and
  `sh scripts/conference/gateway-smoke.sh`.
- [ ] Commit as `feat(anchor): publish isolated static web projections`.

### WU-021A: Canonical v2 handoff protocol

**Spec:** Canonical Web-to-App Handoff wire contract and legacy discrimination.

**Files:**

- Create: `crates/riot-anchor-protocol/src/handoff.rs`
- Modify: `crates/riot-anchor-protocol/src/lib.rs`
- Modify: `crates/riot-anchor-protocol/schema/riot-anchor-v1.cddl`
- Create: `crates/riot-anchor-protocol/tests/handoff_vectors.rs`

**DoD:**

- Handoff canonical CBOR is at most 1800 bytes, contains at most three 192-byte hints, and produces byte-identical HTTPS/custom-scheme paths.
- Signed ticket core, optional full destination, and replaceable hints cannot overwrite each other's authority.
- Malformed/oversize/duplicate/unknown variants and destination namespace mismatch fail closed; legacy v1/open references remain distinguishable.

- [ ] **RED:** Add exact URL/vector, hint replacement, bound, authority-separation, malformed, destination, and legacy-link tests.
- [ ] **GREEN:** Implement canonical handoff codec/URL parsing and CDDL.
- [ ] Run `cargo test -p riot-anchor-protocol --test handoff_vectors`.
- [ ] Commit as `feat(anchor): define canonical v2 handoff`.

### WU-021B: HTTPS APIs, overload pages, and install journey

**Spec:** HTTPS API table; web-to-app journey states; public security headers.

**Depends on:** WU-017B directory services, WU-019 bounded TLS/HTTP ingress, WU-020B validated
projections, and WU-021A handoff codec. HTTPS handlers must be installed into WU-019's real daemon,
not exercised only as detached functions.

**Files:**

- Create: `crates/riot-anchor/src/http.rs`
- Modify: `crates/riot-anchor/src/lib.rs`
- Create: `crates/riot-anchor/tests/http_api.rs`
- Create: `scripts/anchor/http-route-contract.sh`

**DoD:**

- All documented routes serve from repository/projection services, never raw DB calls.
- Feed/search JSON is lossless and bounded; old feed cursor is typed 409; overload is typed 429/503 with `Retry-After` and an accessible fixed error page preserving the URL.
- WU-021A handoffs are served with no query/cookie/fingerprint/redirect and preserve root/destination across install return.
- CSP/HSTS/nosniff/Permissions-Policy/frame/referrer headers apply to success and error responses.

- [ ] **RED:** Add every GET/HEAD method/path/status/shape (including byte-identical headers/status and an empty HEAD body), rejection of all other methods, cursor 409, overload, response ceiling, cache expiry, security header, installed/no-app/install-unavailable/expired/malformed/destination-absent, and legacy-link compatibility test in `http_api.rs`. Add `http-route-contract.sh` to start the bounded test daemon with deterministic certificates/data and probe the public socket; observe failure before the handlers are installed.
- [ ] **GREEN:** Implement service-backed handlers and handoff/install page controls over WU-021A's codec.
- [ ] Run `cargo test -p riot-anchor --test http_api --test projection_publish --test ingress_limits`,
  `cargo test -p riot-anchor-protocol --test handoff_vectors`, and
  `sh scripts/anchor/http-route-contract.sh`.
- [ ] Commit as `feat(anchor): serve directory web and v2 handoff`.

### WU-022A: iOS anchor state models and profile lifecycle adapter

**Spec:** Native UX State Model; Accessibility; typed client results.

**Files:**

- Create: `apps/ios/Riot/Anchors/AnchorFlows.swift`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Create: `apps/ios/RiotTests/AnchorFlowsTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`

**DoD:**

- Host-testable iOS models preserve zero/partial/cached/overloaded/unreachable and all typed Follow/Publish/host-management states.
- Follow consent explains public storage/metadata, shows destination/storage, and drives exact busy/recovery/cancel/quarantine outcomes.
- Publish maintains stable per-anchor rows for hosting/listing/unlisting/refresh/source-change/relist/countdown/cancellation/receipts.
- Host management starts with verified defaults and supports add/verify/consent, enable/disable/remove/reset and replacement semantics.
- `RiotProfileRepository` exposes generated async anchor operations and an awaited, idempotent profile close without relying on wrapper disposal.
- The model loads WU-012C's packaged runtime bootstrap through the generated verifier before
  constructing anchor clients; missing/invalid/development-in-Release resources produce a typed
  unavailable state without dialing.
- iOS and macOS RiotKit targets compile the new shared anchor model in the same commit that creates it.
- VoiceOver/keyboard focus is retained, rows do not reorder under focus, live announcements are polite, countdown announcements are sparse, reduced motion is respected, and QR has copy/share alternatives.

- [ ] **RED:** Add injected-port model tests for every state/copy/action, focus-preserving row identity, install-return, cancellation, recovery, configuration intent, and awaited close ordering.
- [ ] **GREEN:** Implement pure model projections in `AnchorFlows.swift`, expose generated operations/async close through the repository, and register the shared source in both Apple RiotKit targets without refactoring the existing local app directory.
- [ ] Update project files only in the commit work unit if sources are not synchronized automatically; validate `git diff` excludes user workspace state.
- [ ] Run
  `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-derived -only-testing:RiotTests/AnchorFlowsTests`
  and
  `xcodebuild build -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS' -derivedDataPath build/macos-derived`.
- [ ] Commit as `feat(ios): add public host states and profile lifecycle`.

### WU-022B: iOS Explore, Follow, Publish, handoff, and host management

**Spec:** Native UX State Model; Accessibility; Web→Native Handoff.

**Files:**

- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/Riot/RiotApp.swift`
- Modify: `apps/ios/RiotTests/AnchorFlowsTests.swift`
- Modify: `apps/ios/RiotTests/ProfileRecoveryTests.swift`

**DoD:**

- Explore is reachable from first-run and switcher; Follow, Publish, relist/source-change, and host management render WU-022A states without collapsing failures.
- Exact v2 handoff bytes survive install return, cancellation, and process recreation until explicit consent applies them.
- Profile lock, reset, and replacement cancel anchor work and explicitly await `MobileProfile.close()` through the repository; no path treats wrapper disposal as close.
- VoiceOver/keyboard focus, stable row identity, reduced motion, sparse countdown announcements, and QR copy/share alternatives match the approved contract.

- [ ] **RED:** Extend profile recovery/lifecycle tests with injected close ports for lock, reset, replacement, concurrent close, timeout-recoverable, and same-path reopen; extend WU-022A's suite with handoff and shell tests.
- [ ] **GREEN:** Wire the shell, app URL lifecycle, model reset/replacement, handoff consent, and host flows through WU-022A's repository/model adapter.
- [ ] Run
  `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-derived -only-testing:RiotTests/AnchorFlowsTests -only-testing:RiotTests/ProfileRecoveryTests`
  and
  `xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-app-derived`.
- [ ] Commit as `feat(ios): integrate public host flows and awaited close`.

### WU-022C: iOS process runtime and bounded background reconciliation

**Spec:** Native network lifecycle; ongoing reconciliation; platform-bounded task windows.

**Files:**

- Create: `apps/ios/Riot/Anchors/AnchorBackgroundTasks.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/RiotApp.swift`
- Modify: `apps/ios/RiotTests/ProfileRecoveryTests.swift`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`

**DoD:**

- `RiotApp` acquires the singleton application runtime at process start; every profile owns a lease, and controlled final close is awaited only after the last profile close/lease release.
- `BGTaskScheduler` registration uses a fixed identifier and calls only WU-012B's deadline-bounded reconciliation entry point; expiration cancels network work while Rust preserves retry intent.
- Foreground wake and background launch claim the same persisted due schedule, never duplicate a run, resume after restart, and reschedule from the returned exact next-due state.
- Scene/app teardown awaits profile close; wrapper disposal and task expiration are never treated as process-runtime close.
- `AnchorBackgroundTasks.swift` is registered in the explicit iOS project group and target Sources build phase in the same commit that creates it.

- [ ] **RED:** Add injected process-runtime/background-task tests for startup once, multiple profiles, close ordering, due/not-due, duplicate wake, expiration, restart resumption, persisted retry, and next-task scheduling.
- [ ] **GREEN:** Add the thin task adapter to the iOS Xcode target and wire process-runtime/profile-lease lifetime through `RiotApp`/`RiotAppModel`.
- [ ] Run
  `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-derived -only-testing:RiotTests/ProfileRecoveryTests`
  and
  `xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-app-derived`;
  `ProfileRecoveryTests` must instantiate `AnchorBackgroundTasks`, so zero target membership is a compile failure.
- [ ] Commit as `feat(ios): run bounded background anchor reconciliation`.

### WU-022C2: iOS background registration contract

**Spec:** Native network lifecycle; platform-bounded task registration.

**Files:**

- Modify: `apps/ios/Riot/Info.plist`
- Modify: `apps/ios/RiotTests/ProfileRecoveryTests.swift`

**DoD:**

- The fixed `BGTaskScheduler` identifier registered by WU-022C is declared exactly once in `BGTaskSchedulerPermittedIdentifiers` and matches the test-injected scheduler request.
- A missing, duplicated, or mismatched identifier fails the focused contract test before app launch; no broad background mode or network entitlement is added.

- [ ] **RED:** Add a bundle-configuration contract test that reads the built plist and fails while WU-022C's identifier is absent.
- [ ] **GREEN:** Declare the exact permitted identifier in `Info.plist` without widening background capabilities.
- [ ] Run
  `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-derived -only-testing:RiotTests/ProfileRecoveryTests`,
  `xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-app-derived`, and
  `/usr/libexec/PlistBuddy -c 'Print :BGTaskSchedulerPermittedIdentifiers' build/ios-app-derived/Build/Products/Debug-iphonesimulator/Riot.app/Info.plist | grep -Fx 'org.riot.anchor.reconcile'`.
- [ ] Commit as `build(ios): declare bounded anchor reconciliation task`.

### WU-022D: macOS shared-Swift integration closure

**Spec:** Existing-platform compatibility for shared native sources.

**Files:**

- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`
- Create: `apps/macos/RiotTests/AnchorSharedCompileTests.swift`

**DoD:**

- The macOS RiotKit target compiles shared `AnchorFlows.swift`, the updated `AppModel.swift`, `ProfileRepository.swift`, and every shared anchor type referenced by `ConferenceShellView.swift`.
- iOS-only `BGTaskScheduler` and application-lifecycle adapters remain compile-time isolated; macOS uses the shared typed models without silently enabling iOS background behavior.
- The macOS app and portable tests link the generated UniFFI anchor surface and retain all existing shared-source tests.

- [ ] **RED:** Add a macOS compile-contract test that instantiates the shared anchor state/lifecycle seams and observe missing project references or platform guards.
- [ ] **GREEN:** Add the shared source/test references and the minimum explicit platform guards required for the unchanged macOS product surface.
- [ ] Run `xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'` and `xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS -destination 'platform=macOS'`.
- [ ] Commit as `build(macos): compile shared public anchor flows`.

### WU-023A: Android anchor state models and generated binding adapter

**Spec:** Native UX State Model; Accessibility; typed client results.

**Files:**

- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/AnchorFlows.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/RiotController.kt`
- Create: `apps/android/app/src/test/kotlin/org/riot/evidence/AnchorFlowsTest.kt`
- Modify: `apps/android/app/build.gradle.kts`

**DoD:**

- Host-JVM-testable Android models expose the same state distinctions, consent, per-anchor outcomes, host management, handoff preservation, retry/cancel/recovery, copy, and accessibility semantics as iOS.
- Content descriptions distinguish partial/stale/refused/expired/unreachable; focus row keys stay stable; countdown and reduced-motion behavior match the contract.
- `RiotController` exposes the generated async anchor port without duplicating Rust policy, and Gradle contains the coroutine runtime required by generated UniFFI bindings.
- `AnchorFlows` loads WU-012C's packaged runtime bootstrap through the generated verifier before
  constructing anchor clients; invalid/missing resources remain typed unavailable without dialing.

- [ ] **RED:** Add fake-port tests mirroring the iOS matrix and verify no state collapses to generic error/zero results.
- [ ] **GREEN:** Implement pure state mapping and view builders in `AnchorFlows.kt`, expose the generated port from `RiotController`, and add generated async-binding coroutine support to Gradle.
- [ ] Run `(cd apps/android && ./gradlew :app:testDebugUnitTest :app:assembleDebug)`.
- [ ] Commit as `feat(android): add plural public host state models`.

### WU-023B: Android anchor flows and `riot:` handoff integration

**Spec:** Native UX State Model; Accessibility; Web→Native Handoff.

**Files:**

- Modify: `apps/android/app/src/main/AndroidManifest.xml`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/RiotController.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Create: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/AnchorFlowsInstrumentedTest.kt`
- Modify: `apps/android/app/src/androidTest/kotlin/org/riot/evidence/BindingSemanticsTest.kt`

**DoD:**

- Explore is reachable from first-run and the space switcher, and Follow, Publish, relist/source-change, and host management render the typed WU-023A states without collapsing failures.
- An exported, browsable `riot:` intent filter routes exact v2 handoff bytes into a consent sheet; cancellation or process recreation preserves the pending handoff without applying it.
- `RiotController` exposes an idempotent suspending close that cancels anchor operations and awaits the privately owned `MobileProfile.close`; Activity teardown calls and awaits that owner path rather than trusting `AutoCloseable` wrapper disposal.
- TalkBack focus, stable row identity, reduced motion, sparse countdown announcements, and QR copy/share alternatives match the approved accessibility contract.

- [ ] **RED:** Add an instrumentation test that launches a canonical `riot:` v2 intent, verifies byte-preserving pending state and consent, then covers cancel, recreation, malformed input, and successful apply; extend binding semantics with cancel-before-close, awaited profile shutdown, idempotent second close, and no post-close callback cases.
- [ ] **GREEN:** Add the manifest filter, make `RiotController` the explicit async profile-shutdown owner, and wire `MainActivity` to await that owner while integrating WU-023A models/generated ports for first-run, switcher, host management, and handoff lifecycle.
- [ ] Run `(cd apps/android && ./gradlew :app:testDebugUnitTest :app:assembleDebug :app:assembleDebugAndroidTest :app:connectedDebugAndroidTest)`.
- [ ] Commit as `feat(android): integrate public host flows and handoff`.

### WU-023C: Android process runtime and bounded background reconciliation

**Spec:** Native network lifecycle; ongoing reconciliation; platform-bounded task windows.

**Files:**

- Modify: `apps/android/app/src/main/AndroidManifest.xml`
- Create: `apps/android/app/src/main/kotlin/org/riot/evidence/RiotApplication.kt`
- Modify: `apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt`
- Modify: `apps/android/app/src/test/kotlin/org/riot/evidence/AnchorFlowsTest.kt`
- Modify: `apps/android/app/build.gradle.kts`

**DoD:**

- A process `Application` owner acquires the singleton runtime exactly once; activities close profile leases explicitly, and controlled final shutdown/tests await application-runtime close only after all leases end.
- Unique WorkManager work invokes only WU-012B's deadline-bounded reconciliation API; stop/timeout cancels network work while preserving Rust-owned retry intent.
- Foreground wake, worker launch, duplicate worker, restart, and reschedule paths atomically claim one persisted due run and retain exact per-anchor partial outcomes.
- Manifest/application registration and WorkManager constraints do not request broad background/network privileges or move policy into Kotlin.

- [ ] **RED:** Add fake runtime/worker tests for startup once, profile-before-runtime close, due/not-due, unique work, stop/timeout, process restart, persisted retry, and exact rescheduling.
- [ ] **GREEN:** Add the application runtime owner, WorkManager adapter/configuration, lifecycle wiring, and manifest registration over generated UniFFI calls.
- [ ] Run `(cd apps/android && ./gradlew :app:testDebugUnitTest :app:assembleDebug :app:assembleDebugAndroidTest :app:connectedDebugAndroidTest)`.
- [ ] **Human checkpoint D:** Demonstrate web→iOS/Android handoff, background restart/resume, partial-source Explore, one-anchor failover, per-row publish cancellation, and accessible relist/source-change states.
- [ ] Commit as `feat(android): run bounded background anchor reconciliation`.


### WU-026A: Reproducible daemon and renderer OCI images

**Spec:** Deployment and Recovery Contract; renderer isolation boundary; reproducible release inputs.

**Depends on:** WU-019 daemon binary, WU-020B renderer binary, and WU-021B installed HTTPS service.
The image contract builds the completed public service, not an earlier daemon without its routes.

**Files:**

- Create: `deploy/riot-anchor/Dockerfile.anchor`
- Create: `deploy/riot-anchor/Dockerfile.renderer`
- Create: `deploy/riot-anchor/Dockerfile.anchor.dockerignore`
- Create: `deploy/riot-anchor/Dockerfile.renderer.dockerignore`
- Create: `scripts/anchor/image-contract.sh`

**DoD:**

- Multi-stage builds use digest-pinned builder/runtime bases, `cargo build --locked`, an explicit
  target triple, and only the expected stripped binary/config-support files in each runtime image.
- Runtime images are unprivileged, contain no compiler/package manager/source tree/secrets, declare
  only required writable mount points, and set deterministic OCI metadata from an explicit
  `SOURCE_DATE_EPOCH` and revision argument.
- The daemon and renderer are separate images. The renderer image contains no daemon, SQLite,
  network client, shell, or credential-loading surface.
- Both builds use repository root as context with Dockerfile-specific deny-by-default ignore files,
  so the locked workspace manifests and only the source/assets needed by that image are available;
  the nested ignore files are selected by Docker for their matching `-f` path.
- `image-contract.sh` always requires a local OCI builder, builds both images from the locked
  workspace, and inspects user, entrypoint, layers, exposed ports, and filesystem allowlists.
  Missing Docker/buildx is a blocking external prerequisite, never a passing static-only mode.

- [ ] **RED:** Add `scripts/anchor/image-contract.sh`; run it and observe failure because both
  Dockerfiles and their pinned/locked/non-root contracts are absent.
- [ ] **GREEN:** Add both multi-stage Dockerfiles and matching Dockerfile-specific deny-by-default
  ignore files. Make the script run
  `docker build --file deploy/riot-anchor/Dockerfile.anchor --tag riot-anchor:test .` and
  `docker build --file deploy/riot-anchor/Dockerfile.renderer --tag riot-anchor-renderer:test .`,
  then fail on an unpinned `FROM`, unlocked Cargo build, root user, unexpected runtime binary,
  renderer network tool/library, writable root, undeclared port, or missing required workspace input.
- [ ] Run `sh scripts/anchor/image-contract.sh`.
- [ ] Commit as `build(anchor): add reproducible daemon and renderer images`.

### WU-026B: Deployment, readiness, recovery, and operator runbook

**Spec:** Deployment and Recovery Contract; Privacy and Logging; round-18 runbook note.

**Files:**

- Create: `deploy/riot-anchor/compose.yaml`
- Create: `deploy/riot-anchor/riot-anchor.example.toml`
- Create: `deploy/riot-anchor/renderer-seccomp.json`
- Create: `docs/operations/public-anchor-runbook.md`
- Create: `scripts/anchor/deployment-contract.sh`

**DoD:**

- Compose separates daemon/renderer network namespaces and mounts, uses unprivileged read-only renderer root, quota-limited temp volume, daemon-owned published tree, persistent DB, secrets, health/readiness, and bounded logs.
- Compose consumes only WU-026A's daemon and renderer images; it contains no pilot collector or
  recruitment service.
- Readiness probe verifies the security boundary rather than trusting declarative settings.
- Runbook covers bootstrap of at least three anchors/two operators/two failure domains, keys/rotation,
  backups/restore, clone lease, migration/rollback compatibility, graceful drain,
  capacity/removal telemetry, and incident handling.
- Example config contains no secret and every required production value/env path is documented.

- [ ] **RED:** Add `scripts/anchor/deployment-contract.sh` first and observe it fail on the absent Compose/config/runbook. It must reject shared network, writable root, missing quota/seccomp/readiness, unbounded log sink, or missing secret path.
- [ ] **GREEN:** Add deployment manifests/config/runbook and wire only the daemon/renderer images and probes.
- [ ] Run `sh scripts/anchor/image-contract.sh`,
  `docker compose -p riot-anchor-contract -f deploy/riot-anchor/compose.yaml config --quiet`,
  `docker compose -p riot-anchor-contract -f deploy/riot-anchor/compose.yaml up --build --detach --wait`,
  `sh scripts/anchor/deployment-contract.sh --live riot-anchor-contract`,
  `docker compose -p riot-anchor-contract -f deploy/riot-anchor/compose.yaml restart riot-anchor`,
  and `sh scripts/anchor/deployment-contract.sh --live --require-recovered riot-anchor-contract`.
  The live contract must enter both containers, prove the renderer's empty network namespace,
  unprivileged UID, seccomp/read-only-root/quota mounts and absence of daemon/published-tree access,
  prove daemon readiness is withheld until migration/recovery/publication complete, verify the
  persistent test database survives restart, and exercise graceful drain/recovery. It uses only the
  dedicated `riot-anchor-contract` local project and ephemeral test credentials/data. After evidence
  capture, run
  `docker compose -p riot-anchor-contract -f deploy/riot-anchor/compose.yaml down --volumes --remove-orphans`;
  failure or unavailable Docker blocks WU completion.
- [ ] Commit as `ops(anchor): add hardened deployment and recovery runbook`.

### WU-027: Deterministic network, failpoint, and load harness

**Spec:** Deterministic Test Harness; Performance Contract; Edge-Case Matrix.

**Files:**

- Create: `crates/riot-anchor/tests/support/mod.rs`
- Create: `crates/riot-anchor/tests/support/network.rs`
- Create: `crates/riot-anchor/tests/three_anchor_system.rs`
- Create: `crates/riot-anchor/tests/adversarial_load.rs`
- Modify: `scripts/anchor/deployment-contract.sh`

**DoD:**

- `TestAnchor`, `TestClient`, `TestAnchorNetwork`, FakeClock, duplex transport, deterministic gossip, failpoint repository, fake renderer, and metadata-only reports use production interfaces.
- Three independent anchors converge, survive partitions/loss/rotation/restart, preserve hosting/listing separation, and complete one-anchor failover.
- Adversarial HTTP/iroh/control/sync/removal/render/log loads remain within compiled permits/bytes/deadlines and do not starve reserved removal.
- Performance tests measure directory/descriptor/recovery/failover targets without nondeterministic sleeps.
- Deployment contract verifies container isolation/readiness configuration.
- This is a test-only integration unit. Every required production seam is created and tested by its
  owning WU before WU-027. If a production seam is missing, WU-027 stops and opens a separately
  scoped corrective WU against the owning files; it never edits production code outside this scope.
- The parameterized matrix is the active non-pilot subset only: authority/Meadowcap, tickets,
  hosting/listing/removal, sync, peer/gossip, directory/cursors, HTTPS/renderer/handoff,
  client/native lifecycle, overload/failpoints/recovery, and three-anchor failure/convergence.
  Pilot collector/recruitment/measurement/denominator/export/withdrawal cases and pilot key-store
  seams are explicitly excluded.

- [ ] **RED:** Add every active non-pilot Edge-Case Matrix row as a named parameterized case; the
  first run fails because the harness/orchestration is absent, never because a test is skipped.
- [ ] **GREEN:** Compose the already-landed clock, key-store, repository failpoint, control/sync
  transport, accepted-socket/TLS/HTTP limiter, gossip scheduler, renderer, work verifier, and profile
  runtime seams until every active case exercises a public service interface.
- [ ] Run `cargo test -p riot-anchor --test three_anchor_system --test adversarial_load`,
  `cargo test -p riot-anchor --test hosting_failpoints --test listing_failpoints --test checkpoint_recovery`,
  `sh scripts/anchor/image-contract.sh`, and `sh scripts/anchor/deployment-contract.sh`.
- [ ] **Human checkpoint E:** Present deterministic convergence trace, anchor-loss rehearsal, ingress/removal load charts, recovery timing, and deployment probe output.
- [ ] Commit as `test(anchor): add deterministic system and load harness`.

### WU-028: Integration wiring, CI, documentation, and final gates

**Spec:** Quality Gates; Definition of Done; SERVICE-INVENTORY contract.

**Files:**

- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/conference/build-native-core.sh`
- Modify: `crates/xtask/src/main.rs`
- Modify: `SERVICE-INVENTORY.md`
- Create: `docs/operations/public-anchor-release-checklist.md`

**DoD:**

- CI runs protocol/anchor/renderer tests, native feature-closure checks, image/deployment contracts,
  and coverage floors. Native device/build gates remain local because CI is Linux-only.
- Native packaging builds `riot-client-net` transitively for all checked-in Apple/Android targets and proves no server/renderer/anchor daemon dependency in any native resolved graph.
- `xtask validate-contracts` checks protocol CDDL/vectors, dependency closure, compiled limit registry, schema/version compatibility, and legacy pins.
- Release packaging refuses the visibly development-only bootstrap resource and requires an
  operator-supplied, package-signed bootstrap satisfying the compiled three-anchor/two-operator/
  two-failure-domain diversity floor. Pilot configuration is not a trunk release input.
- Inventory and release checklist describe every new service, schema, command, required secret, operator checkpoint, and rollback floor.
- Every active Definition of Done item maps to a passing automated test or the named deployment rehearsal evidence.

- [ ] **RED:** Extend contract/CI tests first and observe failures for absent checks/jobs.
- [ ] **GREEN:** Wire all active crates, binaries, scripts, packaging, inventory, and release evidence;
  assert no deferred pilot crate/service/config is introduced.
- [ ] Run every quality gate listed below with no skipped required suite and no coverage-floor reduction.
- [ ] Run a fresh final adversarial review across the complete design, plan, combined diff, generated bindings, deployment manifests, and test evidence.
- [ ] Commit as `chore(anchor): close integration and release gates`.

## API Contract

All request/query/header/body limits and overload rules are enforced before route-specific work. Public successful JSON responses carry canonical signed bytes as unpadded base64url plus typed projections; they never replace signed bytes with JSON authority.

| Method | Path | Success | Typed errors |
| --- | --- | --- | --- |
| GET | `/.well-known/riot-anchor.json` | 200, ≤16 KiB descriptor + limit profile JSON | 429/503 overloaded |
| GET | `/.well-known/riot-anchor-chain/v1?after=&cursor=` | 200, ≤16 descriptors/60 KiB, redirect-free | 400 invalid cursor; 409 floor unavailable; 429/503 |
| GET | `/api/v1/feed?after=&limit=` | 200 lossless signed feed page | 409 `checkpoint_required`; 400 invalid; 429/503 |
| GET | `/api/v1/feed/snapshot?checkpoint=&cursor=` | 200 immutable signed snapshot page | 400 cursor; 409 checkpoint unavailable; 429/503 |
| GET | `/api/v1/directory?q=&cursor=&limit=` | 200 ≤100 results/4 MiB | 400 query/cursor; 429/503 |
| GET | `/api/v1/directory/<64-hex-root>` | 200 verified item; 404 absent | 400 root; 409 conflict; 429/503 |
| GET | `/open/v2/<base64url-envelope>` | 200 safe projection/install continuation, including an expired-ticket explanation while retained content remains readable | 400 malformed; 404 unavailable projection; 429/503 |
| GET | `/c/<64-hex-root>/` | 200 current hosted projection | 400 root; 404 not hosted; 429/503 |
| GET | `/c/<64-hex-root>/e/<64-hex-entry>` | 200 exact admitted entry | 400 IDs; 404 exact entry absent; 429/503 |

Every GET route also supports HEAD with the identical status and headers and an empty body. POST, PUT, PATCH, DELETE, OPTIONS, CONNECT, TRACE, Range, HTTP/2 preface, WebSocket/Upgrade, and compressed requests are rejected with bounded fixed responses and connection close. Feed cursor below floor is the one typed 409 recovery path. An absent item is never used for overload, authority conflict, or invalid ticket.

The control and sync APIs are the exact CBOR operation/frame tables in the approved design; the plan does not create alternative HTTP mutation endpoints.

## Security Considerations

| Boundary | Untrusted input | Gate |
| --- | --- | --- |
| Cold link/handoff | ticket, destination, hints | Canonical bound → root signature/expiry/transport floor → safe dial → manifest match |
| Control ALPN | length-prefixed canonical CBOR | Connection/session permits → record bound → canonical re-encode/digest → idempotency → work/authority/quota |
| Sync ALPN | OpenNamespace and frames | Ticket/member/mode/token → exact FSM/cursors/digests/chunks → Meadowcap admission → private staging |
| Listing | Willow entry/cap/grant/ticket | Exact coordinate → signature/cap chain/grant/epoch → hosted manifest equality → reserved removal slot |
| Anchor peer | descriptor chain/hello/proof | Bounded chain → fresh current floors → TLS exporter transcript → mutual proof/config rule |
| Public HTTPS | socket/TLS/request | Pre-handshake permits/rates/timeouts → HTTP/1-only parser ceilings → handler/snapshot/CPU/response ceilings |
| Renderer | typed snapshot and output tree | No network/DB → bounded typed text → detached mount → no-follow copy/hash/fsync/read-only transfer |

Production secrets are loaded by file descriptor/path or injected key-store interface: anchor operator key, iroh endpoint key, namespace-token HMAC epochs, cursor HMAC epochs, TLS key/certificate, and deployment instance token/lease credentials. They never live in TOML examples, SQLite community tables, logs, web snapshots, handoff URLs, or crash reports.

## User Flows and Wireframes

```text
Community chooser
┌─────────────────────────────────────────┐
│ Your communities                        │
│ [Find communities online]               │
│ Local communities…                      │
└─────────────────────────────────────────┘

Explore
┌─────────────────────────────────────────┐
│ Find communities online      [Cancel]   │
│ Search [____________________]            │
│ Some public hosts didn't respond [Retry]│
│ Community card · topics · source cover  │
│ [Preview]                               │
└─────────────────────────────────────────┘

Follow consent
┌─────────────────────────────────────────┐
│ Public community · verified organizer   │
│ Stores complete site: 18 MiB / 64 MiB   │
│ Public hosts can observe connection data│
│ Requested: exact article title          │
│ [Cancel]                     [Save]      │
└─────────────────────────────────────────┘

Publish / public hosts
┌─────────────────────────────────────────┐
│ Recommended: 3 independent hosts        │
│ Host A  Hosted · reported through date  │
│ Host B  Listing waits until 14:32       │
│         [Cancel this host] [Check again]│
│ Host C  Source changed [Refresh source] │
│ [Manage hosts] [Cancel all waiting]     │
└─────────────────────────────────────────┘
```

Every loading/empty/partial/stale/overloaded/unreachable/refused/expired/busy/recovering/closed state has the exact user-facing distinction in the design. Technical IDs and proofs remain under Technical details. No state uses color alone.

## External Dependencies and Configuration

- iroh public discovery/relay uses the existing `N0` preset; Riot does not run a custom packet-relay network.
- Production TLS certificates are operator-provided files; tests use deterministic local certificates.
- SQLite is embedded/bundled; no external database exists.
- Renderer uses a local OCI runtime supporting isolated network namespaces, read-only roots, seccomp, and volume quotas.
- Optional reverse proxy/CDN must declare log retention/byte caps equivalent to the daemon or readiness fails.
- The repository can create and test `fixtures/anchor/bootstrap-development-v1.cbor` without external
  authority. A production release requires an operator-supplied, package-signed bootstrap containing
  at least three currently valid signed descriptors from at least two real operators and two
  operator-verified failure domains. Checkpoint A validates this input; if it is unavailable,
  debug/system implementation continues but WU-028's production release contract fails closed rather
  than shipping development hosts.
- No account, payment, invitation for hosting, Tor/Arti, arbitrary media renderer, global firehose, or canonical ranking service is introduced.

## Human Checkpoints

1. **A — protocol/transport closure:** cross-language vectors, feature graphs, ALPN traces, native cross-compilation, and acceptance of the operator-supplied production bootstrap resource.
2. **B — client storage migration:** migrations/backups, lock-order proof, explicit close behavior, regression suites.
3. **C — anchor persistence/security:** failpoint matrix, removal fairness/capacity, maximum-record emergency path.
4. **D — user journey:** web→native handoff, plural Explore, failover, publish/relist/source-change accessibility.
5. **E — operational rehearsal:** three-anchor convergence/loss, adversarial ingress, recovery performance, container isolation.

Each checkpoint pauses execution for explicit user approval.

## Final Verification

Run exactly:

```bash
cargo test --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo run -p xtask -- validate-contracts
(cd apps/gateway && python3 -m unittest discover -s tests)
sh scripts/conference/gateway-smoke.sh
sh scripts/anchor/bootstrap-resource-contract.sh
sh scripts/conference/build-native-core.sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-derived
xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -derivedDataPath build/ios-app-derived
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS -destination 'platform=macOS'
(cd apps/android && ./gradlew :app:testDebugUnitTest :app:assembleDebug :app:assembleDebugAndroidTest :app:connectedDebugAndroidTest)
scripts/web/coverage.sh
sh scripts/anchor/image-contract.sh
sh scripts/anchor/deployment-contract.sh
docker compose -p riot-anchor-contract -f deploy/riot-anchor/compose.yaml up --build --detach --wait
sh scripts/anchor/deployment-contract.sh --live riot-anchor-contract
docker compose -p riot-anchor-contract -f deploy/riot-anchor/compose.yaml restart riot-anchor
sh scripts/anchor/deployment-contract.sh --live --require-recovered riot-anchor-contract
docker compose -p riot-anchor-contract -f deploy/riot-anchor/compose.yaml down --volumes --remove-orphans
```

Then verify:

- no required test executed zero cases or was silently skipped;
- generated bindings are current;
- native resolved graphs exclude `riot-anchor`, `riot-anchor-renderer`, Hyper/rustls server adapters,
  renderer dependencies, and every deferred pilot crate/service name;
- `git diff --check` is clean;
- combined diff contains no unrelated user files;
- `SERVICE-INVENTORY.md`, migrations, protocol CDDL/vectors, deployment config, and release checklist agree;
- all five implementation checkpoints and the final comprehensive adversarial review are recorded.

---

## Coordinator Addendum — Phasing, Deferrals & Risk (2026-07-19)

This addendum originally layered guidance over the frozen sequence. The 2026-07-19 plan repair now
integrates that guidance into the active graph: pilot WU-024/025 are reserved and absent,
deployment is split into WU-026A/B, and M1–M4 plus production operations are the only active scope.
Purpose: ship value incrementally, defer what depends on the real world, and gate units that can
hurt the already-shipping app.

### Ship in milestones, not all-or-nothing
The plan currently lands as one block behind Checkpoint E. Group the WUs into shippable milestones so
capability arrives sooner and each milestone is independently valuable:

| Milestone | Work units | Ships |
| --- | --- | --- |
| **M1 — Protocol + transport** | WU-001–007 | Canonical wire, routed `sync/2`, ALPN router, cross-language vectors. No product yet; pure foundation, fully `cargo test`/vector-verifiable. |
| **M2 — Hosting MVP** | WU-008–012, WU-013–016 | A community can be *hosted on an anchor and followed by a client* — the first real capability. Client storage + core storage-ownership + anchor repo/commit/removal. |
| **M3 — Directory + web + handoff** | WU-011C, WU-017B–021 | Discovery (signed directory/search) + safe web projection + v2 web→app handoff. The "reach" half. |
| **M4 — Native UX** | WU-022–023 | Explore / Follow / Publish / host-management on iOS + Android + macOS. |
| **Production operations** | WU-026A–028 | Reproducible OCI images, isolated deployment, deterministic system/load proof, CI/inventory/release closure. |
| **Pilot (DEFERRED; not a milestone in this plan)** | Reserved former WU-024–025 | Privacy-preserving pilot. **Do not build until M2–M4 are real and a live pilot is actually scheduled.** |

M2, M3, M4 each deliver a usable product without the pilot. Treat M1→M4 as the trunk.

### Defer the pilot (reserved former WU-024/025)
The pilot recruitment ledger + collector depend on things that don't exist yet: **approved human
coordinators, a contact-handling policy, fixed-role credential batches, and operator-supplied
signed public-pilot fixtures** (`bootstrap-public-pilot-v1.cbor`, `pilot-config-public-v1.cbor`).
Building the infrastructure now freezes a large, privacy-sensitive surface long before it can run.
Carve it into its own spec→plan and revisit when a pilot is scheduled. Production operations
WU-026A–028 explicitly exclude pilot services, configs, tests, and release claims.

### Native "Final Verification" is LOCAL-ONLY — say so
CI is Linux-only. `xcodebuild test/build`, `gradlew …connectedDebugAndroidTest`, and
`build-native-core.sh` in the Final Verification block do **not** run in CI — the connected Android
tests need an emulator and iOS doesn't build in CI at all. These are `scripts/green.sh`-class LOCAL
gates. WU-028's CI additions must be the **Linux-runnable dependency-graph/feature-closure assertions**
(native graphs exclude server/renderer/pilot crates), not the device tests. Do not let the plan imply
a CI capability that isn't there — that is exactly how the two prior app-target breaks slipped past
green Linux CI.

### Gate the blast-radius units hardest (they touch the SHIPPING app)
Two clusters modify code the live iOS/Android apps already depend on; a regression here bricks a
shipping product, unlike the greenfield anchor crates:
- **WU-009 / WU-010A / WU-010B** — moves profile-storage ownership out of FFI into a core command
  port and rewrites the FFI persistence/import/sync/registry/close paths. Highest blast radius in the
  plan. Require a fresh adversarial review + full existing-persistence regression + the Checkpoint-B
  migration/backup proof BEFORE it merges to the trunk, and keep it revertible as one unit.
- **WU-015 / WU-016** — atomic composite hosting commit + crash-safe reserved owner removal. The
  hardest correctness; every failpoint must be wholly-absent-or-wholly-committed. Extra scrutiny at
  Checkpoint C; do not weaken a durability guarantee to make a test pass (already a Delivery Rule).

### Durability note
This plan file was UNTRACKED (working-tree only on `overnight/2026-07-18-anchor-protocol`, not on
`origin/main` or any commit) when execution was already ~3 WUs in. A `git clean`/reset would have
destroyed the governing plan. It is now committed. Keep the plan and its design spec tracked on the
branch that executes them.
