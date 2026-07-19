//! WU-002 — canonical positional-CBOR codec and protocol identity digests.
//!
//! Hostile fixtures for the codec (maps, indefinite containers, non-minimal
//! integers, embedded-vs-byte substitution, swapped-length arrays, trailing
//! bytes, unknown versions, set ordering) and golden cross-checks for every
//! digest/preimage construction. The digest cross-checks build each preimage a
//! second, independent way inline and require agreement — so a framing bug in
//! src/digest.rs cannot hide behind a self-referential expected value.

use minicbor::{Decoder, Encoder};
use riot_anchor_protocol::codec::{self, CanonicalRecord, CodecError};
use riot_anchor_protocol::digest::{self, label};

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ---------------------------------------------------------------------------
// A representative canonical record: `[1, a, b_bytes, c?]`, where the version
// integer is 1, `a` is a u64, `b_bytes` is a byte string, and `c` is an optional
// u64 (null or the value). Exercises the codec primitives end to end.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Clone)]
struct Sample {
    a: u64,
    b: Vec<u8>,
    c: Option<u64>,
}

impl CanonicalRecord for Sample {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(4).unwrap();
        e.u64(1).unwrap();
        e.u64(self.a).unwrap();
        e.bytes(&self.b).unwrap();
        match self.c {
            Some(v) => {
                e.u64(v).unwrap();
            }
            None => {
                e.null().unwrap();
            }
        }
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        codec::expect_array(d, 4)?;
        codec::read_version(d, 1)?;
        let a = d.u64().map_err(|_| CodecError::Malformed)?;
        let b = codec::read_bytes_max(d, 1024)?;
        let c = if codec::peek_null(d)? {
            codec::read_null(d)?;
            None
        } else {
            Some(d.u64().map_err(|_| CodecError::Malformed)?)
        };
        Ok(Sample { a, b, c })
    }
}

fn sample_bytes(a: u64, b: &[u8], c: Option<u64>) -> Vec<u8> {
    Sample {
        a,
        b: b.to_vec(),
        c,
    }
    .encode_canonical()
    .unwrap()
}

// ---------------------------------------------------------------------------
// Codec: happy paths
// ---------------------------------------------------------------------------

#[test]
fn roundtrips_both_optional_states() {
    for sample in [
        Sample {
            a: 7,
            b: vec![1, 2, 3],
            c: Some(9),
        },
        Sample {
            a: 0,
            b: vec![],
            c: None,
        },
    ] {
        let bytes = sample.encode_canonical().unwrap();
        let decoded: Sample = codec::decode_canonical(&bytes, 1024).unwrap();
        assert_eq!(decoded, sample);
    }
}

// ---------------------------------------------------------------------------
// Codec: hostile fixtures
// ---------------------------------------------------------------------------

#[test]
fn rejects_input_over_the_record_limit() {
    let bytes = sample_bytes(7, &[1, 2, 3], Some(9));
    let err = codec::decode_canonical::<Sample>(&bytes, bytes.len() - 1).unwrap_err();
    assert_eq!(
        err,
        CodecError::TooLarge {
            limit: bytes.len() - 1,
            actual: bytes.len(),
        }
    );
}

#[test]
fn rejects_trailing_bytes() {
    let mut bytes = sample_bytes(7, &[1, 2, 3], Some(9));
    bytes.push(0x00);
    assert_eq!(
        codec::decode_canonical::<Sample>(&bytes, 1024).unwrap_err(),
        CodecError::TrailingBytes
    );
}

#[test]
fn rejects_indefinite_length_array() {
    // [_ 1, 7, h'', null] — indefinite array where a definite one is required.
    let bytes = vec![0x9f, 0x01, 0x07, 0x40, 0xf6, 0xff];
    assert_eq!(
        codec::decode_canonical::<Sample>(&bytes, 1024).unwrap_err(),
        CodecError::IndefiniteLength
    );
}

#[test]
fn rejects_map_in_place_of_array() {
    // A 4-pair map {1:7, ...} instead of the positional array.
    let bytes = vec![0xa1, 0x01, 0x07];
    assert_eq!(
        codec::decode_canonical::<Sample>(&bytes, 1024).unwrap_err(),
        CodecError::UnexpectedType
    );
}

