use riot_core::apps::inventory::catalog_inventory;
use riot_core::apps::starter::{CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG};

#[test]
fn inventory_reports_eight_current_and_eight_legacy_entries() {
    let inv = catalog_inventory(CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG);
    assert_eq!(inv.iter().filter(|e| e.membership.current).count(), 8);
    assert_eq!(inv.iter().filter(|e| e.membership.legacy).count(), 8);
}

#[test]
fn inventory_entries_carry_app_id_and_pair_sha256_and_byte_sizes() {
    let inv = catalog_inventory(CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG);
    let e = &inv[0];
    assert_eq!(e.app_id.len(), 32);
    assert_eq!(e.manifest_sha256.len(), 32);
    assert_eq!(e.bundle_sha256.len(), 32);
    assert!(e.manifest_bytes_len > 0 && e.bundle_bytes_len > 0);
}

#[test]
fn until_v2_lands_current_and_legacy_share_membership_per_id() {
    // Same bytes today => each app ID is a member of BOTH catalogs.
    let inv = catalog_inventory(CURRENT_STARTER_CATALOG, LEGACY_BUILTIN_CATALOG);
    assert!(inv
        .iter()
        .all(|e| e.membership.current && e.membership.legacy));
}
