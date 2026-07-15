//! What reaches the app-index scanner, and what never does.
//!
//! The scanner re-validates every record it reads. It turns out it almost never
//! has to: the import admission gate (`import/bundle.rs`) already refuses a
//! malformed app-index record outright, so the store cannot hold one. This file
//! pins that down from both directions — it proves the gate refuses the hostile
//! records (so the store stays clean), and it exercises the scanner's real
//! ordering and pairing behaviour on records that *are* admissible.
//!
//! The distinction matters for a reader of the scanner: its per-record
//! `continue` arms are a second, independent line of defence, not the first one.
//! They are unreachable while the gate holds, and they are what keeps the
//! scanner safe if it ever stops holding.

use minicbor::Encoder;
use riot_core::apps::bundle::{encode_app_bundle, AppBundle, AppResource};
use riot_core::apps::endorse::{encode_endorsement, EndorsementMarker};
use riot_core::apps::index::{
    app_bundle_digest, app_index_bundle_path, app_index_endorsement_path, app_index_manifest_path,
    app_index_trust_path, app_pair_bytes, publish_app_index, scan_app_index, APP_INDEX_COMPONENT,
};
use riot_core::apps::manifest::{app_id_for, encode_manifest, AppManifest};
use riot_core::apps::trust::{encode_trust_marker, TrustMarker, TrustMarkerKind};
use riot_core::import::{encode_bundle, BUNDLE_CODEC_ID, BUNDLE_MAGIC};
use riot_core::session::{
    CommitOutcome, EvidenceStore, ImportContext, InspectOutcome, RiotSession,
};
use riot_core::willow::{
    authorise_entry, encode_capability, encode_entry, generate_communal_author, Entry,
    EvidenceAuthor, Path, SignedWillowEntry,
};

/// A distinct valid app pair. `tag` varies the bundle bytes, so each call yields
/// a different content-derived app id.
fn sample_pair(author: &EvidenceAuthor, tag: &str) -> (Vec<u8>, Vec<u8>, [u8; 32]) {
    let manifest = AppManifest {
        name: format!("Checklist {tag}"),
        description: "Shared tasks".into(),
        version: "1.0.0".into(),
        author: author.identity(),
        permissions: vec!["app-data".into()],
        entry_point: "index.html".into(),
    };
    let bundle = AppBundle {
        entry_point: "index.html".into(),
        resources: vec![AppResource {
            path: "index.html".into(),
            content_type: "text/html".into(),
            bytes: format!("<html>{tag}</html>").into_bytes(),
        }],
    };
    let manifest_bytes = encode_manifest(&manifest).expect("manifest");
    let bundle_bytes = encode_app_bundle(&bundle).expect("bundle");
    let app_id = app_id_for(&manifest, &app_bundle_digest(&bundle_bytes)).expect("app id");
    (manifest_bytes, bundle_bytes, app_id)
}

fn signed_at(
    author: &EvidenceAuthor,
    path: Path,
    payload: &[u8],
    timestamp: u64,
) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(author.namespace_id().clone())
        .subspace_id(author.subspace_id())
        .path(path)
        .timestamp(timestamp)
        .payload(payload)
        .build();
    let authorised = authorise_entry(author, entry).expect("authorise");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

/// Assembles a bundle *without* the validation `encode_bundle` performs — the
/// bytes a hostile peer would put on the wire, which our own encoder refuses to
/// produce.
fn raw_bundle(entries: &[SignedWillowEntry]) -> Vec<u8> {
    let mut bytes = BUNDLE_MAGIC.to_vec();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(2).expect("map");
    encoder
        .u8(0)
        .expect("key")
        .str(BUNDLE_CODEC_ID)
        .expect("codec");
    encoder
        .u8(1)
        .expect("key")
        .array(entries.len() as u64)
        .expect("items");
    for signed in entries {
        encoder.map(4).expect("item");
        encoder
            .u8(0)
            .expect("key")
            .bytes(&signed.entry_bytes)
            .expect("entry");
        encoder
            .u8(1)
            .expect("key")
            .bytes(&signed.capability_bytes)
            .expect("capability");
        encoder
            .u8(2)
            .expect("key")
            .bytes(&signed.signature)
            .expect("signature");
        encoder
            .u8(3)
            .expect("key")
            .bytes(&signed.payload_bytes)
            .expect("payload");
    }
    bytes
}

