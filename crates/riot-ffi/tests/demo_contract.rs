//! FFI contract for demo mode, exercised against the REAL committed bundle —
//! `fixtures/demo/riverside/demo-space.riot-evidence`, the signed RIOTE1 bytes
//! `pack_demo_space` emits. Nothing here is mocked: if the demo could only load
//! through a privileged path, these tests would be the ones that failed.
//!
//! Four properties carry the weight:
//!
//! * **Ordinary pipeline.** The bundle commits through the same
//!   `inspect → plan → commit` a peer's bundle does, and afterwards the space
//!   lists, six alerts are live, and four members resolve to real names.
//! * **Additive.** A profile that already lists a real space is left bit-for-bit
//!   alone — the entry ids, which ARE the content hashes, are identical before
//!   and after. A corrupt bundle leaves it identical too: the import is
//!   transactional, so nothing lands half-imported.
//! * **Idempotent.** Entries are content-addressed, so a second load dedupes
//!   through the ordinary join and adds nothing.
//! * **Hiding is not deleting.** Willow is append-only. Hiding un-LISTS the
//!   namespace; the entries stay in the store, unreachable because no listed
//!   space names them.

use riot_ffi::{
    open_local_profile, AlertCertainty, AlertDraftInput, AlertSeverity, AlertUrgency, MobileError,
    MobileProfile,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const DEMO_BUNDLE: &[u8] =
    include_bytes!("../../../fixtures/demo/riverside/demo-space.riot-evidence");

/// The seeded space's title, as `load_demo_space` lists it.
const DEMO_TITLE: &str = "Riverside Tenants Union";

fn demo_bytes() -> Vec<u8> {
    DEMO_BUNDLE.to_vec()
}

fn expires_later() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after unix epoch")
        .as_secs()
        + 3_600
}

/// The committed bundle with one byte of its middle flipped: still a plausible
/// frame, no longer a bundle anything will admit.
fn corrupt_demo_bytes() -> Vec<u8> {
    let mut bytes = demo_bytes();
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0xff;
    bytes
}

/// Live entry ids for the listed space, sorted. An entry id is the content hash
/// of the signed entry, so equality of these lists IS bit-for-bit equality of
/// the space's contents.
fn live_entry_ids(profile: &MobileProfile) -> Vec<String> {
    let mut ids: Vec<String> = profile
        .list_current_entries()
        .expect("list current entries")
        .into_iter()
        .map(|entry| entry.entry_id)
        .collect();
    ids.sort();
    ids
}

fn rendered_names(profile: &MobileProfile) -> Vec<String> {
    profile
        .profile()
        .display_names()
        .expect("display names")
        .into_iter()
        .map(|record| record.rendered)
        .collect()
}

/// A profile with a real space of its own and one signed alert in it — the thing
/// demo mode must never touch.
fn profile_with_real_space() -> Arc<MobileProfile> {
    let profile = open_local_profile().expect("profile");
    profile
        .create_public_space("Berlin Mutual Aid".to_string())
        .expect("create space");
    let draft = profile
        .create_draft_alert(AlertDraftInput {
            valid_from: None,
            expires_at: expires_later(),
            language: "en".to_string(),
            urgency: AlertUrgency::Immediate,
            severity: AlertSeverity::Severe,
            certainty: AlertCertainty::Observed,
            headline: "Water at the north gate".to_string(),
            description: "Refills available all afternoon.".to_string(),
            affected_area_claim: None,
            source_claims: vec!["Two field observers".to_string()],
            ai_assisted: false,
        })
        .expect("draft");
    profile.sign_draft(draft.draft_id).expect("sign");
    profile
}

// MARK: - The ordinary pipeline

#[test]
fn loading_the_demo_bundle_lists_the_space_with_its_alerts_and_members() {
    let profile = open_local_profile().expect("profile");

    let space = profile.load_demo_space(demo_bytes()).expect("load demo");

    assert_eq!(space.title, DEMO_TITLE);
    assert!(space.is_public);
    assert_eq!(space.namespace_id.len(), 64, "namespace id is 32 hex bytes");

    let entries = profile.list_current_entries().expect("entries");
    assert_eq!(entries.len(), 6, "six seeded alerts are live");
    for entry in &entries {
        assert_eq!(
            entry.namespace_id, space.namespace_id,
            "every alert belongs to the listed space"
        );
        assert!(!entry.headline.trim().is_empty(), "alerts carry a headline");
    }

    // The four members render as real names, never as `member-<hex>`, and never
    // bare: the key tag rides along with every one of them.
    let names = rendered_names(&profile);
    for member in ["Ana", "Marcus", "Priya", "Dee"] {
        assert!(
            names.iter().any(|name| name.starts_with(&format!("{member} · "))),
            "{member} resolves to a rendered display name; got {names:?}"
        );
    }
}

// MARK: - Additive: a real space is never touched

#[test]
fn a_listed_real_space_is_bit_for_bit_unchanged_across_a_demo_load() {
    let profile = profile_with_real_space();
    let before = live_entry_ids(&profile);
    assert_eq!(before.len(), 1, "the real space starts with its own alert");

    // Refused, precisely so the real space cannot be displaced: the store is
    // single-namespace in practice, and mixing the demo in would take this
    // person's sync away.
    let error = profile
        .load_demo_space(demo_bytes())
        .expect_err("a real space is never displaced");
    assert!(matches!(error, MobileError::ImportRejected));

    assert_eq!(
        live_entry_ids(&profile),
        before,
        "the real space's entries are identical, byte for byte"
    );
    // And the profile is not left in some third state: it still works.
    assert!(profile.open_sync_session().is_ok(), "sync still opens");
}

