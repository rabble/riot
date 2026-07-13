use riot_core::model::{Certainty, Severity, Urgency};
use riot_core::newswire::{
    decode_editorial_action, decode_news_post, decode_space_descriptor, encode_editorial_action,
    encode_news_post, encode_space_descriptor, AlertProfileV1, EditorialActionKind,
    EditorialActionV1, NewsPostV1, NewswireModelError, OperationalProfileV1, RequestKind,
    RequestProfileV1, SpaceDescriptorV1, MAX_NEWSWIRE_PAYLOAD_BYTES, SPACE_SCHEMA,
};

const SPACE_ID: [u8; 32] = [0x11; 32];
const POST_ID: [u8; 32] = [0x22; 32];

fn space() -> SpaceDescriptorV1 {
    SpaceDescriptorV1 {
        namespace_id: [0x10; 32],
        name: "Riverside Independent Media".into(),
        summary: "Open publishing by and for the community.".into(),
        languages: vec!["en".into()],
        geographic_tags: vec!["riverside".into()],
        topic_tags: vec!["community-media".into()],
        editorial_roster: vec![[0x20; 32]],
        predecessor: None,
        successor: None,
    }
}

fn post() -> NewsPostV1 {
    NewsPostV1 {
        space_descriptor_entry_id: SPACE_ID,
        headline: "Night march reaches the square".into(),
        body: "Witness report from the community assembly.".into(),
        language: "en".into(),
        event_time_unix_seconds: Some(1_800_000_100),
        expires_at_unix_seconds: None,
        coarse_location: Some("central district".into()),
        source_claims: vec!["participant account".into()],
        operational_profile: None,
        ai_assisted: false,
    }
}

fn action() -> EditorialActionV1 {
    EditorialActionV1 {
        space_descriptor_entry_id: SPACE_ID,
        target_entry_id: POST_ID,
        kind: EditorialActionKind::Correct,
        reason: Some("Clarifies the assembly's decision.".into()),
        correction_text: Some("The assembly reconvenes Friday.".into()),
    }
}

#[test]
fn all_newswire_payloads_round_trip_canonically() {
    let post = post();
    let action = action();
    assert_eq!(
        decode_space_descriptor(&encode_space_descriptor(&space()).unwrap()).unwrap(),
        space()
    );
    assert_eq!(
        decode_news_post(&encode_news_post(&post).unwrap()).unwrap(),
        post
    );
    assert_eq!(
        decode_editorial_action(&encode_editorial_action(&action).unwrap()).unwrap(),
        action
    );
}

