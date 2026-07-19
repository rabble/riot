//! Canonical positional-CBOR codec primitives.
//!
//! All anchor-protocol records use deterministic positional CBOR arrays with
//! definite-length containers, minimal integer encodings, textual closed
//! discriminants, and sorted collections — never maps, numeric enum tags, or
//! indefinite lengths (design: "Canonical Anchor Records"). This module provides
//! the shared building blocks; each record type implements [`CanonicalRecord`]
//! and gets canonicality enforcement for free through [`decode_canonical`], which
//! bounds the input, decodes, rejects trailing bytes, and requires byte-identical
//! re-encoding.
//!
//! The re-encode check is what makes canonicality total: any non-minimal integer,
//! indefinite container that still parses, reordered field, or alternate grammar
//! produces different canonical bytes and is rejected, without every decoder
//! needing to hand-check each case.

use minicbor::data::Type;
use minicbor::Decoder;

/// The closed set of ways canonical decoding can fail.
///
/// Deliberately exhaustive (not `#[non_exhaustive]`): the design fixes the set of
/// rejection reasons, and downstream error mapping should stay total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Input exceeded the caller-provided maximum record size before decoding.
    TooLarge {
        /// The caller's byte ceiling.
        limit: usize,
        /// The actual input length.
        actual: usize,
    },
    /// Bytes remained after a complete record was decoded.
    TrailingBytes,
    /// The value decoded but did not re-encode to byte-identical canonical bytes
    /// (non-minimal integer, indefinite container, reordered fields, …).
    NonCanonical,
    /// An indefinite-length container appeared where a definite one is required.
    IndefiniteLength,
    /// A CBOR item had the wrong major type for the position.
    UnexpectedType,
    /// A positional array had the wrong element count for its record type.
    WrongArrayLength {
        /// The count the record type requires.
        expected: u64,
        /// The count that was present.
        actual: u64,
    },
    /// A version-tagged record carried an unrecognized version integer.
    UnknownVersion(u64),
    /// A closed enum / sum carried an unrecognized `snake_case` name.
    UnknownVariant,
    /// A set's elements were not in strictly ascending canonical-byte order.
    UnsortedSet,
    /// A set contained a duplicate element.
    DuplicateSetMember,
    /// A length-prefixed byte string, text, or collection exceeded its field cap.
    LengthOutOfRange,
    /// A generic malformed-CBOR decode failure.
    Malformed,
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CodecError::TooLarge { limit, actual } => {
                write!(f, "record of {actual} bytes exceeds limit of {limit}")
            }
            CodecError::TrailingBytes => f.write_str("trailing bytes after record"),
            CodecError::NonCanonical => f.write_str("non-canonical encoding"),
            CodecError::IndefiniteLength => f.write_str("indefinite-length container"),
            CodecError::UnexpectedType => f.write_str("unexpected CBOR type"),
            CodecError::WrongArrayLength { expected, actual } => {
                write!(f, "array length {actual}, expected {expected}")
            }
            CodecError::UnknownVersion(v) => write!(f, "unknown record version {v}"),
            CodecError::UnknownVariant => f.write_str("unknown enum/sum discriminant"),
            CodecError::UnsortedSet => f.write_str("set elements not in canonical order"),
            CodecError::DuplicateSetMember => f.write_str("duplicate set member"),
            CodecError::LengthOutOfRange => f.write_str("length outside field bounds"),
            CodecError::Malformed => f.write_str("malformed CBOR"),
        }
    }
}

impl std::error::Error for CodecError {}

