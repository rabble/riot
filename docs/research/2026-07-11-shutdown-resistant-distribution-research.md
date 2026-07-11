# Distribution With No Live Connectivity: Sneakernets, Mesh Failures, and DTN Theory

Date: 2026-07-11
Method: Deep-research workflow — 6 search angles, 22 sources fetched, 105 claims extracted, top 25 adversarially verified (3 independent verification votes per claim), synthesized to 11 findings. 24 of 25 votes confirmed, 1 refuted.

## Purpose

`docs/research/2026-07-11-disaster-riot-mutual-aid-evidence-research.md` found that Reporters Without Borders' "Collateral Freedom" achieves censorship resistance by mirroring blocked outlets onto commercial CDNs — but that pattern assumes the reader still has *some* working internet connection to reach the mirror through. Riot's actual target scenario (internet shutdowns, offline-first sync) has no CDN to hide inside. This pass researched real-world evidence for how censorship- or shutdown-resistant *distribution* has actually worked when there is little or no live connectivity to lean on: sneakernets, mesh networking under real shutdowns, one-way radio broadcast, delay-tolerant networking theory, and digital-rights field guidance.

## Summary

The strongest evidence in this pass is physical-media sneakernets moved by ordinary transportation: Cuba's El Paquete Semanal and DakNet's vehicle-mounted data mules are real, sustained, deployed systems with documented mechanisms and multi-year scale. By contrast, app-based Bluetooth/BLE mesh messaging marketed explicitly for protest use (Bridgefy) has a documented double failure — independently audited protocol-level security breaks (no authentication, no effective confidentiality, whole-network DoS from a single crafted message, trackable social graphs) *and*, separately, a practical field failure: Hong Kong 2019 protesters who tried it reported "it just doesn't work" and fell back to paid SMS, not mesh, when crowd-congestion (not a government shutdown) degraded connectivity. Briar is a more rigorously engineered alternative — audited clean at the protocol/crypto level, explicitly designed to fail over from Tor to Bluetooth/Wi-Fi/memory-card sync during blackouts — but its own offline sneakernet-style app-distribution feature was found vulnerable to a physical-proximity brute-force attack, showing offline distribution carries its own attack surface that has to be engineered for directly. Academic DTN theory (custody transfer) and a very recent simulation (Cache to the Future) both point toward demand-driven peer-request retrieval as more robust than the epidemic/flood routing used by essentially every deployed blackout-resistant mesh app — but this is simulation-stage evidence, not a field-proven deployment, whereas the sneakernet/data-mule pattern is the one category here with real, sustained, deployed evidence at meaningful scale.

## Verified Findings

### Sneakernet / physical-media distribution

