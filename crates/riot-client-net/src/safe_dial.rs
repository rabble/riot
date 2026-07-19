//! WU-011B — verified bootstrap and SSRF-guarded safe dialing.
//!
//! This module implements the two halves of the anchor bootstrap dial, both over
//! WU-011A's [`ProfileLease`](crate::runtime::ProfileLease):
//!
//! 1. **Ticket-first verify-before-dial.** [`verify_bootstrap`] admits a
//!    root-signed [`RootSignedTicketCoreEnvelopeV2`] through the SECURITY-CRITICAL
//!    [`admit_public_site_ticket`] (root signature + `require:none` transport
//!    equality) BEFORE any iroh or HTTPS dial is even attempted, and refuses any
//!    endpoint/origin hint that points somewhere other than the verified
//!    descriptor identity — a hint can never redirect the client. [`bootstrap_over_https`]
//!    composes the ticket/hint check with the safe dial so that *no* resolution or
//!    connection happens if verification fails.
//!
//! 2. **SSRF/rebinding-guarded HTTPS.** [`safe_https_get`] resolves the origin
//!    host exactly once through an injected [`SafeResolver`], refuses any
//!    non-globally-routable address (private/loopback/link-local/multicast/
//!    reserved/unspecified), pins that validated address, and dials it through an
//!    injected [`SafeConnector`] on port 443 only. It refuses redirects, requires
//!    SNI/certificate validation, bounds the response, and rejects DNS rebinding
//!    (the connection's actual peer must equal the pinned, validated address).
//!
//! The resolver and connector are trait seams precisely so every rule is testable
//! with fakes and no real DNS, TLS, or network.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use riot_anchor_protocol::{
    admit_public_site_ticket, AdmittedTicket, AnchorDescriptorBodyV1, AuthorityError,
    RootSignedTicketCoreEnvelopeV2, TicketFloor, TransportFloor,
};

use crate::runtime::ProfileLease;

/// Anchor HTTPS origins are dialed on port 443 only.
const HTTPS_PORT: u16 = 443;
/// A descriptor HTTPS origin is at most 255 UTF-8 bytes (design floor).
const MAX_ORIGIN_BYTES: usize = 255;

// ===========================================================================
// SSRF address classification.
// ===========================================================================

/// The exact reason a candidate address is not a permitted public dial target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpRejection {
    /// The unspecified address (`0.0.0.0` / `::`).
    Unspecified,
    /// A loopback address (`127.0.0.0/8` / `::1`).
    Loopback,
    /// An RFC1918 / unique-local private address.
    Private,
    /// A link-local address (`169.254.0.0/16` / `fe80::/10`).
    LinkLocal,
    /// A multicast address.
    Multicast,
    /// Any other non-globally-routable / reserved range (broadcast, CGN/shared,
    /// benchmarking, documentation, `240.0.0.0/4`, `0.0.0.0/8`, …).
    Reserved,
}

impl fmt::Display for IpRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            IpRejection::Unspecified => "unspecified",
            IpRejection::Loopback => "loopback",
            IpRejection::Private => "private",
            IpRejection::LinkLocal => "link-local",
            IpRejection::Multicast => "multicast",
            IpRejection::Reserved => "reserved",
        };
        f.write_str(s)
    }
}

/// Accept only globally routable public unicast addresses; classify every other
/// address with its precise [`IpRejection`]. This is the SSRF guard: the dialed
/// address is always one that passes here.
pub fn classify_public_ip(ip: IpAddr) -> Result<(), IpRejection> {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            // An IPv4-mapped v6 address (`::ffff:a.b.c.d`) is really its v4 target.
            Some(v4) => classify_v4(v4),
            None => classify_v6(v6),
        },
    }
}

