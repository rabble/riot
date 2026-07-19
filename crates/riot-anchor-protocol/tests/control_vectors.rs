//! WU-004 — descriptors, receipts, limits, work stamp, and the `riot/anchor/1`
//! control protocol: golden + hostile canonical vectors.
//!
//! Every assertion transcribes a normative row from
//! `docs/superpowers/specs/2026-07-18-public-community-anchor-network-design.md`:
//! the 82-limit registry and its MVP default/absolute values ("Encoded
//! control-record profile" + the resource table), the descriptor genesis/rotation
//! chain, hosting/listing receipts, the admission work challenge/stamp, and the
//! nine control operations with their exact request/success/refusal envelopes and
//! the closed refusal matrix.

use riot_anchor_protocol::records::{
    AnchorLimitId, AnchorLimitProfileV1, LimitValue, ALL_LIMIT_IDS,
};

// ===========================================================================
// 82-limit registry: IDs, order, and MVP default/absolute values.
// ===========================================================================

/// The registry holds exactly 82 IDs in ascending 1..=82 order.
#[test]
fn limit_registry_is_82_ascending() {
    assert_eq!(ALL_LIMIT_IDS.len(), 82);
    for (index, id) in ALL_LIMIT_IDS.iter().enumerate() {
        assert_eq!(id.id(), (index as u64) + 1);
        assert_eq!(AnchorLimitId::from_id(id.id()), Some(*id));
    }
    assert_eq!(AnchorLimitId::from_id(0), None);
    assert_eq!(AnchorLimitId::from_id(83), None);
}

