# Demo Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Riot demoable end-to-end and make it feel like what it is — real human names, a lived-in seeded space, and P2P sync you can *see*. Design: `docs/superpowers/specs/2026-07-12-demo-polish-design.md`.

**Architecture:** Three layers, each independently testable. (1) **Minimal display names** — a new `profile/` Willow path family (canonical CBOR codec + two-gate import admission + resolver + FFI), because three of four demo beats currently render `member-<hex>`. (2) **Demo mode** — a seeded space shipped as a real signed RIOTE1 bundle, loaded through the ordinary `inspect → plan_all → commit` pipeline behind a hidden toggle. (3) **Motion kit** — five new SwiftUI components in shared RiotKit, macOS-clean, wired into the five screens the demo script touches. Almost everything is a NEW file; only the final integration pass edits shell files another session claims.

**Tech Stack:** Rust (`riot-core` codec/admission/resolver, `riot-ffi` UniFFI surface), `minicbor` (manual canonical encode/decode, mirroring `apps/endorse.rs`), `sha2`, SwiftUI (shared RiotKit sources compiled by both the iOS and macOS targets), XCUITest.

---

## Before you start

1. **This is a heavily-shared checkout.** Run `git status --short` and read `COLLABORATION.md`. Several agent sessions work here concurrently. Rules that are not optional:
   - **Commit with a pathspec**: `git commit -m "msg" -- <files>`. Note `-m` BEFORE `--`. A bare `git commit` has swept other sessions' staged files into the wrong commit more than once.
   - **If a file you are about to edit has foreign uncommitted changes, STOP and report BLOCKED.** Do not reconcile someone else's work-in-progress.
   - Expect a dirty tree from other sessions (fmt noise, half-written RED tests that don't compile). Do **not** gate on `cargo test --workspace` when that's true — use targeted `-p riot-core --test <name>` commands, or check out a clean detached worktree at a known SHA.
   - Post a claim row in `COLLABORATION.md` before Task 1, listing the files below.
2. **`apps/ios/Riot/ConferenceShellView.swift` and `apps/ios/Riot/AppModel.swift` are claimed by the iOS runtime session.** Tasks 1–9 do not touch them. Task 10 is the single coordinated integration window — re-read `COLLABORATION.md` and claim it explicitly before starting Task 10.
3. Verify baseline before Task 1: `cargo test -p riot-core --lib` and `cargo test -p riot-ffi --all-features` green.

## File Structure

**Rust — display names (Tasks 2–5):**
- `crates/riot-core/src/profile/mod.rs` — module root, `ProfileError`
- `crates/riot-core/src/profile/card.rs` — `ProfileCard` canonical codec (Task 2)
- `crates/riot-core/src/profile/path.rs` — `profile/<subspace>/card` path + `classify_profile_path` (Task 3)
- `crates/riot-core/src/profile/resolver.rs` — `write_profile_card`, `resolve_display_names`, `render_display_name` (Task 5)
- `crates/riot-core/src/import/bundle.rs`, `crates/riot-core/src/session.rs` — two-gate admission arms (Task 4)
- `crates/riot-ffi/src/profile_ffi.rs` + `mobile_state.rs` — FFI surface (Task 6)

**Rust — demo fixture (Task 7):**
- `crates/riot-core/examples/pack_demo_space.rs` — deterministic keyless builder
- `fixtures/demo/riverside/` — committed bundle bytes + source content
- `crates/riot-core/tests/demo_fixture_drift.rs` — drift guard

**Swift — new files only (Tasks 8–9):**
- `apps/ios/Riot/Design/Motion/StampSlam.swift`, `SyncRipple.swift`, `RadarPairingView.swift`, `Haptics.swift`, `FinaleBanner.swift`
- `apps/ios/Riot/Demo/DemoMode.swift` — loader + hidden toggle view

**Swift — integration (Task 10, coordinated):**
- `apps/ios/Riot/ConferenceShellView.swift`, `apps/ios/Riot/AppModel.swift`

**Docs (Task 1):** `docs/product/demo-script.md`

---

### Task 1: The demo script

The script is the spec for everything below. Written first, on purpose: any polish not reachable from it is out of scope.

**Files:**
- Create: `docs/product/demo-script.md`

- [ ] **Step 1: Write the script**

Write `docs/product/demo-script.md` with exactly these sections, filled in with concrete on-screen copy (not placeholders):

1. **Setup** — two iPhones, both in airplane mode with Bluetooth on; demo mode loaded on phone A only; phone B is a fresh profile. State this is the *only* configuration the demo is rehearsed in.
2. **Beat 1 — Open (30s).** Phone A opens into the seeded *Riverside Tenants Union* space. Name the six alerts by headline, with the member display name shown against each.
3. **Beat 2 — Discover (45s).** App Directory tab: checklist ("Built into Riot", on) and *Shift Signup* under Available, endorsed by two named groups. Name the groups.
4. **Beat 3 — Trust (45s).** Open Shift Signup's review page → read out its plain-language permissions → "Let everyone here use this" → stamp-slam + haptic → it appears in Tools.
5. **Beat 4 — Sync finale (90s).** Phone B: Connection tab → radar sweep finds phone A → entries arrive on B's board with stamp animations → on phone A check a checklist item → it ripples into B's checklist reading "checked by Ana · a3f9". Closing line, delivered while pointing at both airplane-mode icons: *"No internet. No servers. Just these two phones."*
6. **What can go wrong** — for each beat, the failure mode and the recovery line (e.g. radar finds nothing → "it's looking; BLE takes a few seconds" and keep talking; never restart the app on stage).
7. **What this demo is NOT claiming** — one honest paragraph: this is a local-first prototype, sync is nearby-only (no internet relay), display names are self-claimed and shown with their key suffix precisely because they are not verified.

- [ ] **Step 2: Commit**

```bash
git commit -m "docs: demo script — the artifact that drives the polish work

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- docs/product/demo-script.md
```

---

### Task 2: `ProfileCard` canonical codec

**Files:**
- Create: `crates/riot-core/src/profile/mod.rs`
- Create: `crates/riot-core/src/profile/card.rs`
- Modify: `crates/riot-core/src/lib.rs` — add `pub mod profile;`
- Test: `crates/riot-core/tests/profile_card.rs`

**Pattern to mirror exactly:** `crates/riot-core/src/apps/endorse.rs`. Read it first. Its discipline — definite lengths only, ascending integer map keys, no duplicate/unknown keys, no trailing bytes, and a final **re-encode equality proof** in the decoder — is what makes byte-flip corpora safe, and this codec must have all of it.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/profile_card.rs
use riot_core::profile::card::{
    decode_profile_card, encode_profile_card, ProfileCard, MAX_DISPLAY_NAME_BYTES,
};
use riot_core::profile::ProfileError;

fn sample() -> ProfileCard {
    ProfileCard { display_name: "Ana".to_string() }
}

#[test]
fn profile_card_round_trips() {
    let bytes = encode_profile_card(&sample()).expect("encode");
    assert_eq!(decode_profile_card(&bytes).expect("decode"), sample());
}

#[test]
fn empty_display_name_is_rejected() {
    let card = ProfileCard { display_name: String::new() };
    assert_eq!(
        encode_profile_card(&card),
        Err(ProfileError::FieldInvalid)
    );
}

#[test]
fn oversized_display_name_is_rejected() {
    let card = ProfileCard {
        display_name: "x".repeat(MAX_DISPLAY_NAME_BYTES + 1),
    };
    assert_eq!(
        encode_profile_card(&card),
        Err(ProfileError::FieldInvalid)
    );
}

#[test]
fn truncated_and_trailing_bytes_are_rejected() {
    let mut bytes = encode_profile_card(&sample()).expect("encode");
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(decode_profile_card(&truncated).is_err());
    bytes.push(0x00);
    assert!(decode_profile_card(&bytes).is_err());
}

#[test]
fn invalid_utf8_display_name_is_rejected() {
    // Hand-build a canonical-looking frame whose text field holds invalid
    // UTF-8: map(1), key 0, text(2) = 0xff 0xfe.
    let bytes = vec![0xa1, 0x00, 0x62, 0xff, 0xfe];
    assert!(decode_profile_card(&bytes).is_err());
}

#[test]
fn wrong_type_for_display_name_is_rejected() {
    // map(1), key 0, bytes(3) instead of text.
    let bytes = vec![0xa1, 0x00, 0x43, 0x61, 0x62, 0x63];
    assert!(decode_profile_card(&bytes).is_err());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --test profile_card`
Expected: compile failure — `riot_core::profile` does not exist.

- [ ] **Step 3: Implement**

```rust
// crates/riot-core/src/profile/mod.rs
//! Minimal profiles: a person's self-claimed display name, stored as an
//! ordinary signed Willow entry in their own subspace. Deliberately tiny —
//! one name field, no avatars, no persona linking (see
//! `docs/research/2026-07-11-user-profiles-willow-research.md` for the
//! larger identity design this leaves alone).
//!
//! The name is SELF-CLAIMED and unverified. Rendering rule (Earthstar's,
//! adopted): never show a claimed name without its key-derived suffix —
//! `resolver::render_display_name` is the only sanctioned way to display one.

pub mod card;
pub mod path;
pub mod resolver;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    FieldInvalid,
    PathInvalid,
    Willow(crate::willow::WillowError),
    StoreRejected,
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ProfileError {}

impl From<crate::willow::WillowError> for ProfileError {
    fn from(e: crate::willow::WillowError) -> Self {
        ProfileError::Willow(e)
    }
}
```

For this task only, comment out `pub mod path;` and `pub mod resolver;` (Tasks 3 and 5 add them); uncomment each as its task lands.

```rust
// crates/riot-core/src/profile/card.rs
//! The profile payload: exactly one display-name field, canonically encoded.
//! Same rules as `apps/endorse.rs` — definite lengths, ascending integer
//! keys, no trailing bytes, and a decode-side re-encode equality proof so a
//! non-canonical encoding of the same value can never be admitted.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

use super::ProfileError;

pub const MAX_DISPLAY_NAME_BYTES: usize = 64;
pub const MAX_PROFILE_CARD_BYTES: usize = 256;

/// The number of top-level CBOR map entries a canonical card always has.
const FIELD_COUNT: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileCard {
    /// Self-claimed, unverified. Never render without the key suffix — see
    /// `resolver::render_display_name`.
    pub display_name: String,
}

fn validate(card: &ProfileCard) -> Result<(), ProfileError> {
    if card.display_name.is_empty() || card.display_name.len() > MAX_DISPLAY_NAME_BYTES {
        return Err(ProfileError::FieldInvalid);
    }
    Ok(())
}

pub fn encode_profile_card(card: &ProfileCard) -> Result<Vec<u8>, ProfileError> {
    validate(card)?;

    let mut buffer: Vec<u8> = Vec::new();
    let mut e = Encoder::new(&mut buffer);
    let r: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        e.map(FIELD_COUNT)?;
        e.u8(0)?.str(&card.display_name)?;
        Ok(())
    })();
    r.map_err(|_| ProfileError::FieldInvalid)?;

    if buffer.len() > MAX_PROFILE_CARD_BYTES {
        return Err(ProfileError::FieldInvalid);
    }
    Ok(buffer)
}

