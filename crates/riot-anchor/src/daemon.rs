//! WU-019 increment 1: the runnable anchor control plane.
//!
//! This module assembles the pieces the hosting core already provides —
//! [`AnchorRepository`], [`AnchorControlService`] — into a process that can
//! actually run: open the repository, acquire the single-writer deployment
//! lease, recover startup readiness, open a public iroh endpoint advertising
//! the `riot/anchor/1` control ALPN, and serve control requests through the
//! real service until asked to stop.
//!
//! # Scope
//!
//! Increment 1 is control-plane only. The `sync/2` data path (which needs the
//! FSM drive loop) and the full ingress DoS-hardening tail (the 82 config
//! values, slow-loris/header/Range/compression/keep-alive limits, HTTP/TLS
//! ingress) are both explicitly deferred — see
//! `docs/coordination/2026-07-20-anchor-runnability-gap.md`. `PrepareHost` is
//! served by a fail-closed stub policy ([`StubAdmissionPolicy`]) until a real
//! hosting `AdmissionPolicy` backed by ticket/authority verification lands;
//! `Describe`, `GetWorkChallenge`, and `GetOperation` work end-to-end today.
//!
//! # Single-writer discipline
//!
//! [`AnchorRepository`] is backed by one non-pooled WAL SQLite connection and
//! [`AnchorControlService::handle`] takes `&mut AnchorRepository`. This module
//! never shares that `&mut` across connections: ONE task (the "repo actor",
//! [`run_repo_actor`]) owns the repository and the control service for the
//! daemon's entire lifetime. Every accepted connection's handler only ever
//! sends request bytes to that task over an `mpsc` channel and awaits a
//! `oneshot` reply — it never touches the repository directly. This is Option
//! A from the runnability gap doc (the recommended design).
//!
//! # Key material
//!
//! The daemon loads exactly one 32-byte root secret, from a file path (never a
//! CLI value, and never echoed into diagnostics — see [`SecretKeySource`]) or,
//! for tests, an explicit in-process value. Every other secret the daemon
//! needs (the iroh endpoint identity, the operator Ed25519 signing seed, the
//! deployment-lease holder id and token, the descriptor's genesis randomness,
//! the namespace-token ring secret) is deterministically domain-separated from
//! that one root secret via SHA-256, so restarts are stable without any extra
//! persisted state.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey};
use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, SecretKey};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::{ControlRefusal, PrepareHostV1};
use riot_anchor_protocol::digest::anchor_id as compute_anchor_id;
use riot_anchor_protocol::records::{
    AnchorDescriptorBodyV1, AnchorLimitProfileV1, DescriptorEnvelopeV1, EnabledRole,
    OperatorVerificationKeyV1,
};

use riot_transport::iroh::accept_with_router;
use riot_transport::router::{AlpnRouter, BoundedStream, Deadlines, Exporter, Handler};
use riot_transport::{TransportError, ALPN_ANCHOR_V1};

use crate::admission::bounded_router;
use crate::control::{
    AdmissionPolicy, AnchorControlContext, AnchorControlService, ControlError, ControlHandling,
    PreparePlan,
};
use crate::repository::{AnchorRepository, AnchorRepositoryError};
use crate::work::{OperatorSigner, PressurePolicy, TokenSecretRing};

// ---------------------------------------------------------------------------
// Configuration.
// ---------------------------------------------------------------------------

/// Where the durable [`AnchorRepository`] lives.
#[derive(Debug, Clone)]
pub enum RepoLocation {
    /// A durable on-disk database at this path.
    File(PathBuf),
    /// An in-memory database — tests / ephemeral runs only; nothing survives
    /// restart.
    InMemory,
}

/// Where the daemon's one root secret comes from. The raw bytes are never
/// accepted as a CLI argument and never echoed into diagnostics; see the
/// redacted [`std::fmt::Debug`] impl below.
pub enum SecretKeySource {
    /// Read exactly 32 raw bytes from this path at startup.
    File(PathBuf),
    /// An explicit in-process secret. Test/ephemeral use only — production
    /// wiring must never construct this from a CLI flag or an environment
    /// variable that carries the raw key.
    Ephemeral([u8; 32]),
}

