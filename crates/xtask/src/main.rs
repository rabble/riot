//! Phase 0A build orchestration. `validate-contracts` structurally verifies
//! the frozen-environment contract: exact corrected dependency pins, frozen
//! fixture hashes, resource ceilings, and the unwind panic strategy the FFI
//! catch/quarantine contract depends on. Substring matching is not trusted
//! for anything a TOML/JSON parser can check.

mod hex_codec;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
use uniffi::{GenerateOptions, TargetLanguage};

const WILLOW25_PIN: &str = "=0.6.0-alpha.3";
const BAB_RS_PIN: &str = "=0.8.1";
const HIFITIME_PIN: &str = "=4.3.0";

/// Exact frozen ceiling values from the Revision 5 limits table. Presence
/// checks alone let a mutated ceiling slip through review.
const EXPECTED_CEILINGS: &[(&str, u64)] = &[
    ("artifact_bytes", 8_388_608),
    ("entries_per_bundle", 64),
    ("entry_bytes", 4_096),
    ("signature_bytes", 64),
    ("payload_bytes", 1_048_576),
    ("cbor_nesting", 16),
    ("map_entries", 128),
    ("decoded_cbor_nodes", 16_384),
    ("string_bytes", 65_536),
    ("path_components", 64),
    ("path_component_bytes", 256),
    ("path_total_bytes", 2_048),
    ("authorization_chain_depth", 16),
    ("authorization_bytes_per_entry", 65_536),
    ("authorization_bytes_per_bundle", 2_097_152),
    ("warning_records", 64),
    ("store_entries", 1_024),
    ("store_index_records", 1_024),
    ("store_encoded_entry_bytes", 8_388_608),
    ("durable_receipts", 256),
    ("open_stores_per_session", 1),
    ("open_previews_per_session", 1),
    ("retained_preview_input_bytes", 8_388_608),
    ("retained_preview_output_bytes", 2_097_152),
    ("transaction_snapshot_bytes", 16_777_216),
    ("inspection_target_seconds", 2),
    ("retained_store_budget_bytes", 16_777_216),
    ("namespace_views", 64),
    ("store_charge_entry_bytes", 512),
    ("store_charge_namespace_bytes", 256),
    ("store_charge_receipt_bytes", 256),
    ("store_charge_digest_reference_bytes", 32),
    ("entry_reference_cap", 1_024),
    ("plan_tombstone_bytes", 256),
    ("plans_per_preview", 64),
];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("validate-contracts") => {
            let root = workspace_root();
            let mut failures = validate_contents(&root);
            failures.extend(check_resolved_feature_graph(&root));
            if failures.is_empty() {
                println!("validate-contracts: PASS (structural + resolved feature graph)");
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
        Some("generate-bindings") => match generate_mobile_bindings(&workspace_root()) {
            Ok(out_dir) => {
                println!("generate-bindings: PASS ({})", out_dir.display());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("generate-bindings: FAIL: {error}");
                ExitCode::FAILURE
            }
        },
        Some(other) => {
            eprintln!("unknown xtask command: {other}");
            eprintln!("available: {}", available_commands().join(", "));
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <command>");
            eprintln!("available: {}", available_commands().join(", "));
            ExitCode::FAILURE
        }
    }
}

fn available_commands() -> &'static [&'static str] {
    &["validate-contracts", "generate-bindings"]
}

fn generate_mobile_bindings(root: &Path) -> Result<PathBuf, String> {
    let status = std::process::Command::new("cargo")
        .args(["build", "-p", "riot-ffi", "--lib", "--locked"])
        .current_dir(root)
        .status()
        .map_err(|error| format!("could not build riot-ffi: {error}"))?;
    if !status.success() {
        return Err("cargo build -p riot-ffi --lib --locked failed".into());
    }

    let library = root.join("target").join("debug").join(format!(
        "{}riot_ffi{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    if !library.is_file() {
        return Err(format!(
            "host riot-ffi library absent: {}",
            library.display()
        ));
    }

    let out_dir = root.join("build/generated/riot-ffi");
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)
            .map_err(|error| format!("could not clean {}: {error}", out_dir.display()))?;
    }
    std::fs::create_dir_all(&out_dir)
        .map_err(|error| format!("could not create {}: {error}", out_dir.display()))?;

    let source = Utf8PathBuf::from_path_buf(library)
        .map_err(|path| format!("non-UTF-8 library path: {}", path.display()))?;
    let bindgen_out = Utf8PathBuf::from_path_buf(out_dir.clone())
        .map_err(|path| format!("non-UTF-8 output path: {}", path.display()))?;
    uniffi::generate(GenerateOptions {
        languages: vec![TargetLanguage::Swift, TargetLanguage::Kotlin],
        source,
        out_dir: bindgen_out,
        config_override: None,
        format: false,
        crate_filter: None,
        metadata_no_deps: false,
    })
    .map_err(|error| format!("UniFFI generation failed: {error:#}"))?;
    validate_generated_bindings(&out_dir)?;

    Ok(out_dir)
}