#[test]
fn space_boundaries_are_frozen() {
    let mut valid_cases: Vec<(&str, SpaceDescriptorV1)> = Vec::new();
    let mut value = space();
    value.name = "n".repeat(256);
    valid_cases.push(("name max", value));
    let mut value = space();
    value.summary = "s".repeat(4_096);
    valid_cases.push(("summary max", value));
    let mut value = space();
    value.languages = vec!["aa".into(); 16];
    value.languages[0] = "l".repeat(35);
    valid_cases.push(("language count and bytes", value));
    let mut value = space();
    value.geographic_tags = vec!["g".into(); 32];
    value.geographic_tags[0] = "g".repeat(128);
    valid_cases.push(("geographic tag count and bytes", value));
    let mut value = space();
    value.topic_tags = vec!["t".into(); 32];
    value.topic_tags[0] = "t".repeat(128);
    valid_cases.push(("topic tag count and bytes", value));
    let mut value = space();
    value.editorial_roster = (0..64)
        .map(|index| {
            let mut id = [0u8; 32];
            id[0] = index;
            id
        })
        .collect();
    valid_cases.push(("editorial roster count", value));
    for (name, value) in valid_cases {
        assert!(encode_space_descriptor(&value).is_ok(), "{name}");
    }

    let invalid_cases: Vec<(&str, SpaceDescriptorV1, NewswireModelError)> = vec![
        {
            let mut value = space();
            value.name = "n".repeat(257);
            (
                "name too large",
                value,
                NewswireModelError::FieldTooLarge("name"),
            )
        },
        {
            let mut value = space();
            value.summary = "s".repeat(4_097);
            (
                "summary too large",
                value,
                NewswireModelError::FieldTooLarge("summary"),
            )
        },
        {
            let mut value = space();
            value.languages = vec!["en".into(); 17];
            (
                "too many languages",
                value,
                NewswireModelError::TooManyEntries("languages"),
            )
        },
        {
            let mut value = space();
            value.languages = vec!["e".into()];
            (
                "language too short",
                value,
                NewswireModelError::FieldTooSmall("language"),
            )
        },
        {
            let mut value = space();
            value.languages = vec!["l".repeat(36)];
            (
                "language too large",
                value,
                NewswireModelError::FieldTooLarge("language"),
            )
        },
        {
            let mut value = space();
            value.geographic_tags = vec!["g".into(); 33];
            (
                "too many geographic tags",
                value,
                NewswireModelError::TooManyEntries("geographic_tags"),
            )
        },
        {
            let mut value = space();
            value.geographic_tags = vec!["g".repeat(129)];
            (
                "geographic tag too large",
                value,
                NewswireModelError::FieldTooLarge("geographic_tag"),
            )
        },
        {
            let mut value = space();
            value.topic_tags = vec!["t".into(); 33];
            (
                "too many topic tags",
                value,
                NewswireModelError::TooManyEntries("topic_tags"),
            )
        },
        {
            let mut value = space();
            value.topic_tags = vec!["t".repeat(129)];
            (
                "topic tag too large",
                value,
                NewswireModelError::FieldTooLarge("topic_tag"),
            )
        },
        {
            let mut value = space();
            value.editorial_roster = vec![[0x20; 32]; 65];
            (
                "too many editors",
                value,
                NewswireModelError::TooManyEntries("editorial_roster"),
            )
        },
        {
            let mut value = space();
            value.editorial_roster = vec![[0x20; 32], [0x20; 32]];
            (
                "duplicate editor",
                value,
                NewswireModelError::DuplicateEditorialRosterKey,
            )
        },
    ];
    for (name, value, expected) in invalid_cases {
        assert_eq!(encode_space_descriptor(&value), Err(expected), "{name}");
    }
}

#[test]
fn post_and_action_boundaries_are_frozen() {
    let mut valid_posts: Vec<(&str, NewsPostV1)> = Vec::new();
    let mut value = post();
    value.headline = "h".repeat(512);
    valid_posts.push(("headline max", value));
    let mut value = post();
    value.body = "b".repeat(65_536);
    valid_posts.push(("body max", value));
    let mut value = post();
    value.language = "l".repeat(35);
    valid_posts.push(("language max", value));
    let mut value = post();
    value.coarse_location = Some("c".repeat(2_048));
    valid_posts.push(("location max", value));
    let mut value = post();
    value.source_claims = vec!["s".into(); 16];
    value.source_claims[0] = "s".repeat(1_024);
    valid_posts.push(("source claims count and bytes", value));
    for (name, value) in valid_posts {
        assert!(encode_news_post(&value).is_ok(), "{name}");
    }

    let invalid_posts: Vec<(&str, NewsPostV1, NewswireModelError)> = vec![
        {
            let mut value = post();
            value.headline = "h".repeat(513);
            (
                "headline too large",
                value,
                NewswireModelError::FieldTooLarge("headline"),
            )
        },
        {
            let mut value = post();
            value.body = "b".repeat(65_537);
            (
                "body too large",
                value,
                NewswireModelError::FieldTooLarge("body"),
            )
        },
        {
            let mut value = post();
            value.language = "e".into();
            (
                "language too short",
                value,
                NewswireModelError::FieldTooSmall("language"),
            )
        },
        {
            let mut value = post();
            value.language = "l".repeat(36);
            (
                "language too large",
                value,
                NewswireModelError::FieldTooLarge("language"),
            )
        },
        {
            let mut value = post();
            value.coarse_location = Some("c".repeat(2_049));
            (
                "location too large",
                value,
                NewswireModelError::FieldTooLarge("coarse_location"),
            )
        },
        {
            let mut value = post();
            value.source_claims = vec!["s".into(); 17];
            (
                "too many source claims",
                value,
                NewswireModelError::TooManyEntries("source_claims"),
            )
        },
        {
            let mut value = post();
            value.source_claims = vec!["s".repeat(1_025)];
            (
                "source claim too large",
                value,
                NewswireModelError::FieldTooLarge("source_claim"),
            )
        },
    ];
    for (name, value, expected) in invalid_posts {
        assert_eq!(encode_news_post(&value), Err(expected), "{name}");
    }

    let mut valid = action();
    valid.reason = Some("r".repeat(4_096));
    valid.correction_text = Some("c".repeat(65_536));
    assert!(encode_editorial_action(&valid).is_ok());

    let invalid_actions = [
        {
            let mut value = action();
            value.reason = Some("r".repeat(4_097));
            (value, NewswireModelError::FieldTooLarge("reason"))
        },
        {
            let mut value = action();
            value.correction_text = Some("c".repeat(65_537));
            (value, NewswireModelError::FieldTooLarge("correction_text"))
        },
    ];
    for (value, expected) in invalid_actions {
        assert_eq!(encode_editorial_action(&value), Err(expected));
    }
}

