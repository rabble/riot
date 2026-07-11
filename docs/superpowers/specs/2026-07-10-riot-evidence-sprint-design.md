# Riot Phase 0A Public Kernel Evidence Sprint Design

Date: 2026-07-10 (last status update: 2026-07-11)
Status: Revision 5 REVISE findings are repaired — G0 and G1 both PASS after the second-review repair (see `docs/decisions/phase0a-wu0-report.md`, `docs/decisions/phase0a-wu1-report.md`). G2 (WU2, Task 5) is not yet a full PASS: transaction core, explicit selection, bounded plan lifecycle, three arbiter concurrency races, and both hard byte budgets (16 MiB store + 2 MiB preview) are proven; the hostile-input/log-safety corpus and one untested race (session-close vs. a concurrent plan/preview action) remain — see `docs/decisions/phase0a-wu2b-report.md` for the exact list. WU3 groundwork (UniFFI binding generation, the mobile API surface) has already started in parallel per `COLLABORATION.md`, ahead of a formal full-G2 PASS; this diverges from this report's own "G2 FAIL stops native work" / the WU2B report's more conservative "must not begin until full G2 passes" — flagged as an open sequencing question, not resolved by this document. The platform pivot (shared Rust core, native iOS + Android shells) remains confirmed. A non-gating gateway/reader demo track has landed in parallel (see the dual-mode spec's build phasing; conference gateway shipped at commit `976e965`).

## Purpose

Run a hard-capped 16 aggregate agent-hour sprint that answers one decision:

> Can a small, release-shaped Rust public-data kernel encode, authorize, inspect, and atomically import a signed Riot alert through generated bindings on native iOS and Android runtimes?

The decisive proof is a two-way runtime artifact handoff. An iOS Simulator test creates a `.riot-evidence` bundle through the Rust FFI and an Android emulator imports it; the Android emulator then creates a different bundle and an iOS Simulator test imports it. The shell only carries bytes between runtimes. It does not decode or rewrite them.

This is Phase 0A, not the whole Phase 0 research program. Private groups and declassification have separate threat models and dependency closures. They become Phase 0B and Phase 0C only after separately budgeted designs pass their own review gates.

Research basis: `docs/research/2026-07-10-dual-mode-research-addendum.md` and `docs/research/2026-07-10-willow-implementation-audit.md`.

## Evidence Boundary

Phase 0A proves structural prerequisites for three user outcomes:

1. A public carrier can inspect signer, namespace, size, freshness, object count, validity, and trust status before importing an artifact.
2. A field publisher can create a signed alert on one native runtime and have the other native runtime verify and import the exact bytes without a server.
3. A field recipient can tell whether an historically accepted alert is currently live or has been pruned, while seeing that valid authorship/capability is neither a trust label nor a truth claim.

The sprint implements one durable object kind, `alert`. The ten-kind product vocabulary, coordination flow, private groups, bridge, live transport, trust directories, polished UI, durable storage, and physical-device usability are deferred. A passing signature is not a truth or trust claim; an unknown signer remains visibly unknown.

Each gate ends in exactly one status:

- **PASS:** every required command ran in the frozen environment and produced the specified artifacts and assertions;
- **FAIL:** a reproducible assertion disproved the claim;
- **INCONCLUSIVE:** the evidence could not run or finish because of the time cap, toolchain, dependency, or environment.

Only PASS permits downstream GO. FAIL and INCONCLUSIVE require a revised design or another explicitly budgeted sprint.

The user risk controlled by each gate is explicit: G0 prevents incompatible hashes and untestable runtime behavior; G1 proves portable canonical authorship/authority; G2 prevents partial, order-dependent, or misleading imports and provenance; G3 proves those facts survive generated bindings and real native runtimes.

## Non-Goals

Phase 0A does not deliver or claim:

- production-safe private groups, OpenMLS integration, private envelopes, invitations, or panic wipe;
- declassification or public-to-private clipping;
- crash-safe persistence, streaming import, current Willow Drop Format compatibility, or WTP;
- radio, local-network, gateway, rendezvous, or server transport;
- production Keychain/Keystore key persistence;
- more than the `alert` schema;
- polished SwiftUI/Compose import UX, accessibility validation, or field usability.

## Phase 0A Threat Model

Protected assets are author-key secrecy during the process lifetime, signed-byte integrity, capability enforcement, import-store integrity, bounded availability under hostile input, and the separation of cryptographic facts from trust/truth claims. Adversaries include an untrusted carrier that mutates, duplicates, reorders, truncates, or withholds artifacts; a malicious artifact author; malformed-input resource-exhaustion attempts; and accidental leakage through FFI errors, logs, fixtures, or packaged debug providers.

The evidence trusts the pinned Rust dependencies, OS randomness, generated UniFFI bindings, and uncompromised simulator/emulator processes. Public bundle metadata and content are intentionally plaintext. A valid authorized author may still lie, previously exported bytes cannot be revoked, and process/device compromise is outside this sprint. These are explicit residual risks, not PASS claims.

## Frozen Environment and Dependency Closure

WU0 records the actual versions and lock hashes in `fixtures/manifest.json`. A substitution changes the evidence identity and must be reported.

### Rust

| Dependency | Exact policy |
| --- | --- |
| Rust | `1.95.0` in `rust-toolchain.toml` |
| `willow25` | `=0.6.0-alpha.3`; `default-features = false`, `std` only; Drop Format disabled |
| `bab_rs` | `=0.8.1`; corrected WILLIAM3; direct pin forces the fixed patch in the unified graph |
| `uniffi` | `=0.32.0`; proc-macro scaffolding and generated Swift/Kotlin bindings |
| `camino` | `=1.2.4`; xtask binding-generation paths only; excluded from the release runtime graph |
| `minicbor` | `=2.2.2`; fixed-key, definite-length encoder |
| `cddl-cat` | `=0.7.1`; test/dev validation only |
| `sha2` | `=0.10.9`; bundle/object digests, canonical entry IDs, and proof-bound evidence digests |
| `hifitime` | `=4.3.0`; UTC/TAI conversion from one captured system instant; already in the Willow graph but directly pinned for the clock adapter |
| `ed25519-dalek` | `=2.2.0`; evidence public-author signatures |
| `rand_core` | `=0.6.4` with `getrandom`; OS-backed ephemeral evidence keys |

All workspace dependencies use exact versions and a committed `Cargo.lock`. Gradle dependency locking is enabled and the Android SDK/NDK package revisions are recorded. The release-shaped graph must not enable `willow25/drop_format` or contain OpenMLS, group-lab code, deterministic production randomness, or cryptographic debug features. The alpha pin is evidence-specific: stable 0.5.0 resolves a `bab_rs` version that upstream documents as computing incorrect WILLIAM3 digests.

The evidence `riot-ffi` release artifacts use `panic = "unwind"` so every exported entrypoint can catch a panic, quarantine the session, and return `INTERNAL_ERROR`; `panic = "abort"` is forbidden in the packaged Phase 0A library because it would make that gate untestable. No unwind may cross UniFFI. A future production profile may choose abort only after narrowing the recovery claim and defining host-level process recovery.

### Apple

- Xcode 26.2 and Swift 6.2.3.
- iOS deployment floor 17.0.
- Rust triples `aarch64-apple-ios` and `aarch64-apple-ios-sim`.
- Runtime gate on an arm64 iOS Simulator; unsigned device triple must compile.
- Native XCTest host only; no SwiftUI screen in Phase 0A.

### Android

- Android Gradle Plugin 9.0.1, its built-in Kotlin 2.2.10, Gradle 9.1.0, and JDK 17.
- Android SDK/Build Tools 36.0.0 and NDK 28.2.13676358.
- `minSdk 26`, `compileSdk 36`, `targetSdk 36`.
- Rust/native ABIs `arm64-v8a` and `x86_64`.
- Command-line tools archive `commandlinetools-mac-14742923_latest.zip`; Android Emulator 36.6.11; Platform-Tools 37.0.0.
- Runtime gate on the API 36 `system-images;android-36;google_apis;arm64-v8a` image on this Apple Silicon host, using the `aarch64-linux-android` Rust library. WU0 records the installed image revision and `source.properties` hash before work proceeds. The x86_64 library is compile-only evidence.
- Native Android instrumentation host only; no Compose screen in Phase 0A.

Android tool pins were checked against the official [Android Studio downloads](https://developer.android.com/studio), [Emulator release notes](https://developer.android.com/studio/releases/emulator), and [Platform-Tools release notes](https://developer.android.com/tools/releases/platform-tools) on 2026-07-10.

WU0 verified the installed Android environment: API 36 Google APIs arm64 image revision 7, Emulator 36.6.11, Platform-Tools 37.0.0, Build-Tools 36.0.0, and NDK 28.2.13676358. Any missing or substituted component makes Android-dependent gates INCONCLUSIVE; the sprint never substitutes a JVM-only test.

### Release-shaped graph

```text
riot-core ──> riot-ffi ──> XCTest / Android instrumentation
     └─────> riot-conformance fixtures and reports
```

Planned source structure:

```text
Cargo.toml
.cargo/config.toml  # `xtask = "run --package xtask --"`
rust-toolchain.toml
crates/
  riot-core/src/{model,willow,import}/
  riot-ffi/src/
  riot-conformance/src/
  xtask/src/
schemas/alert.cddl
fixtures/manifest.json
fixtures/objects/
fixtures/willow/
fixtures/imports/
apps/ios/RiotEvidence/
apps/android/
scripts/phase0a/
docs/decisions/
```

## Signed Alert and Willow Authority

### Alert payload

The deterministic signed payload contains:

```text
schema = "org.riot.alert/1"
object_id
revision_id
created_at
valid_from?
expires_at
language
urgency
severity
certainty
headline
description
affected_area_claim?
source_claims[1..]
ai_assisted
```

`expires_at` must be later than `created_at`; `valid_from` remains optional. Urgency, severity, and certainty are closed enums. Phase 0A fixtures require at least one non-empty source claim. These operational constraints are validated before signing and again after decoding.

Signer, namespace, capability, Willow timestamp, and payload digest belong to the Willow entry. Import route, first-seen time, signer-trust label, and receipt time are local facts and never enter the author payload.

CBOR uses definite lengths, integer field keys in ascending order, shortest integer encodings, no floating point, no duplicate keys, and strict rejection of unknown envelope keys. A lossless JSON projection exists only for fixture debugging.

### Authority fixture

The core Willow fixture contains one ephemeral communal author. `RiotSession.open` generates a communal namespace ID, discards the privilege-less communal namespace secret, generates the author subspace keypair, and creates the zero-delegation communal write capability. `AuthorIdentity` exposes the complete 32-byte namespace ID, 32-byte subspace ID, `Communal` namespace kind, and a signing-key ID equal to the public subspace ID. The author subspace secret never crosses FFI.

One session-owned `ClockSource` read produces a `ClockSnapshot` with Willow's `Timestamp` in microseconds TAI since J2000 for join recency and UTC Unix seconds for signed `created_at`; the draft's validity times are checked against that instant. The Phase 0A alert path is `[b"objects", b"alert", object_id_16_bytes, revision_id_16_bytes]`; binary IDs are never truncated. One denial fixture creates a second subspace inside the same communal namespace and proves the first subspace's capability cannot authorize it; creating a second namespace does not satisfy this test.

Owned publication namespaces, delegated curation, feature/correction annotations, and lens behavior remain approved product architecture but are stretch evidence outside G1–G3.

## Evidence Bundle Codec

`RiotEvidenceBundleV1` is a deliberately non-interoperable development codec:

- visible magic `RIOTE1` and version 1;
- deterministic CBOR containing, per item, canonical Willow `Entry` bytes, canonical Meadowcap `WriteCapability` bytes, the exact 64-byte subspace signature, and payload bytes;
- codec ID `org.riot.evidence-bundle/1` and extension `.riot-evidence`;
- no compression;
- at most 64 entries and 8 MiB total bytes.

It is not `.snk`, the current Willow Drop Format, or a WTP stream. No later plan may imply compatibility without authoritative vectors and a separate conformance gate.

The outer CBOR never redefines Willow or Meadowcap fields. Decoding requires `Entry::decode_canonic` and `WriteCapability::decode_canonic` with no trailing bytes, reconstructs `AuthorisationToken::new`, verifies exact payload length and corrected WILLIAM3 digest, and only then checks Meadowcap authorisation and the Riot schema.

The digest vocabulary is fixed:

- `bundle_digest`: SHA-256 of the complete `.riot-evidence` bytes;
- `entry_id`: SHA-256 of domain `riot/willow-entry-id/v1`, length-delimited canonical Entry bytes; this identifies the Willow value independent of its proof;
- `evidence_digest`: SHA-256 of domain `riot/evidence-digest/v1`, length-delimited canonical entry bytes, length-delimited canonical capability bytes, and signature bytes; this identifies the authenticated proof carried by this bundle;
- `payload_digest`: corrected WILLIAM3 committed by the Willow entry over the signed alert bytes;
- `object_digest`: SHA-256 of the deterministic alert payload bytes.

Phase 0A accepts only a communal namespace, a zero-delegation communal write capability whose namespace matches the entry and whose receiver/subspace is the entry author, plus a valid receiver signature. Valid owned, delegated, or otherwise alternate capability proofs are `UNSUPPORTED_CAPABILITY` and ineligible even if generic Meadowcap verification succeeds; those policies remain stretch scope. This narrow predicate makes one supported proof shape per canonical Entry while `entry_id` and `evidence_digest` preserve the architectural distinction.

## Stable FFI and Ownership Model

Phase 0A uses synchronous inspection because the maximum input is 8 MiB. It makes no cancellation or progress-reporting claim.

```text
RiotSession.open(CoreConfig) -> RiotSession
RiotSession.authorIdentity() -> AuthorIdentity
RiotSession.createEvidenceStore() -> EvidenceStore
RiotSession.encodeAlert(AlertDraft) -> SignedWillowEntry
RiotSession.createBundle([SignedWillowEntry]) -> EncodedBundle
RiotSession.inspectBundle(EvidenceStore, bytes, ImportContext) -> InspectBundleResult
RiotSession.deriveProvenance(EvidenceStore, EntryId, TrustContext) -> ProvenanceDisplay
ImportPreview.plan(ImportSelection) -> ImportPlan
ImportPlan.effects() -> PlannedImport
ImportPlan.commit() -> ImportCommitResult
ImportPreview.reject() -> RejectionResult
close() -> CloseResult on RiotSession, EvidenceStore, ImportPreview, and ImportPlan
```

`RiotSession.open` generates one ephemeral Ed25519 evidence author from OS randomness and keeps the private key inside Rust for the session lifetime. Entropy failure returns `ENTROPY_UNAVAILABLE` and exposes no partial session. The session also owns a `ClockSource`. Production uses a fallible system source plus pinned `hifitime`; conformance and `cfg(test)` may inject a deterministic/failing source that is unreachable from release FFI. One `ClockSource::snapshot()` captures one system instant and returns UTC Unix seconds, TAI/J2000 microseconds, and uncertainty derived from it. System-read failure, pre-epoch/range failure, or UTC/TAI conversion failure maps to stable `CLOCK_UNAVAILABLE`. `encodeAlert` calls it before allocating/signing, sets signed `created_at` and Willow timestamp from that snapshot, and validates the draft's absolute `valid_from`/`expires_at`; no partial entry or signer mutation remains on failure. Inspection captures its receipt clock before retaining bytes and leaves no preview/state on failure. Callers cannot supply an independent Willow time. `ImportContext` therefore carries route and trust context, not a caller-authored clock. Key persistence and recovery are deliberately absent.

`ed25519-dalek` uses `default-features = false` with only `std`, `fast`, `zeroize`, and `rand_core`. The signing key is not serializable, printable, cloneable through FFI, or returned by any API; it is zeroized on explicit close and drop. Deterministic clock, ID, and signer implementations live only in `riot-conformance` or `cfg(test)` and cannot be selected by `riot-ffi` release features.

### States and transitions

| Object | States | Legal transitions |
| --- | --- | --- |
| `RiotSession` | `Open`, `Failed`, `Closed` | `open → Open`; boundary panic: `Open → Failed`; `close: Open|Failed → Closed`; repeated close returns `AlreadyClosed` |
| `EvidenceStore` | `Open`, `Closed` | created by one open session; `close: Open → Closed`; parent close closes it; repeated close returns `AlreadyClosed` |
| `ImportPreview` | `Open`, `Committed`, `Rejected`, `Closed` | plan commit: `Open → Committed`; `reject: Open → Rejected`; `close: Open → Closed`; terminal close returns `AlreadyClosed` |
| `ImportPlan` | `Open`, `Committed`, `Superseded`, `ParentConsumed`, `Closed` | created by one preview; a new plan supersedes the prior plan; `commit: Open → Committed`; preview reject/other-plan commit → `ParentConsumed`; explicit/parent close → `Closed` |

Rules:

- every object carries an unguessable session ID and is valid only with its creating session;
- one session-owned `Mutex<SessionState>` contains session, store, preview, generation, index, receipt, and signer state; child objects contain only their IDs plus an `Arc` to this arbiter and have no independent state locks;
- every FFI call acquires that one arbiter before admission checks or mutation, so close/commit/reject have one lock order and one linearization point; the first terminal preview action wins, with the exact plan/preview terminal code determined by the precedence and lifecycle matrix below;
- non-close admission is exact: owning session `Failed` → `SESSION_FAILED`; owning session closed → `OBJECT_CLOSED`; immutable owner ID on a passed store/preview/plan differs → `WRONG_SESSION` without locking the foreign arbiter; locally owned store/handle explicitly or parent-closed → `OBJECT_CLOSED`; superseded plan → `PLAN_SUPERSEDED`; already committed plan → `PLAN_CONSUMED`; rejected/committed preview reached through any other child → `PREVIEW_CONSUMED`; changed store generation → `STALE_PREVIEW`; then validation/limit error. Close remains available on a failed session and returns only `Closed | AlreadyClosed`;
- closing a store marks its preview closed in the same critical section; closing a session marks previews and stores closed before zeroizing its signer;
- any non-close call on a closed parent returns `OBJECT_CLOSED`;
- process restart invalidates all objects;
- `CloseResult` is `Closed | AlreadyClosed` and carries no sensitive detail;
- `RejectionResult` is non-durable and contains only preview ID and `Rejected`; it creates no store receipt;
- FFI failures use stable codes and sanitized developer detail containing no payload, author private data, key material, or untrusted bytes;
- every exported FFI entrypoint catches Rust panic at the boundary, quarantines the session as `Failed`, closes child objects, zeroizes its signer, returns sanitized `INTERNAL_ERROR`, and allows no unwind across UniFFI; later non-close calls return `SESSION_FAILED` despite child terminal flags because that code has precedence. A panic during `open` returns `INTERNAL_ERROR` with no handle; a normal parent close produces `OBJECT_CLOSED` on child non-close calls.

`ImportSelection` contains the preview ID and unique selected preview-entry IDs; their canonical Willow `entry_id` values must also be unique. The preview ID and every selected ID must belong to the open preview. `plan` evaluates the complete selection against the preview's bound store generation and retains the full selection, base generation, and ordered effects under the arbiter. It returns an opaque `ImportPlan` handle, not a host-constructible value; `effects()` returns a copy of `WouldApply { entry_id, pruned_entry_ids[] }`, `WouldBeDominated { entry_id, dominating_entry_ids[] }`, or `AlreadyPresent { entry_id, insertion_receipt_id }`. One plan may be live per preview and at most 64 may be issued over its lifetime; the 65th returns `SESSION_LIMIT`. A new plan atomically supersedes the old one. The arbiter retains a fixed 256-byte-charged tombstone for each issued plan until preview close, preserving exact terminal reasons within a bounded 16 KiB maximum. `commit` reads only the retained issued plan. Under the arbiter it either reproduces those exact effects and swaps state, or returns `STALE_PREVIEW` before duplicate detection; it never trusts substituted host fields or silently commits a different effect. Plan state, tombstones, and returned effect bytes count against the preview-output ceiling. Phase 0A permits one open store and one open preview per session; attempts beyond either bound return `SESSION_LIMIT` before retaining bytes.

Explicit `ImportPlan.close` leaves the preview open and permits a replacement plan; later `effects`/`commit` on that handle return `OBJECT_CLOSED`. A superseded plan returns `PLAN_SUPERSEDED`. Repeating commit on the winning plan returns `PLAN_CONSUMED`. A plan observed after its preview was rejected returns `PREVIEW_CONSUMED`; parent store/session close returns `OBJECT_CLOSED`. These cases are fixtures in core, Swift, and Kotlin.

`TrustContext` is a value containing at most 64 complete trusted public signer IDs. `ImportContext` contains the local route and one `TrustContext`; receipt time comes from the session clock. Trust cannot weaken signature, capability, schema, or size policy. A signer absent from the exact set is `UnknownTrust`.

## Import Transaction, Receipt, and Provenance

### In-memory evidence store

`EvidenceStore` proves logical atomicity, not crash durability. It is a bounded Riot container, not one Willow store: it contains a random store ID, monotonic generation, a map from namespace ID to namespace-local Willow join state, a seen-entry index, and immutable import receipts. Each namespace-local live view contains only authorised entries for that namespace and obeys Willow prefix pruning and recency.

Inspection retains immutable input bytes and binds the preview to:

- codec ID/version and `bundle_digest`;
- destination store ID and base generation;
- import route and local clock snapshot;
- fixed ceilings;
- per-entry preview ID, original bytes/digests, status, eligibility, warnings, and structured diagnostics.

Commit builds one bounded copy-on-write next snapshot under the session arbiter. It validates the whole selection, partitions unseen entries by namespace, and computes one order-independent final join of each namespace's pre-commit live view with its complete selected batch: newer prefixes prune older descendants; equal subspace/path/timestamp ties retain the greatest WILLIAM3 digest, then greatest payload length. It then derives dispositions by `entry_id` from the pre-state and final state. A final-live new entry is `AppliedAtCommit`; its `pruned_entry_ids` contains only entries removed from the pre-commit live view that this winner directly dominates, never same-batch candidates that were not previously committed. A new entry absent from the final live view is `DominatedAtCommit`; its dominators are named from the final live view. Stable `EntryId` is the domain-separated hash of canonical Entry bytes, so identity and dispositions do not depend on proof or bundle order; receipt rows retain original bundle order for presentation. One pointer swap installs the final live views, seen records, first-receipt references, one generation change when at least one entry is new, and the receipt. The old snapshot remains authoritative until that swap; a fault or `STORE_FULL` before it leaves all observable state unchanged.

A preview whose store generation changed returns `STALE_PREVIEW` before duplicate detection. Deduplication uses `entry_id`, with preview entries kept in original bundle order. If a bundle repeats an `entry_id`, inspection rejects the artifact with bundle-scoped `DUPLICATE_ENTRY_ID`; multiplicity never reaches selection or receipt construction. `ImportCommitResult` is `Committed(ImportReceipt) | NoChanges(DuplicateResult)`. `DuplicateResult` contains bundle digest, store ID, unchanged generation, and ordered `(preview_entry_id, entry_id, evidence_digest, first_seen_time, insertion_receipt_id)` rows. Every newly accepted entry has that stable `entry_id` whether it is live or dominated on arrival, and the index permanently points it to its first receipt. A duplicate-only reinspection, plan, and commit returns `NoChanges` and creates no state; calling commit twice on the same consumed plan instead returns `PLAN_CONSUMED`. A mixed new/duplicate import returns `Committed`, increments generation once, and records every disposition.

`willow25::storage::MemoryStore` 0.6.0-alpha.3 is a test-only conformance oracle for live-view permutations. It is not the FFI store because it is `Rc`-based, lacks Riot's hard ceilings and receipt model, and is not the session arbiter's transactional state.

### Preview policy

Each preview entry exposes schema status, full author and namespace IDs, signature status, capability status, signer trust status, encoded size, digests, eligibility, duplicate state, and structured non-sensitive diagnostics. Join effect is selection-dependent and is therefore exposed by `plan`, not guessed independently per entry.

`InspectBundleResult = Preview(ImportPreview) | Rejected(BundleRejection)` is ordinary typed inspection data across UniFFI, not a generic exception. `BundleRejection` contains exactly one structured diagnostic for a fatal artifact rejection. `BundleDiagnostic` has a stable `code`, `scope = Bundle | Item(item_index)`, and `component = OuterFrame | Entry | Capability | Signature | Payload | Authorization | Schema`; optional developer detail is fixed trusted text. Outer magic/version/codec failure, non-canonical or trailing outer CBOR, cumulative bound failure, duplicate entry ID, or an item frame that cannot be isolated returns `Rejected` and creates no preview. Operational/session failures still use typed FFI errors. Once a bounded canonical outer item is isolated, non-canonical/trailing Entry or capability bytes, wrong signature length, payload length or WILLIAM3 mismatch, authorization failure, and schema failure produce an ineligible item with the exact component/code; they do not hide valid sibling items. Authorization uses Willow's checked possibly-authorised-to-authorised conversion; unchecked conversion is forbidden. No diagnostic includes payload text, untrusted bytes, secret data, or attacker-controlled formatting.

If one artifact violates several fatal outer rules, the single rejection code is deterministic: `ARTIFACT_TOO_LARGE`, `BAD_MAGIC`, `OUTER_MALFORMED_OR_NONCANONICAL` (including trailing bytes), `UNSUPPORTED_VERSION_OR_CODEC`, cumulative `LIMIT_EXCEEDED` in decode encounter order, then `DUPLICATE_ENTRY_ID`. Tests combine violations to freeze this precedence.

- invalid signature or invalid capability: hard-ineligible;
- unknown capability: hard-ineligible in Phase 0A;
- unknown signer with valid signature and capability: eligible but labelled `UnknownTrust`;
- any schema other than `org.riot.alert/1`: ineligible as `UNSUPPORTED_SCHEMA` in core evidence;
- expiry remains visible, but expired/not-yet-valid/uncertain-clock policy variants are stretch evidence and do not determine G2.

Selection must be non-empty, contain no duplicates, and reference only eligible entries in this preview.

### Immutable receipt facts

`ImportReceipt` contains codec/version, bundle digest, store ID, before/after generation, receipt ID, route, local receipt time, and one immutable `ImportEntryDisposition` per selected preview entry:

```text
preview_entry_id
entry_id
evidence_digest
object_digest
disposition =
  AppliedAtCommit { entry_id, pruned_entry_ids[] }
  | DominatedAtCommit { entry_id, dominating_entry_ids[] }
  | AlreadyPresent { entry_id, insertion_receipt_id }
first_seen_time
```

`AppliedAtCommit` means the entry was present in the live Willow view produced by that commit. `DominatedAtCommit` means the valid entry was accepted into local seen/receipt history but absent from that resulting live view. `AlreadyPresent` means this exact entry ID was previously accepted. A newly accepted dominated entry changes store history and therefore increments the generation; a duplicate-only retry does not. Receipt wording is deliberately temporal: an entry applied in an earlier receipt can be pruned by a later commit.

Receipts do not contain mutable trust or presentation state.

### Derived provenance display

`deriveProvenance(store, entryId, trustContext)` returns the Phase 0A presentation model from immutable entry and receipt facts plus the caller's current bounded signer-trust set:

1. authorship: complete author subspace, collective namespace, delegated-signer status, signed creation time;
2. cryptography: payload digest, signature status, capability status, with no truth claim;
3. author claims: source and affected-area claims explicitly labelled as author claims;
4. local receipt: bundle digest, route, first-seen and receipt times, immutable receipt disposition;
5. current Willow status: `Live | NotLive { reason = DominatedOnArrival | PrunedLater, dominating_entry_ids[] }`, derived from the present namespace view and first receipt for every historically accepted entry;
6. reader state: signer trust and expiry.

`deriveProvenance` supports every historically accepted stable entry ID, including entries dominated on arrival or pruned later. The seen record retains the bounded immutable entry/authentication facts needed to derive it. For preview-only entries, `PreviewEntry.provenance` uses the inspection-time trust snapshot and the same labelled structure, but local receipt fields are `NotCommitted` and current Willow status is supplied only by the selection's `PlannedImport`. Receipts remain trust-free. Core and native assertions prove that preview trust stays at its captured value while a post-import derivation changes only when its explicit `TrustContext` changes. Curation, corrections/disputes, clock uncertainty, and broader mutable lens snapshots are stretch evidence. Native tests compare the core preview, planned effect, receipt, current-status, and provenance fact records.

## Limits and Fixture Matrix

Callers may lower but never raise `CoreConfig` ceilings.

| Resource | Ceiling |
| --- | --- |
| artifact bytes | 8 MiB |
| entries per bundle / preview | 64 |
| canonical Entry bytes per item | 4 KiB |
| canonical capability bytes per item | 64 KiB; also charged to the 2 MiB bundle authorization total |
| signature bytes per item | exactly 64 |
| encoded payload bytes per item | 1 MiB |
| CBOR nesting | 16 |
| map entries | 128 |
| total decoded CBOR nodes | 16,384 |
| other text/byte string | 64 KiB; payload uses its dedicated 1 MiB limit |
| path components | 64 |
| path component bytes | 256 |
| total path bytes | 2,048 |
| authorization chain depth | 16 |
| authorization bytes | 64 KiB per entry; 2 MiB per bundle |
| warning/status records | 64; one per preview entry |
| expansion ratio | 1:1; compression forbidden |
| store entries / index records | 1,024 each |
| total retained-store charge | 16 MiB across entry, payload, capability, signature, index, receipt, digest-reference, namespace-map, and collection allocations |
| namespace-local live views | 64 |
| durable receipts | 256 |
| pruned/dominating entry-ID references per commit | 1,024 |
| open stores / previews per session | 1 / 1 |
| plans issued per preview | 64 total; one live; 256 charged bytes per retained tombstone |
| retained preview input / output | 8 MiB / 2 MiB |
| next transaction snapshot charge | 16 MiB, in addition to the bounded current store and retained input |
| local inspection target | 2 seconds for the 8 MiB hostile fixture; a miss is measured FAIL/INCONCLUSIVE, never a security pass |

Normatively, a live preview may issue at most 64 plans and each later preview starts a fresh 64-plan budget; while the parent remains live, superseded, closed, and committed child plans retain their exact terminal results, but replacing the preview consumes it and every child so all later old-handle actions return `PREVIEW_CONSUMED` ahead of any child terminal code, then releases that preview's tombstones, bounding retained records at 64 and imposing no session-wide plan cap.

The retained-store charge counts every owned byte-buffer capacity plus conservative fixed charges of 512 bytes per accepted entry/index record, 256 bytes per namespace view, 256 bytes per receipt and receipt row, and 32 bytes per digest reference. Store collections allocate only after their count and byte charges fit; implementations may be more conservative but never undercharge these categories. All length/count/charge arithmetic is checked before allocation. Swift and Kotlin use capped reads of at most 8 MiB + 1 byte rather than trusting pre-read file metadata; Rust independently rechecks the byte ceiling. Every exact-boundary and one-over fixture, including namespace-view and digest-reference limits, must return its expected stable result without partial allocation or store mutation. `STORE_FULL` is decided while staging and before the pointer swap.

### Core gate fixtures

| Fixture | Preview/action expectation | Commit/store expectation |
| --- | --- | --- |
| valid known alert | eligible; planned `WouldApply` | `AppliedAtCommit` with per-entry receipt |
| unknown signer, valid signature/capability | eligible; `UnknownTrust`; planned `WouldApply` | `AppliedAtCommit`; trust remains unknown |
| invalid signature | hard-ineligible, distinct code | unchanged |
| invalid capability | hard-ineligible, distinct code | unchanged |
| malformed/oversized/limit edge | rejected before preview or exact boundary accepted | unchanged on reject |
| duplicate-only | planned `AlreadyPresent` | `NoChanges`; no generation or new durable receipt |
| newer prefix / older descendants | planned `WouldApply`; pruned entry IDs visible before commit | `AppliedAtCommit`; descendants pruned and named in receipt |
| candidate dominated by newer prefix | planned `WouldBeDominated`; dominators visible before commit | `DominatedAtCommit`; stable entry ID, seen index, and receipt committed; live view unchanged |
| equal coordinate tie | eligible | greatest WILLIAM3 digest, then greatest payload length wins |
| distinct namespace/subspace | eligible | no cross-namespace or cross-subspace pruning |
| batch/join permutations | same planned effects keyed by entry ID; rows may preserve input order | commutative, associative, idempotent live view and disposition facts matching alpha.3 `MemoryStore`; pruned lists name pre-state entries only |
| store-full / injected pre-swap failure | stable failure code | exact before-state retained |
| commit versus reject race | one terminal winner | at most one swap/receipt |
| close versus commit race | one terminal winner | state matches winning action |
| session close versus store/preview action | exact error precedence; finishes within 2 seconds | no deadlock; state matches winning action |

The malformed-input tests capture Rust, Swift, Kotlin, and instrumentation logs and assert that no untrusted substring, payload, key, or private signer state appears. A panic fixture must return `INTERNAL_ERROR`; an unwind, process abort, or poisoned usable session fails G2.

### Stretch evidence, excluded from G1–G3

If and only if all core gates pass with budget remaining, agents may add: empty/mixed bundles, unknown schema and explicit opaque consent, unknown capability, expired/not-yet-valid/uncertain-clock states, stale or foreign selections, additional lifecycle races, owned publication and curation authority, feature/correction annotations, and mutable provenance lenses. Missing or failing stretch evidence cannot change a core PASS to FAIL and cannot be cited as implemented product behavior.

## Runtime Handoff Protocol

`scripts/phase0a/cross-runtime-handoff.sh` is the sole G3 orchestrator and performs these steps:

1. Build/package the Rust library for `aarch64-apple-ios-sim` and `aarch64-linux-android`, generate bindings, install the XCTest host on the pinned simulator, and install the test APKs on the pinned arm64 emulator.
2. Run `RiotEvidenceTests/IOSCreatesBundle`. It calls the Swift binding and writes the bundle plus a producer-facts JSON file inside the app's `Documents` directory.
3. Resolve that directory with `xcrun simctl get_app_container "$IOS_UDID" org.riot.evidence data`, copy both files to `build/handoff`, and hash the bundle.
4. Use binary-safe ingress: `adb push build/handoff/ios-generated.riot-evidence /data/local/tmp/ios-generated.riot-evidence`, then `adb shell run-as org.riot.evidence cp /data/local/tmp/ios-generated.riot-evidence files/ios-generated.riot-evidence`.
5. Run one ordered instrumentation scenario: `./gradlew :app:connectedDebugAndroidTest -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.CrossRuntimeHandoffTest`. It imports and commits the iOS artifact, then creates a distinct Android-signed artifact and producer/consumer fact files in the target app's private files directory.
6. Pull the Android artifact and fact files using binary-safe `adb exec-out run-as org.riot.evidence cat files/FILE > build/handoff/FILE`; hash the artifact and copy it into the iOS app's `Documents` container.
7. Run `RiotEvidenceTests/IOSImportsAndroidBundle`, then copy its consumer facts from the resolved container.
8. Compare fact JSON and write `build/evidence/g3-runtime-handoff.json`. The shell may parse fact JSON but never the `.riot-evidence` bytes.

For each leg, facts use schema `org.riot.handoff-facts/1` and divide `preview`, `plan`, `commit`, and `post_commit_provenance` sections. Producer facts include runtime/tool versions, complete author/namespace/subspace IDs, payload fields, Willow timestamp, corrected WILLIAM3 payload digest, canonical `entry_id`, proof-bound `evidence_digest`, bundle/object digests, and artifact byte count. The consumer must assert byte-identical bundle digest, canonical component decoding, matching WILLIAM3, valid signature, valid capability, matching public IDs and payload fields, `UnknownTrust`, planned `WouldApply`, committed `AppliedAtCommit`, and current status `Live`. The two legs must have distinct author, object, entry, evidence, and bundle IDs/digests.

Before the cross-runtime legs, both the XCTest bundle and the Android instrumentation bundle run a generated-binding semantic contract fixture and emit `ios_binding_semantics` and `android_binding_semantics` evidence sections. They assert lossless decoding of `WouldApply`/`AppliedAtCommit`, `WouldBeDominated`/`DominatedAtCommit`, `AlreadyPresent`/`NoChanges`, a later transition from receipt `AppliedAtCommit` to current status `NotLive { PrunedLater }`, one item-scoped canonical diagnostic, and whole-bundle `InspectBundleResult::Rejected`. These native assertions prove both inspection-result variants and the full core vocabulary survive Swift and Kotlin code generation; only the two fresh `WouldApply`/`AppliedAtCommit` artifacts cross runtimes.

The script resolves one simulator UDID and one emulator serial, fails on ambiguity, and runs these concrete test entrypoints:

```text
xcodebuild test -project apps/ios/RiotEvidence/RiotEvidence.xcodeproj -scheme RiotEvidence -destination id=$IOS_UDID -only-testing:RiotEvidenceTests/BindingSemantics
xcodebuild test -project apps/ios/RiotEvidence/RiotEvidence.xcodeproj -scheme RiotEvidence -destination id=$IOS_UDID -only-testing:RiotEvidenceTests/IOSCreatesBundle
xcrun simctl get_app_container $IOS_UDID org.riot.evidence data
adb -s $ANDROID_SERIAL push build/handoff/ios-generated.riot-evidence /data/local/tmp/ios-generated.riot-evidence
adb -s $ANDROID_SERIAL shell run-as org.riot.evidence cp /data/local/tmp/ios-generated.riot-evidence files/ios-generated.riot-evidence
ANDROID_SERIAL=$ANDROID_SERIAL ./gradlew :app:connectedDebugAndroidTest -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.BindingSemanticsTest
ANDROID_SERIAL=$ANDROID_SERIAL ./gradlew :app:connectedDebugAndroidTest -Pandroid.testInstrumentationRunnerArguments.class=org.riot.evidence.CrossRuntimeHandoffTest
adb -s $ANDROID_SERIAL exec-out run-as org.riot.evidence cat files/android-generated.riot-evidence
xcodebuild test -project apps/ios/RiotEvidence/RiotEvidence.xcodeproj -scheme RiotEvidence -destination id=$IOS_UDID -only-testing:RiotEvidenceTests/IOSImportsAndroidBundle
```

The script fails if either file is missing, empty, rewritten, decoded by the shell, changes digest in transit, does not load the packaged native library, or produces divergent facts. This proves codec/ABI/runtime interoperability on emulator and simulator, not radio transport or physical-device readiness.

## TDD and Verification

Each work unit begins with its named failing test, confirms the intended failure, implements the smallest passing behavior, and preserves the fixtures during refactoring.

| WU | First RED evidence | GREEN command |
| --- | --- | --- |
| WU0R | dependency validator rejects `willow25 0.5.0`, old lock hash, missing corrected WILLIAM3 vectors/independent provenance, `panic=abort`, or enabled Drop Format | `cargo xtask validate-contracts` and `cargo test -p riot-conformance william3_` |
| WU1 | alert vectors, one-instant UTC/TAI clock conversion and `CLOCK_UNAVAILABLE` encode failure, canonical Willow component bytes, corrected payload digests, and same-namespace authority denial fail because adapters/policies are incomplete | `cargo test -p riot-core public_` |
| WU2 | inspection `CLOCK_UNAVAILABLE` no-retention failure, join laws/permutations, dispositions, transaction, bounds, rollback, Rust log-safety, panic, and three lifecycle-concurrency tests fail because types are absent | `cargo test -p riot-core -p riot-conformance core_import_` |
| WU3 | native tests fail because generated bindings/libraries, semantic-contract fixtures, and runtime handoff are absent | `scripts/phase0a/cross-runtime-handoff.sh` |
| WU4 | adversarial manifest fails on missing hostile-corpus mutations, closure scan, hashes, and gate decisions | `scripts/phase0a/verify.sh` |

`scripts/phase0a/verify.sh` runs, without omitted command placeholders:

```text
cargo xtask validate-contracts
cargo xtask generate-bindings
cargo xtask package-ios
cargo xtask package-android
cargo test --workspace --all-targets
cargo test -p riot-conformance william3_
cargo test -p riot-conformance core_import_
cargo test -p riot-conformance hostile_bundle_
cargo test -p riot-conformance hostile_alert_
cargo build -p riot-ffi --release --target aarch64-apple-ios
cargo build -p riot-ffi --release --target aarch64-apple-ios-sim
cargo build -p riot-ffi --release --target aarch64-linux-android
cargo build -p riot-ffi --release --target x86_64-linux-android
cargo tree -p riot-ffi --edges normal,build
cargo tree -p riot-ffi -e features
scripts/phase0a/cross-runtime-handoff.sh
```

The verification script additionally rejects `willow25/drop_format`, `bab_rs <0.8.1`, forbidden group/OpenMLS libraries, deterministic providers or seeds, forbidden Ed25519 features (`serde`, `hazmat`, `pem`, `pkcs8`), debug features, plaintext fixture secrets, and secret-bearing symbol/log strings in release XCTest/APK artifacts. Feature-tree policy is authoritative; symbol scanning is defense in depth. The report records commands, environment, exit status, artifact paths, and hashes rather than interpreting a skipped command as PASS.

## Work Units and Hard Budget

The budget is 16 aggregate agent-hours. Parallel agents charge their wall time independently. Work stops at a checkpoint instead of borrowing scope.

`docs/decisions/phase0a-time-ledger.json` is the authoritative accounting record. Each worker records work unit, start/end timestamps or an explicitly reported combined active duration, commits, and evidence paths; overlapping independent workers are summed, not collapsed to elapsed clock time. The gate report reconciles the ledger before starting another work unit. The existing WU0R+WU1 report supplies one combined approximately 2.0-hour duration, which is charged once rather than split or double-counted. Future repair work is charged separately and is not prepaid by that baseline.

| Work unit | Budget | Deliverable and stop rule |
| --- | ---: | --- |
| WU0 — completed platform preflight | 1h spent | arm64 AVD, blank instrumentation, four native Rust targets, contracts, manifest, and original locks verified; platform evidence remains PASS |
| WU0R+WU1 — implemented baseline | 2h spent | corrected resolved graph plus provisional alert/authority/bundle code; G0/G1 PASS claims reopened by review |
| G0/G1 repair | 1.5h | executable validator/profile/vector repairs, fallible entropy/clock, same-namespace denial, value/proof identity, narrow capability policy, structured bounded decoder matrix; WU2 stops until PASS |
| WU2 — Willow join, import, and provenance facts | 4h | namespace-local join laws and oracle permutations, bounded snapshot transaction, three dispositions, essential hostile cases, and arbiter concurrency tests; G2 FAIL stops native work |
| WU3 — FFI and native handoff | 4h | generated Swift/Kotlin bindings, native full-vocabulary contract fixtures, XCTest/instrumentation hosts, container-aware two-way runtime handoff; no UI or JVM substitute |
| Integration contingency | 1.5h | reserved only for dependency, binding, packaging, simulator/emulator, or handoff repair; unused time is not converted to stretch scope |
| WU4 — adversarial verification and report | 2h | bounded hostile corpus, closure/bundle scan, hashes, gate report, GO/REVISE decision |

WU4 always begins at aggregate hour 14 and is not available for feature rescue. If core work plus contingency has not completed by then, unfinished gates are INCONCLUSIVE and WU4 records the evidence that exists.

The reopened G0/G1 repair checkpoint is equally hard: at 1.5 newly charged repair hours, record PASS or INCONCLUSIVE before charging any WU2 time. Repair overrun may use contingency only if explicitly recorded; it never silently consumes WU2/WU3/WU4 reservations.

## Gates

| Gate | PASS evidence | FAIL / INCONCLUSIVE action |
| --- | --- | --- |
| G0 Correct Willow basis | corrected WILLIAM3 vectors with independent provenance, alpha.3/fixed Bab pins, disabled Drop Format, unwind-capable FFI evidence profile, five-target compile, lock hash, and feature closure pass | stop Willow implementation and revise dependency strategy |
| G1 Public model and authority | required operational-alert fields, deterministic Riot/Willow component vectors, one-instant UTC/TAI conversion plus no-partial-entry encode clock failure, one communal-author success, one same-namespace cross-subspace denial, and hostile decoder bounds pass | revise object/authority mapping; do not expand schemas |
| G2 Willow join, import, and provenance | every core fixture row, no-retention inspection clock failure, generation-bound planned effects matching commit, join laws/oracle permutations, three receipt dispositions, stable historical IDs/current status, structured diagnostics, hard store/preview bounds, logical rollback, duplicate result, provenance facts, Rust log/panic safety, and arbiter-concurrency assertion passes | block public import expansion |
| G3 Native runtime handoff | Swift and Kotlin enforce capped reads, preserve the full planned/receipt/current-status/diagnostic vocabulary, and leak no hostile marker in captured native/instrumentation logs; a distinct iOS-generated signed alert imports as `AppliedAtCommit` on Android and a distinct Android-generated alert imports as `AppliedAtCommit` on iOS through packaged generated bindings; corrected WILLIAM3 and all per-leg fact assertions pass | revise ABI/toolchain; do not claim cross-platform core |

Every report contains status, owning work unit, exact commands, frozen environment, evidence paths, hashes, elapsed agent-hours, and next action.

GO requires PASS on all four gates, G0 through G3, and authorizes an implementation plan for the public newswire/file loop and additional public object schemas. It does not authorize private groups, the bridge, live transport, or production security claims.

## Separately Gated Follow-On Evidence

### Phase 0B — Private Group Cryptography

Phase 0B needs its own design review, threat model, dependency closure, and agent-hour budget before code. Its requirements ledger includes:

- canonical signed application-authorization sidecars bound to full parsed MLS commit deltas and policy versions;
- staged/accepted/frozen transition semantics and an explicit pre-detection fork-emission residual risk;
- exact group-control store transactions and policy/key rotation;
- bounded member, proposal, credential, extension, KeyPackage, voucher, transition, group, nonce-ledger, and replay-index state;
- fixed-shape route-key selection with one real-or-dummy AEAD operation, defined keyring scope, overflow behavior, and timing evidence;
- exact invite commitment/proof, outer privacy boundary, sponsor revocation, and offline double-redemption fork/freeze behavior;
- complete sensitive-state inventory, wrapping/backup/panic order, restore tests, native-bundle exclusion checks, and independent cryptographic review.

None of these requirements is considered satisfied by Phase 0A.

### Phase 0C — Declassification Bridge

Phase 0C also needs its own reviewed design and budget. It must choose a coherent immutable-value or session-bound candidate lifecycle, cryptographically bind finalization to the exact reviewed fields/destination/signer, define mutation and abandonment behavior, preserve public originals on clipping, and pass differential noninterference pairs including error and unknown-field behavior.

## External Release Gates

Local transport cannot be called field-ready until tested on physical iOS and Android devices. Object vocabulary and provenance UI cannot be frozen until organizers and mutual-aid practitioners exercise realistic alert and coordination flows under time, power, connectivity, and seizure constraints. Private groups cannot be called production-safe without an independent review of the exact construction, code revision, lockfiles, vectors, platforms, persistence design, and artifact hashes.

The alpha Willow/fjall closure is evidence-only. Before any production release, Riot must re-evaluate a stable corrected Willow release or obtain a smaller upstream feature boundary and rerun dependency, canonical-byte, digest, mobile-build, and independent security gates.
