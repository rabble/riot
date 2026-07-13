use riot_core::model::AlertError;
use riot_core::profile::card::{
    decode_profile_card, encode_profile_card, ProfileCard, MAX_DISPLAY_NAME_BYTES,
};
use riot_core::profile::ProfileError;
use riot_core::willow::WillowError;

fn sample() -> ProfileCard {
    ProfileCard {
        display_name: "Ana".to_string(),
    }
}

#[test]
fn profile_card_round_trips() {
    let bytes = encode_profile_card(&sample()).expect("encode");
    assert_eq!(decode_profile_card(&bytes).expect("decode"), sample());
}

#[test]
fn empty_display_name_is_rejected() {
    let card = ProfileCard {
        display_name: String::new(),
    };
    assert_eq!(encode_profile_card(&card), Err(ProfileError::FieldInvalid));
}

#[test]
fn oversized_display_name_is_rejected() {
    let card = ProfileCard {
        display_name: "x".repeat(MAX_DISPLAY_NAME_BYTES + 1),
    };
    assert_eq!(encode_profile_card(&card), Err(ProfileError::FieldInvalid));
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

#[test]
fn exact_maximum_display_name_round_trips_and_one_more_is_rejected() {
    let exact = ProfileCard {
        display_name: "x".repeat(MAX_DISPLAY_NAME_BYTES),
    };
    let bytes = encode_profile_card(&exact).expect("exact maximum is valid");
    assert_eq!(decode_profile_card(&bytes), Ok(exact));

    let too_large = ProfileCard {
        display_name: "x".repeat(MAX_DISPLAY_NAME_BYTES + 1),
    };
    assert_eq!(
        encode_profile_card(&too_large),
        Err(ProfileError::FieldInvalid)
    );
}

#[test]
fn every_malformed_card_frame_is_rejected() {
    let oversized = vec![0; riot_core::profile::card::MAX_PROFILE_CARD_BYTES + 1];
    assert_eq!(
        decode_profile_card(&oversized),
        Err(ProfileError::FieldInvalid)
    );

    for malformed in [
        vec![],                             // no map
        vec![0xbf, 0x00, 0x61, b'A', 0xff], // indefinite map
        vec![0xa0],                         // wrong field count
        vec![0xa1],                         // missing key
        vec![0xa1, 0x01, 0x61, b'A'],       // wrong key
        vec![0xa1, 0x00],                   // missing value
        vec![0xa1, 0x00, 0x60],             // empty name
        vec![0xa1, 0x18, 0x00, 0x61, b'A'], // non-canonical integer key
        vec![0xa1, 0x00, 0x78, 0x01, b'A'], // non-canonical string length
    ] {
        assert_eq!(
            decode_profile_card(&malformed),
            Err(ProfileError::FieldInvalid),
            "accepted malformed card: {malformed:02x?}"
        );
    }
}

#[test]
fn profile_and_willow_errors_have_stable_display_and_conversion() {
    let willow_errors = [
        WillowError::PathInvalid,
        WillowError::DoesNotAuthorise,
        WillowError::DecodeFailed,
        WillowError::TrailingBytes,
        WillowError::EntropyUnavailable,
        WillowError::ClockUnavailable,
        WillowError::InvalidAlert(AlertError::Malformed),
        WillowError::NamespaceNotCommunal,
        WillowError::SealedIdentityInvalid,
        WillowError::IdentitySealFailed,
    ];
    for error in willow_errors {
        assert_eq!(error.to_string(), format!("{error:?}"));
    }

    let profile_errors = [
        ProfileError::FieldInvalid,
        ProfileError::PathInvalid,
        ProfileError::Willow(WillowError::DecodeFailed),
        ProfileError::StoreRejected,
    ];
    for error in profile_errors {
        assert_eq!(error.to_string(), format!("{error:?}"));
    }

    assert_eq!(
        ProfileError::from(WillowError::TrailingBytes),
        ProfileError::Willow(WillowError::TrailingBytes)
    );
}
