use crate::{
    CHUNK_SIZE, HashChunkContext, HashInnerContext, WIDTH, William3Digest,
    generic::BabInstantiation, hash_chunk, hash_inner,
};

/// Computes the WILLIAM3 digest of the given input bytes, and writes it into `out`.
///
/// This is the simplemost hashing API; it requires the full string to be available at once. See the [William3`Hasher`](crate::William3Hasher) API for incremental hashing.
pub fn batch_hash(bytes: &[u8], out: &mut William3Digest) {
    let (hash_chunk_context, hash_inner_context) =
        (HashChunkContext::new(), HashInnerContext::new());

    let bab_instantiation = BabInstantiation {
        hash_chunk,
        hash_inner,
        hash_chunk_context,
        hash_inner_context,
    };

    crate::generic::batch_hash::<WIDTH, CHUNK_SIZE, _, _>(&bab_instantiation, bytes, &mut out.0);
}

/// Computes the keyed WILLIAM3 digest of the given input bytes for a given `key`, and writes the digest into `out`.
///
/// This is the simplemost hashing API; it requires the full string to be available at once. See the [`Hasher`](crate::Hasher) API for incremental hashing.
pub fn batch_hash_keyed(bytes: &[u8], key: [u32; 8], out: &mut William3Digest) {
    let (hash_chunk_context, hash_inner_context) = (
        HashChunkContext::new_keyed(key),
        HashInnerContext::new_keyed(key),
    );

    let bab_instantiation = BabInstantiation {
        hash_chunk,
        hash_inner,
        hash_chunk_context,
        hash_inner_context,
    };

    crate::generic::batch_hash::<WIDTH, CHUNK_SIZE, _, _>(&bab_instantiation, bytes, &mut out.0);
}

#[cfg(all(feature = "william3", feature = "std"))]
#[test]
fn generate_test_data() {
    let mut data = vec![];
    data.push(vec![]);
    data.push(vec![0]);
    data.push(vec![0, 0]);
    data.push(vec![0, 1]);
    data.push(vec![0; 63]);
    data.push(vec![0; 64]);
    data.push(vec![0; 65]);
    data.push(vec![0; 1023]);
    data.push(vec![1; 1024]);
    data.push(vec![1; 1025]);
    data.push(vec![1; 1087]);
    data.push(vec![1; 1088]);
    data.push(vec![1; 1089]);
    data.push(vec![1; 3072]); // 2048 + 1024
    data.push(vec![1; 3073]); // 2048 + 1024 + 1, first tree on four leaves
    data.push(vec![3; 4097]);

    let mut ascending1 = vec![];
    for i in 0..800 {
        ascending1.push((i % 256) as u8);
    }
    let mut ascending2 = vec![];
    for i in 0..1600 {
        ascending2.push((i % 256) as u8);
    }

    data.push(ascending1);
    data.push(ascending2);

    for bytes in data {
        let mut digest_batch = [0; WIDTH].into();
        crate::batch_hash(&bytes[..], &mut digest_batch);

        println!(
            "input ({:?} bytes):\n{:?}\n\ndigest:\n{:?}\n\n======\n\n",
            bytes.len(),
            &bytes[..],
            digest_batch.as_bytes()
        );
    }
}

