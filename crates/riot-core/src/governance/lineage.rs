//! Capability-lineage forest keyed by Slice-1 fingerprints.

use std::collections::{BTreeMap, BTreeSet};

use super::body::Body;
use super::record::{Fingerprint, GovernanceRecordV1};
use super::RecordKind;

#[derive(Debug, Clone, Default)]
pub struct LineageForest {
    parent_of: BTreeMap<Fingerprint, Fingerprint>,
}

pub fn build_lineage(records: &[GovernanceRecordV1]) -> LineageForest {
    let mut parent_of = BTreeMap::new();
    for record in records {
        match (&record.kind, &record.body) {
            (
                RecordKind::CapabilityIssued,
                Body::CapabilityIssued {
                    covering_parent_fingerprint,
                    child_fingerprint,
                    ..
                },
            )
            | (
                RecordKind::CapabilityRenewed,
                Body::CapabilityRenewed {
                    covering_parent_fingerprint,
                    child_fingerprint,
                    ..
                },
            ) => {
                parent_of.insert(*child_fingerprint, *covering_parent_fingerprint);
            }
            _ => {}
        }
    }
    LineageForest { parent_of }
}

impl LineageForest {
    pub fn descendants_of(&self, target: Fingerprint) -> BTreeSet<Fingerprint> {
        let mut descendants = BTreeSet::from([target]);
        loop {
            let mut grew = false;
            for (child, parent) in &self.parent_of {
                if descendants.contains(parent) && descendants.insert(*child) {
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        descendants
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::test_support::three_hop_lineage_records;

    #[test]
    fn descendants_include_the_subtree_but_not_siblings() {
        let (records, fps) = three_hop_lineage_records();
        let descendants = build_lineage(&records).descendants_of(fps.a);
        assert!(descendants.contains(&fps.a));
        assert!(descendants.contains(&fps.b));
        assert!(descendants.contains(&fps.c));
        assert!(!descendants.contains(&fps.d_sibling));
    }
}
