//! `ActionReceiptV1` authorization sidecars + per-(actor,receiver) hash chains.

use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use super::record::RecordId;
use super::GovernanceError;

const ACTION_HASH_DOMAIN: &[u8] = b"riot/governance-action-hash/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionReceiptV1 {
    pub entry_id: [u8; 32],
    pub capability_fingerprint: [u8; 32],
    pub actor_id: [u8; 32],
    pub receiver: [u8; 32],
    pub actor_sequence: u64,
    pub previous_action_hash: Option<[u8; 32]>,
    pub policy_frontier_hash: [u8; 32],
}

type EncErr = minicbor::encode::Error<core::convert::Infallible>;

/// Canonical `ActionReceiptV1`: a definite 7-entry map with strictly ascending
/// integer keys 0..=6, one per field in declaration order (the record/body
/// discipline). Key 5 is `null` when there is no previous action.
pub fn encode_receipt(r: &ActionReceiptV1) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut e = Encoder::new(&mut buf);
    let go: Result<(), EncErr> = (|| {
        e.map(7)?;
        e.u64(0)?.bytes(&r.entry_id)?;
        e.u64(1)?.bytes(&r.capability_fingerprint)?;
        e.u64(2)?.bytes(&r.actor_id)?;
        e.u64(3)?.bytes(&r.receiver)?;
        e.u64(4)?.u64(r.actor_sequence)?;
        e.u64(5)?;
        match &r.previous_action_hash {
            Some(h) => {
                e.bytes(h)?;
            }
            None => {
                e.null()?;
            }
        }
        e.u64(6)?.bytes(&r.policy_frontier_hash)?;
        Ok(())
    })();
    go.expect("in-memory encode cannot fail");
    buf
}

fn dkey(d: &mut Decoder<'_>, want: u64) -> Result<(), GovernanceError> {
    if d.u64().map_err(|_| GovernanceError::Malformed)? != want {
        return Err(GovernanceError::Malformed);
    }
    Ok(())
}

fn d32(d: &mut Decoder<'_>) -> Result<[u8; 32], GovernanceError> {
    let b = d.bytes().map_err(|_| GovernanceError::Malformed)?;
    <[u8; 32]>::try_from(b).map_err(|_| GovernanceError::Malformed)
}

pub fn decode_receipt(bytes: &[u8]) -> Result<ActionReceiptV1, GovernanceError> {
    let mut d = Decoder::new(bytes);
    match d.map().map_err(|_| GovernanceError::Malformed)? {
        Some(7) => {}
        _ => return Err(GovernanceError::Malformed),
    }
    dkey(&mut d, 0)?;
    let entry_id = d32(&mut d)?;
    dkey(&mut d, 1)?;
    let capability_fingerprint = d32(&mut d)?;
    dkey(&mut d, 2)?;
    let actor_id = d32(&mut d)?;
    dkey(&mut d, 3)?;
    let receiver = d32(&mut d)?;
    dkey(&mut d, 4)?;
    let actor_sequence = d.u64().map_err(|_| GovernanceError::Malformed)?;
    dkey(&mut d, 5)?;
    let previous_action_hash =
        if d.datatype().map_err(|_| GovernanceError::Malformed)? == minicbor::data::Type::Null {
            d.null().map_err(|_| GovernanceError::Malformed)?;
            None
        } else {
            Some(d32(&mut d)?)
        };
    dkey(&mut d, 6)?;
    let policy_frontier_hash = d32(&mut d)?;
    if d.position() != bytes.len() {
        return Err(GovernanceError::TrailingBytes);
    }
    let r = ActionReceiptV1 {
        entry_id,
        capability_fingerprint,
        actor_id,
        receiver,
        actor_sequence,
        previous_action_hash,
        policy_frontier_hash,
    };
    // Canonicality proof: re-encoding must reproduce the input byte-for-byte.
    if encode_receipt(&r) != bytes {
        return Err(GovernanceError::Malformed);
    }
    Ok(r)
}

pub fn action_hash(r: &ActionReceiptV1) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(ACTION_HASH_DOMAIN);
    h.update(encode_receipt(r));
    h.finalize().into()
}

/// Validate the per-(actor,receiver) receipt chain. `privileged_actions` is the
/// set of genuine (non-receipt) privileged-action entry ids that REQUIRE a
/// paired receipt; `genesis_action` (if any) is the single exempt base case.
pub fn validate_action_chain(
    receipts: &[ActionReceiptV1],
    privileged_actions: &BTreeSet<RecordId>,
    genesis_action: Option<RecordId>,
) -> Result<(), GovernanceError> {
    let mut sorted: Vec<&ActionReceiptV1> = receipts.iter().collect();
    sorted.sort_by_key(|r| r.actor_sequence);
    // The set of THIS batch's receipt action-hashes. A receipt whose `entry_id`
    // is one of these is naming a receipt as its action (receipt-of-receipt, or
    // the degenerate self-reference). This guard is CONSTRUCTABLE and isolable
    // from the genuine-action-set check below. (The narrower `entry_id ==
    // action_hash(self)` fixed point is a SHA-256 preimage and unconstructable —
    // it is the harmless degenerate member of this set, not a separate guard, so
    // no phantom code is shipped: the Slice-1 `NonCanonical` lesson.)
    let receipt_hashes: BTreeSet<[u8; 32]> = sorted.iter().map(|r| action_hash(r)).collect();
    let mut prev: Option<[u8; 32]> = None;
    let mut paired: BTreeSet<[u8; 32]> = BTreeSet::new();
    for (i, r) in sorted.iter().enumerate() {
        // (a) entry must be a genuine action (rejects an entry that is not a
        //     recognized privileged action at all).
        if !privileged_actions.contains(&r.entry_id) {
            return Err(GovernanceError::ActionChainInvalid);
        }
        // (b) entry must NOT be a receipt's own action-hash (rejects naming a
        //     receipt — or itself — as the action, ISOLATED from (a)).
        if receipt_hashes.contains(&r.entry_id) {
            return Err(GovernanceError::ActionChainInvalid);
        }
        if !paired.insert(r.entry_id) {
            return Err(GovernanceError::ActionChainInvalid);
        } // one receipt ↔ one action
        if r.actor_sequence != i as u64 || r.previous_action_hash != prev {
            return Err(GovernanceError::ActionChainInvalid); // swapped / tampered link
        }
        prev = Some(action_hash(r));
    }
    // MISSING-PAIR: every privileged action except the genesis base case must
    // have a paired receipt.
    for action in privileged_actions {
        if Some(*action) != genesis_action && !paired.contains(action) {
            return Err(GovernanceError::ActionChainInvalid);
        }
    }
    Ok(())
}