pub fn decode_profile_card(input: &[u8]) -> Result<ProfileCard, ProfileError> {
    if input.len() > MAX_PROFILE_CARD_BYTES {
        return Err(ProfileError::FieldInvalid);
    }

    let mut d = Decoder::new(input);
    let err = |_| ProfileError::FieldInvalid;

    if d.map().map_err(err)? != Some(FIELD_COUNT) {
        return Err(ProfileError::FieldInvalid);
    }
    if d.u8().map_err(err)? != 0 {
        return Err(ProfileError::FieldInvalid);
    }
    if d.datatype().map_err(err)? != Type::String {
        return Err(ProfileError::FieldInvalid);
    }
    let display_name = d.str().map_err(err)?.to_string();

    if d.position() != input.len() {
        return Err(ProfileError::FieldInvalid);
    }

    let card = ProfileCard { display_name };
    validate(&card)?;

    // Canonicality proof: only the exact encoder output is acceptable.
    if encode_profile_card(&card)? != input {
        return Err(ProfileError::FieldInvalid);
    }
    Ok(card)
}
```

Add `pub mod profile;` to `crates/riot-core/src/lib.rs`, alongside the existing `pub mod` lines.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p riot-core --test profile_card`
Expected: 6 passed.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p riot-core --lib -- -D warnings`
Expected: clean.

```bash
git commit -m "feat(profile): add canonical profile-card codec

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- crates/riot-core/src/profile/mod.rs crates/riot-core/src/profile/card.rs crates/riot-core/src/lib.rs crates/riot-core/tests/profile_card.rs
```

---

### Task 3: `profile/` path family and classifier

**Files:**
- Create: `crates/riot-core/src/profile/path.rs`
- Modify: `crates/riot-core/src/profile/mod.rs` — uncomment `pub mod path;`
- Test: `crates/riot-core/tests/profile_path.rs`

**Pattern to mirror:** `crates/riot-core/src/apps/index.rs::classify_app_index_path` (read it). A single-sourced shape classifier is what the import gates consult; local writes and remote admission must never drift apart.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/profile_path.rs
use riot_core::profile::path::{
    classify_profile_path, profile_card_path, PROFILE_COMPONENT, PROFILE_PREFIX_COMPONENT_COUNT,
};
use riot_core::willow::Path;

#[test]
fn card_path_has_expected_shape() {
    let subspace = [7u8; 32];
    let path = profile_card_path(&subspace).expect("path");
    assert_eq!(
        path,
        Path::from_slices(&[PROFILE_COMPONENT, &subspace, b"card"]).expect("path")
    );
}

#[test]
fn classifier_accepts_exactly_the_card_slot() {
    let subspace = [7u8; 32];
    let path = profile_card_path(&subspace).expect("path");
    assert_eq!(classify_profile_path(&path), Some(subspace));
}

#[test]
fn classifier_rejects_every_malformed_shape() {
    let subspace = [7u8; 32];
    let short_id = [7u8; 31];

    // Bare prefix, no subspace.
    let bare = Path::from_slices(&[PROFILE_COMPONENT]).expect("path");
    assert_eq!(classify_profile_path(&bare), None);

    // Subspace but no slot.
    let no_slot = Path::from_slices(&[PROFILE_COMPONENT, &subspace]).expect("path");
    assert_eq!(classify_profile_path(&no_slot), None);

    // Wrong-length subspace.
    let short = Path::from_slices(&[PROFILE_COMPONENT, &short_id, b"card"]).expect("path");
    assert_eq!(classify_profile_path(&short), None);

    // Unknown slot name.
    let unknown = Path::from_slices(&[PROFILE_COMPONENT, &subspace, b"avatar"]).expect("path");
    assert_eq!(classify_profile_path(&unknown), None);

    // Extra trailing component.
    let extra =
        Path::from_slices(&[PROFILE_COMPONENT, &subspace, b"card", b"extra"]).expect("path");
    assert_eq!(classify_profile_path(&extra), None);

    // Different top-level family entirely.
    let other = Path::from_slices(&[b"apps", &subspace, b"card"]).expect("path");
    assert_eq!(classify_profile_path(&other), None);
}

#[test]
fn prefix_component_count_matches_the_built_path() {
    let path = profile_card_path(&[7u8; 32]).expect("path");
    assert_eq!(path.components().count(), PROFILE_PREFIX_COMPONENT_COUNT + 1);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --test profile_path`
