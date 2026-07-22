//! Shared fixtures for the composite `CommitHost` tests: a real operator signer,
//! genuinely-authorised and forged anchor-profile items, prepared-operation setup,
//! staging helpers, and a configurable hosting authority.

#![allow(dead_code)]

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ed25519_dalek::{Signer, SigningKey};

use riot_anchor::control::{
    AdmissionPolicy, AnchorControlContext, AnchorControlService, ControlHandling, PreparePlan,
};
use riot_anchor::hosting::{
    CommitHostContext, HostPlanView, HostingAuthority, ManifestAuthorization,
};
use riot_anchor::repository::{
    AnchorRepository, NewPreparedOperation, OperationKind, RepoTransaction, StagedEntry,
};
use riot_anchor::sync_service::{encode_item, verify_anchor_item};
use riot_anchor::work::{OperatorSigner, PressurePolicy, TokenSecretRing};

use riot_anchor_protocol::authority::SITE_MANIFEST_DIGEST_LABEL;
use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::{
    ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1, ControlResponseV1,
    ControlSuccess, EffectiveOperationLimits, GetOperationState, GetOperationV1, PrepareHostV1,
    PrepareSuccessV1, TerminalOperationOutcome,
};
use riot_anchor_protocol::digest::{anchor_id as compute_anchor_id, digest_v1};
use riot_anchor_protocol::records::{
    AnchorDescriptorBodyV1, AnchorLimitProfileV1, ControlOperationKind, DescriptorEnvelopeV1,
    EnabledRole, HostingReceiptV1, OperatorVerificationKeyV1, PublicSiteTicketV2Core,
    RootSignedTicketCoreEnvelopeV2, TransportFloor,
};
use riot_anchor_protocol::sync2::compute_snapshot_digest;

use riot_core::site::{
    encode_site_manifest, RequireTransport, SiteDisplay, SiteLayout, SiteManifestV1, SiteMemberV1,
    SiteRole, SiteRule, TransportPolicyV1,
};
use riot_core::willow::{
    create_signed_alert, encode_capability, encode_entry, generate_communal_author, AlertDraft,
    Entry, Path, MANIFEST_COMPONENT,
};

use willow25::authorisation::WriteCapability;
use willow25::entry::{NamespaceSecret, SubspaceSecret};
use willow25::groupings::Area;

/// A file-backed temp database (used for restart/reconstruction tests).
pub struct TempDb {
    path: PathBuf,
}

impl TempDb {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!("riot-anchor-host-{}-{}.db", std::process::id(), id));
        let _ = std::fs::remove_file(&path);
        Self { path }
    }
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn open(&self) -> AnchorRepository {
        AnchorRepository::open(&self.path).expect("open anchor repository")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

/// A test operator signer producing real Ed25519 signatures over the receipt
/// preimage.
#[derive(Clone)]
pub struct TestSigner(pub SigningKey);
impl OperatorSigner for TestSigner {
    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        self.0.sign(preimage).to_bytes()
    }
}

pub fn signer() -> TestSigner {
    TestSigner(SigningKey::from_bytes(&[7u8; 32]))
}

pub fn d32(seed: u8) -> [u8; 32] {
    [seed; 32]
}
pub fn d16(seed: u8) -> [u8; 16] {
    [seed; 16]
}

/// An in-memory anchor repository.
pub fn repo() -> AnchorRepository {
    AnchorRepository::open_in_memory().expect("open in-memory anchor repository")
}

/// A genuinely-authorised anchor-profile item plus a forged twin whose signature
/// has been corrupted.
pub struct ItemFixture {
    pub namespace_id: [u8; 32],
    pub entry_id: [u8; 32],
    pub item_bytes: Vec<u8>,
    pub forged_item_bytes: Vec<u8>,
    /// The verified staged projection (genuine).
    pub staged: StagedEntry,
}

