//! WU-003A — the core listing-authority boundary.
//!
//! `O:/directory/listing` is the only listable coordinate. `/directory` is
//! reserved and disjoint from `/manifest`, `/mod`, and `/articles`: an editorial
//! capability rooted under `/articles` can never authorize a listing, and a
//! listing delegation can never reach `/manifest`, `/mod`, or `/articles`. The
//! owner writes listings with the owned zero-delegation capability; a dedicated
//! listing key writes them through a `/directory`-scoped, time-boxed delegation.
//! (The root-signed `ListingDelegateGrantV1` epoch grant and the admission-layer
//! narrowing to exactly `/directory/listing` are canonical/protocol concerns —
//! WU-003B. This unit fixes the path + capability boundary in core.)

use riot_core::willow::{
    is_directory_listing, is_under_articles, is_under_directory, OwnedMasthead, WillowError,
    ARTICLES_COMPONENT, DIRECTORY_COMPONENT, LISTING_COMPONENT, MANIFEST_COMPONENT, MOD_COMPONENT,
};
use willow25::prelude::*;

fn unbounded() -> TimeRange {
    TimeRange::new(0u64.into(), Some(u64::MAX.into()))
}

fn path(parts: &[&[u8]]) -> Path {
    Path::from_slices(parts).expect("path")
}

fn entry_at(
    namespace: &NamespaceId,
    subspace: SubspaceId,
    parts: &[&[u8]],
    timestamp: u64,
) -> Entry {
    Entry::builder()
        .namespace_id(namespace.clone())
        .subspace_id(subspace)
        .path(path(parts))
        .timestamp(timestamp)
        .payload(b"listing-bytes")
        .build()
}

// ---------------------------------------------------------------------------
// Path predicates: /directory is reserved and disjoint from every other region.
// ---------------------------------------------------------------------------

#[test]
fn directory_region_is_disjoint_from_articles_manifest_mod() {
    assert!(is_under_directory(&path(&[
        DIRECTORY_COMPONENT,
        LISTING_COMPONENT
    ])));
    assert!(!is_under_directory(&path(&[ARTICLES_COMPONENT, b"news"])));
    assert!(!is_under_directory(&path(&[MANIFEST_COMPONENT])));
    assert!(!is_under_directory(&path(&[MOD_COMPONENT, b"x"])));
    assert!(!is_under_directory(&path(&[])));

    // And the converse: a listing coordinate is not under /articles.
    assert!(!is_under_articles(&path(&[
        DIRECTORY_COMPONENT,
        LISTING_COMPONENT
    ])));
}

#[test]
fn is_directory_listing_matches_only_the_exact_coordinate() {
    assert!(is_directory_listing(&path(&[
        DIRECTORY_COMPONENT,
        LISTING_COMPONENT
    ])));
    // Arbitrary /directory children are NOT the listing record type.
    assert!(!is_directory_listing(&path(&[
        DIRECTORY_COMPONENT,
        b"other"
    ])));
    // Neither the bare prefix nor a deeper path.
    assert!(!is_directory_listing(&path(&[DIRECTORY_COMPONENT])));
    assert!(!is_directory_listing(&path(&[
        DIRECTORY_COMPONENT,
        LISTING_COMPONENT,
        b"extra"
    ])));
    assert!(!is_directory_listing(&path(&[ARTICLES_COMPONENT, b"news"])));
}

// ---------------------------------------------------------------------------
// Owner zero-delegation capability writes the listing directly (root recovery).
// ---------------------------------------------------------------------------

#[test]
fn owner_zero_delegation_cap_authorizes_the_listing() {
    let m = OwnedMasthead::generate().unwrap();
    let cap = m.owner_write_capability();
    assert!(cap.is_owned() && cap.delegations().is_empty());

    let entry = entry_at(
        m.namespace_id(),
        m.owner_subspace_id(),
        &[DIRECTORY_COMPONENT, LISTING_COMPONENT],
        1_000,
    );
    assert!(
        m.authorise_owner_entry(entry).is_ok(),
        "owner authorises O:/directory/listing"
    );
}

// ---------------------------------------------------------------------------
// A dedicated /directory-scoped listing delegation: what it can and cannot do.
// ---------------------------------------------------------------------------