Expected: compile failure — `riot_core::profile::path` does not exist.

- [ ] **Step 3: Implement**

```rust
// crates/riot-core/src/profile/path.rs
//! Where a profile lives: `profile/<subspace_id>/card`. One slot per person,
//! last-write-wins, in that person's OWN subspace — the entry's subspace must
//! equal the path's subspace component, which `session.rs`'s inspect gate
//! enforces so nobody can write a name into someone else's slot.
//!
//! `classify_profile_path` is the single source of truth for this shape,
//! shared by local writes and the import pipeline's two admission gates —
//! the same discipline as `apps::entry::is_app_data_path` and
//! `apps::index::classify_app_index_path`.

use crate::willow::Path;

use super::ProfileError;

pub const PROFILE_COMPONENT: &[u8] = b"profile";
pub const SUBSPACE_ID_BYTES: usize = 32;
/// `profile` + `<subspace>` — the components before the slot name.
pub const PROFILE_PREFIX_COMPONENT_COUNT: usize = 2;

pub fn profile_card_path(subspace_id: &[u8; SUBSPACE_ID_BYTES]) -> Result<Path, ProfileError> {
    Path::from_slices(&[PROFILE_COMPONENT, subspace_id, b"card"])
        .map_err(|_| ProfileError::PathInvalid)
}

/// The whole `profile/` subtree — used by the resolver's prefix scan.
pub fn profile_prefix() -> Result<Path, ProfileError> {
    Path::from_slices(&[PROFILE_COMPONENT]).map_err(|_| ProfileError::PathInvalid)
}

/// Returns the subspace that owns this profile slot, or `None` for any path
/// that is not exactly `profile/<32-byte subspace>/card`.
pub fn classify_profile_path(path: &Path) -> Option<[u8; SUBSPACE_ID_BYTES]> {
    let mut components = path.components();
    if components.next()?.as_ref() != PROFILE_COMPONENT {
        return None;
    }
    let subspace_id: [u8; SUBSPACE_ID_BYTES] = components.next()?.as_ref().try_into().ok()?;
    if components.next()?.as_ref() != b"card" {
        return None;
    }
    components.next().is_none().then_some(subspace_id)
}

/// True for any path under the reserved `profile/` prefix, well-formed or
/// not. The import gate needs this to *reserve* the prefix: a malformed
/// profile path must be rejected outright, never fall through to the alert
/// schema.
pub fn is_profile_prefixed(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|component| component.as_ref() == PROFILE_COMPONENT)
}
```

Uncomment `pub mod path;` in `crates/riot-core/src/profile/mod.rs`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p riot-core --test profile_path`
Expected: 4 passed.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p riot-core --lib -- -D warnings`
Expected: clean.

```bash
git commit -m "feat(profile): add profile path family and shape classifier

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- crates/riot-core/src/profile/path.rs crates/riot-core/src/profile/mod.rs crates/riot-core/tests/profile_path.rs
```

---

### Task 4: Admit `profile/` entries at the two import gates

**Files:**
- Modify: `crates/riot-core/src/import/bundle.rs` — `verify_frame`'s schema gate
- Modify: `crates/riot-core/src/session.rs` — `inspect`'s binding gate + payload retention
- Test: `crates/riot-core/tests/core_import_profile_entries.rs`
- Modify: `crates/riot-core/Cargo.toml` — `[[test]]` registration

**Why this task exists (do not skip it):** riot-core's import pipeline rejects **every** path family as `UnsupportedSchema` until it is explicitly admitted, at **two** independent gates. Without this task, a profile entry can never commit — not locally, not from a peer. This is the third time this codebase has needed this work (`b4abd93` for app-data, `bac5558` for app-index); read `git show bac5558` for the template.

The two gates and what each must enforce for `profile/`:
- **`verify_frame` (schema)** — the payload must decode as a canonical `ProfileCard`. Also: the `profile/` prefix is **reserved** — a malformed profile path must be rejected outright, never fall through to the alert schema check (`is_profile_prefixed`).
- **`inspect` (binding)** — the entry's subspace must equal the path's subspace component. This is what stops one person writing a display name into another person's slot.

**Payload retention:** profile payloads must be retained with the live entry (the resolver reads them back), exactly as app-data and app-index payloads are.

**Deliberately NOT a check here:** nothing about whether the name is "allowed", unique, or non-offensive. Admission gates stay policy-free; name collisions are handled at render time by the key-suffix rule.

- [ ] **Step 1: Write the failing tests**

Mirror the signed-one-entry-bundle helper in `crates/riot-core/tests/core_import_app_index_entries.rs` (read it first and reuse its helper shape verbatim — it builds a `SignedWillowEntry`, wraps it via `encode_bundle`, and runs `inspect → plan_all → commit`).

```rust
// crates/riot-core/tests/core_import_profile_entries.rs
//
// Helper shape to copy from core_import_app_index_entries.rs:
//   fn signed_at_path(author, path, payload, ts) -> Vec<u8>   // one-item RIOTE1 bundle
//   fn commit_entry(store, bundle_bytes)                      // inspect->plan_all->commit, panics on NoChanges
//   fn expect_unsupported_schema(store, bundle_bytes)         // asserts the rejection diagnostic
//
// Tests:

#[test]
fn valid_profile_card_at_own_slot_commits_and_retains_its_payload() {
    // author writes profile_card_path(author.subspace) with encode_profile_card({display_name:"Ana"})
    // -> commit succeeds; entries_with_prefix(profile_prefix()) returns it WITH payload bytes,
    //    and decode_profile_card(payload) == the card.
}

#[test]
fn garbage_payload_at_profile_slot_is_rejected_as_unsupported_schema() {
    // payload = b"not-cbor" at the author's own card path -> UnsupportedSchema.
}

#[test]
fn profile_written_into_someone_elses_slot_is_rejected_at_inspect() {
    // attacker signs a VALID card, but at profile_card_path(&victim_subspace).
    // The bundle must ENCODE fine (proving verify_frame passes it — the schema is valid),
    // and inspect must return a Preview with eligible_count == 0 and live_count unchanged.
    // Assert the same rejection surface core_import_path_binding.rs asserts.
    // ALSO: first prove the author CAN write their OWN slot, so this test cannot pass vacuously.
}

#[test]
fn malformed_profile_path_does_not_fall_through_to_alert_schema() {
    // path = profile/<32B>/avatar  (classifier -> None, but IS profile-prefixed)
    // payload = a VALID canonical alert.
    // -> must still be UnsupportedSchema. This is the strongest possible witness that
    //    the reserved prefix cannot be rescued by a valid alert payload.
}

#[test]
fn profile_path_with_extra_components_is_rejected() {
    // profile/<32B>/card/extra -> UnsupportedSchema.
}

#[test]
fn alerts_and_app_entries_are_unaffected() {
    // A normal alert and a normal app-data entry still commit exactly as before —
    // proves this change tightened nothing and loosened nothing for existing families.
}
```

