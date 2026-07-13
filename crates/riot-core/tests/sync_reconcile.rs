use riot_core::sync::{missing_entry_ids, SyncError, MAX_SYNC_IDS};

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

#[test]
fn empty_and_disjoint_inventories_preserve_remote_order() {
    assert_eq!(missing_entry_ids(&[], &[]), Ok(vec![]));
    assert_eq!(
        missing_entry_ids(&[id(1)], &[id(2), id(3)]),
        Ok(vec![id(2), id(3)])
    );
}

#[test]
fn local_overlap_is_removed_without_requiring_local_order() {
    assert_eq!(
        missing_entry_ids(&[id(3), id(1), id(1)], &[id(1), id(2), id(3)]),
        Ok(vec![id(2)])
    );
}

#[test]
fn summaries_reject_duplicate_out_of_order_and_over_limit_inputs() {
    assert_eq!(
        missing_entry_ids(&[], &[id(1), id(1)]),
        Err(SyncError::DuplicateEntryId)
    );
    assert_eq!(
        missing_entry_ids(&[], &[id(2), id(1)]),
        Err(SyncError::EntryIdsNotSorted)
    );
    assert_eq!(
        missing_entry_ids(&vec![id(1); MAX_SYNC_IDS + 1], &[]),
        Err(SyncError::TooManyEntryIds)
    );
    assert_eq!(
        missing_entry_ids(&[], &vec![id(1); MAX_SYNC_IDS + 1]),
        Err(SyncError::TooManyEntryIds)
    );
}
