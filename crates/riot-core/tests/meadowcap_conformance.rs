//! Meadowcap Slice 1 conformance: stable golden capability encodings and
//! fingerprints, negative-form rejection, and seeded generative attenuation
//! checks. Golden vectors are a dependency-drift tripwire against the pinned
//! `willow25`/`meadowcap`. Regenerate intentionally with REGEN=1 (see below).

use riot_core::meadowcap::codec::{
    decode_read_capability_bounded, decode_read_capability_canonic,
    decode_write_capability_bounded, encode_read_capability,
};
use riot_core::meadowcap::create::{new_communal_read, new_communal_write, new_owned_write};
use riot_core::meadowcap::delegate::delegate_write;
use riot_core::meadowcap::fingerprint::{
    read_capability_fingerprint, write_capability_fingerprint,
};
use riot_core::meadowcap::MeadowcapError;
use riot_core::willow::{encode_capability, tai_j2000_micros_from_unix_seconds};
use ufotofu::codec_prelude::EncodableExt;
use willow25::authorisation::raw::{Delegation, PossiblyValidWriteCapability};
use willow25::entry::Entry;
use willow25::prelude::{Area, NamespaceId, NamespaceSecret, Path, SubspaceSecret, TimeRange};

// Canonical wire bytes of any encodable willow value (mirrors
// `willow::encode_capability`, which is `pollster::block_on(v.new_vec_storing_encoding())`).
// `pollster` and `ufotofu` are direct `riot-core` deps, so they are available
// to this integration-test crate.
fn encode_value<E: EncodableExt + ?Sized>(v: &E) -> Vec<u8> {
    pollster::block_on(v.new_vec_storing_encoding())
}

const VECTORS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/willow/meadowcap-vectors.json"
);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Produce a GENUINELY non-canonical encoding of a valid write capability:
/// bytes that willow25's lenient `Decodable` accepts as the same valid value
/// but its canonical `DecodableCanonic` rejects. Such forms exist because
/// delegation areas carry compact-width timestamps/path-lengths that admit
/// non-minimal widths. The oracle below (lenient-accepts AND canonic-rejects)
/// certifies genuineness no matter how the candidate was produced, so the
/// search transform need not know willow's exact byte layout.
///
/// Determinism: fixed seeds and a fixed search order. If this heuristic ever
/// fails to find a candidate (returns None and the `expect` below fires), the
/// implementer constructs one directly from willow's compact-width encoding
/// spec and keeps the same oracle assertion — do NOT weaken the oracle.
fn find_non_canonical_write_encoding() -> Option<Vec<u8>> {
    use ufotofu::codec_prelude::{Decodable, DecodableCanonic};
    use ufotofu::producer::clone_from_slice;
    use willow25::authorisation::WriteCapability;

    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let cap = new_owned_write(&ns, owner.corresponding_subspace_id());
    let editor_id = SubspaceSecret::from_bytes(&[8u8; 32]).corresponding_subspace_id();
    // Tiny TimeRange bounds -> minimal (1-byte) compact-width timestamp fields,
    // leaving room to widen. Codec canonicity does not depend on authorisation,
    // so small raw bounds are fine for this fixture.
    let area = Area::new(
        Some(editor_id.clone()),
        Path::from_slices(&[b"a"]).expect("path"),
        TimeRange::new(0u64.into(), Some(1u64.into())),
    );
    let one_hop = delegate_write(&cap, &owner, area, editor_id).expect("attenuate");
    let canonical = encode_capability(&one_hop);

    let lenient_ok = |b: &[u8]| {
        let mut p = clone_from_slice(b);
        pollster::block_on(WriteCapability::decode(&mut p)).is_ok()
    };
    let canonic_ok = |b: &[u8]| {
        let mut p = clone_from_slice(b);
        pollster::block_on(WriteCapability::decode_canonic(&mut p)).is_ok()
    };

    // Widen a compact-width field. The relative-Area encoding
    // (willow-data-model-0.7.0/src/groupings/area.rs:377-398) packs two 2-bit
    // compact-width tags into the area header byte — start_diff at bits 4-5
    // (mask 0x0C) and end_diff at bits 6-7 (mask 0x03) — and the cu64 value
    // bytes follow the header after an optional 32-byte subspace id. A
    // non-canonical form = bump a tag one width class and insert one
    // big-endian leading zero before that value. We don't know which byte is
    // the header or the exact value offset, so we sweep both, oracle-checked:
    // lenient decode accepts AND canonic decode rejects certifies genuineness.
    for i in 0..canonical.len() {
        for (mask, one_class) in [(0x03u8, 0x01u8), (0x0Cu8, 0x04u8)] {
            let class = (canonical[i] & mask) / one_class;
            if class >= 3 {
                continue; // already widest
            }
            let mut m = canonical.clone();
            m[i] = (m[i] & !mask) | ((class + 1) * one_class);
            // Value byte sits somewhere after the header (possibly past a
            // 32-byte subspace id). Try each insertion point.
            for j in (i + 1)..(i + 40).min(m.len()) {
                let mut widened = m[..j].to_vec();
                widened.push(0u8);
                widened.extend_from_slice(&m[j..]);
                if lenient_ok(&widened) && !canonic_ok(&widened) {
                    return Some(widened);
                }
            }
        }
    }
    None
}

