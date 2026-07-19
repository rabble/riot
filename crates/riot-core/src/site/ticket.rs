//! Root-signed site ticket + the pre-connection transport gate (spec §5.1-5.3).
//!
//! The ticket carries the site's transport `require` floor SIGNED BY THE ROOT
//! KEY, so a client authenticates the floor BEFORE opening any connection — an
//! attacker cannot strip `require:arti`→`none` to leak a first-time follower's
//! IP over iroh. [`admit_dial`] is the fail-closed gate: it verifies the
//! signature, checks freshness/rollback, and refuses to dial when the floor
//! names a transport the client cannot provide. Pure sync crypto — no async,
//! no network — so it is exhaustively testable at the boundary.
//!
//! Lives in riot-core (protocol logic; pure ed25519, no iroh/tokio) so the FFI
//! can parse+verify a ticket without pulling in the transport crate. Re-exported
//! from `riot-transport` for the transport callers.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

const TICKET_DOMAIN: &[u8] = b"riot/site-ticket/v1";

/// The transport floor a site demands. Open enum: an unrecognized token is
/// [`Floor::Unknown`] and fails closed — never silently parsed as `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Floor {
    /// No floor — iroh (or any channel) is acceptable.
    None,
    /// Onion only — must dial over arti/Tor; never fall back to iroh.
    Arti,
    /// An unrecognized `require` token. Fails closed.
    Unknown(String),
}

impl Floor {
    /// Parse a `require` floor token. An unrecognized token is `Unknown` (fails
    /// closed), never silently `None`.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "none" => Floor::None,
            "arti" => Floor::Arti,
            other => Floor::Unknown(other.to_string()),
        }
    }
}

/// What transports this client can actually provide right now.
#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    pub iroh: bool,
    pub arti: bool,
}

/// Why a dial was refused before any connection was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportBlocked {
    /// The root signature over the floor did not verify. Never dial.
    BadSignature,
    /// The ticket's expiry has passed (replay window closed).
    Expired,
    /// The ticket's epoch is below the durable per-site floor (downgrade).
    Rollback,
    /// The floor requires a transport this client cannot provide (fail closed).
    RequiresUnavailableTransport(String),
    /// An unrecognized floor token (fail closed).
    UnknownFloor(String),
}

impl std::fmt::Display for TransportBlocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadSignature => write!(f, "ticket root signature did not verify"),
            Self::Expired => write!(f, "ticket expired"),
            Self::Rollback => write!(f, "ticket epoch below durable floor (downgrade)"),
            Self::RequiresUnavailableTransport(t) => {
                write!(f, "site requires {t}, unavailable in this build")
            }
            Self::UnknownFloor(t) => write!(f, "unknown transport floor {t:?}"),
        }
    }
}

impl std::error::Error for TransportBlocked {}

/// A parsed site ticket. The `require` floor is authenticated by `sig` over the
/// canonical payload; nothing here is trusted until [`Ticket::verify`] passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub root: [u8; 32],
    pub namespace: [u8; 32],
    pub require_raw: String,
    pub epoch: u64,
    pub exp: u64,
    pub digest: [u8; 32],
    /// Untrusted seeding hint (iroh node addr) — never site identity, never
    /// consulted before the fail-closed decision. NOT covered by the signature.
    pub node: Option<String>,
    /// The HTTPS bundle URL the phone pulls the owner-signed site bundle from
    /// (Option C HTTP-pull). SIGNED (in the canonical payload), so a tampered or
    /// stripped url breaks the signature — but it is only a fetch HINT, never a
    /// gate: a wrong url is a liveness failure, and the fetched bytes are
    /// re-verified by `import_followed_site_bundle`. Absent on a pre-url or
    /// iroh-only ticket (`None` → the phone has nothing to auto-pull).
    pub url: Option<String>,
    pub sig: [u8; 64],
}