fn validate_generated_bindings(out_dir: &Path) -> Result<(), String> {
    let required = [
        "riot_ffi.swift",
        "riot_ffiFFI.h",
        "riot_ffiFFI.modulemap",
        "uniffi/riot_ffi/riot_ffi.kt",
    ];
    for relative in required {
        let path = out_dir.join(relative);
        let metadata = std::fs::metadata(&path)
            .map_err(|error| format!("generated binding absent at {}: {error}", path.display()))?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(format!(
                "generated binding is not a non-empty file: {}",
                path.display()
            ));
        }
    }
    Ok(())
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
    check_crate_manifests(root, &mut failures);
    check_lockfile(root, &mut failures);
    check_fixture_manifest(root, &mut failures);
    check_schema(root, &mut failures);
    failures
}

/// Scans every crate manifest for dependency declarations that would widen
/// the frozen feature surface past the workspace pins — e.g. a crate-level
/// `willow25 = { ..., features = ["drop_format"] }` that the workspace-level
/// check alone would never see.
fn check_crate_manifests(root: &Path, failures: &mut Vec<String>) {
    let crates_dir = root.join("crates");
    let Ok(entries) = std::fs::read_dir(&crates_dir) else {
        return; // scaffolds without a crates/ tree are validated elsewhere
    };
    for entry in entries.flatten() {
        let manifest = entry.path().join("Cargo.toml");
        let Ok(raw) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let Ok(doc) = raw.parse::<toml::Table>() else {
            failures.push(format!("{}: not valid TOML", manifest.display()));
            continue;
        };
        let crate_name = entry.file_name().to_string_lossy().to_string();
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(deps) = doc.get(section).and_then(|d| d.as_table()) else {
                continue;
            };
            for (dep_name, spec) in deps {
                let features: Vec<&str> = spec
                    .as_table()
                    .and_then(|t| t.get("features"))
                    .and_then(|f| f.as_array())
                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                if dep_name == "willow25" && features.contains(&"drop_format") {
                    failures.push(format!(
                        "crates/{crate_name}/Cargo.toml: {section}.willow25 enables forbidden feature `drop_format`"
                    ));
                }
                if dep_name == "willow25" || dep_name == "bab_rs" {
                    // Crate-level version overrides escape the workspace pin.
                    if spec
                        .as_table()
                        .map(|t| t.contains_key("version"))
                        .unwrap_or(spec.is_str())
                    {
                        failures.push(format!(
                            "crates/{crate_name}/Cargo.toml: {section}.{dep_name} must use workspace = true, not a crate-level version"
                        ));
                    }
                }
            }
        }
    }
}

