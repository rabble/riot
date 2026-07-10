# Riot Evidence Sprint Design

Date: 2026-07-10
Status: Approved by product owner; pending metaswarm design review

## Purpose

Run a 16 agent-hour evidence sprint that turns Riot's highest-risk architectural claims into executable proofs. The sprint must establish whether one Rust protocol core can serve native iOS and Android clients, whether the revised Willow authority model works, whether a public signed object survives the complete offline exchange loop, and whether MLS plus an opaque private envelope is a credible basis for private groups.

This is not a product alpha. Its output is code, fixtures, traces, and binary go/revise/stop decisions.

Research basis: `docs/research/2026-07-10-dual-mode-research-addendum.md`.

## Product Outcomes

The evidence supports four user outcomes:

1. **Public carrier:** A person holding a Riot file wants to inspect signer, source, size, freshness, and object counts before import so that hostile or irrelevant data never silently enters their library.
2. **Field publisher:** A person offline wants to create and sign a typed update, move it through another device, and verify it there so that public information does not depend on a reachable server.
3. **Private group member:** A group member wants to carry encrypted group state over an untrusted channel so that a non-member carrier cannot inspect group identifiers, paths, capabilities, or content.
4. **Collective editor:** An authorized editor wants to publish selected private material publicly through a visible review boundary so that private identifiers and provenance cannot cross accidentally.
5. **Reader under pressure:** A reader wants signer, freshness, expiry, correction, and local-import provenance presented consistently so that they can decide what to act on without confusing curation with truth.

## Success Definition

The sprint succeeds only if its evidence answers each question:

- Can Rust produce the same deterministic signed object bytes on both native platforms?
- Can current `willow25` and Meadowcap express a communal author plus an owned curation authority as designed?
- Can a public object complete sign → export → preview → import → verify with atomic store behavior?
- Can OpenMLS compile for the selected mobile targets and execute add, remove, update, Welcome, and concurrent-commit traces?
- Can an opaque private envelope hide every prohibited inner field and enforce documented size buckets?
- Can the bridge projection prove that no private identifier reaches public bytes?

A negative answer is a valid sprint outcome when backed by a reproducible fixture or trace.

## Non-Goals

The sprint does not deliver:

- production-safe private groups;
- a polished application UI;
- current Willow Drop Format interoperability without authoritative fixtures;
- live WTP, BLE, Wi-Fi Aware, or relay transport;
- a global directory or private rendezvous service;
- a public web gateway;
- media transfer;
- a local LLM;
- user accounts or servers;
- arbitrary HTML rendering.

## Architecture

```text
┌─────────────────────┐       ┌─────────────────────┐
│ SwiftUI evidence app│       │ Compose evidence app│
│ iOS platform adapter│       │ Android adapter     │
└──────────┬──────────┘       └──────────┬──────────┘
           │ generated Swift/Kotlin bindings
           └──────────────┬─────────────┘
                          ▼
              ┌──────────────────────┐
              │ Rust protocol core   │
              │ objects / Willow     │
              │ signatures / import │
              │ bridge / lab crypto │
              └──────────┬───────────┘
                         ▼
              ┌──────────────────────┐
              │ Conformance CLI      │
              │ vectors / traces     │
              │ fuzz and leak corpus │
              └──────────────────────┘
```

### Dependency direction

- Native shells depend on generated bindings and platform adapters.
- The FFI crate depends on stable Rust core interfaces.
- The core depends on object, Willow, import, bridge, and laboratory-crypto modules.
- Platform APIs never enter the core.
- The conformance CLI calls the same public core interfaces as the bindings.
- No module imports either native application.

### Planned source structure

```text
Cargo.toml
crates/
  riot-core/
    src/object/
    src/provenance/
    src/import/
    src/bridge/
  riot-willow/
    src/authority.rs
    src/store.rs
    src/drop_codec.rs
  riot-group-lab/
    src/mls.rs
    src/envelope.rs
    src/invite.rs
  riot-ffi/
    src/lib.rs
  riot-conformance/
    src/main.rs
schemas/
  riot-object.cddl
  alert.cddl
  observation.cddl
  event.cddl
  resource.cddl
  request.cddl
  offer.cddl
  commitment.cddl
  task.cddl
  document.cddl
  annotation.cddl
fixtures/
  objects/
  willow/
  imports/
  groups/
apps/
  ios/RiotEvidence/
  android/app/
docs/decisions/
```

