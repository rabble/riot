//! WU-005 — `riot/sync/2` in-memory duplex reconciliation.
//!
//! Drives two transport-independent [`Sync2Session`]s against each other for the
//! three routed modes over 257+ items (`O`/`C`/`W`), and exercises the hostile FSM
//! paths the design fixes: cursor overlap, request/chunk mismatch, premature EOF,
//! admission rollback, stale-source replication, and the one-way read guarantee.
//! Transcribes "`riot/sync/2`: Routed Paginated Reconciliation", "Composite
//! transaction", and "TDD Slice 2".

mod sync2_harness;

use riot_anchor_protocol::sync2::{
    encode_bundle, ids_page_digest, AdmissionSubject, EntriesChunk, IdsPage, PageComplete,
    SnapshotStart, Sync2Action, Sync2Frame, Sync2Mode, Sync2ModeTag, Sync2Phase, Sync2Refusal,
    Sync2Session,
};

use sync2_harness::{id, item_bytes, items, ns, open_namespace, run_duplex, HarnessRepo, RoleSpec};

fn only_refusal(actions: &[Sync2Action]) -> &Sync2Refusal {
    actions
        .iter()
        .find_map(|a| match a {
            Sync2Action::Send(Sync2Frame::Refuse(r)) => Some(r),
            _ => None,
        })
        .expect("expected a Refuse action")
}

fn has_promotion(actions: &[Sync2Action]) -> bool {
    actions
        .iter()
        .any(|a| matches!(a, Sync2Action::PromoteDirection))
}

// ---------------------------------------------------------------------------
// Happy-path reconciliation for the three modes.
// ---------------------------------------------------------------------------

fn read_committed_pair(
    namespace: [u8; 32],
    count: u32,
) -> (Sync2Session<HarnessRepo>, Sync2Session<HarnessRepo>) {
    let anchor_repo = HarnessRepo::new(namespace, Sync2ModeTag::ReadCommitted).with_plan(vec![(
        Sync2Phase::AnchorToClient,
        RoleSpec::Sender(items(0, count)),
    )]);
    let client_repo = HarnessRepo::new(namespace, Sync2ModeTag::ReadCommitted).with_plan(vec![(
        Sync2Phase::AnchorToClient,
        RoleSpec::Receiver {
            base: Vec::new(),
            fail_admit: None,
        },
    )]);
    let anchor = Sync2Session::responder(anchor_repo);
    let client = Sync2Session::initiator(
        client_repo,
        open_namespace(namespace, Sync2Mode::ReadCommitted),
    );
    (anchor, client)
}

#[test]
fn read_committed_reconciles_257_items_across_o_c_w() {
    // Three independent namespaces stand in for the O/C/W sessions of a public
    // follow; each carries 257 items, forcing a second bounded page (256 + 1).
    for (index, seed) in [10u8, 11, 12].into_iter().enumerate() {
        let namespace = ns(seed);
        let (anchor, client) = read_committed_pair(namespace, 257);
        let report = run_duplex(anchor, client);
        assert!(report.anchor_complete, "anchor incomplete for ns {index}");
        assert!(report.client_complete, "client incomplete for ns {index}");
        assert_eq!(report.anchor_refusal, None);
        assert_eq!(report.client_refusal, None);
        // 256-item page => four 64-ID requests => four chunks; the 1-item page => one.
        assert_eq!(report.admits, 5, "expected five admitted chunks");
        assert_eq!(report.promotions, 1, "one direction promoted");
    }
}

#[test]
fn host_reconcile_staged_converges_bidirectionally() {
    let namespace = ns(20);
    let anchor_items = items(0, 100);
    let client_items = items(50, 100); // ids 50..150 (overlaps the anchor)
    let union_items = items(0, 150); // the client's phase-two committed∪staged view

    let anchor_repo =
        HarnessRepo::new(namespace, Sync2ModeTag::HostReconcileStaged).with_plan(vec![
            (
                Sync2Phase::AnchorToClient,
                RoleSpec::Sender(anchor_items.clone()),
            ),
            (
                Sync2Phase::ClientToAnchor,
                RoleSpec::Receiver {
                    base: anchor_items,
                    fail_admit: None,
                },
            ),
        ]);
    let client_repo =
        HarnessRepo::new(namespace, Sync2ModeTag::HostReconcileStaged).with_plan(vec![
            (
                Sync2Phase::AnchorToClient,
                RoleSpec::Receiver {
                    base: client_items,
                    fail_admit: None,
                },
            ),
            (Sync2Phase::ClientToAnchor, RoleSpec::Sender(union_items)),
        ]);

    let anchor = Sync2Session::responder(anchor_repo);
    let client = Sync2Session::initiator(
        client_repo,
        open_namespace(
            namespace,
            Sync2Mode::HostReconcileStaged {
                operation_id: [1u8; 32],
                namespace_token: [7u8; 32],
            },
        ),
    );
    let report = run_duplex(anchor, client);
    assert!(report.anchor_complete && report.client_complete);
    assert_eq!(report.anchor_refusal, None);
    assert_eq!(report.client_refusal, None);
    // Two directions promote (client host stage in phase one, anchor stage in two).
    assert_eq!(report.promotions, 2);
}