fn import_signed(store: &EvidenceStore, entries: &[SignedWillowEntry]) {
    let bytes = encode_bundle(entries).expect("bundle");
    let preview = store
        .inspect(&bytes, ImportContext::new("app-index-hostile-test"))
        .expect("inspect")
        .expect_preview();
    match preview.plan_all().expect("plan").commit().expect("commit") {
        CommitOutcome::Committed(_) => {}
        CommitOutcome::NoChanges(_) => panic!("expected new entries"),
    }
}

fn store() -> EvidenceStore {
    let session = RiotSession::open().expect("session");
    session.create_store().expect("store")
}

/// Every forged app-index record is refused at admission — twice over. Our own
/// encoder will not even build a bundle containing one, and a bundle assembled
/// behind the encoder's back (as a hostile peer would) is refused at `inspect`,
/// so nothing enters the store.
///
/// The four lies are distinct and all matter:
///   * an endorsement filed under **someone else's subspace** (forging another
///     person's approval),
///   * a payload that is **not an endorsement/trust marker at all**,
///   * a marker that approves a **different app** than the slot it sits in
///     (laundering one signed approval onto another app),
///   * **junk in a bundle slot** (an app "launchable" from bytes that are not a
///     bundle).
///
/// Because none of these can be stored, the scanner's matching per-record
/// rejections are dead defence-in-depth, not a live code path.
#[test]
fn every_forged_app_index_record_is_refused_at_admission() {
    let author = generate_communal_author().expect("author");
    let store = store();
    let (manifest_bytes, bundle_bytes, app_id) = sample_pair(&author, "honest");
    publish_app_index(&store, &author, &manifest_bytes, &bundle_bytes, 1_000).expect("publish");
    let live_before = store.live_count().expect("live count");

    let mine = *author.subspace_id().as_bytes();
    let stranger = [0x5a_u8; 32];
    let other_app_id = [0x77_u8; 32];

    let honest_endorsement = encode_endorsement(&EndorsementMarker {
        app_id,
        note: "I use this every week".into(),
        retracted: false,
    })
    .expect("encode endorsement");
    let laundered_endorsement = encode_endorsement(&EndorsementMarker {
        app_id: other_app_id,
        note: "approval for one app, filed against another".into(),
        retracted: false,
    })
    .expect("encode endorsement");
    let laundered_trust = encode_trust_marker(&TrustMarker {
        app_id: other_app_id,
        author_subspace_id: mine,
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 2_000,
    })
    .expect("encode trust marker");

    let forgeries = [
        (
            "an endorsement filed under a stranger's subspace",
            signed_at(
                &author,
                app_index_endorsement_path(&app_id, &stranger).expect("path"),
                &honest_endorsement,
                2_001,
            ),
        ),
        (
            "an endorsement slot holding something that is not an endorsement",
            signed_at(
                &author,
                app_index_endorsement_path(&app_id, &mine).expect("path"),
                b"not an endorsement",
                2_002,
            ),
        ),
        (
            "an endorsement naming a different app than its slot",
            signed_at(
                &author,
                app_index_endorsement_path(&app_id, &mine).expect("path"),
                &laundered_endorsement,
                2_003,
            ),
        ),
        (
            "a trust marker naming a different app than its slot",
            signed_at(
                &author,
                app_index_trust_path(&app_id, &mine).expect("path"),
                &laundered_trust,
                2_004,
            ),
        ),
        (
            "a trust slot holding something that is not a trust marker",
            signed_at(
                &author,
                app_index_trust_path(&app_id, &mine).expect("path"),
                b"not a trust marker",
                2_005,
            ),
        ),
        (
            "junk in a bundle slot",
            signed_at(
                &author,
                app_index_bundle_path(&app_id).expect("path"),
                b"this is not an app bundle",
                2_006,
            ),
        ),
        (
            "an app id that is not 32 bytes",
            signed_at(
                &author,
                Path::from_slices(&[APP_INDEX_COMPONENT, b"short", b"manifest"]).expect("path"),
                &manifest_bytes,
                2_007,
            ),
        ),
        (
            "a slot name that does not exist",
            signed_at(
                &author,
                Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"rumour"]).expect("path"),
                &manifest_bytes,
                2_008,
            ),
        ),
        (
            "a trailing component past a recognized slot",
            signed_at(
                &author,
                Path::from_slices(&[APP_INDEX_COMPONENT, &app_id, b"manifest", b"extra"])
                    .expect("path"),
                &manifest_bytes,
                2_009,
            ),
        ),
    ];

    for (label, forgery) in &forgeries {
        // A bundle assembled behind our own encoder's back — what a hostile peer
        // actually puts on the wire — is refused at the door: nothing is
        // eligible, so there is nothing a plan could commit.
        //
        // The two halves of the gate live in different places, and this asserts
        // the outcome of both together: *schema* (the payload must decode as the
        // thing its slot is for, and must name the app its slot is under) is
        // bound in the bundle codec, while *slot ownership* (the entry's
        // subspace must equal the identity component in its own path) is bound
        // at `inspect`. An endorsement forged under a stranger's subspace is
        // schema-valid, so only the second half stops it.
        let hostile = raw_bundle(std::slice::from_ref(forgery));
        match store
            .inspect(&hostile, ImportContext::new("hostile-peer"))
            .expect("inspect must not fail, only refuse")
        {
            InspectOutcome::Rejected(_) => {}
            InspectOutcome::Preview(preview) => assert_eq!(
                preview.eligible_count().expect("eligible count"),
                0,
                "{label}: a forged app-index record was admitted"
            ),
        }
    }

    // The store is exactly as it was, and the honest app still resolves.
    assert_eq!(store.live_count().expect("live count"), live_before);
    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.apps.len(), 1);
    assert_eq!(scanned.apps[0].app_id, app_id);
    assert!(scanned.apps[0].bundle_present);
    assert!(scanned.endorsements.is_empty());
    assert!(scanned.spaces.is_empty());
    assert!(scanned.pending_manifests.is_empty());
}

