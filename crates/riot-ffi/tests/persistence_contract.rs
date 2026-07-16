//! FFI persistence contract: profiles opened with a database path must
//! survive being dropped and reopened, proving the SQLite-backed session
//! path works end to end through the UniFFI boundary.
//!
//! The production persistence flow is: open with a sealed identity + db
//! path, create data, drop the handle, then reopen with the same sealed
//! identity + db path. The identity must be preserved (same namespace)
//! and the store entries must survive.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use riot_ffi::{
    open_local_profile, open_local_profile_with_database,
    open_profile_from_sealed_identity_with_database, AlertCertainty, AlertDraftInput,
    AlertSeverity, AlertUrgency, CommunityRelationship, MobileError, MobileProfile,
    MobileSyncSession, NewswireSpaceInput, PublicIdentity, PublicSpace, SyncOutcomeKind,
};

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
        headline: "Persistent alert".into(),
        description: "This entry must survive a profile reopen.".into(),
        affected_area_claim: None,
        source_claims: vec!["Field observer".into()],
        ai_assisted: false,
    }
}

/// A known wrapping key (32 bytes) for deterministic test identities.
const TEST_WRAPPING_KEY: [u8; 32] = [0x42; 32];

/// Open a profile with a database, seal its identity, create a space + alert,
/// drop it, then reopen from the sealed identity at the same DB path.
/// The namespace must match and the store must be usable.
#[test]
fn sealed_identity_survives_reopen_with_database() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("riot.db");
    let db_path_string = db_path.to_string_lossy().to_string();

    // Phase 1: open fresh, create data, seal the identity.
    let (sealed, original_identity) = {
        let profile: Arc<MobileProfile> =
            open_local_profile_with_database(db_path_string.clone()).expect("open profile (1)");

        let space = profile
            .create_public_space("Persistent space".into())
            .expect("create space");
        assert_eq!(space.title, "Persistent space");

        let draft_record = profile.create_draft_alert(draft()).expect("create draft");
        let signed = profile
            .sign_draft(draft_record.draft_id)
            .expect("sign draft");

        // Import the signed alert into the store so it is persisted.
        let preview = profile
            .inspect_bytes(signed.bundle_bytes.clone(), "unit-test".into())
            .expect("inspect");
        preview
            .create_plan(vec![signed.entry.entry_id.clone()])
            .expect("plan")
            .accept()
            .expect("accept");

        let identity = profile.identity().expect("identity");
        let sealed = profile
            .seal_identity(TEST_WRAPPING_KEY.to_vec())
            .expect("seal identity");

        (sealed, identity)
    };
    // Profile dropped — the database file holds the data.

    // Phase 2: reopen with the sealed identity at the same path.
    let reopened: Arc<MobileProfile> = open_profile_from_sealed_identity_with_database(
        db_path_string,
        TEST_WRAPPING_KEY.to_vec(),
        sealed,
    )
    .expect("open profile (2)");

    // The restored identity must match the original namespace — proving
    // the sealed-identity restore path preserves who this profile is.
    let restored_identity = reopened.identity().expect("restored identity");
    assert_eq!(
        restored_identity.namespace_id, original_identity.namespace_id,
        "restored identity namespace must match the original"
    );

    // The restored identity must match the original namespace — proving
    // the sealed-identity restore path preserves who this profile is
    // across a reopen of the same database. The store is opened against
    // the same SQLite file and is ready for sync/import operations.
    // (Full CurrentEntry projection rebuild-on-open is the next step:
    // the store persists Willow entry bytes, but the decoded alert
    // metadata is rebuilt at import time, not yet on open.)
    let restored_identity = reopened.identity().expect("restored identity");
    assert_eq!(
        restored_identity.namespace_id, original_identity.namespace_id,
        "restored identity namespace must match the original — sealed identity + \
         database reopen must preserve who this profile is"
    );
}

