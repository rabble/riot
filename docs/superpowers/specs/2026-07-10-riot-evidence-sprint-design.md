# Riot Evidence Sprint Design

Date: 2026-07-10
Status: Revision 2; approved by product owner and awaiting design-review rerun

## Purpose

Run a hard-capped 16 agent-hour evidence sprint that turns Riot's highest-risk architectural claims into executable proofs. The sprint establishes whether one stable Rust core can serve native iOS and Android clients, whether the revised Willow authority model works, whether a public signed object survives an offline exchange loop, and whether MLS plus an opaque private envelope is a credible research basis for groups.

This is not a product alpha. Its outputs are code, fixtures, traces, limits, and PASS/FAIL/INCONCLUSIVE decisions. A dependency or toolchain problem is INCONCLUSIVE, not evidence that an architecture is impossible.

Research basis: `docs/research/2026-07-10-dual-mode-research-addendum.md`.

## Product Hypotheses and Evidence Boundary

Riot ultimately serves five user outcomes:

1. A public carrier can inspect signer, source, size, freshness, and counts before import.
2. A field publisher can sign an update, move it through another device, and verify it without a server.
3. A private member can carry group state over an untrusted channel without exposing inner group identifiers or content.
4. A collective editor can deliberately declassify selected private content without leaking private metadata.
5. A pressured reader can distinguish authorship, cryptographic validity, source claims, import provenance, curation, correction, freshness, and expiry.

Phase 0 proves structural prerequisites, not usability under pressure.

| Outcome | Phase 0 evidence | Gate | Deferred validation |
| --- | --- | --- | --- |
| Public carrier | Import fixtures produce a platform-independent preview model with explicit eligibility and provenance fields | G3 | Full SwiftUI/Compose flow and practitioner exercise |
| Field publisher | Both native harnesses call the same Rust encoder/verifier and display the same fixture digest/status | G1, G3 | Real authoring and file transfer on physical devices |
| Private member | Rust lab executes authorized MLS transitions and opaque-envelope tests; mobile targets compile the lab dependency closure | G4, G5 | External crypto review and physical-device exchange |
| Collective editor | Group-side allowlist projection and public-side finalization pass differential noninterference tests | G6 | Human review UX on both platforms |
| Pressured reader | `ProvenanceDisplay` fixtures separate claims, signatures, receipts, lenses, freshness, and expiry | G3 | Comprehension and action testing with organizers |

Trust bootstrap is deliberately not solved by a valid signature. Unknown signers remain labelled unknown. Directory feeds, trusted introductions, and curation-lens selection are later designs.

The representative coordination fixture covers:

```text
request
  → verification annotation
  → task claim and handoff annotations
  → offer
  → commitment
  → fulfillment annotation
```

## Success Questions

- Can Rust emit stable deterministic object bytes and transport them correctly through Swift and Kotlin bindings?
- Can current `willow25` and Meadowcap express a communal author plus owned curation authority?
- Can a public object complete inspect → preview → select → atomic logical commit → receipt?
- Can OpenMLS build for selected mobile targets and enforce Riot's membership authorization around add/remove/update operations?
- Can Riot detect and safely freeze conflicting MLS commits, advance offline members through bounded transitions, and exclude removed members?
- Can an opaque private envelope meet the frozen evidence profile and leakage tests?
- Can the bridge prove that public output depends only on allowlisted draft fields?

Each question ends in:

- **PASS:** the pinned environment produces the specified artifact and assertions;
- **FAIL:** a reproducible fixture or trace disproves the claim;
- **INCONCLUSIVE:** the evidence did not run or finish because of time, toolchain, dependency, or environment constraints.

Only PASS permits downstream GO. INCONCLUSIVE means REVISE/reschedule; it never defaults to GO.

## Non-Goals

The sprint does not deliver production-safe groups, polished import/declassification UI, durable crash-safe storage, current Willow Drop Format interoperability, live WTP, local radio transport, directory/rendezvous, gateway, media, local LLM, accounts, servers, arbitrary HTML, or field usability validation.