fn classify_v4(ip: Ipv4Addr) -> Result<(), IpRejection> {
    if ip.is_unspecified() {
        return Err(IpRejection::Unspecified);
    }
    if ip.is_loopback() {
        return Err(IpRejection::Loopback);
    }
    if ip.is_broadcast() {
        return Err(IpRejection::Reserved);
    }
    if ip.is_private() {
        return Err(IpRejection::Private);
    }
    if ip.is_link_local() {
        return Err(IpRejection::LinkLocal);
    }
    if ip.is_multicast() {
        return Err(IpRejection::Multicast);
    }
    if ip.is_documentation() {
        return Err(IpRejection::Reserved);
    }
    let o = ip.octets();
    // `0.0.0.0/8` "this network".
    if o[0] == 0 {
        return Err(IpRejection::Reserved);
    }
    // Shared address space / carrier-grade NAT `100.64.0.0/10`.
    if o[0] == 100 && (o[1] & 0xc0) == 0x40 {
        return Err(IpRejection::Reserved);
    }
    // Benchmarking `198.18.0.0/15`.
    if o[0] == 198 && (o[1] & 0xfe) == 18 {
        return Err(IpRejection::Reserved);
    }
    // Reserved / future use `240.0.0.0/4`.
    if o[0] >= 240 {
        return Err(IpRejection::Reserved);
    }
    Ok(())
}

fn classify_v6(ip: Ipv6Addr) -> Result<(), IpRejection> {
    if ip.is_unspecified() {
        return Err(IpRejection::Unspecified);
    }
    if ip.is_loopback() {
        return Err(IpRejection::Loopback);
    }
    if ip.is_multicast() {
        return Err(IpRejection::Multicast);
    }
    let seg = ip.segments();
    // Link-local unicast `fe80::/10`.
    if (seg[0] & 0xffc0) == 0xfe80 {
        return Err(IpRejection::LinkLocal);
    }
    // Unique-local `fc00::/7`.
    if (seg[0] & 0xfe00) == 0xfc00 {
        return Err(IpRejection::Private);
    }
    // Documentation `2001:db8::/32`.
    if seg[0] == 0x2001 && seg[1] == 0x0db8 {
        return Err(IpRejection::Reserved);
    }
    Ok(())
}

// ===========================================================================
// Safe-dial errors + resolver/connector seam.
// ===========================================================================

/// Every way a safe HTTPS dial can be refused. Each variant names the exact rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeDialError {
    /// The origin is not an `https://` origin.
    NotHttps,
    /// The origin's host component is empty.
    EmptyHost,
    /// The origin exceeds the 255-byte descriptor floor.
    OriginTooLong,
    /// The origin is malformed (e.g. a non-numeric or bracketless authority).
    MalformedOrigin,
    /// The origin names a port other than 443.
    NonStandardPort(u16),
    /// Name resolution failed.
    ResolutionFailed(String),
    /// Resolution returned no addresses.
    NoAddresses,
    /// A resolved or literal address is not globally routable (SSRF guard).
    RejectedAddress(IpRejection),
    /// The connection's peer differs from the pinned, validated address
    /// (DNS-rebinding / connector redirection).
    DnsRebinding {
        /// The address the dial was pinned to.
        pinned: IpAddr,
        /// The address the connection actually reached.
        connected: IpAddr,
    },
    /// The TLS certificate / SNI hostname did not validate.
    CertificateInvalid,
    /// The origin answered with a redirect; redirects are disabled.
    RedirectRefused(u16),
    /// The response status was neither success nor an expected refusal.
    UnexpectedStatus(u16),
    /// The response body exceeded the caller's byte bound.
    ResponseTooLarge {
        /// The configured maximum.
        limit: usize,
        /// The observed length.
        actual: usize,
    },
    /// The profile lease is not active; no network work is permitted.
    LeaseInactive,
    /// The connector reported a transport-level failure.
    Transport(String),
}