#[test]
fn rejects_non_minimal_integer_via_reencode() {
    // Canonical is [0x84, 0x01, 0x07, 0x40, 0xf6]. Re-encode the array header
    // non-minimally as 0x98 0x04 (1-byte count) — decodes fine, re-encodes to
    // 0x84, so the byte-identity check must reject it.
    let bytes = vec![0x98, 0x04, 0x01, 0x07, 0x40, 0xf6];
    assert_eq!(
        codec::decode_canonical::<Sample>(&bytes, 1024).unwrap_err(),
        CodecError::NonCanonical
    );
}

#[test]
fn rejects_embedded_value_where_bytes_required() {
    // Position of `b` holds an empty array (0x80) rather than a byte string.
    let bytes = vec![0x84, 0x01, 0x07, 0x80, 0xf6];
    assert_eq!(
        codec::decode_canonical::<Sample>(&bytes, 1024).unwrap_err(),
        CodecError::UnexpectedType
    );
}

#[test]
fn rejects_unknown_version() {
    let bytes = vec![0x84, 0x02, 0x07, 0x40, 0xf6];
    assert_eq!(
        codec::decode_canonical::<Sample>(&bytes, 1024).unwrap_err(),
        CodecError::UnknownVersion(2)
    );
}

#[test]
fn rejects_wrong_array_length() {
    let bytes = vec![0x83, 0x01, 0x07, 0x40];
    assert_eq!(
        codec::decode_canonical::<Sample>(&bytes, 1024).unwrap_err(),
        CodecError::WrongArrayLength {
            expected: 4,
            actual: 3,
        }
    );
}

#[test]
fn discriminant_must_be_text_not_a_numeric_tag() {
    // A closed enum/sum discriminant is a snake_case tstr; a numeric tag is
    // rejected. This guards the design's "no numeric discriminants" rule.
    let numeric = vec![0x00]; // integer 0 where a discriminant string is expected
    let mut d = Decoder::new(&numeric);
    assert_eq!(
        codec::read_discriminant(&mut d, 32).unwrap_err(),
        CodecError::UnexpectedType
    );

    let text = {
        let mut buf = Vec::new();
        Encoder::new(&mut buf).str("prepare_host").unwrap();
        buf
    };
    let mut d = Decoder::new(&text);
    assert_eq!(
        codec::read_discriminant(&mut d, 32).unwrap(),
        "prepare_host"
    );
}

// ---------------------------------------------------------------------------
// Codec: set ordering primitives
// ---------------------------------------------------------------------------

#[test]
fn set_order_is_strictly_ascending() {
    assert!(codec::assert_set_order(None, b"a").is_ok());
    assert!(codec::assert_set_order(Some(b"a"), b"b").is_ok());
    assert_eq!(
        codec::assert_set_order(Some(b"b"), b"a").unwrap_err(),
        CodecError::UnsortedSet
    );
    assert_eq!(
        codec::assert_set_order(Some(b"a"), b"a").unwrap_err(),
        CodecError::DuplicateSetMember
    );
}