#[test]
fn a_corrupt_bundle_is_refused_and_leaves_a_real_space_untouched() {
    let profile = profile_with_real_space();
    let before = live_entry_ids(&profile);

    let error = profile
        .load_demo_space(corrupt_demo_bytes())
        .expect_err("corrupt bytes are refused");
    assert!(matches!(error, MobileError::ImportRejected));

    assert_eq!(live_entry_ids(&profile), before, "nothing was half-imported");
}

#[test]
fn a_corrupt_bundle_leaves_a_fresh_profile_with_nothing_imported() {
    let profile = open_local_profile().expect("profile");

    let error = profile
        .load_demo_space(corrupt_demo_bytes())
        .expect_err("corrupt bytes are refused");
    assert!(matches!(error, MobileError::ImportRejected));

    // No space was listed, so nothing half-landed. Prove the store is genuinely
    // empty rather than merely unlistable.
    profile
        .create_public_space("Fresh".to_string())
        .expect("a refused demo load leaves the profile usable");
    assert!(
        profile.list_current_entries().expect("entries").is_empty(),
        "a refused load commits nothing"
    );
}

// MARK: - Idempotence

#[test]
fn loading_the_demo_twice_duplicates_nothing() {
    let profile = open_local_profile().expect("profile");

    let first_space = profile.load_demo_space(demo_bytes()).expect("first load");
    let first_ids = live_entry_ids(&profile);
    let first_names = rendered_names(&profile);

    let second_space = profile.load_demo_space(demo_bytes()).expect("second load");
    let second_ids = live_entry_ids(&profile);
    let second_names = rendered_names(&profile);

    assert_eq!(first_space, second_space, "the same space is listed");
    assert_eq!(first_ids.len(), 6);
    assert_eq!(
        first_ids, second_ids,
        "content-addressed entries dedupe through the ordinary join"
    );
    assert_eq!(first_names, second_names, "no duplicate profile cards");

    // Sync still opens afterwards: the inventory covers the store exactly once.
    assert!(profile.open_sync_session().is_ok());
}

// MARK: - The active-sync guard

#[test]
fn demo_mode_is_refused_while_a_sync_session_is_open_and_does_not_brick_it() {
    let profile = open_local_profile().expect("profile");
    profile.load_demo_space(demo_bytes()).expect("load demo");

    let session = profile.open_sync_session().expect("sync session");

    // Both commit through `store.inspect`, which would clobber the preview slot
    // the in-flight review is holding — the same guard `app_data_put` carries.
    assert!(matches!(
        profile.load_demo_space(demo_bytes()),
        Err(MobileError::InvalidInput)
    ));
    assert!(matches!(
        profile.hide_demo_space(),
        Err(MobileError::InvalidInput)
    ));

    // The refusal is clean: the session was not damaged, and a later one opens.
    session.cancel().expect("cancel");
    assert!(profile.open_sync_session().is_ok(), "sync is not bricked");
    assert_eq!(live_entry_ids(&profile).len(), 6, "and nothing was lost");
}

// MARK: - Hiding is un-listing, not deleting

#[test]
fn hiding_unlists_the_demo_and_restores_the_pre_demo_identity() {
    let profile = open_local_profile().expect("profile");
    let identity_before = profile.identity().expect("identity");

    profile.load_demo_space(demo_bytes()).expect("load demo");
    assert_eq!(profile.list_current_entries().expect("entries").len(), 6);

    profile.hide_demo_space().expect("hide");

    // No space lists the demo namespace any more, so nothing in the UI can reach
    // its entries — which are still in the store, because Willow is append-only
    // and there is no delete primitive.
    assert!(
        profile.list_current_entries().is_err(),
        "no space is listed once the demo is hidden"
    );
    assert_eq!(
        profile.identity().expect("identity"),
        identity_before,
        "the person's own identity comes back untouched"
    );

    // A real space made afterwards sees an empty board: the demo's entries are
    // still in the store, but they belong to a namespace nobody lists.
    profile
        .create_public_space("Berlin Mutual Aid".to_string())
        .expect("create space");
    assert!(
        profile.list_current_entries().expect("entries").is_empty(),
        "hidden demo entries are inert, not listed"
    );
}

#[test]
fn hiding_is_a_no_op_when_demo_mode_was_never_on() {
    let profile = profile_with_real_space();
    let before = live_entry_ids(&profile);

    profile.hide_demo_space().expect("hide is a no-op");

    assert_eq!(
        live_entry_ids(&profile),
        before,
        "hide never un-lists a space it did not list"
    );
}

#[test]
fn the_demo_can_be_loaded_again_after_hiding() {
    let profile = open_local_profile().expect("profile");
    let space = profile.load_demo_space(demo_bytes()).expect("load demo");
    profile.hide_demo_space().expect("hide");

    let reloaded = profile.load_demo_space(demo_bytes()).expect("reload");

    assert_eq!(reloaded, space);
    assert_eq!(profile.list_current_entries().expect("entries").len(), 6);
}