## Frozen Environment and Dependency Ledger

WU0 verifies and records the following pins before implementation. `Cargo.lock`, Gradle dependency locks, and the Xcode resolved-package state are evidence artifacts. Any substitution changes the evidence identity and is recorded.

### Rust and protocol dependencies

| Dependency | Pin / feature policy |
| --- | --- |
| Rust | `1.95.0`; workspace `rust-toolchain.toml` |
| `willow25` | `=0.5.0`; default `std`, `dev` only in tests |
| `openmls` | `=0.8.1`; `openmls_rust_crypto` and `fork-resolution`; forbid `crypto-debug`, `content-debug`, `test-utils` outside tests |
| `openmls_rust_crypto` | `=0.5.1` |
| `uniffi` | `=0.32.0`; proc-macro scaffolding, Swift and Kotlin bindings |
| `minicbor` | `=2.2.2`; hand-written fixed-key encoders; no floating point or indefinite lengths |
| `cddl-cat` | `=0.7.1`; test/dev schema validation only |
| `aes-gcm-siv` | `=0.11.1`; evidence private-envelope AEAD, RFC 8452 |
| `sha2` | `=0.10.9`; fixture and replay digests |
| `hmac` | `=0.12.1`; route-tag HMAC-SHA-256 |

All workspace dependencies use exact versions. The lockfile digest is included in `fixtures/manifest.json`.

### Apple target

- Xcode 26.2, Swift 6.2.3.
- Deployment floor: iOS 17.0.
- Rust triples: `aarch64-apple-ios` and `aarch64-apple-ios-sim`.
- Gate build: arm64 iOS Simulator; device triple must compile even when no signing device is present.
- Evidence shell: one SwiftUI screen showing fixture digest and structured status; no import UX claim.

### Android target

- Android Gradle Plugin 9.0.1, Gradle 9.1.0, JDK 17.
- Android SDK/Build Tools 36.0.0, NDK 28.2.13676358.
- `minSdk 26`, `compileSdk 36`, `targetSdk 36`.
- ABIs: `arm64-v8a` and `x86_64`.
- Kotlin 2.2.20; Compose BOM 2026.06.00.
- Gate build: x86_64 JVM/emulator unit target plus arm64 native-library compile.
- Evidence shell: one Compose screen showing fixture digest and structured status; no import UX claim.

The current host has no Android SDK configured. WU0 installs/verifies the pinned SDK or marks Android-dependent gates INCONCLUSIVE with the exact missing component. It does not silently change targets.

## Architecture and Dependency Closures

```text
riot-model ───────────────┐
riot-willow ──────────────┼─> riot-stable ─> riot-stable-ffi ─> SwiftUI / Compose harnesses
riot-import ──────────────┤
riot-bridge-public ───────┘

riot-model ───────────────┐
riot-willow ──────────────┼─> riot-group-lab ─> riot-group-lab-ffi (evidence only)
OpenMLS / crypto provider ┘

riot-conformance depends on both closures for tests and traces.
The stable closure never depends on riot-group-lab or OpenMLS.
```

The release-shaped stable FFI graph is mechanically checked with `cargo tree -p riot-stable-ffi`. It must not contain `riot-group-lab`, `openmls`, deterministic key sources, or crypto-debug features. The group-lab FFI is a separate evidence target and cannot be linked by the stable native harnesses.

### Planned source structure

```text
Cargo.toml
rust-toolchain.toml
crates/
  riot-model/src/
  riot-willow/src/
  riot-import/src/
  riot-bridge-public/src/
  riot-stable/src/
  riot-stable-ffi/src/
  riot-group-lab/src/
  riot-group-lab-ffi/src/
  riot-conformance/src/
schemas/
  riot-envelope.cddl
  alert.cddl
  request.cddl
  offer.cddl
  commitment.cddl
  task.cddl
  annotation.cddl
fixtures/
  manifest.json
  objects/
  willow/
  imports/
  groups/
apps/ios/RiotEvidence/
apps/android/
docs/decisions/
```

