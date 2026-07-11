use std::fs;
use std::path::PathBuf;
use std::process::Command;

use riot_app_cli::{
    inspect, keygen, load_author, pack, InspectError, KeyError, PackError, PackInput,
};
use riot_core::apps::bundle::{decode_app_bundle, MAX_BUNDLE_TOTAL_BYTES};
use riot_core::apps::index::scan_app_index;
use riot_core::apps::manifest::decode_manifest;
use riot_core::import::{decode_bundle, encode_bundle, BundleDecodeOutcome, BUNDLE_MAGIC};
use riot_core::session::{CommitOutcome, ImportContext, RiotSession};
use riot_core::willow::{generate_communal_author, SignedWillowEntry};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app")
}

fn copy_fixture() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for name in ["riot-app.json", "index.html", "app.js"] {
        fs::copy(fixture().join(name), tmp.path().join(name)).unwrap();
    }
    tmp
}

fn pack_fixture(timestamp_micros: u64) -> riot_app_cli::PackOutput {
    let author = generate_communal_author().unwrap();
    pack(PackInput {
        app_dir: &fixture(),
        author: &author,
        timestamp_micros,
    })
    .unwrap()
}

#[test]
fn fixture_pack_is_canonical_stable_and_importable() {
    let author = generate_communal_author().unwrap();
    let first = pack(PackInput {
        app_dir: &fixture(),
        author: &author,
        timestamp_micros: 10,
    })
    .unwrap();
    let second = pack(PackInput {
        app_dir: &fixture(),
        author: &author,
        timestamp_micros: 20,
    })
    .unwrap();

    assert_eq!(first.app_id, second.app_id);
    assert_eq!(
        &first.import_bundle_bytes[..BUNDLE_MAGIC.len()],
        BUNDLE_MAGIC
    );
    let manifest = decode_manifest(&first.manifest_bytes).unwrap();
    let bundle = decode_app_bundle(&first.bundle_bytes).unwrap();
    assert_eq!(manifest.author, author.identity());
    assert_eq!(bundle.entry_point, "index.html");
    assert_eq!(
        bundle
            .resources
            .iter()
            .map(|r| (r.path.as_str(), r.content_type.as_str()))
            .collect::<Vec<_>>(),
        vec![("app.js", "text/javascript"), ("index.html", "text/html")]
    );

    let report = inspect(&first.import_bundle_bytes).unwrap();
    assert_eq!(report.app_id, first.app_id);
    assert_eq!(report.author, author.identity());
    assert_eq!(report.resources, vec!["app.js", "index.html"]);

    let session = RiotSession::open().unwrap();
    let store = session.create_store().unwrap();
    let preview = store
        .inspect(
            &first.import_bundle_bytes,
            ImportContext::new("riot-app-cli integration"),
        )
        .unwrap()
        .expect_preview();
    assert!(matches!(
        preview.plan_all().unwrap().commit().unwrap(),
        CommitOutcome::Committed(_)
    ));
    let scanned = scan_app_index(&store).unwrap();
    assert_eq!(scanned.apps.len(), 1);
    assert_eq!(scanned.apps[0].app_id, first.app_id);
}

