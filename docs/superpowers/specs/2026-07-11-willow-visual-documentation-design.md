# Willow Visual Documentation System

Date: 2026-07-11
Status: Approved in brainstorming; revision 2 pending design review gate

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

## Users and Outcomes

### Protocol-new contributor

**Who:** an engineer or organizer who encounters Willow terminology in a Riot
document without knowing the protocol vocabulary.

**Wants:** to understand how authority works, how data moves when networks fail,
how disconnected copies converge, and which behavior belongs to Willow versus
Riot.

**So that:** they can evaluate the document without first reconstructing the
protocol from scattered internal research.

**When:** landing directly on any technical document rather than entering
through the README.

### Technical or security reviewer

**Who:** a returning engineer, protocol expert, or security auditor.

**Wants:** to skip familiar orientation quickly and locate the document's exact
Willow boundary, protocol maturity, and implemented-versus-proposed status.

**So that:** repeated background does not obstruct review of the local design or
evidence.

**When:** reviewing a design spec, implementation plan, or historical decision.

### Documentation maintainer

**Who:** a contributor adding or updating a Willow-bearing document or asset.

**Wants:** deterministic coverage, provenance, status, and accessibility rules
with actionable validation failures.

**So that:** the visual explanation stays trustworthy as the corpus and upstream
protocol evolve.

**When:** running local validation or opening a pull request.

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

### Reader acceptance study

Within seven days of implementation, five representative readers are tested on
five documents: README, canonical architecture, one design spec, one historical
research or decision record, and one implementation plan. At least four of five
readers must, without following more than one link:

- identify who controls read/write authority;
- name at least three shutdown-capable movement channels;
- explain that Willow stores converge through protocol-defined semantics;
- correctly distinguish Willow-defined behavior from Riot-defined behavior;
- correctly identify implemented Riot behavior versus proposal/sketch work.

Each reader must answer all five questions correctly within five minutes. An
expert or returning reader must reach the first document-specific heading after
the primer within 30 seconds. Failure by two or more readers on any question,
any critical screen-reader navigation failure, horizontal scrolling at 320 CSS
pixels, or inability to bypass the primer within 30 seconds fails acceptance and
requires an editorial/layout revision.

The repository maintainer owns the coverage and status registries. The product
owner owns the initial reader study and repeats it after a primer-version change;
ordinary document additions require automated validation and rendered review,
not a new study.

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
and crisp rules. The normative form is a vertical, numbered three-step sequence,
not a Markdown table: GitHub tables do not stack responsively and would force
horizontal navigation at narrow widths or 200% zoom. Riot does not recolor,
crop, or restyle the Willow artwork. The original illustrations retain their
own warm-paper backgrounds.

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

Each in-scope document contains exactly one prologue bounded by paired markers.
The example below uses asset catalog identifiers because the correct relative
Markdown path depends on the document's directory. The validator resolves each
relative path, then compares its catalog identity and normalized Markdown
structure:

```markdown
<!-- willow-visual-primer:start:v1 -->
## How Riot uses Willow

Riot stores human-signed information in independent Willow spaces, moves it
through whatever channels remain available, and deterministically merges
copies when devices meet again.

### 1. Authority

![A neat capability ticket identifying its receiver, access mode, area, and validity.](../../assets/willow/meadowcap-capability-ticket.png)

Human-controlled keys and Meadowcap capabilities determine who may read or
write which data.

### 2. Movement

![Willow data moving between devices by USB, email, messaging, and local wireless.](../../assets/willow/drop-adhoc-transport-chain.png)

Signed data can travel through files, USB, messaging, local wireless, or later
live synchronization.

### 3. Convergence

![Willow entries arranged by hierarchical path, timestamp, and author subspace.](../../assets/willow/data-model-subspaces.png)

Namespaces, subspaces, paths, timestamps, and payloads let offline stores merge
predictably.

Sources and status: [Data Model](https://willowprotocol.org/specs/data-model/)
(final); [Meadowcap](https://willowprotocol.org/specs/meadowcap/) (final);
[Drop Format](https://willowprotocol.org/specs/drop-format/) (proposal);
[WTP](https://willowprotocol.org/specs/wtp/) (sketch). Illustrations: Willow /
worm-blossom; see [local attribution](../../assets/willow/ATTRIBUTION.md).
<!-- willow-visual-primer:end:v1 -->
```

