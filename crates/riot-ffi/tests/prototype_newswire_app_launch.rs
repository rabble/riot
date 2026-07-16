//! Prototype: the app-side "truth" door. Import the newswire app-drop through
//! the real runtime, make the per-profile trust decision, launch it past the
//! gate, and exercise the WebView data bridge.
//!
//! Mirrors what the native app does when it opens a page=app: install_app
//! (verify + publish into the store) → trust_app (the launch decision) →
//! open_app_execution (the gate, denies-closed unless trusted) → app_data_*
//! (the bridge a WebView calls). The actual pixel render is native; this proves
//! everything up to and including the runtime data door in headless Rust.
//!
//! Run: cargo test -p riot-ffi --test prototype_newswire_app_launch -- --ignored --nocapture
//! (Run the packaging prototype first so the .bundle / .manifest exist.)

use std::fs;
use std::path::PathBuf;

use riot_ffi::open_local_profile;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
#[ignore = "prototype: installs + launches the newswire app-drop; run with --ignored"]
fn install_trust_launch_the_newswire_app() {
    let root = repo_root();
    let manifest = fs::read(root.join("fixtures/newswire/newswire.manifest.cbor"))
        .expect("manifest — run the packaging prototype first");
    let bundle = fs::read(root.join("fixtures/newswire/newswire.bundle")).expect("bundle");

    let profile = open_local_profile().expect("profile");
    let runtime = profile.app_runtime();

    // IMPORT — verify the (manifest, bundle) pair and publish it into the store.
    let app = runtime.install_app(manifest, bundle).expect("install the app-drop");
    eprintln!(
        "installed: app_id={} name={:?} entry_point={}",
        app.app_id, app.name, app.entry_point
    );

    // The launch gate denies-closed before a trust decision is made.
    assert!(
        profile.open_app_execution(app.app_id.clone()).is_err(),
        "an untrusted app must NOT launch"
    );

    // TRUST — the per-profile decision to run this app.
    runtime.trust_app(app.app_id.clone()).expect("trust");
    assert!(runtime.is_app_trusted(app.app_id.clone()).expect("trust query"));

    // LAUNCH — the gate opens only for a trusted app.
    let exec = profile
        .open_app_execution(app.app_id.clone())
        .expect("launch the trusted app");
    assert!(exec.is_valid(), "a fresh launched session is valid");

    // BRIDGE — the runtime data door a WebView calls. Round-trip a key.
    exec.app_data_put("draft/1".into(), b"hello from the launched app".to_vec())
        .expect("bridge put");
    let got = exec.app_data_get("draft/1".into()).expect("bridge get");
    assert_eq!(
        got.as_deref(),
        Some(&b"hello from the launched app"[..]),
        "the launched app reads back its own data"
    );

    // Tearing the session down closes the door (denies-closed afterwards).
    exec.invalidate();
    assert!(exec.app_data_get("draft/1".into()).is_err(), "closed session denies");

    eprintln!("import → trust → launch → bridge round-trip: OK");
}
