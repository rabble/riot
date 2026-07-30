use anyhash::{Hasher, HasherWrite};

use crate::{
    CHUNK_SIZE, HashChunkContext, HashInnerContext, WIDTH, William3Digest,
    generic::{BabHasher as GenericHasher, BabInstantiation},
    hash_chunk, hash_inner,
};

/// A stateful hasher for incrementally computing WILLIAM3 digests.
///
/// Use the [`anyhash::Hasher`] and [`anyhash::HasherWrite`] traits to compute digests. This crate reexports them at the root for convenience.
///
/// ```
/// # #[cfg(feature = "william3")] {
/// use bab_rs::{William3Hasher, William3Digest, batch_hash, WIDTH, Hasher, HasherWrite};
/// let mut hasher = William3Hasher::new();
/// hasher.write(&[0, 1, 2]);
/// hasher.write(&[3, 4]);
/// let digest1 = hasher.finish();
///
/// let mut batch_digest1 = William3Digest::default();
/// batch_hash(&[0, 1, 2, 3, 4], &mut batch_digest1);
///
/// assert_eq!(digest1, batch_digest1);
///
/// // You can continue using the hasher after calling `finish`.
/// hasher.write(&[5, 6]);
///
/// let mut batch_digest2 = William3Digest::default();
/// batch_hash(&[0, 1, 2, 3, 4, 5, 6], &mut batch_digest2);
///
/// assert_eq!(
///     hasher.finish(),
///     batch_digest2,
/// );
/// # }
/// ```
pub struct William3Hasher {
    hasher: GenericHasher<WIDTH, CHUNK_SIZE, HashChunkContext, HashInnerContext>,
}

impl William3Hasher {
    /// Creates a new WILLIAM3 hasher.
    pub fn new() -> Self {
        let bab_instantiation = BabInstantiation {
            hash_chunk,
            hash_inner,
            hash_chunk_context: HashChunkContext::new(),
            hash_inner_context: HashInnerContext::new(),
        };

        Self {
            hasher: GenericHasher::new(bab_instantiation),
        }
    }

    /// Creates a new WILLIAM3 hasher for keyed hashing.
    pub fn new_keyed(key: [u32; 8]) -> Self {
        let bab_instantiation = BabInstantiation {
            hash_chunk,
            hash_inner,
            hash_chunk_context: HashChunkContext::new_keyed(key),
            hash_inner_context: HashInnerContext::new_keyed(key),
        };

        Self {
            hasher: GenericHasher::new(bab_instantiation),
        }
    }
}

impl HasherWrite for William3Hasher {
    fn write(&mut self, bytes: &[u8]) {
        self.hasher.write(bytes)
    }
}

impl Hasher<William3Digest> for William3Hasher {
    fn finish(&self) -> William3Digest {
        self.hasher.finish().into()
    }
}

impl Default for William3Hasher {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn test_hasher() {
    let data = [17u8; CHUNK_SIZE * 16];

    for len in 0..data.len() {
        let mut digest_batch = [0; WIDTH].into();
        crate::batch_hash(&data[..len], &mut digest_batch);

        let mut hasher = William3Hasher::new();
        hasher.write(&data[..len]);
        let digest_hasher = hasher.finish();

        assert_eq!(digest_hasher, digest_batch);
    }
}
