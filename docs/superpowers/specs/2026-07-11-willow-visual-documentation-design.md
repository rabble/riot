# Willow Visual Documentation System

Date: 2026-07-11
Status: Approved in brainstorming; pending design review gate

## Purpose

Riot's technical documents rely heavily on Willow, but the primary protocol
links and the clearest explanations are concentrated in research notes. A
reader who lands directly on a design spec, implementation plan, or decision
report can encounter Willow terminology without first understanding how Riot
uses it.

This design makes every Willow-bearing technical document independently
understandable. It reuses Willow's official illustrations under the same
dual MIT or Apache-2.0 terms as the Willow code, preserves upstream
attribution and alt text, links claims directly to the canonical
specifications, and clearly separates Willow-defined semantics from
Riot-specific behavior.

## User and Outcome

**Who:** an engineer, reviewer, security auditor, organizer, or contributor who
opens any Riot technical document directly.

**Wants:** to understand why Willow is present, how authority works, how data
moves when networks fail, how disconnected copies converge, and which behavior
belongs to Willow versus Riot.

**So that:** they can evaluate Riot's architecture without first reconstructing
the protocol from scattered internal research.

**When:** reading the README, product and architecture documentation, research,
decision reports, approved design specs, or implementation plans.

Success means:

- every in-scope Willow-bearing document contains the complete visual primer;
- every protocol claim has a direct path to an official Willow source;
- every document identifies its Willow/Riot boundary and implementation status;
- no illustration depends on a remote image host;
- all reused assets retain accessible alt text, provenance, integrity metadata,
  and license notices;
- repository validation rejects missing coverage or broken provenance.

Failure means a reader can still land on an in-scope document and encounter
Willow as unexplained jargon, a document implies that a proposal or sketch is
implemented interoperability, or an illustration becomes unavailable or
unattributed.

## Editorial Decision

Every in-scope technical document receives a full, compact visual narrative.
The narrative always appears in this order:

1. **Authority — who may do what?** Human-controlled keys and Meadowcap
   capabilities determine who may read or write which data.
2. **Movement — how does information survive a shutdown?** Signed data can
   travel through files, USB, messaging, local wireless, or later live
   synchronization.
3. **Convergence — how do disconnected copies become one view?** Namespaces,
   subspaces, paths, timestamps, payloads, and deterministic store joins let
   offline devices merge predictably.

This order is deliberate. Riot first establishes human authority and safety,
then communicates shutdown resilience, then explains the protocol machinery.
The visual treatment is GitHub-native white with ordinary Markdown structure
and crisp rules. Riot does not recolor, crop, or restyle the Willow artwork.
The original illustrations retain their own warm-paper backgrounds.

## Canonical Explanation

`docs/architecture/willow-architecture.md` remains the canonical explanation;
no competing architecture overview is introduced. Its opening becomes the
complete visual narrative and its later sections provide the detailed mapping
from Riot concepts to Willow concepts.

The README and all other primers link to that document using a stable
`how-riot-uses-willow` anchor. The canonical explanation covers:

- independent namespaces and per-author subspaces;
- hierarchical paths, timestamps, payloads, prefix pruning, and store joins;
- Meadowcap read and write authority for owned and communal namespaces;
- Riot's signed object schemas and local receipt/provenance layer;
- asynchronous file exchange and current Riot evidence bundles;
- Drop Format and WTP as future interoperability targets, not current claims;
- Encrypted Willow techniques and Riot's separate private-group design;
- the distinction between final protocol foundations and provisional sync
  specifications.

## Repeated Visual Prologue

Each in-scope document contains exactly one marked prologue:

```markdown
<!-- willow-visual-primer:v1 -->
## How Riot uses Willow

Riot stores human-signed information in independent Willow spaces, moves it
through whatever channels remain available, and deterministically merges
copies when devices meet again.

| 1. Authority | 2. Movement | 3. Convergence |
| --- | --- | --- |
| `meadowcap-capability-ticket.png` | `drop-adhoc-transport-chain.png` | `data-model-subspaces.png` |
| Human-controlled keys and Meadowcap capabilities determine who may read or write which data. | Signed data can travel through files, USB, messaging, local wireless, or later live synchronization. | Namespaces, subspaces, paths, timestamps, and payloads let offline stores merge predictably. |
```