Each source unit has one responsibility. `riot-group-lab` is visibly experimental and cannot be imported by a release target without a later design decision.

## Shared Core Contracts

The FFI boundary exposes data-oriented operations, not storage internals or raw secret-key access.

```text
encodeObject(draft) -> EncodedObject
decodeObject(bytes) -> DecodedObject
signPublicObject(encoded, signerHandle, destination) -> SignedEntry
inspectDrop(bytes, importContext) -> ImportPreview
commitImport(previewHandle, selection) -> ImportReceipt
preparePublicDraft(privateObjectHandle) -> DeclassificationDraft
finalizePublicDraft(draft, signerHandle, destination) -> SignedEntry
```

Properties:

- handles are process-local, opaque, and invalid after explicit close;
- byte and object limits are required inputs to parsing operations;
- errors are typed and contain no key material, plaintext private content, or hidden group identifiers;
- the core never logs full payloads, secrets, invitation proofs, or decrypted group envelopes;
- all identifiers shown in diagnostics are complete test identifiers, never truncated lookalikes.

The evidence sprint may adjust names while implementing tests, but the boundary must retain the same responsibilities and dependency direction.

## Object Model

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

Signer, namespace, capability, Willow timestamp, and payload digest remain in the Willow entry. Import route, first-seen time, verification result, local trust, and bridge events remain in a separate receipt.

Payload encoding is deterministic CBOR validated by CDDL. The evidence sprint defines a lossless JSON projection but does not build a gateway.

### Core kinds

The sprint defines schemas for:

- `alert`
- `observation`
- `event`
- `resource`
- `request`
- `offer`
- `commitment`
- `task`
- `document`
- `annotation`

Corrections, translations, curation, disputes, fulfillment, and task-state updates are signed annotations. They do not overwrite the target revision. Concurrent annotations remain visible.

## Open Authority Model

The Willow evidence fixtures contain:

1. a communal namespace with two authors writing only their own subspaces;
2. an owned curation namespace with two delegated curators;
3. a feature annotation targeting one communal entry;
4. a correction from a different authority;
5. a standalone owned publication containing a copied and re-signed object.

The test proves:

- one communal author cannot write another's subspace;
- no communal-space creator root is assumed;
- an owned curator can delegate a path- and time-restricted write capability;
- publication namespace and delegated signer remain distinguishable;
- the raw communal entry remains available regardless of a curation decision.

## Exchange and Import

### Codec boundary

`DropCodec` is an interface with explicit format identity and version. If no conformant current Willow Drop Format implementation or authoritative vector is available during the sprint, Riot uses a development-only deterministic bundle format with a non-Willow magic/version and a file extension that cannot be confused with `.snk` or a conformant Willow drop.

The development codec exists only to prove the application state machine. It is not an interoperability claim.

### Import states

```text
received
  → bounded
  → parsed
  → cryptographically_verified
  → schema_classified
  → previewed
  → committed
```

Rules:

- parsing occurs outside the destination store;
- byte, nesting, path, object-count, and expansion limits apply before allocation where possible;
- unknown schemas may be retained as opaque signed bytes but are never rendered;
- duplicate verified content is idempotent;
- the user sees signer, namespace, counts, bytes, expiry/freshness, unknown schemas, and verification failures before commit;
- selected entries commit atomically;
- any failure before commit leaves the store unchanged;
- wrong-key private import returns a generic not-openable result without a group hint.

### Evidence UI states

Both native shells need only four screens:

1. load fixture/file;
2. show import preview;
3. approve or reject;
4. show verification receipt.

Empty, unknown, invalid, oversized, duplicate, expired, and partially selected imports each have an explicit state. No spinner is allowed without cancellable progress and a byte/object count.

## Private Group Laboratory

### Control and data planes

- OpenMLS manages ordered membership epochs.
- Willow manages signed application data after decryption.
- The private envelope carries MLS control messages and Willow application bytes together.
- A recipient resolves and applies the MLS epoch before importing application entries that require it.

### Required MLS traces

