use std::time::{SystemTime, UNIX_EPOCH};

use riot_ffi::{
    open_local_profile, AlertCertainty, AlertDraftInput, AlertFreshness, AlertSeverity,
    AlertUrgency, MobileError, PublicSpace, SignedAlert,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn expires_later() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after unix epoch")
        .as_secs()
        + 3_600
}

fn draft() -> AlertDraftInput {
    AlertDraftInput {
        valid_from: None,
        expires_at: expires_later(),
        language: "en".into(),
        urgency: AlertUrgency::Immediate,
        severity: AlertSeverity::Severe,
        certainty: AlertCertainty::Observed,
        headline: "Ferry terminal access restricted".into(),
        description: "Use the north entrance; the south pier is closed for inspection.".into(),
        affected_area_claim: None,
        source_claims: vec!["Two field observers".into()],
        ai_assisted: true,
    }
}

fn profile_with_space() -> std::sync::Arc<riot_ffi::MobileProfile> {
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Bounded incident".into())
        .unwrap();
    profile
}

fn import_one(profile: &riot_ffi::MobileProfile, signed: &SignedAlert) {
    let preview = profile
        .inspect_bytes(signed.bundle_bytes.clone(), "nearby-device".into())
        .unwrap();
    preview
        .create_plan(vec![signed.entry.entry_id.clone()])
        .unwrap()
        .accept()
        .unwrap();
}

fn signed_mismatched_path_bundle() -> (PublicSpace, Vec<u8>) {
    use riot_core::import::encode_bundle;
    use riot_core::model::{encode_alert, AlertPayload, Certainty, Severity, Urgency};
    use riot_core::willow::{
        authorise_entry, build_alert_entry, encode_capability, encode_entry,
        generate_communal_author, SignedWillowEntry,
    };

    let author = generate_communal_author().unwrap();
    let identity = author.identity();
    let payload = encode_alert(&AlertPayload {
        object_id: [1; 16],
        revision_id: [2; 16],
        created_at: 1_800_000_000,
        valid_from: None,
        expires_at: 1_900_000_000,
        language: "en".into(),
        urgency: Urgency::Immediate,
        severity: Severity::Severe,
        certainty: Certainty::Observed,
        headline: "Mismatched canonical path".into(),
        description: "The signature is valid but the path object ID differs.".into(),
        affected_area_claim: None,
        source_claims: vec!["adversarial fixture".into()],
        ai_assisted: false,
    })
    .unwrap();
    let entry = build_alert_entry(&author, &[9; 16], &[2; 16], 100, &payload).unwrap();
    let authorised = authorise_entry(&author, entry).unwrap();
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let bundle = encode_bundle(&[SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload,
    }])
    .unwrap();
    (
        PublicSpace {
            namespace_id: hex(&identity.namespace_id),
            title: "Mismatched path fixture".into(),
            is_public: true,
        },
        bundle,
    )
}

#[test]
fn mobile_profile_runs_the_public_space_alert_and_selected_import_flow() {
    let author = open_local_profile().expect("open local author profile");
    let identity = author.identity().expect("public identity");
    assert_eq!(identity.namespace_id.len(), 64, "full namespace id");
    assert_eq!(identity.signing_key_id.len(), 64, "full public signer id");

    let space = author
        .create_public_space("Harbor District Evacuation".into())
        .expect("create public space");
    assert_eq!(space.namespace_id, identity.namespace_id);
    assert!(space.is_public);

    let draft = author
        .create_draft_alert(draft())
        .expect("create local draft");
    let signed = author.sign_draft(draft.draft_id).expect("human sign draft");
    assert_eq!(signed.entry.entry_id.len(), 64, "full canonical entry id");
    assert_eq!(signed.entry.signer_id, identity.signing_key_id);
    assert!(signed.entry.ai_assisted);
    assert!(matches!(signed.entry.freshness, AlertFreshness { .. }));
    assert!(
        !signed.bundle_bytes.is_empty(),
        "signed bundle is portable bytes"
    );

    let local_entries = author.list_current_entries().expect("list local entries");
    assert_eq!(local_entries, vec![signed.entry.clone()]);

    let receiver = open_local_profile().expect("open receiving profile");
    let joined = receiver
        .join_public_space(space.clone())
        .expect("join sender public space");
    let receiver_identity = receiver.identity().expect("receiver public identity");
    assert_eq!(joined.namespace_id, space.namespace_id);
    assert_eq!(receiver_identity.namespace_id, identity.namespace_id);
    assert_ne!(receiver_identity.signing_key_id, identity.signing_key_id);
    let preview = receiver
        .inspect_bytes(signed.bundle_bytes, "nearby-device".into())
        .expect("inspect portable bytes");
    let inspected = preview.eligible_entries().expect("inspect records");
    assert_eq!(inspected, vec![signed.entry.clone()]);

    let plan = preview
        .create_plan(vec![signed.entry.entry_id.clone()])
        .expect("select import entry");
    let acceptance = plan.accept().expect("accept selected plan");
    assert_eq!(
        acceptance.accepted_entry_ids,
        vec![signed.entry.entry_id.clone()]
    );
    assert_eq!(receiver.list_current_entries().unwrap(), vec![signed.entry]);
}

