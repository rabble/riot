//! WU-011B — verified bootstrap and safe dialing.
//!
//! Every test here runs with an injected [`SafeResolver`]/[`SafeConnector`] and a
//! [`ProfileLease`] built over the WU-011A runtime seam with no-op fakes, so
//! NOTHING touches real DNS, TLS, iroh, or the network. The matrix asserts the
//! exact refusal for each SSRF/rebinding/redirect/ticket case.

use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ed25519_dalek::{Signer, SigningKey};

use riot_anchor_protocol::records::{EnabledRole, OperatorVerificationKeyV1};
use riot_anchor_protocol::{
    AnchorDescriptorBodyV1, AuthorityError, PublicSiteTicketV2Core, RootSignedTicketCoreEnvelopeV2,
    TicketFloor, TransportFloor,
};

use riot_client_net::runtime::{
    CancellationToken, EndpointFactory, NetworkEndpoint, ProfileId, ProfileLease, RuntimeFuture,
    RuntimeHost, RuntimeResult, SpawnedTask, TaskSpawner,
};
use riot_client_net::safe_dial::{
    bootstrap_over_https, classify_public_ip, safe_https_get, verify_bootstrap, AnchorIdentity,
    BootstrapError, DialResponse, EndpointHint, IpRejection, PinnedRequest, SafeConnector,
    SafeDialError, SafeResolver,
};

// ---------------------------------------------------------------------------
// Runtime lease built over WU-011A's public seam with no-op fakes (no network).
// ---------------------------------------------------------------------------

struct NoopEndpoint;
impl NetworkEndpoint for NoopEndpoint {
    fn shutdown(&self) {}
}

struct NoopFactory;
impl EndpointFactory for NoopFactory {
    fn create_endpoint(&self) -> RuntimeResult<Arc<dyn NetworkEndpoint>> {
        Ok(Arc::new(NoopEndpoint))
    }
}

struct NoopSpawner;
impl TaskSpawner for NoopSpawner {
    fn spawn(&self, _token: CancellationToken, _task: RuntimeFuture) -> Box<dyn SpawnedTask> {
        Box::new(NoopTask)
    }
}
struct NoopTask;
impl SpawnedTask for NoopTask {
    fn join(self: Box<Self>) {}
}

/// A fresh, active per-profile lease over a no-op runtime.
fn fresh_lease() -> ProfileLease {
    let host = RuntimeHost::new();
    let rt = host
        .get_or_start(&NoopFactory, Arc::new(NoopSpawner))
        .expect("start runtime");
    rt.acquire_profile_lease(ProfileId::new("wu011b"))
        .expect("acquire lease")
}

// ---------------------------------------------------------------------------
// Fake resolver / connector seam.
// ---------------------------------------------------------------------------

/// Resolver that returns a fixed address list and counts calls.
struct FixedResolver {
    ips: Vec<IpAddr>,
    calls: AtomicUsize,
}
impl FixedResolver {
    fn new(ips: Vec<IpAddr>) -> Self {
        Self {
            ips,
            calls: AtomicUsize::new(0),
        }
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
impl SafeResolver for FixedResolver {
    fn resolve(&self, _host: &str) -> Result<Vec<IpAddr>, SafeDialError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.ips.clone())
    }
}

/// Resolver that answers a public address on the first call and a private one on
/// every later call — a classic DNS-rebinding source. Pinning means we must only
/// ever consult it ONCE.
struct RebindingResolver {
    first: IpAddr,
    rebind: IpAddr,
    calls: AtomicUsize,
}
impl RebindingResolver {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
impl SafeResolver for RebindingResolver {
    fn resolve(&self, _host: &str) -> Result<Vec<IpAddr>, SafeDialError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![if n == 0 { self.first } else { self.rebind }])
    }
}

