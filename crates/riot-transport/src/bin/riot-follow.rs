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

use riot_core::session::{EvidenceStore, RiotSession};
use riot_core::site::admit_followed_site_frame;
use riot_core::sync::ByteSyncSession as SyncSession;
use riot_transport::iroh::{addr_from_hint, bind, dial_with_ticket};
use riot_transport::select::{select_transport, TransportChoice};
use riot_transport::ticket::{admit_dial, parse, Capabilities, Ticket, TransportBlocked};
#[cfg(any(feature = "arti", test))]
use riot_transport::Dialer;
use riot_transport::TransportError;

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

#[derive(Debug)]
enum FollowError {
    Blocked(TransportBlocked),
    InvalidInput(String),
    Transport(TransportError),
    Store(String),
}

impl std::fmt::Display for FollowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocked(error) => write!(f, "dial refused (fail-closed): {error}"),
            Self::InvalidInput(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
            Self::Store(error) => write!(f, "local store: {error}"),
        }
    }
}

impl From<TransportError> for FollowError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

async fn dispatch_transport<Tor, TorFuture, Iroh, IrohFuture>(
    ticket: &Ticket,
    caps: &Capabilities,
    now_unix: u64,
    tor: Tor,
    iroh: Iroh,
) -> Result<(TransportChoice, usize), FollowError>
where
    Tor: FnOnce(Ticket) -> TorFuture,
    TorFuture: std::future::Future<Output = Result<usize, FollowError>>,
    Iroh: FnOnce(Ticket) -> IrohFuture,
    IrohFuture: std::future::Future<Output = Result<usize, FollowError>>,
{
    let transport = admitted_transport(ticket, caps, now_unix, 0).map_err(FollowError::Blocked)?;
    let live_count = match transport {
        TransportChoice::Tor => tor(ticket.clone()).await?,
        TransportChoice::Iroh => iroh(ticket.clone()).await?,
        TransportChoice::Neither => {
            return Err(FollowError::InvalidInput(
                "ticket's transport floor is not satisfiable".into(),
            ));
        }
    };
    Ok((transport, live_count))
}

async fn follow<Tor, TorFuture, Iroh, IrohFuture>(
    uri: &str,
    caps: &Capabilities,
    now_unix: u64,
    tor: Tor,
    iroh: Iroh,
) -> Result<(TransportChoice, usize), FollowError>
where
    Tor: FnOnce(Ticket) -> TorFuture,
    TorFuture: std::future::Future<Output = Result<usize, FollowError>>,
    Iroh: FnOnce(Ticket) -> IrohFuture,
    IrohFuture: std::future::Future<Output = Result<usize, FollowError>>,
{
    let ticket =
        parse(uri).map_err(|error| FollowError::InvalidInput(format!("bad ticket: {error:?}")))?;
    dispatch_transport(&ticket, caps, now_unix, tor, iroh).await
}

fn success_message(transport: TransportChoice, live_count: usize) -> Option<String> {
    match transport {
        TransportChoice::Tor => Some(format!(
            "synced over Tor — {live_count} entries now live in the local store"
        )),
        TransportChoice::Iroh => Some(format!(
            "synced — {live_count} entries now live in the local store"
        )),
        TransportChoice::Neither => None,
    }
}

#[tokio::main]
async fn main() {
    let uri = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: riot-follow '<riot://site/v1/... ticket>'");
        std::process::exit(2);
    });
    let caps = capabilities();
    let now_unix = current_unix_time();
    let iroh_caps = caps;
    let result = follow(&uri, &caps, now_unix, sync_over_tor, move |ticket| {
        sync_over_iroh(ticket, iroh_caps, now_unix)
    })
    .await;
    match result {
        Ok((transport, live_count)) => {
            println!(
                "{}",
                success_message(transport, live_count)
                    .expect("Neither is refused before a successful dispatch")
            );
        }
        Err(error) => {
            eprintln!("sync refused/failed: {error}");
            std::process::exit(1);
        }
    }
}

