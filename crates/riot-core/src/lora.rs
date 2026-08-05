//! The LoRa/Meshtastic datagram profile.
//!
//! Design: `docs/superpowers/specs/2026-08-05-lora-meshtastic-transport-design.md`
//!
//! Two ceilings shape everything here, both measured rather than assumed:
//!
//! * **A Meshtastic packet carries 237 bytes.** Anything larger fragments. That
//!   is not the hard part — Riot's BLE transport already chunks to 20 bytes.
//! * **A WILLIAM3 chunk is 1024 bytes and Bab verification is sequential.** A
//!   message spanning two chunks is two verification units, so it cannot be
//!   authenticated as a unit from the digest the signed entry already carries.
//!   Everything sent here therefore stays inside ONE chunk.
//!
//! This is deliberately NOT a [`crate::sync`] transport and must never implement
//! `riot_transport::Dialer`. That trait is an ordered, reliable byte stream;
//! Meshtastic is unordered lossy broadcast. Building a reliable stream over a
//! ~1 kbps shared flood mesh would spend everyone's airtime on retransmissions.
//! Instead each datagram is self-contained and independently verifiable: loss
//! costs exactly that datagram, and arrival order does not matter.
//!
//! Verification reuses what Riot already has. A Willow entry commits to
//! `payload_digest` + `payload_length` with the payload as a separate object, so
//! one Ed25519 signature over the entry covers a payload delivered as many tiny
//! packets. This is the property tinySSB builds side-chains to obtain; Riot gets
//! it from the data model.

use minicbor::{Decoder, Encoder};
use std::collections::BTreeMap;

use crate::willow::william3_digest;

/// Meshtastic's maximum payload per packet, excluding protobuf overhead.
pub const MAX_LORA_PAYLOAD: usize = 237;

/// One WILLIAM3 chunk. The largest payload this profile will carry, so every
/// message is exactly one Bab verification unit. Raising this without changing
/// the verification story would hand callers bytes that cannot be authenticated
/// from a single digest.
pub const MAX_LORA_MESSAGE_BYTES: usize = 1024;

const LORA_CODEC: &str = "org.riot.lora/1";

/// Bytes of CBOR framing reserved per fragment, so a chunk plus its envelope
/// always fits one packet. Measured against the encoder, not guessed: the test
/// suite asserts every emitted fragment is within [`MAX_LORA_PAYLOAD`].
const FRAGMENT_OVERHEAD: usize = 64;

/// The payload bytes carried per fragment.
pub const FRAGMENT_CHUNK_BYTES: usize = MAX_LORA_PAYLOAD - FRAGMENT_OVERHEAD;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoraDatagram {
    /// "This community exists, here is how to join it." The only way to reach a
    /// community you have no prior contact with, and it costs one packet.
    ///
    /// PRIVACY: this is an unencrypted, direction-findable broadcast. It must be
    /// opt-in per community and never default-on — see the design's open
    /// questions before wiring it to a radio.
    JoinBeacon {
        namespace_id: [u8; 32],
        descriptor_entry_id: [u8; 32],
        content_digest: [u8; 32],
    },
    /// One slice of a payload, addressed by the digest the signed entry commits
    /// to. `index`/`total` make reassembly order-independent, because managed
    /// flooding does not preserve order.
    PayloadFragment {
        payload_digest: [u8; 32],
        index: u16,
        total: u16,
        chunk: Vec<u8>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoraError {
    /// Larger than one WILLIAM3 chunk — see [`MAX_LORA_MESSAGE_BYTES`].
    PayloadTooLarge,
    /// Fragments are missing. Never returned alongside partial bytes.
    Incomplete,
    /// Reassembled bytes do not match the committed digest.
    DigestMismatch,
    /// Fragments disagree about how many there are.
    InconsistentTotal,
    MalformedDatagram,
    UnsupportedCodec,
}

pub fn encode_datagram(datagram: &LoraDatagram) -> Result<Vec<u8>, LoraError> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    let _ = encoder.map(3);
    let _ = encoder.u8(0);
    let _ = encoder.str(LORA_CODEC);
    let _ = encoder.u8(1);
    match datagram {
        LoraDatagram::JoinBeacon {
            namespace_id,
            descriptor_entry_id,
            content_digest,
        } => {
            let _ = encoder.u8(0);
            let _ = encoder.u8(2);
            let _ = encoder.array(3);
            let _ = encoder.bytes(namespace_id);
            let _ = encoder.bytes(descriptor_entry_id);
            let _ = encoder.bytes(content_digest);
        }
        LoraDatagram::PayloadFragment {
            payload_digest,
            index,
            total,
            chunk,
        } => {
            let _ = encoder.u8(1);
            let _ = encoder.u8(2);
            let _ = encoder.array(4);
            let _ = encoder.bytes(payload_digest);
            let _ = encoder.u16(*index);
            let _ = encoder.u16(*total);
            let _ = encoder.bytes(chunk);
        }
    }
    Ok(bytes)
}

pub fn decode_datagram(bytes: &[u8]) -> Result<LoraDatagram, LoraError> {
    let mut decoder = Decoder::new(bytes);
    let entries = decoder.map().map_err(|_| LoraError::MalformedDatagram)?;
    if entries != Some(3) {
        return Err(LoraError::MalformedDatagram);
    }
    if decoder.u8().map_err(|_| LoraError::MalformedDatagram)? != 0 {
        return Err(LoraError::MalformedDatagram);
    }
    if decoder.str().map_err(|_| LoraError::MalformedDatagram)? != LORA_CODEC {
        return Err(LoraError::UnsupportedCodec);
    }
    if decoder.u8().map_err(|_| LoraError::MalformedDatagram)? != 1 {
        return Err(LoraError::MalformedDatagram);
    }
    let kind = decoder.u8().map_err(|_| LoraError::MalformedDatagram)?;
    if decoder.u8().map_err(|_| LoraError::MalformedDatagram)? != 2 {
        return Err(LoraError::MalformedDatagram);
    }
    match kind {
        0 => {
            let count = decoder.array().map_err(|_| LoraError::MalformedDatagram)?;
            if count != Some(3) {
                return Err(LoraError::MalformedDatagram);
            }
            Ok(LoraDatagram::JoinBeacon {
                namespace_id: fixed32(&mut decoder)?,
                descriptor_entry_id: fixed32(&mut decoder)?,
                content_digest: fixed32(&mut decoder)?,
            })
        }
        1 => {
            let count = decoder.array().map_err(|_| LoraError::MalformedDatagram)?;
            if count != Some(4) {
                return Err(LoraError::MalformedDatagram);
            }
            Ok(LoraDatagram::PayloadFragment {
                payload_digest: fixed32(&mut decoder)?,
                index: decoder.u16().map_err(|_| LoraError::MalformedDatagram)?,
                total: decoder.u16().map_err(|_| LoraError::MalformedDatagram)?,
                chunk: decoder
                    .bytes()
                    .map_err(|_| LoraError::MalformedDatagram)?
                    .to_vec(),
            })
        }
        _ => Err(LoraError::MalformedDatagram),
    }
}

fn fixed32(decoder: &mut Decoder<'_>) -> Result<[u8; 32], LoraError> {
    decoder
        .bytes()
        .map_err(|_| LoraError::MalformedDatagram)?
        .try_into()
        .map_err(|_| LoraError::MalformedDatagram)
}

/// Splits a payload into packet-sized fragments addressed by its digest.
///
/// Refuses anything past one WILLIAM3 chunk: beyond that a receiver cannot treat
/// the message as a single verification unit, and handing back bytes it could
/// not authenticate as a whole is exactly what this profile exists to avoid.
pub fn fragment(payload: &[u8], payload_digest: [u8; 32]) -> Result<Vec<LoraDatagram>, LoraError> {
    if payload.len() > MAX_LORA_MESSAGE_BYTES {
        return Err(LoraError::PayloadTooLarge);
    }
    let chunks: Vec<&[u8]> = if payload.is_empty() {
        vec![&[]]
    } else {
        payload.chunks(FRAGMENT_CHUNK_BYTES).collect()
    };
    let total = u16::try_from(chunks.len()).map_err(|_| LoraError::PayloadTooLarge)?;
    Ok(chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| LoraDatagram::PayloadFragment {
            payload_digest,
            index: index as u16,
            total,
            chunk: chunk.to_vec(),
        })
        .collect())
}