/// A scripted connector. It records the pinned IP it was asked to dial and the
/// number of calls, and returns a fully configurable response.
struct ScriptedConnector {
    /// The IP the socket "actually" connected to; `None` echoes the pinned IP.
    connected_ip: Option<IpAddr>,
    sni_verified: bool,
    status: u16,
    body: Vec<u8>,
    calls: AtomicUsize,
    last_pinned: Mutex<Option<IpAddr>>,
}
impl ScriptedConnector {
    fn ok() -> Self {
        Self {
            connected_ip: None,
            sni_verified: true,
            status: 200,
            body: b"anchor-descriptor".to_vec(),
            calls: AtomicUsize::new(0),
            last_pinned: Mutex::new(None),
        }
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    fn last_pinned(&self) -> Option<IpAddr> {
        *self.last_pinned.lock().unwrap()
    }
}
impl SafeConnector for ScriptedConnector {
    fn fetch(&self, req: &PinnedRequest) -> Result<DialResponse, SafeDialError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last_pinned.lock().unwrap() = Some(req.pinned_ip);
        Ok(DialResponse {
            connected_ip: self.connected_ip.unwrap_or(req.pinned_ip),
            sni_verified: self.sni_verified,
            status: self.status,
            body: self.body.clone(),
        })
    }
}

/// A connector that must never be dialed; any call is a test failure.
struct ForbiddenConnector {
    calls: AtomicUsize,
}
impl ForbiddenConnector {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
impl SafeConnector for ForbiddenConnector {
    fn fetch(&self, _req: &PinnedRequest) -> Result<DialResponse, SafeDialError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(DialResponse {
            connected_ip: "203.0.113.9".parse().unwrap(),
            sni_verified: true,
            status: 200,
            body: Vec::new(),
        })
    }
}

const PUBLIC_V4: &str = "93.184.216.34";
const ORIGIN: &str = "https://anchor.example";
const WELL_KNOWN: &str = "/.well-known/riot-anchor.json";
const MAX_BODY: usize = 64 * 1024;

fn public_ip() -> IpAddr {
    PUBLIC_V4.parse().unwrap()
}

// ---------------------------------------------------------------------------
// Ticket + descriptor builders.
// ---------------------------------------------------------------------------

/// Build a root-signed v2 ticket, applying `mutate` before signing so the
/// signature always covers the final core (a downgraded/arti ticket still has a
/// VALID signature and reaches the transport/version gate).
fn signed_ticket(
    now: u64,
    mutate: impl FnOnce(&mut PublicSiteTicketV2Core),
) -> RootSignedTicketCoreEnvelopeV2 {
    let sk = SigningKey::from_bytes(&[7u8; 32]);
    let root_id = sk.verifying_key().to_bytes();
    let mut core = PublicSiteTicketV2Core {
        root_id,
        o_namespace_id: [1u8; 32],
        c_namespace_id: [2u8; 32],
        w_namespace_id: [3u8; 32],
        manifest_digest: [4u8; 32],
        manifest_version: 1,
        min_sync_version: 2,
        manifest_required_transport: TransportFloor::RequireNone,
        transport_floor: TransportFloor::RequireNone,
        transport_epoch: 0,
        issued_unix_seconds: now.saturating_sub(60),
        expiry_unix_seconds: now + 3600,
    };
    mutate(&mut core);
    let mut env = RootSignedTicketCoreEnvelopeV2 {
        core,
        root_signature: [0u8; 64],
    };
    let preimage = env.signing_preimage().expect("preimage");
    env.root_signature = sk.sign(&preimage).to_bytes();
    env
}

fn no_floor(env: &RootSignedTicketCoreEnvelopeV2) -> TicketFloor {
    TicketFloor {
        root_id: env.core.root_id,
        highest_transport_epoch: None,
    }
}

const ENDPOINT_ID: [u8; 32] = [9u8; 32];

fn identity() -> AnchorIdentity {
    AnchorIdentity {
        iroh_endpoint_id: ENDPOINT_ID,
        https_origin: ORIGIN.to_string(),
    }
}

// ===========================================================================
// IP classification (SSRF guard, unit level).
// ===========================================================================

#[test]
fn classify_accepts_public_addresses() {
    for ip in [
        "1.1.1.1",
        "8.8.8.8",
        PUBLIC_V4,
        "2606:4700:4700::1111",
        "2620:fe::fe",
    ] {
        let ip: IpAddr = ip.parse().unwrap();
        assert!(classify_public_ip(ip).is_ok(), "{ip} should be public");
    }
}

