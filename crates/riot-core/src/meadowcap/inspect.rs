//! Typed inspection facts. The capability *receiver* is returned as a distinct
//! fact from any entry `subspace_id` coordinate; in an owned namespace these
//! identities are not interchangeable (design "Meadowcap core").

use willow25::authorisation::raw::AccessMode as RawAccessMode;
use willow25::authorisation::{ReadCapability, WriteCapability};
use willow25::prelude::{Area, NamespaceId, SubspaceId};

use super::fingerprint::{read_capability_fingerprint, write_capability_fingerprint, CapabilityFingerprint};
use super::{AccessMode, CapabilityKind};

/// Immutable, typed, non-secret facts about a capability. Never exposes secret
/// material or raw capability bytes beyond the fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySummary {
    pub kind: CapabilityKind,
    pub access_mode: AccessMode,
    pub granted_namespace: NamespaceId,
    /// The capability receiver — NOT an entry subspace coordinate.
    pub receiver: SubspaceId,
    pub granted_area: Area,
    pub chain_depth: usize,
    pub fingerprint: CapabilityFingerprint,
}

fn kind_of(is_owned: bool) -> CapabilityKind {
    if is_owned {
        CapabilityKind::Owned
    } else {
        CapabilityKind::Communal
    }
}

fn access_of(raw: RawAccessMode) -> AccessMode {
    match raw {
        RawAccessMode::Read => AccessMode::Read,
        RawAccessMode::Write => AccessMode::Write,
    }
}

pub fn summarise_write(cap: &WriteCapability) -> CapabilitySummary {
    CapabilitySummary {
        kind: kind_of(cap.is_owned()),
        access_mode: access_of(cap.genesis().access_mode()),
        granted_namespace: cap.granted_namespace().clone(),
        receiver: cap.receiver().clone(),
        granted_area: cap.granted_area(),
        chain_depth: cap.delegations().len(),
        fingerprint: write_capability_fingerprint(cap),
    }
}

pub fn summarise_read(cap: &ReadCapability) -> CapabilitySummary {
    CapabilitySummary {
        kind: kind_of(cap.is_owned()),
        access_mode: access_of(cap.genesis().access_mode()),
        granted_namespace: cap.granted_namespace().clone(),
        receiver: cap.receiver().clone(),
        granted_area: cap.granted_area(),
        chain_depth: cap.delegations().len(),
        fingerprint: read_capability_fingerprint(cap),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meadowcap::create::{new_owned_read, new_owned_write};
    use willow25::prelude::{NamespaceSecret, SubspaceSecret};

    #[test]
    fn owned_write_summary_reports_write_owned_and_receiver_not_namespace() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[4u8; 32]).corresponding_subspace_id();
        let cap = new_owned_write(&ns, receiver.clone());
        let s = summarise_write(&cap);
        assert_eq!(s.kind, CapabilityKind::Owned);
        assert_eq!(s.access_mode, AccessMode::Write);
        assert_eq!(s.receiver, receiver);
        assert_eq!(s.granted_namespace, ns.corresponding_namespace_id());
        assert_eq!(s.chain_depth, 0);
        // Receiver is a subspace id; the owned namespace id is a namespace key.
        // They are distinct fact types and (here) distinct values.
        assert_ne!(s.receiver.as_bytes(), s.granted_namespace.as_bytes());
    }

    #[test]
    fn owned_read_summary_reports_read_owned_and_receiver_not_namespace() {
        // Pins summarise_read (spec line 156 read inspection), mirroring the
        // write-side summary: access mode is Read, and the receiver is a distinct
        // fact from the namespace key.
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[6u8; 32]).corresponding_subspace_id();
        let cap = new_owned_read(&ns, receiver.clone());
        let s = summarise_read(&cap);
        assert_eq!(s.kind, CapabilityKind::Owned);
        assert_eq!(s.access_mode, AccessMode::Read);
        assert_eq!(s.receiver, receiver);
        assert_eq!(s.granted_namespace, ns.corresponding_namespace_id());
        assert_eq!(s.chain_depth, 0);
        assert_ne!(s.receiver.as_bytes(), s.granted_namespace.as_bytes());
    }
}
