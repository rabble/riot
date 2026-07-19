//! WU-005 — `riot/sync/2` routed paginated reconciliation: golden + hostile
//! canonical frame vectors, the closed refusal matrix, and exact FSM traces.
//!
//! Every assertion transcribes a normative row from
//! `docs/superpowers/specs/2026-07-18-public-community-anchor-network-design.md`
//! sections "`riot/sync/2`: Routed Paginated Reconciliation" and "TDD Slice 2":
//! the `OpenNamespace` routing frame and its three modes, the paginated inventory
//! frame set, the sync-snapshot / page digests, the closed sync refusal matrix,
//! and the per-direction sender/receiver FSM.

mod sync2_harness;

use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{PeerContextReason, PeerSide, TransportMode};
use riot_anchor_protocol::records::{AnchorLimitId, LimitValue};
use riot_anchor_protocol::sync2::{
    ids_page_digest, AdmissionSubject, DirectionComplete, EntriesChunk, IdsPage, NamespaceComplete,
    NeedEntries, OpenNamespace, PageComplete, PageNeedsComplete, SnapshotStart, Sync2Action,
    Sync2Frame, Sync2FrameName, Sync2Mode, Sync2ModeTag, Sync2Phase, Sync2Refusal, Sync2Session,
    MAX_SYNC2_FRAME_BYTES,
};

use sync2_harness::{items, ns, open_namespace, HarnessRepo, RoleSpec};

fn d32(seed: u8) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = seed.wrapping_add(i as u8);
    }
    out
}

fn eid(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| seed.wrapping_add(i as u8)).collect()
}

/// Round-trip a frame and assert canonical byte-identity through `decode_canonical`.
fn roundtrip_frame(frame: &Sync2Frame) {
    let bytes = frame.encode_canonical().expect("encode");
    let decoded: Sync2Frame = decode_canonical(&bytes, MAX_SYNC2_FRAME_BYTES).expect("decode");
    assert_eq!(&decoded, frame, "decoded frame must equal original");
    let reencoded = decoded.encode_canonical().expect("re-encode");
    assert_eq!(reencoded, bytes, "canonical bytes must be stable");
}

#[test]
fn open_namespace_read_committed_roundtrips() {
    let frame = Sync2Frame::OpenNamespace(OpenNamespace {
        protocol_version: 2,
        session_id: eid(9, 16),
        ticket_core_bytes: eid(1, 40),
        namespace_id: d32(7),
        mode: Sync2Mode::ReadCommitted,
    });
    roundtrip_frame(&frame);
}

#[test]
fn open_namespace_staged_modes_roundtrip() {
    for mode in [
        Sync2Mode::HostReconcileStaged {
            operation_id: d32(3),
            namespace_token: d32(4),
        },
        Sync2Mode::ReplicaIntoStaged {
            operation_id: d32(5),
            namespace_token: d32(6),
        },
    ] {
        let frame = Sync2Frame::OpenNamespace(OpenNamespace {
            protocol_version: 2,
            session_id: eid(1, 8),
            ticket_core_bytes: eid(2, 40),
            namespace_id: d32(7),
            mode,
        });
        roundtrip_frame(&frame);
    }
}

#[test]
fn inventory_frames_roundtrip() {
    roundtrip_frame(&Sync2Frame::SnapshotStart(SnapshotStart {
        phase: Sync2Phase::AnchorToClient,
        namespace_id: d32(1),
        snapshot_digest: d32(2),
        entry_count: 300,
        logical_bytes: 4096,
    }));
    roundtrip_frame(&Sync2Frame::IdsPage(IdsPage {
        phase: Sync2Phase::AnchorToClient,
        snapshot_digest: d32(2),
        after_exclusive: None,
        entry_ids: vec![eid(1, 8), eid(2, 8), eid(3, 8)],
        done: false,
    }));
    roundtrip_frame(&Sync2Frame::IdsPage(IdsPage {
        phase: Sync2Phase::ClientToAnchor,
        snapshot_digest: d32(2),
        after_exclusive: Some(eid(3, 8)),
        entry_ids: vec![eid(4, 8), eid(5, 8)],
        done: true,
    }));
    roundtrip_frame(&Sync2Frame::NeedEntries(NeedEntries {
        phase: Sync2Phase::AnchorToClient,
        page_digest: d32(3),
        request_id: 0,
        entry_ids: vec![eid(1, 8), eid(2, 8)],
    }));
    roundtrip_frame(&Sync2Frame::PageNeedsComplete(PageNeedsComplete {
        phase: Sync2Phase::AnchorToClient,
        page_digest: d32(3),
    }));
    roundtrip_frame(&Sync2Frame::EntriesChunk(EntriesChunk {
        phase: Sync2Phase::AnchorToClient,
        page_digest: d32(3),
        request_id: 0,
        chunk_index: 0,
        done: true,
        bundle_bytes: eid(9, 64),
    }));
    roundtrip_frame(&Sync2Frame::PageComplete(PageComplete {
        phase: Sync2Phase::AnchorToClient,
        page_digest: d32(3),
    }));
    roundtrip_frame(&Sync2Frame::DirectionComplete(DirectionComplete {
        phase: Sync2Phase::SourceToDestination,
        sender_snapshot_digest: d32(4),
    }));
    roundtrip_frame(&Sync2Frame::NamespaceComplete(NamespaceComplete {
        mode: Sync2ModeTag::ReplicaIntoStaged,
        final_snapshot_digest: d32(5),
    }));
}