impl fmt::Display for SafeDialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SafeDialError::NotHttps => write!(f, "origin is not https"),
            SafeDialError::EmptyHost => write!(f, "origin host is empty"),
            SafeDialError::OriginTooLong => write!(f, "origin exceeds 255 bytes"),
            SafeDialError::MalformedOrigin => write!(f, "origin is malformed"),
            SafeDialError::NonStandardPort(p) => write!(f, "origin port {p} is not 443"),
            SafeDialError::ResolutionFailed(e) => write!(f, "resolution failed: {e}"),
            SafeDialError::NoAddresses => write!(f, "resolution returned no addresses"),
            SafeDialError::RejectedAddress(r) => write!(f, "address rejected: {r}"),
            SafeDialError::DnsRebinding { pinned, connected } => {
                write!(f, "dns rebinding: pinned {pinned}, connected {connected}")
            }
            SafeDialError::CertificateInvalid => write!(f, "certificate/SNI invalid"),
            SafeDialError::RedirectRefused(s) => write!(f, "redirect refused (status {s})"),
            SafeDialError::UnexpectedStatus(s) => write!(f, "unexpected status {s}"),
            SafeDialError::ResponseTooLarge { limit, actual } => {
                write!(f, "response {actual} bytes exceeds limit {limit}")
            }
            SafeDialError::LeaseInactive => write!(f, "profile lease is not active"),
            SafeDialError::Transport(e) => write!(f, "transport error: {e}"),
        }
    }
}

impl std::error::Error for SafeDialError {}

/// Injected name resolver. The real implementation performs a DNS lookup; tests
/// inject a fake. The safe dial calls this AT MOST ONCE per dial and pins the
/// result, so a rebinding resolver cannot influence which address is dialed.
pub trait SafeResolver {
    /// Resolve `host` to its candidate addresses.
    fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, SafeDialError>;
}

/// A request pinned to one already-validated address. The connector MUST connect
/// to exactly `pinned_ip:port` (never re-resolving `sni_host`), and report the
/// address it actually reached so the caller can re-check it.
#[derive(Debug, Clone)]
pub struct PinnedRequest {
    /// The validated address to dial.
    pub pinned_ip: IpAddr,
    /// The port (always 443).
    pub port: u16,
    /// The hostname for SNI and certificate validation.
    pub sni_host: String,
    /// The request path.
    pub path: String,
}

/// The result of a pinned connection.
#[derive(Debug, Clone)]
pub struct DialResponse {
    /// The address the socket actually connected to (re-validated against the pin).
    pub connected_ip: IpAddr,
    /// Whether the TLS certificate matched `sni_host`.
    pub sni_verified: bool,
    /// The HTTP status code.
    pub status: u16,
    /// The (already byte-bounded by the connector, re-bounded here) response body.
    pub body: Vec<u8>,
}

/// Injected connector. The real implementation opens a pinned TLS connection and
/// issues a redirect-free GET; tests inject a fake.
pub trait SafeConnector {
    /// Connect to the pinned address and fetch the request.
    fn fetch(&self, req: &PinnedRequest) -> Result<DialResponse, SafeDialError>;
}

// ===========================================================================
// Origin parsing.
// ===========================================================================

struct HttpsTarget {
    host: String,
    port: u16,
}

/// Parse an `https://host[:port][/path]` origin into a host + port, enforcing the
/// scheme, the 255-byte floor, and port 443.
fn parse_https_origin(origin: &str) -> Result<HttpsTarget, SafeDialError> {
    if origin.len() > MAX_ORIGIN_BYTES {
        return Err(SafeDialError::OriginTooLong);
    }
    let rest = origin
        .strip_prefix("https://")
        .ok_or(SafeDialError::NotHttps)?;
    // The authority stops at the first path/query/fragment delimiter.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(SafeDialError::EmptyHost);
    }
    let (host, port) = if let Some(after_bracket) = authority.strip_prefix('[') {
        // Bracketed IPv6 literal: `[::1]` or `[::1]:443`.
        let end = after_bracket
            .find(']')
            .ok_or(SafeDialError::MalformedOrigin)?;
        let host = after_bracket[..end].to_string();
        let tail = &after_bracket[end + 1..];
        let port = match tail.strip_prefix(':') {
            Some(p) => p
                .parse::<u16>()
                .map_err(|_| SafeDialError::MalformedOrigin)?,
            None if tail.is_empty() => HTTPS_PORT,
            None => return Err(SafeDialError::MalformedOrigin),
        };
        (host, port)
    } else {
        match authority.rsplit_once(':') {
            Some((host, p)) => (
                host.to_string(),
                p.parse::<u16>()
                    .map_err(|_| SafeDialError::MalformedOrigin)?,
            ),
            None => (authority.to_string(), HTTPS_PORT),
        }
    };
    if host.is_empty() {
        return Err(SafeDialError::EmptyHost);
    }
    if port != HTTPS_PORT {
        return Err(SafeDialError::NonStandardPort(port));
    }
    Ok(HttpsTarget { host, port })
}

