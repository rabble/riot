//! `GovernanceRecordV1` canonical CBOR envelope + domain-separated record id.
//!
//! Envelope = definite map, strictly ascending integer keys 0..=10:
//! 0 schema string, 1 kind tag, 2 namespace, 3 parents (definite array of
//! 32-byte ids, strictly ascending), 4 actor_id, 5 receiver, 6 sequence,
//! 7 prev_actor_record (null | bytes32), 8 authorizing_fingerprint, 9 body
//! (per-kind map, `body.rs`), 10 created_display_micros. Canonicality is
//! proven by re-encoding the decoded value and requiring byte identity.

use minicbor::{Decoder, Encoder};
use sha2::{Digest, Sha256};

use super::body::{decode_body, encode_body, kind_of, Body};
use super::{GovernanceError, RecordKind, MAX_GOVERNANCE_RECORD_BYTES, MAX_PARENTS};
// Re-export BOTH id aliases so sibling modules (`actor`, `frontier`, `action`)
// can `use super::record::{RecordId, Fingerprint}` — a private `use` of RecordId
// here would make those imports E0603.
pub use super::{Fingerprint, RecordId};

pub const GOVERNANCE_RECORD_SCHEMA: &str = "org.riot.governance.record/1";
const RECORD_ID_DOMAIN: &[u8] = b"riot/governance-record-id/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceRecordV1 {
    pub kind: RecordKind,
    pub namespace: [u8; 32],
    pub parents: Vec<RecordId>, // sorted, dedup, <= MAX_PARENTS
    pub actor_id: [u8; 32],
    pub receiver: [u8; 32],
    pub sequence: u64,
    pub prev_actor_record: Option<RecordId>,
    pub authorizing_fingerprint: Fingerprint,
    pub body: Body,
    pub created_display_micros: u64,
}

type EncErr = minicbor::encode::Error<core::convert::Infallible>;

fn encode_envelope(
    r: &GovernanceRecordV1,
    parents: &[RecordId],
    e: &mut Encoder<&mut Vec<u8>>,
) -> Result<(), GovernanceError> {
    let head: Result<(), EncErr> = (|| {
        e.map(11)?;
        e.u64(0)?.str(GOVERNANCE_RECORD_SCHEMA)?;
        e.u64(1)?.u64(r.kind.tag())?;
        e.u64(2)?.bytes(&r.namespace)?;
        e.u64(3)?.array(parents.len() as u64)?;
        for p in parents {
            e.bytes(p)?;
        }
        e.u64(4)?.bytes(&r.actor_id)?;
        e.u64(5)?.bytes(&r.receiver)?;
        e.u64(6)?.u64(r.sequence)?;
        e.u64(7)?;
        match &r.prev_actor_record {
            Some(id) => {
                e.bytes(id)?;
            }
            None => {
                e.null()?;
            }
        }
        e.u64(8)?.bytes(&r.authorizing_fingerprint)?;
        e.u64(9)?;
        Ok(())
    })();
    head.map_err(|_| GovernanceError::Malformed)?;
    encode_body(&r.body, e)?;
    let tail: Result<(), EncErr> = (|| {
        e.u64(10)?.u64(r.created_display_micros)?;
        Ok(())
    })();
    tail.map_err(|_| GovernanceError::Malformed)
}

pub fn encode_record(r: &GovernanceRecordV1) -> Vec<u8> {
    let mut sorted = r.parents.clone();
    sorted.sort_unstable();
    sorted.dedup();
    let mut buf = Vec::new();
    let mut enc = Encoder::new(&mut buf);
    encode_envelope(r, &sorted, &mut enc).expect("in-memory encode cannot fail");
    buf
}

fn dkey(d: &mut Decoder<'_>, want: u64) -> Result<(), GovernanceError> {
    if d.u64().map_err(|_| GovernanceError::Malformed)? != want {
        return Err(GovernanceError::Malformed);
    }
    Ok(())
}

fn d32(d: &mut Decoder<'_>) -> Result<[u8; 32], GovernanceError> {
    let b = d.bytes().map_err(|_| GovernanceError::Malformed)?;
    <[u8; 32]>::try_from(b).map_err(|_| GovernanceError::Malformed)
}