#[test]
fn classify_rejects_every_non_global_range() {
    let cases: &[(&str, IpRejection)] = &[
        ("0.0.0.0", IpRejection::Unspecified),
        ("127.0.0.1", IpRejection::Loopback),
        ("10.0.0.7", IpRejection::Private),
        ("172.16.5.5", IpRejection::Private),
        ("192.168.1.1", IpRejection::Private),
        ("169.254.10.10", IpRejection::LinkLocal),
        ("224.0.0.1", IpRejection::Multicast),
        ("240.0.0.1", IpRejection::Reserved),
        ("255.255.255.255", IpRejection::Reserved),
        ("100.64.0.1", IpRejection::Reserved),
        ("198.18.0.1", IpRejection::Reserved),
        ("192.0.2.5", IpRejection::Reserved),
        ("::", IpRejection::Unspecified),
        ("::1", IpRejection::Loopback),
        ("fc00::1", IpRejection::Private),
        ("fd12:3456::1", IpRejection::Private),
        ("fe80::1", IpRejection::LinkLocal),
        ("ff02::1", IpRejection::Multicast),
        ("2001:db8::1", IpRejection::Reserved),
        ("::ffff:10.0.0.1", IpRejection::Private),
    ];
    for (ip, want) in cases {
        let parsed: IpAddr = ip.parse().unwrap();
        assert_eq!(
            classify_public_ip(parsed),
            Err(*want),
            "{ip} should be rejected as {want:?}"
        );
    }
}

// ===========================================================================
// Safe HTTPS dial — SSRF refusals via a fake resolver.
// ===========================================================================

fn assert_resolved_refusal(ip: &str, want: IpRejection) {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![ip.parse().unwrap()]);
    let connector = ScriptedConnector::ok();
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("must refuse non-global resolved address");
    assert_eq!(err, SafeDialError::RejectedAddress(want));
    // The SSRF guard fires BEFORE any dial.
    assert_eq!(connector.calls(), 0, "no dial for a rejected address");
}

#[test]
fn refuses_private_loopback_linklocal_multicast_reserved_unspecified() {
    assert_resolved_refusal("10.0.0.9", IpRejection::Private);
    assert_resolved_refusal("127.0.0.1", IpRejection::Loopback);
    assert_resolved_refusal("169.254.1.1", IpRejection::LinkLocal);
    assert_resolved_refusal("224.0.0.9", IpRejection::Multicast);
    assert_resolved_refusal("240.0.0.9", IpRejection::Reserved);
    assert_resolved_refusal("0.0.0.0", IpRejection::Unspecified);
    assert_resolved_refusal("::1", IpRejection::Loopback);
    assert_resolved_refusal("fe80::9", IpRejection::LinkLocal);
}

#[test]
fn refuses_when_any_resolved_address_is_non_global() {
    // First is public, second is private: the whole dial is refused (SSRF).
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip(), "192.168.0.5".parse().unwrap()]);
    let connector = ScriptedConnector::ok();
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("mixed address set is refused");
    assert_eq!(err, SafeDialError::RejectedAddress(IpRejection::Private));
    assert_eq!(connector.calls(), 0);
}

#[test]
fn refuses_empty_resolution() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![]);
    let connector = ScriptedConnector::ok();
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("no addresses");
    assert_eq!(err, SafeDialError::NoAddresses);
    assert_eq!(connector.calls(), 0);
}

#[test]
fn refuses_ip_literal_origin_in_private_range() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ScriptedConnector::ok();
    // An IP-literal origin is validated directly, never resolved.
    let err = safe_https_get(
        &lease,
        "https://10.0.0.1",
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect_err("private ip literal origin");
    assert_eq!(err, SafeDialError::RejectedAddress(IpRejection::Private));
    assert_eq!(resolver.calls(), 0, "ip literal is not resolved");
    assert_eq!(connector.calls(), 0);
}

// ===========================================================================
// Pinned resolution + DNS-rebinding protection.
// ===========================================================================

#[test]
fn resolves_once_and_pins_defeating_rebinding() {
    let lease = fresh_lease();
    let resolver = RebindingResolver {
        first: public_ip(),
        rebind: "192.168.0.9".parse().unwrap(),
        calls: AtomicUsize::new(0),
    };
    let connector = ScriptedConnector::ok();
    let body = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect("pinned public dial succeeds");
    assert_eq!(body, b"anchor-descriptor");
    // Resolved EXACTLY once; the later private answer is never consulted.
    assert_eq!(resolver.calls(), 1, "resolution is pinned");
    assert_eq!(connector.last_pinned(), Some(public_ip()));
}