#[test]
fn page_digest_is_domain_separated_over_canonical_page() {
    let page = IdsPage {
        phase: Sync2Phase::AnchorToClient,
        snapshot_digest: d32(2),
        after_exclusive: None,
        entry_ids: vec![eid(1, 8), eid(2, 8)],
        done: false,
    };
    let a = ids_page_digest(&page);
    let mut page2 = page.clone();
    page2.done = true;
    assert_ne!(a, ids_page_digest(&page2), "digest binds every field");
}

#[test]
fn busy_is_the_only_retryable_refusal() {
    let busy = Sync2Refusal::Busy {
        limit_id: AnchorLimitId::from_id(12).unwrap(),
        retry_after_seconds: 5,
    };
    assert!(busy.retryable());
    assert_eq!(busy.retry_after_seconds(), Some(5));
    roundtrip_frame(&Sync2Frame::Refuse(busy));

    let terminal = Sync2Refusal::SnapshotMismatch {
        expected_snapshot_digest: d32(1),
        observed_snapshot_digest: d32(2),
    };
    assert!(!terminal.retryable());
    assert_eq!(terminal.retry_after_seconds(), None);
    roundtrip_frame(&Sync2Frame::Refuse(terminal));
}

#[test]
fn admission_subject_tokens_are_closed() {
    assert_eq!(AdmissionSubject::Authority.token(), "authority");
    assert_eq!(AdmissionSubject::Bundle.token(), "bundle");
    assert_eq!(AdmissionSubject::Entry.token(), "entry");
    assert_eq!(AdmissionSubject::from_token("nope"), None);
}

// ===========================================================================
// The complete closed sync refusal matrix: every row round-trips and reports the
// exact retryability the design fixes (only `busy` is retryable).
// ===========================================================================