/// A record whose wire form is canonical positional CBOR.
///
/// Implementors write their exact positional array in [`encode_canonical`] and
/// read the same fields in [`decode_fields`]; [`decode_canonical`] wraps
/// `decode_fields` with the bound/trailing/re-encode checks that guarantee
/// canonicality.
///
/// [`encode_canonical`]: CanonicalRecord::encode_canonical
/// [`decode_fields`]: CanonicalRecord::decode_fields
pub trait CanonicalRecord: Sized {
    /// Produce the complete canonical byte string for this record.
    fn encode_canonical(&self) -> Result<Vec<u8>, CodecError>;
    /// Read the record's fields from a decoder positioned at its first byte.
    /// Implementations must not check trailing bytes or re-encoding; that is done
    /// by [`decode_canonical`].
    fn decode_fields(decoder: &mut Decoder<'_>) -> Result<Self, CodecError>;
}

/// Decode a canonical record, rejecting oversize input, trailing bytes, and any
/// input whose re-encoding is not byte-identical.
///
/// `maximum` bounds allocation: input longer than `maximum` is rejected before
/// any field is read.
pub fn decode_canonical<T: CanonicalRecord>(bytes: &[u8], maximum: usize) -> Result<T, CodecError> {
    if bytes.len() > maximum {
        return Err(CodecError::TooLarge {
            limit: maximum,
            actual: bytes.len(),
        });
    }
    let mut decoder = Decoder::new(bytes);
    let value = T::decode_fields(&mut decoder)?;
    if decoder.position() != bytes.len() {
        return Err(CodecError::TrailingBytes);
    }
    let reencoded = value.encode_canonical()?;
    if reencoded != bytes {
        return Err(CodecError::NonCanonical);
    }
    Ok(value)
}

/// Read a definite-length array header, rejecting the indefinite form.
pub fn definite_array(d: &mut Decoder<'_>) -> Result<u64, CodecError> {
    match d.datatype().map_err(|_| CodecError::Malformed)? {
        Type::Array => d
            .array()
            .map_err(|_| CodecError::Malformed)?
            .ok_or(CodecError::IndefiniteLength),
        Type::ArrayIndef => Err(CodecError::IndefiniteLength),
        Type::Map | Type::MapIndef => Err(CodecError::UnexpectedType),
        _ => Err(CodecError::UnexpectedType),
    }
}

/// Read a definite array header and require an exact element count.
pub fn expect_array(d: &mut Decoder<'_>, expected: u64) -> Result<(), CodecError> {
    let actual = definite_array(d)?;
    if actual != expected {
        return Err(CodecError::WrongArrayLength { expected, actual });
    }
    Ok(())
}

/// Read a leading version integer and require it to equal `expected`, else
/// [`CodecError::UnknownVersion`].
pub fn read_version(d: &mut Decoder<'_>, expected: u64) -> Result<(), CodecError> {
    let v = d.u64().map_err(|_| CodecError::Malformed)?;
    if v != expected {
        return Err(CodecError::UnknownVersion(v));
    }
    Ok(())
}

/// Read a byte string no longer than `max` bytes.
pub fn read_bytes_max(d: &mut Decoder<'_>, max: usize) -> Result<Vec<u8>, CodecError> {
    if d.datatype().map_err(|_| CodecError::Malformed)? != Type::Bytes {
        return Err(CodecError::UnexpectedType);
    }
    let b = d.bytes().map_err(|_| CodecError::Malformed)?;
    if b.len() > max {
        return Err(CodecError::LengthOutOfRange);
    }
    Ok(b.to_vec())
}

/// Read a fixed-width byte string of exactly `N` bytes.
pub fn read_fixed_bytes<const N: usize>(d: &mut Decoder<'_>) -> Result<[u8; N], CodecError> {
    if d.datatype().map_err(|_| CodecError::Malformed)? != Type::Bytes {
        return Err(CodecError::UnexpectedType);
    }
    let b = d.bytes().map_err(|_| CodecError::Malformed)?;
    <[u8; N]>::try_from(b).map_err(|_| CodecError::LengthOutOfRange)
}

/// Read a UTF-8 text string whose byte length is within `[min, max]`.
pub fn read_text_max(d: &mut Decoder<'_>, min: usize, max: usize) -> Result<String, CodecError> {
    if d.datatype().map_err(|_| CodecError::Malformed)? != Type::String {
        return Err(CodecError::UnexpectedType);
    }
    let s = d.str().map_err(|_| CodecError::Malformed)?;
    if s.len() < min || s.len() > max {
        return Err(CodecError::LengthOutOfRange);
    }
    Ok(s.to_string())
}

/// Read a closed-enum / sum discriminant: a `snake_case` text token. The caller
/// matches the returned name and maps an unrecognized one to
/// [`CodecError::UnknownVariant`].
pub fn read_discriminant(d: &mut Decoder<'_>, max: usize) -> Result<String, CodecError> {
    read_text_max(d, 1, max)
}

/// Is the next item CBOR `null`? Used for optional fields that are `null` or a
/// single typed value without changing the array length.
pub fn peek_null(d: &Decoder<'_>) -> Result<bool, CodecError> {
    Ok(d.datatype().map_err(|_| CodecError::Malformed)? == Type::Null)
}

/// Consume a `null`.
pub fn read_null(d: &mut Decoder<'_>) -> Result<(), CodecError> {
    d.null().map_err(|_| CodecError::Malformed)
}

/// Enforce strictly-ascending canonical-byte order across a set, one adjacent
/// pair at a time. Returns [`CodecError::UnsortedSet`] on a decrease and
/// [`CodecError::DuplicateSetMember`] on equality.
pub fn assert_set_order(previous: Option<&[u8]>, current: &[u8]) -> Result<(), CodecError> {
    if let Some(prev) = previous {
        match current.cmp(prev) {
            core::cmp::Ordering::Greater => {}
            core::cmp::Ordering::Equal => return Err(CodecError::DuplicateSetMember),
            core::cmp::Ordering::Less => return Err(CodecError::UnsortedSet),
        }
    }
    Ok(())
}

/// Sort a set's canonical element bytes into the wire order, rejecting duplicates.
pub fn sort_canonical_set(mut elements: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, CodecError> {
    elements.sort();
    for pair in elements.windows(2) {
        if pair[0] == pair[1] {
            return Err(CodecError::DuplicateSetMember);
        }
    }
    Ok(elements)
}
