//! Drives `riot/sync/2` responder sessions inside the single-writer actor
//! thread.
//!
//! Network handlers never touch the repository: they forward decoded events as
//! [`SyncJob`]s and write back whatever frames the actor tells them to.
//!
//! STUB SCOPE: this module currently lands only the job/reply CONTRACT the
//! actor thread compiles against. [`SyncSessionTable`] runs no sessions yet —
//! the real table (the `Open`/`Frame`/`Close` arms driving
//! `riot_anchor_protocol::sync2::fsm::Sync2Session` over the shared repository
//! cell) lands with the sync-driver work unit and replaces the stub body. Until
//! then every job is dropped, fail-closed.

use tokio::sync::oneshot;

use crate::sync_service::SharedRepo;
use crate::work::TokenSecretRing;

/// One `sync/2` session event from a connection handler.
pub struct SyncJob {
    /// Daemon-unique id for the connection's session.
    pub session_id: u64,
    /// The event.
    pub event: SyncEvent,
    /// Frames to transmit back (empty on refusal/termination) + liveness.
    pub reply: oneshot::Sender<SyncReply>,
}

/// A session event.
pub enum SyncEvent {
    /// First frame of the session (must decode to `OpenNamespace`).
    Open {
        /// The raw frame bytes read off the wire.
        frame: Vec<u8>,
        /// The observation time (unix seconds) the session opened at.
        now: u64,
    },
    /// A subsequent frame.
    Frame {
        /// The raw frame bytes read off the wire.
        frame: Vec<u8>,
    },
    /// The connection ended; drop session state.
    Close,
}

/// The actor's answer to one event.
pub struct SyncReply {
    /// Encoded frames to write to the peer, in order.
    pub outbound: Vec<Vec<u8>>,
    /// True when the session is complete or refused — the handler closes after
    /// flushing `outbound`.
    pub terminated: bool,
}

/// All live responder sessions. Lives on the actor thread (once real sessions
/// land it holds `Rc` repository state, so it is deliberately `!Send`).
pub struct SyncSessionTable;

impl SyncSessionTable {
    /// `max_sessions` bounds concurrently live sessions and mirrors the ingress
    /// config value (`config.ingress.max_concurrent_control_sessions` until a
    /// dedicated sync ceiling exists). The stub stores nothing; the real table
    /// enforces the ceiling at `Open`.
    #[must_use]
    pub fn new(_max_sessions: usize) -> Self {
        Self
    }

    /// Dispatch one job. STUB: no `sync/2` session may exist yet, so every job
    /// is dropped — the reply sender drops with it and the connection handler
    /// observes a dead session (fail-closed, nothing is served).
    pub fn handle(&mut self, _shared: &SharedRepo, _token_ring: &TokenSecretRing, job: SyncJob) {
        drop(job);
    }
}
