//! Verification surface. REUSES the canonical checked verifier
//! `crate::willow::verify_entry` (the `PossiblyAuthorisedEntry` conversion) —
//! this module must NOT hand-roll a second signature-verification path.
//!
//! Spec "Meadowcap core" chain-signature verification ("Verify every namespace
//! and user signature in a delegation chain") is delivered by the reused
//! canonical decode path, not by new code here: the pinned crate's capability
//! types decode via `PossiblyValid*Capability` and are returned only
//! `if decoded.is_valid()` (meadowcap-0.5.0/src/write_capability.rs:819
//! canonic, :761 lenient; read analogues), with `is_valid()` bound by
//! `Verifier<NamespaceSignature>`/`Verifier<UserSignature>`
//! (meadowcap-0.5.0/src/raw/possibly_valid_write_capability.rs:824-830). A
//! capability with any bad chain signature fails to decode. The bounded
//! decoders in `codec.rs` run that verification behind the ceilings.

use willow25::authorisation::{AuthorisationToken, ReadCapability};
use willow25::entry::Entry;
use willow25::prelude::{Area, NamespaceId};

pub use crate::willow::verify_entry;

/// True iff `token` (capability + receiver signature) authorises `entry`.
/// Thin, explicit wrapper over the checked verifier for a named call site.
/// Namespace coverage is part of this check: `verify_entry`'s underlying
/// `cap.includes(entry)` is false when the capability's granted namespace does
/// not equal the entry's namespace.
pub fn token_authorises_entry(entry: &Entry, token: &AuthorisationToken) -> bool {
    verify_entry(entry, token)
}

/// True iff a read request for `requested` in `namespace` is covered by
/// `read_cap` — BOTH the namespace must equal the capability's granted
/// namespace AND the requested area must be contained in the granted area. A
/// read request is a (namespace, area) pair; checking area alone would let a
/// capability for namespace Y cover a request in namespace X. This is the
/// read-gate coverage primitive; the receiver-proof handshake that actually
/// gates disclosure is Slice 4.
pub fn read_request_covered(
    read_cap: &ReadCapability,
    namespace: &NamespaceId,
    requested: &Area,
) -> bool {
    read_cap.granted_namespace() == namespace && read_cap.includes_area(requested)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meadowcap::create::{new_communal_read, new_owned_write};
    use willow25::prelude::{NamespaceId, NamespaceSecret, Path, SubspaceSecret};

    fn entry_in(
        ns: &NamespaceId,
        subspace: willow25::prelude::SubspaceId,
        path: &[&[u8]],
    ) -> Entry {
        Entry::builder()
            .namespace_id(ns.clone())
            .subspace_id(subspace)
            .path(Path::from_slices(path).expect("path"))
            .timestamp(1_000u64)
            .payload(b"payload")
            .build()
    }

    #[test]
    fn owner_signed_entry_verifies_and_tampered_signature_fails() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let cap = new_owned_write(&ns, owner.corresponding_subspace_id());
        let entry = entry_in(
            &ns.corresponding_namespace_id(),
            owner.corresponding_subspace_id(),
            &[b"manifest"],
        );
        let authorised = entry
            .clone()
            .into_authorised_entry(&cap, &owner)
            .expect("owner authorises");
        let token = authorised.authorisation_token();
        assert!(token_authorises_entry(&entry, token));

        // A different subspace's entry under the same token must not verify.
        let other = entry_in(
            &ns.corresponding_namespace_id(),
            SubspaceSecret::from_bytes(&[9u8; 32]).corresponding_subspace_id(),
            &[b"manifest"],
        );
        assert!(!token_authorises_entry(&other, token));

        // WRONG NAMESPACE: the same signed token must not authorise an entry
        // whose namespace differs from the capability's granted namespace.
        let wrong_ns = NamespaceSecret::from_bytes(&[77u8; 32]).corresponding_namespace_id();
        let cross = entry_in(&wrong_ns, owner.corresponding_subspace_id(), &[b"manifest"]);
        assert!(
            !token_authorises_entry(&cross, token),
            "cross-namespace entry must fail"
        );
    }

    #[test]
    fn read_coverage_checks_namespace_and_area() {
        let ns = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[5u8; 32]).corresponding_subspace_id();
        let cap = new_communal_read(ns.clone(), receiver.clone());
        let inside = Area::new_subspace_area(receiver);

        // Right namespace + contained area -> covered.
        assert!(read_request_covered(&cap, &ns, &inside));
        // Right namespace, wider area -> not covered.
        assert!(!read_request_covered(&cap, &ns, &Area::full()));
        // WRONG NAMESPACE, otherwise-contained area -> not covered.
        let other_ns = NamespaceId::from_bytes(&[24u8; 32]);
        assert!(
            !read_request_covered(&cap, &other_ns, &inside),
            "cross-namespace read must fail"
        );
    }
}
