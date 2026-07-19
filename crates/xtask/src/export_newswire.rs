//! `export-newswire`: mints a REAL signed newswire (space descriptor + news
//! posts + editorial Feature/Verify actions) through riot-core, projects the
//! collective view, and writes two golden fixtures — the proof-bearing signed
//! record set and the proof-free `riot-public-gateway-export/2` public export
//! the web gateway consumes. This is the newswire twin of
//! `sign_conference_fixture` + the conference public export, unified into one
//! producing command. Signature RE-verification lives in
//! `verify_newswire_export`, mirroring `verify-conference-export`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use riot_core::newswire::{
    contributors, create_signed_editorial_action, create_signed_news_post,
    create_signed_space_descriptor, inspect_news_record, project, ContributorRowV1,
    EditorialActionKind, EditorialActionV1, NewsPostV1, NewswirePayload, PostTreatment,
    ProjectionClockV1, SignedNewswireRecord, SpaceDescriptorV1,
};
use riot_core::profile::card::ProfileCard;
use riot_core::profile::create_signed_profile_card;
use riot_core::willow::{
    entry_id, generate_communal_author_for_namespace, generate_space_organizer_author,
    system_snapshot, SignedWillowEntry,
};
use serde_json::{json, Value};

use crate::hex_codec;

const SPACE_NAME: &str = "RIOT · Independent Newswire";

/// One row of the public export, pre-serialization.
pub struct PublicEntry {
    pub entry_id: [u8; 32],
    pub signer: [u8; 32],
    pub headline: String,
    pub body: String,
    pub ai_assisted: bool,
    pub tai_j2000_micros: u64,
    pub featured: bool,
    pub editorially_verified: bool,
}

/// A signed profile card the producer minted for one of its named authors. The
/// display name is carried alongside so the public export's `contributors[]`
/// can be built without re-reading the card payload.
pub struct SignedProfileCard {
    pub signer: [u8; 32],
    pub display_name: String,
    pub signed: SignedWillowEntry,
}

/// The full in-memory result of minting + projecting the newswire.
pub struct BuiltNewswire {
    pub namespace: [u8; 32],
    pub descriptor_entry_id: [u8; 32],
    pub records: Vec<SignedNewswireRecord>,
    pub public_entries: Vec<PublicEntry>,
    /// Signed display-name cards for the named authors (organizer + roster
    /// editor). Communal/anonymous authors get no card and are absent here.
    pub profile_cards: Vec<SignedProfileCard>,
    /// The content-derived contributor set (`contributors()` of the projection).
    pub contributors: Vec<ContributorRowV1>,
    /// `author_id → display_name` for the authors that carry a card.
    pub display_names: BTreeMap<[u8; 32], String>,
}

/// The activist content the gateway already renders (kept identical in spirit
/// to the ffi generator so the page is unchanged). When WS4 lands a real
/// composite-site newswire namespace, only this content source swaps.
const POSTS: &[(&str, &str)] = &[
    ("Rent strike jumps three more blocks as tenants tear up eviction notices",
     "Four hundred households on Sonnenallee are now withholding rent — the largest coordinated tenant action since the 2023 deposit fight. The union answered eviction filings with a block-by-block watch."),
    ("Port workers walk out in solidarity; container terminal at a standstill",
     "The wildcat action began at the night shift. Cranes idle, 6,000 boxes stranded. Dockers hold the gate until the fired stewards are reinstated."),
    ("Leaked procurement docs show the city quietly bought facial-recognition vans",
     "Four unmarked units, invoiced under \"traffic safety.\" The contract and vendor spec sheet are published in full."),
    ("Medic station open at the old library, side entrance",
     "Volunteers are staffing a first-aid point at the west entrance. Water and shade available."),
    ("Cops massing at the north gate, roughly forty vans",
     "Eyewitness report from the strike blocks. Bring water and legal-observer numbers."),
    ("Drone overhead on Sonnenallee, circling the strike blocks",
     "Low-altitude drone seen over the rent-strike blocks for the past twenty minutes."),
    ("RETRACTED: unverified claim of troops at the depot",
     "This early report could not be substantiated and is hidden by the editors."),
];
// POSTS[6] is Hidden by an editorial Hide action below — it exercises the
// moderation-drop `continue` branch and proves Hidden content never reaches the
// public export. Six Ordinary posts remain public.