Phase 0 implements the common envelope and six kinds needed by the alert and coordination fixtures. `observation`, `event`, `resource`, and `document` remain approved vocabulary but their executable schemas belong to the first implementation plan after the evidence sprint.

## Test Seams

Production-facing logic receives explicit providers:

- `Clock` for signed time, local receipt time, and clock uncertainty;
- `IdSource` for object, revision, operation, store, voucher, and receipt IDs;
- `RandomSource` for production OS randomness and deterministic test vectors;
- `SignerProvider` for purpose-specific signing without exposing private keys through FFI;
- `KeyWrapper` interface, with a fake in Phase 0 and native Keychain/Keystore implementations deferred;
- `EvidenceStore`, with copy-on-write logical transactions, generation numbers, deduplication index, and receipts;
- `FaultInjector`, enabled only in tests, with named failures before validation, after staging, during entry application, before receipt, and before commit swap.

`fixtures/manifest.json` records fixture version, complete identifiers, deterministic test seeds, expected encoded SHA-256, expected decoded values, dependency-lock digest, generating command, and negative mutations. Deterministic providers cannot compile into stable release profiles.

## Stable API and Handle Lifecycle

UniFFI exposes typed records and thread-safe objects rather than naked integer handles.

```text
RiotSession.open(CoreConfig) -> RiotSession
RiotSession.createEvidenceStore() -> EvidenceStoreHandle
RiotSession.encodeObject(ObjectDraft) -> EncodedObject
RiotSession.decodeObject(bytes, DecodeLimits) -> DecodedObject
RiotSession.beginInspection(store, bytes, ImportContext) -> InspectionOperation
InspectionOperation.progress() -> InspectionProgress
InspectionOperation.cancel() -> CancelResult
InspectionOperation.takePreview() -> ImportPreview
ImportPreview.commit(selection) -> ImportReceipt
ImportPreview.reject() -> RejectReceipt
close() on session, store, operation, and preview objects
```

Rules:

- objects are owned by the creating session and are safe to call from one operation at a time;
- `close` is idempotent; every later call returns `STALE_HANDLE`;
- process restart invalidates all objects;
- closing a session cancels operations and closes previews before stores;
- caller limits may lower but never raise `CoreConfig` policy ceilings;
- cancellation is cooperative during reading, parsing, validation, and signature checks and leaves the store unchanged;
- progress reports phase, bytes consumed/total, objects inspected, and whether cancellation is accepted;
- FFI errors use stable non-sensitive codes plus developer detail that contains no payload, key, hidden group, or invitation data;
- native layers localize user-facing messages separately.

Phase 0 uses deterministic in-memory test signers behind `SignerProvider`. Production Keychain/Keystore signing and MLS secret persistence are release-gated work, not claimed here.

## Object, Receipt, and Provenance Models

### Signed content envelope

```text
schema
object_id
revision_id
created_at
valid_from?
expires_at?
language
kind
body
relations[]
source_claims[]
ai_assisted
```

Signer, namespace, capability, Willow timestamp, and payload digest belong to the Willow entry. Local import facts never enter the author's signed payload.

Deterministic CBOR uses definite lengths, integer field keys in fixed order, shortest integer encodings, no floats, no duplicate keys, and strict rejection of unknown envelope keys. JSON is a lossless debugging projection, not the signed form.

### `ProvenanceDisplay`

The core returns a platform-independent model with five labelled layers:

1. **Authorship:** complete author subspace, collective namespace if applicable, delegated-signer status, signed creation time.
2. **Cryptographic status:** payload digest, signature valid/invalid, capability valid/invalid/unknown. It never says the content is true.
3. **Author claims:** source notes and affected area, explicitly labelled "Claimed by author."
4. **Local receipt:** artifact digest, import route, first-seen time, local receipt time, duplicate status.
5. **Reader lens:** selected curation feed, corrections/disputes, expiry/freshness, and clock uncertainty.

