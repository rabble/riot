//! Transport selection — the policy above the fail-closed `admit_dial` gate.
//!
//! Given an admissible ticket and the client's [`Capabilities`],
//! [`select_transport`] decides whether a follower dials over Tor or iroh. The
//! gate (`admit_dial`) has already refused impossible combinations (a
//! `require:arti` ticket for an iroh-only client never reaches here); selection
//! picks among the admissible transports.
//!
//! ## Rule
//!
//! - `require:arti` ⇒ **Tor** (the gate guarantees the client is Tor-capable).
//!   iroh never falls back — that is the activist-safety property the signed
//!   floor exists to enforce.
//! - `onion.is_some() && caps.arti` ⇒ **Tor** (prefer the privacy-protecting
//!   path when the site attests an onion and the client can use it).
//! - an unknown floor ⇒ **Neither** (selection fails closed; the gate would
//!   already have refused, but selection mirrors that conservatism rather than
//!   guessing iroh).
//! - otherwise ⇒ **Iroh** (the fast, easy default).

use crate::ticket::{Capabilities, Floor, Ticket};

/// Which transport a follower should dial over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportChoice {
    /// Dial over Tor (Arti) using the ticket's attested onion address.
    Tor,
    /// Dial over iroh using the ticket's node hint.
    Iroh,
    /// Neither transport is admissible (e.g. an unknown floor). Selection fails
    /// closed; the caller surfaces a refusal rather than guessing.
    Neither,
}

/// Select the transport for an admissible ticket given client capabilities.
///
/// Callers MUST still run `admit_dial` first: this function assumes the ticket
/// is one the gate would allow (signature valid, not expired, floor satisfied).
/// It does not re-check the signature. An unknown floor returns `Neither`.
pub fn select_transport(ticket: &Ticket, caps: &Capabilities) -> TransportChoice {
    match ticket.floor() {
        // require:arti — the gate has already ensured the client is Tor-capable
        // (an iroh-only client never reaches here). Onion-only transport.
        Floor::Arti => TransportChoice::Tor,
        // An unrecognized floor: refuse to guess. (The gate's UnknownFloor
        // refusal precedes this, but selection is conservative on its own.)
        Floor::Unknown(_) => TransportChoice::Neither,
        // floor:none — prefer Tor when the site attests an onion and the client
        // can use it; otherwise iroh.
        Floor::None => {
            if ticket.onion.is_some() && caps.arti {
                TransportChoice::Tor
            } else {
                TransportChoice::Iroh
            }
        }
    }
}