impl std::fmt::Debug for SecretKeySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(_) => f.write_str("SecretKeySource::File(<redacted path>)"),
            Self::Ephemeral(_) => f.write_str("SecretKeySource::Ephemeral(<redacted>)"),
        }
    }
}

/// Which iroh preset the control endpoint binds under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointBinding {
    /// N0 relay + discovery: reachable from anywhere across NAT. Production.
    Public,
    /// Direct-only, no relay/discovery: same-host/LAN loopback. Tests.
    LocalOnly,
}

/// Minimal daemon configuration for WU-019 increment 1 — the control plane.
/// Deliberately NOT the full ingress/DoS configuration surface (82 values);
/// that is the production-hardening tail, out of scope here.
#[derive(Debug)]
pub struct DaemonConfig {
    /// Where the durable repository lives.
    pub repo: RepoLocation,
    /// Where the one root secret comes from.
    pub secret_key: SecretKeySource,
    /// Which iroh preset the control endpoint binds under.
    pub endpoint_binding: EndpointBinding,
    /// The single-writer deployment lease TTL, in seconds.
    pub lease_ttl_secs: u64,
    /// The router's concurrent-session ceiling (see [`crate::admission`]).
    pub max_concurrent_sessions: usize,
}

impl DaemonConfig {
    /// Build a config from environment variables. No CLI value ever carries
    /// key material — only a file *path* is read from the environment.
    ///
    /// - `RIOT_ANCHOR_DB`: a file path, or `memory` for an in-memory repository.
    /// - `RIOT_ANCHOR_SECRET_KEY_PATH`: path to a 32-byte root secret file.
    /// - `RIOT_ANCHOR_LEASE_TTL_SECS` (optional, default 300).
    /// - `RIOT_ANCHOR_MAX_SESSIONS` (optional, default
    ///   [`crate::admission::DEFAULT_MAX_CONCURRENT_SESSIONS`]).
    pub fn from_env() -> Result<Self, DaemonError> {
        let db = std::env::var("RIOT_ANCHOR_DB")
            .map_err(|_| DaemonError::MissingEnv("RIOT_ANCHOR_DB"))?;
        let repo = if db == "memory" {
            RepoLocation::InMemory
        } else {
            RepoLocation::File(PathBuf::from(db))
        };
        let secret_path = std::env::var("RIOT_ANCHOR_SECRET_KEY_PATH")
            .map_err(|_| DaemonError::MissingEnv("RIOT_ANCHOR_SECRET_KEY_PATH"))?;
        let lease_ttl_secs = std::env::var("RIOT_ANCHOR_LEASE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
        let max_concurrent_sessions = std::env::var("RIOT_ANCHOR_MAX_SESSIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(crate::admission::DEFAULT_MAX_CONCURRENT_SESSIONS);
        Ok(Self {
            repo,
            secret_key: SecretKeySource::File(PathBuf::from(secret_path)),
            endpoint_binding: EndpointBinding::Public,
            lease_ttl_secs,
            max_concurrent_sessions,
        })
    }
}

// ---------------------------------------------------------------------------
// Errors.
// ---------------------------------------------------------------------------

/// An error starting or running the daemon. Display messages never embed a
/// secret path or key bytes.
#[derive(Debug)]
#[non_exhaustive]
pub enum DaemonError {
    /// A required environment variable was not set.
    MissingEnv(&'static str),
    /// The secret key could not be loaded or was not exactly 32 bytes. The
    /// message never includes the configured path.
    SecretKey(String),
    /// The durable repository failed to open, lease, or recover.
    Repository(AnchorRepositoryError),
    /// The control-plane codec failed while building startup state (the
    /// descriptor or limit profile).
    Control(String),
    /// Binding the iroh endpoint failed.
    Bind(String),
    /// The single-writer repo actor task panicked.
    ActorPanicked,
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEnv(name) => write!(f, "missing required environment variable {name}"),
            Self::SecretKey(message) => write!(f, "secret key: {message}"),
            Self::Repository(error) => write!(f, "anchor repository: {error}"),
            Self::Control(message) => write!(f, "control plane: {message}"),
            Self::Bind(message) => write!(f, "iroh endpoint bind: {message}"),
            Self::ActorPanicked => write!(f, "the single-writer repository actor task panicked"),
        }
    }
}

impl std::error::Error for DaemonError {}

impl From<AnchorRepositoryError> for DaemonError {
    fn from(error: AnchorRepositoryError) -> Self {
        Self::Repository(error)
    }
}

// ---------------------------------------------------------------------------
// Increment-1 default admission policy + operator signer.
// ---------------------------------------------------------------------------

/// A fail-closed default [`AdmissionPolicy`]. Increment 1 wires the runnable
/// control plane end-to-end, but real hosting authority/capacity checks (the
/// ticket/authority verification a `PrepareHost` must pass) are not yet
/// implemented here, so `PrepareHost` is refused outright rather than silently
/// admitting anything. `Describe` never consults this policy; `GetOperation`
/// never consults it either; `GetWorkChallenge` only needs a pressure band,
/// which this stub reports as "no work required".
#[derive(Debug, Clone, Copy, Default)]
pub struct StubAdmissionPolicy;

impl AdmissionPolicy for StubAdmissionPolicy {
    fn authorize_prepare_host(
        &self,
        _request: &PrepareHostV1,
        _observed_at: u64,
    ) -> Result<PreparePlan, ControlRefusal> {
        // No hosting authority is wired yet (increment 2+): fail closed rather
        // than admit against unverified authority.
        Err(ControlRefusal::InvalidTicketAuthority)
    }