#[test]
fn refuses_connector_that_rebinds_to_a_different_ip() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let mut connector = ScriptedConnector::ok();
    // The socket connects somewhere other than the pinned, validated IP.
    connector.connected_ip = Some("198.51.100.7".parse().unwrap());
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("connected ip diverged from pinned");
    assert_eq!(
        err,
        SafeDialError::DnsRebinding {
            pinned: public_ip(),
            connected: "198.51.100.7".parse().unwrap(),
        }
    );
}

#[test]
fn refuses_connector_that_rebinds_to_a_private_ip() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let mut connector = ScriptedConnector::ok();
    connector.connected_ip = Some("10.1.2.3".parse().unwrap());
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("connected private ip");
    assert_eq!(
        err,
        SafeDialError::DnsRebinding {
            pinned: public_ip(),
            connected: "10.1.2.3".parse().unwrap(),
        }
    );
}

// ===========================================================================
// TLS / redirect / bounds / origin.
// ===========================================================================

#[test]
fn refuses_redirect() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let mut connector = ScriptedConnector::ok();
    connector.status = 302;
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("redirects disabled");
    assert_eq!(err, SafeDialError::RedirectRefused(302));
}

#[test]
fn refuses_invalid_certificate() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let mut connector = ScriptedConnector::ok();
    connector.sni_verified = false;
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("cert/sni mismatch");
    assert_eq!(err, SafeDialError::CertificateInvalid);
}

#[test]
fn refuses_unexpected_status() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let mut connector = ScriptedConnector::ok();
    connector.status = 500;
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("non-200");
    assert_eq!(err, SafeDialError::UnexpectedStatus(500));
}

#[test]
fn refuses_oversize_response() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let mut connector = ScriptedConnector::ok();
    connector.body = vec![0u8; 33];
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, 32)
        .expect_err("body exceeds bound");
    assert_eq!(
        err,
        SafeDialError::ResponseTooLarge {
            limit: 32,
            actual: 33
        }
    );
}

#[test]
fn refuses_non_https_scheme() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ScriptedConnector::ok();
    let err = safe_https_get(
        &lease,
        "http://anchor.example",
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect_err("http rejected");
    assert_eq!(err, SafeDialError::NotHttps);
    assert_eq!(resolver.calls(), 0);
}

#[test]
fn refuses_non_443_port() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ScriptedConnector::ok();
    let err = safe_https_get(
        &lease,
        "https://anchor.example:8443",
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect_err("only 443");
    assert_eq!(err, SafeDialError::NonStandardPort(8443));
}

#[test]
fn accepts_explicit_443_and_bracketed_ipv6_public() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ScriptedConnector::ok();
    // Explicit :443 is fine and still resolves the hostname.
    safe_https_get(
        &lease,
        "https://anchor.example:443",
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect("explicit 443 ok");

    // Bracketed public IPv6 literal dials without resolution.
    let lease2 = fresh_lease();
    let resolver2 = FixedResolver::new(vec![]);
    let connector2 = ScriptedConnector::ok();
    safe_https_get(
        &lease2,
        "https://[2606:4700:4700::1111]:443",
        WELL_KNOWN,
        &resolver2,
        &connector2,
        MAX_BODY,
    )
    .expect("bracketed ipv6 literal ok");
    assert_eq!(resolver2.calls(), 0);
}

#[test]
fn refuses_empty_and_oversize_origin() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ScriptedConnector::ok();
    assert_eq!(
        safe_https_get(&lease, "https://", WELL_KNOWN, &resolver, &connector, MAX_BODY),
        Err(SafeDialError::EmptyHost)
    );
    let long = format!("https://{}.example", "a".repeat(300));
    assert_eq!(
        safe_https_get(&lease, &long, WELL_KNOWN, &resolver, &connector, MAX_BODY),
        Err(SafeDialError::OriginTooLong)
    );
}

// ===========================================================================
// Runtime lease integration.
// ===========================================================================

#[test]
fn refuses_dial_on_released_lease() {
    let mut lease = fresh_lease();
    lease.release();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ScriptedConnector::ok();
    let err = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect_err("released lease cannot dial");
    assert_eq!(err, SafeDialError::LeaseInactive);
    assert_eq!(resolver.calls(), 0);
    assert_eq!(connector.calls(), 0);
}