/// The deterministic vector set. Add rows here; never edit committed hex by hand.
fn build_vectors() -> serde_json::Value {
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();

    // 1. owned genesis write cap
    let owned = new_owned_write(&ns, owner_id.clone());

    // 2. one-hop attenuation to /articles for a bounded micros window
    let editor_id = SubspaceSecret::from_bytes(&[8u8; 32]).corresponding_subspace_id();
    let area = Area::new(
        Some(editor_id.clone()),
        Path::from_slices(&[b"articles", b"news"]).expect("path"),
        TimeRange::new(
            tai_j2000_micros_from_unix_seconds(1_700_000_000)
                .unwrap()
                .into(),
            Some(
                tai_j2000_micros_from_unix_seconds(1_800_000_000)
                    .unwrap()
                    .into(),
            ),
        ),
    );
    let delegated = delegate_write(&owned, &owner, area, editor_id).expect("attenuate");

    // 3. communal read cap
    let read = new_communal_read(
        NamespaceId::from_bytes(&[16u8; 32]),
        SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id(),
    );

    // 4. authorisation token for an owned-write entry (spec line 1124). Pin the
    //    capability bytes AND the receiver signature bytes. ed25519 signing is
    //    deterministic over the canonical entry encoding, so both are stable.
    let token_entry = Entry::builder()
        .namespace_id(ns.corresponding_namespace_id())
        .subspace_id(owner_id.clone())
        .path(Path::from_slices(&[b"manifest"]).expect("path"))
        .timestamp(1_000u64)
        .payload(b"token-fixture")
        .build();
    let authorised = token_entry
        .into_authorised_entry(&owned, &owner)
        .expect("owner authorises");
    let token = authorised.authorisation_token();

    // 5. a genuinely non-canonical write-capability encoding (spec line 1122).
    let non_canonical = find_non_canonical_write_encoding()
        .expect("a non-canonical encoding exists (compact-width timestamp field)");

    serde_json::json!({
        "owned_write_genesis": {
            "encoding_hex": hex(&encode_capability(&owned)),
            "fingerprint_hex": hex(&write_capability_fingerprint(&owned)),
        },
        "owned_write_one_hop_articles": {
            "encoding_hex": hex(&encode_capability(&delegated)),
            "fingerprint_hex": hex(&write_capability_fingerprint(&delegated)),
        },
        "communal_read_genesis": {
            "encoding_hex": hex(&encode_read_capability(&read)),
            "fingerprint_hex": hex(&read_capability_fingerprint(&read)),
        },
        "owned_write_authorisation_token": {
            "capability_hex": hex(&encode_capability(token.capability())),
            "signature_hex": hex(&encode_value(token.signature())),
        },
        "non_canonical_write_encoding": {
            "encoding_hex": hex(&non_canonical),
        },
    })
}

#[test]
fn non_canonical_encoding_is_rejected_but_leniently_decodable() {
    // spec line 1122: a genuinely non-canonical encoding must be rejected by
    // Riot's canonical decoder (Malformed), while willow25's LENIENT decoder
    // accepts it — the lenient acceptance proves it is genuinely non-canonical
    // (a valid value in a non-minimal wire form), not merely corrupt bytes.
    use ufotofu::codec_prelude::Decodable;
    use ufotofu::producer::clone_from_slice;
    use willow25::authorisation::WriteCapability;

    let bytes = find_non_canonical_write_encoding()
        .expect("a non-canonical encoding exists (compact-width timestamp field)");

    let mut p = clone_from_slice(&bytes);
    assert!(
        pollster::block_on(WriteCapability::decode(&mut p)).is_ok(),
        "lenient decode must accept the non-canonical form (genuineness oracle)"
    );
    assert_eq!(
        decode_write_capability_bounded(&bytes),
        Err(MeadowcapError::Malformed),
        "Riot's canonical decoder must reject the non-canonical form"
    );
}

