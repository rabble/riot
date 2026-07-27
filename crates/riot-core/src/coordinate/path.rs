//! Exact, closed Coordinate v1 path families.
//!
//! Coordinate records live under the reserved prefix `coordinate/v1/…`. Unlike
//! newswire, Coordinate has no descriptor family of its own: every record binds
//! to an existing newswire space descriptor by `space_descriptor_entry_id`, so
//! all Coordinate paths are `coordinate / v1 / <descriptor id> / <family> /
//! <tai_j2000_micros> / <payload digest>`.
//!
//! WU-1 lands only the item family; status/verification/action families are
//! added in later work units by extending [`CoordinatePathKind`] (the
//! [`coordinate_path`] match is exhaustive and compiler-forces new arms).

use crate::willow::{EntryId, Path};

use super::CoordinateError;

const ROOT: &[u8] = b"coordinate";
const VERSION: &[u8] = b"v1";
const ITEMS: &[u8] = b"items";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatePathKind {
    Item { space_descriptor_entry_id: EntryId },
}

pub fn coordinate_path(
    kind: CoordinatePathKind,
    tai_j2000_micros: u64,
    payload_digest: &[u8; 32],
) -> Result<Path, CoordinateError> {
    let time = tai_j2000_micros.to_be_bytes();
    let result = match kind {
        CoordinatePathKind::Item {
            space_descriptor_entry_id,
        } => Path::from_slices(&[
            ROOT,
            VERSION,
            &space_descriptor_entry_id,
            ITEMS,
            &time,
            payload_digest,
        ]),
    };
    result.map_err(|_| CoordinateError::PathInvalid)
}

pub fn classify_coordinate_path(path: &Path) -> Option<(CoordinatePathKind, u64, [u8; 32])> {
    let mut components = path.components();
    if components.next()?.as_ref() != ROOT || components.next()?.as_ref() != VERSION {
        return None;
    }

    let space_descriptor_entry_id = components.next()?.as_ref().try_into().ok()?;
    let family = components.next()?;
    let kind = if family.as_ref() == ITEMS {
        CoordinatePathKind::Item {
            space_descriptor_entry_id,
        }
    } else {
        return None;
    };
    let time = components.next()?;
    let digest = components.next()?;
    if components.next().is_some() {
        return None;
    }
    Some((
        kind,
        u64::from_be_bytes(time.as_ref().try_into().ok()?),
        digest.as_ref().try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_family_roundtrips_exact_binary_components() {
        let id = [3; 32];
        let digest = [4; 32];
        let kind = CoordinatePathKind::Item {
            space_descriptor_entry_id: id,
        };
        let path = coordinate_path(kind, 0x0102_0304_0506_0708, &digest).unwrap();
        assert_eq!(
            classify_coordinate_path(&path),
            Some((kind, 0x0102_0304_0506_0708, digest))
        );
        // The time component sits immediately before the digest, big-endian.
        assert_eq!(
            path.components()
                .nth(path.component_count() - 2)
                .unwrap()
                .as_ref(),
            &[1, 2, 3, 4, 5, 6, 7, 8]
        );
    }

    #[test]
    fn malformed_counts_lengths_and_reserved_prefixes_are_closed() {
        let malformed = [
            // Too short — no family.
            Path::from_slices(&[ROOT, VERSION, &[0; 32]]).unwrap(),
            // Descriptor id is not 32 bytes.
            Path::from_slices(&[ROOT, VERSION, &[0; 31], ITEMS, &[0; 8], &[0; 32]]).unwrap(),
            // Unknown family.
            Path::from_slices(&[ROOT, VERSION, &[0; 32], b"posts", &[0; 8], &[0; 32]]).unwrap(),
            // Time component is not 8 bytes.
            Path::from_slices(&[ROOT, VERSION, &[0; 32], ITEMS, &[0; 7], &[0; 32]]).unwrap(),
            // Digest is not 32 bytes.
            Path::from_slices(&[ROOT, VERSION, &[0; 32], ITEMS, &[0; 8], &[0; 31]]).unwrap(),
            // Trailing component.
            Path::from_slices(&[ROOT, VERSION, &[0; 32], ITEMS, &[0; 8], &[0; 32], b"extra"])
                .unwrap(),
            // Wrong root prefix.
            Path::from_slices(&[b"newswire", VERSION, &[0; 32], ITEMS, &[0; 8], &[0; 32]]).unwrap(),
            // Wrong version.
            Path::from_slices(&[ROOT, b"v2", &[0; 32], ITEMS, &[0; 8], &[0; 32]]).unwrap(),
        ];
        for path in malformed {
            assert_eq!(classify_coordinate_path(&path), None, "{path:?}");
        }
    }

    #[test]
    fn excessive_component_count_is_rejected() {
        let mut components: Vec<&[u8]> = vec![ROOT, VERSION, &[0; 32], ITEMS];
        components.extend(std::iter::repeat_n(b"extra".as_slice(), 60));
        let path = Path::from_slices(&components).unwrap();
        assert_eq!(classify_coordinate_path(&path), None);
    }
}