#[test]
fn happy_path_returns_body_over_active_lease() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ScriptedConnector::ok();
    let body = safe_https_get(&lease, ORIGIN, WELL_KNOWN, &resolver, &connector, MAX_BODY)
        .expect("dial succeeds");
    assert_eq!(body, b"anchor-descriptor");
    assert_eq!(connector.calls(), 1);
    assert_eq!(connector.last_pinned(), Some(public_ip()));
}

// ===========================================================================
// Ticket-first verify-before-dial.
// ===========================================================================

#[test]
fn verify_bootstrap_admits_valid_ticket_with_no_hint() {
    let now = 1_000_000;
    let env = signed_ticket(now, |_| {});
    let floor = no_floor(&env);
    let verified = verify_bootstrap(
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        None,
    )
    .expect("valid ticket admitted");
    assert_eq!(verified.identity.iroh_endpoint_id, ENDPOINT_ID);
    assert_eq!(verified.admitted.core.root_id, env.core.root_id);
}

#[test]
fn refuses_expired_ticket_before_any_dial() {
    let now = 1_000_000;
    let env = signed_ticket(now, |c| c.expiry_unix_seconds = now); // now >= expiry
    let floor = no_floor(&env);
    let err = verify_bootstrap(
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        None,
    )
    .expect_err("expired");
    assert_eq!(err, BootstrapError::Ticket(AuthorityError::ExpiredTicket));
}

#[test]
fn refuses_downgraded_ticket() {
    let now = 1_000_000;
    let env = signed_ticket(now, |c| c.min_sync_version = 1);
    let floor = no_floor(&env);
    let err = verify_bootstrap(
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        None,
    )
    .expect_err("downgrade");
    assert_eq!(
        err,
        BootstrapError::Ticket(AuthorityError::UnsupportedVersion)
    );
}

#[test]
fn refuses_require_arti_ticket() {
    let now = 1_000_000;
    let env = signed_ticket(now, |c| c.transport_floor = TransportFloor::RequireArti);
    let floor = no_floor(&env);
    let err = verify_bootstrap(
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        None,
    )
    .expect_err("arti");
    assert_eq!(
        err,
        BootstrapError::Ticket(AuthorityError::UnsupportedTransport)
    );
}

#[test]
fn refuses_client_floor_requiring_arti() {
    // Even a require:none ticket is refused when the client's own floor is arti.
    let now = 1_000_000;
    let env = signed_ticket(now, |_| {});
    let floor = no_floor(&env);
    let err = verify_bootstrap(
        &env,
        &TransportFloor::RequireArti,
        &floor,
        now,
        &identity(),
        None,
    )
    .expect_err("client floor arti");
    assert_eq!(
        err,
        BootstrapError::Ticket(AuthorityError::UnsupportedTransport)
    );
}

// ===========================================================================
// Descriptor endpoint hints cannot override the verified identity.
// ===========================================================================

#[test]
fn refuses_endpoint_hint_that_points_elsewhere() {
    let now = 1_000_000;
    let env = signed_ticket(now, |_| {});
    let floor = no_floor(&env);
    let hint = EndpointHint {
        iroh_endpoint_id: [0xEE; 32], // different endpoint than the verified identity
        https_origin: None,
    };
    let err = verify_bootstrap(
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        Some(&hint),
    )
    .expect_err("hint points elsewhere");
    assert_eq!(err, BootstrapError::EndpointHintMismatch);
}

#[test]
fn refuses_origin_hint_that_points_elsewhere() {
    let now = 1_000_000;
    let env = signed_ticket(now, |_| {});
    let floor = no_floor(&env);
    let hint = EndpointHint {
        iroh_endpoint_id: ENDPOINT_ID, // endpoint matches
        https_origin: Some("https://evil.example".to_string()), // origin does not
    };
    let err = verify_bootstrap(
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        Some(&hint),
    )
    .expect_err("origin hint diverges");
    assert_eq!(err, BootstrapError::OriginHintMismatch);
}

#[test]
fn accepts_matching_hint() {
    let now = 1_000_000;
    let env = signed_ticket(now, |_| {});
    let floor = no_floor(&env);
    let hint = EndpointHint {
        iroh_endpoint_id: ENDPOINT_ID,
        https_origin: Some(ORIGIN.to_string()),
    };
    verify_bootstrap(
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        Some(&hint),
    )
    .expect("hint agrees with verified identity");
}

