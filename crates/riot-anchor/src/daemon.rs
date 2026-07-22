//! The runnable anchor daemon (WU-019 increment 1).
//!
//! This module turns the pure control library into a running network server for
//! the control plane. It binds a PUBLIC iroh endpoint, routes the
//! `riot/anchor/1` control ALPN, and serves community hosting admission
//! (`PrepareHost` / `GetOperation` / `Describe` / `GetWorkChallenge`) against the
//! durable [`AnchorRepository`].
//!
//! # Single-writer actor
//!
//! The router's [`Handler`] must be `Send + Sync` and its future `Send`, but the
//! [`AnchorRepository`] is one non-pooled `rusqlite::Connection` and
//! [`AnchorControlService`] holds mutable state (the token-secret ring) — neither
//! may be shared across concurrent handler invocations. So exactly ONE dedicated
//! OS thread (`anchor-single-writer`) owns the repository and the service: it
//! wraps the repository in `Rc<RefCell<...>>` (deliberately `!Send`, so `sync/2`
//! sessions can share the one cell), receives [`ActorJob`]s over an unbounded
//! [`mpsc`](tokio::sync::mpsc) channel via `blocking_recv`, calls
//! [`AnchorControlService::handle`], encodes the [`ControlResponseV1`], and sends
//! the response bytes back over a [`oneshot`](tokio::sync::oneshot) reply. The
//! anchor/1 handler closure holds only the `mpsc::UnboundedSender` (which is
//! `Clone + Send + Sync`), so many concurrent connections funnel through the one
//! writer without ever aliasing the connection or the ring. The thread stops
//! when every sender is dropped; [`serve`] joins it on shutdown so a panic
//! payload surfaces in the daemon's error path.
//!
//! # The `riot/sync/2` DATA path
//!
//! [`serve`] registers [`sync_handler`] on `ALPN_SYNC_V2` next to the control
//! handler. The sync handler is a thin frame shuttle: it owns NO protocol or
//! admission logic — every frame is forwarded to the single-writer actor as an
//! [`ActorJob::Sync`], where the [`SyncSessionTable`] drives the responder FSM
//! against the shared repository cell, and the handler writes back whatever
//! frames the actor returns.
//!
//! # Lease renewal and the actor watchdog
//!
//! [`serve`] renews the single-writer deployment lease every `lease_ttl / 3`
//! seconds by round-tripping [`ActorJob::RenewLease`] through the actor thread.
//! Every failure shape is FATAL, fail-closed: a refused renewal means a second
//! writer may exist (continuing would fork the anchor); a failed send or a
//! dropped reply means the single-writer thread is dead (serving against it
//! would serve stale state). The renew interval is therefore also the actor
//! WATCHDOG — a dead actor stops the daemon within one interval even when no
//! connection is active.
//!
//! # Deferred scope (increment 2+)
//!
//! * A production [`AdmissionPolicy`] with real capacity accounting, per-source
//!   rate limits, and pressure-band difficulty. [`TicketRootAuthorityPolicy`] is
//!   the smallest REAL increment-1 policy: it performs a genuine cryptographic
//!   ticket-root-signature authority check and defers capacity/pressure.

use std::cell::RefCell;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::task::JoinSet;

use riot_anchor_protocol::authority::{admit_public_site_ticket, AuthorityError, TicketFloor};
use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::control::{
    ControlRefusal, PrepareHostV1, TransportMode, MAX_CONTROL_FRAME_BYTES,
};
use riot_anchor_protocol::records::{
    DescriptorEnvelopeV1, PublicSiteTicketV2Core, TransportFloor, MAX_DESCRIPTOR_ENVELOPE_BYTES,
};
use riot_anchor_protocol::sync2::MAX_SYNC2_FRAME_BYTES;
use riot_transport::iroh::IrohConnection;
use riot_transport::router::{AlpnRouter, BoundedStream, Deadlines, Exporter, Handler};
use riot_transport::{TransportError, ALPN_ANCHOR_V1, ALPN_SYNC_V2};

use crate::admission::IngressLimits;
use crate::config::PersistedSecrets;
use crate::control::{AdmissionPolicy, AnchorControlService, ControlHandling, PreparePlan};
use crate::repository::{AnchorRepository, AnchorRepositoryError};
use crate::sync_driver::{SyncEvent, SyncJob, SyncSessionTable};
use crate::sync_service::SharedRepo;
use crate::work::{OperatorSigner, PressurePolicy};

/// A fresh-entropy source the control actor uses for anchor-minted ids (operation
/// id, work-challenge nonce). Production wiring uses [`os_entropy`]; tests inject
/// a deterministic generator.
pub type EntropyFn = Box<dyn FnMut() -> [u8; 32] + Send>;

/// A clock the anchor/1 handler stamps onto each request. Production uses
/// [`unix_now`]; tests pin a fixed value.
pub type NowFn = Arc<dyn Fn() -> u64 + Send + Sync>;

/// One unit of control work handed from a connection handler to the single-writer
/// actor: the raw request frame, the observation time, and a one-shot reply.
pub struct ControlJob {
    /// The raw canonical control-request frame bytes (the service decodes it).
    pub request: Vec<u8>,
    /// The observation time (unix seconds) for this request.
    pub now: u64,
    /// Where the actor sends the framed reply.
    pub reply: oneshot::Sender<ControlReply>,
}

/// The actor's reply to a [`ControlJob`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlReply {
    /// Send these canonical [`ControlResponseV1`](riot_anchor_protocol::control::ControlResponseV1)
    /// bytes back to the peer as one frame.
    Respond(Vec<u8>),
    /// A bounded protocol failure (or internal error): close the stream with no
    /// response frame.
    Close,
}

