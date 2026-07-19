//! Shared fixtures for the ordinary `SubmitListing` tests: a real operator signer,
//! genuinely root-signed listing entries over owned namespaces (raw `willow25` so
//! the SAME owned-root key that authorises the entry also signs the embedded
//! ticket), a delegated variant, forged twins, and a configurable hosting authority.

#![allow(dead_code)]

use std::cell::RefCell;

use ed25519_dalek::{Signer as _, SigningKey};

use riot_anchor::listing::{
    HostingState, ListingAuthority, ListingContext, RawDelegateGrant, RawListingSubmission,
    SubmitListingService, VerifiedListingCoordinates,
};
use riot_anchor::repository::AnchorRepository;
use riot_anchor::sync_service::encode_item;
use riot_anchor::work::OperatorSigner;

use riot_anchor_protocol::codec::CanonicalRecord;
use riot_anchor_protocol::control::ControlRefusal;
use riot_anchor_protocol::records::{
    CommunityListingV1, ListingDelegateGrantV1, PublicSiteTicketV2Core,
    RootSignedTicketCoreEnvelopeV2, TransportFloor, LISTING_DELEGATE_GRANT_SIGNING_DOMAIN,
};

use riot_core::willow::{
    encode_capability, encode_entry, Entry, Path, DIRECTORY_COMPONENT, LISTING_COMPONENT,
};

use willow25::authorisation::WriteCapability;
use willow25::entry::{NamespaceSecret, SubspaceSecret};
use willow25::groupings::{Area, TimeRange};

/// The test operator signer (real Ed25519 over the receipt/inclusion preimage).
pub struct TestSigner(pub SigningKey);
impl OperatorSigner for TestSigner {
    fn sign(&self, preimage: &[u8]) -> [u8; 64] {
        self.0.sign(preimage).to_bytes()
    }
}

pub fn operator_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

pub fn signer() -> TestSigner {
    TestSigner(operator_key())
}

pub fn d32(seed: u8) -> [u8; 32] {
    [seed; 32]
}
pub fn d16(seed: u8) -> [u8; 16] {
    [seed; 16]
}

pub fn repo() -> AnchorRepository {
    AnchorRepository::open_in_memory().expect("open in-memory anchor repository")
}

pub fn listing_context() -> ListingContext {
    ListingContext {
        anchor_id: d32(1),
        operator_key_id: d32(2),
        descriptor_epoch: 5,
        descriptor_digest: d32(3),
    }
}

pub fn service() -> SubmitListingService<TestListingAuthority, TestSigner> {
    SubmitListingService::new(listing_context(), TestListingAuthority::hosted(), signer())
}

pub fn service_with(
    authority: TestListingAuthority,
) -> SubmitListingService<TestListingAuthority, TestSigner> {
    SubmitListingService::new(listing_context(), authority, signer())
}

/// An owned-namespace root whose Ed25519 secret we retain, so the SAME key that
/// authorises listing entries in namespace `O` also signs the embedded ticket.
pub struct OwnedRoot {
    pub namespace_secret: NamespaceSecret,
    pub root_signing_key: SigningKey,
    pub root_id: [u8; 32],
}

/// Rejection-sample an owned namespace deterministically from `seed`.
pub fn owned_root(seed: u8) -> OwnedRoot {
    for n in 0u16..=1024 {
        let mut secret_bytes = [seed; 32];
        secret_bytes[0] = (n & 0xff) as u8;
        secret_bytes[1] = (n >> 8) as u8;
        let namespace_secret = NamespaceSecret::from_bytes(&secret_bytes);
        let namespace_id = namespace_secret.corresponding_namespace_id();
        if namespace_id.is_owned() {
            let root_signing_key = SigningKey::from_bytes(&secret_bytes);
            // The willow25 namespace id IS the raw Ed25519 verifying key.
            assert_eq!(
                root_signing_key.verifying_key().to_bytes(),
                *namespace_id.as_bytes(),
                "owned namespace id must equal the ed25519 verifying key"
            );
            return OwnedRoot {
                namespace_secret,
                root_signing_key,
                root_id: *namespace_id.as_bytes(),
            };
        }
    }
    panic!("no owned namespace found for seed {seed}");
}

