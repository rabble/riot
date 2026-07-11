use crate::import::{decode_bundle, encode_bundle, BundleDecodeOutcome, ItemStatus};
use crate::willow::{decode_entry_canonic, entry_id, EntryId, SignedWillowEntry};
use willow25::groupings::Namespaced;

use super::{missing_entry_ids, SyncError, SyncFrame, MAX_SYNC_IDS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAction {
    Send(SyncFrame),
    ImportBundle(Vec<u8>),
    Rejected(u8),
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AfterImport {
    SendSummary,
    SendComplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    Idle,
    AwaitingInitialSummary,
    AwaitingInitialRequestOrPeerSummary,
    AwaitingPeerSummary,
    AwaitingRequestOrComplete,
    AwaitingEntries {
        expected: Vec<EntryId>,
        after_import: AfterImport,
    },
    AwaitingImportDecision(AfterImport),
    AwaitingComplete,
    Complete,
}

/// One bounded bidirectional reconciliation exchange. It owns no transport.
/// Received bundles must pass the existing import boundary before the caller
/// invokes `import_accepted`; only then may their public bytes be offered in
/// the reverse half of this exchange.
pub struct ReconcileSession {
    namespace_id: [u8; 32],
    inventory: Vec<(EntryId, SignedWillowEntry)>,
    pending_entries: Vec<SignedWillowEntry>,
    phase: Phase,
}

impl ReconcileSession {
    pub fn new(namespace_id: [u8; 32], entries: Vec<SignedWillowEntry>) -> Result<Self, SyncError> {
        if entries.len() > MAX_SYNC_IDS {
            return Err(SyncError::TooManyEntryIds);
        }
        let mut inventory = Vec::with_capacity(entries.len());
        for signed in entries {
            let entry =
                decode_entry_canonic(&signed.entry_bytes).map_err(|_| SyncError::InvalidBundle)?;
            if entry.namespace_id().as_bytes() != &namespace_id {
                return Err(SyncError::NamespaceMismatch);
            }
            inventory.push((entry_id(&signed.entry_bytes), signed));
        }
        inventory.sort_by_key(|(id, _)| *id);
        if inventory.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(SyncError::DuplicateEntryId);
        }
        let all: Vec<_> = inventory.iter().map(|(_, entry)| entry.clone()).collect();
        encode_bundle(&all).map_err(|_| SyncError::InvalidBundle)?;
        Ok(Self {
            namespace_id,
            inventory,
            pending_entries: Vec::new(),
            phase: Phase::Idle,
        })
    }

    pub fn begin(&mut self) -> Result<SyncAction, SyncError> {
        if self.phase != Phase::Idle {
            return Err(SyncError::UnexpectedFrame);
        }
        self.phase = Phase::AwaitingInitialSummary;
        Ok(SyncAction::Send(SyncFrame::Hello {
            namespace_id: self.namespace_id,
        }))
    }

    pub fn receive(&mut self, frame: SyncFrame) -> Result<SyncAction, SyncError> {
        if frame_namespace(&frame) != self.namespace_id {
            return Err(SyncError::NamespaceMismatch);
        }
        match (&self.phase, frame) {
            (Phase::Complete, _) => Err(SyncError::UnexpectedFrame),
            (_, SyncFrame::Reject { code, .. }) => {
                self.pending_entries.clear();
                self.phase = Phase::Complete;
                Ok(SyncAction::Rejected(code))
            }
            (Phase::Idle, SyncFrame::Hello { .. }) => {
                self.phase = Phase::AwaitingInitialRequestOrPeerSummary;
                Ok(self.summary_action())
            }
            (Phase::AwaitingInitialSummary, SyncFrame::Summary { entry_ids, .. }) => {
                self.request_or_send_summary(&entry_ids)
            }
            (Phase::AwaitingInitialRequestOrPeerSummary, SyncFrame::Request { entry_ids, .. }) => {
                self.send_entries(&entry_ids, Phase::AwaitingPeerSummary)
            }
            (
                Phase::AwaitingInitialRequestOrPeerSummary | Phase::AwaitingPeerSummary,
                SyncFrame::Summary { entry_ids, .. },
            ) => self.request_or_complete(&entry_ids),
            (Phase::AwaitingRequestOrComplete, SyncFrame::Request { entry_ids, .. }) => {
                self.send_entries(&entry_ids, Phase::AwaitingComplete)
            }
            (Phase::AwaitingRequestOrComplete, SyncFrame::Complete { .. })
            | (Phase::AwaitingComplete, SyncFrame::Complete { .. }) => {
                self.phase = Phase::Complete;
                Ok(SyncAction::Complete)
            }
            (
                Phase::AwaitingEntries {
                    expected,
                    after_import,
                },
                SyncFrame::Entries { bundle_bytes, .. },
            ) => {
                self.pending_entries =
                    verify_received_bundle(&bundle_bytes, self.namespace_id, expected)?;
                self.phase = Phase::AwaitingImportDecision(*after_import);
                Ok(SyncAction::ImportBundle(bundle_bytes))
            }
            _ => Err(SyncError::UnexpectedFrame),
        }
    }

    pub fn import_accepted(&mut self) -> Result<SyncAction, SyncError> {
        let Phase::AwaitingImportDecision(after_import) = self.phase else {
            return Err(SyncError::UnexpectedFrame);
        };
        self.retain_pending_entries()?;
        match after_import {
            AfterImport::SendSummary => {
                self.phase = Phase::AwaitingRequestOrComplete;
                Ok(self.summary_action())
            }
            AfterImport::SendComplete => {
                self.phase = Phase::Complete;
                Ok(SyncAction::Send(SyncFrame::Complete {
                    namespace_id: self.namespace_id,
                }))
            }
        }
    }

    pub fn import_rejected(&mut self, code: u8) -> Result<SyncAction, SyncError> {
        if !matches!(self.phase, Phase::AwaitingImportDecision(_)) {
            return Err(SyncError::UnexpectedFrame);
        }
        self.pending_entries.clear();
        self.phase = Phase::Complete;
        Ok(SyncAction::Send(SyncFrame::Reject {
            namespace_id: self.namespace_id,
            code,
        }))
    }

    fn summary_action(&self) -> SyncAction {
        SyncAction::Send(SyncFrame::Summary {
            namespace_id: self.namespace_id,
            entry_ids: self.ids(),
        })
    }

    fn ids(&self) -> Vec<EntryId> {
        self.inventory.iter().map(|(id, _)| *id).collect()
    }

    fn checked_missing(&self, remote: &[EntryId]) -> Result<Vec<EntryId>, SyncError> {
        let missing = missing_entry_ids(&self.ids(), remote)?;
        if self.inventory.len().saturating_add(missing.len()) > MAX_SYNC_IDS {
            return Err(SyncError::TooManyEntryIds);
        }
        Ok(missing)
    }

    fn request_or_send_summary(&mut self, remote: &[EntryId]) -> Result<SyncAction, SyncError> {
        let missing = self.checked_missing(remote)?;
        if missing.is_empty() {
            self.phase = Phase::AwaitingRequestOrComplete;
            Ok(self.summary_action())
        } else {
            self.phase = Phase::AwaitingEntries {
                expected: missing.clone(),
                after_import: AfterImport::SendSummary,
            };
            Ok(SyncAction::Send(SyncFrame::Request {
                namespace_id: self.namespace_id,
                entry_ids: missing,
            }))
        }
    }

    fn request_or_complete(&mut self, remote: &[EntryId]) -> Result<SyncAction, SyncError> {
        let missing = self.checked_missing(remote)?;
        if missing.is_empty() {
            self.phase = Phase::Complete;
            Ok(SyncAction::Send(SyncFrame::Complete {
                namespace_id: self.namespace_id,
            }))
        } else {
            self.phase = Phase::AwaitingEntries {
                expected: missing.clone(),
                after_import: AfterImport::SendComplete,
            };
            Ok(SyncAction::Send(SyncFrame::Request {
                namespace_id: self.namespace_id,
                entry_ids: missing,
            }))
        }
    }

    fn send_entries(
        &mut self,
        requested: &[EntryId],
        next: Phase,
    ) -> Result<SyncAction, SyncError> {
        let selected = self.select(requested)?;
        let bundle_bytes = encode_bundle(&selected).map_err(|_| SyncError::InvalidBundle)?;
        self.phase = next;
        Ok(SyncAction::Send(SyncFrame::Entries {
            namespace_id: self.namespace_id,
            bundle_bytes,
        }))
    }

    fn select(&self, requested: &[EntryId]) -> Result<Vec<SignedWillowEntry>, SyncError> {
        missing_entry_ids(&[], requested)?;
        let mut selected = Vec::with_capacity(requested.len());
        for requested_id in requested {
            let Some((_, entry)) = self.inventory.iter().find(|(id, _)| id == requested_id) else {
                return Err(SyncError::UnknownEntryId);
            };
            selected.push(entry.clone());
        }
        Ok(selected)
    }

    fn retain_pending_entries(&mut self) -> Result<(), SyncError> {
        let pending_ids: Vec<_> = self
            .pending_entries
            .iter()
            .map(|entry| entry_id(&entry.entry_bytes))
            .collect();
        for id in &pending_ids {
            if self.inventory.iter().any(|(known, _)| known == id) {
                return Err(SyncError::DuplicateEntryId);
            }
        }
        self.inventory
            .extend(pending_ids.into_iter().zip(self.pending_entries.drain(..)));
        self.inventory.sort_by_key(|(id, _)| *id);
        Ok(())
    }
}

fn verify_received_bundle(
    bytes: &[u8],
    namespace_id: [u8; 32],
    expected: &[EntryId],
) -> Result<Vec<SignedWillowEntry>, SyncError> {
    let BundleDecodeOutcome::Decoded(decoded) = decode_bundle(bytes) else {
        return Err(SyncError::InvalidBundle);
    };
    let mut received = Vec::with_capacity(decoded.items.len());
    let mut signed_entries = Vec::with_capacity(decoded.items.len());
    for item in decoded.items {
        let ItemStatus::Valid(valid) = item.status else {
            return Err(SyncError::InvalidBundle);
        };
        if valid.entry.namespace_id().as_bytes() != &namespace_id {
            return Err(SyncError::NamespaceMismatch);
        }
        received.push(valid.entry_id);
        signed_entries.push(SignedWillowEntry {
            entry_bytes: item.frame.entry_bytes().to_vec(),
            capability_bytes: item.frame.capability_bytes().to_vec(),
            signature: item
                .frame
                .signature_bytes()
                .try_into()
                .map_err(|_| SyncError::InvalidBundle)?,
            payload_bytes: item.frame.payload_bytes().to_vec(),
        });
    }
    if received != expected {
        return Err(SyncError::InvalidBundle);
    }
    Ok(signed_entries)
}

fn frame_namespace(frame: &SyncFrame) -> [u8; 32] {
    match frame {
        SyncFrame::Hello { namespace_id }
        | SyncFrame::Summary { namespace_id, .. }
        | SyncFrame::Request { namespace_id, .. }
        | SyncFrame::Entries { namespace_id, .. }
        | SyncFrame::Complete { namespace_id }
        | SyncFrame::Reject { namespace_id, .. } => *namespace_id,
    }
}