#[test]
fn validates_manifest_shape_and_resources() {
    let cases = [
        (
            r#"{"name":"x","description":"d","version":"1","entry_point":"index.html","permissions":[],"extra":1}"#,
            "unknown field",
        ),
        (
            r#"{"description":"d","version":"1","entry_point":"index.html","permissions":[]}"#,
            "missing field",
        ),
        (
            r#"{"name":1,"description":"d","version":"1","entry_point":"index.html","permissions":[]}"#,
            "must be a string",
        ),
        (
            r#"{"name":"x","description":"d","version":"1","entry_point":"index.html","permissions":"app-data"}"#,
            "must be an array",
        ),
        (
            r#"{"name":"x","description":"d","version":"1","entry_point":"index.html","permissions":[1]}"#,
            "must be a string",
        ),
        (
            r#"{"name":"x","description":"d","version":"1","entry_point":"index.html","permissions":[],"author":"injected"}"#,
            "unknown field",
        ),
    ];
    for (json, expected) in cases {
        let tmp = copy_fixture();
        fs::write(tmp.path().join("riot-app.json"), json).unwrap();
        let author = generate_communal_author().unwrap();
        let error = pack(PackInput {
            app_dir: tmp.path(),
            author: &author,
            timestamp_micros: 1,
        })
        .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }

    let tmp = copy_fixture();
    fs::write(tmp.path().join("secret.exe"), b"x").unwrap();
    let author = generate_communal_author().unwrap();
    assert!(matches!(
        pack(PackInput { app_dir: tmp.path(), author: &author, timestamp_micros: 1 }),
        Err(PackError::UnsupportedResource { path }) if path == "secret.exe"
    ));

    let tmp = copy_fixture();
    fs::remove_file(tmp.path().join("index.html")).unwrap();
    let author = generate_communal_author().unwrap();
    assert!(matches!(
        pack(PackInput { app_dir: tmp.path(), author: &author, timestamp_micros: 1 }),
        Err(PackError::MissingEntryPoint { entry_point }) if entry_point == "index.html"
    ));
}

#[test]
fn rejects_duplicate_manifest_keys_without_last_wins_parsing() {
    let cases = [
        (
            r#"{"name":"first","name":"second","description":"d","version":"1","entry_point":"index.html","permissions":[]}"#,
            "duplicate field 'name'",
        ),
        (
            r#"{"name":"x","description":"d","version":"1","entry_point":"index.html","permissions":[],"author":"first","author":"second"}"#,
            "unknown field 'author'",
        ),
        (
            r#"{"name":"x","description":"d","version":"1","entry_point":"index.html","permissions":[],"extra":1,"name":"last"}"#,
            "unknown field 'extra'",
        ),
    ];
    for (json, expected) in cases {
        let tmp = copy_fixture();
        fs::write(tmp.path().join("riot-app.json"), json).unwrap();
        let author = generate_communal_author().unwrap();
        assert!(matches!(
            pack(PackInput {
                app_dir: tmp.path(),
                author: &author,
                timestamp_micros: 1,
            }),
            Err(PackError::ManifestJsonInvalid { reason }) if reason.contains(expected)
        ));
    }
}

#[test]
fn nested_paths_are_normalized_sorted_and_size_is_precise() {
    let tmp = copy_fixture();
    fs::create_dir(tmp.path().join("z")).unwrap();
    fs::create_dir(tmp.path().join("a")).unwrap();
    fs::write(tmp.path().join("z/b.css"), b"b{}").unwrap();
    fs::write(tmp.path().join("a/data.json"), b"{}").unwrap();
    fs::write(tmp.path().join("pixel.png"), b"png").unwrap();
    fs::write(tmp.path().join("shape.svg"), b"<svg/>").unwrap();
    let author = generate_communal_author().unwrap();
    let output = pack(PackInput {
        app_dir: tmp.path(),
        author: &author,
        timestamp_micros: 1,
    })
    .unwrap();
    let paths = decode_app_bundle(&output.bundle_bytes)
        .unwrap()
        .resources
        .into_iter()
        .map(|r| r.path)
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "a/data.json",
            "app.js",
            "index.html",
            "pixel.png",
            "shape.svg",
            "z/b.css"
        ]
    );

    let tmp = copy_fixture();
    let actual = MAX_BUNDLE_TOTAL_BYTES + 1;
    fs::write(tmp.path().join("huge.png"), vec![0; actual]).unwrap();
    let author = generate_communal_author().unwrap();
    assert!(matches!(
        pack(PackInput { app_dir: tmp.path(), author: &author, timestamp_micros: 1 }),
        Err(PackError::TooLarge { actual: got, limit }) if got == actual + fs::metadata(tmp.path().join("app.js")).unwrap().len() as usize + fs::metadata(tmp.path().join("index.html")).unwrap().len() as usize && limit == MAX_BUNDLE_TOTAL_BYTES
    ));
}

