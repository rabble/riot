use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::directory::AppProvenance;
use riot_core::apps::manifest::{encode_manifest, AppManifest};
use riot_core::apps::starter::{verify_starter_catalog, STARTER_CATALOG};
use riot_core::willow::generate_communal_author;

fn pair(name: &str) -> (Vec<u8>, Vec<u8>) {
    let author = generate_communal_author().expect("author");
    let bundle_bytes = encode_app_bundle(&AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![AppResource {
            path: "index.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<html></html>".to_vec(),
        }],
    })
    .expect("bundle");
    let manifest_bytes = encode_manifest(&AppManifest {
        name: name.to_string(),
        description: "Built-in tool.".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    })
    .expect("manifest");
    (manifest_bytes, bundle_bytes)
}

#[test]
fn valid_pairs_verify_with_built_in_provenance_and_zero_timestamp() {
    let (m, b) = pair("Checklist");
    let apps = verify_starter_catalog(&[(&m, &b)]);
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].provenance, AppProvenance::BuiltIn);
    assert_eq!(apps[0].manifest_timestamp_micros, 0);
    assert!(apps[0].bundle_present);
}

#[test]
fn corrupted_built_in_is_silently_excluded() {
    let (m, b) = pair("Checklist");
    let mut corrupt = b.clone();
    // Flip the leading CBOR map-header byte, not the trailing byte: per
    // `apps_codec_hostile.rs`'s documented codec contract, a flip that
    // lands inside a resource's raw byte content ("rejected or stays
    // canonical") can decode as a *different, still-canonical* document —
    // that's the codec's tested, intended behavior, not something
    // `verify_starter_catalog` should be asked to catch. A flip of the
    // top-level framing byte, by contrast, is guaranteed to break
    // decoding regardless of content, which is what "corrupted" must mean
    // for this test to be meaningful.
    corrupt[0] ^= 0xFF;
    let apps = verify_starter_catalog(&[(&m, &corrupt)]);
    assert!(
        apps.is_empty(),
        "corrupt bundle must be excluded, not an error"
    );
}

#[test]
fn corrupt_manifest_is_silently_excluded() {
    let (mut manifest, bundle) = pair("Checklist");
    manifest[0] ^= 0xff;
    assert!(verify_starter_catalog(&[(&manifest, &bundle)]).is_empty());
}

#[test]
fn entry_point_mismatch_is_silently_excluded() {
    let (manifest, _) = pair("Checklist");
    let bundle = encode_app_bundle(&AppBundle {
        entry_point: "other.html".to_string(),
        resources: vec![AppResource {
            path: "other.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<html></html>".to_vec(),
        }],
    })
    .expect("bundle");
    assert!(verify_starter_catalog(&[(&manifest, &bundle)]).is_empty());
}

#[test]
fn the_shipped_catalog_verifies_completely() {
    // Guards the embedded catalog forever: every shipped pair must verify.
    let apps = verify_starter_catalog(STARTER_CATALOG);
    assert_eq!(apps.len(), STARTER_CATALOG.len());
}

/// The frozen app_id the checklist source + committed artifacts derive to.
/// Changing the source or repacking under new bytes changes this value —
/// which is correct: new bytes are a new trust decision.
const CHECKLIST_APP_ID_HEX: &str =
    "3fe5f89af18d9244756c8925750280f0c51479030cf3cd7b4d26940b51eaa4b7";

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

#[test]
fn shipped_catalog_contains_the_community_suite_in_demo_order() {
    let apps = verify_starter_catalog(STARTER_CATALOG);
    let names: Vec<&str> = apps.iter().map(|app| app.manifest.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Checklist",
            "Needs & Offers",
            "Events",
            "Decisions",
            "Chat",
            "Dispatches",
            "Wiki",
            "Photo Wall",
        ]
    );
    assert_eq!(apps[0].manifest.name, "Checklist");
    assert_eq!(apps[0].manifest.entry_point, "index.html");
    assert_eq!(to_hex(&apps[0].app_id), CHECKLIST_APP_ID_HEX);
}

/// Drift guard, key-free: every artifact must equal a fresh canonical encode
/// of its committed source directory and the frozen public built-in author.
#[test]
fn committed_artifacts_match_all_committed_sources() {
    use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
    use riot_core::apps::manifest::{decode_manifest, encode_manifest};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/apps");
    let frozen_manifest = decode_manifest(
        &std::fs::read(root.join("checklist.manifest.cbor")).expect("frozen manifest"),
    )
    .expect("decode frozen manifest");

    for slug in [
        "checklist",
        "supply-board",
        "roll-call",
        "quick-poll",
        "chat",
        "dispatches",
        "wiki",
        "photo-wall",
    ] {
        let dir = root.join(slug);
        let mut resources = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("read dir") {
            let entry = entry.expect("entry");
            if !entry.file_type().expect("file type").is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "riot-app.json" {
                continue;
            }
            let content_type = match name.rsplit_once('.').map(|(_, extension)| extension) {
                Some("html") => "text/html",
                Some("js") => "text/javascript",
                Some("css") => "text/css",
                Some("svg") => "image/svg+xml",
                Some("png") => "image/png",
                other => panic!("unsupported starter resource type: {name} ({other:?})"),
            };
            resources.push(AppResource {
                path: name,
                content_type: content_type.to_string(),
                bytes: std::fs::read(entry.path()).expect("read resource"),
            });
        }
        resources.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));

        let source: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("riot-app.json")).expect("read riot-app.json"),
        )
        .expect("parse riot-app.json");
        let entry_point = source["entry_point"].as_str().unwrap().to_string();
        let rebuilt_bundle = encode_app_bundle(&AppBundle {
            entry_point: entry_point.clone(),
            resources,
        })
        .expect("re-encode bundle");
        let committed_bundle =
            std::fs::read(root.join(format!("{slug}.bundle.cbor"))).expect("bundle artifact");
        assert_eq!(
            rebuilt_bundle, committed_bundle,
            "{slug} bundle drift — re-run scripts/apps/repack-starter.sh"
        );

        let rebuilt_manifest = encode_manifest(&AppManifest {
            name: source["name"].as_str().unwrap().to_string(),
            description: source["description"].as_str().unwrap().to_string(),
            version: source["version"].as_str().unwrap().to_string(),
            author: frozen_manifest.author.clone(),
            permissions: source["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|permission| permission.as_str().unwrap().to_string())
                .collect(),
            entry_point,
        })
        .expect("re-encode manifest");
        let committed_manifest =
            std::fs::read(root.join(format!("{slug}.manifest.cbor"))).expect("manifest artifact");
        assert_eq!(
            rebuilt_manifest, committed_manifest,
            "{slug} manifest drift — re-run scripts/apps/repack-starter.sh"
        );
    }
}