// ===========================================================================
// The safe HTTPS dial.
// ===========================================================================

/// Fetch `path` from an anchor's HTTPS `origin` under an SSRF/rebinding guard,
/// scoped to an active [`ProfileLease`].
///
/// Fail-closed order:
/// 1. the lease must be active;
/// 2. `origin` must be an `https://` origin on port 443, within the size floor;
/// 3. the host is resolved EXACTLY ONCE (or, for an IP literal, validated
///    directly) and every candidate address must be globally routable — the
///    first validated address is pinned;
/// 4. the connector dials the pinned address; its actual peer must equal the pin
///    (DNS-rebinding guard), the certificate/SNI must validate, redirects are
///    refused, the status must be success, and the body must fit `max_body`.
pub fn safe_https_get(
    lease: &ProfileLease,
    origin: &str,
    path: &str,
    resolver: &dyn SafeResolver,
    connector: &dyn SafeConnector,
    max_body: usize,
) -> Result<Vec<u8>, SafeDialError> {
    if !lease.is_active() {
        return Err(SafeDialError::LeaseInactive);
    }
    let target = parse_https_origin(origin)?;

    // Resolve-once + validate. An IP literal is validated directly, never resolved.
    let pinned_ip = match target.host.parse::<IpAddr>() {
        Ok(literal) => {
            classify_public_ip(literal).map_err(SafeDialError::RejectedAddress)?;
            literal
        }
        Err(_) => {
            let addrs = resolver.resolve(&target.host)?;
            if addrs.is_empty() {
                return Err(SafeDialError::NoAddresses);
            }
            // Every resolved address must be public; the first is pinned.
            for addr in &addrs {
                classify_public_ip(*addr).map_err(SafeDialError::RejectedAddress)?;
            }
            addrs[0]
        }
    };

    let req = PinnedRequest {
        pinned_ip,
        port: target.port,
        sni_host: target.host,
        path: path.to_string(),
    };
    let resp = connector.fetch(&req)?;

    // DNS-rebinding guard: the socket must have reached exactly the pinned,
    // already-validated address. Any divergence is refused, not followed.
    if resp.connected_ip != pinned_ip {
        return Err(SafeDialError::DnsRebinding {
            pinned: pinned_ip,
            connected: resp.connected_ip,
        });
    }
    if !resp.sni_verified {
        return Err(SafeDialError::CertificateInvalid);
    }
    if (300..400).contains(&resp.status) {
        return Err(SafeDialError::RedirectRefused(resp.status));
    }
    if resp.status != 200 {
        return Err(SafeDialError::UnexpectedStatus(resp.status));
    }
    if resp.body.len() > max_body {
        return Err(SafeDialError::ResponseTooLarge {
            limit: max_body,
            actual: resp.body.len(),
        });
    }
    Ok(resp.body)
}

// ===========================================================================
// Ticket-first verified bootstrap.
// ===========================================================================

/// The verified dial identity of an anchor: the endpoint id and HTTPS origin that
/// its signed descriptor declares. Endpoint/origin hints are checked against this
/// — they can never override it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorIdentity {
    /// The iroh endpoint (node) id the descriptor declares.
    pub iroh_endpoint_id: [u8; 32],
    /// The HTTPS origin the descriptor declares.
    pub https_origin: String,
}

impl AnchorIdentity {
    /// Extract the dial identity from a verified descriptor body. The identity
    /// always comes from the descriptor, never from an unsigned hint.
    pub fn from_descriptor(descriptor: &AnchorDescriptorBodyV1) -> Self {
        AnchorIdentity {
            iroh_endpoint_id: descriptor.current_iroh_endpoint_id,
            https_origin: descriptor.https_origin.clone(),
        }
    }
}

/// An unsigned / shared routing hint (e.g. from a handoff link or QR). It may
/// name where to dial, but only if it agrees with the already-verified identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointHint {
    /// The iroh endpoint id the hint proposes.
    pub iroh_endpoint_id: [u8; 32],
    /// The HTTPS origin the hint proposes, if any.
    pub https_origin: Option<String>,
}