fn news_post(descriptor_entry_id: [u8; 32], headline: &str, body: &str) -> NewsPostV1 {
    NewsPostV1 {
        space_descriptor_entry_id: descriptor_entry_id,
        headline: headline.to_string(),
        body: body.to_string(),
        language: "en".to_string(),
        event_time_unix_seconds: None,
        expires_at_unix_seconds: None,
        coarse_location: None,
        source_claims: vec![],
        operational_profile: None,
        ai_assisted: false,
    }
}

pub fn build_signed_newswire() -> Result<BuiltNewswire, String> {
    // Founder (organizer: namespace == subspace) + one roster editor.
    let founder = generate_space_organizer_author().map_err(|e| format!("founder: {e}"))?;
    let namespace = *founder.namespace_id().as_bytes();
    let editor =
        generate_communal_author_for_namespace(namespace).map_err(|e| format!("editor: {e}"))?;
    let editor_id = *editor.subspace_id().as_bytes();

    let descriptor = SpaceDescriptorV1 {
        namespace_id: namespace,
        name: SPACE_NAME.to_string(),
        summary: "Independent community newswire.".to_string(),
        languages: vec!["en".to_string()],
        geographic_tags: vec![],
        topic_tags: vec![],
        editorial_roster: vec![editor_id],
        predecessor: None,
        successor: None,
    };
    let descriptor_record = create_signed_space_descriptor(&founder, descriptor)
        .map_err(|e| format!("sign descriptor: {e}"))?;
    let descriptor_verified = inspect_news_record(&descriptor_record.signed)
        .map_err(|e| format!("inspect descriptor: {e}"))?;
    let descriptor_entry_id = descriptor_record.entry_id;

    // Posts signed by the organizer; each inspected into a VerifiedNewswireRecord.
    let mut records = vec![descriptor_record.clone()];
    let mut post_ids = Vec::new();
    for (headline, body) in POSTS {
        let record = create_signed_news_post(
            &founder,
            &descriptor_verified,
            news_post(descriptor_entry_id, headline, body),
        )
        .map_err(|e| format!("sign post: {e}"))?;
        post_ids.push(record.entry_id);
        records.push(record);
    }

    // Editors Feature the two leads and Verify the first three, signed by the
    // roster editor (authority: signer ∈ editorial_roster).
    let action = |target: [u8; 32], kind: EditorialActionKind| EditorialActionV1 {
        space_descriptor_entry_id: descriptor_entry_id,
        target_entry_id: target,
        kind,
        reason: None,
        correction_text: None,
    };
    for target in [post_ids[0], post_ids[1]] {
        records.push(
            create_signed_editorial_action(
                &editor,
                &descriptor_verified,
                action(target, EditorialActionKind::Feature),
            )
            .map_err(|e| format!("sign feature: {e}"))?,
        );
    }
    for target in [post_ids[0], post_ids[1], post_ids[2]] {
        records.push(
            create_signed_editorial_action(
                &editor,
                &descriptor_verified,
                action(target, EditorialActionKind::Verify),
            )
            .map_err(|e| format!("sign verify: {e}"))?,
        );
    }
    // Hide the seventh post so the moderation-drop branch is exercised on the real
    // path and Hidden content is proven to never reach the public export. Unlike
    // Feature/Verify, a Hide action REQUIRES a `reason` (riot-core model rule:
    // Correct/Hide/Tombstone/Retract → EditorialReasonRequired if reason is None),
    // so it is built explicitly rather than through the reason-less `action`
    // closure used for the Feature/Verify actions above.
    records.push(
        create_signed_editorial_action(
            &editor,
            &descriptor_verified,
            EditorialActionV1 {
                space_descriptor_entry_id: descriptor_entry_id,
                target_entry_id: post_ids[6],
                kind: EditorialActionKind::Hide,
                reason: Some("Unsubstantiated report retracted by the editors.".to_string()),
                correction_text: None,
            },
        )
        .map_err(|e| format!("sign hide: {e}"))?,
    );

    // Project the collective view from the inspected records (descriptor passed
    // separately, exactly as store::project_space does).
    let verified_records = records
        .iter()
        .skip(1) // skip the descriptor; it is the projection anchor, passed below
        .map(|record| inspect_news_record(&record.signed))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("inspect records: {e}"))?;
    let clock = ProjectionClockV1::system().map_err(|e| format!("clock: {e}"))?;
    let projection = project(&descriptor_verified, &verified_records, clock)
        .map_err(|e| format!("project: {e}"))?;

    // Visible posts = union of front_page + open_wire, de-duped by entry_id,
    // Ordinary only (mirrors newswire.py all_posts + _visible).
    let featured_ids: BTreeSet<[u8; 32]> =
        projection.front_page.iter().map(|p| p.entry_id).collect();
    let mut seen: BTreeSet<[u8; 32]> = BTreeSet::new();
    let mut public_entries = Vec::new();
    for post in projection
        .front_page
        .iter()
        .chain(projection.open_wire.iter())
    {
        if !matches!(post.treatment, PostTreatment::Ordinary) {
            continue; // Hidden/Tombstoned vanish from the public surface.
        }
        if !seen.insert(post.entry_id) {
            continue;
        }
        public_entries.push(PublicEntry {
            entry_id: post.entry_id,
            signer: post.author_id,
            headline: post.headline.clone().unwrap_or_default(),
            body: post.body.clone().unwrap_or_default(),
            ai_assisted: post.ai_assisted,
            tai_j2000_micros: post.tai_j2000_micros,
            featured: featured_ids.contains(&post.entry_id),
            editorially_verified: !post.verification_ids.is_empty(),
        });
    }

    // Mint a signed display-name card for each NAMED author — the organizer desk
    // and the roster editor. A card is a signed Willow record at
    // `profile/<subspace>/card` that SYNCs, so carrying the name in the export is
    // honest, not fabricated. Communal/anonymous authors get no card and stay
    // nameless (absent from `contributors[]`; the gateway renders them as open
    // contributors). This fixture's posts are all organizer-signed, so both
    // contributors are named here.
    let card_micros = system_snapshot()
        .map_err(|e| format!("card clock: {e}"))?
        .tai_j2000_micros;
    let mut profile_cards = Vec::new();
    let mut display_names: BTreeMap<[u8; 32], String> = BTreeMap::new();
    for (author, name) in [(&founder, "RIOT Editorial Desk"), (&editor, "Harbor Desk")] {
        let card = ProfileCard {
            display_name: name.to_string(),
        };
        let signed = create_signed_profile_card(author, &card, card_micros)
            .map_err(|e| format!("mint card {name}: {e}"))?;
        let signer = *author.subspace_id().as_bytes();
        display_names.insert(signer, name.to_string());
        profile_cards.push(SignedProfileCard {
            signer,
            display_name: name.to_string(),
            signed,
        });
    }

    // The content-derived contributor set: every author of a projected post plus
    // every editorial signer, with the organizer marked by `author_id == namespace`.
    let contributor_rows = contributors(&projection, namespace);

    Ok(BuiltNewswire {
        namespace,
        descriptor_entry_id,
        records,
        public_entries,
        profile_cards,
        contributors: contributor_rows,
        display_names,
    })
}

