//! Phase 0A build orchestration. `validate-contracts` structurally verifies
//! the frozen-environment contract: exact corrected dependency pins, frozen
//! fixture hashes, resource ceilings, and the unwind panic strategy the FFI
//! catch/quarantine contract depends on. Substring matching is not trusted
//! for anything a TOML/JSON parser can check.

mod export_newswire;
mod hex_codec;
mod sign_conference_fixture;
mod verify_conference_export;
mod verify_newswire_export;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitCode, ExitStatus, Output};

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
    let mut runner = OsCommandRunner;
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    main_with_manifest_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &std::env::args().skip(1).collect::<Vec<_>>(),
        &mut runner,
        &mut stdout,
        &mut stderr,
    )
}

fn main_with_manifest_dir(
    manifest_dir: &Path,
    args: &[String],
    command_runner: &mut dyn CommandRunner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    match workspace_root_from(manifest_dir) {
        Ok(root) => run(&root, args, command_runner, out, err),
        Err(error) => {
            let _ = writeln!(err, "xtask: {error}").is_ok();
            ExitCode::FAILURE
        }
    }
}

trait CommandRunner {
    fn output(&mut self, program: &str, args: &[&str], root: &Path) -> io::Result<Output>;
    fn status(&mut self, program: &str, args: &[&str], root: &Path) -> io::Result<ExitStatus>;
}

trait BindingGenerator {
    fn generate(&mut self, source: Utf8PathBuf, out_dir: Utf8PathBuf) -> Result<(), String>;
}

struct UniFfiBindingGenerator;

impl BindingGenerator for UniFfiBindingGenerator {
    fn generate(&mut self, source: Utf8PathBuf, out_dir: Utf8PathBuf) -> Result<(), String> {
        uniffi::generate(GenerateOptions {
            languages: vec![TargetLanguage::Swift, TargetLanguage::Kotlin],
            source,
            out_dir,
            config_override: None,
            format: false,
            crate_filter: None,
            metadata_no_deps: false,
        })
        .map_err(|error| format!("UniFFI generation failed: {error:#}"))
    }
}

struct OsCommandRunner;

impl CommandRunner for OsCommandRunner {
    fn output(&mut self, program: &str, args: &[&str], root: &Path) -> io::Result<Output> {
        std::process::Command::new(program)
            .args(args)
            .current_dir(root)
            .output()
    }

    fn status(&mut self, program: &str, args: &[&str], root: &Path) -> io::Result<ExitStatus> {
        std::process::Command::new(program)
            .args(args)
            .current_dir(root)
            .status()
    }
}

fn run(
    root: &Path,
    args: &[String],
    command_runner: &mut dyn CommandRunner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    run_with(
        root,
        args,
        command_runner,
        &mut UniFfiBindingGenerator,
        out,
        err,
    )
}

fn run_with(
    root: &Path,
    args: &[String],
    command_runner: &mut dyn CommandRunner,
    binding_generator: &mut dyn BindingGenerator,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("validate-contracts") => {
            let mut failures = validate_contents(root);
            failures.extend(check_resolved_feature_graph_with(root, command_runner));
            if failures.is_empty() {
                if writeln!(
                    out,
                    "validate-contracts: PASS (structural + resolved feature graph)"
                )
                .is_ok()
                {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            } else {
                let mut wrote_all = writeln!(err, "validate-contracts: FAIL").is_ok();
                for failure in &failures {
                    wrote_all &= writeln!(err, "  {failure}").is_ok();
                }
                wrote_all &= writeln!(err, "{} contract violation(s)", failures.len()).is_ok();
                let _ = wrote_all;
                ExitCode::FAILURE
            }
        }
        Some("generate-bindings") => {
            match generate_mobile_bindings_with(root, command_runner, binding_generator) {
                Ok(out_dir) => {
                    if writeln!(out, "generate-bindings: PASS ({})", out_dir.display()).is_ok() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(error) => {
                    let _ = writeln!(err, "generate-bindings: FAIL: {error}").is_ok();
                    ExitCode::FAILURE
                }
            }
        }
        Some("sign-conference-fixture") => match sign_conference_fixture::run(root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("sign-conference-fixture: FAIL: {error}");
                ExitCode::FAILURE
            }
        },
        Some("verify-conference-export") => match verify_conference_export::run(root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("verify-conference-export: FAIL: {error}");
                ExitCode::FAILURE
            }
        },
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
        Some(other) => {
            let _ = writeln!(err, "unknown xtask command: {other}").is_ok();
            let _ = writeln!(err, "available: {}", available_commands().join(", ")).is_ok();
            ExitCode::FAILURE
        }
        None => {
            let _ = writeln!(err, "usage: cargo xtask <command>").is_ok();
            let _ = writeln!(err, "available: {}", available_commands().join(", ")).is_ok();
            ExitCode::FAILURE
        }
    }
}

fn available_commands() -> &'static [&'static str] {
    &[
        "validate-contracts",
        "generate-bindings",
        "sign-conference-fixture",
        "verify-conference-export",
        "export-newswire",
        "verify-newswire-export",
    ]
}

