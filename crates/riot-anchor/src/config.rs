//! Daemon configuration resolution and service assembly (WU-019 increment 1).
//!
//! This module holds the pure, testable configuration logic the `riot-anchor`
//! binary uses. Everything here takes plain inputs — an args slice and an
//! environment slice `&[(String, String)]` — rather than reading `std::env`
//! directly, so the binary shrinks to a thin shell (read env/argv → call
//! [`resolve_config`] → [`crate::daemon::run`], which loads the persisted
//! secrets and calls [`finalize_service`]) and the parsing/assembly can be
//! unit-tested without process globals.
//!
//! # Configuration surface
//!
//! * `--db <path>` (argv, required) — the SQLite database path.
//! * `RIOT_ANCHOR_OPERATOR_KEY_HEX` / `RIOT_ANCHOR_OPERATOR_KEY_FILE` (required) —
//!   the 32-byte (64 hex chars) Ed25519 operator SECRET seed. Secrets come from
//!   the environment or a file ONLY, never from argv.
//! * `RIOT_ANCHOR_ENDPOINT_KEY_HEX` / `RIOT_ANCHOR_ENDPOINT_KEY_FILE` (optional) —
//!   the 32-byte Ed25519 secret giving the public endpoint its stable NodeId. If
//!   absent, an ephemeral endpoint identity is generated for this run.
//! * `RIOT_ANCHOR_HTTPS_ORIGIN`, `RIOT_ANCHOR_DISPLAY_LABEL`,
//!   `RIOT_ANCHOR_FAILURE_DOMAIN` (optional) — descriptor metadata.
//! * `RIOT_ANCHOR_MAX_CONTROL_SESSIONS` (optional) — concurrency ceiling.
//!
//! The genesis random and the namespace-token secret are DATABASE-durable:
//! [`secret_proposals`] derives first-boot proposals from the operator secret,
//! the daemon persists them first-write-wins (`anchor_secrets`), and
//! [`finalize_service`] assembles the service from the persisted values — so an
//! operator-key rotation cannot silently change the anchor id or orphan
//! outstanding namespace tokens. DEFERRED: descriptor epoch rotation and
//! token-secret epoch rotation.

use std::path::PathBuf;

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

use riot_anchor_protocol::digest::anchor_id as compute_anchor_id;
use riot_anchor_protocol::records::{
    AnchorDescriptorBodyV1, AnchorLimitProfileV1, DescriptorEnvelopeV1, EnabledRole,
    OperatorVerificationKeyV1,
};

use crate::admission::IngressLimits;
use crate::control::{AnchorControlContext, AnchorControlService};
use crate::daemon::{unix_now, DaemonConfig, Ed25519OperatorSigner, TicketRootAuthorityPolicy};
use crate::work::TokenSecretRing;

/// The negotiated sync version this build serves.
pub const SYNC_VERSION: u64 = 2;
/// The prepared-operation lifetime (design: at most one hour).
pub const OPERATION_LIFETIME_SECS: u64 = 3600;
/// How long a freshly minted descriptor stays valid.
pub const DESCRIPTOR_VALID_SECS: u64 = 30 * 24 * 3600;
/// The deployment-lease term.
pub const LEASE_TTL_SECS: u64 = 300;
/// The retention horizon (seconds past commit) stamped into hosting receipts.
pub const REPORTED_RETENTION_SECS: u64 = 30 * 24 * 3600;
/// Operator warning emitted when this run minted a non-durable endpoint identity.
pub const EPHEMERAL_ENDPOINT_WARNING: &str =
    "no RIOT_ANCHOR_ENDPOINT_KEY set; using an EPHEMERAL endpoint identity";

/// The concrete control service the daemon assembles and runs.
pub type AnchorService = AnchorControlService<TicketRootAuthorityPolicy, Ed25519OperatorSigner>;