    fn capacity_for_prepare_host(
        &self,
        _plan: &PreparePlan,
        _observed_at: u64,
    ) -> Result<(), ControlRefusal> {
        Ok(())
    }

    fn pressure_band(&self, _community_root: &[u8; 32], _observed_at: u64) -> PressurePolicy {
        PressurePolicy {
            policy_epoch: 0,
            difficulty: 0,
        }
    }
}

/// The production [`OperatorSigner`]: a real Ed25519 key derived from the
/// daemon's root secret (see [`derive32`]).
struct Ed25519OperatorSigner(SigningKey);

impl OperatorSigner for Ed25519OperatorSigner {
    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        self.0.sign(preimage).to_bytes()
    }
}

// ---------------------------------------------------------------------------
// Key material derivation.
// ---------------------------------------------------------------------------

const LABEL_IROH_SECRET: &[u8] = b"riot-anchor/daemon/iroh-endpoint-secret/v1";
const LABEL_OPERATOR_SEED: &[u8] = b"riot-anchor/daemon/operator-signing-seed/v1";
const LABEL_LEASE_HOLDER: &[u8] = b"riot-anchor/daemon/deployment-lease-holder/v1";
const LABEL_LEASE_TOKEN: &[u8] = b"riot-anchor/daemon/deployment-lease-token/v1";
const LABEL_GENESIS_RANDOM: &[u8] = b"riot-anchor/daemon/genesis-random/v1";
const LABEL_TOKEN_RING_SECRET: &[u8] = b"riot-anchor/daemon/namespace-token-ring-secret/v1";

