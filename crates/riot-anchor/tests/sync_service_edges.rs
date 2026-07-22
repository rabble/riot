//! WU-M2 coverage — the `sync/2` anchor adapter's refusal / edge branches:
//! item-decode rejections and their admission subjects, the immutable
//! [`AnchorSnapshot`] projection, the staging [`AnchorStage`] inventory helpers,
//! and every `open_namespace` lifecycle refusal (invalid mode, missing / non-
//! prepared / expired operation, retired token epoch). These drive the trust-
//! boundary code the happy-path composite-commit tests never reach.

mod hosting_common;

use hosting_common::*;

use std::cell::RefCell;
use std::rc::Rc;

use riot_anchor::repository::{AnchorRepository, OperationStatus, StagedEntry};
use riot_anchor::sync_service::{
    encode_item, verify_anchor_item, AnchorSnapshot, AnchorSyncRepository, ItemReject,
    MAX_ANCHOR_ITEM_BYTES, MAX_ITEM_PAYLOAD_BYTES,
};
use riot_anchor::work::TokenSecretRing;

use riot_anchor_protocol::authority::TicketReason;
use riot_anchor_protocol::codec::decode_canonical;
use riot_anchor_protocol::control::{ControlOutcome, ControlRefusal, ControlResponseV1};
use riot_anchor_protocol::records::{ControlOperationKind, RootSignedTicketCoreEnvelopeV2};
use riot_anchor_protocol::sync2::{
    AdmissionSubject, OpenNamespace, PhaseParty, Sync2DirectionStage, Sync2Mode, Sync2ModeTag,
    Sync2Phase, Sync2Refusal, Sync2Repository, Sync2Snapshot,
};

use riot_anchor_protocol::codec::CanonicalRecord;

const NOW: u64 = 1_000u64;
const EXPIRY: u64 = 1_000_000u64;

// ---------------------------------------------------------------------------
// item decode rejections + admission subjects
// ---------------------------------------------------------------------------

#[test]
fn oversize_item_is_rejected_as_a_bundle_framing_failure() {
    // An item beyond the anchor-profile ceiling never decodes.
    let oversized = vec![0u8; MAX_ANCHOR_ITEM_BYTES + 1];
    assert_eq!(verify_anchor_item(&oversized), Err(ItemReject::Oversize));
    assert_eq!(ItemReject::Oversize.subject(), AdmissionSubject::Bundle);
}

#[test]
fn wrong_version_byte_is_malformed() {
    // A genuine item with its leading version byte corrupted is malformed framing.
    let item = make_item("v");
    let mut bad = item.item_bytes.clone();
    bad[0] = 0x02; // ITEM_VERSION is 1
    assert_eq!(verify_anchor_item(&bad), Err(ItemReject::Malformed));
    assert_eq!(ItemReject::Malformed.subject(), AdmissionSubject::Bundle);
}

#[test]
fn trailing_bytes_after_a_complete_item_are_malformed() {
    let item = make_item("trailing");
    let mut bad = item.item_bytes.clone();
    bad.push(0xff); // one extra byte past the payload
    assert_eq!(verify_anchor_item(&bad), Err(ItemReject::Malformed));
}

#[test]
fn a_truncated_length_prefix_is_malformed() {
    // A single version byte with no room for the entry length prefix.
    assert_eq!(verify_anchor_item(&[1u8]), Err(ItemReject::Malformed));
}

#[test]
fn a_declared_payload_length_over_the_ceiling_is_oversize() {
    // Hand-build an item whose payload-length prefix exceeds the payload ceiling.
    // The ceiling check fires before the bytes are taken, so the frame need not
    // actually carry that many bytes.
    let mut item = Vec::new();
    item.push(1u8); // version
    item.extend_from_slice(&0u32.to_be_bytes()); // entry_len 0
    item.extend_from_slice(&0u32.to_be_bytes()); // cap_len 0
    item.extend_from_slice(&[0u8; 64]); // signature
    item.extend_from_slice(&((MAX_ITEM_PAYLOAD_BYTES as u32) + 1).to_be_bytes());
    assert_eq!(verify_anchor_item(&item), Err(ItemReject::Oversize));
}