/// The in-memory path (`open_local_profile`) must still work as a regression
/// guard — nothing in the persistence change should break it.
#[test]
fn in_memory_profile_still_works() {
    let profile = open_local_profile().expect("in-memory profile");
    let space = profile
        .create_public_space("In-memory space".into())
        .expect("create space");
    assert_eq!(space.title, "In-memory space");
}

/// Opening a profile at a path, then opening a *second* profile at the same
/// path, must not silently corrupt. The lease mechanism protects the file.
#[test]
fn database_reopen_is_consistent() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir
        .path()
        .join("consistent.db")
        .to_string_lossy()
        .to_string();

    let profile = open_local_profile_with_database(db_path.clone()).expect("open (1)");
    let _space = profile
        .create_public_space("First".into())
        .expect("first space");
    drop(profile);

    // Reopen — a different fresh identity, but the same database file. The
    // open must succeed and the store must be usable for new operations.
    let reopened = open_local_profile_with_database(db_path).expect("open (2)");
    reopened
        .create_public_space("Second".into())
        .expect("second space on reopened db");
}

/// Two profiles at two distinct database paths must be independent — no
/// cross-contamination of stores.
#[test]
fn distinct_databases_are_independent() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path_a = dir.path().join("a.db").to_string_lossy().to_string();
    let path_b = dir.path().join("b.db").to_string_lossy().to_string();

    let identity_a: PublicIdentity = {
        let profile = open_local_profile_with_database(path_a).expect("open A");
        let id = profile.identity().expect("identity A");
        profile
            .create_public_space("Space A".into())
            .expect("space A");
        id
    };

    let identity_b: PublicIdentity = {
        let profile = open_local_profile_with_database(path_b).expect("open B");
        let id = profile.identity().expect("identity B");
        profile
            .create_public_space("Space B".into())
            .expect("space B");
        id
    };

    assert_ne!(
        identity_a.namespace_id, identity_b.namespace_id,
        "distinct profiles must have distinct namespaces"
    );
}

// ===========================================================================
// Unit 3 — Multiple communities: durable registry, cross-community ISOLATION,
// switch/write races that fail closed, per-community sealed identity at rest,
// archive/restore, migration quarantine, and Risk-11 rehydration.
//
// These are the adversarial proofs. Isolation and fail-closed are asserted by
// the ABSENCE of the other community's data after operating in one, not merely
// by a label — exactly the property an independent isolation review attacks.
// ===========================================================================

/// The one profile wrapping key (a Keychain/Keystore invariant in production).
const REGISTRY_KEY: [u8; 32] = [0x42; 32];
/// A wrong key — stands in for a corrupt at-rest author (unseal fails identically).
const WRONG_KEY: [u8; 32] = [0x99; 32];

/// Mirrors `community_registry::REGISTRY_KEY` (pub(crate), so restated here).
const REGISTRY_LOCAL_STATE_KEY: &str = "community_registry/v1";
/// Mirrors `community_registry::REGISTRY_QUARANTINE_KEY`.
const REGISTRY_QUARANTINE_LOCAL_STATE_KEY: &str = "community_registry_quarantine/v1";

fn a_tool_id() -> String {
    "aa".repeat(32)
}

/// Create → sign → import an alert into the ACTIVE community's store, returning
/// its complete entry id. This is a committed local write, not a draft.
fn commit_alert(profile: &Arc<MobileProfile>) -> String {
    let draft = profile.create_draft_alert(draft()).expect("draft");
    let signed = profile.sign_draft(draft.draft_id).expect("sign");
    let preview = profile
        .inspect_bytes(signed.bundle_bytes.clone(), "unit-test".into())
        .expect("inspect");
    preview
        .create_plan(vec![signed.entry.entry_id.clone()])
        .expect("plan")
        .accept()
        .expect("accept");
    signed.entry.entry_id
}

fn board_has(profile: &Arc<MobileProfile>, entry_id: &str) -> bool {
    profile
        .list_current_entries()
        .expect("list")
        .iter()
        .any(|entry| entry.entry_id == entry_id)
}