/// Each ID maps to the exact MVP `(effective, absolute)` pair from the design
/// resource table (bytes in bytes, durations in milliseconds, counts as printed).
#[test]
fn limit_profile_mvp_defaults_match_design_table() {
    let profile = AnchorLimitProfileV1::mvp_defaults(0);
    let g = 1024u64 * 1024 * 1024;
    let m = 1024u64 * 1024;
    let k = 1024u64;
    let s = |v: u64, a: u64| (LimitValue::Scalar(v), LimitValue::Scalar(a));
    let c = |v: (u64, u64), a: (u64, u64)| {
        (
            LimitValue::Compound(v.0, v.1),
            LimitValue::Compound(a.0, a.1),
        )
    };
    let expected: [(AnchorLimitId, (LimitValue, LimitValue)); 82] = [
        (
            AnchorLimitId::LogicalRetainedBytesWholeAnchor,
            s(20 * g, 100 * g),
        ),
        (AnchorLimitId::PhysicalRetainedBytes, s(20 * g, 100 * g)),
        (
            AnchorLimitId::OrdinarySqliteDatabaseIncludingWal,
            s(24 * g, 110 * g),
        ),
        (AnchorLimitId::NonPayloadMetadataBytes, s(2 * g, 8 * g)),
        (AnchorLimitId::SqliteWalBytes, s(256 * m, g)),
        (
            AnchorLimitId::EmergencyRemovalMetadataReserve,
            s(768 * m, 3 * g),
        ),
        (
            AnchorLimitId::EmergencyRemovalWalFsyncReserve,
            s(768 * m, 3 * g),
        ),
        (AnchorLimitId::StagedBytes, s(256 * m, g)),
        (AnchorLimitId::LiveStagedOperations, s(10_000, 50_000)),
        (AnchorLimitId::IdempotencyRows, s(100_000, 500_000)),
        (
            AnchorLimitId::IdempotencyRowsPerSourcePer24h,
            s(2_000, 10_000),
        ),
        (
            AnchorLimitId::ReservedRemovalIdempotencyResultRows,
            s(20_000, 100_000),
        ),
        (AnchorLimitId::IncidentConflictRecords, s(10_000, 50_000)),
        (AnchorLimitId::ConflictProofsPerSiteSubject, s(2, 4)),
        (AnchorLimitId::HostedSites, s(10_000, 50_000)),
        (AnchorLimitId::LogicalBytesPerSite, s(64 * m, 256 * m)),
        (AnchorLimitId::LiveEntriesPerNamespace, s(4_096, 16_384)),
        (AnchorLimitId::ItemPayload, s(m, m)),
        (AnchorLimitId::Bundle, c((8 * m, 64), (8 * m, 64))),
        (AnchorLimitId::ConcurrentSyncControlSessions, s(128, 512)),
        (AnchorLimitId::SessionsPerSource, s(4, 16)),
        (AnchorLimitId::SessionsPerSite, s(8, 32)),
        (AnchorLimitId::TcpListenBacklog, s(256, 1_024)),
        (AnchorLimitId::AcceptedPublicHttpsSockets, s(512, 2_048)),
        (AnchorLimitId::PendingTlsHandshakes, s(64, 256)),
        (AnchorLimitId::TlsHandshakesPerSourcePerMinute, s(30, 120)),
        (AnchorLimitId::TlsHandshakesGloballyPerSecond, s(200, 800)),
        (
            AnchorLimitId::TlsClienthelloTotalHandshakeBytes,
            c((16 * k, 64 * k), (16 * k, 64 * k)),
        ),
        (
            AnchorLimitId::TlsHandshakeCpuWallTime,
            c((100, 5_000), (500, 10_000)),
        ),
        (AnchorLimitId::ActivePublicHttpsConnections, s(256, 1_024)),
        (
            AnchorLimitId::HttpRequestsPerKeepAliveConnection,
            s(100, 1_000),
        ),
        (
            AnchorLimitId::HttpIdleAbsoluteConnectionLifetime,
            c((15_000, 300_000), (60_000, 1_800_000)),
        ),
        (
            AnchorLimitId::HttpDecodedHeaderFieldsOneFieldLine,
            c((64, 8 * k), (64, 8 * k)),
        ),
        (AnchorLimitId::ConcurrentPublicHttpsHandlers, s(128, 512)),
        (AnchorLimitId::QueuedPublicHttpsHandlers, s(128, 512)),
        (
            AnchorLimitId::PublicHttpsRequestsPerSourcePerMinute,
            s(120, 600),
        ),
        (
            AnchorLimitId::PublicHttpsRequestsGloballyPerSecond,
            s(500, 2_000),
        ),
        (
            AnchorLimitId::ConcurrentPublicHttpDatabaseSnapshots,
            s(32, 128),
        ),
        (AnchorLimitId::PublicHttpDatabaseSnapshotsPerSource, s(2, 8)),
        (
            AnchorLimitId::PublicHttpQueryCpuWallTime,
            c((250, 2_000), (1_000, 5_000)),
        ),
        (AnchorLimitId::PublicApiResponseBytes, s(m, 4 * m)),
        (AnchorLimitId::OneStaticWebResponse, s(2 * m, 8 * m)),
        (AnchorLimitId::SearchResultsPerPage, s(50, 100)),
        (AnchorLimitId::SearchQueryUtf8Bytes, s(128, 256)),
        (AnchorLimitId::DirectoryListings, s(10_000, 50_000)),
        (AnchorLimitId::DirectoryFeedRecords, s(100_000, 500_000)),
        (AnchorLimitId::VerificationQueueJobs, s(512, 2_048)),
        (AnchorLimitId::VerificationCpuPerRequest, s(500, 2_000)),
        (
            AnchorLimitId::AggregateOutstandingVerificationCpuBudget,
            s(16_000, 64_000),
        ),
        (
            AnchorLimitId::ReservedOwnerRemovalVerificationPermits,
            s(4, 4),
        ),
        (AnchorLimitId::QueuedReservedRemovalJobs, s(256, 1_024)),
        (
            AnchorLimitId::QueuedReservedRemovalCanonicalBytes,
            s(4 * m, 16 * m),
        ),
        (
            AnchorLimitId::ReservedValidRemovalDatabaseWriterPermits,
            s(2, 2),
        ),
        (AnchorLimitId::EmergencyCheckpointWorker, s(1, 1)),
        (
            AnchorLimitId::OwnerRemovalAttemptsPerSourcePerMinute,
            s(10, 40),
        ),
        (
            AnchorLimitId::OwnerRemovalAttemptsGloballyPerSecond,
            s(100, 400),
        ),
        (AnchorLimitId::WorkChallengeSignaturesPerSecond, s(100, 500)),
        (AnchorLimitId::WorkChallengesPerSourcePerMinute, s(30, 120)),
        (AnchorLimitId::StaticProjectionBytes, s(5 * g, 20 * g)),
        (AnchorLimitId::RendererTemporaryFilesystem, s(g, 4 * g)),
        (
            AnchorLimitId::RendererTemporaryFilesInodes,
            s(10_000, 50_000),
        ),
        (AnchorLimitId::ConcurrentRendererJobs, s(4, 16)),
        (
            AnchorLimitId::RendererCpuWallTimePerGeneration,
            s(30_000, 120_000),
        ),
        (AnchorLimitId::PublishedGenerationsPerSite, s(2, 2)),
        (
            AnchorLimitId::LocalOperationalLogBytesAllClasses,
            s(512 * m, 2 * g),
        ),
        (AnchorLimitId::DiagnosticLogBytes, s(128 * m, 512 * m)),
        (AnchorLimitId::RotatedLocalLogFiles, s(128, 512)),
        (AnchorLimitId::ConcurrentGossipSessionsPerPeer, s(2, 4)),
        (AnchorLimitId::GossipTransferPerPeerPerHour, s(256 * m, g)),
        (AnchorLimitId::PendingPublicIrohQuicHandshakes, s(64, 256)),
        (
            AnchorLimitId::PublicIrohQuicHandshakeWallTime,
            s(5_000, 10_000),
        ),
        (
            AnchorLimitId::ControlSyncFirstFrameWallTime,
            s(5_000, 10_000),
        ),
        (
            AnchorLimitId::ControlFrameReadWriteWallTime,
            s(10_000, 30_000),
        ),
        (
            AnchorLimitId::SyncFrameReadWriteWallTime,
            s(30_000, 120_000),
        ),
        (AnchorLimitId::ControlSyncProgressInterval, s(5_000, 15_000)),
        (
            AnchorLimitId::ControlSyncIdleAbsoluteSessionLifetime,
            c((30_000, 900_000), (60_000, 3_600_000)),
        ),
        (AnchorLimitId::SnapshotCursorLifetime, s(900_000, 3_600_000)),
        (
            AnchorLimitId::PublicIrohHandshakesPerSourcePerMinute,
            s(30, 120),
        ),
        (
            AnchorLimitId::PublicIrohHandshakesGloballyPerSecond,
            s(200, 800),
        ),
        (AnchorLimitId::DirectRootPrefilterQueueJobs, s(128, 512)),
        (
            AnchorLimitId::DirectRootPrefilterQueueCanonicalBytes,
            s(4 * m, 16 * m),
        ),
        (
            AnchorLimitId::DirectRootSignatureCpuWallTime,
            c((2, 50), (10, 100)),
        ),
    ];
    assert_eq!(profile.entries.len(), 82);
    for (index, (id, (effective, absolute))) in expected.iter().enumerate() {
        let entry = &profile.entries[index];
        assert_eq!(entry.id, *id, "id at index {index}");
        assert_eq!(entry.id.id(), (index as u64) + 1);
        assert_eq!(&entry.effective, effective, "effective for {:?}", id);
        assert_eq!(&entry.absolute, absolute, "absolute for {:?}", id);
    }
}

// ===========================================================================
// Shared fixtures for the descriptor / receipt / control vectors.
// ===========================================================================

use ed25519_dalek::{Signer, SigningKey};
use riot_anchor_protocol::codec::{decode_canonical, CanonicalRecord, CodecError};
use riot_anchor_protocol::control::{
    verify_descriptor_chain, CheckpointReason, CommitHostV1, ControlOperation, ControlOutcome,
    ControlRefusal, ControlRequestV1, ControlResponseV1, ControlSuccess, CursorKind, CursorReason,
    DescribeSuccessV1, DescribeV1, DescriptorError, EffectiveOperationLimits, FeedPullSuccessV1,
    GetOperationState, GetOperationSuccessV1, GetOperationV1, GetWorkChallengeV1, PeerAuthStage,
    PeerContextReason, PeerSide, PrepareHostV1, PrepareKind, PrepareReplicaV1, PrepareSuccessV1,
    PullDirectoryFeedV1, PullDirectorySnapshotV1, RefusalSubject, RetryScope, SnapshotCursorBodyV1,
    SnapshotCursorV1, SnapshotPullSuccessV1, StorageClass, SubmitListingV1,
    TerminalOperationOutcome, TransportMode,
};
use riot_anchor_protocol::digest::{anchor_id as compute_anchor_id, digest_v1, label, work_proof};
use riot_anchor_protocol::records::{
    AnchorBootstrapV1, AnchorDescriptorBodyV1, AnchorSignedBody, BootstrapDescriptorV1,
    ControlOperationKind, DescriptorEnvelopeV1, DescriptorFloor, EnabledRole, HostingReceiptBodyV1,
    HostingReceiptV1, HostingStatus, ListingReceiptBodyV1, ListingReceiptV1, NamespaceResult,
    OperatorSignedEnvelopeV1, OperatorVerificationKeyV1, PublicSiteTicketV2Core,
    ReplicaPrepareChallengeV1, ReplicaSourceAttestationBodyV1, RootSignedTicketCoreEnvelopeV2,
    TransportFloor, WorkChallengeBodyV1, WorkChallengeV1, WorkStampError, WorkStampV1,
};

