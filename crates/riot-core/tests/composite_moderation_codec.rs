//! Composite-site Unit 3, Task 1 — canonical CBOR codec for the owner-signed
//! moderation records at `O:/mod/`. Mirrors `site_manifest_codec.rs` (and the
//! newswire/model.rs golden-vector discipline): deterministic encode,
//! byte-identical decode (`prove_canonical`), definite lengths, strictly-ordered
//! integer keys, a closed failure vocabulary, a CLOSED `kind` discriminant
//! (unknown value -> reject), exact-32 id fields, and a MAX byte ceiling.
//!
//! No signing here (Task 2) and no admission (Task 4) — pure schema + codec.

use minicbor::Encoder;
use riot_core::site::{
    decode_moderation_record, encode_moderation_record, Endorse, ModEpoch, ModerationRecord,
    ModerationRecordError, Revoke, Tombstone, MAX_MODERATION_RECORD_BYTES,
    MODERATION_RECORD_SCHEMA,
};

fn revoke() -> ModerationRecord {
    ModerationRecord::Revoke(Revoke {
        author_key: [0x11; 32],
        effective_ts: 1_700_000_000,
    })
}

fn tombstone() -> ModerationRecord {
    ModerationRecord::Tombstone(Tombstone {
        target_ns: [0x22; 32],
        target_entry: [0x33; 32],
    })
}

fn mod_epoch() -> ModerationRecord {
    ModerationRecord::ModEpoch(ModEpoch {
        seq: 7,
        ts: 1_700_000_500,
        mod_set_digest: [0x44; 32],
    })
}

fn endorse() -> ModerationRecord {
    ModerationRecord::Endorse(Endorse {
        author_key: [0x55; 32],
    })
}

fn assert_round_trips(record: ModerationRecord) {
    let bytes = encode_moderation_record(&record).expect("encode");
    let decoded = decode_moderation_record(&bytes).expect("decode");
    assert_eq!(decoded, record);
    // Re-encoding the decoded value reproduces the exact bytes (canonical).
    assert_eq!(
        encode_moderation_record(&decoded).expect("re-encode"),
        bytes
    );
}

#[test]
fn revoke_round_trips_byte_identically() {
    assert_round_trips(revoke());
}

#[test]
fn tombstone_round_trips_byte_identically() {
    assert_round_trips(tombstone());
}

#[test]
fn mod_epoch_round_trips_byte_identically() {
    assert_round_trips(mod_epoch());
}

#[test]
fn endorse_round_trips_byte_identically() {
    assert_round_trips(endorse());
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut bytes = encode_moderation_record(&revoke()).expect("encode");
    bytes.push(0);
    assert_eq!(
        decode_moderation_record(&bytes),
        Err(ModerationRecordError::TrailingBytes)
    );
}

#[test]
fn indefinite_length_map_is_rejected() {
    let canonical = encode_moderation_record(&revoke()).expect("encode");
    // Replace the definite top-level map header (0xA3) with an indefinite one
    // (0xBF ... 0xFF). A non-canonical (indefinite) container is a hard reject.
    let mut indefinite = vec![0xbf];
    indefinite.extend_from_slice(&canonical[1..]);
    indefinite.push(0xff);
    assert_eq!(
        decode_moderation_record(&indefinite),
        Err(ModerationRecordError::NonCanonical)
    );
}

#[test]
fn wrong_schema_is_rejected() {
    let mut bytes = encode_moderation_record(&revoke()).expect("encode");
    let position = bytes
        .windows(MODERATION_RECORD_SCHEMA.len())
        .position(|window| window == MODERATION_RECORD_SCHEMA.as_bytes())
        .expect("schema present");
    bytes[position] = b'x';
    assert_eq!(
        decode_moderation_record(&bytes),
        Err(ModerationRecordError::WrongSchema)
    );
}

#[test]
fn misordered_keys_are_rejected() {
    // Envelope keys must be strictly ascending; kind (1) before schema (0) fails.
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(2).unwrap();
    encoder.u8(1).unwrap().u64(0).unwrap();
    encoder
        .u8(0)
        .unwrap()
        .str(MODERATION_RECORD_SCHEMA)
        .unwrap();
    assert_eq!(
        decode_moderation_record(&bytes),
        Err(ModerationRecordError::DuplicateOrMisorderedKey(0))
    );
}

#[test]
fn unknown_kind_is_rejected() {
    // `kind` is a CLOSED discriminant: an unknown code is a hard reject, never a
    // silently-ignored record.
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(3).unwrap();
    encoder
        .u8(0)
        .unwrap()
        .str(MODERATION_RECORD_SCHEMA)
        .unwrap();
    encoder.u8(1).unwrap().u64(99).unwrap();
    encoder.u8(2).unwrap().map(0).unwrap();
    assert_eq!(
        decode_moderation_record(&bytes),
        Err(ModerationRecordError::InvalidEnum("kind"))
    );
}

#[test]
fn oversized_input_is_rejected() {
    // A hostile peer cannot force unbounded decode work: input past the ceiling is
    // rejected before any parsing.
    let bytes = vec![0u8; MAX_MODERATION_RECORD_BYTES + 1];
    assert_eq!(
        decode_moderation_record(&bytes),
        Err(ModerationRecordError::InputTooLarge)
    );
}

#[test]
fn wrong_length_id_field_is_rejected() {
    // The 32-byte id fields are exact: a 31-byte `author_key` is malformed.
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(3).unwrap();
    encoder
        .u8(0)
        .unwrap()
        .str(MODERATION_RECORD_SCHEMA)
        .unwrap();
    encoder.u8(1).unwrap().u64(0).unwrap(); // kind = revoke
    encoder.u8(2).unwrap().map(2).unwrap();
    encoder.u8(0).unwrap().bytes(&[0x11; 31]).unwrap(); // 31 bytes, not 32
    encoder.u8(1).unwrap().u64(5).unwrap();
    assert_eq!(
        decode_moderation_record(&bytes),
        Err(ModerationRecordError::Malformed)
    );
}

#[test]
fn truncated_body_map_is_rejected() {
    // A revoke body is a definite map(2); a map(1) (missing `effective_ts`) fails
    // the exact-count body guard.
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(3).unwrap();
    encoder
        .u8(0)
        .unwrap()
        .str(MODERATION_RECORD_SCHEMA)
        .unwrap();
    encoder.u8(1).unwrap().u64(0).unwrap(); // kind = revoke
    encoder.u8(2).unwrap().map(1).unwrap();
    encoder.u8(0).unwrap().bytes(&[0x11; 32]).unwrap();
    assert_eq!(
        decode_moderation_record(&bytes),
        Err(ModerationRecordError::Malformed)
    );
}

#[test]
fn missing_envelope_body_key_is_rejected() {
    // An envelope carrying only schema + kind (no body, key 2) is rejected.
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(2).unwrap();
    encoder
        .u8(0)
        .unwrap()
        .str(MODERATION_RECORD_SCHEMA)
        .unwrap();
    encoder.u8(1).unwrap().u64(0).unwrap(); // kind = revoke, but no body follows
    assert_eq!(
        decode_moderation_record(&bytes),
        Err(ModerationRecordError::MissingKey(2))
    );
}