#[test]
fn sort_canonical_set_orders_and_rejects_duplicates() {
    let sorted =
        codec::sort_canonical_set(vec![b"c".to_vec(), b"a".to_vec(), b"b".to_vec()]).unwrap();
    assert_eq!(sorted, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    assert_eq!(
        codec::sort_canonical_set(vec![b"a".to_vec(), b"a".to_vec()]).unwrap_err(),
        CodecError::DuplicateSetMember
    );
}

// ---------------------------------------------------------------------------
// Digests: BLAKE3 wiring + digest_v1 framing
// ---------------------------------------------------------------------------

#[test]
fn blake3_wiring_matches_known_vector() {
    // Empty-input BLAKE3, the published test vector — proves we hash with BLAKE3.
    assert_eq!(
        hex(blake3::hash(b"").as_bytes()),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
}

#[test]
fn digest_v1_framing_is_length_prefixed() {
    let lbl = b"riot/test";
    let canonical = b"\x01\x02\x03";
    // Independent construction of the preimage.
    let mut preimage = Vec::new();
    preimage.extend_from_slice(&(lbl.len() as u16).to_be_bytes());
    preimage.extend_from_slice(lbl);
    preimage.extend_from_slice(&(canonical.len() as u64).to_be_bytes());
    preimage.extend_from_slice(canonical);
    let expected = blake3::hash(&preimage);
    assert_eq!(digest::digest_v1(lbl, canonical), *expected.as_bytes());
}

#[test]
fn digest_v1_separates_label_and_body() {
    let base = digest::digest_v1(b"riot/a", b"\x01");
    // Changing the body changes the digest.
    assert_ne!(base, digest::digest_v1(b"riot/a", b"\x02"));
    // Changing the label changes the digest.
    assert_ne!(base, digest::digest_v1(b"riot/b", b"\x01"));
    // A label/body split ambiguity must NOT collide: ("ab","") vs ("a","b").
    assert_ne!(digest::digest_v1(b"ab", b""), digest::digest_v1(b"a", b"b"));
}

// ---------------------------------------------------------------------------
// Digests: specialized preimages, each cross-checked against an inline build
// ---------------------------------------------------------------------------

#[test]
fn namespace_token_hmac_input_has_exact_layout() {
    let op = b"OP";
    let ns = b"NS";
    let expiry: u64 = 0x0102_0304_0506_0708;
    let epoch: u32 = 0x0A0B_0C0D;
    let got = digest::namespace_token_hmac_input(op, ns, expiry, epoch);

    let mut want = Vec::new();
    want.extend_from_slice(&23u16.to_be_bytes());
    want.extend_from_slice(label::NAMESPACE_TOKEN);
    want.extend_from_slice(&2u16.to_be_bytes());
    want.extend_from_slice(op);
    want.extend_from_slice(&2u16.to_be_bytes());
    want.extend_from_slice(ns);
    want.extend_from_slice(&expiry.to_be_bytes());
    want.extend_from_slice(&epoch.to_be_bytes());
    assert_eq!(got, want);
    // Sanity: the hardcoded label length equals the real label length.
    assert_eq!(label::NAMESPACE_TOKEN.len(), 23);
}

#[test]
fn peer_proof_preimage_binds_role_length_prefixed() {
    let role = b"initiator";
    let ptd = [0x11u8; 32];
    let got = digest::peer_proof_signature_preimage(role, &ptd);

    let mut want = Vec::new();
    want.extend_from_slice(&25u16.to_be_bytes());
    want.extend_from_slice(label::PEER_PROOF);
    want.extend_from_slice(&(role.len() as u16).to_be_bytes());
    want.extend_from_slice(role);
    want.extend_from_slice(&ptd);
    assert_eq!(got, want);

    // initiator and responder proofs must differ under the same transcript.
    assert_ne!(
        digest::peer_proof_signature_preimage(b"initiator", &ptd),
        digest::peer_proof_signature_preimage(b"responder", &ptd)
    );
}

#[test]
fn sync_snapshot_digest_hashes_fields_positionally() {
    let ns = b"namespace-id";
    let ids: [&[u8]; 2] = [b"id-a", b"id-bb"];
    let got = digest::sync_snapshot_digest(ns, 2, 4096, &ids);

    let mut preimage = Vec::new();
    preimage.extend_from_slice(label::SYNC_SNAPSHOT);
    preimage.extend_from_slice(&(ns.len() as u32).to_be_bytes());
    preimage.extend_from_slice(ns);
    preimage.extend_from_slice(&2u64.to_be_bytes());
    preimage.extend_from_slice(&4096u64.to_be_bytes());
    for id in ids {
        preimage.extend_from_slice(&(id.len() as u32).to_be_bytes());
        preimage.extend_from_slice(id);
    }
    assert_eq!(got, *blake3::hash(&preimage).as_bytes());
}

#[test]
fn work_proof_binds_challenge_and_counter() {
    let wcd = [0x22u8; 32];
    let got = digest::work_proof(&wcd, 42);

    let mut preimage = Vec::new();
    preimage.extend_from_slice(label::WORK_PROOF);
    preimage.extend_from_slice(&wcd);
    preimage.extend_from_slice(&42u64.to_be_bytes());
    assert_eq!(got, *blake3::hash(&preimage).as_bytes());
    // A different counter yields a different proof.
    assert_ne!(got, digest::work_proof(&wcd, 43));
}

#[test]
fn operator_key_id_and_anchor_id_use_bare_labels() {
    let vk = b"canonical-verification-key-bytes";
    let mut p1 = Vec::new();
    p1.extend_from_slice(label::OPERATOR_KEY_ID);
    p1.extend_from_slice(vk);
    assert_eq!(digest::operator_key_id(vk), *blake3::hash(&p1).as_bytes());

    let pk = [0x33u8; 32];
    let rnd = [0x44u8; 32];
    let mut p2 = Vec::new();
    p2.extend_from_slice(label::ANCHOR_ID);
    p2.extend_from_slice(&pk);
    p2.extend_from_slice(&rnd);
    assert_eq!(digest::anchor_id(&pk, &rnd), *blake3::hash(&p2).as_bytes());
}
