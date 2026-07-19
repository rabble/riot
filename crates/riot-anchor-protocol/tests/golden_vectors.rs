//! WU-006A — Rust/TypeScript cross-language conformance vectors.
//!
//! This test is the **cross-language freeze point** for the anchor protocol wire
//! format. The `riot-anchor-protocol` crate (WU-002..005) is the SOURCE OF TRUTH:
//! every byte in `fixtures/anchor/protocol-v1-vectors.json` and
//! `fixtures/anchor/bootstrap-development-v1.cbor` is generated here from the
//! crate's own public constructors and codecs — never hand-authored — and the
//! TypeScript consumer (`scripts/web/anchor-protocol-vectors.ts`) must reproduce
//! the same bytes independently.
//!
//! The test both EMITS and CONSUMES the fixture:
//!   * EMIT: build every record/digest/signature/preimage from fixed test seeds
//!     (no `SystemTime`/`Date::now`/randomness — the crate forbids them), serialise
//!     to canonical JSON, and (with `RIOT_BLESS_ANCHOR_VECTORS=1`) write it to disk.
//!   * CONSUME: decode every canonical vector, recompute every digest, verify every
//!     signature, re-derive every preimage, and confirm each alternate-grammar /
//!     one-bit mutation is rejected — so a stale checked-in fixture fails here.
//!
//! Regenerate after an intentional wire change:
//!   RIOT_BLESS_ANCHOR_VECTORS=1 cargo test -p riot-anchor-protocol --test golden_vectors

use std::path::PathBuf;

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};

use riot_anchor_protocol::control::{
    ControlOperation, ControlOutcome, ControlRefusal, ControlRequestV1, ControlResponseV1,
    ControlSuccess, DescribeV1, SnapshotCursorBodyV1, SnapshotCursorV1,
};
use riot_anchor_protocol::digest::{
    anchor_id, digest_v1, label, namespace_token_hmac_input, operator_key_id,
    peer_proof_signature_preimage, snapshot_cursor_hmac_input, sync_snapshot_digest, work_proof,
};
use riot_anchor_protocol::records::{
    terminal_capability_digest, AdmittedListingEnvelopeV1, AnchorBootstrapV1,
    AnchorDescriptorBodyV1, AnchorLimitEntry, AnchorLimitProfileV1, AnchorSignedBody,
    BootstrapDescriptorV1, CommunityListingV1, ControlOperationKind, DescriptorEnvelopeV1,
    DescriptorFloor, EnabledRole, HostingReceiptBodyV1, HostingReceiptV1, HostingStatus,
    LimitValue, ListingDelegateGrantV1, ListingReceiptBodyV1, ListingReceiptV1, NamespaceResult,
    OperatorSignedEnvelopeV1, OperatorVerificationKeyV1, PublicSiteTicketV2Core,
    ReplicaPrepareChallengeV1, ReplicaSourceAttestationBodyV1, ReplicaSourceAttestationV1,
    RootSignedTicketCoreEnvelopeV2, TransportFloor, WorkChallengeBodyV1, WorkChallengeV1,
    WorkStampV1,
};
use riot_anchor_protocol::{decode_canonical, CanonicalRecord, CodecError};

// Bare signing domains that are not re-exported as crate constants; pinned here as
// literals and asserted below to be the exact prefix of the crate's own preimage.
const DELEGATE_GRANT_DOMAIN: &[u8] = b"riot/listing-delegate-grant/v1";
const TICKET_DOMAIN: &[u8] = b"riot/public-site-ticket/v2";

// ---------------------------------------------------------------------------
// Small deterministic helpers.
// ---------------------------------------------------------------------------

fn hx(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// A `u64` rendered as a decimal string (all wire integers cross the JSON boundary
/// as strings so a 64-bit value never loses precision in a JS `number`).
fn s(n: u64) -> Value {
    Value::String(n.to_string())
}

/// A distinct, deterministic 32-byte pattern seeded by `tag`.
fn b32(tag: u8) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = tag.wrapping_add(i as u8).wrapping_mul(3).wrapping_add(1);
    }
    out
}

fn b16(tag: u8) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = tag.wrapping_add(i as u8).wrapping_mul(5).wrapping_add(7);
    }
    out
}

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn key_public(sk: &SigningKey) -> [u8; 32] {
    sk.verifying_key().to_bytes()
}

fn opkey(pk: [u8; 32]) -> OperatorVerificationKeyV1 {
    OperatorVerificationKeyV1 { public_key: pk }
}

fn opkey_fields(k: &OperatorVerificationKeyV1) -> Value {
    json!({ "public_key": hx(&k.public_key) })
}

fn limit_value_field(v: LimitValue) -> Value {
    match v {
        LimitValue::Scalar(a) => s(a),
        LimitValue::Compound(a, b) => json!([s(a), s(b)]),
    }
}

fn transport_token(t: TransportFloor) -> &'static str {
    t.token()
}

// ---------------------------------------------------------------------------
// Vector accumulators.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Builder {
    records: Vec<Value>,
    digests: Vec<Value>,
    grammar: Vec<Value>,
}

impl Builder {
    /// Push a record vector: id, record-type tag, semantic `fields`, and the
    /// canonical bytes the TS encoder must reproduce. Optional attachments carry
    /// digests / signatures the TS side must recompute and verify independently.
    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        id: &str,
        record: &str,
        fields: Value,
        canonical: &[u8],
        max_decode: usize,
        digests: Vec<Value>,
        signatures: Vec<Value>,
        notes: &str,
    ) {
        let mut obj = json!({
            "id": id,
            "record": record,
            "notes": notes,
            "fields": fields,
            "canonical_hex": hx(canonical),
            "max_decode_bytes": s(max_decode as u64),
        });
        if !digests.is_empty() {
            obj["digests"] = Value::Array(digests);
        }
        if !signatures.is_empty() {
            obj["signatures"] = Value::Array(signatures);
        }
        self.records.push(obj);
    }

    fn digest_vector(&mut self, v: Value) {
        self.digests.push(v);
    }

    fn grammar_reject(&mut self, desc: &str, record: &str, hostile: &[u8], expect: CodecError) {
        // Prove the hostile encoding is genuinely non-canonical against the crate's
        // decoder before pinning it for the TS side.
        let decoded = decode_record(record, hostile);
        assert_eq!(
            decoded.err().as_ref(),
            Some(&expect),
            "alt-grammar '{desc}' should be rejected as {expect:?}"
        );
        self.grammar.push(json!({
            "desc": desc,
            "record": record,
            "hostile_hex": hx(hostile),
            "expect": format!("{expect:?}"),
        }));
    }
}

/// Attempt to canonically decode `hostile` bytes as the named record type.
fn decode_record(record: &str, bytes: &[u8]) -> Result<(), CodecError> {
    match record {
        "OperatorVerificationKeyV1" => {
            decode_canonical::<OperatorVerificationKeyV1>(bytes, 4096).map(|_| ())
        }
        "PublicSiteTicketV2Core" => {
            decode_canonical::<PublicSiteTicketV2Core>(bytes, 4096).map(|_| ())
        }
        "CommunityListingV1" => decode_canonical::<CommunityListingV1>(bytes, 16_384).map(|_| ()),
        other => panic!("no decoder wired for alt-grammar record {other}"),
    }
}

// ---------------------------------------------------------------------------
// digest / preimage attachment builders.
// ---------------------------------------------------------------------------

/// A `digest_v1(label, canonical(record))` attachment. `message` is `"canonical"`,
/// telling the TS side to frame over the record's own re-derived canonical bytes.
fn digest_over_canonical(name: &str, label_bytes: &[u8], canonical: &[u8]) -> Value {
    let value = digest_v1(label_bytes, canonical);
    json!({
        "name": name,
        "algo": "digest_v1",
        "label_ascii": String::from_utf8(label_bytes.to_vec()).unwrap(),
        "message": "canonical",
        "preimage_hex": hx(&digest_v1_preimage(label_bytes, canonical)),
        "value_hex": hx(&value),
    })
}

