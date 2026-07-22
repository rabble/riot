# Willow-Capability Gap Roadmap

Date: 2026-07-22
Status: Decision aid for sequencing. Not an implementation plan — no TDD tasks, no coverage gates. It orders four known gaps against work already in flight so the owner can decide what to start, what to defer, and what not to touch.

## Scope

Willow (as pinned: `willow25 =0.6.0-alpha.3`, per `docs/research/2026-07-10-willow-implementation-audit.md`) can express four capabilities that Riot's public communities do not currently use: confidential content, scoped read/write capabilities, confidentiality-preserving sync, and destructive/recall editing. Each gap is a case where the protocol is not the blocker — Riot-side enforcement, product design, or an unbuilt subsystem is. This document states, per gap, what Willow supports, what Riot does today, what the approved design's answer is, what "done" means, rough size, and risks; then proposes a sequence.

The willow25 crate already contains read capabilities, owned namespaces, and delegation (see project memory `willow25-access-control-exists`); the write side of that machinery is where the composite-site work finally exercised it (`crates/riot-ffi/src/site_ffi.rs`, `crates/riot-core` owned-cap plumbing, landed Units 0-2). The missing pieces are Riot's enforcement and product surfaces, not upstream protocol features.

## 1. Summary table

| Gap | User-visible harm today | Design status | Implementation status | Earliest sensible start | Blocking dependencies |
|---|---|---|---|---|---|
| 1. E2E-encryptable content | No confidential channel at all; every published item is signed plaintext anyone can read (by design for newswire) | Approved: dual-mode design `docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`, Groups module (Phase 2 Track B). Property-preserving Encrypted Willow explicitly deferred. | Entirely unbuilt. No Phase 0B contract, no implementation plan. | Can start Phase 0B evidence-contract design now (independent of meadowcap). Code start gated on that contract. | MLS library choice; Phase 0B threat-model contract must be written and reviewed first (per dual-mode design line 143). |
| 2. Meadowcap scoped read/write | Write authority is enforced; there is no confidential READ boundary — a peer that holds namespace bytes can read everything in an owned Space. | Approved: `docs/superpowers/specs/2026-07-11-full-meadowcap-management-design.md` (2026-07-11), 8 slices, hard release boundary. | Write-side landed (composite-site Units 0-2 on main). Read gate is slice 4, unbuilt. Slice-1 plan being drafted today (`docs/superpowers/plans/2026-07-22-meadowcap-slice1-capability-core.md`, in draft, not yet on disk). | Slice 1 now (in progress). | Slices 1-3 (capability core, governance, shared admission) precede slice 4 read gate (design lines 1335-1348). |
| 3. Confidential sync (PIO) | Conference-sync `/1` Hello/Summary frames disclose namespace + entry IDs pre-auth — a passive peer learns what a Space contains before proving any capability. | Approved as meadowcap slice 4 (protected-sync state machine). Current `/1` codec marked legacy public-only. | Unbuilt. Depends on the read gate. | After meadowcap slices 1-3; realistically the same milestone as the slice-4 read gate. | Meadowcap slices 1-3; upstream PIO/Confidential Sync/WTP are proposals/sketches, not final. |
| 4. Destructive editing / recall | No tombstone or mutable-pointer schema; a mistaken or harmful entry cannot be logically retracted, only out-competed by a newer prefix. True recall of already-received signed plaintext is impossible. | Deferred ("later schemas") per `docs/research/2026-07-10-willow-implementation-audit.md` findings 9. No dedicated approved design. | Unbuilt. Phase 0A path layout (4 components) deliberately avoids accidental pruning. | Small, independent; can start after a short design pass whenever an editorial need forces it. | Needs a migration/compat story for existing 4-component paths; no upstream blocker. |

## 2. Per-gap detail

### Gap 1 — End-to-end encryptable data

**What Willow supports.** Willow is content-agnostic; entries can carry ciphertext payloads, and the pinned crate does not require plaintext. Willow'25 also has an upstream property-preserving "Encrypted Willow" direction, which the dual-mode design explicitly defers.

