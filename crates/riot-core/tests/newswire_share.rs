//! Reachable error and rejection paths of the Newswire share-reference codec,
//! driven entirely through the public API with real signed records.
//!
//! The happy path (build → encode → decode round-trip on a genuine descriptor)
//! is already proven by `newswire_import.rs`; this suite pins the paths a
//! well-behaved caller never hits but a hostile or corrupted input does: a
//! non-descriptor anchor, a substituted descriptor, and every way a share
//! string can be malformed.

use riot_core::newswire::{
    build_share_reference, create_signed_news_post, create_signed_space_descriptor,
    decode_share_reference, decode_share_reference_with_handle, encode_share_reference,
    encode_share_reference_with_handle, inspect_news_record, verify_descriptor_matches, NewsPostV1,
    ShareReferenceError, SpaceDescriptorV1, VerifiedNewswireRecord, SHARE_REFERENCE_PREFIX,
};
use riot_core::willow::{generate_communal_author_for_namespace, generate_space_organizer_author};

/// A genuine, signed-and-verified space descriptor plus a writer inside the
/// same communal namespace.
fn signed_space() -> (VerifiedNewswireRecord, riot_core::willow::EvidenceAuthor) {
    let organizer = generate_space_organizer_author().expect("organizer");
    let namespace_id = *organizer.namespace_id().as_bytes();
    let writer = generate_communal_author_for_namespace(namespace_id).expect("writer");
    let signed = create_signed_space_descriptor(
        &organizer,
        SpaceDescriptorV1 {
            namespace_id,
            name: "Riverside".into(),
            summary: "A local human newswire.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
            editorial_roster: vec![],
            predecessor: None,
            successor: None,
        },
    )
    .expect("descriptor");
    (
        inspect_news_record(&signed.signed).expect("verified"),
        writer,
    )
}

fn signed_post(
    descriptor: &VerifiedNewswireRecord,
    writer: &riot_core::willow::EvidenceAuthor,
) -> VerifiedNewswireRecord {
    let signed = create_signed_news_post(
        writer,
        descriptor,
        NewsPostV1 {
            space_descriptor_entry_id: descriptor.entry_id(),
            headline: "Update".into(),
            body: "Human report.".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec![],
            operational_profile: None,
            ai_assisted: false,
        },
    )
    .expect("post");
    inspect_news_record(&signed.signed).expect("verified post")
}

#[test]
fn a_reference_round_trips_and_binds_the_descriptor_content_digest() {
    let (descriptor, _writer) = signed_space();
    let reference = build_share_reference(&descriptor).expect("reference");
    assert_eq!(reference.namespace_id, descriptor.namespace_id());
    assert_eq!(reference.descriptor_entry_id, descriptor.entry_id());

    let encoded = encode_share_reference(&reference);
    assert!(encoded.starts_with(SHARE_REFERENCE_PREFIX));
    assert_eq!(decode_share_reference(&encoded), Ok(reference.clone()));
    assert!(verify_descriptor_matches(&reference, &descriptor));
}

#[test]
fn a_non_descriptor_record_cannot_anchor_a_reference() {
    let (descriptor, writer) = signed_space();
    let post = signed_post(&descriptor, &writer);
    assert_eq!(
        build_share_reference(&post),
        Err(ShareReferenceError::NotADescriptor)
    );
    // verify_descriptor_matches folds the build failure into a plain `false`.
    let reference = build_share_reference(&descriptor).expect("reference");
    assert!(!verify_descriptor_matches(&reference, &post));
}

#[test]
fn a_different_descriptor_fails_the_digest_binding() {
    let (descriptor, _writer) = signed_space();
    let reference = build_share_reference(&descriptor).expect("reference");
    // A distinct community produces a distinct namespace, entry id, and digest.
    let (other, _) = signed_space();
    assert!(!verify_descriptor_matches(&reference, &other));
}

#[test]
fn decode_rejects_a_foreign_scheme_prefix() {
    assert_eq!(
        decode_share_reference("https://example.com/not-a-reference"),
        Err(ShareReferenceError::Malformed)
    );
}

#[test]
fn decode_rejects_too_few_and_too_many_coordinates() {
    let hex = "aa".repeat(32);
    let two = format!("{SHARE_REFERENCE_PREFIX}{hex}/{hex}");
    assert_eq!(
        decode_share_reference(&two),
        Err(ShareReferenceError::Malformed)
    );
    let four = format!("{SHARE_REFERENCE_PREFIX}{hex}/{hex}/{hex}/{hex}");
    assert_eq!(
        decode_share_reference(&four),
        Err(ShareReferenceError::Malformed)
    );
}