#[test]
fn mobile_profile_rejects_imports_outside_the_joined_public_namespace() {
    let first = open_local_profile().expect("first profile");
    let first_space = first.create_public_space("First incident".into()).unwrap();

    let second = open_local_profile().expect("second profile");
    second.create_public_space("Other incident".into()).unwrap();
    let draft = second.create_draft_alert(draft()).unwrap();
    let foreign = second.sign_draft(draft.draft_id).unwrap();

    let receiver = open_local_profile().expect("receiver");
    receiver.join_public_space(first_space).unwrap();
    assert!(matches!(
        receiver.inspect_bytes(foreign.bundle_bytes, "nearby-device".into()),
        Err(riot_ffi::MobileError::ImportRejected)
    ));
    assert!(receiver.list_current_entries().unwrap().is_empty());
}

#[test]
fn mobile_profile_validates_drafts_before_bounded_retention() {
    let profile = profile_with_space();
    let mut invalid = Vec::new();

    let mut value = draft();
    value.language = "x".into();
    invalid.push(value);
    let mut value = draft();
    value.headline = " ".into();
    invalid.push(value);
    let mut value = draft();
    value.headline = "h".repeat(513);
    invalid.push(value);
    let mut value = draft();
    value.description = "d".repeat(65_537);
    invalid.push(value);
    let mut value = draft();
    value.affected_area_claim = Some(" ".into());
    invalid.push(value);
    let mut value = draft();
    value.source_claims.clear();
    invalid.push(value);
    let mut value = draft();
    value.source_claims = vec!["claim".into(); 17];
    invalid.push(value);
    let mut value = draft();
    value.source_claims = vec![" ".into()];
    invalid.push(value);
    let mut value = draft();
    value.source_claims = vec!["s".repeat(1_025)];
    invalid.push(value);
    let mut value = draft();
    value.expires_at = 1;
    invalid.push(value);

    for invalid in invalid {
        assert!(matches!(
            profile.create_draft_alert(invalid),
            Err(riot_ffi::MobileError::InvalidInput)
        ));
    }

    for _ in 0..64 {
        profile
            .create_draft_alert(draft())
            .expect("within draft cap");
    }
    assert!(matches!(
        profile.create_draft_alert(draft()),
        Err(riot_ffi::MobileError::SessionLimit)
    ));
}

#[test]
fn mobile_import_selection_is_bounded_and_rejects_duplicates_before_planning() {
    let sender = profile_with_space();
    let space = sender
        .create_public_space("Bounded incident".into())
        .unwrap();
    let draft = sender.create_draft_alert(draft()).unwrap();
    let signed = sender.sign_draft(draft.draft_id).unwrap();

    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space).unwrap();
    let preview = receiver
        .inspect_bytes(signed.bundle_bytes, "nearby-device".into())
        .unwrap();
    assert!(matches!(
        preview.create_plan(vec![signed.entry.entry_id.clone(); 65]),
        Err(riot_ffi::MobileError::SessionLimit)
    ));
    assert!(matches!(
        preview.create_plan(vec![signed.entry.entry_id.clone(); 2]),
        Err(riot_ffi::MobileError::InvalidInput)
    ));
    preview
        .create_plan(vec![signed.entry.entry_id])
        .expect("rejections retain preview workflow");
}