The block uses three separate images rather than a composed collage so each
figure retains its full upstream alt text and source identity. The Markdown
must render acceptably in GitHub light and dark modes and in a plain local
renderer. It must not rely on custom CSS, JavaScript, remote image loading, or
HTML styling that GitHub may sanitize.

Immediately after the primer, every document adds a tailored paragraph:

```markdown
**This document's boundary.** Willow defines ... . Riot defines ... .
Implemented today: ... . Proposed or gated: ... .
```

The boundary paragraph is not boilerplate. It identifies the exact Willow
semantics consumed by that document, the Riot-specific layer it designs or
records, and whether each part is implemented, evidence-only, or future work.

## Scope and Coverage

The coverage roots are:

- `README.md`;
- `docs/product/**/*.md`;
- `docs/architecture/**/*.md`;
- `docs/research/**/*.md`;
- `docs/decisions/**/*.md`;
- `docs/superpowers/specs/**/*.md`;
- `docs/superpowers/plans/**/*.md`.

A coverage manifest explicitly lists every document that receives the primer.
At implementation time, the initial list includes every document in those
roots that materially discusses Willow, Meadowcap, Drop Format, WTP, or
Confidential Sync. Historical research and decision reports receive the same
complete orientation; their original findings remain intact beneath it.

A newly added document is considered materially Willow-bearing when it either:

- contains a direct `willowprotocol.org` specification link; or
- contains at least three case-insensitive occurrences, in total, of `Willow`,
  `Meadowcap`, `Drop Format`, `Willow Transfer Protocol`, `WTP`, or
  `Confidential Sync`.

Such a document must appear in the coverage manifest or in a small explicit
exemption list with a written rationale. Incidental references do not force a
full primer, but exemptions cannot be silent.

Implementation plans retain their executable structure. The visual prologue
appears before plan mechanics and does not alter task numbering or command
snippets.

## Targeted Figures

The three primer illustrations appear everywhere. Documents add further
official figures only when they directly explain the local subject:

| Willow concept | Additional official figures | Riot documents that benefit |
| --- | --- | --- |
| Hierarchical paths and overwrites | initial paths, timestamped overwrite, prefix pruning | data model, evidence store, app-data paths |
| Subspaces and namespaces | path/time/subspace axes, independent namespaces | dual mode, app directory, publication spaces |
| Capability verification | valid/forged capability comic, capability ticket | signed apps, gateway verification, trust and authority |
| Namespace authority | communal-house and owned-house illustrations | open newswire, publications, private-group boundaries |
| Improvised movement | people carrying drops, USB/email/message/local-wireless chain | nearby transport, conference demo, shutdown research |
| Selective synchronization | two stores exchanging selected regions | confidential sync and private-interest research |

Figures are never added merely as decoration. Each occurrence has a caption
stating what Willow shows, how Riot applies it, and the upstream specification
status where maturity affects the claim.

## Asset Catalog and Licensing

Exact upstream files are vendored under `docs/assets/willow/` using semantic,
stable filenames. The initial catalog contains twelve illustrations:

- five Data Model figures: paths, overwrite, prefix pruning, subspaces, and
  namespaces;
- four Meadowcap figures: capability verification, communal namespace, owned
  namespace, and capability ticket;
- two Drop Format figures: improvised carriers and the ad-hoc transport chain;
- one Confidential Sync figure: selective exchange between stores.

`docs/assets/willow/manifest.json` records, for every asset:

- local semantic filename;
- exact upstream URL;
- SHA-256 of the vendored bytes;
- original upstream alt text;
- source specification URL and status;
- license expression `MIT OR Apache-2.0`;
- attribution holder `Willow / worm-blossom`.

`docs/assets/willow/ATTRIBUTION.md`, `LICENSE-MIT`, and `LICENSE-APACHE` ship
beside the assets. Documentation captions use concise attribution and link to
the full local attribution file. Asset bytes are copied exactly; no destructive
optimization, metadata rewrite, recoloring, cropping, or AI alteration occurs.

Remote upstream availability is not a rendering dependency. Upstream URLs are
provenance, not hotlinks.

## Protocol Sources and Status

Documents link directly to the relevant official source rather than only to
internal research:

- [Willow Data Model](https://willowprotocol.org/specs/data-model/) — final;
- [Meadowcap](https://willowprotocol.org/specs/meadowcap/) — final;
- [Willow'25](https://willowprotocol.org/specs/willow25/);
- [Encodings](https://willowprotocol.org/specs/encodings/);
- [Willow Drop Format](https://willowprotocol.org/specs/drop-format/) —
  proposal;
- [Willow Transfer Protocol](https://willowprotocol.org/specs/wtp/) — sketch;
- [Confidential Sync](https://willowprotocol.org/specs/confidential-sync/);
- [Encrypted Willow](https://willowprotocol.org/specs/e2e/).

Status labels are copied from the upstream pages and include their upstream
as-of date when one is published. Riot's documents must not flatten these
different maturity levels into a generic claim that "Willow supports" a
production-ready feature.

## Validation Contract

The existing Rust `xtask` validation path gains a documentation validator. Its
source and tests follow mandatory TDD and the repository's 100% line, branch,
function, and statement coverage requirement.

The RED tests prove that validation rejects:

- a covered document with no primer marker;
- zero or multiple primer markers;
- a covered document with no boundary paragraph;
- a materially Willow-bearing document omitted from both coverage and
  exemptions;
- an exemption with no rationale;
- a missing local image;
- an asset whose SHA-256 differs from the manifest;
- a remote Willow image URL in Markdown;
- an image occurrence with missing or altered alt text;
- an asset with missing source, attribution, or license metadata;
- a protocol-status claim inconsistent with the status registry;
- a missing direct official source link where the document names that
  protocol as a dependency.

The GREEN implementation reads only repository files and performs no network
access. It emits a file path and actionable reason for every failure. Network
link checking remains a deliberate manual/update task so transient upstream
availability cannot make CI flaky.

Positive tests cover a complete document, a justified incidental-reference
exemption, all vendored assets, and the committed repository coverage manifest.
The full quality gate remains:

```text
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo tarpaulin --fail-under 100
cargo xtask validate-contracts
```

The documentation validator is invoked by `validate-contracts`, so it is a
blocking completion and PR gate rather than an optional lint.

## Update Workflow

When upstream artwork or protocol status changes:

1. fetch the exact official asset or specification metadata;
2. review the upstream change rather than accepting it mechanically;
3. update the vendored file, hash, alt text, status, and attribution together;
4. update affected boundary paragraphs if semantics or maturity changed;
5. run the full validation and coverage gates;
6. review the rendered README, canonical architecture document, one design
   spec, one research note, and one implementation plan in both GitHub themes.

The validator never silently downloads or rewrites assets.

## Accessibility

- Original upstream alt text is preserved verbatim in the asset manifest.
- Every Markdown use supplies meaningful alt text; decorative empty alt text is
  not permitted for these explanatory figures.
- Captions explain the concept without requiring readers to distinguish colors.
- The three-panel layout must remain readable at narrow widths and at 200% zoom.
- The prose contains every essential claim; images reinforce rather than replace
  the explanation.

## Risks and Mitigations

**Repetition and drift.** A full primer appears in many documents. The versioned
marker, coverage manifest, and validator keep the shared block structurally
consistent; only the boundary paragraph is intentionally local.

**Documentation bloat.** Vendored images increase repository size and repeated
primers increase page length. Twelve shared image files are referenced rather
than duplicated, and the repeated prose stays compact.

**Protocol overclaiming.** Visual polish can make provisional work look done.
Every boundary paragraph and status label distinguishes final upstream
foundations, upstream proposals/sketches, implemented Riot behavior, and future
gates.

**Attribution loss.** The asset manifest, local licenses, per-use captions, and
blocking validation make provenance part of the documentation contract.

**Historical-document distortion.** Adding orientation must not rewrite the
evidentiary record. Existing research findings, dates, and decisions remain
unchanged except where a demonstrably incorrect link or current-status label is
being corrected explicitly.

## Definition of Done

- The twelve upstream illustrations are vendored byte-for-byte with manifest,
  hashes, alt text, attribution, and both license texts.
- `willow-architecture.md` contains the canonical security → movement →
  convergence explanation and direct official links.
- README and every in-scope Willow-bearing technical document contain the full
  visual primer and a tailored boundary paragraph.
- Relevant documents include additional concept-specific figures and captions.
- Protocol maturity and Riot implementation status are explicit and accurate.
- The documentation validator is built test-first, runs offline, is included in
  `validate-contracts`, and passes the repository's 100% coverage gate.
- Representative pages are visually reviewed in GitHub light and dark themes,
  at narrow width, and at 200% zoom.
- No unrelated source, application UI, protocol behavior, or historical
  conclusion changes.