/// The daemon's resolved configuration.
pub struct Config {
    /// Path to the durable `AnchorRepository` SQLite database.
    db_path: PathBuf,
    /// The Ed25519 operator SECRET seed.
    operator_secret: [u8; 32],
    /// The Ed25519 secret giving the public endpoint its NodeId.
    endpoint_secret: [u8; 32],
    /// Whether this run minted the endpoint identity because none was configured.
    endpoint_identity_is_ephemeral: bool,
    /// Advertised HTTPS origin.
    https_origin: String,
    /// Operator display label.
    display_label: String,
    /// Self-reported failure-domain label.
    failure_domain: String,
    /// Bounded-ingress ceilings.
    ingress: IngressLimits,
}

impl core::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Config")
            .field("db_path", &self.db_path)
            .field("operator_secret", &"<redacted>")
            .field("endpoint_secret", &"<redacted>")
            .field(
                "endpoint_identity_is_ephemeral",
                &self.endpoint_identity_is_ephemeral,
            )
            .field("https_origin", &self.https_origin)
            .field("display_label", &self.display_label)
            .field("failure_domain", &self.failure_domain)
            .field("ingress", &self.ingress)
            .finish()
    }
}

impl Config {
    /// Return the startup warning required when the endpoint identity will
    /// change on restart.
    #[must_use]
    pub fn endpoint_identity_warning(&self) -> Option<&'static str> {
        self.endpoint_identity_is_ephemeral
            .then_some(EPHEMERAL_ENDPOINT_WARNING)
    }

    /// The SQLite database path this deployment is bound to (the daemon opens
    /// it to load the persisted secrets before assembling the service).
    #[must_use]
    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }
}

/// Resolve the daemon configuration from an args slice and an environment slice.
///
/// Pure with one exception: when no endpoint key is configured, a fresh
/// ephemeral endpoint secret is drawn from the OS CSPRNG (so the NodeId is not
/// stable across restarts).
pub fn resolve_config(args: &[String], env: &[(String, String)]) -> Result<Config, String> {
    let db_path = parse_db_arg(args)?;

    let operator_secret = load_secret(env, "RIOT_ANCHOR_OPERATOR_KEY")?.ok_or_else(|| {
        "missing operator key (set RIOT_ANCHOR_OPERATOR_KEY_HEX or _FILE)".to_string()
    })?;

    let (endpoint_secret, endpoint_identity_is_ephemeral) =
        match load_secret(env, "RIOT_ANCHOR_ENDPOINT_KEY")? {
            Some(secret) => (secret, false),
            None => {
                let mut secret = [0u8; 32];
                getrandom::getrandom(&mut secret)
                    .map_err(|error| format!("failed to mint ephemeral endpoint key: {error}"))?;
                (secret, true)
            }
        };

    let ingress = match env_get(env, "RIOT_ANCHOR_MAX_CONTROL_SESSIONS") {
        Some(value) => {
            let parsed = value.parse::<usize>().map_err(|_| {
                "RIOT_ANCHOR_MAX_CONTROL_SESSIONS must be a positive integer".to_string()
            })?;
            IngressLimits::new(parsed)
        }
        None => IngressLimits::default(),
    };

    Ok(Config {
        db_path,
        operator_secret,
        endpoint_secret,
        endpoint_identity_is_ephemeral,
        https_origin: env_or(env, "RIOT_ANCHOR_HTTPS_ORIGIN", "https://localhost"),
        display_label: env_or(env, "RIOT_ANCHOR_DISPLAY_LABEL", "Riot Anchor"),
        failure_domain: env_or(env, "RIOT_ANCHOR_FAILURE_DOMAIN", "unknown"),
        ingress,
    })
}

/// The database-durable anchor secrets: what [`finalize_service`] assembles the
/// service from. On a fresh database these are the [`secret_proposals`]
/// derivations; on every later boot they are whatever `anchor_secrets` already
/// holds (first write wins).
pub struct PersistedSecrets {
    /// The genesis random fixed at anchor genesis (feeds the anchor id).
    pub genesis_random: [u8; 32],
    /// The namespace-token secret (epoch 1) outstanding operations re-derive
    /// their tokens from.
    pub token_secret: [u8; 32],
}

