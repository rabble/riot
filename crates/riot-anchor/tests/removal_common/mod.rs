//! Shared fixtures for the reserved owner-removal tests: a real operator signer,
//! genuinely root-signed tombstones (`listed == false`) over owned namespaces, a
//! delegated tombstone variant, and helpers that put a community into the durable
//! "listed with a reserved slot" state the removal service consumes.

#![allow(dead_code)]

use ed25519_dalek::{Signer as _, SigningKey};

use riot_anchor::removal::{
    RawDelegateGrant, RawRemovalSubmission, RemovalContext, ReservedRemovalService,
};
use riot_anchor::repository::{AnchorRepository, SlotReservation};
use riot_anchor::sync_service::encode_item;
use riot_anchor::work::OperatorSigner;

use riot_anchor_protocol::codec::CanonicalRecord;
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
    SigningKey::from_bytes(&[9u8; 32])
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

pub fn removal_context() -> RemovalContext {
    RemovalContext {
        anchor_id: d32(1),
        operator_key_id: d32(2),
        descriptor_epoch: 5,
        descriptor_digest: d32(3),
    }
}

pub fn service() -> ReservedRemovalService<TestSigner> {
    ReservedRemovalService::new(removal_context(), signer(), 60)
}

/// An owned-namespace root whose Ed25519 secret we retain.
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
            return OwnedRoot {
                namespace_secret,
                root_signing_key,
                root_id: *namespace_id.as_bytes(),
            };
        }
    }
    panic!("no owned namespace found for seed {seed}");
}

#[derive(Clone, Copy)]
pub struct Coords {
    pub root_id: [u8; 32],
    pub o: [u8; 32],
    pub c: [u8; 32],
    pub w: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub manifest_version: u64,
}

pub fn coords_for(root: &OwnedRoot, seed: u8) -> Coords {
    Coords {
        root_id: root.root_id,
        o: root.root_id,
        c: d32(seed.wrapping_add(1)),
        w: d32(seed.wrapping_add(2)),
        manifest_digest: d32(seed.wrapping_add(3)),
        manifest_version: 3,
    }
}

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

/// Encode a signed root-owned item (owner subspace signs a zero-delegation cap).
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

/// A genuine root-owned TOMBSTONE (`listed == false`) plus its coordinates.
pub struct GenuineTombstone {
    pub submission: RawRemovalSubmission,
    pub coords: Coords,
    pub community_id: [u8; 32],
    pub root: OwnedRoot,
}

/// Build a genuine root-owned tombstone at `epoch`/`revision`, valid at `now`.
pub fn genuine_tombstone(seed: u8, epoch: u32, revision: u32, now: u64) -> GenuineTombstone {
    let root = owned_root(seed);
    let coords = coords_for(&root, seed);
    tombstone_for(root, coords, seed, epoch, revision, now)
}

/// Build a genuine root-owned tombstone over an EXISTING root/coords.
pub fn tombstone_for(
    root: OwnedRoot,
    coords: Coords,
    seed: u8,
    epoch: u32,
    revision: u32,
    now: u64,
) -> GenuineTombstone {
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(&root, &coords, now.saturating_sub(1), expiry);
    let payload = listing_payload(
        &coords,
        ticket,
        epoch,
        revision,
        false,
        &format!("Tombstone {seed}/{epoch}/{revision}"),
        now.saturating_sub(1),
        expiry,
    );
    let payload_bytes = payload
        .encode_canonical()
        .expect("encode tombstone payload");
    let owner = SubspaceSecret::from_bytes(&[seed.wrapping_add(50); 32]);
    let item = root_owned_item(&root, &owner, &payload_bytes);
    GenuineTombstone {
        submission: RawRemovalSubmission {
            tombstone_item_bytes: item,
            delegate_grant: None,
        },
        coords,
        community_id: root.root_id,
        root,
    }
}

/// A genuine DELEGATED tombstone: root delegates a `/directory`-scoped cap to a
/// separate key that signs the tombstone, plus a correctly root-signed grant.
pub fn delegated_tombstone(seed: u8, epoch: u32, revision: u32, now: u64) -> GenuineTombstone {
    let root = owned_root(seed);
    let coords = coords_for(&root, seed);
    let expiry = now + 24 * 60 * 60;
    let ticket = root_signed_ticket(&root, &coords, now.saturating_sub(1), expiry);
    let payload = listing_payload(
        &coords,
        ticket,
        epoch,
        revision,
        false,
        &format!("Delegated tombstone {seed}"),
        now.saturating_sub(1),
        expiry,
    );
    let payload_bytes = payload
        .encode_canonical()
        .expect("encode tombstone payload");

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

    GenuineTombstone {
        submission: RawRemovalSubmission {
            tombstone_item_bytes: item,
            delegate_grant: Some(grant),
        },
        coords,
        community_id: root.root_id,
        root,
    }
}

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

/// Flip one byte of the entry SIGNATURE inside an item so `verify_entry` fails.
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

/// Put a community into "listed with a reserved slot" durable state, exactly as
/// the ordinary visibility transition does: insert the community, reserve a slot
/// under the exact per-root two-slot rule, and insert its listing row. Returns
/// the reserved slot index (or `None` if the reservation was blocked).
pub fn list_and_reserve(
    repo: &mut AnchorRepository,
    coords: &Coords,
    request_digest: &[u8; 32],
    now: u64,
) -> Option<u32> {
    let mut tx = repo.begin().expect("begin");
    let community = coords.o;
    // The community row may already exist across relist cycles; ignore a
    // duplicate-primary-key error (SQLite leaves the transaction usable).
    let _ = tx.insert_community(&community, now);
    let reservation = tx
        .reserve_visibility_slot(&community, &coords.root_id, request_digest, now)
        .expect("reserve");
    let slot = match reservation {
        SlotReservation::Reserved(slot) => slot,
        SlotReservation::Blocked { .. } => {
            drop(tx);
            return None;
        }
    };
    tx.insert_listing(
        &community,
        &coords.root_id,
        now,
        now + 24 * 60 * 60,
        now,
        slot,
    )
    .expect("insert listing");
    tx.commit().expect("commit");
    Some(slot)
}
