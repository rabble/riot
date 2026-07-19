//! The signed `url=` ticket extension (Option C HTTP-pull). These are the
//! security proof for the backward-compatible signed-format change: the url is
//! covered by the root signature, cannot be stripped or forged, an old ticket
//! (minted before url existed) still verifies byte-identically, and the url
//! never influences the fail-closed transport gate.

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

#[test]
fn a_ticket_minted_without_url_verifies_and_carries_none() {
    // BACKWARD-COMPAT: a ticket from before `url` existed (None) has a canonical
    // byte-identical to today, so it still verifies; it encodes no url= and
    // round-trips as None.
    let ticket = mint(&root_key(), NS, "none", 1, 10_000, DIGEST, None, None);
    assert!(ticket.verify(), "a no-url ticket verifies unchanged");
    assert!(ticket.url.is_none());
    assert!(
        !ticket.encode().contains("url="),
        "no url field is emitted for a None url"
    );
    assert_eq!(
        parse(&ticket.encode()).unwrap(),
        ticket,
        "round-trips as None"
    );
}

#[test]
fn a_ticket_with_url_verifies_and_round_trips() {
    let url = "https://mirror.example/site/abc.bundle";
    let ticket = mint(
        &root_key(),
        NS,
        "none",
        1,
        10_000,
        DIGEST,
        None,
        Some(url.into()),
    );
    assert!(ticket.verify(), "a signed url verifies");
    assert_eq!(ticket.url.as_deref(), Some(url));
    let reparsed = parse(&ticket.encode()).unwrap();
    assert_eq!(reparsed, ticket, "url round-trips through encode/parse");
    assert!(reparsed.verify(), "the reparsed ticket still verifies");
}

#[test]
fn stripping_the_signed_url_breaks_the_signature() {
    // STRIP: an attacker cannot downgrade a url-ticket to no-url — the verifier
    // recomputes the canonical WITHOUT the url, but the signature was over the
    // canonical WITH it, so verification fails.
    let signed = mint(
        &root_key(),
        NS,
        "none",
        1,
        10_000,
        DIGEST,
        None,
        Some("https://mirror.example/x.bundle".into()),
    );
    assert!(signed.verify());

    let mut stripped = signed.clone();
    stripped.url = None;
    assert!(
        !stripped.verify(),
        "stripping the signed url must break the signature"
    );
    assert!(
        matches!(
            admit_dial(&stripped, &IROH_ONLY, 1_000, 0),
            Err(TransportBlocked::BadSignature)
        ),
        "the gate refuses a url-stripped ticket"
    );
}

#[test]
fn forging_a_url_onto_a_ticket_breaks_the_signature() {
    // FORGE: an attacker who does not hold the root key cannot ADD a url — doing
    // so changes the canonical away from what the root signed.
    let old = mint(&root_key(), NS, "none", 1, 10_000, DIGEST, None, None);
    assert!(old.verify());

    let mut forged = old.clone();
    forged.url = Some("https://evil.example/x.bundle".into());
    assert!(
        !forged.verify(),
        "adding a url without the root key must break the signature"
    );
}

#[test]
fn the_url_never_influences_the_fail_closed_gate() {
    // NO-GATE: url is a fetch hint, not a transport-floor gate. It never flips the
    // admit_dial decision — a floor:none ticket dials whatever its url, and a
    // floor:arti ticket is still refused for an iroh-only client despite a url.
    let none_floor = mint(
        &root_key(),
        NS,
        "none",
        1,
        10_000,
        DIGEST,
        None,
        Some("https://whatever.example/x".into()),
    );
    assert!(
        admit_dial(&none_floor, &IROH_ONLY, 1_000, 0).is_ok(),
        "floor none + iroh caps dials; the url is ignored by the gate"
    );

    let arti_floor = mint(
        &root_key(),
        NS,
        "arti",
        1,
        10_000,
        DIGEST,
        None,
        Some("https://whatever.example/x".into()),
    );
    assert!(
        matches!(
            admit_dial(&arti_floor, &IROH_ONLY, 1_000, 0),
            Err(TransportBlocked::RequiresUnavailableTransport(_))
        ),
        "floor arti still blocks an iroh-only client; the url does not flip the gate"
    );
}