fn listing_delegate(m: &OwnedMasthead, key: &SubspaceSecret, time: TimeRange) -> WriteCapability {
    let area = Area::new(
        Some(key.corresponding_subspace_id()),
        path(&[DIRECTORY_COMPONENT]),
        time,
    );
    m.delegate_listing(key.corresponding_subspace_id(), area)
        .expect("delegate listing under /directory")
}

#[test]
fn listing_delegate_can_write_the_listing_but_not_other_regions() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[21u8; 32]);
    let id = key.corresponding_subspace_id();
    let cap = listing_delegate(&m, &key, unbounded());
    assert_eq!(cap.receiver(), &id);
    assert_eq!(cap.granted_namespace(), m.namespace_id());

    // POSITIVE: /directory/listing authorises.
    let good = entry_at(
        m.namespace_id(),
        id.clone(),
        &[DIRECTORY_COMPONENT, LISTING_COMPONENT],
        5,
    );
    assert!(
        good.into_authorised_entry(&cap, &key).is_ok(),
        "listing delegate authorises O:/directory/listing"
    );

    // NEGATIVE: the same cap cannot reach /manifest, /mod, or /articles.
    for escaped in [
        vec![MANIFEST_COMPONENT],
        vec![MOD_COMPONENT, b"revoke"],
        vec![ARTICLES_COMPONENT, b"news"],
    ] {
        let bad = entry_at(m.namespace_id(), id.clone(), &escaped, 5);
        assert!(
            bad.into_authorised_entry(&cap, &key).is_err(),
            "listing delegate must NOT authorise {escaped:?}"
        );
    }
}

#[test]
fn editorial_cap_cannot_authorize_a_listing() {
    let m = OwnedMasthead::generate().unwrap();
    let editor = SubspaceSecret::from_bytes(&[7u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let area = Area::new(
        Some(editor_id.clone()),
        path(&[ARTICLES_COMPONENT, b"news"]),
        unbounded(),
    );
    let editorial = m
        .delegate_section(editor_id.clone(), area)
        .expect("delegate section");

    let listing = entry_at(
        m.namespace_id(),
        editor_id,
        &[DIRECTORY_COMPONENT, LISTING_COMPONENT],
        5,
    );
    assert!(
        listing.into_authorised_entry(&editorial, &editor).is_err(),
        "an /articles editorial cap must never authorise O:/directory/listing"
    );
}

#[test]
fn listing_delegation_escaping_directory_is_refused() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[9u8; 32]);
    let id = key.corresponding_subspace_id();
    // An area whose path is /articles (not under /directory) must be refused by
    // the belt check, independent of what Meadowcap would allow.
    let bad_area = Area::new(Some(id.clone()), path(&[ARTICLES_COMPONENT]), unbounded());
    assert!(
        matches!(
            m.delegate_listing(id, bad_area),
            Err(WillowError::DelegationAreaEscapesDirectory)
        ),
        "a listing delegation whose area escapes /directory must be refused"
    );
}

#[test]
fn listing_delegate_receiver_mismatch_fails() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[21u8; 32]);
    let cap = listing_delegate(&m, &key, unbounded());

    // A different subspace signs a listing: the entry subspace is not the cap's
    // receiver, so authorisation fails.
    let other = SubspaceSecret::from_bytes(&[99u8; 32]);
    let other_id = other.corresponding_subspace_id();
    let entry = entry_at(
        m.namespace_id(),
        other_id,
        &[DIRECTORY_COMPONENT, LISTING_COMPONENT],
        5,
    );
    assert!(
        entry.into_authorised_entry(&cap, &other).is_err(),
        "a listing signed by a non-receiver subspace must not authorise"
    );
}

#[test]
fn listing_delegate_time_escape_fails() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[21u8; 32]);
    let id = key.corresponding_subspace_id();
    // Bounded window [100, 200); an entry timestamped 1000 escapes it.
    let cap = listing_delegate(&m, &key, TimeRange::new(100u64.into(), Some(200u64.into())));
    let entry = entry_at(
        m.namespace_id(),
        id,
        &[DIRECTORY_COMPONENT, LISTING_COMPONENT],
        1_000,
    );
    assert!(
        entry.into_authorised_entry(&cap, &key).is_err(),
        "a listing timestamped outside the delegation window must not authorise"
    );
}