/// The coordinates shared by a genuine listing, its ticket, and the authority stub.
#[derive(Clone, Copy)]
pub struct Coords {
    pub root_id: [u8; 32],
    pub o: [u8; 32],
    pub c: [u8; 32],
    pub w: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub manifest_version: u64,
}

impl Coords {
    pub fn verified(&self) -> VerifiedListingCoordinates {
        VerifiedListingCoordinates {
            root_id: self.root_id,
            o_namespace_id: self.o,
            c_namespace_id: self.c,
            w_namespace_id: self.w,
            manifest_digest: self.manifest_digest,
            manifest_version: self.manifest_version,
        }
    }
}

/// Sign a `RootSignedTicketCoreEnvelopeV2` with the owned root's Ed25519 key.
pub fn root_signed_ticket(root: &OwnedRoot, coords: &Coords, issued: u64, expiry: u64) -> Vec<u8> {
    let core = PublicSiteTicketV2Core {
        root_id: coords.root_id,
        o_namespace_id: coords.o,
        c_namespace_id: coords.c,
        w_namespace_id: coords.w,
        manifest_digest: coords.manifest_digest,
        manifest_version: coords.manifest_version,
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

/// Build a `CommunityListingV1` payload for the given coordinates/ticket.
#[allow(clippy::too_many_arguments)]
pub fn listing_payload(
    coords: &Coords,
    ticket_core_bytes: Vec<u8>,
    listing_epoch: u32,
    listing_revision: u32,
    listed: bool,
    title: &str,
    issued: u64,
    expiry: u64,
) -> CommunityListingV1 {
    CommunityListingV1 {
        root_id: coords.root_id,
        o_namespace_id: coords.o,
        c_namespace_id: coords.c,
        w_namespace_id: coords.w,
        manifest_digest: coords.manifest_digest,
        manifest_version: coords.manifest_version,
        ticket_core_bytes,
        listing_epoch,
        listing_revision,
        listed,
        title: title.into(),
        summary: "a community".into(),
        topic_tags: vec![],
        languages: vec![],
        region: None,
        issued_unix_seconds: issued,
        expiry_unix_seconds: expiry,
    }
}

/// Encode a signed root-owned listing item (owner subspace signs; zero-delegation
/// owned cap minted straight from the namespace root secret).
pub fn root_owned_item(root: &OwnedRoot, owner: &SubspaceSecret, payload_bytes: &[u8]) -> Vec<u8> {
    let owner_id = owner.corresponding_subspace_id();
    let cap = WriteCapability::new_owned(&root.namespace_secret, owner_id.clone());
    let path = Path::from_slices(&[DIRECTORY_COMPONENT, LISTING_COMPONENT]).expect("listing path");
    let entry = Entry::builder()
        .namespace_id(root.namespace_secret.corresponding_namespace_id())
        .subspace_id(owner_id)
        .path(path)
        .timestamp(1_000u64)
        .payload(payload_bytes)
        .build();
    let authorised = entry
        .into_authorised_entry(&cap, owner)
        .expect("owner authorises O:/directory/listing");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    encode_item(
        &encode_entry(authorised.entry()),
        &encode_capability(token.capability()),
        &signature.to_bytes(),
        payload_bytes,
    )
}

/// A genuine root-owned listing submission plus its coordinates.
pub struct GenuineListing {
    pub submission: RawListingSubmission,
    pub coords: Coords,
    pub community_id: [u8; 32],
    pub listing_epoch: u32,
    pub listing_revision: u32,
    pub expiry: u64,
    pub root: OwnedRoot,
}

/// A genuine root-owned listing at `listing_epoch`/`listing_revision`, valid at
/// `now` (expiry `now + 1 day`), with a distinct title so different revisions have
/// distinct digests.
pub fn genuine_listing(seed: u8, epoch: u32, revision: u32, now: u64) -> GenuineListing {
    let root = owned_root(seed);
    let coords = Coords {
        root_id: root.root_id,
        o: root.root_id,
        c: d32(seed.wrapping_add(1)),
        w: d32(seed.wrapping_add(2)),
        manifest_digest: d32(seed.wrapping_add(3)),
        manifest_version: 3,
    };
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(&root, &coords, now.saturating_sub(1), expiry);
    let payload = listing_payload(
        &coords,
        ticket,
        epoch,
        revision,
        true,
        &format!("Community {seed}/{epoch}/{revision}"),
        now.saturating_sub(1),
        expiry,
    );
    let payload_bytes = payload.encode_canonical().expect("encode listing payload");
    let owner = SubspaceSecret::from_bytes(&[seed.wrapping_add(50); 32]);
    let item = root_owned_item(&root, &owner, &payload_bytes);
    GenuineListing {
        submission: RawListingSubmission {
            listing_item_bytes: item,
            delegate_grant: None,
        },
        coords,
        community_id: root.root_id,
        listing_epoch: epoch,
        listing_revision: revision,
        expiry,
        root,
    }
}

/// A genuine root-owned listing over an EXISTING root + coordinates (for refresh
/// and changed-body cases). Returns just the submission.
pub fn genuine_listing_for(
    root: &OwnedRoot,
    coords: Coords,
    epoch: u32,
    revision: u32,
    now: u64,
) -> RawListingSubmission {
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(root, &coords, now.saturating_sub(1), expiry);
    let payload = listing_payload(
        &coords,
        ticket,
        epoch,
        revision,
        true,
        &format!("Refresh {epoch}/{revision}"),
        now.saturating_sub(1),
        expiry,
    );
    let payload_bytes = payload.encode_canonical().expect("encode listing payload");
    let owner = SubspaceSecret::from_bytes(&[0x55; 32]);
    let item = root_owned_item(root, &owner, &payload_bytes);
    RawListingSubmission {
        listing_item_bytes: item,
        delegate_grant: None,
    }
}

/// A genuine DELEGATED listing: the root delegates a `/directory`-scoped cap to a
/// separate key that signs the entry, plus a correctly root-signed grant.
pub fn delegated_listing(seed: u8, epoch: u32, revision: u32, now: u64) -> GenuineListing {
    let root = owned_root(seed);
    let coords = Coords {
        root_id: root.root_id,
        o: root.root_id,
        c: d32(seed.wrapping_add(1)),
        w: d32(seed.wrapping_add(2)),
        manifest_digest: d32(seed.wrapping_add(3)),
        manifest_version: 3,
    };
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(&root, &coords, now.saturating_sub(1), expiry);
    let payload = listing_payload(
        &coords,
        ticket,
        epoch,
        revision,
        true,
        &format!("Delegated {seed}/{epoch}/{revision}"),
        now.saturating_sub(1),
        expiry,
    );
    let payload_bytes = payload.encode_canonical().expect("encode listing payload");

    // Root delegates a /directory-scoped cap to `delegate`.
    let owner = SubspaceSecret::from_bytes(&[seed.wrapping_add(60); 32]);
    let owner_id = owner.corresponding_subspace_id();
    let delegate = SubspaceSecret::from_bytes(&[seed.wrapping_add(70); 32]);
    let delegate_id = delegate.corresponding_subspace_id();

    let mut cap = WriteCapability::new_owned(&root.namespace_secret, owner_id.clone());
    let area = Area::new(
        Some(delegate_id.clone()),
        Path::from_slices(&[DIRECTORY_COMPONENT]).expect("directory area"),
        TimeRange::new(0u64.into(), Some(u64::MAX.into())),
    );
    cap.try_delegate(&owner, area, delegate_id.clone())
        .expect("delegate under /directory");

    let path = Path::from_slices(&[DIRECTORY_COMPONENT, LISTING_COMPONENT]).expect("listing path");
    let entry = Entry::builder()
        .namespace_id(root.namespace_secret.corresponding_namespace_id())
        .subspace_id(delegate_id.clone())
        .path(path)
        .timestamp(1_000u64)
        .payload(&payload_bytes)
        .build();
    let authorised = entry
        .into_authorised_entry(&cap, &delegate)
        .expect("delegate authorises O:/directory/listing");
    let token = authorised.authorisation_token();
    let signature: ed25519_dalek::Signature = token.signature().clone().into();
    let item = encode_item(
        &encode_entry(authorised.entry()),
        &encode_capability(token.capability()),
        &signature.to_bytes(),
        &payload_bytes,
    );

    let grant = root_signed_grant(&root, *delegate_id.as_bytes(), epoch, expiry);

    GenuineListing {
        submission: RawListingSubmission {
            listing_item_bytes: item,
            delegate_grant: Some(grant),
        },
        coords,
        community_id: root.root_id,
        listing_epoch: epoch,
        listing_revision: revision,
        expiry,
        root,
    }
}

/// A correctly root-signed delegate grant for `delegate_key` at `epoch`.
pub fn root_signed_grant(
    root: &OwnedRoot,
    delegate_key: [u8; 32],
    epoch: u32,
    expiry: u64,
) -> RawDelegateGrant {
    let grant = ListingDelegateGrantV1 {
        root_id: root.root_id,
        delegate_key,
        terminal_capability_digest: [0u8; 32],
        listing_epoch: epoch,
        issued_unix_seconds: 0,
        expiry_unix_seconds: expiry,
    };
    let grant_bytes = grant.encode_canonical().expect("encode grant");
    let mut preimage = LISTING_DELEGATE_GRANT_SIGNING_DOMAIN.to_vec();
    preimage.extend_from_slice(&grant_bytes);
    let signature = root.root_signing_key.sign(&preimage).to_bytes();
    RawDelegateGrant {
        grant_bytes,
        signature,
    }
}

/// Flip one byte of the entry SIGNATURE inside a listing item, so `verify_entry`
/// fails. The item layout is: version(1) | u32 entry_len | entry | u32 cap_len |
/// cap | 64-byte signature | u32 payload_len | payload.
pub fn forge_item_signature(item: &[u8]) -> Vec<u8> {
    let mut forged = item.to_vec();
    let entry_len = u32::from_be_bytes([forged[1], forged[2], forged[3], forged[4]]) as usize;
    let cap_len_at = 1 + 4 + entry_len;
    let cap_len = u32::from_be_bytes([
        forged[cap_len_at],
        forged[cap_len_at + 1],
        forged[cap_len_at + 2],
        forged[cap_len_at + 3],
    ]) as usize;
    let sig_at = cap_len_at + 4 + cap_len;
    forged[sig_at] ^= 0x01;
    forged
}

/// A configurable hosting authority for the listing service.
pub struct TestListingAuthority {
    pub result: RefCell<Result<HostingState, ControlRefusal>>,
}

impl TestListingAuthority {
    pub fn hosted() -> Self {
        Self {
            result: RefCell::new(Ok(HostingState {
                full_site_root: d32(0),
                current_site_generation: 1,
            })),
        }
    }
    pub fn hosted_root(full_site_root: [u8; 32]) -> Self {
        Self {
            result: RefCell::new(Ok(HostingState {
                full_site_root,
                current_site_generation: 1,
            })),
        }
    }
    pub fn refusing(refusal: ControlRefusal) -> Self {
        Self {
            result: RefCell::new(Err(refusal)),
        }
    }
}

impl ListingAuthority for TestListingAuthority {
    fn resolve_hosting(
        &self,
        _coordinates: &VerifiedListingCoordinates,
        _observed_at: u64,
    ) -> Result<HostingState, ControlRefusal> {
        self.result.borrow().clone()
    }
}

/// Insert the hosted community row a listing accept expects to already exist.
pub fn insert_hosted_community(repo: &mut AnchorRepository, community_id: [u8; 32], now: u64) {
    let mut tx = repo.begin().expect("begin");
    tx.insert_community(&community_id, now)
        .expect("insert community");
    tx.commit().expect("commit community");
}
