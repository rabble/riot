use minicbor::Encoder;
use riot_core::apps::endorse::{
    decode_endorsement, encode_endorsement, EndorsementMarker, MAX_ENDORSEMENT_NOTE_BYTES,
};
use riot_core::apps::AppsError;

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