/// Several honest trust markers from one organizer come back in a stable,
/// coordinate-derived order — not in whatever order the store happened to
/// iterate them.
#[test]
fn several_trust_markers_are_ordered_by_their_coordinates() {
    let author = generate_communal_author().expect("author");
    let store = store();
    let mine = *author.subspace_id().as_bytes();

    let (manifest_a, bundle_a, app_a) = sample_pair(&author, "alpha");
    let (manifest_b, bundle_b, app_b) = sample_pair(&author, "beta");
    publish_app_index(&store, &author, &manifest_a, &bundle_a, 1_000).expect("publish a");
    publish_app_index(&store, &author, &manifest_b, &bundle_b, 1_001).expect("publish b");

    let marker = |app_id: [u8; 32], timestamp: u64| {
        encode_trust_marker(&TrustMarker {
            app_id,
            author_subspace_id: mine,
            kind: TrustMarkerKind::Trust,
            timestamp_micros: timestamp,
        })
        .expect("encode trust marker")
    };

    import_signed(
        &store,
        &[
            signed_at(
                &author,
                app_index_trust_path(&app_a, &mine).expect("path"),
                &marker(app_a, 2_000),
                2_000,
            ),
            signed_at(
                &author,
                app_index_trust_path(&app_b, &mine).expect("path"),
                &marker(app_b, 2_001),
                2_001,
            ),
        ],
    );

    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.spaces.len(), 1, "one namespace carries the markers");
    let markers = &scanned.spaces[0].markers;
    assert_eq!(markers.len(), 2);

    // Sorted by (app_id, author, timestamp): app id first, so the order is a
    // pure function of content and cannot be steered by insertion order.
    let mut expected = [app_a, app_b];
    expected.sort();
    assert_eq!(
        markers
            .iter()
            .map(|marker| marker.app_id)
            .collect::<Vec<_>>(),
        expected.to_vec()
    );
    assert!(markers
        .iter()
        .all(|marker| marker.author_subspace_id == mine));
}

