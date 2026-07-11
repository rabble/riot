use std::fs;
use std::path::PathBuf;
use std::process::Command;

use ed25519_dalek::Signature;
use riot_app_cli::{
    inspect, keygen, load_author, pack, InspectError, KeyError, PackError, PackInput,
};
use riot_core::apps::bundle::{
    decode_app_bundle, AppBundle, AppResource, MAX_BUNDLE_RESOURCES, MAX_BUNDLE_TOTAL_BYTES,
};
use riot_core::apps::index::{
    app_bundle_digest, app_index_bundle_path, app_index_manifest_path, scan_app_index,
};
use riot_core::apps::manifest::{
    app_id_for, decode_manifest, encode_manifest, AppManifest, MAX_APP_PERMISSIONS,
    MAX_MANIFEST_BYTES,
};
use riot_core::import::{decode_bundle, encode_bundle, BundleDecodeOutcome, BUNDLE_MAGIC};
use riot_core::session::{CommitOutcome, ImportContext, RiotSession};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    EvidenceAuthor, NamespaceKind, Path as WillowPath, SignedWillowEntry,
};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-app")
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap()
}

fn copy_fixture() -> tempfile::TempDir {
    let tmp = tempdir();
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
fn bounds_manifest_and_permissions_before_materializing_untrusted_input() {
    let tmp = copy_fixture();
    fs::write(
        tmp.path().join("riot-app.json"),
        vec![b' '; MAX_MANIFEST_BYTES + 1],
    )
    .unwrap();
    let author = generate_communal_author().unwrap();
    assert!(
        matches!(pack(PackInput { app_dir: tmp.path(), author: &author, timestamp_micros: 1 }), Err(PackError::TooLarge { actual, limit }) if actual == MAX_MANIFEST_BYTES + 1 && limit == MAX_MANIFEST_BYTES)
    );

    let tmp = copy_fixture();
    let permissions = (0..=MAX_APP_PERMISSIONS)
        .map(|_| "\"x\"")
        .collect::<Vec<_>>()
        .join(",");
    fs::write(tmp.path().join("riot-app.json"), format!(r#"{{"name":"x","description":"d","version":"1","entry_point":"index.html","permissions":[{permissions}]}}"#)).unwrap();
    let author = generate_communal_author().unwrap();
    assert!(matches!(
        pack(PackInput {
            app_dir: tmp.path(),
            author: &author,
            timestamp_micros: 1
        }),
        Err(PackError::ManifestJsonInvalid { .. })
    ));
}

#[test]
fn enforces_resource_count_during_traversal() {
    let tmp = copy_fixture();
    for index in 0..MAX_BUNDLE_RESOURCES {
        fs::write(tmp.path().join(format!("extra-{index}.js")), b"x").unwrap();
    }
    let author = generate_communal_author().unwrap();
    assert!(
        matches!(pack(PackInput { app_dir: tmp.path(), author: &author, timestamp_micros: 1 }), Err(PackError::TooManyResources { actual, limit }) if actual == MAX_BUNDLE_RESOURCES + 1 && limit == MAX_BUNDLE_RESOURCES)
    );
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
        Err(PackError::TooLarge { actual: got, limit }) if got == actual + fs::metadata(tmp.path().join("app.js")).unwrap().len() as usize && limit == MAX_BUNDLE_TOTAL_BYTES
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

#[cfg(unix)]
#[test]
fn rejects_hidden_control_and_overdeep_paths() {
    let tmp = copy_fixture();
    fs::write(tmp.path().join(".hidden.js"), b"bad").unwrap();
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
    fs::write(tmp.path().join("bad\nname.js"), b"bad").unwrap();
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
    let mut directory = tmp.path().to_path_buf();
    for _ in 0..64 {
        directory.push("d");
        fs::create_dir(&directory).unwrap();
    }
    fs::write(directory.join("over.js"), b"bad").unwrap();
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

#[test]
fn inspect_rejects_spoofed_full_author_identity_and_mixed_timestamps() {
    let attacker = generate_communal_author().unwrap();
    let victim = generate_communal_author().unwrap();
    let bundle = AppBundle {
        entry_point: "index.html".into(),
        resources: vec![AppResource {
            path: "index.html".into(),
            content_type: "text/html".into(),
            bytes: b"ok".to_vec(),
        }],
    };
    let bundle_bytes = riot_core::apps::bundle::encode_app_bundle(&bundle).unwrap();
    let mut author = attacker.identity();
    author.namespace_id = victim.identity().namespace_id;
    author.signing_key_id = victim.identity().signing_key_id;
    author.namespace_kind = NamespaceKind::Communal;
    let manifest = AppManifest {
        name: "spoof".into(),
        description: "spoof".into(),
        version: "1".into(),
        author,
        permissions: vec![],
        entry_point: "index.html".into(),
    };
    let manifest_bytes = encode_manifest(&manifest).unwrap();
    let app_id = app_id_for(&manifest, &app_bundle_digest(&bundle_bytes)).unwrap();
    let spoofed = encode_bundle(&[
        signed_at(
            &attacker,
            app_index_manifest_path(&app_id).unwrap(),
            &manifest_bytes,
            7,
        ),
        signed_at(
            &attacker,
            app_index_bundle_path(&app_id).unwrap(),
            &bundle_bytes,
            7,
        ),
    ])
    .unwrap();
    assert!(matches!(
        inspect(&spoofed),
        Err(InspectError::IncoherentPair { .. })
    ));

    let legitimate = AppManifest {
        author: attacker.identity(),
        ..manifest
    };
    let manifest_bytes = encode_manifest(&legitimate).unwrap();
    let app_id = app_id_for(&legitimate, &app_bundle_digest(&bundle_bytes)).unwrap();
    let mixed_time = encode_bundle(&[
        signed_at(
            &attacker,
            app_index_manifest_path(&app_id).unwrap(),
            &manifest_bytes,
            7,
        ),
        signed_at(
            &attacker,
            app_index_bundle_path(&app_id).unwrap(),
            &bundle_bytes,
            8,
        ),
    ])
    .unwrap();
    assert!(matches!(
        inspect(&mixed_time),
        Err(InspectError::IncoherentPair { .. })
    ));
}

#[test]
fn inspect_and_cli_reject_terminal_control_fields_without_echoing_them() {
    let author = generate_communal_author().unwrap();
    let bundle = AppBundle {
        entry_point: "index.html".into(),
        resources: vec![AppResource {
            path: "index.html".into(),
            content_type: "text/html".into(),
            bytes: b"ok".to_vec(),
        }],
    };
    let bundle_bytes = riot_core::apps::bundle::encode_app_bundle(&bundle).unwrap();
    let manifest = AppManifest {
        name: "evil\u{1b}[31m".into(),
        description: "d".into(),
        version: "1".into(),
        author: author.identity(),
        permissions: vec![],
        entry_point: "index.html".into(),
    };
    let manifest_bytes = encode_manifest(&manifest).unwrap();
    let app_id = app_id_for(&manifest, &app_bundle_digest(&bundle_bytes)).unwrap();
    let artifact = encode_bundle(&[
        signed_at(
            &author,
            app_index_manifest_path(&app_id).unwrap(),
            &manifest_bytes,
            7,
        ),
        signed_at(
            &author,
            app_index_bundle_path(&app_id).unwrap(),
            &bundle_bytes,
            7,
        ),
    ])
    .unwrap();
    assert!(matches!(
        inspect(&artifact),
        Err(InspectError::IncoherentPair { .. })
    ));
    let tmp = tempdir();
    let path = tmp.path().join("hostile.riot");
    fs::write(&path, artifact).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_riot-app"))
        .arg("inspect")
        .arg(path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.contains(&0x1b));
    assert!(!output.stdout.contains(&0x1b));
}

#[test]
fn pack_errors_escape_hostile_json_keys_and_paths() {
    let tmp = copy_fixture();
    fs::write(tmp.path().join("riot-app.json"), r#"{"name":"x","description":"d","version":"1","entry_point":"index.html","permissions":[],"evil\u001bkey":1}"#).unwrap();
    let author = generate_communal_author().unwrap();
    let error = pack(PackInput {
        app_dir: tmp.path(),
        author: &author,
        timestamp_micros: 1,
    })
    .unwrap_err()
    .to_string();
    assert!(!error.contains('\u{1b}'));
    assert!(error.contains("\\u{1b}"));

    let tmp = copy_fixture();
    fs::write(tmp.path().join("bad\nname.js"), b"x").unwrap();
    let author = generate_communal_author().unwrap();
    let error = pack(PackInput {
        app_dir: tmp.path(),
        author: &author,
        timestamp_micros: 1,
    })
    .unwrap_err()
    .to_string();
    assert!(!error.contains('\n'));
    assert!(error.contains("\\n"));
}

fn signed_at(
    author: &EvidenceAuthor,
    path: WillowPath,
    payload: &[u8],
    timestamp: u64,
) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(timestamp)
        .payload(payload)
        .build();
    let authorised = authorise_entry(author, entry).unwrap();
    let token = authorised.authorisation_token();
    let signature: Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
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
    let tmp = tempdir();
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
    let generated_again = tempdir();
    let generated_again_dir = generated_again.path().join("keys");
    keygen(&generated_again_dir).unwrap();
    let unrelated_valid_key = fs::read(generated_again_dir.join("author.wrapkey")).unwrap();
    fs::write(&key_path, unrelated_valid_key).unwrap();
    assert!(matches!(
        load_author(&key_dir),
        Err(KeyError::InvalidSealedIdentity)
    ));

    // Regenerate a coherent pair for the sealed-file tamper assertion.
    let coherent = tempdir();
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

#[cfg(unix)]
#[test]
fn load_author_rejects_symlinks_unsafe_modes_and_oversized_files() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let parent = tempdir();
    let keys = parent.path().join("keys");
    keygen(&keys).unwrap();
    let wrap = keys.join("author.wrapkey");
    fs::set_permissions(&wrap, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(load_author(&keys), Err(KeyError::InvalidWrapKey)));
    fs::set_permissions(&wrap, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&wrap, vec![b'0'; 65]).unwrap();
    assert!(matches!(load_author(&keys), Err(KeyError::InvalidWrapKey)));

    fs::remove_file(&wrap).unwrap();
    let target = parent.path().join("target");
    fs::write(&target, vec![b'0'; 64]).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&target, &wrap).unwrap();
    assert!(load_author(&keys).is_err());

    let alias = parent.path().join("alias");
    symlink(parent.path(), &alias).unwrap();
    assert!(load_author(&alias.join("keys")).is_err());

    fs::remove_file(&wrap).unwrap();
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let fifo = CString::new(wrap.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(load_author(&keys).is_err());
}

#[test]
fn keygen_requires_the_destination_directory_not_to_exist() {
    let tmp = tempdir();
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
    let tmp = tempdir();
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