/// Derive the first-boot secret PROPOSALS from the operator secret. These are
/// only proposals: the daemon persists them first-write-wins and the persisted
/// values (which a proposal can never displace) are what [`finalize_service`]
/// builds from.
#[must_use]
pub fn secret_proposals(config: &Config) -> PersistedSecrets {
    PersistedSecrets {
        genesis_random: derive(b"riot/anchor/genesis-random/v1", &config.operator_secret),
        token_secret: derive(b"riot/anchor/token-secret/v1", &config.operator_secret),
    }
}

/// Assemble the runnable [`DaemonConfig`] + control service from a [`Config`]
/// alone, using the derived [`secret_proposals`] directly. Bit-identical to the
/// fresh-database daemon path ([`secret_proposals`] → persist →
/// [`finalize_service`]); the daemon itself loads the persisted secrets first.
#[must_use]
pub fn assemble_service(config: Config) -> (DaemonConfig, AnchorService) {
    let proposals = secret_proposals(&config);
    finalize_service(config, proposals)
}

/// Assemble the runnable [`DaemonConfig`] + control service from a [`Config`]
/// and the database-durable [`PersistedSecrets`].
///
/// Builds the operator-signed descriptor/context (on the persisted genesis
/// random), the real Ed25519 signer, the canonical-gate admission policy, and
/// the namespace-token ring (on the persisted token secret) and deployment
/// token.
#[must_use]
pub fn finalize_service(
    config: Config,
    secrets: PersistedSecrets,
) -> (DaemonConfig, AnchorService) {
    let operator = SigningKey::from_bytes(&config.operator_secret);
    let context = build_control_context(&operator, &config, &secrets.genesis_random);
    let policy = TicketRootAuthorityPolicy::new(context.sync_version);
    let signer = Ed25519OperatorSigner::from_secret_bytes(config.operator_secret);

    // The namespace-token secret must be SECRET and stable across restarts (so a
    // prepared operation's tokens re-derive): the persisted, database-bound value.
    let mut service = AnchorControlService::new(
        context,
        policy,
        signer,
        TokenSecretRing::new(1, secrets.token_secret),
    );
    // Thread the deployment's receipt-retention horizon into the Commit service.
    service.set_reported_retention(REPORTED_RETENTION_SECS);

    let daemon_config = DaemonConfig {
        db_path: config.db_path,
        endpoint_secret_key: config.endpoint_secret,
        // PER-PROCESS lease identity: `holder_id` is a zero PLACEHOLDER here —
        // `daemon::serve` overwrites it with a fresh random draw from the
        // daemon's entropy at startup (its first entropy use). It must NOT be
        // derived from the operator secret: an operator-derived holder made a
        // second daemon started from the SAME config present identical
        // holder+token and renew the first daemon's lease in place — two live
        // writers forking one database. With a per-process holder the
        // double-start presents the same deployment token but a DIFFERENT
        // holder and is refused `LeaseHeld`, exiting fatally instead of
        // forking. The deployment token stays operator-derived — a secret,
        // stable value binding the database to this deployment/operator (a
        // different operator's token keeps failing `LeaseTokenMismatch`).
        holder_id: [0u8; 32],
        deployment_token: derive(b"riot/anchor/deployment-token/v1", &config.operator_secret),
        lease_ttl_secs: LEASE_TTL_SECS,
        ingress: config.ingress,
    };

    (daemon_config, service)
}

