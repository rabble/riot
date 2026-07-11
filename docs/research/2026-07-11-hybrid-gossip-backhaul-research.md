# Hybrid Local Gossip + Opportunistic Internet Backhaul: Design Precedent

Date: 2026-07-11
Method: Deep-research workflow — 6 search angles, 24 sources fetched, 90 claims extracted, top 25 adversarially verified (3 independent verification votes per claim), synthesized to 11 findings. 21 of 25 votes confirmed, 4 refuted.

## Purpose

Riot should not assume a binary online/offline world. Most devices most of the time will be local-only, syncing over peer-to-peer gossip (Bluetooth/Wi-Fi Direct/local mesh). But some devices at some times will have partial or full internet access, and those devices should be able to opportunistically bridge — backhauling locally-gossiped content out to a server/wider network when they get a connection, and pulling updates back down into the local mesh. This is a hybrid gossip-plus-opportunistic-backhaul model, not pure offline mesh and not pure client-server.

This pass researched real-world precedent and technical design for exactly that pattern: systems where the *same* protocol and data model support both direct local peer sync and relay-through-an-online-node, with concrete detail on how they reconcile data across both paths and how bridge/gateway nodes get selected or trusted.

## Summary

Real-world hybrid gossip+backhaul systems converge on one design pattern: keep the merge/reconciliation logic transport-agnostic — an append-only log or causal DAG diffed by sequence number or dependency pointers, never by transport identity — and make bridge/relay/gateway-node selection opt-in and unauthenticated rather than elected or trust-gated. Briar (Bramble Transport/Sync Protocol) and Secure Scuttlebutt (Epidemic Broadcast Trees + pub servers) both prove this works in shipped, security-audited software: the same protocol runs over local transports (Bluetooth/Wi-Fi/memory cards, LAN UDP) and over an online relay (Tor; a pub server), and consistency recovers afterward purely from log/DAG structure — there is no special "reconciliation across paths" code, because the merge function never knows which path data arrived on. Meshtastic shows the same opt-in-relay pattern at the mesh-radio layer (any internet-connected node can become an MQTT gateway just by configuring credentials, no election) — but critically, Meshtastic's MQTT bridging was *not* confirmed to preserve end-to-end encryption across the bridge, an important negative finding: gateway nodes/brokers can plausibly see plaintext. Academic DTN/PRoPHET gateway-selection research remains simulation-only in the sources reviewed, with no evidence of production deployment. Reticulum's medium-agnostic auto-gateway claim did not survive verification and should be treated as unconfirmed.

## Verified Findings

### DTN/opportunistic-networking academic baseline

- **Gateway-selection literature (epidemic routing, PRoPHET) is simulation-only, not deployed.** The representative improved-PRoPHET paper (Han & Chung 2015) evaluates purely on the Helsinki ONE simulator, with no testbed or field deployment even in its future-work section. [high confidence] Relay/forwarding selection in this line of work is a two-phase heuristic: epidemic-style flooding while forwarding-counter and hop-counter stay below thresholds, then switching to a numeric "delivery predictability" score derived from historical contact frequency — i.e. a statistical score, not identity-based authorization.

### Briar: Tor backhaul + local mesh, one sync protocol

- **Briar's transport and sync layers are explicitly separated and transport-agnostic.** The Bramble Transport Protocol (BTP) wraps any channel that can deliver a best-effort byte stream — Bluetooth, Wi-Fi/LAN, Tor, removable media — and the Bramble Sync Protocol (BSP) is written once above that abstraction with no transport-specific merge code. Briar's own docs confirm the app syncs via Bluetooth/Wi-Fi/memory cards offline and via Tor online, inside the same app and protocol. [high confidence] (briarproject.org, BSP.md, BTP.md)
- **Reconciliation across transports happens through causal consistency, not transport-aware logic.** Every message carries explicit dependency pointers forming a DAG; a message is delivered to the client only once all its dependencies have been delivered, guaranteeing every device processes messages in the same order regardless of which transport or peer relayed them. [high confidence] (BSP.md, quoted directly)
- *Refuted, do not rely on:* a claim that Briar tracks per-transport max-latency to enable seamless mid-sync transport handoff [0-3]. The safer, confirmed claim is only the DAG/causal-consistency delivery mechanism above — not an explicit latency-tracked handoff feature.

### Secure Scuttlebutt: pub servers as ordinary, untrusted peers