fn sk(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}
fn pk(k: &SigningKey) -> [u8; 32] {
    k.verifying_key().to_bytes()
}
fn vkey(k: &SigningKey) -> OperatorVerificationKeyV1 {
    OperatorVerificationKeyV1 { public_key: pk(k) }
}
fn d32(seed: u8) -> [u8; 32] {
    [seed; 32]
}
fn d16(seed: u8) -> [u8; 16] {
    [seed; 16]
}
fn triple(a: u8, b: u8, c: u8) -> [[u8; 32]; 3] {
    [d32(a), d32(b), d32(c)]
}

fn roundtrip<T>(value: &T, max: usize)
where
    T: CanonicalRecord + PartialEq + std::fmt::Debug,
{
    let bytes = value.encode_canonical().expect("encode");
    let decoded = decode_canonical::<T>(&bytes, max).expect("decode");
    assert_eq!(&decoded, value, "round-trip mismatch");
    // Trailing byte must be rejected.
    let mut extended = bytes.clone();
    extended.push(0);
    assert!(matches!(
        decode_canonical::<T>(&extended, max + 1),
        Err(CodecError::TrailingBytes)
    ));
}

fn sign_body<B: AnchorSignedBody>(body: B, k: &SigningKey) -> OperatorSignedEnvelopeV1<B> {
    let mut env = OperatorSignedEnvelopeV1 {
        body,
        operator_signature: [0u8; 64],
    };
    let preimage = env.signing_preimage().expect("preimage");
    env.operator_signature = k.sign(&preimage).to_bytes();
    env
}

fn ticket_core(root: &SigningKey) -> RootSignedTicketCoreEnvelopeV2 {
    let core = PublicSiteTicketV2Core {
        root_id: pk(root),
        o_namespace_id: d32(10),
        c_namespace_id: d32(11),
        w_namespace_id: d32(12),
        manifest_digest: d32(13),
        manifest_version: 3,
        min_sync_version: 2,
        manifest_required_transport: TransportFloor::RequireNone,
        transport_floor: TransportFloor::RequireNone,
        transport_epoch: 1,
        issued_unix_seconds: 1000,
        expiry_unix_seconds: 2000,
    };
    let mut env = RootSignedTicketCoreEnvelopeV2 {
        core,
        root_signature: [0u8; 64],
    };
    let preimage = env.signing_preimage().expect("preimage");
    env.root_signature = root.sign(&preimage).to_bytes();
    env
}

#[allow(clippy::too_many_arguments)]
fn descriptor(
    genesis: &SigningKey,
    genesis_random: [u8; 32],
    current: &SigningKey,
    predecessor: Option<&SigningKey>,
    epoch: u64,
    previous_digest: Option<[u8; 32]>,
    issued_at: u64,
    expires_at: u64,
) -> DescriptorEnvelopeV1 {
    let anchor_id = compute_anchor_id(&pk(genesis), &genesis_random);
    let current_key = vkey(current);
    let body = AnchorDescriptorBodyV1 {
        anchor_id,
        genesis_operator_public_key: pk(genesis),
        genesis_random_256_bits: genesis_random,
        current_operator_verification_key: current_key,
        current_operator_key_id: current_key.operator_key_id().unwrap(),
        descriptor_epoch: epoch,
        previous_descriptor_digest: previous_digest,
        current_iroh_endpoint_id: d32(40),
        https_origin: "https://anchor.example".to_string(),
        operator_display_label: "Example Anchor".to_string(),
        self_reported_failure_domain_label: "eu-west".to_string(),
        supported_control_versions: vec![1],
        supported_sync_versions: vec![1, 2],
        enabled_roles: vec![EnabledRole::Host, EnabledRole::Mirror],
        limit_profile_digest: d32(50),
        predecessor_operator_verification_key: predecessor.map(vkey),
        issued_at,
        expires_at,
    };
    let mut env = DescriptorEnvelopeV1 {
        body,
        current_signature: [0u8; 64],
        predecessor_signature: None,
    };
    let cur_preimage = env.current_signing_preimage().unwrap();
    env.current_signature = current.sign(&cur_preimage).to_bytes();
    if let Some(psk) = predecessor {
        let pred_preimage = env.predecessor_signing_preimage().unwrap();
        env.predecessor_signature = Some(psk.sign(&pred_preimage).to_bytes());
    }
    env
}

// ===========================================================================
// Descriptors: canonical round-trip, key-id + AnchorId recompute, signatures.
// ===========================================================================

#[test]
fn descriptor_genesis_round_trips_and_verifies() {
    let g = sk(1);
    let env = descriptor(&g, d32(99), &g, None, 0, None, 1000, 5000);
    roundtrip(&env, 8 * 1024);
    assert!(env.verify_current().is_ok());
    assert_eq!(env.body.recomputed_anchor_id(), env.body.anchor_id);
    assert_eq!(
        env.body
            .current_operator_verification_key
            .operator_key_id()
            .unwrap(),
        env.body.current_operator_key_id
    );
}

#[test]
fn operator_verification_key_round_trips() {
    roundtrip(&vkey(&sk(7)), 256);
}

// ===========================================================================
// verify_descriptor_chain
// ===========================================================================

fn genesis_floor(
    g: &SigningKey,
    genesis_random: [u8; 32],
) -> (DescriptorEnvelopeV1, DescriptorFloor) {
    let env = descriptor(g, genesis_random, g, None, 0, None, 1000, 5000);
    let floor = DescriptorFloor {
        anchor_id: env.body.anchor_id,
        descriptor_epoch: 0,
        descriptor_digest: env.descriptor_digest().unwrap(),
        operator_verification_key: vkey(g),
    };
    (env, floor)
}