Write these out fully against the real helpers before implementing. Register the test in `crates/riot-core/Cargo.toml` with `required-features = ["conformance"]`, matching its siblings (`core_import_app_index_entries`).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --features conformance --test core_import_profile_entries`
Expected: the happy-path tests FAIL with `UnsupportedSchema` (the gates don't know `profile/` yet). Adversarial tests may pass vacuously at this point — that's expected.

- [ ] **Step 3: Extend `verify_frame`'s schema gate**

In `crates/riot-core/src/import/bundle.rs`, the current gate is a `let schema_ok = if is_app_data_path(...) { true } else { match classify_app_index_path(...) { ... None => { !is_malformed_app_index_path && decode_alert(...).is_ok() } } };` (read it — around line 526). Extend the `None` arm so the profile family is handled and its prefix reserved:

```rust
            None => {
                let is_malformed_app_index_path =
                    entry.path().components().next().is_some_and(|component| {
                        component.as_ref() == crate::apps::index::APP_INDEX_COMPONENT
                    });
                if is_malformed_app_index_path {
                    false
                } else if crate::profile::path::is_profile_prefixed(entry.path()) {
                    // Reserved prefix: only the exact card slot with a canonical
                    // card payload is admissible. A malformed profile path can
                    // NEVER fall through to the alert schema below.
                    crate::profile::path::classify_profile_path(entry.path()).is_some()
                        && crate::profile::card::decode_profile_card(&frame.payload_bytes).is_ok()
                } else {
                    crate::model::decode_alert(&frame.payload_bytes).is_ok()
                }
            }
```

Adapt to the exact code shape present — the invariant is what matters: alert paths keep the alert check, app-data stays opaque, each app-index slot keeps its decoder, the profile card slot gets its decoder, both reserved prefixes reject malformed paths outright, everything else is `UnsupportedSchema`.

- [ ] **Step 4: Extend `inspect`'s binding gate and payload retention**

In `crates/riot-core/src/session.rs` (around line 623), the current chain is `let path_matches = if is_app_data { true } else if let Some(slot) = app_index_slot { ... } else { <alert binding> };`. Add the profile arm **before** the alert fallback:

```rust
                let profile_subspace = crate::profile::path::classify_profile_path(
                    willow25::groupings::Keylike::path(authorised.entry()),
                );
                let path_matches = if is_app_data {
                    true
                } else if let Some(slot) = app_index_slot {
                    // ... existing endorsement/trust/manifest/bundle arms, unchanged ...
                } else if let Some(subspace_id) = profile_subspace {
                    // A profile slot belongs to exactly the subspace named in its
                    // path: nobody writes a display name into someone else's slot.
                    *willow25::groupings::Keylike::subspace_id(authorised.entry()).as_bytes()
                        == subspace_id
                } else {
                    // ... existing alert binding, unchanged ...
                };
```

And extend retention (currently `let retain_payload = is_app_data || app_index_slot.is_some();`) so the resolver can read cards back:

```rust
                    // Profile payloads are retained too: the resolver reads
                    // display names back from live entries.
                    let retain_payload =
                        is_app_data || app_index_slot.is_some() || profile_subspace.is_some();
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p riot-core --features conformance --test core_import_profile_entries`
Expected: 6 passed.

Run: `cargo test -p riot-core --features conformance --test core_import_app_index_entries --test core_import_app_entries --test core_import_path_binding`
Expected: all green — this proves the gates did not loosen for alerts, app-data, or app-index.

- [ ] **Step 6: Clippy and commit**

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean. (If it fails only on files another session left dirty, use a clean worktree — see "Before you start".)

```bash
git commit -m "feat(core): admit profile entries at the import boundary

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- crates/riot-core/src/import/bundle.rs crates/riot-core/src/session.rs crates/riot-core/tests/core_import_profile_entries.rs crates/riot-core/Cargo.toml
```

---

### Task 5: Write, resolve, and render display names

**Files:**
- Create: `crates/riot-core/src/profile/resolver.rs`
- Modify: `crates/riot-core/src/profile/mod.rs` — uncomment `pub mod resolver;`
- Test: `crates/riot-core/tests/profile_resolver.rs` (+ `[[test]]` registration, `required-features = ["conformance"]`)

**`render_display_name` is the heart of this task.** The Earthstar rule, adopted deliberately (see the research doc): a self-claimed name is **never** rendered bare. Every surface shows `Ana · a3f9`. Two people can both claim "Ana"; their suffixes differ, and nothing merges them. A person with no profile entry renders as `member · a3f9` — the same shape, so the UI never has two layouts.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/riot-core/tests/profile_resolver.rs
use riot_core::profile::card::ProfileCard;
use riot_core::profile::resolver::{render_display_name, resolve_display_names, write_profile_card};
use riot_core::session::RiotSession;
use riot_core::willow::generate_communal_author;

#[test]
fn render_always_appends_the_key_suffix() {
    let subspace = [0xa3, 0xf9, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(render_display_name(Some("Ana"), &subspace), "Ana · a3f91122");
}

#[test]
fn render_falls_back_to_member_for_an_unknown_subspace() {
    let subspace = [0xa3, 0xf9, 0x11, 0x22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(render_display_name(None, &subspace), "member · a3f91122");
}

#[test]
fn same_name_different_key_never_collides_in_rendering() {
    let a = [0xaa; 32];
    let b = [0xbb; 32];
    assert_ne!(render_display_name(Some("Ana"), &a), render_display_name(Some("Ana"), &b));
}

#[test]
fn write_then_resolve_round_trips() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    write_profile_card(&store, &author, &ProfileCard { display_name: "Ana".into() }, 1)
        .expect("write");

    let names = resolve_display_names(&store).expect("resolve");
    let subspace = *author.subspace_id().as_bytes();
    assert_eq!(names.get(&subspace).map(String::as_str), Some("Ana"));
}

#[test]
fn a_later_write_replaces_the_earlier_name_in_the_same_slot() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");

    write_profile_card(&store, &author, &ProfileCard { display_name: "Ana".into() }, 1)
        .expect("write");
    write_profile_card(&store, &author, &ProfileCard { display_name: "Ana R.".into() }, 2)
        .expect("rewrite");

    let names = resolve_display_names(&store).expect("resolve");
    let subspace = *author.subspace_id().as_bytes();
    assert_eq!(names.len(), 1, "one slot per person, last write wins");
    assert_eq!(names.get(&subspace).map(String::as_str), Some("Ana R."));
}

#[test]
fn resolve_is_empty_when_nobody_has_a_profile() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    assert!(resolve_display_names(&store).expect("resolve").is_empty());
}
```