/// One unit of work for the single-writer actor thread.
pub enum ActorJob {
    /// A `riot/anchor/1` control frame (existing behavior).
    Control(ControlJob),
    /// A `riot/sync/2` session event (the DATA-path driver; sessions run
    /// inside the actor thread because their repository handle is `!Send`).
    Sync(SyncJob),
    /// Renew the single-writer deployment lease in place. Sent by [`serve`]'s
    /// renew interval (`lease_ttl / 3`); the arm re-acquires via
    /// [`AnchorRepository::acquire_deployment_lease`], whose same-holder +
    /// same-token path renews without advancing the epoch.
    RenewLease {
        /// This deployment instance's lease holder id (must match startup).
        holder_id: [u8; 32],
        /// The deployment-instance token bound to the database.
        deployment_token: [u8; 32],
        /// The lease term in seconds to extend by.
        lease_ttl_secs: u64,
        /// The observation time (unix seconds) of the renewal.
        now: u64,
        /// Where the actor reports success or the renewal refusal.
        reply: oneshot::Sender<Result<(), String>>,
    },
}

/// Spawn the single-writer actor on its own OS thread (`anchor-single-writer`).
/// The thread OWNS `repo` and `service` for its lifetime — the repository
/// behind `Rc<RefCell<...>>`, deliberately `!Send`, so `riot/sync/2` sessions
/// can share the one cell — and processes every [`ActorJob`] serially, so the
/// non-`Sync` connection and the mutable token ring are never aliased. Returns
/// the `mpsc::UnboundedSender` the handlers clone plus the thread's
/// `JoinHandle`. The thread stops when every sender is dropped; joining the
/// handle surfaces its panic payload if it died.
pub fn spawn_control_actor<P, S>(
    repo: AnchorRepository,
    service: AnchorControlService<P, S>,
    entropy: EntropyFn,
    max_sync_sessions: usize,
) -> (mpsc::UnboundedSender<ActorJob>, std::thread::JoinHandle<()>)
where
    P: AdmissionPolicy + Send + 'static,
    S: OperatorSigner + Send + 'static,
{
    // UNBOUNDED so that cleanup delivery is structurally lossless: the
    // `SyncCloseGuard` destructor MUST be able to enqueue a session's Close
    // even while the queue is busy (a bounded channel's `try_send` silently
    // dropped Closes under routine backpressure, permanently stranding
    // `SyncSessionTable` slots). The queue depth is still bounded in practice
    // despite the unbounded type: every connection handler holds at most ONE
    // in-flight job at a time (it awaits the oneshot reply before reading the
    // next frame), the ingress semaphore caps concurrent handlers at
    // `max_concurrent_control_sessions`, and each sync session contributes at
    // most one guard Close — so depth is bounded by roughly
    // `max_sessions * 2 + 1`. Backpressure lives where it belongs: at the
    // ingress semaphore and the per-handler reply await, not in this channel.
    let (tx, rx) = mpsc::unbounded_channel::<ActorJob>();
    let handle = std::thread::Builder::new()
        .name("anchor-single-writer".into())
        .spawn(move || actor_loop(repo, service, entropy, rx, max_sync_sessions))
        .expect("spawn anchor single-writer thread");
    (tx, handle)
}

