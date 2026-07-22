//! Deterministic policy evaluator: `(journal, now) -> PolicySnapshot`.

use std::collections::{BTreeMap, BTreeSet};

use super::authorize::verify_capability_issuance;
use super::body::Body;
use super::frontier::{frontier_hash, frontier_of, topological_reduce};
use super::record::{record_id, Fingerprint, GovernanceRecordV1};
use super::{actor, RecordKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySnapshot {
    pub frontier_hash: [u8; 32],
    pub active_fingerprints: BTreeSet<Fingerprint>,
    pub revoked: BTreeSet<Fingerprint>,
    pub actor_bindings: BTreeMap<[u8; 32], BTreeSet<[u8; 32]>>,
    pub action_heads: BTreeMap<([u8; 32], [u8; 32]), [u8; 32]>,
}

pub fn evaluate(records: &[GovernanceRecordV1], now: Option<u64>) -> PolicySnapshot {
    let (accepted, _) = topological_reduce(records);
    const TEN_MINUTES: u64 = 10 * 60 * 1_000_000;
    let horizon = now.map(|time| time.saturating_add(TEN_MINUTES));
    let in_time: Vec<GovernanceRecordV1> = accepted
        .iter()
        .filter(|record| {
            horizon.map_or(true, |limit| record.created_display_micros <= limit)
        })
        .cloned()
        .collect();

    let mut active = BTreeSet::new();
    if now.is_some() {
        for record in &in_time {
            if matches!(
                record.kind,
                RecordKind::CapabilityIssued | RecordKind::CapabilityRenewed
            ) && verify_capability_issuance(&record.body).is_ok()
            {
                match &record.body {
                    Body::CapabilityIssued {
                        child_fingerprint,
                        ..
                    }
                    | Body::CapabilityRenewed {
                        child_fingerprint,
                        ..
                    } => {
                        active.insert(*child_fingerprint);
                    }
                    _ => unreachable!("record kind and body must agree"),
                }
            }
        }
    }

    if now.is_some() {
        active = apply_role_restrictions(&in_time, active);
    }
    let (active, revoked) = if now.is_some() {
        super::revoke::apply_revocations(&in_time, active)
    } else {
        (active, BTreeSet::new())
    };

    PolicySnapshot {
        frontier_hash: frontier_hash(&frontier_of(&accepted)),
        active_fingerprints: active,
        revoked,
        actor_bindings: actor::actor_bindings(&in_time),
        action_heads: BTreeMap::new(),
    }
}

fn ancestors_of(
    id: &[u8; 32],
    by_id: &BTreeMap<[u8; 32], &GovernanceRecordV1>,
) -> BTreeSet<[u8; 32]> {
    let mut ancestors = BTreeSet::new();
    let mut stack = by_id
        .get(id)
        .map(|record| record.parents.clone())
        .unwrap_or_default();
    while let Some(parent) = stack.pop() {
        if ancestors.insert(parent) {
            if let Some(record) = by_id.get(&parent) {
                stack.extend(record.parents.iter().copied());
            }
        }
    }
    ancestors
}

fn apply_role_restrictions(
    accepted: &[GovernanceRecordV1],
    mut active: BTreeSet<Fingerprint>,
) -> BTreeSet<Fingerprint> {
    let by_id: BTreeMap<[u8; 32], &GovernanceRecordV1> = accepted
        .iter()
        .map(|record| (record_id(record), record))
        .collect();
    let mut by_role: BTreeMap<[u8; 32], Vec<([u8; 32], Vec<Fingerprint>)>> = BTreeMap::new();
    for record in accepted {
        if let (
            RecordKind::RoleDecision,
            Body::RoleDecision {
                role_instance_id,
                granted_fingerprints,
                ..
            },
        ) = (&record.kind, &record.body)
        {
            by_role
                .entry(*role_instance_id)
                .or_default()
                .push((record_id(record), granted_fingerprints.clone()));
        }
    }

    for decisions in by_role.values() {
        let frontier: Vec<&([u8; 32], Vec<Fingerprint>)> = decisions
            .iter()
            .filter(|(id, _)| {
                !decisions.iter().any(|(other, _)| {
                    other != id && ancestors_of(other, &by_id).contains(id)
                })
            })
            .collect();
        if frontier.is_empty() {
            continue;
        }
        let union: BTreeSet<Fingerprint> = decisions
            .iter()
            .flat_map(|(_, granted)| granted.iter().copied())
            .collect();
        let mut effective: BTreeSet<Fingerprint> = frontier[0].1.iter().copied().collect();
        for (_, granted) in frontier.iter().skip(1) {
            let granted: BTreeSet<Fingerprint> = granted.iter().copied().collect();
            effective = effective.intersection(&granted).copied().collect();
        }
        for fingerprint in union.difference(&effective) {
            active.remove(fingerprint);
        }
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::{
        genesis_record, issued_record, renewal_after_revoke, revoke_record, seeded_journal,
        three_hop_lineage_records_with_sibling_active, two_concurrent_role_restrictions,
    };

    #[test]
    fn evaluate_is_deterministic_over_shuffled_input() {
        let journal = seeded_journal(50);
        let baseline = evaluate(&journal, Some(2_000_000_000_000));
        for seed in 0..8 {
            let mut shuffled = journal.clone();
            let rotation = (seed * 7) % shuffled.len();
            shuffled.rotate_left(rotation);
            assert_eq!(evaluate(&shuffled, Some(2_000_000_000_000)), baseline);
        }
    }

    #[test]
    fn evaluate_reads_no_wall_clock() {
        let journal = seeded_journal(20);
        assert_eq!(
            evaluate(&journal, Some(1_777_000_000_000)),
            evaluate(&journal, Some(1_777_000_000_000))
        );
    }

    #[test]
    fn a_forged_issuance_never_becomes_active() {
        let mut forged = issued_record([9u8; 32], 8);
        if let Body::CapabilityIssued {
            child_fingerprint,
            ..
        } = &mut forged.body
        {
            *child_fingerprint = [0xFF; 32];
        }
        let fingerprint = match &forged.body {
            Body::CapabilityIssued {
                child_fingerprint,
                ..
            } => *child_fingerprint,
            _ => unreachable!(),
        };
        let journal = vec![genesis_record([9u8; 32]), forged];
        assert!(!evaluate(&journal, Some(2_000_000_000_000))
            .active_fingerprints
            .contains(&fingerprint));
    }

    #[test]
    fn revoking_mid_chain_removes_descendants_from_the_active_snapshot() {
        let (mut records, fingerprints) = three_hop_lineage_records_with_sibling_active();
        // Revoking B invalidates B and C while leaving A and its sibling D
        // branch active; this isolates transitive descendant invalidation from
        // the sibling case.
        records.push(revoke_record(fingerprints.b));
        let snapshot = evaluate(&records, Some(2_000_000_000_000));
        assert!(!snapshot.active_fingerprints.contains(&fingerprints.b));
        assert!(!snapshot.active_fingerprints.contains(&fingerprints.c));
        assert!(snapshot.active_fingerprints.contains(&fingerprints.a));
        assert!(snapshot
            .active_fingerprints
            .contains(&fingerprints.d_sibling));
    }

    #[test]
    fn a_renewal_after_revocation_mints_a_fresh_active_fingerprint() {
        let (records, old, new) = renewal_after_revoke();
        let snapshot = evaluate(&records, Some(2_000_000_000_000));
        assert!(snapshot.revoked.contains(&old));
        assert!(snapshot.active_fingerprints.contains(&new));
        assert!(!snapshot.active_fingerprints.contains(&old));
    }

    #[test]
    fn a_record_more_than_ten_minutes_ahead_is_quarantined() {
        use crate::willow::tai_j2000_micros_from_unix_seconds as tai;
        let now = tai(1_800_000_000).unwrap();
        let mut journal = seeded_journal(4);
        let index = journal
            .iter()
            .position(|record| record.kind == RecordKind::CapabilityIssued)
            .unwrap();
        let fingerprint = match &journal[index].body {
            Body::CapabilityIssued {
                child_fingerprint,
                ..
            } => *child_fingerprint,
            _ => unreachable!(),
        };
        journal[index].created_display_micros = now + 10 * 60 * 1_000_000 + 1;
        assert!(!evaluate(&journal, Some(now))
            .active_fingerprints
            .contains(&fingerprint));
        journal[index].created_display_micros = now + 10 * 60 * 1_000_000;
        assert!(evaluate(&journal, Some(now))
            .active_fingerprints
            .contains(&fingerprint));
    }

    #[test]
    fn an_unavailable_clock_blocks_activation_but_not_dag_ordering() {
        let journal = seeded_journal(6);
        let with_clock = evaluate(&journal, Some(2_000_000_000_000));
        let without_clock = evaluate(&journal, None);
        assert!(without_clock.active_fingerprints.is_empty());
        assert_eq!(without_clock.frontier_hash, with_clock.frontier_hash);
    }

    #[test]
    fn concurrent_role_restrictions_intersect_in_the_evaluator() {
        let (records, survivor) = two_concurrent_role_restrictions();
        let snapshot = evaluate(&records, Some(2_000_000_000_000));
        assert!(snapshot.active_fingerprints.contains(&survivor));
        assert_eq!(
            snapshot
                .active_fingerprints
                .iter()
                .filter(|fingerprint| {
                    crate::governance::test_support::is_role_fp(fingerprint)
                })
                .count(),
            1
        );
    }
}
