//! Prototype: package the newswire web viewer as a signed Willow app-drop.
//!
//! Proves the "web app = a packaged Willow drop" idea against the REAL apps API
//! (`riot_core::apps::bundle`): take the rendered viewer (index.html) plus the
//! projected newswire data, build an `AppBundle`, run the format's own egress
//! fence (`scan_bundle_egress` — refuses phone-home/WebRTC content by byte
//! scan), encode to the canonical drop bytes, content-address it
//! (`app_bundle_digest`), and confirm it round-trips through `decode_app_bundle`.
//!
//! Run: cargo test -p riot-core --test prototype_newswire_app_bundle -- --ignored --nocapture
//! Output: fixtures/newswire/newswire.bundle (the drop) + its app digest.

use std::fs;
use std::path::{Path, PathBuf};

use riot_core::apps::bundle::{
    app_bundle_digest, decode_app_bundle, encode_app_bundle, scan_bundle_egress, AppBundle,
    AppResource,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read_or(path: &Path, fallback: &str) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|_| fallback.as_bytes().to_vec())
}

#[test]
#[ignore = "prototype: writes an app-drop; run with --ignored"]
fn package_newswire_viewer_as_app_drop() {
    let root = repo_root();

    // The rendered viewer (Python gateway output) + the projected data. Falls
    // back to a stub so the prototype runs even before `build.py` is invoked.
    let html = read_or(
        &root.join("deploy/cf-mirror/dist/index.html"),
        "<!doctype html><meta charset=\"utf-8\"><title>RIOT Newswire</title><p>newswire viewer</p>",
    );
    let export = read_or(&root.join("fixtures/newswire/newswire-export-v1.json"), "{}");

    let bundle = AppBundle {
        entry_point: "index.html".into(),
        resources: vec![
            AppResource {
                path: "index.html".into(),
                content_type: "text/html".into(),
                bytes: html,
            },
            AppResource {
                path: "newswire-export.json".into(),
                content_type: "application/json".into(),
                bytes: export,
            },
        ],
    };

    // Format-level fence: a bundled app may not carry WebRTC/peer-escape content.
    scan_bundle_egress(&bundle).expect("newswire viewer must pass the egress scan");

    let encoded = encode_app_bundle(&bundle).expect("encode app bundle");
    let digest = app_bundle_digest(&encoded);
    let decoded = decode_app_bundle(&encoded).expect("decode app bundle");
    assert_eq!(decoded, bundle, "the app-drop must round-trip byte-identically");

    let out = root.join("fixtures/newswire/newswire.bundle");
    fs::write(&out, &encoded).expect("write bundle");

    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    eprintln!("wrote {} ({} bytes)", out.display(), encoded.len());
    eprintln!("entry_point = index.html · resources = index.html + newswire-export.json");
    eprintln!("app_bundle_digest = {hex}");
}
