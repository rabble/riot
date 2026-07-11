# Conference gateway per-entry signature verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the conference gateway's single, always-negative "Fixture verification: fixture unverified" banner with a real, per-entry cryptographic verification result — computed once at fixture-build time by `riot-core`'s existing `verify_entry()` — and rendered by the unchanged, dependency-free `apps/gateway` HTTP path.

**Architecture:** Two new `cargo xtask` subcommands do the crypto, entirely offline from the running gateway: `sign-conference-fixture` generates two real communal-namespace keypairs and re-signs the two fixture entries (replacing the field literally named `opaque_package_shape_placeholder_not_a_signature`), and `verify-conference-export` reads those real signed entries, calls `riot_core::willow::verify_entry()` on each, and writes only the boolean result — never signature or capability bytes — into the public export JSON. `apps/gateway/riot_gateway.py` changes only to parse and render the new per-entry field.

**Tech Stack:** Rust (`crates/riot-core`, `crates/xtask`, `willow25` v0.6.0-alpha.3), Python 3 stdlib (`apps/gateway`), `segno` (QR regeneration, one-off/dev-only).

**Design decision carried from the spec:** `riot-core`'s only real signing path today is alert-shaped (`AlertDraft` → `create_signed_alert`). There is no per-kind ("observation"/"resource"/"request"/"offer") signing model. Rather than add new crypto/model surface (which the spec explicitly rules out — "adds no new cryptography"), every fixture entry is signed via the existing `AlertDraft`/`create_signed_alert` machinery regardless of its gateway-facing `kind`; non-`alert` kinds get neutral filler values (`Urgency::Unknown`, `Severity::Unknown`, `Certainty::Observed`) for the CAP-specific fields that don't apply to them. This is invisible to the public export — only the verified/invalid boolean ever leaves the source fixture — so it's a private implementation choice, not a user-facing behavior change. A follow-up could generalize `riot-core`'s signing model per kind; that's out of scope here.

**Consequence carried from the spec:** the fixture's current namespace/author identifiers (`PUBLIC_NAMESPACE`, both authors' `nostr_pubkey`/`willow_subspace_id`) are illustrative hex, not real keys, and cannot be preserved once real signing is introduced — a real Ed25519/Willow public key can only ever be the output of generating a real keypair, never a pre-chosen value. This plan regenerates them and updates every place that hardcodes the old values.

---

### Task 1: Hex codec + xtask dependencies

**Files:**
- Modify: `crates/xtask/Cargo.toml`
- Create: `crates/xtask/src/hex_codec.rs`
- Modify: `crates/xtask/src/main.rs:1-15` (module declarations)

- [ ] **Step 1: Add the new dependencies**

Edit `crates/xtask/Cargo.toml`, adding two lines to `[dependencies]`:

```toml
[dependencies]
toml = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
uniffi = { workspace = true, features = ["bindgen"] }
camino = { workspace = true }
riot-core = { path = "../riot-core" }
willow25 = { workspace = true }
```

- [ ] **Step 2: Write `crates/xtask/src/hex_codec.rs`**

```rust
//! Minimal hex codec for the conference fixture sign/verify tools. The
//! workspace has no `hex` crate dependency; this mirrors the existing
//! hand-rolled helpers (`sha256_hex` in `xtask::main`, `decode_hex` in
//! `riot-core`'s `conference_fixture.rs` test).

pub fn encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn decode(value: &str, label: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{label} must be lowercase hexadecimal"));
    }
    (0..value.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&value[i..i + 2], 16)
                .map_err(|_| format!("{label} must be lowercase hexadecimal"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_arbitrary_bytes() {
        let bytes = [0u8, 1, 2, 254, 255, 16, 128];
        assert_eq!(decode(&encode(&bytes), "test").unwrap(), bytes);
    }

    #[test]
    fn rejects_odd_length() {
        assert!(decode("abc", "test").is_err());
    }

    #[test]
    fn rejects_non_hex() {
        assert!(decode("zz", "test").is_err());
    }
}
```

- [ ] **Step 3: Declare the module in `main.rs`**

Add near the top of `crates/xtask/src/main.rs` (alongside any existing `mod`/`use` lines at the top of the file):

```rust
mod hex_codec;
```

- [ ] **Step 4: Run the new unit tests**

Run: `cargo test -p xtask hex_codec`
Expected: 3 tests pass (`roundtrips_arbitrary_bytes`, `rejects_odd_length`, `rejects_non_hex`).

- [ ] **Step 5: Commit**

```bash
git add crates/xtask/Cargo.toml crates/xtask/src/hex_codec.rs crates/xtask/src/main.rs
git commit -m "feat(xtask): add hex codec and riot-core/willow25 deps for conference signing"
```

---

### Task 2: `sign-conference-fixture` — real signatures for the source fixture

This subcommand generates two fresh keypairs sharing one communal namespace,
signs the two fixture entries via `riot_core::willow::create_signed_alert`,
and rewrites `fixtures/conference/incident-space-v1.json` +
`fixtures/conference/package-manifest-v1.json` with real identifiers and
real signature/entry/capability bytes (hex-encoded). `canonical_sha256` is
intentionally left stale here — Task 4 recomputes and re-pins it, matching
how every other pinned hash in this codebase is deliberately hand-updated,
never auto-derived at test time.

**Files:**
- Create: `crates/xtask/src/sign_conference_fixture.rs`
- Modify: `crates/xtask/src/main.rs`

- [ ] **Step 1: Write the module**

