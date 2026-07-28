//! Transitive capability revocation and action-chain cutoffs.

use std::collections::{BTreeMap, BTreeSet};

use super::action::{action_hash, ActionReceiptV1};
use super::body::Body;
use super::lineage::build_lineage;
use super::record::{Fingerprint, GovernanceRecordV1};
use super::RecordKind;

pub fn apply_revocations(
    records: &[GovernanceRecordV1],
    granted: BTreeSet<Fingerprint>,
) -> (BTreeSet<Fingerprint>, BTreeSet<Fingerprint>) {
    let forest = build_lineage(records);
    let mut revoked = BTreeSet::new();
    for record in records {
        if record.kind == RecordKind::CapabilityRevoked {
            if let Body::CapabilityRevoked {
                target_fingerprint, ..
            } = &record.body
            {
                revoked.extend(forest.descendants_of(*target_fingerprint));
            }
        }
    }
    let active = granted.difference(&revoked).copied().collect();
    (active, revoked)
}

/// Returns whether a write action is on or before the pinned action-chain head.
/// A missing cutoff and a non-ancestral fork are both audit-only.
pub fn is_action_active(
    action: &ActionReceiptV1,
    cutoffs: &BTreeMap<([u8; 32], [u8; 32]), [u8; 32]>,
    chain: &BTreeMap<[u8; 32], Option<[u8; 32]>>,
) -> bool {
    let Some(head) = cutoffs.get(&(action.actor_id, action.receiver)) else {
        return false;
    };
    let target = action_hash(action);
    let mut cursor = Some(*head);
    while let Some(current) = cursor {
        if current == target {
            return true;
        }
        cursor = chain.get(&current).copied().flatten();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::{
        fingerprint_of_issued, revoke_record, three_hop_lineage_records,
    };

    #[test]
    fn revoking_mid_chain_invalidates_all_descendants() {
        let (records, fps) = three_hop_lineage_records();
        let mut records = records;
        records.push(revoke_record(fps.a));
        let granted = [fps.a, fps.b, fps.c, fps.d_sibling].into_iter().collect();
        let (active, revoked) = apply_revocations(&records, granted);
        assert!(revoked.contains(&fps.a));
        assert!(revoked.contains(&fps.b));
        assert!(revoked.contains(&fps.c));
        assert!(!active.contains(&fps.a));
        assert!(!active.contains(&fps.b));
        assert!(!active.contains(&fps.c));
        assert!(active.contains(&fps.d_sibling));
    }

    #[test]
    fn revoke_wins_over_a_concurrent_grant() {
        let (records, fps) = three_hop_lineage_records();
        let mut records = records;
        records.push(revoke_record(fps.b));
        let (active, _) = apply_revocations(&records, [fps.b].into_iter().collect());
        assert!(!active.contains(&fps.b));
    }

    #[test]
    fn a_capability_outside_the_revoked_subtree_is_untouched() {
        let (records, fps) = three_hop_lineage_records();
        let mut records = records;
        records.push(revoke_record(fps.c));
        let (active, revoked) = apply_revocations(&records, [fps.a, fps.c].into_iter().collect());
        assert!(revoked.contains(&fps.c));
        assert!(!revoked.contains(&fps.a));
        assert!(active.contains(&fps.a));
        let _ = fingerprint_of_issued;
    }
}

#[cfg(test)]
mod cutoff_tests {
    use super::*;
    use crate::governance::action::action_receipt_chain;

    fn chain_map(receipts: &[ActionReceiptV1]) -> BTreeMap<[u8; 32], Option<[u8; 32]>> {
        receipts
            .iter()
            .map(|receipt| (action_hash(receipt), receipt.previous_action_hash))
            .collect()
    }

    #[test]
    fn cutoff_keeps_ancestors_and_drops_post_cutoff_descendants() {
        let (receipts, _) = action_receipt_chain(3);
        let chain = chain_map(&receipts);
        let mut cutoffs = BTreeMap::new();
        cutoffs.insert(
            (receipts[0].actor_id, receipts[0].receiver),
            action_hash(&receipts[1]),
        );
        assert!(is_action_active(&receipts[0], &cutoffs, &chain));
        assert!(is_action_active(&receipts[1], &cutoffs, &chain));
        assert!(!is_action_active(&receipts[2], &cutoffs, &chain));
    }

    #[test]
    fn a_forked_branch_not_ancestral_to_the_head_is_audit_only() {
        let (receipts, _) = action_receipt_chain(2);
        let mut fork = receipts[1].clone();
        fork.entry_id = [0x77; 32];
        fork.previous_action_hash = Some(action_hash(&receipts[0]));
        let chain = chain_map(&[receipts[0].clone(), receipts[1].clone(), fork.clone()]);
        let mut cutoffs = BTreeMap::new();
        cutoffs.insert(
            (receipts[0].actor_id, receipts[0].receiver),
            action_hash(&receipts[1]),
        );
        assert!(!is_action_active(&fork, &cutoffs, &chain));
    }

    #[test]
    fn no_cutoff_entry_means_no_active_action() {
        let (receipts, _) = action_receipt_chain(2);
        assert!(!is_action_active(
            &receipts[0],
            &BTreeMap::new(),
            &chain_map(&receipts)
        ));
    }

    #[test]
    fn classification_is_arrival_order_independent() {
        let (receipts, _) = action_receipt_chain(3);
        let mut cutoffs = BTreeMap::new();
        cutoffs.insert(
            (receipts[0].actor_id, receipts[0].receiver),
            action_hash(&receipts[1]),
        );
        let chain = chain_map(&receipts);
        let baseline: Vec<bool> = receipts
            .iter()
            .map(|action| is_action_active(action, &cutoffs, &chain))
            .collect();
        for seed in 0..8 {
            let mut shuffled = receipts.clone();
            let rotation = seed % shuffled.len();
            shuffled.rotate_left(rotation);
            assert_eq!(
                receipts
                    .iter()
                    .map(|action| is_action_active(action, &cutoffs, &chain_map(&shuffled)))
                    .collect::<Vec<_>>(),
                baseline
            );
        }
    }
}