/// A ticket that was admitted before any dial, together with the verified anchor
/// dial identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBootstrap {
    /// The admitted root-signed ticket.
    pub admitted: AdmittedTicket,
    /// The verified dial identity.
    pub identity: AnchorIdentity,
}

/// Why a verified bootstrap was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapError {
    /// The root ticket did not admit (signature/version/transport/expiry/…).
    Ticket(AuthorityError),
    /// A hint named a different iroh endpoint than the verified identity.
    EndpointHintMismatch,
    /// A hint named a different HTTPS origin than the verified identity.
    OriginHintMismatch,
    /// The verified dial itself was refused.
    Dial(SafeDialError),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BootstrapError::Ticket(e) => write!(f, "ticket refused: {e}"),
            BootstrapError::EndpointHintMismatch => {
                write!(f, "endpoint hint does not match verified identity")
            }
            BootstrapError::OriginHintMismatch => {
                write!(f, "origin hint does not match verified identity")
            }
            BootstrapError::Dial(e) => write!(f, "dial refused: {e}"),
        }
    }
}

impl std::error::Error for BootstrapError {}

/// Verify the root ticket and reconcile any hint BEFORE any dial.
///
/// This is the ticket-first boundary: [`admit_public_site_ticket`] runs first
/// (root signature, `min_sync_version == 2`, `require:none` transport equality,
/// expiry, epoch), so an expired/downgraded/`require:arti` ticket is refused with
/// its exact reason before iroh or HTTPS is touched. A hint is then permitted to
/// name the endpoint only if it agrees with the verified descriptor identity; a
/// hint pointing elsewhere is refused, never followed.
///
/// Manifest coordinate matching is intentionally out of scope here (it happens at
/// listing resolution); bootstrap admits on signature + transport + freshness.
pub fn verify_bootstrap(
    ticket: &RootSignedTicketCoreEnvelopeV2,
    client_floor: &TransportFloor,
    ticket_floor: &TicketFloor,
    now: u64,
    identity: &AnchorIdentity,
    hint: Option<&EndpointHint>,
) -> Result<VerifiedBootstrap, BootstrapError> {
    // TICKET-FIRST: admit before any dial consideration.
    let admitted = admit_public_site_ticket(ticket, None, client_floor, ticket_floor, now)
        .map_err(BootstrapError::Ticket)?;

    // A hint may not redirect us away from the verified identity.
    if let Some(hint) = hint {
        if hint.iroh_endpoint_id != identity.iroh_endpoint_id {
            return Err(BootstrapError::EndpointHintMismatch);
        }
        if let Some(origin) = &hint.https_origin {
            if origin != &identity.https_origin {
                return Err(BootstrapError::OriginHintMismatch);
            }
        }
    }

    Ok(VerifiedBootstrap {
        admitted,
        identity: identity.clone(),
    })
}

/// Verify the ticket + hint, then safely dial the verified HTTPS origin.
///
/// The verification runs first and returns before any resolution or connection,
/// so a refused ticket or a mismatched hint means the resolver and connector are
/// never touched. Only the verified descriptor's own origin is dialed.
#[allow(clippy::too_many_arguments)]
pub fn bootstrap_over_https(
    lease: &ProfileLease,
    ticket: &RootSignedTicketCoreEnvelopeV2,
    client_floor: &TransportFloor,
    ticket_floor: &TicketFloor,
    now: u64,
    identity: &AnchorIdentity,
    hint: Option<&EndpointHint>,
    well_known_path: &str,
    resolver: &dyn SafeResolver,
    connector: &dyn SafeConnector,
    max_body: usize,
) -> Result<(VerifiedBootstrap, Vec<u8>), BootstrapError> {
    let verified = verify_bootstrap(ticket, client_floor, ticket_floor, now, identity, hint)?;
    let body = safe_https_get(
        lease,
        &verified.identity.https_origin,
        well_known_path,
        resolver,
        connector,
        max_body,
    )
    .map_err(BootstrapError::Dial)?;
    Ok((verified, body))
}