#[test]
fn chain_advances_to_valid_head() {
    let g = sk(1);
    let (env0, floor) = genesis_floor(&g, d32(99));
    let k1 = sk(2);
    let env1 = descriptor(
        &g,
        d32(99),
        &k1,
        Some(&g),
        1,
        Some(env0.descriptor_digest().unwrap()),
        1200,
        6000,
    );
    let head = verify_descriptor_chain(floor, [env1.clone()].into_iter(), 1500).expect("chain");
    assert_eq!(head.descriptor_epoch, 1);
    assert_eq!(head.descriptor_digest, env1.descriptor_digest().unwrap());
    assert_eq!(head.operator_verification_key, vkey(&k1));
}

#[test]
fn chain_empty_returns_floor_unchanged() {
    let g = sk(1);
    let (_e, floor) = genesis_floor(&g, d32(99));
    let head =
        verify_descriptor_chain(floor.clone(), std::iter::empty(), 1500).expect("empty chain");
    assert_eq!(head, floor);
}

#[test]
fn chain_rejects_bad_current_signature() {
    let g = sk(1);
    let (env0, floor) = genesis_floor(&g, d32(99));
    let mut env1 = descriptor(
        &g,
        d32(99),
        &sk(2),
        Some(&g),
        1,
        Some(env0.descriptor_digest().unwrap()),
        1200,
        6000,
    );
    env1.current_signature[0] ^= 0xff;
    assert_eq!(
        verify_descriptor_chain(floor, [env1].into_iter(), 1500),
        Err(DescriptorError::BadCurrentSignature)
    );
}

#[test]
fn chain_rejects_wrong_predecessor_digest() {
    let g = sk(1);
    let (_e, floor) = genesis_floor(&g, d32(99));
    let env1 = descriptor(&g, d32(99), &sk(2), Some(&g), 1, Some(d32(200)), 1200, 6000);
    assert_eq!(
        verify_descriptor_chain(floor, [env1].into_iter(), 1500),
        Err(DescriptorError::WrongPredecessorDigest)
    );
}

#[test]
fn chain_rejects_epoch_skip() {
    let g = sk(1);
    let (env0, floor) = genesis_floor(&g, d32(99));
    let env2 = descriptor(
        &g,
        d32(99),
        &sk(2),
        Some(&g),
        2,
        Some(env0.descriptor_digest().unwrap()),
        1200,
        6000,
    );
    assert_eq!(
        verify_descriptor_chain(floor, [env2].into_iter(), 1500),
        Err(DescriptorError::EpochNotIncrementing)
    );
}

#[test]
fn chain_rejects_expired_head() {
    let g = sk(1);
    let (env0, floor) = genesis_floor(&g, d32(99));
    let env1 = descriptor(
        &g,
        d32(99),
        &sk(2),
        Some(&g),
        1,
        Some(env0.descriptor_digest().unwrap()),
        1200,
        6000,
    );
    // now >= expires_at is expired head.
    assert_eq!(
        verify_descriptor_chain(floor, [env1].into_iter(), 6000),
        Err(DescriptorError::HeadExpired)
    );
}

#[test]
fn chain_rejects_bad_predecessor_signature() {
    let g = sk(1);
    let (env0, floor) = genesis_floor(&g, d32(99));
    let mut env1 = descriptor(
        &g,
        d32(99),
        &sk(2),
        Some(&g),
        1,
        Some(env0.descriptor_digest().unwrap()),
        1200,
        6000,
    );
    env1.predecessor_signature = Some([9u8; 64]);
    assert_eq!(
        verify_descriptor_chain(floor, [env1].into_iter(), 1500),
        Err(DescriptorError::BadPredecessorSignature)
    );
}

#[test]
fn chain_rejects_hop_cap() {
    let g = sk(1);
    let (env0, floor) = genesis_floor(&g, d32(99));
    let mut chain = Vec::new();
    let mut prev_digest = env0.descriptor_digest().unwrap();
    let mut prev_key = g.clone();
    for epoch in 1..=33u64 {
        let current = sk(100 + epoch as u8);
        let env = descriptor(
            &g,
            d32(99),
            &current,
            Some(&prev_key),
            epoch,
            Some(prev_digest),
            1200,
            6000,
        );
        prev_digest = env.descriptor_digest().unwrap();
        prev_key = current;
        chain.push(env);
    }
    assert_eq!(
        verify_descriptor_chain(floor, chain.into_iter(), 1500),
        Err(DescriptorError::HopsCapExceeded)
    );
}

// ===========================================================================
// Receipts + work challenge/stamp.
// ===========================================================================

fn hosting_receipt(op: &SigningKey) -> HostingReceiptV1 {
    let body = HostingReceiptBodyV1 {
        anchor_id: d32(1),
        operator_key_id: d32(2),
        descriptor_epoch: 4,
        descriptor_digest: d32(3),
        hosting_operation_id: d32(4),
        full_site_root: d32(5),
        manifest_digest: d32(6),
        manifest_version: 7,
        base_site_generation: 8,
        committed_site_generation: 9,
        ordered_namespace_results: vec![
            NamespaceResult {
                namespace_id: d32(10),
                snapshot_digest: d32(11),
                entry_count: 12,
            },
            NamespaceResult {
                namespace_id: d32(13),
                snapshot_digest: d32(14),
                entry_count: 15,
            },
            NamespaceResult {
                namespace_id: d32(16),
                snapshot_digest: d32(17),
                entry_count: 18,
            },
        ],
        status: HostingStatus::Committed,
        accepted_at: 1000,
        reported_retention_through: 9000,
        limit_profile_digest: d32(20),
    };
    sign_body(body, op)
}

fn listing_receipt(op: &SigningKey) -> ListingReceiptV1 {
    let body = ListingReceiptBodyV1 {
        anchor_id: d32(1),
        operator_key_id: d32(2),
        descriptor_epoch: 4,
        descriptor_digest: d32(3),
        listing_digest: d32(30),
        full_site_root: d32(5),
        accepted_listing_epoch: 2,
        accepted_listing_revision: 5,
        feed_coordinate: 42,
        accepted_at: 1000,
        expires_at: 9000,
        request_idempotency_key: d16(77),
    };
    sign_body(body, op)
}