fn digest_v1_preimage(label_bytes: &[u8], canonical: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(label_bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(label_bytes);
    out.extend_from_slice(&(canonical.len() as u64).to_be_bytes());
    out.extend_from_slice(canonical);
    out
}

/// An ed25519 signature attachment: the exact preimage (`domain || message`) plus
/// the public key and 64-byte signature. TS re-derives the preimage and verifies.
fn signature(
    name: &str,
    public_key: &[u8; 32],
    domain: &[u8],
    message: &str,
    preimage: &[u8],
    sig: &[u8; 64],
) -> Value {
    json!({
        "name": name,
        "public_key_hex": hx(public_key),
        "domain_ascii": String::from_utf8(domain.to_vec()).unwrap(),
        "message": message,
        "preimage_hex": hx(preimage),
        "signature_hex": hx(sig),
    })
}

// ===========================================================================
// The generator.
// ===========================================================================

fn build_vectors() -> (Value, Vec<u8>) {
    let mut b = Builder::default();

    // -- shared keys -------------------------------------------------------
    let root_sk = signing_key(7);
    let root_pk = key_public(&root_sk);
    let op_sk = signing_key(11);
    let op_pk = key_public(&op_sk);
    let pred_sk = signing_key(13);
    let pred_pk = key_public(&pred_sk);

    // ---------------------------------------------------------------------
    // 1. OperatorVerificationKeyV1  (+ operator_key_id bare-label digest)
    // ---------------------------------------------------------------------
    let op_key = opkey(op_pk);
    let op_key_bytes = op_key.encode_canonical().unwrap();
    let op_key_id = op_key.operator_key_id().unwrap();
    assert_eq!(op_key_id, operator_key_id(&op_key_bytes));
    b.record(
        "operator_verification_key",
        "OperatorVerificationKeyV1",
        opkey_fields(&op_key),
        &op_key_bytes,
        4096,
        vec![json!({
            "name": "operator_key_id",
            "algo": "operator_key_id",
            "label_ascii": String::from_utf8(label::OPERATOR_KEY_ID.to_vec()).unwrap(),
            "message": "canonical",
            "preimage_hex": hx(&[label::OPERATOR_KEY_ID, &op_key_bytes].concat()),
            "value_hex": hx(&op_key_id),
        })],
        vec![],
        "algorithm is the implicit `ed25519` token; operator_key_id is a bare-label BLAKE3 (no length prefixes).",
    );

    // ---------------------------------------------------------------------
    // 2. PublicSiteTicketV2Core  (implicit version; TransportFloor sentinels)
    // ---------------------------------------------------------------------
    let o_ns = b32(0x20);
    let c_ns = b32(0x30);
    let w_ns = b32(0x40);
    let manifest_digest = b32(0x50);
    let ticket_core = PublicSiteTicketV2Core {
        root_id: root_pk,
        o_namespace_id: o_ns,
        c_namespace_id: c_ns,
        w_namespace_id: w_ns,
        manifest_digest,
        manifest_version: 7,
        min_sync_version: 2,
        manifest_required_transport: TransportFloor::RequireNone,
        transport_floor: TransportFloor::RequireNone,
        transport_epoch: 3,
        issued_unix_seconds: 1_760_000_000,
        expiry_unix_seconds: 1_760_086_400,
    };
    let ticket_core_bytes = ticket_core.encode_canonical().unwrap();
    let ticket_core_fields = json!({
        "root_id": hx(&ticket_core.root_id),
        "o_namespace_id": hx(&ticket_core.o_namespace_id),
        "c_namespace_id": hx(&ticket_core.c_namespace_id),
        "w_namespace_id": hx(&ticket_core.w_namespace_id),
        "manifest_digest": hx(&ticket_core.manifest_digest),
        "manifest_version": s(ticket_core.manifest_version),
        "min_sync_version": s(ticket_core.min_sync_version),
        "manifest_required_transport": transport_token(ticket_core.manifest_required_transport),
        "transport_floor": transport_token(ticket_core.transport_floor),
        "transport_epoch": s(ticket_core.transport_epoch as u64),
        "issued_unix_seconds": s(ticket_core.issued_unix_seconds),
        "expiry_unix_seconds": s(ticket_core.expiry_unix_seconds),
    });
    b.record(
        "public_site_ticket_core",
        "PublicSiteTicketV2Core",
        ticket_core_fields.clone(),
        &ticket_core_bytes,
        1024,
        vec![],
        vec![],
        "12 positional fields, NO leading version int (version carried by envelope tag / signing domain).",
    );

    // ---------------------------------------------------------------------
    // 3. RootSignedTicketCoreEnvelopeV2 (nested embed + ed25519 + digest_v1)
    // ---------------------------------------------------------------------
    let mut ticket_preimage = TICKET_DOMAIN.to_vec();
    ticket_preimage.extend_from_slice(&ticket_core_bytes);
    let ticket_sig = root_sk.sign(&ticket_preimage).to_bytes();
    let ticket_env = RootSignedTicketCoreEnvelopeV2 {
        core: ticket_core.clone(),
        root_signature: ticket_sig,
    };
    assert_eq!(ticket_env.signing_preimage().unwrap(), ticket_preimage);
    let ticket_env_bytes = ticket_env.encode_canonical().unwrap();
    let ticket_env_digest = ticket_env.root_signed_ticket_core_digest().unwrap();
    b.record(
        "root_signed_ticket_core_envelope",
        "RootSignedTicketCoreEnvelopeV2",
        json!({ "core": ticket_core_fields, "root_signature": hx(&ticket_sig) }),
        &ticket_env_bytes,
        1024,
        vec![digest_over_canonical(
            "root_signed_ticket_core_digest",
            label::PUBLIC_SITE_TICKET_SIGNED_CORE,
            &ticket_env_bytes,
        )],
        vec![signature(
            "root_signature",
            &root_pk,
            TICKET_DOMAIN,
            "ticket_core_canonical",
            &ticket_preimage,
            &ticket_sig,
        )],
        "tag `2`; the core is embedded as a nested CBOR array (not double-encoded bytes); signature over DOMAIN||core.",
    );

    // ---------------------------------------------------------------------
    // 4. ListingDelegateGrantV1 (implicit version; bare-domain signature)
    // ---------------------------------------------------------------------
    let terminal_cap_canonical = vec![0xa0u8, 0x01, 0x02, 0x03]; // opaque terminal capability bytes
    let terminal_cap_digest = terminal_capability_digest(&terminal_cap_canonical);
    let grant = ListingDelegateGrantV1 {
        root_id: root_pk,
        delegate_key: b32(0x60),
        terminal_capability_digest: terminal_cap_digest,
        listing_epoch: 4,
        issued_unix_seconds: 1_760_000_000,
        expiry_unix_seconds: 1_760_500_000,
    };
    let grant_bytes = grant.encode_canonical().unwrap();
    let grant_preimage = grant.signing_preimage().unwrap();
    assert!(grant_preimage.starts_with(DELEGATE_GRANT_DOMAIN));
    let grant_sig = root_sk.sign(&grant_preimage).to_bytes();
    b.record(
        "listing_delegate_grant",
        "ListingDelegateGrantV1",
        json!({
            "root_id": hx(&grant.root_id),
            "delegate_key": hx(&grant.delegate_key),
            "terminal_capability_digest": hx(&grant.terminal_capability_digest),
            "listing_epoch": s(grant.listing_epoch as u64),
            "issued_unix_seconds": s(grant.issued_unix_seconds),
            "expiry_unix_seconds": s(grant.expiry_unix_seconds),
        }),
        &grant_bytes,
        512,
        vec![],
        vec![signature(
            "grant_signature",
            &root_pk,
            DELEGATE_GRANT_DOMAIN,
            "grant_canonical",
            &grant_preimage,
            &grant_sig,
        )],
        "6-field body, implicit version (signing domain carries it); signature travels separately.",
    );

    // terminal_capability_digest as a standalone digest_v1 example.
    b.digest_vector(json!({
        "id": "terminal_capability_digest",
        "algo": "digest_v1_over_message",
        "label_ascii": String::from_utf8(riot_terminal_label().to_vec()).unwrap(),
        "note": "digest_v1 over an opaque terminal-capability canonical byte string",
        "inputs": { "message_hex": hx(&terminal_cap_canonical) },
        "preimage_hex": hx(&digest_v1_preimage(riot_terminal_label(), &terminal_cap_canonical)),
        "value_hex": hx(&terminal_cap_digest),
    }));

    // ---------------------------------------------------------------------
    // 5. CommunityListingV1 (schema token; sorted byte/text sets; optional region)
    //    Two vectors: region present (unsorted sets on input to exercise sorting),
    //    and region null.
    // ---------------------------------------------------------------------
    let topic_tags_unsorted = vec![b"climate".to_vec(), b"housing".to_vec(), b"aid".to_vec()];
    let languages_unsorted = vec!["pt-BR".to_string(), "en".to_string(), "es".to_string()];
    let listing = CommunityListingV1 {
        root_id: root_pk,
        o_namespace_id: o_ns,
        c_namespace_id: c_ns,
        w_namespace_id: w_ns,
        manifest_digest,
        manifest_version: 7,
        ticket_core_bytes: ticket_env_bytes.clone(),
        listing_epoch: 4,
        listing_revision: 1,
        listed: true,
        title: "Riverside Mutual Aid".to_string(),
        summary: "Community wire for the riverside neighbourhood.".to_string(),
        topic_tags: topic_tags_unsorted.clone(),
        languages: languages_unsorted.clone(),
        region: Some(b"us-ca".to_vec()),
        issued_unix_seconds: 1_760_000_000,
        expiry_unix_seconds: 1_760_500_000,
    };
    let listing_bytes = listing.encode_canonical().unwrap();
    let listing_fields = |lst: &CommunityListingV1| -> Value {
        json!({
            "schema": COMMUNITY_LISTING_SCHEMA(),
            "root_id": hx(&lst.root_id),
            "o_namespace_id": hx(&lst.o_namespace_id),
            "c_namespace_id": hx(&lst.c_namespace_id),
            "w_namespace_id": hx(&lst.w_namespace_id),
            "manifest_digest": hx(&lst.manifest_digest),
            "manifest_version": s(lst.manifest_version),
            "ticket_core_bytes": hx(&lst.ticket_core_bytes),
            "listing_epoch": s(lst.listing_epoch as u64),
            "listing_revision": s(lst.listing_revision as u64),
            "listed": lst.listed,
            "title": lst.title,
            "summary": lst.summary,
            "topic_tags": lst.topic_tags.iter().map(|t| Value::String(hx(t))).collect::<Vec<_>>(),
            "languages": lst.languages.iter().map(|l| Value::String(l.clone())).collect::<Vec<_>>(),
            "region": match &lst.region { Some(r) => Value::String(hx(r)), None => Value::Null },
            "issued_unix_seconds": s(lst.issued_unix_seconds),
            "expiry_unix_seconds": s(lst.expiry_unix_seconds),
        })
    };
    b.record(
        "community_listing_region",
        "CommunityListingV1",
        listing_fields(&listing),
        &listing_bytes,
        16_384,
        vec![],
        vec![],
        "explicit schema tstr; topic_tags (byte set) and languages (text set) given UNSORTED to force canonical sorting; region present.",
    );

    let listing_no_region = CommunityListingV1 {
        region: None,
        topic_tags: vec![],
        languages: vec!["en".to_string()],
        listed: false,
        ..listing.clone()
    };
    let listing_no_region_bytes = listing_no_region.encode_canonical().unwrap();
    b.record(
        "community_listing_null_region",
        "CommunityListingV1",
        listing_fields(&listing_no_region),
        &listing_no_region_bytes,
        16_384,
        vec![],
        vec![],
        "region null sentinel; empty topic_tags set; `listed=false` unlisting tombstone.",
    );

    // ---------------------------------------------------------------------
    // 6. AdmittedListingEnvelopeV1 (optional grant; listing_digest = digest_v1)
    //    Two vectors: delegated (grant present) and root-owned (grant null).
    // ---------------------------------------------------------------------
    let admitted_delegated = AdmittedListingEnvelopeV1 {
        signed_listing_entry_bytes: listing_bytes.clone(),
        capability_chain_bytes: vec![0x01, 0x02, 0x03, 0x04],
        delegate_grant_bytes: Some(grant_bytes.clone()),
    };
    let admitted_delegated_bytes = admitted_delegated.encode_canonical().unwrap();
    b.record(
        "admitted_listing_envelope_delegated",
        "AdmittedListingEnvelopeV1",
        json!({
            "signed_listing_entry_bytes": hx(&admitted_delegated.signed_listing_entry_bytes),
            "capability_chain_bytes": hx(&admitted_delegated.capability_chain_bytes),
            "delegate_grant_bytes": Value::String(hx(&grant_bytes)),
        }),
        &admitted_delegated_bytes,
        16_384,
        vec![digest_over_canonical(
            "listing_digest",
            label::ADMITTED_LISTING_ENVELOPE,
            &admitted_delegated_bytes,
        )],
        vec![],
        "delegated (grant present).",
    );

    let admitted_root = AdmittedListingEnvelopeV1 {
        signed_listing_entry_bytes: listing_bytes.clone(),
        capability_chain_bytes: vec![0x05, 0x06],
        delegate_grant_bytes: None,
    };
    let admitted_root_bytes = admitted_root.encode_canonical().unwrap();
    b.record(
        "admitted_listing_envelope_root_owned",
        "AdmittedListingEnvelopeV1",
        json!({
            "signed_listing_entry_bytes": hx(&admitted_root.signed_listing_entry_bytes),
            "capability_chain_bytes": hx(&admitted_root.capability_chain_bytes),
            "delegate_grant_bytes": Value::Null,
        }),
        &admitted_root_bytes,
        16_384,
        vec![digest_over_canonical(
            "listing_digest",
            label::ADMITTED_LISTING_ENVELOPE,
            &admitted_root_bytes,
        )],
        vec![],
        "root-owned (zero delegation → grant null sentinel).",
    );

    // ---------------------------------------------------------------------
    // 7. AnchorLimitProfileV1::mvp_defaults (LimitValue scalar/compound; digest_v1)
    // ---------------------------------------------------------------------
    let profile = AnchorLimitProfileV1::mvp_defaults(1);
    let profile_bytes = profile.encode_canonical().unwrap();
    let profile_digest = profile.limit_profile_digest().unwrap();
    let entries_json: Vec<Value> = profile
        .entries
        .iter()
        .map(|e: &AnchorLimitEntry| {
            json!({
                "id": s(e.id.id()),
                "effective": limit_value_field(e.effective),
                "absolute": limit_value_field(e.absolute),
            })
        })
        .collect();
    b.record(
        "anchor_limit_profile_mvp",
        "AnchorLimitProfileV1",
        json!({ "profile_epoch": s(profile.profile_epoch), "entries": entries_json }),
        &profile_bytes,
        8192,
        vec![digest_over_canonical(
            "limit_profile_digest",
            label::LIMIT_PROFILE,
            &profile_bytes,
        )],
        vec![],
        "all 82 limits, ascending; exercises LimitValue Scalar (u64) and Compound ([first,second]).",
    );

    // ---------------------------------------------------------------------
    // 8. DescriptorFloor
    // ---------------------------------------------------------------------
    let genesis_pk = key_public(&signing_key(21));
    let genesis_rand = b32(0x70);
    let anchor = anchor_id(&genesis_pk, &genesis_rand);
    let descriptor_digest_placeholder = b32(0x80);
    let floor = DescriptorFloor {
        anchor_id: anchor,
        descriptor_epoch: 0,
        descriptor_digest: descriptor_digest_placeholder,
        operator_verification_key: op_key,
    };
    let floor_bytes = floor.encode_canonical().unwrap();
    let floor_fields = |f: &DescriptorFloor| {
        json!({
            "anchor_id": hx(&f.anchor_id),
            "descriptor_epoch": s(f.descriptor_epoch),
            "descriptor_digest": hx(&f.descriptor_digest),
            "operator_verification_key": opkey_fields(&f.operator_verification_key),
        })
    };
    b.record(
        "descriptor_floor",
        "DescriptorFloor",
        floor_fields(&floor),
        &floor_bytes,
        4096,
        vec![],
        vec![],
        "version-scoped 4-tuple, no leading version int; embeds an OperatorVerificationKeyV1.",
    );

    // ---------------------------------------------------------------------
    // 9. AnchorDescriptorBodyV1 + DescriptorEnvelopeV1 (epoch>0: predecessor
    //    signature + null-at-genesis handled by a second body). anchor_id bare
    //    digest; descriptor_digest = digest_v1; current & predecessor signatures.
    // ---------------------------------------------------------------------
    let current_op = opkey(op_pk);
    let pred_op = opkey(pred_pk);
    let body = AnchorDescriptorBodyV1 {
        anchor_id: anchor,
        genesis_operator_public_key: genesis_pk,
        genesis_random_256_bits: genesis_rand,
        current_operator_verification_key: current_op,
        current_operator_key_id: current_op.operator_key_id().unwrap(),
        descriptor_epoch: 5,
        previous_descriptor_digest: Some(b32(0x90)),
        current_iroh_endpoint_id: b32(0xa0),
        https_origin: "https://anchor.example.org".to_string(),
        operator_display_label: "Example Anchor Co-op".to_string(),
        self_reported_failure_domain_label: "eu-west".to_string(),
        supported_control_versions: vec![1, 2],
        supported_sync_versions: vec![2],
        enabled_roles: vec![
            EnabledRole::Mirror,
            EnabledRole::Host,
            EnabledRole::Directory,
            EnabledRole::Gossip,
        ],
        limit_profile_digest: profile_digest,
        predecessor_operator_verification_key: Some(pred_op),
        issued_at: 1_760_000_000,
        expires_at: 1_760_600_000,
    };
    assert_eq!(body.recomputed_anchor_id(), anchor);
    let body_bytes = body.encode_canonical().unwrap();
    let body_fields = |bd: &AnchorDescriptorBodyV1| {
        json!({
            "anchor_id": hx(&bd.anchor_id),
            "genesis_operator_public_key": hx(&bd.genesis_operator_public_key),
            "genesis_random_256_bits": hx(&bd.genesis_random_256_bits),
            "current_operator_verification_key": opkey_fields(&bd.current_operator_verification_key),
            "current_operator_key_id": hx(&bd.current_operator_key_id),
            "descriptor_epoch": s(bd.descriptor_epoch),
            "previous_descriptor_digest": match bd.previous_descriptor_digest { Some(d) => Value::String(hx(&d)), None => Value::Null },
            "current_iroh_endpoint_id": hx(&bd.current_iroh_endpoint_id),
            "https_origin": bd.https_origin,
            "operator_display_label": bd.operator_display_label,
            "self_reported_failure_domain_label": bd.self_reported_failure_domain_label,
            "supported_control_versions": bd.supported_control_versions.iter().map(|v| s(*v)).collect::<Vec<_>>(),
            "supported_sync_versions": bd.supported_sync_versions.iter().map(|v| s(*v)).collect::<Vec<_>>(),
            "enabled_roles": bd.enabled_roles.iter().map(|r| Value::String(r.token().to_string())).collect::<Vec<_>>(),
            "limit_profile_digest": hx(&bd.limit_profile_digest),
            "predecessor_operator_verification_key": match &bd.predecessor_operator_verification_key { Some(k) => opkey_fields(k), None => Value::Null },
            "issued_at": s(bd.issued_at),
            "expires_at": s(bd.expires_at),
        })
    };
    b.record(
        "anchor_descriptor_body_epoch5",
        "AnchorDescriptorBodyV1",
        body_fields(&body),
        &body_bytes,
        8192,
        vec![json!({
            "name": "anchor_id",
            "algo": "anchor_id",
            "label_ascii": String::from_utf8(label::ANCHOR_ID.to_vec()).unwrap(),
            "inputs": { "genesis_operator_public_key": hx(&genesis_pk), "genesis_random_256_bits": hx(&genesis_rand) },
            "preimage_hex": hx(&[label::ANCHOR_ID, &genesis_pk[..], &genesis_rand[..]].concat()),
            "value_hex": hx(&anchor),
        })],
        vec![],
        "19-field body; enabled_roles given UNSORTED to force canonical order host/mirror/directory/gossip; u64 version sets; predecessor key present.",
    );

    // DescriptorEnvelopeV1 (sign current + predecessor).
    let mut current_preimage = label::DESCRIPTOR_SIG.to_vec();
    current_preimage.extend_from_slice(&body_bytes);
    let current_sig = op_sk.sign(&current_preimage).to_bytes();
    let body_hash = blake3::hash(&body_bytes);
    let mut pred_preimage = label::DESCRIPTOR_TRANSITION_SIG.to_vec();
    pred_preimage.extend_from_slice(body_hash.as_bytes());
    let pred_sig = pred_sk.sign(&pred_preimage).to_bytes();
    let envelope = DescriptorEnvelopeV1 {
        body: body.clone(),
        current_signature: current_sig,
        predecessor_signature: Some(pred_sig),
    };
    assert_eq!(
        envelope.current_signing_preimage().unwrap(),
        current_preimage
    );
    assert_eq!(
        envelope.predecessor_signing_preimage().unwrap(),
        pred_preimage
    );
    envelope.verify_current().unwrap();
    let envelope_bytes = envelope.encode_canonical().unwrap();
    let envelope_digest = envelope.descriptor_digest().unwrap();
    b.record(
        "descriptor_envelope_epoch5",
        "DescriptorEnvelopeV1",
        json!({
            "body": body_fields(&body),
            "current_signature": hx(&current_sig),
            "predecessor_signature": Value::String(hx(&pred_sig)),
        }),
        &envelope_bytes,
        8192,
        vec![digest_over_canonical(
            "descriptor_digest",
            label::DESCRIPTOR_ENVELOPE,
            &envelope_bytes,
        )],
        vec![
            signature(
                "current_signature",
                &op_pk,
                label::DESCRIPTOR_SIG,
                "body_canonical",
                &current_preimage,
                &current_sig,
            ),
            signature(
                "predecessor_signature",
                &pred_pk,
                label::DESCRIPTOR_TRANSITION_SIG,
                "blake3(body_canonical)",
                &pred_preimage,
                &pred_sig,
            ),
        ],
        "current sig over DOMAIN||body; predecessor (transition) sig over DOMAIN||BLAKE3(body) — the two signature preimages are deliberately different.",
    );

    // Genesis body (epoch 0): null previous digest + null predecessor key.
    let genesis_body = AnchorDescriptorBodyV1 {
        descriptor_epoch: 0,
        previous_descriptor_digest: None,
        predecessor_operator_verification_key: None,
        ..body.clone()
    };
    let genesis_body_bytes = genesis_body.encode_canonical().unwrap();
    b.record(
        "anchor_descriptor_body_genesis",
        "AnchorDescriptorBodyV1",
        body_fields(&genesis_body),
        &genesis_body_bytes,
        8192,
        vec![],
        vec![],
        "epoch 0 — previous_descriptor_digest and predecessor key are both the null sentinel.",
    );

    // ---------------------------------------------------------------------
    // 10. HostingReceiptBodyV1 + HostingReceiptV1 (HostingStatus; NamespaceResult)
    // ---------------------------------------------------------------------
    let ns_results = vec![
        NamespaceResult {
            namespace_id: o_ns,
            snapshot_digest: b32(0xb0),
            entry_count: 12,
        },
        NamespaceResult {
            namespace_id: c_ns,
            snapshot_digest: b32(0xb4),
            entry_count: 34,
        },
        NamespaceResult {
            namespace_id: w_ns,
            snapshot_digest: b32(0xb8),
            entry_count: 56,
        },
    ];
    let hosting_body = HostingReceiptBodyV1 {
        anchor_id: anchor,
        operator_key_id: op_key_id,
        descriptor_epoch: 5,
        descriptor_digest: envelope_digest,
        hosting_operation_id: b32(0xc0),
        full_site_root: root_pk,
        manifest_digest,
        manifest_version: 7,
        base_site_generation: 2,
        committed_site_generation: 3,
        ordered_namespace_results: ns_results.clone(),
        status: HostingStatus::Committed,
        accepted_at: 1_760_010_000,
        reported_retention_through: 1_790_000_000,
        limit_profile_digest: profile_digest,
    };
    let hosting_body_bytes = hosting_body.encode_canonical().unwrap();
    let ns_result_fields = |r: &NamespaceResult| {
        json!({
            "namespace_id": hx(&r.namespace_id),
            "snapshot_digest": hx(&r.snapshot_digest),
            "entry_count": s(r.entry_count),
        })
    };
    let hosting_body_fields = json!({
        "anchor_id": hx(&hosting_body.anchor_id),
        "operator_key_id": hx(&hosting_body.operator_key_id),
        "descriptor_epoch": s(hosting_body.descriptor_epoch),
        "descriptor_digest": hx(&hosting_body.descriptor_digest),
        "hosting_operation_id": hx(&hosting_body.hosting_operation_id),
        "full_site_root": hx(&hosting_body.full_site_root),
        "manifest_digest": hx(&hosting_body.manifest_digest),
        "manifest_version": s(hosting_body.manifest_version),
        "base_site_generation": s(hosting_body.base_site_generation),
        "committed_site_generation": s(hosting_body.committed_site_generation),
        "ordered_namespace_results": hosting_body.ordered_namespace_results.iter().map(ns_result_fields).collect::<Vec<_>>(),
        "status": "committed",
        "accepted_at": s(hosting_body.accepted_at),
        "reported_retention_through": s(hosting_body.reported_retention_through),
        "limit_profile_digest": hx(&hosting_body.limit_profile_digest),
    });
    b.record(
        "hosting_receipt_body",
        "HostingReceiptBodyV1",
        hosting_body_fields.clone(),
        &hosting_body_bytes,
        4096,
        vec![],
        vec![],
        "16-field receipt body; HostingStatus `committed` sentinel; ordered O/C/W NamespaceResult triple.",
    );

    let mut hosting_preimage = HostingReceiptBodyV1::SIGNING_DOMAIN.to_vec();
    hosting_preimage.extend_from_slice(&hosting_body_bytes);
    let hosting_sig = op_sk.sign(&hosting_preimage).to_bytes();
    let hosting_env: HostingReceiptV1 = OperatorSignedEnvelopeV1 {
        body: hosting_body.clone(),
        operator_signature: hosting_sig,
    };
    assert_eq!(hosting_env.signing_preimage().unwrap(), hosting_preimage);
    hosting_env.verify(&op_pk).unwrap();
    let hosting_env_bytes = hosting_env.encode_canonical().unwrap();
    b.record(
        "hosting_receipt_envelope",
        "HostingReceiptV1",
        json!({ "body": hosting_body_fields, "operator_signature": hx(&hosting_sig) }),
        &hosting_env_bytes,
        4096,
        vec![],
        vec![signature(
            "operator_signature",
            &op_pk,
            HostingReceiptBodyV1::SIGNING_DOMAIN,
            "body_canonical",
            &hosting_preimage,
            &hosting_sig,
        )],
        "OperatorSignedEnvelopeV1<HostingReceiptBodyV1>; [1, body, 64-byte sig].",
    );

    // ---------------------------------------------------------------------
    // 11. ListingReceiptBodyV1 + ListingReceiptV1
    // ---------------------------------------------------------------------
    let listing_receipt_body = ListingReceiptBodyV1 {
        anchor_id: anchor,
        operator_key_id: op_key_id,
        descriptor_epoch: 5,
        descriptor_digest: envelope_digest,
        listing_digest: digest_v1(label::ADMITTED_LISTING_ENVELOPE, &admitted_root_bytes),
        full_site_root: root_pk,
        accepted_listing_epoch: 4,
        accepted_listing_revision: 1,
        feed_coordinate: 42,
        accepted_at: 1_760_010_500,
        expires_at: 1_760_600_000,
        request_idempotency_key: b16(0x01),
    };
    let listing_receipt_body_bytes = listing_receipt_body.encode_canonical().unwrap();
    let listing_receipt_body_fields = json!({
        "anchor_id": hx(&listing_receipt_body.anchor_id),
        "operator_key_id": hx(&listing_receipt_body.operator_key_id),
        "descriptor_epoch": s(listing_receipt_body.descriptor_epoch),
        "descriptor_digest": hx(&listing_receipt_body.descriptor_digest),
        "listing_digest": hx(&listing_receipt_body.listing_digest),
        "full_site_root": hx(&listing_receipt_body.full_site_root),
        "accepted_listing_epoch": s(listing_receipt_body.accepted_listing_epoch as u64),
        "accepted_listing_revision": s(listing_receipt_body.accepted_listing_revision as u64),
        "feed_coordinate": s(listing_receipt_body.feed_coordinate),
        "accepted_at": s(listing_receipt_body.accepted_at),
        "expires_at": s(listing_receipt_body.expires_at),
        "request_idempotency_key": hx(&listing_receipt_body.request_idempotency_key),
    });
    b.record(
        "listing_receipt_body",
        "ListingReceiptBodyV1",
        listing_receipt_body_fields.clone(),
        &listing_receipt_body_bytes,
        4096,
        vec![],
        vec![],
        "13-field receipt body; 128-bit idempotency key is a fixed 16-byte string.",
    );

    let mut listing_receipt_preimage = ListingReceiptBodyV1::SIGNING_DOMAIN.to_vec();
    listing_receipt_preimage.extend_from_slice(&listing_receipt_body_bytes);
    let listing_receipt_sig = op_sk.sign(&listing_receipt_preimage).to_bytes();
    let listing_receipt_env: ListingReceiptV1 = OperatorSignedEnvelopeV1 {
        body: listing_receipt_body.clone(),
        operator_signature: listing_receipt_sig,
    };
    listing_receipt_env.verify(&op_pk).unwrap();
    let listing_receipt_env_bytes = listing_receipt_env.encode_canonical().unwrap();
    let listing_receipt_env_fields = json!({
        "body": listing_receipt_body_fields,
        "operator_signature": hx(&listing_receipt_sig),
    });
    b.record(
        "listing_receipt_envelope",
        "ListingReceiptV1",
        listing_receipt_env_fields.clone(),
        &listing_receipt_env_bytes,
        4096,
        vec![],
        vec![signature(
            "operator_signature",
            &op_pk,
            ListingReceiptBodyV1::SIGNING_DOMAIN,
            "body_canonical",
            &listing_receipt_preimage,
            &listing_receipt_sig,
        )],
        "OperatorSignedEnvelopeV1<ListingReceiptBodyV1>.",
    );

    // ---------------------------------------------------------------------
    // 12. WorkChallengeBodyV1 + WorkChallengeV1 (envelope_digest label) + WorkStampV1
    // ---------------------------------------------------------------------
    let work_body = WorkChallengeBodyV1 {
        anchor_id: anchor,
        operator_key_id: op_key_id,
        descriptor_epoch: 5,
        descriptor_digest: envelope_digest,
        operation_kind: ControlOperationKind::SubmitListing,
        idempotency_key: b16(0x02),
        work_target_digest: b32(0xd0),
        community_root: root_pk,
        random_challenge: b32(0xd8),
        policy_epoch: 9,
        difficulty: 0,
        issued_at: 1_760_000_000,
        expires_at: 1_760_000_300,
    };
    let work_body_bytes = work_body.encode_canonical().unwrap();
    let work_body_fields = json!({
        "anchor_id": hx(&work_body.anchor_id),
        "operator_key_id": hx(&work_body.operator_key_id),
        "descriptor_epoch": s(work_body.descriptor_epoch),
        "descriptor_digest": hx(&work_body.descriptor_digest),
        "operation_kind": work_body.operation_kind.token(),
        "idempotency_key": hx(&work_body.idempotency_key),
        "work_target_digest": hx(&work_body.work_target_digest),
        "community_root": hx(&work_body.community_root),
        "random_challenge": hx(&work_body.random_challenge),
        "policy_epoch": s(work_body.policy_epoch),
        "difficulty": s(work_body.difficulty),
        "issued_at": s(work_body.issued_at),
        "expires_at": s(work_body.expires_at),
    });
    b.record(
        "work_challenge_body",
        "WorkChallengeBodyV1",
        work_body_fields.clone(),
        &work_body_bytes,
        4096,
        vec![],
        vec![],
        "14-field challenge body; ControlOperationKind `submit_listing` token; difficulty 0.",
    );

    let mut work_preimage = WorkChallengeBodyV1::SIGNING_DOMAIN.to_vec();
    work_preimage.extend_from_slice(&work_body_bytes);
    let work_sig = op_sk.sign(&work_preimage).to_bytes();
    let work_env: WorkChallengeV1 = OperatorSignedEnvelopeV1 {
        body: work_body.clone(),
        operator_signature: work_sig,
    };
    work_env.verify(&op_pk).unwrap();
    let work_env_bytes = work_env.encode_canonical().unwrap();
    let work_env_digest = digest_v1(label::WORK_CHALLENGE_ENVELOPE, &work_env_bytes);
    b.record(
        "work_challenge_envelope",
        "WorkChallengeV1",
        json!({ "body": work_body_fields, "operator_signature": hx(&work_sig) }),
        &work_env_bytes,
        4096,
        vec![digest_over_canonical(
            "work_challenge_digest",
            label::WORK_CHALLENGE_ENVELOPE,
            &work_env_bytes,
        )],
        vec![signature(
            "operator_signature",
            &op_pk,
            WorkChallengeBodyV1::SIGNING_DOMAIN,
            "body_canonical",
            &work_preimage,
            &work_sig,
        )],
        "OperatorSignedEnvelopeV1<WorkChallengeBodyV1>; envelope_digest uses the work-challenge-envelope label.",
    );

    // WorkStampV1: proof over the challenge envelope. difficulty 0 → counter 0 valid.
    let counter = 0u64;
    let proof = work_proof(&work_env_digest, counter);
    let work_stamp = WorkStampV1 {
        challenge_envelope_bytes: work_env_bytes.clone(),
        counter,
        proof_bytes: proof,
    };
    work_stamp.verify(&op_pk).unwrap();
    let work_stamp_bytes = work_stamp.encode_canonical().unwrap();
    b.record(
        "work_stamp",
        "WorkStampV1",
        json!({
            "challenge_envelope_bytes": hx(&work_stamp.challenge_envelope_bytes),
            "counter": s(work_stamp.counter),
            "proof_bytes": hx(&work_stamp.proof_bytes),
        }),
        &work_stamp_bytes,
        4096,
        vec![json!({
            "name": "work_proof",
            "algo": "work_proof",
            "label_ascii": String::from_utf8(label::WORK_PROOF.to_vec()).unwrap(),
            "inputs": { "work_challenge_digest": hx(&work_env_digest), "counter": s(counter) },
            "preimage_hex": hx(&[label::WORK_PROOF, &work_env_digest[..], &counter.to_be_bytes()].concat()),
            "value_hex": hx(&proof),
        })],
        vec![],
        "proof = BLAKE3(domain || work_challenge_digest || u64be(counter)); bare-label preimage.",
    );

    // ---------------------------------------------------------------------
    // 13. ReplicaPrepareChallengeV1
    // ---------------------------------------------------------------------
    let replica_challenge = ReplicaPrepareChallengeV1 {
        destination_anchor_id: b32(0xe0),
        random_256_bit_nonce: b32(0xe4),
        prepare_idempotency_key: b16(0x03),
        full_site_root: root_pk,
        issued_at: 1_760_000_000,
        expires_at: 1_760_000_060,
    };
    let replica_challenge_bytes = replica_challenge.encode_canonical().unwrap();
    b.record(
        "replica_prepare_challenge",
        "ReplicaPrepareChallengeV1",
        json!({
            "destination_anchor_id": hx(&replica_challenge.destination_anchor_id),
            "random_256_bit_nonce": hx(&replica_challenge.random_256_bit_nonce),
            "prepare_idempotency_key": hx(&replica_challenge.prepare_idempotency_key),
            "full_site_root": hx(&replica_challenge.full_site_root),
            "issued_at": s(replica_challenge.issued_at),
            "expires_at": s(replica_challenge.expires_at),
        }),
        &replica_challenge_bytes,
        4096,
        vec![],
        vec![],
        "7-field prepare challenge (leading version int).",
    );

    // ---------------------------------------------------------------------
    // 14. ReplicaSourceAttestationBodyV1 + ReplicaSourceAttestationV1
    // ---------------------------------------------------------------------
    let attestation_body = ReplicaSourceAttestationBodyV1 {
        source_anchor_id: anchor,
        source_current_operator_key_id: op_key_id,
        source_current_descriptor_epoch: 5,
        source_current_descriptor_digest: envelope_digest,
        destination_anchor_id: b32(0xe0),
        peer_transcript_digest: b32(0xf0),
        destination_prepare_nonce: b32(0xe4),
        prepare_idempotency_key: b16(0x03),
        full_site_root: root_pk,
        manifest_digest,
        manifest_version: 7,
        root_signed_ticket_core_digest: ticket_env_digest,
        source_site_generation: 3,
        ordered_namespace_snapshot_digests: [b32(0xb0), b32(0xb4), b32(0xb8)],
        issued_at: 1_760_000_000,
        expires_at: 1_760_000_300,
    };
    let attestation_body_bytes = attestation_body.encode_canonical().unwrap();
    let attestation_body_fields = json!({
        "source_anchor_id": hx(&attestation_body.source_anchor_id),
        "source_current_operator_key_id": hx(&attestation_body.source_current_operator_key_id),
        "source_current_descriptor_epoch": s(attestation_body.source_current_descriptor_epoch),
        "source_current_descriptor_digest": hx(&attestation_body.source_current_descriptor_digest),
        "destination_anchor_id": hx(&attestation_body.destination_anchor_id),
        "peer_transcript_digest": hx(&attestation_body.peer_transcript_digest),
        "destination_prepare_nonce": hx(&attestation_body.destination_prepare_nonce),
        "prepare_idempotency_key": hx(&attestation_body.prepare_idempotency_key),
        "full_site_root": hx(&attestation_body.full_site_root),
        "manifest_digest": hx(&attestation_body.manifest_digest),
        "manifest_version": s(attestation_body.manifest_version),
        "root_signed_ticket_core_digest": hx(&attestation_body.root_signed_ticket_core_digest),
        "source_site_generation": s(attestation_body.source_site_generation),
        "ordered_namespace_snapshot_digests": attestation_body.ordered_namespace_snapshot_digests.iter().map(|d| Value::String(hx(d))).collect::<Vec<_>>(),
        "issued_at": s(attestation_body.issued_at),
        "expires_at": s(attestation_body.expires_at),
    });
    b.record(
        "replica_source_attestation_body",
        "ReplicaSourceAttestationBodyV1",
        attestation_body_fields.clone(),
        &attestation_body_bytes,
        4096,
        vec![],
        vec![],
        "16-field body; manifest_digest_and_version is the nested [digest, version]; ordered O/C/W snapshot triple.",
    );

    let mut attestation_preimage = ReplicaSourceAttestationBodyV1::SIGNING_DOMAIN.to_vec();
    attestation_preimage.extend_from_slice(&attestation_body_bytes);
    let attestation_sig = op_sk.sign(&attestation_preimage).to_bytes();
    let attestation_env: ReplicaSourceAttestationV1 = OperatorSignedEnvelopeV1 {
        body: attestation_body.clone(),
        operator_signature: attestation_sig,
    };
    attestation_env.verify(&op_pk).unwrap();
    let attestation_env_bytes = attestation_env.encode_canonical().unwrap();
    b.record(
        "replica_source_attestation_envelope",
        "ReplicaSourceAttestationV1",
        json!({ "body": attestation_body_fields, "operator_signature": hx(&attestation_sig) }),
        &attestation_env_bytes,
        4096,
        vec![digest_over_canonical(
            "replica_source_attestation_digest",
            label::REPLICA_SOURCE_ATTESTATION_ENVELOPE,
            &attestation_env_bytes,
        )],
        vec![signature(
            "operator_signature",
            &op_pk,
            ReplicaSourceAttestationBodyV1::SIGNING_DOMAIN,
            "body_canonical",
            &attestation_preimage,
            &attestation_sig,
        )],
        "OperatorSignedEnvelopeV1<ReplicaSourceAttestationBodyV1>.",
    );

    // ---------------------------------------------------------------------
    // 15. SnapshotCursorBodyV1 (+ HMAC input) + SnapshotCursorV1
    // ---------------------------------------------------------------------
    let cursor_body = SnapshotCursorBodyV1 {
        checkpoint_digest: b32(0x11),
        snapshot_generation_id: 8,
        next_ordinal: 5,
        previous_root: Some(b32(0x22)),
        issued_at: 1_760_000_000,
        expires_at: 1_760_003_600,
        cursor_secret_epoch: 2,
    };
    let cursor_body_bytes = cursor_body.encode_canonical().unwrap();
    let cursor_hmac_input = cursor_body.cursor_tag_hmac_input().unwrap();
    assert_eq!(
        cursor_hmac_input,
        snapshot_cursor_hmac_input(&cursor_body_bytes)
    );
    let cursor_body_fields = json!({
        "checkpoint_digest": hx(&cursor_body.checkpoint_digest),
        "snapshot_generation_id": s(cursor_body.snapshot_generation_id),
        "next_ordinal": s(cursor_body.next_ordinal),
        "previous_root": match cursor_body.previous_root { Some(r) => Value::String(hx(&r)), None => Value::Null },
        "issued_at": s(cursor_body.issued_at),
        "expires_at": s(cursor_body.expires_at),
        "cursor_secret_epoch": s(cursor_body.cursor_secret_epoch as u64),
    });
    b.record(
        "snapshot_cursor_body",
        "SnapshotCursorBodyV1",
        cursor_body_fields.clone(),
        &cursor_body_bytes,
        4096,
        vec![json!({
            "name": "snapshot_cursor_hmac_input",
            "algo": "snapshot_cursor_hmac_input",
            "label_ascii": String::from_utf8(label::DIRECTORY_SNAPSHOT_CURSOR.to_vec()).unwrap(),
            "message": "canonical",
            "preimage_hex": hx(&cursor_hmac_input),
            "note": "HMAC-SHA256 INPUT bytes only (u16be(33)||label||u64be(len)||canonical); the keyed MAC lives with the anchor secret.",
        })],
        vec![],
        "8-field cursor body; previous_root present.",
    );

    let cursor = SnapshotCursorV1 {
        body: cursor_body.clone(),
        cursor_tag: b32(0x33),
    };
    let cursor_bytes = cursor.encode_canonical().unwrap();
    b.record(
        "snapshot_cursor",
        "SnapshotCursorV1",
        json!({ "body": cursor_body_fields, "cursor_tag": hx(&cursor.cursor_tag) }),
        &cursor_bytes,
        4096,
        vec![],
        vec![],
        "[1, SnapshotCursorBodyV1, 32-byte cursor tag]; the tag is opaque at this layer.",
    );

    // ---------------------------------------------------------------------
    // 16. BootstrapDescriptorV1 x3 + AnchorBootstrapV1 (development set)
    // ---------------------------------------------------------------------
    let bootstrap = development_bootstrap(&profile_digest);
    assert!(
        bootstrap.meets_diversity_floor(),
        "development bootstrap must still meet the >=3 descriptors / >=2 operators structural floor"
    );
    let bootstrap_bytes = bootstrap.encode_canonical().unwrap();
    let bootstrap_fields = json!({
        "descriptors": bootstrap.descriptors.iter().map(|d| json!({
            "floor": floor_fields(&d.floor),
            "https_origin": d.https_origin,
            "roles": d.roles.iter().map(|r| Value::String(r.token().to_string())).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    b.record(
        "anchor_bootstrap_development",
        "AnchorBootstrapV1",
        bootstrap_fields,
        &bootstrap_bytes,
        65_536,
        vec![],
        vec![],
        "DEVELOPMENT-ONLY three-descriptor bootstrap (origins are *.dev.invalid); meets the structural diversity floor but is not a public-pilot default set — release validation must refuse it.",
    );

    // ---------------------------------------------------------------------
    // 17. ControlRequestV1 (Describe) + control_request_digest / work_target_digest
    // ---------------------------------------------------------------------
    let describe = ControlRequestV1 {
        idempotency_key: b16(0x04),
        operation: ControlOperation::Describe(DescribeV1),
    };
    let describe_bytes = describe.encode_canonical().unwrap();
    let control_request_digest = describe.operation.control_request_digest().unwrap();
    let work_target_digest = describe.operation.work_target_digest().unwrap();
    let control_digest_body_with = describe.operation.control_digest_body(true).unwrap();
    let control_digest_body_without = describe.operation.control_digest_body(false).unwrap();
    b.record(
        "control_request_describe",
        "ControlRequestV1",
        json!({
            "operation_kind": "describe",
            "idempotency_key": hx(&describe.idempotency_key),
            "semantic": { "kind": "describe" },
        }),
        &describe_bytes,
        8192,
        vec![
            json!({
                "name": "control_request_digest",
                "algo": "digest_v1",
                "label_ascii": String::from_utf8(label::CONTROL_REQUEST_BODY.to_vec()).unwrap(),
                "message": "control_digest_body_with_work_stamp",
                "message_hex": hx(&control_digest_body_with),
                "preimage_hex": hx(&digest_v1_preimage(label::CONTROL_REQUEST_BODY, &control_digest_body_with)),
                "value_hex": hx(&control_request_digest),
            }),
            json!({
                "name": "work_target_digest",
                "algo": "digest_v1",
                "label_ascii": String::from_utf8(label::WORK_TARGET.to_vec()).unwrap(),
                "message": "control_digest_body_null_work_stamp",
                "message_hex": hx(&control_digest_body_without),
                "preimage_hex": hx(&digest_v1_preimage(label::WORK_TARGET, &control_digest_body_without)),
                "value_hex": hx(&work_target_digest),
            }),
        ],
        vec![],
        "ControlRequestV1 = [1, kind, idempotency_key, semantic_body]; control_request_digest and work_target_digest are the SAME ControlDigestBodyV1 under two different labels (describe carries no work stamp).",
    );

    // ---------------------------------------------------------------------
    // 18. ControlResponseV1 — refusal nesting (NotHosted) and success nesting.
    // ---------------------------------------------------------------------
    let refusal = ControlResponseV1 {
        kind: ControlOperationKind::CommitHost,
        outcome: ControlOutcome::Refused(ControlRefusal::NotHosted),
    };
    let refusal_bytes = refusal.encode_canonical().unwrap();
    b.record(
        "control_response_refused_not_hosted",
        "ControlResponseV1",
        json!({
            "kind": "commit_host",
            "outcome": { "type": "refused", "refusal": { "code": "not_hosted" } },
        }),
        &refusal_bytes,
        8192,
        vec![],
        vec![],
        "[1, kind, [\"refused\", refusal]]; NotHosted derives subject `listing`, retryable true, retry_after null, details [\"none\"].",
    );

    let success = ControlResponseV1 {
        kind: ControlOperationKind::SubmitListing,
        outcome: ControlOutcome::Success(ControlSuccess::SubmitListing(Box::new(
            listing_receipt_env.clone(),
        ))),
    };
    let success_bytes = success.encode_canonical().unwrap();
    b.record(
        "control_response_success_submit_listing",
        "ControlResponseV1",
        json!({
            "kind": "submit_listing",
            "outcome": {
                "type": "success",
                "success": { "kind": "submit_listing", "listing_receipt": listing_receipt_env_fields },
            },
        }),
        &success_bytes,
        8192,
        vec![],
        vec![],
        "[1, kind, [\"success\", [1, listing_receipt]]]; the success payload embeds the ListingReceiptV1 canonical bytes.",
    );

    // ---------------------------------------------------------------------
    // Standalone digest / preimage vectors.
    // ---------------------------------------------------------------------
    // sync_snapshot_digest (length-prefixed fields + sorted entry ids).
    let ns_id = o_ns;
    let e1 = b32(0x01);
    let e2 = b32(0x02);
    let e3 = b32(0x03);
    let mut sorted_ids = [e1.to_vec(), e3.to_vec(), e2.to_vec()];
    sorted_ids.sort();
    let sorted_refs: Vec<&[u8]> = sorted_ids.iter().map(|v| v.as_slice()).collect();
    let snap = sync_snapshot_digest(&ns_id, 3, 4096, &sorted_refs);
    b.digest_vector(json!({
        "id": "sync_snapshot_digest",
        "algo": "sync_snapshot",
        "label_ascii": String::from_utf8(label::SYNC_SNAPSHOT.to_vec()).unwrap(),
        "inputs": {
            "namespace_id": hx(&ns_id),
            "entry_count": s(3),
            "logical_bytes": s(4096),
            "sorted_entry_ids": sorted_ids.iter().map(|v| Value::String(hx(v))).collect::<Vec<_>>(),
        },
        "value_hex": hx(&snap),
        "note": "BLAKE3(label || u32be(len(ns)) || ns || u64be(count) || u64be(logical) || for each id: u32be(len)||id). entry_ids are pre-sorted.",
    }));

    // namespace_token_hmac_input (input bytes only; hardcoded u16be(23)).
    let nt_op = b16(0x05);
    let nt_input = namespace_token_hmac_input(&nt_op, &ns_id, 1_760_000_900, 4);
    b.digest_vector(json!({
        "id": "namespace_token_hmac_input",
        "algo": "namespace_token_hmac_input",
        "label_ascii": String::from_utf8(label::NAMESPACE_TOKEN.to_vec()).unwrap(),
        "inputs": {
            "operation_id": hx(&nt_op),
            "namespace_id": hx(&ns_id),
            "operation_expiry_unix_seconds": s(1_760_000_900),
            "token_secret_epoch": s(4),
        },
        "preimage_hex": hx(&nt_input),
        "note": "HMAC-SHA256 INPUT ONLY: u16be(23)||label||u16be(len(op))||op||u16be(len(ns))||ns||u64be(expiry)||u32be(epoch).",
    }));

    // peer_proof_signature_preimage for BOTH peer roles (+ a signature over one).
    let transcript_digest = b32(0xf0);
    for role in ["initiator", "responder"] {
        let preimage = peer_proof_signature_preimage(role.as_bytes(), &transcript_digest);
        let sig = op_sk.sign(&preimage).to_bytes();
        b.digest_vector(json!({
            "id": format!("peer_proof_preimage_{role}"),
            "algo": "peer_proof_preimage",
            "label_ascii": String::from_utf8(label::PEER_PROOF.to_vec()).unwrap(),
            "inputs": { "role": role, "peer_transcript_digest": hx(&transcript_digest) },
            "preimage_hex": hx(&preimage),
            "signature": {
                "public_key_hex": hx(&op_pk),
                "signature_hex": hx(&sig),
            },
            "note": "u16be(25)||label||u16be(len(role))||role||peer_transcript_digest; role is the exact lowercase ASCII initiator/responder.",
        }));
    }

    // ---------------------------------------------------------------------
    // Alternate-grammar rejection vectors (proven non-canonical by the crate).
    // ---------------------------------------------------------------------
    // Non-minimal version integer inside OperatorVerificationKeyV1.
    let mut nonminimal = vec![0x83]; // array(3)
    nonminimal.extend_from_slice(&[0x18, 0x01]); // uint 1 as a two-byte non-minimal encoding
    nonminimal.extend_from_slice(&op_key_bytes[2..]); // skip original header+version: "ed25519" + pubkey
    b.grammar_reject(
        "non-minimal version integer",
        "OperatorVerificationKeyV1",
        &nonminimal,
        CodecError::NonCanonical,
    );

    // Indefinite-length outer array.
    let mut indefinite = vec![0x9f]; // array(*)
    indefinite.extend_from_slice(&op_key_bytes[1..]);
    indefinite.push(0xff); // break
    b.grammar_reject(
        "indefinite-length array",
        "OperatorVerificationKeyV1",
        &indefinite,
        CodecError::IndefiniteLength,
    );

    // Trailing bytes after a complete record.
    let mut trailing = op_key_bytes.clone();
    trailing.push(0x00);
    b.grammar_reject(
        "trailing bytes",
        "OperatorVerificationKeyV1",
        &trailing,
        CodecError::TrailingBytes,
    );

    // Map where an array is required (protocol never uses maps).
    let map_bytes = vec![0xa1, 0x01, 0x02];
    b.grammar_reject(
        "map where array required",
        "OperatorVerificationKeyV1",
        &map_bytes,
        CodecError::UnexpectedType,
    );

    // Unsorted byte-set (topic_tags) in a CommunityListingV1: re-encode a valid
    // listing but swap two topic tags out of canonical order.
    let unsorted = reorder_topic_tags(&listing_bytes);
    b.grammar_reject(
        "unsorted topic_tags set",
        "CommunityListingV1",
        &unsorted,
        CodecError::UnsortedSet,
    );

    // ---------------------------------------------------------------------
    // Raw BLAKE3 known-answer vectors from the reference `blake3` crate, so the
    // vendored TS BLAKE3 is validated across chunk boundaries (multi-block,
    // single-chunk-1024, and multi-chunk tree) against the reference — not just
    // memorised constants. Input length `n` is the pattern byte `i % 251`.
    let blake3_kats: Vec<Value> = [0usize, 1, 64, 1023, 1024, 1025, 3072, 4097]
        .into_iter()
        .map(|n| {
            let input: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
            json!({ "input_len": s(n as u64), "hash_hex": hx(blake3::hash(&input).as_bytes()) })
        })
        .collect();

    // ---------------------------------------------------------------------
    // Assemble the fixture document.
    // ---------------------------------------------------------------------
    let doc = json!({
        "protocol": "riot-anchor-protocol",
        "format_version": 1,
        "source_of_truth": "riot-anchor-protocol Rust crate (WU-002..005); regenerate via RIOT_BLESS_ANCHOR_VECTORS=1 cargo test -p riot-anchor-protocol --test golden_vectors",
        "encoding_conventions": {
            "integers": "every wire integer (u16/u32/u64) crosses the JSON boundary as a decimal STRING; encode minimally by numeric value",
            "bytes": "byte strings and fixed-width fields are lowercase hex strings",
            "text": "text strings are JSON strings, encoded as CBOR major type 3",
            "optional": "null is the CBOR null sentinel (0xf6); otherwise the typed value",
            "enum": "closed enums are their exact snake_case wire token",
            "set": "sets are given possibly-UNSORTED; the encoder sorts by canonical element bytes and rejects duplicates",
            "nested": "nested records are objects encoded in place (embedded CBOR value, not double-encoded bytes)"
        },
        "sentinels": {
            "transport_floor": ["require_none", "require_arti"],
            "enabled_role": ["directory", "gossip", "host", "mirror"],
            "control_operation_kind": [
                "describe", "get_work_challenge", "prepare_host", "commit_host",
                "submit_listing", "prepare_replica", "pull_directory_feed",
                "pull_directory_snapshot", "get_operation"
            ],
            "hosting_status": ["committed"],
            "control_outcome": ["success", "refused"],
            "version_tags": {
                "root_signed_ticket_core_envelope": 2,
                "generic_operator_signed_envelope": 1
            }
        },
        "vectors": b.records,
        "digest_vectors": b.digests,
        "alternate_grammar": b.grammar,
        "blake3_kats": blake3_kats,
    });

    (doc, bootstrap_bytes)
}

// A couple of small const-accessor shims (constants not re-exported at the crate root).
#[allow(non_snake_case)]
fn COMMUNITY_LISTING_SCHEMA() -> &'static str {
    riot_anchor_protocol::records::COMMUNITY_LISTING_SCHEMA
}

fn riot_terminal_label() -> &'static [u8] {
    b"riot/listing-terminal-capability/v1"
}

/// Build the deterministic development bootstrap: three descriptors across two
/// operators/failure domains, all with visibly non-routable `.dev.invalid` origins.
fn development_bootstrap(profile_digest: &[u8; 32]) -> AnchorBootstrapV1 {
    let make = |op_seed: u8, anchor_tag: u8, origin: &str, roles: Vec<EnabledRole>| {
        let op = opkey(key_public(&signing_key(op_seed)));
        BootstrapDescriptorV1 {
            floor: DescriptorFloor {
                anchor_id: b32(anchor_tag),
                descriptor_epoch: 0,
                descriptor_digest: *profile_digest,
                operator_verification_key: op,
            },
            https_origin: origin.to_string(),
            roles,
        }
    };
    AnchorBootstrapV1 {
        descriptors: vec![
            make(
                101,
                0x01,
                "https://alpha.dev.invalid",
                vec![EnabledRole::Host, EnabledRole::Directory],
            ),
            make(
                101,
                0x02,
                "https://alpha-2.dev.invalid",
                vec![EnabledRole::Host, EnabledRole::Mirror],
            ),
            make(
                102,
                0x03,
                "https://beta.dev.invalid",
                vec![EnabledRole::Host, EnabledRole::Gossip],
            ),
        ],
    }
}

/// Given a canonical CommunityListingV1 with >=2 topic tags, produce a byte-for-byte
/// copy whose topic-tag set is emitted in a non-ascending order (an alternate grammar
/// the decoder must reject with `UnsortedSet`). We rebuild from the decoded record by
/// hand-encoding so only the set order changes.
fn reorder_topic_tags(listing_bytes: &[u8]) -> Vec<u8> {
    use minicbor::Encoder;
    let listing: CommunityListingV1 =
        decode_canonical(listing_bytes, 16_384).expect("valid listing");
    assert!(
        listing.topic_tags.len() >= 2,
        "need >=2 topic tags to reorder"
    );
    // Encode canonically, then locate the topic-tag set and swap its first two
    // elements. Simplest robust approach: re-encode the whole record but write the
    // topic-tag set in reverse (sorted-descending) order, everything else canonical.
    let mut sorted: Vec<Vec<u8>> = listing
        .topic_tags
        .iter()
        .map(|t| {
            let mut item = Vec::new();
            Encoder::new(&mut item).bytes(t).unwrap();
            item
        })
        .collect();
    sorted.sort();
    sorted.reverse(); // now strictly descending → UnsortedSet on decode

    // Rebuild the full record bytes: reuse the canonical prefix up to the topic-tag
    // array, then emit descending tags, then the canonical suffix. To avoid brittle
    // offset math, re-encode field-by-field mirroring the record layout.
    let mut buf = Vec::new();
    {
        let mut e = Encoder::new(&mut buf);
        e.array(18).unwrap();
        e.str(COMMUNITY_LISTING_SCHEMA()).unwrap();
        e.bytes(&listing.root_id).unwrap();
        e.bytes(&listing.o_namespace_id).unwrap();
        e.bytes(&listing.c_namespace_id).unwrap();
        e.bytes(&listing.w_namespace_id).unwrap();
        e.bytes(&listing.manifest_digest).unwrap();
        e.u64(listing.manifest_version).unwrap();
        e.bytes(&listing.ticket_core_bytes).unwrap();
        e.u32(listing.listing_epoch).unwrap();
        e.u32(listing.listing_revision).unwrap();
        e.bool(listing.listed).unwrap();
        e.str(&listing.title).unwrap();
        e.str(&listing.summary).unwrap();
        e.array(sorted.len() as u64).unwrap();
    }
    for item in &sorted {
        buf.extend_from_slice(item);
    }
    // languages set (canonical order)
    let mut langs: Vec<Vec<u8>> = listing
        .languages
        .iter()
        .map(|l| {
            let mut item = Vec::new();
            Encoder::new(&mut item).str(l).unwrap();
            item
        })
        .collect();
    langs.sort();
    {
        let mut e = Encoder::new(&mut buf);
        e.array(langs.len() as u64).unwrap();
    }
    for item in &langs {
        buf.extend_from_slice(item);
    }
    {
        let mut e = Encoder::new(&mut buf);
        match &listing.region {
            Some(r) => e.bytes(r).unwrap(),
            None => e.null().unwrap(),
        };
        e.u64(listing.issued_unix_seconds).unwrap();
        e.u64(listing.expiry_unix_seconds).unwrap();
    }
    buf
}

// ===========================================================================
// The blessing / verification test.
// ===========================================================================

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn vectors_path() -> PathBuf {
    repo_root().join("fixtures/anchor/protocol-v1-vectors.json")
}

fn bootstrap_path() -> PathBuf {
    repo_root().join("fixtures/anchor/bootstrap-development-v1.cbor")
}

#[test]
fn golden_vectors_emit_and_consume() {
    let (doc, bootstrap_bytes) = build_vectors();
    let json_text = format!("{}\n", serde_json::to_string_pretty(&doc).unwrap());

    let bless = std::env::var_os("RIOT_BLESS_ANCHOR_VECTORS").is_some();
    if bless {
        std::fs::write(vectors_path(), &json_text).unwrap();
        std::fs::write(bootstrap_path(), &bootstrap_bytes).unwrap();
    }

    // EMIT contract: the checked-in files must be byte-identical to a fresh
    // regeneration (a stale fixture fails here — no silent drift).
    let on_disk = std::fs::read_to_string(vectors_path()).unwrap_or_else(|_| {
        panic!(
            "missing {} — run with RIOT_BLESS_ANCHOR_VECTORS=1 to generate",
            vectors_path().display()
        )
    });
    assert_eq!(
        on_disk, json_text,
        "protocol-v1-vectors.json is stale; regenerate with RIOT_BLESS_ANCHOR_VECTORS=1"
    );
    let on_disk_cbor = std::fs::read(bootstrap_path()).unwrap();
    assert_eq!(
        on_disk_cbor, bootstrap_bytes,
        "bootstrap-development-v1.cbor is stale; regenerate with RIOT_BLESS_ANCHOR_VECTORS=1"
    );

    consume(&doc, &bootstrap_bytes);
}

/// CONSUME: independently re-verify every vector the way a fresh reader would.
fn consume(doc: &Value, bootstrap_bytes: &[u8]) {
    // Every record vector decodes canonically and re-encodes to the same bytes.
    for v in doc["vectors"].as_array().unwrap() {
        let hex = v["canonical_hex"].as_str().unwrap();
        let bytes = unhex(hex);
        let record = v["record"].as_str().unwrap();
        let max = v["max_decode_bytes"]
            .as_str()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        // Decode + re-encode round trip through the crate for the record types with
        // a top-level canonical wrapper (all of them).
        roundtrip(record, &bytes, max);

        // Recompute every attached digest_v1-over-canonical.
        if let Some(digs) = v["digests"].as_array() {
            for d in digs {
                if d["algo"] == "digest_v1" && d["message"] == "canonical" {
                    let label_ascii = d["label_ascii"].as_str().unwrap().as_bytes();
                    let recomputed = digest_v1(label_ascii, &bytes);
                    assert_eq!(
                        hx(&recomputed),
                        d["value_hex"].as_str().unwrap(),
                        "digest mismatch"
                    );
                }
            }
        }
        // Verify every attached signature.
        if let Some(sigs) = v["signatures"].as_array() {
            for sgn in sigs {
                let pk = to_arr32(&unhex(sgn["public_key_hex"].as_str().unwrap()));
                let preimage = unhex(sgn["preimage_hex"].as_str().unwrap());
                let sig = to_arr64(&unhex(sgn["signature_hex"].as_str().unwrap()));
                assert!(
                    ed_verify(&pk, &preimage, &sig),
                    "signature {} failed to verify",
                    sgn["name"]
                );
                // A one-bit mutation of the preimage must fail.
                let mut tampered = preimage.clone();
                tampered[0] ^= 0x01;
                assert!(
                    !ed_verify(&pk, &tampered, &sig),
                    "tampered signature verified"
                );
            }
        }
    }

    // The development bootstrap parses but is NOT release-eligible.
    let bootstrap: AnchorBootstrapV1 = decode_canonical(bootstrap_bytes, 65_536).unwrap();
    assert!(bootstrap.meets_diversity_floor());
    assert!(
        !is_release_eligible(&bootstrap),
        "development bootstrap must be refused by release validation"
    );

    // Alternate-grammar hostile encodings are rejected (already asserted at build
    // time; re-assert on the serialised doc so a hand-edited fixture is caught too).
    for g in doc["alternate_grammar"].as_array().unwrap() {
        let hostile = unhex(g["hostile_hex"].as_str().unwrap());
        let record = g["record"].as_str().unwrap();
        assert!(
            decode_record(record, &hostile).is_err(),
            "alt-grammar '{}' unexpectedly decoded",
            g["desc"]
        );
    }
}

/// Release validation: a public-pilot default set must be package-signed and use
/// real routable origins. The development set is deliberately marked with
/// `.dev.invalid` origins, so this returns false — the release build refuses it.
fn is_release_eligible(bootstrap: &AnchorBootstrapV1) -> bool {
    bootstrap.meets_diversity_floor()
        && bootstrap
            .descriptors
            .iter()
            .all(|d| !d.https_origin.contains(".dev.invalid"))
}

fn roundtrip(record: &str, bytes: &[u8], max: usize) {
    macro_rules! rt {
        ($ty:ty) => {{
            let decoded: $ty = decode_canonical(bytes, max)
                .unwrap_or_else(|e| panic!("{record} failed canonical decode: {e:?}"));
            assert_eq!(
                decoded.encode_canonical().unwrap(),
                bytes,
                "{record} re-encode mismatch"
            );
        }};
    }
    match record {
        "OperatorVerificationKeyV1" => rt!(OperatorVerificationKeyV1),
        "PublicSiteTicketV2Core" => rt!(PublicSiteTicketV2Core),
        "RootSignedTicketCoreEnvelopeV2" => rt!(RootSignedTicketCoreEnvelopeV2),
        "ListingDelegateGrantV1" => rt!(ListingDelegateGrantV1),
        "CommunityListingV1" => rt!(CommunityListingV1),
        "AdmittedListingEnvelopeV1" => rt!(AdmittedListingEnvelopeV1),
        "AnchorLimitProfileV1" => rt!(AnchorLimitProfileV1),
        "DescriptorFloor" => rt!(DescriptorFloor),
        "AnchorDescriptorBodyV1" => rt!(AnchorDescriptorBodyV1),
        "DescriptorEnvelopeV1" => rt!(DescriptorEnvelopeV1),
        "HostingReceiptBodyV1" => rt!(HostingReceiptBodyV1),
        "HostingReceiptV1" => rt!(HostingReceiptV1),
        "ListingReceiptBodyV1" => rt!(ListingReceiptBodyV1),
        "ListingReceiptV1" => rt!(ListingReceiptV1),
        "WorkChallengeBodyV1" => rt!(WorkChallengeBodyV1),
        "WorkChallengeV1" => rt!(WorkChallengeV1),
        "WorkStampV1" => rt!(WorkStampV1),
        "ReplicaPrepareChallengeV1" => rt!(ReplicaPrepareChallengeV1),
        "ReplicaSourceAttestationBodyV1" => rt!(ReplicaSourceAttestationBodyV1),
        "ReplicaSourceAttestationV1" => rt!(ReplicaSourceAttestationV1),
        "SnapshotCursorBodyV1" => rt!(SnapshotCursorBodyV1),
        "SnapshotCursorV1" => rt!(SnapshotCursorV1),
        "AnchorBootstrapV1" => rt!(AnchorBootstrapV1),
        "ControlRequestV1" => rt!(ControlRequestV1),
        "ControlResponseV1" => rt!(ControlResponseV1),
        other => panic!("no roundtrip wired for {other}"),
    }
}

// --- tiny local crypto/hex utilities (no new deps) ------------------------

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "odd hex length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn to_arr32(v: &[u8]) -> [u8; 32] {
    <[u8; 32]>::try_from(v).unwrap()
}

fn to_arr64(v: &[u8]) -> [u8; 64] {
    <[u8; 64]>::try_from(v).unwrap()
}

fn ed_verify(pk: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> bool {
    use ed25519_dalek::{Signature, VerifyingKey};
    match VerifyingKey::from_bytes(pk) {
        Ok(k) => k.verify_strict(msg, &Signature::from_bytes(sig)).is_ok(),
        Err(_) => false,
    }
}