#[test]
fn anchor_identity_is_taken_from_the_descriptor() {
    let descriptor = sample_descriptor();
    let id = AnchorIdentity::from_descriptor(&descriptor);
    assert_eq!(id.iroh_endpoint_id, descriptor.current_iroh_endpoint_id);
    assert_eq!(id.https_origin, descriptor.https_origin);
}

// ===========================================================================
// bootstrap_over_https — ticket-first: no dial happens if the ticket is refused.
// ===========================================================================

#[test]
fn bootstrap_dials_only_after_ticket_and_hint_verify() {
    let now = 1_000_000;
    let env = signed_ticket(now, |_| {});
    let floor = no_floor(&env);
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ScriptedConnector::ok();
    let (verified, body) = bootstrap_over_https(
        &lease,
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        None,
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect("verified bootstrap dials");
    assert_eq!(body, b"anchor-descriptor");
    assert_eq!(verified.identity.https_origin, ORIGIN);
    assert_eq!(connector.calls(), 1);
}

#[test]
fn bootstrap_never_dials_when_ticket_is_refused() {
    let now = 1_000_000;
    let env = signed_ticket(now, |c| c.expiry_unix_seconds = now);
    let floor = no_floor(&env);
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ForbiddenConnector::new();
    let err = bootstrap_over_https(
        &lease,
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        None,
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect_err("expired ticket blocks the dial");
    assert_eq!(err, BootstrapError::Ticket(AuthorityError::ExpiredTicket));
    // Ticket-first: nothing was resolved or dialed.
    assert_eq!(resolver.calls(), 0);
    assert_eq!(connector.calls(), 0);
}

#[test]
fn bootstrap_never_dials_when_hint_points_elsewhere() {
    let now = 1_000_000;
    let env = signed_ticket(now, |_| {});
    let floor = no_floor(&env);
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![public_ip()]);
    let connector = ForbiddenConnector::new();
    let hint = EndpointHint {
        iroh_endpoint_id: [0x11; 32],
        https_origin: None,
    };
    let err = bootstrap_over_https(
        &lease,
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        Some(&hint),
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect_err("mismatched hint blocks the dial");
    assert_eq!(err, BootstrapError::EndpointHintMismatch);
    assert_eq!(connector.calls(), 0);
}

#[test]
fn bootstrap_propagates_dial_refusal() {
    let now = 1_000_000;
    let env = signed_ticket(now, |_| {});
    let floor = no_floor(&env);
    let lease = fresh_lease();
    // Ticket verifies, but the origin resolves to a private address.
    let resolver = FixedResolver::new(vec!["10.0.0.4".parse().unwrap()]);
    let connector = ScriptedConnector::ok();
    let err = bootstrap_over_https(
        &lease,
        &env,
        &TransportFloor::RequireNone,
        &floor,
        now,
        &identity(),
        None,
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect_err("dial refused after ticket verifies");
    assert_eq!(
        err,
        BootstrapError::Dial(SafeDialError::RejectedAddress(IpRejection::Private))
    );
    assert_eq!(connector.calls(), 0);
}

// ---------------------------------------------------------------------------
// A full descriptor body, used only to prove AnchorIdentity::from_descriptor.
// ---------------------------------------------------------------------------

fn sample_descriptor() -> AnchorDescriptorBodyV1 {
    AnchorDescriptorBodyV1 {
        anchor_id: [1u8; 32],
        genesis_operator_public_key: [2u8; 32],
        genesis_random_256_bits: [3u8; 32],
        current_operator_verification_key: OperatorVerificationKeyV1 {
            public_key: [4u8; 32],
        },
        current_operator_key_id: [5u8; 32],
        descriptor_epoch: 0,
        previous_descriptor_digest: None,
        current_iroh_endpoint_id: ENDPOINT_ID,
        https_origin: ORIGIN.to_string(),
        operator_display_label: "op".to_string(),
        self_reported_failure_domain_label: "fd".to_string(),
        supported_control_versions: vec![1],
        supported_sync_versions: vec![2],
        enabled_roles: vec![EnabledRole::Directory],
        limit_profile_digest: [6u8; 32],
        predecessor_operator_verification_key: None,
        issued_at: 1,
        expires_at: 2,
    }
}

// ===========================================================================
// Additional origin/classify edge branches.
// ===========================================================================

#[test]
fn classify_rejects_this_network_and_ipv4_compat_forms() {
    // `0.0.0.0/8` (non-unspecified host bits) is "this network", reserved.
    assert_eq!(
        classify_public_ip("0.1.2.3".parse().unwrap()),
        Err(IpRejection::Reserved)
    );
    // `198.19.x` is the second half of the benchmarking `/15`.
    assert_eq!(
        classify_public_ip("198.19.0.1".parse().unwrap()),
        Err(IpRejection::Reserved)
    );
}

#[test]
fn accepts_bracketed_ipv6_without_port() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![]);
    let connector = ScriptedConnector::ok();
    // No explicit port on a bracketed literal defaults to 443 and dials directly.
    safe_https_get(
        &lease,
        "https://[2606:4700:4700::1111]",
        WELL_KNOWN,
        &resolver,
        &connector,
        MAX_BODY,
    )
    .expect("bracketed ipv6 without port ok");
    assert_eq!(resolver.calls(), 0);
    assert_eq!(connector.calls(), 1);
}

#[test]
fn refuses_malformed_bracketed_origins() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![]);
    let connector = ScriptedConnector::ok();
    // Trailing junk after the closing bracket.
    assert_eq!(
        safe_https_get(
            &lease,
            "https://[2606:4700:4700::1111]junk",
            WELL_KNOWN,
            &resolver,
            &connector,
            MAX_BODY,
        ),
        Err(SafeDialError::MalformedOrigin)
    );
    // No closing bracket at all.
    assert_eq!(
        safe_https_get(
            &lease,
            "https://[2606:4700:4700::1111",
            WELL_KNOWN,
            &resolver,
            &connector,
            MAX_BODY,
        ),
        Err(SafeDialError::MalformedOrigin)
    );
    // Non-numeric port.
    assert_eq!(
        safe_https_get(
            &lease,
            "https://anchor.example:https",
            WELL_KNOWN,
            &resolver,
            &connector,
            MAX_BODY,
        ),
        Err(SafeDialError::MalformedOrigin)
    );
}