#[test]
fn golden_vectors_match_committed_fixture() {
    let current = build_vectors();
    if std::env::var("REGEN").is_ok() {
        std::fs::write(
            VECTORS_PATH,
            format!("{}\n", serde_json::to_string_pretty(&current).unwrap()),
        )
        .unwrap();
        return;
    }
    let committed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(VECTORS_PATH).expect("vectors file"))
            .expect("valid json");
    assert_eq!(
        current, committed,
        "capability encodings/fingerprints drifted from committed golden vectors"
    );
}

#[test]
fn reencoding_a_decoded_capability_is_byte_identical() {
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let cap = new_owned_write(&ns, owner.corresponding_subspace_id());
    let bytes = encode_capability(&cap);
    let decoded = decode_write_capability_bounded(&bytes).expect("bounded decode");
    assert_eq!(encode_capability(&decoded), bytes);
}

#[test]
fn negative_forms_are_rejected() {
    let read = new_communal_read(
        NamespaceId::from_bytes(&[16u8; 32]),
        SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id(),
    );
    let bytes = encode_read_capability(&read);

    // trailing byte
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        decode_read_capability_canonic(&trailing),
        Err(MeadowcapError::TrailingBytes)
    );

    // flipped signature byte -> Malformed. The cap must be OWNED: an owned
    // genesis encoding ends with the 64-byte initial-authorisation signature
    // (meadowcap-0.5.0 raw/mod.rs:748-750 — namespace_key, user_key,
    // initial_authorisation), so flipping the last byte corrupts a signature.
    // A COMMUNAL genesis carries no signature at all; flipping its last byte
    // just yields a DIFFERENT valid user key, which decodes fine.
    // Canonical decode's is_valid() rejects the bad signature; there is no
    // separate NonCanonical code (single-variant assertion, not an OR-match
    // that could pass vacuously).
    use riot_core::meadowcap::create::new_owned_read;
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owned_read = new_owned_read(
        &ns,
        SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id(),
    );
    let mut owned_bytes = encode_read_capability(&owned_read);
    if let Some(last) = owned_bytes.last_mut() {
        *last ^= 0xff;
    }
    assert_eq!(
        decode_read_capability_canonic(&owned_bytes),
        Err(MeadowcapError::Malformed)
    );
}

#[test]
fn wrong_access_mode_bytes_are_rejected() {
    // Read-capability bytes fed to the WRITE decoder (and vice versa) must be
    // rejected. The wrapper's decode errors when the decoded genesis access mode
    // is the other mode (meadowcap-0.5.0 raw/possibly_valid_write_capability.rs:997/:1050);
    // Riot surfaces that as Malformed. (spec line 1122)
    let read = new_communal_read(
        NamespaceId::from_bytes(&[16u8; 32]),
        SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id(),
    );
    let read_bytes = encode_read_capability(&read);
    assert_eq!(
        decode_write_capability_bounded(&read_bytes),
        Err(MeadowcapError::Malformed),
        "read bytes must not decode as a write capability"
    );

    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let write = new_owned_write(&ns, owner.corresponding_subspace_id());
    let write_bytes = encode_capability(&write);
    assert_eq!(
        decode_read_capability_bounded(&write_bytes),
        Err(MeadowcapError::Malformed),
        "write bytes must not decode as a read capability"
    );
}