#[test]
fn accepting_a_plan_consumes_both_mobile_plan_and_preview_handles() {
    let sender = profile_with_space();
    let space = sender
        .create_public_space("Bounded incident".into())
        .unwrap();
    let draft = sender.create_draft_alert(draft()).unwrap();
    let signed = sender.sign_draft(draft.draft_id).unwrap();

    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space).unwrap();
    let preview = receiver
        .inspect_bytes(signed.bundle_bytes, "nearby-device".into())
        .unwrap();
    let plan = preview.create_plan(vec![signed.entry.entry_id]).unwrap();
    plan.accept().unwrap();

    assert!(matches!(
        preview.eligible_entries(),
        Err(riot_ffi::MobileError::PreviewConsumed)
    ));
    assert!(matches!(
        plan.accept(),
        Err(riot_ffi::MobileError::PlanConsumed)
    ));
}

#[test]
fn rejected_invalid_only_inspect_preserves_the_prior_preview_and_plan() {
    let sender = profile_with_space();
    let space = sender
        .create_public_space("Bounded incident".into())
        .unwrap();
    let draft = sender.create_draft_alert(draft()).unwrap();
    let signed = sender.sign_draft(draft.draft_id).unwrap();
    let mut invalid_only = signed.bundle_bytes.clone();
    *invalid_only.last_mut().expect("non-empty bundle") ^= 1;

    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space).unwrap();
    let preview = receiver
        .inspect_bytes(signed.bundle_bytes, "nearby-device".into())
        .unwrap();
    let plan = preview
        .create_plan(vec![signed.entry.entry_id.clone()])
        .unwrap();

    assert!(matches!(
        receiver.inspect_bytes(invalid_only, "nearby-device".into()),
        Err(riot_ffi::MobileError::ImportRejected)
    ));
    assert_eq!(preview.eligible_entries().unwrap(), vec![signed.entry]);
    plan.accept().expect("prior selected plan remains usable");
}

#[test]
fn correctly_signed_alert_with_payload_ids_mismatched_to_entry_path_is_rejected() {
    let (space, bundle) = signed_mismatched_path_bundle();
    let receiver = open_local_profile().unwrap();
    receiver.join_public_space(space).unwrap();

    assert!(matches!(
        receiver.inspect_bytes(bundle, "nearby-device".into()),
        Err(MobileError::ImportRejected)
    ));
}

#[test]
fn current_entries_are_identical_after_opposite_import_orders() {
    let sender = open_local_profile().unwrap();
    let space = sender
        .create_public_space("Ordering fixture".into())
        .unwrap();
    let first_draft = sender.create_draft_alert(draft()).unwrap();
    let first = sender.sign_draft(first_draft.draft_id).unwrap();
    let mut second_input = draft();
    second_input.headline = "Second ordering alert".into();
    let second_draft = sender.create_draft_alert(second_input).unwrap();
    let second = sender.sign_draft(second_draft.draft_id).unwrap();

    let receiver_a = open_local_profile().unwrap();
    receiver_a.join_public_space(space.clone()).unwrap();
    import_one(&receiver_a, &first);
    import_one(&receiver_a, &second);

    let receiver_b = open_local_profile().unwrap();
    receiver_b.join_public_space(space).unwrap();
    import_one(&receiver_b, &second);
    import_one(&receiver_b, &first);

    let entries_a = receiver_a.list_current_entries().unwrap();
    let entries_b = receiver_b.list_current_entries().unwrap();
    assert_eq!(entries_a, entries_b);
    assert!(entries_a
        .windows(2)
        .all(|pair| pair[0].entry_id < pair[1].entry_id));
}

#[test]
fn exported_mobile_surface_does_not_publish_key_material_or_willow_generics() {
    // This is intentionally a contract over the FFI declarations, rather
    // than the crate implementation: internals may use Core types, but the
    // generated mobile API must never name them.
    let surface = include_str!("../src/mobile_api.rs");
    for forbidden in [
        "EvidenceAuthor",
        "SubspaceSecret",
        "NamespaceSecret",
        "WriteCapability",
        "AuthorisedEntry",
        "willow25::",
        "private_key",
        "secret_key",
    ] {
        assert!(
            !surface.contains(forbidden),
            "FFI declaration leaks forbidden type or material: {forbidden}"
        );
    }
}
