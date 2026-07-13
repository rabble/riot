# Newswire Core Slice 1 Implementation Plan

Plan review gate: **REVISED AFTER ITERATION 2; PENDING. Do not execute until all three plan reviewers pass this exact text.**

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Riot core create, import, retain, and deterministically project one community's signed open Newswire records end to end.

**Architecture:** Add one focused `newswire` module beside the existing alert and app protocols. Canonical CBOR payloads and content-derived Willow paths are validated at import; the existing evidence store retains verified entry facts plus exact Newswire payload bytes for projection; a pure reducer derives the open wire, collective front page, editorial history, quarantine, and Earlier view from a descriptor-pinned record set. Complete signed bytes remain in the existing caller/transport inventory rather than being duplicated in `JoinState`. This is the first executable vertical slice: native UI, browser/WASM, gateway rendering, sync-inventory persistence, multi-space persistence, and media remain separate plans after the core contract is proven.

**Tech Stack:** Rust 2021, `minicbor`, Willow'25/Meadowcap, SHA-256 Riot `EntryId`, WILLIAM3 payload digests, existing `RiotSession` evidence store, Rust integration tests, committed golden vectors.

---

## Scope boundary and usable outcome

This plan is complete when a Rust integration test can:

1. create a signed descriptor with a fixed editorial roster;
2. create freeform, operational-alert, and operational-request posts from two
   ordinary communal authors;
3. create feature, verify, correct, hide, tombstone, and retract actions from a recognized editor;
4. import the identical signed bytes through the ordinary evidence bundle path;
5. rebuild the same projection from a fresh store; and
6. prove an unknown editor, cross-space action, bad path binding, future-dated record, and gateway-local ordering cannot alter the collective projection.

This plan deliberately does not add Swift, Kotlin, JavaScript, WASM, HTTP,
directory, governance, media, roster rotation, or gateway code. The next plan
may expose this stable core through FFI/native UI without changing the wire
format.

The current store intentionally discards capability/signature evidence after
verified admission. This slice does not change that ownership boundary:
`SignedNewswireRecord` supplies complete bytes to the existing transport or
mobile inventory, while `EvidenceStore` retains only the verified Willow entry
and Newswire payload needed to rebuild projections. Both fresh-store rebuild
tests import the same committed complete signed fixtures; they never claim the
projection store can re-export evidence.

Backward compatibility is one-way and explicit: no released record family
currently owns the reserved `newswire/v1` prefix, so adding its closed schema
branch cannot reinterpret a valid alert, app, app-index, or profile entry.
Removing the module and its two admission branches restores the prior behavior;
existing record codecs and path families are not rewritten. Newswire payload
retention uses the store's existing payload-byte accounting and limits.

## Frozen wire limits

These limits are protocol constants in `newswire/model.rs`, measured in UTF-8
bytes after canonical decoding:

| Field | Limit |
| --- | ---: |
| Complete Newswire payload | 131,072 bytes |
| Space name | 256 bytes |
| Space summary | 4,096 bytes |
| Languages | 16 entries, 2–35 bytes each |
| Geographic tags | 32 entries, 1–128 bytes each |
| Topic tags | 32 entries, 1–128 bytes each |
| Editorial roster | 64 unique 32-byte `SubspaceId` values |
| Headline | 512 bytes |
| Body or editorial correction | 65,536 bytes |
| Coarse location | 2,048 bytes |
| Source claims | 16 entries, 1–1,024 bytes each |
| Request contact instructions | 2,048 bytes |
| Editorial reason | 4,096 bytes |
| Records accepted by one projection call | 1,024 |

All text must be non-empty after `trim()` when present. Unknown keys, duplicate
or misordered keys, indefinite arrays/maps, invalid UTF-8, trailing bytes,
wrong schemas, and non-canonical re-encodings fail closed.

## File map