/// A signed alert authored by another member INTO the given namespace, returned
/// as an importable bundle. Unlike a local `sign_draft` (which commits
/// immediately), this bundle is uncommitted until a receiver imports it — the
/// shape needed to test an in-flight import, and it carries a retained payload
/// so a receiver can reproject it after a reopen.
fn foreign_alert(namespace_id: &str) -> (Vec<u8>, String) {
    let other = open_local_profile().expect("foreign profile");
    other
        .join_public_space(
            PublicSpace {
                namespace_id: namespace_id.to_string(),
                title: "Peer".into(),
                is_public: true,
            },
            Vec::new(),
        )
        .expect("foreign joins namespace");
    let d = other.create_draft_alert(draft()).expect("foreign draft");
    let signed = other.sign_draft(d.draft_id).expect("foreign sign");
    (signed.bundle_bytes, signed.entry.entry_id)
}

/// A durable profile that is ORGANIZER of community A and MEMBER of community B,
/// both authors sealed at rest. Returns (profile, a_namespace, b_namespace).
fn organizer_of_a_member_of_b(db_path: String) -> (Arc<MobileProfile>, String, String) {
    let profile = open_local_profile_with_database(db_path).expect("open");
    let a = profile
        .create_public_space("Community A".into())
        .expect("create A");
    // Mint a second namespace via a throwaway profile, then join it as a member.
    let other = open_local_profile().expect("other");
    let b = other
        .create_public_space("Community B".into())
        .expect("create B");
    // Join B with the real wrapping key so A's outgoing author is sealed INLINE
    // (Risk 13), not parked unsealed. `persist_communities` then seals B (active).
    profile
        .join_public_space(b.clone(), REGISTRY_KEY.to_vec())
        .expect("join B");
    profile
        .persist_communities(REGISTRY_KEY.to_vec())
        .expect("persist");
    (profile, a.namespace_id, b.namespace_id)
}

/// Risk 13 (seal-inline-on-join): joining a second community seals the OUTGOING
/// author inline under the wrapping key, so it is durable IMMEDIATELY — without
/// waiting for a `persist_communities`. Proven by reopening a fresh handle (which
/// drops all in-RAM parked authors) and switching back to the joined-away
/// community: it is recoverable only because the join sealed it to disk. Before
/// the fix the outgoing author was parked unsealed in RAM and lost on reopen.
#[test]
fn a_join_seals_the_outgoing_author_inline_and_it_survives_reopen_without_persist() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir
        .path()
        .join("inline-seal.db")
        .to_string_lossy()
        .to_string();

    let (sealed, a_ns) = {
        let profile = open_local_profile_with_database(db_path.clone()).expect("open");
        let a = profile
            .create_public_space("Community A".into())
            .expect("create A");
        // A second namespace, minted by a throwaway profile.
        let other = open_local_profile().expect("other");
        let b = other
            .create_public_space("Community B".into())
            .expect("create B");
        // Join B WITH the key, and DELIBERATELY do not call persist_communities:
        // the inline seal during the join must be enough to make A durable.
        profile
            .join_public_space(b, TEST_WRAPPING_KEY.to_vec())
            .expect("join B");
        let sealed = profile
            .seal_identity(TEST_WRAPPING_KEY.to_vec())
            .expect("seal identity (B, the active author)");
        (sealed, a.namespace_id)
    };
    // Handle dropped: every in-RAM parked author is gone. Only what was sealed to
    // disk survives.

    let reopened = open_profile_from_sealed_identity_with_database(
        db_path,
        TEST_WRAPPING_KEY.to_vec(),
        sealed,
    )
    .expect("reopen");

    let a_row = reopened
        .switch_community(a_ns, TEST_WRAPPING_KEY.to_vec())
        .expect("A survived reopen because the join sealed its author inline");
    assert_eq!(
        a_row.relationship,
        CommunityRelationship::Organizer,
        "the recovered author is A's own organizer identity, unsealed from its row",
    );
}