fn record_kind(payload: &NewswirePayload) -> &'static str {
    match payload {
        NewswirePayload::SpaceDescriptor(_) => "space_descriptor",
        NewswirePayload::NewsPost(_) => "news_post",
        NewswirePayload::EditorialAction(_) => "editorial_action",
        NewswirePayload::NewsComment(_) => "news_comment",
        NewswirePayload::NewsReaction(_) => "news_reaction",
    }
}

fn signed_record_json(record: &SignedNewswireRecord) -> Result<Value, String> {
    // `SignedNewswireRecord` exposes no signer id directly; derive it by
    // re-inspecting the signed entry (VerifiedNewswireRecord::signer_id is the
    // only public accessor) rather than widening riot-core's public API.
    let signer = inspect_news_record(&record.signed)
        .map_err(|e| format!("signer: {e}"))?
        .signer_id();
    let mut value = json!({
        "record_kind": record_kind(&record.payload),
        "willow_entry_id": hex_codec::encode(&record.entry_id),
        "signer": hex_codec::encode(&signer),
        "willow_entry_bytes": hex_codec::encode(&record.signed.entry_bytes),
        "willow_capability_bytes": hex_codec::encode(&record.signed.capability_bytes),
        "signature": hex_codec::encode(&record.signed.signature),
    });
    if let NewswirePayload::NewsPost(post) = &record.payload {
        value["headline"] = json!(post.headline);
        value["body"] = json!(post.body);
        value["ai_assisted"] = json!(post.ai_assisted);
        value["tai_j2000_micros"] = json!(record.snapshot.tai_j2000_micros);
    }
    if let NewswirePayload::EditorialAction(action) = &record.payload {
        value["action_kind"] = json!(format!("{:?}", action.kind));
        value["target_entry_id"] = json!(hex_codec::encode(&action.target_entry_id));
    }
    if let NewswirePayload::NewsComment(comment) = &record.payload {
        value["body"] = json!(comment.body);
        value["parent_entry_id"] = json!(hex_codec::encode(&comment.parent_entry_id));
        value["tai_j2000_micros"] = json!(record.snapshot.tai_j2000_micros);
    }
    if let NewswirePayload::NewsReaction(reaction) = &record.payload {
        value["kind"] = json!(format!("{:?}", reaction.kind));
        value["active"] = json!(reaction.active);
        value["parent_entry_id"] = json!(hex_codec::encode(&reaction.parent_entry_id));
        value["tai_j2000_micros"] = json!(record.snapshot.tai_j2000_micros);
    }
    Ok(value)
}