Note on timestamps: writes to the same path must use **strictly increasing** timestamps — `commit_at` returns `AppsError::StaleWrite` for an equal-or-older write to an existing slot. Check the landed `commit_at` behavior before writing the test and match it.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p riot-core --features conformance --test profile_resolver`
Expected: compile failure — `riot_core::profile::resolver` does not exist.

- [ ] **Step 3: Implement**

```rust
// crates/riot-core/src/profile/resolver.rs
//! Writing a profile, reading everyone's back, and the ONE sanctioned way to
//! render a name.

use std::collections::BTreeMap;

use crate::session::{commit_at, EvidenceStore};
use crate::willow::identity::EvidenceAuthor;

use super::card::{decode_profile_card, encode_profile_card, ProfileCard};
use super::path::{classify_profile_path, profile_card_path, profile_prefix, SUBSPACE_ID_BYTES};
use super::ProfileError;

/// Writes the person's own card into their own slot. Signed and committed
/// through the same `inspect → plan_all → commit` pipeline as every other
/// entry — no privileged write path.
pub fn write_profile_card(
    store: &EvidenceStore,
    author: &EvidenceAuthor,
    card: &ProfileCard,
    willow_timestamp_micros: u64,
) -> Result<(), ProfileError> {
    let payload = encode_profile_card(card)?;
    let path = profile_card_path(author.subspace_id().as_bytes())?;
    commit_at(store, author, &path, &payload, willow_timestamp_micros)
        .map_err(|_| ProfileError::StoreRejected)
}

/// Every display name this device knows: `subspace_id → display_name`.
///
/// Defense in depth: an entry whose payload fails to decode, or whose author
/// subspace does not match its path slot, is SKIPPED rather than erroring —
/// the import gates already reject both, but a scan must never be the thing
/// that a malformed entry can break.
pub fn resolve_display_names(
    store: &EvidenceStore,
) -> Result<BTreeMap<[u8; SUBSPACE_ID_BYTES], String>, ProfileError> {
    let prefix = profile_prefix()?;
    let entries = store
        .entries_with_prefix(&prefix)
        .map_err(|_| ProfileError::StoreRejected)?;

    let mut names = BTreeMap::new();
    for (_id, entry, payload) in entries {
        let Some(slot_subspace) = classify_profile_path(willow25::groupings::Keylike::path(&entry))
        else {
            continue;
        };
        if *willow25::groupings::Keylike::subspace_id(&entry).as_bytes() != slot_subspace {
            continue;
        }
        let Some(payload) = payload else { continue };
        let Ok(card) = decode_profile_card(&payload) else {
            continue;
        };
        names.insert(slot_subspace, card.display_name);
    }
    Ok(names)
}

/// The ONE sanctioned way to display a person. A self-claimed name is never
/// shown bare: it always carries the first 4 bytes of its subspace as a
/// hex suffix, so two people claiming "Ana" are always distinguishable and
/// impersonation is visible rather than silent. Someone with no profile
/// renders in the same shape, as `member · <suffix>`.
pub fn render_display_name(name: Option<&str>, subspace_id: &[u8; SUBSPACE_ID_BYTES]) -> String {
    let suffix: String = subspace_id[..4].iter().map(|b| format!("{b:02x}")).collect();
    match name {
        Some(name) => format!("{name} · {suffix}"),
        None => format!("member · {suffix}"),
    }
}
```

Note: `entries_with_prefix` returns 3-tuples `(EntryId, Entry, Option<Arc<[u8]>>)` — the payload is retained for profile entries by Task 4. Confirm the exact tuple shape in `session.rs` before writing this and adapt.

Uncomment `pub mod resolver;` in `crates/riot-core/src/profile/mod.rs`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p riot-core --features conformance --test profile_resolver`
Expected: 6 passed.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p riot-core --all-features --all-targets -- -D warnings`
Expected: clean.

```bash
git commit -m "feat(profile): write, resolve, and render display names with key suffixes

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- crates/riot-core/src/profile/resolver.rs crates/riot-core/src/profile/mod.rs crates/riot-core/tests/profile_resolver.rs crates/riot-core/Cargo.toml
```

---

### Task 6: Display-name FFI

**Files:**
- Create: `crates/riot-ffi/src/profile_ffi.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs` — delegators
- Modify: `crates/riot-ffi/src/lib.rs` — register the module
- Test: `crates/riot-ffi/tests/profile_contract.rs`

- [ ] **Step 1: Re-confirm the landed FFI patterns**

Read `crates/riot-ffi/src/apps_ffi.rs` and the app methods in `mobile_state.rs` **in full** before writing anything. Confirm: the `#[uniffi::export]` object shape, the `with_active` delegation pattern, `MobileError`'s variants, how a write path obtains a Willow timestamp (`next_app_write_timestamp` — reuse it, do not invent a clock), and whether writes need the `sync_session_is_active` guard (`app_data_put` has one because `store.inspect` clobbers the shared preview slot — **a profile write commits through `inspect` too, so it needs the same guard**; mirror it and test it).

- [ ] **Step 2: Write the failing contract tests**

In `crates/riot-ffi/tests/profile_contract.rs`, mirroring the harness in `apps_contract.rs`:

1. `set_display_name("Ana")` then `my_display_name()` returns `"Ana · <suffix>"` (rendered, never bare).
2. Before any `set_display_name`, `my_display_name()` returns `"member · <suffix>"` — the fallback shape.
3. `set_display_name("")` and a 65-byte name both fail with the mapped error variant (the core codec is the single enforcement point — do not pre-validate in FFI).
4. `app_display_name()` (the `riot.whoami()` source) now returns the **rendered** name, so the checklist writes "Ana · a3f9" into `updated_by` rather than `member-a3f9c2b1`.
5. Calling `set_display_name` while a sync session is active fails with the same guard error `app_data_put` gives, and does not brick a later `open_sync_session()`.
6. A second `set_display_name` replaces the first (no duplicate slot).

- [ ] **Step 3: Implement**

`profile_ffi.rs` exposes, on the same session object pattern the apps FFI uses:
- `set_display_name(name: String) -> Result<(), MobileError>` — builds a `ProfileCard`, calls `write_profile_card` with a timestamp from the existing helper, behind the active-sync guard.
- `my_display_name() -> Result<String, MobileError>` — `resolve_display_names` + `render_display_name` for the profile's own subspace.
- `display_names() -> Result<Vec<DisplayNameRecord>, MobileError>` where `DisplayNameRecord { subspace_id: Vec<u8>, rendered: String }` — the id→name map the UI needs for board rows, endorsement lists, and checklist attribution. Follow the id convention settled in the apps FFI: **raw `Vec<u8>`**, not hex.

Change `app_display_name` (`mobile_state.rs:1374`, currently `format!("member-{}", hex(&subspace_id[..4]))`) to return `render_display_name(resolved_name_for_own_subspace, &subspace_id)`.

**Note:** this makes the *rendering* correct, but it is NOT sufficient on its own — an app that stores that string into its own data has stored a **snapshot** that a later rename can never repair. **Task 6b** is what actually fixes this, by making `whoami()` return a stable `{id, displayName, tag}` and having apps store the **id**. Do not consider "the checklist shows a real name" done at the end of this task.

- [ ] **Step 4: Gates and commit**

Run: `cargo test -p riot-ffi --all-features`
Expected: all green including the 6 new contract tests.

Run: `cargo clippy -p riot-core -p riot-ffi --all-features --all-targets -- -D warnings`
Expected: clean.

