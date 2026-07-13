//! Closed, canonical payloads for community Newswire spaces.

mod model;

pub use model::{
    decode_editorial_action, decode_news_post, decode_space_descriptor, encode_editorial_action,
    encode_news_post, encode_space_descriptor, AlertProfileV1, EditorialActionKind,
    EditorialActionV1, NewsPostV1, NewswireModelError, OperationalProfileV1, RequestKind,
    RequestProfileV1, SpaceDescriptorV1, ACTION_SCHEMA, MAX_NEWSWIRE_PAYLOAD_BYTES, POST_SCHEMA,
    SPACE_SCHEMA,
};
