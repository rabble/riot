//! Minimal bounded-ingress guard for the anchor control plane (WU-019 increment
//! 1).
//!
//! For increment 1 the ingress envelope is deliberately small and rests on
//! bounds the transport and protocol layers already enforce:
//!
//! * **Per-connection resource bounds** — the router ([`riot_transport::router`])
//!   holds every stalled/abusive peer to one session permit, one bidirectional
//!   stream, and the full deadline ladder (handshake / first-frame / progress /
//!   whole-frame / idle / absolute).
//! * **Per-request byte cap** — the control service rejects any frame larger than
//!   [`MAX_CONTROL_FRAME_BYTES`](riot_anchor_protocol::control::MAX_CONTROL_FRAME_BYTES)
//!   as a bounded protocol failure before any durable work.
//! * **Concurrency cap** — [`IngressLimits::max_concurrent_control_sessions`]
//!   sizes the router's session semaphore, so the anchor admits at most that many
//!   in-flight control sessions and refuses the rest as `Busy` without allocating.
//!
//! DEFERRED(WU-019 increment 2): full ingress shedding — per-source rate limits,
//! global-headroom back-pressure feeding the admission `busy`/`over_quota`
//! refusals, and adaptive pressure-band difficulty — is later scope. This module
//! intentionally does NOT implement those; it only sizes the concurrency ceiling.

/// The bounded-ingress ceilings the daemon applies at accept time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngressLimits {
    /// The maximum number of concurrent `riot/anchor/1` control sessions. Sizes
    /// the router's session semaphore; excess connections are refused as `Busy`.
    pub max_concurrent_control_sessions: usize,
}

impl IngressLimits {
    /// The conservative increment-1 default concurrency ceiling.
    pub const DEFAULT_MAX_CONTROL_SESSIONS: usize = 256;

    /// Construct limits with an explicit concurrency ceiling. A zero ceiling is
    /// clamped to one so the anchor always admits at least one session.
    #[must_use]
    pub fn new(max_concurrent_control_sessions: usize) -> Self {
        Self {
            max_concurrent_control_sessions: max_concurrent_control_sessions.max(1),
        }
    }
}

impl Default for IngressLimits {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_CONTROL_SESSIONS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_the_documented_ceiling() {
        assert_eq!(
            IngressLimits::default().max_concurrent_control_sessions,
            IngressLimits::DEFAULT_MAX_CONTROL_SESSIONS
        );
    }

    #[test]
    fn zero_ceiling_is_clamped_to_one() {
        assert_eq!(IngressLimits::new(0).max_concurrent_control_sessions, 1);
    }

    #[test]
    fn explicit_ceiling_is_preserved() {
        assert_eq!(IngressLimits::new(12).max_concurrent_control_sessions, 12);
    }
}
