//! Reproduction for the iOS `testJoinAdditionalCommunityHoldsBothIsolates…`
//! failure: joining a SECOND community after the first has signed an alert
//! returns `SessionLimit`. Walks the same FFI sequence the Swift repository
//! does, so the failure is pinned to core/FFI rather than to Swift.

use riot_ffi::{open_local_profile, open_local_profile_with_database, AlertDraftInput};

fn alert_input(headline: &str) -> AlertDraftInput {
    AlertDraftInput {
        valid_from: None,
        expires_at: 4_000_000_000,
        language: "en".into(),
        urgency: riot_ffi::AlertUrgency::Immediate,
        severity: riot_ffi::AlertSeverity::Severe,
        certainty: riot_ffi::AlertCertainty::Observed,
        headline: headline.into(),
        description: "Bring jugs to the union hall".into(),
        affected_area_claim: None,
        source_claims: vec!["Union hall notice".into()],
        ai_assisted: false,
    }
}

#[test]
fn joining_a_second_community_after_signing_an_alert_succeeds() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("join.db").to_string_lossy().to_string();
    let profile = open_local_profile_with_database(db).expect("durable profile");

    // Community A, with one signed alert on its board.
    let a = profile
        .create_public_space("Community A".into())
        .expect("create A");
    let draft = profile
        .create_draft_alert(alert_input("Water shut off on 3rd St"))
        .expect("draft");
    profile.sign_draft(draft.draft_id).expect("sign");
    assert!(
        !profile.list_current_entries().expect("entries").is_empty(),
        "A has a board entry before the join"
    );

    // Community B lives on another device.
    let origin = open_local_profile().expect("origin profile");
    let b = origin
        .create_public_space("Community B".into())
        .expect("create B");

    // Join B by share reference, exactly as the iOS repository does — note the
    // descriptor id is a placeholder, which is what a share reference carries
    // before the descriptor itself has synced.
    let joined = profile.join_newswire_community(
        riot_ffi::PublicSpace {
            namespace_id: b.namespace_id.clone(),
            title: "Community B".into(),
            is_public: true,
        },
        "1".repeat(64),
        vec![5u8; 32],
    );

    match joined {
        Ok(space) => assert_eq!(space.namespace_id, b.namespace_id),
        Err(other) => panic!("joining a second community failed: {other:?}"),
    }

    assert_eq!(
        profile.list_communities().expect("held").len(),
        2,
        "both communities are held"
    );

    // Switching back to A must also work.
    profile
        .switch_community(a.namespace_id.clone(), vec![5u8; 32])
        .expect("switch back to A");
}

/// Control: the identical sequence with NO alert signed into A. If this passes
/// while the test above fails, the alert entry is what breaks the rebuilt offer.
#[test]
fn switching_back_without_an_alert_succeeds() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("join2.db").to_string_lossy().to_string();
    let profile = open_local_profile_with_database(db).expect("durable profile");

    let a = profile
        .create_public_space("Community A".into())
        .expect("create A");
    let origin = open_local_profile().expect("origin profile");
    let b = origin
        .create_public_space("Community B".into())
        .expect("create B");

    profile
        .join_newswire_community(
            riot_ffi::PublicSpace {
                namespace_id: b.namespace_id.clone(),
                title: "Community B".into(),
                is_public: true,
            },
            "1".repeat(64),
            vec![5u8; 32],
        )
        .expect("join B");

    profile
        .switch_community(a.namespace_id.clone(), vec![5u8; 32])
        .expect("switch back to A without an alert");
}
