//! Actor/device binding facts and per-actor sequence hash chains.

use std::collections::{BTreeMap, BTreeSet};

use super::body::Body;
use super::record::{record_id, GovernanceRecordV1, RecordId};
use super::{GovernanceError, RecordKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorChain {
    pub actor_id: [u8; 32],
    pub ordered: Vec<RecordId>,
    pub head_sequence: u64,
}

pub fn validate_actor_chain(
    records_for_actor: &[GovernanceRecordV1],
) -> Result<ActorChain, GovernanceError> {
    if records_for_actor.is_empty() {
        return Err(GovernanceError::ActorChainBroken);
    }
    let actor_id = records_for_actor[0].actor_id;
    let mut sorted: Vec<&GovernanceRecordV1> = records_for_actor.iter().collect();
    sorted.sort_by_key(|r| r.sequence);
    let mut ordered = Vec::with_capacity(sorted.len());
    let mut prev_id: Option<RecordId> = None;
    for (index, record) in sorted.iter().enumerate() {
        if record.actor_id != actor_id
            || record.sequence != index as u64          // no gaps / no forks
            || record.prev_actor_record != prev_id
        // correct link
        {
            return Err(GovernanceError::ActorChainBroken);
        }
        let id = record_id(record);
        ordered.push(id);
        prev_id = Some(id);
    }
    Ok(ActorChain {
        actor_id,
        head_sequence: (ordered.len() - 1) as u64,
        ordered,
    })
}

pub fn actor_bindings(records: &[GovernanceRecordV1]) -> BTreeMap<[u8; 32], BTreeSet<[u8; 32]>> {
    let mut out: BTreeMap<[u8; 32], BTreeSet<[u8; 32]>> = BTreeMap::new();
    for r in records {
        if r.kind == RecordKind::ActorBinding {
            if let Body::ActorBinding { bound_receiver, .. } = &r.body {
                out.entry(r.actor_id).or_default().insert(*bound_receiver);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::{actor_record, binding_record};

    fn chain(actor: [u8; 32], count: u64) -> Vec<GovernanceRecordV1> {
        let mut out = Vec::new();
        let mut prev = None;
        for seq in 0..count {
            let r = actor_record(actor, seq, prev);
            prev = Some(record_id(&r));
            out.push(r);
        }
        out
    }

    #[test]
    fn a_valid_three_record_chain_is_accepted() {
        assert_eq!(
            validate_actor_chain(&chain([7u8; 32], 3))
                .unwrap()
                .head_sequence,
            2
        );
    }
    #[test]
    fn a_gap_is_rejected() {
        let mut c = chain([7u8; 32], 2);
        c[1].sequence = 2;
        assert_eq!(
            validate_actor_chain(&c),
            Err(GovernanceError::ActorChainBroken)
        );
    }
    #[test]
    fn a_fork_is_rejected() {
        let mut c = chain([7u8; 32], 2);
        c[1].sequence = 0;
        assert_eq!(
            validate_actor_chain(&c),
            Err(GovernanceError::ActorChainBroken)
        );
    }
    #[test]
    fn a_wrong_prev_link_is_rejected() {
        let mut c = chain([7u8; 32], 2);
        c[1].prev_actor_record = Some([0xAB; 32]);
        assert_eq!(
            validate_actor_chain(&c),
            Err(GovernanceError::ActorChainBroken)
        );
    }
    #[test]
    fn one_actor_binds_multiple_receivers() {
        let a = [9u8; 32];
        let b = actor_bindings(&[binding_record(a, [1u8; 32]), binding_record(a, [2u8; 32])]);
        let r = b.get(&a).unwrap();
        assert!(r.contains(&[1u8; 32]) && r.contains(&[2u8; 32]));
    }
}