/// Dial the seed over iroh (the default fast path). Used when the ticket has no
/// attested onion, or when this build lacks the `arti` feature.
async fn sync_over_iroh(
    ticket: Ticket,
    caps: Capabilities,
    now_unix: u64,
) -> Result<usize, FollowError> {
    let node_hint = ticket
        .node
        .clone()
        .ok_or_else(|| FollowError::InvalidInput("ticket has no node hint to dial".into()))?;
    let peer = addr_from_hint(&node_hint)
        .map_err(|error| FollowError::InvalidInput(format!("invalid node hint: {error}")))?;
    let endpoint = bind().await?;
    let (store, session) = open_follow_state(&ticket)?;
    let root = ticket.namespace;

    let result = dial_with_ticket(
        &endpoint,
        &ticket,
        &caps,
        now_unix,
        0,
        peer,
        session,
        |bytes| import_bundle(&store, root, bytes, "iroh-follow"),
    )
    .await;

    result?;
    store
        .live_count()
        .map_err(|error| FollowError::Store(format!("{error:?}")))
}

#[cfg(any(feature = "arti", test))]
async fn sync_with_dialer<D>(
    ticket: &Ticket,
    dialer: D,
    import_source: &'static str,
) -> Result<usize, FollowError>
where
    D: Dialer + Unpin,
{
    let (store, session) = open_follow_state(ticket)?;
    let root = ticket.namespace;
    riot_transport::run_dial(dialer, session, true, |bytes| {
        import_bundle(&store, root, bytes, import_source)
    })
    .await?;
    store
        .live_count()
        .map_err(|error| FollowError::Store(format!("{error:?}")))
}

fn open_follow_state(ticket: &Ticket) -> Result<(EvidenceStore, SyncSession), FollowError> {
    let store_session =
        RiotSession::open().map_err(|error| FollowError::Store(format!("{error:?}")))?;
    let store = store_session
        .create_store()
        .map_err(|error| FollowError::Store(format!("{error:?}")))?;
    let session = SyncSession::new(ticket.namespace, vec![])
        .map_err(|error| FollowError::Store(format!("{error:?}")))?;
    Ok((store, session))
}

fn import_bundle(
    store: &EvidenceStore,
    root: [u8; 32],
    bytes: &[u8],
    source: &'static str,
) -> bool {
    admit_followed_site_frame(store, root, bytes, source).is_ok()
}

/// Dial the seed over Tor, using the ticket's attested onion address. Only
/// available when this crate is built with the `arti` feature.
#[cfg(feature = "arti")]
async fn sync_over_tor(ticket: Ticket) -> Result<usize, FollowError> {
    let onion = ticket.onion.clone().ok_or_else(|| {
        FollowError::InvalidInput(
            "ticket requires Tor but carries no attested onion address".into(),
        )
    })?;
    eprintln!("bootstrapping Tor (this can take tens of seconds)…");
    let tor = bootstrap_tor().await?;
    eprintln!("Tor ready; dialing {onion}");
    let dialer = TorDialer::new(tor, onion);
    sync_with_dialer(&ticket, dialer, "tor-follow").await
}

#[cfg(not(feature = "arti"))]
async fn sync_over_tor(_ticket: Ticket) -> Result<usize, FollowError> {
    Err(FollowError::InvalidInput(
        "ticket selects Tor, but this build has no Tor support".into(),
    ))
}

#[cfg(test)]
mod tests {
    use std::future::{ready, Ready};
    use std::sync::{Arc, Mutex};

    use super::{
        admitted_transport, capabilities, current_unix_time, dispatch_transport, follow,
        success_message, sync_over_iroh, sync_over_tor, sync_with_dialer, FollowError,
    };
    use riot_core::sync::ByteSyncSession;
    use riot_core::willow::site_paths::MOD_COMPONENT;
    use riot_core::willow::{
        encode_capability, encode_entry, Entry, OwnedMasthead, Path, SignedWillowEntry,
    };
    use riot_transport::router::{BoxRead, BoxWrite};
    use riot_transport::select::TransportChoice;
    use riot_transport::ticket::{mint, Capabilities, Ticket, TransportBlocked};
    use riot_transport::{Dialer, TransportError};

    fn ticket(namespace: [u8; 32], require: &str, onion: Option<&str>) -> Ticket {
        let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        mint(
            &key,
            namespace,
            require,
            1,
            u64::MAX,
            [0x22; 32],
            Some("unused-iroh-hint".into()),
            None,
            onion.map(str::to_owned),
        )
    }

    #[derive(Default)]
    struct LoopbackDialer {
        stream: Option<(BoxWrite, BoxRead)>,
    }