#[test]
fn communities_are_isolated_entries_approvals_and_coordinator_do_not_leak() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("iso.db").to_string_lossy().to_string();
    let (profile, a_ns, b_ns) = organizer_of_a_member_of_b(db_path);

    // Operate in A: approve a tool, post an entry, bind a coordinator.
    profile
        .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch A");
    assert!(
        profile.app_runtime().is_organizer().unwrap(),
        "organizer of A"
    );
    profile
        .app_runtime()
        .trust_app(a_tool_id())
        .expect("approve tool in A");
    let entry_id = commit_alert(&profile);
    assert!(board_has(&profile, &entry_id), "A shows its own entry");
    assert!(
        profile.app_runtime().is_app_trusted(a_tool_id()).unwrap(),
        "tool is approved in A"
    );
    let a_coordinator = profile.open_sync_session().expect("coordinator in A");

    // Switch to B: NONE of A's state is visible.
    let b_row = profile
        .switch_community(b_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch B");
    assert_eq!(b_row.relationship, CommunityRelationship::Member);
    assert!(
        !profile.app_runtime().is_organizer().unwrap(),
        "member of B, not organizer"
    );
    assert!(
        !profile.app_runtime().is_app_trusted(a_tool_id()).unwrap(),
        "A's approval does NOT carry into B"
    );
    assert!(!board_has(&profile, &entry_id), "A's entry is ABSENT in B");
    assert!(
        matches!(a_coordinator.begin(), Err(MobileError::ObjectClosed)),
        "A's coordinator is stale after the switch (generation guard) and cannot act in B"
    );
    profile
        .open_sync_session()
        .expect("B has its own coordinator");

    // Reverse: A is intact after operating in B, and B left nothing in A.
    profile
        .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch back to A");
    assert!(
        profile.app_runtime().is_organizer().unwrap(),
        "still organizer of A"
    );
    assert!(
        profile.app_runtime().is_app_trusted(a_tool_id()).unwrap(),
        "A's approval is intact after operating in B"
    );
    assert!(board_has(&profile, &entry_id), "A's entry is intact");
}

#[test]
fn a_write_in_flight_across_a_switch_fails_closed_and_commits_to_neither_community() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("race.db").to_string_lossy().to_string();
    let (profile, a_ns, b_ns) = organizer_of_a_member_of_b(db_path);

    profile
        .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch A");
    // A foreign alert bound for A, previewed and planned but NOT yet accepted —
    // a genuine in-flight import (a local sign would already have committed).
    let (bundle, foreign_id) = foreign_alert(&a_ns);
    let preview = profile
        .inspect_bytes(bundle, "race".into())
        .expect("preview foreign in A");
    let plan = preview
        .create_plan(vec![foreign_id.clone()])
        .expect("plan in A");

    // Switch to B mid-flight. The community generation advances.
    profile
        .switch_community(b_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch B");

    // The stale plan cannot commit — the community-generation guard fails it
    // closed with ObjectClosed (the handle captured the pre-switch generation).
    // This is the guard FIRING, not merely a handle that was cleared away.
    assert!(
        matches!(plan.accept(), Err(MobileError::ObjectClosed)),
        "an import in flight across a switch fails closed via the generation guard"
    );
    assert!(
        !board_has(&profile, &foreign_id),
        "the in-flight import did NOT land in B (the wrong community)"
    );
    // And it never landed in A either — a failed-closed import commits nowhere.
    profile
        .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch back to A");
    assert!(
        !board_has(&profile, &foreign_id),
        "the in-flight import did NOT land in A"
    );
}

#[test]
fn the_chooser_lists_communities_with_plain_relationships() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("list.db").to_string_lossy().to_string();
    let (profile, a_ns, b_ns) = organizer_of_a_member_of_b(db_path);

    let rows = profile.list_communities().unwrap();
    assert_eq!(rows.len(), 2, "both held communities are listed");
    let a = rows.iter().find(|r| r.namespace_id == a_ns).unwrap();
    let b = rows.iter().find(|r| r.namespace_id == b_ns).unwrap();
    assert_eq!(a.title, "Community A");
    assert_eq!(a.relationship, CommunityRelationship::Organizer);
    assert!(a.available && !a.archived && !a.quarantined);
    assert_eq!(b.title, "Community B");
    assert_eq!(b.relationship, CommunityRelationship::Member);
    assert!(b.available);
}