/// Reassembles fragments of ONE payload and verifies the result against the
/// digest they carry. Order-independent; duplicates are harmless.
///
/// Returns [`LoraError::Incomplete`] rather than short bytes when a fragment is
/// missing, and [`LoraError::DigestMismatch`] rather than the bytes when
/// verification fails.
pub fn reassemble(fragments: &[LoraDatagram]) -> Result<Vec<u8>, LoraError> {
    let mut reassembler = Reassembler::default();
    let mut completed = None;
    for fragment in fragments {
        if let Some(payload) = reassembler.accept(fragment)? {
            completed = Some(payload);
        }
    }
    match completed {
        Some(payload) => Ok(payload),
        None if reassembler.saw_mismatch => Err(LoraError::DigestMismatch),
        None => Err(LoraError::Incomplete),
    }
}

/// Accumulates fragments as a radio delivers them, one at a time, and reports a
/// payload the moment it is whole AND verified.
///
/// Keyed by payload digest, so traffic for another message — the mesh is shared,
/// and carries other communities — cannot corrupt the one being assembled.
#[derive(Debug, Default)]
pub struct Reassembler {
    in_flight: BTreeMap<[u8; 32], Partial>,
    saw_mismatch: bool,
}

#[derive(Debug)]
struct Partial {
    total: u16,
    chunks: BTreeMap<u16, Vec<u8>>,
}

impl Reassembler {
    /// Takes one datagram. Returns the payload only when every fragment has
    /// arrived and the bytes verify against the committed digest.
    ///
    /// A beacon is not part of any payload, so it yields nothing here.
    pub fn accept(&mut self, datagram: &LoraDatagram) -> Result<Option<Vec<u8>>, LoraError> {
        let LoraDatagram::PayloadFragment {
            payload_digest,
            index,
            total,
            chunk,
        } = datagram
        else {
            return Ok(None);
        };
        let partial = self.in_flight.entry(*payload_digest).or_insert(Partial {
            total: *total,
            chunks: BTreeMap::new(),
        });
        if partial.total != *total {
            return Err(LoraError::InconsistentTotal);
        }
        partial.chunks.insert(*index, chunk.clone());
        if partial.chunks.len() != usize::from(partial.total) {
            return Ok(None);
        }
        let payload: Vec<u8> = partial.chunks.values().flatten().copied().collect();
        // Verify against the digest the signed entry already commits to. A
        // mismatch drops the attempt — the sender can resend, and a caller is
        // never handed bytes that failed their own commitment.
        if william3_digest(&payload) != *payload_digest {
            self.in_flight.remove(payload_digest);
            self.saw_mismatch = true;
            return Err(LoraError::DigestMismatch);
        }
        self.in_flight.remove(payload_digest);
        Ok(Some(payload))
    }
}
