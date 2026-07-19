//! `admit_followed_site_frame` — the single canonical followed-site admission
//! gate. These are the core-level adversarial cases the FFI (Option B / WU2) and
//! transport (WU3) callers all inherit. Requires `conformance` for store
//! construction, like the other core_import tests.

use riot_core::import::encode_bundle;
use riot_core::session::{EvidenceStore, RiotSession};
use riot_core::site::{admit_followed_site_frame, FollowedSiteAdmitError};
use riot_core::willow::site_paths::{ARTICLES_COMPONENT, MOD_COMPONENT};
use riot_core::willow::{
    encode_capability, encode_entry, Entry, NamespaceId, Path, SignedWillowEntry,
};
use willow25::prelude::{NamespaceSecret, SubspaceSecret, WriteCapability};

/// An owned site whose secrets the test controls — it can sign at any path under
/// its `new_owned` cap.
struct OwnedSite {
    root: [u8; 32],
    namespace_id: NamespaceId,
    owner_secret: SubspaceSecret,
    owner_cap: WriteCapability,
}

fn owned_site(namespace_seed: u8, owner_seed: u8) -> OwnedSite {
    let mut seed = [namespace_seed; 32];
    let namespace_secret = loop {
        let candidate = NamespaceSecret::from_bytes(&seed);
        if candidate.corresponding_namespace_id().is_owned() {
            break candidate;
        }
        seed[0] = seed[0].wrapping_add(1);
    };
    let namespace_id = namespace_secret.corresponding_namespace_id();
    let owner_secret = SubspaceSecret::from_bytes(&[owner_seed; 32]);
    let owner_id = owner_secret.corresponding_subspace_id();
    let owner_cap = WriteCapability::new_owned(&namespace_secret, owner_id);
    OwnedSite {
        root: *namespace_id.as_bytes(),
        namespace_id,
        owner_secret,
        owner_cap,
    }
}

fn owner_sign(site: &OwnedSite, path: &[&[u8]], ts: u64, payload: &[u8]) -> SignedWillowEntry {
    let entry = Entry::builder()
        .namespace_id(site.namespace_id.clone())
        .subspace_id(site.owner_secret.corresponding_subspace_id())
        .path(Path::from_slices(path).expect("path"))
        .timestamp(ts)
        .payload(payload)
        .build();
    let authorised = entry
        .into_authorised_entry(&site.owner_cap, &site.owner_secret)
        .expect("owner cap authorises its own entry");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    SignedWillowEntry {
        entry_bytes: encode_entry(authorised.entry()),
        capability_bytes: encode_capability(token.capability()),
        signature: signature.to_bytes(),
        payload_bytes: payload.to_vec(),
    }
}

fn store() -> (RiotSession, EvidenceStore) {
    let session = RiotSession::open().expect("session");
    let store = session.create_store().expect("store");
    (session, store)
}

#[test]
fn admits_and_commits_an_owner_mod_and_articles_bundle_rooted_at_the_site() {
    let site = owned_site(0x40, 0x04);
    let (_s, store) = store();
    let m = owner_sign(&site, &[MOD_COMPONENT, b"m1"], 100, b"mod-record");
    let a = owner_sign(
        &site,
        &[ARTICLES_COMPONENT, b"news", b"a1"],
        101,
        b"article-body",
    );
    let bundle = encode_bundle(&[m, a]).expect("bundle");

    let imported =
        admit_followed_site_frame(&store, site.root, &bundle, "test-follow").expect("admitted");

    assert_eq!(imported, 2, "both owned records committed");
    assert_eq!(
        store.live_count().expect("live"),
        2,
        "committed into the store"
    );
}

#[test]
fn rejects_a_communal_entry_on_the_followed_site_channel() {
    // A communal entry lives in a communal namespace, never the owned root — so
    // decoding it under Some(owned_root) marks it Invalid and the whole bundle
    // rejects. Only owned records rooted at the site ride this channel.
    let site = owned_site(0x41, 0x05);
    let (_s, store) = store();
    let author = riot_core::willow::generate_communal_author().expect("communal author");
    let draft = riot_core::willow::AlertDraft {
        valid_from: None,
        expires_at: u64::MAX - 1,
        language: "en".into(),
        urgency: riot_core::model::Urgency::Immediate,
        severity: riot_core::model::Severity::Severe,
        certainty: riot_core::model::Certainty::Observed,
        headline: "communal".into(),
        description: "not owned".into(),
        affected_area_claim: None,
        source_claims: vec!["x".into()],
        ai_assisted: false,
    };
    let alert = riot_core::willow::create_signed_alert(&author, draft).expect("alert");
    let bundle = encode_bundle(&[alert.signed]).expect("bundle");

    assert!(
        matches!(
            admit_followed_site_frame(&store, site.root, &bundle, "test-follow"),
            Err(FollowedSiteAdmitError::Rejected)
        ),
        "a communal entry must not ride the owned followed-site channel"
    );
    assert_eq!(store.live_count().expect("live"), 0, "nothing committed");
}

#[test]
fn rejects_an_owned_entry_not_rooted_at_the_passed_root() {
    // An entry from site A, admitted under root B → non-admissible under Some(B).
    let site_a = owned_site(0x42, 0x06);
    let site_b = owned_site(0x43, 0x07);
    let (_s, store) = store();
    let m = owner_sign(&site_a, &[MOD_COMPONENT, b"m1"], 100, b"mod");
    let bundle = encode_bundle(&[m]).expect("bundle");

    assert!(
        matches!(
            admit_followed_site_frame(&store, site_b.root, &bundle, "test-follow"),
            Err(FollowedSiteAdmitError::Rejected)
        ),
        "an entry not rooted at the passed root must be rejected"
    );
    assert_eq!(store.live_count().expect("live"), 0);
}

#[test]
fn rejects_an_empty_bundle() {
    let site = owned_site(0x44, 0x08);
    let (_s, store) = store();
    let bundle = encode_bundle(&[]).expect("empty bundle");

    assert!(matches!(
        admit_followed_site_frame(&store, site.root, &bundle, "test-follow"),
        Err(FollowedSiteAdmitError::Rejected)
    ));
}