- Create `crates/riot-core/src/newswire/mod.rs` — public Newswire types and exports.
- Create `crates/riot-core/src/newswire/model.rs` — payload types, validation, and strict canonical CBOR codecs.
- Create `crates/riot-core/src/newswire/path.rs` — content-derived Willow path construction and classification.
- Create `crates/riot-core/src/newswire/entry.rs` — signed descriptor/post/action factories and structural inspection.
- Create `crates/riot-core/src/newswire/projection.rs` — pure deterministic reducer and presentation-safe output.
- Create `crates/riot-core/src/newswire/store.rs` — scan a verified `EvidenceStore` and project one pinned descriptor.
- Modify `crates/riot-core/src/lib.rs` — export `newswire`.
- Modify `crates/riot-core/src/session.rs` — structurally admit and retain valid Newswire payloads.
- Modify `crates/riot-core/src/import/bundle.rs` — recognize reserved Newswire paths before alert fallback.
- Modify `crates/riot-core/src/import/join.rs` — update retained-payload documentation only; no join semantic change.
- Modify `crates/riot-core/Cargo.toml` — register conformance integration tests.
- Create `crates/riot-core/tests/newswire_codec.rs`.
- Create `crates/riot-core/tests/newswire_entry.rs`.
- Create `crates/riot-core/tests/newswire_import.rs`.
- Create `crates/riot-core/tests/newswire_projection.rs`.
- Create `crates/riot-core/tests/newswire_end_to_end.rs`.
- Create `crates/riot-core/examples/pack_newswire_vectors.rs`.
- Create `fixtures/newswire/manifest.json`, three `.cbor` payload fixtures, and three complete `.riot-evidence` signed-record fixtures.
- Create `scripts/newswire/repack-vectors.sh`.

No FFI or application file is touched in this slice.

## Task 1: Freeze canonical Newswire payloads

**Files:**
- Create: `crates/riot-core/src/newswire/mod.rs`
- Create: `crates/riot-core/src/newswire/model.rs`
- Modify: `crates/riot-core/src/lib.rs`
- Modify: `crates/riot-core/Cargo.toml`
- Test: `crates/riot-core/tests/newswire_codec.rs`

- [ ] **Step 1: Write failing public codec tests**

Define fixtures in the test with these public types and exact field names:

```rust
use riot_core::newswire::{
    decode_editorial_action, decode_news_post, decode_space_descriptor,
    encode_editorial_action, encode_news_post, encode_space_descriptor,
    AlertProfileV1, EditorialActionKind, EditorialActionV1, NewsPostV1,
    OperationalProfileV1, RequestKind, RequestProfileV1, SpaceDescriptorV1,
};

const SPACE_ID: [u8; 32] = [0x11; 32];
const POST_ID: [u8; 32] = [0x22; 32];

fn space() -> SpaceDescriptorV1 {
    SpaceDescriptorV1 {
        namespace_id: [0x10; 32],
        name: "Riverside Independent Media".into(),
        summary: "Open publishing by and for the community.".into(),
        languages: vec!["en".into()],
        geographic_tags: vec!["riverside".into()],
        topic_tags: vec!["community-media".into()],
        editorial_roster: vec![[0x20; 32]],
        predecessor: None,
        successor: None,
    }
}

#[test]
fn all_newswire_payloads_round_trip_canonically() {
    let post = NewsPostV1 {
        space_descriptor_entry_id: SPACE_ID,
        headline: "Night march reaches the square".into(),
        body: "Witness report from the community assembly.".into(),
        language: "en".into(),
        event_time_unix_seconds: Some(1_800_000_100),
        expires_at_unix_seconds: None,
        coarse_location: Some("central district".into()),
        source_claims: vec!["participant account".into()],
        operational_profile: None,
        ai_assisted: false,
    };
    let action = EditorialActionV1 {
        space_descriptor_entry_id: SPACE_ID,
        target_entry_id: POST_ID,
        kind: EditorialActionKind::Correct,
        reason: Some("Clarifies the assembly's decision.".into()),
        correction_text: Some("The assembly reconvenes Friday.".into()),
    };
    assert_eq!(decode_space_descriptor(&encode_space_descriptor(&space()).unwrap()).unwrap(), space());
    assert_eq!(decode_news_post(&encode_news_post(&post).unwrap()).unwrap(), post);
    assert_eq!(decode_editorial_action(&encode_editorial_action(&action).unwrap()).unwrap(), action);
}
```

Add table-driven RED cases for every frozen boundary, duplicate roster keys,
invalid action-field combinations, operational alert and request requirements,
unknown CBOR keys, misordered keys, indefinite maps, trailing bytes, and a
one-byte non-canonical integer encoding.

Run:

```bash
cargo test -p riot-core --features conformance --test newswire_codec
```

Expected: FAIL because `riot_core::newswire` does not exist.

- [ ] **Step 2: Add the closed data model and exact schema maps**

Create the following public model, without serde or open-ended value fields:

```rust
pub const SPACE_SCHEMA: &str = "org.riot.newswire.space/1";
pub const POST_SCHEMA: &str = "org.riot.newswire.post/1";
pub const ACTION_SCHEMA: &str = "org.riot.newswire.editorial-action/1";
pub const MAX_NEWSWIRE_PAYLOAD_BYTES: usize = 131_072;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceDescriptorV1 {
    pub namespace_id: [u8; 32],
    pub name: String,
    pub summary: String,
    pub languages: Vec<String>,
    pub geographic_tags: Vec<String>,
    pub topic_tags: Vec<String>,
    pub editorial_roster: Vec<[u8; 32]>,
    pub predecessor: Option<[u8; 32]>,
    pub successor: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertProfileV1 {
    pub urgency: crate::model::Urgency,
    pub severity: crate::model::Severity,
    pub certainty: crate::model::Certainty,
    pub valid_from_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind { Need = 0, Offer = 1 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestProfileV1 {
    pub kind: RequestKind,
    pub needed_by_unix_seconds: Option<u64>,
    pub contact_instructions: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationalProfileV1 {
    Alert(AlertProfileV1),
    Request(RequestProfileV1),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsPostV1 {
    pub space_descriptor_entry_id: [u8; 32],
    pub headline: String,
    pub body: String,
    pub language: String,
    pub event_time_unix_seconds: Option<u64>,
    pub expires_at_unix_seconds: Option<u64>,
    pub coarse_location: Option<String>,
    pub source_claims: Vec<String>,
    pub operational_profile: Option<OperationalProfileV1>,
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorialActionKind {
    Feature = 0, Verify = 1, Correct = 2,
    Hide = 3, Tombstone = 4, Retract = 5,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorialActionV1 {
    pub space_descriptor_entry_id: [u8; 32],
    pub target_entry_id: [u8; 32],
    pub kind: EditorialActionKind,
    pub reason: Option<String>,
    pub correction_text: Option<String>,
}
```

Encode definite CBOR maps with ascending integer keys. Space keys are `0..=9`,
post keys are `0..=10`, and action keys are `0..=5`; optional keys are omitted.
Key `0` is always the schema string. `OperationalProfileV1` is a closed map:
key `0` is `0` for Alert or `1` for Request, and key `1` contains the closed
variant map. Alert carries urgency, severity, certainty, and optional
valid-from; it requires post expiry, coarse location, and at least one source
claim. Request carries Need/Offer, optional needed-by, and required contact
instructions; it requires post expiry and coarse location. No other profile
tag is accepted. Export one `NewswireModelError` enum with stable variants
naming every validation failure; do not return raw minicbor errors.

Register only the test file created in this task:

```toml
[[test]]
name = "newswire_codec"
required-features = ["conformance"]
```

- [ ] **Step 3: Prove canonical and hostile cases pass**

Run:

```bash
cargo test -p riot-core --features conformance --test newswire_codec
cargo test -p riot-core --all-features
```

Expected: all codec tests PASS and existing tests remain green.

- [ ] **Step 4: Commit Task 1**

```bash
git add crates/riot-core/src/lib.rs crates/riot-core/src/newswire \
  crates/riot-core/tests/newswire_codec.rs crates/riot-core/Cargo.toml
git diff --cached --check
git commit -m "feat(newswire): add canonical payload codecs"
```

## Task 2: Bind payloads to incomparable Willow paths and signatures

**Files:**
- Create: `crates/riot-core/src/newswire/path.rs`
- Create: `crates/riot-core/src/newswire/entry.rs`
- Modify: `crates/riot-core/src/newswire/mod.rs`
- Modify: `crates/riot-core/Cargo.toml`
- Test: `crates/riot-core/tests/newswire_entry.rs`

- [ ] **Step 1: Write failing path, signature, and authority tests**

Use deterministic authors and clocks behind `conformance`. Assert these exact
properties:

```rust
let digest = william3_digest(&payload_bytes);
let expected = Path::from_slices(&[
    b"newswire", b"v1", &space_id, b"posts",
    &snapshot.tai_j2000_micros.to_be_bytes(), &digest,
]).unwrap();
assert_eq!(entry.path(), &expected);
assert_eq!(entry.payload_digest_bytes(), digest);
assert!(verify_entry(&entry, &token));
```

Add RED tests proving:

- descriptor paths are `newswire/v1/descriptors/<u64be-time>/<digest>`;
- action paths use the pinned descriptor plus `actions`;
- equal signer/time/payload produces the same Riot `EntryId`;
- distinct payloads at equal depth do not prune each other under `plan_join`;
- descriptor creation rejects `organizer_subspace_id != namespace_id`;
- a post author must be in the descriptor namespace;
- an action factory rejects an editor absent from the fixed roster;
- `inspect_news_record` rejects path time/digest mismatch, payload/signer
  mismatch, wrong descriptor path component, malformed payload, bad capability,
  and bad signature.

Run:

```bash
cargo test -p riot-core --features conformance --test newswire_entry
```

Expected: FAIL because the path and factory APIs do not exist.

Register `newswire_entry` only when its file exists:

```toml
[[test]]
name = "newswire_entry"
required-features = ["conformance"]
```

- [ ] **Step 2: Add exact path builders and a closed classifier**

Expose only typed builders and this classifier shape:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewswirePathKind {
    Descriptor,
    Post { space_descriptor_entry_id: EntryId },
    EditorialAction { space_descriptor_entry_id: EntryId },
}

pub fn newswire_path(
    kind: NewswirePathKind,
    tai_j2000_micros: u64,
    payload_digest: &[u8; 32],
) -> Result<Path, NewswireError>;

pub fn classify_newswire_path(path: &Path) -> Option<(
    NewswirePathKind,
    u64,
    [u8; 32],
)>;
```

Classification accepts exact component counts and raw byte lengths only. The
timestamp component is eight-byte big-endian; the digest and descriptor ID are
32 bytes. A path merely beginning with `newswire/v1` but not matching a closed
family is invalid, never an alert.

- [ ] **Step 3: Add signed factories and structural inspection**

Use one shared internal function that takes `ClockSnapshot`, encodes the
payload, computes WILLIAM3, constructs the typed path, authorizes the entry, and
returns:

```rust
pub struct SignedNewswireRecord {
    pub signed: SignedWillowEntry,
    pub entry_id: EntryId,
    pub snapshot: ClockSnapshot,
    pub payload: NewswirePayload,
}

pub enum NewswirePayload {
    SpaceDescriptor(SpaceDescriptorV1),
    NewsPost(NewsPostV1),
    EditorialAction(EditorialActionV1),
}

pub struct VerifiedNewswireRecord {
    pub entry_id: EntryId,
    pub namespace_id: [u8; 32],
    pub signer_id: [u8; 32],
    pub tai_j2000_micros: u64,
    pub payload: NewswirePayload,
}

pub fn inspect_news_record(
    signed: &SignedWillowEntry,
) -> Result<VerifiedNewswireRecord, NewswireError>;

pub(crate) fn inspect_verified_components(
    entry: &Entry,
    payload_bytes: &[u8],
) -> Result<VerifiedNewswireRecord, NewswireError>;
```

Production factories call `system_snapshot()` and expose no injectable clock.
`*_with_clock` variants exist only under `conformance`. Structural inspection
verifies canonical entry/capability decoding, capability/signature, payload
digest/length, canonical payload bytes, exact path time/digest, descriptor
founder binding, and envelope/payload duplicated values. Descriptor-dependent
post/action authority is checked later against the pinned descriptor.

- [ ] **Step 4: Run and commit Task 2**

```bash
cargo test -p riot-core --features conformance --test newswire_entry
cargo test -p riot-core --all-features
git add crates/riot-core/src/newswire crates/riot-core/tests/newswire_entry.rs \
  crates/riot-core/Cargo.toml