/// A minted profile card as a proof-bearing signed-space record. `record_kind`
/// is `"profile_card"`; the display name rides along so the verifier and any
/// reader can bind the name to its author without re-decoding the payload.
fn signed_card_json(card: &SignedProfileCard) -> Value {
    json!({
        "record_kind": "profile_card",
        "willow_entry_id": hex_codec::encode(&entry_id(&card.signed.entry_bytes)),
        "signer": hex_codec::encode(&card.signer),
        "willow_entry_bytes": hex_codec::encode(&card.signed.entry_bytes),
        "willow_capability_bytes": hex_codec::encode(&card.signed.capability_bytes),
        "signature": hex_codec::encode(&card.signed.signature),
        "display_name": card.display_name,
    })
}

/// The public `contributors[]` block: one row per content-derived contributor
/// that carries a display-name card. An author with NO card is OMITTED — a
/// communal/anonymous poster is legitimately nameless and the gateway renders it
/// as an open contributor, never a fabricated name.
fn contributors_block(rows: &[ContributorRowV1], names: &BTreeMap<[u8; 32], String>) -> Vec<Value> {
    rows.iter()
        .filter_map(|row| {
            names.get(&row.author_id).map(|display_name| {
                json!({
                    "author_id": hex_codec::encode(&row.author_id),
                    "display_name": display_name,
                    "is_organizer": row.is_organizer,
                    "contribution_count": row.contribution_count,
                })
            })
        })
        .collect()
}

fn rfc3339_utc(unix_seconds: u64) -> String {
    // Minimal, dependency-free UTC formatter (proleptic Gregorian). xtask has
    // no chrono; this only stamps the informational `generated_at`.
    let days = (unix_seconds / 86_400) as i64;
    let secs_of_day = unix_seconds % 86_400;
    let (h, m, s) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
    let (mut y, mut d) = (1970i64, days);
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let yd = if leap { 366 } else { 365 };
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0usize;
    while d >= months[mo] as i64 {
        d -= months[mo] as i64;
        mo += 1;
    }
    format!("{y:04}-{:02}-{:02}T{h:02}:{m:02}:{s:02}Z", mo + 1, d + 1)
}