Preview and post-commit receipt use the same labels. The post-commit screen/type is called `ImportReceipt`, not "verification receipt."

## Willow Authority Fixture

The fixture includes two communal authors, an owned curation namespace with delegated curators, feature and correction annotations, and an owned publication copy.

Tests prove one communal author cannot write another's subspace, no communal creator root exists, owned curators can receive path/time restrictions, publication namespace and signer remain distinguishable, and raw entries remain available regardless of lenses.

## Import Contract

### Evidence bundle codec

Phase 0 uses `RiotEvidenceBundleV1`: visible magic `RIOTE1`, version 1, followed by deterministic CBOR containing complete Willow entries, authorization material, and payloads. It forbids compression and carries at most 512 entries within the 8 MiB artifact ceiling. Its codec ID is `org.riot.evidence-bundle/1`; its extension is `.riot-evidence`.

This format exists only to exercise import. It is intentionally incompatible with `.snk` and the current Willow Drop Format, and no production plan may rename it into apparent compatibility.

### Evidence store and transaction boundary

`EvidenceStore` is an in-memory copy-on-write store selected to prove logical atomicity, not crash durability. It has a random store ID, monotonic generation, content-digest index, entries, and receipts.

An inspection operation retains immutable input bytes and binds:

- codec ID and version;
- bundle digest;
- destination store ID and base generation;
- import route and local clock snapshot;
- fixed policy ceilings;
- per-entry stable preview ID, original entry digest, status, and eligibility.

`ImportPreview.commit` rejects a different store generation as `STALE_PREVIEW`. One copy-on-write swap commits selected entries, deduplication indexes, generation increment, and receipt together. Fault injection before the swap leaves the observable logical state unchanged. Byte-stable persistence and crash recovery are not claimed.

### Preview and receipt

Each preview entry contains:

- preview entry ID and original digest;
- object kind/schema or opaque status;
- author and namespace;
- cryptographic/capability status;
- freshness/expiry and clock uncertainty;
- encoded bytes;
- `eligible`, `already_present`, and `requires_opaque_consent` flags;
- an ineligibility reason when applicable.

Invalid signatures/capabilities are never eligible. Unknown verified schemas require explicit opaque consent and remain non-renderable. Selection must be non-empty, reference only this open preview, contain no duplicates, and include consent for every unknown. Reject/cancel closes the preview. Retrying a committed bundle is idempotent and produces an already-present receipt.

`ImportReceipt` contains codec/version, bundle digest, store ID, before/after generation, selected entry digests, inserted/already-present counts, receipt ID, import route, and local receipt time.

### Import limits

Evidence ceilings are fixed in `CoreConfig`; callers may only lower them.

| Resource | Ceiling |
| --- | --- |
| artifact bytes | 8 MiB |
| objects | 512 |
| encoded object bytes | 1 MiB |
| CBOR nesting | 16 |
| map entries | 128 |
| text/byte string | 64 KiB except the bounded object body |
| path components | 64 |
| path component bytes | 256 |
| total path bytes | 2,048 |
| expansion ratio | 1:1; Phase 0 formats forbid compression |
| temporary logical store growth | artifact size + 8 MiB |
| inspection wall target | 2 seconds for an 8 MiB local fixture; exceeding reports measured FAIL/INCONCLUSIVE, never a security pass |
| cancellation poll interval | at most 64 KiB or 32 objects |
| MLS generation gap | 128 |
| retained prior epochs | 4 |
| simultaneous forks | 2; additional forks freeze and reject |
| envelope key trials | 32, executed in constant-shape loop |

Integer arithmetic is checked before allocation. Native front doors reject files larger than 8 MiB before materializing bytes for the evidence FFI. Streaming import is a later implementation requirement.

### Fixture-to-state matrix

