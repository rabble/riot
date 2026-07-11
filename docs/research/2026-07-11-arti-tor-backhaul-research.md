# Arti as Riot's Tor Backhaul: Maturity, Embeddability, Audit History

Date: 2026-07-11
Method: Deep-research workflow — 6 search angles, 25 sources fetched, 100 claims extracted, top 25 adversarially verified (3 independent verification votes per claim), synthesized to 5 findings. 19 of 25 votes confirmed, 6 refuted.

## Purpose

`docs/research/2026-07-11-hybrid-gossip-backhaul-research.md` found Briar the strongest real-world precedent for hybrid local-gossip + opportunistic-internet-backhaul, using Tor as its backhaul transport — but Briar bundles the standard C `tor` daemon. Riot's core is Rust, targeting iOS/Android via UniFFI. This pass investigated **Arti**, the Tor Project's from-scratch Rust reimplementation of Tor, as the concrete mechanism for backhauling over Tor from inside `riot-core` instead of bundling a separate C daemon.

## Summary

Arti is architecturally well-suited to Riot's Rust core in principle: the Tor Project's own docs name direct in-Rust API embedding (the `arti_client` crate) as the primary integration method, and Arti has been self-declared "ready for production use" as a client since v1.0.0 (2022), with onion-service client/host support shipped incrementally since roughly v1.1–1.4 and still receiving work in 1.8.0 (Dec 2025). But the mobile/FFI story Riot actually needs is immature: Arti itself has no stable Rust API and ships no UniFFI or other high-level bindings; its own iOS guide says developers must hand-roll raw `#[no_mangle] extern "C"` FFI, that async/futures don't translate cleanly to Swift, and that the guide "hasn't been tested by many people." Relay/directory-authority functionality — needed for Riot to host inbound backhaul/bridge connections — was still under active backend development as of the Dec 2025 1.8.0 release. A June 2025 Cure53 audit of the Onionmasq/Tor-VPN-for-Android mobile tunnel layer found the project's only High-severity bug, plus another FFI-specific issue, precisely in its mobile FFI boundary code (Apple/Android JNI) — direct proof-of-risk for the kind of FFI surface Riot's own UniFFI layer would create. No primary-source evidence was found, positive or negative, on mobile battery/memory/circuit-latency costs of running Arti, nor on Tor Project guidance specific to intermittently-connected mobile nodes acting as relays/bridges — genuine open questions.

## Verified Findings

### Embedding model: in-Rust library is the blessed path, but pre-1.0

- **Arti's own docs name direct in-Rust library embedding via `arti_client` as the primary/recommended integration method.** It's described as "the highest-level library crate in Arti, and the one that nearly all client-only programs should use," exposing Tor connections as async streams (`AsyncRead`/`AsyncWrite`) — architecturally the right shape for `riot-core` (itself Rust) to embed Arti in-process, without needing Arti's own FFI layer at all. [high confidence] (arti.torproject.org, docs.rs, Arti's GitLab repo)
- `arti_client` itself is still pre-1.0 (v0.44.0 as of mid-2026), and the Tor Project warns of "fairly frequent semver bumps" requiring active maintenance by dependents — a live dependency risk for Riot, not a one-time integration cost.

### Mobile/FFI story: acknowledged, unsolved gap

- **Arti explicitly has no stable Rust API and ships no proper mobile bindings** — no UniFFI, no cbindgen-generated bindings provided by the project. Its own iOS integration guide requires developers to hand-write raw `#[no_mangle] extern "C"` FFI functions, notes that Arti's heavy reliance on Rust futures has "no easy way" to be used from Swift (recommending blocking-on-futures or manual callback passing as workarounds), and states a proper blocking embedding API "will eventually" exist but does not yet, soliciting outside help to design it. The guide itself warns it "hasn't been tested by many people" and has "rough edges." Separately, `arti_client`'s own crate docs recommend non-Rust callers spawn the `arti` CLI as a SOCKS proxy subprocess rather than link the library directly: "We don't yet offer an API that would be nice to expose via FFI." [high confidence, 7 merged claims all 3-0] Zero mentions of UniFFI appear anywhere across Arti's own docs.
- **Arti 1.4.0 (Feb 2025) introduced an RPC interface** as its replacement for C Tor's control port, explicitly framed as an alternative way to integrate Arti into other applications without directly embedding the Rust library — a viable non-library integration path (run Arti as a companion process) that would sidestep the FFI-immaturity problem, but reintroduces the "bundle a separate daemon" architecture Riot would be choosing Arti specifically to avoid. [high confidence] Not yet declared stable by the RPC team.