    impl Dialer for LoopbackDialer {
        async fn connect(&mut self) -> Result<(BoxWrite, BoxRead), TransportError> {
            self.stream.take().ok_or(TransportError::StreamClosed)
        }
    }

    fn counting_dial(
        calls: Arc<Mutex<usize>>,
        live_count: usize,
    ) -> impl FnOnce(Ticket) -> Ready<Result<usize, FollowError>> {
        move |_| {
            *calls.lock().unwrap() += 1;
            ready(Ok(live_count))
        }
    }

    fn signed(masthead: &OwnedMasthead, object: u8) -> SignedWillowEntry {
        let payload = format!("follow CLI fixture {object}").into_bytes();
        let entry = Entry::builder()
            .namespace_id(masthead.namespace_id().clone())
            .subspace_id(masthead.owner_subspace_id())
            .path(Path::from_slices(&[MOD_COMPONENT, &[object]]).expect("path"))
            .timestamp(u64::from(object))
            .payload(&payload)
            .build();
        let authorised = masthead
            .authorise_owner_entry(entry)
            .expect("owner authorises");
        let token = authorised.authorisation_token();
        let signature: ed25519_dalek::Signature = token.signature().clone().into();
        SignedWillowEntry {
            entry_bytes: encode_entry(authorised.entry()),
            capability_bytes: encode_capability(token.capability()),
            signature: signature.to_bytes(),
            payload_bytes: payload,
        }
    }

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