```rust
//! Regenerates the conference incident-space fixture with real Ed25519
//! signatures, replacing the illustrative placeholder field and identifiers.
//! Production riot-core signing is alert-shaped only (`AlertDraft` /
//! `create_signed_alert`); every fixture entry is signed through that path
//! regardless of its gateway-facing `kind`. Non-"alert" kinds get neutral
//! filler for the CAP-specific fields (Urgency::Unknown, Severity::Unknown,
//! Certainty::Observed) since only the verified/invalid boolean this proves
//! ever reaches the public export — the CAP shape itself is never exposed.

use std::fs;
use std::path::Path;

use riot_core::model::{Certainty, Severity, Urgency};
use riot_core::willow::{
    create_signed_alert, entry_id, generate_communal_author, generate_communal_author_for_namespace,
    AlertDraft,
};
use serde_json::{json, Value};

use crate::hex_codec;

/// Far enough in the future that `expires_at > created_at` always holds,
/// regardless of when this tool is run (2100-01-01T00:00:00Z, Unix seconds).
/// The value is never exposed publicly — it only satisfies riot-core's
/// AlertPayload validation inside the opaque signed payload.
const FAR_FUTURE_EXPIRY: u64 = 4_102_444_800;

pub fn run(root: &Path) -> Result<(), String> {
    let fixture_path = root.join("fixtures/conference/incident-space-v1.json");
    let manifest_path = root.join("fixtures/conference/package-manifest-v1.json");

    let raw = fs::read_to_string(&fixture_path)
        .map_err(|error| format!("read {}: {error}", fixture_path.display()))?;
    let mut doc: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("parse {}: {error}", fixture_path.display()))?;

    let founder = generate_communal_author()
        .map_err(|error| format!("generate founding author: {error}"))?;
    let namespace_bytes = *founder.namespace_id().as_bytes();
    let second =
        generate_communal_author_for_namespace(namespace_bytes)
            .map_err(|error| format!("generate second author: {error}"))?;

    let authors = doc["authors"]
        .as_array()
        .cloned()
        .ok_or("incident fixture: authors must be an array")?;
    if authors.len() != 2 {
        return Err("incident fixture: expected exactly 2 authors".to_string());
    }

    let signers = [&founder, &second];
    for (index, author) in signers.iter().enumerate() {
        let subspace_hex = hex_codec::encode(author.subspace_id().as_bytes());
        doc["authors"][index]["nostr_pubkey"] = json!(subspace_hex);
        doc["authors"][index]["willow_subspace_id"] = json!(subspace_hex);
    }
    doc["namespace"]["id"] = json!(hex_codec::encode(&namespace_bytes));

    let entries = doc["entries"]
        .as_array()
        .cloned()
        .ok_or("incident fixture: entries must be an array")?;

    for (index, entry) in entries.iter().enumerate() {
        let kind = entry["kind"]
            .as_str()
            .ok_or("incident entry: kind must be a string")?
            .to_string();
        let headline = entry["title"]
            .as_str()
            .ok_or("incident entry: title must be a string")?
            .to_string();
        let description = entry["body"]
            .as_str()
            .ok_or("incident entry: body must be a string")?
            .to_string();
        let ai_assisted = entry["ai_assisted_draft"]
            .as_bool()
            .ok_or("incident entry: ai_assisted_draft must be a bool")?;

        // The current fixture always signs with the first ("founding")
        // author; a later pass could map each entry to its own claimed
        // author. Both authors share the namespace already.
        let author = signers[0];

        let (urgency, severity, certainty) = if kind == "alert" {
            (Urgency::Immediate, Severity::Severe, Certainty::Observed)
        } else {
            (Urgency::Unknown, Severity::Unknown, Certainty::Observed)
        };

        let draft = AlertDraft {
            valid_from: None,
            expires_at: FAR_FUTURE_EXPIRY,
            language: "en".to_string(),
            urgency,
            severity,
            certainty,
            headline,
            description,
            affected_area_claim: None,
            source_claims: vec!["riot conference gateway fixture".to_string()],
            ai_assisted,
        };

        let signed_alert = create_signed_alert(author, draft)
            .map_err(|error| format!("sign entry {index} ({kind}): {error}"))?;
        let signed = signed_alert.signed;

        let entry_hex = hex_codec::encode(&entry_id(&signed.entry_bytes));
        doc["entries"][index]["willow_entry_id"] = json!(entry_hex);
        doc["entries"][index]["author_nostr_pubkey"] =
            json!(hex_codec::encode(author.subspace_id().as_bytes()));
        doc["entries"][index]["willow_entry_bytes"] = json!(hex_codec::encode(&signed.entry_bytes));
        doc["entries"][index]["willow_capability_bytes"] =
            json!(hex_codec::encode(&signed.capability_bytes));
        doc["entries"][index]["signature"] = json!(hex_codec::encode(&signed.signature));
        if let Some(map) = doc["entries"][index].as_object_mut() {
            map.remove("opaque_package_shape_placeholder_not_a_signature");
        }
    }

    let pretty = serde_json::to_string_pretty(&doc)
        .map_err(|error| format!("serialize {}: {error}", fixture_path.display()))?;
    fs::write(&fixture_path, pretty + "\n")
        .map_err(|error| format!("write {}: {error}", fixture_path.display()))?;

    let manifest_raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let mut manifest: Value = serde_json::from_str(&manifest_raw)
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    manifest["namespace"] = json!(hex_codec::encode(&namespace_bytes));
    let manifest_pretty = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("serialize {}: {error}", manifest_path.display()))?;
    fs::write(&manifest_path, manifest_pretty + "\n")
        .map_err(|error| format!("write {}: {error}", manifest_path.display()))?;

    println!(
        "sign-conference-fixture: PASS (namespace={})",
        hex_codec::encode(&namespace_bytes)
    );
    Ok(())
}
```