/// Build one genuinely-authorised item (a signed communal alert), and a forged
/// twin with a flipped signature byte.
pub fn make_item(headline: &str) -> ItemFixture {
    let author = generate_communal_author().expect("communal author");
    let draft = AlertDraft {
        valid_from: None,
        expires_at: u64::MAX - 1,
        language: "en".into(),
        urgency: riot_core::model::Urgency::Immediate,
        severity: riot_core::model::Severity::Severe,
        certainty: riot_core::model::Certainty::Observed,
        headline: headline.into(),
        description: "composite host test entry".into(),
        affected_area_claim: None,
        source_claims: vec!["src".into()],
        ai_assisted: false,
    };
    let alert = create_signed_alert(&author, draft).expect("signed alert");
    let signed = alert.signed;
    let item_bytes = encode_item(
        &signed.entry_bytes,
        &signed.capability_bytes,
        &signed.signature,
        &signed.payload_bytes,
    );
    let mut forged_signature = signed.signature;
    forged_signature[0] ^= 0x01;
    let forged_item_bytes = encode_item(
        &signed.entry_bytes,
        &signed.capability_bytes,
        &forged_signature,
        &signed.payload_bytes,
    );
    let staged = verify_anchor_item(&item_bytes).expect("genuine item verifies");
    ItemFixture {
        namespace_id: staged.namespace_id,
        entry_id: staged.entry_id,
        item_bytes,
        forged_item_bytes,
        staged,
    }
}

/// A staged entry carrying a forged item under otherwise-genuine metadata. Used to
/// prove the composite Commit re-verifies staged bytes and never trusts the stage.
pub fn forged_staged(item: &ItemFixture) -> StagedEntry {
    let mut forged = item.staged.clone();
    forged.item_bytes = item.forged_item_bytes.clone();
    forged
}

/// Insert a prepared host operation whose stored prepared response captures the
/// ordered O/C/W namespaces and base generation.
#[allow(clippy::too_many_arguments)]
pub fn insert_prepared_operation(
    repo: &mut AnchorRepository,
    operation_id: [u8; 32],
    ordered_namespaces: [[u8; 32]; 3],
    ordered_tokens: [[u8; 32]; 3],
    base_generation: u64,
    now: u64,
    operation_expiry: u64,
    token_secret_epoch: u32,
) {
    insert_prepared_operation_with_ticket(
        repo,
        operation_id,
        ordered_namespaces,
        ordered_tokens,
        base_generation,
        now,
        operation_expiry,
        token_secret_epoch,
        None,
    );
}

/// [`insert_prepared_operation`] plus the root-signed ticket envelope bytes a
/// real PrepareHost persists on the operation row (the composite Commit's ONLY
/// ticket source). `None` models a pre-migration row.
#[allow(clippy::too_many_arguments)]
pub fn insert_prepared_operation_with_ticket(
    repo: &mut AnchorRepository,
    operation_id: [u8; 32],
    ordered_namespaces: [[u8; 32]; 3],
    ordered_tokens: [[u8; 32]; 3],
    base_generation: u64,
    now: u64,
    operation_expiry: u64,
    token_secret_epoch: u32,
    ticket_envelope_bytes: Option<Vec<u8>>,
) {
    let profile = AnchorLimitProfileV1::mvp_defaults(0);
    let success = PrepareSuccessV1 {
        operation_id,
        base_site_generation: base_generation,
        ordered_namespace_host_plan: ordered_namespaces,
        ordered_namespace_tokens: ordered_tokens,
        ordered_retained_snapshot_digests: [d32(90), d32(91), d32(92)],
        sync_version: 2,
        effective_operation_limits: EffectiveOperationLimits::from_profile(&profile),
        operation_expiry,
    };
    let response = ControlResponseV1 {
        kind: ControlOperationKind::PrepareHost,
        outcome: ControlOutcome::Success(ControlSuccess::PrepareHost(Box::new(success))),
    };
    let prepare_response_bytes = response
        .encode_canonical()
        .expect("encode prepare response");
    let mut tx = repo.begin().expect("begin");
    tx.insert_operation(&NewPreparedOperation {
        operation_id,
        originating_kind: OperationKind::Host,
        token_secret_epoch,
        base_generation,
        created_at: now,
        operation_expiry,
        retention_deadline: operation_expiry + 24 * 60 * 60,
        prepare_response_bytes,
    })
    .expect("insert operation");
    if let Some(ticket) = ticket_envelope_bytes {
        tx.store_operation_ticket(&operation_id, &ticket)
            .expect("store operation ticket");
    }
    tx.commit().expect("commit operation");
}