Run: `cargo xtask generate-bindings && cargo xtask validate-contracts`
Expected: PASS; grep the generated Swift and Kotlin for `setDisplayName`, `myDisplayName`, `displayNames` — all three must appear.

```bash
git commit -m "feat(ffi): expose display names; whoami returns a real rendered name

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- crates/riot-ffi/src/profile_ffi.rs crates/riot-ffi/src/mobile_state.rs crates/riot-ffi/src/lib.rs crates/riot-ffi/tests/profile_contract.rs
```

---

### Task 6b: Apps store an author **id**, not a name snapshot — and the checklist repack

**Files:**
- Modify: `fixtures/apps/checklist/app.js` — store `updated_by_id`, resolve names at render
- Modify: `apps/ios/Riot/Apps/RiotJS.swift` and `apps/android/.../apps/RiotJsShim.kt` — `whoami()` shape + new `riot.profile(id)`
- Modify: `crates/riot-ffi/src/apps_ffi.rs` / `mobile_state.rs` — back the two bridge calls
- Repack: `cargo run -p riot-core --example pack_checklist` → new `fixtures/apps/checklist.*.cbor` + the starter-catalog app_id pin
- Test: checklist bridge tests on both platforms; the starter drift guard

**⚠️ This task MUST land before Task 7,** and it is time-critical for a reason that has nothing to do with this plan's convenience. **Read this before starting:**

The checklist stores `updated_by` **into its own item value at write time** — a name *snapshot*. If display names ship that way, then the moment Ana renames herself, every item she ever checked still shows her old name **forever**, and no rename can repair them. The fix is to store an **id** and resolve the name **at render time**:

- `riot.whoami()` → `{ id, displayName, tag }` (stable id + current rendering)
- the app stores **`updated_by_id`**, never a name
- new bridge call `riot.profile(id)` → `{ displayName, tag }`, called when drawing each row

**Why the ordering is forced:** editing `app.js` changes the bundle bytes → changes the content-derived `app_id` → **every space's organizer must re-approve the checklist.** Doing this now, while the app is barely deployed, costs nothing. Doing it after the demo means a forced re-approval event in front of real users. And Task 7's demo fixture *embeds a checklist app_id* — pack it before this lands and the fixture pins a stale id.

**Coordination (do this first):** `fixtures/apps/checklist/`, `RiotJS.swift`, and `RiotJsShim.kt` belong to the iOS/Android runtime sessions. They **raised this finding themselves** and expect the change. Post a claim row in `COLLABORATION.md` naming these files and confirm no conflicting in-flight work before editing. If either runtime session has uncommitted changes to them → STOP, BLOCKED.

- [ ] **Step 1: FFI — back the two bridge calls**

`whoami` must return the id alongside the rendering. Add a UniFFI record `WhoAmI { id: Vec<u8>, display_name: String, tag: String }` (id = the raw 32-byte subspace; `tag` = the 8-hex key suffix, so JS can render `Ana · a3f9` without re-deriving it) and `profile_for(id: Vec<u8>) -> Result<WhoAmI, MobileError>` resolving through Task 5's `resolve_display_names` + `render_display_name`. An unknown id resolves to the `member · <tag>` fallback — never an error, because an app must be able to render a row authored by someone whose profile hasn't synced yet.

- [ ] **Step 2: Bridge — iOS and Android**

`RiotJS.swift`: `whoami` returns the record as `{ id, displayName, tag }` (id as a lowercase hex string across the JS boundary — JS has no byte arrays here; hex is what the rest of the bridge already uses). Add `profile: function (id) { return call("profile", { id: id }); }`. Mirror exactly in `RiotJsShim.kt` and its `@JavascriptInterface` host.

- [ ] **Step 3: The checklist app**

In `fixtures/apps/checklist/app.js`: store `updated_by_id: me.id` instead of `updated_by: me.displayName`; when rendering a row, call `riot.profile(row.value.updated_by_id)` and show `displayName · tag`. **Back-compat:** an item written by the old code has `updated_by` and no `updated_by_id` — render that stored string as-is (it is a legacy snapshot, and there is no id to resolve). Do not crash on it, do not migrate it.

- [ ] **Step 4: Repack and re-pin**

Run: `cargo run -p riot-core --example pack_checklist`
This regenerates the packed bytes AND changes the checklist's `app_id`. Update every pin of the old id (the starter-catalog test pin — grep the old id hex across the repo; the current one is `aa9633…`). The starter drift guard is what proves you repacked; it must go green.

- [ ] **Step 5: Gates and commit**