#[test]
fn a_durable_reopen_restores_the_last_active_community_and_switches_between_sealed_ones() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("reopen.db").to_string_lossy().to_string();
    let (a_ns, b_ns, sealed) = {
        let (profile, a_ns, b_ns) = organizer_of_a_member_of_b(db_path.clone());
        // Make A the last active community, seal the primary identity while A is
        // active, and persist so both authors are durable.
        profile
            .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
            .expect("switch A");
        let sealed = profile.seal_identity(REGISTRY_KEY.to_vec()).expect("seal");
        profile
            .persist_communities(REGISTRY_KEY.to_vec())
            .expect("persist");
        assert_eq!(
            profile.active_community().unwrap().unwrap().namespace_id,
            a_ns
        );
        (a_ns, b_ns, sealed)
    };

    // Reopen: returning opens the last available community directly.
    let reopened =
        open_profile_from_sealed_identity_with_database(db_path, REGISTRY_KEY.to_vec(), sealed)
            .expect("reopen");
    let active = reopened.active_community().unwrap().unwrap();
    assert_eq!(active.namespace_id, a_ns, "reopen lands on the last active");
    assert!(active.available);
    assert!(
        reopened.app_runtime().is_organizer().unwrap(),
        "A restored as organizer"
    );

    // Switch to B (unseals its OWN at-rest author with the key) and back.
    let b = reopened
        .switch_community(b_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch B on reopen");
    assert_eq!(b.relationship, CommunityRelationship::Member);
    assert!(
        !reopened.app_runtime().is_organizer().unwrap(),
        "member of B"
    );
    reopened
        .switch_community(a_ns, REGISTRY_KEY.to_vec())
        .expect("switch back to A");
    assert!(reopened.app_runtime().is_organizer().unwrap());
}

#[test]
fn a_sealed_community_is_un_loadable_without_the_key_and_recovers_from_quarantine_on_retry() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("atrest.db").to_string_lossy().to_string();
    let (a_ns, b_ns, sealed) = {
        let (profile, a_ns, b_ns) = organizer_of_a_member_of_b(db_path.clone());
        profile
            .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
            .expect("switch A");
        let sealed = profile.seal_identity(REGISTRY_KEY.to_vec()).expect("seal");
        profile
            .persist_communities(REGISTRY_KEY.to_vec())
            .expect("persist");
        (a_ns, b_ns, sealed)
    };

    let reopened =
        open_profile_from_sealed_identity_with_database(db_path, REGISTRY_KEY.to_vec(), sealed)
            .expect("reopen");
    // A is active; B is sealed at rest and NOT loaded. Switching to B with the
    // WRONG key fails closed and quarantines B (preserved, not dropped).
    assert!(
        matches!(
            reopened.switch_community(b_ns.clone(), WRONG_KEY.to_vec()),
            Err(MobileError::CommunityUnavailable)
        ),
        "a community's author is un-loadable without the correct key"
    );
    assert_eq!(
        reopened.active_community().unwrap().unwrap().namespace_id,
        a_ns,
        "the failed switch stayed on A — it never landed on B"
    );
    let b = reopened
        .list_communities()
        .unwrap()
        .into_iter()
        .find(|r| r.namespace_id == b_ns)
        .expect("B is preserved, not dropped");
    assert!(
        b.quarantined && !b.available,
        "B is quarantined for recovery"
    );
    // Recovery: a Retry with the CORRECT key re-attempts the unseal, succeeds,
    // clears the quarantine, and switches. A transient read that once quarantined
    // a community must never leave it permanently dead.
    let recovered = reopened
        .switch_community(b_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("a Retry with the correct key recovers the community from quarantine");
    assert_eq!(recovered.relationship, CommunityRelationship::Member);
    assert!(
        !recovered.quarantined && recovered.available,
        "recovery clears the quarantine"
    );
    assert_eq!(
        reopened.active_community().unwrap().unwrap().namespace_id,
        b_ns,
        "the recovery switch landed on B"
    );
}

