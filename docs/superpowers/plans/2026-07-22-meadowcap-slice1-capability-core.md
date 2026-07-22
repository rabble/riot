# Meadowcap Slice 1: Capability Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the focused `riot-core::meadowcap` module named in the design's "Meadowcap core" section: a single home that wraps the pinned `willow25` capability implementation and owns all protocol-level operations for Slice 1 — canonical encode/decode of read **and** write capabilities, communal/owned creation, attenuating delegation, typed inspection (access mode, receiver, namespace, granted area, chain depth), signature/coverage verification, capability fingerprints, structural ceilings, and stable golden conformance fixtures. It returns typed facts and stable rejection codes to consumers; it contains no `Moderator`, `AppApprover`, admission, sync, governance, or FFI concepts.

**Architecture:** New crate module `crates/riot-core/src/meadowcap/` with submodules `mod` (types, errors, ceilings, re-exports), `create`, `codec`, `delegate`, `inspect`, `verify`, `fingerprint`. It builds **on top of** the existing `willow` module rather than duplicating it: it re-exports and calls `willow::verify_entry`, `willow::encode_capability`, `willow::decode_capability_canonic`, and `willow::tai_j2000_micros_from_unix_seconds`, and reuses the private `willow::decode_canonic_exact` (promoted to `pub(crate)`). Read capabilities are entirely new (Riot has never minted one). Existing owned-write-cap minting/delegation in `willow/masthead.rs` (composite-site Unit 0, landed) is a legitimate *policy caller* of the new core mechanism; it is **not** rewritten in this slice.

**Tech Stack:** Rust 2021, `willow25 = "=0.6.0-alpha.3"` (alpha pin is load-bearing; see below), `meadowcap` (re-exported through willow25), `sha2` (already a `riot-core` dependency, used by `willow/digest.rs`), `pollster` + `ufotofu` (already `riot-core` deps). **No new crate dependencies are introduced by this slice**, so `fixtures/manifest.json`'s `cargo_lock_sha256` does not change. Fixtures and tests use `willow25` with the `dev` feature, already a `riot-core` dev-dependency.

---

## Load-bearing project constraints (read before coding)

1. **Alpha pins are load-bearing.** `willow25 = "=0.6.0-alpha.3"` and `bab_rs = "=0.8.1"` are pinned because stable releases compute incorrect WILLIAM3 digests (`docs/research/2026-07-10-willow-implementation-audit.md`). `cargo xtask validate-contracts` fails if the resolved version drifts (`crates/xtask/src/main.rs:592`). Do not bump either crate.
2. **No new dependencies.** Everything this slice needs (`sha2`, `pollster`, `ufotofu`, `willow25`, `willow25/dev` in dev-deps, `serde_json` in dev-deps) is already resolved in `Cargo.lock`. Because no dep edge is added, `cargo_lock_sha256` in `fixtures/manifest.json` is unchanged. **If a task ever adds a dependency, it must also run `cargo xtask validate-contracts`, read the printed `actual` hash, and update `cargo_lock_sha256`.**
3. **Willow entry/area timestamps are TAI/J2000 MICROSECONDS, not Unix seconds.** A `TimeRange` built from raw Unix seconds authorizes essentially zero real entries (Riot shipped and fixed exactly this bug: #72 → #73). Every `TimeRange` in production code, tests, and fixtures must convert via `willow::tai_j2000_micros_from_unix_seconds`. Task 8 pins this as an explicit negative test.
4. **Shared checkout — pathspec commits only.** Multiple agents share this working tree. Every commit is `git add <explicit paths>` — never `git add -A`, never `git add .`. Never `--no-verify`. Never `git stash` (the stash stack is global to the checkout).
5. **Reuse the canonical gate.** Verification MUST route through `willow::verify_entry` (the checked `PossiblyAuthorisedEntry` conversion). Do not hand-roll a second signature-verification path — that is a recurring defect class in this repo.
6. **Scope discipline — Slice 1 only.** Codec, creation, delegation, inspection, verification, fingerprints, conformance fixtures. Explicitly OUT of scope (later slices, do not build): the admission engine's inspect/plan/commit rewrite (Slice 3), protected-sync handshake / PIO / relative confidential capability encoding / `ProtectedDrop` (Slice 4), the governance ledger / `GovernanceRecordV1` / action-receipt / revocation cutoff machinery (Slice 2), the `AuthorityRepository` / secure vault / recovery / migration (Slice 5), manifest V2 / app broker (Slice 6), and **all FFI/UniFFI surface and UI** (Slices 5–7). If a design paragraph in the "Meadowcap core" list names something in those slices, it is noted out-of-scope here, not implemented.

---

## willow25 API inventory

Source read from the pinned crate at
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/willow25-0.6.0-alpha.3/`
(module `src/authorisation/`, `src/groupings/area.rs`) and cross-checked against current Riot usage in `crates/riot-core/src/willow/{mod.rs,masthead.rs,owned.rs,identity.rs,clock.rs,digest.rs}`.

### What the pinned crate provides (reuse — do not reinvent)

**Capability types** — `willow25::authorisation::{WriteCapability, ReadCapability, AuthorisationToken, AuthorisedEntry, PossiblyAuthorisedEntry}`. `WriteCapability` and `ReadCapability` are distinct thin wrappers over `meadowcap::WriteCapability<…>` / `meadowcap::ReadCapability<…>`; the static type discriminates access mode, and `genesis().access_mode()` returns `AccessMode::Write`/`AccessMode::Read` for cross-checking.

**Creation** (identical shape for read and write):
- `WriteCapability::new_communal(namespace_key: NamespaceId, user_key: SubspaceId) -> Self` — granted area = `Area::new_subspace_area(user_key)`, zero delegations, `is_owned() == false`.
- `WriteCapability::new_owned<K>(keypair: &K, user_key: SubspaceId) -> Self where K: Signer<NamespaceSignature> + Keypair<VerifyingKey = NamespaceId>` — granted area = `Area::full()`, zero delegations, `is_owned() == true`. `NamespaceSecret` implements the required `Signer`/`Keypair` bounds (proven by `masthead.rs::owner_write_capability`).
- `ReadCapability::new_communal(...)`, `ReadCapability::new_owned(...)` — same signatures, same semantics, `genesis().access_mode() == AccessMode::Read`.

**Delegation (attenuation only)** — `cap.try_delegate<K>(&mut self, keypair: &K, new_area: Area, new_receiver: SubspaceId) -> Result<(), meadowcap::raw::InvalidCapability>` where `K: Signer<SubspaceSignature> + Keypair<VerifyingKey = SubspaceId>`. Returns `Err` if `new_area` is not contained in the current granted area (authority expansion) or if `keypair`'s public key is not the current receiver (wrong signer). `SubspaceSecret` satisfies the bounds (proven by `masthead.rs::delegate_section`). A panicking `delegate(...)` variant also exists — **use only `try_delegate`**.

**Canonical encoding / decoding** — both cap types implement `Encodable`, `EncodableKnownLength` (`len_of_encoding() -> usize`, used for the byte ceiling), `Decodable`, and `DecodableCanonic`. Riot already wraps the **write** path: `willow::encode_capability(&WriteCapability) -> Vec<u8>` and `willow::decode_capability_canonic(&[u8]) -> Result<WriteCapability, WillowError>`, both built on `pollster::block_on(cap.new_vec_storing_encoding())` and the private `decode_canonic_exact::<T>` (rejects trailing bytes). The **read** path has no Riot wrapper yet.

**Receiver / granted-area / structure inspection:**
- `cap.receiver() -> &SubspaceId` — the capability receiver, **distinct** from any entry `subspace_id` (design requirement: return them as separate facts).
- `cap.granted_namespace() -> &NamespaceId`.
- `cap.granted_area() -> Area` (by value) / `granted_area_ref() -> Option<&Area>` (efficient; `None` before any delegation).
- `cap.is_owned() -> bool`.
- `cap.delegations() -> &[Delegation]` — chain depth = `.len()`.
- `cap.genesis() -> &Genesis` with `access_mode() -> AccessMode`, `namespace_key() -> &NamespaceId`, `user_key() -> &SubspaceId`.
- `cap.includes<T: Namespaced + Coordinatelike>(&t) -> bool` and `cap.includes_area(&Area) -> bool` — the coverage primitives (used for "is a read request covered by this read capability").

**Areas** (`willow25::prelude::Area`, `TimeRange`, `Timestamp`, `Path`):
- `Area::new(subspace: Option<SubspaceId>, path: Path, times: TimeRange)`, `Area::full()`, `Area::new_subspace_area(SubspaceId)`.
- Accessors `area.subspace() -> Option<&SubspaceId>`, `area.path() -> &Path`, `area.times() -> &TimeRange`.
- `TimeRange::new(start: Timestamp, end: Option<Timestamp>)` (proven by `masthead.rs` tests: `TimeRange::new(0u64.into(), Some(u64::MAX.into()))`); `Timestamp: From<u64>`, value = TAI/J2000 microseconds.

**Verification / authorisation:**
- `Entry::into_authorised_entry(&cap: &WriteCapability, &secret: &SubspaceSecret) -> Result<AuthorisedEntry, _>` — mint.
- `PossiblyAuthorisedEntry::new(entry, token).into_authorised_entry()` — the **checked** verify path, already wrapped as `willow::verify_entry(&Entry, &AuthorisationToken) -> bool`. **Reuse this.**
- `AuthorisationToken::{new(cap, sig), capability() -> &WriteCapability, signature() -> &SubspaceSignature, into_parts(), does_authorise<E>(&E) -> bool}`.

**Riot helpers already present (reuse):** `willow::tai_j2000_micros_from_unix_seconds(u64) -> Result<u64, WillowError>` (the microsecond converter), `willow::{encode_capability, decode_capability_canonic, verify_entry, decode_entry_canonic}`, and the `sha2`-based domain-separated digest pattern in `willow/digest.rs`.

### Identified upstream gaps (things the design needs that the pinned crate lacks — this slice fills them Riot-side)

1. **No capability fingerprint.** The pinned crate exposes no fingerprint. The design pins the exact preimage `SHA-256("riot/meadowcap-fingerprint/v1" || canonical_capability_bytes)` with **no length prefix** (deliberately different from `willow/digest.rs::entry_id`, which prepends a `u32` length). Implemented in `meadowcap/fingerprint.rs`. This fingerprint is the join key the governance ledger (Slice 2) will use, so its bytes must match the spec exactly.
2. **No structural ceilings.** `delegations().len()` and `len_of_encoding()` are exposed, but willow25 enforces no depth or byte limit. The design pins a max delegation depth of 16 and 64 KiB of encoded capability bytes; Riot must gate these **before** recursive verification. Implemented in `meadowcap/codec.rs` as `decode_*_bounded`.
3. **No Riot read-capability codec/creation wrappers.** Read caps exist upstream but Riot has never created, encoded, decoded, or inspected one. New in `meadowcap/{create,codec,inspect}.rs`.
4. **No stable rejection-code taxonomy for capabilities.** `try_delegate`/decode return `InvalidCapability`/`Blame` (opaque). The design requires stable, testable, non-secret diagnostics. New `MeadowcapError` enum in `meadowcap/mod.rs` with the variants Slice 1 can actually *produce*: `Malformed`, `TrailingBytes`, `ChainTooDeep`, `CapabilityTooLarge`, `AuthorityExpanding`, `ReceiverMismatch`. Two design-named conditions fold into `Malformed` by construction because willow25's canonical decode already rejects them and Slice 1 has no post-decode producer for a distinct code: **non-canonical encodings** (rejected by `decode_canonic`) and **wrong-access-mode bytes** (read bytes to the write decoder or vice versa, rejected by the wrapper's access-mode check). Both folds are documented on the enum and pinned by dedicated Task 9 negative tests; wrong-namespace is a read/write *authorisation* failure (a boolean-false from `token_authorises_entry`/`read_request_covered`), not a decode code, and is tested in Task 6. Declaring an unreachable `NonCanonical`/`WrongMode` code was rejected as dead-code. (The *fuller* admission taxonomy — stale-policy, revoked, missing-parent, etc. — belongs to Slices 2–3 and is out of scope here.)
5. **No Unix-seconds bridge in willow25.** `TimeRange` is micros-only; the design's UI-facing expiries are wall-clock. Riot's `tai_j2000_micros_from_unix_seconds` is the only correct bridge; the plan mandates it everywhere a `TimeRange` is built.

**No blocking upstream gaps.** Every Slice-1 requirement is satisfiable against `willow25 0.6.0-alpha.3` as pinned; nothing here requires an upstream change or a version bump. The confidentiality-preserving *relative* read-capability encoding (`PriorCapEntryPair`, PIO) exists upstream but is Slice 4 and out of scope.

---

## File Structure

Created:

| Path | Responsibility |
| --- | --- |
| `crates/riot-core/src/meadowcap/mod.rs` | Module root: `MeadowcapError` (stable codes), `AccessMode`, `CapabilityKind`, ceilings consts (`MAX_DELEGATION_DEPTH`, `MAX_CAPABILITY_ENCODED_BYTES`), submodule declarations, and the public re-export surface (incl. re-exporting `ReadCapability`, `WriteCapability`, and `willow::verify_entry`). |
| `crates/riot-core/src/meadowcap/create.rs` | Communal/owned read+write capability creation wrappers over willow25 (owned takes `&NamespaceSecret`, communal takes `NamespaceId`). |
| `crates/riot-core/src/meadowcap/codec.rs` | Canonical encode/decode for read caps; `decode_write_capability_bounded` / `decode_read_capability_bounded` applying depth + byte ceilings; reuses `willow::decode_canonic_exact` and `willow::encode_capability`. |
| `crates/riot-core/src/meadowcap/delegate.rs` | `delegate_write` / `delegate_read` attenuation primitives mapping `InvalidCapability` to stable `MeadowcapError`. |
| `crates/riot-core/src/meadowcap/inspect.rs` | `CapabilitySummary` typed facts (kind, access mode, granted namespace, receiver, granted area, chain depth, fingerprint) with receiver kept separate from entry subspace; `summarise_write` / `summarise_read`. |
| `crates/riot-core/src/meadowcap/verify.rs` | Re-export of `willow::verify_entry`; `token_authorises_entry`; `read_request_covered` (read-cap coverage predicate). |
| `crates/riot-core/src/meadowcap/fingerprint.rs` | `CapabilityFingerprint` and exact-preimage `write_capability_fingerprint` / `read_capability_fingerprint`. |
| `crates/riot-core/tests/meadowcap_conformance.rs` | Integration home for golden byte vectors, seeded generative attenuation tests, and negative-form fixtures; writes/asserts `fixtures/willow/meadowcap-vectors.json`. |
| `fixtures/willow/meadowcap-vectors.json` | Golden canonical capability encodings + fingerprints (deterministic seeds). Contract-pinned by `meadowcap_vectors_sha256`. |

Modified:

| Path | Change |
| --- | --- |
| `crates/riot-core/src/lib.rs` | Add `pub mod meadowcap;`. |
| `crates/riot-core/src/willow/mod.rs` | Promote `decode_canonic_exact` from private `fn` to `pub(crate) fn` so `meadowcap::codec` reuses it instead of duplicating the trailing-byte guard. No behavior change. |
| `fixtures/manifest.json` | Add `env.meadowcap_vectors_sha256` (SHA-256 of the new vectors file), mirroring `william3_vectors_sha256`. |
| `crates/xtask/src/main.rs` | Add a `meadowcap_vectors_sha256` check mirroring the `william3_vectors_sha256` block (~line 641) so `validate-contracts` pins the new fixture. |

---

## Tasks

### Task 0 — Scaffold the `meadowcap` module, error taxonomy, and ceilings

**Files:** Create `crates/riot-core/src/meadowcap/mod.rs`; Modify `crates/riot-core/src/lib.rs`.

- [ ] **Write the failing test.** In `crates/riot-core/src/meadowcap/mod.rs`:

```rust
//! Meadowcap capability core (Slice 1). Wraps the pinned `willow25` Meadowcap
//! implementation and owns all protocol-level capability operations: creation,
//! delegation, canonical codec, inspection, verification, and fingerprints.
//!
//! This module contains no admission, sync, governance, or FFI concepts. It
//! returns typed facts and the stable rejection codes in `MeadowcapError`.

pub mod codec;
pub mod create;
pub mod delegate;
pub mod fingerprint;
pub mod inspect;
pub mod verify;

pub use willow25::authorisation::{ReadCapability, WriteCapability};

/// Maximum delegation-chain depth admitted before recursive verification.
/// Pinned by the design's resource ceilings. Lowering requires measured
/// fixtures; raising requires a security review and updated exhaustion tests.
pub const MAX_DELEGATION_DEPTH: usize = 16;

/// Maximum encoded capability size (bytes) admitted before allocation/verify.
pub const MAX_CAPABILITY_ENCODED_BYTES: usize = 64 * 1024;

/// Meadowcap access mode as a typed fact (never string-parsed by consumers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
}