git diff --cached --check
git commit -m "feat(newswire): sign descriptor-bound records"
```

Expected: all tests PASS; no new Newswire entry can prune a distinct equal-depth
record.

## Task 3: Admit and retain Newswire records through the ordinary store

**Files:**
- Modify: `crates/riot-core/src/import/bundle.rs`
- Modify: `crates/riot-core/src/session.rs`
- Modify: `crates/riot-core/src/import/join.rs`
- Create: `crates/riot-core/src/newswire/store.rs`
- Modify: `crates/riot-core/src/newswire/mod.rs`
- Modify: `crates/riot-core/Cargo.toml`
- Test: `crates/riot-core/tests/newswire_import.rs`

- [ ] **Step 1: Write failing import tests**

Build a bundle containing one descriptor, two posts, and one editorial action.
Import it through `RiotSession::open()`, `create_store()`, `inspect()`,
`plan_all()`, and `commit()`. Assert four eligible records, four live complete
EntryIds, retained payload bytes under the descriptor/post/action prefixes,
idempotent re-import, and order-independent import.

Add mixed-bundle tests proving one invalid Newswire sibling does not poison a
valid sibling, and a malformed `newswire/v1/...` entry never falls through to
alert or opaque app admission.

Run:

```bash
cargo test -p riot-core --features conformance --test newswire_import
```

Expected: FAIL because bundle schema verification rejects Newswire before
session admission can recognize or retain it.

Register `newswire_import` only when its file exists:

```toml
[[test]]
name = "newswire_import"
required-features = ["conformance"]
```

- [ ] **Step 2: Reserve Newswire at bundle verification, then retain it in `inspect_inner`**

In `import/bundle.rs`, add this branch inside `AppIndexSlot::None`, before the
existing malformed reserved-path and profile/alert fallback:

```rust
if crate::newswire::is_newswire_prefix(entry.path()) {
    crate::newswire::inspect_verified_components(&entry, &frame.payload_bytes).is_ok()
} else {
    let is_malformed_reserved_path =
        entry.path().components().next().is_some_and(|component| {
            let component = component.as_ref();
            component == crate::apps::index::APP_INDEX_COMPONENT
                || component == crate::apps::entry::APPS_COMPONENT
        });
    if is_malformed_reserved_path {
        false
    } else if crate::profile::path::is_profile_prefixed(entry.path()) {
        crate::profile::path::classify_profile_path(entry.path()).is_some()
            && crate::profile::card::decode_profile_card(&frame.payload_bytes).is_ok()
    } else {
        crate::model::decode_alert(&frame.payload_bytes).is_ok()
    }
}
```

This makes `newswire/v1` a reserved family: malformed Newswire never falls
through to alert decoding, while an invalid item remains isolated from valid
siblings by the existing item-status machinery.

Compute Newswire classification before the alert fallback:

```rust
let path = willow25::groupings::Keylike::path(authorised.entry());
let valid_newswire = crate::newswire::is_newswire_prefix(path)
    && crate::newswire::inspect_verified_components(
        authorised.entry(),
        item.frame.payload_bytes(),
    )
    .is_ok();
let path_matches = if is_app_data {
    true
} else if let Some(slot) = app_index_slot {
    match slot {
        crate::apps::index::AppIndexSlot::Endorsement {
            endorser_subspace_id,
            ..
        } => {
            *willow25::groupings::Keylike::subspace_id(authorised.entry()).as_bytes()
                == endorser_subspace_id
        }
        crate::apps::index::AppIndexSlot::Trust {
            organizer_subspace_id,
            ..
        } => {
            *willow25::groupings::Keylike::subspace_id(authorised.entry()).as_bytes()
                == organizer_subspace_id
        }
        crate::apps::index::AppIndexSlot::Manifest { .. }
        | crate::apps::index::AppIndexSlot::Bundle { .. } => true,
    }
} else if let Some(subspace_id) = profile_subspace {
    *willow25::groupings::Keylike::subspace_id(authorised.entry()).as_bytes()
        == subspace_id
} else if crate::newswire::is_newswire_prefix(path) {
    valid_newswire
} else {
    decode_alert(item.frame.payload_bytes())
        .ok()
        .and_then(|alert| {
            alert_entry_path_matches_payload(
                item.frame.entry_bytes(),
                &alert.object_id,
                &alert.revision_id,
            )
            .ok()
        })
        .unwrap_or(false)
};
let retain_payload = is_app_data
    || app_index_slot.is_some()
    || profile_subspace.is_some()
    || valid_newswire;
```

Also add a public `is_newswire_prefix(&Path) -> bool` that returns true only
when the first two raw components are `newswire` and `v1`.

Update `join.rs` comments from “app-data and app-index” to “typed consumers that
must rebuild from payload bytes”; do not change Willow join semantics.

- [ ] **Step 3: Add typed store scans**

Implement:

```rust
pub fn load_space_descriptor(
    store: &EvidenceStore,
    descriptor_id: EntryId,
) -> Result<VerifiedNewswireRecord, NewswireStoreError>;