/// Inspects the RESOLVED feature graph of the release crate — the layer the
/// structural manifest checks cannot see. Runs `cargo tree` with the locked
/// graph and rejects forbidden features/crates in the riot-ffi closure.
pub fn check_resolved_feature_graph(root: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    let output = std::process::Command::new("cargo")
        .args(["tree", "-p", "riot-ffi", "-e", "features", "--locked"])
        .current_dir(root)
        .output();
    let Ok(output) = output else {
        failures.push("feature graph: cargo tree could not run".into());
        return failures;
    };
    if !output.status.success() {
        failures.push("feature graph: cargo tree --locked failed (lock drift?)".into());
        return failures;
    }
    let tree = String::from_utf8_lossy(&output.stdout);
    for (needle, why) in [
        (
            "willow25 feature \"drop_format\"",
            "drop_format is disabled in Phase 0A",
        ),
        ("openmls", "no group code in the Phase 0A graph"),
        (
            "riot-core feature \"conformance\"",
            "test/conformance injection APIs must not reach the release closure",
        ),
    ] {
        if tree.contains(needle) {
            failures.push(format!("feature graph: found `{needle}` — {why}"));
        }
    }
    for line in tree.lines() {
        if line.contains("bab_rs v0.") && !line.contains("bab_rs v0.8.1") {
            failures.push(format!(
                "feature graph: wrong bab_rs version in closure: {line}"
            ));
        }
    }
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

    // Ceilings are exact frozen values from the Revision 5 limits table.
    // Presence alone is not enough: a mutated value is a contract violation.
    let ceilings = &doc["ceilings"];
    for (key, expected) in EXPECTED_CEILINGS {
        match ceilings.get(*key).and_then(|v| v.as_u64()) {
            Some(actual) if actual == *expected => {}
            Some(actual) => failures.push(format!(
                "fixtures/manifest.json: ceilings.{key} must be exactly {expected} (found {actual})"
            )),
            None => failures.push(format!(
                "fixtures/manifest.json: ceilings.{key} absent or not an integer (must be {expected})"
            )),
        }
    }
    if ceilings
        .get("expansion_ratio")
        .and_then(|v| v.as_str())
        .is_none_or(|s| !s.starts_with("1:1"))
    {
        failures.push("fixtures/manifest.json: ceilings.expansion_ratio must state 1:1".into());
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

    #[test]
    fn exposes_locked_binding_generation_command() {
        assert!(available_commands().contains(&"generate-bindings"));
    }

    #[test]
    fn generated_binding_contract_requires_both_native_languages() {
        let dir = temp_dir("binding-contract");
        std::fs::create_dir_all(dir.join("uniffi/riot_ffi")).unwrap();
        std::fs::write(dir.join("riot_ffi.swift"), "// swift").unwrap();
        std::fs::write(dir.join("riot_ffiFFI.h"), "// header").unwrap();
        std::fs::write(dir.join("riot_ffiFFI.modulemap"), "// module").unwrap();

        assert!(validate_generated_bindings(&dir).is_err());
        std::fs::write(
            dir.join("uniffi/riot_ffi/riot_ffi.kt"),
            "// kotlin bindings",
        )
        .unwrap();
        assert_eq!(validate_generated_bindings(&dir), Ok(()));
    }

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
        let mut ceilings: String = EXPECTED_CEILINGS
            .iter()
            .map(|(k, v)| format!("\"{k}\": {v}"))
            .collect::<Vec<_>>()
            .join(",");
        ceilings.push_str(",\"expansion_ratio\": \"1:1 - compression forbidden\"");
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
    fn rejects_mutated_ceiling_value() {
        // The exact regression the independent review demonstrated:
        // artifact_bytes changed from 8 MiB to 1 must fail, not pass.
        let dir = temp_dir("mutated-ceiling");
        good_scaffold(&dir);
        let lock_hash = sha256_hex(good_lock().as_bytes());
        let vectors_hash = sha256_hex(b"{\"vectors\":[]}");
        let mutated = manifest_with(&lock_hash, &vectors_hash)
            .replace("\"artifact_bytes\": 8388608", "\"artifact_bytes\": 1");
        std::fs::write(dir.join("fixtures/manifest.json"), mutated).unwrap();
        let failures = validate_contents(&dir);
        assert!(
            failures
                .iter()
                .any(|f| f.contains("artifact_bytes") && f.contains("exactly 8388608")),
            "mutated ceiling must be named exactly: {failures:?}"
        );
    }

    #[test]
    fn rejects_crate_level_drop_format_enablement() {
        // The second accepted regression: a crate-level dependency
        // declaration enabling drop_format that the workspace check misses.
        let dir = temp_dir("crate-dropfmt");
        good_scaffold(&dir);
        std::fs::create_dir_all(dir.join("crates/riot-core")).unwrap();
        std::fs::write(
            dir.join("crates/riot-core/Cargo.toml"),
            r#"[package]
name = "riot-core"
version = "0.1.0"
[dependencies]
willow25 = { workspace = true, features = ["drop_format"] }
"#,
        )
        .unwrap();
        let failures = validate_contents(&dir);
        assert!(
            failures
                .iter()
                .any(|f| f.contains("riot-core") && f.contains("drop_format")),
            "crate-level drop_format must be named: {failures:?}"
        );
    }

    #[test]
    fn rejects_crate_level_version_override() {
        let dir = temp_dir("crate-version-override");
        good_scaffold(&dir);
        std::fs::create_dir_all(dir.join("crates/riot-core")).unwrap();
        std::fs::write(
            dir.join("crates/riot-core/Cargo.toml"),
            r#"[package]
name = "riot-core"
version = "0.1.0"
[dependencies]
willow25 = { version = "=0.5.0" }
"#,
        )
        .unwrap();
        let failures = validate_contents(&dir);
        assert!(
            failures.iter().any(|f| f.contains("workspace = true")),
            "crate-level version override must be named: {failures:?}"
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