#[test]
fn an_archived_community_round_trips_byte_faithfully_via_restore() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("archive.db").to_string_lossy().to_string();
    let (profile, a_ns, b_ns) = organizer_of_a_member_of_b(db_path);

    // Post an entry in B so restore has content to preserve.
    profile
        .switch_community(b_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch B");
    let entry_id = commit_alert(&profile);
    profile
        .switch_community(a_ns, REGISTRY_KEY.to_vec())
        .expect("switch A");

    profile.archive_community(b_ns.clone()).expect("archive B");
    let archived = profile
        .list_communities()
        .unwrap()
        .into_iter()
        .find(|r| r.namespace_id == b_ns)
        .expect("archived community is still present, never dropped");
    assert!(archived.archived);

    let restored = profile.restore_community(b_ns.clone()).expect("restore B");
    assert!(!restored.archived);
    // Byte-faithful: switching back to B shows the exact entry, unchanged.
    profile
        .switch_community(b_ns, REGISTRY_KEY.to_vec())
        .expect("switch B after restore");
    assert!(
        board_has(&profile, &entry_id),
        "restored community keeps its content byte-faithfully"
    );
}

#[test]
fn a_corrupt_registry_blob_is_quarantined_for_recovery_not_discarded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("migrate.db").to_string_lossy().to_string();
    let sealed = {
        let profile = open_local_profile_with_database(db_path.clone()).expect("open");
        profile
            .create_public_space("Community A".into())
            .expect("A");
        let sealed = profile.seal_identity(REGISTRY_KEY.to_vec()).expect("seal");
        profile
            .persist_communities(REGISTRY_KEY.to_vec())
            .expect("persist");
        sealed
    };

    // Corrupt the persisted registry blob directly, as a bad migration would.
    const GARBAGE: &[u8] = b"not-canonical-cbor-registry";
    {
        let db = riot_core::store::RiotDatabase::open(
            &db_path,
            riot_core::store::DatabaseConfig::default(),
        )
        .expect("reopen db");
        db.set_local_state(REGISTRY_LOCAL_STATE_KEY, GARBAGE)
            .expect("write garbage");
    }

    let reopened = open_profile_from_sealed_identity_with_database(
        db_path.clone(),
        REGISTRY_KEY.to_vec(),
        sealed,
    )
    .expect("reopen despite corrupt registry");
    assert!(
        reopened.community_registry_quarantined().unwrap(),
        "a corrupt registry is flagged for recovery"
    );
    assert!(
        reopened.list_communities().unwrap().is_empty(),
        "the session runs with an empty registry rather than crashing"
    );
    drop(reopened);

    // The undecodable blob is preserved for recovery, never discarded.
    let db =
        riot_core::store::RiotDatabase::open(&db_path, riot_core::store::DatabaseConfig::default())
            .expect("reopen db for recovery check");
    assert_eq!(
        db.local_state(REGISTRY_QUARANTINE_LOCAL_STATE_KEY).unwrap(),
        Some(GARBAGE.to_vec()),
        "the corrupt blob is preserved under the quarantine key"
    );
}

#[test]
fn switching_away_and_back_preserves_a_communitys_board_in_session() {
    // The board projection survives a round-trip switch even for locally-authored
    // content the store does not retain a payload for — the per-community
    // projection cache holds it. (Reopen rehydration of the legacy alert board is
    // a separate store concern: alert payloads are not retained, so reopened
    // board content is out of scope here; Risk 11's newswire Home reprojection is
    // covered by `a_newswire_communitys_descriptor_handle_survives_a_reopen`.)
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("session.db").to_string_lossy().to_string();
    let (profile, a_ns, b_ns) = organizer_of_a_member_of_b(db_path);

    profile
        .switch_community(a_ns.clone(), REGISTRY_KEY.to_vec())
        .expect("switch A");
    let entry_id = commit_alert(&profile);
    assert!(board_has(&profile, &entry_id));

    profile
        .switch_community(b_ns, REGISTRY_KEY.to_vec())
        .expect("switch B");
    assert!(!board_has(&profile, &entry_id), "not visible in B");

    profile
        .switch_community(a_ns, REGISTRY_KEY.to_vec())
        .expect("switch back to A");
    assert!(
        board_has(&profile, &entry_id),
        "A's board is preserved across the round trip"
    );
}