The paths above are exact for this design document. The documentation tool
computes the correct relative path for documents at other depths. The canonical
structured definition supplies exact text, catalog IDs, upstream alt text,
captions, sources, statuses, and attribution. Each rendered document contains
ordinary valid relative Markdown destinations.
The validator uses a CommonMark/GFM-aware parser, ignores marker-like text in
fenced or inline code, and compares the parsed content between the paired
markers against that canonical definition after normalizing line wrapping and
resolving relative paths. Changed prose, reordered steps, substituted images,
changed alt text, missing captions, missing sources, or missing attribution are
all failures.

The block uses three separate, vertically ordered images rather than a composed
collage so each figure retains its full upstream alt text and source identity.
It renders without horizontal navigation in GitHub light and dark modes, a
plain local renderer, 320 CSS pixels, and 200% zoom. It must not rely on custom
CSS, JavaScript, remote image loading, or HTML styling that GitHub may sanitize.

Immediately after the primer, every document adds a tailored paragraph:

```markdown
**This document's boundary.** **Willow defines:** ... . **Riot defines:** ... .
**Implemented today:** ... . **Proposed or gated:** ... .
```

The boundary paragraph follows that fixed four-part label pattern but is not
boilerplate. It identifies the exact Willow
semantics consumed by that document, the Riot-specific layer it designs or
records, and whether each part is implemented, evidence-only, or future work.
It appears immediately after the end marker. The primer H2 may appear in a
generated table of contents; because it is identical and first, expert readers
can jump directly to the following document-specific heading.

## Machine-Readable Contracts

All validator inputs are UTF-8 JSON with unknown fields rejected, duplicate JSON
keys rejected, and arrays required to be in ascending canonical order.

`docs/assets/willow/primer.json` is the single canonical primer definition:

```json
{
  "version": 1,
  "heading": "How Riot uses Willow",
  "thesis": "Riot stores human-signed information in independent Willow spaces, moves it through whatever channels remain available, and deterministically merges copies when devices meet again.",
  "steps": [
    { "number": 1, "heading": "Authority", "asset_id": "meadowcap-capability-ticket", "caption": "Human-controlled keys and Meadowcap capabilities determine who may read or write which data." },
    { "number": 2, "heading": "Movement", "asset_id": "drop-adhoc-transport-chain", "caption": "Signed data can travel through files, USB, messaging, local wireless, or later live synchronization." },
    { "number": 3, "heading": "Convergence", "asset_id": "data-model-subspaces", "caption": "Namespaces, subspaces, paths, timestamps, and payloads let offline stores merge predictably." }
  ],
  "protocols": ["data-model", "meadowcap", "drop-format", "wtp"],
  "attribution_path": "docs/assets/willow/ATTRIBUTION.md"
}
```

`docs/assets/willow/protocols.json` is the reviewed local status registry. Each
record has exactly `id`, `name`, `official_url`, `status`, `upstream_as_of`,
`verified_on`, and `evidence_url`. `status` is one of `final`, `proposal`,
`sketch`, or `status-not-stated`. Dates are ISO 8601 or `null` when upstream
publishes no date. The validator checks consistency against this reviewed
snapshot; it does **not** claim to prove current upstream truth. Changing the
registry requires fresh upstream review and product-owner approval.
Its top level is `{ "version": 1, "protocols": [...] }`; IDs are unique and
sorted. The initial IDs are `confidential-sync`, `data-model`, `drop-format`,
`encodings`, `encrypted-willow`, `meadowcap`, `willow25`, and `wtp`.

`docs/assets/willow/coverage.json` has this exact top-level shape:

```json
{
  "version": 1,
  "roots": ["README.md", "docs/product", "docs/architecture", "docs/research", "docs/decisions", "docs/superpowers/specs", "docs/superpowers/plans"],
  "documents": [
    { "path": "README.md", "protocols": ["data-model", "meadowcap", "drop-format", "wtp", "encrypted-willow"], "extra_asset_ids": [] }
  ],
  "exemptions": [
    { "path": "example.md", "owner": "repository-maintainer", "reviewed_on": "2026-07-11", "rationale": "Example schema only; this record is not present in the committed manifest." }
  ]
}
```

The illustrative exemption above describes the schema and is not committed as
an actual bypass; the initial `exemptions` array is empty. Document paths are normalized repository-relative UTF-8 paths
using `/`; absolute paths, empty components, `.`, `..`, backslashes, NUL/control
characters, normalization collisions, symlinks, and paths outside the fixed
roots are rejected before any file is opened. Documents and exemptions must be
unique, disjoint, existent regular files, and sorted. Rationales and owners must
be nonempty; review dates may not be in the future.

Each covered document declares its local dependency set in one machine-readable
comment immediately before the start marker, for example:

```markdown
<!-- willow-protocols:v1 data-model meadowcap drop-format wtp -->
```

IDs must match the sorted `protocols` array in `coverage.json`. The validator
uses these declarations—not natural-language inference—to require exact
official links and reviewed status labels.

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
The initial scope is frozen at these 31 documents:

1. `README.md`
2. `docs/architecture/willow-architecture.md`
3. `docs/decisions/phase0a-wu0-report.md`
4. `docs/decisions/phase0a-wu1-report.md`
5. `docs/decisions/phase0a-wu2a-report.md`
6. `docs/decisions/riot-conference-sync.md`
7. `docs/product/product-brief.md`
8. `docs/research/2026-07-10-dual-mode-research-addendum.md`
9. `docs/research/2026-07-10-initial-research.md`
10. `docs/research/2026-07-10-mutual-aid-coordination-research.md`
11. `docs/research/2026-07-10-willow-implementation-audit.md`
12. `docs/research/2026-07-11-app-ecosystem-bundled-apps-research.md`
13. `docs/research/2026-07-11-disaster-riot-mutual-aid-evidence-research.md`
14. `docs/research/2026-07-11-hybrid-gossip-backhaul-research.md`
15. `docs/research/2026-07-11-shutdown-resistant-distribution-research.md`
16. `docs/superpowers/plans/2026-07-10-riot-phase0a-public-kernel.md`
17. `docs/superpowers/plans/2026-07-10-riot-prototype.md`
18. `docs/superpowers/plans/2026-07-11-app-directory.md`
19. `docs/superpowers/plans/2026-07-11-conference-gateway-signature-verification.md`
20. `docs/superpowers/plans/2026-07-11-riot-conference-native-demo.md`
21. `docs/superpowers/plans/2026-07-11-signed-js-apps-core-platform.md`
22. `docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`
23. `docs/superpowers/specs/2026-07-10-riot-evidence-sprint-design.md`
24. `docs/superpowers/specs/2026-07-11-app-directory-design.md`
25. `docs/superpowers/specs/2026-07-11-conference-gateway-signature-verification-design.md`
26. `docs/superpowers/specs/2026-07-11-full-meadowcap-management-design.md`
27. `docs/superpowers/specs/2026-07-11-js-apps-runtime-ios-design.md`
28. `docs/superpowers/specs/2026-07-11-nearby-transport-design.md`
29. `docs/superpowers/specs/2026-07-11-riot-conference-native-demo-design.md`
30. `docs/superpowers/specs/2026-07-11-signed-js-apps-design.md`
31. `docs/superpowers/specs/2026-07-11-willow-visual-documentation-design.md`

Historical research and decision reports receive the same complete orientation;
their original findings remain intact beneath it. A visually distinct
`Current protocol context (added YYYY-MM-DD)` heading separates newly added
status framing from the dated historical record.

A newly added document is considered materially Willow-bearing when its parsed
prose, headings, link labels, or destinations either:

- contains a direct `willowprotocol.org` specification link; or
- contains at least one case-insensitive occurrence of `Willow`,
  `Meadowcap`, `Drop Format`, `Willow Transfer Protocol`, `WTP`, or
  `Confidential Sync`.

Fenced code, inline code, HTML comments, generated `.clearance-*` directories,
and the canonical primer itself do not count. Such a document must appear in
the coverage manifest or in a small explicit exemption list with owner,
rationale, and review date. Incidental references do not force a full primer,
but exemptions cannot be silent.

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

Additional figures are blocking when declared in `coverage.json`. The exact
initial mappings, beyond the three primer assets, are:

| Extra asset IDs | Documents |
| --- | --- |
| all nine non-primer catalog assets | `docs/architecture/willow-architecture.md` |
| `data-model-paths`, `data-model-overwrite`, `data-model-prefix-pruning` | the three `phase0a-wu*-report.md` files; `2026-07-10-willow-implementation-audit.md`; `2026-07-10-riot-phase0a-public-kernel.md`; `2026-07-10-riot-evidence-sprint-design.md` |
| `data-model-namespaces`, `meadowcap-capability-verification`, `meadowcap-communal-namespace`, `meadowcap-owned-namespace` | `2026-07-10-dual-mode-research-addendum.md`; `2026-07-10-riot-dual-mode-design.md`; both app-directory spec/plan files; both signed-JS-apps spec/plan files; `2026-07-11-full-meadowcap-management-design.md`; both conference-gateway-signature-verification spec/plan files |
| `drop-improvised-carriers`, `confidential-sync-selective-exchange` | `docs/decisions/riot-conference-sync.md`; `2026-07-10-initial-research.md`; `2026-07-11-hybrid-gossip-backhaul-research.md`; `2026-07-11-shutdown-resistant-distribution-research.md`; `2026-07-11-nearby-transport-design.md`; both conference-native-demo spec/plan files; `2026-07-10-riot-prototype.md` |
| none | README; product brief; mutual-aid research; disaster/mutual-aid evidence research; app-ecosystem research; JS-runtime-iOS design; this visual-documentation design |

Paths in the committed manifest are the full repository-relative paths from the
31-item scope list; basenames above are unambiguous shorthand only in this
human-readable table. A required extra figure must occur exactly once with its
catalog alt text, local source attribution, and a document-specific caption.

## Asset Catalog and Licensing

Exact upstream files are vendored under `docs/assets/willow/` using semantic,
stable filenames. Rabble explicitly confirmed on 2026-07-11 that the Willow
website illustrations are distributed under the same open-source terms as the
code. Before asset bytes land, that confirmation is recorded in
`docs/assets/willow/LICENSE-EVIDENCE.md`; if the confirmer is not authorized to
license the artwork, implementation remains blocked until a Willow/worm-blossom
copyright holder supplies written confirmation. The initial catalog contains
twelve illustrations:

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
- immutable upstream content identifier from the hashed asset filename;
- upstream repository, full source commit, and repository-relative source path
  when a public source repository exists, otherwise explicit `null` values plus
  the content-addressed website path;
- acquisition date and reviewer;
- byte length, decoded width, decoded height, and media type;
- license expression `MIT OR Apache-2.0`;
- attribution display text `Willow / worm-blossom`, separately from the
  copyright-holder evidence in `LICENSE-EVIDENCE.md`.

Its exact top level is `{ "version": 1, "assets": [...] }`. Each asset record
has exactly these keys: `id`, `local_path`, `upstream_url`, `sha256`,
`alt_text`, `source_spec_url`, `source_spec_status`,
`upstream_content_id`, `upstream_repository`, `upstream_commit`,
`upstream_source_path`, `acquired_on`, `reviewed_by`, `byte_length`, `width`,
`height`, `media_type`, `license`, and `attribution`. Asset IDs and paths are
unique and sorted by `id`; `sha256` is exactly 64 lowercase hexadecimal
characters; `media_type` is exactly `image/png`; `license` is exactly
`MIT OR Apache-2.0`; `local_path` is a normalized path under
`docs/assets/willow/`. Repository, commit, and source path are either all
non-null or all null. When null, `upstream_content_id` and the content-addressed
upstream URL are still required.

