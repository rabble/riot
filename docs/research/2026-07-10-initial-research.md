# Initial Research: Willow, SneakerWeb, Bitchat, and Offline Packets

Date: 2026-07-10

## Summary

Riot should be a new app, not a thin feature inside bitchat. Bitchat is useful as a possible local transport, but the product need is broader: people need durable, inspectable, updateable information when the internet is degraded or shut down. Chat handles immediate coordination; Riot should handle living local knowledge.

The strongest technical foundation is Willow plus SneakerWeb-like rendering:

- Willow gives signed, mergeable, path-addressed data.
- Meadowcap gives decentralized write/read authority and delegation.
- Willow Drop Format gives a bytestring package for offline import/export.
- Willow Transfer Protocol can later provide request/response live sync.
- SneakerWeb shows how Willow entries can become offline websites, one subspace per site/domain.

## Source Notes

### SneakerWeb

SneakerWeb describes itself as a parallel web transported by physical media. Its homepage says websites are stored on user devices and transferred as `.snk` files, with local offline browsing from a collection.

The protocol spec is directly relevant:

- It is a peer-to-peer network for sharing static websites built on Willow'25.
- Each sneakerwebsite is stored as Willow entries in a fixed communal namespace.
- Each website uses one subspace, making a one-to-one mapping between subspaces and domains.
- Willow paths map to local URL paths.
- URLs ending in `/` resolve to `index.html`.
- Backends are encouraged to show preview information from `sneakerweb.html`.
- The suggested exchange format is Willow Drop Format, with `.snk` as the recommended extension.

Sources:

- https://sneakerweb.org/
- https://sneakerweb.org/spec

### Willow

Willow's data model is path-addressed bytes with timestamps, subspaces, and namespaces. Stores merge through a join operation, including newer-write replacement and prefix pruning. This is a better model for evolving field information than append-only chat.

Important properties for Riot:

- `namespace_id` can scope a packet, incident, community, or app space.
- `subspace_id` can identify a signer, site, organization, or device.
- `path` can identify rendered pages or structured data objects.
- `timestamp` and payload digest semantics provide deterministic merge behavior.
- Willow is generic, but Willow'25 provides recommended parameters.

Sources:

- https://willowprotocol.org/specs/data-model/
- https://willowprotocol.org/specs/willow25/

### Meadowcap

Meadowcap is Willow's capability system. It supports owned and communal namespaces, delegation, and restrictions by subspace, path, and timestamp.

Potential Riot mappings:

- An organizer can delegate write capability for `/alerts/`.
- A medic team can write `/medical/`.
- Translators can write `/i18n/<lang>/`.
- Unknown users can write under `/reports/unverified/<subspace>/`.
- Trusted signers can supersede or prune stale or unsafe information.

Source:

- https://willowprotocol.org/specs/meadowcap/

### Drop Format

Willow Drop Format packages arbitrary sets of entries and payloads into one bytestring. The spec explicitly frames drops as suitable for user-improvised channels such as USB keys, SD cards, email attachments, instant messages, torrents, or any other means that can transfer a bytestring. The spec recommends encrypting drops for transport.

This should be Riot's first exchange format because it is simple, robust, and works without live peer negotiation.

Source:

- https://willowprotocol.org/specs/drop-format/

### Willow Transfer Protocol and Confidential Sync

There are two "live sync" tracks:

- Willow Transfer Protocol is a simpler request/response protocol with setup, challenge signing, and request/response operations. It is the pragmatic first live sync candidate.
- Willow Confidential Sync is more powerful: private interest overlap, partial sync, range reconciliation, payload transfer, resource control, and LCMUX. It is likely too much for the first implementation.

For Riot, WTP should come before Confidential Sync.

Sources:

- https://willowprotocol.org/specs/wtp/
- https://willowprotocol.org/specs/confidential-sync/

### Bitchat

Bitchat is useful research because it shows an existing BLE mesh with packet fragmentation, file transfer, gossip sync, and store-and-forward behavior. But it is chat-first and packet-centric.

Important observations from local source inspection:

- Existing file transfer is capped at 1 MiB payload.
- BLE fragmentation uses roughly 469 byte chunks.
- Sync is packet-type specific and short-window for fragments/files.
- It has a Nostr bridge and packet carrier concepts that could later inspire Riot transports.

Conclusion: bitchat can be a future transport adapter or companion, but Riot should own its data model and product flow.

Local source references from `/Users/rabble/code/explorations/bitchat`:

- `localPackages/BitFoundation/Sources/BitFoundation/MessageType.swift`
- `bitchat/Protocols/BitchatFilePacket.swift`
- `bitchat/Services/TransportConfig.swift`
- `bitchat/Sync/GossipSyncManager.swift`

## Product Conclusion

The install boundary on iOS means Riot must be useful before a shutdown and able to create new content during a shutdown. Preloading static packs is not enough. The app must include authoring, signing, import/export, and local rendering from day one.

The local LLM should help users draft, translate, summarize, and format updates from user-provided facts. It must not be treated as a source of truth.