/// A manifest whose bundle has not arrived is *pending*, not an app: it may be
/// shown as "still arriving" but must never be launched, and it resolves to no
/// bytes. Several pending manifests come back in a stable coordinate order.
#[test]
fn pending_manifests_are_listed_in_a_stable_order_and_are_not_apps() {
    let author = generate_communal_author().expect("author");
    let store = store();

    let (manifest_a, bundle_a, app_a) = sample_pair(&author, "alpha");
    let (manifest_b, _bundle_b, app_b) = sample_pair(&author, "beta");
    assert_ne!(app_a, app_b);

    // Both manifests land; neither bundle does.
    import_signed(
        &store,
        &[
            signed_at(
                &author,
                app_index_manifest_path(&app_a).expect("path"),
                &manifest_a,
                1_000,
            ),
            signed_at(
                &author,
                app_index_manifest_path(&app_b).expect("path"),
                &manifest_b,
                1_001,
            ),
        ],
    );

    let scanned = scan_app_index(&store).expect("scan");
    assert!(
        scanned.apps.is_empty(),
        "a manifest without its bundle is not a launchable app"
    );
    assert_eq!(scanned.pending_manifests.len(), 2);

    let mut expected = [app_a, app_b];
    expected.sort();
    assert_eq!(
        scanned
            .pending_manifests
            .iter()
            .map(|pending| pending.claimed_app_id)
            .collect::<Vec<_>>(),
        expected.to_vec(),
        "pending manifests must be ordered by their Willow coordinates"
    );

    // Neither resolves to bytes — which is exactly what makes them un-openable.
    assert_eq!(app_pair_bytes(&store, &app_a).expect("resolve"), None);
    assert_eq!(app_pair_bytes(&store, &app_b).expect("resolve"), None);

    // The bundle arriving completes that one pair, and only that one.
    import_signed(
        &store,
        &[signed_at(
            &author,
            app_index_bundle_path(&app_a).expect("path"),
            &bundle_a,
            1_002,
        )],
    );
    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.apps.len(), 1);
    assert_eq!(scanned.apps[0].app_id, app_a);
    assert_eq!(scanned.pending_manifests.len(), 1);
    assert_eq!(scanned.pending_manifests[0].claimed_app_id, app_b);
    assert!(app_pair_bytes(&store, &app_a).expect("resolve").is_some());
}

/// Resolving an app's bytes steps over the other records that share its
/// prefix — an endorsement lives under `app-index/<app_id>/` too — and returns
/// the manifest and bundle, not whatever else happened to be filed there.
#[test]
fn resolving_an_apps_bytes_ignores_the_other_records_under_its_prefix() {
    let author = generate_communal_author().expect("author");
    let store = store();
    let (manifest_bytes, bundle_bytes, app_id) = sample_pair(&author, "shared-prefix");
    publish_app_index(&store, &author, &manifest_bytes, &bundle_bytes, 1_000).expect("publish");

    let mine = *author.subspace_id().as_bytes();
    let endorsement = encode_endorsement(&EndorsementMarker {
        app_id,
        note: "sits under the same prefix as the pair".into(),
        retracted: false,
    })
    .expect("encode endorsement");
    let trust = encode_trust_marker(&TrustMarker {
        app_id,
        author_subspace_id: mine,
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 2_001,
    })
    .expect("encode trust marker");

    import_signed(
        &store,
        &[
            signed_at(
                &author,
                app_index_endorsement_path(&app_id, &mine).expect("path"),
                &endorsement,
                2_000,
            ),
            signed_at(
                &author,
                app_index_trust_path(&app_id, &mine).expect("path"),
                &trust,
                2_001,
            ),
        ],
    );

    let pair = app_pair_bytes(&store, &app_id)
        .expect("resolve")
        .expect("the pair is present");
    assert_eq!(pair.manifest_bytes, manifest_bytes);
    assert_eq!(pair.bundle_bytes, bundle_bytes);

    // And the endorsement and trust marker are still read as what they are.
    let scanned = scan_app_index(&store).expect("scan");
    assert_eq!(scanned.endorsements.len(), 1);
    assert_eq!(scanned.spaces.len(), 1);
    assert_eq!(scanned.spaces[0].markers.len(), 1);
}
