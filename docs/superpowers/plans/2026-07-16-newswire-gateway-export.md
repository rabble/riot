# WS1 — Signed newswire gateway export (Rust/xtask) — TDD Implementation Plan

## ⚠️ COORDINATOR PREAMBLE: branch reality + a live-sibling schema fork (read before executing)

The planning agent researched the **main working checkout** (`design/composite-site-manifest`,
a live sibling session's branch), NOT `origin/main`. Both facts are true and both matter:

- **On `origin/main`** (where this plan will ultimately land): `apps/gateway/newswire.py`
  STILL has `sample_view()` at line 201; `build.py:28` renders it; there is **no**
  `fixtures/newswire/newswire-export-v1.json` and **no** generator. So on main, the newswire
  IS still demo — the original WS1 premise holds for main.
- **On `design/composite-site-manifest`** (a live sibling, not yet merged): the newswire
  already renders **real projected signed records** via `fixtures/newswire/newswire-export-v1.json`
  (schema `riot.newswire.export/1`), produced by a `#[ignore]` riot-ffi generator. `sample_view`
  is gone there.

**Therefore the real state is a fork, and WS1 must be sequenced with two coordination facts:**
1. A **live sibling** already has a working newswire→gateway render path on a different schema
   (`riot.newswire.export/1`, random keys, no proof bytes, no reverification). Do not silently
   invalidate it.
2. **Coordinator's architectural call (thesis of PR #11's program plan):** the gateway is a
   stateless renderer of ONE signed schema — `riot-public-gateway-export/2`, the schema the
   board already proves. Unifying the newswire onto it (with proof bytes + independent
   reverification) is the target. The sibling's `riot.newswire.export/1` was an interim.
   WS1 writes NEW file paths (`fixtures/newswire/{signed-space-v1.json,gateway-space/public-export-v1.json}`)
   and touches nothing the sibling owns, so it can land without conflict; retiring
   `riot.newswire.export/1` + rewiring `newswire.py` is the jointly-owned **WS1-b** follow-up,
   coordinated with the `newswire.py` owner BEFORE either lands the rewire.

**Gate before execution:** confirm schema-unification alignment with the design/composite-site
session (or the user) so WS1-b isn't contested. WS1 itself (producer + verifier + goldens) is
safe to build in parallel now. The rest of this doc is the planning agent's verified plan.

---

## The real gap WS1 closes — conference-board parity on rigor

Whichever branch you start from, the newswire lacks the board's rigor:

| Property | Conference board (has it) | Newswire (missing it) |
|---|---|---|
| Export schema | `riot-public-gateway-export/2` | `riot.newswire.export/1` |
| Produced by a committed `xtask` command | ✅ `sign-conference-fixture` | ❌ a `#[ignore]` riot-ffi test |
| Committed signed-record fixture **with proof bytes** (entry/capability/signature) | ✅ `incident-space-v1.json` | ❌ export carries only a projected `verified` bool, no proof bytes |
| Independent signature **re-verification** xtask that stamps per-entry `verification_status` | ✅ `verify-conference-export` | ❌ none |
| Reproducible, mutually-consistent golden pair (signed fixture ↔ public export) | ✅ | ❌ single non-reverifiable snapshot |

**WS1 goal (restated to match reality):** stand up the conference-board pipeline for the
newswire — two new xtask commands (`export-newswire`, `verify-newswire-export`) that produce
a committed **`riot-public-gateway-export/2`** newswire export from **real signed newswire
records**, plus a committed **signed-record golden fixture carrying the proof bytes** that
`verify-newswire-export` independently re-verifies exactly as `verify-conference-export`
validates `public-export-v1.json`.

**Out of scope for WS1 (Rust/xtask lane) — flagged as coordination seams below:** rewiring
`apps/gateway/newswire.py` to consume the new `riot-public-gateway-export/2` shape, and
retiring the `#[ignore]` riot-ffi generator. Those are downstream (Python-gateway) steps.

---

## Goal

Add two `cargo xtask` subcommands, mirroring the conference board's proven
sign→verify pipeline, against **real signed newswire Willow records** built in-process
through `riot-core`:

1. **`export-newswire`** — mints a signed newswire space (descriptor + news posts +
   editorial Feature/Verify actions), projects the collective view, and writes **two
   committed golden fixtures**:
   - `fixtures/newswire/signed-space-v1.json` — every signed record **with proof bytes**
     (`willow_entry_bytes`, `willow_capability_bytes`, `signature`, `willow_entry_id`).
   - `fixtures/newswire/gateway-space/public-export-v1.json` — the public,
     proof-free **`riot-public-gateway-export/2`** export the gateway consumes.
2. **`verify-newswire-export`** — independently re-verifies each public entry's Ed25519
   signature against the signed-record fixture's proof bytes (reusing the conference
   verifier's pure core), binds proofs to public rows by `entry_id`, and unconditionally
   stamps each entry's `verification_status` and the export `schema` — exactly as
   `verify-conference-export` does (`verify_conference_export.rs:67-155`).

## Architecture

- **Same shared-core, direct-`riot-core` approach as `sign_conference_fixture.rs`.** xtask
  already depends on `riot-core` (`crates/xtask/Cargo.toml`) and calls
  `riot_core::willow::create_signed_alert` directly. WS1 calls the newswire equivalents:
  `riot_core::newswire::{create_signed_space_descriptor, create_signed_news_post,
  create_signed_editorial_action, inspect_news_record, project, contributors}`, all `pub`
  (`crates/riot-core/src/newswire/mod.rs:56-82`).
- **Two-axis verification (the key model difference from the conference — spelled out).**
  The conference has ONE axis: does the Ed25519 signature verify (`signature_verified` /
  `signature_invalid`). The newswire has **two independent axes**, and the export must
  carry both:
  - **Signature validity** (the conference axis, *mirrored exactly*): every newswire record
    is a signed Willow entry; `verify-newswire-export` re-checks the Ed25519 signature and
    stamps `verification_status` with the **same string constants** the conference uses.
  - **Editorial verification** (newswire-only, *additive*): "signed by the collective" vs
    "unverified · read with care" is NOT about signature validity — it is whether a roster
    **editor signed a `Verify` editorial action** against the post. In the data model this
    is `ProjectedPost.verification_ids` (non-empty ⇒ verified;
    `projection.rs:102`), and "front page" is a signed `Feature` action
    (`ProjectedPost` appears in `projection.front_page`). The public export carries these as
    additive booleans `editorially_verified` and `featured`. These have **no conference
    field to mirror** (the conference export has neither), so they are new fields — the
    honest reading of "carry that distinction."