pub fn load_space_records(
    store: &EvidenceStore,
    descriptor_id: EntryId,
) -> Result<Vec<VerifiedNewswireRecord>, NewswireStoreError>;
```

The descriptor scan uses `newswire/v1/descriptors`; post/action scans use the
exact pinned descriptor prefix. Every retained payload is decoded again and
matched to the stored entry's path, digest, namespace, signer, timestamp, and
EntryId. An absent descriptor, duplicate descriptor ID, missing retained
payload, malformed retained record, or more than 1,024 records returns a stable
typed error instead of a partial projection.

- [ ] **Step 4: Run and commit Task 3**

```bash
cargo test -p riot-core --features conformance --test newswire_import
cargo test -p riot-core --all-features
git add crates/riot-core/src/session.rs crates/riot-core/src/import/join.rs \
  crates/riot-core/src/import/bundle.rs crates/riot-core/src/newswire \
  crates/riot-core/tests/newswire_import.rs crates/riot-core/Cargo.toml
git diff --cached --check
git commit -m "feat(newswire): admit signed records into the evidence store"
```

## Task 4: Implement the deterministic editorial projection

**Files:**
- Create: `crates/riot-core/src/newswire/projection.rs`
- Modify: `crates/riot-core/src/newswire/store.rs`
- Modify: `crates/riot-core/src/newswire/mod.rs`
- Modify: `crates/riot-core/Cargo.toml`
- Test: `crates/riot-core/tests/newswire_projection.rs`

- [ ] **Step 1: Write the failing reducer matrix**

Construct verified records directly with fixed IDs and times. Cover every row
of this matrix in named tests:

| Case | Expected current result |
| --- | --- |
| No posts | empty Open wire and Front page |
| Eligible posts | descending `(TAI time, EntryId)` |
| exact duplicate `EntryId` inputs | one logical record and one projection row |
| exactly 1,024 distinct records | accepted |
| 1,025 distinct records | stable `PROJECTION_LIMIT_EXCEEDED` error |
| Expired post | Earlier only |
| `time > clock.tai + 600_000_000` | quarantine only |
| future-cutoff overflow | `CLOCK_OUT_OF_RANGE` |
| active feature | Front page, greatest feature key |
| active verify | every signer retained, no score |
| active correct | immutable original plus ordered corrections |
| active hide | warning projection, body behind inspectable detail |
| active tombstone | no body, source, location, or correction text in ordinary projection |
| later valid retract | target action inactive, both remain in history |
| retract of retract/later/missing/wrong-space target | no effect |
| feature/verify/correct/hide/tombstone targeting absent, action, or wrong-space post | no effect |
| unknown editor or forged descriptor binding | no collective effect |
| arrival permutations | byte-for-byte equal projection |

Run:

```bash
cargo test -p riot-core --features conformance --test newswire_projection
```

Expected: FAIL because the reducer does not exist.

Register `newswire_projection` only when its file exists:

```toml
[[test]]
name = "newswire_projection"
required-features = ["conformance"]
```

The overflow case is a unit test inside `projection.rs`, where the test module
can construct the private clock fields at `u64::MAX`; no production or
conformance API accepts independently chosen Unix and TAI values.

- [ ] **Step 2: Add presentation-safe projection types**

Use complete IDs and named state, never a blended ranking:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionClockV1 {
    unix_seconds: u64,
    tai_j2000_micros: u64,
}

impl ProjectionClockV1 {
    pub fn system() -> Result<Self, crate::willow::WillowError> {
        Ok(Self::from_snapshot(crate::willow::system_snapshot()?))
    }

    pub fn unix_seconds(&self) -> u64 { self.unix_seconds }
    pub fn tai_j2000_micros(&self) -> u64 { self.tai_j2000_micros }

    fn from_snapshot(snapshot: crate::willow::ClockSnapshot) -> Self {
        Self {
            unix_seconds: snapshot.unix_seconds,
            tai_j2000_micros: snapshot.tai_j2000_micros,
        }
    }

    #[cfg(feature = "conformance")]
    pub fn from_unix_seconds(unix_seconds: i64) -> Result<Self, crate::willow::WillowError> {
        let snapshot = crate::willow::snapshot_from_unix_seconds(unix_seconds, 0)?;
        Ok(Self::from_snapshot(snapshot))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewswireProjection {
    pub open_wire: Vec<ProjectedPost>,
    pub front_page: Vec<ProjectedPost>,
    pub earlier: Vec<ProjectedPost>,
    pub future_quarantine: Vec<EntryId>,
    pub editorial_history: Vec<ProjectedEditorialAction>,
}

pub enum PostTreatment {
    Ordinary,
    Hidden { actions: Vec<EntryId> },
    Tombstoned { actions: Vec<EntryId> },
}
```