/// Whether a capability is rooted in a communal or an owned namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    Communal,
    Owned,
}

/// Stable, non-secret capability rejection codes for Slice 1 surfaces
/// (creation, delegation, codec, inspection, verification). The broader
/// admission taxonomy (stale-policy, revoked, missing-parent, …) is Slice 3.
///
/// NOTE on folded conditions (both proven by crate source, tested in Task 9):
/// - **Non-canonical encodings** are rejected by willow25's canonical decode,
///   which uses `produce_decoded_canonic`
///   (`meadowcap-0.5.0/src/raw/possibly_valid_write_capability.rs:1048` — this is
///   the underlying meadowcap crate's file, 1157 lines, NOT the 587-line
///   willow25 wrapper file of the same name); the `DecodableCanonic` contract
///   rejects non-minimal compact-width forms as decode errors, so they surface
///   here as `Malformed`. There is deliberately no `NonCanonical` variant —
///   nothing in Slice 1 can construct one after decode, so declaring it would
///   ship an unreachable code. Slice 3's management taxonomy may reintroduce a
///   distinct code if a producer emerges.
/// - **Wrong-access-mode bytes** (read-capability bytes fed to the write decoder
///   or vice versa) are rejected because the decoder returns `Err` unless the
///   decoded access mode matches
///   (`meadowcap-0.5.0/src/raw/possibly_valid_write_capability.rs:997` in the
///   non-canonic path, `:1050` in the canonic path; the read decoder has the
///   symmetric Read check), so they too surface as `Malformed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeadowcapError {
    /// Bytes did not decode as a canonical, valid capability of the requested
    /// access mode. Covers structural garbage, invalid chain signatures,
    /// non-canonical encodings, and wrong-access-mode bytes (see the note above).
    Malformed,
    /// Canonical value did not consume all input bytes.
    TrailingBytes,
    /// Delegation chain deeper than `MAX_DELEGATION_DEPTH`.
    ChainTooDeep { depth: usize, max: usize },
    /// Encoded capability larger than `MAX_CAPABILITY_ENCODED_BYTES`.
    CapabilityTooLarge { bytes: usize, max: usize },
    /// Delegation would widen authority (new area not contained in prior area).
    AuthorityExpanding,
    /// Delegation signer's public key is not the capability's current receiver.
    ReceiverMismatch,
}

impl std::fmt::Display for MeadowcapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for MeadowcapError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceilings_match_design_resource_limits() {
        assert_eq!(MAX_DELEGATION_DEPTH, 16);
        assert_eq!(MAX_CAPABILITY_ENCODED_BYTES, 64 * 1024);
    }

    #[test]
    fn access_mode_and_kind_are_distinct_facts() {
        assert_ne!(AccessMode::Read, AccessMode::Write);
        assert_ne!(CapabilityKind::Communal, CapabilityKind::Owned);
    }
}
```

  This will not compile until the submodule files exist. Create empty placeholder files (`create.rs`, `codec.rs`, `delegate.rs`, `fingerprint.rs`, `inspect.rs`, `verify.rs`) each containing only `//! placeholder` for now; each subsequent task fills one in. Add `pub mod meadowcap;` to `crates/riot-core/src/lib.rs` after `pub mod import;`.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::mod::tests` — expected failure: compile error (`file not found for module` for the empty submodules, or unresolved submodule contents). Create the six empty placeholder files, then re-run; the two `mod::tests` cases pass.
- [ ] **Commit.** `git add crates/riot-core/src/meadowcap/mod.rs crates/riot-core/src/meadowcap/create.rs crates/riot-core/src/meadowcap/codec.rs crates/riot-core/src/meadowcap/delegate.rs crates/riot-core/src/meadowcap/fingerprint.rs crates/riot-core/src/meadowcap/inspect.rs crates/riot-core/src/meadowcap/verify.rs crates/riot-core/src/lib.rs && git commit -m "feat(meadowcap): scaffold capability-core module, errors, ceilings"`

### Task 1 — Read + write capability creation wrappers

**Files:** Modify `crates/riot-core/src/meadowcap/create.rs`.

- [ ] **Write the failing test.** Replace the placeholder with:

```rust
//! Communal and owned read/write capability creation over `willow25`.

use willow25::authorisation::{ReadCapability, WriteCapability};
use willow25::prelude::{NamespaceId, NamespaceSecret, SubspaceId};

/// A communal write capability for `user_key`'s own subspace in `namespace`.
pub fn new_communal_write(namespace: NamespaceId, user_key: SubspaceId) -> WriteCapability {
    WriteCapability::new_communal(namespace, user_key)
}

/// A communal read capability for `user_key`'s own subspace in `namespace`.
pub fn new_communal_read(namespace: NamespaceId, user_key: SubspaceId) -> ReadCapability {
    ReadCapability::new_communal(namespace, user_key)
}

/// An owned write capability granting `Area::full()` of the owned namespace,
/// received by `user_key`. `namespace_secret` is the owned-namespace root; it
/// stays inside `riot-core` and never crosses FFI.
pub fn new_owned_write(namespace_secret: &NamespaceSecret, user_key: SubspaceId) -> WriteCapability {
    WriteCapability::new_owned(namespace_secret, user_key)
}

/// An owned read capability granting `Area::full()` of the owned namespace.
pub fn new_owned_read(namespace_secret: &NamespaceSecret, user_key: SubspaceId) -> ReadCapability {
    ReadCapability::new_owned(namespace_secret, user_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use willow25::prelude::{Area, SubspaceSecret};

    fn owned_namespace_secret() -> NamespaceSecret {
        // Seeded: ed25519 keygen is deterministic, so tests are reproducible.
        NamespaceSecret::from_bytes(&[3u8; 32])
    }

    #[test]
    fn owned_write_cap_is_owned_full_area_zero_delegation() {
        let ns = owned_namespace_secret();
        let receiver = SubspaceSecret::from_bytes(&[4u8; 32]).corresponding_subspace_id();
        let cap = new_owned_write(&ns, receiver.clone());
        assert!(cap.is_owned());
        assert!(cap.delegations().is_empty());
        assert_eq!(cap.receiver(), &receiver);
        assert_eq!(cap.granted_namespace(), &ns.corresponding_namespace_id());
        assert_eq!(cap.granted_area(), Area::full());
    }

    #[test]
    fn communal_read_cap_is_not_owned_and_scopes_to_receiver_subspace() {
        let namespace = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[5u8; 32]).corresponding_subspace_id();
        let cap = new_communal_read(namespace.clone(), receiver.clone());
        assert!(!cap.is_owned());
        assert_eq!(cap.receiver(), &receiver);
        assert_eq!(cap.granted_namespace(), &namespace);
        assert_eq!(cap.granted_area(), Area::new_subspace_area(receiver));
    }
}
```

  Note: `NamespaceSecret::corresponding_namespace_id()` and `SubspaceSecret::corresponding_subspace_id()` are the exact accessors used in `willow/owned.rs` and `willow/masthead.rs`; `NamespaceId::from_bytes` is used in the willow25 doctests. If `NamespaceId::from_bytes(&[16u8; 32])` happens to be an *owned* id (unlikely for a fixed communal test vector but namespace-kind is a top-bit convention), pick a seed whose `is_communal()` holds; assert it in the test.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::create` — expected failure: `cannot find function new_communal_write` (placeholder body). Fill in the module body above, re-run; both tests pass.
- [ ] **Commit.** `git add crates/riot-core/src/meadowcap/create.rs && git commit -m "feat(meadowcap): communal/owned read+write capability creation"`

### Task 2 — Canonical read-capability codec (encode, canonical decode, trailing-byte reject)

**Files:** Modify `crates/riot-core/src/willow/mod.rs` (promote `decode_canonic_exact` to `pub(crate)`); Modify `crates/riot-core/src/meadowcap/codec.rs`.

- [ ] **Write the failing test.** In `crates/riot-core/src/meadowcap/codec.rs`:

```rust
//! Canonical read-capability codec and ceiling-bounded decoders. Reuses the
//! write-cap codec in `crate::willow` rather than duplicating it.

use ufotofu::codec_prelude::EncodableExt;
use willow25::authorisation::ReadCapability;

use super::MeadowcapError;

/// Canonical encoding of a read capability (mirrors `willow::encode_capability`
/// for the write path).
pub fn encode_read_capability(capability: &ReadCapability) -> Vec<u8> {
    pollster::block_on(capability.new_vec_storing_encoding())
}

/// Canonical decode of a read capability, rejecting trailing bytes. Reuses the
/// shared `willow::decode_canonic_exact` guard.
pub fn decode_read_capability_canonic(bytes: &[u8]) -> Result<ReadCapability, MeadowcapError> {
    crate::willow::decode_canonic_exact::<ReadCapability>(bytes).map_err(|e| match e {
        crate::willow::WillowError::TrailingBytes => MeadowcapError::TrailingBytes,
        _ => MeadowcapError::Malformed,
    })
}

#[cfg(test)]
mod tests {
    use super::super::create::new_communal_read;
    use super::*;
    use willow25::prelude::{NamespaceId, SubspaceSecret};

    fn a_read_cap() -> ReadCapability {
        let ns = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id();
        new_communal_read(ns, receiver)
    }

    #[test]
    fn read_capability_roundtrips_canonically() {
        let cap = a_read_cap();
        let bytes = encode_read_capability(&cap);
        let decoded = decode_read_capability_canonic(&bytes).expect("canonical decode");
        assert_eq!(decoded, cap);
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let cap = a_read_cap();
        let mut bytes = encode_read_capability(&cap);
        bytes.push(0x00);
        assert_eq!(
            decode_read_capability_canonic(&bytes),
            Err(MeadowcapError::TrailingBytes)
        );
    }

    #[test]
    fn garbage_bytes_are_malformed() {
        assert_eq!(
            decode_read_capability_canonic(&[0xff, 0xff, 0xff, 0xff]),
            Err(MeadowcapError::Malformed)
        );
    }

    #[test]
    fn owned_read_cap_is_owned_full_area_and_round_trips() {
        // Exercises new_owned_read (spec line 153): the owned read-capability
        // creation path is otherwise untested. Owned genesis grants Area::full()
        // with Read access, and its canonical encoding round-trips.
        use super::super::create::new_owned_read;
        use willow25::authorisation::raw::AccessMode;
        use willow25::prelude::{Area, NamespaceSecret};

        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[6u8; 32]).corresponding_subspace_id();
        let cap = new_owned_read(&ns, receiver.clone());
        assert!(cap.is_owned());
        assert_eq!(cap.receiver(), &receiver);
        assert_eq!(cap.granted_namespace(), &ns.corresponding_namespace_id());
        assert_eq!(cap.granted_area(), Area::full());
        assert_eq!(cap.genesis().access_mode(), AccessMode::Read);

        let bytes = encode_read_capability(&cap);
        assert_eq!(decode_read_capability_canonic(&bytes).expect("round-trip"), cap);
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::codec` — expected failure: `function decode_canonic_exact is private` (E0603). In `crates/riot-core/src/willow/mod.rs` change `fn decode_canonic_exact<T>` to `pub(crate) fn decode_canonic_exact<T>` (line ~187). Re-run; the three codec tests pass.
- [ ] **Commit.** `git add crates/riot-core/src/willow/mod.rs crates/riot-core/src/meadowcap/codec.rs && git commit -m "feat(meadowcap): canonical read-capability codec, reuse shared trailing-byte guard"`

### Task 3 — Ceiling-bounded decoders (depth 16, 64 KiB) for read + write

**Files:** Modify `crates/riot-core/src/meadowcap/codec.rs`.

- [ ] **Write the failing test.** Append to `codec.rs`:

```rust
use willow25::authorisation::WriteCapability;

use super::{MAX_CAPABILITY_ENCODED_BYTES, MAX_DELEGATION_DEPTH};

/// Decode a write capability from canonical bytes, enforcing the byte and
/// delegation-depth ceilings *before* returning it for verification.
pub fn decode_write_capability_bounded(bytes: &[u8]) -> Result<WriteCapability, MeadowcapError> {
    if bytes.len() > MAX_CAPABILITY_ENCODED_BYTES {
        return Err(MeadowcapError::CapabilityTooLarge {
            bytes: bytes.len(),
            max: MAX_CAPABILITY_ENCODED_BYTES,
        });
    }
    let cap = crate::willow::decode_capability_canonic(bytes).map_err(|e| match e {
        crate::willow::WillowError::TrailingBytes => MeadowcapError::TrailingBytes,
        _ => MeadowcapError::Malformed,
    })?;
    let depth = cap.delegations().len();
    if depth > MAX_DELEGATION_DEPTH {
        return Err(MeadowcapError::ChainTooDeep {
            depth,
            max: MAX_DELEGATION_DEPTH,
        });
    }
    Ok(cap)
}

/// Read-capability analogue of `decode_write_capability_bounded`.
pub fn decode_read_capability_bounded(bytes: &[u8]) -> Result<ReadCapability, MeadowcapError> {
    if bytes.len() > MAX_CAPABILITY_ENCODED_BYTES {
        return Err(MeadowcapError::CapabilityTooLarge {
            bytes: bytes.len(),
            max: MAX_CAPABILITY_ENCODED_BYTES,
        });
    }
    let cap = decode_read_capability_canonic(bytes)?;
    let depth = cap.delegations().len();
    if depth > MAX_DELEGATION_DEPTH {
        return Err(MeadowcapError::ChainTooDeep {
            depth,
            max: MAX_DELEGATION_DEPTH,
        });
    }
    Ok(cap)
}

#[cfg(test)]
mod ceiling_tests {
    use super::*;

    #[test]
    fn oversized_input_is_rejected_before_decode() {
        let bytes = vec![0u8; MAX_CAPABILITY_ENCODED_BYTES + 1];
        assert_eq!(
            decode_write_capability_bounded(&bytes),
            Err(MeadowcapError::CapabilityTooLarge {
                bytes: MAX_CAPABILITY_ENCODED_BYTES + 1,
                max: MAX_CAPABILITY_ENCODED_BYTES,
            })
        );
        assert_eq!(
            decode_read_capability_bounded(&bytes),
            Err(MeadowcapError::CapabilityTooLarge {
                bytes: MAX_CAPABILITY_ENCODED_BYTES + 1,
                max: MAX_CAPABILITY_ENCODED_BYTES,
            })
        );
    }
}
```

  The depth-ceiling positive/negative test needs a chain of 17 delegations; build it with the `delegate_write` primitive from Task 5. Add this cross-check in Task 5's test module (a 17-hop chain that `decode_write_capability_bounded` rejects with `ChainTooDeep`) rather than here, to keep this task's dependencies minimal.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::codec::ceiling_tests` — expected failure: `cannot find function decode_write_capability_bounded`. Fill in the bodies above, re-run; the oversize test passes.
- [ ] **Commit.** `git add crates/riot-core/src/meadowcap/codec.rs && git commit -m "feat(meadowcap): ceiling-bounded read/write capability decoders"`

### Task 4 — Typed inspection: `CapabilitySummary`, access mode, receiver-vs-subspace separation

**Files:** Modify `crates/riot-core/src/meadowcap/inspect.rs`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Typed inspection facts. The capability *receiver* is returned as a distinct
//! fact from any entry `subspace_id` coordinate; in an owned namespace these
//! identities are not interchangeable (design "Meadowcap core").

use willow25::authorisation::raw::AccessMode as RawAccessMode;
use willow25::authorisation::{ReadCapability, WriteCapability};
use willow25::prelude::{Area, NamespaceId, SubspaceId};

use super::fingerprint::{read_capability_fingerprint, write_capability_fingerprint, CapabilityFingerprint};
use super::{AccessMode, CapabilityKind};

/// Immutable, typed, non-secret facts about a capability. Never exposes secret
/// material or raw capability bytes beyond the fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySummary {
    pub kind: CapabilityKind,
    pub access_mode: AccessMode,
    pub granted_namespace: NamespaceId,
    /// The capability receiver — NOT an entry subspace coordinate.
    pub receiver: SubspaceId,
    pub granted_area: Area,
    pub chain_depth: usize,
    pub fingerprint: CapabilityFingerprint,
}