#[test]
fn hosting_receipt_round_trips_and_verifies() {
    let op = sk(3);
    let receipt = hosting_receipt(&op);
    roundtrip(&receipt, 4 * 1024);
    assert!(receipt.verify(&pk(&op)).is_ok());
    assert!(receipt.verify(&pk(&sk(4))).is_err());
}

#[test]
fn listing_receipt_round_trips_and_verifies() {
    let op = sk(3);
    let receipt = listing_receipt(&op);
    roundtrip(&receipt, 4 * 1024);
    assert!(receipt.verify(&pk(&op)).is_ok());
    let mut tampered = receipt.clone();
    tampered.operator_signature[0] ^= 1;
    assert!(tampered.verify(&pk(&op)).is_err());
}

fn work_challenge(op: &SigningKey, difficulty: u64) -> WorkChallengeV1 {
    let body = WorkChallengeBodyV1 {
        anchor_id: d32(1),
        operator_key_id: d32(2),
        descriptor_epoch: 4,
        descriptor_digest: d32(3),
        operation_kind: ControlOperationKind::PrepareHost,
        idempotency_key: d16(9),
        work_target_digest: d32(60),
        community_root: d32(61),
        random_challenge: d32(62),
        policy_epoch: 7,
        difficulty,
        issued_at: 1000,
        expires_at: 1300,
    };
    sign_body(body, op)
}

fn make_stamp(challenge: &WorkChallengeV1, counter: u64) -> WorkStampV1 {
    let envelope_bytes = challenge.encode_canonical().unwrap();
    let challenge_digest = digest_v1(label::WORK_CHALLENGE_ENVELOPE, &envelope_bytes);
    let proof = work_proof(&challenge_digest, counter);
    WorkStampV1 {
        challenge_envelope_bytes: envelope_bytes,
        counter,
        proof_bytes: proof,
    }
}

#[test]
fn work_stamp_valid_difficulty_zero_verifies() {
    let op = sk(3);
    let challenge = work_challenge(&op, 0);
    let stamp = make_stamp(&challenge, 0);
    roundtrip(&stamp, 4 * 1024);
    let body = stamp.verify(&pk(&op)).expect("verify");
    assert_eq!(body.difficulty, 0);
}

#[test]
fn work_stamp_bad_challenge_signature_rejected() {
    let op = sk(3);
    let challenge = work_challenge(&op, 0);
    let stamp = make_stamp(&challenge, 0);
    assert_eq!(
        stamp.verify(&pk(&sk(4))),
        Err(WorkStampError::BadChallengeSignature)
    );
}

#[test]
fn work_stamp_bad_proof_rejected() {
    let op = sk(3);
    let challenge = work_challenge(&op, 0);
    let mut stamp = make_stamp(&challenge, 0);
    stamp.proof_bytes[0] ^= 0xff;
    assert_eq!(stamp.verify(&pk(&op)), Err(WorkStampError::BadProof));
}

#[test]
fn work_stamp_insufficient_work_rejected() {
    let op = sk(3);
    let challenge = work_challenge(&op, 24); // 24 leading zero bits required
    let stamp = make_stamp(&challenge, 0); // counter 0 almost surely fails
    assert_eq!(
        stamp.verify(&pk(&op)),
        Err(WorkStampError::InsufficientWork)
    );
}

// ===========================================================================
// Bootstrap + snapshot cursor.
// ===========================================================================

fn bootstrap_descriptor(operator: &SigningKey, epoch: u64) -> BootstrapDescriptorV1 {
    BootstrapDescriptorV1 {
        floor: DescriptorFloor {
            anchor_id: d32(epoch as u8),
            descriptor_epoch: epoch,
            descriptor_digest: d32(epoch as u8 + 100),
            operator_verification_key: vkey(operator),
        },
        https_origin: "https://a.example".to_string(),
        roles: vec![EnabledRole::Host, EnabledRole::Mirror],
    }
}

#[test]
fn bootstrap_round_trips_and_enforces_diversity() {
    let bs = AnchorBootstrapV1 {
        descriptors: vec![
            bootstrap_descriptor(&sk(1), 1),
            bootstrap_descriptor(&sk(2), 2),
            bootstrap_descriptor(&sk(1), 3),
        ],
    };
    roundtrip(&bs, 16 * 1024);
    assert!(bs.meets_diversity_floor());

    let too_few = AnchorBootstrapV1 {
        descriptors: vec![
            bootstrap_descriptor(&sk(1), 1),
            bootstrap_descriptor(&sk(1), 2),
        ],
    };
    assert!(!too_few.meets_diversity_floor());

    let one_operator = AnchorBootstrapV1 {
        descriptors: vec![
            bootstrap_descriptor(&sk(1), 1),
            bootstrap_descriptor(&sk(1), 2),
            bootstrap_descriptor(&sk(1), 3),
        ],
    };
    assert!(!one_operator.meets_diversity_floor());
}

#[test]
fn bootstrap_rejects_over_sixteen_descriptors() {
    let bs = AnchorBootstrapV1 {
        descriptors: (0..17).map(|i| bootstrap_descriptor(&sk(1), i)).collect(),
    };
    assert!(matches!(
        bs.encode_canonical(),
        Err(CodecError::LengthOutOfRange)
    ));
}

#[test]
fn snapshot_cursor_round_trips() {
    let cursor = SnapshotCursorV1 {
        body: SnapshotCursorBodyV1 {
            checkpoint_digest: d32(1),
            snapshot_generation_id: 7,
            next_ordinal: 3,
            previous_root: Some(d32(2)),
            issued_at: 1000,
            expires_at: 2000,
            cursor_secret_epoch: 4,
        },
        cursor_tag: d32(9),
    };
    roundtrip(&cursor, 4 * 1024);
    // The HMAC input embeds the u16be(33) label length and the canonical body.
    let input = cursor.body.cursor_tag_hmac_input().unwrap();
    assert_eq!(&input[0..2], &33u16.to_be_bytes());
    assert_eq!(&input[2..2 + 33], label::DIRECTORY_SNAPSHOT_CURSOR);
}

// ===========================================================================
// The closed refusal matrix: every row round-trips; cross-pairings fail decode.
// ===========================================================================