`ProjectedPost` may expose body, location, sources, and correction text only
for `Ordinary`/explicit hidden-detail views. `Tombstoned` retains complete post
ID, signer, entry time, action signer, reason, and history IDs only.
The clock fields remain private: production obtains both values from one
`system_snapshot`; conformance obtains both from one pinned hifitime conversion.
There is no constructor accepting independent Unix and TAI values.

- [ ] **Step 3: Implement the reducer exactly once**

The reducer takes the verified descriptor, records, and clock. It first rejects
an inconsistent descriptor namespace/founder, deduplicates exact `EntryId`
inputs while requiring duplicate values to be structurally identical, and
rejects more than 1,024 distinct records with `PROJECTION_LIMIT_EXCEEDED`. It
filters records to the descriptor namespace and pinned descriptor ID; derives
action actor from the verified entry signer; applies the fixed unique roster;
uses checked TAI cutoff addition; orders eligible records by `(tai, EntryId)`;
marks a non-retraction action inactive when any later valid retraction targets
it; forbids retraction targets that are retractions; then applies tombstone,
hide, feature, verify, and correction rules from the approved design. No map or
filesystem iteration order enters output.

For each non-retraction action, resolve `target_entry_id` to an eligible
`NewsPost` under the same pinned descriptor before it can become active. An
absent target, an editorial-action target, a descriptor target, or a post from
another descriptor has no effect and remains visible only in history. Resolve
retraction targets separately under the earlier/non-retraction rule.

Expose the store composition without duplicating reducer logic:

```rust
pub fn project_space(
    store: &EvidenceStore,
    descriptor_id: EntryId,
    clock: ProjectionClockV1,
) -> Result<NewswireProjection, NewswireStoreError> {
    let descriptor = load_space_descriptor(store, descriptor_id)?;
    let records = load_space_records(store, descriptor_id)?;
    project(&descriptor, &records, clock).map_err(Into::into)
}
```

- [ ] **Step 4: Run and commit Task 4**

```bash
cargo test -p riot-core --features conformance --test newswire_projection
cargo test -p riot-core --all-features
git add crates/riot-core/src/newswire crates/riot-core/tests/newswire_projection.rs \
  crates/riot-core/Cargo.toml
git diff --cached --check
git commit -m "feat(newswire): derive deterministic collective views"
```

## Task 5: Commit cross-runtime golden vectors and the core vertical proof

**Files:**
- Create: `crates/riot-core/examples/pack_newswire_vectors.rs`
- Create: `fixtures/newswire/manifest.json`
- Create: `fixtures/newswire/space-v1.cbor`
- Create: `fixtures/newswire/post-v1.cbor`
- Create: `fixtures/newswire/editorial-action-v1.cbor`
- Create: `fixtures/newswire/space-v1.riot-evidence`
- Create: `fixtures/newswire/post-v1.riot-evidence`
- Create: `fixtures/newswire/editorial-action-v1.riot-evidence`
- Create: `scripts/newswire/repack-vectors.sh`
- Create: `crates/riot-core/tests/newswire_end_to_end.rs`
- Modify: `crates/riot-core/Cargo.toml`

- [ ] **Step 1: Write the failing drift and rebuild test**

The test reads the three payload fixtures and three complete signed
`.riot-evidence` fixtures. It strictly decodes/re-encodes every payload,
bundle, canonical entry, capability, and signature; verifies capability and
signature; and compares payload SHA-256, Riot EntryId, and evidence digest to
lowercase full IDs in `fixtures/newswire/manifest.json`. It also asserts the
manifest's fixed `unix_seconds` and `tai_j2000_micros` equal
`ProjectionClockV1::from_unix_seconds(unix_seconds)`, pinning the paired
hifitime/Willow conversion. It then imports the committed signed bytes into
store A, imports the same records in reverse order into fresh store B, and
asserts:

```rust
assert_eq!(
    project_space(&store_a, descriptor_id, clock).unwrap(),
    project_space(&store_b, descriptor_id, clock).unwrap(),
);
```

The scenario includes freeform, alert-profile, and request-profile posts; two
publishers; one recognized editor; one unknown editor; feature, verification,
correction, hide plus retraction; tombstone on a second post; an expired post;
and a future-quarantined post. Assert no ID is truncated in the manifest or
debug projection.

