use riot_core::apps::directory::{
    assemble_directory, AppProvenance, DirectoryInputs, EndorsementRecord, IndexedApp, SpaceTrust,
};
use riot_core::apps::manifest::AppManifest;
use riot_core::apps::trust::{TrustMarker, TrustMarkerKind};
use riot_core::willow::identity::{AuthorIdentity, NamespaceKind};

fn identity(seed: u8) -> AuthorIdentity {
    AuthorIdentity {
        namespace_id: [seed; 32],
        subspace_id: [seed; 32],
        namespace_kind: NamespaceKind::Communal,
        signing_key_id: [seed; 32],
    }
}

fn manifest(name: &str, author_seed: u8, version: &str) -> AppManifest {
    AppManifest {
        name: name.to_string(),
        description: "Does a thing for your group.".to_string(),
        version: version.to_string(),
        author: identity(author_seed),
        permissions: vec!["own-app-data".to_string()],
        entry_point: "index.html".to_string(),
    }
}

fn indexed(app_id: [u8; 32], m: AppManifest, carrier: [u8; 32], ts: u64) -> IndexedApp {
    IndexedApp {
        app_id,
        manifest: m,
        bundle_present: true,
        provenance: AppProvenance::Carried {
            carrier_subspace_id: carrier,
        },
        manifest_timestamp_micros: ts,
    }
}

fn empty_inputs() -> DirectoryInputs {
    DirectoryInputs {
        apps: vec![],
        endorsements: vec![],
        spaces: vec![],
        met_subspace_ids: vec![],
    }
}

#[test]
fn same_app_id_from_two_carriers_lists_once() {
    let m = manifest("Checklist", 1, "1.0.0");
    let mut inputs = empty_inputs();
    inputs.apps = vec![
        indexed([7u8; 32], m.clone(), [2u8; 32], 10),
        indexed([7u8; 32], m, [3u8; 32], 20),
    ];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings.len(), 1);
    assert_eq!(listings[0].app_id, [7u8; 32]);
}

#[test]
fn built_in_provenance_wins_over_carried_for_same_app_id() {
    let m = manifest("Checklist", 1, "1.0.0");
    let mut built_in = indexed([7u8; 32], m.clone(), [0u8; 32], 0);
    built_in.provenance = AppProvenance::BuiltIn;
    let mut inputs = empty_inputs();
    inputs.apps = vec![indexed([7u8; 32], m, [3u8; 32], 20), built_in];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings.len(), 1);
    assert_eq!(listings[0].provenance, AppProvenance::BuiltIn);
}

#[test]
fn same_name_different_author_never_merges() {
    let mut inputs = empty_inputs();
    inputs.apps = vec![
        indexed(
            [1u8; 32],
            manifest("Shift Signup", 1, "1.0.0"),
            [9u8; 32],
            10,
        ),
        indexed(
            [2u8; 32],
            manifest("Shift Signup", 2, "1.0.0"),
            [9u8; 32],
            10,
        ),
    ];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings.len(), 2);
    assert!(listings.iter().all(|l| l.superseded_by.is_none()));
}

#[test]
fn newer_manifest_from_same_author_and_name_supersedes_older() {
    let mut inputs = empty_inputs();
    inputs.apps = vec![
        indexed([1u8; 32], manifest("Checklist", 1, "1.0.0"), [9u8; 32], 10),
        indexed([2u8; 32], manifest("Checklist", 1, "1.1.0"), [9u8; 32], 20),
    ];
    let listings = assemble_directory(&inputs);
    let old = listings
        .iter()
        .find(|l| l.app_id == [1u8; 32])
        .expect("old");
    let new = listings
        .iter()
        .find(|l| l.app_id == [2u8; 32])
        .expect("new");
    assert_eq!(old.superseded_by, Some([2u8; 32]));
    assert_eq!(new.superseded_by, None);
}

#[test]
fn endorsements_dedup_by_subspace_skip_retracted_and_split_met_unmet() {
    let mut inputs = empty_inputs();
    inputs.apps = vec![indexed(
        [7u8; 32],
        manifest("Checklist", 1, "1.0.0"),
        [9u8; 32],
        10,
    )];
    inputs.endorsements = vec![
        EndorsementRecord {
            app_id: [7u8; 32],
            endorser_subspace_id: [4u8; 32],
            retracted: false,
        },
        // Same endorser twice: counts once.
        EndorsementRecord {
            app_id: [7u8; 32],
            endorser_subspace_id: [4u8; 32],
            retracted: false,
        },
        // Retracted: does not count.
        EndorsementRecord {
            app_id: [7u8; 32],
            endorser_subspace_id: [5u8; 32],
            retracted: true,
        },
        // Unmet endorser.
        EndorsementRecord {
            app_id: [7u8; 32],
            endorser_subspace_id: [6u8; 32],
            retracted: false,
        },
    ];
    inputs.met_subspace_ids = vec![[4u8; 32]];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings[0].endorsements.met_subspace_ids, vec![[4u8; 32]]);
    assert_eq!(listings[0].endorsements.unmet_count, 1);
}

#[test]
fn trusted_in_reflects_per_space_trust_evaluation() {
    let organizer = [8u8; 32];
    let mut inputs = empty_inputs();
    inputs.apps = vec![indexed(
        [7u8; 32],
        manifest("Checklist", 1, "1.0.0"),
        [9u8; 32],
        10,
    )];
    inputs.spaces = vec![
        SpaceTrust {
            space_namespace_id: [10u8; 32],
            markers: vec![TrustMarker {
                app_id: [7u8; 32],
                author_subspace_id: organizer,
                kind: TrustMarkerKind::Trust,
                timestamp_micros: 5,
            }],
            organizer_subspace_ids: vec![organizer],
        },
        SpaceTrust {
            space_namespace_id: [11u8; 32],
            markers: vec![],
            organizer_subspace_ids: vec![organizer],
        },
    ];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings[0].trusted_in_spaces, vec![[10u8; 32]]);
}

#[test]
fn listings_sort_by_met_endorsements_then_name() {
    let mut inputs = empty_inputs();
    inputs.apps = vec![
        indexed(
            [1u8; 32],
            manifest("Zebra Notes", 1, "1.0.0"),
            [9u8; 32],
            10,
        ),
        indexed(
            [2u8; 32],
            manifest("Alpha Notes", 2, "1.0.0"),
            [9u8; 32],
            10,
        ),
        indexed([3u8; 32], manifest("Ride Board", 3, "1.0.0"), [9u8; 32], 10),
    ];
    inputs.endorsements = vec![EndorsementRecord {
        app_id: [3u8; 32],
        endorser_subspace_id: [4u8; 32],
        retracted: false,
    }];
    inputs.met_subspace_ids = vec![[4u8; 32]];
    let listings = assemble_directory(&inputs);
    assert_eq!(listings[0].app_id, [3u8; 32]); // endorsed first
    assert_eq!(listings[1].name, "Alpha Notes"); // then name order
    assert_eq!(listings[2].name, "Zebra Notes");
}