/// Stage entries (already verified or deliberately forged) into an operation's
/// private staging in one short transaction.
pub fn stage_entries(
    repo: &mut AnchorRepository,
    operation_id: [u8; 32],
    entries: Vec<StagedEntry>,
    stage_deadline: u64,
) {
    let mut tx = repo.begin().expect("begin");
    tx.ensure_staged_operation(&operation_id, b"host", 1000, stage_deadline)
        .expect("ensure staged operation");
    for entry in &entries {
        tx.stage_entry(&operation_id, entry).expect("stage entry");
    }
    tx.commit().expect("commit staging");
}

/// Compute the declared O/C/W snapshot digests a client would send for a Commit,
/// from the committed base plus the staged entries.
pub fn declared_digests(
    repo: &AnchorRepository,
    operation_id: [u8; 32],
    ordered_namespaces: [[u8; 32]; 3],
) -> [[u8; 32]; 3] {
    let mut digests = [[0u8; 32]; 3];
    for (index, namespace_id) in ordered_namespaces.iter().enumerate() {
        let committed = repo.committed_entries(namespace_id).unwrap_or_default();
        let staged = repo
            .staged_entries(&operation_id, namespace_id)
            .unwrap_or_default();
        let mut ids: Vec<Vec<u8>> = committed.iter().map(|(id, _)| id.clone()).collect();
        let mut logical: u64 = committed.iter().map(|(_, item)| item.len() as u64).sum();
        for entry in &staged {
            ids.push(entry.entry_id.to_vec());
            logical += entry.item_bytes.len() as u64;
        }
        digests[index] = compute_snapshot_digest(namespace_id, logical, &ids);
    }
    digests
}

pub fn commit_context() -> CommitHostContext {
    CommitHostContext {
        anchor_id: d32(1),
        operator_key_id: d32(2),
        descriptor_epoch: 5,
        descriptor_digest: d32(3),
        limit_profile_digest: d32(4),
        reported_retention_secs: 30 * 24 * 60 * 60,
    }
}

/// A configurable Commit-time authority. It resolves a manifest that authorises an
/// exact O/C/W routing (defaulting to the operation's own plan) and can be told to
/// refuse at capacity or manifest resolution.
pub struct TestAuthority {
    pub ordered_namespaces: [[u8; 32]; 3],
    pub manifest_digest: [u8; 32],
    pub manifest_version: u64,
    pub capacity_refusal: RefCell<Option<riot_anchor_protocol::control::ControlRefusal>>,
    pub manifest_refusal: RefCell<Option<riot_anchor_protocol::control::ControlRefusal>>,
    pub routing_override: RefCell<Option<[[u8; 32]; 3]>>,
}

impl TestAuthority {
    pub fn new(ordered_namespaces: [[u8; 32]; 3]) -> Self {
        TestAuthority {
            ordered_namespaces,
            manifest_digest: d32(55),
            manifest_version: 3,
            capacity_refusal: RefCell::new(None),
            manifest_refusal: RefCell::new(None),
            routing_override: RefCell::new(None),
        }
    }
    pub fn refuse_capacity(self, refusal: riot_anchor_protocol::control::ControlRefusal) -> Self {
        *self.capacity_refusal.borrow_mut() = Some(refusal);
        self
    }
    pub fn refuse_manifest(self, refusal: riot_anchor_protocol::control::ControlRefusal) -> Self {
        *self.manifest_refusal.borrow_mut() = Some(refusal);
        self
    }
    pub fn override_routing(self, routing: [[u8; 32]; 3]) -> Self {
        *self.routing_override.borrow_mut() = Some(routing);
        self
    }
}

// ---------------------------------------------------------------------------
// A REAL owned-root composite site: owner-signed `/manifest`, communal C/W
// members, and a matching root-signed ticket. The owned namespace id IS the
// raw Ed25519 verifying key, so the SAME retained secret that authorises the
// manifest entry also signs the ticket.
// ---------------------------------------------------------------------------

/// An owned-namespace root whose Ed25519 secret we retain.
pub struct OwnedSiteRoot {
    pub namespace_secret: NamespaceSecret,
    pub root_signing_key: SigningKey,
    pub root_id: [u8; 32],
}