1. create a three-member group;
2. send and open a private application envelope;
3. add a member with a one-time KeyPackage and Welcome;
4. remove a member and prove the removed member cannot open the next epoch;
5. update a member key and exercise recovery behavior;
6. generate two valid commits for one epoch and exercise the selected fork-resolution rule;
7. hold one member offline across multiple epochs and document catch-up or rejoin behavior.

If the mobile targets do not build, the concurrent-commit rule is not deterministic, or offline recovery requires an unavailable centralized service, Gate G4 fails and private groups remain research-only.

### Opaque envelope

A private artifact exposes only the minimum framing necessary to reject unsupported versions and enforce a maximum size. Group ID, epoch, membership, namespace, subspace, path, timestamp, capabilities, inner counts, and payloads are authenticated ciphertext.

The evidence format uses standard library primitives and documented padding buckets. It must never be described as audited or production-ready.

Inspection tests search raw artifacts for:

- every known group and namespace identifier;
- every member public key;
- path components;
- capability bytes;
- object titles and bodies;
- unpadded inner lengths and counts.

The only accepted disclosure is artifact framing plus its documented padded bucket.

### Invite states

```text
issued → presented → redemption_requested → committed → welcomed
   └──────────────→ revoked
```

The lab models expiry, revocation, replay, double redemption, member offline state, and a losing concurrent commit. "Single use" means one canonical committed redemption, not that a copied file disappears.

## Bridge

### Private to public

`preparePublicDraft` performs an allowlist projection. It includes typed body fields and the AI-assistance taint. It excludes group, membership, capability, private signer, receipt, import-channel, private relation, and storage identifiers.

The public object receives a new object ID, revision ID, destination namespace, and purpose-specific public signer. Source and location claims require explicit user confirmation.

### Public to private

The complete original public entry, payload, signature, capability, and namespace provenance remain intact. The clipping member adds a private annotation. No public bytes are rewritten to make them look locally authored.

### Noninterference gate

A corpus of private drafts containing canary values for every prohibited field passes through the bridge. Gate G6 passes only if none of those values appear in the public deterministic CBOR or JSON projection.

## Key Storage and Panic Behavior

The native evidence apps expose a platform key-wrapper interface:

- iOS Keychain with device-only, passcode-bound accessibility where available;
- Android Keystore with non-exportable hardware backing where available.

The group lab stores a random per-group storage key wrapped by that platform key. Panic wipe order is:

1. invalidate in-memory handles;
2. delete the wrapped group key;
3. zero transient secret buffers where supported;
4. schedule encrypted file and cache removal;
5. record only a non-sensitive local completion state.

The design does not claim secure erasure of every flash block or of content already exported to another device.

## Error and Conflict Handling

| Condition | Required behavior |
| --- | --- |
| Malformed or oversized input | Reject in quarantine; destination store remains unchanged. |
| Unknown schema | Preserve only as opaque verified bytes if the user chooses; never render. |
| Invalid signature or capability | Show invalid status; never offer normal ingest. |
| Duplicate object | Treat import as idempotent and report already present. |
| Expired operational object | Show prominently; importing does not make it current. |
| Local clock uncertainty | Show signed time and local receipt time separately; do not silently upgrade freshness. |
| Concurrent object revisions | Preserve both and show the conflict or applicable signed relations. |
| Concurrent task claims | Preserve both claims; no accidental last-writer-wins assignment. |
| Ambiguous MLS epoch/fork | Freeze private publication and envelope creation until deterministic resolution. |
| Removed member imports old data | Permit verification of authorized old epochs; never grant access to later epochs. |
| Wrong private key | Return a generic cannot-open result without group identity. |
| Interrupted commit | Roll back atomically; a later retry remains idempotent. |

## Verification Strategy

### Deterministic and algebraic tests

- Rust, Swift, and Kotlin produce or consume the same golden CBOR fixtures.
- Willow join fixtures exercise commutativity, associativity, and idempotence.
- Object relation resolution preserves concurrent revisions and annotations.
- Encoding is stable across repeated runs and map insertion order.

### Parser and import tests