#[test]
fn test_expected_hashes() {
    let mut data = vec![];
    data.push((
        vec![],
        [
            150, 211, 76, 84, 120, 69, 130, 49, 227, 100, 118, 121, 82, 170, 234, 2, 163, 29, 34,
            3, 198, 111, 67, 101, 105, 46, 249, 31, 53, 16, 104, 210,
        ],
    ));
    data.push((
        vec![0],
        [
            79, 65, 176, 115, 198, 130, 113, 226, 5, 85, 74, 19, 90, 208, 3, 225, 171, 29, 183,
            201, 89, 81, 47, 90, 173, 29, 169, 147, 79, 79, 192, 87,
        ],
    ));
    data.push((
        vec![0, 0],
        [
            173, 77, 89, 179, 237, 27, 198, 66, 38, 175, 99, 146, 218, 115, 64, 3, 9, 186, 77, 8,
            210, 63, 108, 183, 215, 250, 245, 214, 111, 2, 245, 121,
        ],
    ));
    data.push((
        vec![0, 1],
        [
            198, 254, 38, 32, 195, 167, 244, 170, 69, 27, 91, 205, 254, 192, 53, 114, 248, 196,
            144, 129, 182, 100, 41, 122, 23, 31, 183, 199, 122, 155, 240, 191,
        ],
    ));
    data.push((
        vec![0; 63],
        [
            6, 18, 53, 4, 161, 118, 167, 21, 4, 110, 103, 37, 248, 79, 90, 130, 225, 147, 173, 171,
            12, 199, 19, 192, 43, 145, 103, 11, 69, 253, 50, 122,
        ],
    ));
    data.push((
        vec![0; 64],
        [
            45, 15, 76, 38, 148, 217, 210, 239, 109, 30, 192, 102, 72, 242, 255, 71, 227, 171, 85,
            143, 59, 195, 232, 222, 3, 222, 108, 10, 143, 125, 146, 9,
        ],
    ));
    data.push((
        vec![0; 65],
        [
            149, 189, 201, 68, 209, 106, 93, 66, 251, 144, 138, 170, 183, 113, 41, 50, 153, 125,
            160, 211, 116, 227, 115, 255, 5, 79, 180, 20, 235, 225, 48, 48,
        ],
    ));
    data.push((
        vec![0; 1023],
        [
            23, 25, 148, 21, 59, 247, 41, 160, 182, 187, 158, 183, 93, 209, 126, 105, 223, 65, 200,
            92, 121, 85, 111, 76, 35, 228, 29, 161, 243, 6, 23, 144,
        ],
    ));
    data.push((
        vec![1; 1024],
        [
            242, 177, 126, 219, 185, 216, 149, 65, 56, 20, 89, 207, 65, 27, 7, 116, 80, 61, 55,
            190, 199, 61, 17, 28, 25, 234, 232, 117, 202, 92, 25, 93,
        ],
    ));
    data.push((
        vec![1; 1025],
        [
            23, 171, 242, 62, 196, 160, 1, 7, 88, 83, 61, 223, 113, 29, 174, 240, 156, 45, 16, 229,
            222, 226, 13, 220, 131, 57, 71, 21, 56, 199, 220, 8,
        ],
    ));
    data.push((
        vec![1; 1087],
        [
            52, 36, 113, 74, 216, 3, 137, 29, 155, 75, 115, 138, 6, 206, 196, 80, 207, 195, 223,
            35, 6, 9, 76, 208, 77, 75, 212, 251, 124, 9, 63, 24,
        ],
    ));
    data.push((
        vec![1; 1088],
        [
            23, 254, 112, 63, 57, 202, 247, 79, 6, 248, 125, 175, 71, 101, 92, 27, 238, 163, 83,
            128, 144, 95, 62, 152, 1, 219, 133, 90, 39, 149, 65, 132,
        ],
    ));
    data.push((
        vec![1; 1089],
        [
            224, 200, 132, 239, 108, 200, 11, 216, 38, 7, 88, 185, 151, 116, 134, 143, 152, 213,
            65, 148, 94, 201, 194, 188, 83, 161, 70, 12, 165, 91, 115, 153,
        ],
    ));
    data.push((
        vec![1; 3072],
        [
            180, 42, 181, 191, 145, 233, 207, 155, 109, 182, 187, 197, 142, 186, 233, 250, 181, 53,
            83, 97, 61, 165, 233, 197, 36, 95, 212, 40, 154, 228, 24, 118,
        ],
    ));
    data.push((
        vec![1; 3073],
        [
            152, 103, 64, 183, 88, 190, 69, 174, 166, 22, 210, 76, 70, 171, 26, 47, 126, 149, 106,
            212, 241, 230, 152, 111, 219, 161, 102, 194, 17, 133, 199, 151,
        ],
    ));
    data.push((
        vec![3; 4097],
        [
            5, 198, 237, 1, 13, 121, 253, 174, 34, 84, 220, 9, 209, 99, 69, 36, 55, 122, 246, 211,
            167, 156, 130, 93, 254, 123, 209, 30, 113, 116, 65, 154,
        ],
    ));

    let mut ascending1 = vec![];
    for i in 0..800 {
        ascending1.push((i % 256) as u8);
    }
    let mut ascending2 = vec![];
    for i in 0..1600 {
        ascending2.push((i % 256) as u8);
    }

    data.push((
        ascending1,
        [
            101, 11, 95, 159, 241, 252, 13, 24, 255, 203, 183, 205, 52, 201, 107, 227, 70, 158, 4,
            58, 204, 125, 171, 38, 253, 185, 22, 84, 204, 158, 198, 148,
        ],
    ));
    data.push((
        ascending2,
        [
            251, 14, 43, 111, 228, 23, 133, 127, 165, 194, 250, 25, 105, 25, 141, 7, 69, 119, 185,
            38, 166, 218, 145, 121, 147, 67, 199, 78, 192, 85, 128, 249,
        ],
    ));

    for (bytes, expected) in data {
        let mut digest_batch = [0; WIDTH].into();
        crate::batch_hash(&bytes[..], &mut digest_batch);

        assert_eq!(
            digest_batch.as_bytes().as_slice(),
            expected.as_slice(),
            "{:?} bytes: {:?}",
            bytes.len(),
            bytes
        );
    }
}