fn all_refusals() -> Vec<ControlRefusal> {
    vec![
        ControlRefusal::InvalidTicketAuthority,
        ControlRefusal::InvalidManifestAuthority,
        ControlRefusal::InvalidListingAuthority,
        ControlRefusal::InvalidOperationAuthority,
        ControlRefusal::UnsupportedVersion {
            supported_versions: vec![1, 2],
        },
        ControlRefusal::AdmissionOverQuota {
            limit_id: AnchorLimitId::HostedSites,
            effective_value: LimitValue::Scalar(10_000),
            observed_value: LimitValue::Scalar(10_001),
            retry_after_seconds: 5,
        },
        ControlRefusal::CommitOverQuota {
            limit_id: AnchorLimitId::StagedBytes,
            effective_value: LimitValue::Compound(1, 2),
            observed_value: LimitValue::Scalar(9),
            retry_after_seconds: 5,
        },
        ControlRefusal::UnsupportedTransport {
            required_mode: TransportMode::RequireArti,
            observed_mode: TransportMode::RequireNone,
        },
        ControlRefusal::ManifestTransportMismatch {
            expected_digest: d32(1),
            observed_digest: d32(2),
        },
        ControlRefusal::NotHosted,
        ControlRefusal::ListingManifestMismatch {
            expected_digest: d32(1),
            observed_digest: d32(2),
        },
        ControlRefusal::CommitManifestMismatch {
            expected_digest: d32(1),
            observed_digest: d32(2),
        },
        ControlRefusal::SnapshotMismatch {
            expected_snapshot_digest: d32(1),
            observed_snapshot_digest: d32(2),
        },
        ControlRefusal::TicketExpired {
            expires_at: 2000,
            observed_at: 2500,
        },
        ControlRefusal::ListingExpired {
            expires_at: 2000,
            observed_at: 2500,
        },
        ControlRefusal::WorkExpired {
            expires_at: 2000,
            observed_at: 2500,
        },
        ControlRefusal::ListingEquivocation {
            first_digest: d32(1),
            second_digest: d32(2),
        },
        ControlRefusal::ManifestEquivocation {
            first_digest: d32(1),
            second_digest: d32(2),
        },
        ControlRefusal::AnchorProfileOversize {
            observed_bytes: 100,
            maximum_bytes: 64,
        },
        ControlRefusal::SiteTooLarge {
            required_class: StorageClass::ProfileTotal,
            advertised_bytes: 100,
            local_limit_bytes: 64,
        },
        ControlRefusal::WorkRequired {
            policy_epoch: 7,
            difficulty: 20,
        },
        ControlRefusal::StaleBase {
            current_generation: 9,
            ordered_namespace_snapshot_digests: triple(1, 2, 3),
        },
        ControlRefusal::StaleSource {
            attested_generation: 9,
            observed_generation: 8,
            ordered_observed_namespace_snapshot_digests: triple(4, 5, 6),
        },
        ControlRefusal::AttestationConsumed {
            replica_source_attestation_digest: d32(1),
        },
        ControlRefusal::AlreadyUnlisted,
        ControlRefusal::RemovalReplayWindow {
            earliest_retry_at: 5000,
            retry_after_seconds: 60,
        },
        ControlRefusal::IdempotencyConflict,
        ControlRefusal::OperationNotFound {
            operation_id: d32(1),
        },
        ControlRefusal::OperationExpired {
            operation_id: d32(1),
            expires_at: 2000,
        },
        ControlRefusal::CheckpointUnavailable {
            checkpoint_digest: d32(1),
            reason: CheckpointReason::Unknown,
        },
        ControlRefusal::CursorInvalid {
            cursor_kind: CursorKind::Feed,
            reason: CursorReason::AfterHead,
            checkpoint_digest: Some(d32(1)),
            floor_sequence: Some(2),
            head_sequence: Some(9),
        },
        ControlRefusal::CursorInvalid {
            cursor_kind: CursorKind::Snapshot,
            reason: CursorReason::Malformed,
            checkpoint_digest: None,
            floor_sequence: None,
            head_sequence: None,
        },
        ControlRefusal::PeerContextChanged {
            side: PeerSide::Source,
            prior_descriptor_digest: d32(1),
            latest_descriptor_digest: Some(d32(2)),
            reason: PeerContextReason::ProcessRestart,
        },
        ControlRefusal::PeerContextChanged {
            side: PeerSide::Destination,
            prior_descriptor_digest: d32(1),
            latest_descriptor_digest: None,
            reason: PeerContextReason::TransportLoss,
        },
        ControlRefusal::AdmissionBusy {
            limit_id: AnchorLimitId::HostedSites,
            retry_after_seconds: 5,
        },
        ControlRefusal::RemovalBusy {
            limit_id: AnchorLimitId::HostedSites,
            retry_after_seconds: 5,
        },
        ControlRefusal::CommitBusy {
            limit_id: AnchorLimitId::HostedSites,
            retry_after_seconds: 5,
        },
        ControlRefusal::PeerAuthFailed {
            stage: PeerAuthStage::DescriptorExchange,
        },
    ]
}

#[test]
fn every_refusal_row_round_trips() {
    for refusal in all_refusals() {
        roundtrip(&refusal, 4096);
    }
}

#[test]
fn refusal_derived_tuple_matches_matrix() {
    let r = ControlRefusal::AdmissionOverQuota {
        limit_id: AnchorLimitId::HostedSites,
        effective_value: LimitValue::Scalar(1),
        observed_value: LimitValue::Scalar(2),
        retry_after_seconds: 5,
    };
    assert_eq!(r.subject(), RefusalSubject::Capacity);
    assert!(r.retryable());
    assert_eq!(r.retry_after_seconds(), Some(5));
    assert_eq!(r.retry_scope(), RetryScope::SameRequestAfterDelay);

    let never = ControlRefusal::InvalidTicketAuthority;
    assert_eq!(never.subject(), RefusalSubject::Ticket);
    assert!(!never.retryable());
    assert_eq!(never.retry_after_seconds(), None);
    assert_eq!(never.retry_scope(), RetryScope::Never);

    let commit = ControlRefusal::CommitBusy {
        limit_id: AnchorLimitId::StagedBytes,
        retry_after_seconds: 3,
    };
    assert_eq!(commit.retry_scope(), RetryScope::SameOperationNewCommitKey);
}

// Hand-craft hostile refusals with minicbor to prove cross-pairings fail decode.
use minicbor::Encoder as RawEncoder;