- property tests generate bounded valid and invalid envelopes;
- fuzz targets cover CBOR, CDDL validation boundaries, capabilities, drop codec, private envelope, and invite transitions;
- adversarial fixtures include huge declared lengths, excessive nesting, path-limit edges, duplicate IDs, truncated ciphertext, and decompression/expansion bombs;
- every pre-commit failure asserts a byte-for-byte unchanged destination store.

### Cryptographic trace tests

- deterministic test-only keys reproduce MLS and envelope traces;
- release code cannot enable deterministic key generation or crypto-debug features;
- removed-member, wrong-key, replay, stale-epoch, and concurrent-commit cases are mandatory;
- raw artifact disclosure snapshots are checked in as fixtures.

### Platform tests

- both native projects compile against generated bindings;
- both open the same public fixture and display the same verification result;
- key wrappers reject use under their configured locked/unauthorized conditions where testable;
- panic removes the wrapped group key before filesystem cleanup;
- no test or diagnostic truncates cryptographic identifiers.

## Work Units

### WU0 — Contracts and research (hours 0–2)

Artifacts:

- accepted research addendum;
- architecture and threat-model decisions;
- CDDL envelope and ten kind schemas;
- fixture manifest and disclosure-canary inventory.

Exit gate: every later test names the contract or fixture it proves.

### WU1 — Rust core and Willow proof (hours 2–5)

Artifacts:

- Rust workspace;
- deterministic object codec;
- current `willow25` adapter;
- communal/owned authority fixtures;
- conformance CLI and golden vectors.

Exit gate G2: Willow authority fixtures pass without assuming communal root authority.

### WU2 — Native bindings (hours 5–8)

Artifacts:

- UniFFI interface;
- Swift package/Xcode evidence target;
- Android Gradle/Compose evidence target;
- cross-language fixture tests.

Exit gate G1: Rust, Swift, and Kotlin agree on fixture bytes and verification results.

### WU3 — Public offline loop (hours 8–11)

Artifacts:

- explicit development `DropCodec` if required;
- quarantine parser and preview;
- atomic import receipt;
- four-screen evidence UI on both clients.

Exit gate G3: one signed public object round-trips between platform fixtures and verifies identically.

### WU4 — Private crypto laboratory (hours 11–14)

Artifacts:

- required OpenMLS traces;
- comparison HPKE epoch trace;
- padded opaque envelope fixture;
- invite lifecycle model;
- leakage matrix.

Exit gates G4 and G5: membership/fork behavior is reproducible and prohibited inner data is absent from the artifact.

### WU5 — Adversarial validation and decision (hours 14–16)

Artifacts:

- fuzz/property smoke results;
- bridge canary result;
- cross-platform build result;
- protocol maturity/dependency ledger;
- go/revise/stop report for kernel, newswire, groups, directory, and live sync.

Exit gate G6: private canaries do not occur in public output.

## Binary Gates

| Gate | Pass condition | Failure action |
| --- | --- | --- |
| G1 Shared core | Golden vectors and verification results match in Rust, Swift, Kotlin | Stop native feature work and repair ABI/core boundary. |
| G2 Willow authority | Communal author and owned curation tests pass | Revise namespace model. |
| G3 Public loop | Signed object completes atomic cross-platform round trip | Block newswire shell expansion. |
| G4 Group control | MLS lifecycle, fork, and offline trace reproduce | Keep groups research-only. |
| G5 Envelope privacy | Prohibited inner fields are absent and padding matches policy | Redesign envelope; do not demo private drops. |
| G6 Bridge isolation | No private canary reaches public output | Block publish-out. |

## Likely Post-Sprint Decisions

- **Go if G1–G3 pass:** shared object/provenance kernel, native bindings, newswire file loop.
- **Conditional on G4–G5 and external review:** private groups.
- **Separate evidence spike:** directory feeds and rendezvous.
- **Dependency/conformance project:** current Willow Drop Format.
- **Deferred:** live WTP, BLE, gateway, media, and local LLM.

## External Release Gates

Private groups cannot be called production-safe until an independent cryptographic reviewer approves the construction and implementation. Local transport cannot be called field-ready until tested on physical iOS and Android devices across the supported OS/hardware matrix. Object vocabulary cannot be frozen until organizer and mutual-aid practitioners exercise it in realistic scenarios.

These gates do not block the evidence sprint; they block later release claims.