fn kind_of(is_owned: bool) -> CapabilityKind {
    if is_owned {
        CapabilityKind::Owned
    } else {
        CapabilityKind::Communal
    }
}

fn access_of(raw: RawAccessMode) -> AccessMode {
    match raw {
        RawAccessMode::Read => AccessMode::Read,
        RawAccessMode::Write => AccessMode::Write,
    }
}

pub fn summarise_write(cap: &WriteCapability) -> CapabilitySummary {
    CapabilitySummary {
        kind: kind_of(cap.is_owned()),
        access_mode: access_of(cap.genesis().access_mode()),
        granted_namespace: cap.granted_namespace().clone(),
        receiver: cap.receiver().clone(),
        granted_area: cap.granted_area(),
        chain_depth: cap.delegations().len(),
        fingerprint: write_capability_fingerprint(cap),
    }
}

pub fn summarise_read(cap: &ReadCapability) -> CapabilitySummary {
    CapabilitySummary {
        kind: kind_of(cap.is_owned()),
        access_mode: access_of(cap.genesis().access_mode()),
        granted_namespace: cap.granted_namespace().clone(),
        receiver: cap.receiver().clone(),
        granted_area: cap.granted_area(),
        chain_depth: cap.delegations().len(),
        fingerprint: read_capability_fingerprint(cap),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meadowcap::create::{new_owned_read, new_owned_write};
    use willow25::prelude::{NamespaceSecret, SubspaceSecret};

    #[test]
    fn owned_write_summary_reports_write_owned_and_receiver_not_namespace() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[4u8; 32]).corresponding_subspace_id();
        let cap = new_owned_write(&ns, receiver.clone());
        let s = summarise_write(&cap);
        assert_eq!(s.kind, CapabilityKind::Owned);
        assert_eq!(s.access_mode, AccessMode::Write);
        assert_eq!(s.receiver, receiver);
        assert_eq!(s.granted_namespace, ns.corresponding_namespace_id());
        assert_eq!(s.chain_depth, 0);
        // Receiver is a subspace id; the owned namespace id is a namespace key.
        // They are distinct fact types and (here) distinct values.
        assert_ne!(s.receiver.as_bytes(), s.granted_namespace.as_bytes());
    }

    #[test]
    fn owned_read_summary_reports_read_owned_and_receiver_not_namespace() {
        // Pins summarise_read (spec line 156 read inspection), mirroring the
        // write-side summary: access mode is Read, and the receiver is a distinct
        // fact from the namespace key.
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[6u8; 32]).corresponding_subspace_id();
        let cap = new_owned_read(&ns, receiver.clone());
        let s = summarise_read(&cap);
        assert_eq!(s.kind, CapabilityKind::Owned);
        assert_eq!(s.access_mode, AccessMode::Read);
        assert_eq!(s.receiver, receiver);
        assert_eq!(s.granted_namespace, ns.corresponding_namespace_id());
        assert_eq!(s.chain_depth, 0);
        assert_ne!(s.receiver.as_bytes(), s.granted_namespace.as_bytes());
    }
}
```

  Confirm the exact `AccessMode` import path during implementation: the willow25 doctests use `willow25::authorisation::raw::AccessMode` with variants `Read`/`Write` and `genesis().access_mode()` returns it. If the wrapper re-exports it at `willow25::authorisation::AccessMode`, use that path — do not invent a new enum for the raw value; map it into Riot's `AccessMode`.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::inspect` — expected failure: unresolved `fingerprint::write_capability_fingerprint` (Task 7 not yet done). Implement Task 7 first if executing strictly in order, **or** temporarily stub the fingerprint calls; the recommended order is Task 7 before Task 4. Re-run after both are present; both summary tests pass (`owned_write_summary_...` and `owned_read_summary_...`).
- [ ] **Commit.** `git add crates/riot-core/src/meadowcap/inspect.rs && git commit -m "feat(meadowcap): typed capability summaries, receiver-vs-subspace separation"`

### Task 5 — Attenuating delegation with stable rejection codes (read + write)

**Files:** Modify `crates/riot-core/src/meadowcap/delegate.rs`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Attenuation-only delegation. Wraps `willow25`'s `try_delegate`, mapping its
//! opaque `InvalidCapability` to stable `MeadowcapError` codes. Delegation can
//! only narrow authority (design core principle 5, "Attenuation only").
//!
//! IMPORTANT: import `InvalidCapability` from `willow25::authorisation::raw`,
//! NEVER `meadowcap::raw` — `meadowcap` is a transitive-only dependency of the
//! workspace (re-exported through willow25) and naming it directly is E0433
//! *and* would require adding a `meadowcap` dep edge, which changes
//! `Cargo.lock` and breaks the `cargo_lock_sha256` pin in
//! `fixtures/manifest.json`. Do not add a `meadowcap` dependency.

use willow25::authorisation::raw::InvalidCapability;
use willow25::authorisation::{ReadCapability, WriteCapability};
use willow25::prelude::{Area, SubspaceId, SubspaceSecret};

use super::MeadowcapError;

/// Delegate a write capability to `new_receiver`, restricting it to `new_area`.
/// `signer` must be the current receiver's secret. Returns a new capability;
/// the input is cloned so the caller's capability is untouched.
pub fn delegate_write(
    parent: &WriteCapability,
    signer: &SubspaceSecret,
    new_area: Area,
    new_receiver: SubspaceId,
) -> Result<WriteCapability, MeadowcapError> {
    // Disambiguate the two failure causes willow25 collapses into one opaque
    // `InvalidCapability`: a wrong signer is detectable Riot-side (no new
    // willow25 API) by comparing the signer's public key to the current
    // receiver BEFORE delegating, so consumers get a stable ReceiverMismatch.
    if &signer.corresponding_subspace_id() != parent.receiver() {
        return Err(MeadowcapError::ReceiverMismatch);
    }
    let mut cap = parent.clone();
    cap.try_delegate(signer, new_area, new_receiver)
        .map_err(map_invalid)?;
    Ok(cap)
}

/// Read-capability analogue of `delegate_write`.
pub fn delegate_read(
    parent: &ReadCapability,
    signer: &SubspaceSecret,
    new_area: Area,
    new_receiver: SubspaceId,
) -> Result<ReadCapability, MeadowcapError> {
    if &signer.corresponding_subspace_id() != parent.receiver() {
        return Err(MeadowcapError::ReceiverMismatch);
    }
    let mut cap = parent.clone();
    cap.try_delegate(signer, new_area, new_receiver)
        .map_err(map_invalid)?;
    Ok(cap)
}

/// After the receiver pre-check above, the only remaining `try_delegate`
/// failure is an area that is not contained in the parent's granted area
/// (authority expansion). Map it to the stable code.
fn map_invalid(_e: InvalidCapability) -> MeadowcapError {
    MeadowcapError::AuthorityExpanding
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meadowcap::codec::{
        decode_read_capability_bounded, decode_write_capability_bounded, encode_read_capability,
    };
    use crate::meadowcap::create::{new_owned_read, new_owned_write};
    use crate::willow::{encode_capability, tai_j2000_micros_from_unix_seconds};
    use willow25::prelude::{NamespaceSecret, Path, TimeRange};

    fn micros_range(from_unix: u64, to_unix: u64) -> TimeRange {
        // MICROSECONDS, never raw seconds — see load-bearing constraint 3.
        let start = tai_j2000_micros_from_unix_seconds(from_unix).expect("start micros");
        let end = tai_j2000_micros_from_unix_seconds(to_unix).expect("end micros");
        TimeRange::new(start.into(), Some(end.into()))
    }

    #[test]
    fn valid_attenuation_narrows_area_and_moves_receiver() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let cap = new_owned_write(&ns, owner.corresponding_subspace_id());

        let editor_id = SubspaceSecret::from_bytes(&[8u8; 32]).corresponding_subspace_id();
        let area = Area::new(
            Some(editor_id.clone()),
            Path::from_slices(&[b"articles", b"news"]).expect("path"),
            micros_range(1_700_000_000, 1_800_000_000),
        );
        let delegated = delegate_write(&cap, &owner, area, editor_id.clone()).expect("attenuate");
        assert_eq!(delegated.receiver(), &editor_id);
        assert_eq!(delegated.delegations().len(), 1);
    }

    #[test]
    fn read_delegation_narrows_area_moves_receiver_and_rejects_widening() {
        // Pins BOTH halves of the read-delegation surface (spec line 156): a
        // valid read attenuation, and a widening rejection. Also exercises
        // decode_read_capability_bounded's happy path (warning a).
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let cap = new_owned_read(&ns, owner.corresponding_subspace_id());
        assert_eq!(cap.granted_area(), Area::full());

        let editor = SubspaceSecret::from_bytes(&[8u8; 32]);
        let editor_id = editor.corresponding_subspace_id();
        let narrow = Area::new(
            Some(editor_id.clone()),
            Path::from_slices(&[b"content"]).expect("path"),
            micros_range(1_700_000_000, 1_800_000_000),
        );
        let delegated =
            delegate_read(&cap, &owner, narrow.clone(), editor_id.clone()).expect("attenuate read");
        assert_eq!(delegated.receiver(), &editor_id, "receiver moved to editor");
        assert_eq!(delegated.delegations().len(), 1, "chain depth incremented");
        assert_eq!(delegated.granted_area(), narrow, "granted area narrowed to the delegated area");
        assert_ne!(delegated.granted_area(), Area::full(), "granted area is no longer full");

        // Happy-path bounded read decode returns the same valid capability.
        let bytes = encode_read_capability(&delegated);
        assert_eq!(
            decode_read_capability_bounded(&bytes).expect("bounded read decode"),
            delegated
        );

        // NEGATIVE: widening a delegated read cap back to full is rejected.
        let leaf = SubspaceSecret::from_bytes(&[10u8; 32]).corresponding_subspace_id();
        assert_eq!(
            delegate_read(&delegated, &editor, Area::full(), leaf),
            Err(MeadowcapError::AuthorityExpanding)
        );
    }

    #[test]
    fn wrong_signer_is_rejected_as_receiver_mismatch() {
        // The signing secret is NOT the capability's current receiver, so the
        // delegation must fail-closed with the stable ReceiverMismatch code
        // (the Riot-side pre-check), never a misleading AuthorityExpanding.
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let cap = new_owned_write(&ns, owner.corresponding_subspace_id());

        let impostor = SubspaceSecret::from_bytes(&[42u8; 32]); // not the receiver
        let target = SubspaceSecret::from_bytes(&[43u8; 32]).corresponding_subspace_id();
        let area = Area::new(
            Some(target.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            micros_range(1_700_000_000, 1_800_000_000),
        );
        assert_eq!(
            delegate_write(&cap, &impostor, area, target),
            Err(MeadowcapError::ReceiverMismatch)
        );
    }

    #[test]
    fn widening_to_full_area_is_rejected() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        // Delegate once to a narrow area, then try to widen back to full.
        let cap = new_owned_write(&ns, owner.corresponding_subspace_id());
        let mid = SubspaceSecret::from_bytes(&[9u8; 32]);
        let mid_id = mid.corresponding_subspace_id();
        let narrow = Area::new(
            Some(mid_id.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            micros_range(1_700_000_000, 1_800_000_000),
        );
        let cap = delegate_write(&cap, &owner, narrow, mid_id.clone()).expect("narrow");
        let widen = Area::full();
        assert_eq!(
            delegate_write(&cap, &mid, widen, SubspaceSecret::from_bytes(&[10u8; 32]).corresponding_subspace_id()),
            Err(MeadowcapError::AuthorityExpanding)
        );
    }

    #[test]
    fn seventeen_hop_chain_is_rejected_by_bounded_decode() {
        // Depth ceiling cross-check (Task 3): build a 17-deep chain and confirm
        // decode_write_capability_bounded rejects it with ChainTooDeep.
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let mut signer = SubspaceSecret::from_bytes(&[4u8; 32]);
        let mut cap = new_owned_write(&ns, signer.corresponding_subspace_id());
        for i in 0..17u8 {
            let next = SubspaceSecret::from_bytes(&[100u8.wrapping_add(i); 32]);
            let next_id = next.corresponding_subspace_id();
            // Owned genesis grants Area::full(); every hop re-grants full (still
            // contained), so only depth grows.
            cap = delegate_write(&cap, &signer, Area::full(), next_id).expect("hop");
            signer = next;
        }
        assert_eq!(cap.delegations().len(), 17);
        let bytes = encode_capability(&cap);
        assert_eq!(
            decode_write_capability_bounded(&bytes),
            Err(MeadowcapError::ChainTooDeep { depth: 17, max: 16 })
        );
    }
}
```

  During implementation, verify whether `Area::full()` re-delegation is accepted by `try_delegate` (full ⊆ full). If willow25 rejects a same-area re-grant, use a strictly-nested path sequence (`/a`, `/a/b`, …) that stays ≥17 deep, or re-grant the identical narrowed area. Adjust only the chain construction, not the asserted ceiling.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::delegate` — expected failure: `cannot find function delegate_write`/`delegate_read`. Fill in the bodies, re-run; all delegate tests pass: `valid_attenuation_narrows_area_and_moves_receiver`, `read_delegation_narrows_area_moves_receiver_and_rejects_widening`, `wrong_signer_is_rejected_as_receiver_mismatch`, `widening_to_full_area_is_rejected`, `seventeen_hop_chain_is_rejected_by_bounded_decode`, and the `time_unit_tests`/`ceiling` cross-checks.
