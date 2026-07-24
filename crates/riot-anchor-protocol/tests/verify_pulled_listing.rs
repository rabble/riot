//! WU-1 (relay community directory) — the pure client-side pulled-listing verifier.
//!
//! `verify_pulled_listing` is the seam a later FFI `pull_directory` (WU-3) calls to
//! re-verify a directory row **without trusting the anchor**. It reuses the exact
//! canonical gates already in this crate:
//!
//!   * [`OperatorSignedEnvelopeV1::verify`] against the app's PINNED operator key
//!     (the relay identity) — a lying/compromised relay that did not sign the
//!     checkpoint with the pinned key is rejected;
//!   * [`admit_public_site_ticket`] over the row's embedded
//!     `RootSignedTicketCoreEnvelopeV2` — the community's own O-root signature,
//!     independent of the anchor, plus transport-floor + expiry;
//!   * the same `root/O/C/W/manifest` coordinate binding
//!     `SubmitListingService::verify_submission` performs, so a row whose display
//!     record disagrees with its signed ticket is dropped.
//!
//! Adversarial cases are first-class here: bad operator key, forged ticket,
//! coordinate disagreement, and expiry each reject with a distinct closed reason.

use ed25519_dalek::{Signer, SigningKey};
use minicbor::{Decoder, Encoder};

use riot_anchor_protocol::authority::{verify_pulled_listing, AuthorityError, TicketReason};
use riot_anchor_protocol::codec::{CanonicalRecord, CodecError};
use riot_anchor_protocol::records::{
    AnchorSignedBody, CommunityListingV1, OperatorSignedEnvelopeV1, PublicSiteTicketV2Core,
    RootSignedTicketCoreEnvelopeV2, TransportFloor,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const O_NS: [u8; 32] = [0x0a; 32];
const C_NS: [u8; 32] = [0x0c; 32];
const W_NS: [u8; 32] = [0x0e; 32];
const MANIFEST_DIGEST: [u8; 32] = [0x0d; 32];
const MANIFEST_VERSION: u64 = 4;

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn root_pubkey(sk: &SigningKey) -> [u8; 32] {
    sk.verifying_key().to_bytes()
}

/// A minimal operator-signed checkpoint body. WU-1 keeps the verifier generic over
/// any [`AnchorSignedBody`]; WU-2 supplies the real `DirectoryCheckpointBodyV1`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestCheckpointBody {
    feed_head: [u8; 32],
}

impl CanonicalRecord for TestCheckpointBody {
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError> {
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        e.array(1).map_err(|_| CodecError::Malformed)?;
        e.bytes(&self.feed_head)
            .map_err(|_| CodecError::Malformed)?;
        Ok(buf)
    }

    fn decode_fields(d: &mut Decoder<'_>) -> Result<Self, CodecError> {
        if d.array().map_err(|_| CodecError::Malformed)? != Some(1) {
            return Err(CodecError::Malformed);
        }
        let raw = d.bytes().map_err(|_| CodecError::Malformed)?;
        let feed_head: [u8; 32] = raw.try_into().map_err(|_| CodecError::Malformed)?;
        Ok(TestCheckpointBody { feed_head })
    }
}

impl AnchorSignedBody for TestCheckpointBody {
    const SIGNING_DOMAIN: &'static [u8] = b"riot/test-checkpoint/v1";
}

/// A well-formed ticket core whose coordinates match the listing below, with an
/// all-`require_none` transport and a 90-day-safe validity window (`expiry` 2000).
fn core_for(root: [u8; 32]) -> PublicSiteTicketV2Core {
    PublicSiteTicketV2Core {
        root_id: root,
        o_namespace_id: O_NS,
        c_namespace_id: C_NS,
        w_namespace_id: W_NS,
        manifest_digest: MANIFEST_DIGEST,
        manifest_version: MANIFEST_VERSION,
        min_sync_version: 2,
        manifest_required_transport: TransportFloor::RequireNone,
        transport_floor: TransportFloor::RequireNone,
        transport_epoch: 3,
        issued_unix_seconds: 500,
        expiry_unix_seconds: 2000,
    }
}

fn sign_core(sk: &SigningKey, core: PublicSiteTicketV2Core) -> RootSignedTicketCoreEnvelopeV2 {
    let canonical = core.encode_canonical().unwrap();
    let mut preimage = RootSignedTicketCoreEnvelopeV2::SIGNING_DOMAIN.to_vec();
    preimage.extend_from_slice(&canonical);
    let sig = sk.sign(&preimage).to_bytes();
    RootSignedTicketCoreEnvelopeV2 {
        core,
        root_signature: sig,
    }
}

/// A listing whose display coordinates agree with `ticket_core_bytes`.
fn listing_for(root: [u8; 32], ticket_core_bytes: Vec<u8>) -> CommunityListingV1 {
    CommunityListingV1 {
        root_id: root,
        o_namespace_id: O_NS,
        c_namespace_id: C_NS,
        w_namespace_id: W_NS,
        manifest_digest: MANIFEST_DIGEST,
        manifest_version: MANIFEST_VERSION,
        ticket_core_bytes,
        listing_epoch: 4,
        listing_revision: 1,
        listed: true,
        title: "Riverside Mutual Aid".to_string(),
        summary: "Community wire for the riverside neighbourhood.".to_string(),
        topic_tags: vec![b"aid".to_vec(), b"housing".to_vec()],
        languages: vec!["en".to_string(), "es".to_string()],
        region: Some(b"us-ca".to_vec()),
        issued_unix_seconds: 500,
        expiry_unix_seconds: 2000,
        steward_name: Some("Rosa & the riverside crew".to_string()),
    }
}