**What Riot does today.** The public newswire is signed plaintext by design and permanently so — this is an honesty-contract commitment, not a limitation to be fixed (`docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md` decisions 2 and threat-model row "Traffic interception", line 135). There is no confidential channel of any kind in the shipping app.

**The approved design's answer.** Confidentiality lives entirely in the unbuilt Groups module: an MLS control plane ordering membership epochs plus complete Willow group-drops that are encrypted and padded as opaque artifacts, decrypted by members before local validation and merge (dual-mode design lines 80-92). The design targets confidentiality for group identifiers, membership material, Willow metadata, and content inside a drop; it does not claim to hide artifact existence, timing, channel, or padded size (line 26).

**What "done" means.** Per the dual-mode design, Track B does not release until MLS/mobile viability, concurrent-commit handling, long-offline recovery, and independent cryptographic review are settled (line 154). Phase 0B must first prove the MLS/private-envelope/invite claims under its own reviewed threat model and agent-hour budget (line 143) — that contract does not exist yet.

**Rough size.** Very large. A separate phase (0B evidence sprint) plus a full module. Not measured in slices.

**Risks.** MLS library choice on mobile is unresolved and load-bearing (dual-mode open questions, line 154). Mobile transport is local-only today (BLE-nearby + local-network IP; `riot-ffi` has zero iroh dependency — project memory `riot-mobile-transport-local-only`), so opaque group-drops would ride the same nearby transports first. Rendezvous is a separate deferred design (line 90).

### Gap 2 — Meadowcap scoped read/write capabilities

**What Willow supports.** Meadowcap (final upstream 2025-11-21) answers "may this receiver read or write this Willow area?" (`docs/superpowers/specs/2026-07-11-full-meadowcap-management-design.md` line 32). Read capabilities, owned namespaces, and delegation are present in the pinned crate.

**What Riot does today.** Write authority is genuinely enforced: composite-site Units 0-2 landed on main with `owner_write_capability`, `delegate_section` (refuses non-`/articles`), and cap-chain admission rooted at the followed site (`.beads/plans/active-plan.md` Unit 0; project memory `site-manifest-signer-binds-namespace-key`, `composite-site-vs-newswire-editorial`). There is no confidential read boundary — nothing gates who may read an owned Space's entries; possession of namespace bytes is effectively read authority.

**The approved design's answer.** The full-meadowcap design makes Meadowcap the real authority layer for both read and write, with governance kept separate from protocol validity (lines 24-37). It decomposes into 8 reviewed slices (lines 1308-1327): (1) capability codec/creation/delegation/verification/fixtures; (2) governance schemas + deterministic evaluator + transitive revocation; (3) shared contextual admission for local writes, imports, and synced entries; (4) protected-sync handshake + PIO/capability exchange + encrypted reconciliation + ProtectedDrop V1; (5) Open/Managed Space creation, roles, membership, recovery, migration; (6) Manifest V2 + permission algebra + app approvals; (7) native management/consent/recovery UX; (8) cross-device conformance + field exercise + security review.

**What "done" means.** The release boundary is explicit and strict: "No partial slice is marketed as full managed-Space security. The minimum releasable journey requires all eight" — create a managed Space, complete recovery protection, invite a second device/person, grant a restricted role, perform protected sync, approve an app subset, revoke the role offline, and reconcile to the same policy without a server (lines 1329-1333). The design also fails if a read capability works as a bearer token without receiver proof (line 1304).

**Rough size.** 8 slices, of which slice 1 is being planned today. Slices 1-3 are foundational and reusable; slices 5-8 are the bulk of the user-facing managed-Space experience.

**Risks.** The read gate (slice 4) depends on upstream PIO/Confidential Sync, which are proposals, not final (lines 18-21) — Riot commits only to a transport-independent contract with no interop claim. The write-side admission chokepoint is `bundle.rs verify_frame` via `decode_bundle_with_root` (project memory `composite-site-vs-newswire-editorial`); slice 3's shared admission must not hand-roll a subset of that gate (project memory `riot-reuse-canonical-gate`).

