//! Phase 0A build orchestration. `validate-contracts` structurally verifies
//! the frozen-environment contract: exact corrected dependency pins, frozen
//! fixture hashes, resource ceilings, and the unwind panic strategy the FFI
//! catch/quarantine contract depends on. Substring matching is not trusted
//! for anything a TOML/JSON parser can check.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::{Digest, Sha256};

const WILLOW25_PIN: &str = "=0.6.0-alpha.3";
const BAB_RS_PIN: &str = "=0.8.1";
const HIFITIME_PIN: &str = "=4.3.0";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("validate-contracts") => {
            let failures = validate_contents(&workspace_root());
            if failures.is_empty() {
                println!("validate-contracts: PASS");
                ExitCode::SUCCESS
            } else {
                eprintln!("validate-contracts: FAIL");
                for failure in &failures {
                    eprintln!("  {failure}");
                }
                eprintln!("{} contract violation(s)", failures.len());
                ExitCode::FAILURE
            }
        }
        Some(other) => {
            eprintln!("unknown xtask command: {other}");
            eprintln!("available: validate-contracts");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <command>");
            eprintln!("available: validate-contracts");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Every contract violation as one human-readable line. Empty means PASS.
pub fn validate_contents(root: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    check_workspace_manifest(root, &mut failures);
    check_lockfile(root, &mut failures);
    check_fixture_manifest(root, &mut failures);
    check_schema(root, &mut failures);
    failures
}

fn check_workspace_manifest(root: &Path, failures: &mut Vec<String>) {
    let path = root.join("Cargo.toml");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        failures.push("Cargo.toml: file absent".into());
        return;
    };
    let doc = match raw.parse::<toml::Table>() {
        Ok(doc) => doc,
        Err(e) => {
            failures.push(format!("Cargo.toml: not valid TOML ({e})"));
            return;
        }
    };

    let deps = doc
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .cloned()
        .unwrap_or(toml::Value::Table(Default::default()));

    check_dep(
        &deps,
        "willow25",
        WILLOW25_PIN,
        Some((false, &["std"], &["drop_format"])),
        failures,
    );
    check_dep(
        &deps,
        "bab_rs",
        BAB_RS_PIN,
        Some((false, &["william3"], &[])),
        failures,
    );
    check_dep(&deps, "hifitime", HIFITIME_PIN, None, failures);

    let panic_strategy = doc
        .get("profile")
        .and_then(|p| p.get("release"))
        .and_then(|r| r.get("panic"))
        .and_then(|v| v.as_str());
    if panic_strategy != Some("unwind") {
        failures.push(format!(
            "Cargo.toml: profile.release.panic must be \"unwind\" for the FFI catch/quarantine contract (found {panic_strategy:?})"
        ));
    }
}