// Seeded builders shared with Task 11 (cutoff) and Task 14 (vectors).
#[cfg(any(test, feature = "conformance"))]
pub fn action_receipt_chain(n: usize) -> (Vec<ActionReceiptV1>, BTreeSet<RecordId>) {
    let mut receipts = Vec::with_capacity(n);
    let mut ids: BTreeSet<RecordId> = BTreeSet::new();
    let mut prev: Option<[u8; 32]> = None;
    for i in 0..n {
        let mut entry_id = [0u8; 32];
        entry_id[0] = 0xA0;
        entry_id[1] = i as u8;
        let r = ActionReceiptV1 {
            entry_id,
            capability_fingerprint: [0x11; 32],
            actor_id: [7u8; 32],
            receiver: [8u8; 32],
            actor_sequence: i as u64,
            previous_action_hash: prev,
            policy_frontier_hash: [0x22; 32],
        };
        prev = Some(action_hash(&r));
        ids.insert(entry_id);
        receipts.push(r);
    }
    (receipts, ids)
}
#[cfg(any(test, feature = "conformance"))]
pub fn seeded_action_receipt() -> ActionReceiptV1 {
    action_receipt_chain(1).0.remove(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_three_action_chain_is_accepted() {
        let (r, ids) = action_receipt_chain(3);
        let genesis = *ids.iter().next().unwrap(); // treat first as the exempt base
        assert_eq!(validate_action_chain(&r, &ids, Some(genesis)), Ok(()));
    }
    #[test]
    fn a_privileged_action_with_no_receipt_is_rejected() {
        let (r, mut ids) = action_receipt_chain(2);
        ids.insert([0xCD; 32]); // an extra privileged action with no paired receipt
        assert_eq!(
            validate_action_chain(&r, &ids, None),
            Err(GovernanceError::ActionChainInvalid)
        );
    }
    #[test]
    fn the_genesis_base_case_needs_no_receipt() {
        let mut ids = BTreeSet::new();
        ids.insert([0x11; 32]);
        assert_eq!(validate_action_chain(&[], &ids, Some([0x11; 32])), Ok(()));
    }
    #[test]
    fn a_receipt_pointing_at_a_non_genuine_action_is_rejected_by_the_genuine_action_check() {
        let (mut r, ids) = action_receipt_chain(2);
        r[1].entry_id = [0x55; 32]; // not in `ids` → fails guard (a) only
        assert_eq!(
            validate_action_chain(&r, &ids, None),
            Err(GovernanceError::ActionChainInvalid)
        );
    }
    #[test]
    fn a_receipt_naming_a_receipt_hash_is_rejected_even_when_listed_as_an_action() {
        // ISOLATES guard (b): set entry_id to receipt-0's action-hash AND insert
        // that hash into `ids` so guard (a) PASSES — only the receipt-hash guard
        // can fire. This covers receipt-of-receipt and the degenerate self-ref.
        let (mut r, mut ids) = action_receipt_chain(2);
        let h0 = action_hash(&r[0]);
        r[1].entry_id = h0;
        ids.insert(h0);
        assert_eq!(
            validate_action_chain(&r, &ids, None),
            Err(GovernanceError::ActionChainInvalid)
        );
    }
    #[test]
    fn a_tampered_previous_action_hash_is_rejected() {
        let (mut r, ids) = action_receipt_chain(2);
        r[1].previous_action_hash = Some([0xEE; 32]);
        assert_eq!(
            validate_action_chain(&r, &ids, None),
            Err(GovernanceError::ActionChainInvalid)
        );
    }
    #[test]
    fn two_receipts_pairing_one_action_are_rejected() {
        let (mut r, mut ids) = action_receipt_chain(2);
        r[1].entry_id = r[0].entry_id;
        ids.insert(r[0].entry_id);
        assert_eq!(
            validate_action_chain(&r, &ids, None),
            Err(GovernanceError::ActionChainInvalid)
        );
    }
    #[test]
    fn receipt_round_trips_and_rejects_trailing_bytes() {
        let (r, _) = action_receipt_chain(1);
        let bytes = encode_receipt(&r[0]);
        assert_eq!(decode_receipt(&bytes).unwrap(), r[0]);
        let mut t = bytes.clone();
        t.push(0);
        assert_eq!(decode_receipt(&t), Err(GovernanceError::TrailingBytes));
    }
}
