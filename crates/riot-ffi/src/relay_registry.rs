//! Durable relay records used by the non-local pull surface.
//!
//! A relay is transport state, not community state: it is a stable NodeId and
//! the ticket bytes that admit the profile to the communities served there.
//! The registry is deliberately opaque to riot-core and lives beside the FFI
//! profile's other durable local state.

use minicbor::data::Type;
use minicbor::{Decoder, Encoder};

use crate::mobile_api::RelayRecord;

pub(crate) const RELAY_REGISTRY_KEY: &str = "relay_registry/v1";
pub(crate) const RELAY_REGISTRY_QUARANTINE_KEY: &str = "relay_registry_quarantine/v1";

const REGISTRY_VERSION: u8 = 1;
const MAX_RELAYS: u64 = 256;
const RECORD_FIELDS: u64 = 3;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RelayRegistry {
    /// Newest remembered relay first. This makes an explicitly added relay the
    /// next pull target without introducing a separate mutable preference.
    pub(crate) relays: Vec<RelayRecord>,
}

#[derive(Debug)]
pub(crate) struct RelayRegistryCorrupt;

impl RelayRegistry {
    pub(crate) fn upsert(&mut self, relay: RelayRecord) {
        self.relays
            .retain(|existing| existing.node_id != relay.node_id);
        self.relays.insert(0, relay);
    }

    pub(crate) fn next(&self) -> Option<RelayRecord> {
        self.relays.first().cloned()
    }

    #[cfg(feature = "net")]
    pub(crate) fn mark_answered(&mut self, node_id: &str, answered_at: u64) -> bool {
        let Some(relay) = self
            .relays
            .iter_mut()
            .find(|relay| relay.node_id == node_id)
        else {
            return false;
        };
        relay.last_answered_unix_seconds = Some(answered_at);
        true
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut encoder = Encoder::new(&mut buffer);
        encoder.array(2).expect("vec encoder infallible");
        encoder
            .u8(REGISTRY_VERSION)
            .expect("vec encoder infallible");
        encoder
            .array(self.relays.len() as u64)
            .expect("vec encoder infallible");
        for relay in &self.relays {
            encoder
                .array(RECORD_FIELDS)
                .expect("vec encoder infallible");
            encoder.str(&relay.node_id).expect("vec encoder infallible");
            encoder
                .bytes(&relay.ticket_bytes)
                .expect("vec encoder infallible");
            match relay.last_answered_unix_seconds {
                Some(value) => encoder.u64(value).expect("vec encoder infallible"),
                None => encoder.null().expect("vec encoder infallible"),
            };
        }
        buffer
    }

    pub(crate) fn decode(input: &[u8]) -> Result<Self, RelayRegistryCorrupt> {
        let mut decoder = Decoder::new(input);
        if decoder.array().map_err(|_| RelayRegistryCorrupt)? != Some(2) {
            return Err(RelayRegistryCorrupt);
        }
        if decoder.u8().map_err(|_| RelayRegistryCorrupt)? != REGISTRY_VERSION {
            return Err(RelayRegistryCorrupt);
        }
        let count = decoder
            .array()
            .map_err(|_| RelayRegistryCorrupt)?
            .ok_or(RelayRegistryCorrupt)?;
        if count > MAX_RELAYS {
            return Err(RelayRegistryCorrupt);
        }
        let mut relays = Vec::with_capacity(count as usize);
        for _ in 0..count {
            if decoder.array().map_err(|_| RelayRegistryCorrupt)? != Some(RECORD_FIELDS) {
                return Err(RelayRegistryCorrupt);
            }
            let node_id = decoder.str().map_err(|_| RelayRegistryCorrupt)?.to_string();
            let ticket_bytes = decoder.bytes().map_err(|_| RelayRegistryCorrupt)?.to_vec();
            let last_answered_unix_seconds =
                if decoder.datatype().map_err(|_| RelayRegistryCorrupt)? == Type::Null {
                    decoder.null().map_err(|_| RelayRegistryCorrupt)?;
                    None
                } else {
                    Some(decoder.u64().map_err(|_| RelayRegistryCorrupt)?)
                };
            relays.push(RelayRecord {
                node_id,
                ticket_bytes,
                last_answered_unix_seconds,
            });
        }
        if decoder.position() != input.len() {
            return Err(RelayRegistryCorrupt);
        }
        Ok(Self { relays })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relay(node_id: &str) -> RelayRecord {
        RelayRecord {
            node_id: node_id.to_string(),
            ticket_bytes: vec![1, 2, 3],
            last_answered_unix_seconds: Some(42),
        }
    }

    #[test]
    fn newest_relay_is_next_and_round_trips() {
        let mut registry = RelayRegistry::default();
        registry.upsert(relay("11"));
        registry.upsert(relay("22"));

        assert_eq!(registry.next(), Some(relay("22")));
        assert_eq!(RelayRegistry::decode(&registry.encode()).unwrap(), registry);
    }
}