#[test]
fn refuses_empty_host_with_port_only_authority() {
    let lease = fresh_lease();
    let resolver = FixedResolver::new(vec![]);
    let connector = ScriptedConnector::ok();
    assert_eq!(
        safe_https_get(
            &lease,
            "https://:443",
            WELL_KNOWN,
            &resolver,
            &connector,
            MAX_BODY
        ),
        Err(SafeDialError::EmptyHost)
    );
}

// ===========================================================================
// Display / error surface (each refusal renders a distinct, non-empty message).
// ===========================================================================

#[test]
fn ip_rejection_and_dial_errors_render() {
    use std::error::Error;

    for r in [
        IpRejection::Unspecified,
        IpRejection::Loopback,
        IpRejection::Private,
        IpRejection::LinkLocal,
        IpRejection::Multicast,
        IpRejection::Reserved,
    ] {
        assert!(!r.to_string().is_empty());
    }

    let errs = [
        SafeDialError::NotHttps,
        SafeDialError::EmptyHost,
        SafeDialError::OriginTooLong,
        SafeDialError::MalformedOrigin,
        SafeDialError::NonStandardPort(8443),
        SafeDialError::ResolutionFailed("nxdomain".into()),
        SafeDialError::NoAddresses,
        SafeDialError::RejectedAddress(IpRejection::Private),
        SafeDialError::DnsRebinding {
            pinned: public_ip(),
            connected: "10.0.0.1".parse().unwrap(),
        },
        SafeDialError::CertificateInvalid,
        SafeDialError::RedirectRefused(301),
        SafeDialError::UnexpectedStatus(503),
        SafeDialError::ResponseTooLarge {
            limit: 1,
            actual: 2,
        },
        SafeDialError::LeaseInactive,
        SafeDialError::Transport("reset".into()),
    ];
    for e in &errs {
        assert!(!e.to_string().is_empty());
        assert!(e.source().is_none());
    }
}

#[test]
fn bootstrap_errors_render() {
    use std::error::Error;

    let errs = [
        BootstrapError::Ticket(AuthorityError::ExpiredTicket),
        BootstrapError::EndpointHintMismatch,
        BootstrapError::OriginHintMismatch,
        BootstrapError::Dial(SafeDialError::CertificateInvalid),
    ];
    for e in &errs {
        assert!(!e.to_string().is_empty());
        assert!(e.source().is_none());
    }
}
