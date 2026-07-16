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

#[derive(Debug)]
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
            record["willow_entry_bytes"]
                .as_str()
                .ok_or("willow_entry_bytes")?,
            "willow_entry_bytes",
        )?;
        let capability_bytes = hex_codec::decode(
            record["willow_capability_bytes"]
                .as_str()
                .ok_or("willow_capability_bytes")?,
            "willow_capability_bytes",
        )?;
        let signature: [u8; 64] = hex_codec::decode(
            record["signature"].as_str().ok_or("signature")?,
            "signature",
        )?
        .try_into()
        .map_err(|_| "signature must be exactly 64 bytes".to_string())?;
        index.insert(
            id,
            Proof {
                entry_bytes,
                capability_bytes,
                signature,
            },
        );
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
        &fs::read_to_string(&signed_path)
            .map_err(|e| format!("read {}: {e}", signed_path.display()))?,
    )
    .map_err(|e| format!("parse {}: {e}", signed_path.display()))?;
    let mut export: Value = serde_json::from_str(
        &fs::read_to_string(&export_path)
            .map_err(|e| format!("read {}: {e}", export_path.display()))?,
    )
    .map_err(|e| format!("parse {}: {e}", export_path.display()))?;

    let index = index_signed_records(&signed)?;

    // NO integrity-pass short-circuit. The conference verifier does NOT hard-error
    // on an invalid signature — it STAMPS `signature_invalid` per entry and keeps
    // going (verify_conference_export.rs:58-67). Mirror that exactly: a hard-error
    // integrity pass here would (a) diverge from the proven board behaviour and
    // (b) make the per-entry `else { signature_invalid }` arm below unreachable —
    // a phantom guard. The per-entry loop is the single, total verification path.
    let export_entries = export["entries"]
        .as_array()
        .cloned()
        .ok_or("public export: entries must be an array")?;
    let mut verified_count = 0usize;
    for (position, entry) in export_entries.iter().enumerate() {
        let proof = proof_for(&index, entry, position)?;
        let valid = verify_signed_entry(
            &proof.entry_bytes,
            &proof.capability_bytes,
            &proof.signature,
        )?;
        if valid {
            verified_count += 1;
        }
        let status = if valid {
            VERIFICATION_STATUS_VALID
        } else {
            VERIFICATION_STATUS_INVALID
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn binding_requires_a_matching_signed_record_for_every_public_entry() {
        // A 64-byte (128 hex char) signature so `index_signed_records` — which
        // enforces the exact signature length — accepts this hand-built record.
        let signature = "0".repeat(128);
        let signed = json!({ "records": [
            { "willow_entry_id": "aa", "willow_entry_bytes": "00",
              "willow_capability_bytes": "00", "signature": signature }
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
        let root =
            std::env::temp_dir().join(format!("riot-verify-nw-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        assert!(run(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn run_stamps_signature_invalid_when_a_bound_records_entry_bytes_are_tampered() {
        let root = std::env::temp_dir().join(format!("riot-verify-nw-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        copy_dir_recursive(
            &repo_root().join("fixtures/newswire"),
            &root.join("fixtures/newswire"),
        );

        // Corrupt the entry_bytes of the signed record that backs the FIRST public
        // entry, keeping its willow_entry_id so the public row still binds to it.
        let export_path = root.join("fixtures/newswire/gateway-space/public-export-v1.json");
        let signed_path = root.join("fixtures/newswire/signed-space-v1.json");
        let export: Value =
            serde_json::from_str(&fs::read_to_string(&export_path).unwrap()).unwrap();
        let target_id = export["entries"][0]["entry_id"]
            .as_str()
            .unwrap()
            .to_string();
        let mut signed: Value =
            serde_json::from_str(&fs::read_to_string(&signed_path).unwrap()).unwrap();
        for record in signed["records"].as_array_mut().unwrap() {
            if record["willow_entry_id"] == Value::from(target_id.clone()) {
                // Flip the first byte of the hex entry_bytes to break the signature.
                let bytes = record["willow_entry_bytes"].as_str().unwrap();
                let flipped = if bytes.starts_with('0') {
                    format!("1{}", &bytes[1..])
                } else {
                    format!("0{}", &bytes[1..])
                };
                record["willow_entry_bytes"] = Value::from(flipped);
            }
        }
        fs::write(
            &signed_path,
            serde_json::to_string_pretty(&signed).unwrap() + "\n",
        )
        .unwrap();

        run(&root).expect("verify still completes — it stamps, it does not hard-error");

        let stamped: Value =
            serde_json::from_str(&fs::read_to_string(&export_path).unwrap()).unwrap();
        assert_eq!(
            stamped["entries"][0]["verification_status"], VERIFICATION_STATUS_INVALID,
            "the tampered entry is stamped signature_invalid"
        );
        // The other entries still verify — stamping is per-entry, not all-or-nothing.
        assert_eq!(
            stamped["entries"][1]["verification_status"],
            VERIFICATION_STATUS_VALID
        );
        let _ = fs::remove_dir_all(&root);
    }
}