- **Moderation honoured:** only `PostTreatment::Ordinary` posts are exported;
  `Hidden`/`Tombstoned` are dropped (matches `newswire.py:_visible`, `newswire.py:31-33`,
  and the projection's redaction contract, `projection.rs:82-105`).
- **Binding by `entry_id`, not by array position.** The conference verifier pairs fixture
  and export entries positionally and *then* asserts `check_entry_identity`
  (`verify_conference_export.rs:98-101`). The newswire **projection reorders** posts
  (front page vs open wire, sorted), so positional pairing is unsafe here. WS1 binds each
  public row to its proof by looking the row's `entry_id` up in a map built from the signed
  fixture — strictly stronger than positional pairing, and it is exactly the concern
  `check_entry_identity` was added to guard. This is a deliberate, documented adaptation.

## Tech Stack

Rust 2021; `riot-core` (path dep), `willow25` (workspace pin `=0.6.0-alpha.3`),
`serde_json`, `sha2` — all already in `crates/xtask/Cargo.toml`; the existing
`crate::hex_codec` (encode/decode). No new dependencies. Tests: `cargo test -p xtask`.
Format/lint gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-features
-- -D warnings`.

---

## File Structure

**Create:**
- `crates/xtask/src/export_newswire.rs` — `run(root) -> Result<(), String>`; builds signed
  records + writes both golden fixtures. Pure helpers unit-tested; `run()` tested against a
  temp root.
- `crates/xtask/src/verify_newswire_export.rs` — `run(root) -> Result<(), String>` +
  pure `stamp`/binding helpers; re-verifies signatures and stamps `verification_status`.
  Reuses `crate::verify_conference_export::verify_signed_entry`.

**Modify:**
- `crates/xtask/src/main.rs` — add `mod export_newswire;` / `mod verify_newswire_export;`
  (after line 9); register `export-newswire` and `verify-newswire-export` dispatch arms
  (alongside `main.rs:201-214`); add both to `available_commands()` (`main.rs:228-235`);
  extend the `conference_fixture_commands_report_success_and_failure` test
  (`main.rs:938-988`) to also cover the two newswire commands.

**Committed golden fixtures (generated by running `export-newswire` once, then committed):**
- `fixtures/newswire/signed-space-v1.json` — signed records with proof bytes.
- `fixtures/newswire/gateway-space/public-export-v1.json` — the public
  `riot-public-gateway-export/2` export. (Note this deliberately reuses the same
  `gateway-space/public-export-v1.json` basename the conference uses, under the
  `fixtures/newswire/` tree — parallel structure. The gateway's `DEFAULT_EXPORT_PATH`
  points at the *conference* copy today, `riot_gateway.py:319`; pointing the newswire route
  at the newswire copy is the downstream gateway seam.)

**Untouched (explicitly):** `apps/gateway/newswire.py`,
`crates/riot-ffi/tests/generate_newswire_export.rs`, `fixtures/newswire/newswire-export-v1.json`,
`fixtures/newswire/newswire-golden-1.json` (the encoding golden — unrelated).

---

## Interface Contract (verified against the real conference fixture)

`fixtures/conference/gateway-space/public-export-v1.json` (read in full) has these EXACT
field names — top-level: `entries`, `export_revision`, `generated_at`, `namespace`,
`renderer_profile`, `schema`, `source_fixture`, `source_fixture_sha256`, `source_manifest`,
`source_manifest_sha256`, `title`, `visibility`; per-entry: `ai_assisted`, `body`,
`entry_id`, `freshness`, `kind`, `signer`, `title`, `verification_status`. The schema value
is `riot-public-gateway-export/2` and `verification_status` values are `signature_verified`
/ `signature_invalid` (`verify_conference_export.rs:20-27`).

**WS1 public export (`fixtures/newswire/gateway-space/public-export-v1.json`) — mirrors the
shared fields exactly, adds the two editorial fields, and drops the two `source_manifest*`
fields (the newswire has no package manifest):**

```json
{
  "schema": "riot-public-gateway-export/2",
  "export_revision": "newswire-gateway-export-v1",
  "generated_at": "<RFC3339 UTC, from the export clock>",
  "namespace": "<hex 32-byte communal namespace id>",
  "renderer_profile": "newswire-front/1",
  "source_fixture": "fixtures/newswire/signed-space-v1.json",
  "source_fixture_sha256": "<sha256 hex of the signed-space fixture bytes>",
  "title": "RIOT · Independent Newswire",
  "visibility": "public",
  "entries": [
    {
      "entry_id": "<hex willow entry id>",
      "signer": "<hex author subspace id>",
      "kind": "post",
      "title": "<headline>",
      "body": "<body>",
      "ai_assisted": false,
      "tai_j2000_micros": 837420622000000,
      "featured": true,
      "editorially_verified": true,
      "verification_status": "signature_verified"
    }
  ]
}
```

Notes on faithful divergences (all intentional, all documented in code comments):
- **`kind` is always `"post"`** for the newswire (the conference used `"alert"`/`"observation"`).
- **`tai_j2000_micros` (integer) replaces the conference's ISO `freshness` string.** The
  conference `freshness` was hand-authored in its source fixture; the newswire export is a
  pure function of *signed* bytes and xtask has no date-formatting dependency (no `chrono`).
  Carrying the raw signed timestamp keeps the export honest and dependency-free; the gateway
  formats it at render time. (`generated_at` at top level is set once from the export clock's
  `unix_seconds`, formatted with a tiny local RFC3339 helper — see Task 1 Step 5.)
- **`featured` / `editorially_verified`** are the additive editorial-axis fields (see
  Architecture). No conference analog exists.
- **`source_manifest`/`source_manifest_sha256` omitted** — the newswire has no
  package-manifest (the conference stamps its manifest namespace in
  `sign_conference_fixture.rs:128-136`; the newswire has none).

**WS1 signed-record fixture (`fixtures/newswire/signed-space-v1.json`) — the proof-bytes
half, analogous to `incident-space-v1.json`'s per-entry `willow_*`/`signature` fields
(`sign_conference_fixture.rs:110-117`):**

```json
{
  "schema": "riot.newswire.signed-space/1",
  "namespace": "<hex 32-byte communal namespace id>",
  "descriptor_entry_id": "<hex>",
  "records": [
    {
      "record_kind": "space_descriptor",
      "willow_entry_id": "<hex>",
      "signer": "<hex subspace id>",
      "willow_entry_bytes": "<hex>",
      "willow_capability_bytes": "<hex>",
      "signature": "<hex 64 bytes = 128 hex chars>"
    },
    { "record_kind": "news_post", "willow_entry_id": "…", "signer": "…",
      "willow_entry_bytes": "…", "willow_capability_bytes": "…", "signature": "…",
      "headline": "…", "body": "…", "ai_assisted": false, "tai_j2000_micros": 0 },
    { "record_kind": "editorial_action", "willow_entry_id": "…", "signer": "…",
      "willow_entry_bytes": "…", "willow_capability_bytes": "…", "signature": "…",
      "action_kind": "Feature", "target_entry_id": "…" }
  ]
}
```

The signed fixture carries proof bytes for **all** records (descriptor, posts, actions);
the public export lists only visible **posts**. `verify-newswire-export` re-verifies every
record's signature (integrity pass) and, per public row, binds by `entry_id` and stamps.
The public export never carries proof bytes — respecting the gateway's `_FORBIDDEN_FIELD_PARTS`
boundary (`verify_conference_export.rs:1-7`), which rejects any public field name containing
`capability`/`secret`/`receipt`/etc.

---

## Coordination Seams

- **WS4 (owned-namespace composite-site Unit 1/2) is the eventual real content source.** WS1
  builds the verifiable pipeline **now** against signed records whose *content* is generated
  inline in `export_newswire.rs` (the same six activist headlines the ffi generator uses —
  `generate_newswire_export.rs:84-97` — so the rendered page is unchanged in spirit). When
  WS4 lands a real composite-site newswire namespace, only the **content source** inside
  `export_newswire::run` swaps (read WS4's signed records instead of minting them); the
  fixture schema, `verify-newswire-export`, and the gateway contract stay put. The seam is a
  single function boundary.
- **Gateway rewire (downstream, Python lane — coordinate with the `newswire.py` owner).**
  Today `newswire.py` reads `riot.newswire.export/1` (`front_page[]`/`open_wire[]`/
  `contributors[]`). WS1 emits the conference-parity `riot-public-gateway-export/2` flat
  `entries[]` with `featured`/`editorially_verified`. Pointing the newswire route at the new
  file and adapting `render_newswire` to partition `entries[]` by `featured` is a follow-up
  (WS1-b); it is not in WS1's Rust/xtask scope. Until then both exports coexist.
- **Input source actually found in code:** real signed records are minted in-process via
  `riot_core::newswire::create_signed_*` + `project` (proven by `newswire/entry.rs` tests,
  `entry.rs:530-569`, and the ffi generator). No pre-existing on-disk newswire signed-record
  fixture exists to read — WS1 creates the first one.

---

## TDD Tasks

Effort/coverage: `.coverage-thresholds.json` is the source of truth (tarpaulin lines
~94.6%, llvm branches ~83.2%). New xtask modules must clear those floors — mirror the
conference modules' test density (pure helpers unit-tested with hand-built inputs; `run()`
tested against a temp-root copy of the committed fixtures; a missing-fixture failure arm).

---

### Task 1 — `export-newswire` mints signed records and writes both golden fixtures

**1.1 — Write the failing test for the pure export builder.**

Create `crates/xtask/src/export_newswire.rs`. Start with a test that the in-process build
produces records whose signatures all verify and a projection where the featured/verified
posts are correctly derived. Put this at the bottom of the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use riot_core::newswire::inspect_news_record;

    #[test]
    fn build_mints_signed_records_that_all_verify_and_project_editorially() {
        let built = build_signed_newswire().expect("build signed newswire");

        // Every signed record's signature verifies structurally.
        for record in &built.records {
            inspect_news_record(&record.signed)
                .expect("each minted record is a valid signed newswire entry");
        }

        // Two posts were Featured and three Verified (see build_signed_newswire).
        let featured: usize = built
            .public_entries
            .iter()
            .filter(|entry| entry.featured)
            .count();
        let verified: usize = built
            .public_entries
            .iter()
            .filter(|entry| entry.editorially_verified)
            .count();
        assert_eq!(featured, 2, "two Feature actions promote two posts");
        assert_eq!(verified, 3, "three Verify actions mark three posts");
        assert!(
            built.public_entries.len() >= 6,
            "all six Ordinary posts are exported"
        );
    }
}
```

**1.2 — Run it; watch it fail to compile (no `build_signed_newswire` yet).**

```
cargo test -p xtask --lib export_newswire
```
Expected: `error[E0425]: cannot find function 'build_signed_newswire'` (RED).

**1.3 — Implement the builder (minimal) to make 1.1 pass.**

Add above the tests in `export_newswire.rs`:

```rust
//! `export-newswire`: mints a REAL signed newswire (space descriptor + news
//! posts + editorial Feature/Verify actions) through riot-core, projects the
//! collective view, and writes two golden fixtures — the proof-bearing signed
//! record set and the proof-free `riot-public-gateway-export/2` public export
//! the web gateway consumes. This is the newswire twin of
//! `sign_conference_fixture` + the conference public export, unified into one
//! producing command. Signature RE-verification lives in
//! `verify_newswire_export`, mirroring `verify-conference-export`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use riot_core::newswire::{
    contributors, create_signed_editorial_action, create_signed_news_post,
    create_signed_space_descriptor, inspect_news_record, project, EditorialActionKind,
    EditorialActionV1, NewsPostV1, NewswirePayload, PostTreatment, ProjectionClockV1,
    SignedNewswireRecord, SpaceDescriptorV1,
};
use riot_core::willow::{generate_communal_author_for_namespace, generate_space_organizer_author};
use serde_json::{json, Value};

use crate::hex_codec;

const SPACE_NAME: &str = "RIOT · Independent Newswire";

/// One row of the public export, pre-serialization.
pub struct PublicEntry {
    pub entry_id: [u8; 32],
    pub signer: [u8; 32],
    pub headline: String,
    pub body: String,
    pub ai_assisted: bool,
    pub tai_j2000_micros: u64,
    pub featured: bool,
    pub editorially_verified: bool,
}

/// The full in-memory result of minting + projecting the newswire.
pub struct BuiltNewswire {
    pub namespace: [u8; 32],
    pub descriptor_entry_id: [u8; 32],
    pub records: Vec<SignedNewswireRecord>,
    pub public_entries: Vec<PublicEntry>,
}

/// The activist content the gateway already renders (kept identical in spirit
/// to the ffi generator so the page is unchanged). When WS4 lands a real
/// composite-site newswire namespace, only this content source swaps.
const POSTS: &[(&str, &str)] = &[
    ("Rent strike jumps three more blocks as tenants tear up eviction notices",
     "Four hundred households on Sonnenallee are now withholding rent — the largest coordinated tenant action since the 2023 deposit fight. The union answered eviction filings with a block-by-block watch."),
    ("Port workers walk out in solidarity; container terminal at a standstill",
     "The wildcat action began at the night shift. Cranes idle, 6,000 boxes stranded. Dockers hold the gate until the fired stewards are reinstated."),
    ("Leaked procurement docs show the city quietly bought facial-recognition vans",
     "Four unmarked units, invoiced under \"traffic safety.\" The contract and vendor spec sheet are published in full."),
    ("Medic station open at the old library, side entrance",
     "Volunteers are staffing a first-aid point at the west entrance. Water and shade available."),
    ("Cops massing at the north gate, roughly forty vans",
     "Eyewitness report from the strike blocks. Bring water and legal-observer numbers."),
    ("Drone overhead on Sonnenallee, circling the strike blocks",
     "Low-altitude drone seen over the rent-strike blocks for the past twenty minutes."),
];

fn news_post(descriptor_entry_id: [u8; 32], headline: &str, body: &str) -> NewsPostV1 {
    NewsPostV1 {
        space_descriptor_entry_id: descriptor_entry_id,
        headline: headline.to_string(),
        body: body.to_string(),
        language: "en".to_string(),
        event_time_unix_seconds: None,
        expires_at_unix_seconds: None,
        coarse_location: None,
        source_claims: vec![],
        operational_profile: None,
        ai_assisted: false,
    }
}

pub fn build_signed_newswire() -> Result<BuiltNewswire, String> {
    // Founder (organizer: namespace == subspace) + one roster editor.
    let founder = generate_space_organizer_author().map_err(|e| format!("founder: {e}"))?;
    let namespace = *founder.namespace_id().as_bytes();
    let editor = generate_communal_author_for_namespace(namespace)
        .map_err(|e| format!("editor: {e}"))?;
    let editor_id = *editor.subspace_id().as_bytes();

    let descriptor = SpaceDescriptorV1 {
        namespace_id: namespace,
        name: SPACE_NAME.to_string(),
        summary: "Independent community newswire.".to_string(),
        languages: vec!["en".to_string()],
        geographic_tags: vec![],
        topic_tags: vec![],
        editorial_roster: vec![editor_id],
        predecessor: None,
        successor: None,
    };
    let descriptor_record = create_signed_space_descriptor(&founder, descriptor)
        .map_err(|e| format!("sign descriptor: {e}"))?;
    let descriptor_verified = inspect_news_record(&descriptor_record.signed)
        .map_err(|e| format!("inspect descriptor: {e}"))?;
    let descriptor_entry_id = descriptor_record.entry_id;

    // Posts signed by the organizer; each inspected into a VerifiedNewswireRecord.
    let mut records = vec![descriptor_record.clone()];
    let mut post_ids = Vec::new();
    for (headline, body) in POSTS {
        let record = create_signed_news_post(
            &founder,
            &descriptor_verified,
            news_post(descriptor_entry_id, headline, body),
        )
        .map_err(|e| format!("sign post: {e}"))?;
        post_ids.push(record.entry_id);
        records.push(record);
    }

    // Editors Feature the two leads and Verify the first three, signed by the
    // roster editor (authority: signer ∈ editorial_roster).
    let action = |target: [u8; 32], kind: EditorialActionKind| EditorialActionV1 {
        space_descriptor_entry_id: descriptor_entry_id,
        target_entry_id: target,
        kind,
        reason: None,
        correction_text: None,
    };
    for target in [post_ids[0], post_ids[1]] {
        records.push(
            create_signed_editorial_action(&editor, &descriptor_verified, action(target, EditorialActionKind::Feature))
                .map_err(|e| format!("sign feature: {e}"))?,
        );
    }
    for target in [post_ids[0], post_ids[1], post_ids[2]] {
        records.push(
            create_signed_editorial_action(&editor, &descriptor_verified, action(target, EditorialActionKind::Verify))
                .map_err(|e| format!("sign verify: {e}"))?,
        );
    }

    // Project the collective view from the inspected records (descriptor passed
    // separately, exactly as store::project_space does).
    let verified_records = records
        .iter()
        .skip(1) // skip the descriptor; it is the projection anchor, passed below
        .map(|record| inspect_news_record(&record.signed))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("inspect records: {e}"))?;
    let clock = ProjectionClockV1::system().map_err(|e| format!("clock: {e}"))?;
    let projection = project(&descriptor_verified, &verified_records, clock)
        .map_err(|e| format!("project: {e}"))?;

    // Visible posts = union of front_page + open_wire, de-duped by entry_id,
    // Ordinary only (mirrors newswire.py all_posts + _visible).
    let featured_ids: BTreeSet<[u8; 32]> =
        projection.front_page.iter().map(|p| p.entry_id).collect();
    let mut seen: BTreeSet<[u8; 32]> = BTreeSet::new();
    let mut public_entries = Vec::new();
    for post in projection.front_page.iter().chain(projection.open_wire.iter()) {
        if !matches!(post.treatment, PostTreatment::Ordinary) {
            continue; // Hidden/Tombstoned vanish from the public surface.
        }
        if !seen.insert(post.entry_id) {
            continue;
        }
        public_entries.push(PublicEntry {
            entry_id: post.entry_id,
            signer: post.author_id,
            headline: post.headline.clone().unwrap_or_default(),
            body: post.body.clone().unwrap_or_default(),
            ai_assisted: post.ai_assisted,
            tai_j2000_micros: post.tai_j2000_micros,
            featured: featured_ids.contains(&post.entry_id),
            editorially_verified: !post.verification_ids.is_empty(),
        });
    }

    // Touch contributors so the derivation is exercised (parity with the ffi
    // generator; not serialized into the public export in WS1).
    let _ = contributors(&projection, namespace);

    Ok(BuiltNewswire {
        namespace,
        descriptor_entry_id,
        records,
        public_entries,
    })
}
```

**1.4 — Run 1.1; it passes (GREEN).**

```
cargo test -p xtask --lib export_newswire
```
Expected: `test export_newswire::tests::build_mints_signed_records_that_all_verify_and_project_editorially ... ok`.

**1.5 — Write the failing test for `run()` (the fixture writer).**

Append to the test module:

```rust
    #[test]
    fn run_writes_both_golden_fixtures_in_a_consistent_state() {
        let root = std::env::temp_dir().join(format!("riot-export-nw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        run(&root).expect("export succeeds into a fresh root");

        let signed: Value = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/newswire/signed-space-v1.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(signed["schema"], "riot.newswire.signed-space/1");
        let signed_records = signed["records"].as_array().unwrap();
        assert!(signed_records.len() >= 1 + 6 + 5, "descriptor + 6 posts + 5 actions");
        for record in signed_records {
            assert_eq!(record["signature"].as_str().unwrap().len(), 128);
            assert_eq!(record["willow_entry_id"].as_str().unwrap().len(), 64);
            assert!(!record["willow_entry_bytes"].as_str().unwrap().is_empty());
            assert!(!record["willow_capability_bytes"].as_str().unwrap().is_empty());
        }

        let export: Value = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/newswire/gateway-space/public-export-v1.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(export["schema"], "riot-public-gateway-export/2");
        assert_eq!(export["visibility"], "public");
        let entries = export["entries"].as_array().unwrap();
        assert!(!entries.is_empty());
        for entry in entries {
            // Proof-free public boundary: no signature/capability/entry-bytes.
            assert!(entry.get("signature").is_none());
            assert!(entry.get("willow_capability_bytes").is_none());
            assert!(entry.get("willow_entry_bytes").is_none());
            assert_eq!(entry["kind"], "post");
            assert!(entry["entry_id"].as_str().unwrap().len() == 64);
        }
        // source_fixture_sha256 matches the signed fixture bytes on disk.
        let signed_bytes =
            fs::read(root.join("fixtures/newswire/signed-space-v1.json")).unwrap();
        assert_eq!(
            export["source_fixture_sha256"].as_str().unwrap(),
            crate::sha256_hex(&signed_bytes)
        );

        let _ = fs::remove_dir_all(&root);
    }
```

**1.6 — Run it; watch it fail (no `run` yet).**

```
cargo test -p xtask --lib export_newswire::tests::run_writes_both_golden_fixtures_in_a_consistent_state
```
Expected: `error[E0425]: cannot find function 'run'` (RED).

**1.7 — Implement `run()` and the serializers.**

Add to `export_newswire.rs` (above the tests). Note `crate::sha256_hex` already exists
(`main.rs:316-321`); expose it by changing `fn sha256_hex` to `pub(crate) fn sha256_hex`
in `main.rs` as part of this step.

```rust
fn record_kind(payload: &NewswirePayload) -> &'static str {
    match payload {
        NewswirePayload::SpaceDescriptor(_) => "space_descriptor",
        NewswirePayload::NewsPost(_) => "news_post",
        NewswirePayload::EditorialAction(_) => "editorial_action",
    }
}

fn signed_record_json(record: &SignedNewswireRecord) -> Value {
    let mut value = json!({
        "record_kind": record_kind(&record.payload),
        "willow_entry_id": hex_codec::encode(&record.entry_id),
        "signer": hex_codec::encode(record.signed_signer_id()),
        "willow_entry_bytes": hex_codec::encode(&record.signed.entry_bytes),
        "willow_capability_bytes": hex_codec::encode(&record.signed.capability_bytes),
        "signature": hex_codec::encode(&record.signed.signature),
    });
    if let NewswirePayload::NewsPost(post) = &record.payload {
        value["headline"] = json!(post.headline);
        value["body"] = json!(post.body);
        value["ai_assisted"] = json!(post.ai_assisted);
        value["tai_j2000_micros"] = json!(record.snapshot.tai_j2000_micros);
    }
    if let NewswirePayload::EditorialAction(action) = &record.payload {
        value["action_kind"] = json!(format!("{:?}", action.kind));
        value["target_entry_id"] = json!(hex_codec::encode(&action.target_entry_id));
    }
    value
}

fn rfc3339_utc(unix_seconds: u64) -> String {
    // Minimal, dependency-free UTC formatter (proleptic Gregorian). xtask has
    // no chrono; this only stamps the informational `generated_at`.
    let days = (unix_seconds / 86_400) as i64;
    let secs_of_day = unix_seconds % 86_400;
    let (h, m, s) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    let (mut y, mut d) = (1970i64, days);
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let yd = if leap { 366 } else { 365 };
        if d < yd { break; }
        d -= yd;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 0usize;
    while d >= months[mo] as i64 {
        d -= months[mo] as i64;
        mo += 1;
    }
    format!(
        "{y:04}-{:02}-{:02}T{h:02}:{m:02}:{s:02}Z",
        mo + 1,
        d + 1
    )
}

pub fn run(root: &Path) -> Result<(), String> {
    let built = build_signed_newswire()?;

    let signed_doc = json!({
        "schema": "riot.newswire.signed-space/1",
        "namespace": hex_codec::encode(&built.namespace),
        "descriptor_entry_id": hex_codec::encode(&built.descriptor_entry_id),
        "records": built.records.iter().map(signed_record_json).collect::<Vec<_>>(),
    });
    let signed_dir = root.join("fixtures/newswire");
    fs::create_dir_all(&signed_dir).map_err(|e| format!("mkdir {}: {e}", signed_dir.display()))?;
    let signed_path = signed_dir.join("signed-space-v1.json");
    let signed_bytes = serde_json::to_string_pretty(&signed_doc)
        .map_err(|e| format!("serialize signed fixture: {e}"))?
        + "\n";
    fs::write(&signed_path, &signed_bytes)
        .map_err(|e| format!("write {}: {e}", signed_path.display()))?;

    let clock = ProjectionClockV1::system().map_err(|e| format!("clock: {e}"))?;
    let export = json!({
        "schema": "riot-public-gateway-export/2",
        "export_revision": "newswire-gateway-export-v1",
        "generated_at": rfc3339_utc(clock.unix_seconds()),
        "namespace": hex_codec::encode(&built.namespace),
        "renderer_profile": "newswire-front/1",
        "source_fixture": "fixtures/newswire/signed-space-v1.json",
        "source_fixture_sha256": crate::sha256_hex(signed_bytes.as_bytes()),
        "title": SPACE_NAME,
        "visibility": "public",
        "entries": built.public_entries.iter().map(|entry| json!({
            "entry_id": hex_codec::encode(&entry.entry_id),
            "signer": hex_codec::encode(&entry.signer),
            "kind": "post",
            "title": entry.headline,
            "body": entry.body,
            "ai_assisted": entry.ai_assisted,
            "tai_j2000_micros": entry.tai_j2000_micros,
            "featured": entry.featured,
            "editorially_verified": entry.editorially_verified,
        })).collect::<Vec<_>>(),
    });
    let export_dir = signed_dir.join("gateway-space");
    fs::create_dir_all(&export_dir).map_err(|e| format!("mkdir {}: {e}", export_dir.display()))?;
    let export_path = export_dir.join("public-export-v1.json");
    fs::write(
        &export_path,
        serde_json::to_string_pretty(&export).map_err(|e| format!("serialize export: {e}"))? + "\n",
    )
    .map_err(|e| format!("write {}: {e}", export_path.display()))?;

    println!(
        "export-newswire: PASS (namespace={}, {} public entries)",
        hex_codec::encode(&built.namespace),
        built.public_entries.len()
    );
    Ok(())
}
```

> **Helper needed on `SignedNewswireRecord`:** the code above calls
> `record.signed_signer_id()`. `SignedNewswireRecord` (`newswire/entry.rs:26-32`) exposes
> `signed`, `entry_id`, `snapshot`, `payload` but **not** the signer id directly. Rather than
> widen riot-core's public API, derive the signer inside xtask by decoding the entry:
> replace `hex_codec::encode(record.signed_signer_id())` with
> `hex_codec::encode(&inspect_news_record(&record.signed).map_err(|e| format!("signer: {e}"))?.signer_id())`
> — `VerifiedNewswireRecord::signer_id()` is public (`entry.rs:80-82`). Adjust
> `signed_record_json` to return `Result<Value, String>` and `?`-propagate. (Chosen because
> it stays within the existing public API; the plan's Self-Review notes this.)

**1.8 — Run 1.5; it passes (GREEN).**

```
cargo test -p xtask --lib export_newswire
```
Expected: both `export_newswire::tests::*` pass.

**1.9 — Commit.**

```
cargo fmt --all && cargo clippy -p xtask --all-features -- -D warnings
git add crates/xtask/src/export_newswire.rs crates/xtask/src/main.rs
git commit -m "feat(xtask): export-newswire mints signed newswire + public export

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
(`main.rs` is in this commit only for the `pub(crate) fn sha256_hex` visibility change +
`mod export_newswire;`. Subcommand registration lands in Task 3.)

---

### Task 2 — `verify-newswire-export` re-verifies signatures and stamps `verification_status`

**2.1 — Write the failing test for the pure binding helper.**

Create `crates/xtask/src/verify_newswire_export.rs`. First test the pure lookup/binding:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn binding_requires_a_matching_signed_record_for_every_public_entry() {
        let signed = json!({ "records": [
            { "willow_entry_id": "aa", "willow_entry_bytes": "00",
              "willow_capability_bytes": "00", "signature": "00" }
        ]});
        let index = index_signed_records(&signed).unwrap();
        assert!(index.contains_key("aa"));

        let present = json!({ "entry_id": "aa" });
        assert!(proof_for(&index, &present, 0).is_ok());

        let missing = json!({ "entry_id": "bb" });
        let error = proof_for(&index, &missing, 2).expect_err("unbound entry rejected");
        assert!(error.contains("index 2"));
        assert!(error.contains("bb"));
    }
}
```

**2.2 — Run it; fails to compile (RED).**

```
cargo test -p xtask --lib verify_newswire_export
```
Expected: `cannot find function 'index_signed_records'` / `'proof_for'`.

**2.3 — Implement the module (reusing the conference verifier core).**

```rust
//! Verifies each newswire public-export entry's real Ed25519 signature against
//! the proof bytes in `signed-space-v1.json`, then writes the public,
//! proof-free per-entry `verification_status` — the newswire twin of
//! `verify-conference-export`. Because the projection reorders posts, entries
//! bind to their proofs by `entry_id` (a map), which is strictly stronger than
//! the conference's positional pairing. Signature checking reuses the
//! conference verifier's pure core, so both surfaces share one crypto path.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::hex_codec;
use crate::verify_conference_export::{
    verify_signed_entry, EXPORT_SCHEMA, VERIFICATION_STATUS_INVALID, VERIFICATION_STATUS_VALID,
};

struct Proof {
    entry_bytes: Vec<u8>,
    capability_bytes: Vec<u8>,
    signature: [u8; 64],
}

fn index_signed_records(signed: &Value) -> Result<BTreeMap<String, Proof>, String> {
    let records = signed["records"]
        .as_array()
        .ok_or("signed fixture: records must be an array")?;
    let mut index = BTreeMap::new();
    for record in records {
        let id = record["willow_entry_id"]
            .as_str()
            .ok_or("signed record: willow_entry_id must be a string")?
            .to_string();
        let entry_bytes = hex_codec::decode(
            record["willow_entry_bytes"].as_str().ok_or("willow_entry_bytes")?,
            "willow_entry_bytes",
        )?;
        let capability_bytes = hex_codec::decode(
            record["willow_capability_bytes"].as_str().ok_or("willow_capability_bytes")?,
            "willow_capability_bytes",
        )?;
        let signature: [u8; 64] = hex_codec::decode(
            record["signature"].as_str().ok_or("signature")?,
            "signature",
        )?
        .try_into()
        .map_err(|_| "signature must be exactly 64 bytes".to_string())?;
        index.insert(id, Proof { entry_bytes, capability_bytes, signature });
    }
    Ok(index)
}

fn proof_for<'a>(
    index: &'a BTreeMap<String, Proof>,
    export_entry: &Value,
    position: usize,
) -> Result<&'a Proof, String> {
    let id = export_entry["entry_id"]
        .as_str()
        .ok_or("public export entry: entry_id must be a string")?;
    index.get(id).ok_or_else(|| {
        format!("public entry at index {position} (entry_id {id}) has no signed record to bind to")
    })
}

pub fn run(root: &Path) -> Result<(), String> {
    let signed_path = root.join("fixtures/newswire/signed-space-v1.json");
    let export_path = root.join("fixtures/newswire/gateway-space/public-export-v1.json");

    let signed: Value = serde_json::from_str(
        &fs::read_to_string(&signed_path).map_err(|e| format!("read {}: {e}", signed_path.display()))?,
    )
    .map_err(|e| format!("parse {}: {e}", signed_path.display()))?;
    let mut export: Value = serde_json::from_str(
        &fs::read_to_string(&export_path).map_err(|e| format!("read {}: {e}", export_path.display()))?,
    )
    .map_err(|e| format!("parse {}: {e}", export_path.display()))?;

    let index = index_signed_records(&signed)?;

    // Integrity pass: every signed record's signature must verify.
    for (id, proof) in &index {
        if !verify_signed_entry(&proof.entry_bytes, &proof.capability_bytes, &proof.signature)? {
            return Err(format!("signed record {id} failed signature verification"));
        }
    }

    let export_entries = export["entries"]
        .as_array()
        .cloned()
        .ok_or("public export: entries must be an array")?;
    let mut verified_count = 0usize;
    for (position, entry) in export_entries.iter().enumerate() {
        let proof = proof_for(&index, entry, position)?;
        let valid =
            verify_signed_entry(&proof.entry_bytes, &proof.capability_bytes, &proof.signature)?;
        if valid {
            verified_count += 1;
        }
        let status = if valid { VERIFICATION_STATUS_VALID } else { VERIFICATION_STATUS_INVALID };
        export["entries"][position]["verification_status"] = json!(status);
    }
    if let Some(map) = export.as_object_mut() {
        map.remove("verification_status");
    }
    export["schema"] = json!(EXPORT_SCHEMA);

    fs::write(
        &export_path,
        serde_json::to_string_pretty(&export).map_err(|e| format!("serialize export: {e}"))? + "\n",
    )
    .map_err(|e| format!("write {}: {e}", export_path.display()))?;

    println!(
        "verify-newswire-export: PASS ({verified_count}/{} entries signature-verified)",
        export_entries.len()
    );
    Ok(())
}
```

> `EXPORT_SCHEMA`, `VERIFICATION_STATUS_VALID`, `VERIFICATION_STATUS_INVALID`, and
> `verify_signed_entry` are already `pub` in `verify_conference_export.rs:20-65` — no changes
> to that module.

**2.4 — Run 2.1; it passes (GREEN).**

```
cargo test -p xtask --lib verify_newswire_export
```

**2.5 — Add the `run()` success test against the committed goldens.**

This test needs the committed golden fixtures. Generate them now (Task 4 depends on this
too, but we produce them here to make the test real), then write the test:

```
cargo run -p xtask -- export-newswire   # writes both goldens into the real tree
```
(After Task 3 registers the subcommand. If running Task 2 before Task 3, temporarily call
`export_newswire::run(&workspace_root)` from a scratch `#[test]`, or reorder so Task 3
precedes this step. The plan orders Task 3 before this test in execution — see note.)

Append to the `verify_newswire_export` tests:

```rust
    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn copy_dir_recursive(source: &std::path::Path, dest: &std::path::Path) {
        std::fs::create_dir_all(dest).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = dest.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir_recursive(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    #[test]
    fn run_stamps_signature_verified_for_the_consistent_committed_goldens() {
        let root = std::env::temp_dir().join(format!("riot-verify-nw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        copy_dir_recursive(
            &repo_root().join("fixtures/newswire"),
            &root.join("fixtures/newswire"),
        );

        run(&root).expect("verify succeeds against the consistent committed goldens");

        let export: Value = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/newswire/gateway-space/public-export-v1.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(export["schema"], EXPORT_SCHEMA);
        assert!(export.get("verification_status").is_none());
        for entry in export["entries"].as_array().unwrap() {
            assert_eq!(entry["verification_status"], VERIFICATION_STATUS_VALID);
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn run_reports_a_missing_fixture() {
        let root = std::env::temp_dir().join(format!("riot-verify-nw-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        assert!(run(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }
```

**2.6 — Run; passes (GREEN).**

```
cargo test -p xtask --lib verify_newswire_export
```
Expected all `verify_newswire_export::tests::*` pass, including
`... run_stamps_signature_verified_for_the_consistent_committed_goldens ... ok`.

**2.7 — Commit.**

```
cargo fmt --all && cargo clippy -p xtask --all-features -- -D warnings
git add crates/xtask/src/verify_newswire_export.rs
git commit -m "feat(xtask): verify-newswire-export re-verifies signatures + stamps status

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3 — Register both subcommands in `main.rs` (do BEFORE Task 2.5's `cargo run`)

**3.1 — Write the failing dispatch test.** Extend
`conference_fixture_commands_report_success_and_failure` (`main.rs:938-988`) — or add a
sibling test — to include the two newswire commands. The existing helper
`copy_conference_fixtures` copies `fixtures/conference`; add a `copy_newswire_fixtures`
that copies `fixtures/newswire`. Then:

```rust
    #[test]
    fn newswire_fixture_commands_report_success_and_failure() {
        // export-newswire needs no on-disk input (it mints records), so a fresh
        // root suffices for its success arm; verify needs the committed goldens.
        let export_root = temp_dir("newswire-export-ok");
        let mut runner = ScriptedRunner { output: None, status: None };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            run(&export_root, &["export-newswire".into()], &mut runner, &mut out, &mut err),
            ExitCode::SUCCESS
        );
        assert!(err.is_empty());

        let verify_root = temp_dir("newswire-verify-ok");
        copy_newswire_fixtures(&verify_root);
        let mut runner = ScriptedRunner { output: None, status: None };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            run(&verify_root, &["verify-newswire-export".into()], &mut runner, &mut out, &mut err),
            ExitCode::SUCCESS
        );
        assert!(err.is_empty());

        // Failure arm: verify against an empty root has no fixtures.
        let missing = temp_dir("newswire-verify-missing");
        let mut runner = ScriptedRunner { output: None, status: None };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            run(&missing, &["verify-newswire-export".into()], &mut runner, &mut out, &mut err),
            ExitCode::FAILURE
        );
    }