#[cfg(unix)]
#[test]
fn rejects_symlinks() {
    use std::os::unix::fs::symlink;
    let tmp = copy_fixture();
    symlink(
        tmp.path().join("index.html"),
        tmp.path().join("linked.html"),
    )
    .unwrap();
    let author = generate_communal_author().unwrap();
    assert!(matches!(
        pack(PackInput { app_dir: tmp.path(), author: &author, timestamp_micros: 1 }),
        Err(PackError::Symlink { path }) if path == "linked.html"
    ));
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_and_non_portable_resource_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let tmp = copy_fixture();
    fs::write(tmp.path().join("bad\\name.js"), b"bad").unwrap();
    let author = generate_communal_author().unwrap();
    assert!(matches!(
        pack(PackInput {
            app_dir: tmp.path(),
            author: &author,
            timestamp_micros: 1
        }),
        Err(PackError::InvalidResourcePath { .. })
    ));

    let tmp = copy_fixture();
    let bad_name = OsString::from_vec(vec![b'b', b'a', b'd', 0xff, b'.', b'j', b's']);
    if fs::write(tmp.path().join(bad_name), b"bad").is_err() {
        return; // This Unix filesystem itself rejects non-UTF-8 names.
    }
    let author = generate_communal_author().unwrap();
    assert!(matches!(
        pack(PackInput {
            app_dir: tmp.path(),
            author: &author,
            timestamp_micros: 1
        }),
        Err(PackError::InvalidResourcePath { .. })
    ));
}

#[test]
fn inspect_rejects_partial_and_tampered_artifacts() {
    let output = pack_fixture(7);
    assert!(matches!(
        inspect(&output.import_bundle_bytes[..20]),
        Err(InspectError::InvalidImportBundle { .. })
    ));
    let mut tampered = output.import_bundle_bytes;
    *tampered.last_mut().unwrap() ^= 1;
    assert!(inspect(&tampered).is_err());

    let author = generate_communal_author().unwrap();
    let first = pack(PackInput {
        app_dir: &fixture(),
        author: &author,
        timestamp_micros: 7,
    })
    .unwrap();
    let other_dir = copy_fixture();
    fs::write(other_dir.path().join("app.js"), b"different").unwrap();
    let second = pack(PackInput {
        app_dir: other_dir.path(),
        author: &author,
        timestamp_micros: 7,
    })
    .unwrap();
    let first_items = signed_items(&first.import_bundle_bytes);
    let second_items = signed_items(&second.import_bundle_bytes);
    let manifest_entry = first_items
        .into_iter()
        .find(|item| decode_manifest(&item.payload_bytes).is_ok())
        .unwrap();
    let bundle_entry = second_items
        .into_iter()
        .find(|item| decode_app_bundle(&item.payload_bytes).is_ok())
        .unwrap();
    let mismatched = encode_bundle(&[manifest_entry, bundle_entry]).unwrap();
    assert!(matches!(
        inspect(&mismatched),
        Err(InspectError::IncoherentPair { .. })
    ));
}

fn signed_items(bytes: &[u8]) -> Vec<SignedWillowEntry> {
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(bytes) else {
        panic!("packed bundle did not decode")
    };
    decoded
        .items
        .into_iter()
        .map(|item| SignedWillowEntry {
            entry_bytes: item.frame.entry_bytes().to_vec(),
            capability_bytes: item.frame.capability_bytes().to_vec(),
            signature: item.frame.signature_bytes().try_into().unwrap(),
            payload_bytes: item.frame.payload_bytes().to_vec(),
        })
        .collect()
}

