//! Transport selection: given a ticket and the client's capabilities, decide
//! whether a follower dials over Tor or iroh. This is the policy above the
//! fail-closed `admit_dial` gate — the gate already refuses impossible
//! combinations; `select_transport` picks among the admissible ones.
//!
//! The selection rule (from the dual-transport design):
//! - `require:arti`  → MUST use Tor. iroh never falls back. (The gate enforces
//!   that a non-Tor client refuses entirely; here we only ever see admissible
//!   tickets, so require:arti ⇒ Tor by construction.)
//! - `onion.is_some() && caps.arti` → PREFER Tor. The site attests an onion
//!   address and the client can use it; Tor hides the client↔seed relationship
//!   from the iroh relay operator.
//! - otherwise → iroh (the fast, easy default).

use riot_transport::select::{select_transport, TransportChoice};
use riot_transport::ticket::{Capabilities, Floor, Ticket};

fn ticket_with(require: &str, onion: Option<&str>) -> Ticket {
    let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    riot_transport::ticket::mint(
        &key,
        [0x11; 32],
        require,
        1,
        u64::MAX,
        [0u8; 32],
        None,
        None,
        onion.map(str::to_string),
    )
}

const IROH_ONLY: Capabilities = Capabilities {
    iroh: true,
    arti: false,
};
const TOR_CAPABLE: Capabilities = Capabilities {
    iroh: true,
    arti: true,
};
const ONION: &str = "abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijk";

#[test]
fn require_arti_forces_tor_when_client_is_capable() {
    // require:arti + a Tor-capable client ⇒ Tor. This is the gate's payoff: the
    // ticket demands onion-only transport and the client can provide it.
    let t = ticket_with("arti", Some(ONION));
    assert_eq!(select_transport(&t, &TOR_CAPABLE), TransportChoice::Tor);
    assert_eq!(t.floor(), Floor::Arti, "sanity: the floor is arti");
}

#[test]
fn an_attested_onion_with_tor_caps_prefers_tor() {
    // floor:none but the site attests an onion AND the client has Tor ⇒ prefer
    // Tor. The client chooses the privacy-protecting path even though iroh is
    // admissible. This is the "privacy mode when available" rule.
    let t = ticket_with("none", Some(ONION));
    assert_eq!(
        select_transport(&t, &TOR_CAPABLE),
        TransportChoice::Tor,
        "an attested onion + tor caps prefers Tor"
    );
}

#[test]
fn an_onion_without_tor_caps_falls_back_to_iroh() {
    // The site attests an onion but the client is iroh-only ⇒ iroh. The onion
    // address is an option, not a requirement (floor is none); a client that
    // can't do Tor still reaches the seed over iroh.
    let t = ticket_with("none", Some(ONION));
    assert_eq!(
        select_transport(&t, &IROH_ONLY),
        TransportChoice::Iroh,
        "iroh-only client falls back to iroh despite an attested onion"
    );
}

#[test]
fn no_onion_uses_iroh_regardless_of_caps() {
    // No attested onion ⇒ iroh, even for a Tor-capable client. There is no Tor
    // dial target to use. (A future Tor-bridge-without-onion mode would change
    // this; today Tor requires an onion address.)
    let t = ticket_with("none", None);
    assert_eq!(
        select_transport(&t, &TOR_CAPABLE),
        TransportChoice::Iroh,
        "no onion ⇒ iroh even with tor caps"
    );
}

#[test]
fn no_onion_no_onion_floor_iroh_only_client_uses_iroh() {
    // The plain default: no onion, floor none, iroh-only client ⇒ iroh.
    let t = ticket_with("none", None);
    assert_eq!(select_transport(&t, &IROH_ONLY), TransportChoice::Iroh);
}

#[test]
fn an_unknown_floor_is_neither_transport() {
    // An unknown floor token (e.g. a future "nym") selects neither transport —
    // selection fails closed rather than guessing. (admit_dial would already
    // have refused this; select_transport mirrors that conservatism.)
    let t = ticket_with("nym", None);
    assert_eq!(
        select_transport(&t, &TOR_CAPABLE),
        TransportChoice::Neither,
        "an unknown floor selects neither transport"
    );
}