- [ ] **Step 2: Declare the module and wire the subcommand into `main.rs`**

Add near the top of `crates/xtask/src/main.rs`:

```rust
mod sign_conference_fixture;
```

In the `match args.next().as_deref()` block, add a new arm before the
catch-all `Some(other)` arm (matching the existing `generate-bindings` arm's
shape exactly):

```rust
        Some("sign-conference-fixture") => match sign_conference_fixture::run(&workspace_root()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("sign-conference-fixture: FAIL: {error}");
                ExitCode::FAILURE
            }
        },
```

Add `"sign-conference-fixture"` to the `available_commands()` slice:

```rust
fn available_commands() -> &'static [&'static str] {
    &[
        "validate-contracts",
        "generate-bindings",
        "sign-conference-fixture",
        "verify-conference-export",
    ]
}
```

(`"verify-conference-export"` is listed now so the usage text is right after
Task 3 too — it's a no-op string until Task 3 adds its match arm.)

- [ ] **Step 3: Compile-check**

Run: `cargo check -p xtask`
Expected: compiles clean (this only exercises the new code paths at
type-check time; the tool itself is exercised in Task 5).

- [ ] **Step 4: Commit**

```bash
git add crates/xtask/src/sign_conference_fixture.rs crates/xtask/src/main.rs
git commit -m "feat(xtask): add sign-conference-fixture subcommand"
```

---

### Task 3: `verify-conference-export` — real per-entry verification

Reads the now-really-signed source fixture, calls `verify_entry()` per
entry, and writes `public-export-v1.json` with a per-entry
`verification_status` — never copying signature/capability/entry bytes into
the public file.

**Files:**
- Create: `crates/xtask/src/verify_conference_export.rs`
- Modify: `crates/xtask/src/main.rs`

- [ ] **Step 1: Write the module, with its verification logic as a testable pure function**

```rust
//! Verifies each conference fixture entry's real signature and writes the
//! public, proof-free per-entry verification_status into the gateway export.
//! No signature, capability, or entry bytes are copied into the public file
//! — only the boolean `verify_entry()` result, matching the existing
//! `_FORBIDDEN_FIELD_PARTS` boundary in `apps/gateway/riot_gateway.py`,
//! which already refuses any public field whose name contains
//! "capability", "secret", "receipt", etc.

use std::fs;
use std::path::Path;

use riot_core::willow::{decode_capability_canonic, decode_entry_canonic, verify_entry, AuthorisationToken};
use serde_json::{json, Value};
use willow25::prelude::SubspaceSignature;

use crate::hex_codec;

pub const VERIFICATION_STATUS_VALID: &str = "signature_verified";
pub const VERIFICATION_STATUS_INVALID: &str = "signature_invalid";

/// Pure verification core, independent of file I/O, so it's directly
/// unit-testable with hand-built byte inputs (see the tests below).
pub fn verify_signed_entry(
    entry_bytes: &[u8],
    capability_bytes: &[u8],
    signature: &[u8; 64],
) -> Result<bool, String> {
    let entry = decode_entry_canonic(entry_bytes).map_err(|error| format!("decode entry: {error}"))?;
    let capability =
        decode_capability_canonic(capability_bytes).map_err(|error| format!("decode capability: {error}"))?;
    let token = AuthorisationToken::new(capability, SubspaceSignature::from(*signature));
    Ok(verify_entry(&entry, &token))
}

pub fn run(root: &Path) -> Result<(), String> {
    let fixture_path = root.join("fixtures/conference/incident-space-v1.json");
    let export_path = root.join("fixtures/conference/gateway-space/public-export-v1.json");

    let raw = fs::read_to_string(&fixture_path)
        .map_err(|error| format!("read {}: {error}", fixture_path.display()))?;
    let fixture: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("parse {}: {error}", fixture_path.display()))?;

    let export_raw = fs::read_to_string(&export_path)
        .map_err(|error| format!("read {}: {error}", export_path.display()))?;
    let mut export: Value = serde_json::from_str(&export_raw)
        .map_err(|error| format!("parse {}: {error}", export_path.display()))?;

    let fixture_entries = fixture["entries"]
        .as_array()
        .ok_or("incident fixture: entries must be an array")?;
    let export_entries = export["entries"]
        .as_array()
        .cloned()
        .ok_or("public export: entries must be an array")?;
    if fixture_entries.len() != export_entries.len() {
        return Err(format!(
            "entry count mismatch: fixture has {}, export has {}",
            fixture_entries.len(),
            export_entries.len()
        ));
    }

    let mut verified_count = 0usize;
    for (index, entry) in fixture_entries.iter().enumerate() {
        let entry_bytes = hex_codec::decode(
            entry["willow_entry_bytes"]
                .as_str()
                .ok_or("incident entry: willow_entry_bytes must be a string")?,
            "willow_entry_bytes",
        )?;
        let capability_bytes = hex_codec::decode(
            entry["willow_capability_bytes"]
                .as_str()
                .ok_or("incident entry: willow_capability_bytes must be a string")?,
            "willow_capability_bytes",
        )?;
        let signature: [u8; 64] = hex_codec::decode(
            entry["signature"]
                .as_str()
                .ok_or("incident entry: signature must be a string")?,
            "signature",
        )?
        .try_into()
        .map_err(|_| "signature must be exactly 64 bytes".to_string())?;

        let valid = verify_signed_entry(&entry_bytes, &capability_bytes, &signature)?;
        if valid {
            verified_count += 1;
        }
        let status = if valid {
            VERIFICATION_STATUS_VALID
        } else {
            VERIFICATION_STATUS_INVALID
        };
        export["entries"][index]["verification_status"] = json!(status);
    }
    if let Some(map) = export.as_object_mut() {
        map.remove("verification_status");
    }

    let pretty = serde_json::to_string_pretty(&export)
        .map_err(|error| format!("serialize {}: {error}", export_path.display()))?;
    fs::write(&export_path, pretty + "\n")
        .map_err(|error| format!("write {}: {error}", export_path.display()))?;

    println!(
        "verify-conference-export: PASS ({verified_count}/{} entries signature-verified)",
        export_entries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use riot_core::model::{Certainty, Severity, Urgency};
    use riot_core::willow::{create_signed_alert, generate_communal_author, AlertDraft};

    fn draft() -> AlertDraft {
        AlertDraft {
            valid_from: None,
            expires_at: 4_102_444_800,
            language: "en".to_string(),
            urgency: Urgency::Immediate,
            severity: Severity::Severe,
            certainty: Certainty::Observed,
            headline: "Test alert".to_string(),
            description: "Test description".to_string(),
            affected_area_claim: None,
            source_claims: vec!["test".to_string()],
            ai_assisted: false,
        }
    }

    #[test]
    fn genuine_signature_verifies() {
        let author = generate_communal_author().unwrap();
        let signed = create_signed_alert(&author, draft()).unwrap().signed;
        let valid = verify_signed_entry(&signed.entry_bytes, &signed.capability_bytes, &signed.signature)
            .unwrap();
        assert!(valid);
    }

    #[test]
    fn tampered_entry_bytes_do_not_verify() {
        let author = generate_communal_author().unwrap();
        let signed = create_signed_alert(&author, draft()).unwrap().signed;
        let mut tampered_entry = signed.entry_bytes.clone();
        let last = tampered_entry.len() - 1;
        tampered_entry[last] ^= 0xFF;
        let valid = verify_signed_entry(&tampered_entry, &signed.capability_bytes, &signed.signature)
            .unwrap();
        assert!(!valid);
    }

    #[test]
    fn signature_from_a_different_key_does_not_verify() {
        let author = generate_communal_author().unwrap();
        let other_author = generate_communal_author().unwrap();
        let signed = create_signed_alert(&author, draft()).unwrap().signed;
        let other_signed = create_signed_alert(&other_author, draft()).unwrap().signed;
        let valid = verify_signed_entry(
            &signed.entry_bytes,
            &signed.capability_bytes,
            &other_signed.signature,
        )
        .unwrap();
        assert!(!valid);
    }
}
```

- [ ] **Step 2: Run the new unit tests before wiring the CLI**

Run: `cargo test -p xtask verify_conference_export`
Expected: 3 tests pass — `genuine_signature_verifies`,
`tampered_entry_bytes_do_not_verify`, `signature_from_a_different_key_does_not_verify`.

- [ ] **Step 3: Declare the module and wire the subcommand into `main.rs`**

Add near the top of `crates/xtask/src/main.rs`:

```rust
mod verify_conference_export;
```

Add the match arm (the `available_commands()` slice was already updated in
Task 2 to include this name):

```rust
        Some("verify-conference-export") => match verify_conference_export::run(&workspace_root()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("verify-conference-export: FAIL: {error}");
                ExitCode::FAILURE
            }
        },
```

- [ ] **Step 4: Compile-check**

Run: `cargo check -p xtask`
Expected: compiles clean.

- [ ] **Step 5: Commit**

```bash
git add crates/xtask/src/verify_conference_export.rs crates/xtask/src/main.rs
git commit -m "feat(xtask): add verify-conference-export subcommand"
```

---

### Task 4: Update `riot-core`'s fixture-shape test for the new fields

The fixture test has a strict, closed-key CBOR canonical-freeze encoder
(`crates/riot-core/tests/conference_fixture.rs:125-196`,
`canonical_fixture_bytes`) that encodes each entry as an 8-key CBOR map
(`encoder.map(8)` at line 164) and separately asserts an exact 8-key JSON
key set per entry (`expect_exact_keys` at lines 271-284). Task 2 adds two
*variable-length* fields per entry (`willow_entry_bytes`,
`willow_capability_bytes`) and renames
`opaque_package_shape_placeholder_not_a_signature` to `signature` (same
64-byte length, new name) — both the map arity and the exact-keys list
must grow to 10, and the existing `decode_hex` helper can't be reused
as-is for the two new fields since it asserts a *fixed* byte length, not
variable-length hex.

**Files:**
- Modify: `crates/riot-core/tests/conference_fixture.rs`

- [ ] **Step 1: Add a variable-length hex decoder next to the existing fixed-length one**

Insert this function immediately after `decode_hex` (after line 74, right
before `fn is_normalized_site_route`):

```rust
fn decode_hex_any_length(value: &str, label: &str) -> Vec<u8> {
    assert_eq!(
        value.len() % 2,
        0,
        "{label} must be an even-length hexadecimal string"
    );
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{label} must be hexadecimal"
    );
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("validated hex"))
        .collect()
}
```

- [ ] **Step 2: Update the canonical CBOR encoder (`canonical_fixture_bytes`)**

At line 164, change the entry map arity from 8 to 10:

```rust
            encoder.map(10)?;
```

Replace lines 182-186 (the placeholder-signature encoding, currently the
last field in the per-entry loop) with three fields — the renamed
signature plus the two new byte blobs, at indices 7, 8, 9:

```rust
            encoder.u8(7)?.bytes(&decode_hex(
                string(entry, "signature"),
                64,
                "entry.signature",
            ))?;
            encoder.u8(8)?.bytes(&decode_hex_any_length(
                string(entry, "willow_entry_bytes"),
                "entry.willow_entry_bytes",
            ))?;
            encoder.u8(9)?.bytes(&decode_hex_any_length(
                string(entry, "willow_capability_bytes"),
                "entry.willow_capability_bytes",
            ))?;
```

- [ ] **Step 3: Update the exact-keys list and the per-entry assertions in the test function**

Replace lines 271-284 (`expect_exact_keys(entry, &[...], "entry")`):

```rust
        expect_exact_keys(
            entry,
            &[
                "kind",
                "willow_entry_id",
                "author_nostr_pubkey",
                "title",
                "body",
                "created_at",
                "ai_assisted_draft",
                "signature",
                "willow_entry_bytes",
                "willow_capability_bytes",
            ],
            "entry",
        );
```

Replace lines 300-304 (the placeholder-signature decode assertion in the
loop body):

```rust
        decode_hex(string(entry, "signature"), 64, "entry.signature");
        decode_hex_any_length(string(entry, "willow_entry_bytes"), "entry.willow_entry_bytes");
        decode_hex_any_length(
            string(entry, "willow_capability_bytes"),
            "entry.willow_capability_bytes",
        );
```

- [ ] **Step 4: Run the fixture test to see the real diff against current file state**

Run: `cargo test -p riot-core --test conference_fixture 2>&1 | head -60`
Expected: FAIL — the fixture on disk still has the old placeholder shape
(Task 2/3 haven't been run against the real files yet; that's Task 5). Read
the failure output: it should fail because the *old* fixture file is
missing the new keys / still has the old key name, confirming the test
itself now expects the new shape correctly. This step sanity-checks the
test edit, not fixture regeneration.

- [ ] **Step 5: Commit the test update**

```bash
git add crates/riot-core/tests/conference_fixture.rs
git commit -m "test(riot-core): expect real signature/entry/capability fields in conference fixture"
```

(This commit is expected to leave `cargo test -p riot-core --test
conference_fixture` red until Task 5 regenerates the fixture files — that's
fine; Task 5 is next and turns it green.)

---

### Task 5: Regenerate the fixtures and re-pin every dependent hash

This is where the tools actually run against the real files, and where
every hardcoded hash/identifier that depends on fixture *content* gets
updated to match. Nothing here is guesswork — each value is read back from
the actual regenerated files.

**Files:**
- Modify (generated content, not hand-edited): `fixtures/conference/incident-space-v1.json`, `fixtures/conference/package-manifest-v1.json`, `fixtures/conference/gateway-space/public-export-v1.json`, `fixtures/conference/gateway-space/open-in-riot-v1.svg`
- Modify (hand-edited to match): `apps/gateway/riot_gateway.py` (constants only — see Task 6 for the rest of that file's changes)

- [ ] **Step 1: Run the sign step**

Run: `cargo run --locked --package xtask -- sign-conference-fixture`
Expected: `sign-conference-fixture: PASS (namespace=<64 hex chars>)`. Note
the printed namespace value — you'll need it in Step 3.

- [ ] **Step 2: Recompute and re-pin `canonical_sha256` in the incident fixture**

Run: `cargo test -p riot-core --test conference_fixture 2>&1 | grep -A2 canonical_sha256`
Expected output includes the mismatch, e.g. `left: "<old hash>", right:
"<new hash>"` (exact labels depend on the assertion macro used in the
test — read whichever side is computed from the live file bytes, not the
literal expected-string side). Copy that computed value into
`fixtures/conference/incident-space-v1.json`'s `canonical_sha256` field by
hand.

Run: `cargo test -p riot-core --test conference_fixture`
Expected: PASS, all tests green.

- [ ] **Step 3: Run the verify step**

Run: `cargo run --locked --package xtask -- verify-conference-export`
Expected: `verify-conference-export: PASS (2/2 entries signature-verified)`.

- [ ] **Step 4: Regenerate the QR SVG for the new namespace**

The namespace changed in Step 1, so the QR code's encoded
`riot://open?namespace=...` value must change too. Use a throwaway virtual
environment (the workspace has no Python dependencies checked in):

```bash
python3 -m venv /tmp/riot-qr-venv
/tmp/riot-qr-venv/bin/pip install --quiet segno==1.6.6
```

Then run (replace `<NAMESPACE>` with the exact hex printed in Step 1):

```bash
/tmp/riot-qr-venv/bin/python3 - "<NAMESPACE>" <<'PY'
import re
import sys
import segno

namespace = sys.argv[1]
value = f"riot://open?namespace={namespace}"
qr = segno.make(value, error="m")
qr.save(
    "/tmp/riot-qr.svg",
    kind="svg",
    xmldecl=False,
    svgclass=None,
    lineclass="qrline",
    title="Open in Riot",
    desc=value,
    border=4,
)
svg = open("/tmp/riot-qr.svg").read()
svg = svg.replace(
    '<svg xmlns="http://www.w3.org/2000/svg" width="49" height="49">',
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 49 49" width="196" '
    'height="196" role="img" aria-labelledby="qr-title qr-desc" '
    'data-generator="Segno 1.6.6" data-module-count="41" '
    f'data-qr-value="{value}">',
)
svg = svg.replace("<title>", '<title id="qr-title">')
svg = svg.replace("<desc>", '<desc id="qr-desc">')
with open("fixtures/conference/gateway-space/open-in-riot-v1.svg", "w") as out:
    out.write(svg)
print("wrote", len(svg), "bytes")
PY
```

Run this from the repository root (`REPO_ROOT`), so the relative output
path resolves correctly. This reproduces the checked-in file's exact
attribute structure (`viewBox`, `role`, `aria-labelledby`,
`data-generator`, `data-module-count`, `data-qr-value`, `id`-tagged
`<title>`/`<desc>`) — verified during plan authoring to produce a
byte-identical QR module path to the file it replaces, for a
same-length payload string.

If `qr.version != 6` or `qr.error != 'M'` for the new namespace (print
`qr.version`/`qr.error` to check — both should stay stable since the
payload's total byte length is unchanged: `len("riot://open?namespace=")
+ 64` is fixed regardless of which real hex digits fill the namespace),
stop and re-read `apps/gateway/tests/test_gateway.py`'s
`_decode_version_6_m_qr`/`_version_6_function_modules` before proceeding —
those are hardcoded to version-6/error-M structure.

- [ ] **Step 5: Recompute every pinned constant in `riot_gateway.py`**

Run:

```bash
python3 - <<'PY'
import hashlib
from pathlib import Path

root = Path(".")
export = root / "fixtures/conference/gateway-space/public-export-v1.json"
fixture = root / "fixtures/conference/incident-space-v1.json"
manifest = root / "fixtures/conference/package-manifest-v1.json"
qr = root / "fixtures/conference/gateway-space/open-in-riot-v1.svg"

for label, path in (
    ("PINNED_EXPORT_SHA256", export),
    ("SOURCE_FIXTURE_SHA256", fixture),
    ("SOURCE_MANIFEST_SHA256", manifest),
    ("PINNED_QR_SVG_SHA256", qr),
):
    print(label, "=", hashlib.sha256(path.read_bytes()).hexdigest())
PY
```

Update these four constants in `apps/gateway/riot_gateway.py`
(`PINNED_EXPORT_SHA256`, `SOURCE_FIXTURE_SHA256`, `SOURCE_MANIFEST_SHA256`,
`PINNED_QR_SVG_SHA256`) to the printed values, and update
`PUBLIC_NAMESPACE` to the namespace hex printed in Step 1. (The full set of
`riot_gateway.py` code changes, beyond these five constants, is Task 6.)

- [ ] **Step 6: Commit the regenerated fixtures**

```bash
git add fixtures/conference/incident-space-v1.json \
        fixtures/conference/package-manifest-v1.json \
        fixtures/conference/gateway-space/public-export-v1.json \
        fixtures/conference/gateway-space/open-in-riot-v1.svg
git commit -m "chore(fixtures): regenerate conference fixture with real signatures"
```

(`riot_gateway.py`'s constant updates are committed together with the rest
of Task 6's changes, since that file isn't fully consistent until Task 6
completes.)

---

### Task 6: `apps/gateway/riot_gateway.py` — parse and render the real status

**Files:**
- Modify: `apps/gateway/riot_gateway.py`

- [ ] **Step 1: Bump the schema revision and update the five constants from Task 5**

```python
EXPORT_SCHEMA = "riot-public-gateway-export/2"
```

(Leave `EXPORT_REVISION`, `RENDERER_PROFILE`, `INCIDENT_TITLE`,
`SOURCE_FIXTURE`, `SOURCE_MANIFEST` unchanged — only the five values from
Task 5 Step 5 plus this schema string change.)

- [ ] **Step 2: Replace the document-level verification field with a per-entry one**

Remove `VERIFICATION_STATUS = "fixture_unverified"` and add:

```python
VERIFICATION_STATUS_VALID = "signature_verified"
VERIFICATION_STATUS_INVALID = "signature_invalid"
ALLOWED_VERIFICATION_STATUSES = frozenset(
    {VERIFICATION_STATUS_VALID, VERIFICATION_STATUS_INVALID}
)
```

In `_ENTRY_FIELDS`, add `"verification_status"`:

```python
_ENTRY_FIELDS = frozenset(
    {"kind", "entry_id", "signer", "title", "body", "freshness", "ai_assisted", "verification_status"}
)
```

In `_TOP_LEVEL_FIELDS`, remove `"verification_status"` (it moves to the
entry level):

```python
_TOP_LEVEL_FIELDS = frozenset(
    {
        "schema",
        "export_revision",
        "renderer_profile",
        "source_fixture",
        "source_fixture_sha256",
        "source_manifest",
        "source_manifest_sha256",
        "namespace",
        "visibility",
        "title",
        "generated_at",
        "entries",
    }
)
```

- [ ] **Step 3: Update `PublicEntry`, `PublicGateway`, `_validate_document`, and `_parse_entry`**

```python
@dataclass(frozen=True)
class PublicEntry:
    kind: str
    entry_id: str
    signer: str
    title: str
    body: str
    freshness: str
    ai_assisted: bool
    verification_status: str
```

In the `PublicGateway` dataclass field list, delete this line (it's no
longer document-level, `verification_status` now lives on each
`PublicEntry`):

```python
    verification_status: str
```

In `PublicGateway.from_file`, delete this line from the `object.__setattr__`
sequence:

```python
        object.__setattr__(gateway, "verification_status", VERIFICATION_STATUS)
```

`render()`'s call to `_render_page` is updated in Step 5 below (it drops
the now-removed `self.verification_status` positional argument).

In `_validate_document`, remove this block entirely (the check now lives
per-entry, not document-wide):

```python
    if document.get("verification_status") != VERIFICATION_STATUS:
        raise GatewayError("fixture verification status is not permitted")
```

In `_parse_entry`, add validation and pass-through for the new field:

```python
def _parse_entry(value: object) -> PublicEntry:
    if not isinstance(value, Mapping) or set(value) != _ENTRY_FIELDS:
        raise GatewayError("entry fields are not permitted")
    kind = value.get("kind")
    if kind not in ALLOWED_KINDS:
        raise GatewayError("entry kind is not permitted")
    entry_id = _require_id(value.get("entry_id"), "entry_id")
    signer = _require_id(value.get("signer"), "signer")
    title = _require_text(value.get("title"), "title")
    body = _require_text(value.get("body"), "body")
    freshness = _require_timestamp(value.get("freshness"), "freshness")
    ai_assisted = value.get("ai_assisted")
    if not isinstance(ai_assisted, bool):
        raise GatewayError("ai_assisted must be a boolean")
    verification_status = value.get("verification_status")
    if verification_status not in ALLOWED_VERIFICATION_STATUSES:
        raise GatewayError("entry verification status is not permitted")
    return PublicEntry(kind, entry_id, signer, title, body, freshness, ai_assisted, verification_status)
```

- [ ] **Step 4: Add verification badge CSS**

In the `STYLE_CSS` constant, add two new rules near the existing `.kind--*`
rules (reusing the existing color tokens — no new palette values):

```css
.verify {
  display: inline-block;
  margin: 0 0 0.5rem 0.4rem;
  padding: 0.1rem 0.5rem;
  font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, sans-serif;
  font-weight: 700;
  font-size: 0.7rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  border-radius: 2px;
  border: 1px solid currentColor;
}
.verify--valid { color: var(--anchor); }
.verify--invalid { background: var(--hazard); border-color: var(--hazard); color: #fff; }
```

Since `STYLE_CSS` is hashed into the CSP's `style-src` directive
(`_STYLE_CSS_HASH` in this same file), this edit changes
`CONTENT_SECURITY_POLICY`'s value automatically — no manual re-pinning
needed here (unlike the fixture hashes in Task 5, this one is
self-computed from the constant, by design).

- [ ] **Step 5: Update `_render_page` and `_render_entry`**

In `_render_page`, remove the "Fixture verification" banner and replace it
with a summary computed from the entries:

```python
def _render_page(
    title: str,
    namespace: str,
    qr_svg: str,
    entries: tuple[PublicEntry, ...],
) -> str:
    escaped_title = escape(title)
    verified_count = sum(1 for entry in entries if entry.verification_status == VERIFICATION_STATUS_VALID)
    cards = "".join(_render_entry(entry) for entry in entries)
    namespace_uri = f"riot://open?namespace={namespace}"
    return f"""<!doctype html>
<html lang=\"en\">
<head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{escaped_title} · Riot</title><style>{STYLE_CSS}</style></head>
<body>
<main class=\"board\">
  <p class=\"eyebrow\">Public Riot export · renderer profile: {RENDERER_PROFILE}</p>
  <p class=\"fixture-status\">Signature verification: <span class=\"fixture-status__tag\">{verified_count} of {len(entries)} entries signature-verified</span></p>
  <h1 class=\"headline\">{escaped_title}</h1>
  <p class=\"subhead\">Available offline from this local public export.</p>
  <div class=\"ticket\">
    <div class=\"ticket__main\">
      <p class=\"ticket__action\"><a class=\"ticket__link\" href=\"{namespace_uri}\">Open in Riot</a></p>
      <p class=\"ticket__namespace\">Public namespace: <code>{namespace}</code></p>
    </div>
    <div class=\"ticket__qr\">{qr_svg}</div>
  </div>
  <h2 class=\"entries-label\">Incident entries</h2>
  <section aria-label=\"Incident entries\" class=\"entries\">{cards}</section>
</main>
</body>
</html>"""
```

`render()` in `PublicGateway` calls `_render_page` with one fewer
positional argument now (`verification_status` is gone from the document
level); update that call site to drop it:

```python
        return _render_page(
            self.title,
            self.namespace,
            _load_qr_svg(),
            entries,
        )
```

In `_render_entry`, replace the static "Claimed author (unverified
fixture)" wording with a real verified/invalid badge, keeping the claimed
signer's hash visible (it's still just a claim, not proof on its own):

```python
def _render_entry(entry: PublicEntry) -> str:
    assisted = "AI-assisted draft" if entry.ai_assisted else "Human-authored draft"
    if entry.verification_status == VERIFICATION_STATUS_VALID:
        verify_badge = '<span class="verify verify--valid">Signature verified</span>'
    else:
        verify_badge = '<span class="verify verify--invalid">Signature invalid</span>'
    return f"""
<article class=\"entry entry--{entry.kind}\">
  <span class=\"kind kind--{entry.kind}\">{escape(entry.kind.title())}</span>{verify_badge}
  <h2 class=\"entry__title\">{escape(entry.title)}</h2>
  <p class=\"entry__body\">{escape(entry.body)}</p>
  <p class=\"entry__meta\"><span>Claimed author: <code>{entry.signer}</code></span><span>Freshness: <time datetime=\"{entry.freshness}\">{entry.freshness}</time></span><span>{assisted}</span></p>
</article>"""
```

- [ ] **Step 6: Run the gateway's own module import as a smoke check**

Run: `python3 -c "import sys; sys.path.insert(0, 'apps/gateway'); import riot_gateway; print(riot_gateway.PublicGateway.from_file(riot_gateway.DEFAULT_EXPORT_PATH).render('/site/')[:200])"`
Expected: prints the start of a rendered HTML page with no traceback. If it
raises `GatewayError`, the constants from Task 5 Step 5 don't match the
regenerated files byte-for-byte — recheck them before continuing.

- [ ] **Step 7: Commit**

```bash
git add apps/gateway/riot_gateway.py
git commit -m "feat(gateway): render real per-entry signature verification"
```

---

### Task 7: Update the Python test suite

**Files:**
- Modify: `apps/gateway/tests/test_gateway.py`

- [ ] **Step 1: Update hardcoded hex/namespace values**

Every literal hex string in the test file that matches the *old* pinned
namespace/signer/entry-id values needs to become the new value from Task
5. Search the file for the old `PUBLIC_NAMESPACE` value
(`3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c`) and the
old signer value
(`d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a`) — both
appear in several `assertIn`/`assertEqual` calls (e.g. the
`riot://open?namespace=...` assertions, `data-qr-value="..."` assertions,
and `QR_SVG_SHA256`). Replace every occurrence with the corresponding new
value.  Also update `QR_SVG_SHA256` at the top of the file to the
`PINNED_QR_SVG_SHA256` value computed in Task 5 Step 5.

- [ ] **Step 2: Update wording assertions**

Replace:
```python
        self.assertIn("Claimed author (unverified fixture):", page)
```
with:
```python
        self.assertIn("Claimed author:", page)
        self.assertIn("Signature verified", page)
```
(there are two occurrences of the old string in the file — the
`test_renders_unverified_fixture_provenance_freshness_ai_offline_and_open_in_riot`
test and the smoke test module-level constant list is in the shell script,
not here — only fix the Python occurrences.) Rename that test method to
`test_renders_signature_verification_provenance_freshness_ai_offline_and_open_in_riot`
so its name matches what it actually checks.

- [ ] **Step 3: Add a negative-path test proving invalid signatures render, not vanish**

Add this test to `PublicGatewayTest`:

```python
    def test_renders_signature_invalid_entries_without_upgrading_or_dropping_them(self) -> None:
        document = json.loads(EXPORT.read_text())
        candidate = json.loads(json.dumps(document))
        candidate["entries"][0]["verification_status"] = "signature_invalid"
        self.assertIsNone(PublicGateway.validate_document(candidate))

        entries = gateway_module._validate_document(candidate)
        self.assertEqual(entries[0].verification_status, "signature_invalid")
        self.assertEqual(entries[1].verification_status, "signature_verified")

    def test_rejects_unknown_verification_status(self) -> None:
        document = json.loads(EXPORT.read_text())
        candidate = json.loads(json.dumps(document))
        candidate["entries"][0]["verification_status"] = "totally_trusted"
        with self.assertRaisesRegex(GatewayError, "verification status"):
            PublicGateway.validate_document(candidate)
```

- [ ] **Step 4: Run the full Python suite**

Run: `python3 -m unittest apps.gateway.tests.test_gateway -v`
Expected: all tests pass, including the 2 new ones.

- [ ] **Step 5: Commit**

```bash
git add apps/gateway/tests/test_gateway.py
git commit -m "test(gateway): update fixture identifiers and add signature-status tests"
```

---

### Task 8: Update the shell smoke test

**Files:**
- Modify: `scripts/conference/gateway-smoke.sh`

- [ ] **Step 1: Update the hardcoded namespace in the required-substrings list and wording**

Replace the old namespace hex in the `data-qr-value="riot://open?namespace=..."`
required string with the new value from Task 5. Replace:

```sh
    "Claimed author (unverified fixture):",
```

with:

```sh
    "Claimed author:",
    "Signature verified",
```

- [ ] **Step 2: Run the smoke test**

Run: `./scripts/conference/gateway-smoke.sh`
Expected: prints `gateway-smoke: local revision=... sha256=...` with exit
code 0.

- [ ] **Step 3: Commit**

```bash
git add scripts/conference/gateway-smoke.sh
git commit -m "test(gateway): update smoke test for real per-entry verification"
```

---

### Task 9: Full verification pass

**Files:** none (verification only)

- [ ] **Step 1: Run the whole Rust workspace test suite**

Run: `cargo test --workspace`
Expected: all crates pass, including the new `xtask` unit tests (Tasks 1–3)
and the updated `riot-core` conference fixture test (Task 4).

- [ ] **Step 2: Run `cargo xtask validate-contracts`**

Run: `cargo run --locked --package xtask -- validate-contracts`
Expected: `validate-contracts: PASS` — confirms the new `riot-core`/`willow25`
dependency on `xtask` didn't leak the `conformance` feature or otherwise
widen the resolved feature graph in a way the contract check disallows.

- [ ] **Step 3: Run the full Python suite and the shell smoke test once more, end to end**

Run: `python3 -m unittest apps.gateway.tests.test_gateway -v && ./scripts/conference/gateway-smoke.sh`
Expected: both green.

- [ ] **Step 4: Manual visual check**

Run: `python3 apps/gateway/server.py --port 8080` (background it or use a
second terminal), then `curl -s http://127.0.0.1:8080/site/ | grep -E "Signature (verified|invalid)|entries signature-verified"`.
Expected: shows "2 of 2 entries signature-verified" and two "Signature
verified" badges (both fixture entries were legitimately signed by Task 2,
so no `signature_invalid` badge is expected in the checked-in fixture —
that state is only exercised by the tests in Tasks 3 and 7, deliberately,
via tampered bytes rather than shipped in real fixture data).

No commit for this task — it's pure verification of everything committed
in Tasks 1–8.