/// Rejection-sample an owned namespace deterministically from `seed`.
pub fn owned_site_root(seed: u8) -> OwnedSiteRoot {
    for n in 0u16..=1024 {
        let mut secret_bytes = [seed; 32];
        secret_bytes[0] = (n & 0xff) as u8;
        secret_bytes[1] = (n >> 8) as u8;
        let namespace_secret = NamespaceSecret::from_bytes(&secret_bytes);
        let namespace_id = namespace_secret.corresponding_namespace_id();
        if namespace_id.is_owned() {
            let root_signing_key = SigningKey::from_bytes(&secret_bytes);
            assert_eq!(
                root_signing_key.verifying_key().to_bytes(),
                *namespace_id.as_bytes(),
                "owned namespace id must equal the ed25519 verifying key"
            );
            return OwnedSiteRoot {
                namespace_secret,
                root_signing_key,
                root_id: *namespace_id.as_bytes(),
            };
        }
    }
    panic!("no owned namespace found for seed {seed}");
}

/// A complete site fixture: the owned root, the canonical manifest payload, the
/// owner-signed `/manifest` staged entry, one communal entry each for C and W,
/// and the matching root-signed ticket envelope.
pub struct SiteFixture {
    pub root: OwnedSiteRoot,
    pub root_id: [u8; 32],
    /// Ordered `[O, C, W]` namespace ids (O == root_id).
    pub namespaces: [[u8; 32]; 3],
    pub manifest_version: u64,
    pub manifest_digest: [u8; 32],
    /// The canonical manifest CBOR (the `/manifest` entry payload).
    pub manifest_payload_bytes: Vec<u8>,
    /// The owner-signed `/manifest` item, verified into staged form.
    pub manifest_staged: StagedEntry,
    /// One genuine communal entry in the C namespace.
    pub c_staged: StagedEntry,
    /// One genuine communal entry in the W namespace.
    pub w_staged: StagedEntry,
    /// The matching root-signed ticket envelope, canonically encoded.
    pub ticket_envelope_bytes: Vec<u8>,
}

/// The manifest CBOR for a site rooted at `root` with communal C/W members.
fn site_manifest_bytes(
    root: [u8; 32],
    c_namespace: [u8; 32],
    w_namespace: [u8; 32],
    version: u64,
) -> Vec<u8> {
    let manifest = SiteManifestV1 {
        root,
        members: vec![
            SiteMemberV1 {
                ns: root,
                role: SiteRole::Masthead,
                rule: SiteRule::OwnedWrite,
                display: SiteDisplay::FrontArticles,
            },
            SiteMemberV1 {
                ns: c_namespace,
                role: SiteRole::Comments,
                rule: SiteRule::CommunalOpen,
                display: SiteDisplay::UnderArticles,
            },
            SiteMemberV1 {
                ns: w_namespace,
                role: SiteRole::OpenWire,
                rule: SiteRule::CommunalOpen,
                display: SiteDisplay::WireColumn,
            },
        ],
        moderation_path: vec![b"mod".to_vec()],
        transport_policy: TransportPolicyV1 {
            allow: vec![],
            require: RequireTransport::None,
        },
        version,
        layout: SiteLayout::SiteDefault,
        sections: vec![],
    };
    encode_site_manifest(&manifest).expect("encode site manifest")
}

/// Sign a matching `RootSignedTicketCoreEnvelopeV2` with the owned root's key.
fn sign_site_ticket(
    root: &OwnedSiteRoot,
    namespaces: [[u8; 32]; 3],
    manifest_digest: [u8; 32],
    manifest_version: u64,
    issued: u64,
    expiry: u64,
) -> Vec<u8> {
    let core = PublicSiteTicketV2Core {
        root_id: root.root_id,
        o_namespace_id: namespaces[0],
        c_namespace_id: namespaces[1],
        w_namespace_id: namespaces[2],
        manifest_digest,
        manifest_version,
        min_sync_version: 2,
        manifest_required_transport: TransportFloor::RequireNone,
        transport_floor: TransportFloor::RequireNone,
        transport_epoch: 1,
        issued_unix_seconds: issued,
        expiry_unix_seconds: expiry,
    };
    let mut envelope = RootSignedTicketCoreEnvelopeV2 {
        core,
        root_signature: [0u8; 64],
    };
    let preimage = envelope.signing_preimage().expect("ticket preimage");
    envelope.root_signature = root.root_signing_key.sign(&preimage).to_bytes();
    envelope.encode_canonical().expect("encode ticket envelope")
}

