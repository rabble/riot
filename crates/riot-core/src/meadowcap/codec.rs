//! Canonical read-capability codec and ceiling-bounded decoders. Reuses the
//! write-cap codec in `crate::willow` rather than duplicating it.

use ufotofu::codec_prelude::EncodableExt;
use willow25::authorisation::ReadCapability;
use willow25::authorisation::WriteCapability;

use super::MeadowcapError;
use super::{MAX_CAPABILITY_ENCODED_BYTES, MAX_DELEGATION_DEPTH};

/// Canonical encoding of a read capability (mirrors `willow::encode_capability`
/// for the write path).
pub fn encode_read_capability(capability: &ReadCapability) -> Vec<u8> {
    pollster::block_on(capability.new_vec_storing_encoding())
}

/// Canonical decode of a read capability, rejecting trailing bytes. Reuses the
/// shared `willow::decode_canonic_exact` guard.
pub fn decode_read_capability_canonic(bytes: &[u8]) -> Result<ReadCapability, MeadowcapError> {
    crate::willow::decode_canonic_exact::<ReadCapability>(bytes).map_err(|e| match e {
        crate::willow::WillowError::TrailingBytes => MeadowcapError::TrailingBytes,
        _ => MeadowcapError::Malformed,
    })
}

/// Decode a write capability from canonical bytes, enforcing the byte and
/// delegation-depth ceilings *before* returning it for verification.
pub fn decode_write_capability_bounded(bytes: &[u8]) -> Result<WriteCapability, MeadowcapError> {
    if bytes.len() > MAX_CAPABILITY_ENCODED_BYTES {
        return Err(MeadowcapError::CapabilityTooLarge {
            bytes: bytes.len(),
            max: MAX_CAPABILITY_ENCODED_BYTES,
        });
    }
    let cap = crate::willow::decode_capability_canonic(bytes).map_err(|e| match e {
        crate::willow::WillowError::TrailingBytes => MeadowcapError::TrailingBytes,
        _ => MeadowcapError::Malformed,
    })?;
    let depth = cap.delegations().len();
    if depth > MAX_DELEGATION_DEPTH {
        return Err(MeadowcapError::ChainTooDeep {
            depth,
            max: MAX_DELEGATION_DEPTH,
        });
    }
    Ok(cap)
}

/// Read-capability analogue of `decode_write_capability_bounded`.
pub fn decode_read_capability_bounded(bytes: &[u8]) -> Result<ReadCapability, MeadowcapError> {
    if bytes.len() > MAX_CAPABILITY_ENCODED_BYTES {
        return Err(MeadowcapError::CapabilityTooLarge {
            bytes: bytes.len(),
            max: MAX_CAPABILITY_ENCODED_BYTES,
        });
    }
    let cap = decode_read_capability_canonic(bytes)?;
    let depth = cap.delegations().len();
    if depth > MAX_DELEGATION_DEPTH {
        return Err(MeadowcapError::ChainTooDeep {
            depth,
            max: MAX_DELEGATION_DEPTH,
        });
    }
    Ok(cap)
}

#[cfg(test)]
mod tests {
    use super::super::create::new_communal_read;
    use super::*;
    use willow25::prelude::{NamespaceId, SubspaceSecret};

    fn a_read_cap() -> ReadCapability {
        let ns = NamespaceId::from_bytes(&[16u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[7u8; 32]).corresponding_subspace_id();
        new_communal_read(ns, receiver)
    }

    #[test]
    fn read_capability_roundtrips_canonically() {
        let cap = a_read_cap();
        let bytes = encode_read_capability(&cap);
        let decoded = decode_read_capability_canonic(&bytes).expect("canonical decode");
        assert_eq!(decoded, cap);
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let cap = a_read_cap();
        let mut bytes = encode_read_capability(&cap);
        bytes.push(0x00);
        assert_eq!(
            decode_read_capability_canonic(&bytes),
            Err(MeadowcapError::TrailingBytes)
        );
    }

    #[test]
    fn garbage_bytes_are_malformed() {
        assert_eq!(
            decode_read_capability_canonic(&[0xff, 0xff, 0xff, 0xff]),
            Err(MeadowcapError::Malformed)
        );
    }

    #[test]
    fn owned_read_cap_is_owned_full_area_and_round_trips() {
        // Exercises new_owned_read (spec line 153): the owned read-capability
        // creation path is otherwise untested. Owned genesis grants Area::full()
        // with Read access, and its canonical encoding round-trips.
        use super::super::create::new_owned_read;
        use willow25::authorisation::raw::AccessMode;
        use willow25::prelude::{Area, NamespaceSecret};

        let ns = NamespaceSecret::from_bytes(&[3u8; 32]);
        let receiver = SubspaceSecret::from_bytes(&[6u8; 32]).corresponding_subspace_id();
        let cap = new_owned_read(&ns, receiver.clone());
        assert!(cap.is_owned());
        assert_eq!(cap.receiver(), &receiver);
        assert_eq!(cap.granted_namespace(), &ns.corresponding_namespace_id());
        assert_eq!(cap.granted_area(), Area::full());
        assert_eq!(cap.genesis().access_mode(), AccessMode::Read);

        let bytes = encode_read_capability(&cap);
        assert_eq!(decode_read_capability_canonic(&bytes).expect("round-trip"), cap);
    }
}

#[cfg(test)]
mod ceiling_tests {
    use super::*;

    #[test]
    fn oversized_input_is_rejected_before_decode() {
        let bytes = vec![0u8; MAX_CAPABILITY_ENCODED_BYTES + 1];
        assert_eq!(
            decode_write_capability_bounded(&bytes),
            Err(MeadowcapError::CapabilityTooLarge {
                bytes: MAX_CAPABILITY_ENCODED_BYTES + 1,
                max: MAX_CAPABILITY_ENCODED_BYTES,
            })
        );
        assert_eq!(
            decode_read_capability_bounded(&bytes),
            Err(MeadowcapError::CapabilityTooLarge {
                bytes: MAX_CAPABILITY_ENCODED_BYTES + 1,
                max: MAX_CAPABILITY_ENCODED_BYTES,
            })
        );
    }
}