fn all_refusals() -> Vec<Sync2Refusal> {
    use riot_anchor_protocol::TicketReason;
    vec![
        Sync2Refusal::UnsupportedVersion {
            supported_versions: vec![2],
        },
        Sync2Refusal::InvalidTicket {
            reason: TicketReason::Signature,
        },
        Sync2Refusal::InvalidTicket {
            reason: TicketReason::Root,
        },
        Sync2Refusal::InvalidTicket {
            reason: TicketReason::Structure,
        },
        Sync2Refusal::ExpiredTicket {
            expires_at: 100,
            observed_at: 200,
        },
        Sync2Refusal::TransportMismatch {
            required_mode: TransportMode::RequireArti,
            observed_mode: TransportMode::RequireNone,
        },
        Sync2Refusal::NamespaceNotMember {
            namespace_id: d32(1),
        },
        Sync2Refusal::ManifestMismatch {
            expected_digest: d32(1),
            observed_digest: d32(2),
        },
        Sync2Refusal::InvalidMode {
            observed_mode: Sync2ModeTag::HostReconcileStaged,
        },
        Sync2Refusal::OperationNotFound {
            operation_id: d32(3),
        },
        Sync2Refusal::InvalidNamespaceToken {
            namespace_id: d32(4),
        },
        Sync2Refusal::OperationExpired {
            operation_id: d32(5),
            expires_at: 1,
            observed_at: 2,
        },
        Sync2Refusal::UnexpectedFrame {
            phase: Sync2Phase::ClientToAnchor,
            expected_frame_names: vec![Sync2FrameName::NeedEntries, Sync2FrameName::PageComplete],
            observed_frame_name: Sync2FrameName::EntriesChunk,
        },
        Sync2Refusal::CursorRegression {
            after_exclusive: Some(eid(3, 8)),
            observed_first_id: eid(2, 8),
        },
        Sync2Refusal::CursorRegression {
            after_exclusive: None,
            observed_first_id: eid(1, 8),
        },
        Sync2Refusal::PageMismatch {
            expected_page_digest: d32(1),
            observed_page_digest: d32(2),
        },
        Sync2Refusal::SnapshotMismatch {
            expected_snapshot_digest: d32(1),
            observed_snapshot_digest: d32(2),
        },
        Sync2Refusal::StaleSource {
            attested_generation: 7,
            observed_generation: 9,
            observed_namespace_snapshot_digests: vec![d32(1), d32(2), d32(3)],
        },
        Sync2Refusal::RequestMismatch { request_id: 3 },
        Sync2Refusal::ChunkMismatch {
            request_id: 1,
            expected_index: 0,
            observed_index: 2,
        },
        Sync2Refusal::FrameOversize {
            observed_bytes: 9_000_000,
            maximum_bytes: 8_388_608,
        },
        Sync2Refusal::AdmissionFailed {
            subject: AdmissionSubject::Authority,
        },
        Sync2Refusal::AdmissionFailed {
            subject: AdmissionSubject::Bundle,
        },
        Sync2Refusal::AdmissionFailed {
            subject: AdmissionSubject::Entry,
        },
        Sync2Refusal::QuotaExceeded {
            limit_id: AnchorLimitId::from_id(4).unwrap(),
            effective_value: LimitValue::Scalar(10),
            observed_value: LimitValue::Compound(3, 4),
        },
        Sync2Refusal::Busy {
            limit_id: AnchorLimitId::from_id(7).unwrap(),
            retry_after_seconds: 30,
        },
        Sync2Refusal::PeerContextChanged {
            side: PeerSide::Source,
            prior_descriptor_digest: d32(1),
            latest_descriptor_digest: Some(d32(2)),
            reason: PeerContextReason::DescriptorRotation,
        },
        Sync2Refusal::PeerContextChanged {
            side: PeerSide::Destination,
            prior_descriptor_digest: d32(3),
            latest_descriptor_digest: None,
            reason: PeerContextReason::TransportLoss,
        },
    ]
}

#[test]
fn every_refusal_row_roundtrips_and_only_busy_is_retryable() {
    for refusal in all_refusals() {
        let is_busy = matches!(refusal, Sync2Refusal::Busy { .. });
        assert_eq!(
            refusal.retryable(),
            is_busy,
            "only busy retryable: {refusal:?}"
        );
        assert_eq!(refusal.retry_after_seconds().is_some(), is_busy);
        roundtrip_frame(&Sync2Frame::Refuse(refusal));
    }
}

// ===========================================================================
// Hostile canonical encodings — a dependency-free CBOR builder crafts each
// malformation and asserts the exact rejection.
// ===========================================================================

fn c_arr(items: &[Vec<u8>]) -> Vec<u8> {
    assert!(items.len() < 24);
    let mut v = vec![0x80 | items.len() as u8];
    for item in items {
        v.extend_from_slice(item);
    }
    v
}
fn c_txt(s: &str) -> Vec<u8> {
    assert!(s.len() < 24);
    let mut v = vec![0x60 | s.len() as u8];
    v.extend_from_slice(s.as_bytes());
    v
}
fn c_uint(v: u64) -> Vec<u8> {
    assert!(v < 24);
    vec![v as u8]
}
fn c_uint_nonminimal(v: u8) -> Vec<u8> {
    // One-byte-argument form (0x18) for a value that fits inline: non-minimal.
    vec![0x18, v]
}
fn c_bytes(b: &[u8]) -> Vec<u8> {
    let mut v = if b.len() < 24 {
        vec![0x40 | b.len() as u8]
    } else {
        vec![0x58, b.len() as u8]
    };
    v.extend_from_slice(b);
    v
}
fn c_bool(b: bool) -> Vec<u8> {
    vec![if b { 0xF5 } else { 0xF4 }]
}
fn c_null() -> Vec<u8> {
    vec![0xF6]
}

fn decode_err(bytes: &[u8]) -> CodecError {
    decode_canonical::<Sync2Frame>(bytes, MAX_SYNC2_FRAME_BYTES).expect_err("must reject")
}

