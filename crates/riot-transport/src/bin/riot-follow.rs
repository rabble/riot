//! riot-follow — sync a Riot namespace from a seed, given a follow ticket.
//!
//! Verifies the root-signed ticket (fail-closed) BEFORE dialing, then dials the
//! seed by NodeId (discovery resolves the address), reconciles, and admits the
//! received entries through the real preview→commit boundary. Prints how many
//! landed.
//!
//! Usage: riot-follow 'riot://site/v1/<ns>?root=...&require=none&...&node=<id>&sig=...'

use riot_core::session::{ImportContext, RiotSession};
use riot_core::sync::ByteSyncSession;
use riot_transport::iroh::{addr_from_hint, bind, dial_with_ticket};
use riot_transport::ticket::{parse, Capabilities};

#[tokio::main]
async fn main() {
    let uri = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: riot-follow '<riot://site/v1/... ticket>'");
        std::process::exit(2);
    });
    let ticket = parse(&uri).unwrap_or_else(|e| {
        eprintln!("bad ticket: {e:?}");
        std::process::exit(2);
    });

    let node_hint = ticket.node.clone().unwrap_or_else(|| {
        eprintln!("ticket has no node hint to dial");
        std::process::exit(2);
    });
    let peer = addr_from_hint(&node_hint).expect("peer addr from hint");

    let endpoint = bind().await.expect("bind follower");
    let session = ByteSyncSession::new(ticket.namespace, vec![]).expect("session");

    let store_session = RiotSession::open().expect("session");
    let store = store_session.create_store().expect("store");

    // `now` well within the ticket's dev expiry; durable floor 0 (first follow).
    let result = dial_with_ticket(
        &endpoint,
        &ticket,
        &Capabilities {
            iroh: true,
            arti: false,
        },
        1_000_000,
        0,
        peer,
        session,
        |bytes| {
            store
                .inspect(bytes, ImportContext::new("iroh-follow"))
                .expect("inspect")
                .expect_preview()
                .plan_all()
                .expect("plan")
                .commit()
                .expect("commit");
            true
        },
    )
    .await;

    match result {
        Ok(_) => {
            let n = store.live_count().unwrap_or(0);
            println!("synced — {n} entries now live in the local store");
        }
        Err(e) => {
            eprintln!("sync refused/failed: {e}");
            std::process::exit(1);
        }
    }
}