fn generate_mobile_bindings_with(
    root: &Path,
    command_runner: &mut dyn CommandRunner,
    binding_generator: &mut dyn BindingGenerator,
) -> Result<PathBuf, String> {
    let status = command_runner
        .status(
            "cargo",
            &["build", "-p", "riot-ffi", "--lib", "--locked"],
            root,
        )
        .map_err(|error| format!("could not build riot-ffi: {error}"))?;
    if !status.success() {
        return Err("cargo build -p riot-ffi --lib --locked failed".into());
    }

    let root_utf8 = utf8_path(root.to_path_buf(), "workspace")?;
    let library = root_utf8.join("target").join("debug").join(format!(
        "{}riot_ffi{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    if !library.is_file() {
        return Err(format!("host riot-ffi library absent: {library}"));
    }

    let out_dir = root_utf8.join("build/generated/riot-ffi");
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)
            .map_err(|error| format!("could not clean {out_dir}: {error}"))?;
    }
    std::fs::create_dir_all(&out_dir)
        .map_err(|error| format!("could not create {out_dir}: {error}"))?;

    binding_generator.generate(library, out_dir.clone())?;
    validate_generated_bindings(out_dir.as_std_path())?;

    Ok(out_dir.into_std_path_buf())
}

fn utf8_path(path: PathBuf, label: &str) -> Result<Utf8PathBuf, String> {
    Utf8PathBuf::from_path_buf(path)
        .map_err(|path| format!("non-UTF-8 {label} path: {}", path.display()))
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

fn workspace_root_from(manifest_dir: &Path) -> Result<PathBuf, String> {
    manifest_dir
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "could not discover workspace root from {}",
                manifest_dir.display()
            )
        })
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
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
    check_resolved_feature_graph_with(root, &mut OsCommandRunner)
}

