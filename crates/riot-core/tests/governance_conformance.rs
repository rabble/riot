use riot_core::governance::action;
use riot_core::governance::record::{encode_record, record_id};
use riot_core::governance::{test_support as ts, RecordKind};

const VECTORS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/governance/governance-vectors.json"
);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn build_vectors() -> serde_json::Value {
    let mut kinds = serde_json::Map::new();
    for tag in 0..=21 {
        let kind = RecordKind::from_tag(tag).unwrap();
        let record = ts::seeded_record_for(kind);
        kinds.insert(
            format!("{kind:?}"),
            serde_json::json!({
                "encoding_hex": hex(&encode_record(&record)),
                "record_id_hex": hex(&record_id(&record)),
            }),
        );
    }
    let receipt = action::seeded_action_receipt();
    serde_json::json!({
        "records": kinds,
        "action_receipt": {
            "encoding_hex": hex(&action::encode_receipt(&receipt)),
            "action_hash_hex": hex(&action::action_hash(&receipt)),
        }
    })
}

#[test]
fn golden_vectors_match_committed_fixture() {
    let current = build_vectors();
    if std::env::var("REGEN").is_ok() {
        std::fs::create_dir_all(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/governance"
        ))
        .unwrap();
        std::fs::write(
            VECTORS_PATH,
            format!("{}\n", serde_json::to_string_pretty(&current).unwrap()),
        )
        .unwrap();
        return;
    }
    let committed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(VECTORS_PATH).expect("vectors file")).unwrap();
    assert_eq!(
        current, committed,
        "governance encodings/record_ids drifted"
    );
}

#[test]
fn every_kind_has_a_vector() {
    let vectors = build_vectors();
    assert_eq!(vectors["records"].as_object().unwrap().len(), 22);
    assert!(vectors["action_receipt"].is_object());
}

#[test]
fn no_record_authorizes_itself() {
    let record = ts::self_authorizing_record();
    assert_eq!(
        riot_core::governance::authorize::authorize_record(&record, &Default::default()),
        Err(riot_core::governance::GovernanceError::SelfAuthorization)
    );
}

#[test]
fn concurrent_role_restrictions_intersect() {
    let (records, survivor) = ts::two_concurrent_role_restrictions();
    let snapshot = riot_core::governance::evaluator::evaluate(&records, Some(2_000_000_000_000));
    assert!(snapshot.active_fingerprints.contains(&survivor));
    assert_eq!(
        snapshot
            .active_fingerprints
            .iter()
            .filter(|fingerprint| ts::is_role_fp(fingerprint))
            .count(),
        1
    );
}

#[test]
fn appeal_resolution_never_restores_revoked_authority() {
    let (records, fingerprint) = ts::revoke_then_favorable_appeal();
    let snapshot = riot_core::governance::evaluator::evaluate(&records, Some(2_000_000_000_000));
    assert!(snapshot.revoked.contains(&fingerprint));
    assert!(!snapshot.active_fingerprints.contains(&fingerprint));
}

#[test]
fn competing_migration_candidates_remain_a_fork() {
    assert_eq!(
        riot_core::governance::authorize::selected_migration(&ts::two_competing_migrations()),
        None
    );
}