fn raw_refusal(build: impl FnOnce(&mut RawEncoder<&mut Vec<u8>>)) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut e = RawEncoder::new(&mut buf);
    build(&mut e);
    buf
}

#[test]
fn refusal_wrong_subject_pairing_fails_decode() {
    // invalid_ticket_authority must carry subject "ticket", not "manifest".
    let bytes = raw_refusal(|e| {
        e.array(5).unwrap();
        e.str("invalid_ticket_authority").unwrap();
        e.str("manifest").unwrap();
        e.bool(false).unwrap();
        e.null().unwrap();
        e.array(1).unwrap();
        e.str("none").unwrap();
    });
    assert!(decode_canonical::<ControlRefusal>(&bytes, 4096).is_err());
}

#[test]
fn refusal_unknown_code_fails_decode() {
    let bytes = raw_refusal(|e| {
        e.array(5).unwrap();
        e.str("totally_made_up").unwrap();
        e.str("operation").unwrap();
        e.bool(false).unwrap();
        e.null().unwrap();
        e.array(1).unwrap();
        e.str("none").unwrap();
    });
    assert!(matches!(
        decode_canonical::<ControlRefusal>(&bytes, 4096),
        Err(CodecError::UnknownVariant)
    ));
}

#[test]
fn refusal_wrong_retryable_fails_decode() {
    // invalid_ticket_authority is not retryable.
    let bytes = raw_refusal(|e| {
        e.array(5).unwrap();
        e.str("invalid_ticket_authority").unwrap();
        e.str("ticket").unwrap();
        e.bool(true).unwrap();
        e.null().unwrap();
        e.array(1).unwrap();
        e.str("none").unwrap();
    });
    assert!(matches!(
        decode_canonical::<ControlRefusal>(&bytes, 4096),
        Err(CodecError::NonCanonical)
    ));
}

#[test]
fn refusal_null_retry_where_required_fails_decode() {
    // admission_busy requires a nonzero retry_after; null must fail.
    let bytes = raw_refusal(|e| {
        e.array(5).unwrap();
        e.str("admission_busy").unwrap();
        e.str("capacity").unwrap();
        e.bool(true).unwrap();
        e.null().unwrap();
        e.array(2).unwrap();
        e.str("capacity").unwrap();
        e.u64(15).unwrap();
    });
    assert!(matches!(
        decode_canonical::<ControlRefusal>(&bytes, 4096),
        Err(CodecError::NonCanonical)
    ));
}

#[test]
fn refusal_zero_retry_where_required_fails_decode() {
    let bytes = raw_refusal(|e| {
        e.array(5).unwrap();
        e.str("admission_busy").unwrap();
        e.str("capacity").unwrap();
        e.bool(true).unwrap();
        e.u64(0).unwrap();
        e.array(2).unwrap();
        e.str("capacity").unwrap();
        e.u64(15).unwrap();
    });
    assert!(decode_canonical::<ControlRefusal>(&bytes, 4096).is_err());
}

// ===========================================================================
// Control operations: every request body + every response payload.
// ===========================================================================

fn all_operations() -> Vec<ControlOperation> {
    let root = sk(5);
    vec![
        ControlOperation::Describe(DescribeV1),
        ControlOperation::GetWorkChallenge(GetWorkChallengeV1 {
            intended_operation_kind: ControlOperationKind::PrepareHost,
            intended_idempotency_key: d16(1),
            community_root: d32(2),
            work_target_digest: d32(3),
        }),
        ControlOperation::PrepareHost(Box::new(PrepareHostV1 {
            root_signed_ticket_core: ticket_core(&root),
            ordered_namespace_snapshot_digests: triple(10, 11, 12),
            work_stamp: Some(make_stamp(&work_challenge(&sk(6), 0), 0)),
        })),
        ControlOperation::CommitHost(CommitHostV1 {
            operation_id: d32(4),
            ordered_namespace_snapshot_digests: triple(13, 14, 15),
        }),
        ControlOperation::SubmitListing(SubmitListingV1 {
            admitted_listing_envelope_bytes: vec![0xa1, 0x01, 0x02],
            work_stamp: None,
        }),
        ControlOperation::PrepareReplica(Box::new(PrepareReplicaV1 {
            replica_prepare_challenge: ReplicaPrepareChallengeV1 {
                destination_anchor_id: d32(20),
                random_256_bit_nonce: d32(21),
                prepare_idempotency_key: d16(22),
                full_site_root: d32(23),
                issued_at: 1000,
                expires_at: 1060,
            },
            replica_source_attestation: sign_body(
                ReplicaSourceAttestationBodyV1 {
                    source_anchor_id: d32(30),
                    source_current_operator_key_id: d32(31),
                    source_current_descriptor_epoch: 2,
                    source_current_descriptor_digest: d32(32),
                    destination_anchor_id: d32(20),
                    peer_transcript_digest: d32(33),
                    destination_prepare_nonce: d32(21),
                    prepare_idempotency_key: d16(22),
                    full_site_root: d32(23),
                    manifest_digest: d32(34),
                    manifest_version: 5,
                    root_signed_ticket_core_digest: d32(35),
                    source_site_generation: 7,
                    ordered_namespace_snapshot_digests: triple(36, 37, 38),
                    issued_at: 1000,
                    expires_at: 1300,
                },
                &sk(9),
            ),
            root_signed_ticket_core: ticket_core(&root),
            ordered_namespace_snapshot_digests: triple(10, 11, 12),
        })),
        ControlOperation::PullDirectoryFeed(PullDirectoryFeedV1 {
            after_sequence: 40,
            limit: 32,
        }),
        ControlOperation::PullDirectorySnapshot(PullDirectorySnapshotV1 {
            checkpoint_digest: d32(50),
            snapshot_cursor_bytes: Some(vec![0x01, 0x02]),
        }),
        ControlOperation::GetOperation(GetOperationV1 {
            operation_id: d32(60),
        }),
    ]
}

#[test]
fn every_control_request_round_trips() {
    for operation in all_operations() {
        let request = ControlRequestV1 {
            idempotency_key: d16(200),
            operation,
        };
        roundtrip(&request, 64 * 1024);
    }
}