    #[tokio::test]
    async fn dispatches_only_the_admitted_transport() {
        let tor_calls = Arc::new(Mutex::new(0));
        let iroh_calls = Arc::new(Mutex::new(0));
        let tor_ticket = ticket(
            [0x11; 32],
            "none",
            Some("abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijk"),
        );
        let result = dispatch_transport(
            &tor_ticket,
            &Capabilities {
                iroh: true,
                arti: true,
            },
            1,
            counting_dial(tor_calls.clone(), 7),
            counting_dial(iroh_calls.clone(), 99),
        )
        .await
        .unwrap();
        assert_eq!(result, (TransportChoice::Tor, 7));
        assert_eq!(*tor_calls.lock().unwrap(), 1);
        assert_eq!(*iroh_calls.lock().unwrap(), 0);

        let tor_calls = Arc::new(Mutex::new(0));
        let iroh_calls = Arc::new(Mutex::new(0));
        let iroh_ticket = ticket([0x11; 32], "none", None);
        let result = dispatch_transport(
            &iroh_ticket,
            &Capabilities {
                iroh: true,
                arti: true,
            },
            1,
            counting_dial(tor_calls.clone(), 99),
            counting_dial(iroh_calls.clone(), 3),
        )
        .await
        .unwrap();
        assert_eq!(result, (TransportChoice::Iroh, 3));
        assert_eq!(*tor_calls.lock().unwrap(), 0);
        assert_eq!(*iroh_calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn generic_dial_path_imports_the_received_bundle() {
        let masthead = OwnedMasthead::generate().expect("masthead");
        let namespace = *masthead.namespace_id().as_bytes();
        let ticket = ticket(namespace, "none", None);
        let seed =
            ByteSyncSession::new(namespace, vec![signed(&masthead, 1), signed(&masthead, 2)])
                .unwrap();

        let (follower_stream, seed_stream) = tokio::io::duplex(1 << 16);
        let (follower_read, follower_write) = tokio::io::split(follower_stream);
        let (mut seed_read, mut seed_write) = tokio::io::split(seed_stream);
        let dialer = LoopbackDialer {
            stream: Some((Box::pin(follower_write), Box::pin(follower_read))),
        };

        let follower = sync_with_dialer(&ticket, dialer, "test-follow");
        let responder =
            riot_transport::pump(seed, &mut seed_write, &mut seed_read, false, |_| true);
        let (live_count, responder) = tokio::join!(follower, responder);

        assert_eq!(live_count.unwrap(), 2);
        assert!(responder.unwrap().is_terminal());
    }

    #[test]
    fn build_capabilities_and_clock_are_real() {
        assert!(capabilities().iroh);
        assert_eq!(capabilities().arti, cfg!(feature = "arti"));
        assert!(current_unix_time() > 1_700_000_000);
    }

    #[tokio::test]
    async fn missing_transport_targets_fail_before_network_initialization() {
        let no_node = ticket([0x11; 32], "none", None);
        let error = sync_over_iroh(
            Ticket {
                node: None,
                ..no_node
            },
            Capabilities {
                iroh: true,
                arti: false,
            },
            1,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("no node hint"));

        let invalid_node = ticket([0x11; 32], "none", None);
        let error = sync_over_iroh(
            invalid_node,
            Capabilities {
                iroh: true,
                arti: false,
            },
            1,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("invalid node hint"));

        let no_onion = ticket([0x11; 32], "arti", None);
        let error = sync_over_tor(no_onion).await.unwrap_err();
        #[cfg(feature = "arti")]
        assert!(error.to_string().contains("no attested onion"));
        #[cfg(not(feature = "arti"))]
        assert!(error.to_string().contains("no Tor support"));
    }

    #[tokio::test]
    async fn valid_iroh_target_reaches_bounded_network_initialization() {
        let peer_key = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32])
            .verifying_key()
            .to_bytes();
        let peer_hex = peer_key
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut target = ticket([0x11; 32], "none", None);
        target.node = Some(format!("{peer_hex}@127.0.0.1:9"));

        let outcome = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            sync_over_iroh(
                target,
                Capabilities {
                    iroh: true,
                    arti: false,
                },
                1,
            ),
        )
        .await;
        match outcome {
            Err(_) | Ok(Err(FollowError::Transport(_))) => {}
            Ok(other) => panic!("unreachable peer unexpectedly completed: {other:?}"),
        }
    }

    #[test]
    fn follow_errors_preserve_refusal_transport_and_store_context() {
        assert!(FollowError::Blocked(TransportBlocked::Expired)
            .to_string()
            .contains("fail-closed"));
        assert_eq!(
            FollowError::InvalidInput("bad input".into()).to_string(),
            "bad input"
        );
        assert!(FollowError::from(TransportError::StreamClosed)
            .to_string()
            .contains("closed"));
        assert_eq!(
            FollowError::Store("unavailable".into()).to_string(),
            "local store: unavailable"
        );
    }

    #[tokio::test]
    async fn follow_parses_dispatches_and_preserves_fail_closed_errors() {
        let invalid_calls = Arc::new(Mutex::new(0));
        let invalid = follow(
            "not-a-ticket",
            &Capabilities {
                iroh: true,
                arti: true,
            },
            1,
            counting_dial(invalid_calls.clone(), 99),
            counting_dial(invalid_calls.clone(), 99),
        )
        .await
        .unwrap_err();
        assert!(invalid.to_string().contains("bad ticket"));
        assert_eq!(*invalid_calls.lock().unwrap(), 0);

        let tor_ticket = ticket(
            [0x11; 32],
            "none",
            Some("abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijk"),
        );
        let tor_calls = Arc::new(Mutex::new(0));
        let result = follow(
            &tor_ticket.encode(),
            &Capabilities {
                iroh: true,
                arti: true,
            },
            1,
            counting_dial(tor_calls.clone(), 9),
            counting_dial(tor_calls.clone(), 99),
        )
        .await
        .unwrap();
        assert_eq!(result, (TransportChoice::Tor, 9));
        assert_eq!(*tor_calls.lock().unwrap(), 1);

        let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let expired = mint(
            &key,
            [0x11; 32],
            "none",
            1,
            100,
            [0x22; 32],
            Some("unused-iroh-hint".into()),
            None,
            None,
        );
        let expired_calls = Arc::new(Mutex::new(0));
        let error = follow(
            &expired.encode(),
            &Capabilities {
                iroh: true,
                arti: true,
            },
            101,
            counting_dial(expired_calls.clone(), 99),
            counting_dial(expired_calls.clone(), 99),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("fail-closed"));
        assert_eq!(*expired_calls.lock().unwrap(), 0);
    }

    #[test]
    fn success_messages_name_the_selected_transport_and_count() {
        assert_eq!(
            success_message(TransportChoice::Tor, 2),
            Some("synced over Tor — 2 entries now live in the local store".into())
        );
        assert_eq!(
            success_message(TransportChoice::Iroh, 3),
            Some("synced — 3 entries now live in the local store".into())
        );
        assert_eq!(success_message(TransportChoice::Neither, 4), None);
    }
}