/// Build the full owned-root site fixture: manifest v`manifest_version`, ticket
/// valid over `[issued, expiry)`.
pub fn make_site_fixture(
    seed: u8,
    manifest_version: u64,
    ticket_issued: u64,
    ticket_expiry: u64,
) -> SiteFixture {
    let root = owned_site_root(seed);
    let c_item = make_item("site-c-entry");
    let w_item = make_item("site-w-entry");
    let namespaces = [root.root_id, c_item.namespace_id, w_item.namespace_id];

    let payload = site_manifest_bytes(
        root.root_id,
        c_item.namespace_id,
        w_item.namespace_id,
        manifest_version,
    );
    let manifest_digest = digest_v1(SITE_MANIFEST_DIGEST_LABEL, &payload);

    // Owner-signed `/manifest`: a zero-delegation owned cap minted straight from
    // the namespace root secret, entry authorised by the owner subspace.
    let owner = SubspaceSecret::from_bytes(&[seed ^ 0x5A; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let cap = WriteCapability::new_owned(&root.namespace_secret, owner_id.clone());
    let entry = Entry::builder()
        .namespace_id(root.namespace_secret.corresponding_namespace_id())
        .subspace_id(owner_id)
        .path(Path::from_slices(&[MANIFEST_COMPONENT]).expect("manifest path"))
        .timestamp(1_000u64)
        .payload(&payload)
        .build();
    let authorised = entry
        .into_authorised_entry(&cap, &owner)
        .expect("owner authorises /manifest");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let item_bytes = encode_item(
        &encode_entry(authorised.entry()),
        &encode_capability(token.capability()),
        &signature.to_bytes(),
        &payload,
    );
    let manifest_staged = verify_anchor_item(&item_bytes).expect("manifest item verifies");

    let ticket_envelope_bytes = sign_site_ticket(
        &root,
        namespaces,
        manifest_digest,
        manifest_version,
        ticket_issued,
        ticket_expiry,
    );

    SiteFixture {
        root_id: root.root_id,
        root,
        namespaces,
        manifest_version,
        manifest_digest,
        manifest_payload_bytes: payload,
        manifest_staged,
        c_staged: c_item.staged,
        w_staged: w_item.staged,
        ticket_envelope_bytes,
    }
}

/// TRAP 1 fodder: a `/manifest` entry authorised by a DELEGATED owned cap whose
/// full area covers `/manifest`. It passes ordinary admission and genuinely
/// verifies, but must never be accepted as the manifest signer.
pub fn make_delegated_manifest_item(site: &SiteFixture) -> StagedEntry {
    let owner = SubspaceSecret::from_bytes(&[0x21; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let editor = SubspaceSecret::from_bytes(&[0x22; 32]);
    let editor_id = editor.corresponding_subspace_id();

    let mut delegated = WriteCapability::new_owned(&site.root.namespace_secret, owner_id);
    delegated
        .try_delegate(&owner, Area::full(), editor_id.clone())
        .expect("delegate full area");

    let entry = Entry::builder()
        .namespace_id(site.root.namespace_secret.corresponding_namespace_id())
        .subspace_id(editor_id)
        .path(Path::from_slices(&[MANIFEST_COMPONENT]).expect("manifest path"))
        .timestamp(1_100u64)
        .payload(&site.manifest_payload_bytes)
        .build();
    let authorised = entry
        .into_authorised_entry(&delegated, &editor)
        .expect("editor authorises with full-area delegated cap");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let item_bytes = encode_item(
        &encode_entry(authorised.entry()),
        &encode_capability(token.capability()),
        &signature.to_bytes(),
        &site.manifest_payload_bytes,
    );
    verify_anchor_item(&item_bytes).expect("delegated manifest item passes ordinary admission")
}

/// TRAP 2 fodder: the genuine owner-signed `/manifest` ENTRY, but the carried
/// item payload swapped for a DIFFERENT (higher-version) manifest encoding the
/// ticket points at. `validate_site_manifest` alone would accept the swapped
/// payload; only the payload↔entry digest binding refuses it.
pub struct PayloadSwappedManifest {
    /// The staged entry whose item carries the swapped payload.
    pub staged: StagedEntry,
    /// A root-signed ticket naming the SWAPPED payload's digest/version.
    pub ticket_envelope_bytes: Vec<u8>,
}

pub fn make_payload_swapped_manifest_item(site: &SiteFixture) -> PayloadSwappedManifest {
    // A different-but-valid manifest for the same site (version bumped), never
    // signed by the owner as an entry.
    let swapped_payload = site_manifest_bytes(
        site.root_id,
        site.namespaces[1],
        site.namespaces[2],
        site.manifest_version + 1,
    );
    let swapped_digest = digest_v1(SITE_MANIFEST_DIGEST_LABEL, &swapped_payload);

    // Reuse the genuine item's entry + capability + signature, swap the payload.
    let genuine = &site.manifest_staged;
    let mut staged = genuine.clone();
    // Rebuild the item bytes with the swapped payload; entry bytes untouched.
    // Item layout: version(1) | u32 entry_len | entry | u32 cap_len | cap |
    // 64-byte signature | u32 payload_len | payload.
    let item = &genuine.item_bytes;
    let entry_len = u32::from_be_bytes([item[1], item[2], item[3], item[4]]) as usize;
    let cap_len_at = 1 + 4 + entry_len;
    let cap_len = u32::from_be_bytes([
        item[cap_len_at],
        item[cap_len_at + 1],
        item[cap_len_at + 2],
        item[cap_len_at + 3],
    ]) as usize;
    let sig_at = cap_len_at + 4 + cap_len;
    let entry_bytes = &item[5..5 + entry_len];
    let capability_bytes = &item[cap_len_at + 4..cap_len_at + 4 + cap_len];
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&item[sig_at..sig_at + 64]);
    staged.item_bytes = encode_item(entry_bytes, capability_bytes, &signature, &swapped_payload);

    let ticket_envelope_bytes = sign_site_ticket(
        &site.root,
        site.namespaces,
        swapped_digest,
        site.manifest_version + 1,
        1_000,
        1_000 + 24 * 60 * 60,
    );
    PayloadSwappedManifest {
        staged,
        ticket_envelope_bytes,
    }
}

// ---------------------------------------------------------------------------
// A minimal control service, used only to reconstruct a committed operation's
// receipt through the real `GetOperation` lifecycle after a restart.
// ---------------------------------------------------------------------------

pub struct DummyPolicy;
impl AdmissionPolicy for DummyPolicy {
    fn authorize_prepare_host(
        &self,
        _request: &PrepareHostV1,
        _observed_at: u64,
        _highest_transport_epoch: Option<u32>,
    ) -> Result<PreparePlan, ControlRefusal> {
        Ok(PreparePlan {
            community_root: d32(0),
            ordered_namespace_host_plan: [d32(0); 3],
            ordered_retained_snapshot_digests: [d32(0); 3],
            base_generation: 0,
        })
    }
    fn capacity_for_prepare_host(
        &self,
        _plan: &PreparePlan,
        _observed_at: u64,
    ) -> Result<(), ControlRefusal> {
        Ok(())
    }
    fn pressure_band(&self, _community_root: &[u8; 32], _observed_at: u64) -> PressurePolicy {
        PressurePolicy {
            policy_epoch: 0,
            difficulty: 0,
        }
    }
}

fn descriptor(operator: &SigningKey) -> DescriptorEnvelopeV1 {
    let genesis_random = d32(99);
    let anchor = compute_anchor_id(&operator.verifying_key().to_bytes(), &genesis_random);
    let current_key = OperatorVerificationKeyV1 {
        public_key: operator.verifying_key().to_bytes(),
    };
    let body = AnchorDescriptorBodyV1 {
        anchor_id: anchor,
        genesis_operator_public_key: operator.verifying_key().to_bytes(),
        genesis_random_256_bits: genesis_random,
        current_operator_verification_key: current_key,
        current_operator_key_id: current_key.operator_key_id().unwrap(),
        descriptor_epoch: 0,
        previous_descriptor_digest: None,
        current_iroh_endpoint_id: d32(40),
        https_origin: "https://anchor.example".to_string(),
        operator_display_label: "Example Anchor".to_string(),
        self_reported_failure_domain_label: "eu-west".to_string(),
        supported_control_versions: vec![1],
        supported_sync_versions: vec![1, 2],
        enabled_roles: vec![EnabledRole::Host, EnabledRole::Mirror],
        limit_profile_digest: d32(50),
        predecessor_operator_verification_key: None,
        issued_at: 1000,
        expires_at: 5000,
    };
    let mut env = DescriptorEnvelopeV1 {
        body,
        current_signature: [0u8; 64],
        predecessor_signature: None,
    };
    let preimage = env.current_signing_preimage().unwrap();
    env.current_signature = operator.sign(&preimage).to_bytes();
    env
}

/// A control service whose only exercised path here is `GetOperation`.
pub fn control_service() -> AnchorControlService<DummyPolicy, TestSigner> {
    let operator = SigningKey::from_bytes(&[7u8; 32]);
    let desc = descriptor(&operator);
    let current_key = OperatorVerificationKeyV1 {
        public_key: operator.verifying_key().to_bytes(),
    };
    let context = AnchorControlContext {
        anchor_id: desc.body.anchor_id,
        operator_key_id: current_key.operator_key_id().unwrap(),
        operator_public_key: operator.verifying_key().to_bytes(),
        descriptor_epoch: 0,
        descriptor_digest: desc.descriptor_digest().unwrap(),
        descriptor: desc,
        limit_profile: AnchorLimitProfileV1::mvp_defaults(0),
        sync_version: 2,
        operation_lifetime_secs: 3600,
    };
    AnchorControlService::new(
        context,
        DummyPolicy,
        TestSigner(operator),
        TokenSecretRing::new(0, d32(200)),
    )
}

/// Reconstruct a committed operation's hosting receipt through `GetOperation`.
pub fn get_operation_receipt(
    service: &AnchorControlService<DummyPolicy, TestSigner>,
    repo: &mut AnchorRepository,
    operation_id: [u8; 32],
    now: u64,
) -> HostingReceiptV1 {
    let request = ControlRequestV1 {
        idempotency_key: [0u8; 16],
        operation: ControlOperation::GetOperation(GetOperationV1 { operation_id }),
    };
    let bytes = request.encode_canonical().expect("encode get_operation");
    let mut entropy = || d32(0);
    let handling = service
        .handle(repo, &bytes, now, &mut entropy)
        .expect("handle get_operation");
    let response = match handling {
        ControlHandling::Responded(response) => response,
        other => panic!("expected response, got {other:?}"),
    };
    match response.outcome {
        ControlOutcome::Success(ControlSuccess::GetOperation(success)) => match success.state {
            GetOperationState::Terminal {
                outcome: TerminalOperationOutcome::Committed(receipt),
            } => *receipt,
            other => panic!("expected committed terminal, got {other:?}"),
        },
        other => panic!("expected get_operation success, got {other:?}"),
    }
}

impl HostingAuthority for TestAuthority {
    fn commit_capacity(
        &self,
        _community_root: &[u8; 32],
        _observed_at: u64,
    ) -> Result<(), riot_anchor_protocol::control::ControlRefusal> {
        match self.capacity_refusal.borrow().clone() {
            Some(refusal) => Err(refusal),
            None => Ok(()),
        }
    }
    fn resolve_manifest(
        &self,
        _tx: &RepoTransaction<'_>,
        plan: &HostPlanView,
        _observed_at: u64,
    ) -> Result<ManifestAuthorization, riot_anchor_protocol::control::ControlRefusal> {
        if let Some(refusal) = self.manifest_refusal.borrow().clone() {
            return Err(refusal);
        }
        let ordered = self
            .routing_override
            .borrow()
            .unwrap_or(self.ordered_namespaces);
        Ok(ManifestAuthorization {
            community_id: plan.community_root,
            full_site_root: plan.community_root,
            manifest_digest: self.manifest_digest,
            manifest_version: self.manifest_version,
            ordered_namespaces: ordered,
            manifest_bytes: self.manifest_digest.to_vec(),
        })
    }
}
