//! DIAGNOSTIC (not shipped): reproduce the TestFlight 0.1.1 (1020) launch
//! crash — a Rust double-panic abort inside `MobileNetRuntime::sync_with_anchor`
//! when dialing the LIVE deployed relay with the baked ticket, exactly as
//! `RiotAppModel.syncFromRelay()` does at app open.
//!
//! Run: `cargo test -p riot-ffi --features net --test net_live_relay -- --ignored --nocapture`
#![cfg(feature = "net")]

use riot_ffi::net::bind_net_runtime;
use riot_ffi::open_local_profile_with_database;

/// The deployed relay's stable NodeId (mirrors `AnchorRelayDefaults.relayNodeId`).
const RELAY_NODE_ID: &str = "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8";

/// The baked community ticket hex (mirrors `AnchorRelayDefaults.communityTicketHex`).
const COMMUNITY_TICKET_HEX: &str = "83028c58207f6c42e7988f6ee2654cf3e1177c614086d54e0dcd9f1905c8460083036472c358207f6c42e7988f6ee2654cf3e1177c614086d54e0dcd9f1905c8460083036472c3582026f1ad8ff8789248f171487257cc5a0a0e6d17f24469ad107377d961f6b78a8a5820452760690dc2b6d0d73c3ce5a1b9985751def04945d3d7d00121cff42e9ef54458204ee5784092f6176e5599d68dd31d7de1d2c2b970f504e0975ac78994f77ebb951a6a62989f026c726571756972655f6e6f6e656c726571756972655f6e6f6e65011a6a62b28f1a6ad8080f5840badb5fa31067a5c330ba16ca97fbedf1ba9201c981c5014175721ccb2af61c83723514116260ef952516d0fcc0b474455b0ac8a4dd3fa39c019c77848fed9e00";

fn hex_decode(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
        .collect()
}

#[test]
#[ignore = "hits the live GCP relay; diagnostic repro only"]
fn live_relay_sync_does_not_crash() {
    let db = std::env::temp_dir().join(format!(
        "riot-live-relay-repro-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let profile = open_local_profile_with_database(db.to_string_lossy().into_owned())
        .expect("phone profile opens");
    let net = bind_net_runtime().expect("net runtime binds");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let outcome = net.sync_with_anchor(
        profile,
        RELAY_NODE_ID.to_string(),
        hex_decode(COMMUNITY_TICKET_HEX),
        now,
    );
    match outcome {
        Ok(outcome) => eprintln!("SYNC OK: {outcome:?}"),
        Err(error) => eprintln!("SYNC ERR (no crash): {error}"),
    }
}