#[test]
fn replica_into_staged_streams_one_way_into_destination() {
    let namespace = ns(30);
    let source_items = items(0, 100);
    let dest_base = items(0, 50); // the destination already holds a subset

    let dest_repo = HarnessRepo::new(namespace, Sync2ModeTag::ReplicaIntoStaged).with_plan(vec![(
        Sync2Phase::SourceToDestination,
        RoleSpec::Receiver {
            base: dest_base,
            fail_admit: None,
        },
    )]);
    let source_repo =
        HarnessRepo::new(namespace, Sync2ModeTag::ReplicaIntoStaged).with_plan(vec![(
            Sync2Phase::SourceToDestination,
            RoleSpec::Sender(source_items),
        )]);

    let destination = Sync2Session::responder(dest_repo);
    let source = Sync2Session::initiator(
        source_repo,
        open_namespace(
            namespace,
            Sync2Mode::ReplicaIntoStaged {
                operation_id: [3u8; 32],
                namespace_token: [4u8; 32],
            },
        ),
    );
    let report = run_duplex(destination, source);
    assert!(report.anchor_complete && report.client_complete);
    assert_eq!(report.anchor_refusal, None);
    assert_eq!(report.client_refusal, None);
    assert_eq!(report.promotions, 1);
}

// ---------------------------------------------------------------------------
// Hostile FSM paths.
// ---------------------------------------------------------------------------

/// A `ReadCommitted` client receiver, already started (OpenNamespace emitted).
fn started_receiver(
    namespace: [u8; 32],
    base: Vec<(Vec<u8>, Vec<u8>)>,
    fail_admit: Option<AdmissionSubject>,
) -> Sync2Session<HarnessRepo> {
    let repo = HarnessRepo::new(namespace, Sync2ModeTag::ReadCommitted).with_plan(vec![(
        Sync2Phase::AnchorToClient,
        RoleSpec::Receiver { base, fail_admit },
    )]);
    let mut session =
        Sync2Session::initiator(repo, open_namespace(namespace, Sync2Mode::ReadCommitted));
    let _ = session.start();
    session
}

fn snapshot_start(namespace: [u8; 32], snapshot_digest: [u8; 32], entry_count: u64) -> Sync2Frame {
    Sync2Frame::SnapshotStart(SnapshotStart {
        phase: Sync2Phase::AnchorToClient,
        namespace_id: namespace,
        snapshot_digest,
        entry_count,
        logical_bytes: 0,
    })
}

#[test]
fn cursor_overlap_between_pages_is_rejected() {
    let namespace = ns(40);
    // The stage already holds the first page so no content is requested; that lets
    // us advance the receiver to the second page cleanly.
    let mut client = started_receiver(namespace, items(0, 2), None);
    let sd = [0x99u8; 32];
    assert!(client.on_frame(snapshot_start(namespace, sd, 3)).is_empty());

    let page1 = IdsPage {
        phase: Sync2Phase::AnchorToClient,
        snapshot_digest: sd,
        after_exclusive: None,
        entry_ids: vec![id(0), id(1)],
        done: false,
    };
    let _ = client.on_frame(Sync2Frame::IdsPage(page1.clone()));
    let pd1 = ids_page_digest(&page1);
    assert!(client
        .on_frame(Sync2Frame::PageComplete(PageComplete {
            phase: Sync2Phase::AnchorToClient,
            page_digest: pd1,
        }))
        .is_empty());

    // Second page overlaps: its first ID re-uses the cursor.
    let page2 = IdsPage {
        phase: Sync2Phase::AnchorToClient,
        snapshot_digest: sd,
        after_exclusive: Some(id(1)),
        entry_ids: vec![id(1), id(2)],
        done: true,
    };
    let actions = client.on_frame(Sync2Frame::IdsPage(page2));
    assert!(matches!(
        only_refusal(&actions),
        Sync2Refusal::CursorRegression { .. }
    ));
}

/// Drive a receiver to a single done page with one outstanding request, returning
/// the page digest.
fn receiver_awaiting_one_chunk(namespace: [u8; 32]) -> (Sync2Session<HarnessRepo>, [u8; 32]) {
    let mut client = started_receiver(namespace, Vec::new(), None);
    let sd = [0x21u8; 32];
    let _ = client.on_frame(snapshot_start(namespace, sd, 1));
    let page = IdsPage {
        phase: Sync2Phase::AnchorToClient,
        snapshot_digest: sd,
        after_exclusive: None,
        entry_ids: vec![id(0)],
        done: true,
    };
    let pd = ids_page_digest(&page);
    let _ = client.on_frame(Sync2Frame::IdsPage(page));
    (client, pd)
}