#[test]
fn work_target_digest_ignores_work_stamp_but_request_digest_does_not() {
    let with_stamp = ControlOperation::PrepareHost(Box::new(PrepareHostV1 {
        root_signed_ticket_core: ticket_core(&sk(5)),
        ordered_namespace_snapshot_digests: triple(10, 11, 12),
        work_stamp: Some(make_stamp(&work_challenge(&sk(6), 0), 0)),
    }));
    let without_stamp = ControlOperation::PrepareHost(Box::new(PrepareHostV1 {
        root_signed_ticket_core: ticket_core(&sk(5)),
        ordered_namespace_snapshot_digests: triple(10, 11, 12),
        work_stamp: None,
    }));
    // work_target_digest sets the stamp slot to null in both cases.
    assert_eq!(
        with_stamp.work_target_digest().unwrap(),
        without_stamp.work_target_digest().unwrap()
    );
    // control_request_digest includes the actual stamp, so it differs.
    assert_ne!(
        with_stamp.control_request_digest().unwrap(),
        without_stamp.control_request_digest().unwrap()
    );
}

fn prepare_success() -> PrepareSuccessV1 {
    let profile = AnchorLimitProfileV1::mvp_defaults(0);
    PrepareSuccessV1 {
        operation_id: d32(1),
        base_site_generation: 5,
        ordered_namespace_host_plan: triple(10, 11, 12),
        ordered_namespace_tokens: triple(20, 21, 22),
        ordered_retained_snapshot_digests: triple(30, 31, 32),
        sync_version: 2,
        effective_operation_limits: EffectiveOperationLimits::from_profile(&profile),
        operation_expiry: 9000,
    }
}

fn response(kind: ControlOperationKind, success: ControlSuccess) -> ControlResponseV1 {
    ControlResponseV1 {
        kind,
        outcome: ControlOutcome::Success(success),
    }
}

#[test]
fn every_control_response_success_round_trips() {
    let g = sk(1);
    let descriptor_env = descriptor(&g, d32(99), &g, None, 0, None, 1000, 5000);
    let responses = vec![
        response(
            ControlOperationKind::Describe,
            ControlSuccess::Describe(Box::new(DescribeSuccessV1 {
                descriptor: descriptor_env,
                limit_profile: AnchorLimitProfileV1::mvp_defaults(0),
            })),
        ),
        response(
            ControlOperationKind::GetWorkChallenge,
            ControlSuccess::GetWorkChallenge(Box::new(work_challenge(&sk(6), 0))),
        ),
        response(
            ControlOperationKind::PrepareHost,
            ControlSuccess::PrepareHost(Box::new(prepare_success())),
        ),
        response(
            ControlOperationKind::CommitHost,
            ControlSuccess::CommitHost(Box::new(hosting_receipt(&sk(3)))),
        ),
        response(
            ControlOperationKind::SubmitListing,
            ControlSuccess::SubmitListing(Box::new(listing_receipt(&sk(3)))),
        ),
        response(
            ControlOperationKind::PrepareReplica,
            ControlSuccess::PrepareReplica(Box::new(prepare_success())),
        ),
        response(
            ControlOperationKind::PullDirectoryFeed,
            ControlSuccess::PullDirectoryFeed(FeedPullSuccessV1::Page {
                inclusions: vec![vec![0x01], vec![0x02, 0x03]],
                floor_sequence: 1,
                head_sequence: 9,
                head_digest: d32(7),
                done: true,
            }),
        ),
        response(
            ControlOperationKind::PullDirectorySnapshot,
            ControlSuccess::PullDirectorySnapshot(SnapshotPullSuccessV1 {
                checkpoint_bytes: vec![0xaa, 0xbb],
                snapshot_record_bytes: Some(vec![0xcc]),
                next_cursor_bytes: None,
                done: false,
            }),
        ),
    ];
    for r in responses {
        roundtrip(&r, 64 * 1024);
    }
}

#[test]
fn feed_checkpoint_required_round_trips() {
    let r = response(
        ControlOperationKind::PullDirectoryFeed,
        ControlSuccess::PullDirectoryFeed(FeedPullSuccessV1::CheckpointRequired {
            checkpoint_bytes: vec![0x01, 0x02],
            snapshot_cursor_bytes: vec![0x03],
        }),
    );
    roundtrip(&r, 64 * 1024);
}

#[test]
fn refused_response_round_trips_for_each_op() {
    for kind in [
        ControlOperationKind::CommitHost,
        ControlOperationKind::SubmitListing,
        ControlOperationKind::PrepareReplica,
    ] {
        let r = ControlResponseV1 {
            kind,
            outcome: ControlOutcome::Refused(ControlRefusal::IdempotencyConflict),
        };
        roundtrip(&r, 64 * 1024);
    }
}

#[test]
fn get_operation_prepared_and_terminal_round_trip() {
    let prepared = response(
        ControlOperationKind::GetOperation,
        ControlSuccess::GetOperation(Box::new(GetOperationSuccessV1 {
            operation_id: d32(1),
            originating_prepare_kind: PrepareKind::PrepareHost,
            state: GetOperationState::Prepared {
                operation_expiry: 9000,
                prepare_success: Box::new(prepare_success()),
            },
        })),
    );
    roundtrip(&prepared, 64 * 1024);

    let committed = response(
        ControlOperationKind::GetOperation,
        ControlSuccess::GetOperation(Box::new(GetOperationSuccessV1 {
            operation_id: d32(1),
            originating_prepare_kind: PrepareKind::PrepareReplica,
            state: GetOperationState::Terminal {
                outcome: TerminalOperationOutcome::Committed(Box::new(hosting_receipt(&sk(3)))),
            },
        })),
    );
    roundtrip(&committed, 64 * 1024);

    let refused = response(
        ControlOperationKind::GetOperation,
        ControlSuccess::GetOperation(Box::new(GetOperationSuccessV1 {
            operation_id: d32(1),
            originating_prepare_kind: PrepareKind::PrepareHost,
            state: GetOperationState::Terminal {
                outcome: TerminalOperationOutcome::Refused(ControlRefusal::OperationExpired {
                    operation_id: d32(1),
                    expires_at: 2000,
                }),
            },
        })),
    );
    roundtrip(&refused, 64 * 1024);
}

#[test]
fn effective_operation_limits_hold_all_82_ascending() {
    let profile = AnchorLimitProfileV1::mvp_defaults(0);
    let limits = EffectiveOperationLimits::from_profile(&profile);
    assert_eq!(limits.0.len(), 82);
    for (index, (id, _value)) in limits.0.iter().enumerate() {
        assert_eq!(id.id(), (index as u64) + 1);
    }
}