- [ ] **Commit.** `git add crates/riot-core/src/meadowcap/delegate.rs && git commit -m "feat(meadowcap): attenuating read/write delegation with stable rejection codes"`

### Task 6 — Verification surface: reuse `verify_entry`, add token+coverage predicates

**Files:** Modify `crates/riot-core/src/meadowcap/verify.rs`.

**Spec line 157 ("Verify every namespace and user signature in a delegation chain") is delivered here — by the reused canonical decode path, not by new code.** The pinned crate's `WriteCapability`/`ReadCapability` are the *validated* Meadowcap types: their `Decodable`/`DecodableCanonic` impls decode into a `PossiblyValid*Capability` and return the valid type only `if decoded.is_valid()` (`meadowcap-0.5.0/src/write_capability.rs:819` for the canonic path and `:761` for the lenient path, plus the `read_capability.rs` analogues). `is_valid()` is bound by `NamespacePublicKey: Verifier<NamespaceSignature>` and `UserPublicKey: Verifier<UserSignature>` (`meadowcap-0.5.0/src/raw/possibly_valid_write_capability.rs:824-830`), so every genesis namespace signature and every delegation user signature in the chain is cryptographically verified during decode; a capability with any bad chain signature fails to decode. Therefore Riot's `willow::decode_capability_canonic` (write) and Task 2's `decode_read_capability_canonic` (read) already perform full chain-signature verification, and the bounded decoders in Task 3 run that verification behind the depth/size ceilings. This module adds the *entry-authorisation* and *read-coverage* checks on top; it deliberately does not re-verify chain signatures, which would duplicate the canonical decode gate (reuse rule). Task 9's `delegation_chain_signature_tamper_is_rejected` test pins this behaviour end to end.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Verification surface. REUSES the canonical checked verifier
//! `crate::willow::verify_entry` (the `PossiblyAuthorisedEntry` conversion) —
//! this module must NOT hand-roll a second signature-verification path.

use willow25::authorisation::{AuthorisationToken, ReadCapability};
use willow25::entry::Entry;
use willow25::prelude::{Area, NamespaceId};

pub use crate::willow::verify_entry;

/// True iff `token` (capability + receiver signature) authorises `entry`.
/// Thin, explicit wrapper over the checked verifier for a named call site.
/// Namespace coverage is part of this check: `verify_entry`'s underlying
/// `cap.includes(entry)` is false when the capability's granted namespace does
/// not equal the entry's namespace.
pub fn token_authorises_entry(entry: &Entry, token: &AuthorisationToken) -> bool {
    verify_entry(entry, token)
}

/// True iff a read request for `requested` in `namespace` is covered by
/// `read_cap` — BOTH the namespace must equal the capability's granted
/// namespace AND the requested area must be contained in the granted area. A
/// read request is a (namespace, area) pair; checking area alone would let a
/// capability for namespace Y cover a request in namespace X. This is the
/// read-gate coverage primitive; the receiver-proof handshake that actually
/// gates disclosure is Slice 4.
pub fn read_request_covered(
    read_cap: &ReadCapability,
    namespace: &NamespaceId,
    requested: &Area,
) -> bool {
    read_cap.granted_namespace() == namespace && read_cap.includes_area(requested)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meadowcap::create::{new_communal_read, new_owned_write};
    use willow25::prelude::{NamespaceId, NamespaceSecret, Path, SubspaceSecret, TimeRange};

    fn entry_in(ns: &NamespaceId, subspace: willow25::prelude::SubspaceId, path: &[&[u8]]) -> Entry {
        Entry::builder()
            .namespace_id(ns.clone())
            .subspace_id(subspace)
            .path(Path::from_slices(path).expect("path"))
            .timestamp(1_000u64)
            .payload(b"payload")
            .build()
    }

    #[test]
    fn owner_signed_entry_verifies_and_tampered_signature_fails() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let cap = new_owned_write(&ns, owner.corresponding_subspace_id());
        let entry = entry_in(&ns.corresponding_namespace_id(), owner.corresponding_subspace_id(), &[b"manifest"]);
        let authorised = entry
            .clone()
            .into_authorised_entry(&cap, &owner)
            .expect("owner authorises");
        let token = authorised.authorisation_token();
        assert!(token_authorises_entry(&entry, token));

        // A different subspace's entry under the same token must not verify.
        let other = entry_in(&ns.corresponding_namespace_id(), SubspaceSecret::from_bytes(&[9u8; 32]).corresponding_subspace_id(), &[b"manifest"]);
        assert!(!token_authorises_entry(&other, token));

        // WRONG NAMESPACE: the same signed token must not authorise an entry
        // whose namespace differs from the capability's granted namespace.
        let wrong_ns = NamespaceSecret::from_bytes(&[77u8; 32]).corresponding_namespace_id();
        let cross = entry_in(&wrong_ns, owner.corresponding_subspace_id(), &[b"manifest"]);
        assert!(!token_authorises_entry(&cross, token), "cross-namespace entry must fail");
    }

    #[test]
    fn read_coverage_checks_namespace_and_area() {
        let ns = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[5u8; 32]).corresponding_subspace_id();
        let cap = new_communal_read(ns.clone(), receiver.clone());
        let inside = Area::new_subspace_area(receiver);

        // Right namespace + contained area -> covered.
        assert!(read_request_covered(&cap, &ns, &inside));
        // Right namespace, wider area -> not covered.
        assert!(!read_request_covered(&cap, &ns, &Area::full()));
        // WRONG NAMESPACE, otherwise-contained area -> not covered.
        let other_ns = NamespaceId::from_bytes(&[24u8; 32]);
        assert!(!read_request_covered(&cap, &other_ns, &inside), "cross-namespace read must fail");
    }
}
```

  During implementation confirm `ReadCapability::includes_area(&Area)` is the exact Area⊆granted-area primitive (it is, per the inventory) and that `NamespaceId: PartialEq` (it is — used in `granted_namespace` assertions elsewhere). Keep `read_request_covered`'s three-argument signature stable; callers depend on the namespace check. Pick communal `NamespaceId::from_bytes` seeds whose `is_communal()` holds so `new_communal_read` is well-formed.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::verify` — expected failure: `cannot find function token_authorises_entry` / `read_request_covered` (placeholder body). Fill in the module body above, re-run; both tests pass (including the cross-namespace assertions).
- [ ] **Commit.** `git add crates/riot-core/src/meadowcap/verify.rs && git commit -m "feat(meadowcap): verification surface reusing verify_entry, read-coverage predicate"`

### Task 7 — Capability fingerprints (exact spec preimage, domain separation)

**Files:** Modify `crates/riot-core/src/meadowcap/fingerprint.rs`.

- [ ] **Write the failing test.** Replace the placeholder:

```rust
//! Capability fingerprints. The design pins the EXACT preimage
//! `SHA-256("riot/meadowcap-fingerprint/v1" || canonical_capability_bytes)`
//! with NO length prefix — deliberately different from
//! `willow::digest::entry_id`, which prepends a u32 length. This fingerprint is
//! the join key the governance ledger (Slice 2) uses; its bytes must match the
//! spec exactly. The canonical bytes already bind type, access mode, namespace,
//! receiver, area, and every delegation signature, so read and write caps of
//! the same shape produce different fingerprints.

use sha2::{Digest, Sha256};
use willow25::authorisation::{ReadCapability, WriteCapability};

use super::codec::encode_read_capability;

pub type CapabilityFingerprint = [u8; 32];

const FINGERPRINT_DOMAIN: &[u8] = b"riot/meadowcap-fingerprint/v1";

fn fingerprint_of_canonical_bytes(canonical: &[u8]) -> CapabilityFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_DOMAIN);
    hasher.update(canonical);
    hasher.finalize().into()
}

pub fn write_capability_fingerprint(cap: &WriteCapability) -> CapabilityFingerprint {
    fingerprint_of_canonical_bytes(&crate::willow::encode_capability(cap))
}

pub fn read_capability_fingerprint(cap: &ReadCapability) -> CapabilityFingerprint {
    fingerprint_of_canonical_bytes(&encode_read_capability(cap))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meadowcap::create::{new_communal_read, new_communal_write};
    use willow25::prelude::{NamespaceId, SubspaceSecret};

    #[test]
    fn fingerprint_is_deterministic_and_domain_separated() {
        let ns = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id();
        let cap = new_communal_write(ns.clone(), receiver.clone());

        let fp1 = write_capability_fingerprint(&cap);
        let fp2 = write_capability_fingerprint(&cap);
        assert_eq!(fp1, fp2, "fingerprint must be deterministic");

        // Domain separation: a raw SHA-256 of the same bytes (no domain) differs.
        let raw: [u8; 32] = Sha256::digest(crate::willow::encode_capability(&cap)).into();
        assert_ne!(fp1, raw, "domain prefix must change the digest");
    }

    #[test]
    fn read_and_write_caps_of_same_shape_have_different_fingerprints() {
        let ns = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id();
        let w = write_capability_fingerprint(&new_communal_write(ns.clone(), receiver.clone()));
        let r = read_capability_fingerprint(&new_communal_read(ns, receiver));
        assert_ne!(w, r, "access mode is bound in the canonical bytes");
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::fingerprint` — expected failure: `cannot find function write_capability_fingerprint`. Fill in the body, re-run; both tests pass. (Implement this task before Task 4, which consumes these functions.)
- [ ] **Commit.** `git add crates/riot-core/src/meadowcap/fingerprint.rs && git commit -m "feat(meadowcap): exact-preimage capability fingerprints"`