#[test]
fn non_canonical_entry_bytes_reject_as_an_entry_subject() {
    // Well-framed item, but the entry bytes are not a canonical Willow entry.
    let sig = [0u8; 64];
    let item = encode_item(&[0xffu8; 8], &[0u8; 4], &sig, &[]);
    assert_eq!(
        verify_anchor_item(&item),
        Err(ItemReject::NonCanonicalEntry)
    );
    assert_eq!(
        ItemReject::NonCanonicalEntry.subject(),
        AdmissionSubject::Entry
    );
    assert_eq!(
        ItemReject::NonCanonicalCapability.subject(),
        AdmissionSubject::Entry
    );
    assert_eq!(
        ItemReject::PayloadMismatch.subject(),
        AdmissionSubject::Entry
    );
    assert_eq!(
        ItemReject::DoesNotAuthorise.subject(),
        AdmissionSubject::Entry
    );
}

#[test]
fn payload_length_mismatch_is_rejected() {
    // Re-frame a genuine entry/cap/sig with a payload of the WRONG length: the
    // entry's declared payload_length no longer matches the carried bytes.
    let item = make_item("len");
    let parts = decode_parts(&item.item_bytes);
    let mut wrong = parts.payload.clone();
    wrong.push(0x00); // change the length
    let reframed = encode_item(&parts.entry, &parts.cap, &parts.sig, &wrong);
    assert_eq!(
        verify_anchor_item(&reframed),
        Err(ItemReject::PayloadMismatch)
    );
}

#[test]
fn payload_digest_mismatch_is_rejected() {
    // Same length, different bytes: the WILLIAM3 digest no longer matches.
    let item = make_item("digest");
    let parts = decode_parts(&item.item_bytes);
    assert!(!parts.payload.is_empty(), "fixture payload is non-empty");
    let mut tampered = parts.payload.clone();
    tampered[0] ^= 0xff;
    let reframed = encode_item(&parts.entry, &parts.cap, &parts.sig, &tampered);
    assert_eq!(
        verify_anchor_item(&reframed),
        Err(ItemReject::PayloadMismatch)
    );
}

/// Minimal re-decode of the anchor-item framing so a test can re-frame the parts.
struct Parts {
    entry: Vec<u8>,
    cap: Vec<u8>,
    sig: [u8; 64],
    payload: Vec<u8>,
}
fn decode_parts(item: &[u8]) -> Parts {
    let mut cursor = 1usize; // skip version
    let read_len = |c: &mut usize| -> usize {
        let v = u32::from_be_bytes([item[*c], item[*c + 1], item[*c + 2], item[*c + 3]]) as usize;
        *c += 4;
        v
    };
    let entry_len = read_len(&mut cursor);
    let entry = item[cursor..cursor + entry_len].to_vec();
    cursor += entry_len;
    let cap_len = read_len(&mut cursor);
    let cap = item[cursor..cursor + cap_len].to_vec();
    cursor += cap_len;
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&item[cursor..cursor + 64]);
    cursor += 64;
    let payload_len = read_len(&mut cursor);
    let payload = item[cursor..cursor + payload_len].to_vec();
    Parts {
        entry,
        cap,
        sig,
        payload,
    }
}

// ---------------------------------------------------------------------------
// AnchorSnapshot — the immutable committed-base projection
// ---------------------------------------------------------------------------

/// Insert a community and promote `entries` into its committed namespace so a
/// snapshot can be materialised over real rows.
fn commit_entries(repo: &mut AnchorRepository, community: [u8; 32], entries: &[StagedEntry]) {
    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community, NOW).expect("community");
    for entry in entries {
        tx.insert_committed_entry(&community, 0, entry)
            .expect("commit entry");
    }
    tx.commit().expect("commit");
}