| Fixture | Visible state | Enabled actions | Store effect |
| --- | --- | --- | --- |
| empty drop | Empty | Reject | None |
| valid known object | Eligible; provenance shown | Select, commit, reject | Selected insert + receipt |
| unknown but verified schema | Opaque; consent required | Consent+select, reject | Opaque bytes only if selected |
| invalid signature/capability | Invalid; reason shown | Reject | None |
| oversized/malformed | Rejected before preview | Close | None |
| duplicate | Already present | Commit receipt or reject | No duplicate entry |
| expired object | Eligible but expired label | Select, commit, reject | Selected insert; remains expired |
| mixed/partial selection | Per-entry eligibility | Eligible subset, reject | Only selected entries |
| empty selection | Selection error | Select or reject | None |
| stale preview | Stale | Reinspect or close | None |
| cancelled inspection | Cancelled | Close/retry | None |
| injected commit failure | Commit failed | Retry/close | Before-state retained |

Swift and Kotlin state-model tests consume the same serialized preview fixtures. Phase 0 does not claim polished UI parity.

## Private Group Laboratory

### Key separation and authorization

Each evidence persona has distinct Ed25519 keys for:

- public/newswire signing;
- Willow group subspace signing;
- MLS leaf signatures;
- group-control authorization.

A `GroupPolicyV1`, signed by the group policy key, binds group context, accepted credential type, current members, and roles. A valid MLS message is insufficient by itself.

- members may update their own MLS leaf;
- `membership_admin` may add, remove, and approve recovery;
- `invite_sponsor` may issue vouchers but cannot commit an add unless also an admin;
- every add/remove/recovery commit carries a `RiotControlAuthorization` binding parent epoch, parent confirmed-transcript hash, MLS commit digest, action, target credential, and voucher ID when applicable;
- unauthorized but cryptographically valid commits are rejected and tested.

The evidence credential is an OpenMLS BasicCredential containing a random Riot group-persona ID and purpose. Public persona material is forbidden.

### MLS transition contract

Application data for epoch N+1 cannot be created until the N→N+1 control transition is durably accepted in the evidence store.

- A control envelope is encrypted with epoch N exporter keys and contains the authorized MLS Commit for N→N+1.
- Existing members, including a member being removed, may open the transition; only members in N+1 derive the next data key.
- New members receive the MLS Welcome through the invite response and then accept N+1 data.
- Data envelopes for N+1 are never bundled ahead of the accepted control transition.
- Offline members advance one retained control envelope at a time. More than four missed epochs requires rejoin.

### Fork and rollback evidence policy

Riot does not invent an automatic cryptographic winner in Phase 0.

- only commits with valid `RiotControlAuthorization` are candidates;
- accepting one commit records parent transcript hash, accepted commit digest, new epoch, and a monotonic local high-water mark;
- two authorized commits for the same parent enter `FrozenFork`; no new private publication, membership transition, or envelope creation is permitted;
- all candidate states remain bounded at two and their secrets are held only for the lab trace;
- explicit abort deletes losing candidate secrets;
- rejoin/successor-group recovery is recorded as required product work;
- a fully restored old device snapshot cannot be detected without an external or cross-device anchor. This is an expected G4 limitation and blocks production claims even if fork detection passes.

The malicious-carrier trace withholds, reorders, duplicates, and later releases commits. PASS requires uniform freeze and no new-epoch data emission. It does not claim availability or rollback resistance.

### Frozen evidence envelope profile

The private-envelope experiment uses this exact profile:

- ciphersuite: AES-256-GCM-SIV (`aes-gcm-siv` 0.11.1, RFC 8452);
- AEAD key: 32-byte MLS exporter output using label `riot/drop/aead/v1` and context `role || parent_epoch`;
- route key: separate 32-byte exporter output using label `riot/drop/route/v1` and the same context;
- nonce: 96 random bits from `RandomSource`; a per-epoch nonce ledger rejects reuse; duplicate-nonce tests are mandatory;
- route tag: first 16 bytes of HMAC-SHA-256(route key, nonce || role);
- visible header/AAD: ASCII `RIOTP1`, version 1, role, nonce, route tag, and bucket size;
- plaintext: 32-bit inner length, inner bytes, then random padding to the selected bucket before encryption;
- buckets: 4 KiB, 16 KiB, 64 KiB, 256 KiB, 1 MiB, 4 MiB, 8 MiB;
- overflow above 8 MiB: reject;
- padding validation: ciphertext must exactly match the declared bucket; inner length must fit; all header fields are authenticated;
- replay identity: SHA-256 of header plus ciphertext, stored in the receipt index;
- key selection: compute tags against exactly 32 slots (real active/retained epoch keys plus dummy keys), then attempt AEAD only for a unique match; zero or multiple matches return the same generic cannot-open error;
- wrong key, wrong AAD, malformed padding, unknown version, replay, and tamper use non-oracular error codes at the user boundary.

The visible header reveals that this is a Riot private artifact, its role, and its padded size. It does not claim traffic unlinkability. Route tags vary per nonce and are tested for non-repetition.

### Invite voucher invariant

`InviteVoucherV1` is signed by an `invite_sponsor` key over:

- domain `riot/invite/v1`;
- random 256-bit voucher ID and commitment to a 256-bit secret;
- group-policy digest and issued parent epoch;
- maximum acceptable parent epoch;
- optional wall-clock expiry;
- allowed initial role;
- sponsor credential ID.

The invitee redemption request signs the voucher ID, group-policy digest, one-time MLS KeyPackage hash, and secret proof with the invitee MLS signature key. Redemption requires current sponsor authorization, a parent epoch within the bound, an unconsumed voucher, and unused KeyPackage. Clock rollback or uncertainty disables unattended expiry acceptance and requires an online-with-the-group admin decision; epoch bounds remain mandatory.

Voucher consumption, accepted add-commit digest, and KeyPackage deletion occur in one logical control transaction. Replay, cross-group substitution, KeyPackage substitution/reuse, revoked sponsor, expired epoch, losing fork, and double redemption are negative fixtures.

### Group laboratory limits and gates

G4 splits into:

- **G4a membership authorization:** mobile target compilation, create/add/remove/update/Welcome, and unauthorized-valid commit rejection;
- **G4b transition safety:** bounded offline advance, removed-member exclusion, replay behavior, malicious-carrier fork freeze, and documented snapshot-rollback limitation.

G5 covers the frozen envelope profile, known-answer vectors, nonce/AAD/tamper/replay/key-trial/padding tests, and byte-level disclosure search.

Any FAIL or INCONCLUSIVE result keeps groups research-only. Passing G4/G5 still requires external cryptographic review.

## Bridge Boundary

### Group side

Only `riot-group-lab` can read a private object. It emits a value-only `DeclassificationCandidate` containing closed, per-kind allowlisted fields plus `ai_assisted`. It contains no private handle, ID, relation, receipt, ordering key, source/location default, extension map, or unknown nested field.

Cancelling or abandoning projection invalidates the candidate, clears transient buffers, leaves both stores unchanged, and creates no public receipt.

### Public side

`riot-bridge-public` accepts only `DeclassificationCandidate` values. A review operation requires explicit destination namespace, public signer, source claims, location claims, and confirmation booleans. Finalization requires a reviewed-draft object and generates public object/revision IDs solely from the public `IdSource` after review.

Public-to-private clipping belongs to the group closure: it verifies and retains the original public bytes unchanged, then creates a distinct private annotation.

### Differential noninterference

G6 uses pairs of private fixtures whose allowlisted fields are identical while every prohibited field varies, including values, lengths, order, nested extensions, relations, receipts, paths, signers, group IDs, capabilities, source defaults, and error-triggering mutations.

