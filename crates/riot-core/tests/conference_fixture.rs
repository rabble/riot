//! Conference demo fixture contract: a fixed public package, not executable
//! content or a private-sync format.

use std::collections::BTreeSet;
use std::path::PathBuf;

use minicbor::Encoder;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

const TITLE: &str = "Harbor District Evacuation";
const RENDERER_PROFILE: &str = "incident-board/1";
const ALLOWED_KINDS: [&str; 5] = ["alert", "observation", "resource", "request", "offer"];

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/conference")
        .join(name)
}

fn read_json(name: &str) -> Value {
    let path = fixture_path(name);
    serde_json::from_slice(&std::fs::read(&path).expect("conference fixture must exist"))
        .unwrap_or_else(|error| panic!("{} must contain valid JSON: {error}", path.display()))
}

fn object<'a>(value: &'a Value, label: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{label} must be a JSON object"))
}

fn array<'a>(value: &'a Value, label: &str) -> &'a [Value] {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{label} must be a JSON array"))
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> &'a str {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{key} must be a string"))
}

fn bool_value(object: &Map<String, Value>, key: &str) -> bool {
    object
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("{key} must be a boolean"))
}

fn expect_exact_keys(object: &Map<String, Value>, expected: &[&str], label: &str) {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "{label} must not grow an implicit surface"
    );
}

fn decode_hex(value: &str, expected_len: usize, label: &str) -> Vec<u8> {
    assert_eq!(
        value.len(),
        expected_len * 2,
        "{label} must be a full {expected_len}-byte hexadecimal identifier"
    );
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{label} must be hexadecimal"
    );
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("validated hex"))
        .collect()
}

fn decode_hex_any_length(value: &str, label: &str) -> Vec<u8> {
    assert_eq!(
        value.len() % 2,
        0,
        "{label} must be an even-length hexadecimal string"
    );
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{label} must be hexadecimal"
    );
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("validated hex"))
        .collect()
}

fn is_normalized_site_route(route: &str) -> bool {
    let Some(path) = route.strip_prefix("/site/") else {
        return false;
    };

    !path.is_empty()
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn reject_private_or_executable_surface(value: &Value, path: &str) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let lower = key.to_ascii_lowercase();
                assert!(
                    !lower.contains("private")
                        && !lower.contains("group")
                        && !lower.contains("secret"),
                    "{path}.{key} crosses the public fixture boundary"
                );
                reject_private_or_executable_surface(child, &format!("{path}.{key}"));
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                reject_private_or_executable_surface(child, &format!("{path}[{index}]"));
            }
        }
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            assert!(
                !lower.contains("javascript:")
                    && !lower.contains("<script")
                    && !lower.contains("http://")
                    && !lower.contains("https://"),
                "{path} must not contain executable or remote content"
            );
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn canonical_fixture_bytes(fixture: &Value) -> Vec<u8> {
    let root = object(fixture, "fixture");
    let namespace = object(root.get("namespace").expect("namespace"), "namespace");
    let incident = object(root.get("incident").expect("incident"), "incident");
    let authors = array(root.get("authors").expect("authors"), "authors");
    let entries = array(root.get("entries").expect("entries"), "entries");
    let routes = array(
        incident.get("rendered_routes").expect("rendered_routes"),
        "rendered_routes",
    );

    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    let result: Result<_, minicbor::encode::Error<core::convert::Infallible>> = (|| {
        encoder.map(6)?;
        encoder.u8(0)?.str(string(root, "schema"))?;
        encoder
            .u8(1)?
            .bytes(&decode_hex(string(namespace, "id"), 32, "namespace.id"))?;
        encoder.u8(2)?.str(string(incident, "title"))?;
        encoder.u8(3)?.array(authors.len() as u64)?;
        for author in authors {
            let author = object(author, "author");
            encoder.map(3)?;
            encoder.u8(0)?.bytes(&decode_hex(
                string(author, "nostr_pubkey"),
                32,
                "author.nostr_pubkey",
            ))?;
            encoder.u8(1)?.bytes(&decode_hex(
                string(author, "willow_subspace_id"),
                32,
                "author.willow_subspace_id",
            ))?;
            encoder.u8(2)?.str(string(author, "display_name"))?;
        }
        encoder.u8(4)?.array(entries.len() as u64)?;
        for entry in entries {
            let entry = object(entry, "entry");
            encoder.map(10)?;
            encoder.u8(0)?.str(string(entry, "kind"))?;
            encoder.u8(1)?.bytes(&decode_hex(
                string(entry, "willow_entry_id"),
                32,
                "entry.willow_entry_id",
            ))?;
            encoder.u8(2)?.bytes(&decode_hex(
                string(entry, "author_nostr_pubkey"),
                32,
                "entry.author_nostr_pubkey",
            ))?;
            encoder.u8(3)?.str(string(entry, "title"))?;
            encoder.u8(4)?.str(string(entry, "body"))?;
            encoder.u8(5)?.str(string(entry, "created_at"))?;
            encoder
                .u8(6)?
                .bool(bool_value(entry, "ai_assisted_draft"))?;
            encoder.u8(7)?.bytes(&decode_hex(
                string(entry, "signature"),
                64,
                "entry.signature",
            ))?;
            encoder.u8(8)?.bytes(&decode_hex_any_length(
                string(entry, "willow_entry_bytes"),
                "entry.willow_entry_bytes",
            ))?;
            encoder.u8(9)?.bytes(&decode_hex_any_length(
                string(entry, "willow_capability_bytes"),
                "entry.willow_capability_bytes",
            ))?;
        }
        encoder.u8(5)?.array(routes.len() as u64)?;
        for route in routes {
            encoder.str(route.as_str().expect("route must be a string"))?;
        }
        Ok(())
    })();
    result.expect("bounded fixture values must encode as CBOR");
    bytes
}

