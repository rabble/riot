//! Composite-site Unit 2 Task 4 — the durable manifest version floor MUST
//! survive an app restart. A memory-only floor re-opens rollback on relaunch;
//! these tests persist the floor through a real file-backed `RiotDatabase`,
//! drop the handle (simulated relaunch), reopen from disk, and prove a
//! rollback/downgrade is STILL refused.

#![cfg(feature = "sqlite")]

use std::fs;
use std::path::PathBuf;

use riot_core::site::manifest::{
    RequireTransport, SiteDisplay, SiteLayout, SiteManifestV1, SiteMemberV1, SiteRole, SiteRule,
    TransportPolicyV1,
};
use riot_core::site::{admit_manifest_version, VersionFloorOutcome};
use riot_core::store::{DatabaseConfig, RiotDatabase};

/// A unique temp directory removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "riot-vfloor-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        Self(base)
    }

    fn database(&self) -> PathBuf {
        self.0.join("riot.db")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const ROOT: [u8; 32] = [0x42; 32];

fn manifest(version: u64, require: RequireTransport) -> SiteManifestV1 {
    SiteManifestV1 {
        root: ROOT,
        members: vec![SiteMemberV1 {
            ns: ROOT,
            role: SiteRole::Masthead,
            rule: SiteRule::OwnedWrite,
            display: SiteDisplay::FrontArticles,
        }],
        moderation_path: vec![b"mod".to_vec()],
        transport_policy: TransportPolicyV1 {
            allow: vec![],
            require,
        },
        version,
        layout: SiteLayout::SiteDefault,
    }
}

#[test]
fn version_rollback_is_refused_after_restart() {
    let dir = TestDir::new("rollback");
    let path = dir.database();

    // First launch: seed the floor at version 5, then close the app.
    {
        let db = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
        assert_eq!(
            admit_manifest_version(&db, &ROOT, &manifest(5, RequireTransport::None)),
            Ok(VersionFloorOutcome::Accepted)
        );
    }

    // Relaunch from disk: a version-4 manifest must STILL be refused — the floor
    // is durable, not memory-only.
    {
        let db = RiotDatabase::open(&path, DatabaseConfig::default()).expect("reopen");
        assert_eq!(
            admit_manifest_version(&db, &ROOT, &manifest(4, RequireTransport::None)),
            Ok(VersionFloorOutcome::RollbackRejected)
        );
    }
}

#[test]
fn require_downgrade_at_higher_version_is_refused_after_restart() {
    let dir = TestDir::new("downgrade");
    let path = dir.database();

    // First launch: floor at version 5 requiring arti (strictness 1).
    {
        let db = RiotDatabase::open(&path, DatabaseConfig::default()).expect("open");
        assert_eq!(
            admit_manifest_version(&db, &ROOT, &manifest(5, RequireTransport::Arti)),
            Ok(VersionFloorOutcome::Accepted)
        );
    }

    // Relaunch: a HIGHER version (6) that lowers require arti -> none passes the
    // version check but must still be refused (require-monotonicity), from disk.
    {
        let db = RiotDatabase::open(&path, DatabaseConfig::default()).expect("reopen");
        assert_eq!(
            admit_manifest_version(&db, &ROOT, &manifest(6, RequireTransport::None)),
            Ok(VersionFloorOutcome::RequireDowngradeRejected)
        );
        // A legitimate higher version keeping the require floor still advances.
        assert_eq!(
            admit_manifest_version(&db, &ROOT, &manifest(6, RequireTransport::Arti)),
            Ok(VersionFloorOutcome::Accepted)
        );
    }
}