- **El Paquete Semanal is a hierarchical physical sneakernet.** A high-level "matriz" assembles a roughly 1TB weekly package from curated content, which propagates through mid-level distributors to consumers across all Cuban provinces via hard drives moved by car, plane, train, and bus; the buyer supplies their own storage media, at roughly $2 for a full copy in Havana, operating since approximately 2008 as a de facto offline-internet substitute. [3-0] (Univ. of Havana/Clarkson ACM COMPASS'18 paper, corroborated by CHI'18, CBC, El Toque)
- **Toleration is conditional on distributor self-censorship** under an informal "no politics, no pornography" policy, identified by the paper's authors as a key reason the unofficial system has avoided government disruption. [2-1] A real historical tradeoff worth naming: censorship-resistance can be traded for distributor tolerance.
- **Dead Drops is a zero-infrastructure, zero-protocol distribution model**: USB flash drives literally cemented into walls, buildings, and curbs in public space; users transfer files by physically plugging a laptop into the fixed drive on-site, no server or network handshake involved — 1,400+ locations across dozens of countries since 2010. [3-0] (creator's own documentation, corroborated by Wikipedia and independent tech press)
- **DakNet used vehicle-mounted (bus/motorbike) mobile access points as physical store-and-forward data mules** between village kiosks and an internet hub — first deployed in a remote Cambodian province in September 2003 and in 20 villages in Orissa, India, at roughly two orders of magnitude lower cost than landline alternatives. [3-0] (Kevin Fall's foundational 2003 DTN paper, corroborated by IEEE Computer/MIT Media Lab)
- *Refuted, do not rely on:* that El Paquete has become "the primary source of entertainment for millions of Cubans" given ~38.8% internet penetration [1-2] — the mechanism and scale (1TB/week since ~2008) is well-documented, but total-population-reliance is not.

### Mesh networking under real shutdowns (adversarial findings)

- **Bridgefy's Bluetooth mesh protocol had no authentication, no effective confidentiality, incorrect cryptographic implementations, and protocol-level information leakage** — letting adversaries read messages, impersonate any user to any other user, build social graphs of protest participants, and shut down the entire mesh network with a single maliciously crafted message (a decompression "zip bomb" that hangs every forwarding node until reinstall). Despite this, Bridgefy saw real adoption during protests in Hong Kong, India, Iran, Lebanon, Zimbabwe, and the US. A 2022 follow-up study found the core flaws persisted even after Bridgefy adopted the Signal/libsignal protocol. [3-0] (CT-RSA 2021 peer-reviewed paper, Royal Holloway press release, 2022 USENIX Security follow-up)
- **Interviewed Hong Kong 2019 protesters said Bridgefy simply didn't work** ("it just doesn't work" — one participant), and none had successfully used it to communicate; when protesters actually lost connectivity (crowd-density network congestion, not a government shutdown), the working fallback was paid SMS over cellular, not mesh networking. [3-0] (peer-reviewed 11-participant interview study) Download-spike/marketing numbers do not indicate functional real-world delivery.
- **goTenna Mesh's proprietary "Aspen Grove" protocol imposes a hard maximum hop limit of 6** for relaying a message across the mesh, and reaching that ceiling may require a paid subscription — a hard architectural cap on zero-infrastructure propagation distance. [2-1] (goTenna's own spec sheet)

### Briar: audited design, one real weakness

- **Briar falls back from Tor (online) to Bluetooth/Wi-Fi/memory cards (offline) inside the same audited app.** Cure53 (2017) and Radically Open Security via the Open Technology Fund (2023) both audited it; the 2023 audit of the Android/desktop clients, protocols, and cryptography found zero high-risk issues (1 moderate, 5 low-risk). [3-0] Real limits exist: ~10-30m BLE range, higher battery drain, need for a critical mass of nearby contacts.
- **Briar's one moderate-severity finding was in its own offline app-distribution feature**: sharing the Briar APK itself over Wi-Fi using relatively short pre-shared keys (PSKs), letting a physically-proximate attacker brute-force the PSK and inject malware into the app package during first-time transfer. Retested and resolved by early 2024. [3-0] (OTF/Radically Open Security audit, issue OTF-002) This is the single most directly transferable finding for Riot: any offline/physical peer-to-peer install or content-distribution path is its own attack surface distinct from network security, and needs long/high-entropy shared secrets or out-of-band key verification — not just "it's offline so it's safe."

### Delay-tolerant networking theory

- **DTN architecture replaces end-to-end acknowledgment with hop-by-hop "custody transfer"**: each DTN node with persistent storage takes acknowledged responsibility for reliable delivery to the next hop, explicitly justified because in challenged networks a message's delivery time may exceed the sending node's own operational lifetime — relieving resource-poor end nodes of the need to hold data until final delivery. [2-1] (Kevin Fall, SIGCOMM 2003, foundational and widely cited; concept retained in RFC 4838/5050)
- **A June 2026 preprint ("Cache to the Future") argues epidemic/flood routing — used by every surveyed blackout-resistant mesh app including Briar and Bridgefy — is inferior to demand-driven retrieval**, where "leecher" devices request specific content from peers they encounter. In city-scale simulation (25,000-person mobility trace, 500m grid cells, Bluetooth-only propagation) over a simulated 2-month blackout, users could retrieve 75% of a 100,000-page archive with median latency under 24 hours for the 10,000 most popular pages, and the demand-driven approach degraded far less than epidemic routing under adversarial/Sybil conditions. [mixed 2-1/3-0] Single not-yet-peer-reviewed preprint, simulation only — not a field deployment — but the most directly relevant evidence for Riot's sync-protocol choice.

## Cross-Cutting Patterns

1. **Physical/vehicle-based sneakernets have real, sustained, multi-year deployed evidence at meaningful scale; app-layer BLE mesh does not.** El Paquete and DakNet are the strongest evidence in this entire pass; Bridgefy is the strongest counter-evidence (real deployment, real failure).
2. **Marketing/download metrics do not indicate functional delivery.** Bridgefy saw a real download spike during Hong Kong 2019, and still failed operationally — the working fallback was boring cellular SMS.
3. **Independent audit catches protocol-breaking flaws that field deployment alone did not surface.** Bridgefy shipped and was used in real protests for years before academic audits found its core flaws; audit-before-reliance, not adoption-as-validation, is the standard Riot should hold its own transports to.
4. **Offline/physical-proximity distribution is its own attack surface, not a safe-by-default fallback.** Briar's one real audit finding was in its sneakernet-style app-sharing feature, not its online protocol.
5. **Epidemic/flood routing is the incumbent design in every deployed system reviewed, but emerging (unproven) theory argues demand-driven retrieval is more robust**, especially against Sybil/spam disruption.
6. **Hard architectural propagation limits are real and sometimes commercially gated** (goTenna's 6-hop ceiling), meaning any design assuming indefinite multi-hop relay needs a bridging layer.

## Design Implications for Riot

- **Prioritize a hierarchical physical-media/data-mule sync path as a first-class distribution mechanism, not a fallback.** El Paquete and DakNet are the best-evidenced patterns in this research: a periodically-refreshed package plus a courier/transit-route/data-mule network. This fits directly with Riot's existing Willow Drop Format plan (`docs/research/2026-07-10-initial-research.md`) — drops are explicitly designed for USB/SD/courier transport already; this research reinforces investing there rather than treating device-to-device BLE mesh as the primary channel.
- **Do not build or depend on a custom BLE mesh transport without independent cryptographic audit before relying on it in a shutdown scenario.** Bridgefy's failure mode (blind pre-parse forwarding enabling a whole-network DoS from one malicious message) is a concrete thing to test for — any Riot mesh transport must assume forwarding peers are untrusted.
- **Treat any offline/physical-proximity distribution or pairing mechanism (sneakernet app updates, dead-drop caches, device-to-device invite/pairing) as its own attack surface**, per Briar's PSK finding — require long/high-entropy shared secrets or out-of-band verification (QR code, in-person code comparison), not "offline implies safe." This directly extends the invite-lifecycle design already specified in the dual-mode research addendum.
- **Prefer demand-driven (request-based) peer retrieval over epidemic/flood replication for Riot's local sync layer where feasible**, per the Cache to the Future evidence — while treating it as promising but simulation-stage, not a proven pattern to copy uncritically.
- **Budget for a hard propagation ceiling in any hop-based relay design** (goTenna's 6-hop limit as a concrete data point) rather than assuming indefinite multi-hop reach; plan a data-mule/store-and-forward layer to bridge beyond it, consistent with DTN's custody-transfer model — a relay node with persistent storage takes acknowledged responsibility for further delivery so individual devices don't need to stay online.
- **Do not repeat the "download spike ≠ working tool" mistake.** Any Riot pilot or evidence-sprint metric should track actual successful message/content delivery in the field, not installs or activity counts, especially under real congestion or shutdown conditions.

## Coverage Holes and Open Questions

Three of the six research angles produced **no surviving verified claims** and are genuine, not merely open, coverage holes:

- **Samizdat / USSR-era manual self-publishing and copying** — no claims were verified on this angle at all.
- **One-way radio/satellite broadcast distribution** (Toosheh into Iran, shortwave numbers stations, pirate FM during protests/coups) — sources were fetched (Wikipedia, IEEE Spectrum, Iran Human Rights) but zero claims survived to synthesis. A channel requiring no return path and no internet at all remains unresearched for Riot's purposes.
- **Digital-rights field guidance for full shutdowns** (Access Now #KeepItOn, EFF) beyond generic VPN advice, which doesn't apply when there's no connectivity at all — sources were fetched but produced no verified claims.

Additional open questions:

- Has any organization deployed a Briar-like offline-fallback system or a Cache-to-the-Future-style demand-driven retrieval network at real protest/disaster scale (not simulation)? This pass found strong simulation evidence and strong audited-design evidence, but no field-deployment data at comparable scale to El Paquete or DakNet.
- How does Briar's physical-proximity PSK attack generalize to Riot's own planned offline sync/pairing mechanisms, and what specific mitigation should Riot adopt before shipping any sneakernet-style install or sync path?

### Sourcing caveats

The Bridgefy findings describe app versions and audits from 2020-2022; the 2022 USENIX follow-up confirmed core flaws persisted even after a Signal-protocol adoption, but the app's current security state was not independently reverified here. The Cache to the Future simulation's mobility trace (YJMob100K) is documented by its own source only as a "dense Japanese city" — treat any more specific city label as unverified. The goTenna hop-limit and DTN custody-transfer claims each rest on a single primary source at a 2-1 (not unanimous) vote — plausible and sourced, not heavily corroborated.

## Primary Sources

- Univ. of Havana/Clarkson, El Paquete Semanal study (ACM COMPASS 2018) — https://lin-web.clarkson.edu/~jmatthew/publications/ElPaquete.pdf
- Wikipedia, "El Paquete Semanal" — https://en.wikipedia.org/wiki/El_Paquete_Semanal
- Society for Cultural Anthropology, Paquete Semanal series — https://www.culanth.org/fieldsights/series/paquete-semanal
- Dead Drops project — https://deaddrops.com/
- Wikipedia, "USB dead drop" — https://en.wikipedia.org/wiki/USB_dead_drop
- Bridgefy security analysis (IACR ePrint 2021/214) — https://eprint.iacr.org/2021/214.pdf
- Bridgefy security analysis (Springer, CT-RSA 2021) — https://link.springer.com/chapter/10.1007/978-3-030-75539-3_16
- Royal Holloway press release on Bridgefy — https://www.royalholloway.ac.uk/about-us/news/using-messaging-service-bridgefy-could-have-dire-consequences-for-users-if-privacy-protection-issues-aren-t-fixed/
- Techdirt, "Bridgefy messaging app hyped as great for protesters is a security mess" — https://www.techdirt.com/2020/08/27/bridgefy-messaging-app-hyped-as-great-protesters-is-security-mess/
- Hong Kong Anti-ELAB mesh-messaging interview study (arXiv) — https://arxiv.org/pdf/2105.14869
- Briar, "How it works" — https://briarproject.org/how-it-works/
- Briar independent security audit (Open Technology Fund) — https://www.opentech.fund/security-safety-audits/briar-security-audit/
- goTenna Mesh specs — https://gotennamesh.com/pages/gotenna-mesh-specs
- Kevin Fall, "A Delay-Tolerant Network Architecture for Challenged Internets" (SIGCOMM 2003) — https://dl.acm.org/doi/10.1145/863955.863960
- "Cache to the Future" preprint (arXiv, 2026) — https://arxiv.org/pdf/2606.17245
- Wikipedia, "Samizdat" (fetched, no surviving claims) — https://en.wikipedia.org/wiki/Samizdat
- Wikipedia, "Toosheh" (fetched, no surviving claims) — https://en.wikipedia.org/wiki/Toosheh
- IEEE Spectrum, Iran internet blackout / satellite TV (fetched, no surviving claims) — https://spectrum.ieee.org/iran-internet-blackout-satellite-tv
- Iran Human Rights, Toosheh interview (fetched, no surviving claims) — https://iranhumanrights.org/2016/03/toosheh-mehdi-yahyanejad/
- EFF, hacker's guide to circumventing internet shutdowns (fetched, no surviving claims) — https://www.eff.org/deeplinks/2026/05/hackers-guide-circumventing-internet-shutdowns
- Access Now, #KeepItOn campaign (fetched, no surviving claims) — https://www.accessnow.org/campaign/keepiton/