#[test]
fn blank_present_text_is_rejected() {
    let mut invalid = space();
    invalid.name = " \t".into();
    assert_eq!(
        encode_space_descriptor(&invalid),
        Err(NewswireModelError::FieldEmpty("name"))
    );
    let mut invalid = post();
    invalid.coarse_location = Some("  ".into());
    assert_eq!(
        encode_news_post(&invalid),
        Err(NewswireModelError::FieldEmpty("coarse_location"))
    );
    let mut invalid = action();
    invalid.reason = Some("\n".into());
    assert_eq!(
        encode_editorial_action(&invalid),
        Err(NewswireModelError::FieldEmpty("reason"))
    );
}

#[test]
fn editorial_action_field_combinations_are_closed() {
    let mut correct_without_text = action();
    correct_without_text.correction_text = None;
    assert_eq!(
        encode_editorial_action(&correct_without_text),
        Err(NewswireModelError::CorrectionTextRequired)
    );

    for kind in [
        EditorialActionKind::Feature,
        EditorialActionKind::Verify,
        EditorialActionKind::Hide,
        EditorialActionKind::Tombstone,
        EditorialActionKind::Retract,
    ] {
        let mut invalid = action();
        invalid.kind = kind;
        assert_eq!(
            encode_editorial_action(&invalid),
            Err(NewswireModelError::CorrectionTextForbidden),
            "{kind:?}"
        );
    }

    for kind in [
        EditorialActionKind::Correct,
        EditorialActionKind::Hide,
        EditorialActionKind::Tombstone,
        EditorialActionKind::Retract,
    ] {
        let mut invalid = action();
        invalid.kind = kind;
        invalid.reason = None;
        if kind != EditorialActionKind::Correct {
            invalid.correction_text = None;
        }
        assert_eq!(
            encode_editorial_action(&invalid),
            Err(NewswireModelError::EditorialReasonRequired),
            "{kind:?}"
        );
    }
}