#[test]
fn snapshot_over_committed_entries_reports_count_bytes_ids_and_lookup() {
    // A genuine committed entry, plus a second entry placed in the SAME namespace
    // by construction (each communal author gets its own unlinkable namespace, so
    // two independent alerts never share one — we derive the sibling deterministically).
    let a = make_item("alpha");
    let namespace_id = a.namespace_id;
    let community = d32(0x70);

    let mut b = a.staged.clone();
    b.entry_id = d32(0x01);
    b.payload_digest = d32(0x02);
    b.payload_length = 3;
    b.item_bytes = vec![0xAB; 20];

    let mut repo = repo();
    commit_entries(&mut repo, community, &[a.staged.clone(), b.clone()]);

    let snapshot = AnchorSnapshot::from_committed(&repo, &namespace_id);
    assert_eq!(snapshot.entry_count(), 2);
    assert_eq!(
        snapshot.logical_bytes(),
        (a.item_bytes.len() + b.item_bytes.len()) as u64
    );

    // sorted_entry_ids is deterministic ascending.
    let ids = snapshot.sorted_entry_ids();
    assert_eq!(ids.len(), 2);
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "ids come back sorted");

    // item_bytes resolves a present id and misses an absent one.
    assert_eq!(
        snapshot.item_bytes(&a.entry_id).as_deref(),
        Some(a.item_bytes.as_slice())
    );
    assert!(snapshot.item_bytes(&d32(0xEE)).is_none());

    // The digest is stable across re-materialisation of the same committed base.
    let again = AnchorSnapshot::from_committed(&repo, &namespace_id);
    assert_eq!(snapshot.snapshot_digest(), again.snapshot_digest());
}

#[test]
fn snapshot_over_an_empty_namespace_is_empty() {
    let repo = repo();
    let snapshot = AnchorSnapshot::from_committed(&repo, &d32(0x71));
    assert_eq!(snapshot.entry_count(), 0);
    assert_eq!(snapshot.logical_bytes(), 0);
    assert!(snapshot.sorted_entry_ids().is_empty());
}

// ---------------------------------------------------------------------------
// AnchorStage — the operation-private staging inventory helpers
// ---------------------------------------------------------------------------

/// Route an `OpenNamespace` for a genuine prepared host operation and return the
/// receiver stage.
fn open_stage(
    shared: &Rc<RefCell<AnchorRepository>>,
    ring: &TokenSecretRing,
    operation_id: [u8; 32],
    namespace_id: [u8; 32],
) -> riot_anchor::sync_service::AnchorStage {
    let token = ring
        .derive(0, &operation_id, &namespace_id, EXPIRY)
        .unwrap();
    let adapter = AnchorSyncRepository::new(Rc::clone(shared), ring.clone(), NOW);
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1, 2, 3],
        ticket_core_bytes: vec![9u8; 40],
        namespace_id,
        mode: Sync2Mode::HostReconcileStaged {
            operation_id,
            namespace_token: token,
        },
    };
    let opened = adapter.open_namespace(&open).expect("routes");
    for (_, party) in opened.parties {
        if let PhaseParty::Receiver(stage) = party {
            return stage;
        }
    }
    panic!("no receiver stage");
}

fn prepared(repo: &mut AnchorRepository, operation_id: [u8; 32], namespace_id: [u8; 32]) {
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let token = ring
        .derive(0, &operation_id, &namespace_id, EXPIRY)
        .unwrap();
    insert_prepared_operation(
        repo,
        operation_id,
        [namespace_id, d32(0x61), d32(0x62)],
        [token, d32(0), d32(0)],
        0,
        1_000,
        EXPIRY,
        0,
    );
}

#[test]
fn stage_missing_and_resulting_digest_account_for_staged_entries() {
    let item = make_item("stage");
    let namespace_id = item.namespace_id;
    let operation_id = d32(0x52);
    let ring = TokenSecretRing::new(0, [3u8; 32]);

    let mut base = repo();
    prepared(&mut base, operation_id, namespace_id);
    let shared = Rc::new(RefCell::new(base));

    let digest_before = {
        let stage = open_stage(&shared, &ring, operation_id, namespace_id);
        // Nothing staged yet: an advertised id is missing.
        assert_eq!(
            stage.missing(&[item.entry_id.to_vec()]),
            vec![item.entry_id.to_vec()]
        );
        stage.resulting_digest(&namespace_id)
    };

    // Admit the genuine entry into staging.
    {
        let mut stage = open_stage(&shared, &ring, operation_id, namespace_id);
        stage
            .admit(
                &[item.entry_id.to_vec()],
                std::slice::from_ref(&item.item_bytes),
            )
            .expect("admit");
    }

    let stage = open_stage(&shared, &ring, operation_id, namespace_id);
    // The staged id is no longer missing; an unrelated id still is.
    let missing = stage.missing(&[item.entry_id.to_vec(), d32(0xAB).to_vec()]);
    assert_eq!(missing, vec![d32(0xAB).to_vec()]);
    // The resulting digest changed once an entry landed in staging.
    assert_ne!(stage.resulting_digest(&namespace_id), digest_before);
    // promote() is a documented no-op (the composite Commit promotes atomically).
    let mut stage = stage;
    stage.promote();
}

