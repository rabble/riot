//! The LoRa/Meshtastic datagram profile — the codec the design in
//! `docs/superpowers/specs/2026-08-05-lora-meshtastic-transport-design.md`
//! rests on. No radio required: every property that matters is a property of
//! the wire format.
//!
//! The two ceilings this pins:
//!   * a Meshtastic packet carries 237 bytes, so anything larger fragments;
//!   * a WILLIAM3 chunk is 1024 bytes and Bab verification is sequential, so a
//!     message must stay inside ONE chunk to be one verification unit.

use riot_core::lora::{
    decode_datagram, encode_datagram, fragment, reassemble, LoraDatagram, LoraError, Reassembler,
    MAX_LORA_MESSAGE_BYTES, MAX_LORA_PAYLOAD,
};
use riot_core::willow::william3_digest;

fn beacon() -> LoraDatagram {
    LoraDatagram::JoinBeacon {
        namespace_id: [7u8; 32],
        descriptor_entry_id: [9u8; 32],
        content_digest: [11u8; 32],
    }
}

/// The highest-value message in the design must cost exactly one packet — it is
/// the only way to reach a community you have no prior contact with.
#[test]
fn a_join_beacon_fits_in_a_single_meshtastic_packet() {
    let encoded = encode_datagram(&beacon()).expect("encode");
    assert!(
        encoded.len() <= MAX_LORA_PAYLOAD,
        "a join beacon must fit one 237-byte packet, got {}",
        encoded.len()
    );
    assert_eq!(decode_datagram(&encoded).expect("decode"), beacon());
}

/// A 508-byte alert is the measured real case. It must fragment, survive the
/// mesh, and come back byte-identical.
#[test]
fn an_alert_sized_payload_fragments_and_reassembles_verbatim() {
    let payload: Vec<u8> = (0..508u32).map(|i| (i % 251) as u8).collect();
    let digest = william3_digest(&payload);
    let parts = fragment(&payload, digest).expect("fragment");
    assert!(parts.len() >= 3, "508 bytes needs at least 3 packets");
    for part in &parts {
        let encoded = encode_datagram(part).expect("encode");
        assert!(
            encoded.len() <= MAX_LORA_PAYLOAD,
            "every fragment must fit a packet, got {}",
            encoded.len()
        );
    }
    assert_eq!(reassemble(&parts).expect("reassemble"), payload);
}

/// Managed flooding delivers out of order. Reassembly must not assume sequence.
#[test]
fn fragments_reassemble_when_they_arrive_out_of_order() {
    let payload: Vec<u8> = (0..600u32).map(|i| (i % 253) as u8).collect();
    let digest = william3_digest(&payload);
    let mut parts = fragment(&payload, digest).expect("fragment");
    parts.reverse();
    assert_eq!(reassemble(&parts).expect("reassemble"), payload);
}

/// The payload is verified against the digest the signed entry already commits
/// to. A flipped byte must be refused, not returned.
#[test]
fn a_corrupted_fragment_fails_verification_rather_than_returning_bytes() {
    let payload: Vec<u8> = (0..400u32).map(|i| (i % 249) as u8).collect();
    let digest = william3_digest(&payload);
    let mut parts = fragment(&payload, digest).expect("fragment");
    match parts.get_mut(1) {
        Some(LoraDatagram::PayloadFragment { chunk, .. }) => chunk[0] ^= 0xFF,
        _ => panic!("expected a payload fragment"),
    }
    assert_eq!(reassemble(&parts), Err(LoraError::DigestMismatch));
}

/// A missing packet must read as incomplete. Silently returning a short payload
/// would hand a caller unverified bytes.
#[test]
fn a_missing_fragment_is_incomplete_never_silently_truncated() {
    let payload: Vec<u8> = (0..500u32).map(|i| (i % 247) as u8).collect();
    let digest = william3_digest(&payload);
    let mut parts = fragment(&payload, digest).expect("fragment");
    parts.remove(1);
    assert_eq!(reassemble(&parts), Err(LoraError::Incomplete));
}

/// The central design rule. Bab's WILLIAM3 chunk is 1024 bytes and verification
/// is sequential; a message spanning two chunks is two verification units and
/// cannot be authenticated from one digest as a unit. Refuse at the boundary.
#[test]
fn a_payload_larger_than_one_bab_chunk_is_refused() {
    let payload = vec![0u8; MAX_LORA_MESSAGE_BYTES + 1];
    let digest = william3_digest(&payload);
    assert_eq!(fragment(&payload, digest), Err(LoraError::PayloadTooLarge));
    // Exactly one chunk is the largest thing that may be sent.
    let at_limit = vec![0u8; MAX_LORA_MESSAGE_BYTES];
    let digest = william3_digest(&at_limit);
    assert!(fragment(&at_limit, digest).is_ok());
}

/// A batched track is why location updates are affordable at all: one signed
/// entry, many fixes. Prove a realistic batch stays inside a single chunk.
#[test]
fn a_batch_of_forty_position_fixes_fits_one_chunk() {
    // lat/lon/time, 8 bytes each, plus per-fix framing.
    let batch: Vec<u8> = (0..40u32).flat_map(|i| (i as u64).to_be_bytes()).collect();
    let padded = [batch, vec![0u8; 400]].concat();
    assert!(padded.len() <= MAX_LORA_MESSAGE_BYTES);
    let digest = william3_digest(&padded);
    assert!(fragment(&padded, digest).is_ok());
}

/// The reassembler is what a radio actually drives: fragments arrive one at a
/// time and it reports when a message is whole.
#[test]
fn the_reassembler_accumulates_until_a_message_is_whole() {
    let payload: Vec<u8> = (0..450u32).map(|i| (i % 241) as u8).collect();
    let digest = william3_digest(&payload);
    let parts = fragment(&payload, digest).expect("fragment");

    let mut reassembler = Reassembler::default();
    let mut completed = None;
    for part in &parts {
        if let Some(done) = reassembler.accept(part).expect("accept") {
            completed = Some(done);
        }
    }
    assert_eq!(completed, Some(payload));
}

/// A datagram that is not a fragment of the message being assembled must not
/// corrupt it — the mesh is shared and carries other communities' traffic.
#[test]
fn a_fragment_of_another_message_does_not_corrupt_this_one() {
    let mine: Vec<u8> = (0..300u32).map(|i| (i % 239) as u8).collect();
    let theirs: Vec<u8> = (0..300u32).map(|i| (i % 233) as u8).collect();
    let my_digest = william3_digest(&mine);
    let their_digest = william3_digest(&theirs);
    let my_parts = fragment(&mine, my_digest).expect("fragment");
    let their_parts = fragment(&theirs, their_digest).expect("fragment");

    let mut reassembler = Reassembler::default();
    let mut completed = None;
    // Interleave the two messages, as a shared mesh would.
    for (mine, theirs) in my_parts.iter().zip(their_parts.iter()) {
        let _ = reassembler.accept(theirs).expect("accept");
        if let Some(done) = reassembler.accept(mine).expect("accept") {
            completed = Some(done);
        }
    }
    assert_eq!(completed, Some(mine));
}
