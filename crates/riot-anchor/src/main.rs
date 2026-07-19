//! The `riot-anchor` control-plane daemon binary (WU-019 increment 1).
//!
//! Boots the anchor control plane: opens the durable `AnchorRepository`, acquires
//! the single-writer deployment lease, binds a PUBLIC iroh endpoint, and serves
//! the `riot/anchor/1` control ALPN (community hosting admission). The
//! `riot/sync/2` DATA path is a separate later increment and is NOT served here.
//!
//! # Configuration
//!
//! * `--db <path>` (argv, required) — the SQLite database path.
//! * `RIOT_ANCHOR_OPERATOR_KEY_HEX` / `RIOT_ANCHOR_OPERATOR_KEY_FILE` (required) —
//!   the 32-byte (64 hex chars) Ed25519 operator SECRET seed. Secrets are read
//!   from the environment or a file ONLY, never from argv.
//! * `RIOT_ANCHOR_ENDPOINT_KEY_HEX` / `RIOT_ANCHOR_ENDPOINT_KEY_FILE` (optional) —
//!   the 32-byte Ed25519 secret giving the public endpoint its stable NodeId. If
//!   absent, an ephemeral endpoint identity is generated for this run.
//! * `RIOT_ANCHOR_HTTPS_ORIGIN`, `RIOT_ANCHOR_DISPLAY_LABEL`,
//!   `RIOT_ANCHOR_FAILURE_DOMAIN` (optional) — descriptor metadata.
//! * `RIOT_ANCHOR_MAX_CONTROL_SESSIONS` (optional) — concurrency ceiling.
//!
//! DEFERRED(WU-019 increment 2): durable persistence of the genesis random, the
//! namespace-token secret, and the deployment/endpoint identities (increment 1
//! derives the stable ones deterministically from the operator secret so a
//! restart is coherent; a dedicated persisted-genesis store lands later), plus
//! descriptor epoch rotation.

use std::path::PathBuf;
use std::process::ExitCode;

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

use riot_anchor::admission::IngressLimits;
use riot_anchor::control::{AnchorControlContext, AnchorControlService};
use riot_anchor::daemon::{self, DaemonConfig, Ed25519OperatorSigner, TicketRootAuthorityPolicy};
use riot_anchor::work::TokenSecretRing;
use riot_anchor_protocol::digest::anchor_id as compute_anchor_id;
use riot_anchor_protocol::records::{
    AnchorDescriptorBodyV1, AnchorLimitProfileV1, DescriptorEnvelopeV1, EnabledRole,
    OperatorVerificationKeyV1,
};

const SYNC_VERSION: u64 = 2;
const OPERATION_LIFETIME_SECS: u64 = 3600;
const DESCRIPTOR_VALID_SECS: u64 = 30 * 24 * 3600;
const LEASE_TTL_SECS: u64 = 300;

