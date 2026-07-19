//! The core `/articles/` editor-delegation authority boundary.
//!
//! `delegate_section` mints the owned-masthead editor capability: a
//! `/articles`-scoped, receiver-bound, time-boxed write delegation. This suite
//! is the `/articles` mirror of `listing_authority_boundary.rs` (which fixes the
//! `/directory` listing boundary) — it pins the properties that the section
//! delegation MUST enforce and that were previously only partially covered by
//! the inline masthead unit tests:
//!
//!   * the section cap authorises only its own `/articles/<section>` subtree —
//!     not sibling sections, and not `/manifest`, `/mod`, or `/directory`;
//!   * a NON-receiver subspace cannot wield the cap (receiver binding);
//!   * an entry timestamped OUTSIDE the delegation window is refused
//!     (time-box) — the exact property whose FFI wrapper regressed in #76 when
//!     the box was built in the wrong time unit; and
//!   * the belt refuses a delegation whose area escapes `/articles` at mint time.
//!
//! Authorisation is always checked through the real willow25 `into_authorised_entry`
//! path with the receiver's own secret, so these are not tautologies.

use riot_core::willow::{
    is_under_articles, OwnedMasthead, WillowError, ARTICLES_COMPONENT, DIRECTORY_COMPONENT,
    LISTING_COMPONENT, MANIFEST_COMPONENT, MOD_COMPONENT,
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
        .payload(b"article-bytes")
        .build()
}

/// Delegate the `/articles/<section>` editor capability to `key`, time-boxed to `time`.
fn section_delegate(
    m: &OwnedMasthead,
    key: &SubspaceSecret,
    section: &[u8],
    time: TimeRange,
) -> WriteCapability {
    let area = Area::new(
        Some(key.corresponding_subspace_id()),
        path(&[ARTICLES_COMPONENT, section]),
        time,
    );
    m.delegate_section(key.corresponding_subspace_id(), area)
        .expect("delegate section under /articles")
}

// ---------------------------------------------------------------------------
// Path predicate: /articles is a region distinct from every sibling.
// ---------------------------------------------------------------------------

#[test]
fn articles_region_is_disjoint_from_manifest_mod_directory() {
    assert!(is_under_articles(&path(&[ARTICLES_COMPONENT, b"news"])));
    assert!(is_under_articles(&path(&[
        ARTICLES_COMPONENT,
        b"news",
        b"post-1"
    ])));
    assert!(!is_under_articles(&path(&[MANIFEST_COMPONENT])));
    assert!(!is_under_articles(&path(&[MOD_COMPONENT, b"revoke"])));
    assert!(!is_under_articles(&path(&[
        DIRECTORY_COMPONENT,
        LISTING_COMPONENT
    ])));
    // The bare region prefix is not itself "under" /articles as a writable leaf.
    assert!(!is_under_articles(&path(&[])));
}

// ---------------------------------------------------------------------------
// Owner zero-delegation capability writes an article directly (root authority).
// ---------------------------------------------------------------------------

#[test]
fn owner_zero_delegation_cap_authorizes_an_article() {
    let m = OwnedMasthead::generate().unwrap();
    let cap = m.owner_write_capability();
    assert!(cap.is_owned() && cap.delegations().is_empty());

    let entry = entry_at(
        m.namespace_id(),
        m.owner_subspace_id(),
        &[ARTICLES_COMPONENT, b"news", b"post-1"],
        1_000,
    );
    assert!(
        m.authorise_owner_entry(entry).is_ok(),
        "owner authorises an /articles entry directly"
    );
}

// ---------------------------------------------------------------------------
// The section delegation: what it can and cannot reach.
// ---------------------------------------------------------------------------

#[test]
fn section_delegate_can_write_its_section_but_not_other_regions() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[21u8; 32]);
    let id = key.corresponding_subspace_id();
    let cap = section_delegate(&m, &key, b"news", unbounded());
    assert_eq!(cap.receiver(), &id);
    assert_eq!(cap.granted_namespace(), m.namespace_id());

    // POSITIVE: an entry under /articles/news authorises.
    let good = entry_at(
        m.namespace_id(),
        id.clone(),
        &[ARTICLES_COMPONENT, b"news", b"post-1"],
        5,
    );
    assert!(
        good.into_authorised_entry(&cap, &key).is_ok(),
        "section delegate authorises its own /articles/news subtree"
    );

    // NEGATIVE: the same cap cannot reach /manifest, /mod, or /directory.
    for escaped in [
        vec![MANIFEST_COMPONENT],
        vec![MOD_COMPONENT, b"revoke"],
        vec![DIRECTORY_COMPONENT, LISTING_COMPONENT],
    ] {
        let bad = entry_at(m.namespace_id(), id.clone(), &escaped, 5);
        assert!(
            bad.into_authorised_entry(&cap, &key).is_err(),
            "section delegate must NOT authorise {escaped:?}"
        );
    }
}

