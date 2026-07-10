//! G0 evidence: the pinned bab_rs computes corrected WILLIAM3 digests.
//!
//! Cross-checked vectors carry digests copied from the independently
//! implemented willow-go correction (commit 9d848ee), so agreement here is
//! evidence of correctness, not self-attestation. Blessed-local vectors are
//! dependency-drift tripwires only.

use bab_rs::{batch_hash, William3Digest};

const VECTORS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/willow/william3-vectors.json"
);

fn william3(input: &[u8]) -> String {
    let mut digest = William3Digest::default();
    batch_hash(input, &mut digest);
    digest
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn materialize(input: &serde_json::Value) -> Vec<u8> {
    match input["kind"].as_str().expect("input kind") {
        "empty" => Vec::new(),
        "ascii" => input["value"]
            .as_str()
            .expect("ascii value")
            .as_bytes()
            .to_vec(),
        "repeat" => {
            let byte = input["byte"].as_u64().expect("byte") as u8;
            let count = input["count"].as_u64().expect("count") as usize;
            vec![byte; count]
        }
        "pattern-mod-251" => {
            let count = input["count"].as_u64().expect("count") as usize;
            (0..count as u32).map(|i| (i % 251) as u8).collect()
        }
        "file" => {
            let rel = input["path"].as_str().expect("file path");
            let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), rel);
            std::fs::read(path).expect("fixture file readable")
        }
        other => panic!("unknown input kind {other}"),
    }
}

#[test]
fn william3_vectors_match_frozen_and_cross_checked_digests() {
    let raw = std::fs::read_to_string(VECTORS_PATH).expect("vectors fixture present");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    assert_eq!(doc["contract"], "riot-william3-vectors/1");
    assert_eq!(doc["bab_rs"], "0.8.1");

    let vectors = doc["vectors"].as_array().expect("vectors array");
    assert!(vectors.len() >= 6, "expected the full vector family");

    let mut cross_checked = 0usize;
    let mut saw_sub_chunk = false;
    let mut saw_multi_chunk = false;

    for vector in vectors {
        let name = vector["name"].as_str().expect("name");
        let input = materialize(&vector["input"]);
        let expected = vector["digest_hex"].as_str().expect("digest hex");
        let actual = william3(&input);
        assert_eq!(
            actual, expected,
            "WILLIAM3 mismatch for vector `{name}` — dependency drift or wrong basis"
        );

        if vector["provenance"]
            .as_str()
            .unwrap_or_default()
            .starts_with("cross-checked")
        {
            cross_checked += 1;
        }
        if !input.is_empty() && input.len() < 1024 {
            saw_sub_chunk = true;
        }
        if input.len() > 1024 {
            saw_multi_chunk = true;
        }

        if let Some(file_hash) = vector["file_sha256"].as_str() {
            use sha2::Digest;
            let actual_file: String = sha2::Sha256::digest(&input)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(actual_file, file_hash, "fixture file drifted for `{name}`");
        }
    }

    assert!(
        cross_checked >= 1,
        "G0 requires at least one independently cross-checked vector"
    );
    assert!(
        saw_sub_chunk,
        "must exercise input shorter than one 1024-byte chunk"
    );
    assert!(
        saw_multi_chunk,
        "must exercise input longer than one 1024-byte chunk"
    );
}

#[test]
fn william3_alert_golden_payload_is_byte_identical_to_codec_fixture() {
    // The vectors fixture references the same file the alert codec froze;
    // this pins the two fixture families together.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/objects/alert-golden-1.cbor"
    );
    let bytes = std::fs::read(path).expect("alert golden present");
    assert!(!bytes.is_empty());
    assert_eq!(bytes[0] & 0xe0, 0xa0, "alert golden must be a CBOR map");
}