#[test]
fn key_files_round_trip_are_private_and_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let key_dir = tmp.path().join("keys");
    let generated = keygen(&key_dir).unwrap();
    assert_eq!(
        load_author(&key_dir).unwrap().identity(),
        generated.identity
    );
    assert!(matches!(
        keygen(&key_dir),
        Err(KeyError::AlreadyExists { .. })
    ));
    assert!(generated.warning.contains("anyone"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in ["author.wrapkey", "author.sealed"] {
            assert_eq!(
                fs::metadata(key_dir.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    let key_path = key_dir.join("author.wrapkey");
    let original_key = fs::read(&key_path).unwrap();
    for bad in [b"abc".as_slice(), &[b'g'; 64][..]] {
        fs::write(&key_path, bad).unwrap();
        let error = match load_author(&key_dir) {
            Ok(_) => panic!("bad wrapping key accepted"),
            Err(error) => error.to_string(),
        };
        assert!(!error.contains("abc"));
        assert!(!error.contains(String::from_utf8_lossy(bad).as_ref()));
    }
    fs::write(&key_path, original_key).unwrap();
    let wrong_key = vec![b'0'; 64];
    fs::write(&key_path, &wrong_key).unwrap();
    assert!(matches!(
        load_author(&key_dir),
        Err(KeyError::InvalidSealedIdentity)
    ));
    let key_bytes = fs::read(key_dir.join("author.wrapkey")).unwrap();
    assert_eq!(key_bytes, wrong_key);

    // Restore the original valid key before testing authenticated ciphertext tampering.
    let generated_again = tempfile::tempdir().unwrap();
    let generated_again_dir = generated_again.path().join("keys");
    keygen(&generated_again_dir).unwrap();
    let unrelated_valid_key = fs::read(generated_again_dir.join("author.wrapkey")).unwrap();
    fs::write(&key_path, unrelated_valid_key).unwrap();
    assert!(matches!(
        load_author(&key_dir),
        Err(KeyError::InvalidSealedIdentity)
    ));

    // Regenerate a coherent pair for the sealed-file tamper assertion.
    let coherent = tempfile::tempdir().unwrap();
    let coherent_dir = coherent.path().join("keys");
    keygen(&coherent_dir).unwrap();
    let sealed_path = coherent_dir.join("author.sealed");
    let mut sealed = fs::read(&sealed_path).unwrap();
    *sealed.last_mut().unwrap() ^= 1;
    fs::write(sealed_path, sealed).unwrap();
    assert!(matches!(
        load_author(&coherent_dir),
        Err(KeyError::InvalidSealedIdentity)
    ));
}

#[test]
fn keygen_requires_the_destination_directory_not_to_exist() {
    let tmp = tempfile::tempdir().unwrap();
    let existing = tmp.path().join("existing");
    fs::create_dir(&existing).unwrap();
    assert!(matches!(
        keygen(&existing),
        Err(KeyError::AlreadyExists { .. })
    ));
    assert_eq!(fs::read_dir(&existing).unwrap().count(), 0);

    let existing_file = tmp.path().join("existing-file");
    fs::write(&existing_file, b"reserved").unwrap();
    assert!(matches!(
        keygen(&existing_file),
        Err(KeyError::AlreadyExists { .. })
    ));
    assert_eq!(fs::read(&existing_file).unwrap(), b"reserved");
}

#[test]
fn command_line_keygen_pack_and_inspect_smoke() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = tmp.path().join("keys");
    let artifact = tmp.path().join("hello.riot");
    let binary = env!("CARGO_BIN_EXE_riot-app");
    assert!(Command::new(binary)
        .args(["keygen", "--out"])
        .arg(&keys)
        .status()
        .unwrap()
        .success());
    assert!(Command::new(binary)
        .arg("pack")
        .arg(fixture())
        .arg("--key-dir")
        .arg(&keys)
        .arg("--out")
        .arg(&artifact)
        .args(["--timestamp-micros", "123"])
        .status()
        .unwrap()
        .success());
    let inspected = Command::new(binary)
        .arg("inspect")
        .arg(&artifact)
        .output()
        .unwrap();
    assert!(inspected.status.success());
    let stdout = String::from_utf8(inspected.stdout).unwrap();
    assert!(stdout.contains("name: Hello Riot"));
    assert!(stdout.contains("author signing key id: "));
    assert!(stdout.contains("app id: "));
    assert!(stdout.contains("  app.js"));
    assert!(stdout.contains("  index.html"));
}