#[test]
fn section_delegate_cannot_write_a_sibling_section() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[21u8; 32]);
    let id = key.corresponding_subspace_id();
    // Delegated to /articles/news only.
    let cap = section_delegate(&m, &key, b"news", unbounded());

    // A sibling section /articles/sports is a different path prefix — refused.
    let sibling = entry_at(
        m.namespace_id(),
        id,
        &[ARTICLES_COMPONENT, b"sports", b"post-1"],
        5,
    );
    assert!(
        sibling.into_authorised_entry(&cap, &key).is_err(),
        "a /articles/news cap must NOT authorise /articles/sports"
    );
}

#[test]
fn section_delegate_receiver_mismatch_fails() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[21u8; 32]);
    let cap = section_delegate(&m, &key, b"news", unbounded());

    // A DIFFERENT subspace signs an /articles/news entry: the entry subspace is
    // not the cap's receiver, so authorisation fails (receiver binding).
    let other = SubspaceSecret::from_bytes(&[99u8; 32]);
    let other_id = other.corresponding_subspace_id();
    let entry = entry_at(
        m.namespace_id(),
        other_id,
        &[ARTICLES_COMPONENT, b"news", b"post-1"],
        5,
    );
    assert!(
        entry.into_authorised_entry(&cap, &other).is_err(),
        "an /articles entry signed by a non-receiver subspace must not authorise"
    );
}

#[test]
fn section_delegate_time_escape_fails() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[21u8; 32]);
    let id = key.corresponding_subspace_id();
    // Bounded window [100, 200); an entry timestamped 1000 escapes it. This is
    // the core-level guarantee behind the #76 FFI fix (the cap's TimeRange must
    // be in the same unit as the entry timestamp, or every real entry escapes).
    let cap = section_delegate(
        &m,
        &key,
        b"news",
        TimeRange::new(100u64.into(), Some(200u64.into())),
    );
    let entry = entry_at(
        m.namespace_id(),
        id,
        &[ARTICLES_COMPONENT, b"news", b"post-1"],
        1_000,
    );
    assert!(
        entry.into_authorised_entry(&cap, &key).is_err(),
        "an /articles entry timestamped outside the delegation window must not authorise"
    );
}

#[test]
fn section_delegation_escaping_articles_is_refused() {
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[9u8; 32]);
    let id = key.corresponding_subspace_id();
    // Areas whose path is NOT under /articles must be refused by the belt check
    // at mint time, independent of what Meadowcap would otherwise allow.
    for escaped in [
        vec![MOD_COMPONENT],
        vec![DIRECTORY_COMPONENT],
        vec![MANIFEST_COMPONENT],
    ] {
        let bad_area = Area::new(Some(id.clone()), path(&escaped), unbounded());
        assert!(
            matches!(
                m.delegate_section(id.clone(), bad_area),
                Err(WillowError::DelegationAreaEscapesArticles)
            ),
            "a section delegation whose area escapes /articles must be refused: {escaped:?}"
        );
    }
}

#[test]
fn listing_cap_cannot_authorize_an_article() {
    // The converse of `editorial_cap_cannot_authorize_a_listing`: a /directory
    // listing delegation must never reach /articles.
    let m = OwnedMasthead::generate().unwrap();
    let key = SubspaceSecret::from_bytes(&[7u8; 32]);
    let id = key.corresponding_subspace_id();
    let listing_area = Area::new(Some(id.clone()), path(&[DIRECTORY_COMPONENT]), unbounded());
    let listing_cap = m
        .delegate_listing(id.clone(), listing_area)
        .expect("delegate listing");

    let article = entry_at(
        m.namespace_id(),
        id,
        &[ARTICLES_COMPONENT, b"news", b"post-1"],
        5,
    );
    assert!(
        article.into_authorised_entry(&listing_cap, &key).is_err(),
        "a /directory listing cap must never authorise an /articles entry"
    );
}
