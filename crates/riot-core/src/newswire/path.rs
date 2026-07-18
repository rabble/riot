//! Exact, closed Newswire v1 path families.

use crate::willow::{EntryId, Path};

use super::NewswireError;

const ROOT: &[u8] = b"newswire";
const VERSION: &[u8] = b"v1";
const DESCRIPTORS: &[u8] = b"descriptors";
const POSTS: &[u8] = b"posts";
const ACTIONS: &[u8] = b"actions";
const COMMENTS: &[u8] = b"comments";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewswirePathKind {
    Descriptor,
    Post { space_descriptor_entry_id: EntryId },
    EditorialAction { space_descriptor_entry_id: EntryId },
    Comment { space_descriptor_entry_id: EntryId },
}

pub fn newswire_path(
    kind: NewswirePathKind,
    tai_j2000_micros: u64,
    payload_digest: &[u8; 32],
) -> Result<Path, NewswireError> {
    let time = tai_j2000_micros.to_be_bytes();
    let result = match kind {
        NewswirePathKind::Descriptor => {
            Path::from_slices(&[ROOT, VERSION, DESCRIPTORS, &time, payload_digest])
        }
        NewswirePathKind::Post {
            space_descriptor_entry_id,
        } => Path::from_slices(&[
            ROOT,
            VERSION,
            &space_descriptor_entry_id,
            POSTS,
            &time,
            payload_digest,
        ]),
        NewswirePathKind::EditorialAction {
            space_descriptor_entry_id,
        } => Path::from_slices(&[
            ROOT,
            VERSION,
            &space_descriptor_entry_id,
            ACTIONS,
            &time,
            payload_digest,
        ]),
        NewswirePathKind::Comment {
            space_descriptor_entry_id,
        } => Path::from_slices(&[
            ROOT,
            VERSION,
            &space_descriptor_entry_id,
            COMMENTS,
            &time,
            payload_digest,
        ]),
    };
    result.map_err(|_| NewswireError::PathInvalid)
}

pub fn classify_newswire_path(path: &Path) -> Option<(NewswirePathKind, u64, [u8; 32])> {
    let mut components = path.components();
    if components.next()?.as_ref() != ROOT || components.next()?.as_ref() != VERSION {
        return None;
    }

    let third = components.next()?;
    if third.as_ref() == DESCRIPTORS {
        let time = components.next()?;
        let digest = components.next()?;
        if components.next().is_some() {
            return None;
        }
        return Some((
            NewswirePathKind::Descriptor,
            u64::from_be_bytes(time.as_ref().try_into().ok()?),
            digest.as_ref().try_into().ok()?,
        ));
    }

    let space_descriptor_entry_id = third.as_ref().try_into().ok()?;
    let family = components.next()?;
    let kind = if family.as_ref() == POSTS {
        NewswirePathKind::Post {
            space_descriptor_entry_id,
        }
    } else if family.as_ref() == ACTIONS {
        NewswirePathKind::EditorialAction {
            space_descriptor_entry_id,
        }
    } else if family.as_ref() == COMMENTS {
        NewswirePathKind::Comment {
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
    fn all_families_roundtrip_exact_binary_components() {
        let id = [3; 32];
        let digest = [4; 32];
        for kind in [
            NewswirePathKind::Descriptor,
            NewswirePathKind::Post {
                space_descriptor_entry_id: id,
            },
            NewswirePathKind::EditorialAction {
                space_descriptor_entry_id: id,
            },
            NewswirePathKind::Comment {
                space_descriptor_entry_id: id,
            },
        ] {
            let path = newswire_path(kind, 0x0102_0304_0506_0708, &digest).unwrap();
            assert_eq!(
                classify_newswire_path(&path),
                Some((kind, 0x0102_0304_0506_0708, digest))
            );
            assert_eq!(
                path.components()
                    .nth(path.component_count() - 2)
                    .unwrap()
                    .as_ref(),
                &[1, 2, 3, 4, 5, 6, 7, 8]
            );
        }
    }

    #[test]
    fn malformed_counts_lengths_and_reserved_prefixes_are_closed() {
        let malformed = [
            Path::from_slices(&[ROOT, VERSION]).unwrap(),
            Path::from_slices(&[ROOT, VERSION, DESCRIPTORS, &[0; 7], &[0; 32]]).unwrap(),
            Path::from_slices(&[ROOT, VERSION, DESCRIPTORS, &[0; 8], &[0; 31]]).unwrap(),
            Path::from_slices(&[ROOT, VERSION, &[0; 31], POSTS, &[0; 8], &[0; 32]]).unwrap(),
            Path::from_slices(&[ROOT, VERSION, &[0; 32], b"alerts", &[0; 8], &[0; 32]]).unwrap(),
            Path::from_slices(&[ROOT, VERSION, &[0; 32], POSTS, &[0; 8], &[0; 32], b"extra"])
                .unwrap(),
            Path::from_slices(&[b"other", VERSION, DESCRIPTORS, &[0; 8], &[0; 32]]).unwrap(),
            Path::from_slices(&[ROOT, VERSION, &[0; 32], COMMENTS, &[0; 7], &[0; 32]]).unwrap(),
            Path::from_slices(&[
                ROOT, VERSION, &[0; 32], COMMENTS, &[0; 8], &[0; 32], b"extra",
            ])
            .unwrap(),
        ];
        for path in malformed {
            assert_eq!(classify_newswire_path(&path), None, "{path:?}");
        }
    }

    #[test]
    fn excessive_component_count_is_rejected() {
        let mut components: Vec<&[u8]> = vec![ROOT, VERSION, DESCRIPTORS];
        components.extend(std::iter::repeat_n(b"extra".as_slice(), 61));
        let path = Path::from_slices(&components).unwrap();
        assert_eq!(path.component_count(), 64);
        assert_eq!(classify_newswire_path(&path), None);
    }

    #[test]
    fn willow_rejects_an_impossible_component_before_classification() {
        assert!(Path::from_slices(&[&vec![0; 4097]]).is_err());
    }
}