### Task 8 — Time-unit trap guard (micros TimeRange authorises entries; seconds authorises zero)

**Files:** Modify `crates/riot-core/src/meadowcap/delegate.rs` (add a `time_unit_tests` module).

- [ ] **Write the failing test.** This pins the exact bug the project shipped in #72 and fixed in #73: a `TimeRange` built from raw Unix seconds authorises zero real (micros-stamped) entries; the same range built through `tai_j2000_micros_from_unix_seconds` authorises the entry.

```rust
#[cfg(test)]
mod time_unit_tests {
    use super::*;
    use crate::meadowcap::create::new_owned_write;
    use crate::willow::tai_j2000_micros_from_unix_seconds;
    use willow25::entry::Entry;
    use willow25::prelude::{NamespaceSecret, Path, TimeRange};

    fn entry_at_micros(ns_secret: &NamespaceSecret, subspace: willow25::prelude::SubspaceId, micros: u64) -> Entry {
        Entry::builder()
            .namespace_id(ns_secret.corresponding_namespace_id())
            .subspace_id(subspace)
            .path(Path::from_slices(&[b"articles", b"post"]).expect("path"))
            .timestamp(micros)
            .payload(b"p")
            .build()
    }

    #[test]
    fn micros_range_covers_entry_but_seconds_range_covers_nothing() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let owner_id = owner.corresponding_subspace_id();
        let cap = new_owned_write(&ns, owner_id.clone());

        // A real entry stamped in the production unit (micros).
        let unix = 1_700_000_000u64;
        let micros = tai_j2000_micros_from_unix_seconds(unix).expect("micros");
        let entry = entry_at_micros(&ns, owner_id.clone(), micros);

        // CORRECT: a micros-domain area delegated for [unix-1day, unix+1day].
        let good_area = Area::new(
            Some(owner_id.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            TimeRange::new(
                tai_j2000_micros_from_unix_seconds(unix - 86_400).unwrap().into(),
                Some(tai_j2000_micros_from_unix_seconds(unix + 86_400).unwrap().into()),
            ),
        );
        let good = delegate_write(&cap, &owner, good_area, owner_id.clone()).expect("attenuate");
        assert!(good.includes(&entry), "micros-domain cap must cover a micros entry");

        // TRAP: the same window built from RAW SECONDS. J2000 micros for 2023
        // are ~7.3e17; a range ending at ~1.7e9 seconds ends astronomically
        // before the entry, so it covers zero real entries.
        let bad_area = Area::new(
            Some(owner_id.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            TimeRange::new((unix - 86_400).into(), Some((unix + 86_400).into())),
        );
        let bad = delegate_write(&cap, &owner, bad_area, owner_id).expect("attenuate");
        assert!(!bad.includes(&entry), "raw-seconds cap must cover NOTHING real");
    }
}
```

- [ ] **Run it and watch it fail.** `cargo test -p riot-core meadowcap::delegate::time_unit_tests` — if `delegate_write`/`includes` are already present it should pass immediately (it is a regression tripwire, not new production code). If it fails, the bug is real — fix the production `TimeRange` construction, never the assertion. Confirm `WriteCapability::includes(&Entry)` is the coverage primitive (inventory); it is.
- [ ] **Commit.** `git add crates/riot-core/src/meadowcap/delegate.rs && git commit -m "test(meadowcap): pin micros-vs-seconds TimeRange authorisation trap"`

### Task 9 — Conformance golden vectors, negative-form fixtures, seeded generative tests, contract pin

**Files:** Create `crates/riot-core/tests/meadowcap_conformance.rs`, `fixtures/willow/meadowcap-vectors.json`; Modify `fixtures/manifest.json`, `crates/xtask/src/main.rs`.

This task lives in an integration test because `crates/riot-core`'s dev-dependencies already include `willow25` (with `dev`) and `serde_json` — no new dependency, so `cargo_lock_sha256` is unchanged. Golden bytes are reproducible: ed25519 keygen and signing are deterministic, so fixed seeds → fixed canonical bytes → fixed fingerprints.

- [ ] **Write the failing test.** `crates/riot-core/tests/meadowcap_conformance.rs`:

```rust
//! Meadowcap Slice 1 conformance: stable golden capability encodings and
//! fingerprints, negative-form rejection, and seeded generative attenuation
//! checks. Golden vectors are a dependency-drift tripwire against the pinned
//! `willow25`/`meadowcap`. Regenerate intentionally with REGEN=1 (see below).

use ufotofu::codec_prelude::EncodableExt;
use riot_core::meadowcap::codec::{
    decode_read_capability_bounded, decode_read_capability_canonic, decode_write_capability_bounded,
    encode_read_capability,
};
use riot_core::meadowcap::create::{new_communal_read, new_communal_write, new_owned_write};
use riot_core::meadowcap::delegate::delegate_write;
use riot_core::meadowcap::fingerprint::{read_capability_fingerprint, write_capability_fingerprint};
use riot_core::meadowcap::MeadowcapError;
use riot_core::willow::{encode_capability, tai_j2000_micros_from_unix_seconds};
use willow25::authorisation::raw::{Delegation, PossiblyValidWriteCapability};
use willow25::entry::Entry;
use willow25::prelude::{Area, NamespaceId, NamespaceSecret, Path, SubspaceSecret, TimeRange};

// Canonical wire bytes of any encodable willow value (mirrors
// `willow::encode_capability`, which is `pollster::block_on(v.new_vec_storing_encoding())`).
// `pollster` and `ufotofu` are direct `riot-core` deps, so they are available
// to this integration-test crate.
fn encode_value<E: EncodableExt + ?Sized>(v: &E) -> Vec<u8> {
    pollster::block_on(v.new_vec_storing_encoding())
}

const VECTORS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/willow/meadowcap-vectors.json");

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Produce a GENUINELY non-canonical encoding of a valid write capability:
/// bytes that willow25's lenient `Decodable` accepts as the same valid value
/// but its canonical `DecodableCanonic` rejects. Such forms exist because
/// delegation areas carry compact-width timestamps/path-lengths that admit
/// non-minimal widths. The oracle below (lenient-accepts AND canonic-rejects)
/// certifies genuineness no matter how the candidate was produced, so the
/// search transform need not know willow's exact byte layout.
///
/// Determinism: fixed seeds and a fixed search order. If this heuristic ever
/// fails to find a candidate (returns None and the `expect` below fires), the
/// implementer constructs one directly from willow's compact-width encoding
/// spec and keeps the same oracle assertion — do NOT weaken the oracle.
fn find_non_canonical_write_encoding() -> Option<Vec<u8>> {
    use ufotofu::codec_prelude::{Decodable, DecodableCanonic};
    use ufotofu::producer::clone_from_slice;
    use willow25::authorisation::WriteCapability;

    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let cap = new_owned_write(&ns, owner.corresponding_subspace_id());
    let editor_id = SubspaceSecret::from_bytes(&[8u8; 32]).corresponding_subspace_id();
    // Tiny TimeRange bounds -> minimal (1-byte) compact-width timestamp fields,
    // leaving room to widen. Codec canonicity does not depend on authorisation,
    // so small raw bounds are fine for this fixture.
    let area = Area::new(
        Some(editor_id.clone()),
        Path::from_slices(&[b"a"]).expect("path"),
        TimeRange::new(0u64.into(), Some(1u64.into())),
    );
    let one_hop = delegate_write(&cap, &owner, area, editor_id).expect("attenuate");
    let canonical = encode_capability(&one_hop);

    let lenient_ok = |b: &[u8]| {
        let mut p = clone_from_slice(b);
        pollster::block_on(WriteCapability::decode(&mut p)).is_ok()
    };
    let canonic_ok = |b: &[u8]| {
        let mut p = clone_from_slice(b);
        pollster::block_on(WriteCapability::decode_canonic(&mut p)).is_ok()
    };

    // Widen a compact-width field: set a bit in a candidate tag byte and insert
    // leading zero bytes after it. Bounded, deterministic search.
    for i in 0..canonical.len() {
        for extra in [1usize, 3, 7] {
            for bit in 0..8u8 {
                let mut m = canonical.clone();
                m[i] |= 1 << bit;
                let mut widened = m[..=i].to_vec();
                widened.extend(std::iter::repeat(0u8).take(extra));
                widened.extend_from_slice(&m[i + 1..]);
                if lenient_ok(&widened) && !canonic_ok(&widened) {
                    return Some(widened);
                }
            }
        }
    }
    None
}

/// The deterministic vector set. Add rows here; never edit committed hex by hand.
fn build_vectors() -> serde_json::Value {
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();

    // 1. owned genesis write cap
    let owned = new_owned_write(&ns, owner_id.clone());

    // 2. one-hop attenuation to /articles for a bounded micros window
    let editor_id = SubspaceSecret::from_bytes(&[8u8; 32]).corresponding_subspace_id();
    let area = Area::new(
        Some(editor_id.clone()),
        Path::from_slices(&[b"articles", b"news"]).expect("path"),
        TimeRange::new(
            tai_j2000_micros_from_unix_seconds(1_700_000_000).unwrap().into(),
            Some(tai_j2000_micros_from_unix_seconds(1_800_000_000).unwrap().into()),
        ),
    );
    let delegated = delegate_write(&owned, &owner, area, editor_id).expect("attenuate");

    // 3. communal read cap
    let read = new_communal_read(NamespaceId::from_bytes(&[16u8; 32]), SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id());

    // 4. authorisation token for an owned-write entry (spec line 1124). Pin the
    //    capability bytes AND the receiver signature bytes. ed25519 signing is
    //    deterministic over the canonical entry encoding, so both are stable.
    let token_entry = Entry::builder()
        .namespace_id(ns.corresponding_namespace_id())
        .subspace_id(owner_id.clone())
        .path(Path::from_slices(&[b"manifest"]).expect("path"))
        .timestamp(1_000u64)
        .payload(b"token-fixture")
        .build();
    let authorised = token_entry
        .into_authorised_entry(&owned, &owner)
        .expect("owner authorises");
    let token = authorised.authorisation_token();

    // 5. a genuinely non-canonical write-capability encoding (spec line 1122).
    let non_canonical = find_non_canonical_write_encoding()
        .expect("a non-canonical encoding exists (compact-width timestamp field)");

    serde_json::json!({
        "owned_write_genesis": {
            "encoding_hex": hex(&encode_capability(&owned)),
            "fingerprint_hex": hex(&write_capability_fingerprint(&owned)),
        },
        "owned_write_one_hop_articles": {
            "encoding_hex": hex(&encode_capability(&delegated)),
            "fingerprint_hex": hex(&write_capability_fingerprint(&delegated)),
        },
        "communal_read_genesis": {
            "encoding_hex": hex(&encode_read_capability(&read)),
            "fingerprint_hex": hex(&read_capability_fingerprint(&read)),
        },
        "owned_write_authorisation_token": {
            "capability_hex": hex(&encode_capability(token.capability())),
            "signature_hex": hex(&encode_value(token.signature())),
        },
        "non_canonical_write_encoding": {
            "encoding_hex": hex(&non_canonical),
        },
    })
}

#[test]
fn non_canonical_encoding_is_rejected_but_leniently_decodable() {
    // spec line 1122: a genuinely non-canonical encoding must be rejected by
    // Riot's canonical decoder (Malformed), while willow25's LENIENT decoder
    // accepts it — the lenient acceptance proves it is genuinely non-canonical
    // (a valid value in a non-minimal wire form), not merely corrupt bytes.
    use ufotofu::codec_prelude::Decodable;
    use ufotofu::producer::clone_from_slice;
    use willow25::authorisation::WriteCapability;

    let bytes = find_non_canonical_write_encoding()
        .expect("a non-canonical encoding exists (compact-width timestamp field)");

    let mut p = clone_from_slice(&bytes);
    assert!(
        pollster::block_on(WriteCapability::decode(&mut p)).is_ok(),
        "lenient decode must accept the non-canonical form (genuineness oracle)"
    );
    assert_eq!(
        decode_write_capability_bounded(&bytes),
        Err(MeadowcapError::Malformed),
        "Riot's canonical decoder must reject the non-canonical form"
    );
}

#[test]
fn golden_vectors_match_committed_fixture() {
    let current = build_vectors();
    if std::env::var("REGEN").is_ok() {
        std::fs::write(VECTORS_PATH, format!("{}\n", serde_json::to_string_pretty(&current).unwrap())).unwrap();
        return;
    }
    let committed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(VECTORS_PATH).expect("vectors file")).expect("valid json");
    assert_eq!(current, committed, "capability encodings/fingerprints drifted from committed golden vectors");
}

#[test]
fn reencoding_a_decoded_capability_is_byte_identical() {
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let cap = new_owned_write(&ns, owner.corresponding_subspace_id());
    let bytes = encode_capability(&cap);
    let decoded = decode_write_capability_bounded(&bytes).expect("bounded decode");
    assert_eq!(encode_capability(&decoded), bytes);
}

#[test]
fn negative_forms_are_rejected() {
    let read = new_communal_read(NamespaceId::from_bytes(&[16u8; 32]), SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id());
    let mut bytes = encode_read_capability(&read);

    // trailing byte
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(decode_read_capability_canonic(&trailing), Err(MeadowcapError::TrailingBytes));

    // flipped signature byte -> Malformed. Canonical decode's is_valid() rejects
    // the bad signature; there is no separate NonCanonical code (single-variant
    // assertion, not an OR-match that could pass vacuously).
    if let Some(last) = bytes.last_mut() {
        *last ^= 0xff;
    }
    assert_eq!(decode_read_capability_canonic(&bytes), Err(MeadowcapError::Malformed));
}

#[test]
fn wrong_access_mode_bytes_are_rejected() {
    // Read-capability bytes fed to the WRITE decoder (and vice versa) must be
    // rejected. The wrapper's decode errors when the decoded genesis access mode
    // is the other mode (meadowcap-0.5.0 raw/possibly_valid_write_capability.rs:997/:1050);
    // Riot surfaces that as Malformed. (spec line 1122)
    let read = new_communal_read(
        NamespaceId::from_bytes(&[16u8; 32]),
        SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id(),
    );
    let read_bytes = encode_read_capability(&read);
    assert_eq!(
        decode_write_capability_bounded(&read_bytes),
        Err(MeadowcapError::Malformed),
        "read bytes must not decode as a write capability"
    );

    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let write = new_owned_write(&ns, owner.corresponding_subspace_id());
    let write_bytes = encode_capability(&write);
    assert_eq!(
        decode_read_capability_bounded(&write_bytes),
        Err(MeadowcapError::Malformed),
        "write bytes must not decode as a read capability"
    );
}

#[test]
fn reordered_delegation_chain_is_rejected() {
    // spec line 1122 "reordered chains". FEASIBILITY NOTE: this deliberately
    // uses a COMMUNAL genesis via `PossiblyValidWriteCapability::new_communal(
    // namespace, receiver)` — a 2-arg constructor that takes NO
    // `NamespaceSignature` (willow25 raw/possibly_valid_write_capability.rs:237).
    // The owned constructor `new_owned(namespace, receiver, initial_authorisation)`
    // is NOT usable here because that genesis `NamespaceSignature` is unreachable
    // from riot-core (willow25's `Genesis` exposes no signature accessor). The
    // communal path sidesteps that entirely — no signature reconstruction, no
    // byte-offset splicing. `append_delegation` panics if a delegation's area is
    // not contained in the prior area, so BOTH hops use the SAME area; equal
    // areas satisfy containment in either order, isolating the reorder to the
    // position-bound signatures (each handover includes the prior delegation).
    let namespace = NamespaceId::from_bytes(&[16u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let genesis = new_communal_write(namespace.clone(), owner_id.clone());

    // Both hops grant the SAME area (owner's subspace, path /a, full time).
    let shared_area = Area::new(
        Some(owner_id.clone()),
        Path::from_slices(&[b"a"]).expect("path"),
        TimeRange::new(0u64.into(), Some(u64::MAX.into())),
    );
    let a = SubspaceSecret::from_bytes(&[8u8; 32]);
    let a_id = a.corresponding_subspace_id();
    let one_hop = delegate_write(&genesis, &owner, shared_area.clone(), a_id).expect("hop A");
    let b_id = SubspaceSecret::from_bytes(&[9u8; 32]).corresponding_subspace_id();
    let two_hop = delegate_write(&one_hop, &a, shared_area, b_id).expect("hop B");
    let two_hop_bytes = encode_capability(&two_hop);

    // The valid chain's two delegations, in order [A, B].
    let dels: Vec<Delegation> = two_hop.delegations().to_vec();
    assert_eq!(dels.len(), 2);

    // POSITIVE CONTROL / self-checking precondition: rebuilding the SAME genesis
    // with the SAME delegations IN ORDER via the raw builder must reproduce the
    // valid cap's EXACT bytes, and those bytes must decode. If a future crate
    // bump changes the encoding or delegation layout, this precondition fails
    // loudly here — rather than the reorder assertion below passing for the
    // wrong reason.
    let mut in_order = PossiblyValidWriteCapability::new_communal(namespace.clone(), owner_id.clone());
    in_order.append_delegation(dels[0].clone());
    in_order.append_delegation(dels[1].clone());
    assert_eq!(encode_value(&in_order), two_hop_bytes, "raw in-order rebuild must reproduce the valid encoding");
    assert!(decode_write_capability_bounded(&two_hop_bytes).is_ok(), "valid chain must decode");

    // REORDERED [B, A]: the first delegation now carries B's signature (made by
    // A's key), which fails validation against the genesis receiver. Canonical
    // decode (which runs is_valid over the chain) must reject it.
    let mut reordered = PossiblyValidWriteCapability::new_communal(namespace, owner_id);
    reordered.append_delegation(dels[1].clone());
    reordered.append_delegation(dels[0].clone());
    assert_eq!(
        decode_write_capability_bounded(&encode_value(&reordered)),
        Err(MeadowcapError::Malformed),
        "a position-swapped delegation chain must fail signature validation"
    );
}

#[test]
fn delegation_chain_signature_tamper_is_rejected() {
    // Spec line 157 end-to-end: build a signed OWNED genesis and delegate twice
    // (multi-hop). Canonical decode verifies every namespace and user signature
    // in the chain, so no single byte of the encoding — including the
    // intermediate delegation's 64-byte signature region — can be flipped and
    // still decode to a valid capability.
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let genesis = new_owned_write(&ns, owner_id.clone());

    // hop 1 -> mid: this delegation's signature is the "intermediate" region.
    let mid = SubspaceSecret::from_bytes(&[8u8; 32]);
    let mid_id = mid.corresponding_subspace_id();
    let area1 = Area::new(
        Some(mid_id.clone()),
        Path::from_slices(&[b"articles"]).expect("path"),
        TimeRange::new(
            tai_j2000_micros_from_unix_seconds(1_700_000_000).unwrap().into(),
            Some(tai_j2000_micros_from_unix_seconds(1_800_000_000).unwrap().into()),
        ),
    );
    let one_hop = delegate_write(&genesis, &owner, area1, mid_id.clone()).expect("hop1");

    // hop 2 -> leaf (outer delegation).
    let leaf = SubspaceSecret::from_bytes(&[9u8; 32]).corresponding_subspace_id();
    let area2 = Area::new(
        Some(leaf.clone()),
        Path::from_slices(&[b"articles", b"news"]).expect("path"),
        TimeRange::new(
            tai_j2000_micros_from_unix_seconds(1_700_000_000).unwrap().into(),
            Some(tai_j2000_micros_from_unix_seconds(1_800_000_000).unwrap().into()),
        ),
    );
    let two_hop = delegate_write(&one_hop, &mid, area2, leaf).expect("hop2");
    assert_eq!(two_hop.delegations().len(), 2);

    let good = encode_capability(&two_hop);
    assert!(decode_write_capability_bounded(&good).is_ok(), "pristine chain must decode");

    // Every single-byte corruption must be rejected by canonical decode. The
    // swept set includes the intermediate delegation's signature bytes, so a
    // tampered mid-chain signature is provably rejected.
    for i in 0..good.len() {
        let mut tampered = good.clone();
        tampered[i] ^= 0xff;
        assert!(
            decode_write_capability_bounded(&tampered).is_err(),
            "byte {i} (a signature/key/area/length byte) was not load-bearing: tampered chain still decoded"
        );
    }
}

#[test]
fn seeded_generative_attenuation_never_expands_authority() {
    // Deterministic generative sweep (no proptest dependency): for many seeds,
    // a one-hop delegation's granted area must be contained in the parent's.
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let parent = new_owned_write(&ns, owner_id.clone());

    for seed in 0u8..64 {
        let receiver = SubspaceSecret::from_bytes(&[seed; 32]).corresponding_subspace_id();
        let area = Area::new(
            Some(receiver.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            TimeRange::new(
                tai_j2000_micros_from_unix_seconds(1_700_000_000).unwrap().into(),
                Some(tai_j2000_micros_from_unix_seconds(1_700_000_000 + seed as u64 * 1000).unwrap().into()),
            ),
        );
        let child = delegate_write(&parent, &owner, area.clone(), receiver).expect("attenuate");
        // The parent (owned, Area::full()) must include the child's granted area.
        assert!(parent.includes_area(&child.granted_area()), "child area escaped parent for seed {seed}");
    }
}

#[test]
fn seeded_generative_invalid_trees_are_all_rejected() {
    // spec line 1126: from each generated valid tree, derive INVALID variants
    // and assert every one is rejected. Seeded and deterministic — no rng/clock.
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();

    for seed in 1u8..48 {
        let parent = new_owned_write(&ns, owner_id.clone());
        let receiver = SubspaceSecret::from_bytes(&[seed; 32]).corresponding_subspace_id();
        let area = Area::new(
            Some(receiver.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            TimeRange::new(
                tai_j2000_micros_from_unix_seconds(1_700_000_000).unwrap().into(),
                Some(tai_j2000_micros_from_unix_seconds(1_800_000_000).unwrap().into()),
            ),
        );
        let valid = delegate_write(&parent, &owner, area.clone(), receiver.clone()).expect("valid tree");

        // (1) Wrong-signer delegation attempt: a non-receiver secret must fail.
        let impostor = SubspaceSecret::from_bytes(&[seed.wrapping_add(128); 32]);
        assert_eq!(
            delegate_write(&valid, &impostor, area.clone(), receiver.clone()),
            Err(MeadowcapError::ReceiverMismatch),
            "wrong-signer delegation must be rejected (seed {seed})"
        );

        // (2) Seeded byte-flip corruption of the valid encoding must not decode.
        let bytes = encode_capability(&valid);
        let offset = (seed as usize).wrapping_mul(7) % bytes.len();
        let mut corrupt = bytes.clone();
        corrupt[offset] ^= 0xff;
        assert!(
            decode_write_capability_bounded(&corrupt).is_err(),
            "byte-flip at {offset} must be rejected (seed {seed})"
        );

        // (3) Over-depth chain (17 hops) must be rejected by the depth ceiling.
        // First signer is the genesis receiver (a fresh secret of the same
        // fixed bytes as `owner`, so no `SubspaceSecret` clone is needed).
        let mut deep = new_owned_write(&ns, owner_id.clone());
        let mut signer = SubspaceSecret::from_bytes(&[4u8; 32]);
        for i in 0..17u8 {
            let next = SubspaceSecret::from_bytes(&[seed.wrapping_add(i).wrapping_add(1); 32]);
            deep = delegate_write(&deep, &signer, Area::full(), next.corresponding_subspace_id()).expect("hop");
            signer = next;
        }
        assert_eq!(
            decode_write_capability_bounded(&encode_capability(&deep)),
            Err(MeadowcapError::ChainTooDeep { depth: 17, max: 16 }),
            "over-depth chain must be rejected (seed {seed})"
        );
    }
}
```

  The over-depth loop re-grants `Area::full()` each hop; if willow25's `try_delegate` rejects a same-area re-grant (see the Task 5 note), replace it with a strictly-nested path sequence (`/a`, `/a/b`, …) kept ≥17 deep — adjust construction only, never the asserted ceiling.

  Note: `riot_core::meadowcap::codec`/`create`/etc. must be `pub` paths — the `mod.rs` submodule declarations are `pub mod`, satisfying this. If `riot-core` gates any of these behind a feature, expose the conformance surface without the release-graph `conformance` feature (these are ordinary `pub` items, not injectable constructors, so no gate is needed). `Entry::into_authorised_entry`, `AuthorisedEntry::authorisation_token`, and `WriteCapability::decode`/`decode_canonic` are willow25 methods reached through its prelude/traits (`Entry`, `AuthorisedEntry`, and `ufotofu::codec_prelude::{Decodable, DecodableCanonic}`); if a call does not resolve, add the specific willow25 trait/type import (the compiler names it) rather than a prelude glob, to avoid name clashes with the meadowcap re-exports.

- [ ] **Run it and watch it fail.** `cargo test -p riot-core --test meadowcap_conformance` — expected failure: `golden_vectors_match_committed_fixture` panics (fixture file absent). Generate it: `REGEN=1 cargo test -p riot-core --test meadowcap_conformance golden_vectors_match_committed_fixture` (this also pins the authorization-token and non-canonical vectors), then re-run without `REGEN` — all nine tests pass: `golden_vectors_match_committed_fixture`, `reencoding_a_decoded_capability_is_byte_identical`, `negative_forms_are_rejected`, `wrong_access_mode_bytes_are_rejected`, `reordered_delegation_chain_is_rejected`, `delegation_chain_signature_tamper_is_rejected`, `non_canonical_encoding_is_rejected_but_leniently_decodable`, `seeded_generative_attenuation_never_expands_authority`, `seeded_generative_invalid_trees_are_all_rejected`.
- [ ] **Pin the fixture as a contract.** Compute the hash and record it:
  - `shasum -a 256 fixtures/willow/meadowcap-vectors.json` (note the hex).
  - Add `"meadowcap_vectors_sha256": "<hex>"` to the `env` object in `fixtures/manifest.json`, next to `william3_vectors_sha256`.
  - In `crates/xtask/src/main.rs`, immediately after the `william3_vectors_sha256` block (~line 641), add the mirror:

```rust
match (
    env["meadowcap_vectors_sha256"].as_str(),
    std::fs::read(root.join("fixtures/willow/meadowcap-vectors.json")),
) {
    (Some(recorded), Ok(bytes)) => {
        let actual = sha256_hex(&bytes);
        if recorded != actual {
            failures.push(format!(
                "fixtures/manifest.json: meadowcap_vectors_sha256 mismatch (recorded {recorded}, actual {actual})"
            ));
        }
    }
    _ => failures.push(
        "fixtures/manifest.json: meadowcap_vectors_sha256 missing/empty or vectors file unreadable".into(),
    ),
}
```

  Use whatever helper the `william3` block uses to hash (`sha256_hex` or inline `Sha256`); match its exact style so the diff is minimal. Do **not** touch `cargo_lock_sha256` — no dependency was added.

- [ ] **Verify the contract gate.** `cargo xtask validate-contracts` — expected: `validate-contracts: PASS`. If it reports a `meadowcap_vectors_sha256 mismatch`, copy the printed `actual` into `fixtures/manifest.json`.
- [ ] **Commit.** `git add crates/riot-core/tests/meadowcap_conformance.rs fixtures/willow/meadowcap-vectors.json fixtures/manifest.json crates/xtask/src/main.rs && git commit -m "test(meadowcap): golden vectors, negative forms, seeded attenuation; pin vectors as contract"`

---

## Verification (run before declaring the slice complete)

Scoped iteration during development uses `cargo test -p riot-core meadowcap` and `cargo test -p riot-core --test meadowcap_conformance`. **Final verification must be the full workspace** — this repo has been burned by scoped `-p` tests hiding cross-crate breaks in matched-on enums (a `riot-core` enum change silently broke `riot-ffi`'s `match` for ~7 commits). Run, in order, and confirm each is green:

- [ ] `cargo build --workspace --all-features`
- [ ] `cargo test --workspace --all-features`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-features -- -D warnings`
- [ ] `cargo xtask validate-contracts` — must print `validate-contracts: PASS (structural + resolved feature graph)`; confirms the `willow25`/`bab_rs` pins are intact, the `meadowcap_vectors_sha256` fixture is pinned, and `cargo_lock_sha256` is unchanged (no dependency was added).
- [ ] Coverage: `.coverage-thresholds.json` is the source of truth (tarpaulin lines floor 97, llvm lines 95 / branches 83). Run the CI gate `cargo llvm-cov --workspace --all-features --fail-under-lines 95` (or the local composite `scripts/web/coverage.sh`). New `meadowcap` code is small, pure, and heavily tested; keep it at or above the floors. Do not lower a floor.

**Self-review checklist for the implementer:** every Slice-1 requirement from the design's "Meadowcap core" list maps to a task —
- Construct communal/owned read+write caps → Task 1.
- Canonically encode / exactly decode → Tasks 2–3.
- Delegate with area restriction (attenuation only) → Task 5.
- Inspect access mode / receiver / namespace / granted area / chain depth → Task 4.
- **Verify every namespace and user signature in a delegation chain (spec line 157)** → performed by the reused canonical decode path (Tasks 2–3), pinned end-to-end by Task 9's `delegation_chain_signature_tamper_is_rejected`; documented in Task 6.
- Verify an entry-authorisation token against its entry → Task 6, reusing `verify_entry`.
- Verify a read request is covered by a read capability (namespace + area) → Task 6 (`read_request_covered`), incl. wrong-namespace rejection.
- **Read halves of the capability surface (spec lines 153/155/156)** — all exercised, mirroring the write side:
  - Read-capability creation → Task 1 (`communal_read_cap_...`) + Task 2 (`owned_read_cap_is_owned_full_area_and_round_trips`).
  - Read-capability canonical codec / round-trip → Task 2 (`read_capability_roundtrips_canonically`, `owned_read_cap_is_owned_full_area_and_round_trips`).
  - Read-capability delegation (attenuation + widening rejection + bounded-decode happy path) → Task 5 (`read_delegation_narrows_area_moves_receiver_and_rejects_widening`).
  - Read-capability inspection/summary → Task 4 (`owned_read_summary_reports_read_owned_and_receiver_not_namespace`).
- Communal-write round-trip is covered by Task 9's reordered-chain positive control (`decode_write_capability_bounded(&two_hop_bytes).is_ok()` over a communal 2-hop chain); owned-write round-trip by `reencoding_a_decoded_capability_is_byte_identical`.
- Return the capability receiver separately from the entry subspace → Task 4.
- Capability fingerprints → Task 7.
- Stable rejection codes, incl. `ReceiverMismatch` actually produced and tested (wrong-signer, Task 5) and chain-signature tamper (Task 9) → Task 0 enum + Tasks 3/5/9.
- Structural ceilings (depth 16, 64 KiB) → Task 3.

Core-conformance coverage (design ~lines 1116–1130) mapped explicitly:
- **Non-canonical forms rejected (line 1122)** → `NonCanonical` deleted as unreachable; folds to `Malformed` (documented on the enum with crate citation); pinned by Task 9 `non_canonical_encoding_is_rejected_but_leniently_decodable` with a lenient-vs-canonic genuineness oracle.
- **Reordered chains rejected (line 1122)** → Task 9 `reordered_delegation_chain_is_rejected` (raw PossiblyValid swapped-append).
- **Wrong modes rejected (line 1122)** → Task 9 `wrong_access_mode_bytes_are_rejected` (read bytes to write decoder and vice versa → `Malformed`).
- **Tampered signatures / reordered / trailing / wrong receiver (lines 1121–1123)** → Task 9 tamper sweep + Task 5 wrong-signer.
- **Wrong namespaces rejected (line 1123)** → Task 6 cross-namespace `token_authorises_entry` and `read_request_covered` false-assertions.
- **Stable golden capability AND authorization-token bytes (line 1124)** → Task 9 `owned_write_authorisation_token` vector (capability + signature hex) under the `meadowcap_vectors_sha256` pin.
- **Property tests over generated valid AND invalid trees (line 1126)** → Task 9 `seeded_generative_attenuation_never_expands_authority` (valid) + `seeded_generative_invalid_trees_are_all_rejected` (wrong-signer, byte-flip, over-depth), all seeded/deterministic.
- Conformance golden + negative-form + chain-tamper + generative fixtures → Task 9.

Anything in that design paragraph not listed here belongs to a later slice and is out of scope.