```

(Add `copy_newswire_fixtures` modeled on `copy_conference_fixtures`, `main.rs:930-936`.)

**3.2 — Run; RED** (`unknown xtask command: export-newswire`, ExitCode mismatch).

```
cargo test -p xtask --lib newswire_fixture_commands
```

**3.3 — Register the commands.** In `main.rs`:
- After line 9, add: `mod export_newswire;` and `mod verify_newswire_export;`
- In `run_with`'s match (after the `verify-conference-export` arm, `main.rs:208-214`):

```rust
        Some("export-newswire") => match export_newswire::run(root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("export-newswire: FAIL: {error}");
                ExitCode::FAILURE
            }
        },
        Some("verify-newswire-export") => match verify_newswire_export::run(root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("verify-newswire-export: FAIL: {error}");
                ExitCode::FAILURE
            }
        },
```

- Add both to `available_commands()` (`main.rs:228-235`):

```rust
    &[
        "validate-contracts",
        "generate-bindings",
        "sign-conference-fixture",
        "verify-conference-export",
        "export-newswire",
        "verify-newswire-export",
    ]
```

**3.4 — Run; GREEN.**

```
cargo test -p xtask --lib
```
Expected: all xtask tests pass, including `newswire_fixture_commands_report_success_and_failure`.

**3.5 — Commit.**

```
cargo fmt --all && cargo clippy --workspace --all-features -- -D warnings
git add crates/xtask/src/main.rs
git commit -m "feat(xtask): register export-newswire + verify-newswire-export subcommands

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4 — Generate and commit the golden fixtures

