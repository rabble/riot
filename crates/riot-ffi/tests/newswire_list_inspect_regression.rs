//! Regression for the FFI list/inspect path and newswire records.
//!
//! Unit 1A completed the newswire create/project/core-import path, but the FFI
//! demo-load surface (`inspectable_entries`) and the alert board
//! (`list_current_entries`) still classified only app-data, app-index, and
//! profile entries as "non-alert". A newswire entry fell through to the alert
//! branch: `inspectable_entries` rejected any bundle carrying one, and
//! `list_current_entries` returned `Internal`. It had nothing to do with the
//! demo — the moment anyone creates a newswire post through the ordinary FFI and
//! the board is listed, the bug fires. These pin it closed WITHOUT the Riverside
//! fixture: a real `create_newswire_post`, then both paths must survive it.

use riot_ffi::{open_local_profile, NewswirePostInput, NewswireSpaceInput};

#[test]
fn a_newswire_post_survives_list_current_entries_and_inspect_bytes() {
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Community".into())
        .expect("space");

    // A real newswire space + post through the ordinary FFI, committed to the
    // store exactly as a native app would create them.
    let space = profile
        .create_newswire_space(NewswireSpaceInput {
            name: "Community wire".into(),
            summary: "Local human reports.".into(),
            languages: vec!["en".into()],
            geographic_tags: vec![],
            topic_tags: vec![],
            editorial_roster: vec![],
        })
        .expect("create newswire space");
    let post = profile
        .create_newswire_post(NewswirePostInput {
            space_descriptor_entry_id: space.entry_id.clone(),
            headline: "A human report".into(),
            body: "Something happened downtown this afternoon.".into(),
            language: "en".into(),
            event_time_unix_seconds: None,
            expires_at_unix_seconds: None,
            coarse_location: None,
            source_claims: vec![],
            operational_profile: None,
            ai_assisted: false,
        })
        .expect("create newswire post");

    // list_current_entries must SURVIVE the live newswire entries — before the
    // fix it returned `Internal` because a newswire entry has no alert row — and
    // must never surface a newswire record as an alert.
    let entries = profile
        .list_current_entries()
        .expect("the alert board survives live newswire entries");
    assert!(
        entries.is_empty(),
        "a newswire space + post produce no alert rows"
    );

    // inspect_bytes runs `inspectable_entries`; a bundle carrying a newswire
    // record must be admitted for review, not rejected outright. (The record is
    // already live, so this is the idempotent case — inspection still runs the
    // whole-bundle classifier the fix touches.)
    let preview = profile
        .inspect_bytes(post.signed_bytes, "portable".into())
        .expect("inspectable_entries admits a newswire record instead of rejecting it");
    assert!(
        preview
            .eligible_entries()
            .expect("eligible entries")
            .is_empty(),
        "a newswire record is a hidden, non-alert entry — never an eligible alert row"
    );
}