/// Operator-sign a checkpoint under `operator_sk`.
fn checkpoint(operator_sk: &SigningKey) -> OperatorSignedEnvelopeV1<TestCheckpointBody> {
    let mut envelope = OperatorSignedEnvelopeV1 {
        body: TestCheckpointBody {
            feed_head: [0x77; 32],
        },
        operator_signature: [0u8; 64],
    };
    let preimage = envelope.signing_preimage().unwrap();
    envelope.operator_signature = operator_sk.sign(&preimage).to_bytes();
    envelope
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn well_formed_row_verifies_and_projects_display_fields() {
    let root_sk = signing_key(7);
    let operator_sk = signing_key(9);
    let envelope = sign_core(&root_sk, core_for(root_pubkey(&root_sk)));
    let ticket_core_bytes = envelope.encode_canonical().unwrap();
    let listing = listing_for(root_pubkey(&root_sk), ticket_core_bytes.clone());
    let cp = checkpoint(&operator_sk);

    let row = verify_pulled_listing(&cp, &root_pubkey(&operator_sk), &listing, 1000)
        .expect("a well-formed pulled row must verify");

    assert_eq!(row.root_id, root_pubkey(&root_sk));
    assert_eq!(row.o_namespace_id, O_NS);
    assert_eq!(row.c_namespace_id, C_NS);
    assert_eq!(row.w_namespace_id, W_NS);
    assert_eq!(row.title, "Riverside Mutual Aid");
    assert_eq!(
        row.steward_name.as_deref(),
        Some("Rosa & the riverside crew")
    );
    assert_eq!(row.ticket_core_bytes, ticket_core_bytes);
    assert_eq!(row.expiry_unix_seconds, 2000);
    // The verified ticket-core digest is surfaced for the caller's join route.
    assert_eq!(
        row.root_signed_ticket_core_digest,
        envelope.root_signed_ticket_core_digest().unwrap()
    );
}

#[test]
fn wrong_operator_key_is_rejected() {
    let root_sk = signing_key(7);
    let operator_sk = signing_key(9);
    let wrong_operator = signing_key(10);
    let envelope = sign_core(&root_sk, core_for(root_pubkey(&root_sk)));
    let listing = listing_for(root_pubkey(&root_sk), envelope.encode_canonical().unwrap());
    let cp = checkpoint(&operator_sk);

    // The checkpoint was signed by `operator_sk`, but the app pins a DIFFERENT key.
    let err =
        verify_pulled_listing(&cp, &root_pubkey(&wrong_operator), &listing, 1000).unwrap_err();
    assert_eq!(err, AuthorityError::UntrustedCheckpoint);
}

#[test]
fn forged_ticket_signature_is_rejected() {
    let root_sk = signing_key(7);
    let operator_sk = signing_key(9);
    let mut envelope = sign_core(&root_sk, core_for(root_pubkey(&root_sk)));
    // Flip a bit in the O-root signature — the anchor cannot forge this.
    envelope.root_signature[0] ^= 0x01;
    let listing = listing_for(root_pubkey(&root_sk), envelope.encode_canonical().unwrap());
    let cp = checkpoint(&operator_sk);

    let err = verify_pulled_listing(&cp, &root_pubkey(&operator_sk), &listing, 1000).unwrap_err();
    assert_eq!(err, AuthorityError::InvalidTicket(TicketReason::Signature));
}

#[test]
fn coordinate_disagreement_between_row_and_ticket_is_rejected() {
    let root_sk = signing_key(7);
    let operator_sk = signing_key(9);
    let envelope = sign_core(&root_sk, core_for(root_pubkey(&root_sk)));
    let mut listing = listing_for(root_pubkey(&root_sk), envelope.encode_canonical().unwrap());
    // A lying anchor rewrites the row's C namespace; the SIGNED ticket still says C_NS.
    listing.c_namespace_id = [0xff; 32];
    let cp = checkpoint(&operator_sk);

    let err = verify_pulled_listing(&cp, &root_pubkey(&operator_sk), &listing, 1000).unwrap_err();
    assert_eq!(err, AuthorityError::ManifestMismatch);
}

#[test]
fn expired_ticket_is_rejected() {
    let root_sk = signing_key(7);
    let operator_sk = signing_key(9);
    let envelope = sign_core(&root_sk, core_for(root_pubkey(&root_sk)));
    let listing = listing_for(root_pubkey(&root_sk), envelope.encode_canonical().unwrap());
    let cp = checkpoint(&operator_sk);

    // Ticket expiry is inclusive: now == expiry is expired.
    let err = verify_pulled_listing(&cp, &root_pubkey(&operator_sk), &listing, 2000).unwrap_err();
    assert_eq!(err, AuthorityError::ExpiredTicket);
}

#[test]
fn malformed_embedded_ticket_bytes_are_rejected() {
    let root_sk = signing_key(7);
    let operator_sk = signing_key(9);
    // The row claims a ticket but carries garbage bytes for it.
    let listing = listing_for(root_pubkey(&root_sk), vec![0x00, 0x01, 0x02]);
    let cp = checkpoint(&operator_sk);

    let err = verify_pulled_listing(&cp, &root_pubkey(&operator_sk), &listing, 1000).unwrap_err();
    assert!(matches!(err, AuthorityError::MalformedRecord(_)));
}