/// The single-writer loop: owns the repository behind the shared cell and
/// serializes every control, sync, and lease job. Runs until the channel
/// closes (every sender dropped).
fn actor_loop<P, S>(
    repo: AnchorRepository,
    service: AnchorControlService<P, S>,
    mut entropy: EntropyFn,
    mut rx: mpsc::UnboundedReceiver<ActorJob>,
    max_sync_sessions: usize,
) where
    P: AdmissionPolicy,
    S: OperatorSigner,
{
    let shared: SharedRepo = Rc::new(RefCell::new(repo));
    let mut sync_sessions = SyncSessionTable::new(max_sync_sessions);
    while let Some(job) = rx.blocking_recv() {
        match job {
            ActorJob::Control(job) => {
                // Keep the borrow scope tight: sync-session handling borrows
                // the same cell.
                let reply = {
                    let mut repo = shared.borrow_mut();
                    match service.handle(&mut repo, &job.request, job.now, &mut *entropy) {
                        Ok(ControlHandling::Responded(response)) => {
                            match response.encode_canonical() {
                                Ok(bytes) => ControlReply::Respond(bytes),
                                // Encoding a response the service itself built
                                // should never fail; if it does, close this
                                // stream rather than serve a corrupt frame.
                                // Other connections keep being served.
                                Err(_) => ControlReply::Close,
                            }
                        }
                        Ok(ControlHandling::ProtocolFailure(_failure)) => ControlReply::Close,
                        // A durable-store/codec error ends this stream but
                        // keeps the actor (and the daemon) alive for other
                        // connections.
                        Err(_error) => ControlReply::Close,
                    }
                };
                // The handler may have gone away (peer dropped); that is fine.
                let _ = job.reply.send(reply);
            }
            ActorJob::Sync(job) => sync_sessions.handle(&shared, service.token_ring(), job),
            ActorJob::RenewLease {
                holder_id,
                deployment_token,
                lease_ttl_secs,
                now,
                reply,
            } => {
                // The same-holder + same-token re-acquire IS the renew-in-place
                // path (it keeps the lease epoch for an active holder); any
                // refusal means the lease was lost, stolen, or token-mismatched.
                let result = shared
                    .borrow_mut()
                    .acquire_deployment_lease(&holder_id, &deployment_token, now, lease_ttl_secs)
                    .map(|_lease| ())
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
        }
    }
}

/// Build the `riot/anchor/1` [`Handler`]. It reads control frames off the bounded
/// stream and, for each, hands a [`ControlJob`] to the actor and writes the
/// actor's reply back — one request/response per frame, looping until the peer
/// closes the stream. A [`ControlReply::Close`] (bounded protocol failure) ends
/// the session with no response frame.
pub fn control_handler(tx: mpsc::UnboundedSender<ActorJob>, now_fn: NowFn) -> Handler {
    Arc::new(move |mut stream: BoundedStream, _exporter: Exporter| {
        let tx = tx.clone();
        let now_fn = Arc::clone(&now_fn);
        Box::pin(async move {
            loop {
                let request = match stream.read_frame().await {
                    Ok(frame) => frame,
                    // Peer finished and closed cleanly: the session is complete.
                    Err(TransportError::StreamClosed) => return Ok(()),
                    Err(error) => return Err(error),
                };
                let (reply_tx, reply_rx) = oneshot::channel();
                let job = ControlJob {
                    request,
                    now: (now_fn)(),
                    reply: reply_tx,
                };
                if tx.send(ActorJob::Control(job)).is_err() {
                    return Err(TransportError::Io(std::io::Error::other(
                        "anchor control actor unavailable",
                    )));
                }
                match reply_rx.await {
                    Ok(ControlReply::Respond(bytes)) => stream.write_frame(&bytes).await?,
                    Ok(ControlReply::Close) => return Ok(()),
                    Err(_) => {
                        return Err(TransportError::Io(std::io::Error::other(
                            "anchor control actor dropped reply",
                        )))
                    }
                }
            }
        }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
    })
}

/// The next daemon-unique `riot/sync/2` session id.
fn next_session_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// RAII guard guaranteeing the actor learns a `riot/sync/2` connection is gone
/// on EVERY handler exit path: normal completion, early `?` returns from a
/// failed write, and — crucially — router-level cancellation (the absolute
/// session lifetime and the extra-stream violation both DROP the handler
/// future, so no code after the read loop ever runs). Without it a live
/// session strands in the [`SyncSessionTable`] forever, permanently consuming
/// one of the bounded `max_sessions` slots. This guard is the ONLY Close
/// mechanism — the handler body sends no explicit Close of its own.
struct SyncCloseGuard {
    tx: mpsc::UnboundedSender<ActorJob>,
    session_id: u64,
}

impl Drop for SyncCloseGuard {
    fn drop(&mut self) {
        // Structurally lossless: on the unbounded channel `send` fails ONLY
        // when the receiver is gone — i.e. the actor thread has exited and the
        // whole session table died with it, so there is nothing left to clean
        // up. That is the single acceptable loss; a busy queue can never drop
        // this Close. The reply receiver is a throwaway: nobody is left to
        // read it (the table tolerates a dropped receiver).
        let (reply, _throwaway) = oneshot::channel();
        let _ = self.tx.send(ActorJob::Sync(SyncJob {
            session_id: self.session_id,
            event: SyncEvent::Close,
            reply,
        }));
    }
}

/// Build the `riot/sync/2` [`Handler`]: a thin frame shuttle to the actor. The
/// first frame of a connection is the session's `Open` (stamped with the
/// current time); every later frame is a plain `Frame` event. The handler owns
/// no protocol state beyond that split — decode, routing, admission, and
/// refusals all happen inside the actor's [`SyncSessionTable`] — and its
/// [`SyncCloseGuard`] tells the actor when the connection is gone (on every
/// exit path, including future-drop cancellation) so session state is dropped.
pub fn sync_handler(tx: mpsc::UnboundedSender<ActorJob>, now_fn: NowFn) -> Handler {
    Arc::new(move |mut stream: BoundedStream, _exporter: Exporter| {
        let tx = tx.clone();
        let now_fn = Arc::clone(&now_fn);
        Box::pin(async move {
            let session_id = next_session_id();
            // Created BEFORE the read loop: from here on, every way this
            // future can end (return, `?`, drop) delivers the session's Close.
            let _close_guard = SyncCloseGuard {
                tx: tx.clone(),
                session_id,
            };
            let mut opened = false;
            loop {
                let frame = match stream.read_frame().await {
                    Ok(frame) => frame,
                    // Peer finished and closed cleanly: the session is over.
                    Err(TransportError::StreamClosed) => return Ok(()),
                    Err(error) => return Err(error),
                };
                let event = if opened {
                    SyncEvent::Frame { frame }
                } else {
                    opened = true;
                    SyncEvent::Open {
                        frame,
                        now: (now_fn)(),
                    }
                };
                let (reply_tx, reply_rx) = oneshot::channel();
                if tx
                    .send(ActorJob::Sync(SyncJob {
                        session_id,
                        event,
                        reply: reply_tx,
                    }))
                    .is_err()
                {
                    return Err(TransportError::Io(std::io::Error::other(
                        "anchor sync actor unavailable",
                    )));
                }
                let reply = match reply_rx.await {
                    Ok(reply) => reply,
                    Err(_) => {
                        return Err(TransportError::Io(std::io::Error::other(
                            "anchor sync actor dropped reply",
                        )))
                    }
                };
                for outbound in &reply.outbound {
                    stream.write_frame(outbound).await?;
                }
                if reply.terminated {
                    // Deliverability: dropping the connection right after the
                    // final write races QUIC teardown and can discard the
                    // still-unacknowledged closing frames (refusal or
                    // NamespaceComplete). The peer closes its side once it has
                    // read everything, so hold the session open until its
                    // FIN/close arrives; whatever ends the drain is teardown
                    // noise on an already-terminated session.
                    while stream.read_frame().await.is_ok() {}
                    return Ok(());
                }
            }
        }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
    })
}

/// A REAL Ed25519 operator signer that loads its secret from operator config
/// (rather than the fixed test key the unit tests use). It signs the
/// control-plane preimages (work challenges, descriptors) the protocol expects.
/// `Clone` because the control service clones it into the Commit host service.
#[derive(Clone)]
pub struct Ed25519OperatorSigner {
    key: ed25519_dalek::SigningKey,
}

impl Ed25519OperatorSigner {
    /// Load the operator signer from a 32-byte Ed25519 secret seed.
    #[must_use]
    pub fn from_secret_bytes(secret: [u8; 32]) -> Self {
        Self {
            key: ed25519_dalek::SigningKey::from_bytes(&secret),
        }
    }

    /// The operator's public verification key (must match
    /// `AnchorControlContext::operator_public_key`).
    #[must_use]
    pub fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
}

impl OperatorSigner for Ed25519OperatorSigner {
    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        self.key.sign(preimage).to_bytes()
    }
}

/// The increment-1 admission policy. Its ticket authority check DELEGATES to the
/// canonical [`admit_public_site_ticket`] gate — the same security-critical
/// function the sibling `listing.rs` / `removal.rs` paths use — rather than
/// hand-rolling a subset. That gate enforces, fail-closed and in order: bounded
/// canonical structure, the root Ed25519 signature, `min_sync_version == 2`
/// exactly, the transport floor (require_arti is refused on the MVP clearnet
/// anchor), the 90-day ticket-lifetime cap, inclusive expiry, and epoch rollback.
/// On success the community root and ordered `O`/`C`/`W` namespaces are taken from
/// the admitted (verified) ticket core.
///
/// DEFERRED(WU-019 increment 2):
/// * capacity accounting ([`capacity_for_prepare_host`](AdmissionPolicy::capacity_for_prepare_host)
///   always admits — no `busy`/`over_quota` back-pressure yet);
/// * pressure banding ([`pressure_band`](AdmissionPolicy::pressure_band) returns
///   difficulty 0 — no admission work is required yet);
/// * base-generation and retained-snapshot tracking from the repository
///   (`base_generation` is 0 and the retained digests echo the request).
pub struct TicketRootAuthorityPolicy {
    sync_version: u64,
}

impl TicketRootAuthorityPolicy {
    /// Construct the policy bound to the anchor's negotiated `sync_version` (the
    /// same value carried in [`crate::control::AnchorControlContext::sync_version`]).
    #[must_use]
    pub fn new(sync_version: u64) -> Self {
        Self { sync_version }
    }

    /// Map a canonical [`AuthorityError`] onto the control-plane [`ControlRefusal`]
    /// vocabulary. The three distinguished cases carry actionable detail; every
    /// other authority fault (bad signature, structure, epoch rollback, manifest
    /// mismatch) collapses to `invalid_ticket_authority`.
    fn map_authority_error(
        &self,
        core: &PublicSiteTicketV2Core,
        observed_at: u64,
        error: AuthorityError,
    ) -> ControlRefusal {
        match error {
            AuthorityError::UnsupportedTransport => ControlRefusal::UnsupportedTransport {
                required_mode: ticket_required_transport_mode(core),
                observed_mode: TransportMode::RequireNone,
            },
            AuthorityError::UnsupportedVersion => ControlRefusal::UnsupportedVersion {
                supported_versions: vec![self.sync_version],
            },
            AuthorityError::ExpiredTicket => ControlRefusal::TicketExpired {
                expires_at: core.expiry_unix_seconds,
                observed_at,
            },
            _ => ControlRefusal::InvalidTicketAuthority,
        }
    }
}

/// The transport mode a ticket DEMANDS, for the `unsupported_transport` refusal
/// message: the ticket's own `transport_floor` if it is non-`require_none`,
/// otherwise its `manifest_required_transport`.
fn ticket_required_transport_mode(core: &PublicSiteTicketV2Core) -> TransportMode {
    let floor = if core.transport_floor != TransportFloor::RequireNone {
        core.transport_floor
    } else {
        core.manifest_required_transport
    };
    match floor {
        TransportFloor::RequireNone => TransportMode::RequireNone,
        TransportFloor::RequireArti => TransportMode::RequireArti,
    }
}

impl AdmissionPolicy for TicketRootAuthorityPolicy {
    fn authorize_prepare_host(
        &self,
        request: &PrepareHostV1,
        observed_at: u64,
        highest_transport_epoch: Option<u32>,
    ) -> Result<PreparePlan, ControlRefusal> {
        let envelope = &request.root_signed_ticket_core;

        // REAL authority: delegate to the canonical, security-critical ticket gate
        // (signature + exact version + transport floor + 90-day lifetime cap +
        // expiry + epoch rollback). No manifest is available at PrepareHost, and
        // the anchor offers only require_none transport; per-root epoch floor
        // tracking is increment-2 scope, so no prior epoch is asserted here.
        let admitted = admit_public_site_ticket(
            envelope,
            None,
            &TransportFloor::RequireNone,
            &TicketFloor {
                root_id: envelope.core.root_id,
                highest_transport_epoch,
            },
            observed_at,
        )
        .map_err(|error| self.map_authority_error(&envelope.core, observed_at, error))?;

        let core = &admitted.core;
        Ok(PreparePlan {
            community_root: core.root_id,
            ordered_namespace_host_plan: [
                core.o_namespace_id,
                core.c_namespace_id,
                core.w_namespace_id,
            ],
            // DEFERRED(WU-019 increment 2): read the anchor's currently-retained
            // snapshot digests from the repository; increment 1 echoes what the
            // client reported it is hosting.
            ordered_retained_snapshot_digests: request.ordered_namespace_snapshot_digests,
            // DEFERRED(WU-019 increment 2): capture the durable base site
            // generation from the repository.
            base_generation: 0,
        })
    }

    fn capacity_for_prepare_host(
        &self,
        _plan: &PreparePlan,
        _observed_at: u64,
    ) -> Result<(), ControlRefusal> {
        // DEFERRED(WU-019 increment 2): real capacity accounting / global
        // headroom back-pressure. Increment 1 admits (no busy/over_quota gate).
        Ok(())
    }

    fn pressure_band(&self, _community_root: &[u8; 32], _observed_at: u64) -> PressurePolicy {
        // DEFERRED(WU-019 increment 2): adaptive pressure banding. Increment 1
        // requires no admission work (difficulty 0).
        PressurePolicy {
            policy_epoch: 0,
            difficulty: 0,
        }
    }
}

/// The daemon's static deployment configuration.
pub struct DaemonConfig {
    /// Path to the durable `AnchorRepository` SQLite database.
    pub db_path: PathBuf,
    /// The 32-byte Ed25519 secret giving the PUBLIC iroh endpoint its stable
    /// NodeId (so peers can discover and dial the anchor). This is the endpoint
    /// identity, distinct from the operator signing key.
    pub endpoint_secret_key: [u8; 32],
    /// This deployment PROCESS's single-writer lease holder id.
    ///
    /// Assembly ([`crate::config::finalize_service`]) fills a zero placeholder;
    /// [`serve`] OVERWRITES it with a fresh per-process random value — its
    /// first draw from the daemon's [`EntropyFn`], before any other startup
    /// work. The holder id is deliberately NOT derived from the operator
    /// secret: an operator-derived holder made a second daemon started from
    /// the same config present identical holder+token and renew the first
    /// daemon's lease in place — two live writers forking one database. Tests
    /// pin the holder by injecting a deterministic entropy fn.
    pub holder_id: [u8; 32],
    /// The deployment-instance token bound to the database (clone detection).
    pub deployment_token: [u8; 32],
    /// The deployment-lease term in seconds.
    pub lease_ttl_secs: u64,
    /// Bounded-ingress ceilings (concurrency).
    pub ingress: IngressLimits,
}

/// An error that prevents the daemon from starting or running.
#[derive(Debug)]
pub enum DaemonError {
    /// Opening the repository or acquiring the deployment lease failed.
    Repository(AnchorRepositoryError),
    /// Binding the public endpoint or serving a connection failed.
    Transport(TransportError),
    /// Persisted operator/descriptor state was malformed or incoherent.
    Configuration(String),
}

impl core::fmt::Display for DaemonError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "anchor daemon repository error: {error}"),
            Self::Transport(error) => write!(formatter, "anchor daemon transport error: {error}"),
            Self::Configuration(error) => {
                write!(formatter, "anchor daemon configuration error: {error}")
            }
        }
    }
}

