use minicbor::Encoder;
use riot_core::apps::endorse::{
    decode_endorsement, encode_endorsement, write_endorsement, EndorsementMarker,
    MAX_ENDORSEMENT_BYTES, MAX_ENDORSEMENT_NOTE_BYTES,
};
use riot_core::apps::AppsError;
use riot_core::session::RiotSession;
use riot_core::willow::generate_communal_author;

fn sample() -> EndorsementMarker {
    EndorsementMarker {
        app_id: [7u8; 32],
        note: "we ran jail support with this".to_string(),
        retracted: false,
    }
}

#[test]
fn endorsement_round_trips() {
    let marker = sample();
    let bytes = encode_endorsement(&marker).expect("encode");
    assert_eq!(decode_endorsement(&bytes).expect("decode"), marker);
}

#[test]
fn empty_note_is_allowed() {
    let marker = EndorsementMarker {
        note: String::new(),
        ..sample()
    };
    let bytes = encode_endorsement(&marker).expect("encode");
    assert_eq!(decode_endorsement(&bytes).expect("decode"), marker);
}

#[test]
fn retracted_round_trips() {
    let marker = EndorsementMarker {
        retracted: true,
        ..sample()
    };
    let bytes = encode_endorsement(&marker).expect("encode");
    assert!(decode_endorsement(&bytes).expect("decode").retracted);
}

#[test]
fn oversized_note_is_rejected() {
    let marker = EndorsementMarker {
        note: "x".repeat(MAX_ENDORSEMENT_NOTE_BYTES + 1),
        ..sample()
    };
    assert_eq!(
        encode_endorsement(&marker),
        Err(AppsError::EndorsementFieldInvalid)
    );
}

#[test]
fn tampered_bytes_are_rejected() {
    let mut bytes = encode_endorsement(&sample()).expect("encode");
    // Truncation and trailing garbage must both fail the canonical decoder.
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(decode_endorsement(&truncated).is_err());
    bytes.push(0x00);
    assert!(decode_endorsement(&bytes).is_err());
}

#[test]
fn invalid_utf8_note_is_rejected() {
    // Invalid UTF-8 where the note text should be is a malformed frame
    // (minicbor's str() rejects non-UTF-8). Hand-built bytes, same style
    // as public_bundle_rejects_invalid_utf8_codec.
    // map(3), key 0, bytes len 32 (0x58 0x20)
    let mut buffer: Vec<u8> = vec![0xa3, 0x00, 0x58, 0x20];
    buffer.extend_from_slice(&[7u8; 32]);
    // key 1, text string len 2 with invalid UTF-8 (0xff 0xfe),
    // key 2, retracted = false
    buffer.extend_from_slice(&[0x01, 0x62, 0xff, 0xfe, 0x02, 0x00]);
    assert_eq!(
        decode_endorsement(&buffer),
        Err(AppsError::EndorsementFieldInvalid)
    );
}

#[test]
fn malformed_app_id_is_rejected() {
    // Wrong-length byte strings (31 and 33 bytes) and a wrong-type value
    // (text string) at key 0 must all fail decode.
    fn with_app_id(write_app_id: impl FnOnce(&mut Encoder<&mut Vec<u8>>)) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::new();
        let mut e = Encoder::new(&mut buffer);
        e.map(3).unwrap();
        e.u8(0).unwrap();
        write_app_id(&mut e);
        e.u8(1)
            .unwrap()
            .str("we ran jail support with this")
            .unwrap();
        e.u8(2).unwrap().u8(0).unwrap();
        buffer
    }

    let short = with_app_id(|e| {
        e.bytes(&[7u8; 31]).unwrap();
    });
    assert_eq!(
        decode_endorsement(&short),
        Err(AppsError::EndorsementFieldInvalid)
    );

    let long = with_app_id(|e| {
        e.bytes(&[7u8; 33]).unwrap();
    });
    assert_eq!(
        decode_endorsement(&long),
        Err(AppsError::EndorsementFieldInvalid)
    );

    let wrong_type = with_app_id(|e| {
        e.str("0707070707070707070707070707070707070707070707070707070707070707")
            .unwrap();
    });
    assert_eq!(
        decode_endorsement(&wrong_type),
        Err(AppsError::EndorsementFieldInvalid)
    );
}