With fixed public test providers, group-side candidates and pre-signing public projections must be byte-identical across each pair. Unknown fields reject rather than disappear. Additional tests cover encoded, hashed, normalized, and length-only canaries. Literal substring scanning remains a supplemental check, not the oracle.

## Sensitive-State Inventory and Panic Scope

Phase 0 does not implement or claim platform panic wipe. WU0 records the required inventory:

| Material | Intended persistence | Panic action / release requirement |
| --- | --- | --- |
| Willow persona signing keys | wrapped under per-persona key | destroy wrapper; clear in-memory signer |
| MLS leaf/signature secrets | wrapped group control store | destroy before file cleanup |
| MLS epoch/ratchet/fork secrets | wrapped group control store; bounded epochs/forks | destroy all current, retained, and candidate state |
| KeyPackage private keys / Welcome | wrapped invite/control store | consume/delete on join; destroy on panic |
| voucher secrets and redemption requests | wrapped invite store | delete and invalidate handles |
| per-group storage key | wrapped by platform key | delete first persistent root |
| decrypted entries and previews | memory / protected temporary storage only | invalidate, zero where supported, remove previews/caches |
| DB journals, thumbnails, share-sheet files, crash snapshots, clipboard, notifications | platform-specific | exclude or sanitize; each needs a later platform test |

Production requires device-only backup exclusions, locked-file protection, native Keychain/Keystore wrappers, background/snapshot redaction, and post-restart/post-filesystem-restore tests. Secure erasure of exported copies or every flash block is never claimed.

## TDD and Verification Matrix

Each work unit starts with its named RED test, confirms failure for the intended missing behavior, implements the smallest GREEN behavior, then refactors without changing fixtures.

| WU | First RED test and expected failure | GREEN command and expected result |
| --- | --- | --- |
| WU0 | Docs/fixture validator rejects absent pins, limits, hashes, and requirement ownership | `cargo xtask validate-contracts`; PASS with zero missing fields |
| WU1 | `riot-model` fixture test fails because deterministic encoder and six schemas do not exist; Willow test fails because adapter is absent | `cargo test -p riot-model -p riot-willow`; all golden/authority tests PASS |
| WU2 | Swift/Kotlin tests fail because generated stable bindings and native libraries are absent | `xcodebuild test -project apps/ios/RiotEvidence/RiotEvidence.xcodeproj -scheme RiotEvidence -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2'` and `./gradlew :app:testDebugUnitTest`; both report the WU0 fixture digest/status |
| WU3 | import tests fail at absent preview/transaction types; each fault point initially mutates or cannot compile | `cargo test -p riot-import`; full fixture-state matrix PASS and before-state equals after-state on every injected failure |
| WU4 | membership tests fail at missing authorization; envelope known-answer and tamper tests fail at absent codec | `cargo test -p riot-group-lab`; G4a/G4b/G5 matrix emits PASS/FAIL/INCONCLUSIVE JSON |
| WU5 | bridge paired fixtures differ or APIs do not enforce reviewed state | `cargo test -p riot-bridge-public -p riot-conformance`; G6 differential matrix PASS and report generated |

Additional verification:

- `cargo test --workspace --all-targets`;
- `cargo build -p riot-group-lab-ffi --target aarch64-apple-ios` and `cargo build -p riot-group-lab-ffi --target aarch64-linux-android`;
- property tests for Willow join commutativity, associativity, and idempotence;
- bounded fuzz smoke for CBOR, development bundle, import state, invite, and private envelope;
- `cargo tree -p riot-stable-ffi` release-closure assertion;
- scan Rust, FFI, Swift, Kotlin logs/errors and release symbols for deterministic seeds, debug features, plaintext canaries, and secret-bearing names;
- exact dependency versions and artifact hashes recorded in the report.

## Work Units and Hard Budget

The 16 hours are aggregate agent-hours. The sprint is two checkpointed slices; work stops at each gate rather than borrowing scope silently.

### Slice A — Stable public evidence (hours 0–8)