/// Domain-separated SHA-256 derivation: `SHA256(label || root_secret)`. Keeps
/// every sub-key mathematically independent of the others even though they
/// all trace back to one loaded root secret.
fn derive32(root_secret: &[u8; 32], label: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(label);
    hasher.update(root_secret);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Load the root secret. Errors never embed the configured path.
fn load_secret(source: &SecretKeySource) -> Result<[u8; 32], DaemonError> {
    match source {
        SecretKeySource::Ephemeral(bytes) => Ok(*bytes),
        SecretKeySource::File(path) => {
            let bytes = std::fs::read(path)
                .map_err(|_| DaemonError::SecretKey("failed to read secret key file".into()))?;
            let array: [u8; 32] = bytes.try_into().map_err(|_| {
                DaemonError::SecretKey("secret key file must be exactly 32 bytes".into())
            })?;
            Ok(array)
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Build the signed descriptor envelope + operator coordinates the control
/// context needs. `iroh_endpoint_id` is the node id the daemon's endpoint will
/// bind under (computed from the same derived iroh secret, so it is stable
/// without actually opening a socket first).
struct OperatorIdentity {
    descriptor: DescriptorEnvelopeV1,
    anchor_id: [u8; 32],
    operator_key_id: [u8; 32],
    operator_public_key: [u8; 32],
    descriptor_digest: [u8; 32],
    limit_profile: AnchorLimitProfileV1,
}

fn build_operator_identity(
    operator_seed: &[u8; 32],
    genesis_random: [u8; 32],
    iroh_endpoint_id: [u8; 32],
    issued_at: u64,
) -> Result<OperatorIdentity, DaemonError> {
    let signing_key = SigningKey::from_bytes(operator_seed);
    let operator_public_key = signing_key.verifying_key().to_bytes();
    let current_key = OperatorVerificationKeyV1 {
        public_key: operator_public_key,
    };
    let operator_key_id = current_key
        .operator_key_id()
        .map_err(|error| DaemonError::Control(format!("operator key id: {error:?}")))?;
    let anchor_id = compute_anchor_id(&operator_public_key, &genesis_random);

    // Increment 1 stub: a single fixed MVP-default limit profile. Operator
    // tuning of per-class ceilings is future work.
    let limit_profile = AnchorLimitProfileV1::mvp_defaults(0);
    let limit_profile_digest = limit_profile
        .limit_profile_digest()
        .map_err(|error| DaemonError::Control(format!("limit profile digest: {error:?}")))?;

    let body = AnchorDescriptorBodyV1 {
        anchor_id,
        genesis_operator_public_key: operator_public_key,
        genesis_random_256_bits: genesis_random,
        current_operator_verification_key: current_key,
        current_operator_key_id: operator_key_id,
        descriptor_epoch: 0,
        previous_descriptor_digest: None,
        current_iroh_endpoint_id: iroh_endpoint_id,
        https_origin: "https://anchor.invalid".to_string(),
        operator_display_label: "riot-anchor (increment 1)".to_string(),
        self_reported_failure_domain_label: "unspecified".to_string(),
        supported_control_versions: vec![1],
        supported_sync_versions: vec![2],
        enabled_roles: vec![EnabledRole::Host],
        limit_profile_digest,
        predecessor_operator_verification_key: None,
        issued_at,
        // Increment 1 stub: no descriptor-rotation lifecycle yet, so a long
        // fixed validity window stands in for a real refresh policy.
        expires_at: issued_at.saturating_add(10 * 365 * 24 * 60 * 60),
    };
    let mut descriptor = DescriptorEnvelopeV1 {
        body,
        current_signature: [0u8; 64],
        predecessor_signature: None,
    };
    let preimage = descriptor
        .current_signing_preimage()
        .map_err(|error| DaemonError::Control(format!("descriptor preimage: {error:?}")))?;
    descriptor.current_signature = signing_key.sign(&preimage).to_bytes();
    let descriptor_digest = descriptor
        .descriptor_digest()
        .map_err(|error| DaemonError::Control(format!("descriptor digest: {error:?}")))?;

    Ok(OperatorIdentity {
        descriptor,
        anchor_id,
        operator_key_id,
        operator_public_key,
        descriptor_digest,
        limit_profile,
    })
}

// ---------------------------------------------------------------------------
// The single-writer repo actor.
// ---------------------------------------------------------------------------

/// The one message every accepted control connection sends the repo actor: the
/// raw request frame plus the wall-clock `now`, and a `oneshot` to carry the
/// reply back.
enum RepoCommand {
    HandleControl {
        request_bytes: Vec<u8>,
        now: u64,
        reply: oneshot::Sender<Result<ControlHandling, ControlError>>,
    },
}

/// The task that owns the [`AnchorRepository`] and [`AnchorControlService`]
/// for the daemon's entire lifetime. Every command is handled to completion
/// before the next is read off the channel, so `&mut AnchorRepository` is
/// never shared or interleaved across connections — the single-writer
/// discipline the design requires.
async fn run_repo_actor(
    mut repo: AnchorRepository,
    service: AnchorControlService<StubAdmissionPolicy, Ed25519OperatorSigner>,
    mut commands: mpsc::Receiver<RepoCommand>,
) {
    while let Some(command) = commands.recv().await {
        match command {
            RepoCommand::HandleControl {
                request_bytes,
                now,
                reply,
            } => {
                let mut entropy = || {
                    let mut buf = [0u8; 32];
                    getrandom::getrandom(&mut buf).expect("OS randomness source");
                    buf
                };
                let outcome = service.handle(&mut repo, &request_bytes, now, &mut entropy);
                // The receiver may have dropped (e.g. the connection reset
                // mid-flight); a failed send just means nobody is listening.
                let _ = reply.send(outcome);
            }
        }
    }
}

/// Build the per-connection control handler. It never touches the repository
/// directly: it reads exactly one bounded frame, hands the bytes to the repo
/// actor, and writes back exactly one bounded frame.
fn build_control_handler(repo_tx: mpsc::Sender<RepoCommand>) -> Handler {
    Arc::new(move |mut stream: BoundedStream, _exporter: Exporter| {
        let repo_tx = repo_tx.clone();
        Box::pin(async move {
            let request_bytes = stream.read_frame().await?;
            let now = unix_now();
            let (reply_tx, reply_rx) = oneshot::channel();
            repo_tx
                .send(RepoCommand::HandleControl {
                    request_bytes,
                    now,
                    reply: reply_tx,
                })
                .await
                .map_err(|_| {
                    TransportError::Io(std::io::Error::other("anchor control actor stopped"))
                })?;
            let outcome = reply_rx.await.map_err(|_| {
                TransportError::Io(std::io::Error::other("anchor control actor reply dropped"))
            })?;
            match outcome {
                Ok(ControlHandling::Responded(response)) => {
                    let bytes = response.encode_canonical().map_err(|error| {
                        TransportError::Io(std::io::Error::other(format!(
                            "encode control response: {error:?}"
                        )))
                    })?;
                    stream.write_frame(&bytes).await
                }
                Ok(ControlHandling::ProtocolFailure(failure)) => {
                    // Design "bounded protocol failure/close": end the stream
                    // with no result rather than writing a response frame.
                    Err(TransportError::Io(std::io::Error::other(format!(
                        "control protocol failure: {failure:?}"
                    ))))
                }
                Err(control_error) => Err(TransportError::Io(std::io::Error::other(
                    control_error.to_string(),
                ))),
            }
        }) as Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>>
    })
}

// ---------------------------------------------------------------------------
// The daemon.
// ---------------------------------------------------------------------------

async fn bind_endpoint(
    iroh_secret: [u8; 32],
    alpns: Vec<Vec<u8>>,
    binding: EndpointBinding,
) -> Result<Endpoint, DaemonError> {
    let secret_key = SecretKey::from_bytes(&iroh_secret);
    let result = match binding {
        EndpointBinding::Public => {
            Endpoint::builder(presets::N0)
                .secret_key(secret_key)
                .alpns(alpns)
                .bind()
                .await
        }
        EndpointBinding::LocalOnly => {
            Endpoint::builder(presets::N0DisableRelay)
                .secret_key(secret_key)
                .alpns(alpns)
                .bind()
                .await
        }
    };
    result.map_err(|error| DaemonError::Bind(error.to_string()))
}

/// A running (or about-to-run) anchor control-plane daemon: an open repository
/// (via its single-writer actor), a bound iroh endpoint advertising
/// `riot/anchor/1`, and the router that admits and dispatches to it.
pub struct Daemon {
    endpoint: Endpoint,
    router: AlpnRouter,
    repo_tx: mpsc::Sender<RepoCommand>,
    actor_handle: JoinHandle<()>,
    readiness_tx: watch::Sender<bool>,
    node_id: [u8; 32],
    anchor_id: [u8; 32],
}

impl Daemon {
    /// Assemble and bind the daemon: open the repository, acquire the
    /// single-writer deployment lease, recover startup readiness, build the
    /// real [`AnchorControlService`], spawn its owning actor task, and bind
    /// the public control endpoint. Returns the daemon plus a readiness
    /// watch — `true` once [`Daemon::run`] is actively serving.
    pub async fn start(config: DaemonConfig) -> Result<(Self, watch::Receiver<bool>), DaemonError> {
        let root_secret = load_secret(&config.secret_key)?;
        let iroh_secret = derive32(&root_secret, LABEL_IROH_SECRET);
        let operator_seed = derive32(&root_secret, LABEL_OPERATOR_SEED);
        let lease_holder_id = derive32(&root_secret, LABEL_LEASE_HOLDER);
        let lease_token = derive32(&root_secret, LABEL_LEASE_TOKEN);
        let genesis_random = derive32(&root_secret, LABEL_GENESIS_RANDOM);
        let token_ring_secret = derive32(&root_secret, LABEL_TOKEN_RING_SECRET);

        let now = unix_now();

        // 1. Open the repository and recover startup readiness BEFORE serving
        //    a single connection.
        let mut repo = match &config.repo {
            RepoLocation::File(path) => open_repo_file(path)?,
            RepoLocation::InMemory => AnchorRepository::open_in_memory()?,
        };
        repo.acquire_deployment_lease(&lease_holder_id, &lease_token, now, config.lease_ttl_secs)?;
        repo.recover_readiness(now)?;

        // 2. The iroh endpoint id is derived (not bound) up front so the
        //    descriptor can bind it before the socket actually opens.
        let iroh_public_key = SecretKey::from_bytes(&iroh_secret).public();
        let node_id = *iroh_public_key.as_bytes();

        let identity = build_operator_identity(&operator_seed, genesis_random, node_id, now)?;
        let context = AnchorControlContext {
            anchor_id: identity.anchor_id,
            operator_key_id: identity.operator_key_id,
            operator_public_key: identity.operator_public_key,
            descriptor_epoch: 0,
            descriptor_digest: identity.descriptor_digest,
            descriptor: identity.descriptor,
            limit_profile: identity.limit_profile,
            sync_version: 2,
            operation_lifetime_secs: 3600,
        };
        let service = AnchorControlService::new(
            context,
            StubAdmissionPolicy,
            Ed25519OperatorSigner(SigningKey::from_bytes(&operator_seed)),
            TokenSecretRing::new(1, token_ring_secret),
        );

        // 3. Register the control handler and bind the endpoint under the
        //    router's EXACT alpn set (never riot-transport's bind_public /
        //    bind_seed, which hardcode riot/sync/1).
        let (repo_tx, repo_rx) = mpsc::channel::<RepoCommand>(32);
        let mut router = bounded_router(config.max_concurrent_sessions);
        router.register(
            ALPN_ANCHOR_V1,
            Deadlines::control(),
            build_control_handler(repo_tx.clone()),
        );
        let endpoint = bind_endpoint(iroh_secret, router.alpns(), config.endpoint_binding).await?;

        let actor_handle = tokio::spawn(run_repo_actor(repo, service, repo_rx));
        let (readiness_tx, readiness_rx) = watch::channel(false);

        Ok((
            Self {
                endpoint,
                router,
                repo_tx,
                actor_handle,
                readiness_tx,
                node_id,
                anchor_id: identity.anchor_id,
            },
            readiness_rx,
        ))
    }

    /// This daemon's iroh node id (the identity a peer dials).
    #[must_use]
    pub fn node_id(&self) -> [u8; 32] {
        self.node_id
    }

    /// This daemon's stable anchor id (the descriptor's `anchor_id`).
    #[must_use]
    pub fn anchor_id(&self) -> [u8; 32] {
        self.anchor_id
    }

    /// This daemon's dialable address, once at least one direct address has
    /// been discovered (polls briefly — see
    /// `riot_transport::iroh::dialable_addr`).
    pub async fn dialable_addr(&self) -> EndpointAddr {
        riot_transport::iroh::dialable_addr(&self.endpoint).await
    }

    /// Serve accepted connections until `shutdown` reports `true`, then close
    /// the endpoint, stop the repo actor, and return. Marks the daemon ready
    /// (via the receiver [`Daemon::start`] returned) once it starts accepting.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) -> Result<(), DaemonError> {
        let Daemon {
            endpoint,
            router,
            repo_tx,
            actor_handle,
            readiness_tx,
            ..
        } = self;

        let _ = readiness_tx.send(true);

        loop {
            tokio::select! {
                biased;
                _ = shutdown.wait_for(|ready_to_stop| *ready_to_stop) => break,
                result = accept_with_router(&endpoint, &router) => {
                    if let Err(_failure) = result {
                        // A per-connection failure (unknown ALPN, permit
                        // exhaustion, a bounded-lifecycle timeout, a stream
                        // violation, a protocol failure) is bounded and closes
                        // only that connection; the daemon keeps serving.
                    }
                }
            }
        }

        let _ = readiness_tx.send(false);
        // Dropping every sender lets the actor's `recv()` return `None` and
        // exit cleanly. `router` also holds a clone (inside the handler
        // closure), so it must be dropped too before we await the actor.
        drop(repo_tx);
        drop(router);
        endpoint.close().await;
        actor_handle.await.map_err(|_| DaemonError::ActorPanicked)
    }
}

fn open_repo_file(path: &Path) -> Result<AnchorRepository, DaemonError> {
    Ok(AnchorRepository::open(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_key_source_debug_redacts_path_and_bytes() {
        let file_source = SecretKeySource::File(PathBuf::from("/very/secret/anchor.key"));
        let file_debug = format!("{file_source:?}");
        assert!(!file_debug.contains("/very/secret/anchor.key"));

        let ephemeral_source = SecretKeySource::Ephemeral([0x42u8; 32]);
        let ephemeral_debug = format!("{ephemeral_source:?}");
        let hex_bytes: String = [0x42u8; 32].iter().map(|b| format!("{b:02x}")).collect();
        assert!(!ephemeral_debug.contains(&hex_bytes));
    }

    #[test]
    fn daemon_config_debug_never_leaks_secret_path() {
        let config = DaemonConfig {
            repo: RepoLocation::InMemory,
            secret_key: SecretKeySource::File(PathBuf::from("/very/secret/anchor.key")),
            endpoint_binding: EndpointBinding::LocalOnly,
            lease_ttl_secs: 300,
            max_concurrent_sessions: 8,
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("/very/secret/anchor.key"));
    }

    #[test]
    fn derive32_is_deterministic_and_domain_separated() {
        let root = [9u8; 32];
        assert_eq!(
            derive32(&root, LABEL_IROH_SECRET),
            derive32(&root, LABEL_IROH_SECRET)
        );
        assert_ne!(
            derive32(&root, LABEL_IROH_SECRET),
            derive32(&root, LABEL_OPERATOR_SEED)
        );
    }

    #[test]
    fn load_secret_rejects_a_file_of_the_wrong_length() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "riot-anchor-daemon-test-badkey-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, [1u8; 4]).expect("write short key file");
        let result = load_secret(&SecretKeySource::File(path.clone()));
        let _ = std::fs::remove_file(&path);
        let error = result.expect_err("short key file must be rejected");
        let message = error.to_string();
        assert!(!message.contains(&path.display().to_string()));
    }
}