`docs/assets/willow/ATTRIBUTION.md`, `LICENSE-MIT`, and `LICENSE-APACHE` ship
beside the assets. Documentation captions use concise attribution and link to
the full local attribution file. Asset bytes are copied exactly; no destructive
optimization, metadata rewrite, recoloring, cropping, or AI alteration occurs.

Only static PNG is accepted. Each file must have a `.png` extension and PNG
magic bytes, decode successfully with a bounded PNG decoder, be at most 5 MiB,
at most 8192 pixels in either dimension, and at most 40 megapixels. APNG
animation chunks, trailing polyglot content, embedded active content, SVG,
HTML, external references, and MIME/extension mismatch are rejected. The
validator never follows symlinks and verifies canonical containment under
`docs/assets/willow/` before opening a path.

Manifest hashes detect repository drift; they do not authenticate an upstream
publisher because an attacker could change an asset and hash together. Upstream
content IDs, source revisions when available, license evidence, and mandatory
human review form the provenance boundary. Asset, license, protocol-registry,
coverage, or exemption changes require explicit repository-maintainer and
product-owner review; CI alone never labels them authentic or current.

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

The existing Rust `xtask` validation path gains
`crates/xtask/src/documentation.rs`. It exposes a pure
`validate_documentation(root: &Path) -> Vec<String>` boundary; command wiring
stays in `main.rs`. Traversal and diagnostics are deterministically sorted,
each document is read once, all errors are aggregated, and symlinks are never
followed.

Markdown is parsed with a pinned CommonMark/GFM-aware parser. Inline and
reference-style Markdown images are supported and validated. Raw HTML images,
`picture`/`source`, data URIs, protocol-relative paths, absolute filesystem
paths, `file:` URLs, encoded schemes, and remote image schemes are forbidden.
Ordinary `https:` links to official protocol sources remain allowed. Every
image destination must resolve, after lexical normalization and canonical
containment checks, to a declared regular PNG under `docs/assets/willow/`.

The source and tests follow mandatory TDD and the repository coverage contract.
`.coverage-thresholds.json` is the sole configured gate and currently names
`cargo tarpaulin --fail-under 100`. This design no longer claims that that
command independently reports lines, branches, functions, and statements: the
declared four-metric threshold and the Rust enforcement command are a
pre-existing mismatch. Before implementation completion, the project owner must
either approve a verified Rust coverage command that reports and gates all four
declared metrics and update `.coverage-thresholds.json`, or explicitly revise
the threshold schema to the metrics the configured Rust tool actually measures.
The validator work cannot be marked complete while the source-of-truth file
overstates its enforcement.

The RED tests prove that validation rejects:

- a covered document with no primer marker;
- zero, multiple, nested, reversed, or unpaired primer markers, ignoring marker
  text inside parsed code;
- any mutation to canonical thesis, headings, step order, asset identities, alt
  text, captions, source/status line, attribution, or primer version after
  whitespace and relative-path normalization;
- a covered document with no boundary paragraph;
- a materially Willow-bearing document omitted from both coverage and
  exemptions;
- an exemption with no rationale;
- malformed JSON, unknown fields, duplicate keys, unsorted arrays, duplicate
  records, coverage/exemption overlap, nonexistent paths, out-of-root paths,
  future review dates, and missing registries;
- a missing local image;
- an asset whose SHA-256 differs from the manifest;
- traversal, absolute paths, symlinks, normalization collisions, NUL/control
  characters, backslashes, and paths that escape repository or asset roots;
- remote, protocol-relative, data, file, HTML, or encoded-scheme image sources;
- an image occurrence with missing or altered alt text;
- an asset with missing source, attribution, or license metadata;
- extension/magic mismatch, malformed PNG, APNG, trailing polyglot bytes,
  oversize bytes/dimensions/pixels, or a non-PNG asset;
