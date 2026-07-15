//! Two guards over the seeded demo space.
//!
//! 1. The committed bundle bytes equal a fresh deterministic rebuild. Editing
//!    `content.json` without repacking fails here — the same guard the checklist
//!    starter fixture has.
//! 2. The committed bytes actually import, through the ORDINARY
//!    `inspect → plan_all → commit` pipeline, and yield the space the demo
//!    script describes. This is the real proof: the seed is not special-cased
//!    anywhere, so if a peer's bundle would be admitted, so is this one — and if
//!    the demo would be empty on stage, this test is red first.

use std::collections::{BTreeMap, BTreeSet};

use riot_core::apps::directory::{assemble_directory, DirectoryInputs};
use riot_core::apps::index::scan_app_index;
use riot_core::apps::starter::{verify_starter_catalog, STARTER_CATALOG};
use riot_core::apps::trust::{is_trusted, TrustMarkerKind};
use riot_core::demo_fixture::{build_demo_bundle_from_source, demo_bundle_path};
use riot_core::newswire::{
    classify_newswire_path, decode_news_post, decode_space_descriptor, NewswirePathKind,
};
use riot_core::profile::resolver::{render_display_name, resolve_display_names};
use riot_core::session::{ImportContext, RiotSession};
use riot_core::willow::{Path, ALERT_COMPONENT, OBJECTS_COMPONENT};
use willow25::groupings::Keylike;

/// The committed organizer-shaped namespace id: the founding collective's own
/// subspace public key, reused as the space's namespace id. Ground for
/// communality in `content.json`; pinned here so a silent reseed is a loud
/// failure rather than a demo that quietly loses its organizer.
const ORGANIZER_COORDINATE_HEX: &str =
    "1b050d1133d08c98a905977b78207d7739fff94a11939355380cf78bca88e756";

#[test]
fn committed_demo_bundle_equals_a_fresh_deterministic_rebuild() {
    let committed = std::fs::read(demo_bundle_path()).expect("committed demo bundle");
    let rebuilt = build_demo_bundle_from_source().expect("rebuild from committed content.json");
    assert_eq!(
        committed, rebuilt,
        "fixtures/demo/riverside is stale — re-run: \
         cargo run -p riot-core --features conformance --example pack_demo_space"
    );
}