/// The canonical bytes the root signs: domain-separated, length-framed, and
/// including the RAW `require` string so any floor token round-trips.
///
/// UNAMBIGUOUS by construction: every variable-length field is length-framed and
/// the field order is fixed, so no two distinct field-sets can produce the same
/// bytes. Optional fields (currently `url`) append at the END, in a FIXED order,
/// each length-framed, and ONLY when present — so a ticket minted before `url`
/// existed (`None`) has a canonical byte-identical to today (backward-compat),
/// while a present `url` is covered by the signature (and cannot be stripped or
/// forged without the root key).
fn canonical(
    root: &[u8; 32],
    namespace: &[u8; 32],
    require_raw: &str,
    epoch: u64,
    exp: u64,
    digest: &[u8; 32],
    url: Option<&str>,
) -> Vec<u8> {
    let mut m =
        Vec::with_capacity(TICKET_DOMAIN.len() + 32 + 32 + 4 + require_raw.len() + 8 + 8 + 32);
    m.extend_from_slice(TICKET_DOMAIN);
    m.extend_from_slice(root);
    m.extend_from_slice(namespace);
    m.extend_from_slice(&(require_raw.len() as u32).to_be_bytes());
    m.extend_from_slice(require_raw.as_bytes());
    m.extend_from_slice(&epoch.to_be_bytes());
    m.extend_from_slice(&exp.to_be_bytes());
    m.extend_from_slice(digest);
    // Fixed-order optional-field tail (append future signed optionals HERE, in a
    // stable order, each length-framed). `None` appends nothing.
    if let Some(url) = url {
        m.extend_from_slice(&(url.len() as u32).to_be_bytes());
        m.extend_from_slice(url.as_bytes());
    }
    m
}

impl Ticket {
    pub fn floor(&self) -> Floor {
        Floor::parse(&self.require_raw)
    }

    /// Verify the root signature over the canonical floor payload. `root` is the
    /// site identity (the O-namespace owner key); the signature must be by it.
    pub fn verify(&self) -> bool {
        let Ok(key) = VerifyingKey::from_bytes(&self.root) else {
            return false;
        };
        let sig = Signature::from_bytes(&self.sig);
        let msg = canonical(
            &self.root,
            &self.namespace,
            &self.require_raw,
            self.epoch,
            self.exp,
            &self.digest,
            self.url.as_deref(),
        );
        key.verify(&msg, &sig).is_ok()
    }

    /// Encode as a `riot://site/v1/...` share URI.
    pub fn encode(&self) -> String {
        let mut s = format!(
            "riot://site/v1/{}?root={}&require={}&epoch={}&exp={}&digest={}",
            hex(&self.namespace),
            hex(&self.root),
            self.require_raw,
            self.epoch,
            self.exp,
            hex(&self.digest),
        );
        if let Some(node) = &self.node {
            s.push_str("&node=");
            s.push_str(node);
        }
        if let Some(url) = &self.url {
            s.push_str("&url=");
            s.push_str(url);
        }
        s.push_str("&sig=");
        s.push_str(&hex(&self.sig));
        s
    }
}

/// Mint a ticket: the owner signs the floor with the root key. Used at site
/// creation and whenever the owner tightens `require` (bumping `epoch`).
#[allow(clippy::too_many_arguments)]
pub fn mint(
    root_signing_key: &ed25519_dalek::SigningKey,
    namespace: [u8; 32],
    require_raw: &str,
    epoch: u64,
    exp: u64,
    digest: [u8; 32],
    node: Option<String>,
    url: Option<String>,
) -> Ticket {
    use ed25519_dalek::Signer;
    let root = root_signing_key.verifying_key().to_bytes();
    let msg = canonical(
        &root,
        &namespace,
        require_raw,
        epoch,
        exp,
        &digest,
        url.as_deref(),
    );
    let sig = root_signing_key.sign(&msg).to_bytes();
    Ticket {
        root,
        namespace,
        require_raw: require_raw.to_string(),
        epoch,
        exp,
        digest,
        node,
        url,
        sig,
    }
}

