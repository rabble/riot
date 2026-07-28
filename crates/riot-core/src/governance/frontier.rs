//! Governance frontier + frontier hash + topological DAG reduction. Display
//! timestamps never order governance; ordering is the causal DAG with a
//! `record_id` tiebreak, so every peer reduces the same journal identically.

use super::record::{record_id, GovernanceRecordV1, RecordId};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const FRONTIER_DOMAIN: &[u8] = b"riot/governance-frontier/v1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontier {
    pub accepted: BTreeSet<RecordId>,
}

pub fn frontier_hash(f: &Frontier) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(FRONTIER_DOMAIN);
    for id in &f.accepted {
        h.update(id);
    } // BTreeSet iterates ascending → deterministic
    h.finalize().into()
}

pub fn topological_reduce(
    records: &[GovernanceRecordV1],
) -> (Vec<GovernanceRecordV1>, Vec<GovernanceRecordV1>) {
    let mut sorted: Vec<GovernanceRecordV1> = records.to_vec();
    sorted.sort_by_key(record_id);
    let mut accepted_ids: BTreeSet<RecordId> = BTreeSet::new();
    let mut accepted = Vec::new();
    let mut remaining = sorted;
    loop {
        let mut progressed = false;
        let mut pending = Vec::new();
        for r in remaining.into_iter() {
            if r.parents.iter().all(|p| accepted_ids.contains(p)) {
                accepted_ids.insert(record_id(&r));
                accepted.push(r);
                progressed = true;
            } else {
                pending.push(r);
            }
        }
        remaining = pending;
        if !progressed || remaining.is_empty() {
            break;
        }
    }
    (accepted, remaining)
}

pub fn frontier_of(accepted: &[GovernanceRecordV1]) -> Frontier {
    Frontier {
        accepted: accepted.iter().map(record_id).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::child_record;

    fn linear() -> [GovernanceRecordV1; 3] {
        let a = child_record(&[], 1);
        let b = child_record(&[record_id(&a)], 2);
        let c = child_record(&[record_id(&b)], 3);
        [a, b, c]
    }

    #[test]
    fn dag_reduces_in_causal_order_regardless_of_input_order() {
        let [a, b, c] = linear();
        let (acc, pend) = topological_reduce(&[c.clone(), a.clone(), b.clone()]);
        assert!(pend.is_empty());
        let pos = |r: &GovernanceRecordV1| acc.iter().position(|x| x == r).unwrap();
        assert!(pos(&a) < pos(&b) && pos(&b) < pos(&c));
    }
    #[test]
    fn missing_parent_stays_pending_not_error() {
        let [_, b, c] = linear();
        let (acc, pend) = topological_reduce(&[b, c]);
        assert!(acc.is_empty() && pend.len() == 2);
    }
    #[test]
    fn display_timestamps_never_reorder() {
        // Bake the (deliberately inverted) display timestamps in BEFORE wiring
        // the parent links, so `b`'s parent reference matches `a`'s final
        // record id. `a` carries the LATER display timestamp yet must still
        // precede `b` because it is `b`'s causal parent — proving display never
        // orders the reduction. (The plan mutated display AFTER `linear()` wired
        // the links, which — since Task 2's `record_id` covers
        // `created_display_micros` — silently broke `b`'s parent edge and left
        // it unaccepted.)
        let mut a = child_record(&[], 1);
        a.created_display_micros = 9000;
        let mut b = child_record(&[record_id(&a)], 2);
        b.created_display_micros = 1000;
        let c = child_record(&[record_id(&b)], 3);
        let (acc, _) = topological_reduce(&[a.clone(), b.clone(), c]);
        let pos = |r: &GovernanceRecordV1| acc.iter().position(|x| x == r).unwrap();
        assert!(pos(&a) < pos(&b));
    }
    #[test]
    fn frontier_hash_is_order_free_and_domain_separated() {
        let [a, b, c] = linear();
        assert_eq!(
            frontier_hash(&frontier_of(&[a.clone(), b.clone(), c.clone()])),
            frontier_hash(&frontier_of(&[c, b, a]))
        );
    }
}