pub fn decode_record(bytes: &[u8]) -> Result<GovernanceRecordV1, GovernanceError> {
    if bytes.len() > MAX_GOVERNANCE_RECORD_BYTES {
        return Err(GovernanceError::RecordTooLarge {
            bytes: bytes.len(),
            max: MAX_GOVERNANCE_RECORD_BYTES,
        });
    }
    let mut d = Decoder::new(bytes);
    match d.map().map_err(|_| GovernanceError::Malformed)? {
        Some(11) => {}
        _ => return Err(GovernanceError::Malformed),
    }
    dkey(&mut d, 0)?;
    let schema = d.str().map_err(|_| GovernanceError::Malformed)?;
    if schema != GOVERNANCE_RECORD_SCHEMA {
        return Err(GovernanceError::Malformed);
    }
    dkey(&mut d, 1)?;
    let tag = d.u64().map_err(|_| GovernanceError::Malformed)?;
    let kind = RecordKind::from_tag(tag)?;
    dkey(&mut d, 2)?;
    let namespace = d32(&mut d)?;
    dkey(&mut d, 3)?;
    let n = d
        .array()
        .map_err(|_| GovernanceError::Malformed)?
        .ok_or(GovernanceError::Malformed)?;
    if n as usize > MAX_PARENTS {
        return Err(GovernanceError::ParentsInvalid);
    }
    let mut parents = Vec::with_capacity(n as usize);
    for _ in 0..n {
        parents.push(d32(&mut d)?);
    }
    // Strictly ascending = sorted AND deduplicated.
    if parents.windows(2).any(|w| w[0] >= w[1]) {
        return Err(GovernanceError::ParentsInvalid);
    }
    dkey(&mut d, 4)?;
    let actor_id = d32(&mut d)?;
    dkey(&mut d, 5)?;
    let receiver = d32(&mut d)?;
    dkey(&mut d, 6)?;
    let sequence = d.u64().map_err(|_| GovernanceError::Malformed)?;
    dkey(&mut d, 7)?;
    let prev_actor_record =
        if d.datatype().map_err(|_| GovernanceError::Malformed)? == minicbor::data::Type::Null {
            d.null().map_err(|_| GovernanceError::Malformed)?;
            None
        } else {
            Some(d32(&mut d)?)
        };
    dkey(&mut d, 8)?;
    let authorizing_fingerprint = d32(&mut d)?;
    dkey(&mut d, 9)?;
    let body = decode_body(kind, &mut d)?;
    if kind_of(&body) != kind {
        return Err(GovernanceError::Malformed);
    }
    dkey(&mut d, 10)?;
    let created_display_micros = d.u64().map_err(|_| GovernanceError::Malformed)?;
    if d.position() != bytes.len() {
        return Err(GovernanceError::TrailingBytes);
    }
    let record = GovernanceRecordV1 {
        kind,
        namespace,
        parents,
        actor_id,
        receiver,
        sequence,
        prev_actor_record,
        authorizing_fingerprint,
        body,
        created_display_micros,
    };
    // Canonicality proof: re-encoding must reproduce the input byte-for-byte.
    if encode_record(&record) != bytes {
        return Err(GovernanceError::Malformed);
    }
    Ok(record)
}

pub fn record_id(r: &GovernanceRecordV1) -> RecordId {
    let mut h = Sha256::new();
    h.update(RECORD_ID_DOMAIN);
    h.update(encode_record(r));
    h.finalize().into()
}

/// Emits the envelope with caller-supplied (possibly unsorted) parent bytes so
/// the decoder's sort/dedup guard is exercisable; `encode_record` always sorts.
#[cfg(test)]
pub fn test_only_encode_with_parents(r: &GovernanceRecordV1, parents: &[[u8; 32]]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut enc = Encoder::new(&mut buf);
    encode_envelope(r, parents, &mut enc).expect("test encode");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genesis() -> GovernanceRecordV1 {
        GovernanceRecordV1 {
            kind: RecordKind::Genesis,
            namespace: [9u8; 32],
            parents: vec![],
            actor_id: [1u8; 32],
            receiver: [2u8; 32],
            sequence: 0,
            prev_actor_record: None,
            authorizing_fingerprint: [7u8; 32],
            body: Body::Genesis,
            created_display_micros: 1000,
        }
    }

    #[test]
    fn record_round_trips_canonically() {
        let r = genesis();
        assert_eq!(decode_record(&encode_record(&r)).unwrap(), r);
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let mut b = encode_record(&genesis());
        b.push(0);
        assert_eq!(decode_record(&b), Err(GovernanceError::TrailingBytes));
    }

    #[test]
    fn descending_or_dup_parents_are_rejected() {
        // encode_record always emits sorted parents, so craft the CBOR directly.
        let bytes = test_only_encode_with_parents(&genesis(), &[[9u8; 32], [1u8; 32]]);
        assert_eq!(decode_record(&bytes), Err(GovernanceError::ParentsInvalid));
        let dup = test_only_encode_with_parents(&genesis(), &[[5u8; 32], [5u8; 32]]);
        assert_eq!(decode_record(&dup), Err(GovernanceError::ParentsInvalid));
    }

    #[test]
    fn oversized_record_is_rejected_before_decode() {
        assert_eq!(
            decode_record(&vec![0u8; MAX_GOVERNANCE_RECORD_BYTES + 1]),
            Err(GovernanceError::RecordTooLarge {
                bytes: MAX_GOVERNANCE_RECORD_BYTES + 1,
                max: MAX_GOVERNANCE_RECORD_BYTES
            })
        );
    }

    #[test]
    fn record_id_is_domain_separated_from_the_fingerprint_domain() {
        let r = genesis();
        let raw: [u8; 32] = Sha256::digest(encode_record(&r)).into();
        assert_ne!(record_id(&r), raw);
        let mut h = Sha256::new();
        h.update(b"riot/meadowcap-fingerprint/v1");
        h.update(encode_record(&r));
        let mc: [u8; 32] = h.finalize().into();
        assert_ne!(
            record_id(&r),
            mc,
            "record-id domain must not collide with the fingerprint domain"
        );
    }
}
