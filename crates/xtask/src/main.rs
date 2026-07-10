//! Phase 0A build orchestration. `validate-contracts` verifies that the
//! frozen-environment contract files exist and contain every required
//! section before any feature work is allowed to claim evidence.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("validate-contracts") => validate_contracts(),
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
    // xtask runs from the workspace via the cargo alias; CARGO_MANIFEST_DIR
    // is crates/xtask, so the root is two levels up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

struct Contract {
    file: &'static str,
    required: &'static [&'static str],
}

/// Every contract file and the exact keys it must contain. A missing file or
/// key is reported individually so the RED run enumerates the full gap list.
const CONTRACTS: &[Contract] = &[
    Contract {
        file: "rust-toolchain.toml",
        required: &["1.95.0"],
    },
    Contract {
        file: "Cargo.lock",
        required: &["willow25", "uniffi", "minicbor", "sha2", "ed25519-dalek"],
    },
    Contract {
        file: "schemas/alert.cddl",
        required: &["org.riot.alert/1"],
    },
    Contract {
        file: "fixtures/manifest.json",
        required: &[
            // frozen environment pins
            "\"rust\"",
            "\"xcode\"",
            "\"swift\"",
            "\"ios_deployment_floor\"",
            "\"android_gradle_plugin\"",
            "\"gradle\"",
            "\"jdk\"",
            "\"android_build_tools\"",
            "\"android_ndk\"",
            "\"android_platform\"",
            "\"android_system_image\"",
            "\"android_system_image_revision\"",
            "\"commandlinetools\"",
            "\"platform_tools\"",
            "\"emulator\"",
            "\"cargo_lock_sha256\"",
            "\"gradle_locks_sha256\"",
            // resource ceilings (must mirror the sprint design table)
            "\"ceilings\"",
            "\"artifact_bytes\"",
            "\"entries_per_bundle\"",
            "\"payload_bytes\"",
            "\"cbor_nesting\"",
            "\"map_entries\"",
            "\"decoded_cbor_nodes\"",
            "\"string_bytes\"",
            "\"path_components\"",
            "\"path_component_bytes\"",
            "\"path_total_bytes\"",
            "\"authorization_chain_depth\"",
            "\"authorization_bytes_per_entry\"",
            "\"authorization_bytes_per_bundle\"",
            "\"warning_records\"",
            "\"store_entries\"",
            "\"store_encoded_entry_bytes\"",
            "\"durable_receipts\"",
            "\"open_stores_per_session\"",
            "\"open_previews_per_session\"",
            "\"retained_preview_input_bytes\"",
            "\"retained_preview_output_bytes\"",
            "\"transaction_snapshot_bytes\"",
            "\"inspection_target_seconds\"",
            // fixture ownership: which work unit owns which fixture family
            "\"fixture_ownership\"",
            "\"objects\"",
            "\"willow\"",
            "\"imports\"",
            // gate report format
            "\"report_fields\"",
            "\"status\"",
            "\"owning_work_unit\"",
            "\"commands\"",
            "\"environment\"",
            "\"evidence_paths\"",
            "\"hashes\"",
            "\"elapsed_agent_hours\"",
            "\"next_action\"",
        ],
    },
];

fn validate_contracts() -> ExitCode {
    let root = workspace_root();
    let mut failures: Vec<String> = Vec::new();

    for contract in CONTRACTS {
        let path = root.join(contract.file);
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                for key in contract.required {
                    if !contents.contains(key) {
                        failures.push(format!("{}: missing required {key}", contract.file));
                    }
                }
            }
            Err(_) => failures.push(format!("{}: file absent", contract.file)),
        }
    }

    if failures.is_empty() {
        println!("validate-contracts: PASS ({} contract files)", CONTRACTS.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("validate-contracts: FAIL");
        for failure in &failures {
            eprintln!("  {failure}");
        }
        eprintln!("{} missing contract element(s)", failures.len());
        ExitCode::FAILURE
    }
}
