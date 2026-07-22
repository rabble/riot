//! Cross-city demo — HOST role.
//!
//! Mints a real composite site (owned O root with an owner-signed `/manifest`,
//! communal C/W entries, and a root-signed public-site ticket), then drives the
//! full hosting lifecycle against a running anchor:
//!
//! 1. `PrepareHost` over the `riot/anchor/1` control plane,
//! 2. `riot/sync/2` push of all three namespaces (`HostReconcileStaged`),
//! 3. `CommitHost` — a signed hosting receipt comes back.
//!
//! On success it prints `TICKET=<hex>` — hand that string to `demo_follow` on
//! any other machine (the cross-city half) to pull the committed site back.
//!
//! Anchor address: `ANCHOR_ADDR` (a `<node_id_hex>@<ip:port>` hint, printed by
//! `demo_anchor`) or `ANCHOR_NODE_ID` (discovery resolves a public anchor).
//!
//! ```sh
//! ANCHOR_ADDR=... cargo run -p riot-anchor --features daemon --example demo_host
//! ```
//!
//! Fixture minting and the initiator FSMs are the e2e test's own helpers
//! (`tests/hosting_common/mod.rs`), included verbatim so the demo and the test
//! suite can never drift apart.

#[path = "../tests/hosting_common/mod.rs"]
mod hosting_common;

mod demo_common;

use std::process::ExitCode;

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::control::{
    CommitHostV1, ControlOperation, ControlOutcome, ControlRequestV1, ControlSuccess,
    PrepareHostV1, PrepareSuccessV1,
};
use riot_anchor_protocol::records::{RootSignedTicketCoreEnvelopeV2, MAX_TICKET_CORE_BYTES};

use demo_common::{
    anchor_addr_from_env, bind_client_endpoint, control_round_trip, drive_sync2, now_secs,
    random_idempotency_key, random_secret, stage, to_hex,
};
use hosting_common::{client_snapshot_digest, push_initiator, SiteFixture, SyncItem};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("demo_host: {message}");
            ExitCode::FAILURE
        }
    }
}

/// The `(entry_id, item_bytes)` sync items of the site fixture, ordered O/C/W.
fn site_items(site: &SiteFixture) -> [SyncItem; 3] {
    [
        (
            site.manifest_staged.entry_id.to_vec(),
            site.manifest_staged.item_bytes.clone(),
        ),
        (
            site.c_staged.entry_id.to_vec(),
            site.c_staged.item_bytes.clone(),
        ),
        (
            site.w_staged.entry_id.to_vec(),
            site.w_staged.item_bytes.clone(),
        ),
    ]
}

/// A canonical `PrepareHost` frame carrying the site's root-signed ticket.
fn prepare_frame(site: &SiteFixture, idempotency_key: [u8; 16]) -> Result<Vec<u8>, String> {
    let ticket = decode_canonical::<RootSignedTicketCoreEnvelopeV2>(
        &site.ticket_envelope_bytes,
        MAX_TICKET_CORE_BYTES + 128,
    )
    .map_err(|error| format!("site ticket did not decode: {error:?}"))?;
    ControlRequestV1 {
        idempotency_key,
        operation: ControlOperation::PrepareHost(Box::new(PrepareHostV1 {
            root_signed_ticket_core: ticket,
            ordered_namespace_snapshot_digests: [[0u8; 32]; 3],
            work_stamp: None,
        })),
    }
    .encode_canonical()
    .map_err(|error| format!("encode PrepareHost: {error:?}"))
}

/// A canonical `CommitHost` frame.
fn commit_frame(
    operation_id: [u8; 32],
    ordered_namespace_snapshot_digests: [[u8; 32]; 3],
    idempotency_key: [u8; 16],
) -> Result<Vec<u8>, String> {
    ControlRequestV1 {
        idempotency_key,
        operation: ControlOperation::CommitHost(CommitHostV1 {
            operation_id,
            ordered_namespace_snapshot_digests,
        }),
    }
    .encode_canonical()
    .map_err(|error| format!("encode CommitHost: {error:?}"))
}

async fn run() -> Result<(), String> {
    let anchor_addr = anchor_addr_from_env()?;

    stage("binding client endpoint");
    let client = bind_client_endpoint().await?;

    // A fresh random seed each run mints a NEW owned site root, and the
    // wall-clock manifest version stays monotonic — so repeated demo runs
    // against the same anchor database never trip the durable per-root
    // rollback floors.
    let now = now_secs();
    let seed = random_secret()?[0];
    stage("minting a demo composite site (owned O root, communal C/W, root-signed ticket)");
    let site = hosting_common::make_site_fixture(seed, now, now.saturating_sub(100), now + 3600);
    println!("    site root (O):    {}", to_hex(&site.root_id));
    println!("    C namespace:      {}", to_hex(&site.namespaces[1]));
    println!("    W namespace:      {}", to_hex(&site.namespaces[2]));
    println!("    manifest digest:  {}", to_hex(&site.manifest_digest));

    stage("PrepareHost (riot/anchor/1 control plane)");
    let response = control_round_trip(
        &client,
        anchor_addr.clone(),
        prepare_frame(&site, random_idempotency_key()?)?,
    )
    .await?;
    let prepared: PrepareSuccessV1 = match response.outcome {
        ControlOutcome::Success(ControlSuccess::PrepareHost(success)) => *success,
        other => return Err(format!("PrepareHost was not admitted: {other:?}")),
    };
    println!("    operation id:     {}", to_hex(&prepared.operation_id));
    println!("    operation expiry: {} (unix)", prepared.operation_expiry);

    stage("riot/sync/2 push: staging O, C, W (HostReconcileStaged)");
    let items = site_items(&site);
    let labels = ["O (/manifest)", "C", "W"];
    for (index, item) in items.iter().enumerate() {
        let session = push_initiator(
            site.namespaces[index],
            prepared.operation_id,
            prepared.ordered_namespace_tokens[index],
            vec![item.clone()],
        );
        let session = drive_sync2(&client, anchor_addr.clone(), session).await?;
        if !session.is_complete() {
            return Err(format!(
                "push of namespace {} refused: {:?}",
                labels[index],
                session.refusal()
            ));
        }
        println!(
            "    pushed {} — 1 entry, {} bytes",
            labels[index],
            item.1.len()
        );
    }

    stage("CommitHost: promoting the staged site to committed");
    let declared = [
        client_snapshot_digest(&site.namespaces[0], &items[..1]),
        client_snapshot_digest(&site.namespaces[1], &items[1..2]),
        client_snapshot_digest(&site.namespaces[2], &items[2..3]),
    ];
    let committed = control_round_trip(
        &client,
        anchor_addr,
        commit_frame(prepared.operation_id, declared, random_idempotency_key()?)?,
    )
    .await?;
    match committed.outcome {
        ControlOutcome::Success(ControlSuccess::CommitHost(receipt)) => {
            println!(
                "    hosting receipt: site {} manifest {} (operation {})",
                to_hex(&receipt.body.full_site_root),
                to_hex(&receipt.body.manifest_digest),
                to_hex(&receipt.body.hosting_operation_id),
            );
        }
        other => return Err(format!("CommitHost was not admitted: {other:?}")),
    }

    stage("HOST DONE — hand this ticket to demo_follow on the other machine");
    println!("TICKET={}", to_hex(&site.ticket_envelope_bytes));
    Ok(())
}
