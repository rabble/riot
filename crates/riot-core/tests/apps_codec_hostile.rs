//! Hostile-corpus evidence for the apps codecs. `decode_manifest` and
//! `decode_app_bundle` parse bytes that arrive from other people's devices
//! (app installs travel over sync and side-channels), so they get the same
//! adversarial treatment Phase 0A gave the evidence bundle codec: truncation
//! sweeps, exhaustive byte-flip sweeps pinned to the canonicality guarantee,
//! trailing garbage, forged CBOR count headers (no allocation-before-bounds),
//! and deterministic random garbage. Every case must return `Err` or — for a
//! flip that happens to produce a *different valid canonical document* —
//! re-encode to exactly the mutated input. Panics fail the test by
//! definition.

use riot_core::apps::bundle::{
    decode_app_bundle, encode_app_bundle, AppBundle, AppResource, MAX_BUNDLE_TOTAL_BYTES,
};
use riot_core::apps::manifest::{
    decode_manifest, encode_manifest, AppManifest, MAX_MANIFEST_BYTES,
};
use riot_core::willow::generate_communal_author;

fn sample_manifest_bytes() -> Vec<u8> {
    let author = generate_communal_author().expect("author");
    let manifest = AppManifest {
        name: "Checklist".to_string(),
        description: "Lets people add and check off shared to-dos.".to_string(),
        version: "1.0.0".to_string(),
        author: author.identity(),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    };
    encode_manifest(&manifest).expect("encode manifest")
}

fn sample_bundle_bytes() -> Vec<u8> {
    let bundle = AppBundle {
        entry_point: "index.html".to_string(),
        resources: vec![
            AppResource {
                path: "index.html".to_string(),
                content_type: "text/html".to_string(),
                bytes: b"<html>checklist</html>".to_vec(),
            },
            AppResource {
                path: "app.js".to_string(),
                content_type: "text/javascript".to_string(),
                bytes: b"console.log('hi')".to_vec(),
            },
        ],
    };
    encode_app_bundle(&bundle).expect("encode bundle")
}

/// Deterministic xorshift64* — no new dev-dependency, reproducible corpus.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn fill(&mut self, buffer: &mut [u8]) {
        for chunk in buffer.chunks_mut(8) {
            let bytes = self.next().to_le_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&bytes[..len]);
        }
    }
}

#[test]
fn truncated_manifest_never_decodes() {
    let bytes = sample_manifest_bytes();
    for len in 0..bytes.len() {
        assert!(
            decode_manifest(&bytes[..len]).is_err(),
            "prefix of length {len} decoded"
        );
    }
}

#[test]
fn truncated_bundle_never_decodes() {
    let bytes = sample_bundle_bytes();
    for len in 0..bytes.len() {
        assert!(
            decode_app_bundle(&bytes[..len]).is_err(),
            "prefix of length {len} decoded"
        );
    }
}

#[test]
fn every_manifest_byte_flip_is_rejected_or_stays_canonical() {
    let bytes = sample_manifest_bytes();
    for position in 0..bytes.len() {
        for mask in [0xffu8, 0x01] {
            let mut mutated = bytes.clone();
            mutated[position] ^= mask;
            if let Ok(decoded) = decode_manifest(&mutated) {
                // A flip inside string content can yield a different but
                // still-valid document; the canonicality guarantee then
                // demands the accepted bytes ARE its canonical encoding.
                let reencoded = encode_manifest(&decoded).expect("accepted doc re-encodes");
                assert_eq!(
                    reencoded, mutated,
                    "byte {position} flip {mask:#x} accepted non-canonical bytes"
                );
            }
        }
    }
}

#[test]
fn every_bundle_byte_flip_is_rejected_or_stays_canonical() {
    let bytes = sample_bundle_bytes();
    for position in 0..bytes.len() {
        for mask in [0xffu8, 0x01] {
            let mut mutated = bytes.clone();
            mutated[position] ^= mask;
            if let Ok(decoded) = decode_app_bundle(&mutated) {
                let reencoded = encode_app_bundle(&decoded).expect("accepted doc re-encodes");
                assert_eq!(
                    reencoded, mutated,
                    "byte {position} flip {mask:#x} accepted non-canonical bytes"
                );
            }
        }
    }
}