### Gap 3 — Confidential interest-overlap sync (PIO)

**What Willow supports.** Private Interest Overlap and Confidential Sync are upstream proposals that let two peers discover shared interest without disclosing their full holdings pre-authentication.

**What Riot does today.** The conference-sync `/1` Hello/Summary frames disclose namespace and entry IDs before any capability is proven (`docs/decisions/riot-conference-sync.md`). The meadowcap design marks that codec legacy public-only and specifies the replacement protected-sync state machine as slice 4.

**The approved design's answer.** Slice 4: a protected-sync handshake with receiver-authenticated read gating — a peer discloses entries only after proof of control of the receiver key of a covering read capability (design use case 5, lines 57-59; sequencing constraint line 1343). Riot defines a transport-independent authorization contract and makes no WTP or Confidential Sync interoperability claim until each conformance gate passes (lines 18-21).

**What "done" means.** Receiver-authenticated read gating precedes any claim that protected sync is available (line 1343). This is the same milestone as Gap 2's read gate — they are one slice.

**Rough size.** Part of slice 4; not separable from the read gate in practice.

**Risks.** Highest upstream-immaturity risk of the four: WTP is still a sketch and Confidential Sync is an unratified proposal. Auto-propagation of owned-namespace content to followers does not exist today (`install_sync_inventory` prunes to active community namespaces — project memory `riot-composite-owned-ns-no-autosync`), so a read-gated sync path has limited surface to protect until that propagation is built; this weakens the near-term urgency of Gap 3 relative to Gap 2's write/governance slices.

### Gap 4 — Logical destructive editing / recall

**What Willow supports.** Prefix pruning: a strictly newer entry at a prefix path dominates and removes older descendants (Willow join semantics; audit finding 5). This is the mechanism for tombstones and mutable pointers.

**What Riot does today.** Phase 0A uses 4-component paths (`objects`, `alert`, 16-byte object ID, 16-byte revision ID) that deliberately keep immutable revisions unrelated by prefix — accidental pruning is designed out (`docs/research/2026-07-10-willow-implementation-audit.md` finding 9, lines 33 and 112-118). There is no intentional tombstone or mutable-pointer schema. IMC-style "hide, not delete" is expressed today only as `moderation_action` annotations (reader-applied), not as data removal.

**The approved design's answer.** The audit names intentional tombstones and mutable pointers as "later schemas" (finding 9); no dedicated approved design exists. Crucially, true recall or secure erasure of already-received signed plaintext is impossible — this is honesty-contract territory, and the bounded-future-access answer is the Groups module's MLS epochs (Gap 1), not a newswire recall feature.

**What "done" means.** A reviewed schema for intentional prefix-pruning tombstones and/or mutable pointers, plus a migration story for existing 4-component paths, plus explicit marketing language that logical retraction is not erasure. There is no upstream blocker.

**Rough size.** Small and independent — a schema addition and a migration, not a subsystem.

**Risks.** Migration/compat: adding a mutable-pointer or tombstone path layout changes how existing entries relate by prefix; done carelessly it could retroactively prune durable data. Must be designed against the audit's join-semantics invariants. Compatibility with the honesty contract: any UI that reads as "delete" over a channel where copies already propagated is a misleading claim.

## 3. Sequencing proposal

> **Owner decision (2026-07-22), supersedes item 2 below:** MLS/Groups work — including the Phase 0B evidence-contract design — moves to the very end of the roadmap. Rationale: MLS is very hard for Riot's offline-first mobile posture; p2panda is abandoning MLS (shrinking Rust p2p-MLS prior art); Groups and property-preserving encryption depend on upstream Willow-team maturity, while everything else Riot can build alone. Destructive-editing schemas (item 4 below) are also promoted from "hold" to an early parallel track. The operative sequencing now lives in `docs/superpowers/plans/2026-07-22-willow-gap-master-plan.md`.

The structural facts that drive ordering:

- Meadowcap slices 1-3 (capability core, governance, shared admission) are prerequisites for both the read gate / protected sync (slice 4 = Gaps 2-read and 3) and for healthier owned-site admission (they replace the current admission source without weakening its schema checks, design line 1337). They pay off twice, so they are the highest-leverage next investment.
- Private groups (Gap 1) are independent of the meadowcap slices at the start — MLS + opaque drops need no read caps — and only converge later (both eventually want scoped roles). Gap 1 is gated on writing and reviewing a Phase 0B evidence contract, which is design work that can proceed in parallel without competing for the same implementation surface as meadowcap.
- Destructive-editing schemas (Gap 4) are small and independent but need a migration story; they should wait for a concrete editorial need rather than being built speculatively.
- Confidential sync (Gap 3) has the weakest near-term payoff because owned-namespace auto-propagation to followers does not exist yet — there is little protected traffic to protect until that lands.

Recommended order, interleaved with in-flight work (composite-site units, iOS surface units, spaces-first rungs, the anchor-network daemon):

1. **Continue meadowcap slice 1 (capability core) now**, then slices 2-3. This is the load-bearing foundation and is already starting. It also cleans up the write-side admission the composite-site work depends on.
2. **In parallel, write the Phase 0B private-groups evidence contract** (design only — MLS library evaluation, threat model, invite state machine). This unblocks Gap 1 without contending for meadowcap's implementation surface. Do not start Groups module code until this contract passes review.
3. **Defer the read gate + protected sync (Gaps 2-read and 3, slice 4)** until slices 1-3 are on main. It cannot land honestly before then (bearer-token read = design failure, line 1304), and its value grows once owned-namespace propagation to followers exists.
4. **Do not start destructive-editing schemas (Gap 4) yet.** Hold until an editorial workflow forces a real tombstone/mutable-pointer need; then do a short design pass with a migration plan. Building it speculatively risks the accidental-pruning failure mode the Phase 0A path layout was designed to avoid.
5. **Do not begin Groups module implementation, and make no confidentiality claim anywhere, until Phase 0B is reviewed.** Gap 1 is the largest single body of unbuilt work; starting it before its evidence contract exists repeats the pre-contract mistakes the phased design was written to prevent.

## 4. Honesty-contract implications

The website carries a blocking editorial contract gate — `scripts/marketing/protocol-page-contracts.mjs` — that pins honesty strings and locks marketing/guide copy against the app's actual capabilities (project memory `marketing-contract-gate-couples-web-and-app`). Every claim in the roadmap above has a paired marketing constraint:

- **No confidentiality or encryption claim may appear on the site or in the guide until the corresponding subsystem lands.** The newswire's "signed plaintext, anyone can read" framing is a permanent honesty commitment (dual-mode threat model, line 135) and must not be softened. Any future "private groups" or "encrypted" claim requires the Groups module to actually ship AND the contract gate to be updated in the same change.
- **"Managed-Space security" language is release-boundary-gated.** Per the meadowcap design, no partial slice may be marketed as full managed-Space security (line 1329). The site must not describe scoped roles, revocation, or protected sync as available until all eight slices land. Marketing slice 1-3 progress as "security" would violate the design's own release rule.
- **Any new confidentiality claim needs the contract gate updated atomically.** Because `protocol-page-contracts.mjs` is a byte-identical mirror gate coupling web and app, a new claim that is not simultaneously reflected in the pinned strings will either fail CI or ship an unbacked promise. Treat the gate update as part of the same change as the feature, never a follow-up.

## 5. Stale-state cleanup note

`.beads/plans/active-plan.md` is stale. It reads `status: unit0-complete; unit1-next` and describes Unit 1 as the next unblocked step, but composite-site work has since landed through Unit 6 partially (native UI slice #68, moderation, resolver, follow-by-ticket on both iOS and Android per `git log`), and the project has moved on to the anchor-network daemon, iOS surface units, spaces-first rungs, and the meadowcap slice-1 plan being drafted today. Recommendation: refresh `active-plan.md` to point at the meadowcap slice-1 plan (once it lands on disk) as the current active work, and record composite-site Units 1-6 status accurately, before the next agent primes off a stale position.
