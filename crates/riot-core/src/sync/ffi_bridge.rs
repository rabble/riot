//! Byte-only adapter for native bindings.
//!
//! The protocol state machine remains in [`ReconcileSession`]. This adapter
//! only canonicalizes incoming/outgoing frames and holds at most one outbound
//! frame until a transport consumes it. No Willow or sync enum needs to cross
//! a language boundary.

use crate::willow::SignedWillowEntry;

use super::{decode_frame, encode_frame, ReconcileSession, SyncAction, SyncError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteSyncOutcome {
    FrameReady,
    ImportBundle(Vec<u8>),
    Rejected(u8),
    Complete,
}

pub struct ByteSyncSession {
    reconcile: ReconcileSession,
    outbound: Option<Vec<u8>>,
    terminal: bool,
}

impl ByteSyncSession {
    pub fn new(
        namespace_id: [u8; 32],
        inventory: Vec<SignedWillowEntry>,
    ) -> Result<Self, SyncError> {
        Ok(Self {
            reconcile: ReconcileSession::new(namespace_id, inventory)?,
            outbound: None,
            terminal: false,
        })
    }

    pub fn begin(&mut self) -> Result<ByteSyncOutcome, SyncError> {
        self.require_empty_outbound()?;
        let action = self.reconcile.begin()?;
        self.apply(action)
    }

    pub fn receive_bytes(&mut self, bytes: &[u8]) -> Result<ByteSyncOutcome, SyncError> {
        self.require_empty_outbound()?;
        let frame = decode_frame(bytes)?;
        let action = self.reconcile.receive(frame)?;
        self.apply(action)
    }

    pub fn import_accepted(&mut self) -> Result<ByteSyncOutcome, SyncError> {
        self.require_empty_outbound()?;
        let action = self.reconcile.import_accepted()?;
        self.apply(action)
    }

    pub fn import_rejected(&mut self, code: u8) -> Result<ByteSyncOutcome, SyncError> {
        self.require_empty_outbound()?;
        let action = self.reconcile.import_rejected(code)?;
        self.apply(action)
    }

    pub fn take_outbound_frame(&mut self) -> Option<Vec<u8>> {
        self.outbound.take()
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    fn require_empty_outbound(&self) -> Result<(), SyncError> {
        if self.outbound.is_some() {
            Err(SyncError::UnexpectedFrame)
        } else {
            Ok(())
        }
    }

    fn apply(&mut self, action: SyncAction) -> Result<ByteSyncOutcome, SyncError> {
        match action {
            SyncAction::Send(frame) => {
                self.terminal = matches!(
                    frame,
                    super::SyncFrame::Complete { .. } | super::SyncFrame::Reject { .. }
                );
                self.outbound = Some(encode_frame(&frame)?);
                Ok(ByteSyncOutcome::FrameReady)
            }
            SyncAction::ImportBundle(bytes) => Ok(ByteSyncOutcome::ImportBundle(bytes)),
            SyncAction::Rejected(code) => {
                self.terminal = true;
                Ok(ByteSyncOutcome::Rejected(code))
            }
            SyncAction::Complete => {
                self.terminal = true;
                Ok(ByteSyncOutcome::Complete)
            }
        }
    }
}
