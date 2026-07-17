//! riot-seed — an always-on iroh seed node for a Riot namespace.
//!
//! Persists a stable NodeId (so followers keep finding it) and a site
//! (namespace + signed entries + owner key), reseeding what it holds to any
//! follower over the public internet (N0 relay + discovery, NAT-traversing).
//! Prints a root-signed follow ticket on startup.
//!
//! Env:
//!   RIOT_SEED_DIR   state dir (default ./riot-seed-state)
//!   RIOT_SEED_DEMO  entries to generate on first run (default 3)

use std::path::PathBuf;

use riot_transport::iroh::{bind_public, dialable_addr, endpoint_addr_hint, node_id};
use riot_transport::seed::{generate_demo_site, hex, rand32, run_seed, SiteState};

#[tokio::main]
async fn main() {
    let dir =
        PathBuf::from(std::env::var("RIOT_SEED_DIR").unwrap_or_else(|_| "riot-seed-state".into()));
    std::fs::create_dir_all(&dir).expect("state dir");
    let key_path = dir.join("node.key");
    let site_path = dir.join("site.bin");

    // Stable NodeId across restarts.
    let node_key = load_or_init(&key_path, || rand32().to_vec());
    let node_key: [u8; 32] = node_key.try_into().expect("32-byte node key");

    // Persistent site (namespace + inventory + owner key).
    let site = if site_path.exists() {
        SiteState::load(&site_path).expect("load site")
    } else {
        let n: u8 = std::env::var("RIOT_SEED_DEMO")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);
        let s = generate_demo_site(n);
        s.save(&site_path).expect("save site");
        s
    };

    let endpoint = bind_public(node_key).await.expect("bind public endpoint");
    let id = node_id(&endpoint);

    // Wait for a dialable address, then carry NodeId + direct addrs in the hint
    // (direct dial on LAN/public; discovery-by-id as fallback).
    let addr = dialable_addr(&endpoint).await;
    let hint = endpoint_addr_hint(&addr);
    let exp = 4_000_000_000u64; // long dev-testnet expiry
    let ticket = site.ticket(hint, 1, exp);

    eprintln!("riot-seed up");
    eprintln!("  node id   : {}", hex(&id));
    eprintln!("  namespace : {}", hex(&site.namespace));
    eprintln!("  entries   : {}", site.inventory.len());
    eprintln!("  ticket    : {}", ticket.encode());
    eprintln!("reseeding — followers can now sync this namespace. Ctrl-C to stop.");

    run_seed(&endpoint, &site).await;
}

fn load_or_init(path: &std::path::Path, init: impl FnOnce() -> Vec<u8>) -> Vec<u8> {
    if let Ok(b) = std::fs::read(path) {
        return b;
    }
    let b = init();
    std::fs::write(path, &b).expect("write key");
    b
}