#[test]
fn admit_rejects_an_id_that_disagrees_with_the_verified_entry() {
    let item = make_item("mismatch");
    let namespace_id = item.namespace_id;
    let operation_id = d32(0x53);
    let ring = TokenSecretRing::new(0, [3u8; 32]);

    let mut base = repo();
    prepared(&mut base, operation_id, namespace_id);
    let shared = Rc::new(RefCell::new(base));

    let mut stage = open_stage(&shared, &ring, operation_id, namespace_id);
    // The advertised inventory id does not match the verified entry id.
    let result = stage.admit(
        &[d32(0x01).to_vec()],
        std::slice::from_ref(&item.item_bytes),
    );
    assert_eq!(result, Err(AdmissionSubject::Entry));
    // Nothing was staged.
    assert_eq!(
        shared
            .borrow()
            .staged_entries(&operation_id, &namespace_id)
            .unwrap()
            .len(),
        0
    );
}

// ---------------------------------------------------------------------------
// open_namespace lifecycle refusals
// ---------------------------------------------------------------------------

fn adapter_over(
    repo: AnchorRepository,
    ring: TokenSecretRing,
    now: u64,
) -> (Rc<RefCell<AnchorRepository>>, AnchorSyncRepository) {
    let shared = Rc::new(RefCell::new(repo));
    let adapter = AnchorSyncRepository::new(Rc::clone(&shared), ring, now);
    (shared, adapter)
}

#[test]
fn replica_mode_is_still_invalid_for_this_responder() {
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let (_shared, adapter) = adapter_over(repo(), ring, NOW);

    let mode = Sync2Mode::ReplicaIntoStaged {
        operation_id: d32(0x01),
        namespace_token: d32(0x02),
    };
    let observed_tag = mode.tag();
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1],
        ticket_core_bytes: vec![9u8; 40],
        namespace_id: d32(0x40),
        mode,
    };
    match adapter.open_namespace(&open) {
        Err(Sync2Refusal::InvalidMode { observed_mode }) => {
            assert_eq!(observed_mode, observed_tag)
        }
        Err(other) => panic!("expected InvalidMode, got {other:?}"),
        Ok(_) => panic!("expected InvalidMode refusal, got a routed namespace"),
    }
}

// ---------------------------------------------------------------------------
// ReadCommitted — the public follower pull path (SECURITY SURFACE)
// ---------------------------------------------------------------------------

/// The observation time for ReadCommitted tests: after ticket issuance (1_000),
/// before ticket expiry.
const READ_NOW: u64 = 2_000;
/// The site fixture's ticket expiry.
const TICKET_EXPIRY: u64 = 90_000;

fn read_committed_open(ticket_core_bytes: Vec<u8>, namespace_id: [u8; 32]) -> OpenNamespace {
    OpenNamespace {
        protocol_version: 2,
        session_id: vec![1],
        ticket_core_bytes,
        namespace_id,
        mode: Sync2Mode::ReadCommitted,
    }
}

/// A committed site plus an adapter observing at `now`.
fn committed_site_adapter(
    seed: u8,
    now: u64,
) -> (
    hosting_common::SiteFixture,
    Rc<RefCell<AnchorRepository>>,
    AnchorSyncRepository,
) {
    let site = make_site_fixture(seed, 3, 1_000, TICKET_EXPIRY);
    let mut base = repo();
    commit_site_fixture(&mut base, &site, NOW);
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let (shared, adapter) = adapter_over(base, ring, now);
    (site, shared, adapter)
}