impl std::error::Error for DaemonError {}

impl From<AnchorRepositoryError> for DaemonError {
    fn from(error: AnchorRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<TransportError> for DaemonError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

/// Current wall-clock time as unix seconds (saturating at 0 before the epoch).
#[must_use]
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// A cryptographically-secure OS-backed entropy source for the control actor.
#[must_use]
pub fn os_entropy() -> EntropyFn {
    Box::new(|| {
        let mut buffer = [0u8; 32];
        getrandom::getrandom(&mut buffer).expect("OS entropy source unavailable");
        buffer
    })
}

/// The ALPNs this daemon's endpoints advertise: the `riot/anchor/1` control
/// plane and the `riot/sync/2` data path.
fn anchor_alpns() -> Vec<Vec<u8>> {
    vec![ALPN_ANCHOR_V1.to_vec(), ALPN_SYNC_V2.to_vec()]
}

fn bind_io_error(error: impl core::fmt::Display) -> DaemonError {
    DaemonError::Transport(TransportError::Io(std::io::Error::other(error.to_string())))
}

/// Bind the PUBLIC anchor endpoint (relay + discovery, dialable across NAT).
///
/// NOTE: the transport crate's `iroh::bind_public` hard-codes the `riot/sync/1`
/// ALPN, so it cannot serve `riot/anchor/1`. We build the public endpoint here
/// (the `daemon` module is the only place iroh may be used directly) with the
/// same public `N0` preset but advertising the anchor control ALPN so it is
/// negotiable.
async fn bind_anchor_endpoint(secret: [u8; 32]) -> Result<iroh::Endpoint, DaemonError> {
    use iroh::endpoint::presets;
    use iroh::{Endpoint, SecretKey};
    Endpoint::builder(presets::N0)
        .secret_key(SecretKey::from_bytes(&secret))
        .alpns(anchor_alpns())
        .bind()
        .await
        .map_err(bind_io_error)
}

/// Bind a LOCAL anchor endpoint (direct only, `N0DisableRelay` — no relay/DNS),
/// advertising the control ALPN. This is the in-process / LAN counterpart of
/// [`bind_anchor_endpoint`], used by end-to-end tests (and any local-only
/// deployment) that dial the daemon directly by address.
pub async fn bind_local_anchor_endpoint(secret: [u8; 32]) -> Result<iroh::Endpoint, DaemonError> {
    use iroh::endpoint::presets;
    use iroh::{Endpoint, SecretKey};
    Endpoint::builder(presets::N0DisableRelay)
        .secret_key(SecretKey::from_bytes(&secret))
        .alpns(anchor_alpns())
        .bind()
        .await
        .map_err(bind_io_error)
}

/// Load the database-durable anchor secrets, or atomically persist the derived
/// `proposals` on first boot (first write wins — see
/// [`AnchorRepository::load_or_initialize_secret`]). Opens the repository
/// briefly and standalone: the daemon calls this BEFORE assembling the service
/// so the control context and token ring are built from the persisted values.
pub fn load_or_initialize_secrets(
    db_path: &std::path::Path,
    proposals: &PersistedSecrets,
) -> Result<PersistedSecrets, DaemonError> {
    let mut repo = AnchorRepository::open(db_path)?;
    let genesis_random =
        repo.load_or_initialize_secret("genesis_random", &proposals.genesis_random)?;
    let token_secret =
        repo.load_or_initialize_secret("token_secret_v1", &proposals.token_secret)?;
    Ok(PersistedSecrets {
        genesis_random,
        token_secret,
    })
}

/// Run the anchor daemon on a freshly bound PUBLIC endpoint until `shutdown`
/// resolves.
///
/// Loads-or-initializes the database-durable secrets (genesis random +
/// namespace-token secret), assembles the service from the PERSISTED values
/// (so a changed operator derivation can never silently change the anchor id
/// or token secret), then binds the public endpoint and delegates to [`serve`]
/// — whose first act is drawing the PER-PROCESS lease holder id from
/// `entropy` (see [`DaemonConfig::holder_id`]).
pub async fn run(
    config: crate::config::Config,
    entropy: EntropyFn,
    shutdown: impl Future<Output = ()> + Send,
) -> Result<(), DaemonError> {
    let proposals = crate::config::secret_proposals(&config);
    let persisted = load_or_initialize_secrets(config.db_path(), &proposals)?;
    let (daemon_config, service) = crate::config::finalize_service(config, persisted);
    let endpoint = bind_anchor_endpoint(daemon_config.endpoint_secret_key).await?;
    serve(endpoint, daemon_config, service, entropy, shutdown).await
}

/// Serve the control plane on an already-bound `endpoint` until `shutdown`
/// resolves.
///
/// Startup: draw the PER-PROCESS lease holder id (the first entropy use), open
/// the repository, acquire the single-writer deployment lease (fail-closed if
/// another live holder holds it), run readiness recovery, spawn the control
/// actor, and register the `riot/anchor/1` handler. Then accept connections in
/// a loop; a single failing connection (unknown ALPN, timeout, busy) is logged
/// and does not stop the loop. The `endpoint` must advertise the control ALPN
/// (use [`bind_anchor_endpoint`] / [`bind_local_anchor_endpoint`]).
///
/// Taking the endpoint as a parameter is the test seam: an end-to-end test binds
/// a LOCAL endpoint, drives real connections through the accept loop + handler +
/// actor, and signals `shutdown` to stop.
pub async fn serve<P, S>(
    endpoint: iroh::Endpoint,
    mut config: DaemonConfig,
    mut service: AnchorControlService<P, S>,
    mut entropy: EntropyFn,
    shutdown: impl Future<Output = ()> + Send,
) -> Result<(), DaemonError>
where
    P: AdmissionPolicy + Send + 'static,
    S: OperatorSigner + Send + 'static,
{
    // PER-PROCESS lease identity — the daemon's FIRST entropy draw, before any
    // other startup work. The holder id must NOT be derived from the operator
    // secret: with an operator-derived holder, a second daemon started from
    // the SAME config (the realistic accidental double-start this lease
    // exists to prevent) presented identical holder+token, took the
    // renew-in-place path, and started fine — two live writers forking one
    // database, each extending the other's lease. With a fresh per-process
    // draw the double-start presents the same deployment token but a
    // DIFFERENT holder: `holder_active && !same_holder` → `LeaseHeld`, and
    // the second process exits fatally instead of forking. The first daemon
    // is unaffected — its own holder+token keep renewing in place below. The
    // deployment token stays operator-derived (it binds the database to this
    // deployment/operator; a different operator's token keeps failing
    // `LeaseTokenMismatch`).
    config.holder_id = entropy();

    let mut repo = AnchorRepository::open(&config.db_path)?;

    let now = unix_now();
    // Single-writer guard: fail closed if a different live deployment holds it.
    // The accept loop below renews this lease every `lease_ttl / 3` seconds.
    let _lease = repo.acquire_deployment_lease(
        &config.holder_id,
        &config.deployment_token,
        now,
        config.lease_ttl_secs,
    )?;
    repo.recover_readiness(now)?;

    // Bind one canonical descriptor to this database. Restarts reuse those
    // exact bytes; rebuilding epoch 0 from the wall clock would equivocate.
    let proposed_descriptor = service
        .descriptor()
        .encode_canonical()
        .map_err(|error| DaemonError::Configuration(error.to_string()))?;
    let persisted_descriptor = repo.load_or_initialize_descriptor(
        &service.descriptor().body.current_operator_key_id,
        &proposed_descriptor,
    )?;
    let persisted_descriptor = decode_canonical::<DescriptorEnvelopeV1>(
        &persisted_descriptor,
        MAX_DESCRIPTOR_ENVELOPE_BYTES,
    )
    .map_err(|error| DaemonError::Configuration(error.to_string()))?;
    service
        .install_persisted_descriptor(persisted_descriptor, now)
        .map_err(|error| DaemonError::Configuration(error.to_string()))?;

    // One shared ceiling for control sessions and (future) sync sessions until
    // a dedicated sync limit exists in the ingress config.
    let max_sessions = config.ingress.max_concurrent_control_sessions;
    let (tx, actor) = spawn_control_actor(repo, service, entropy, max_sessions);
    let now_fn: NowFn = Arc::new(unix_now);

    let mut router = AlpnRouter::new(max_sessions);
    router.register_with_max_frame(
        ALPN_ANCHOR_V1,
        Deadlines::control(),
        MAX_CONTROL_FRAME_BYTES,
        control_handler(tx.clone(), Arc::clone(&now_fn)),
    );
    // The sync/2 ceiling is the PROTOCOL's (a maximal EntriesChunk bundle plus
    // envelope headroom, `MAX_SYNC2_FRAME_BYTES`) — the router's default sync
    // ceiling is tighter and would length-reject a maximal chunk before decode.
    router.register_with_max_frame(
        ALPN_SYNC_V2,
        Deadlines::sync(),
        MAX_SYNC2_FRAME_BYTES,
        sync_handler(tx.clone(), Arc::clone(&now_fn)),
    );

    // The lease renew cadence — also the single-writer WATCHDOG cadence: every
    // tick round-trips the actor thread, so a dead actor is detected within
    // one interval even when no connection is active. The first tick fires
    // immediately (a harmless renew-in-place right after startup acquisition).
    let lease_renew_period = std::time::Duration::from_secs((config.lease_ttl_secs / 3).max(1));
    let mut lease_renew = tokio::time::interval(lease_renew_period);
    lease_renew.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let router = Arc::new(router);
    // Bound incomplete handshakes as well as routed sessions. An accepted
    // connection holds one permit from before its server-side handshake until
    // dispatch ends; excess attempts are refused before allocating a task.
    let ingress_permits = Arc::new(Semaphore::new(max_sessions));
    let mut sessions = JoinSet::new();
    // A fatal renew/watchdog failure: the loop breaks with this set and the
    // daemon returns it after teardown.
    let mut fatal: Option<DaemonError> = None;
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
        _ = &mut shutdown => break,
        _ = lease_renew.tick() => {
            // Renew the single-writer deployment lease in place. THREE failure
            // shapes are all fatal, fail-closed: a refused renewal means a
            // second writer may exist (continuing would fork the anchor); a
            // failed send or a dropped reply means the single-writer thread
            // is dead (serving against it would serve stale state).
            let (reply_tx, reply_rx) = oneshot::channel();
            let job = ActorJob::RenewLease {
                holder_id: config.holder_id,
                deployment_token: config.deployment_token,
                lease_ttl_secs: config.lease_ttl_secs,
                now: (now_fn)(),
                reply: reply_tx,
            };
            if tx.send(job).is_err() {
                fatal = Some(DaemonError::Configuration(
                    "anchor single-writer actor died".to_string(),
                ));
                break;
            }
            match reply_rx.await {
                Ok(Ok(())) => {}
                Ok(Err(refusal)) => {
                    fatal = Some(DaemonError::Configuration(format!(
                        "deployment lease lost: {refusal}"
                    )));
                    break;
                }
                Err(_) => {
                    fatal = Some(DaemonError::Configuration(
                        "anchor single-writer actor died".to_string(),
                    ));
                    break;
                }
            }
        }
        Some(joined) = sessions.join_next(), if !sessions.is_empty() => {
            if let Err(error) = joined {
                eprintln!("anchor: control session task ended: {error}");
            }
        }
        incoming = endpoint.accept() => {
            let Some(incoming) = incoming else {
                return Err(DaemonError::Transport(TransportError::StreamClosed));
            };
            let Ok(permit) = Arc::clone(&ingress_permits).try_acquire_owned() else {
                incoming.refuse();
                continue;
            };
            let router = Arc::clone(&router);
            sessions.spawn(async move {
                let result = match incoming.await {
                    Ok(connection) => router.dispatch(IrohConnection::new(connection)).await,
                    Err(error) => Err(TransportError::Io(std::io::Error::other(
                        error.to_string(),
                    ))),
                };
                if let Err(error) = result {
                    // A single connection failing must never kill the accept loop.
                    eprintln!("anchor: control connection ended: {error}");
                }
                drop(permit);
            });
            }
        }
    }