#[test]
fn reordered_delegation_chain_is_rejected() {
    // spec line 1122 "reordered chains". FEASIBILITY NOTE: this deliberately
    // uses a COMMUNAL genesis via `PossiblyValidWriteCapability::new_communal(
    // namespace, receiver)` — a 2-arg constructor that takes NO
    // `NamespaceSignature` (willow25 raw/possibly_valid_write_capability.rs:237).
    // The owned constructor `new_owned(namespace, receiver, initial_authorisation)`
    // is NOT usable here because that genesis `NamespaceSignature` is unreachable
    // from riot-core (willow25's `Genesis` exposes no signature accessor). The
    // communal path sidesteps that entirely — no signature reconstruction, no
    // byte-offset splicing. `append_delegation` panics if a delegation's area is
    // not contained in the prior area, so BOTH hops use the SAME area; equal
    // areas satisfy containment in either order, isolating the reorder to the
    // position-bound signatures (each handover includes the prior delegation).
    let namespace = NamespaceId::from_bytes(&[16u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let genesis = new_communal_write(namespace.clone(), owner_id.clone());

    // Both hops grant the SAME area (owner's subspace, path /a, full time).
    let shared_area = Area::new(
        Some(owner_id.clone()),
        Path::from_slices(&[b"a"]).expect("path"),
        TimeRange::new(0u64.into(), Some(u64::MAX.into())),
    );
    let a = SubspaceSecret::from_bytes(&[8u8; 32]);
    let a_id = a.corresponding_subspace_id();
    let one_hop = delegate_write(&genesis, &owner, shared_area.clone(), a_id).expect("hop A");
    let b_id = SubspaceSecret::from_bytes(&[9u8; 32]).corresponding_subspace_id();
    let two_hop = delegate_write(&one_hop, &a, shared_area, b_id).expect("hop B");
    let two_hop_bytes = encode_capability(&two_hop);

    // The valid chain's two delegations, in order [A, B].
    let dels: Vec<Delegation> = two_hop.delegations().to_vec();
    assert_eq!(dels.len(), 2);

    // POSITIVE CONTROL / self-checking precondition: rebuilding the SAME genesis
    // with the SAME delegations IN ORDER via the raw builder must reproduce the
    // valid cap's EXACT bytes, and those bytes must decode. If a future crate
    // bump changes the encoding or delegation layout, this precondition fails
    // loudly here — rather than the reorder assertion below passing for the
    // wrong reason.
    let mut in_order =
        PossiblyValidWriteCapability::new_communal(namespace.clone(), owner_id.clone());
    in_order.append_delegation(dels[0].clone());
    in_order.append_delegation(dels[1].clone());
    assert_eq!(
        encode_value(&in_order),
        two_hop_bytes,
        "raw in-order rebuild must reproduce the valid encoding"
    );
    assert!(
        decode_write_capability_bounded(&two_hop_bytes).is_ok(),
        "valid chain must decode"
    );

    // REORDERED [B, A]: the first delegation now carries B's signature (made by
    // A's key), which fails validation against the genesis receiver. Canonical
    // decode (which runs is_valid over the chain) must reject it.
    let mut reordered = PossiblyValidWriteCapability::new_communal(namespace, owner_id);
    reordered.append_delegation(dels[1].clone());
    reordered.append_delegation(dels[0].clone());
    assert_eq!(
        decode_write_capability_bounded(&encode_value(&reordered)),
        Err(MeadowcapError::Malformed),
        "a position-swapped delegation chain must fail signature validation"
    );
}

#[test]
fn delegation_chain_signature_tamper_is_rejected() {
    // Spec line 157 end-to-end: build a signed OWNED genesis and delegate twice
    // (multi-hop). Canonical decode verifies every namespace and user signature
    // in the chain, so no single byte of the encoding — including the
    // intermediate delegation's 64-byte signature region — can be flipped and
    // still decode to a valid capability.
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let genesis = new_owned_write(&ns, owner_id.clone());

    // hop 1 -> mid: this delegation's signature is the "intermediate" region.
    // Subspace None (all subspaces under /articles) so hop 2 can restrict to
    // the leaf's subspace — a Some(mid) area would NOT contain a Some(leaf)
    // area and try_delegate would reject hop 2 as AuthorityExpanding.
    let mid = SubspaceSecret::from_bytes(&[8u8; 32]);
    let mid_id = mid.corresponding_subspace_id();
    let area1 = Area::new(
        None,
        Path::from_slices(&[b"articles"]).expect("path"),
        TimeRange::new(
            tai_j2000_micros_from_unix_seconds(1_700_000_000)
                .unwrap()
                .into(),
            Some(
                tai_j2000_micros_from_unix_seconds(1_800_000_000)
                    .unwrap()
                    .into(),
            ),
        ),
    );
    let one_hop = delegate_write(&genesis, &owner, area1, mid_id.clone()).expect("hop1");

    // hop 2 -> leaf (outer delegation).
    let leaf = SubspaceSecret::from_bytes(&[9u8; 32]).corresponding_subspace_id();
    let area2 = Area::new(
        Some(leaf.clone()),
        Path::from_slices(&[b"articles", b"news"]).expect("path"),
        TimeRange::new(
            tai_j2000_micros_from_unix_seconds(1_700_000_000)
                .unwrap()
                .into(),
            Some(
                tai_j2000_micros_from_unix_seconds(1_800_000_000)
                    .unwrap()
                    .into(),
            ),
        ),
    );
    let two_hop = delegate_write(&one_hop, &mid, area2, leaf).expect("hop2");
    assert_eq!(two_hop.delegations().len(), 2);

    let good = encode_capability(&two_hop);
    assert!(
        decode_write_capability_bounded(&good).is_ok(),
        "pristine chain must decode"
    );

    // Every single-byte corruption must be rejected by canonical decode. The
    // swept set includes the intermediate delegation's signature bytes, so a
    // tampered mid-chain signature is provably rejected.
    for i in 0..good.len() {
        let mut tampered = good.clone();
        tampered[i] ^= 0xff;
        assert!(
            decode_write_capability_bounded(&tampered).is_err(),
            "byte {i} (a signature/key/area/length byte) was not load-bearing: tampered chain still decoded"
        );
    }
}

#[test]
fn seeded_generative_attenuation_never_expands_authority() {
    // Deterministic generative sweep (no proptest dependency): for many seeds,
    // a one-hop delegation's granted area must be contained in the parent's.
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();
    let parent = new_owned_write(&ns, owner_id.clone());

    for seed in 0u8..64 {
        let receiver = SubspaceSecret::from_bytes(&[seed; 32]).corresponding_subspace_id();
        let area = Area::new(
            Some(receiver.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            TimeRange::new(
                tai_j2000_micros_from_unix_seconds(1_700_000_000)
                    .unwrap()
                    .into(),
                Some(
                    // (seed + 1): seed 0 would make start == end, an EMPTY
                    // TimeRange, and willow's Area::new panics on EmptyGrouping.
                    tai_j2000_micros_from_unix_seconds(1_700_000_000 + (seed as u64 + 1) * 1000)
                        .unwrap()
                        .into(),
                ),
            ),
        );
        let child = delegate_write(&parent, &owner, area.clone(), receiver).expect("attenuate");
        // The parent (owned, Area::full()) must include the child's granted area.
        assert!(
            parent.includes_area(&child.granted_area()),
            "child area escaped parent for seed {seed}"
        );
    }
}

#[test]
fn seeded_generative_invalid_trees_are_all_rejected() {
    // spec line 1126: from each generated valid tree, derive INVALID variants
    // and assert every one is rejected. Seeded and deterministic — no rng/clock.
    let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
    let owner = SubspaceSecret::from_bytes(&[4u8; 32]);
    let owner_id = owner.corresponding_subspace_id();

    for seed in 1u8..48 {
        let parent = new_owned_write(&ns, owner_id.clone());
        let receiver = SubspaceSecret::from_bytes(&[seed; 32]).corresponding_subspace_id();
        let area = Area::new(
            Some(receiver.clone()),
            Path::from_slices(&[b"articles"]).expect("path"),
            TimeRange::new(
                tai_j2000_micros_from_unix_seconds(1_700_000_000)
                    .unwrap()
                    .into(),
                Some(
                    tai_j2000_micros_from_unix_seconds(1_800_000_000)
                        .unwrap()
                        .into(),
                ),
            ),
        );
        let valid =
            delegate_write(&parent, &owner, area.clone(), receiver.clone()).expect("valid tree");

        // (1) Wrong-signer delegation attempt: a non-receiver secret must fail.
        let impostor = SubspaceSecret::from_bytes(&[seed.wrapping_add(128); 32]);
        assert_eq!(
            delegate_write(&valid, &impostor, area.clone(), receiver.clone()),
            Err(MeadowcapError::ReceiverMismatch),
            "wrong-signer delegation must be rejected (seed {seed})"
        );

        // (2) Seeded byte-flip corruption of the valid encoding must not decode.
        let bytes = encode_capability(&valid);
        let offset = (seed as usize).wrapping_mul(7) % bytes.len();
        let mut corrupt = bytes.clone();
        corrupt[offset] ^= 0xff;
        assert!(
            decode_write_capability_bounded(&corrupt).is_err(),
            "byte-flip at {offset} must be rejected (seed {seed})"
        );

        // (3) Over-depth chain (17 hops) must be rejected by the depth ceiling.
        // First signer is the genesis receiver (a fresh secret of the same
        // fixed bytes as `owner`, so no `SubspaceSecret` clone is needed).
        let mut deep = new_owned_write(&ns, owner_id.clone());
        let mut signer = SubspaceSecret::from_bytes(&[4u8; 32]);
        for i in 0..17u8 {
            let next = SubspaceSecret::from_bytes(&[seed.wrapping_add(i).wrapping_add(1); 32]);
            deep = delegate_write(
                &deep,
                &signer,
                Area::full(),
                next.corresponding_subspace_id(),
            )
            .expect("hop");
            signer = next;
        }
        assert_eq!(
            decode_write_capability_bounded(&encode_capability(&deep)),
            Err(MeadowcapError::ChainTooDeep { depth: 17, max: 16 }),
            "over-depth chain must be rejected (seed {seed})"
        );
    }
}
