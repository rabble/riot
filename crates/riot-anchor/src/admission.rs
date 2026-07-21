//! Increment-1 accepted-connection admission.
//!
//! This is deliberately small: a bounded concurrent-session permit, reusing
//! `riot-transport`'s [`AlpnRouter`] envelope (one session permit per
//! connection, released the instant the session ends; exactly one
//! bidirectional application stream; the full handshake/first-frame/progress/
//! frame/idle/absolute deadline ladder — see `riot_transport::router`). The
//! full WU-019 ingress DoS-hardening tail — the 82 config values, slow-loris/
//! header/Range/compression/keep-alive limits, HTTP/TLS ingress — is
//! explicitly OUT OF SCOPE for this increment; see
//! `docs/coordination/2026-07-20-anchor-runnability-gap.md`.

use riot_transport::router::AlpnRouter;

/// The default concurrent-session ceiling for increment 1: enough headroom for
/// a handful of simultaneous control round-trips with no operator tuning.
/// Per-partition fairness / load shedding is future work (the full ingress
/// DoD).
pub const DEFAULT_MAX_CONCURRENT_SESSIONS: usize = 64;

/// Build a router with the increment-1 admission bound: at most
/// `max_concurrent_sessions` connections are ever mid-session at once, beyond
/// which a new connection is refused (`riot_transport::TransportError::Busy`)
/// before any protocol work runs. Callers register protocol handlers on the
/// returned router (see [`crate::daemon`]).
#[must_use]
pub fn bounded_router(max_concurrent_sessions: usize) -> AlpnRouter {
    AlpnRouter::new(max_concurrent_sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_router_starts_with_every_permit_available() {
        let router = bounded_router(4);
        assert_eq!(router.available_permits(), 4);
        assert!(router.alpns().is_empty(), "no handlers registered yet");
    }

    #[test]
    fn default_ceiling_is_a_sane_positive_bound() {
        assert!(DEFAULT_MAX_CONCURRENT_SESSIONS > 0);
    }
}