pub fn run(root: &Path) -> Result<(), String> {
    let built = build_signed_newswire()?;

    // Newswire records first, then the profile-card records (same array, mixed
    // `record_kind`); the verifier re-verifies every record's signature the same
    // way, keyed by `willow_entry_id`.
    let mut signed_records = built
        .records
        .iter()
        .map(signed_record_json)
        .collect::<Result<Vec<_>, _>>()?;
    signed_records.extend(built.profile_cards.iter().map(signed_card_json));
    let signed_doc = json!({
        "schema": "riot.newswire.signed-space/1",
        "namespace": hex_codec::encode(&built.namespace),
        "descriptor_entry_id": hex_codec::encode(&built.descriptor_entry_id),
        "records": signed_records,
    });
    let signed_dir = root.join("fixtures/newswire");
    fs::create_dir_all(&signed_dir).map_err(|e| format!("mkdir {}: {e}", signed_dir.display()))?;
    let signed_path = signed_dir.join("signed-space-v1.json");
    let signed_bytes = serde_json::to_string_pretty(&signed_doc)
        .map_err(|e| format!("serialize signed fixture: {e}"))?
        + "\n";
    fs::write(&signed_path, &signed_bytes)
        .map_err(|e| format!("write {}: {e}", signed_path.display()))?;

    let clock = ProjectionClockV1::system().map_err(|e| format!("clock: {e}"))?;
    let export = json!({
        "schema": "riot-public-gateway-export/2",
        "export_revision": "newswire-gateway-export-v1",
        "generated_at": rfc3339_utc(clock.unix_seconds()),
        "namespace": hex_codec::encode(&built.namespace),
        "renderer_profile": "newswire-front/1",
        "source_fixture": "fixtures/newswire/signed-space-v1.json",
        "source_fixture_sha256": crate::sha256_hex(signed_bytes.as_bytes()),
        "title": SPACE_NAME,
        "visibility": "public",
        "entries": built.public_entries.iter().map(|entry| json!({
            "entry_id": hex_codec::encode(&entry.entry_id),
            "signer": hex_codec::encode(&entry.signer),
            "kind": "post",
            "title": entry.headline,
            "body": entry.body,
            "ai_assisted": entry.ai_assisted,
            "tai_j2000_micros": entry.tai_j2000_micros,
            "featured": entry.featured,
            "editorially_verified": entry.editorially_verified,
        })).collect::<Vec<_>>(),
        // Additive display-identity block (WS1-b): who the named authors are.
        // `entries[]` above is unchanged and forward-compatible.
        "contributors": contributors_block(&built.contributors, &built.display_names),
    });
    let export_dir = signed_dir.join("gateway-space");
    fs::create_dir_all(&export_dir).map_err(|e| format!("mkdir {}: {e}", export_dir.display()))?;
    let export_path = export_dir.join("public-export-v1.json");
    fs::write(
        &export_path,
        serde_json::to_string_pretty(&export).map_err(|e| format!("serialize export: {e}"))? + "\n",
    )
    .map_err(|e| format!("write {}: {e}", export_path.display()))?;

    println!(
        "export-newswire: PASS (namespace={}, {} public entries)",
        hex_codec::encode(&built.namespace),
        built.public_entries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use riot_core::newswire::inspect_news_record;

    #[test]
    fn build_mints_signed_records_that_all_verify_and_project_editorially() {
        let built = build_signed_newswire().expect("build signed newswire");

        // Every signed record's signature verifies structurally.
        for record in &built.records {
            inspect_news_record(&record.signed)
                .expect("each minted record is a valid signed newswire entry");
        }

        // Two posts were Featured and three Verified (see build_signed_newswire).
        let featured: usize = built
            .public_entries
            .iter()
            .filter(|entry| entry.featured)
            .count();
        let verified: usize = built
            .public_entries
            .iter()
            .filter(|entry| entry.editorially_verified)
            .count();
        assert_eq!(featured, 2, "two Feature actions promote two posts");
        assert_eq!(verified, 3, "three Verify actions mark three posts");
        // Seven posts minted, one Hidden by an editorial Hide action → exactly six
        // Ordinary posts reach the public surface (moderation-drop branch exercised).
        assert_eq!(
            built.public_entries.len(),
            6,
            "the Hidden seventh post is dropped; six Ordinary posts are exported"
        );
        assert!(
            !built
                .public_entries
                .iter()
                .any(|entry| entry.headline.starts_with("RETRACTED")),
            "the Hidden post must never appear in the public export"
        );
    }

    #[test]
    fn run_writes_both_golden_fixtures_in_a_consistent_state() {
        let root = std::env::temp_dir().join(format!("riot-export-nw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        run(&root).expect("export succeeds into a fresh root");

        let signed: Value = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/newswire/signed-space-v1.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(signed["schema"], "riot.newswire.signed-space/1");
        let signed_records = signed["records"].as_array().unwrap();
        assert_eq!(
            signed_records.len(),
            1 + 7 + 6 + 2,
            "descriptor + 7 posts + 6 actions (2 Feature + 3 Verify + 1 Hide) + 2 profile cards"
        );
        for record in signed_records {
            assert_eq!(record["signature"].as_str().unwrap().len(), 128);
            assert_eq!(record["willow_entry_id"].as_str().unwrap().len(), 64);
            assert!(!record["willow_entry_bytes"].as_str().unwrap().is_empty());
            assert!(!record["willow_capability_bytes"]
                .as_str()
                .unwrap()
                .is_empty());
        }
        // WS1-b: the two named authors carry signed profile-card records.
        let cards: Vec<&Value> = signed_records
            .iter()
            .filter(|r| r["record_kind"] == "profile_card")
            .collect();
        assert_eq!(cards.len(), 2, "organizer + roster editor each have a card");
        for card in &cards {
            assert!(!card["display_name"].as_str().unwrap().is_empty());
        }

        let export: Value = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/newswire/gateway-space/public-export-v1.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(export["schema"], "riot-public-gateway-export/2");
        assert_eq!(export["visibility"], "public");
        let entries = export["entries"].as_array().unwrap();
        assert!(!entries.is_empty());
        for entry in entries {
            // Proof-free public boundary: no signature/capability/entry-bytes.
            assert!(entry.get("signature").is_none());
            assert!(entry.get("willow_capability_bytes").is_none());
            assert!(entry.get("willow_entry_bytes").is_none());
            assert_eq!(entry["kind"], "post");
            assert!(entry["entry_id"].as_str().unwrap().len() == 64);
        }
        // source_fixture_sha256 matches the signed fixture bytes on disk.
        let signed_bytes = fs::read(root.join("fixtures/newswire/signed-space-v1.json")).unwrap();
        assert_eq!(
            export["source_fixture_sha256"].as_str().unwrap(),
            crate::sha256_hex(&signed_bytes)
        );

        // WS1-b: the additive contributors[] block names the desks and marks the
        // organizer. entries[] stays proof-free (asserted above), unchanged.
        let contributors = export["contributors"].as_array().unwrap();
        assert_eq!(contributors.len(), 2, "organizer + roster editor");
        let organizer = contributors
            .iter()
            .find(|c| c["is_organizer"] == true)
            .expect("an organizer contributor");
        assert_eq!(organizer["display_name"], "RIOT Editorial Desk");
        assert!(organizer["contribution_count"].as_u64().unwrap() > 0);
        assert!(
            contributors
                .iter()
                .any(|c| c["display_name"] == "Harbor Desk" && c["is_organizer"] == false),
            "the roster editor is a named, non-organizer contributor"
        );
        // Display-only boundary: a contributor row carries no key material.
        for contributor in contributors {
            assert!(contributor.get("signature").is_none());
            assert!(contributor.get("willow_capability_bytes").is_none());
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn contributors_block_names_the_carded_and_omits_the_nameless() {
        let organizer = [0x11u8; 32];
        let editor = [0x22u8; 32];
        let anon = [0x33u8; 32]; // a communal poster with no card
        let rows = vec![
            ContributorRowV1 {
                author_id: organizer,
                is_organizer: true,
                contribution_count: 6,
            },
            ContributorRowV1 {
                author_id: editor,
                is_organizer: false,
                contribution_count: 4,
            },
            ContributorRowV1 {
                author_id: anon,
                is_organizer: false,
                contribution_count: 1,
            },
        ];
        let mut names = BTreeMap::new();
        names.insert(organizer, "RIOT Editorial Desk".to_string());
        names.insert(editor, "Harbor Desk".to_string());

        let block = contributors_block(&rows, &names);

        assert_eq!(block.len(), 2, "the card-less communal author is omitted");
        assert!(block
            .iter()
            .any(|c| c["display_name"] == "RIOT Editorial Desk"
                && c["is_organizer"] == true
                && c["contribution_count"] == 6));
        assert!(block
            .iter()
            .any(|c| c["display_name"] == "Harbor Desk" && c["is_organizer"] == false));
        assert!(
            !block
                .iter()
                .any(|c| c["author_id"] == hex_codec::encode(&anon)),
            "a nameless author never appears in contributors[]"
        );
    }
}