/// Build a real, operator-signed anchor descriptor + control context on the
/// given genesis random (fixed at anchor genesis; database-durable).
#[must_use]
pub fn build_control_context(
    operator: &SigningKey,
    config: &Config,
    genesis_random: &[u8; 32],
) -> AnchorControlContext {
    let operator_public = operator.verifying_key().to_bytes();
    let genesis_random = *genesis_random;
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

    let now = unix_now();
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
#[must_use]
pub fn derive(domain: &[u8], seed: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(seed);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// Find the value of `--db <path>` (or `--db=<path>`) in `args`.
pub fn parse_db_arg(args: &[String]) -> Result<PathBuf, String> {
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

/// The first value bound to `key` in the environment slice, if any.
fn env_get(env: &[(String, String)], key: &str) -> Option<String> {
    env.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
}

/// The value bound to `key`, or `default` when unset.
fn env_or(env: &[(String, String)], key: &str, default: &str) -> String {
    env_get(env, key).unwrap_or_else(|| default.to_string())
}

/// Load a 32-byte secret from `{PREFIX}_HEX` (64 hex chars) or `{PREFIX}_FILE`
/// (a file whose trimmed contents are 64 hex chars). Returns `Ok(None)` when
/// neither is set.
fn load_secret(env: &[(String, String)], prefix: &str) -> Result<Option<[u8; 32]>, String> {
    if let Some(hex) = env_get(env, &format!("{prefix}_HEX")) {
        return parse_hex32(hex.trim())
            .map(Some)
            .ok_or_else(|| format!("{prefix}_HEX must be 64 hex characters"));
    }
    if let Some(path) = env_get(env, &format!("{prefix}_FILE")) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    const HEX64: &str = "0101010101010101010101010101010101010101010101010101010101010101";

    #[test]
    fn config_debug_redacts_secret_material() {
        let config = Config {
            db_path: PathBuf::from("/tmp/redaction.db"),
            operator_secret: [201u8; 32],
            endpoint_secret: [202u8; 32],
            endpoint_identity_is_ephemeral: false,
            https_origin: "https://redaction.test".to_string(),
            display_label: "Redaction".to_string(),
            failure_domain: "test".to_string(),
            ingress: IngressLimits::default(),
        };

        let debug = format!("{config:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("201, 201"));
        assert!(!debug.contains("202, 202"));
    }

    #[test]
    fn parse_db_arg_space_and_equals_forms() {
        assert_eq!(
            parse_db_arg(&args(&["--db", "/tmp/a.db"])).unwrap(),
            PathBuf::from("/tmp/a.db")
        );
        assert_eq!(
            parse_db_arg(&args(&["--db=/tmp/b.db"])).unwrap(),
            PathBuf::from("/tmp/b.db")
        );
    }

    #[test]
    fn parse_db_arg_errors() {
        assert!(parse_db_arg(&args(&[])).unwrap_err().contains("missing"));
        assert!(parse_db_arg(&args(&["--db"]))
            .unwrap_err()
            .contains("requires a path"));
        assert!(parse_db_arg(&args(&["--nope"]))
            .unwrap_err()
            .contains("unexpected argument"));
    }

    #[test]
    fn parse_hex32_validates_length_and_alphabet() {
        assert_eq!(parse_hex32(HEX64), Some([1u8; 32]));
        assert_eq!(parse_hex32("00ff"), None); // too short
        assert_eq!(
            parse_hex32("zz01010101010101010101010101010101010101010101010101010101010101"),
            None
        ); // non-hex
           // Upper-case is accepted.
        assert_eq!(parse_hex32(&"AB".repeat(32)), Some([0xABu8; 32]));
    }

    #[test]
    fn resolve_config_full_success() {
        let config = resolve_config(
            &args(&["--db", "/tmp/anchor.db"]),
            &env(&[
                ("RIOT_ANCHOR_OPERATOR_KEY_HEX", HEX64),
                ("RIOT_ANCHOR_ENDPOINT_KEY_HEX", &"02".repeat(32)),
                ("RIOT_ANCHOR_HTTPS_ORIGIN", "https://anchor.test"),
                ("RIOT_ANCHOR_DISPLAY_LABEL", "Test Anchor"),
                ("RIOT_ANCHOR_FAILURE_DOMAIN", "eu-test"),
                ("RIOT_ANCHOR_MAX_CONTROL_SESSIONS", "12"),
            ]),
        )
        .unwrap();
        assert_eq!(config.db_path, PathBuf::from("/tmp/anchor.db"));
        assert_eq!(config.operator_secret, [1u8; 32]);
        assert_eq!(config.endpoint_secret, [2u8; 32]);
        assert_eq!(config.https_origin, "https://anchor.test");
        assert_eq!(config.display_label, "Test Anchor");
        assert_eq!(config.failure_domain, "eu-test");
        assert_eq!(config.ingress.max_concurrent_control_sessions, 12);
        assert_eq!(config.endpoint_identity_warning(), None);
    }

    #[test]
    fn resolve_config_defaults_and_ephemeral_endpoint() {
        // No endpoint key → an ephemeral one is minted; metadata falls back to
        // defaults; ingress default applies.
        let config = resolve_config(
            &args(&["--db=/tmp/x.db"]),
            &env(&[("RIOT_ANCHOR_OPERATOR_KEY_HEX", HEX64)]),
        )
        .unwrap();
        assert_eq!(config.https_origin, "https://localhost");
        assert_eq!(config.display_label, "Riot Anchor");
        assert_eq!(config.failure_domain, "unknown");
        assert_eq!(
            config.ingress.max_concurrent_control_sessions,
            IngressLimits::DEFAULT_MAX_CONTROL_SESSIONS
        );
        assert_ne!(
            config.endpoint_secret, [0u8; 32],
            "ephemeral key was minted"
        );
        assert_eq!(
            config.endpoint_identity_warning(),
            Some("no RIOT_ANCHOR_ENDPOINT_KEY set; using an EPHEMERAL endpoint identity")
        );
    }

    #[test]
    fn resolve_config_missing_operator_key_errors() {
        let error = resolve_config(&args(&["--db", "/tmp/x.db"]), &env(&[])).unwrap_err();
        assert!(error.contains("missing operator key"), "{error}");
    }

    #[test]
    fn resolve_config_bad_operator_hex_errors() {
        let error = resolve_config(
            &args(&["--db", "/tmp/x.db"]),
            &env(&[("RIOT_ANCHOR_OPERATOR_KEY_HEX", "deadbeef")]),
        )
        .unwrap_err();
        assert!(error.contains("64 hex"), "{error}");
    }

    #[test]
    fn resolve_config_bad_max_sessions_errors() {
        let error = resolve_config(
            &args(&["--db", "/tmp/x.db"]),
            &env(&[
                ("RIOT_ANCHOR_OPERATOR_KEY_HEX", HEX64),
                ("RIOT_ANCHOR_MAX_CONTROL_SESSIONS", "not-a-number"),
            ]),
        )
        .unwrap_err();
        assert!(error.contains("positive integer"), "{error}");
    }

    #[test]
    fn load_secret_from_file_path() {
        let mut path = std::env::temp_dir();
        path.push(format!("riot-anchor-key-{}.hex", std::process::id()));
        std::fs::write(&path, format!("  {HEX64}\n")).unwrap();
        let env = env(&[("RIOT_ANCHOR_OPERATOR_KEY_FILE", path.to_str().unwrap())]);
        let secret = load_secret(&env, "RIOT_ANCHOR_OPERATOR_KEY")
            .unwrap()
            .unwrap();
        assert_eq!(secret, [1u8; 32]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn derive_is_deterministic_and_domain_separated() {
        let seed = [9u8; 32];
        assert_eq!(derive(b"a", &seed), derive(b"a", &seed));
        assert_ne!(derive(b"a", &seed), derive(b"b", &seed));
        assert_ne!(derive(b"a", &seed), derive(b"a", &[8u8; 32]));
    }

    #[test]
    fn assemble_service_builds_coherent_context() {
        let config = Config {
            db_path: PathBuf::from("/tmp/y.db"),
            operator_secret: [3u8; 32],
            endpoint_secret: [4u8; 32],
            endpoint_identity_is_ephemeral: false,
            https_origin: "https://a".to_string(),
            display_label: "A".to_string(),
            failure_domain: "d".to_string(),
            ingress: IngressLimits::new(7),
        };
        let (daemon_config, _service) = assemble_service(config);
        assert_eq!(daemon_config.db_path, PathBuf::from("/tmp/y.db"));
        assert_eq!(daemon_config.ingress.max_concurrent_control_sessions, 7);
        // The holder id is a zero PLACEHOLDER — `daemon::serve` overwrites it
        // with a per-process random draw at startup. It must NOT be an
        // operator derivation (an operator-derived holder let a same-config
        // double-start renew the lease in place and fork the database). The
        // deployment token stays a deterministic operator-secret derivation
        // binding the database to the deployment.
        assert_eq!(daemon_config.holder_id, [0u8; 32]);
        assert_eq!(
            daemon_config.deployment_token,
            derive(b"riot/anchor/deployment-token/v1", &[3u8; 32])
        );
        assert_ne!(daemon_config.holder_id, daemon_config.deployment_token);
    }

    #[test]
    fn finalize_service_binds_the_persisted_secrets() {
        // Persisted values that DIFFER from what this operator secret derives:
        // the assembled service must be built from the persisted values, not
        // re-derive its own (anchor identity is bound to the database).
        let config = Config {
            db_path: PathBuf::from("/tmp/persisted.db"),
            operator_secret: [3u8; 32],
            endpoint_secret: [4u8; 32],
            endpoint_identity_is_ephemeral: false,
            https_origin: "https://a".to_string(),
            display_label: "A".to_string(),
            failure_domain: "d".to_string(),
            ingress: IngressLimits::default(),
        };
        let persisted = PersistedSecrets {
            genesis_random: [21u8; 32],
            token_secret: [22u8; 32],
        };
        let derived = secret_proposals(&config);
        assert_ne!(derived.genesis_random, persisted.genesis_random);
        assert_ne!(derived.token_secret, persisted.token_secret);

        let (_daemon_config, service) = finalize_service(config, persisted);
        assert_eq!(
            service.descriptor().body.genesis_random_256_bits,
            [21u8; 32],
            "the descriptor carries the persisted genesis random"
        );
        assert_eq!(
            service.descriptor().body.anchor_id,
            service.descriptor().body.recomputed_anchor_id(),
            "the anchor id is recomputed from the persisted genesis random"
        );
        assert_eq!(
            service.token_ring().secret(1),
            Some(&[22u8; 32]),
            "the token ring holds the persisted token secret"
        );
    }

    #[test]
    fn assemble_service_equals_finalize_with_derived_proposals() {
        // Fresh-database behavior is bit-identical: assembling directly and
        // finalizing with the derived proposals produce the same identity.
        let make_config = || Config {
            db_path: PathBuf::from("/tmp/fresh.db"),
            operator_secret: [3u8; 32],
            endpoint_secret: [4u8; 32],
            endpoint_identity_is_ephemeral: false,
            https_origin: "https://a".to_string(),
            display_label: "A".to_string(),
            failure_domain: "d".to_string(),
            ingress: IngressLimits::default(),
        };
        let (_, assembled) = assemble_service(make_config());
        let config = make_config();
        let proposals = secret_proposals(&config);
        let (_, finalized) = finalize_service(config, proposals);
        assert_eq!(
            assembled.descriptor().body.genesis_random_256_bits,
            finalized.descriptor().body.genesis_random_256_bits
        );
        assert_eq!(
            assembled.descriptor().body.anchor_id,
            finalized.descriptor().body.anchor_id
        );
        assert_eq!(
            assembled.token_ring().secret(1),
            finalized.token_ring().secret(1)
        );
    }

    #[test]
    fn build_control_context_shape() {
        let config = Config {
            db_path: PathBuf::from("/tmp/z.db"),
            operator_secret: [5u8; 32],
            endpoint_secret: [6u8; 32],
            endpoint_identity_is_ephemeral: false,
            https_origin: "https://origin".to_string(),
            display_label: "Label".to_string(),
            failure_domain: "fd".to_string(),
            ingress: IngressLimits::default(),
        };
        let operator = SigningKey::from_bytes(&config.operator_secret);
        let genesis_random = secret_proposals(&config).genesis_random;
        let context = build_control_context(&operator, &config, &genesis_random);
        assert_eq!(context.sync_version, SYNC_VERSION);
        assert_eq!(context.operation_lifetime_secs, OPERATION_LIFETIME_SECS);
        assert_eq!(
            context.operator_public_key,
            operator.verifying_key().to_bytes()
        );
        assert_eq!(context.descriptor.body.https_origin, "https://origin");
        assert_eq!(context.descriptor.body.operator_display_label, "Label");
        // The descriptor carries a real self-signature over its preimage.
        let preimage = context.descriptor.current_signing_preimage().unwrap();
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&context.operator_public_key).unwrap();
        let sig = ed25519_dalek::Signature::from_bytes(&context.descriptor.current_signature);
        assert!(vk.verify_strict(&preimage, &sig).is_ok());
    }
}