#[test]
fn unknown_request_id_in_chunk_is_rejected() {
    let namespace = ns(41);
    let (mut client, pd) = receiver_awaiting_one_chunk(namespace);
    let chunk = EntriesChunk {
        phase: Sync2Phase::AnchorToClient,
        page_digest: pd,
        request_id: 99, // no such outstanding request
        chunk_index: 0,
        done: true,
        bundle_bytes: encode_bundle(&[item_bytes(0)]).unwrap(),
    };
    let actions = client.on_frame(Sync2Frame::EntriesChunk(chunk));
    assert!(matches!(
        only_refusal(&actions),
        Sync2Refusal::RequestMismatch { request_id: 99 }
    ));
}

#[test]
fn forked_chunk_index_is_rejected() {
    let namespace = ns(42);
    let (mut client, pd) = receiver_awaiting_one_chunk(namespace);
    let chunk = EntriesChunk {
        phase: Sync2Phase::AnchorToClient,
        page_digest: pd,
        request_id: 0,
        chunk_index: 1, // must start at zero
        done: true,
        bundle_bytes: encode_bundle(&[item_bytes(0)]).unwrap(),
    };
    let actions = client.on_frame(Sync2Frame::EntriesChunk(chunk));
    assert!(matches!(
        only_refusal(&actions),
        Sync2Refusal::ChunkMismatch {
            request_id: 0,
            expected_index: 0,
            observed_index: 1,
        }
    ));
}

#[test]
fn admission_failure_rolls_back_without_promotion() {
    let namespace = ns(43);
    let mut client = started_receiver(namespace, Vec::new(), Some(AdmissionSubject::Entry));
    let sd = [0x21u8; 32];
    let _ = client.on_frame(snapshot_start(namespace, sd, 1));
    let page = IdsPage {
        phase: Sync2Phase::AnchorToClient,
        snapshot_digest: sd,
        after_exclusive: None,
        entry_ids: vec![id(0)],
        done: true,
    };
    let pd = ids_page_digest(&page);
    let _ = client.on_frame(Sync2Frame::IdsPage(page));
    let chunk = EntriesChunk {
        phase: Sync2Phase::AnchorToClient,
        page_digest: pd,
        request_id: 0,
        chunk_index: 0,
        done: true,
        bundle_bytes: encode_bundle(&[item_bytes(0)]).unwrap(),
    };
    let actions = client.on_frame(Sync2Frame::EntriesChunk(chunk));
    assert!(matches!(
        only_refusal(&actions),
        Sync2Refusal::AdmissionFailed {
            subject: AdmissionSubject::Entry
        }
    ));
    assert!(
        !has_promotion(&actions),
        "a failed admission must not promote"
    );
    assert!(!client.is_complete());
}

#[test]
fn premature_eof_leaves_no_promotion_or_completion() {
    let namespace = ns(44);
    let mut client = started_receiver(namespace, Vec::new(), None);
    let sd = [0x21u8; 32];
    let _ = client.on_frame(snapshot_start(namespace, sd, 2));
    let page = IdsPage {
        phase: Sync2Phase::AnchorToClient,
        snapshot_digest: sd,
        after_exclusive: None,
        entry_ids: vec![id(0), id(1)],
        done: false,
    };
    let actions = client.on_frame(Sync2Frame::IdsPage(page));
    // The peer disconnects here: no further frames arrive.
    assert!(!has_promotion(&actions));
    assert!(
        !client.is_complete(),
        "an interrupted session never completes"
    );
    assert!(client.refusal().is_none());
}

#[test]
fn replica_source_refuses_stale_before_snapshot_start() {
    let namespace = ns(45);
    let stale = Sync2Refusal::StaleSource {
        attested_generation: 7,
        observed_generation: 9,
        observed_namespace_snapshot_digests: vec![[1u8; 32], [2u8; 32], [3u8; 32]],
    };
    let source_repo = HarnessRepo::new(namespace, Sync2ModeTag::ReplicaIntoStaged)
        .with_plan(vec![(
            Sync2Phase::SourceToDestination,
            RoleSpec::Sender(items(0, 3)),
        )])
        .with_stale_source(stale);
    let mut source = Sync2Session::initiator(
        source_repo,
        open_namespace(
            namespace,
            Sync2Mode::ReplicaIntoStaged {
                operation_id: [3u8; 32],
                namespace_token: [4u8; 32],
            },
        ),
    );
    let actions = source.start();
    // The OpenNamespace is sent first, then the stale-source refusal — and never a
    // SnapshotStart.
    let names: Vec<_> = actions
        .iter()
        .filter_map(|a| match a {
            Sync2Action::Send(f) => Some(f.name()),
            _ => None,
        })
        .collect();
    assert_eq!(names.first().map(|n| n.token()), Some("open_namespace"));
    assert!(matches!(
        only_refusal(&actions),
        Sync2Refusal::StaleSource { .. }
    ));
    assert!(!names.iter().any(|n| n.token() == "snapshot_start"));
    assert!(source.is_terminated());
}