    endpoint.close().await;
    sessions.abort_all();
    while sessions.join_next().await.is_some() {}
    // Every remaining `ActorJob` sender lives in the router's handlers and the
    // renew loop's `tx`; drop both so the single-writer thread sees channel
    // closure and exits. Joining then surfaces a panic payload from the actor
    // thread in the daemon's error path instead of silently discarding it.
    drop(router);
    // On a CLEAN shutdown, relinquish the single-writer lease by expiring it
    // in place (a same-holder renew with ttl 0), so an immediate restart —
    // which presents a fresh per-process holder id — can take the lease
    // without waiting out the TTL. Best-effort: a dead actor or a refusal is
    // ignored (the TTL then bounds the lockout). On a FATAL exit the lease is
    // deliberately left standing: in the lease-lost case another holder owns
    // it, and in the actor-death case there is no writer thread left to run
    // the relinquish anyway.
    if fatal.is_none() {
        let (reply_tx, reply_rx) = oneshot::channel();
        let relinquish = ActorJob::RenewLease {
            holder_id: config.holder_id,
            deployment_token: config.deployment_token,
            lease_ttl_secs: 0,
            now: unix_now(),
            reply: reply_tx,
        };
        if tx.send(relinquish).is_ok() {
            let _ = reply_rx.await;
        }
    }
    drop(tx);
    let joined = tokio::task::spawn_blocking(move || actor.join()).await;
    // A fatal renew/watchdog failure is the daemon's own diagnosis and wins
    // over the join outcome (in the watchdog case the dead thread's panic
    // payload merely restates why the actor died).
    if let Some(error) = fatal {
        return Err(error);
    }
    let joined = joined.map_err(|error| {
        DaemonError::Configuration(format!("anchor single-writer join task failed: {error}"))
    })?;
    joined.map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic payload".to_string());
        DaemonError::Configuration(format!("anchor single-writer actor panicked: {message}"))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use riot_anchor_protocol::authority::TicketReason;