#[test]
fn endorsement_note_accepts_the_exact_limit() {
    let marker = EndorsementMarker {
        note: "x".repeat(MAX_ENDORSEMENT_NOTE_BYTES),
        ..sample()
    };
    assert_eq!(
        decode_endorsement(&encode_endorsement(&marker).unwrap()).unwrap(),
        marker
    );
}

#[test]
fn write_endorsement_commits_the_canonical_marker() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");
    write_endorsement(&store, &author, &sample(), 1).expect("write");
    assert_eq!(store.live_count().expect("live count"), 1);
}

#[test]
fn write_endorsement_rejects_an_invalid_marker_before_committing() {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let author = generate_communal_author().expect("author");
    let marker = EndorsementMarker {
        note: "x".repeat(MAX_ENDORSEMENT_NOTE_BYTES + 1),
        ..sample()
    };
    assert_eq!(
        write_endorsement(&store, &author, &marker, 1),
        Err(AppsError::EndorsementFieldInvalid)
    );
    assert_eq!(store.live_count().expect("live count"), 0);
}

fn raw_endorsement(
    pairs: u64,
    keys: [u64; 3],
    app_id_len: usize,
    note: Option<&str>,
    retracted: u8,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(pairs).unwrap();
    encoder
        .u64(keys[0])
        .unwrap()
        .bytes(&vec![7; app_id_len])
        .unwrap();
    encoder.u64(keys[1]).unwrap();
    match note {
        Some(note) => {
            encoder.str(note).unwrap();
        }
        None => {
            encoder.bytes(b"not text").unwrap();
        }
    }
    encoder.u64(keys[2]).unwrap().u8(retracted).unwrap();
    bytes
}

#[test]
fn endorsement_decoder_rejects_field_count_order_unknown_and_boolean_errors() {
    let invalid = [
        raw_endorsement(2, [0, 1, 2], 32, Some("note"), 0),
        raw_endorsement(3, [1, 1, 2], 32, Some("note"), 0),
        raw_endorsement(3, [0, 0, 2], 32, Some("note"), 0),
        raw_endorsement(3, [0, 1, 3], 32, Some("note"), 0),
        raw_endorsement(3, [0, 1, 2], 32, Some("note"), 2),
        raw_endorsement(3, [0, 1, 2], 32, None, 0),
    ];
    for bytes in invalid {
        assert_eq!(
            decode_endorsement(&bytes),
            Err(AppsError::EndorsementFieldInvalid)
        );
    }
}

#[test]
fn endorsement_decoder_rejects_overlong_note_and_noncanonical_key_width() {
    let overlong = "x".repeat(MAX_ENDORSEMENT_NOTE_BYTES + 1);
    let bytes = raw_endorsement(3, [0, 1, 2], 32, Some(&overlong), 0);
    assert_eq!(
        decode_endorsement(&bytes),
        Err(AppsError::EndorsementFieldInvalid)
    );

    let mut noncanonical = encode_endorsement(&sample()).unwrap();
    noncanonical.splice(1..2, [0x18, 0x00]);
    assert_eq!(
        decode_endorsement(&noncanonical),
        Err(AppsError::EndorsementFieldInvalid)
    );

    assert_eq!(
        decode_endorsement(&vec![0; MAX_ENDORSEMENT_BYTES + 1]),
        Err(AppsError::EndorsementFieldInvalid)
    );
}

#[test]
fn endorsement_decoder_rejects_indefinite_and_truncated_structures() {
    let mut through_app_id = Vec::new();
    let mut encoder = Encoder::new(&mut through_app_id);
    encoder
        .map(3)
        .unwrap()
        .u8(0)
        .unwrap()
        .bytes(&[7; 32])
        .unwrap();

    let mut through_note = through_app_id.clone();
    Encoder::new(&mut through_note)
        .u8(1)
        .unwrap()
        .str("note")
        .unwrap();

    let mut through_note_key = through_app_id.clone();
    Encoder::new(&mut through_note_key).u8(1).unwrap();

    let invalid = [
        Vec::new(),
        vec![0xbf],
        vec![0xa3],
        through_app_id,
        through_note_key,
        through_note,
    ];
    for bytes in invalid {
        assert_eq!(
            decode_endorsement(&bytes),
            Err(AppsError::EndorsementFieldInvalid)
        );
    }
}