/// requirements: (default_features_must_be_false, required_features, forbidden_features)
fn check_dep(
    deps: &toml::Value,
    name: &str,
    pin: &str,
    requirements: Option<(bool, &[&str], &[&str])>,
    failures: &mut Vec<String>,
) {
    let Some(dep) = deps.get(name) else {
        failures.push(format!(
            "Cargo.toml: workspace dependency `{name}` absent (must be {pin})"
        ));
        return;
    };
    let version = match dep {
        toml::Value::String(s) => Some(s.as_str()),
        toml::Value::Table(t) => t.get("version").and_then(|v| v.as_str()),
        _ => None,
    };
    if version != Some(pin) {
        failures.push(format!(
            "Cargo.toml: `{name}` must be pinned {pin} (found {version:?})"
        ));
    }
    if let Some((default_off, required, forbidden)) = requirements {
        let table = dep.as_table();
        let default_features = table
            .and_then(|t| t.get("default-features"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if default_off && default_features {
            failures.push(format!(
                "Cargo.toml: `{name}` must set default-features = false"
            ));
        }
        let features: Vec<String> = table
            .and_then(|t| t.get("features"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        for feature in required {
            if !features.iter().any(|f| f == feature) {
                failures.push(format!(
                    "Cargo.toml: `{name}` must enable feature `{feature}`"
                ));
            }
        }
        for feature in forbidden {
            if features.iter().any(|f| f == feature) {
                failures.push(format!(
                    "Cargo.toml: `{name}` must not enable feature `{feature}`"
                ));
            }
        }
    }
}

fn check_lockfile(root: &Path, failures: &mut Vec<String>) {
    let path = root.join("Cargo.lock");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        failures.push("Cargo.lock: file absent".into());
        return;
    };
    let Ok(doc) = raw.parse::<toml::Table>() else {
        failures.push("Cargo.lock: not valid TOML".into());
        return;
    };
    let packages = doc
        .get("package")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    let versions_of = |name: &str| -> Vec<String> {
        packages
            .iter()
            .filter(|p| p.get("name").and_then(|n| n.as_str()) == Some(name))
            .filter_map(|p| {
                p.get("version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect()
    };

    let willow = versions_of("willow25");
    if willow != vec!["0.6.0-alpha.3".to_string()] {
        failures.push(format!(
            "Cargo.lock: willow25 must resolve exactly [0.6.0-alpha.3] (found {willow:?})"
        ));
    }
    let bab = versions_of("bab_rs");
    if bab != vec!["0.8.1".to_string()] {
        failures.push(format!(
            "Cargo.lock: bab_rs must resolve exactly [0.8.1] — pre-0.8 versions compute incorrect WILLIAM3 digests (found {bab:?})"
        ));
    }
    if !versions_of("openmls").is_empty() {
        failures.push("Cargo.lock: openmls must not be in the Phase 0A graph".into());
    }
}

fn check_fixture_manifest(root: &Path, failures: &mut Vec<String>) {
    let path = root.join("fixtures/manifest.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        failures.push("fixtures/manifest.json: file absent".into());
        return;
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&raw) else {
        failures.push("fixtures/manifest.json: not valid JSON".into());
        return;
    };
    let env = &doc["environment"];

    // The recorded lock hash must match the actual lockfile bytes.
    match (
        env["cargo_lock_sha256"].as_str(),
        std::fs::read(root.join("Cargo.lock")),
    ) {
        (Some(recorded), Ok(actual_bytes)) => {
            let actual = sha256_hex(&actual_bytes);
            if recorded != actual {
                failures.push(format!(
                    "fixtures/manifest.json: cargo_lock_sha256 mismatch (recorded {recorded}, actual {actual})"
                ));
            }
        }
        _ => failures.push(
            "fixtures/manifest.json: cargo_lock_sha256 missing or Cargo.lock unreadable".into(),
        ),
    }

    // The WILLIAM3 vector fixture must be frozen by hash.
    match (
        env["william3_vectors_sha256"].as_str(),
        std::fs::read(root.join("fixtures/willow/william3-vectors.json")),
    ) {
        (Some(recorded), Ok(actual_bytes)) if !recorded.is_empty() => {
            let actual = sha256_hex(&actual_bytes);
            if recorded != actual {
                failures.push(format!(
                    "fixtures/manifest.json: william3_vectors_sha256 mismatch (recorded {recorded}, actual {actual})"
                ));
            }
        }
        _ => failures.push(
            "fixtures/manifest.json: william3_vectors_sha256 missing/empty or vectors file unreadable".into(),
        ),
    }

    let ceilings = &doc["ceilings"];
    const REQUIRED_CEILINGS: &[&str] = &[
        "artifact_bytes",
        "entries_per_bundle",
        "payload_bytes",
        "cbor_nesting",
        "map_entries",
        "decoded_cbor_nodes",
        "string_bytes",
        "path_components",
        "path_component_bytes",
        "path_total_bytes",
        "authorization_chain_depth",
        "authorization_bytes_per_entry",
        "authorization_bytes_per_bundle",
        "warning_records",
        "store_entries",
        "store_index_records",
        "store_encoded_entry_bytes",
        "durable_receipts",
        "open_stores_per_session",
        "open_previews_per_session",
        "retained_preview_input_bytes",
        "retained_preview_output_bytes",
        "transaction_snapshot_bytes",
        "inspection_target_seconds",
        // Reopened-review additions: store charge model and plan limits.
        "retained_store_budget_bytes",
        "namespace_views",
        "store_charge_entry_bytes",
        "store_charge_namespace_bytes",
        "store_charge_receipt_bytes",
        "store_charge_digest_reference_bytes",
        "entry_reference_cap",
        "plan_tombstone_bytes",
        "plans_per_preview",
    ];
    for key in REQUIRED_CEILINGS {
        if ceilings.get(*key).map(|v| v.is_u64() || v.is_string()) != Some(true) {
            failures.push(format!("fixtures/manifest.json: ceilings.{key} absent"));
        }
    }

    for key in ["objects", "willow", "imports"] {
        if doc["fixture_ownership"].get(key).is_none() {
            failures.push(format!(
                "fixtures/manifest.json: fixture_ownership.{key} absent"
            ));
        }
    }

    const REPORT_FIELDS: &[&str] = &[
        "status",
        "owning_work_unit",
        "commands",
        "environment",
        "evidence_paths",
        "hashes",
        "elapsed_agent_hours",
        "next_action",
    ];
    let report_fields: Vec<String> = doc["report_fields"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    for field in REPORT_FIELDS {
        if !report_fields.iter().any(|f| f == field) {
            failures.push(format!(
                "fixtures/manifest.json: report_fields missing `{field}`"
            ));
        }
    }
}

fn check_schema(root: &Path, failures: &mut Vec<String>) {
    match std::fs::read_to_string(root.join("schemas/alert.cddl")) {
        Ok(contents) if contents.contains("org.riot.alert/1") => {}
        Ok(_) => failures.push("schemas/alert.cddl: missing schema id org.riot.alert/1".into()),
        Err(_) => failures.push("schemas/alert.cddl: file absent".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scaffold(dir: &Path, workspace_toml: &str, lock: &str, manifest: &str) {
        std::fs::create_dir_all(dir.join("fixtures/willow")).unwrap();
        std::fs::create_dir_all(dir.join("schemas")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), workspace_toml).unwrap();
        std::fs::write(dir.join("Cargo.lock"), lock).unwrap();
        std::fs::write(dir.join("fixtures/manifest.json"), manifest).unwrap();
        std::fs::write(
            dir.join("fixtures/willow/william3-vectors.json"),
            b"{\"vectors\":[]}",
        )
        .unwrap();
        std::fs::write(
            dir.join("schemas/alert.cddl"),
            "alert = \"org.riot.alert/1\"",
        )
        .unwrap();
    }

    fn good_workspace_toml() -> String {
        format!(
            r#"[workspace]
[workspace.dependencies]
willow25 = {{ version = "{WILLOW25_PIN}", default-features = false, features = ["std"] }}
bab_rs = {{ version = "{BAB_RS_PIN}", default-features = false, features = ["william3"] }}
hifitime = "{HIFITIME_PIN}"
[profile.release]
panic = "unwind"
"#
        )
    }

    fn good_lock() -> &'static str {
        r#"version = 4
[[package]]
name = "willow25"
version = "0.6.0-alpha.3"
[[package]]
name = "bab_rs"
version = "0.8.1"
"#
    }

    fn manifest_with(lock_hash: &str, vectors_hash: &str) -> String {
        let ceilings: String = [
            "artifact_bytes",
            "entries_per_bundle",
            "payload_bytes",
            "cbor_nesting",
            "map_entries",
            "decoded_cbor_nodes",
            "string_bytes",
            "path_components",
            "path_component_bytes",
            "path_total_bytes",
            "authorization_chain_depth",
            "authorization_bytes_per_entry",
            "authorization_bytes_per_bundle",
            "warning_records",
            "store_entries",
            "store_index_records",
            "store_encoded_entry_bytes",
            "durable_receipts",
            "open_stores_per_session",
            "open_previews_per_session",
            "retained_preview_input_bytes",
            "retained_preview_output_bytes",
            "transaction_snapshot_bytes",
            "inspection_target_seconds",
            "retained_store_budget_bytes",
            "namespace_views",
            "store_charge_entry_bytes",
            "store_charge_namespace_bytes",
            "store_charge_receipt_bytes",
            "store_charge_digest_reference_bytes",
            "entry_reference_cap",
            "plan_tombstone_bytes",
            "plans_per_preview",
        ]
        .iter()
        .map(|k| format!("\"{k}\": 1"))
        .collect::<Vec<_>>()
        .join(",");
        format!(
            r#"{{
  "environment": {{ "cargo_lock_sha256": "{lock_hash}", "william3_vectors_sha256": "{vectors_hash}" }},
  "ceilings": {{ {ceilings} }},
  "fixture_ownership": {{ "objects": "WU1", "willow": "WU1", "imports": "WU2" }},
  "report_fields": ["status","owning_work_unit","commands","environment","evidence_paths","hashes","elapsed_agent_hours","next_action"]
}}"#
        )
    }

    fn good_scaffold(dir: &Path) {
        let lock = good_lock();
        let vectors = b"{\"vectors\":[]}";
        scaffold(
            dir,
            &good_workspace_toml(),
            lock,
            &manifest_with(&sha256_hex(lock.as_bytes()), &sha256_hex(vectors)),
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("riot-xtask-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn accepts_the_corrected_contract() {
        let dir = temp_dir("good");
        good_scaffold(&dir);
        let failures = validate_contents(&dir);
        assert!(failures.is_empty(), "unexpected failures: {failures:?}");
    }

    #[test]
    fn rejects_obsolete_willow_pin() {
        let dir = temp_dir("old-willow");
        good_scaffold(&dir);
        std::fs::write(
            dir.join("Cargo.toml"),
            good_workspace_toml().replace(WILLOW25_PIN, "=0.5.0"),
        )
        .unwrap();
        let failures = validate_contents(&dir);
        assert!(
            failures
                .iter()
                .any(|f| f.contains("willow25") && f.contains(WILLOW25_PIN)),
            "must name the willow25 pin violation: {failures:?}"
        );
    }

    #[test]
    fn rejects_obsolete_bab_rs_in_lock() {
        let dir = temp_dir("old-bab");
        good_scaffold(&dir);
        let bad_lock = good_lock().replace("0.8.1", "0.6.3");
        std::fs::write(dir.join("Cargo.lock"), &bad_lock).unwrap();
        // Keep the recorded hash consistent so only the version check fires.
        std::fs::write(
            dir.join("fixtures/manifest.json"),
            manifest_with(
                &sha256_hex(bad_lock.as_bytes()),
                &sha256_hex(b"{\"vectors\":[]}"),
            ),
        )
        .unwrap();
        let failures = validate_contents(&dir);
        assert!(
            failures
                .iter()
                .any(|f| f.contains("bab_rs") && f.contains("incorrect WILLIAM3")),
            "must name the bab_rs basis violation: {failures:?}"
        );
    }

    #[test]
    fn rejects_stale_lock_hash() {
        let dir = temp_dir("stale-hash");
        good_scaffold(&dir);
        std::fs::write(
            dir.join("fixtures/manifest.json"),
            manifest_with(&"0".repeat(64), &sha256_hex(b"{\"vectors\":[]}")),
        )
        .unwrap();
        let failures = validate_contents(&dir);
        assert!(
            failures
                .iter()
                .any(|f| f.contains("cargo_lock_sha256 mismatch")),
            "must name the lock hash mismatch: {failures:?}"
        );
    }

    #[test]
    fn rejects_abort_panic_strategy() {
        let dir = temp_dir("abort");
        good_scaffold(&dir);
        std::fs::write(
            dir.join("Cargo.toml"),
            good_workspace_toml().replace("panic = \"unwind\"", "panic = \"abort\""),
        )
        .unwrap();
        let failures = validate_contents(&dir);
        assert!(
            failures
                .iter()
                .any(|f| f.contains("panic") && f.contains("unwind")),
            "must name the panic strategy violation: {failures:?}"
        );
    }

    #[test]
    fn rejects_drop_format_feature() {
        let dir = temp_dir("dropfmt");
        good_scaffold(&dir);
        std::fs::write(
            dir.join("Cargo.toml"),
            good_workspace_toml().replace("[\"std\"]", "[\"std\", \"drop_format\"]"),
        )
        .unwrap();
        let failures = validate_contents(&dir);
        assert!(
            failures.iter().any(|f| f.contains("drop_format")),
            "must name the drop_format violation: {failures:?}"
        );
    }

    #[test]
    fn rejects_missing_vector_hash_and_new_ceilings() {
        let dir = temp_dir("missing-bits");
        good_scaffold(&dir);
        let lock_hash = sha256_hex(good_lock().as_bytes());
        std::fs::write(
            dir.join("fixtures/manifest.json"),
            format!(
                r#"{{
  "environment": {{ "cargo_lock_sha256": "{lock_hash}" }},
  "ceilings": {{ "artifact_bytes": 1 }},
  "fixture_ownership": {{ "objects": "x", "willow": "x", "imports": "x" }},
  "report_fields": []
}}"#
            ),
        )
        .unwrap();
        let failures = validate_contents(&dir);
        assert!(failures
            .iter()
            .any(|f| f.contains("william3_vectors_sha256")));
        assert!(failures
            .iter()
            .any(|f| f.contains("retained_store_budget_bytes")));
        assert!(failures.iter().any(|f| f.contains("plans_per_preview")));
        assert!(failures.iter().any(|f| f.contains("report_fields missing")));
    }
}
