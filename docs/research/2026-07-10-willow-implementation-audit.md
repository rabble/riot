# Riot Willow Implementation Audit

Date: 2026-07-10
Status: Accepted research amendment; Phase 0A Revision 5 must incorporate these findings before WU1 Willow work

## Question

Does the Phase 0A design use the current Willow specification and Rust implementation correctly enough to produce meaningful evidence on native iOS and Android?

## Sources and Provenance

The old [`earthstar-project/willow-rs`](https://github.com/earthstar-project/willow-rs) GitHub repository is archived. Its final README and commit state that development moved to the canonical [`worm-blossom/willow_rs`](https://codeberg.org/worm-blossom/willow_rs) repository on 2025-10-23. GitHub is therefore useful historical evidence, not the current implementation source.

This audit used:

- the canonical Willow [Data Model](https://willowprotocol.org/specs/data-model/), [Meadowcap](https://willowprotocol.org/specs/meadowcap/), [Willow'25](https://willowprotocol.org/specs/willow25/), and [encoding](https://willowprotocol.org/specs/encodings/) specifications;
- `willow25` 0.5.0 and 0.6.0-alpha.3 source packages from crates.io;
- canonical `willow_rs` main commit `17b1a057c35a0da3710fdebb57804fad4a19cc3c` from 2026-07-06;
- the `willow25` and `bab_rs` changelogs;
- current canonical issues [#51, drop payload imports](https://codeberg.org/worm-blossom/willow_rs/issues/51), [#54, CI](https://codeberg.org/worm-blossom/willow_rs/issues/54), [#55, storage](https://codeberg.org/worm-blossom/willow_rs/issues/55), and [#56, optional storage/WASM](https://codeberg.org/worm-blossom/willow_rs/issues/56);
- [`Deln0r/willow-go`](https://github.com/Deln0r/willow-go) only as an independent interoperability cross-check, never as protocol authority.

## Executive Findings

1. **`willow25 =0.5.0` is not an acceptable digest pin.** It resolves `bab_rs 0.6.x`. The canonical `bab_rs` changelog says every version before 0.7 computed incorrect WILLIAM3 digests and that 0.8 “truly fixes” them. `willow25` 0.6.0-alpha.1 explicitly upgraded to `bab_rs 0.8` and regenerated its default digests.
2. **Pin `willow25 =0.6.0-alpha.3`, disable its default Drop Format feature, and force `bab_rs =0.8.1`.** Alpha.3 includes corrected digest parameters plus duplicate-entry fixes in `MemoryStore` and `PersistentStore`. The Cargo lockfile remains the evidence identity.
3. **Do not use the alpha Drop Format in Phase 0A.** It is new, its payload-import limitations remain open as issue #51, and the canonical repository still has no hosted CI (issue #54). Keep Riot's visibly non-interoperable evidence bundle.
4. **Use Willow's canonical encoders inside the Riot bundle.** The bundle should carry canonical `Entry` bytes, canonical `WriteCapability` bytes, the 64-byte subspace signature, and exact payload bytes. Riot CBOR frames those byte strings but does not redefine their field encodings.
5. **A valid Willow import is a join, not append-by-digest.** Prefix pruning and deterministic recency are the data model's defining semantics. The Riot store must separate the live Willow view from immutable local receipt/index facts.
6. **Each Willow store is single-namespace.** Riot's `EvidenceStore` may be a bounded map of namespace IDs to namespace-local Willow join states, but it must not call the aggregate map itself one Willow store.
7. **An accepted entry can be dominated immediately.** Import outcomes therefore need `Applied`, `Dominated`, and `AlreadyPresent`, not only `Inserted` and `AlreadyPresent`.
8. **Willow time and alert time are distinct.** Willow recommends microseconds of TAI since J2000 and the Rust type implements that conversion. Riot's signed alert fields use UTC Unix seconds for product interchange. Both views come from one clock snapshot and remain separately labelled.
9. **Production alert paths must avoid accidental prefix pruning.** Phase 0A uses four components: `objects`, `alert`, the 16-byte object ID, and the 16-byte revision ID. Intentional tombstones or mutable pointers are later schemas.
10. **The corrected dependency is mobile-compilable but heavy.** A minimal create/authorise/verify probe compiled with Rust 1.95.0 for `aarch64-apple-darwin`, `aarch64-apple-ios-sim`, `aarch64-apple-ios`, `aarch64-linux-android`, and `x86_64-linux-android`. Even with `drop_format` disabled, the umbrella crate pulls storage, `fjall`, and async filesystem dependencies. This is acceptable evidence debt, not an approved production closure.

## Correct Dependency Profile

```toml
willow25 = { version = "=0.6.0-alpha.3", default-features = false, features = ["std"] }
bab_rs = { version = "=0.8.1", default-features = false, features = ["william3"] }
```

The direct `bab_rs` pin forces the corrected patch release in the unified dependency graph. It does not remove storage features enabled by `willow25`'s internal dependency; `cargo tree -e features` must record that residual closure.

The release decision is evidence-specific:

- the alpha version is required because the latest stable version hashes payloads incorrectly;
- Drop Format remains disabled;
- Riot uses only entry, path, timestamp, canonical encoding, Meadowcap, and conformance-store APIs;
- any version or lockfile change invalidates the WILLIAM3 and canonical-byte vectors;
- production planning must re-evaluate a stable corrected release or a smaller upstream feature boundary.

## Exact Willow Semantics Phase 0A Must Prove

### Identity and authority

- A Willow'25 namespace ID and subspace ID are each 32-byte Ed25519 public-key encodings.
- A namespace is communal when the least significant bit of its namespace ID is zero.
- In a communal namespace, the namespace secret grants no root privilege.
- A zero-delegation communal write capability is valid for the named author's subspace.
- The authorisation token is the valid write capability plus a signature by the capability receiver over the canonical entry encoding.
- Verification must use safe conversion from `PossiblyAuthorisedEntry` to `AuthorisedEntry`; Riot never calls the unchecked conversion.

The public author identity exposed across FFI is:

```text
namespace_id: 32 bytes
subspace_id: 32 bytes
signing_key_id: the same public identity as subspace_id
namespace_kind: Communal
```

The ephemeral communal namespace secret may be generated by the upstream helper to obtain a communal public key, but it confers no authority and is discarded immediately. The subspace secret is the actual author signing secret and remains inside Rust until session zeroization.

### Payload digest and canonical bytes

- `payload_digest` is corrected WILLIAM3 over the exact alert CBOR payload.
- `object_digest` is Riot's SHA-256 over the same alert bytes for local artifact tooling.
- `entry_bytes` come from `willow25::Entry::encode` and must pass `decode_canonic` with no trailing bytes.
- `capability_bytes` come from `willow25::WriteCapability::encode` and must pass `decode_canonic` with no trailing bytes.
- the subspace signature is exactly 64 bytes and reconstructs `AuthorisationToken::new(capability, signature)`.
- payload length must equal the entry length, and corrected WILLIAM3(payload) must equal the entry payload digest before authorisation is accepted.

Riot's `entry_digest` is domain-separated so concatenation is unambiguous:

```text
SHA-256(
  "riot/entry-digest/v1" ||
  u32be(len(entry_bytes)) || entry_bytes ||
  u32be(len(capability_bytes)) || capability_bytes ||
  signature_bytes
)
```

### Time and path

One `ClockSnapshot` provides:

- `willow_timestamp`: `willow25::Timestamp`, microseconds TAI since J2000, used for join recency;
- `created_at`, `valid_from`, and `expires_at`: UTC Unix seconds inside the signed alert payload;
- local clock uncertainty for preview provenance.

The evidence path is:

```text
[b"objects", b"alert", object_id_16_bytes, revision_id_16_bytes]
```

IDs are binary components, not truncated display strings. This path keeps immutable revisions unrelated by prefix. A later schema may add mutable indices or intentional pruning markers, but Phase 0A does not.

### Store join

`EvidenceStore` is a bounded Riot container:

```text
EvidenceStore
├── namespace_views: NamespaceId -> NamespaceJoinState
├── seen_entry_index: entry_digest -> first receipt facts
└── receipts
```

Each `NamespaceJoinState` contains only authorised entries for one namespace and obeys the Willow join:

1. Union existing live entries with the candidate.
2. Within one subspace, remove an entry when another strictly newer entry lies at a prefix of its path.
3. For equal subspace/path/timestamp, retain the greatest WILLIAM3 payload digest.
4. If those are also equal, retain the greatest payload length.

The implementation uses a bounded copy-on-write snapshot for logical atomicity. `willow25::storage::MemoryStore` from alpha.3 is a conformance oracle in tests, not the FFI store: it uses `Rc`, is not the session's thread-safe transactional model, and does not enforce Riot's resource ceilings.

Required algebraic and permutation evidence:

- join is commutative, associative, and idempotent;
- distinct namespaces and subspaces do not prune one another;
- a newer prefix prunes older descendants;
- an older prefix coexists with newer descendants;
- equal-coordinate ties resolve digest then length;
- every tested insertion permutation matches the alpha.3 `MemoryStore` live view.

### Import dispositions

Per selected entry:

```text
Applied {
  entry_id,
  pruned_entry_digests[]
}

Dominated {
  dominating_entry_digests[]
}

AlreadyPresent {
  entry_id,
  insertion_receipt_id
}
```

`Applied` means the entry is in the resulting live namespace view. `Dominated` means it was valid and accepted into local receipt/index history but is not in the resulting live view. `AlreadyPresent` is a replay of an entry digest already accepted locally.

A commit containing a previously unseen `Dominated` entry changes the seen index and creates a receipt, so it increments the Riot store generation. A duplicate-only commit remains `NoChanges` and creates no second receipt.

## Evidence Bundle V1 Refinement

The deterministic CBOR outer frame contains, per item:

```text
entry_bytes: bytes
capability_bytes: bytes
signature_bytes: bytes .size 64
payload_bytes: bytes
```

Decode order is bounded outer CBOR, canonical Entry, canonical capability, fixed signature, payload length/digest, Meadowcap authorisation, Riot schema, then preview policy. No untrusted entry reaches join staging before all prior checks pass.

This is still not Willow Drop Format. The current alpha Drop Format is useful only for a later comparison fixture once issue #51 and upstream test posture improve.

## WU0 Impact

The existing WU0 environment and native-platform preflight remain valid, but its dependency portion is **REVISE** rather than PASS until all of the following occur:

1. update the workspace pin and fixture manifest;
2. regenerate `Cargo.lock` and its recorded SHA-256;
3. add corrected WILLIAM3 empty, short, partial-block, and multi-block golden vectors;
4. rerun the five-target compile probe and `cargo tree -e features`;
5. update the WU0 report with the reason stable 0.5.0 was rejected.

No alert model work needs to be discarded. Willow entry, bundle, and store code must not be implemented against 0.5.0.