#[test]
fn read_committed_with_a_valid_ticket_serves_the_committed_snapshot() {
    let (site, shared, adapter) = committed_site_adapter(0x31, READ_NOW);

    for namespace_id in site.namespaces {
        let opened = adapter
            .open_namespace(&read_committed_open(
                site.ticket_envelope_bytes.clone(),
                namespace_id,
            ))
            .expect("a valid ticket over a committed community routes");

        assert_eq!(opened.namespace_id, namespace_id);
        assert_eq!(opened.mode, Sync2ModeTag::ReadCommitted);
        assert!(opened.stale_source.is_none());

        // Serve-only: exactly ONE phase, and the anchor is the SENDER
        // (the FSM's one-way committed mode).
        assert_eq!(opened.parties.len(), 1, "single sender direction");
        let (phase, party) = &opened.parties[0];
        assert_eq!(*phase, Sync2Phase::AnchorToClient);
        let snapshot = match party {
            PhaseParty::Sender(snapshot) => snapshot,
            PhaseParty::Receiver(_) => panic!("ReadCommitted must never open a receiver stage"),
        };

        // The served snapshot IS the committed base.
        let expected = AnchorSnapshot::from_committed(&shared.borrow(), &namespace_id);
        assert_eq!(
            snapshot.entry_count(),
            1,
            "fixture commits one entry per namespace"
        );
        assert_eq!(snapshot.entry_count(), expected.entry_count());
        assert_eq!(snapshot.logical_bytes(), expected.logical_bytes());
        assert_eq!(snapshot.snapshot_digest(), expected.snapshot_digest());
    }
}

#[test]
fn read_committed_refuses_a_forged_root_signature() {
    let (site, _shared, adapter) = committed_site_adapter(0x32, READ_NOW);

    let mut envelope = decode_canonical::<RootSignedTicketCoreEnvelopeV2>(
        &site.ticket_envelope_bytes,
        site.ticket_envelope_bytes.len(),
    )
    .expect("fixture ticket decodes");
    envelope.root_signature[0] ^= 0x01;
    let forged = envelope
        .encode_canonical()
        .expect("re-encode forged ticket");

    match adapter.open_namespace(&read_committed_open(forged, site.namespaces[1])) {
        Err(Sync2Refusal::InvalidTicket { reason }) => {
            assert_eq!(reason, TicketReason::Signature)
        }
        Err(other) => panic!("expected InvalidTicket(signature), got {other:?}"),
        Ok(_) => panic!("a forged root signature must never route"),
    }
}

#[test]
fn read_committed_refuses_undecodable_ticket_bytes() {
    let (site, _shared, adapter) = committed_site_adapter(0x33, READ_NOW);

    match adapter.open_namespace(&read_committed_open(vec![0xFF; 40], site.namespaces[1])) {
        Err(Sync2Refusal::InvalidTicket { reason }) => {
            assert_eq!(reason, TicketReason::Structure)
        }
        Err(other) => panic!("expected InvalidTicket(structure), got {other:?}"),
        Ok(_) => panic!("garbage ticket bytes must never route"),
    }
}

#[test]
fn read_committed_refuses_an_expired_ticket() {
    // Observe at exactly the ticket expiry: expiry is inclusive.
    let (site, _shared, adapter) = committed_site_adapter(0x34, TICKET_EXPIRY);

    match adapter.open_namespace(&read_committed_open(
        site.ticket_envelope_bytes.clone(),
        site.namespaces[1],
    )) {
        Err(Sync2Refusal::ExpiredTicket {
            expires_at,
            observed_at,
        }) => {
            assert_eq!(expires_at, TICKET_EXPIRY);
            assert_eq!(observed_at, TICKET_EXPIRY);
        }
        Err(other) => panic!("expected ExpiredTicket, got {other:?}"),
        Ok(_) => panic!("an expired ticket must never route"),
    }
}

#[test]
fn read_committed_refuses_a_namespace_outside_the_tickets_ocw_set() {
    let (site, _shared, adapter) = committed_site_adapter(0x35, READ_NOW);

    let foreign = d32(0xEE);
    assert!(!site.namespaces.contains(&foreign));
    match adapter.open_namespace(&read_committed_open(
        site.ticket_envelope_bytes.clone(),
        foreign,
    )) {
        Err(Sync2Refusal::NamespaceNotMember { namespace_id }) => {
            assert_eq!(namespace_id, foreign)
        }
        Err(other) => panic!("expected NamespaceNotMember, got {other:?}"),
        Ok(_) => panic!("a namespace outside the ticket's O/C/W set must never route"),
    }
}

