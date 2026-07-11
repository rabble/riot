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
    let marker = EndorsementMarker { note: String::new(), ..sample() };
    let bytes = encode_endorsement(&marker).expect("encode");
    assert_eq!(decode_endorsement(&bytes).expect("decode"), marker);
}

#[test]
fn retracted_round_trips() {
    let marker = EndorsementMarker { retracted: true, ..sample() };
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