**4.1 — Produce the goldens from the real tree.**

```
cargo run -p xtask -- export-newswire
cargo run -p xtask -- verify-newswire-export
```
Expected stdout:
```
export-newswire: PASS (namespace=<64 hex>, 6 public entries)
verify-newswire-export: PASS (6/6 entries signature-verified)
```

**4.2 — Sanity-check the outputs.**

```
python3 -c "import json;d=json.load(open('fixtures/newswire/gateway-space/public-export-v1.json'));print(d['schema']); \
assert d['schema']=='riot-public-gateway-export/2'; \
assert all(e['verification_status']=='signature_verified' for e in d['entries']); \
assert sum(e['featured'] for e in d['entries'])==2; \
assert sum(e['editorially_verified'] for e in d['entries'])==3; \
print('entries', len(d['entries']))"
```
Expected: `riot-public-gateway-export/2` then `entries 6`.

**4.3 — Confirm the full xtask suite is green against the just-committed goldens.**

```
cargo test -p xtask --all-features
```

**4.4 — Commit the goldens.**

```
git add fixtures/newswire/signed-space-v1.json fixtures/newswire/gateway-space/public-export-v1.json
git commit -m "chore(fixtures): commit signed newswire + public gateway export goldens

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Verification before completion (run all, confirm output)

```
cargo test -p xtask --all-features                       # all xtask tests pass
cargo fmt --all -- --check                               # no diffs
cargo clippy --workspace --all-features -- -D warnings   # clean
cargo run -p xtask -- verify-newswire-export             # PASS (6/6)
# Coverage floor (source of truth = .coverage-thresholds.json):
cargo tarpaulin -p xtask --all-features --fail-under <thresholds.tarpaulin.lines>
```
Do not claim done until every command above is run and its real output confirmed
(`superpowers:verification-before-completion`).

---

## Self-Review

**Spec coverage (against the WS1 brief's PRODUCE list):**
- ✅ `crates/xtask/src/export_newswire.rs` (new) — Task 1.
- ✅ `crates/xtask/src/verify_newswire_export.rs` (new) — Task 2. Not folded in; kept a
  separate module mirroring `verify_conference_export.rs`.
- ✅ `crates/xtask/src/main.rs` (modify) — subcommand registration + `sha256_hex`
  visibility — Task 3.
- ✅ Golden fixture under `fixtures/newswire/` — Task 4 (`signed-space-v1.json` +
  `gateway-space/public-export-v1.json`).
- ✅ Export is verifiable by `verify-newswire-export` exactly as `verify-conference-export`
  validates `public-export-v1.json` (same crypto core reused).
- ✅ Interface contract `{ "schema": "riot-public-gateway-export/2", …, "entries": [ {
  <record>, "verification_status": <…> } ] }` — field names read from the real conference
  fixture; shared fields mirrored, divergences enumerated in the Interface Contract section.
- ✅ E-vs-W distinction carried: `featured` + `editorially_verified` (editorial axis) beside
  `verification_status` (signature axis) — the two-axis model is called out explicitly.
- ✅ WS4 seam (content source swap) and gateway-rewire seam documented.
- ✅ Input source found in code documented (in-process minting via `create_signed_*` +
  `project`; no pre-existing on-disk signed newswire fixture).

**Stale-premise adaptation (brief said the model might differ "in a way that changes the
approach"):** flagged at top — `sample_view()` does not exist; the newswire already renders
projected signed records; the real gap is conference-parity rigor (verifiable, committed,
`riot-public-gateway-export/2`, signature-reverified). The plan targets that gap, not a
non-existent demo-data replacement.

**Placeholder scan:** no `todo!()`, no `unimplemented!()`, no `...` in any code step. Every
function body is complete. The one API-shape caveat (`SignedNewswireRecord` has no
`signed_signer_id()`) is resolved inline in Task 1.7 with the concrete
`inspect_news_record(...).signer_id()` substitution.

**Type consistency:** verified against source —
`create_signed_space_descriptor/news_post/editorial_action` return
`Result<SignedNewswireRecord, NewswireError>` (`entry.rs:230-261`); `SignedNewswireRecord`
fields `signed: SignedWillowEntry {entry_bytes, capability_bytes, signature:[u8;64],
payload_bytes}`, `entry_id: [u8;32]`, `snapshot: ClockSnapshot {unix_seconds,
tai_j2000_micros,…}`, `payload: NewswirePayload` (`entry.rs:26-38`, `entry/willow entry.rs:33-38`).
`inspect_news_record -> Result<VerifiedNewswireRecord, NewswireError>` with `signer_id() ->
[u8;32]` (`entry.rs:80-82`). `project(&VerifiedNewswireRecord, &[VerifiedNewswireRecord],
ProjectionClockV1) -> Result<NewswireProjection, NewswireProjectionError>`
(`projection.rs:177`); `NewswireProjection { open_wire, front_page, earlier,
future_quarantine, editorial_history }` (`projection.rs:119-126`); `ProjectedPost {entry_id:[u8;32],
author_id:[u8;32], tai_j2000_micros, headline:Option<String>, body:Option<String>,
ai_assisted, verification_ids:Vec<[u8;32]>, treatment: PostTreatment, …}`
(`projection.rs:82-105`). `contributors(&NewswireProjection, [u8;32]) -> Vec<ContributorRowV1>`
(`contributors.rs:42`). Authority: descriptor founder must be a communal organizer
(namespace==subspace), editorial actions must be signed by a `editorial_roster` member —
satisfied by `generate_space_organizer_author()` founder + `generate_communal_author_for_namespace`
editor whose id is placed in the roster (proven by `entry.rs:530-569`). Reused conference
constants/fn (`verify_conference_export.rs:20-65`) are all `pub`. `crate::sha256_hex`
(`main.rs:316-321`) is made `pub(crate)`. `crate::hex_codec::{encode,decode}` used exactly as
in `verify_conference_export.rs:103-122`.

**Determinism / reproducibility note:** keys are random per `export-newswire` run, so the two
goldens change wholesale on every regeneration — identical to the conference model, where
`sign-conference-fixture` re-signs with fresh keys. `verify-newswire-export` only asserts
*internal* consistency (signatures verify + every public row binds to a signed record), so a
freshly regenerated pair verifies. CI runs `validate-contracts` only (`ci.yml:49`); the two
newswire commands are exercised via xtask unit tests, exactly as the conference commands are.
Regenerate the goldens intentionally (Task 4), never incidentally.