/// Parse a `riot://site/v1/<namespace>?...` ticket URI. Structural only — the
/// signature is NOT checked here; callers must run [`admit_dial`] before dialing.
pub fn parse(uri: &str) -> Result<Ticket, TicketParseError> {
    let rest = uri
        .strip_prefix("riot://site/v1/")
        .ok_or(TicketParseError::BadScheme)?;
    let (ns_hex, query) = rest.split_once('?').ok_or(TicketParseError::MissingQuery)?;
    let namespace = hex32(ns_hex).ok_or(TicketParseError::BadField("namespace"))?;

    let mut root = None;
    let mut require_raw = None;
    let mut epoch = None;
    let mut exp = None;
    let mut digest = None;
    let mut node = None;
    let mut url = None;
    let mut sig = None;
    for pair in query.split('&') {
        let (k, v) = pair
            .split_once('=')
            .ok_or(TicketParseError::BadField("pair"))?;
        match k {
            "root" => root = Some(hex32(v).ok_or(TicketParseError::BadField("root"))?),
            "require" => require_raw = Some(v.to_string()),
            "epoch" => epoch = Some(v.parse().map_err(|_| TicketParseError::BadField("epoch"))?),
            "exp" => exp = Some(v.parse().map_err(|_| TicketParseError::BadField("exp"))?),
            "digest" => digest = Some(hex32(v).ok_or(TicketParseError::BadField("digest"))?),
            "node" => node = Some(v.to_string()),
            "url" => url = Some(v.to_string()),
            "sig" => sig = Some(hex64(v).ok_or(TicketParseError::BadField("sig"))?),
            _ => {} // ignore unknown params (forward-compat), never affects the floor
        }
    }
    Ok(Ticket {
        root: root.ok_or(TicketParseError::BadField("root"))?,
        namespace,
        require_raw: require_raw.ok_or(TicketParseError::BadField("require"))?,
        epoch: epoch.ok_or(TicketParseError::BadField("epoch"))?,
        exp: exp.ok_or(TicketParseError::BadField("exp"))?,
        digest: digest.ok_or(TicketParseError::BadField("digest"))?,
        node,
        url,
        sig: sig.ok_or(TicketParseError::BadField("sig"))?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TicketParseError {
    BadScheme,
    MissingQuery,
    BadField(&'static str),
}

/// The pre-connection fail-closed gate (§5.1-5.3). Returns `Ok(())` ONLY when it
/// is safe to dial with the current capabilities; every failure mode refuses to
/// dial rather than downgrade. `durable_epoch_floor` is the highest epoch this
/// client has durably seen for the site (0 for a first-time follow).
pub fn admit_dial(
    ticket: &Ticket,
    caps: &Capabilities,
    now_unix: u64,
    durable_epoch_floor: u64,
) -> Result<(), TransportBlocked> {
    // 1. Authenticate the floor BEFORE anything else — no dial on a bad sig.
    if !ticket.verify() {
        return Err(TransportBlocked::BadSignature);
    }
    // 2. Freshness: expired ticket, or an epoch below the durable floor (a
    //    downgrade against a returning follower) — refuse.
    if now_unix > ticket.exp {
        return Err(TransportBlocked::Expired);
    }
    if ticket.epoch < durable_epoch_floor {
        return Err(TransportBlocked::Rollback);
    }
    // 3. Fail closed on the authenticated floor.
    match ticket.floor() {
        Floor::Unknown(t) => Err(TransportBlocked::UnknownFloor(t)),
        Floor::Arti if !caps.arti => Err(TransportBlocked::RequiresUnavailableTransport(
            "arti".into(),
        )),
        Floor::None if !caps.iroh => Err(TransportBlocked::RequiresUnavailableTransport(
            "iroh".into(),
        )),
        Floor::Arti | Floor::None => Ok(()),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    hex_decode(s)?.try_into().ok()
}

fn hex64(s: &str) -> Option<[u8; 64]> {
    hex_decode(s)?.try_into().ok()
}