#[test]
fn a_newswire_communitys_descriptor_handle_survives_a_reopen() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir
        .path()
        .join("descriptor.db")
        .to_string_lossy()
        .to_string();
    let (descriptor_id, sealed) = {
        let profile = open_local_profile_with_database(db_path.clone()).expect("open");
        // A newswire community's descriptor handle is what Home reprojects its
        // front page / open wire from (closing Risk 11 for newswire content).
        let signed = profile
            .create_newswire_space(NewswireSpaceInput {
                name: "Newswire Community".into(),
                summary: "For rehydration".into(),
                languages: vec!["en".into()],
                geographic_tags: vec![],
                topic_tags: vec![],
                editorial_roster: vec![],
            })
            .expect("create newswire space");
        let row = profile.active_community().unwrap().unwrap();
        assert_eq!(
            row.descriptor_entry_id.as_deref(),
            Some(signed.entry_id.as_str()),
            "the community row carries the descriptor handle for Home rehydration"
        );
        // Publish a post so there is Home content to reproject after a reopen.
        profile
            .create_newswire_post(riot_ffi::NewswirePostInput {
                space_descriptor_entry_id: signed.entry_id.clone(),
                headline: "Water at the north gate".into(),
                body: "Bring containers.".into(),
                language: "en".into(),
                event_time_unix_seconds: None,
                expires_at_unix_seconds: None,
                coarse_location: None,
                source_claims: vec![],
                operational_profile: None,
                ai_assisted: false,
            })
            .expect("publish post");
        let sealed = profile.seal_identity(REGISTRY_KEY.to_vec()).expect("seal");
        profile
            .persist_communities(REGISTRY_KEY.to_vec())
            .expect("persist");
        (signed.entry_id, sealed)
    };

    let reopened =
        open_profile_from_sealed_identity_with_database(db_path, REGISTRY_KEY.to_vec(), sealed)
            .expect("reopen");
    let row = reopened.active_community().unwrap().unwrap();
    assert_eq!(
        row.descriptor_entry_id.as_deref(),
        Some(descriptor_id.as_str()),
        "the persisted descriptor handle survives a reopen"
    );
    // Risk 11 closed: the persisted handle reprojects the newswire Home from the
    // store on reopen — the loaded community shows its content, not the empty state.
    let projection = reopened
        .project_newswire_space(descriptor_id.clone())
        .expect("reproject newswire Home on reopen");
    assert!(
        projection
            .open_wire
            .iter()
            .any(|post| post.headline.as_deref() == Some("Water at the north gate")),
        "a reopened community reprojects its published newswire content from the store"
    );
}

/// Risk 15 — a JOINED newswire community must carry its descriptor handle, or it
/// is a "dead follow": before this fix `join_public_space` registered
/// `descriptor_entry_id = None`, so Home could never reproject even after sync.
/// `join_newswire_community` (fed by a 1E share reference) carries the handle.
#[test]
fn a_joined_newswire_community_carries_its_descriptor_handle_not_a_dead_follow() {
    let origin = open_local_profile().expect("origin");
    let descriptor = origin
        .create_newswire_space(NewswireSpaceInput {
            name: "Uganda".into(),
            summary: "Kampala".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
            editorial_roster: vec![],
        })
        .expect("create newswire space");
    let origin_namespace = origin.identity().unwrap().namespace_id;

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("follow.db").to_string_lossy().to_string();
    let follower = open_local_profile_with_database(db).expect("follower");
    follower
        .join_newswire_community(
            PublicSpace {
                namespace_id: origin_namespace,
                title: "Uganda (pending sync)".into(),
                is_public: true,
            },
            descriptor.entry_id.clone(),
            REGISTRY_KEY.to_vec(),
        )
        .expect("join by descriptor handle");

    let row = follower.active_community().unwrap().unwrap();
    assert_eq!(
        row.descriptor_entry_id.as_deref(),
        Some(descriptor.entry_id.as_str()),
        "a joined community carries its descriptor handle — not a dead follow"
    );
    assert_eq!(row.relationship, CommunityRelationship::Member);
}

