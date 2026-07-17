//! The pre-connection fail-closed transport gate (spec §5.1-5.3, §8) — the
//! activist-safety tests. Every failure mode must REFUSE to dial, never
//! downgrade to a leaking transport.

use ed25519_dalek::SigningKey;
use riot_transport::ticket::{admit_dial, mint, parse, Capabilities, Floor, TransportBlocked};

const NS: [u8; 32] = [0x11; 32];
const DIGEST: [u8; 32] = [0x22; 32];

fn root() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn caps(iroh: bool, arti: bool) -> Capabilities {
    Capabilities { iroh, arti }
}

#[test]
fn valid_none_floor_ticket_verifies_and_admits_an_iroh_dial() {
    let t = mint(&root(), NS, "none", 1, 10_000, DIGEST, None);
    assert!(t.verify());
    assert_eq!(t.floor(), Floor::None);
    assert_eq!(admit_dial(&t, &caps(true, false), 1_000, 0), Ok(()));
}

#[test]
fn a_tampered_signature_never_dials() {
    let mut t = mint(&root(), NS, "none", 1, 10_000, DIGEST, None);
    t.sig[0] ^= 0x01;
    assert!(!t.verify());
    assert_eq!(
        admit_dial(&t, &caps(true, false), 1_000, 0),
        Err(TransportBlocked::BadSignature)
    );
}

#[test]
fn a_flipped_require_floor_breaks_the_signature() {
    // The attacker's core move: strip require:arti -> none to leak the IP.
    let mut t = mint(&root(), NS, "arti", 1, 10_000, DIGEST, None);
    t.require_raw = "none".to_string(); // downgrade in place
    assert!(
        !t.verify(),
        "the floor is signed; flipping it breaks the sig"
    );
    assert_eq!(
        admit_dial(&t, &caps(true, false), 1_000, 0),
        Err(TransportBlocked::BadSignature)
    );
}

#[test]
fn require_arti_without_arti_fails_closed_never_falls_back_to_iroh() {
    // THE activist-safety test: an arti-only site is never dialed over iroh.
    let t = mint(&root(), NS, "arti", 1, 10_000, DIGEST, None);
    assert!(t.verify());
    assert_eq!(
        admit_dial(&t, &caps(true, false), 1_000, 0),
        Err(TransportBlocked::RequiresUnavailableTransport(
            "arti".into()
        ))
    );
}

#[test]
fn an_expired_ticket_is_refused() {
    let t = mint(&root(), NS, "none", 1, 500, DIGEST, None);
    assert_eq!(
        admit_dial(&t, &caps(true, false), 1_000, 0),
        Err(TransportBlocked::Expired)
    );
}

#[test]
fn an_epoch_below_the_durable_floor_is_a_rollback() {
    // A returning follower who has seen epoch 5 refuses a validly-signed epoch-2
    // ticket (an owner tightened require, an attacker replays the old floor).
    let t = mint(&root(), NS, "none", 2, 10_000, DIGEST, None);
    assert!(t.verify());
    assert_eq!(
        admit_dial(&t, &caps(true, false), 1_000, 5),
        Err(TransportBlocked::Rollback)
    );
}

#[test]
fn an_unknown_floor_token_fails_closed_not_parsed_as_none() {
    let t = mint(&root(), NS, "nym", 1, 10_000, DIGEST, None);
    assert!(t.verify());
    assert!(matches!(t.floor(), Floor::Unknown(_)));
    assert!(matches!(
        admit_dial(&t, &caps(true, false), 1_000, 0),
        Err(TransportBlocked::UnknownFloor(_))
    ));
}

#[test]
fn ticket_encode_parse_round_trips_and_survives_verification() {
    let t = mint(&root(), NS, "arti", 3, 10_000, DIGEST, Some("hint".into()));
    let parsed = parse(&t.encode()).expect("parse");
    assert_eq!(parsed, t);
    assert!(parsed.verify());
}
