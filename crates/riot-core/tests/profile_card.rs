use riot_core::profile::card::{
    decode_profile_card, encode_profile_card, ProfileCard, MAX_DISPLAY_NAME_BYTES,
};
use riot_core::profile::ProfileError;

fn sample() -> ProfileCard {
    ProfileCard { display_name: "Ana".to_string() }
}

#[test]
fn profile_card_round_trips() {
    let bytes = encode_profile_card(&sample()).expect("encode");
    assert_eq!(decode_profile_card(&bytes).expect("decode"), sample());
}

#[test]
fn empty_display_name_is_rejected() {
    let card = ProfileCard { display_name: String::new() };
    assert_eq!(
        encode_profile_card(&card),
        Err(ProfileError::FieldInvalid)
    );
}

#[test]
fn oversized_display_name_is_rejected() {
    let card = ProfileCard {
        display_name: "x".repeat(MAX_DISPLAY_NAME_BYTES + 1),
    };
    assert_eq!(
        encode_profile_card(&card),
        Err(ProfileError::FieldInvalid)
    );
}

#[test]
fn truncated_and_trailing_bytes_are_rejected() {
    let mut bytes = encode_profile_card(&sample()).expect("encode");
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(decode_profile_card(&truncated).is_err());
    bytes.push(0x00);
    assert!(decode_profile_card(&bytes).is_err());
}

#[test]
fn invalid_utf8_display_name_is_rejected() {
    // Hand-build a canonical-looking frame whose text field holds invalid
    // UTF-8: map(1), key 0, text(2) = 0xff 0xfe.
    let bytes = vec![0xa1, 0x00, 0x62, 0xff, 0xfe];
    assert!(decode_profile_card(&bytes).is_err());
}

#[test]
fn wrong_type_for_display_name_is_rejected() {
    // map(1), key 0, bytes(3) instead of text.
    let bytes = vec![0xa1, 0x00, 0x43, 0x61, 0x62, 0x63];
    assert!(decode_profile_card(&bytes).is_err());
}