### Client/onion-service maturity vs. relay maturity

- **Client functionality was self-declared "ready for production use" at v1.0.0 (Sept 2022)**, claiming similar privacy/usability/stability to C Tor for client connections. Onion-service client and host support has been built up incrementally since (~1.1.8–1.2.0 infrastructure work, 1.2.7–1.4.0 onion-service releases), with 1.8.0 (Dec 2025) adding further quality-of-life work including an experimental command to migrate C-Tor onion-service client-authorization keys into Arti's keystore — onion-service support is real and not feature-gated, it has simply grown incrementally. [high confidence]
- **By contrast, relay/directory-authority functionality — the piece needed for a Riot node to accept inbound backhaul/bridge connections — was still described as under active backend development as of 1.8.0**, with the OR port listener (required to accept inbound relay connections) still being implemented at that release. [high confidence] This is the single most important gap for Riot's specific "some nodes bridge inbound" use case, distinct from ordinary outbound client use or hosting an onion service on top of client infrastructure.
- *Refuted, do not rely on:* that relay support at 1.4.0 meant Arti couldn't yet host onion services at all [0-3]; that onion-service support exists only behind opt-in feature flags [1-2]; that Arti at 1.0.0 explicitly lacked onion-service support [0-3]. Onion-service hosting is real and stable-ish; relay/inbound-listener capability specifically is what remains unfinished.

### Independent security audit: the one High-severity bug was in mobile FFI code

- **A June 2025 Cure53 audit (commissioned by the Tor Project, six named specialists, full source access) covered TorVPN for Android and Onionmasq** — the Rust-based tunnel layer that forwards traffic (TCP/UDP parsing, DNS, routing) into the Tor network through Arti. The audited codebase already contains dedicated mobile FFI wrapper crates for iOS (`onionmasq-apple`, using unsafe raw-pointer functions for an iOS NetworkExtension) and Android (`onionmasq-mobile`, exposing JNI entry points). **The audit's single High-severity finding across all 18 issues — an out-of-bounds memory read from an untrusted length value — was located in the Apple FFI code**, and a second, distinct Low-severity finding (a double file-descriptor close) was found in the Android/JNI FFI code. Both FFI-boundary bugs, not core Tor/circuit-logic bugs. [high confidence, verified against the full 26-page Cure53 PDF] This audited the tunnel/VPN wrapper layer sitting in front of Arti, not Arti's own onion-routing/circuit-building code — but it is a direct proof-of-risk for exactly the architectural seam Riot's own UniFFI-based iOS/Android bindings would create.
- *Refuted, do not rely on:* a claim that the audit found Arti's core Tor integration architecturally robust with only best-practice-level findings and no serious vulnerabilities [1-2] — the High-severity finding is real, in the mobile FFI layer specifically.

## Cross-Cutting Patterns

1. **Arti's maturity is bimodal: client-side is production-declared, inbound/relay-side is still under construction.** Riot's design should not assume symmetric maturity for outbound backhaul (mature) and inbound bridge-hosting (immature).
2. **The mobile embedding gap is not merely undocumented — it is the Tor Project's own acknowledged, actively-unsolved problem**, not something Riot would be pioneering against silent documentation. The project explicitly asks for outside help designing the blocking embedding API.
3. **Every documented severe security bug adjacent to Arti found so far lives at the mobile FFI boundary**, not in the core protocol implementation — a direct, evidenced argument for treating that boundary as the highest-risk surface in any Riot integration, consistent with the general "offline/physical-proximity and FFI boundaries are their own attack surface" pattern already found in the Briar research.
4. **Two viable integration shapes exist with different tradeoffs**: in-process library linking (mature client API, immature FFI/mobile story) vs. spawning Arti as a subprocess and talking to it over the new RPC interface (avoids the FFI-immaturity problem, but reintroduces a bundled-daemon architecture and is itself not yet declared stable).

## Design Implications for Riot