    fn core() -> PublicSiteTicketV2Core {
        PublicSiteTicketV2Core {
            root_id: [0u8; 32],
            o_namespace_id: [0u8; 32],
            c_namespace_id: [0u8; 32],
            w_namespace_id: [0u8; 32],
            manifest_digest: [0u8; 32],
            manifest_version: 0,
            min_sync_version: 2,
            manifest_required_transport: TransportFloor::RequireNone,
            transport_floor: TransportFloor::RequireNone,
            transport_epoch: 0,
            issued_unix_seconds: 0,
            expiry_unix_seconds: 100,
        }
    }

    #[test]
    fn map_authority_error_covers_each_distinguished_variant() {
        let policy = TicketRootAuthorityPolicy::new(2);
        let mut arti = core();
        arti.transport_floor = TransportFloor::RequireArti;
        assert_eq!(
            policy.map_authority_error(&arti, 50, AuthorityError::UnsupportedTransport),
            ControlRefusal::UnsupportedTransport {
                required_mode: TransportMode::RequireArti,
                observed_mode: TransportMode::RequireNone,
            }
        );
        assert_eq!(
            policy.map_authority_error(&core(), 50, AuthorityError::UnsupportedVersion),
            ControlRefusal::UnsupportedVersion {
                supported_versions: vec![2],
            }
        );
        assert_eq!(
            policy.map_authority_error(&core(), 250, AuthorityError::ExpiredTicket),
            ControlRefusal::TicketExpired {
                expires_at: 100,
                observed_at: 250,
            }
        );
        // Every other authority fault collapses to invalid_ticket_authority.
        for other in [
            AuthorityError::InvalidTicket(TicketReason::Signature),
            AuthorityError::InvalidTicket(TicketReason::Root),
            AuthorityError::InvalidTicket(TicketReason::Structure),
            AuthorityError::EpochRollback,
            AuthorityError::ManifestMismatch,
        ] {
            assert_eq!(
                policy.map_authority_error(&core(), 50, other),
                ControlRefusal::InvalidTicketAuthority
            );
        }
    }