#[test]
fn read_committed_refuses_a_community_this_anchor_never_committed() {
    // A perfectly valid ticket — but the repository holds NO committed manifest
    // for its community. Nothing to serve, nothing to compare: refuse.
    let site = make_site_fixture(0x36, 3, 1_000, TICKET_EXPIRY);
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let (_shared, adapter) = adapter_over(repo(), ring, READ_NOW);

    match adapter.open_namespace(&read_committed_open(
        site.ticket_envelope_bytes.clone(),
        site.namespaces[1],
    )) {
        Err(Sync2Refusal::ManifestMismatch {
            expected_digest,
            observed_digest,
        }) => {
            assert_eq!(expected_digest, site.manifest_digest);
            assert_eq!(observed_digest, [0u8; 32], "no committed manifest observed");
        }
        Err(other) => panic!("expected ManifestMismatch, got {other:?}"),
        Ok(_) => panic!("an uncommitted community must never route"),
    }
}

#[test]
fn read_committed_refuses_a_ticket_naming_a_different_manifest_digest() {
    let (site, _shared, adapter) = committed_site_adapter(0x37, READ_NOW);

    // A validly ROOT-SIGNED ticket for the same site naming a DIFFERENT
    // (higher-version) manifest digest than the one this anchor committed.
    let swapped = make_payload_swapped_manifest_item(&site);

    match adapter.open_namespace(&read_committed_open(
        swapped.ticket_envelope_bytes.clone(),
        site.namespaces[1],
    )) {
        Err(Sync2Refusal::ManifestMismatch {
            expected_digest,
            observed_digest,
        }) => {
            assert_ne!(expected_digest, site.manifest_digest);
            assert_eq!(observed_digest, site.manifest_digest);
        }
        Err(other) => panic!("expected ManifestMismatch, got {other:?}"),
        Ok(_) => panic!("a ticket naming a different committed manifest must never route"),
    }
}

#[test]
fn read_committed_refuses_a_transport_epoch_below_the_durable_floor() {
    let site = make_site_fixture(0x38, 3, 1_000, TICKET_EXPIRY);
    let mut base = repo();
    commit_site_fixture(&mut base, &site, NOW);
    // Advance the durable per-root transport-epoch floor PAST the fixture
    // ticket's epoch (the fixture signs epoch 1).
    {
        let mut tx = base.begin().expect("begin");
        tx.advance_ticket_transport_epoch(&site.root_id, 2)
            .expect("advance epoch floor");
        tx.commit().expect("commit floor");
    }
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let (_shared, adapter) = adapter_over(base, ring, READ_NOW);

    // Epoch rollback collapses into the generic fail-closed ticket refusal
    // (the closed sync/2 vocabulary carries no epoch detail — mirror of the
    // control plane collapsing it into invalid_ticket_authority).
    match adapter.open_namespace(&read_committed_open(
        site.ticket_envelope_bytes.clone(),
        site.namespaces[1],
    )) {
        Err(Sync2Refusal::InvalidTicket { reason }) => {
            assert_eq!(reason, TicketReason::Structure)
        }
        Err(other) => panic!("expected InvalidTicket(structure), got {other:?}"),
        Ok(_) => panic!("a rolled-back transport epoch must never route"),
    }
}

#[test]
fn a_missing_operation_is_operation_not_found() {
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let operation_id = d32(0x54);
    let (_shared, adapter) = adapter_over(repo(), ring, NOW);
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1],
        ticket_core_bytes: vec![9u8; 40],
        namespace_id: d32(0x40),
        mode: Sync2Mode::HostReconcileStaged {
            operation_id,
            namespace_token: d32(0x02),
        },
    };
    assert!(matches!(
        adapter.open_namespace(&open),
        Err(Sync2Refusal::OperationNotFound { operation_id: id }) if id == operation_id
    ));
}

