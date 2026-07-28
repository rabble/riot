//! Attenuation-only delegation. Wraps `willow25`'s `try_delegate`, mapping its
//! opaque `InvalidCapability` to stable `MeadowcapError` codes. Delegation can
//! only narrow authority (design core principle 5, "Attenuation only").
//!
//! IMPORTANT: import `InvalidCapability` from `willow25::authorisation::raw`,
//! NEVER `meadowcap::raw` — `meadowcap` is a transitive-only dependency of the
//! workspace (re-exported through willow25) and naming it directly is E0433
//! *and* would require adding a `meadowcap` dep edge, which changes
//! `Cargo.lock` and breaks the `cargo_lock_sha256` pin in
//! `fixtures/manifest.json`. Do not add a `meadowcap` dependency.

use willow25::authorisation::raw::InvalidCapability;
use willow25::authorisation::{ReadCapability, WriteCapability};
use willow25::prelude::{Area, SubspaceId, SubspaceSecret};

use super::MeadowcapError;

/// Delegate a write capability to `new_receiver`, restricting it to `new_area`.
/// `signer` must be the current receiver's secret. Returns a new capability;
/// the input is cloned so the caller's capability is untouched.
pub fn delegate_write(
    parent: &WriteCapability,
    signer: &SubspaceSecret,
    new_area: Area,
    new_receiver: SubspaceId,
) -> Result<WriteCapability, MeadowcapError> {
    // Disambiguate the two failure causes willow25 collapses into one opaque
    // `InvalidCapability`: a wrong signer is detectable Riot-side (no new
    // willow25 API) by comparing the signer's public key to the current
    // receiver BEFORE delegating, so consumers get a stable ReceiverMismatch.
    if &signer.corresponding_subspace_id() != parent.receiver() {
        return Err(MeadowcapError::ReceiverMismatch);
    }
    let mut cap = parent.clone();
    cap.try_delegate(signer, new_area, new_receiver)
        .map_err(map_invalid)?;
    Ok(cap)
}

/// Read-capability analogue of `delegate_write`.
pub fn delegate_read(
    parent: &ReadCapability,
    signer: &SubspaceSecret,
    new_area: Area,
    new_receiver: SubspaceId,
) -> Result<ReadCapability, MeadowcapError> {
    if &signer.corresponding_subspace_id() != parent.receiver() {
        return Err(MeadowcapError::ReceiverMismatch);
    }
    let mut cap = parent.clone();
    cap.try_delegate(signer, new_area, new_receiver)
        .map_err(map_invalid)?;
    Ok(cap)
}

