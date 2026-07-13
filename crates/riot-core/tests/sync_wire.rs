use minicbor::Encoder;
use riot_core::import::MAX_BUNDLE_BYTES;
use riot_core::sync::{
    decode_frame, encode_frame, SyncError, SyncFrame, MAX_SYNC_FRAME_BYTES, MAX_SYNC_IDS,
};

const NAMESPACE: [u8; 32] = [0x42; 32];
const CODEC: &str = "org.riot.conference-sync/1";

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn raw_frame(kind: u8, body: impl FnOnce(&mut Encoder<&mut Vec<u8>>)) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes);
    encoder.map(3).unwrap();
    encoder.u8(0).unwrap().str(CODEC).unwrap();
    encoder.u8(1).unwrap().u8(kind).unwrap();
    encoder.u8(2).unwrap();
    body(&mut encoder);
    bytes
}

fn hello_bytes() -> Vec<u8> {
    encode_frame(&SyncFrame::Hello {
        namespace_id: NAMESPACE,
    })
    .unwrap()
}

#[test]
fn every_wire_frame_round_trips_at_numeric_boundaries() {
    let exact_ids: Vec<_> = (0..MAX_SYNC_IDS).map(|value| id(value as u8)).collect();
    let frames = [
        SyncFrame::Hello {
            namespace_id: NAMESPACE,
        },
        SyncFrame::Summary {
            namespace_id: NAMESPACE,
            entry_ids: vec![],
        },
        SyncFrame::Summary {
            namespace_id: NAMESPACE,
            entry_ids: exact_ids.clone(),
        },
        SyncFrame::Request {
            namespace_id: NAMESPACE,
            entry_ids: exact_ids,
        },
        SyncFrame::Entries {
            namespace_id: NAMESPACE,
            bundle_bytes: vec![0; MAX_BUNDLE_BYTES],
        },
        SyncFrame::Complete {
            namespace_id: NAMESPACE,
        },
        SyncFrame::Reject {
            namespace_id: NAMESPACE,
            code: u8::MIN,
        },
        SyncFrame::Reject {
            namespace_id: NAMESPACE,
            code: u8::MAX,
        },
    ];

    for frame in frames {
        let encoded = encode_frame(&frame).expect("boundary frame encodes");
        assert_eq!(decode_frame(&encoded), Ok(frame));
    }
}

#[test]
fn encoder_rejects_duplicate_unsorted_over_limit_ids_and_oversized_bundles() {
    for (entry_ids, expected) in [
        (vec![id(1), id(1)], SyncError::DuplicateEntryId),
        (vec![id(2), id(1)], SyncError::EntryIdsNotSorted),
        (vec![id(7); MAX_SYNC_IDS + 1], SyncError::TooManyEntryIds),
    ] {
        assert_eq!(
            encode_frame(&SyncFrame::Request {
                namespace_id: NAMESPACE,
                entry_ids,
            }),
            Err(expected)
        );
    }

    assert_eq!(
        encode_frame(&SyncFrame::Entries {
            namespace_id: NAMESPACE,
            bundle_bytes: vec![0; MAX_BUNDLE_BYTES + 1],
        }),
        Err(SyncError::BundleTooLarge)
    );
    assert_eq!(
        decode_frame(&vec![0; MAX_SYNC_FRAME_BYTES + 1]),
        Err(SyncError::FrameTooLarge)
    );
}

#[test]
fn decoder_rejects_each_header_error_and_unknown_kind() {
    let mut wrong_map_length = hello_bytes();
    wrong_map_length[0] = 0xa2;
    assert_eq!(
        decode_frame(&wrong_map_length),
        Err(SyncError::MalformedFrame)
    );

    let mut wrong_first_key = hello_bytes();
    wrong_first_key[1] = 1;
    assert_eq!(
        decode_frame(&wrong_first_key),
        Err(SyncError::MalformedFrame)
    );

    let wrong_codec_type = raw_frame(0, |encoder| {
        encoder.bytes(&NAMESPACE).unwrap();
    });
    let mut wrong_codec_type = wrong_codec_type;
    wrong_codec_type[2] = 0x40;
    assert_eq!(
        decode_frame(&wrong_codec_type),
        Err(SyncError::MalformedFrame)
    );

    let mut unsupported = hello_bytes();
    let codec_start = unsupported
        .windows(CODEC.len())
        .position(|window| window == CODEC.as_bytes())
        .unwrap();
    unsupported[codec_start] = b'x';
    assert_eq!(decode_frame(&unsupported), Err(SyncError::UnsupportedCodec));

    let after_codec = codec_start + CODEC.len();
    let mut wrong_kind_key = hello_bytes();
    wrong_kind_key[after_codec] = 2;
    assert_eq!(
        decode_frame(&wrong_kind_key),
        Err(SyncError::MalformedFrame)
    );

    let mut wrong_kind_type = hello_bytes();
    wrong_kind_type[after_codec + 1] = 0x60;
    assert_eq!(
        decode_frame(&wrong_kind_type),
        Err(SyncError::MalformedFrame)
    );

    let mut wrong_body_key = hello_bytes();
    wrong_body_key[after_codec + 2] = 3;
    assert_eq!(
        decode_frame(&wrong_body_key),
        Err(SyncError::MalformedFrame)
    );

    let mut unknown_kind = hello_bytes();
    unknown_kind[after_codec + 1] = 6;
    assert_eq!(decode_frame(&unknown_kind), Err(SyncError::MalformedFrame));
}