#[test]
fn hostile_unknown_frame_name_rejected() {
    let bytes = c_arr(&[c_txt("bogus_frame"), c_uint(1)]);
    assert_eq!(decode_err(&bytes), CodecError::UnknownVariant);
}

#[test]
fn hostile_wrong_array_length_rejected() {
    // snapshot_start requires 6 elements; give it 2.
    let bytes = c_arr(&[c_txt("snapshot_start"), c_txt("anchor_to_client")]);
    assert!(matches!(
        decode_err(&bytes),
        CodecError::WrongArrayLength { expected: 6, .. }
    ));
}

#[test]
fn hostile_non_minimal_integer_rejected() {
    let bytes = c_arr(&[
        c_txt("snapshot_start"),
        c_txt("anchor_to_client"),
        c_bytes(&d32(1)),
        c_bytes(&d32(2)),
        c_uint_nonminimal(5), // entry_count, non-minimally encoded
        c_uint(10),
    ]);
    assert_eq!(decode_err(&bytes), CodecError::NonCanonical);
}

#[test]
fn hostile_trailing_bytes_rejected() {
    let frame = Sync2Frame::PageComplete(PageComplete {
        phase: Sync2Phase::AnchorToClient,
        page_digest: d32(1),
    });
    let mut bytes = frame.encode_canonical().unwrap();
    bytes.push(0x00);
    assert_eq!(decode_err(&bytes), CodecError::TrailingBytes);
}

#[test]
fn hostile_oversize_frame_rejected_by_bound() {
    let bytes = Sync2Frame::PageComplete(PageComplete {
        phase: Sync2Phase::AnchorToClient,
        page_digest: d32(1),
    })
    .encode_canonical()
    .unwrap();
    // Bound the decode below the frame size.
    assert!(matches!(
        decode_canonical::<Sync2Frame>(&bytes, 4),
        Err(CodecError::TooLarge { .. })
    ));
}

#[test]
fn hostile_unsorted_page_ids_rejected_on_decode() {
    let bytes = c_arr(&[
        c_txt("ids_page"),
        c_txt("anchor_to_client"),
        c_bytes(&d32(1)),
        c_null(),
        c_arr(&[c_bytes(&eid(9, 4)), c_bytes(&eid(1, 4))]), // descending
        c_bool(false),
    ]);
    assert_eq!(decode_err(&bytes), CodecError::UnsortedSet);
}

#[test]
fn unsorted_page_ids_rejected_on_encode() {
    let page = IdsPage {
        phase: Sync2Phase::AnchorToClient,
        snapshot_digest: d32(1),
        after_exclusive: None,
        entry_ids: vec![eid(9, 4), eid(1, 4)],
        done: false,
    };
    assert_eq!(
        Sync2Frame::IdsPage(page).encode_canonical(),
        Err(CodecError::UnsortedSet)
    );
}

#[test]
fn hostile_busy_with_false_retryable_rejected() {
    let bytes = c_arr(&[
        c_txt("refuse"),
        c_txt("busy"),
        c_bool(false), // busy must be retryable
        c_uint(5),
        c_arr(&[c_txt("capacity"), c_uint(12)]),
    ]);
    assert_eq!(decode_err(&bytes), CodecError::NonCanonical);
}

#[test]
fn hostile_busy_with_null_retry_after_rejected() {
    let bytes = c_arr(&[
        c_txt("refuse"),
        c_txt("busy"),
        c_bool(true),
        c_null(), // busy requires a nonzero retry_after
        c_arr(&[c_txt("capacity"), c_uint(12)]),
    ]);
    assert_eq!(decode_err(&bytes), CodecError::NonCanonical);
}

#[test]
fn hostile_terminal_refusal_marked_retryable_rejected() {
    let bytes = c_arr(&[
        c_txt("refuse"),
        c_txt("snapshot_mismatch"),
        c_bool(true), // terminal codes must be non-retryable
        c_null(),
        c_arr(&[c_txt("snapshot"), c_bytes(&d32(1)), c_bytes(&d32(2))]),
    ]);
    assert_eq!(decode_err(&bytes), CodecError::NonCanonical);
}

#[test]
fn hostile_unknown_refusal_code_rejected() {
    let bytes = c_arr(&[
        c_txt("refuse"),
        c_txt("bogus_code"),
        c_bool(false),
        c_null(),
        c_arr(&[c_txt("x")]),
    ]);
    assert_eq!(decode_err(&bytes), CodecError::UnknownVariant);
}