/// After the receiver pre-check above, the only remaining `try_delegate`
/// failure is an area that is not contained in the parent's granted area
/// (authority expansion). Map it to the stable code.
fn map_invalid(_e: InvalidCapability) -> MeadowcapError {
    MeadowcapError::AuthorityExpanding
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meadowcap::codec::{
        decode_read_capability_bounded, decode_write_capability_bounded, encode_read_capability,
    };
    use crate::meadowcap::create::{new_owned_read, new_owned_write};
    use crate::willow::{encode_capability, tai_j2000_micros_from_unix_seconds};
    use willow25::prelude::{NamespaceSecret, Path, TimeRange};

    fn micros_range(from_unix: u64, to_unix: u64) -> TimeRange {
        // MICROSECONDS, never raw seconds — see load-bearing constraint 3.
        let start = tai_j2000_micros_from_unix_seconds(from_unix).expect("start micros");
        let end = tai_j2000_micros_from_unix_seconds(to_unix).expect("end micros");
        TimeRange::new(start.into(), Some(end.into()))
    }

    #[test]
    fn valid_attenuation_narrows_area_and_moves_receiver() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let cap = new_owned_write(&ns, owner.corresponding_subspace_id());

        let editor_id = SubspaceSecret::from_bytes(&[8u8; 32]).corresponding_subspace_id();
        let area = Area::new(
            Some(editor_id.clone()),
            Path::from_slices(&[b"articles", b"news"]).expect("path"),
            micros_range(1_700_000_000, 1_800_000_000),
        );
        let delegated = delegate_write(&cap, &owner, area, editor_id.clone()).expect("attenuate");
        assert_eq!(delegated.receiver(), &editor_id);
        assert_eq!(delegated.delegations().len(), 1);
    }

    #[test]
    fn read_delegation_narrows_area_moves_receiver_and_rejects_widening() {
        // Pins BOTH halves of the read-delegation surface (spec line 156): a
        // valid read attenuation, and a widening rejection. Also exercises
        // decode_read_capability_bounded's happy path (warning a).
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let cap = new_owned_read(&ns, owner.corresponding_subspace_id());
        assert_eq!(cap.granted_area(), Area::full());

        let editor = SubspaceSecret::from_bytes(&[8u8; 32]);
        let editor_id = editor.corresponding_subspace_id();
        let narrow = Area::new(
            Some(editor_id.clone()),
            Path::from_slices(&[b"content"]).expect("path"),
            micros_range(1_700_000_000, 1_800_000_000),
        );
        let delegated =
            delegate_read(&cap, &owner, narrow.clone(), editor_id.clone()).expect("attenuate read");
        assert_eq!(delegated.receiver(), &editor_id, "receiver moved to editor");
        assert_eq!(delegated.delegations().len(), 1, "chain depth incremented");
        assert_eq!(
            delegated.granted_area(),
            narrow,
            "granted area narrowed to the delegated area"
        );
        assert_ne!(
            delegated.granted_area(),
            Area::full(),
            "granted area is no longer full"
        );

        // Happy-path bounded read decode returns the same valid capability.
        let bytes = encode_read_capability(&delegated);
        assert_eq!(
            decode_read_capability_bounded(&bytes).expect("bounded read decode"),
            delegated
        );

        // NEGATIVE: widening a delegated read cap back to full is rejected.
        let leaf = SubspaceSecret::from_bytes(&[10u8; 32]).corresponding_subspace_id();
        assert_eq!(
            delegate_read(&delegated, &editor, Area::full(), leaf),
            Err(MeadowcapError::AuthorityExpanding)
        );
    }

    #[test]
    fn wrong_signer_is_rejected_as_receiver_mismatch() {
        // The signing secret is NOT the capability's current receiver, so the
        // delegation must fail-closed with the stable ReceiverMismatch code
        // (the Riot-side pre-check), never a misleading AuthorityExpanding.
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let cap = new_owned_write(&ns, owner.corresponding_subspace_id());

        let impostor = SubspaceSecret::from_bytes(&[42u8; 32]); // not the receiver
        let target = SubspaceSecret::from_bytes(&[43u8; 32]).corresponding_subspace_id();
        let area = Area::new(
            Some(target.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            micros_range(1_700_000_000, 1_800_000_000),
        );
        assert_eq!(
            delegate_write(&cap, &impostor, area, target),
            Err(MeadowcapError::ReceiverMismatch)
        );
    }

    #[test]
    fn widening_to_full_area_is_rejected() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        // Delegate once to a narrow area, then try to widen back to full.
        let cap = new_owned_write(&ns, owner.corresponding_subspace_id());
        let mid = SubspaceSecret::from_bytes(&[9u8; 32]);
        let mid_id = mid.corresponding_subspace_id();
        let narrow = Area::new(
            Some(mid_id.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            micros_range(1_700_000_000, 1_800_000_000),
        );
        let cap = delegate_write(&cap, &owner, narrow, mid_id.clone()).expect("narrow");
        let widen = Area::full();
        assert_eq!(
            delegate_write(
                &cap,
                &mid,
                widen,
                SubspaceSecret::from_bytes(&[10u8; 32]).corresponding_subspace_id()
            ),
            Err(MeadowcapError::AuthorityExpanding)
        );
    }

    #[test]
    fn seventeen_hop_chain_is_rejected_by_bounded_decode() {
        // Depth ceiling cross-check (Task 3): build a 17-deep chain and confirm
        // decode_write_capability_bounded rejects it with ChainTooDeep.
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let mut signer = SubspaceSecret::from_bytes(&[4u8; 32]);
        let mut cap = new_owned_write(&ns, signer.corresponding_subspace_id());
        for i in 0..17u8 {
            let next = SubspaceSecret::from_bytes(&[100u8.wrapping_add(i); 32]);
            let next_id = next.corresponding_subspace_id();
            // Owned genesis grants Area::full(); every hop re-grants full (still
            // contained), so only depth grows.
            cap = delegate_write(&cap, &signer, Area::full(), next_id).expect("hop");
            signer = next;
        }
        assert_eq!(cap.delegations().len(), 17);
        let bytes = encode_capability(&cap);
        assert_eq!(
            decode_write_capability_bounded(&bytes),
            Err(MeadowcapError::ChainTooDeep { depth: 17, max: 16 })
        );
    }
}

#[cfg(test)]
mod time_unit_tests {
    use super::*;
    use crate::meadowcap::create::new_owned_write;
    use crate::willow::tai_j2000_micros_from_unix_seconds;
    use willow25::entry::Entry;
    use willow25::prelude::{NamespaceSecret, Path, TimeRange};

    fn entry_at_micros(
        ns_secret: &NamespaceSecret,
        subspace: willow25::prelude::SubspaceId,
        micros: u64,
    ) -> Entry {
        Entry::builder()
            .namespace_id(ns_secret.corresponding_namespace_id())
            .subspace_id(subspace)
            .path(Path::from_slices(&[b"articles", b"post"]).expect("path"))
            .timestamp(micros)
            .payload(b"p")
            .build()
    }

    #[test]
    fn micros_range_covers_entry_but_seconds_range_covers_nothing() {
        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
        let owner_id = owner.corresponding_subspace_id();
        let cap = new_owned_write(&ns, owner_id.clone());

        // A real entry stamped in the production unit (micros).
        let unix = 1_700_000_000u64;
        let micros = tai_j2000_micros_from_unix_seconds(unix).expect("micros");
        let entry = entry_at_micros(&ns, owner_id.clone(), micros);

        // CORRECT: a micros-domain area delegated for [unix-1day, unix+1day].
        let good_area = Area::new(
            Some(owner_id.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            TimeRange::new(
                tai_j2000_micros_from_unix_seconds(unix - 86_400)
                    .unwrap()
                    .into(),
                Some(
                    tai_j2000_micros_from_unix_seconds(unix + 86_400)
                        .unwrap()
                        .into(),
                ),
            ),
        );
        let good = delegate_write(&cap, &owner, good_area, owner_id.clone()).expect("attenuate");
        assert!(
            good.includes(&entry),
            "micros-domain cap must cover a micros entry"
        );

        // TRAP: the same window built from RAW SECONDS. J2000 micros for 2023
        // are ~7.3e17; a range ending at ~1.7e9 seconds ends astronomically
        // before the entry, so it covers zero real entries.
        let bad_area = Area::new(
            Some(owner_id.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            TimeRange::new((unix - 86_400).into(), Some((unix + 86_400).into())),
        );
        let bad = delegate_write(&cap, &owner, bad_area, owner_id).expect("attenuate");
        assert!(
            !bad.includes(&entry),
            "raw-seconds cap must cover NOTHING real"
        );
    }
}
