//! Verifies each conference fixture entry's real signature and writes the
//! public, proof-free per-entry verification_status into the gateway export.
//! No signature, capability, or entry bytes are copied into the public file
//! — only the boolean `verify_entry()` result, matching the existing
//! `_FORBIDDEN_FIELD_PARTS` boundary in `apps/gateway/riot_gateway.py`,
//! which already refuses any public field whose name contains
//! "capability", "secret", "receipt", etc.

use std::fs;
use std::path::Path;

use riot_core::willow::{
    decode_capability_canonic, decode_entry_canonic, verify_entry, AuthorisationToken,
};
use serde_json::{json, Value};
use willow25::prelude::SubspaceSignature;

use crate::hex_codec;

pub const VERIFICATION_STATUS_VALID: &str = "signature_verified";
pub const VERIFICATION_STATUS_INVALID: &str = "signature_invalid";
/// The export shape this tool produces. Verification moved from a single
/// document-level placeholder to a real per-entry `verification_status`
/// (see below), so this tool owns and unconditionally stamps the export's
/// `schema` field to reflect that shape change — the same way it already
/// owns and stamps each entry's `verification_status`.
pub const EXPORT_SCHEMA: &str = "riot-public-gateway-export/2";

/// Confirms that a fixture entry and an export entry at the same array index
/// actually describe the same Willow entry, so that positional pairing alone
/// is never trusted to bind a proof to the wrong piece of public data.
/// Pure and file-I/O-free, so it's directly unit-testable (see tests below).
pub fn check_entry_identity(
    fixture_entry: &Value,
    export_entry: &Value,
    index: usize,
) -> Result<(), String> {
    let fixture_entry_id = fixture_entry["willow_entry_id"]
        .as_str()
        .ok_or("incident entry: willow_entry_id must be a string")?;
    let export_entry_id = export_entry["entry_id"]
        .as_str()
        .ok_or("public export entry: entry_id must be a string")?;
    if fixture_entry_id != export_entry_id {
        return Err(format!(
            "entry identity mismatch at index {index}: fixture willow_entry_id {fixture_entry_id} does not match export entry_id {export_entry_id}"
        ));
    }
    Ok(())
}

/// Pure verification core, independent of file I/O, so it's directly
/// unit-testable with hand-built byte inputs (see the tests below).
pub fn verify_signed_entry(
    entry_bytes: &[u8],
    capability_bytes: &[u8],
    signature: &[u8; 64],
) -> Result<bool, String> {
    let entry =
        decode_entry_canonic(entry_bytes).map_err(|error| format!("decode entry: {error}"))?;
    let capability = decode_capability_canonic(capability_bytes)
        .map_err(|error| format!("decode capability: {error}"))?;
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
        // Positional pairing alone is not enough to bind a proof to the right
        // public entry: confirm the fixture's willow_entry_id actually matches
        // the export entry's entry_id at this index before trusting the index.
        check_entry_identity(entry, &export_entries[index], index)?;

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
    // Drop the stale document-level placeholder status: verification now
    // lives per-entry (set above), not as a single top-level field.
    if let Some(map) = export.as_object_mut() {
        map.remove("verification_status");
    }
    // Unconditionally stamp the export's schema version: this tool is the
    // sole producer of this export shape, so it owns this field the same
    // way it owns each entry's verification_status above.
    export["schema"] = json!(EXPORT_SCHEMA);

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
        let valid = verify_signed_entry(
            &signed.entry_bytes,
            &signed.capability_bytes,
            &signed.signature,
        )
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
        let valid =
            verify_signed_entry(&tampered_entry, &signed.capability_bytes, &signed.signature)
                .unwrap();
        assert!(!valid);
    }

    #[test]
    fn entry_identity_check_rejects_reordered_entries() {
        let fixture_entry = json!({ "willow_entry_id": "aaa111" });
        let export_entry_matching = json!({ "entry_id": "aaa111" });
        let export_entry_mismatched = json!({ "entry_id": "bbb222" });

        assert!(check_entry_identity(&fixture_entry, &export_entry_matching, 0).is_ok());

        let error = check_entry_identity(&fixture_entry, &export_entry_mismatched, 3)
            .expect_err("mismatched entry_id must be rejected");
        assert!(error.contains("index 3"));
        assert!(error.contains("aaa111"));
        assert!(error.contains("bbb222"));
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
