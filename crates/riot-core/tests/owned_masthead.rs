//! Integration: an owner generates a masthead, mints its cap, delegates a section
//! editor cap under /articles, signs a manifest entry, and seals/restores the root.
use riot_core::willow::{is_under_articles, OwnedMasthead, ARTICLES_COMPONENT, MANIFEST_COMPONENT};
use willow25::prelude::*;

#[test]
fn owner_lifecycle_mint_delegate_sign_seal_restore() {
    let m = OwnedMasthead::generate().expect("masthead");
    assert!(m.namespace_id().is_owned());

    // owner cap
    let owner_cap = m.owner_write_capability();
    assert!(owner_cap.is_owned() && owner_cap.delegations().is_empty());

    // delegate a Culture-section editor
    let editor = SubspaceSecret::from_bytes(&[3u8; 32]);
    let editor_id = editor.corresponding_subspace_id();
    let area = Area::new(
        Some(editor_id.clone()),
        Path::from_slices(&[ARTICLES_COMPONENT, b"culture"]).expect("path"),
        TimeRange::new(0u64.into(), Some(u64::MAX.into())),
    );
    assert!(is_under_articles(area.path()));
    let editor_cap = m
        .delegate_section(editor_id.clone(), area)
        .expect("delegate");
    assert_eq!(editor_cap.receiver(), &editor_id);

    // sign/verify: owner authorises a /manifest write
    let owner_entry = Entry::builder()
        .namespace_id(m.namespace_id().clone())
        .subspace_id(m.owner_subspace_id())
        .path(Path::from_slices(&[MANIFEST_COMPONENT]).expect("path"))
        .timestamp(1u64)
        .payload(b"manifest-bytes")
        .build();
    assert!(
        m.authorise_owner_entry(owner_entry).is_ok(),
        "owner authorises /manifest"
    );

    // seal + restore
    let key = [0x77; 32];
    let sealed = m.seal(&key).unwrap();
    let restored = OwnedMasthead::open_sealed(&key, &sealed).unwrap();
    assert_eq!(*restored.namespace_id(), *m.namespace_id());
    assert_eq!(restored.owner_subspace_id(), m.owner_subspace_id());
}
