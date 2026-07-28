//! Drives `riot/sync/2` responder sessions inside the single-writer actor
//! thread.
//!
//! Network handlers never touch the repository: they forward decoded events as
//! [`SyncJob`]s and write back whatever frames the actor tells them to. The
//! table owns the live [`Sync2Session`]s over the shared `!Send` repository
//! cell; the frame codec is the canonical [`Sync2Frame`] encoding (the same
//! entry points the `riot-anchor-protocol` sync2 tests use) — the driver
//! invents no framing and hand-rolls no admission check: every authority and
//! lifecycle gate lives in [`AnchorSyncRepository`].

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use tokio::sync::oneshot;

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord};
use riot_anchor_protocol::sync2::{Sync2Action, Sync2Frame, Sync2Session, MAX_SYNC2_FRAME_BYTES};

use crate::sync_service::{AnchorSyncRepository, SharedRepo};
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

/// All live responder sessions. Lives on the actor thread (it holds the `Rc`
/// repository cell inside each session, so it is deliberately `!Send`).
pub struct SyncSessionTable {
    sessions: HashMap<u64, Sync2Session<AnchorSyncRepository>>,
    /// Session ids that finished a FULL successful exchange.
    completed: BTreeSet<u64>,
    max_sessions: usize,
}

impl SyncSessionTable {
    /// `max_sessions` bounds concurrently live sessions and mirrors the ingress
    /// config value (`config.ingress.max_concurrent_control_sessions` until a
    /// dedicated sync ceiling exists).
    #[must_use]
    pub fn new(max_sessions: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            completed: BTreeSet::new(),
            max_sessions,
        }
    }

    /// Dispatch one job: `Open`/`Frame`/`Close` route to the three methods
    /// below; the reply is always sent, even on internal error (terminated).
    pub fn handle(&mut self, shared: &SharedRepo, token_ring: &TokenSecretRing, job: SyncJob) {
        let mut outbound = Vec::new();
        let terminated = match job.event {
            SyncEvent::Open { frame, now } => !self.open(
                shared,
                token_ring,
                job.session_id,
                frame,
                now,
                &mut outbound,
            ),
            SyncEvent::Frame { frame } => self.frame(shared, job.session_id, frame, &mut outbound),
            SyncEvent::Close => {
                self.close(job.session_id);
                true
            }
        };
        // The handler may have gone away (peer dropped); that is fine.
        let _ = job.reply.send(SyncReply {
            outbound,
            terminated,
        });
    }

    /// Route a session's first frame. Returns `true` iff a live session was
    /// stored: a capacity refusal (empty outbound), a non-`OpenNamespace` or
    /// undecodable frame (empty outbound), or a routing refusal from the
    /// repository (the `Refuse` frame is in `outbound`) all return `false`.
    pub fn open(
        &mut self,
        shared: &SharedRepo,
        token_ring: &TokenSecretRing,
        session_id: u64,
        frame: Vec<u8>,
        now: u64,
        outbound: &mut Vec<Vec<u8>>,
    ) -> bool {
        // Session ceiling first: refuse before decoding anything.
        if self.sessions.len() >= self.max_sessions || self.sessions.contains_key(&session_id) {
            return false;
        }
        // The canonical frame codec — the same decode entry point the protocol
        // crate's own sync2 tests use.
        let open = match decode_canonical::<Sync2Frame>(&frame, MAX_SYNC2_FRAME_BYTES) {
            Ok(frame @ Sync2Frame::OpenNamespace(_)) => frame,
            // Not an OpenNamespace (or not a canonical frame): terminate with
            // nothing served.
            Ok(_) | Err(_) => return false,
        };
        // Every admission decision is the adapter's: the driver only shuttles.
        let repo = AnchorSyncRepository::new(Rc::clone(shared), token_ring.clone(), now);
        let mut session = Sync2Session::responder(repo);
        let mut actions = session.start();
        actions.extend(session.on_frame(open));
        if !collect_sends(actions, outbound) {
            return false;
        }
        if session.is_terminated() {
            // A refusal at open (its `Refuse` frame is already in `outbound`),
            // or — degenerately — an instant completion.
            if session.is_complete() {
                self.completed.insert(session_id);
            }
            return false;
        }
        self.sessions.insert(session_id, session);
        true
    }

    /// Feed a subsequent frame. Returns `true` when the session is finished
    /// after this frame (complete, refused, codec-dead, or unknown) — the
    /// handler flushes `outbound` and closes.
    pub fn frame(
        &mut self,
        _shared: &SharedRepo,
        session_id: u64,
        frame: Vec<u8>,
        outbound: &mut Vec<Vec<u8>>,
    ) -> bool {
        let Some(session) = self.sessions.get_mut(&session_id) else {
            return true;
        };
        let decoded = match decode_canonical::<Sync2Frame>(&frame, MAX_SYNC2_FRAME_BYTES) {
            Ok(frame) => frame,
            Err(_) => {
                // A non-canonical frame kills the session fail-closed.
                self.sessions.remove(&session_id);
                return true;
            }
        };
        // `Admit`/`PromoteDirection`/`Complete` are internal FSM actions already
        // applied by the session/repository — the driver only transmits `Send`s.
        let actions = session.on_frame(decoded);
        let encoded_ok = collect_sends(actions, outbound);
        let finished = !encoded_ok || session.is_terminated();
        if finished {
            let complete = encoded_ok && session.is_complete();
            self.sessions.remove(&session_id);
            if complete {
                self.completed.insert(session_id);
            }
        }
        finished
    }

    /// The connection ended: drop ALL of the session's state — the live
    /// session AND its completion marker. The daemon handler's close guard
    /// delivers a `Close` for every session id on every exit path (ids are
    /// never reused), so pruning here bounds the `completed` set: it holds
    /// only sessions whose connection is still draining.
    pub fn close(&mut self, session_id: u64) {
        self.sessions.remove(&session_id);
        self.completed.remove(&session_id);
    }

    /// True when `session_id` finished a full successful exchange. Completion
    /// state lives only until the session's `Close` arrives (see
    /// [`Self::close`]) — callers (tests) must query BEFORE closing.
    #[must_use]
    pub fn is_complete(&self, session_id: u64) -> bool {
        self.completed.contains(&session_id)
    }
}

/// Encode every `Send` action into `outbound`, in order. Returns `false` on an
/// encoding fault (a frame the FSM built failed canonical encoding — should be
/// unreachable; the session is then terminated fail-closed by the caller).
fn collect_sends(actions: Vec<Sync2Action>, outbound: &mut Vec<Vec<u8>>) -> bool {
    for action in actions {
        if let Sync2Action::Send(frame) = action {
            match frame.encode_canonical() {
                Ok(bytes) => outbound.push(bytes),
                Err(_) => return false,
            }
        }
    }
    true
}
