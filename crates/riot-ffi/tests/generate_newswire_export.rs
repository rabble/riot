//! Generator: mint a REAL newswire from signed Willow records and write the
//! projected collective view to a JSON export the web gateway renders.
//!
//! This is the honest pipeline — create space → sign news posts → sign
//! editorial Feature/Verify actions → project_newswire_space /
//! _contributors → serialize the projection. Nothing about the front page,
//! open wire, authors or verification is hand-authored: it is all derived from
//! signed records, exactly as a native app derives it.
//!
//! Run explicitly to (re)generate the fixture:
//!   cargo test -p riot-ffi --test generate_newswire_export -- --ignored --nocapture
//!
//! Output: fixtures/newswire/newswire-export-v1.json (git-ignored generated art;
//! signer keys are random per run, so entry ids differ each generation).

use std::fs;
use std::path::PathBuf;

use riot_ffi::{
    open_local_profile, NewswireEditorialActionInput, NewswireEditorialActionKind,
    NewswirePostInput, NewswireProjectedPost, NewswireSpaceInput,
};
use serde_json::{json, Value};

const SUMMARY: &str = "An independent, worker- and tenant-led newswire. Open publishing for \
anyone; editorial features are signed by the collective. No owners, no ads, no trackers — \
just movement media that travels peer-to-peer and mirrors anywhere.";
const LANGUAGES: [&str; 2] = ["en", "de"];
const TOPICS: [&str; 6] = ["housing", "labor", "surveillance", "ecology", "repression", "migration"];
const GEO: [&str; 1] = ["Berlin"];

fn space_input(name: &str) -> NewswireSpaceInput {
    NewswireSpaceInput {
        name: name.into(),
        summary: SUMMARY.into(),
        languages: LANGUAGES.iter().map(|s| s.to_string()).collect(),
        geographic_tags: GEO.iter().map(|s| s.to_string()).collect(),
        topic_tags: TOPICS.iter().map(|s| s.to_string()).collect(),
        editorial_roster: vec![],
    }
}

fn post_input(space: &str, headline: &str, body: &str) -> NewswirePostInput {
    NewswirePostInput {
        space_descriptor_entry_id: space.into(),
        headline: headline.into(),
        body: body.into(),
        language: "en".into(),
        event_time_unix_seconds: None,
        expires_at_unix_seconds: None,
        coarse_location: None,
        source_claims: vec![],
        operational_profile: None,
        ai_assisted: false,
    }
}

fn post_json(post: &NewswireProjectedPost) -> Value {
    json!({
        "entry_id": post.entry_id,
        "author": {
            "id": post.author.id,
            "display_name": post.author.display_name,
            "tag": post.author.tag,
            "rendered": post.author.rendered,
        },
        "tai_j2000_micros": post.tai_j2000_micros,
        "headline": post.headline,
        "body": post.body,
        "ai_assisted": post.ai_assisted,
        // Verified iff an editor signed a Verify action against this post.
        "verified": !post.verification_ids.is_empty(),
        "treatment": format!("{:?}", post.treatment),
    })
}

#[test]
#[ignore = "generator: writes a fixture; run with --ignored"]
fn generate_real_newswire_export() {
    let profile = open_local_profile().expect("profile");
    profile
        .profile()
        .set_display_name("Harbor Desk".into())
        .expect("set display name");

    let space = profile
        .create_newswire_space(space_input("RIOT · Independent Newswire"))
        .expect("create space");

    // Mint real signed news posts.
    let posts = [
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
    ];
    let mut ids = Vec::new();
    for (headline, body) in posts {
        let record = profile
            .create_newswire_post(post_input(&space.entry_id, headline, body))
            .expect("create post");
        ids.push(record.entry_id);
    }

    // Editors Feature the two lead stories and Verify the three reported ones.
    // Feature → front page; Verify → the post carries a verification.
    let feature = |target: &str| NewswireEditorialActionInput {
        space_descriptor_entry_id: space.entry_id.clone(),
        target_entry_id: target.into(),
        kind: NewswireEditorialActionKind::Feature,
        reason: None,
        correction_text: None,
    };
    let verify = |target: &str| NewswireEditorialActionInput {
        space_descriptor_entry_id: space.entry_id.clone(),
        target_entry_id: target.into(),
        kind: NewswireEditorialActionKind::Verify,
        reason: None,
        correction_text: None,
    };
    for target in [&ids[0], &ids[1]] {
        profile.create_newswire_editorial_action(feature(target)).expect("feature");
    }
    for target in [&ids[0], &ids[1], &ids[2]] {
        profile.create_newswire_editorial_action(verify(target)).expect("verify");
    }

    // Project the collective view + contributors from the signed records.
    let projection = profile
        .project_newswire_space(space.entry_id.clone())
        .expect("project");
    let contributors = profile
        .project_newswire_contributors(space.entry_id.clone())
        .expect("contributors");

    assert!(!projection.front_page.is_empty(), "features must promote to the front page");
    assert!(!projection.open_wire.is_empty(), "unfeatured posts stay on the open wire");

    let export = json!({
        "schema": "riot.newswire.export/1",
        "note": "REAL projected view of signed Willow newswire records — not hand-authored.",
        "space": {
            "name": "RIOT · Independent Newswire",
            "descriptor_entry_id": space.entry_id,
            "summary": SUMMARY,
            "languages": LANGUAGES,
            "topics": TOPICS,
            "geographic": GEO,
        },
        "front_page": projection.front_page.iter().map(post_json).collect::<Vec<_>>(),
        "open_wire": projection.open_wire.iter().map(post_json).collect::<Vec<_>>(),
        "contributors": contributors.iter().map(|c| json!({
            "id": c.author.id,
            "rendered": c.author.rendered,
            "display_name": c.author.display_name,
            "is_organizer": c.is_organizer,
            "contribution_count": c.contribution_count,
        })).collect::<Vec<_>>(),
    });

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("fixtures/newswire/newswire-export-v1.json");
    fs::create_dir_all(out.parent().unwrap()).expect("mkdir");
    fs::write(&out, serde_json::to_string_pretty(&export).unwrap() + "\n").expect("write export");
    eprintln!("wrote {}", out.display());
    eprintln!(
        "front_page={} open_wire={} contributors={}",
        projection.front_page.len(),
        projection.open_wire.len(),
        contributors.len()
    );
}