#[test]
fn decode_rejects_wrong_length_uppercase_and_non_hex_coordinates() {
    let good = "aa".repeat(32);
    // 63 characters — one short of a 32-byte hex coordinate.
    let short = format!("{SHARE_REFERENCE_PREFIX}{}/{good}/{good}", "a".repeat(63));
    assert_eq!(
        decode_share_reference(&short),
        Err(ShareReferenceError::Malformed)
    );
    // Uppercase hex is rejected even though it is otherwise valid hex.
    let upper = format!("{SHARE_REFERENCE_PREFIX}{}/{good}/{good}", "AA".repeat(32));
    assert_eq!(
        decode_share_reference(&upper),
        Err(ShareReferenceError::Malformed)
    );
    // Correct length and case, but a non-hex digit.
    let non_hex = format!("{SHARE_REFERENCE_PREFIX}{}/{good}/{good}", "zz".repeat(32));
    assert_eq!(
        decode_share_reference(&non_hex),
        Err(ShareReferenceError::Malformed)
    );
}

#[test]
fn share_reference_error_renders_a_stable_debug_code() {
    assert_eq!(ShareReferenceError::Malformed.to_string(), "Malformed");
    assert_eq!(
        ShareReferenceError::NotADescriptor.to_string(),
        "NotADescriptor"
    );
    assert_eq!(
        ShareReferenceError::EncodingFailed.to_string(),
        "EncodingFailed"
    );
}

// ---------------------------------------------------------------------------
// Bundled-handle URIs: the self-certifying `+shortname` prefix rides on the
// join URI so one string carries both identity and reachability.
// ---------------------------------------------------------------------------

#[test]
fn a_bundled_handle_uri_round_trips_and_carries_the_shortname() {
    let (descriptor, _writer) = signed_space();
    let reference = build_share_reference(&descriptor).expect("reference");

    let encoded = encode_share_reference_with_handle(&reference, "riverside");
    // The form: riot://newswire/join/v1/+riverside/<ns>/<entry>/<digest>
    assert!(
        encoded.starts_with("riot://newswire/join/v1/+riverside/"),
        "the +shortname prefix follows the scheme, got {encoded:?}"
    );

    let (decoded_ref, shortname) =
        decode_share_reference_with_handle(&encoded).expect("round-trip");
    assert_eq!(decoded_ref, reference);
    assert_eq!(shortname.as_deref(), Some("riverside"));
}

#[test]
fn a_bundled_handle_uri_still_decodes_through_the_plain_decoder() {
    // The plain `decode_share_reference` MUST accept the prefixed form too —
    // stripping the `+shortname/` and returning just the reference. This is the
    // backward-compat property that lets the existing join machinery consume
    // bundled URIs with no change.
    let (descriptor, _writer) = signed_space();
    let reference = build_share_reference(&descriptor).expect("reference");
    let encoded = encode_share_reference_with_handle(&reference, "riverside");

    assert_eq!(
        decode_share_reference(&encoded),
        Ok(reference),
        "the plain decoder strips the +shortname prefix"
    );
}

#[test]
fn a_bare_uri_without_a_handle_prefix_still_decodes_with_handle_none() {
    // The handle-aware decoder MUST accept the bare form too — shortname is None.
    let (descriptor, _writer) = signed_space();
    let reference = build_share_reference(&descriptor).expect("reference");
    let bare = encode_share_reference(&reference);

    let (decoded_ref, shortname) = decode_share_reference_with_handle(&bare).expect("bare decodes");
    assert_eq!(decoded_ref, reference);
    assert_eq!(shortname, None);
}

#[test]
fn a_bundled_handle_with_a_bad_shortname_is_rejected() {
    let (descriptor, _writer) = signed_space();
    let reference = build_share_reference(&descriptor).expect("reference");
    // Uppercase, underscore, and too-short shortnames are not valid handle labels.
    for bad in ["Riverside", "river_side", "ab"] {
        // Forge the URI directly since encode validates; decode is the authority.
        let bare = encode_share_reference(&reference);
        let forged = bare.replacen(
            SHARE_REFERENCE_PREFIX,
            &format!("{SHARE_REFERENCE_PREFIX}+{bad}/"),
            1,
        );
        assert!(
            decode_share_reference_with_handle(&forged).is_err(),
            "shortname {bad:?} should be rejected"
        );
    }
}

#[test]
fn a_minus_sigil_prefix_is_rejected_on_a_communal_namespace() {
    // A `-shortname` (owned) prefix on a communal namespace (even final byte) is
    // a sigil/parity lie and must be rejected.
    let (descriptor, _writer) = signed_space();
    let reference = build_share_reference(&descriptor).expect("reference");
    let bare = encode_share_reference(&reference);
    let forged = bare.replacen(
        SHARE_REFERENCE_PREFIX,
        &format!("{SHARE_REFERENCE_PREFIX}-bad/"),
        1,
    );
    assert!(
        decode_share_reference_with_handle(&forged).is_err(),
        "a - sigil on a communal namespace must be rejected"
    );
}