#[test]
fn conference_fixture_freezes_a_public_deterministic_incident_package() {
    let fixture = read_json("incident-space-v1.json");
    let root = object(&fixture, "fixture");
    expect_exact_keys(
        root,
        &[
            "schema",
            "namespace",
            "incident",
            "authors",
            "entries",
            "canonical_sha256",
        ],
        "fixture",
    );
    assert_eq!(string(root, "schema"), "riot-conference-incident-space/1");
    reject_private_or_executable_surface(&fixture, "fixture");

    let namespace = object(root.get("namespace").expect("namespace"), "namespace");
    expect_exact_keys(namespace, &["id", "visibility"], "namespace");
    assert_eq!(string(namespace, "visibility"), "public");
    decode_hex(string(namespace, "id"), 32, "namespace.id");

    let incident = object(root.get("incident").expect("incident"), "incident");
    expect_exact_keys(incident, &["title", "rendered_routes"], "incident");
    assert_eq!(string(incident, "title"), TITLE);
    let routes = array(
        incident.get("rendered_routes").expect("rendered_routes"),
        "rendered_routes",
    );
    assert!(
        !routes.is_empty(),
        "the fixed renderer needs at least one route"
    );
    for route in routes {
        let route = route.as_str().expect("route must be a string");
        assert!(
            is_normalized_site_route(route),
            "route must be a normalized safe path below /site/: {route}"
        );
    }

    let authors = array(root.get("authors").expect("authors"), "authors");
    assert_eq!(authors.len(), 2, "fixture must name exactly two authors");
    let mut author_ids = BTreeSet::new();
    for author in authors {
        let author = object(author, "author");
        expect_exact_keys(
            author,
            &["display_name", "nostr_pubkey", "willow_subspace_id"],
            "author",
        );
        let nostr = decode_hex(string(author, "nostr_pubkey"), 32, "author.nostr_pubkey");
        decode_hex(
            string(author, "willow_subspace_id"),
            32,
            "author.willow_subspace_id",
        );
        assert!(
            author_ids.insert(nostr),
            "authors must be distinct full identifiers"
        );
    }

    let entries = array(root.get("entries").expect("entries"), "entries");
    assert!(
        !entries.is_empty(),
        "fixture must include public package-shape entries"
    );
    let mut ai_assisted_drafts = 0;
    for entry in entries {
        let entry = object(entry, "entry");
        expect_exact_keys(
            entry,
            &[
                "kind",
                "willow_entry_id",
                "author_nostr_pubkey",
                "title",
                "body",
                "created_at",
                "ai_assisted_draft",
                "signature",
                "willow_entry_bytes",
                "willow_capability_bytes",
            ],
            "entry",
        );
        assert!(ALLOWED_KINDS.contains(&string(entry, "kind")));
        decode_hex(
            string(entry, "willow_entry_id"),
            32,
            "entry.willow_entry_id",
        );
        let author = decode_hex(
            string(entry, "author_nostr_pubkey"),
            32,
            "entry.author_nostr_pubkey",
        );
        assert!(
            author_ids.contains(&author),
            "entry author must be one of the two fixture authors"
        );
        decode_hex(string(entry, "signature"), 64, "entry.signature");
        decode_hex_any_length(string(entry, "willow_entry_bytes"), "entry.willow_entry_bytes");
        decode_hex_any_length(
            string(entry, "willow_capability_bytes"),
            "entry.willow_capability_bytes",
        );
        ai_assisted_drafts += usize::from(bool_value(entry, "ai_assisted_draft"));
    }
    assert_eq!(
        ai_assisted_drafts, 1,
        "exactly one entry is an AI-assisted draft"
    );

    let first = canonical_fixture_bytes(&fixture);
    let second = canonical_fixture_bytes(&fixture);
    assert_eq!(
        first, second,
        "canonical fixture bytes must be deterministic"
    );
    let actual_hash = format!("{:x}", Sha256::digest(first));
    assert_eq!(string(root, "canonical_sha256"), actual_hash);
}

