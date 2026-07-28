//! Seed site-state durability and helpers, no network: `generate_demo_site`
//! builds a verifiable inventory, `save`/`load` round-trips byte-exactly,
//! truncated or missing state fails closed, and a minted follow ticket
//! verifies against the site root key.

use riot_transport::seed::{generate_demo_site, hex, rand32, SiteState};
use riot_transport::ticket;

#[test]
fn demo_site_has_requested_inventory_and_matching_namespace() {
    let site = generate_demo_site(3);
    assert_eq!(site.inventory.len(), 3);
    assert_ne!(site.namespace, [0u8; 32]);
    for entry in &site.inventory {
        assert!(!entry.entry_bytes.is_empty());
        assert!(!entry.capability_bytes.is_empty());
        assert!(!entry.payload_bytes.is_empty());
        assert_ne!(entry.signature, [0u8; 64]);
    }
}

#[test]
fn demo_site_ticket_verifies_against_root_key() {
    let site = generate_demo_site(1);
    let t = site.ticket("nodehint".to_string(), 1, 2);
    assert!(t.verify());
    let round_tripped = ticket::parse(&t.encode()).expect("encode then parse");
    assert!(round_tripped.verify());
}

#[test]
fn save_load_round_trip_is_byte_exact() {
    let site = generate_demo_site(4);
    let path = std::env::temp_dir().join(format!(
        "riot-seed-state-roundtrip-{}-{}.bin",
        std::process::id(),
        hex(&rand32()[..4])
    ));
    site.save(&path).expect("save");
    let loaded = SiteState::load(&path).expect("load");
    assert_eq!(loaded.root_key, site.root_key);
    assert_eq!(loaded.namespace, site.namespace);
    assert_eq!(loaded.inventory.len(), site.inventory.len());
    for (a, b) in loaded.inventory.iter().zip(site.inventory.iter()) {
        assert_eq!(a.entry_bytes, b.entry_bytes);
        assert_eq!(a.capability_bytes, b.capability_bytes);
        assert_eq!(a.signature, b.signature);
        assert_eq!(a.payload_bytes, b.payload_bytes);
    }
    std::fs::remove_file(&path).ok();
}

#[test]
fn load_rejects_truncated_state() {
    let site = generate_demo_site(2);
    let path = std::env::temp_dir().join(format!(
        "riot-seed-state-truncated-{}-{}.bin",
        std::process::id(),
        hex(&rand32()[..4])
    ));
    site.save(&path).expect("save");
    let full = std::fs::read(&path).expect("read");
    // Every proper prefix of a multi-entry state must fail, never partially
    // load: the cursor hits a truncated field no matter where the cut lands.
    for cut in [0, 10, 63, 64, 67, full.len() / 2, full.len() - 1] {
        std::fs::write(&path, &full[..cut]).expect("write truncated");
        let result = SiteState::load(&path);
        assert!(result.is_err(), "prefix of {cut} bytes must not load");
    }
    std::fs::remove_file(&path).ok();
}

#[test]
fn load_missing_file_is_an_error() {
    let path = std::env::temp_dir().join(format!(
        "riot-seed-state-missing-{}-{}.bin",
        std::process::id(),
        hex(&rand32()[..4])
    ));
    assert!(SiteState::load(&path).is_err());
}

#[test]
fn empty_inventory_round_trips() {
    let site = SiteState {
        root_key: rand32(),
        namespace: rand32(),
        inventory: Vec::new(),
    };
    let path = std::env::temp_dir().join(format!(
        "riot-seed-state-empty-{}-{}.bin",
        std::process::id(),
        hex(&rand32()[..4])
    ));
    site.save(&path).expect("save");
    let loaded = SiteState::load(&path).expect("load");
    assert!(loaded.inventory.is_empty());
    assert_eq!(loaded.root_key, site.root_key);
    assert_eq!(loaded.namespace, site.namespace);
    std::fs::remove_file(&path).ok();
}

#[test]
fn hex_formats_lowercase_pairs() {
    assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    assert_eq!(hex(&[]), "");
}

#[test]
fn rand32_draws_are_not_constant() {
    let a = rand32();
    let b = rand32();
    assert_ne!(a, [0u8; 32]);
    assert_ne!(a, b);
}
