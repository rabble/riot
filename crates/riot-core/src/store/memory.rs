//! In-memory persistence adapter retained as the conformance oracle.

use super::evidence::{EvidenceMutation, EvidenceSnapshot};
use crate::session::SessionError;

#[derive(Default)]
pub(crate) struct MemoryEvidenceStore;

impl MemoryEvidenceStore {
    pub(crate) fn load(&self) -> Result<EvidenceSnapshot, SessionError> {
        Ok(EvidenceSnapshot::empty())
    }

    pub(crate) fn persist(&self, _mutation: &EvidenceMutation) -> Result<(), SessionError> {
        Ok(())
    }
}