#[test]
fn decoder_rejects_wrong_body_shapes_lengths_and_types() {
    let cases = [
        raw_frame(0, |encoder| {
            encoder.u8(0).unwrap();
        }),
        raw_frame(0, |encoder| {
            encoder.bytes(&NAMESPACE[..31]).unwrap();
        }),
        raw_frame(1, |encoder| {
            encoder.array(1).unwrap().bytes(&NAMESPACE).unwrap();
        }),
        raw_frame(1, |encoder| {
            encoder.array(2).unwrap().u8(0).unwrap().array(0).unwrap();
        }),
        raw_frame(1, |encoder| {
            encoder
                .array(2)
                .unwrap()
                .bytes(&NAMESPACE)
                .unwrap()
                .begin_array()
                .unwrap()
                .end()
                .unwrap();
        }),
        raw_frame(1, |encoder| {
            encoder
                .array(2)
                .unwrap()
                .bytes(&NAMESPACE)
                .unwrap()
                .array(1)
                .unwrap()
                .bytes(&NAMESPACE[..31])
                .unwrap();
        }),
        raw_frame(3, |encoder| {
            encoder.array(1).unwrap().bytes(&NAMESPACE).unwrap();
        }),
        raw_frame(3, |encoder| {
            encoder
                .array(2)
                .unwrap()
                .bytes(&NAMESPACE[..31])
                .unwrap()
                .bytes(&[])
                .unwrap();
        }),
        raw_frame(3, |encoder| {
            encoder
                .array(2)
                .unwrap()
                .bytes(&NAMESPACE)
                .unwrap()
                .u8(0)
                .unwrap();
        }),
        raw_frame(5, |encoder| {
            encoder.array(1).unwrap().bytes(&NAMESPACE).unwrap();
        }),
        raw_frame(4, |encoder| {
            encoder.bytes(&NAMESPACE[..31]).unwrap();
        }),
        raw_frame(5, |encoder| {
            encoder
                .array(2)
                .unwrap()
                .bytes(&NAMESPACE[..31])
                .unwrap()
                .u8(0)
                .unwrap();
        }),
        raw_frame(5, |encoder| {
            encoder
                .array(2)
                .unwrap()
                .bytes(&NAMESPACE)
                .unwrap()
                .str("not-a-code")
                .unwrap();
        }),
    ];

    for bytes in cases {
        assert_eq!(decode_frame(&bytes), Err(SyncError::MalformedFrame));
    }

    let over_count = raw_frame(2, |encoder| {
        encoder
            .array(2)
            .unwrap()
            .bytes(&NAMESPACE)
            .unwrap()
            .array((MAX_SYNC_IDS + 1) as u64)
            .unwrap();
    });
    assert_eq!(decode_frame(&over_count), Err(SyncError::TooManyEntryIds));

    let wrong_count_type = raw_frame(2, |encoder| {
        encoder
            .array(2)
            .unwrap()
            .bytes(&NAMESPACE)
            .unwrap()
            .u8(0)
            .unwrap();
    });
    assert_eq!(
        decode_frame(&wrong_count_type),
        Err(SyncError::MalformedFrame)
    );

    let duplicate_summary = raw_frame(1, |encoder| {
        encoder
            .array(2)
            .unwrap()
            .bytes(&NAMESPACE)
            .unwrap()
            .array(2)
            .unwrap()
            .bytes(&id(1))
            .unwrap()
            .bytes(&id(1))
            .unwrap();
    });
    assert_eq!(
        decode_frame(&duplicate_summary),
        Err(SyncError::DuplicateEntryId)
    );
}

#[test]
fn decoder_distinguishes_trailing_from_reencoded_noncanonical_frames() {
    let canonical = hello_bytes();
    let mut trailing = canonical.clone();
    trailing.push(0);
    assert_eq!(decode_frame(&trailing), Err(SyncError::NonCanonicalFrame));

    let mut widened_key = Vec::with_capacity(canonical.len() + 1);
    widened_key.push(canonical[0]);
    widened_key.extend_from_slice(&[0x18, 0x00]);
    widened_key.extend_from_slice(&canonical[2..]);
    assert_eq!(
        decode_frame(&widened_key),
        Err(SyncError::NonCanonicalFrame)
    );
}