fn main() -> ExitCode {
    let config = match Config::from_env_and_args() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("riot-anchor: {message}");
            eprintln!(
                "usage: riot-anchor --db <path>  \
                 (operator key via RIOT_ANCHOR_OPERATOR_KEY_HEX or _FILE)"
            );
            return ExitCode::from(2);
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("riot-anchor: failed to start tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(serve(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("riot-anchor: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve(config: Config) -> Result<(), daemon::DaemonError> {
    let operator = SigningKey::from_bytes(&config.operator_secret);
    let context = build_control_context(&operator, &config);
    let policy = TicketRootAuthorityPolicy::new(context.sync_version);
    let signer = Ed25519OperatorSigner::from_secret_bytes(config.operator_secret);

    // The namespace-token secret must be SECRET and stable across restarts (so a
    // prepared operation's tokens re-derive). Increment 1 derives it from the
    // operator SECRET; DEFERRED(WU-019 increment 2): a dedicated persisted random
    // secret with rotation.
    let token_secret = derive(b"riot/anchor/token-secret/v1", &config.operator_secret);
    let service = AnchorControlService::new(
        context,
        policy,
        signer,
        TokenSecretRing::new(1, token_secret),
    );

    let daemon_config = DaemonConfig {
        db_path: config.db_path,
        endpoint_secret_key: config.endpoint_secret,
        // The lease holder id is the operator identity; the deployment token is a
        // secret, stable value bound to this operator's database.
        holder_id: operator.verifying_key().to_bytes(),
        deployment_token: derive(b"riot/anchor/deployment-token/v1", &config.operator_secret),
        lease_ttl_secs: LEASE_TTL_SECS,
        ingress: config.ingress,
    };

    let shutdown = async {
        // Serve until SIGINT (Ctrl-C); on any signal error, run indefinitely.
        let _ = tokio::signal::ctrl_c().await;
    };

    daemon::run(daemon_config, service, daemon::os_entropy(), shutdown).await
}

/// Build a real, operator-signed anchor descriptor + control context.
fn build_control_context(operator: &SigningKey, config: &Config) -> AnchorControlContext {
    let operator_public = operator.verifying_key().to_bytes();
    // DEFERRED(WU-019 increment 2): the genesis random is fixed at anchor genesis
    // and persisted. Increment 1 derives a stable value from the operator secret.
    let genesis_random = derive(b"riot/anchor/genesis-random/v1", &config.operator_secret);
    let anchor_id = compute_anchor_id(&operator_public, &genesis_random);
    let current_key = OperatorVerificationKeyV1 {
        public_key: operator_public,
    };
    let operator_key_id = current_key
        .operator_key_id()
        .expect("operator key id encodes");

    // The endpoint NodeId is the Ed25519 public key of the endpoint secret.
    let endpoint_node_id = SigningKey::from_bytes(&config.endpoint_secret)
        .verifying_key()
        .to_bytes();

    let limit_profile = AnchorLimitProfileV1::mvp_defaults(0);
    let limit_profile_digest = limit_profile
        .limit_profile_digest()
        .expect("limit profile digests");

    let now = daemon::unix_now();
    let body = AnchorDescriptorBodyV1 {
        anchor_id,
        genesis_operator_public_key: operator_public,
        genesis_random_256_bits: genesis_random,
        current_operator_verification_key: current_key,
        current_operator_key_id: operator_key_id,
        descriptor_epoch: 0,
        previous_descriptor_digest: None,
        current_iroh_endpoint_id: endpoint_node_id,
        https_origin: config.https_origin.clone(),
        operator_display_label: config.display_label.clone(),
        self_reported_failure_domain_label: config.failure_domain.clone(),
        supported_control_versions: vec![1],
        supported_sync_versions: vec![1, SYNC_VERSION],
        enabled_roles: vec![EnabledRole::Host],
        limit_profile_digest,
        predecessor_operator_verification_key: None,
        issued_at: now,
        expires_at: now.saturating_add(DESCRIPTOR_VALID_SECS),
    };
    let mut descriptor = DescriptorEnvelopeV1 {
        body,
        current_signature: [0u8; 64],
        predecessor_signature: None,
    };
    let preimage = descriptor
        .current_signing_preimage()
        .expect("descriptor signing preimage encodes");
    descriptor.current_signature = operator.sign(&preimage).to_bytes();
    let descriptor_digest = descriptor.descriptor_digest().expect("descriptor digests");

    AnchorControlContext {
        anchor_id,
        operator_key_id,
        operator_public_key: operator_public,
        descriptor_epoch: 0,
        descriptor_digest,
        descriptor,
        limit_profile,
        sync_version: SYNC_VERSION,
        operation_lifetime_secs: OPERATION_LIFETIME_SECS,
    }
}

/// A deterministic 32-byte value: `SHA-256(domain || seed)`.
fn derive(domain: &[u8], seed: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(seed);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// The daemon's resolved configuration.
struct Config {
    db_path: PathBuf,
    operator_secret: [u8; 32],
    endpoint_secret: [u8; 32],
    https_origin: String,
    display_label: String,
    failure_domain: String,
    ingress: IngressLimits,
}

impl Config {
    fn from_env_and_args() -> Result<Self, String> {
        let db_path = parse_db_arg()?;

        let operator_secret = load_secret("RIOT_ANCHOR_OPERATOR_KEY")?.ok_or_else(|| {
            "missing operator key (set RIOT_ANCHOR_OPERATOR_KEY_HEX or _FILE)".to_string()
        })?;

        // The endpoint identity is optional; without it we mint an ephemeral one
        // for this run (the NodeId will not be stable across restarts).
        let endpoint_secret = match load_secret("RIOT_ANCHOR_ENDPOINT_KEY")? {
            Some(secret) => secret,
            None => {
                let mut secret = [0u8; 32];
                getrandom::getrandom(&mut secret)
                    .map_err(|error| format!("failed to mint ephemeral endpoint key: {error}"))?;
                eprintln!(
                    "riot-anchor: no RIOT_ANCHOR_ENDPOINT_KEY set; using an EPHEMERAL endpoint identity"
                );
                secret
            }
        };

        let ingress = match std::env::var("RIOT_ANCHOR_MAX_CONTROL_SESSIONS") {
            Ok(value) => {
                let parsed = value.parse::<usize>().map_err(|_| {
                    "RIOT_ANCHOR_MAX_CONTROL_SESSIONS must be a positive integer".to_string()
                })?;
                IngressLimits::new(parsed)
            }
            Err(_) => IngressLimits::default(),
        };

        Ok(Self {
            db_path,
            operator_secret,
            endpoint_secret,
            https_origin: env_or("RIOT_ANCHOR_HTTPS_ORIGIN", "https://localhost"),
            display_label: env_or("RIOT_ANCHOR_DISPLAY_LABEL", "Riot Anchor"),
            failure_domain: env_or("RIOT_ANCHOR_FAILURE_DOMAIN", "unknown"),
            ingress,
        })
    }
}

fn parse_db_arg() -> Result<PathBuf, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut db_path: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--db" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| "--db requires a path".to_string())?;
            db_path = Some(PathBuf::from(value));
        } else if let Some(rest) = arg.strip_prefix("--db=") {
            db_path = Some(PathBuf::from(rest));
        } else {
            return Err(format!("unexpected argument: {arg}"));
        }
        index += 1;
    }
    db_path.ok_or_else(|| "missing required --db <path>".to_string())
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Load a 32-byte secret from `{PREFIX}_HEX` (64 hex chars) or `{PREFIX}_FILE`
/// (a file whose trimmed contents are 64 hex chars). Returns `Ok(None)` when
/// neither is set.
fn load_secret(prefix: &str) -> Result<Option<[u8; 32]>, String> {
    if let Ok(hex) = std::env::var(format!("{prefix}_HEX")) {
        return parse_hex32(hex.trim())
            .map(Some)
            .ok_or_else(|| format!("{prefix}_HEX must be 64 hex characters"));
    }
    if let Ok(path) = std::env::var(format!("{prefix}_FILE")) {
        let contents = std::fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {prefix}_FILE ({path}): {error}"))?;
        return parse_hex32(contents.trim())
            .map(Some)
            .ok_or_else(|| format!("{prefix}_FILE must contain 64 hex characters"));
    }
    Ok(None)
}

/// Parse exactly 64 hex characters into 32 bytes.
fn parse_hex32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = hex.as_bytes();
    for (index, slot) in out.iter_mut().enumerate() {
        let high = hex_nibble(bytes[index * 2])?;
        let low = hex_nibble(bytes[index * 2 + 1])?;
        *slot = (high << 4) | low;
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