// ===========================================================================
// FSM traces: routing the three modes, frame/phase ordering, and the one-way
// read guarantee.
// ===========================================================================

fn only_refusal(actions: &[Sync2Action]) -> &Sync2Refusal {
    let mut found = None;
    for action in actions {
        if let Sync2Action::Send(Sync2Frame::Refuse(r)) = action {
            found = Some(r);
        }
    }
    found.expect("expected a Refuse action")
}

fn read_committed_anchor(namespace: [u8; 32], count: u32) -> Sync2Session<HarnessRepo> {
    let repo = HarnessRepo::new(namespace, Sync2ModeTag::ReadCommitted).with_plan(vec![(
        Sync2Phase::AnchorToClient,
        RoleSpec::Sender(items(0, count)),
    )]);
    Sync2Session::responder(repo)
}

#[test]
fn responder_routes_read_committed_and_sends_snapshot_start_first() {
    let namespace = ns(1);
    let mut anchor = read_committed_anchor(namespace, 3);
    let actions = anchor.on_frame(Sync2Frame::OpenNamespace(open_namespace(
        namespace,
        Sync2Mode::ReadCommitted,
    )));
    assert_eq!(anchor.mode(), Sync2ModeTag::ReadCommitted);
    // First emitted frame is SnapshotStart, then a single done IdsPage.
    let sends: Vec<Sync2FrameName> = actions
        .iter()
        .filter_map(|a| match a {
            Sync2Action::Send(f) => Some(f.name()),
            _ => None,
        })
        .collect();
    assert_eq!(
        sends,
        vec![Sync2FrameName::SnapshotStart, Sync2FrameName::IdsPage]
    );
}

#[test]
fn responder_refuses_when_open_namespace_routing_fails() {
    let namespace = ns(1);
    let repo = HarnessRepo::new(namespace, Sync2ModeTag::HostReconcileStaged).with_open_error(
        Sync2Refusal::OperationNotFound {
            operation_id: d32(9),
        },
    );
    let mut anchor = Sync2Session::responder(repo);
    let actions = anchor.on_frame(Sync2Frame::OpenNamespace(open_namespace(
        namespace,
        Sync2Mode::HostReconcileStaged {
            operation_id: d32(9),
            namespace_token: d32(8),
        },
    )));
    assert!(matches!(
        only_refusal(&actions),
        Sync2Refusal::OperationNotFound { .. }
    ));
    assert!(anchor.is_terminated());
}

#[test]
fn responder_rejects_non_open_first_frame() {
    let mut anchor = read_committed_anchor(ns(1), 2);
    let actions = anchor.on_frame(Sync2Frame::PageComplete(PageComplete {
        phase: Sync2Phase::AnchorToClient,
        page_digest: d32(1),
    }));
    match only_refusal(&actions) {
        Sync2Refusal::UnexpectedFrame {
            expected_frame_names,
            observed_frame_name,
            ..
        } => {
            assert_eq!(expected_frame_names, &vec![Sync2FrameName::OpenNamespace]);
            assert_eq!(*observed_frame_name, Sync2FrameName::PageComplete);
        }
        other => panic!("expected unexpected_frame, got {other:?}"),
    }
}

#[test]
fn read_committed_anchor_rejects_inbound_data_frame_one_way() {
    // The public read path exposes a one-way committed snapshot: a client cannot
    // push an EntriesChunk to mutate anchor state.
    let namespace = ns(1);
    let mut anchor = read_committed_anchor(namespace, 2);
    let _ = anchor.on_frame(Sync2Frame::OpenNamespace(open_namespace(
        namespace,
        Sync2Mode::ReadCommitted,
    )));
    let actions = anchor.on_frame(Sync2Frame::EntriesChunk(EntriesChunk {
        phase: Sync2Phase::AnchorToClient,
        page_digest: d32(1),
        request_id: 0,
        chunk_index: 0,
        done: true,
        bundle_bytes: vec![0x80],
    }));
    assert!(matches!(
        only_refusal(&actions),
        Sync2Refusal::UnexpectedFrame { .. }
    ));
    // No admission or promotion could have occurred.
    assert!(!actions
        .iter()
        .any(|a| matches!(a, Sync2Action::Admit(_) | Sync2Action::PromoteDirection)));
}