    #[test]
    fn required_transport_mode_prefers_ticket_floor_then_manifest() {
        let mut arti_floor = core();
        arti_floor.transport_floor = TransportFloor::RequireArti;
        assert_eq!(
            ticket_required_transport_mode(&arti_floor),
            TransportMode::RequireArti
        );

        let mut arti_manifest = core();
        arti_manifest.manifest_required_transport = TransportFloor::RequireArti;
        assert_eq!(
            ticket_required_transport_mode(&arti_manifest),
            TransportMode::RequireArti
        );

        assert_eq!(
            ticket_required_transport_mode(&core()),
            TransportMode::RequireNone
        );
    }

    #[test]
    fn daemon_error_display_and_from_impls() {
        let repo: DaemonError = AnchorRepositoryError::LeaseExpired.into();
        assert!(matches!(repo, DaemonError::Repository(_)));
        assert!(repo.to_string().contains("repository"));

        let transport: DaemonError = TransportError::UnknownAlpn.into();
        assert!(matches!(transport, DaemonError::Transport(_)));
        assert!(transport.to_string().contains("transport"));
    }

    #[test]
    fn unix_now_is_after_2023() {
        // Sanity: the clock returns a plausible present-day unix timestamp.
        assert!(unix_now() > 1_700_000_000);
    }

    #[test]
    fn startup_secrets_survive_a_changed_proposal_across_reopen() {
        let mut db_path = std::env::temp_dir();
        db_path.push(format!(
            "riot-anchor-secrets-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let first = load_or_initialize_secrets(
            &db_path,
            &PersistedSecrets {
                genesis_random: [41u8; 32],
                token_secret: [42u8; 32],
            },
        )
        .expect("first boot initializes");
        assert_eq!(first.genesis_random, [41u8; 32]);
        assert_eq!(first.token_secret, [42u8; 32]);

        // A later boot proposing DIFFERENT derivations (an operator-key
        // rotation) gets the database-bound originals back.
        let second = load_or_initialize_secrets(
            &db_path,
            &PersistedSecrets {
                genesis_random: [51u8; 32],
                token_secret: [52u8; 32],
            },
        )
        .expect("second boot loads");
        assert_eq!(second.genesis_random, [41u8; 32]);
        assert_eq!(second.token_secret, [42u8; 32]);

        let _ = std::fs::remove_file(&db_path);
    }
}