Run: `cargo test --workspace --all-features` (green), `xcodebuild test -scheme RiotKit` (green — the existing `ChecklistFlowUITests` end-to-end must still pass; it is the real proof the bridge change didn't break the app), and the Android JVM checklist tests.

```bash
git commit -m "feat(apps): store author id, not a name snapshot; resolve names at render

Renames now update all history. Changes the checklist app_id — organizers
re-approve once, deliberately taken now while the app is barely deployed.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- fixtures/apps/checklist apps/ios/Riot/Apps/RiotJS.swift apps/android/app/src/main/kotlin/org/riot/evidence/apps/RiotJsShim.kt crates/riot-ffi/src/apps_ffi.rs crates/riot-ffi/src/mobile_state.rs crates/riot-core/src/apps/starter.rs crates/riot-core/tests/apps_starter.rs
```

(Adjust the pathspec to the files you actually touched — including whatever holds the app_id pin.)

---

### Task 6c: Hostile-corpus test for the profile codec

**Files:**
- Modify: `crates/riot-core/tests/apps_codec_hostile.rs` (or create `profile_codec_hostile.rs` if that file's structure resists extension — match its existing pattern either way)

Task 2's review verified by hand that `decode_profile_card` survives truncation sweeps, byte-flip sweeps with a canonicality assertion, trailing garbage, forged CBOR headers claiming huge counts, indefinite-length maps/strings, non-canonical integer widths, and 1024 rounds of deterministic random garbage — **but nothing in the tree pins that behavior.** This codec receives attacker-controlled bytes at the Task 4 import gate; the sibling codecs facing the same threat model have exactly this suite. Add `profile_card` to it.

- [ ] **Step 1:** Read `crates/riot-core/tests/apps_codec_hostile.rs` and extend it for `ProfileCard` with the same properties it already asserts for manifest/bundle: every truncation rejected; every byte-flip either rejected or still canonical (re-encode equality); trailing garbage rejected; forged huge-count headers cause no OOM/panic; deterministic random garbage never panics.
- [ ] **Step 2:** `cargo test -p riot-core --test apps_codec_hostile` → all green (existing + new).
- [ ] **Step 3:** Commit: `test(profile): hostile corpus for the profile-card codec`.

---

### Task 7: The seeded demo space

**Files:**
- Create: `crates/riot-core/examples/pack_demo_space.rs`
- Create: `fixtures/demo/riverside/` — source content + committed bundle bytes
- Create: `crates/riot-core/tests/demo_fixture_drift.rs` (+ `[[test]]` registration)

**Read `crates/riot-core/examples/pack_checklist.rs` first — it is the exact template.** Critical, hard-won constraint recorded there: the starter fixture is packed by a **deterministic keyless riot-core example**, NOT by the `riot-app` CLI, because the CLI signs with a fresh key and its output is therefore not reproducible — which would break the drift guard. Do the same here.

- [ ] **Step 1: Write the fixture source content**

`fixtures/demo/riverside/content.json` (read by the packer, committed, human-editable):
- Space title: `Riverside Tenants Union`.
- Four demo members with display names: `Ana`, `Marcus`, `Priya`, `Dee` — each gets a **profile card entry** (this is why Tasks 2–5 come first).
- Six alerts, each authored by one of those members, with real headlines and descriptions matching the demo script (Task 1). Realistic, non-hysterical copy: a courthouse support ask, a supply drop, a know-your-rights reminder, a ride-share offer, a meeting time change, a lost-and-found.
- One app-index pair for a *Shift Signup* app (manifest + bundle), plus **two endorsement markers** from two other demo subspaces so the storefront shows "endorsed by two groups".
- A half-done checklist: three app-data entries under the checklist's app id, two unchecked, one checked by `Ana`.

Timestamps are **fixed constants in the content file**, never `now()` — the bundle must be byte-reproducible.

- [ ] **Step 2: Write the drift-guard test first (it will fail)**

```rust
// crates/riot-core/tests/demo_fixture_drift.rs
// The committed bundle bytes MUST equal a fresh deterministic rebuild from
// the committed source content. Editing content.json without repacking fails
// here — the same guard the checklist fixture has.

#[test]
fn committed_demo_bundle_equals_a_fresh_deterministic_rebuild() {
    let committed = std::fs::read("../../fixtures/demo/riverside/demo-space.riot-evidence")
        .expect("committed bundle");
    let rebuilt = riot_core::demo_fixture::build_demo_bundle_from_source()
        .expect("rebuild from committed content.json");
    assert_eq!(
        committed, rebuilt,
        "fixtures/demo/riverside is stale — re-run: cargo run -p riot-core --example pack_demo_space"
    );
}

#[test]
fn the_demo_bundle_imports_cleanly_and_yields_the_expected_shape() {
    // inspect -> plan_all -> commit the committed bytes into a fresh store, then assert:
    //  - six alerts live
    //  - four profile cards resolve to Ana/Marcus/Priya/Dee
    //  - the Shift Signup app appears in assemble_directory with exactly 2 endorsements
    //  - the checklist has 3 items, exactly 1 checked, whose updated_by renders with a key suffix
    // This is the real proof: the seed goes through the SAME pipeline as any peer's bundle.
}
```

Put the shared builder logic in a small `pub mod demo_fixture` in riot-core — create `crates/riot-core/src/demo_fixture.rs` and register it with `pub mod demo_fixture;` in `crates/riot-core/src/lib.rs` (behind `#[cfg(any(test, feature = "conformance"))]` if that matches how the checklist packer shares code — check `pack_checklist.rs` and mirror whatever it does) so the example binary and the drift test call the *same* function and cannot drift apart.

- [ ] **Step 3: Write the packer**

`crates/riot-core/examples/pack_demo_space.rs`: reads `content.json`, generates the demo authors **deterministically from fixed seeds in the content file** (so the same members produce the same subspace ids on every rebuild — the checklist packer's keyless determinism trick; read how it does it and copy), builds every entry (profile cards, alerts, app-index pair, endorsements, checklist app-data), signs them, and emits one RIOTE1 bundle to `fixtures/demo/riverside/demo-space.riot-evidence`.

- [ ] **Step 4: Pack, run tests, commit**

Run: `cargo run -p riot-core --example pack_demo_space`
Then: `cargo test -p riot-core --features conformance --test demo_fixture_drift`
Expected: 2 passed.

Run it **twice** and confirm the bundle bytes are unchanged the second time — that is the determinism proof.

```bash
git commit -m "feat(demo): seeded Riverside Tenants Union space as a real signed bundle

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- crates/riot-core/examples/pack_demo_space.rs crates/riot-core/src/demo_fixture.rs crates/riot-core/src/lib.rs fixtures/demo/riverside crates/riot-core/tests/demo_fixture_drift.rs crates/riot-core/Cargo.toml
```

---

### Task 8: Demo-mode loader and hidden toggle (FFI + Swift)

**Files:**
- Modify: `crates/riot-ffi/src/profile_ffi.rs` (or a small `demo_ffi.rs` — match how the apps FFI is organized): `load_demo_space(bytes: Vec<u8>) -> Result<(), MobileError>`
- Create: `apps/ios/Riot/Demo/DemoMode.swift`
- Test: `crates/riot-ffi/tests/demo_contract.rs`; `apps/ios/RiotTests/DemoModeTests.swift`

**On "hide", be honest (from the spec):** Willow is append-only; there is **no delete primitive and this plan does not invent one**. "Hide demo space" makes the profile stop listing the demo namespace. **Step 1 is to find out how the profile's space list is actually stored** (`create_public_space` / `join_public_space` in `mobile_state.rs`) and report what you find. If hiding turns out to need a new persisted "hidden namespaces" concept, that is a small explicit addition — call it out in your report, do not smuggle it in.

- [ ] **Step 1: Investigate and report the space-list storage** (no code)

Read `create_public_space`, `join_public_space`, and how `PublicSpace` is persisted. Write down, in the task's commit message: where the space list lives, whether a namespace can be un-listed without deleting entries, and what "hide" therefore means concretely.

- [ ] **Step 2: Failing FFI test**

`load_demo_space(committed_bundle_bytes)` → the demo space is present and listed; a pre-existing real space is **bit-for-bit unchanged** (assert its entry ids before and after); calling it twice is idempotent (no duplicate entries — the content-addressed entries dedupe through the normal join).

- [ ] **Step 3: Implement the FFI loader**

`load_demo_space` runs the bytes through the ordinary `inspect → plan_all → commit` path (the same one `inspect_bytes` uses — reuse it, do not add a privileged import path) and lists the resulting namespace as a space.

**Failure copy (from the spec's plain-language table):** if the bundle is corrupt or rejected, the person sees exactly **"Couldn't load the demo space"** — no diagnostic codes, no "bundle"/"namespace"/"signature" vocabulary. The import pipeline is transactional, so a failed load leaves the app in its previous state with nothing half-imported; add a test that a deliberately-corrupted bundle produces that error AND leaves a pre-existing real space untouched.

- [ ] **Step 4: Swift toggle**

`apps/ios/Riot/Demo/DemoMode.swift`: a `DemoModeView` with "Load demo space" / "Hide demo space", reached by a **long-press on the version string** (`.onLongPressGesture`). It reads the fixture bytes from the app bundle's Resources.

**CRITICAL, hard-won:** shipping data files must be added to the app target's **Resources build phase**, and the Riot app target's Debug config **does not define `DEBUG`** — so `#if DEBUG` fallbacks silently vanish and the file is simply missing at runtime. Add `fixtures/demo/riverside/demo-space.riot-evidence` to the Resources phase of the app target in `Riot.xcodeproj`, and write the loading code with **no `#if DEBUG` guard**.

- [ ] **Step 5: Swift test + commit**

`DemoModeTests`: the fixture resource is present in the bundle (this test is what catches the Resources-phase mistake) and loading it surfaces the seeded space in the model.

Run: `xcodebuild test -scheme RiotKit` (match the exact scheme/destination the repo's existing iOS tests use).

```bash
git commit -m "feat(demo): demo-mode loader behind a hidden toggle

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- crates/riot-ffi/src/demo_ffi.rs crates/riot-ffi/tests/demo_contract.rs apps/ios/Riot/Demo/DemoMode.swift apps/ios/RiotTests/DemoModeTests.swift apps/ios/Riot.xcodeproj/project.pbxproj
```

---

### Task 9: The motion kit (five new files, macOS-clean)

**Files:**
- Create: `apps/ios/Riot/Design/Motion/StampSlam.swift`, `SyncRipple.swift`, `RadarPairingView.swift`, `Haptics.swift`, `FinaleBanner.swift`
- Test: `apps/ios/RiotTests/MotionTests.swift`

**The two platform rules that make macOS free — non-negotiable:**
1. `Haptics` wraps `UIImpactFeedbackGenerator` in `#if os(iOS)` and compiles to a **no-op stub** on macOS. Call sites are identical on both platforms; nothing calls into UIKit on macOS.
2. **No `import UIKit` may appear in any other Motion file.** Everything else is pure SwiftUI. (Precedent for why this matters: `RiotHeader.swift` needed an `#if os(iOS)` guard around `.toolbar(.hidden, for: .navigationBar)` — an iOS-only SwiftUI API — because it was the single macOS compile blocker across all shared sources.)

Use the existing design tokens throughout: `RiotTheme.pink(for:)` for the stamp, `RiotTheme.ink/inkSoft`, `.riot(.mono/.poster)` fonts. This is a motion layer on the established identity, not a new look.

- [ ] **Step 1: Write the failing tests**

`apps/ios/RiotTests/MotionTests.swift`:
- Each of the five components renders in light and dark without crashing (host them in a `UIHostingController` / `NSHostingController` as the repo's existing view tests do — copy that pattern).
- `Haptics.trustThunk()`, `.syncComplete()`, `.arrival()` are callable and return without error (on the test platform this proves the stub compiles and is safe to call).
- `RadarPairingView` given zero peers shows the searching state; given one peer shows that peer's **rendered** display name (the `Ana · a3f9` shape from Task 5 — the radar must not print raw hex).

- [ ] **Step 2: Implement the five components**

- **`StampSlam`** — a `ViewModifier`: on appear, scale 1.35 → 0.94 → 1.0 with a small rotation (≈ -3° → 0°), `.spring(response: 0.28, dampingFraction: 0.55)`, tinted with the existing pink. Exposed as `.riotStampSlam(trigger:)` so it can fire on a value change, not just on appear. **One animation, two payoffs** — this is used for BOTH entry arrival and the trust confirmation; do not write a second stamp effect.
- **`SyncRipple`** — a ring that scales 0.6 → 1.4 while fading 0.5 → 0, over ~0.7s, drawn behind the item; plus an attribution label that fades in beneath ("checked by Ana · a3f9").
- **`RadarPairingView`** — concentric rings + a sweeping arc (a rotating gradient), discovered peers popping in as labeled dots with the stamp-slam. Zero peers → "Looking for people nearby…" (never an error).
- **`Haptics`** — an `enum` with three static funcs, `#if os(iOS)` bodies, empty on macOS.
- **`FinaleBanner`** — a dismissible bar: "No internet. No servers. Just these phones." Uses the poster font, hard border, paper2 background.

- [ ] **Step 3: Run tests, verify macOS compiles**

Run: `xcodebuild test -scheme RiotKit` (iOS) — expect all green.
Then build the **macOS** target (see `apps/macos/` and `docs/superpowers/specs/2026-07-11-riot-macos-design.md` for the scheme name) and confirm it compiles **with the Motion files included**. This is the step that catches a leaked `UIKit` import. If the macOS target does not yet compile for reasons unrelated to your files, say so explicitly in your report rather than silently skipping this check.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(ui): motion kit — stamp-slam, sync ripple, radar pairing, haptics, finale banner

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- apps/ios/Riot/Design/Motion apps/ios/RiotTests/MotionTests.swift apps/ios/Riot.xcodeproj/project.pbxproj
```

---

### Task 10: Integration pass (the one coordinated window)

**Files:**
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Test: `apps/ios/RiotUITests/DemoScriptUITests.swift`

**STOP — coordinate first.** These two files are claimed by the iOS runtime session in `COLLABORATION.md`. Before touching them: re-read that file, confirm the claim is released (or post a row claiming a short integration window and wait for acknowledgement), and run `git status --short` on both files. **If either has foreign uncommitted changes, STOP → BLOCKED.**

- [ ] **Step 1: Wire display names through the model**

`AppModel`: expose the `display_names()` map from Task 6 and use `rendered` names everywhere a person currently appears as hex — board rows, the app review sheet's author line, endorsement lists. Never render a raw subspace id or a bare name.

- [ ] **Step 2: Wire the motion kit into the five demo screens**

- **Board** (`IncidentBoardView`): new entries arrive with `.riotStampSlam(trigger:)` + `SyncRipple` + `Haptics.arrival()`.
- **App review sheet**: the "Let everyone here use this" button fires `Haptics.trustThunk()` and stamp-slams the confirmation.
- **Tools**: a newly-trusted app slams in.
- **Connection** (`ConnectionStatusView`): replace the plain pairing state with `RadarPairingView`; `Haptics.syncComplete()` when a sync round finishes.
- **Shell**: `FinaleBanner`, shown only in demo mode.

Keep every change additive. Do not restructure navigation, do not touch `RiotTabBar`, and do not modify screens outside the five in the demo script.

- [ ] **Step 3: XCUITest the whole demo script**

`apps/ios/RiotUITests/DemoScriptUITests.swift` — mirror `RiotUITests/ChecklistFlowUITests` (the existing end-to-end test; read it for the accessibility-based tap injection pattern). Walk the script: launch → load demo space via the hidden toggle → seeded board shows six alerts with human names → Directory shows Shift Signup with two endorsements → open review → trust it → it appears in Tools. The two-phone sync finale is **not** automatable here (it needs two devices and real radios) — verify that manually and say so.

- [ ] **Step 4: Full gate and commit**

Run: `xcodebuild test -scheme RiotKit` — all green, including the new UI test.
Run: `cargo test --workspace --all-features` — green.
Build the macOS target — green.

```bash
git commit -m "feat(ui): wire display names and motion into the demo screens

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>" -- apps/ios/Riot/ConferenceShellView.swift apps/ios/Riot/AppModel.swift apps/ios/RiotUITests/DemoScriptUITests.swift apps/ios/Riot.xcodeproj/project.pbxproj
```

Then release the claim in `COLLABORATION.md` with the test evidence.

---

## After this plan lands

1. Update the `COLLABORATION.md` claim row to **Done, released** with the final commit list, `cargo test --workspace --all-features` result, `xcodebuild test` result, and the macOS build result.
2. **Rehearse the demo on two real iPhones.** Nothing in this plan proves the finale — XCUITest cannot drive two devices with real radios. The plan is not "done" until someone has run the script end-to-end on hardware, in airplane mode, twice.
3. **Known dependency, stated plainly:** profile entries are a new path family, so — exactly like app entries — they will not cross the sync surface until the sync-inclusion work (app-directory **Task 5b**, owned by another session) lands. Until then, seeded profiles render correctly on each device from the local fixture, but the live "phone B learns Ana's name over the air" beat does not work. When 5b lands, **verify its participating-entry predicate and generalize it to include `profile/`** rather than adding a second parallel mechanism. The investigation memo for 5b (`b501ce4`, in the app-directory plan) explains the invariant it must maintain; the same invariant governs profile entries.