#### WU0 — Contracts and pins (hours 0–2)

Freeze toolchains, dependencies, limits, schema subset, fixture manifest, outcome/test ownership, secret inventory, and PASS/FAIL/INCONCLUSIVE report format. If the Android toolchain cannot be installed/verified inside this budget, Android gates become INCONCLUSIVE.

#### WU1 — Model and Willow authority (hours 2–5)

Create the stable crates, deterministic codec, six executable schemas, coordination fixture, Willow/Meadowcap authority fixture, and conformance CLI.

#### WU2 — Stable FFI and native harnesses (hours 5–8)

Generate stable Swift/Kotlin bindings and compile the one-screen native harnesses. G1 proves ABI transport of Rust-produced bytes and status; it does not claim independent Swift/Kotlin CBOR implementations.

Checkpoint: if G1 or G2 is FAIL, stop downstream implementation. If a platform is INCONCLUSIVE, continue host-side research but do not mark cross-platform GO.

### Slice B — Import, group, and bridge evidence (hours 8–16)

#### WU3 — Import transaction model (hours 8–10.5)

Implement in-memory logical transactions, preview/receipt models, cancellation, fixed limits, and the fixture-state matrix. No multi-screen UI.

#### WU4 — Private group lab (hours 10.5–14)

Implement OpenMLS authorization/transition traces, fork freeze, invite invariant, and the frozen envelope profile. Remove the earlier custom-HPKE comparison from scope: MLS failure blocks groups rather than triggering weaker custom group crypto.

#### WU5 — Bridge and decision report (hours 14–16)

Implement group-side allowlist projection, reviewed public finalization state, public-to-private clip fixture, differential noninterference, dependency/logging checks, bounded fuzz smoke, and the per-gate report.

## Gates

| Gate | PASS evidence | FAIL / INCONCLUSIVE action |
| --- | --- | --- |
| G1 Stable cross-platform core | Rust fixture digest/status traverses pinned Swift and Kotlin bindings; both target closures compile | Stop native expansion / revise environment or ABI |
| G2 Willow authority and schema subset | communal/owned fixtures and six schemas pass; coordination flow round-trips | Revise authority/object mapping |
| G3 Import/provenance | every fixture-state row, limits, cancellation, stale preview, rollback, idempotence, and receipt assertion passes | Block newswire import expansion |
| G4a Membership authorization | pinned mobile compile plus authorized lifecycle and unauthorized-valid negative tests | Groups remain research-only |
| G4b Transition safety | retained offline chain, removal exclusion, replay, malicious-carrier fork freeze, and rollback limitation report | Groups remain research-only |
| G5 Envelope profile | known-answer, nonce/AAD/tamper/replay/key-trial/padding/limit/disclosure tests pass | No private-drop demo |
| G6 Bridge isolation | closed schemas, reviewed-state enforcement, public-to-private preservation, and differential noninterference pass | Block publish-out |

Every gate report contains status, owning work unit, commands, environment, evidence paths, artifact hashes, and next action.

## Likely Decisions

- GO on G1–G3: shared model/provenance kernel, native binding architecture, newswire file-loop implementation plan.
- Conditional on G4a/G4b/G5 plus independent review: private groups.
- Separate research: directory/rendezvous and trust bootstrap.
- Dependency/conformance project: current Willow Drop Format.
- Deferred: production storage/panic, full UI, live WTP, local transport, gateway, media, paper round trips, and local LLM.

## External Release Gates

Private groups cannot be called production-safe until an independent reviewer approves the exact construction, commit, lockfile, vectors, platforms, and artifact hashes. Any cryptographic-construction or dependency change invalidates that approval.

Local transport cannot be called field-ready until tested on physical iOS and Android devices. Object vocabulary and provenance UI cannot be frozen until organizers and mutual-aid practitioners exercise the request → verification → dispatch → handoff → fulfillment flow under realistic time, power, connectivity, and seizure constraints.
