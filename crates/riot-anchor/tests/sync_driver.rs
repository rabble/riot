//! In-process tests for the REAL `SyncSessionTable`: full `HostReconcileStaged`
//! pushes driven as raw frame bytes through the table (no network), plus the
//! fail-closed refusals — unknown namespace token, expired operation, and the
//! session-capacity ceiling — proven at the DRIVER level (the adapter-level
//! `sync_service_edges` tests never exercise the table).
//!
//! Fixtures come from a real initiator `Sync2Session` transcript
//! (`hosting_common::prepared_sync2_push_fixture`) — no hand-encoded frames.

#![cfg(feature = "daemon")]

mod hosting_common;

use hosting_common::{
    open_frame_with_token, operation_has_staged_entries, prepared_sync2_push_fixture,
};
use riot_anchor::sync_driver::SyncSessionTable;

const NOW: u64 = 1_000_000;
const EXPIRY: u64 = NOW + 3600;

#[test]
fn session_table_drives_host_reconcile_push_to_staging() {
    let fixture = prepared_sync2_push_fixture(NOW, EXPIRY);
    let mut table = SyncSessionTable::new(4);

    let mut outbound = Vec::new();
    let opened = table.open(
        &fixture.shared,
        &fixture.token_ring,
        1,
        fixture.open_frame.clone(),
        NOW,
        &mut outbound,
    );
    assert!(opened, "responder must accept the prepared-operation open");
    assert!(
        !outbound.is_empty(),
        "the anchor answers the open with its committed-base snapshot frames"
    );

    for frame in fixture.entry_frames.clone() {
        table.frame(&fixture.shared, 1, frame, &mut outbound);
    }
    assert!(table.is_complete(1), "push must reach NamespaceComplete");
    // Staged (not committed) entries exist for the operation:
    assert!(
        operation_has_staged_entries(
            &fixture.shared,
            &fixture.operation_id,
            &fixture.namespace_id
        ),
        "the pushed entry must land in operation-private staging"
    );
    assert!(
        fixture
            .shared
            .borrow()
            .committed_entries(&fixture.namespace_id)
            .unwrap_or_default()
            .is_empty(),
        "a push stages; it never commits"
    );
}

#[test]
fn close_prunes_both_live_and_completion_state() {
    let fixture = prepared_sync2_push_fixture(NOW, EXPIRY);
    let mut table = SyncSessionTable::new(4);
    let mut outbound = Vec::new();
    assert!(table.open(
        &fixture.shared,
        &fixture.token_ring,
        1,
        fixture.open_frame.clone(),
        NOW,
        &mut outbound,
    ));
    for frame in fixture.entry_frames.clone() {
        table.frame(&fixture.shared, 1, frame, &mut outbound);
    }
    // Completion state lives until the connection's Close arrives (the daemon
    // handler's RAII guard delivers it on every exit path) — query BEFORE close.
    assert!(table.is_complete(1));
    table.close(1);
    assert!(
        !table.is_complete(1),
        "close must prune completion state too, or the completed set grows without bound",
    );
    // Closing an id that was never (or no longer) live is a no-op.
    table.close(1);
    table.close(99);
}

#[test]
fn session_table_refuses_unknown_token_expired_operation_and_capacity() {
    // (a) Unknown namespace_token: token bytes flipped — open() must refuse
    //     (no session stored, nothing staged), and the refusal frame goes back.
    {
        let fixture = prepared_sync2_push_fixture(NOW, EXPIRY);
        let mut flipped = fixture.namespace_token;
        flipped[0] ^= 0x01;
        let bad_open = open_frame_with_token(&fixture, flipped);
        let mut table = SyncSessionTable::new(4);
        let mut outbound = Vec::new();
        let opened = table.open(
            &fixture.shared,
            &fixture.token_ring,
            1,
            bad_open,
            NOW,
            &mut outbound,
        );
        assert!(!opened, "a flipped namespace token must be refused");
        assert!(
            !outbound.is_empty(),
            "the token refusal is transmitted to the peer"
        );
        assert!(!table.is_complete(1));
        // Frames for the refused session hit a dead id: terminated, no effect.
        let mut later = Vec::new();
        for frame in fixture.entry_frames.clone() {
            assert!(
                table.frame(&fixture.shared, 1, frame, &mut later),
                "frames for a refused session are terminated"
            );
        }
        assert!(later.is_empty(), "a dead session transmits nothing");
        assert!(!operation_has_staged_entries(
            &fixture.shared,
            &fixture.operation_id,
            &fixture.namespace_id
        ));
    }

    // (b) Expired operation: the fixture's operation expired before `now` —
    //     the AnchorSyncRepository lifecycle check fires; nothing staged.
    {
        let fixture = prepared_sync2_push_fixture(NOW - 4000, NOW - 10);
        let mut table = SyncSessionTable::new(4);
        let mut outbound = Vec::new();
        let opened = table.open(
            &fixture.shared,
            &fixture.token_ring,
            1,
            fixture.open_frame.clone(),
            NOW,
            &mut outbound,
        );
        assert!(!opened, "an expired operation must refuse the open");
        assert!(!operation_has_staged_entries(
            &fixture.shared,
            &fixture.operation_id,
            &fixture.namespace_id
        ));
    }

    // (c) Capacity: a one-session table refuses a second open with VALID
    //     credentials (terminated, empty outbound) while session 1 still drives
    //     to completion.
    {
        let fixture = prepared_sync2_push_fixture(NOW, EXPIRY);
        let mut table = SyncSessionTable::new(1);
        let mut outbound = Vec::new();
        assert!(table.open(
            &fixture.shared,
            &fixture.token_ring,
            1,
            fixture.open_frame.clone(),
            NOW,
            &mut outbound,
        ));

        let mut second = Vec::new();
        let opened = table.open(
            &fixture.shared,
            &fixture.token_ring,
            2,
            fixture.open_frame.clone(),
            NOW,
            &mut second,
        );
        assert!(!opened, "the capacity ceiling refuses a second session");
        assert!(second.is_empty(), "a capacity refusal transmits nothing");

        // Session 1 is unharmed: the full push still completes and stages.
        for frame in fixture.entry_frames.clone() {
            table.frame(&fixture.shared, 1, frame, &mut outbound);
        }
        assert!(table.is_complete(1), "session 1 still drives to completion");
        assert!(!table.is_complete(2));
        assert!(operation_has_staged_entries(
            &fixture.shared,
            &fixture.operation_id,
            &fixture.namespace_id
        ));
    }
}