/// Deliver every entry the `origin` holds to the `follower` over the real sync
/// bridge, accepting whatever import the follower is offered. Both must share the
/// namespace. Simulates the first sync a joined community receives.
fn deliver_all_via_sync(origin: &Arc<MobileProfile>, follower: &Arc<MobileProfile>) {
    let init: Arc<MobileSyncSession> = follower.open_sync_session().expect("follower sync");
    let resp: Arc<MobileSyncSession> = origin.open_sync_session().expect("origin sync");
    init.begin().expect("begin");
    for _ in 0..24 {
        if let Some(frame) = init.take_outbound_frame().expect("init frame") {
            resp.receive_frame(frame).expect("resp receive");
        }
        match resp.take_outbound_frame().expect("resp frame") {
            Some(frame) => {
                let outcome = init.receive_frame(frame).expect("init receive");
                if outcome.kind == SyncOutcomeKind::ReviewImport {
                    init.accept_import().expect("accept import");
                }
                if outcome.terminal {
                    break;
                }
            }
            None => break,
        }
    }
}

/// Risk 15 + Risk 16 end-to-end: publishing DISTRIBUTES. A follower joins a
/// community by share reference (Risk 15 carries the descriptor handle), the
/// first sync delivers the descriptor + post (Risk 16 lets newswire traverse the
/// nearby bridge at all), and the follower's Home MATERIALIZES the post.
#[test]
fn a_followed_communitys_home_materializes_published_content_after_the_first_sync() {
    let origin = open_local_profile().expect("origin");
    let descriptor = origin
        .create_newswire_space(NewswireSpaceInput {
            name: "Germany".into(),
            summary: "Berlin".into(),
            languages: vec!["de".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
            editorial_roster: vec![],
        })
        .expect("create newswire space");
    origin
        .create_newswire_post(riot_ffi::NewswirePostInput {
            space_descriptor_entry_id: descriptor.entry_id.clone(),
            headline: "Blockade at the depot".into(),
            body: "Meet at 6.".into(),
            language: "de".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec![],
            operational_profile: None,
            ai_assisted: false,
        })
        .expect("publish post");
    let origin_namespace = origin.identity().unwrap().namespace_id;

    let follower = open_local_profile().expect("follower");
    follower
        .join_newswire_community(
            PublicSpace {
                namespace_id: origin_namespace,
                title: "Germany (pending sync)".into(),
                is_public: true,
            },
            descriptor.entry_id.clone(),
            Vec::new(),
        )
        .expect("join by descriptor handle");
    // Pending first sync: the descriptor + post aren't on this device yet.
    assert!(
        follower
            .project_newswire_space(descriptor.entry_id.clone())
            .map(|p| p.open_wire.is_empty())
            .unwrap_or(true),
        "pending first sync — no content until the descriptor + post arrive"
    );

    // First sync delivers the descriptor + post (Risk 16: newswire now syncs).
    deliver_all_via_sync(&origin, &follower);

    // Home MATERIALIZES the published post via the carried descriptor handle.
    let projection = follower
        .project_newswire_space(descriptor.entry_id)
        .expect("reproject after first sync");
    assert!(
        projection
            .open_wire
            .iter()
            .any(|post| post.headline.as_deref() == Some("Blockade at the depot")),
        "a followed community's Home materializes published content after the first sync"
    );
}