#[test]
fn trailing_garbage_is_rejected() {
    let manifest = sample_manifest_bytes();
    let bundle = sample_bundle_bytes();
    for garbage in [&[0x00u8][..], b"junk"] {
        let mut padded_manifest = manifest.clone();
        padded_manifest.extend_from_slice(garbage);
        assert!(decode_manifest(&padded_manifest).is_err());

        let mut padded_bundle = bundle.clone();
        padded_bundle.extend_from_slice(garbage);
        assert!(decode_app_bundle(&padded_bundle).is_err());
    }
}

#[test]
fn forged_huge_count_headers_are_rejected_without_allocation() {
    // A hostile encoder can claim any collection size in a CBOR header
    // without supplying the elements. Both decoders must bounds-check the
    // claimed count before reserving memory. Headers below are hand-built:
    // 0xbf is indefinite map (non-canonical), 0x9b + 8 bytes is a 64-bit
    // array length, 0xba + 4 bytes a 32-bit map length.
    let indefinite_map = [0xbfu8];
    assert!(decode_manifest(&indefinite_map).is_err());
    assert!(decode_app_bundle(&indefinite_map).is_err());

    // Manifest: map(9), key 0 name, then a permissions-shaped huge array
    // would sit at key 7 — but the decoder must already reject the huge
    // top-level map claim itself.
    let mut huge_map = vec![0xbau8];
    huge_map.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode_manifest(&huge_map).is_err());
    assert!(decode_app_bundle(&huge_map).is_err());

    // Bundle: valid map(2) + entry_point, then a resources array claiming
    // 2^64-1 elements. Must fail on the count check, not by allocating.
    let mut forged_bundle = vec![
        0xa2, // map(2)
        0x00, // key 0
        0x6a, // text(10)
    ];
    forged_bundle.extend_from_slice(b"index.html");
    forged_bundle.push(0x01); // key 1
    forged_bundle.push(0x9b); // array, 64-bit length follows
    forged_bundle.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(decode_app_bundle(&forged_bundle).is_err());

    // Manifest twin: map(9) with keys 0..=6 valid, then permissions array
    // claiming 2^32 entries.
    let manifest = sample_manifest_bytes();
    // Find key 7's array header in the canonical bytes: key byte 0x07
    // followed by 0x81 (array(1)) in the sample. Splice a huge claim in.
    let marker = [0x07u8, 0x81];
    let position = manifest
        .windows(2)
        .position(|w| w == marker)
        .expect("sample manifest contains permissions array header");
    let mut forged_manifest = manifest[..position + 1].to_vec();
    forged_manifest.push(0x9a); // array, 32-bit length follows
    forged_manifest.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode_manifest(&forged_manifest).is_err());
}

#[test]
fn oversized_inputs_are_rejected_up_front() {
    let oversized_manifest = vec![0u8; MAX_MANIFEST_BYTES + 1];
    assert!(decode_manifest(&oversized_manifest).is_err());
    let oversized_bundle = vec![0u8; MAX_BUNDLE_TOTAL_BYTES + 1];
    assert!(decode_app_bundle(&oversized_bundle).is_err());
}

#[test]
fn deterministic_random_garbage_never_decodes() {
    let mut rng = Xorshift(0x5eed_5eed_5eed_5eed);
    for round in 0..512 {
        let len = (rng.next() % 2_048) as usize + 1;
        let mut buffer = vec![0u8; len];
        rng.fill(&mut buffer);
        assert!(
            decode_manifest(&buffer).is_err(),
            "garbage round {round} decoded as manifest"
        );
        assert!(
            decode_app_bundle(&buffer).is_err(),
            "garbage round {round} decoded as bundle"
        );
    }
}