- **Pubs are always-online rendezvous peers running the identical client protocol as any device.** Joining a pub makes members mutually visible/replicable within 2 hops, letting NAT'd/offline-heavy peers reach a wider network with no special relay protocol. Local peers separately discover each other via constant UDP broadcast on the LAN — a distinct discovery channel — but once connected, all peers, local or pub-relayed, use the identical secret-handshake and replication protocol. There is no separate data model for local vs. relayed sync. [high confidence] (Scuttlebutt protocol guide)
- **Replication is uniform and path-agnostic.** Peers exchange vector-clock "notes" (feed-id → sequence number + receive flag) via the Epidemic Broadcast Tree (EBT) protocol, now preferred over the legacy request/response method (kept as fallback). Because each feed is a strictly-ordered, append-only, signed log, reconciliation reduces to "exchange missing sequence numbers since last known index," not CRDT-style content merging — and this is identical whether the last sync happened directly or via a pub relay. An independent academic formalization (Kermarrec, Lavoie & Tschudin) confirms stores merge purely by diffing per-log frontiers over an abstracted "reliable channel" explicitly stated to cover both internet and local/USB connections, with no relay-specific merge variant — multi-hop propagation through an intermediary is just repeated pairwise application of the same update algorithm. [high confidence]
- **Bridge/relay trust is governed entirely by the social follow/block graph, not a separate authorization mechanism**, and no cryptographic trust in the relaying peer is required for data integrity because feeds are self-verifying (signed, hash-chained, append-only). A pub or any intermediate peer can carry another peer's log or files on their behalf without being authorized first. Pubs are an optional convenience layer, not a structural requirement — the core design is "a global gossip-protocol mesh without any host dependencies." [high confidence] (EBT README, ssb-server README)
- *Refuted, do not rely on:* a specific real-world anecdote of a user reconnecting after months offline and catching up via a single terse resync of ~4000 messages [0-3]. The underlying frontier-diffing mechanism would support this in principle, but the anecdote itself is unconfirmed field evidence.

### Meshtastic: opt-in gateway, unconfirmed confidentiality

- **Mesh-to-MQTT bridging is fully opt-in and unauthenticated.** Any node with internet connectivity (Wi-Fi/Ethernet, or via a paired phone) can become a gateway simply by configuring MQTT broker credentials and enabling per-channel uplink/downlink flags. There is no election, authorization, or trust mechanism for who becomes the gateway; the docs explicitly acknowledge multiple uncoordinated gateways can coexist and produce duplicate messages, and the widely-used default public broker uses shared, published, unchanged credentials — making the one nominal credential gate effectively open. [high confidence] (meshtastic.org docs)
- *Refuted, treat as an open risk, not a confirmed protection:* a claim that MQTT bridging preserves end-to-end channel encryption across the bridge so the broker/gateway node cannot read plaintext [0-3]. Riot should not assume an opportunistic-bridge design modeled on Meshtastic automatically preserves confidentiality without explicit end-to-end encryption independent of the bridge.

### Reticulum: unconfirmed

- *Refuted, do not rely on:* a claim that Reticulum is medium-agnostic with automatic gateway formation at multi-interface nodes, bridging LoRa/packet-radio/TCP/I2P without core-protocol changes [0-3]. Reticulum's actual gateway-authorization/trust mechanism is an open research gap, not a confirmed precedent.

## Cross-Cutting Patterns

1. **Separate "how bytes move" from "how state merges."** Both strongest precedents (Briar, SSB) split transport (Bluetooth/Wi-Fi/memory-card/Tor for Briar; UDP-LAN/pub-TCP for SSB) from merge logic (causal DAG delivery for Briar; append-only log + frontier diff for SSB), and neither special-cases the backhaul/relay path in its merge logic — the same reconciliation function runs whether data arrived locally or via relay.
2. **Relay participation is cheap to trust, because trust isn't required for integrity.** Briar's Tor relay is just another best-effort transport under the same encrypted sync layer; SSB's pubs and any-peer relays don't need to be trusted because feeds are self-verifying (signed + hash-chained). A dishonest relay can at most withhold or delay data, not forge it.
3. **Gateway/bridge selection is opt-in and unauthenticated at the network layer across all three deployed systems** (Briar, SSB, Meshtastic) — nobody elects a gateway; any device with connectivity can offer to relay. Academic DTN gateway-election heuristics exist only in simulation and are not a proven production alternative.
4. **"Anyone can bridge" and "the bridge can't read content" are separate properties that must each be engineered — they don't come for free together.** Briar and SSB get both (transport-agnostic encryption/self-verifying logs). Meshtastic gets only the first; its confidentiality-across-the-bridge property is unconfirmed and plausibly absent.
5. **Multiple uncoordinated bridges are expected, not exceptional.** Meshtastic explicitly tolerates and documents duplicate-message production from multiple simultaneous gateways rather than preventing it.

## Design Implications for Riot