#[test]
fn conference_routes_reject_unsafe_or_non_normalized_site_paths() {
    for route in [
        "/site/",
        "/site//alerts",
        "/site/../admin",
        "/site/./alerts",
        "/site/%2e%2e/admin",
        "/site/%2E%2E/admin",
        "/site/incident%2fadmin",
        "/site/incident%2Fadmin",
        "/site/%252e%252e/admin",
        "/site/alerts?next=/admin",
        "/site/alerts#admin",
        "/site/ALERTS",
    ] {
        assert!(
            !is_normalized_site_route(route),
            "unsafe or non-normalized route must be rejected: {route}"
        );
    }

    for route in ["/site/incident-board", "/site/incident-board/alerts"] {
        assert!(
            is_normalized_site_route(route),
            "normalized route must remain accepted: {route}"
        );
    }
}

#[test]
fn conference_manifest_is_fixed_public_and_non_executable() {
    let manifest = read_json("package-manifest-v1.json");
    let root = object(&manifest, "manifest");
    expect_exact_keys(
        root,
        &[
            "schema",
            "renderer_profile",
            "namespace",
            "title",
            "allowed_object_kinds",
        ],
        "manifest",
    );
    reject_private_or_executable_surface(&manifest, "manifest");
    assert_eq!(string(root, "schema"), "riot-conference-package/1");
    assert_eq!(string(root, "renderer_profile"), RENDERER_PROFILE);
    assert_eq!(string(root, "title"), TITLE);
    decode_hex(string(root, "namespace"), 32, "manifest.namespace");
    let fixture = read_json("incident-space-v1.json");
    let fixture_root = object(&fixture, "fixture");
    let fixture_namespace = object(
        fixture_root.get("namespace").expect("namespace"),
        "fixture.namespace",
    );
    assert_eq!(
        string(root, "namespace"),
        string(fixture_namespace, "id"),
        "manifest namespace must exactly match the incident fixture's public namespace"
    );
    let kinds = array(
        root.get("allowed_object_kinds")
            .expect("allowed_object_kinds"),
        "allowed_object_kinds",
    );
    assert_eq!(kinds.len(), ALLOWED_KINDS.len());
    for (actual, expected) in kinds.iter().zip(ALLOWED_KINDS) {
        assert_eq!(actual.as_str(), Some(expected));
    }
}