- **Do not plan on Arti as an inbound bridge/relay node in the near term.** The OR-port listener needed to accept inbound Tor connections was still under active development as of Dec 2025. Riot's backhaul design (per the hybrid-gossip-backhaul research) should treat "device with connectivity offers itself as an onion service host" (outbound-capable, mature) as the near-term target, not "device accepts inbound relay traffic" (immature).
- **Treat the UniFFI/mobile FFI boundary as the highest-risk, highest-engineering-cost part of an Arti integration**, not a mechanical wrapper-generation step — Cure53's only High-severity finding in the whole Onionmasq audit was exactly there. Any Riot FFI layer around Arti should get independent security review specifically at that boundary, mirroring the Briar PSK-attack-surface lesson from the shutdown-resistant-distribution research: offline/embedding boundaries need dedicated scrutiny, not inherited assumptions from the core library's own maturity.
- **Consider the RPC-interface/subprocess model as a fallback if hand-rolled FFI proves too costly**, accepting the tradeoff of bundling Arti as a companion process (closer to what Briar already does with C tor) rather than true in-process embedding — a real, evidenced alternative, not a hypothetical one, though not yet declared stable itself.
- **Investigate Guardian Project's Onionmasq (`onionmasq-mobile`, `onionmasq-apple`) as a reference implementation or reusable component** before hand-rolling a fresh UniFFI layer against raw `arti_client` per Arti's own self-admittedly untested, "rough-edged" iOS guide — Onionmasq already implements a working mobile FFI + tunnel-forwarding layer atop Arti and has been through one Cure53 audit, which fixed the specific bugs found.
- **This does not overturn the prior finding that Tor-based backhaul is a strong pattern in principle** (Briar's Bramble Sync Protocol runs identically over Tor and local transports) — it specifically informs *which Rust implementation and integration shape* Riot should pick, and flags that "backhaul over Tor" and "accept inbound bridge connections over Tor" are not equally ready today.

## Coverage Holes and Open Questions

- **No primary-source evidence was found on Arti's (or Orbot's, Tor Browser for Android's, Onion Browser for iOS's) actual mobile battery, memory, or circuit-build-latency figures** — this was an explicit research question and remains genuinely unanswered, not answered-negatively. Needs a dedicated follow-up pass against Guardian Project/Orbot documentation specifically.
- **No claims survived on Tor Project guidance or warnings about intermittently-connected mobile devices acting as relays, bridges, or onion-service hosts** (battery drain, IP/NAT churn, anonymity-set concerns) — also a real coverage hole, not a negative finding.
- Is the Guardian-Project-hosted "Arti Mobile" repo (early test builds for iOS/Android, described as Tor-Project-supported but living outside Arti's own crate) a viable near-term FFI/UniFFI bridge Riot could adopt or fork, and how mature/audited is it relative to the hand-rolled-FFI path in Arti's own iOS.md?
- Would it be lower-risk for Riot to study or directly reuse patterns from Onionmasq's `onionmasq-mobile`/`onionmasq-apple` crates, given they already implement a working, once-audited mobile FFI + tunnel layer atop Arti?

### Sourcing caveats

Several claims are anchored to specific Arti releases (1.0.0 Sept 2022, 1.4.0 Feb 2025, 1.8.0 Dec 2025); the latest confirmed crate version referenced was `arti-client` 0.44.0 / Arti 2.5.0 (~mid-2026). Given Arti's stated "fairly frequent semver bumps" and ongoing relay-development cadence, the relay/OR-port-listener and mobile-FFI-immaturity findings should be treated as accurate as of this pass but worth re-checking if Riot's design phase slips more than a few months.

## Primary Sources

- Arti integrating-arti guide — https://arti.torproject.org/integrating-arti/
- `arti_client` crate docs — https://docs.rs/arti-client/latest/arti_client/
- Arti GitLab repository — https://gitlab.torproject.org/tpo/core/arti
- Arti iOS integration guide — https://gitlab.torproject.org/tpo/core/arti/-/blob/main/doc/iOS.md
- Arti iOS guide (docs mirror) — https://tpo.pages.torproject.net/core/arti/integrating-arti/custom-wrappers/iOS/
- Tor Project blog, "Arti 1.0.0 is released" — https://blog.torproject.org/arti_100_released/
- Tor Project blog, "Arti 1.4.0 is released" — https://blog.torproject.org/arti_1_4_0_released/
- Tor Project blog, "Arti 1.8.0 is released" — https://blog.torproject.org/arti_1_8_0_released/
- Cure53 audit of Tor VPN for Android / Onionmasq (PDF) — https://www.torproject.org/static/findoc/code_audits/Cure53_audit_jul_2025.pdf
- Tor Project blog, "Code audit: Tor VPN" — https://blog.torproject.org/code-audit-tor-vpn/
- Guardian Project, "Arti: next-gen Tor on mobile" — https://guardianproject.info/2023/03/04/arti-next-gen-tor-on-mobile/
- Tor community, relay requirements — https://community.torproject.org/relay/relays-requirements/
- Tor community, Snowflake relay setup — https://community.torproject.org/relay/setup/snowflake/
