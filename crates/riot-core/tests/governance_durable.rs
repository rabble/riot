use riot_core::governance::repository::AuthorityRepository;
use riot_core::governance::test_support::{
    fingerprint_of_issued, genesis_record, issued_record, issued_record_with_missing_parent,
    revoke_record,
};
use riot_core::store::{DatabaseConfig, RiotDatabase};

const NOW: u64 = 2_000_000_000_000;

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "riot-governance-{tag}-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn snapshot_survives_restart_and_rebuild_is_deterministic() {
    let path = temp_path("restart");
    let genesis = genesis_record([9u8; 32]);
    let issued = issued_record([9u8; 32], 8);
    let before = {
        let repository = AuthorityRepository::sqlite(
            RiotDatabase::open(&path, DatabaseConfig::default()).unwrap(),
        );
        repository.ingest(&genesis).unwrap();
        repository.ingest(&issued).unwrap();
        repository.snapshot(NOW).unwrap()
    };
    let after =
        AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap())
            .snapshot(NOW)
            .unwrap();
    let again =
        AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap())
            .snapshot(NOW)
            .unwrap();
    assert_eq!(before, after);
    assert_eq!(after, again);
    assert!(after
        .active_fingerprints
        .contains(&fingerprint_of_issued(&issued)));
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_revoked_capability_is_not_resurrected_after_restart() {
    let path = temp_path("revoke");
    let issued = issued_record([9u8; 32], 8);
    let fingerprint = fingerprint_of_issued(&issued);
    {
        let repository = AuthorityRepository::sqlite(
            RiotDatabase::open(&path, DatabaseConfig::default()).unwrap(),
        );
        repository.ingest(&genesis_record([9u8; 32])).unwrap();
        repository.ingest(&issued).unwrap();
        repository.ingest(&revoke_record(fingerprint)).unwrap();
    }
    let snapshot =
        AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap())
            .snapshot(NOW)
            .unwrap();
    assert!(snapshot.revoked.contains(&fingerprint));
    assert!(!snapshot.active_fingerprints.contains(&fingerprint));
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_restored_backup_is_quarantined_and_activates_no_authority() {
    let source = temp_path("source");
    let backup = temp_path("backup");
    let destination = temp_path("destination");
    let issued = issued_record([9u8; 32], 8);
    let fingerprint = fingerprint_of_issued(&issued);
    let manifest = {
        let database = RiotDatabase::open(&source, DatabaseConfig::default()).unwrap();
        let repository = AuthorityRepository::sqlite(database.clone());
        repository.ingest(&genesis_record([9u8; 32])).unwrap();
        repository.ingest(&issued).unwrap();
        database.backup_to(&backup).unwrap()
    };
    let database =
        RiotDatabase::restore_from(&destination, &backup, &manifest, DatabaseConfig::default())
            .unwrap();
    assert!(database.authority_quarantined().unwrap());
    let repository = AuthorityRepository::sqlite(database);
    assert!(repository
        .load_journal()
        .unwrap()
        .iter()
        .any(|record| record.kind == riot_core::governance::RecordKind::CapabilityIssued));
    let snapshot = repository.snapshot_respecting_quarantine(NOW).unwrap();
    assert!(snapshot.active_fingerprints.is_empty());
    assert!(!snapshot.active_fingerprints.contains(&fingerprint));
    for path in [source, backup, destination] {
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn a_missing_parent_record_stays_quarantined() {
    let path = temp_path("orphan");
    {
        let repository = AuthorityRepository::sqlite(
            RiotDatabase::open(&path, DatabaseConfig::default()).unwrap(),
        );
        repository
            .ingest(&issued_record_with_missing_parent([9u8; 32], 8))
            .unwrap();
    }
    let snapshot =
        AuthorityRepository::sqlite(RiotDatabase::open(&path, DatabaseConfig::default()).unwrap())
            .snapshot(NOW)
            .unwrap();
    assert!(snapshot.active_fingerprints.is_empty());
    let _ = std::fs::remove_file(path);
}
