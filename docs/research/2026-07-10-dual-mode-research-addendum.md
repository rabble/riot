# Riot Dual-Mode Research Addendum

Date: 2026-07-10
Status: Accepted research basis for the Phase 0 evidence-sprint sequence

## Purpose

This addendum tests the assumptions in `docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md` against the current Willow specifications and implementations, modern group-encryption work, emergency-information standards, and current iOS and Android platform APIs.

The dual-mode product decision remains sound: Riot should keep open publishing and private groups in separate stores, with content crossing only through an explicit bridge. The research changes several lower-level claims and narrows the first implementation milestone to executable evidence.

## Executive Findings

1. **Willow is a family of specifications and implementations at different maturity levels.** Meadowcap and the Data Model are Final. The current Drop Format is Candidate. Willow'25 and Confidential Sync are Proposals. WTP is a Sketch. The Rust website lags the repository: stable `willow25` 0.5.0 contains storage, and 0.6.0 alpha contains Drop Format, but current upstream issues still cover Drop imports, storage efficiency, and missing CI. The focused implementation audit is authoritative for Phase 0A pins and join behavior.
2. **Encrypted Willow is not a complete private-group construction.** It does not hide namespace IDs, subspace IDs, timestamps, ciphertext lengths, path-prefix equality, or Meadowcap authorization tokens. Group membership epochs, key rotation, invitations, storage encryption, padding, and traffic-analysis claims remain application responsibilities.
3. **A communal namespace cannot have a creator-controlled root curation path.** In Meadowcap, each communal author controls their own subspace. Curation therefore needs one or more owned companion namespaces.
4. **A globally writable directory cannot be guaranteed tiny.** Per-record size caps do not bound record count or Sybil writers. Discovery should use plural signed directory feeds, expiry, and local budgets instead of one canonical global store.
5. **Meadowcap delegates authority; it does not merge several people into one signer.** A publication namespace can be the stable collective identity while delegated, purpose-specific member keys remain visible as the actual signers.
6. **The private plane should use an ordered membership control plane and a mergeable data plane.** MLS is the production candidate for group membership epochs. Willow remains the group content model. Private drops should encrypt and pad a complete Willow drop as an opaque blob before carriage.
7. **The original Swift-only prototype plan no longer fits the selected platform goal.** Riot will target iOS and Android together with native SwiftUI and Jetpack Compose shells around a shared Rust core exposed through UniFFI.
8. **The next milestone is a 16 agent-hour public-kernel evidence sprint.** It must produce an executable two-way native-runtime artifact handoff, not production-security claims or polished product UI. Private-group cryptography and the bridge follow only as separately reviewed, separately budgeted evidence sprints.

## Willow Maturity Matrix

| Layer | Current published status | Implementation implication for Riot |
| --- | --- | --- |
| Willow Data Model | Main specification | Use through the current Rust `willow25` crate and verify merge laws locally. |
| Meadowcap | Final (2025-11-21) | Use for communal/owned authority experiments and write-capability delegation. |
| Drop Format | Candidate (per the Willow change log) | Alpha Rust implementation exists, but payload-import limitations remain open. Keep a `DropCodec` boundary and do not claim interoperability without authoritative vectors and a separate conformance gate. |
| Willow'25 | Proposal | Pin `willow25 0.6.0-alpha.3` plus corrected `bab_rs 0.8.1`; stable 0.5.0 computes obsolete WILLIAM3 digests. Retain fixtures across every upgrade. |
| Confidential Sync | Proposal (2025-11-21) | Defer. It is not required for file-based evidence or member-only local merge. |
| WTP | Sketch (2026-01-29) | Defer live WTP. Exercise exchange over files and in-memory ordered byte streams first. |
| Rust implementation | Data Model, Meadowcap, storage; alpha Drop Format | Use canonical entry/capability encoders, but keep Riot's bounded transactional store and evidence codec. Treat upstream storage/Drop as conformance inputs, not production-ready solved dependencies. |

Sources:

- [Willow specifications index](https://willowprotocol.org/specs/)
- [Meadowcap](https://willowprotocol.org/specs/meadowcap/)
- [Willow Drop Format](https://willowprotocol.org/specs/drop-format/)
- [Willow'25](https://willowprotocol.org/specs/willow25/)
- [Willow Confidential Sync](https://willowprotocol.org/specs/confidential-sync/)
- [Willow Transfer Protocol](https://willowprotocol.org/specs/wtp/)
- [Willow changes and implementation status](https://willowprotocol.org/more/changes/)
- [Willow in Rust](https://willowprotocol.org/rust/)
- [`willow25` Rust documentation](https://docs.rs/willow25/latest/willow25/)
- [`willow_rs` canonical repository](https://codeberg.org/worm-blossom/willow_rs)
- [`bab_rs` changelog](https://codeberg.org/worm-blossom/bab_rs/src/branch/main/CHANGELOG.md)
- `docs/research/2026-07-10-willow-implementation-audit.md`

## Open-Plane Corrections

### Open submissions and curation

An open submission space remains a communal namespace. Each author writes in their own subspace. The raw feed remains readable without accepting an editorial authority.

Curation moves to an owned companion namespace. It contains signed annotations such as `features`, `corrects`, `disputes`, and `updates-status` that target immutable public object revisions. Multiple curation namespaces may cover the same open submission space. A reader chooses which lenses to apply.

A publication is also an owned namespace, but it contains standalone copied and re-signed output rather than only annotations. A collective may operate an open submission space, one or more curation lenses, and a publication as linked but independent namespaces.

This structure follows Meadowcap's actual distinction: communal authority begins with a subspace key, while owned namespaces begin with the namespace key and support delegated authority.

### Collective identity

The publication namespace key is the collective's durable public identity. Editors receive restricted write capabilities and sign with purpose-specific subspace keys that are not reused for private groups or unrelated public activity.

Clients display both facts:

- published by the collective namespace;
- signed by the delegated publishing key.

Riot does not share a collective root key between editors and does not imply that a capability lets one key impersonate another.

### Discovery

Riot standardizes a directory record schema, not one canonical globally writable directory namespace. Discovery sources are signed directory feeds, each represented by an owned public namespace. The app may ship seed feeds, but users can add, remove, and share alternatives.

Each device applies local retention rules:

- maximum bytes and record count per feed;
- mandatory expiry;
- bounded region and time windows;
- signer/feed trust preferences;
- no automatic payload download from a pointer.

This is an explicit change from the original global communal directory. It preserves plural discovery without promising a global permissionless store that is simultaneously tiny and abuse-resistant.

## Private-Plane Findings

### What Encrypted Willow exposes

Willow's encryption analysis states that:

- payload lengths and digests remain observable, although padding can reduce plaintext-length leakage;
- timestamps remain plaintext because store joins require numeric comparison;
- namespace IDs and subspace IDs remain stable and visible;
- encrypted paths must preserve equal prefixes;
- Meadowcap authorization tokens cannot be encrypted while remaining verifiable to an untrusted peer.

Source: [Encrypted Willow](https://willowprotocol.org/specs/e2e/).

Therefore, Encrypted Willow alone does not justify the original claim that an intercepted store reveals no group, member-count, or activity metadata.

### Revised construction

The first private design uses two layers:

1. **MLS control plane.** Ordered epochs manage member add, remove, update, and recovery. A separately designed Phase 0B evidence sprint will evaluate OpenMLS, including concurrent commits and offline catch-up; the 16 agent-hour Phase 0A public-kernel sprint does not implement or validate this layer.
2. **Willow data plane.** Signed group objects remain normal Willow entries. A member decrypts a complete private-drop envelope, validates it, and merges the inner entries locally.

The private-drop envelope hides all inner Willow metadata from non-member carriers:

- namespace and subspace IDs;
- paths and timestamps;
- capabilities;
- entries and payloads;
- group membership control messages.

The outer artifact is padded into documented size buckets and authenticated with standard AEAD. Riot will not invent new cryptographic primitives.

MLS is a suitable candidate because it standardizes asynchronous group key establishment, membership changes, forward secrecy, and post-compromise security. It does not solve Riot's delivery, conflict, metadata, or application-authorization policies. The application must define canonical handling for concurrent commits and must test how long-offline members recover.

Sources:

- [RFC 9420: Messaging Layer Security](https://www.rfc-editor.org/info/rfc9420/)
- [RFC 9750: MLS Architecture](https://datatracker.ietf.org/doc/html/rfc9750)
- [OpenMLS](https://github.com/openmls/openmls)

### Invite lifecycle

An invite file cannot be intrinsically single-use or revocable while fully offline; copies are indistinguishable until a group member evaluates them. Riot therefore models invitation as state:

1. An authorized sponsor signs a random, expiring voucher with a single-redemption policy.
2. The invitee binds a one-time MLS KeyPackage to proof of the voucher secret.
3. An authorized member checks the latest revocation and redemption state, then commits the add operation.
4. The invitee receives the MLS Welcome and encrypted application role/capability state.

In-person QR/NFC can perform all four steps in one session. A portable artifact performs the first two steps before the invitee next contacts a current member. Concurrent redemption is a modeled conflict: one canonical membership commit wins and the other request requires a new voucher.

### Privacy claims after revision

The target construction can claim, subject to implementation review:

- group content confidentiality;
- authenticated authorship inside the group;
- no plaintext group identifiers or Willow metadata in a private drop;
- unlinkable public and private persona keys;
- member removal protecting future epochs;
- cryptographic per-group wipe by destroying a wrapped local storage key.

It does not claim to hide:

- the existence of an artifact;
- transfer time, channel, endpoints, or frequency;
- padded size bucket;
- membership from other members;
- past content already received or exported by a removed member;
- current content from an attacker controlling an unlocked member device;
- all traffic-analysis signals.

### At-rest key handling

Each device has a local wrapping key protected by the platform's secure key facility. Per-group storage keys are independently wrapped so panic wipe can destroy one group's key before background file cleanup.

- iOS uses the most restrictive practical Keychain accessibility, normally `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`. Ed25519 keys are not assumed to be Secure Enclave keys because Apple's documented Secure Enclave signing support is not Ed25519.
- Android uses a non-exportable Android Keystore key, preferring hardware-backed/StrongBox isolation when available.

Sources:

- [Apple: Restricting keychain item accessibility](https://developer.apple.com/documentation/security/restricting-keychain-item-accessibility)
- [Apple Secure Enclave algorithms](https://developer.apple.com/documentation/cryptokit/secureenclave)
- [Android Keystore](https://developer.android.com/privacy-and-security/keystore)

## Object Vocabulary Addendum

### Signed envelope

Every structured payload has a deterministic signed-content envelope:

- schema identifier and version;
- stable random object ID;
- unique revision ID;
- created time;
- optional validity start and expiry;
- language;
- typed body;
- signed relations to other revisions;
- author-provided source claims;
- required `ai_assisted` boolean.

Signer ID, namespace, capability, payload digest, and Willow timestamp belong to the Willow entry rather than being duplicated inside the payload. Import channel, first-seen time, local trust decisions, and bridge history belong to a separate receipt so local observations cannot be mistaken for the author's signed claim.

Signed payloads use deterministic CBOR with CDDL schemas. Gateways expose a lossless JSON projection. Source: [RFC 8949: CBOR](https://www.rfc-editor.org/rfc/rfc8949.html).

### Core kinds

| Kind | Purpose | Standards influence |
| --- | --- | --- |
| `alert` | Urgent instruction with urgency, severity, certainty, area, and required expiry | OASIS CAP |
| `observation` | A signer's bounded statement of what was observed, where, when, and how | EDXL-SitRep |
| `event` | Scheduled activity with start, end, status, and optional location | iCalendar `VEVENT` |
| `resource` | A service, facility, supply, access method, availability, and location | Open Referral HSDS |
| `request` | A need for an item/service with quantity, priority, area, and expiry | EDXL-RM |
| `offer` | Available capacity with constraints and validity | EDXL-RM |
| `commitment` | A signed link between a request and offer; not proof of fulfillment | EDXL-RM |
| `task` | Dispatchable work whose assignments and state changes remain signed | Mutual-aid workflow extension |
| `document` | Bulletin, guide, checklist, or other durable reference | Riot/SneakerWeb profile |
| `annotation` | Correction, dispute, translation, feature, verification, fulfillment, or lifecycle statement | Riot provenance model |

Profiles preserve useful product vocabulary without multiplying wire kinds:

- `route_status` and `field_report` are observation profiles;
- `checklist` and `announcement` are document profiles;
- `need` is the UI label for a request;
- correction, translation, and curation are annotation relations;
- check-in is a groups-only profile with mandatory expiry and location minimization.

Global `confidence` is removed. `certainty` belongs to alerts and other object-specific epistemic fields belong in their schemas.

Sources:

- [OASIS Common Alerting Protocol 1.2](https://docs.oasis-open.org/emergency/cap/v1.2/pr01/CAP-v1.2-PR01.html)
- [OASIS EDXL Situation Reporting](https://docs.oasis-open.org/emergency/edxl-sitrep/v1.0/edxl-sitrep-v1.0.html)
- [OASIS EDXL Resource Messaging](https://docs.oasis-open.org/emergency/edxl-rm/v1.0/os/EDXL-RM-v1.0-OS.html)
- [Open Referral HSDS](https://docs.openreferral.org/en/3.1/hsds/overview.html)
- [RFC 5545: iCalendar](https://www.rfc-editor.org/info/rfc5545/)
- [RFC 7946: GeoJSON](https://www.rfc-editor.org/info/rfc7946/)

## Bridge Findings

The bridge is a typed declassification boundary, not a storage copy API.

### Group to newswire

The group module produces a draft containing only explicitly allowlisted content fields plus a mandatory AI-assistance taint. Group IDs, private signer IDs, capabilities, private relations, receipts, and source paths never enter the draft. A user reviews source and location fields, chooses a public destination, and creates a new public object with a new ID and public signature.

### Newswire to group

The group stores the original public Willow entry, payload, signature, capability, and namespace provenance intact inside the encrypted group store. The clipping member adds a separate private annotation. The original remains independently verifiable.

### Group rendezvous

Rendezvous is not considered an ordinary bridge action. Its format remains a separate privacy/abuse research item. A record can be pseudorandom and fixed-size without making its publication timing, feed, or transport metadata invisible.

## Platform and Transport Findings

The selected client architecture is:

- native SwiftUI on iOS;
- native Jetpack Compose on Android;
- a shared Rust protocol/crypto/object core;
- generated Swift and Kotlin bindings through UniFFI.

Sources:

- [UniFFI user guide](https://mozilla.github.io/uniffi-rs/)
- [OpenMLS platform status](https://github.com/openmls/openmls)

The exchange ladder is:

1. files and operating-system share surfaces;
2. authenticated streams on a common LAN using Apple's Network framework and Android DNS-SD/NSD;
3. a Wi-Fi Aware interoperability spike for infrastructure-free transfer on supported modern devices;
4. BLE, relay, and other adapters only after the byte protocol and resource controls are proven.

MultipeerConnectivity is removed from the forward architecture because Apple now marks it deprecated and recommends Network framework for peer-to-peer networking. Apple peer-to-peer Wi-Fi is Apple-only; Wi-Fi Aware is the standards-based cross-platform candidate on newer hardware.

Sources:

- [Apple: Choosing the right networking API](https://developer.apple.com/documentation/technotes/tn3151-choosing-the-right-networking-api)
- [Apple Wi-Fi Aware](https://developer.apple.com/documentation/WiFiAware)
- [Android Network Service Discovery](https://developer.android.com/develop/connectivity/wifi/use-nsd)
- [Android Wi-Fi Aware](https://developer.android.com/develop/connectivity/wifi/wifi-aware)

## Revised Build Order

### Phase 0 evidence-sprint sequence

- **Phase 0A — public kernel:** prove one operational alert, a communal Willow authority path and denial, bounded preview-first atomic import, generated Swift/Kotlin bindings, and two-way iOS Simulator↔Android emulator artifact handoff.
- **Phase 0B — private groups:** under a separately reviewed threat model and budget, evaluate MLS authorization/transitions, the opaque private envelope, invite state, persistence limits, and adversarial traces.
- **Phase 0C — declassification:** under a separately reviewed information-flow contract and budget, prove the deliberate bridge and differential noninterference.

### First implementation slice

Build the shared kernel and newswire file loop only if the evidence gates pass. A development drop codec must be visibly non-interoperable and must sit behind the same interface intended for current Willow Drop Format support.

### Private implementation slice

Proceed only if the MLS/mobile/fork and envelope-disclosure gates pass, followed by independent cryptographic design review. A failed private gate blocks group release; it does not justify silently shipping the weaker comparison construction.

### Later slices

- plural directory feeds and rendezvous abuse/privacy spike;
- current Willow Drop Format conformance or adoption of an upstream implementation;
- live local transport and WTP once the protocol is sufficiently stable;
- gateway and public web onboarding;
- local LLM with per-module process/context isolation.

## Remaining External Gates

The following cannot be honestly completed by agentic coding alone:

- independent cryptographic review before private groups are called production-safe;
- physical iOS/Android transfer testing across supported OS/hardware combinations;
- organizer and mutual-aid practitioner validation of vocabulary and workflows;
- usability exercises under time pressure, low battery, intermittent contact, and device seizure scenarios;
- coordination with Willow maintainers on current Drop Format vectors and implementation timing.

These are release gates, not reasons to delay the relevant separately scoped evidence sprint.