fn check_resolved_feature_graph_with(
    root: &Path,
    command_runner: &mut dyn CommandRunner,
) -> Vec<String> {
    let mut failures = Vec::new();
    let output = command_runner.output(
        "cargo",
        &["tree", "-p", "riot-ffi", "-e", "features", "--locked"],
        root,
    );
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

    // The Meadowcap capability vector fixture must be frozen by hash.
    match (
        env["meadowcap_vectors_sha256"].as_str(),
        std::fs::read(root.join("fixtures/willow/meadowcap-vectors.json")),
    ) {
        (Some(recorded), Ok(actual_bytes)) if !recorded.is_empty() => {
            let actual = sha256_hex(&actual_bytes);
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

    // The governance record vector fixture must be frozen by hash.
    match (
        env["governance_vectors_sha256"].as_str(),
        std::fs::read(root.join("fixtures/governance/governance-vectors.json")),
    ) {
        (Some(recorded), Ok(actual_bytes)) if !recorded.is_empty() => {
            let actual = sha256_hex(&actual_bytes);
            if recorded != actual {
                failures.push(format!(
                    "fixtures/manifest.json: governance_vectors_sha256 mismatch (recorded {recorded}, actual {actual})"
                ));
            }
        }
        _ => failures.push(
            "fixtures/manifest.json: governance_vectors_sha256 missing/empty or vectors file unreadable".into(),
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
    use std::cell::RefCell;
    use std::io;
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    struct ScriptedBindingGenerator {
        result: Result<(), String>,
        write_required: bool,
        calls: Vec<(Utf8PathBuf, Utf8PathBuf)>,
    }

    impl ScriptedBindingGenerator {
        fn new(result: Result<(), String>, write_required: bool) -> Self {
            Self {
                result,
                write_required,
                calls: Vec::new(),
            }
        }
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"))
        }
    }

    impl BindingGenerator for ScriptedBindingGenerator {
        fn generate(&mut self, source: Utf8PathBuf, out_dir: Utf8PathBuf) -> Result<(), String> {
            self.calls.push((source, out_dir.clone()));
            self.result.clone()?;
            if self.write_required {
                for relative in [
                    "riot_ffi.swift",
                    "riot_ffiFFI.h",
                    "riot_ffiFFI.modulemap",
                    "uniffi/riot_ffi/riot_ffi.kt",
                ] {
                    let path = out_dir.join(relative);
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, b"generated").unwrap();
                }
            }
            Ok(())
        }
    }

    struct ScriptedRunner {
        output: Option<io::Result<Output>>,
        status: Option<io::Result<ExitStatus>>,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum RecordedCommand {
        Output {
            program: String,
            args: Vec<String>,
            root: PathBuf,
        },
        Status {
            program: String,
            args: Vec<String>,
            root: PathBuf,
        },
    }

    thread_local! {
        static SCRIPTED_CALLS: RefCell<Vec<RecordedCommand>> = const { RefCell::new(Vec::new()) };
    }

    impl ScriptedRunner {
        fn with_status(status: io::Result<ExitStatus>) -> Self {
            SCRIPTED_CALLS.with(|calls| calls.borrow_mut().clear());
            Self {
                output: None,
                status: Some(status),
            }
        }

        fn take_calls(&self) -> Vec<RecordedCommand> {
            SCRIPTED_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
        }
    }

    impl CommandRunner for ScriptedRunner {
        fn output(&mut self, program: &str, args: &[&str], root: &Path) -> io::Result<Output> {
            SCRIPTED_CALLS.with(|calls| {
                calls.borrow_mut().push(RecordedCommand::Output {
                    program: program.into(),
                    args: args.iter().map(|arg| (*arg).into()).collect(),
                    root: root.to_path_buf(),
                });
            });
            self.output.take().expect("unexpected output command")
        }

        fn status(&mut self, program: &str, args: &[&str], root: &Path) -> io::Result<ExitStatus> {
            SCRIPTED_CALLS.with(|calls| {
                calls.borrow_mut().push(RecordedCommand::Status {
                    program: program.into(),
                    args: args.iter().map(|arg| (*arg).into()).collect(),
                    root: root.to_path_buf(),
                });
            });
            self.status.take().expect("unexpected status command")
        }
    }

    fn command_output(code: i32, stdout: &str) -> Output {
        Output {
            status: command_status(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[cfg(unix)]
    fn command_status(code: i32) -> ExitStatus {
        ExitStatus::from_raw(code << 8)
    }

    #[cfg(windows)]
    fn command_status(code: i32) -> ExitStatus {
        ExitStatus::from_raw(code as u32)
    }

    #[test]
    fn run_dispatches_validation_and_reports_usage_errors() {
        let dir = temp_dir("run-dispatch");
        good_scaffold(&dir);
        let mut runner = ScriptedRunner {
            output: Some(Ok(command_output(0, "clean graph"))),
            status: None,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            run(
                &dir,
                &["validate-contracts".into()],
                &mut runner,
                &mut stdout,
                &mut stderr,
            ),
            ExitCode::SUCCESS
        );
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            "validate-contracts: PASS (structural + resolved feature graph)\n"
        );
        assert!(stderr.is_empty());
        assert_eq!(
            runner.take_calls(),
            [RecordedCommand::Output {
                program: "cargo".into(),
                args: vec![
                    "tree".into(),
                    "-p".into(),
                    "riot-ffi".into(),
                    "-e".into(),
                    "features".into(),
                    "--locked".into(),
                ],
                root: dir.clone(),
            }]
        );

        let mut runner = ScriptedRunner {
            output: Some(Ok(command_output(0, "clean graph"))),
            status: None,
        };
        assert_eq!(
            run(
                &dir,
                &["validate-contracts".into()],
                &mut runner,
                &mut FailingWriter,
                &mut Vec::new(),
            ),
            ExitCode::FAILURE
        );

        for args in [Vec::new(), vec!["unknown".into()]] {
            let mut runner = ScriptedRunner {
                output: None,
                status: None,
            };
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            assert_eq!(
                run(&dir, &args, &mut runner, &mut stdout, &mut stderr),
                ExitCode::FAILURE
            );
            assert!(stdout.is_empty());
            assert!(String::from_utf8(stderr).unwrap().contains("available:"));
        }
    }

    fn copy_dir_recursive(source: &Path, dest: &Path) {
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

    /// Copies the committed conference fixtures into a private root so the two
    /// fixture commands can be dispatched successfully without the sign command
    /// rewriting (or the verify command re-stamping) the real repository files.
    fn copy_conference_fixtures(dest_root: &Path) {
        let real_root = workspace_root_from(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        copy_dir_recursive(
            &real_root.join("fixtures/conference"),
            &dest_root.join("fixtures/conference"),
        );
    }

    /// Copies the committed newswire goldens into a private root so
    /// verify-newswire-export can be dispatched successfully without re-stamping
    /// the real repository files.
    fn copy_newswire_fixtures(dest_root: &Path) {
        let real_root = workspace_root_from(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        copy_dir_recursive(
            &real_root.join("fixtures/newswire"),
            &dest_root.join("fixtures/newswire"),
        );
    }

    #[test]
    fn conference_fixture_commands_report_success_and_failure() {
        // Success arms: each command runs against a faithful private copy of the
        // committed fixtures, so the dispatch's Ok branch is taken and returns
        // SUCCESS while the real repository fixtures are never touched.
        for command in ["sign-conference-fixture", "verify-conference-export"] {
            let root = temp_dir(&format!("conference-ok-{command}"));
            copy_conference_fixtures(&root);
            let mut runner = ScriptedRunner {
                output: None,
                status: None,
            };
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            assert_eq!(
                run(
                    &root,
                    &[command.into()],
                    &mut runner,
                    &mut stdout,
                    &mut stderr
                ),
                ExitCode::SUCCESS,
                "{command} against a valid fixture copy should succeed"
            );
            assert!(stderr.is_empty());
        }

        // Failure arms: an empty root has no fixtures, so each command's run()
        // returns Err and the dispatch reports FAILURE.
        for command in ["sign-conference-fixture", "verify-conference-export"] {
            let root = temp_dir(&format!("conference-missing-{command}"));
            let mut runner = ScriptedRunner {
                output: None,
                status: None,
            };
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            assert_eq!(
                run(
                    &root,
                    &[command.into()],
                    &mut runner,
                    &mut stdout,
                    &mut stderr
                ),
                ExitCode::FAILURE,
                "{command} against an empty root should fail"
            );
        }
    }

    #[test]
    fn newswire_fixture_commands_report_success_and_failure() {
        // export-newswire needs no on-disk input (it mints records), so a fresh
        // root suffices for its success arm; verify needs the committed goldens.
        let export_root = temp_dir("newswire-export-ok");
        let mut runner = ScriptedRunner {
            output: None,
            status: None,
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            run(
                &export_root,
                &["export-newswire".into()],
                &mut runner,
                &mut out,
                &mut err
            ),
            ExitCode::SUCCESS
        );
        assert!(err.is_empty());

        let verify_root = temp_dir("newswire-verify-ok");
        copy_newswire_fixtures(&verify_root);
        let mut runner = ScriptedRunner {
            output: None,
            status: None,
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            run(
                &verify_root,
                &["verify-newswire-export".into()],
                &mut runner,
                &mut out,
                &mut err
            ),
            ExitCode::SUCCESS
        );
        assert!(err.is_empty());

        // Failure arm: verify against an empty root has no fixtures.
        let missing = temp_dir("newswire-verify-missing");
        let mut runner = ScriptedRunner {
            output: None,
            status: None,
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            run(
                &missing,
                &["verify-newswire-export".into()],
                &mut runner,
                &mut out,
                &mut err
            ),
            ExitCode::FAILURE
        );
    }

    #[test]
    fn run_reports_validation_and_binding_build_failures() {
        let dir = temp_dir("run-failures");
        good_scaffold(&dir);
        std::fs::remove_file(dir.join("schemas/alert.cddl")).unwrap();
        let mut runner = ScriptedRunner {
            output: Some(Err(io::Error::other("cargo unavailable"))),
            status: None,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            run(
                &dir,
                &["validate-contracts".into()],
                &mut runner,
                &mut stdout,
                &mut stderr,
            ),
            ExitCode::FAILURE
        );
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("schemas/alert.cddl: file absent"));
        assert!(stderr.contains("cargo tree could not run"));

        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(1))),
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            run(
                &dir,
                &["generate-bindings".into()],
                &mut runner,
                &mut stdout,
                &mut stderr,
            ),
            ExitCode::FAILURE
        );
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("cargo build -p riot-ffi --lib --locked failed"));
    }

    #[test]
    fn resolved_graph_reports_status_and_every_forbidden_component() {
        let dir = temp_dir("resolved-graph");
        let mut runner = ScriptedRunner {
            output: Some(Ok(command_output(1, "ignored"))),
            status: None,
        };
        assert_eq!(
            check_resolved_feature_graph_with(&dir, &mut runner),
            vec!["feature graph: cargo tree --locked failed (lock drift?)"]
        );

        let graph = r#"
willow25 feature "drop_format"
openmls v1.0.0
riot-core feature "conformance"
bab_rs v0.7.0
"#;
        let mut runner = ScriptedRunner {
            output: Some(Ok(command_output(0, graph))),
            status: None,
        };
        let failures = check_resolved_feature_graph_with(&dir, &mut runner);
        assert_eq!(failures.len(), 4, "{failures:?}");
        for expected in ["drop_format", "openmls", "conformance", "wrong bab_rs"] {
            assert!(failures.iter().any(|failure| failure.contains(expected)));
        }
    }

    #[test]
    fn binding_build_reports_spawn_and_missing_library_failures() {
        let dir = temp_dir("binding-build-failures");
        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Err(io::Error::other("cargo missing"))),
        };
        assert_eq!(
            generate_mobile_bindings_with(&dir, &mut runner, &mut UniFfiBindingGenerator)
                .unwrap_err(),
            "could not build riot-ffi: cargo missing"
        );

        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        assert!(
            generate_mobile_bindings_with(&dir, &mut runner, &mut UniFfiBindingGenerator)
                .unwrap_err()
                .contains("host riot-ffi library absent")
        );
    }

    #[test]
    fn binding_generation_seam_covers_success_generation_and_validation_failures() {
        let dir = temp_dir("binding-generation-success");
        let library = dir.join("target/debug").join(format!(
            "{}riot_ffi{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ));
        std::fs::create_dir_all(library.parent().unwrap()).unwrap();
        std::fs::write(&library, b"fixture library").unwrap();
        let stale = dir.join("build/generated/riot-ffi/stale");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(stale, b"stale").unwrap();
        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        let mut generator = ScriptedBindingGenerator::new(Ok(()), true);
        let generated = generate_mobile_bindings_with(&dir, &mut runner, &mut generator).unwrap();
        assert_eq!(generated, dir.join("build/generated/riot-ffi"));
        assert!(generated.join("riot_ffi.swift").is_file());
        assert!(!generated.join("stale").exists());

        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            run_with(
                &dir,
                &["generate-bindings".into()],
                &mut runner,
                &mut generator,
                &mut stdout,
                &mut stderr,
            ),
            ExitCode::SUCCESS
        );
        assert!(String::from_utf8(stdout)
            .unwrap()
            .contains("generate-bindings: PASS"));
        assert!(stderr.is_empty());

        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        assert_eq!(
            run_with(
                &dir,
                &["generate-bindings".into()],
                &mut runner,
                &mut generator,
                &mut FailingWriter,
                &mut Vec::new(),
            ),
            ExitCode::FAILURE
        );

        let dir = temp_dir("binding-generation-error");
        let library = dir.join("target/debug").join(format!(
            "{}riot_ffi{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ));
        std::fs::create_dir_all(library.parent().unwrap()).unwrap();
        std::fs::write(&library, b"fixture library").unwrap();
        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        let mut generator =
            ScriptedBindingGenerator::new(Err("injected generator failure".into()), false);
        assert_eq!(
            generate_mobile_bindings_with(&dir, &mut runner, &mut generator).unwrap_err(),
            "injected generator failure"
        );

        let dir = temp_dir("binding-validation-through-command");
        let library = dir.join("target/debug").join(format!(
            "{}riot_ffi{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ));
        std::fs::create_dir_all(library.parent().unwrap()).unwrap();
        std::fs::write(&library, b"fixture library").unwrap();
        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        let mut generator = ScriptedBindingGenerator::new(Ok(()), false);
        assert!(
            generate_mobile_bindings_with(&dir, &mut runner, &mut generator)
                .unwrap_err()
                .contains("generated binding absent")
        );

        let dir = temp_dir("binding-validation-error");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(validate_generated_bindings(&dir)
            .unwrap_err()
            .contains("generated binding absent"));
        std::fs::write(dir.join("riot_ffi.swift"), b"").unwrap();
        assert!(validate_generated_bindings(&dir)
            .unwrap_err()
            .contains("not a non-empty file"));
        std::fs::remove_file(dir.join("riot_ffi.swift")).unwrap();
        std::fs::create_dir(dir.join("riot_ffi.swift")).unwrap();
        assert!(validate_generated_bindings(&dir)
            .unwrap_err()
            .contains("not a non-empty file"));
    }

    #[test]
    fn binding_generation_reports_output_directory_filesystem_errors() {
        let prepare = |label: &str| {
            let dir = temp_dir(label);
            let library = dir.join("target/debug").join(format!(
                "{}riot_ffi{}",
                std::env::consts::DLL_PREFIX,
                std::env::consts::DLL_SUFFIX
            ));
            std::fs::create_dir_all(library.parent().unwrap()).unwrap();
            std::fs::write(library, b"fixture library").unwrap();
            dir
        };

        let dir = prepare("binding-clean-error");
        std::fs::create_dir_all(dir.join("build/generated")).unwrap();
        std::fs::write(dir.join("build/generated/riot-ffi"), b"not a directory").unwrap();
        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        let mut generator = ScriptedBindingGenerator::new(Ok(()), false);
        assert!(
            generate_mobile_bindings_with(&dir, &mut runner, &mut generator)
                .unwrap_err()
                .contains("could not clean")
        );

        let dir = prepare("binding-create-error");
        std::fs::create_dir_all(dir.join("build")).unwrap();
        std::fs::write(dir.join("build/generated"), b"not a directory").unwrap();
        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        assert!(
            generate_mobile_bindings_with(&dir, &mut runner, &mut generator)
                .unwrap_err()
                .contains("could not create")
        );
    }

    #[cfg(unix)]
    #[test]
    fn binding_generation_rejects_non_utf8_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![b'r', 0xff]));
        assert!(utf8_path(path.clone(), "library")
            .unwrap_err()
            .contains("non-UTF-8 library path"));
        assert!(utf8_path(path, "output")
            .unwrap_err()
            .contains("non-UTF-8 output path"));

        let root = PathBuf::from(OsString::from_vec(vec![b'r', 0xff]));
        let mut runner = ScriptedRunner {
            output: None,
            status: Some(Ok(command_status(0))),
        };
        let mut generator = ScriptedBindingGenerator::new(Ok(()), false);
        assert!(
            generate_mobile_bindings_with(&root, &mut runner, &mut generator)
                .unwrap_err()
                .contains("non-UTF-8 workspace path")
        );
    }

    #[test]
    fn real_runner_workspace_root_and_entry_point_are_exercised() {
        let root = workspace_root_from(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(root.join("Cargo.toml").is_file());
        let mut runner = OsCommandRunner;
        let output = runner.output("rustc", &["--version"], &root).unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("rustc "));
        assert!(runner
            .status("rustc", &["--version"], &root)
            .unwrap()
            .success());
        assert!(check_resolved_feature_graph(&root).is_empty());
        let status = main();
        assert!([ExitCode::SUCCESS, ExitCode::FAILURE].contains(&status));

        let mut runner = ScriptedRunner {
            output: None,
            status: None,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            main_with_manifest_dir(Path::new("/"), &[], &mut runner, &mut stdout, &mut stderr,),
            ExitCode::FAILURE
        );
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("could not discover workspace root"));
    }

    #[test]
    fn real_generator_and_test_adapters_cover_failure_and_flush_paths() {
        let temp = temp_dir("real-generator-error");
        let mut generator = UniFfiBindingGenerator;
        let error = generator
            .generate(
                Utf8PathBuf::from("/definitely/missing/libriot_ffi"),
                Utf8PathBuf::from_path_buf(temp).unwrap(),
            )
            .unwrap_err();
        assert!(error.contains("UniFFI generation failed"));

        let mut writer = FailingWriter;
        assert_eq!(
            writer.flush().unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );

        let out = Utf8PathBuf::from_path_buf(temp_dir("scripted-empty")).unwrap();
        let mut scripted = ScriptedBindingGenerator::new(Ok(()), false);
        scripted
            .generate(Utf8PathBuf::from("ignored"), out)
            .unwrap();
    }

    #[test]
    fn validators_cover_unreadable_crate_manifest_untyped_dependency_and_missing_lock() {
        let unreadable = temp_dir("unreadable-crate-manifest");
        good_scaffold(&unreadable);
        std::fs::create_dir_all(unreadable.join("crates/bad/Cargo.toml")).unwrap();
        std::fs::create_dir_all(unreadable.join("crates/unrelated")).unwrap();
        std::fs::write(
            unreadable.join("crates/unrelated/Cargo.toml"),
            "[package]\nname = \"unrelated\"\nversion = \"0.0.0\"\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        assert!(validate_contents(&unreadable).is_empty());

        let untyped = temp_dir("untyped-dependency");
        good_scaffold(&untyped);
        let workspace = good_workspace_toml().replace(
            "willow25 = { version = \"=0.6.0-alpha.3\", default-features = false, features = [\"std\"] }",
            "willow25 = 42",
        );
        std::fs::write(untyped.join("Cargo.toml"), workspace).unwrap();
        let failures = validate_contents(&untyped);
        let willow_failure = failures
            .iter()
            .find(|failure| failure.contains("willow25"))
            .unwrap();
        assert!(willow_failure.contains("found None"));

        let missing_lock = temp_dir("missing-lock-with-manifest");
        good_scaffold(&missing_lock);
        std::fs::remove_file(missing_lock.join("Cargo.lock")).unwrap();
        let failures = validate_contents(&missing_lock);
        assert!(failures
            .iter()
            .any(|failure| failure.contains("Cargo.lock: file absent")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("cargo_lock_sha256 missing or Cargo.lock unreadable")));

        let empty_vector_hash = temp_dir("empty-vector-hash");
        good_scaffold(&empty_vector_hash);
        let lock_hash = sha256_hex(good_lock().as_bytes());
        std::fs::write(
            empty_vector_hash.join("fixtures/manifest.json"),
            manifest_with(&lock_hash, ""),
        )
        .unwrap();
        assert!(validate_contents(&empty_vector_hash)
            .iter()
            .any(|failure| failure.contains("missing/empty")));

        let mut direct = Vec::new();
        let deps: toml::Value = "dep = { version = \"1\", default-features = true }"
            .parse::<toml::Table>()
            .unwrap()
            .into();
        check_dep(&deps, "dep", "1", Some((true, &[], &[])), &mut direct);
        assert_eq!(
            direct,
            ["Cargo.toml: `dep` must set default-features = false"]
        );

        let mut direct = Vec::new();
        let deps: toml::Value = "dep = { version = \"1\", default-features = false }"
            .parse::<toml::Table>()
            .unwrap()
            .into();
        check_dep(&deps, "dep", "1", Some((true, &[], &[])), &mut direct);
        assert!(direct.is_empty());
    }

    #[test]
    fn records_exact_binding_build_and_generator_contracts_and_root_discovery() {
        let dir = temp_dir("recorded-binding-contract");
        let library = dir.join("target/debug").join(format!(
            "{}riot_ffi{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ));
        std::fs::create_dir_all(library.parent().unwrap()).unwrap();
        std::fs::write(&library, b"fixture library").unwrap();
        let mut runner = ScriptedRunner::with_status(Ok(command_status(0)));
        let mut generator = ScriptedBindingGenerator::new(Ok(()), true);
        generate_mobile_bindings_with(&dir, &mut runner, &mut generator).unwrap();
        assert_eq!(
            runner.take_calls(),
            [RecordedCommand::Status {
                program: "cargo".into(),
                args: vec![
                    "build".into(),
                    "-p".into(),
                    "riot-ffi".into(),
                    "--lib".into(),
                    "--locked".into()
                ],
                root: dir.clone(),
            }]
        );
        assert_eq!(generator.calls.len(), 1);
        assert_eq!(
            generator.calls[0].0,
            Utf8PathBuf::from_path_buf(library).unwrap()
        );
        assert_eq!(
            generator.calls[0].1,
            Utf8PathBuf::from_path_buf(dir.join("build/generated/riot-ffi")).unwrap()
        );

        assert_eq!(
            workspace_root_from(Path::new("/workspace/crates/xtask")),
            Ok(PathBuf::from("/workspace"))
        );
        assert_eq!(
            workspace_root_from(Path::new("/")),
            Err("could not discover workspace root from /".into())
        );
    }

    #[test]
    fn dependency_predicates_cover_complete_name_feature_and_default_truth_tables() {
        let cases = [
            (
                "unrelated",
                "serde = { version = \"1\", features = [\"drop_format\"] }",
                Vec::<&str>::new(),
            ),
            (
                "willow-clean",
                "willow25 = { workspace = true, features = [\"std\"] }",
                vec![],
            ),
            (
                "willow-drop",
                "willow25 = { workspace = true, features = [\"drop_format\"] }",
                vec!["drop_format"],
            ),
            (
                "willow-version",
                "willow25 = \"0.6\"",
                vec!["workspace = true"],
            ),
            ("bab-clean", "bab_rs = { workspace = true }", vec![]),
            (
                "bab-version",
                "bab_rs = \"0.8.1\"",
                vec!["workspace = true"],
            ),
        ];
        for (label, dependency, expected) in cases {
            let root = temp_dir(label);
            std::fs::create_dir_all(root.join("crates/example")).unwrap();
            std::fs::write(
                root.join("crates/example/Cargo.toml"),
                format!(
                    "[package]\nname = \"example\"\nversion = \"0.0.0\"\n[dependencies]\n{dependency}\n"
                ),
            )
            .unwrap();
            let mut failures = Vec::new();
            check_crate_manifests(&root, &mut failures);
            assert_eq!(failures.len(), expected.len(), "{label}: {failures:?}");
            for expected_text in expected {
                assert!(
                    failures
                        .iter()
                        .any(|failure| failure.contains(expected_text)),
                    "{label}: missing {expected_text:?} in {failures:?}"
                );
            }
        }

        for (default_off, default_features, expect_failure) in [
            (false, false, false),
            (false, true, false),
            (true, false, false),
            (true, true, true),
        ] {
            let deps: toml::Value =
                format!("dep = {{ version = \"1\", default-features = {default_features} }}")
                    .parse::<toml::Table>()
                    .unwrap()
                    .into();
            let mut failures = Vec::new();
            check_dep(
                &deps,
                "dep",
                "1",
                Some((default_off, &[], &[])),
                &mut failures,
            );
            assert_eq!(
                failures
                    .iter()
                    .any(|failure| failure.contains("default-features = false")),
                expect_failure,
                "default_off={default_off}, default_features={default_features}: {failures:?}"
            );
        }
    }

    #[test]
    fn validators_report_missing_malformed_and_mismatched_artifacts() {
        let empty = temp_dir("validator-empty");
        let failures = validate_contents(&empty);
        for expected in [
            "Cargo.toml: file absent",
            "Cargo.lock: file absent",
            "fixtures/manifest.json: file absent",
            "schemas/alert.cddl: file absent",
        ] {
            assert!(failures.iter().any(|failure| failure.contains(expected)));
        }

        let malformed = temp_dir("validator-malformed");
        good_scaffold(&malformed);
        std::fs::write(malformed.join("Cargo.toml"), "not = [toml").unwrap();
        std::fs::write(malformed.join("Cargo.lock"), "not = [toml").unwrap();
        std::fs::write(malformed.join("fixtures/manifest.json"), "not json").unwrap();
        std::fs::write(malformed.join("schemas/alert.cddl"), "wrong schema").unwrap();
        let failures = validate_contents(&malformed);
        for expected in [
            "Cargo.toml: not valid TOML",
            "Cargo.lock: not valid TOML",
            "fixtures/manifest.json: not valid JSON",
            "missing schema id",
        ] {
            assert!(failures.iter().any(|failure| failure.contains(expected)));
        }

        let dependencies = temp_dir("validator-dependencies");
        good_scaffold(&dependencies);
        let workspace = good_workspace_toml()
            .replace("hifitime = \"=4.3.0\"", "")
            .replace("features = [\"std\"]", "features = []")
            .replace("features = [\"william3\"]", "features = []");
        std::fs::write(dependencies.join("Cargo.toml"), workspace).unwrap();
        let failures = validate_contents(&dependencies);
        for expected in [
            "dependency `hifitime` absent",
            "enable feature `std`",
            "enable feature `william3`",
        ] {
            assert!(failures.iter().any(|failure| failure.contains(expected)));
        }

        let crate_manifest = temp_dir("validator-crate-manifest");
        good_scaffold(&crate_manifest);
        std::fs::create_dir_all(crate_manifest.join("crates/bad")).unwrap();
        std::fs::write(crate_manifest.join("crates/bad/Cargo.toml"), "not = [toml").unwrap();
        let failures = validate_contents(&crate_manifest);
        assert!(failures
            .iter()
            .any(|failure| failure.contains("not valid TOML")));

        let lock = temp_dir("validator-lock-contents");
        good_scaffold(&lock);
        let bad_lock = r#"version = 4
[[package]]
name = "willow25"
version = "0.5.0"
[[package]]
name = "bab_rs"
version = "0.8.1"
[[package]]
name = "openmls"
version = "1.0.0"
"#;
        std::fs::write(lock.join("Cargo.lock"), bad_lock).unwrap();
        std::fs::write(
            lock.join("fixtures/manifest.json"),
            manifest_with(
                &sha256_hex(bad_lock.as_bytes()),
                &sha256_hex(b"{\"vectors\":[]}"),
            ),
        )
        .unwrap();
        let failures = validate_contents(&lock);
        assert!(failures.iter().any(|failure| failure.contains("willow25")));
        assert!(failures.iter().any(|failure| failure.contains("openmls")));

        let fixture = temp_dir("validator-fixture-details");
        good_scaffold(&fixture);
        let lock_hash = sha256_hex(good_lock().as_bytes());
        let bad_manifest = manifest_with(&lock_hash, &"f".repeat(64))
            .replace("1:1 - compression forbidden", "2:1")
            .replace("\"objects\": \"WU1\",", "")
            .replace("\"status\",", "");
        std::fs::write(fixture.join("fixtures/manifest.json"), bad_manifest).unwrap();
        let failures = validate_contents(&fixture);
        for expected in [
            "william3_vectors_sha256 mismatch",
            "expansion_ratio",
            "fixture_ownership.objects",
            "report_fields missing `status`",
        ] {
            assert!(failures.iter().any(|failure| failure.contains(expected)));
        }
    }

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
        std::fs::create_dir_all(dir.join("fixtures/governance")).unwrap();
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
            dir.join("fixtures/willow/meadowcap-vectors.json"),
            b"{\"meadowcap\":[]}",
        )
        .unwrap();
        std::fs::write(
            dir.join("fixtures/governance/governance-vectors.json"),
            b"{\"records\":{}}",
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
        let meadowcap_hash = sha256_hex(b"{\"meadowcap\":[]}");
        let governance_hash = sha256_hex(b"{\"records\":{}}");
        let mut ceilings: String = EXPECTED_CEILINGS
            .iter()
            .map(|(k, v)| format!("\"{k}\": {v}"))
            .collect::<Vec<_>>()
            .join(",");
        ceilings.push_str(",\"expansion_ratio\": \"1:1 - compression forbidden\"");
        format!(
            r#"{{
  "environment": {{ "cargo_lock_sha256": "{lock_hash}", "william3_vectors_sha256": "{vectors_hash}", "meadowcap_vectors_sha256": "{meadowcap_hash}", "governance_vectors_sha256": "{governance_hash}" }},
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
        let failure = failures
            .iter()
            .find(|failure| failure.contains("willow25"))
            .unwrap();
        assert!(
            failure.contains(WILLOW25_PIN),
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
        let failure = failures
            .iter()
            .find(|failure| failure.contains("bab_rs"))
            .unwrap();
        assert!(
            failure.contains("incorrect WILLIAM3"),
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
        let failure = failures
            .iter()
            .find(|failure| failure.contains("panic"))
            .unwrap();
        assert!(
            failure.contains("unwind"),
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
        let failure = failures
            .iter()
            .find(|failure| failure.contains("artifact_bytes"))
            .unwrap();
        assert!(
            failure.contains("exactly 8388608"),
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
        let failure = failures
            .iter()
            .find(|failure| failure.contains("riot-core"))
            .unwrap();
        assert!(
            failure.contains("drop_format"),
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

    /// The two fixture subcommands report a failing run as a non-zero exit
    /// status rather than panicking or, worse, exiting zero. Pointed at a root
    /// with no workspace in it, each has nothing to read and must say so.
    ///
    /// This exercises the dispatch arms, not the fixture logic: `root` is a
    /// temporary directory, so neither command can touch the repository's real
    /// fixtures.
    #[test]
    fn a_failing_fixture_subcommand_exits_non_zero() {
        for command in ["sign-conference-fixture", "verify-conference-export"] {
            let dir = temp_dir(&format!("fixture-dispatch-{command}"));
            let mut runner = ScriptedRunner {
                output: None,
                status: Some(Ok(command_status(0))),
            };
            let mut generator = ScriptedBindingGenerator::new(Ok(()), false);
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            assert_eq!(
                run_with(
                    &dir,
                    &[command.to_string()],
                    &mut runner,
                    &mut generator,
                    &mut stdout,
                    &mut stderr,
                ),
                ExitCode::FAILURE,
                "{command} must fail when it has no workspace to read"
            );
        }
    }
}