#[test]
fn a_committed_operation_is_no_longer_openable() {
    let namespace_id = d32(0x40);
    let operation_id = d32(0x55);
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let token = ring
        .derive(0, &operation_id, &namespace_id, EXPIRY)
        .unwrap();

    let mut base = repo();
    prepared(&mut base, operation_id, namespace_id);
    // Terminalise the operation: its tokens are now invalid.
    {
        let mut tx = base.begin().expect("begin");
        tx.set_operation_terminal(&operation_id, OperationStatus::Committed, b"receipt")
            .expect("terminalize");
        tx.commit().expect("commit");
    }

    let (_shared, adapter) = adapter_over(base, ring, NOW);
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1],
        ticket_core_bytes: vec![9u8; 40],
        namespace_id,
        mode: Sync2Mode::HostReconcileStaged {
            operation_id,
            namespace_token: token,
        },
    };
    assert!(matches!(
        adapter.open_namespace(&open),
        Err(Sync2Refusal::OperationNotFound { .. })
    ));
}

#[test]
fn an_expired_operation_is_refused_as_expired() {
    let namespace_id = d32(0x40);
    let operation_id = d32(0x56);
    let ring = TokenSecretRing::new(0, [3u8; 32]);
    let token = ring
        .derive(0, &operation_id, &namespace_id, EXPIRY)
        .unwrap();

    let mut base = repo();
    prepared(&mut base, operation_id, namespace_id); // expiry == EXPIRY

    // Observe at exactly the expiry (inclusive) → expired.
    let (_shared, adapter) = adapter_over(base, ring, EXPIRY);
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1],
        ticket_core_bytes: vec![9u8; 40],
        namespace_id,
        mode: Sync2Mode::HostReconcileStaged {
            operation_id,
            namespace_token: token,
        },
    };
    match adapter.open_namespace(&open) {
        Err(Sync2Refusal::OperationExpired {
            operation_id: id,
            expires_at,
            observed_at,
        }) => {
            assert_eq!(id, operation_id);
            assert_eq!(expires_at, EXPIRY);
            assert_eq!(observed_at, EXPIRY);
        }
        Err(other) => panic!("expected OperationExpired, got {other:?}"),
        Ok(_) => panic!("expected OperationExpired refusal, got a routed namespace"),
    }
}

#[test]
fn a_token_minted_under_a_retired_epoch_is_invalid() {
    // The operation was minted under token_secret_epoch 5, but the ring only holds
    // epoch 0: derivation returns None → the presented token cannot be validated.
    let namespace_id = d32(0x40);
    let operation_id = d32(0x57);
    let ring = TokenSecretRing::new(0, [3u8; 32]);

    let mut base = repo();
    insert_prepared_operation(
        &mut base,
        operation_id,
        [namespace_id, d32(0x61), d32(0x62)],
        [d32(0x11), d32(0), d32(0)],
        0,
        1_000,
        EXPIRY,
        5, // token_secret_epoch not present in the ring
    );

    let (_shared, adapter) = adapter_over(base, ring, NOW);
    let open = OpenNamespace {
        protocol_version: 2,
        session_id: vec![1],
        ticket_core_bytes: vec![9u8; 40],
        namespace_id,
        mode: Sync2Mode::HostReconcileStaged {
            operation_id,
            namespace_token: d32(0x11),
        },
    };
    assert!(matches!(
        adapter.open_namespace(&open),
        Err(Sync2Refusal::InvalidNamespaceToken { namespace_id: id }) if id == namespace_id
    ));
}

// ---------------------------------------------------------------------------
// ordered_host_plan
// ---------------------------------------------------------------------------

#[test]
fn ordered_host_plan_returns_none_for_a_non_prepare_response() {
    // A refusal response carries no ordered O/C/W host plan.
    let response = ControlResponseV1 {
        kind: ControlOperationKind::PrepareHost,
        outcome: ControlOutcome::Refused(ControlRefusal::NotHosted),
    };
    let bytes = response.encode_canonical().expect("encode");
    assert!(riot_anchor::sync_service::ordered_host_plan(&bytes).is_none());
}

#[test]
fn ordered_host_plan_returns_none_for_undecodable_bytes() {
    assert!(riot_anchor::sync_service::ordered_host_plan(&[0xff, 0x00, 0x13]).is_none());
}