- **Give the local sync path (future Willow Transfer Protocol) and the public web gateway/stateless mirror one shared reconciliation primitive** — ideally a Willow-native causal/log-diffing structure analogous to Briar's dependency-DAG delivery or SSB's per-log sequence/frontier diffing — so the gateway is just another opportunistic peer/transport, not a special-cased sync path requiring separate merge code. This is a direct, positive precedent for the "public web gateway (stateless mirror)" already planned in `docs/product/product-brief.md` and the dual-mode design.
- **Make bridge/gateway-node selection opt-in and unauthenticated at the network layer**, following the Briar/SSB/Meshtastic precedent: any device with connectivity can offer to relay. Do not attempt an elected/authorized gateway role — the only production alternative in this research (DTN/PRoPHET scoring) is simulation-only.
- **Do not let confidentiality depend on trusting the bridge.** Follow SSB's model (self-verifying signed/hashed content, so a relay can withhold but not forge) or Briar's model (end-to-end encrypted transport-agnostic sync) rather than Meshtastic's model, where bridge confidentiality is unconfirmed and plausibly absent. Anything crossing a Riot gateway/relay should be signed, and where sensitive, end-to-end encrypted independent of which node relays it — this reinforces the existing private-drop-envelope design in the dual-mode research addendum, which already assumes carriers see no plaintext.
- **Design for idempotent merge/dedup, not a single canonical gateway.** Expect multiple uncoordinated bridge nodes to coexist, as Meshtastic explicitly tolerates; Riot's Willow join/merge semantics (newer-write replacement, prefix pruning) already fit this, but explicit dedup at the gateway-ingest boundary should be treated as a requirement, not an edge case.
- **A follow/block-graph-style scoping mechanism (SSB's EBT gating) is a plausible way to bound what an untrusted bridge is expected or allowed to relay**, separate from the cryptographic trust question — worth considering alongside Riot's existing Meadowcap path-scoped capabilities for who can write where.

## Coverage Holes and Open Questions

- No verified findings were obtained for deployed community/disaster mesh networks with documented internet-gateway nodes (NYC Mesh, Freifunk, Serval Project, Commotion Wireless, RightMesh) — what do these actually document about gateway-node trust, selection, and failure modes when the sole uplink node goes offline? Genuine gap; worth a dedicated follow-up pass.
- Reticulum's actual mechanism for gateway/bridge authorization across heterogeneous interfaces (LoRa, TCP/IP, I2P) remains unconfirmed after the prior claim was refuted — does it have any documented trust model for multi-interface gateway nodes, or is it fully open like Meshtastic?
- Does Meshtastic (or a comparable LoRa mesh design) have any deployed pattern that restores end-to-end confidentiality across an MQTT bridge (e.g., channel-key encryption preserved through the broker), or is plaintext-at-the-broker simply an accepted risk in current deployments?
- Is there any real-world, non-simulated deployment of DTN/opportunistic-networking gateway-selection heuristics (PRoPHET-style delivery-predictability scoring) anywhere — humanitarian, military, or research-network context — that would give Riot more than a simulation-only precedent to draw on?

### Sourcing caveats

Confidence is high only where multiple primary sources or unanimous 3-0 votes exist (Briar's BSP/BTP specs, SSB's protocol guide + EBT repo + academic paper, Meshtastic's own docs). The DTN/PRoPHET simulation-only finding rests on one representative paper and should be read as indicative of a broader pattern in that literature, not an exhaustive survey. Four claims were explicitly checked and refuted in this pass (Briar per-transport latency handoff, SSB's specific long-gap-reconnection anecdote, Meshtastic's encryption-preservation-across-bridge, Reticulum's medium-agnostic auto-gateway) — they're recorded for transparency but must not be treated as established.

## Primary Sources

- Briar Sync Protocol (BSP) spec — https://code.briarproject.org/briar/briar-spec/blob/master/protocols/BSP.md
- Briar Transport Protocol (BTP) spec — https://code.briarproject.org/briar/briar-spec/blob/master/protocols/BTP.md
- Briar, "How it works" — https://briarproject.org/how-it-works/
- Briar independent security audit (Open Technology Fund) — https://www.opentech.fund/security-safety-audits/briar-security-audit/
- Scuttlebutt Protocol Guide — https://ssbc.github.io/scuttlebutt-protocol-guide/
- Epidemic Broadcast Trees (EBT) protocol explainer — http://dev.planetary.social/replication/ebt.html
- `epidemic-broadcast-trees` reference implementation README — https://github.com/ssbc/epidemic-broadcast-trees/blob/master/README.md
- `ssb-server` README — https://github.com/ssbc/ssb-server
- Kermarrec, Lavoie & Tschudin, "Gossiping with Append-Only Logs in Secure-Scuttlebutt" — https://www.researchgate.net/publication/348239763_Gossiping_with_Append-Only_Logs_in_Secure-Scuttlebutt
- Meshtastic MQTT integration docs — https://meshtastic.org/docs/software/integrations/mqtt/
- Meshtastic MQTT module configuration docs — https://meshtastic.org/docs/configuration/module/mqtt/
- Reticulum network manual — https://reticulum.network/manual/networks.html
- Han & Chung, "An Improved PRoPHET Routing Protocol in Delay Tolerant Network" (2015) — https://onlinelibrary.wiley.com/doi/10.1155/2015/623090
- Wikipedia, "Routing in delay-tolerant networking" (background/secondary) — https://en.wikipedia.org/wiki/Routing_in_delay-tolerant_networking
