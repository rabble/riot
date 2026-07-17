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
use riot_core::apps::manifest::{app_id_for, decode_manifest, encode_manifest, AppManifest};
use riot_core::willow::identity::generate_space_organizer_author;

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

/// The other half: OPEN the drop. Decode the committed bundle and reconstitute
/// the runnable site from the bundle bytes ALONE — proving the drop is a working
/// web app, not just bytes that round-trip. Writes /tmp/newswire-from-drop.
#[test]
#[ignore = "prototype: unpacks the drop to /tmp; run with --ignored"]
fn open_the_drop_and_reconstitute_the_web_app() {
    let root = repo_root();
    let bytes = fs::read(root.join("fixtures/newswire/newswire.bundle"))
        .expect("run package_newswire_viewer_as_app_drop first to write the bundle");
    let bundle = decode_app_bundle(&bytes).expect("decode the drop");

    let entry = bundle
        .resources
        .iter()
        .find(|r| r.path == bundle.entry_point)
        .expect("entry point resource present");
    assert_eq!(entry.content_type, "text/html");
    assert!(
        entry.bytes.windows(9).any(|w| w == b"<!doctype"),
        "entry point is real HTML"
    );

    // Reconstitute the whole site from the drop and nothing else.
    let out = Path::new("/tmp/newswire-from-drop");
    let _ = fs::remove_dir_all(out);
    for r in &bundle.resources {
        let dest = out.join(&r.path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(&dest, &r.bytes).unwrap();
    }
    eprintln!(
        "opened drop: {} resources → {} (entry_point {})",
        bundle.resources.len(),
        out.display(),
        bundle.entry_point
    );
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
    let export = read_or(
        &root.join("fixtures/newswire/newswire-export-v1.json"),
        "{}",
    );

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
    assert_eq!(
        decoded, bundle,
        "the app-drop must round-trip byte-identically"
    );

    let out = root.join("fixtures/newswire/newswire.bundle");
    fs::write(&out, &encoded).expect("write bundle");

    // Wrap the drop in a signed manifest → a launchable app. The manifest names
    // a real author identity; app_id_for binds manifest + bundle_digest into the
    // content-addressed AppId a space grants its per-app trust decision to.
    let author = generate_space_organizer_author().expect("author identity");
    let manifest = AppManifest {
        name: "RIOT Newswire".into(),
        description: "Independent newswire — projected from signed Willow records.".into(),
        version: "1".into(),
        author: author.identity(),
        permissions: vec![],
        entry_point: "index.html".into(),
    };
    assert_eq!(
        manifest.entry_point, bundle.entry_point,
        "manifest must point at the bundle's entry"
    );

    let manifest_bytes = encode_manifest(&manifest).expect("encode manifest");
    let app_id = app_id_for(&manifest, &digest).expect("app id");
    assert_eq!(
        decode_manifest(&manifest_bytes).expect("decode manifest"),
        manifest,
        "manifest round-trips"
    );

    let manifest_out = root.join("fixtures/newswire/newswire.manifest.cbor");
    fs::write(&manifest_out, &manifest_bytes).expect("write manifest");

    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    eprintln!("wrote {} ({} bytes)", out.display(), encoded.len());
    eprintln!(
        "wrote {} ({} bytes)",
        manifest_out.display(),
        manifest_bytes.len()
    );
    eprintln!("entry_point = index.html · resources = index.html + newswire-export.json");
    eprintln!("app_bundle_digest = {}", hex(&digest));
    eprintln!(
        "author           = {}",
        hex(&author.identity().signing_key_id)
    );
    eprintln!("app_id           = {}", hex(&app_id));
}
