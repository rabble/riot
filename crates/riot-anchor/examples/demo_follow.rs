//! Cross-city demo — FOLLOWER role.
//!
//! Takes the root-signed public-site ticket `demo_host` printed (`TICKET=...`)
//! and pulls the committed site back from the anchor over `riot/sync/2`
//! `ReadCommitted` — the store-and-forward half of the cross-city demo: the
//! host can be offline by now; the anchor serves what was committed.
//!
//! Every pulled item is re-verified CLIENT-SIDE through the same canonical
//! item gate the anchor admits with (`verify_anchor_item`): entry, capability,
//! and Ed25519/Meadowcap signature — the follower never trusts the anchor's
//! bytes blindly.
//!
//! ```sh
//! ANCHOR_ADDR=... cargo run -p riot-anchor --features daemon --example demo_follow -- <ticket-hex>
//! ```
//!
//! (The ticket may also be passed via the `TICKET` environment variable.)

#[path = "../tests/hosting_common/mod.rs"]
mod hosting_common;

mod demo_common;

use std::process::ExitCode;

use riot_anchor::sync_service::verify_anchor_item;
use riot_anchor_protocol::codec::decode_canonical;
use riot_anchor_protocol::records::{RootSignedTicketCoreEnvelopeV2, MAX_TICKET_CORE_BYTES};

use demo_common::{
    anchor_addr_from_env, bind_client_endpoint, drive_sync2, from_hex, stage, to_hex,
};
use hosting_common::pull_initiator;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("demo_follow: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let anchor_addr = anchor_addr_from_env()?;
    let ticket_hex = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("TICKET").ok())
        .ok_or_else(|| {
            "usage: demo_follow <ticket-hex>  (or set TICKET; demo_host prints it)".to_string()
        })?;
    let ticket_bytes = from_hex(ticket_hex.trim())?;

    let envelope = decode_canonical::<RootSignedTicketCoreEnvelopeV2>(
        &ticket_bytes,
        MAX_TICKET_CORE_BYTES + 128,
    )
    .map_err(|error| format!("ticket did not decode: {error:?}"))?;
    let namespaces = [
        envelope.core.o_namespace_id,
        envelope.core.c_namespace_id,
        envelope.core.w_namespace_id,
    ];
    stage("decoded root-signed ticket");
    println!("    site root:        {}", to_hex(&envelope.core.root_id));
    println!(
        "    manifest digest:  {}",
        to_hex(&envelope.core.manifest_digest)
    );
    println!("    manifest version: {}", envelope.core.manifest_version);

    stage("binding client endpoint");
    let client = bind_client_endpoint().await?;

    let labels = ["O (masthead)", "C (comments)", "W (wire)"];
    let mut total_entries = 0usize;
    let mut total_bytes = 0usize;
    for (index, namespace_id) in namespaces.iter().enumerate() {
        stage(&format!(
            "riot/sync/2 pull: ReadCommitted {} {}",
            labels[index],
            to_hex(namespace_id)
        ));
        let (session, admitted) = pull_initiator(*namespace_id, ticket_bytes.clone());
        let session = drive_sync2(&client, anchor_addr.clone(), session).await?;
        if !session.is_complete() {
            return Err(format!(
                "pull of namespace {} refused: {:?}",
                labels[index],
                session.refusal()
            ));
        }
        let items = admitted.borrow().clone();
        for (entry_id, item_bytes) in &items {
            // Client-side re-verification through the canonical item gate.
            verify_anchor_item(item_bytes)
                .map_err(|error| format!("pulled item failed verification: {error:?}"))?;
            println!(
                "    entry {} verified ({} bytes)",
                to_hex(entry_id),
                item_bytes.len()
            );
            total_entries += 1;
            total_bytes += item_bytes.len();
        }
    }

    stage(&format!(
        "FOLLOW DONE — pulled and verified {total_entries} entries ({total_bytes} bytes) across O/C/W"
    ));
    Ok(())
}
