//! Manifest-visitor evidence for `riot_app_cli::pack`. The custom serde
//! `visit_map` bounds every string field through `bounded_json_string`. These
//! tests drive both the successful parse of a fully valid manifest (the Ok
//! construction and each bounded-argument site) and the rejection of a field
//! that exceeds its `MAX_APP_*_BYTES` ceiling, asserting the stable public
//! error message each time.

use std::fs;
use std::path::PathBuf;

use riot_app_cli::{pack, PackInput};
use riot_core::apps::manifest::{
    MAX_APP_DESCRIPTION_BYTES, MAX_APP_ENTRY_POINT_BYTES, MAX_APP_NAME_BYTES,
    MAX_APP_PERMISSION_BYTES, MAX_APP_VERSION_BYTES,
};
use riot_core::willow::generate_communal_author;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app")
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap()
}

/// A temp app directory with the fixture resources and the given manifest JSON.
fn app_dir_with_manifest(json: &str) -> tempfile::TempDir {
    let tmp = tempdir();
    for name in ["index.html", "app.js"] {
        fs::copy(fixture().join(name), tmp.path().join(name)).unwrap();
    }
    fs::write(tmp.path().join("riot-app.json"), json).unwrap();
    tmp
}

fn pack_dir(dir: &std::path::Path) -> Result<riot_app_cli::PackOutput, riot_app_cli::PackError> {
    let author = generate_communal_author().unwrap();
    pack(PackInput {
        app_dir: dir,
        author: &author,
        timestamp_micros: 1,
    })
}

fn manifest_json(name: &str, description: &str, version: &str, entry_point: &str) -> String {
    format!(
        r#"{{"name":{name},"description":{description},"version":{version},"entry_point":{entry_point},"permissions":["app-data"]}}"#,
        name = serde_json::to_string(name).unwrap(),
        description = serde_json::to_string(description).unwrap(),
        version = serde_json::to_string(version).unwrap(),
        entry_point = serde_json::to_string(entry_point).unwrap(),
    )
}

// ---------------------------------------------------------------------------
// The success path: a fully valid manifest parses and packs. This exercises
// each `bounded_json_string(..)` call's Ok arm and the final `ManifestInput`
// construction.
// ---------------------------------------------------------------------------

#[test]
fn a_fully_valid_manifest_parses_and_packs() {
    let json = manifest_json("Hello Riot", "A tiny test app", "1.0.0", "index.html");
    let dir = app_dir_with_manifest(&json);
    let output = pack_dir(dir.path()).expect("valid manifest packs");
    // The manifest bytes round-trip to the same canonical name.
    let manifest = riot_core::apps::manifest::decode_manifest(&output.manifest_bytes)
        .expect("decode manifest");
    assert_eq!(manifest.name, "Hello Riot");
    assert_eq!(manifest.entry_point, "index.html");
}

// ---------------------------------------------------------------------------
// The error path: each string field, when it exceeds its ceiling, is rejected
// with the shared bounded-string message. entry_point stays valid in the other
// cases so that the failing field is the one under test.
// ---------------------------------------------------------------------------

fn assert_bounded_rejection(json: &str, field: &str) {
    let dir = app_dir_with_manifest(json);
    let error = pack_dir(dir.path())
        .expect_err("over-limit field must be rejected")
        .to_string();
    // The Display impl escapes the quotes around the field name.
    let needle = format!("\\'{field}\\' is empty, too long, or contains control characters");
    assert!(error.contains(&needle), "expected {needle:?} in {error:?}");
}

#[test]
fn over_limit_name_is_rejected() {
    let json = manifest_json(
        &"n".repeat(MAX_APP_NAME_BYTES + 1),
        "ok",
        "1.0.0",
        "index.html",
    );
    assert_bounded_rejection(&json, "name");
}

#[test]
fn over_limit_description_is_rejected() {
    let json = manifest_json(
        "Hello",
        &"d".repeat(MAX_APP_DESCRIPTION_BYTES + 1),
        "1.0.0",
        "index.html",
    );
    assert_bounded_rejection(&json, "description");
}

#[test]
fn over_limit_version_is_rejected() {
    let json = manifest_json(
        "Hello",
        "ok",
        &"1".repeat(MAX_APP_VERSION_BYTES + 1),
        "index.html",
    );
    assert_bounded_rejection(&json, "version");
}

#[test]
fn over_limit_entry_point_is_rejected() {
    let json = manifest_json(
        "Hello",
        "ok",
        "1.0.0",
        &"e".repeat(MAX_APP_ENTRY_POINT_BYTES + 1),
    );
    assert_bounded_rejection(&json, "entry_point");
}

#[test]
fn over_limit_permission_is_rejected() {
    let long = "p".repeat(MAX_APP_PERMISSION_BYTES + 1);
    let json = format!(
        r#"{{"name":"Hello","description":"ok","version":"1.0.0","entry_point":"index.html","permissions":[{}]}}"#,
        serde_json::to_string(&long).unwrap(),
    );
    let dir = app_dir_with_manifest(&json);
    let error = pack_dir(dir.path())
        .expect_err("over-limit permission must be rejected")
        .to_string();
    assert!(
        error.contains(
            "permission at index 0\\' is empty, too long, or contains control characters"
        ),
        "unexpected error: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// A missing field triggers the `ok_or_else(missing_field)` arm of the final
// `ManifestInput` construction.
// ---------------------------------------------------------------------------

#[test]
fn a_missing_field_is_rejected_as_missing() {
    // No `permissions` key.
    let json =
        r#"{"name":"Hello","description":"ok","version":"1.0.0","entry_point":"index.html"}"#;
    let dir = app_dir_with_manifest(json);
    let error = pack_dir(dir.path())
        .expect_err("missing field must be rejected")
        .to_string();
    assert!(
        error.contains("missing field `permissions`"),
        "unexpected error: {error:?}"
    );
}