Run:

```bash
cargo test -p riot-core --features conformance --test newswire_end_to_end
```

Expected: FAIL because vectors and packer do not exist.

Register `newswire_end_to_end` only when its file exists:

```toml
[[test]]
name = "newswire_end_to_end"
required-features = ["conformance"]
```

- [ ] **Step 2: Add deterministic vector generation**

The example uses fixed, clearly named conformance-only fixture secrets to
construct a founding organizer (`namespace_id == subspace_id`), an ordinary
publisher, and a roster editor. It calls the production-equivalent canonical
encoders, signed factories, and evidence-bundle encoder; writes the three
payload files, the three complete single-record `.riot-evidence` files, and a
stable pretty JSON manifest with one trailing newline. The manifest's only
top-level keys are `clock` and `records`. `clock` contains the fixed integer
`unix_seconds = 1783000000` and the exact integer `tai_j2000_micros` returned by
the pinned conversion. `records` contains exactly three rows in space, post,
editorial-action dependency order. Each row contains `name`, `payload_file`,
`bundle_file`, and the computed lowercase 64-hex `payload_sha256`, `entry_id`,
and `evidence_digest`; generation fails unless every hex string decodes to 32
bytes and every filename is unique. The script is the only regeneration entry
point:

Register the packer beside the existing conformance-only example:

```toml
[[example]]
name = "pack_newswire_vectors"
required-features = ["conformance"]
```

```bash
#!/usr/bin/env bash
set -euo pipefail
cargo run -p riot-core --features conformance --example pack_newswire_vectors
cargo test -p riot-core --features conformance --test newswire_end_to_end
```

Do not place signing secrets in the fixture manifest or generated artifacts.
The conformance-only packer source may contain deterministic fixture seeds;
`cargo xtask validate-contracts` must continue proving those constructors and
seeds are absent from the `riot-ffi` release graph.

- [ ] **Step 3: Regenerate, verify, and commit Task 5**

```bash
sh scripts/newswire/repack-vectors.sh
cargo test -p riot-core --features conformance --test newswire_end_to_end
cargo test --workspace --all-features
git add crates/riot-core/examples/pack_newswire_vectors.rs \
  crates/riot-core/tests/newswire_end_to_end.rs crates/riot-core/Cargo.toml \
  fixtures/newswire scripts/newswire/repack-vectors.sh
git diff --cached --check
git commit -m "test(newswire): prove signed store rebuild convergence"
```

## Task 6: Final contract, coverage, and release-surface verification

**Files:**
- Modify only when tests expose a Newswire gap: files already named in Tasks 1–5
- Do not modify: `.coverage-thresholds.json`

- [ ] **Step 1: Run formatting, static checks, and the full workspace tests**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
```

Expected: all commands exit 0.

- [ ] **Step 2: Run the repository's sole coverage source of truth**

Read `.coverage-thresholds.json`, then run its exact enforcement command:

```bash
cargo tarpaulin --fail-under 100
```

Expected: exit 0 with at least 100% line coverage. If unrelated baseline debt
keeps the repository-wide command red, report the exact measured result and do
not claim task completion, lower the threshold, or broaden this feature's file
scope without Rabble's direction.

- [ ] **Step 3: Verify the release surface contains no injectable clock or editorial web key path**

```bash
cargo test -p riot-core --test release_surface
cargo xtask validate-contracts
! rg -n "EditorialAction|editorial.*key" crates/riot-ffi apps/gateway
```

Expected: release-surface and contract validation pass; the search finds no new
FFI/browser editorial-key API because this slice exports Rust core only.

- [ ] **Step 4: Inspect final scope and commit any test-only corrections**

```bash
git status --short
git diff --check
git log --oneline -6
```

Expected: only declared Newswire paths changed, all product commits are present,
and unrelated dirty worktree files remain untouched. If verification required a
correction, commit only its declared paths with:

```bash
git commit -m "test(newswire): close core contract coverage"
```

## Follow-on plans after this slice passes

1. Riot FFI plus native one-space reading, publishing, and editorial-action UI.
2. Gateway/browser publishing with a durable local outbox and no editorial keys.
3. Multi-community persistence, switching, discovery, and existing Tools integration.

None of those plans may change the approved V1 wire bytes without a new protocol
version and migration review.