#[test]
fn operational_profiles_enforce_post_requirements() {
    let alert = OperationalProfileV1::Alert(AlertProfileV1 {
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        valid_from_unix_seconds: Some(1_800_000_000),
    });
    let request = OperationalProfileV1::Request(RequestProfileV1 {
        kind: RequestKind::Need,
        needed_by_unix_seconds: Some(1_800_000_500),
        contact_instructions: "Meet at the community kitchen.".into(),
    });

    for profile in [alert.clone(), request.clone()] {
        let mut valid = post();
        valid.expires_at_unix_seconds = Some(1_800_000_900);
        valid.operational_profile = Some(profile);
        let bytes = encode_news_post(&valid).unwrap();
        assert_eq!(decode_news_post(&bytes).unwrap(), valid);
    }

    let mut max_contact = post();
    max_contact.expires_at_unix_seconds = Some(1_800_000_900);
    max_contact.operational_profile = Some(OperationalProfileV1::Request(RequestProfileV1 {
        kind: RequestKind::Offer,
        needed_by_unix_seconds: None,
        contact_instructions: "c".repeat(2_048),
    }));
    assert!(encode_news_post(&max_contact).is_ok());

    let cases = [
        {
            let mut value = post();
            value.operational_profile = Some(alert.clone());
            value.expires_at_unix_seconds = None;
            (
                "alert expiry",
                value,
                NewswireModelError::AlertExpiryRequired,
            )
        },
        {
            let mut value = post();
            value.operational_profile = Some(alert.clone());
            value.expires_at_unix_seconds = Some(1_800_000_900);
            value.coarse_location = None;
            (
                "alert location",
                value,
                NewswireModelError::AlertLocationRequired,
            )
        },
        {
            let mut value = post();
            value.operational_profile = Some(alert);
            value.expires_at_unix_seconds = Some(1_800_000_900);
            value.source_claims.clear();
            (
                "alert source",
                value,
                NewswireModelError::AlertSourceClaimRequired,
            )
        },
        {
            let mut value = post();
            value.operational_profile = Some(request.clone());
            value.expires_at_unix_seconds = None;
            (
                "request expiry",
                value,
                NewswireModelError::RequestExpiryRequired,
            )
        },
        {
            let mut value = post();
            value.operational_profile = Some(request);
            value.expires_at_unix_seconds = Some(1_800_000_900);
            value.coarse_location = None;
            (
                "request location",
                value,
                NewswireModelError::RequestLocationRequired,
            )
        },
    ];
    for (name, value, expected) in cases {
        assert_eq!(encode_news_post(&value), Err(expected), "{name}");
    }

    let mut invalid_contact = post();
    invalid_contact.expires_at_unix_seconds = Some(1_800_000_900);
    invalid_contact.operational_profile = Some(OperationalProfileV1::Request(RequestProfileV1 {
        kind: RequestKind::Offer,
        needed_by_unix_seconds: None,
        contact_instructions: " ".into(),
    }));
    assert_eq!(
        encode_news_post(&invalid_contact),
        Err(NewswireModelError::FieldEmpty("contact_instructions"))
    );
    if let Some(OperationalProfileV1::Request(profile)) = &mut invalid_contact.operational_profile {
        profile.contact_instructions = "c".repeat(2_049);
    }
    assert_eq!(
        encode_news_post(&invalid_contact),
        Err(NewswireModelError::FieldTooLarge("contact_instructions"))
    );
}

#[test]
fn hostile_cbor_is_rejected_fail_closed() {
    let canonical = encode_space_descriptor(&space()).unwrap();
    assert_eq!(canonical[0], 0xa8, "fixture omits both optional space keys");

    let mut unknown = canonical.clone();
    unknown[0] = 0xa9;
    unknown.extend_from_slice(&[0x18, 0x63, 0x00]);

    let misordered = {
        let mut bytes = Vec::new();
        let mut encoder = minicbor::Encoder::new(&mut bytes);
        encoder.map(2).unwrap();
        encoder.u8(1).unwrap().bytes(&[0x10; 32]).unwrap();
        encoder.u8(0).unwrap().str(SPACE_SCHEMA).unwrap();
        bytes
    };
    let indefinite = {
        let mut bytes = vec![0xbf, 0x00];
        minicbor::Encoder::new(&mut bytes)
            .str(SPACE_SCHEMA)
            .unwrap();
        bytes.push(0xff);
        bytes
    };
    let mut trailing = canonical.clone();
    trailing.push(0x00);
    let mut widened = Vec::with_capacity(canonical.len() + 1);
    widened.push(canonical[0]);
    widened.extend_from_slice(&[0x18, 0x00]);
    widened.extend_from_slice(&canonical[2..]);

    let cases = [
        ("unknown key", unknown, NewswireModelError::UnknownKey(99)),
        (
            "misordered key",
            misordered,
            NewswireModelError::DuplicateOrMisorderedKey(0),
        ),
        (
            "indefinite map",
            indefinite,
            NewswireModelError::NonCanonical,
        ),
        (
            "trailing bytes",
            trailing,
            NewswireModelError::TrailingBytes,
        ),
        ("widened integer", widened, NewswireModelError::NonCanonical),
    ];
    for (name, bytes, expected) in cases {
        assert_eq!(decode_space_descriptor(&bytes), Err(expected), "{name}");
    }

    let oversized = vec![0u8; MAX_NEWSWIRE_PAYLOAD_BYTES + 1];
    assert_eq!(
        decode_space_descriptor(&oversized),
        Err(NewswireModelError::InputTooLarge)
    );
    let exact_limit = vec![0u8; MAX_NEWSWIRE_PAYLOAD_BYTES];
    assert_ne!(
        decode_space_descriptor(&exact_limit),
        Err(NewswireModelError::InputTooLarge)
    );
}
