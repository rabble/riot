//! The signed `onion=` ticket extension: a v3 onion service address the root
//! key attests, so a Tor-capable follower knows it is reaching the *site owner's*
//! onion and not an attacker's. These are the security proof for the
//! backward-compatible signed-format change, mirroring `site_ticket_url.rs`:
//!
//! - the onion address is covered by the root signature (cannot be stripped or
//!   forged without the root key), UNLIKE the unsigned `node=` iroh hint;
//! - an old ticket (minted before `onion` existed) still verifies byte-identically;
//! - the onion address never influences the fail-closed transport gate on its own
//!   (the gate keys off `require`, not `onion`), but a `require:arti` ticket with
//!   `onion` + a Tor-capable client now admits instead of always refusing.

use riot_core::site::ticket::{admit_dial, mint, parse, Capabilities, TransportBlocked};

fn root_key() -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])
}

const NS: [u8; 32] = [0x11; 32];
const DIGEST: [u8; 32] = [0x22; 32];
const IROH_ONLY: Capabilities = Capabilities {
    iroh: true,
    arti: false,
};
const TOR_CAPABLE: Capabilities = Capabilities {
    iroh: true,
    arti: true,
};

/// A well-formed v3 onion service id: 32 bytes base32-encoded = 52 chars, plus
/// the 6-char checksum/version suffix = 56 chars. We use a stable 56-char
/// fixture; the ticket treats it as an opaque attested string.
const ONION: &str = "abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijk";

#[test]
fn a_ticket_minted_without_onion_verifies_and_carries_none() {
    // BACKWARD-COMPAT: a ticket from before `onion` existed (None) has a canonical
    // byte-identical to today, so it still verifies; it encodes no onion= and
    // round-trips as None.
    let ticket = mint(&root_key(), NS, "none", 1, 10_000, DIGEST, None, None, None);
    assert!(ticket.verify(), "a no-onion ticket verifies unchanged");
    assert!(ticket.onion.is_none());
    assert!(
        !ticket.encode().contains("onion="),
        "no onion field is emitted for a None onion"
    );
    assert_eq!(
        parse(&ticket.encode()).unwrap(),
        ticket,
        "round-trips as None"
    );
}

#[test]
fn a_ticket_with_onion_verifies_and_round_trips() {
    let ticket = mint(
        &root_key(),
        NS,
        "none",
        1,
        10_000,
        DIGEST,
        None,
        None,
        Some(ONION.into()),
    );
    assert!(ticket.verify(), "a signed onion verifies");
    assert_eq!(ticket.onion.as_deref(), Some(ONION));
    let reparsed = parse(&ticket.encode()).unwrap();
    assert_eq!(reparsed, ticket, "onion round-trips through encode/parse");
    assert!(reparsed.verify(), "the reparsed ticket still verifies");
}

#[test]
fn stripping_the_signed_onion_breaks_the_signature() {
    // STRIP: an attacker cannot downgrade an onion-ticket to no-onion — the
    // verifier recomputes the canonical WITHOUT the onion, but the signature was
    // over the canonical WITH it, so verification fails.
    let signed = mint(
        &root_key(),
        NS,
        "none",
        1,
        10_000,
        DIGEST,
        None,
        None,
        Some(ONION.into()),
    );
    assert!(signed.verify());

    let mut stripped = signed.clone();
    stripped.onion = None;
    assert!(
        !stripped.verify(),
        "stripping the signed onion must break the signature"
    );
    assert!(
        matches!(
            admit_dial(&stripped, &TOR_CAPABLE, 1_000, 0),
            Err(TransportBlocked::BadSignature)
        ),
        "the gate refuses an onion-stripped ticket"
    );
}

#[test]
fn relabeling_the_signed_onion_as_url_breaks_the_signature() {
    // RELABEL: optional signed fields must be domain-separated. An attacker
    // cannot move the onion value into `url=` and thereby make transport
    // selection believe the signed ticket has no onion.
    let signed = mint(
        &root_key(),
        NS,
        "none",
        1,
        10_000,
        DIGEST,
        None,
        None,
        Some(ONION.into()),
    );
    assert!(signed.verify());

    let mut relabeled = signed;
    relabeled.url = relabeled.onion.take();
    assert!(
        !relabeled.verify(),
        "moving onion into url must change the signed canonical payload"
    );
    assert!(
        matches!(
            admit_dial(&relabeled, &TOR_CAPABLE, 1_000, 0),
            Err(TransportBlocked::BadSignature)
        ),
        "the gate refuses a ticket whose signed onion was relabeled as url"
    );
}

#[test]
fn forging_an_onion_onto_a_ticket_breaks_the_signature() {
    // FORGE: an attacker who does not hold the root key cannot ADD or SUBSTITUTE
    // an onion — doing so changes the canonical away from what the root signed.
    // This is the load-bearing property: unlike the unsigned `node=` hint, an
    // attacker cannot redirect a follower to their own onion service.
    let old = mint(&root_key(), NS, "none", 1, 10_000, DIGEST, None, None, None);
    assert!(old.verify());

    let mut forged = old.clone();
    forged.onion = Some(ONION.into());
    assert!(
        !forged.verify(),
        "adding an onion without the root key must break the signature"
    );
}

#[test]
fn substituting_the_onion_breaks_the_signature() {
    // SUBSTITUTE: an attacker who sees a legitimately-signed onion ticket cannot
    // swap the onion address for their own while keeping the signature valid.
    let signed = mint(
        &root_key(),
        NS,
        "none",
        1,
        10_000,
        DIGEST,
        None,
        None,
        Some(ONION.into()),
    );
    assert!(signed.verify());

    let mut swapped = signed.clone();
    swapped.onion = Some("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".into());
    assert!(
        !swapped.verify(),
        "swapping the signed onion must break the signature"
    );
}

#[test]
fn require_arti_with_onion_and_tor_caps_admits() {
    // THE PAYOFF: a `require:arti` ticket that carries an attested onion address
    // now ADMITS for a Tor-capable client, instead of always refusing. Before the
    // onion/tor work, `admit_dial` refused every `require:arti` ticket because no
    // client ever set `arti: true`. This is the gate-flip the transport wiring
    // relies on.
    let ticket = mint(
        &root_key(),
        NS,
        "arti",
        1,
        10_000,
        DIGEST,
        None,
        None,
        Some(ONION.into()),
    );
    assert!(
        admit_dial(&ticket, &TOR_CAPABLE, 1_000, 0).is_ok(),
        "require:arti + onion + tor caps admits"
    );
    // But an iroh-only client is still refused, even with an onion present.
    assert!(
        matches!(
            admit_dial(&ticket, &IROH_ONLY, 1_000, 0),
            Err(TransportBlocked::RequiresUnavailableTransport(_))
        ),
        "require:arti still blocks an iroh-only client"
    );
}

#[test]
fn onion_alone_does_not_flip_the_gate_for_floor_none() {
    // NO-GATE-INVERSION: the presence of an onion never changes the admit_dial
    // decision for a floor:none ticket — an iroh-only client still dials a
    // floor:none ticket whether or not an onion is present. The onion is an
    // attested address, not a gate; selection between iroh and Tor happens above
    // the gate, in the transport layer.
    let ticket = mint(
        &root_key(),
        NS,
        "none",
        1,
        10_000,
        DIGEST,
        None,
        None,
        Some(ONION.into()),
    );
    assert!(
        admit_dial(&ticket, &IROH_ONLY, 1_000, 0).is_ok(),
        "floor none + iroh caps dials regardless of an onion address"
    );
}
