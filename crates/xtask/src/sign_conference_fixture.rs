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
    create_signed_alert, entry_id, generate_communal_author,
    generate_communal_author_for_namespace, AlertDraft,
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

    let founder =
        generate_communal_author().map_err(|error| format!("generate founding author: {error}"))?;
    let namespace_bytes = *founder.namespace_id().as_bytes();
    let second = generate_communal_author_for_namespace(namespace_bytes)
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