- a protocol declaration inconsistent with coverage or the reviewed local
  registry;
- a canonical status/source line inconsistent with declared protocol IDs;
- a missing direct official source link for a declared protocol dependency;
- a required additional asset missing, duplicated, remotely sourced, or used
  without its exact alt text and document-specific caption.

The GREEN implementation reads only repository files and performs no network
access. It emits a file path and actionable reason for every failure. It states
that protocol-status checks are consistency checks against reviewed local
evidence, not proof of current upstream state. Network link checking remains a
deliberate manual/update task so transient upstream availability cannot make CI
flaky.

Tests use collision-safe temporary repository fixtures that are automatically
cleaned. Unit fixtures cover parser, path, manifest, PNG, primer-normalization,
registry, and diagnostic behavior without depending on the live repository.
A separate integration test validates the committed repository. The RED evidence
is recorded by running each new focused test before implementation and observing
the expected contract failure. GREEN requires those tests plus the repository
integration test. REFACTOR extracts typed manifest parsing, path containment,
Markdown normalization, and PNG inspection helpers, then reruns the same focused
and full gates with no behavior change.

Positive tests cover canonical primer equality, stable ordering, a complete
document, a justified incidental-reference exemption, all vendored assets, and
the committed repository coverage manifest. The full quality gate uses the
command recorded in `.coverage-thresholds.json` plus:

```text
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo xtask validate-contracts
```

The documentation validator is invoked by `validate-contracts`, so it is a
blocking completion and PR gate rather than an optional lint.

## Update Workflow

When upstream artwork or protocol status changes:

1. fetch the exact official asset or specification metadata only from a public,
   unauthenticated source with no userinfo, query token, fragment secret,
   cookie, or authorization header;
2. review the upstream change rather than accepting it mechanically;
3. update the vendored file, immutable provenance, hash, alt text, status,
   attribution, acquisition record, and license evidence together;
4. update affected boundary paragraphs if semantics or maturity changed;
5. run the full validation and coverage gates;
6. review the rendered README, canonical architecture document, one design
   spec, one research note, and one implementation plan in both GitHub themes.

The validator never silently downloads or rewrites assets.
Temporary fetch files remain outside the repository and secret-bearing URLs or
fetch artifacts are rejected from manifests.

## Accessibility

- Original upstream alt text is preserved verbatim in the asset manifest.
- Every Markdown use supplies meaningful alt text; decorative empty alt text is
  not permitted for these explanatory figures.
- Captions explain the concept without requiring readers to distinguish colors.
- The vertical three-step layout must require no horizontal navigation at 320
  CSS pixels or 200% zoom.
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

**License misunderstanding.** Software licensing does not automatically cover
website art. The product-owner confirmation is recorded, authority to license is
verified before vendoring, and asset landing remains blocked without adequate
written evidence.

**Historical-document distortion.** Adding orientation must not rewrite the
evidentiary record. Existing research findings, dates, and decisions remain
unchanged except where a demonstrably incorrect link or current-status label is
being corrected explicitly.

## Definition of Done

- The twelve upstream PNG illustrations are vendored byte-for-byte with bounded
  format validation, immutable-source provenance, manifest, hashes, alt text,
  attribution, adequate asset-specific license evidence, and both license texts.
- `willow-architecture.md` contains the canonical security → movement →
  convergence explanation and direct official links.
- README and every in-scope Willow-bearing technical document contain the full
  visual primer and a tailored boundary paragraph.
- Relevant documents include additional concept-specific figures and captions.
- Protocol maturity and Riot implementation status are explicit and accurate.
- The documentation validator is built through recorded RED-GREEN-REFACTOR
  cycles, runs offline, is included in `validate-contracts`, and passes the
  reconciled command in `.coverage-thresholds.json` without overstating which
  metrics that command measures.
- Representative pages are visually reviewed in GitHub light and dark themes,
  at 320 CSS pixels, and at 200% zoom, and the five-reader acceptance study
  passes within seven days.
- No unrelated source, application UI, protocol behavior, or historical
  conclusion changes.
