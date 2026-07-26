//! riot-follow — sync a Riot namespace from a seed, given a follow ticket.
//!
//! Verifies the root-signed ticket (fail-closed) BEFORE dialing, then selects a
//! transport (Tor when the ticket attests an onion and this build supports it;
//! otherwise iroh), reconciles, and admits the received entries through the real
//! preview→commit boundary. Prints how many landed.
//!
//! Usage: riot-follow 'riot://site/v1/<ns>?root=...&require=none&...&node=<id>&sig=...'
//!
//! Build with `--features arti` to enable Tor dialing for tickets that carry an
//! attested `onion=` address or that `require=arti`.

use riot_core::session::{ImportContext, RiotSession};
use riot_core::sync::ByteSyncSession as SyncSession;
use riot_transport::iroh::{addr_from_hint, bind, dial_with_ticket};
use riot_transport::select::{select_transport, TransportChoice};
use riot_transport::ticket::{admit_dial, parse, Capabilities, Ticket, TransportBlocked};

// The Tor path is only available when this crate is built with `arti`.
#[cfg(feature = "arti")]
use riot_transport::arti::arti_impl::bootstrap_tor;
#[cfg(feature = "arti")]
use riot_transport::arti::TorDialer;

/// The client's transport capabilities. With the `arti` feature, the client can
/// provide Tor; without it, iroh only.
fn capabilities() -> Capabilities {
    Capabilities {
        iroh: true,
        #[cfg(feature = "arti")]
        arti: true,
        #[cfg(not(feature = "arti"))]
        arti: false,
    }
}

/// Authenticate and freshness-check a ticket before consulting any field that
/// can choose or initialize a network transport.
fn admitted_transport(
    ticket: &Ticket,
    caps: &Capabilities,
    now_unix: u64,
    durable_epoch_floor: u64,
) -> Result<TransportChoice, TransportBlocked> {
    admit_dial(ticket, caps, now_unix, durable_epoch_floor)?;
    Ok(select_transport(ticket, caps))
}

fn current_unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before the Unix epoch")
        .as_secs()
}

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

    let caps = capabilities();
    let now_unix = current_unix_time();
    let transport = admitted_transport(&ticket, &caps, now_unix, 0).unwrap_or_else(|blocked| {
        eprintln!("dial refused (fail-closed): {blocked}");
        std::process::exit(1);
    });
    match transport {
        #[cfg(feature = "arti")]
        TransportChoice::Tor => sync_over_tor(&ticket).await,
        #[cfg(not(feature = "arti"))]
        TransportChoice::Tor => {
            // select_transport only returns Tor when caps.arti is true, which is
            // never the case without the `arti` feature. Defensive: if a future
            // change makes this reachable, fail closed rather than dial iroh.
            eprintln!("ticket selects Tor, but this build has no Tor support");
            std::process::exit(2);
        }
        TransportChoice::Iroh => sync_over_iroh(&ticket, &caps, now_unix).await,
        TransportChoice::Neither => {
            eprintln!("ticket's transport floor is not satisfiable");
            std::process::exit(2);
        }
    }
}

/// Dial the seed over iroh (the default fast path). Used when the ticket has no
/// attested onion, or when this build lacks the `arti` feature.
async fn sync_over_iroh(
    ticket: &riot_transport::ticket::Ticket,
    caps: &Capabilities,
    now_unix: u64,
) {
    let node_hint = ticket.node.clone().unwrap_or_else(|| {
        eprintln!("ticket has no node hint to dial");
        std::process::exit(2);
    });
    let peer = addr_from_hint(&node_hint).expect("peer addr from hint");
    let endpoint = bind().await.expect("bind follower");
    let session = SyncSession::new(ticket.namespace, vec![]).expect("session");

    let store_session = RiotSession::open().expect("session");
    let store = store_session.create_store().expect("store");

    let result = dial_with_ticket(
        &endpoint,
        ticket,
        caps,
        now_unix,
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

    finish(result, &store);
}

/// Dial the seed over Tor, using the ticket's attested onion address. Only
/// available when this crate is built with the `arti` feature.
#[cfg(feature = "arti")]
async fn sync_over_tor(ticket: &riot_transport::ticket::Ticket) {
    let onion = ticket.onion.clone().unwrap_or_else(|| {
        // select_transport picked Tor but there's no onion: this only happens
        // for require:arti tickets with no attested address — a malformed ticket.
        eprintln!("ticket requires Tor but carries no attested onion address");
        std::process::exit(2);
    });

    let store_session = RiotSession::open().expect("session");
    let store = store_session.create_store().expect("store");

    eprintln!("bootstrapping Tor (this can take tens of seconds)…");
    let tor = bootstrap_tor().await.unwrap_or_else(|e| {
        eprintln!("tor bootstrap failed: {e}");
        std::process::exit(1);
    });
    eprintln!("Tor ready; dialing {onion}");

    let dialer = TorDialer::new(tor, onion);
    let session = SyncSession::new(ticket.namespace, vec![]).expect("session");
    let result = riot_transport::run_dial(dialer, session, true, |bytes| {
        store
            .inspect(bytes, ImportContext::new("tor-follow"))
            .expect("inspect")
            .expect_preview()
            .plan_all()
            .expect("plan")
            .commit()
            .expect("commit");
        true
    })
    .await;

    match result {
        Ok(_) => {
            let n = store.live_count().unwrap_or(0);
            println!("synced over Tor — {n} entries now live in the local store");
        }
        Err(e) => {
            eprintln!("sync refused/failed: {e}");
            std::process::exit(1);
        }
    }
}

/// Common result printing + exit for both transports.
fn finish(
    result: Result<SyncSession, riot_transport::TransportError>,
    store: &riot_core::session::EvidenceStore,
) {
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

#[cfg(test)]
mod tests {
    use super::admitted_transport;
    use riot_transport::select::TransportChoice;
    use riot_transport::ticket::{mint, Capabilities, TransportBlocked};

    #[test]
    fn expired_ticket_is_refused_before_transport_selection() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let ticket = mint(
            &key,
            [0x11; 32],
            "none",
            1,
            100,
            [0x22; 32],
            Some("attacker-controlled-iroh-hint".into()),
            None,
            Some("abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijk".into()),
        );
        let caps = Capabilities {
            iroh: true,
            arti: true,
        };

        assert_eq!(
            admitted_transport(&ticket, &caps, 101, 0),
            Err(TransportBlocked::Expired)
        );
        assert_eq!(
            riot_transport::select::select_transport(&ticket, &caps),
            TransportChoice::Tor,
            "without the admission wrapper this ticket would select a network transport"
        );
    }
}