#[test]
fn the_demo_bundle_imports_cleanly_and_yields_the_expected_shape() {
    let committed = std::fs::read(demo_bundle_path()).expect("committed demo bundle");

    // No privileged seed path: the demo bundle goes through exactly what a
    // peer's bundle goes through.
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    let preview = store
        .inspect(&committed, ImportContext::new("demo-space"))
        .expect("inspect")
        .expect_preview();
    let eligible = preview.eligible_count().expect("eligible count");
    let plan = preview.plan_all().expect("plan");
    plan.commit().expect("commit");
    assert_eq!(
        store.live_count().expect("live count"),
        eligible,
        "every eligible entry in the demo bundle must go live"
    );

    // --- Beat 1: six alerts, and four people with names ---------------------
    let alerts = store
        .entries_with_prefix(&Path::from_slices(&[OBJECTS_COMPONENT, ALERT_COMPONENT]).unwrap())
        .expect("alert entries");
    assert_eq!(alerts.len(), 6, "the demo script names six alerts");

    let names = resolve_display_names(&store).expect("resolve names");
    let by_name: BTreeMap<&str, [u8; 32]> = names
        .iter()
        .map(|(subspace, name)| (name.as_str(), *subspace))
        .collect();
    for member in ["Ana", "Marcus", "Priya", "Dee"] {
        assert!(
            by_name.contains_key(member),
            "no profile card resolves to '{member}' — the board would render member-<hex>"
        );
    }

    // --- Beat 2: Shift Signup, endorsed by two named groups ------------------
    let scanned = scan_app_index(&store).expect("scan index");
    // Everyone who endorsed anything here is someone this phone has "met" — the
    // demo runs on a phone that already holds the space, and the directory only
    // NAMES endorsers it has met.
    let met: Vec<[u8; 32]> = scanned
        .endorsements
        .iter()
        .map(|record| record.endorser_subspace_id)
        .collect();
    let listings = assemble_directory(&DirectoryInputs {
        apps: scanned.apps.clone(),
        endorsements: scanned.endorsements.clone(),
        spaces: scanned.spaces.clone(),
        met_subspace_ids: met,
    });
    let shift_signup = listings
        .iter()
        .find(|listing| listing.name == "Shift Signup")
        .expect("Shift Signup must appear in the directory");
    assert!(
        shift_signup.bundle_present,
        "Shift Signup's bundle must be present or it can never be opened"
    );
    let endorsers = &shift_signup.endorsements.met_subspace_ids;
    assert_eq!(
        (endorsers.len(), shift_signup.endorsements.unmet_count),
        (2, 0),
        "the demo script reads out exactly two endorsing groups"
    );
    let endorsing_groups: Vec<String> = endorsers
        .iter()
        .map(|subspace| render_display_name(names.get(subspace).map(String::as_str), subspace))
        .collect();
    for group in ["Eastside Tenant Council", "Courtyard Mutual Aid"] {
        assert!(
            endorsing_groups
                .iter()
                .any(|rendered| rendered.starts_with(&format!("{group} · "))),
            "the endorsement line must name '{group}', got {endorsing_groups:?}"
        );
    }

    // --- Beat 4: the half-done checklist, attributed by id -------------------
    let checklist_app_id = verify_starter_catalog(STARTER_CATALOG)
        .first()
        .expect("starter catalog")
        .app_id;
    let items = store
        .entries_with_prefix(&Path::from_slices(&[b"apps", &checklist_app_id]).unwrap())
        .expect("checklist items");
    assert_eq!(items.len(), 3, "the seeded checklist is three items");

    let checked: Vec<serde_json::Value> = items
        .iter()
        .map(|(_, _, payload)| {
            let payload = payload.as_ref().expect("app-data payloads are retained");
            serde_json::from_slice::<serde_json::Value>(payload).expect("item value is JSON")
        })
        .filter(|value| value["done"].as_bool().unwrap_or(false))
        .collect();
    assert_eq!(checked.len(), 1, "exactly one item starts checked");

    // The item stores an ID, not a name — a stored name is a snapshot no rename
    // could ever repair. It must resolve, through the profile resolver, to Ana.
    let id_hex = checked[0]["updated_by_id"]
        .as_str()
        .expect("a checked item carries updated_by_id, never a name");
    assert!(
        checked[0].get("updated_by").is_none(),
        "the fixture must not store a legacy name snapshot"
    );
    let subspace = subspace_from_hex(id_hex);
    assert_eq!(
        render_display_name(names.get(&subspace).map(String::as_str), &subspace),
        render_display_name(Some("Ana"), &by_name["Ana"]),
        "the checked item must resolve to Ana's rendered name"
    );
    assert!(
        render_display_name(names.get(&subspace).map(String::as_str), &subspace)
            .starts_with("Ana · a3f9"),
        "the demo script reads Ana's tag out loud as a3f9"
    );

    // --- Beat 5: the organizer coordinate and its nine Trust markers --------
    // The space is organizer-shaped: its namespace id IS the founding
    // collective's own subspace key, so the recognized-organizer coordinate is
    // derivable from the space itself. The organizer has signed a Trust marker
    // for every tool the demo shows. Without these markers a member could
    // inspect the tools but never open them — the bug this unit fixes.
    assert_eq!(
        scanned.spaces.len(),
        1,
        "the demo is one communal space with one organizer"
    );
    let space_trust = &scanned.spaces[0];
    let organizer_coordinate = space_trust.space_namespace_id;
    let organizer_hex: String = organizer_coordinate
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        organizer_hex, ORGANIZER_COORDINATE_HEX,
        "the recognized-organizer coordinate is the committed organizer-shaped namespace id"
    );

    assert_eq!(
        space_trust.markers.len(),
        9,
        "eight starter apps plus Shift Signup are organizer-trusted"
    );
    for marker in &space_trust.markers {
        assert_eq!(
            marker.author_subspace_id, organizer_coordinate,
            "every Trust marker sits at the recognized-organizer coordinate (subspace == namespace)"
        );
        assert_eq!(
            marker.kind,
            TrustMarkerKind::Trust,
            "the demo approves its tools, it never revokes"
        );
    }

    // The organizer's markers grant authority; a coordinate that is not the
    // recognized organizer grants nothing — the admission verifies, it never
    // shortcuts.
    let starter_ids: BTreeSet<[u8; 32]> = verify_starter_catalog(STARTER_CATALOG)
        .iter()
        .map(|indexed| indexed.app_id)
        .collect();
    let marker_ids: BTreeSet<[u8; 32]> = space_trust.markers.iter().map(|m| m.app_id).collect();
    assert!(
        starter_ids.is_subset(&marker_ids),
        "every starter app the Tools surface shows is organizer-trusted"
    );
    assert_eq!(
        marker_ids.difference(&starter_ids).count(),
        1,
        "the ninth trusted app is the demo's own Shift Signup"
    );
    for app_id in &marker_ids {
        assert!(
            is_trusted(app_id, &space_trust.markers, &[organizer_coordinate]),
            "the organizer's marker makes the tool openable for every member"
        );
        assert!(
            !is_trusted(app_id, &space_trust.markers, &[[0x77; 32]]),
            "authority is honored only from the recognized organizer, never anyone else"
        );
    }

    // --- Beat 6: the open newswire — one signed descriptor, two posts -------
    // Home reads the newswire. Only an organizer-shaped author may sign the
    // SpaceDescriptor; it pins the descriptor path family and this namespace,
    // and names the founding editorial roster. Members write ordinary open-wire
    // posts beneath it. This is additive to the six alerts above.
    let newswire_entries = store
        .entries_with_prefix(&Path::from_slices(&[b"newswire", b"v1"]).unwrap())
        .expect("newswire entries");
    let mut descriptor_paths = 0usize;
    let mut post_paths = 0usize;
    for (_, entry, payload) in &newswire_entries {
        let payload = payload.as_ref().expect("newswire payloads are retained");
        match classify_newswire_path(entry.path()) {
            Some((NewswirePathKind::Descriptor, _, _)) => {
                descriptor_paths += 1;
                let descriptor =
                    decode_space_descriptor(payload).expect("the descriptor payload decodes");
                assert_eq!(
                    descriptor.namespace_id, organizer_coordinate,
                    "the descriptor names this space's namespace"
                );
                assert_eq!(
                    *entry.subspace_id().as_bytes(),
                    organizer_coordinate,
                    "only the organizer signs the SpaceDescriptor"
                );
                assert_eq!(
                    descriptor.editorial_roster.len(),
                    2,
                    "a real founding editorial roster of two editors"
                );
            }
            Some((NewswirePathKind::Post { .. }, _, _)) => {
                post_paths += 1;
                decode_news_post(payload).expect("the post payload decodes");
            }
            other => panic!("unexpected newswire path shape: {other:?}"),
        }
    }
    assert_eq!(descriptor_paths, 1, "exactly one signed SpaceDescriptor");
    assert_eq!(post_paths, 2, "two open-wire posts under it");
}

fn subspace_from_hex(hex: &str) -> [u8; 32] {
    assert_eq!(hex.len(), 64, "an id is 32 bytes of lowercase hex");
    let mut out = [0u8; 32];
    for (slot, pair) in out.iter_mut().zip(hex.as_bytes().chunks(2)) {
        let nibble = |b: u8| match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            _ => panic!("invalid hex digit"),
        };
        *slot = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    out
}
