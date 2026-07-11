# Willow Visual Docs and Marketing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every Willow-bearing Riot technical document independently explain Authority → Movement → Convergence, add the same official Willow visual story to the static marketing site, and republish the existing Workers deployment.

**Architecture:** Vendor exact upstream PNGs with local provenance, then use a typed Rust `xtask` library to validate canonical primer blocks, per-document boundaries, source/status declarations, asset integrity, and the byte-identical marketing source/deploy copies. The documentation corpus remains ordinary Markdown; the marketing site remains static HTML/CSS with no runtime request to Willow. Deployment is a separate final work unit and targets only the existing `riot-protest-net-marketing.protestnet.workers.dev` worker.

**Tech Stack:** Rust 2021, `pulldown-cmark`, `png`, `unicode-normalization`, `percent-encoding`, JSON manifests, GitHub Markdown, static HTML/CSS, Playwright, Cloudflare Wrangler.

**Governing design:** `docs/superpowers/specs/2026-07-11-willow-visual-documentation-design.md`

---

## File Structure

- `docs/assets/willow/`: canonical Willow PNGs, license evidence, attribution, protocol/asset/coverage/primer registries.
- `crates/xtask/src/lib.rs`: importable test boundary for repository validators.
- `crates/xtask/src/documentation/`: typed JSON parsing, Markdown normalization, path containment, PNG inspection, corpus and marketing validation.
- `crates/xtask/tests/documentation_validation.rs`: collision-safe fixture repositories and committed-repository integration coverage.
- Every repository Markdown file outside generated/vendor/cache directories: material-Willow discovery, canonical primer or explicit exemption, local boundary, direct official sources, and targeted figures.
- `marketing/assets/willow/`: source-side copies of the three marketing figures.
- `marketing/public/assets/willow/`: Wrangler-deployed copies of the same figures.
- `marketing/index.html`, `marketing/public/index.html`: byte-identical visual explainer and existing site.
- `marketing/README.md`: accurate live deployment and republish instructions.
- `COLLABORATION.md`: implementation and deployed-version handoff.

## Task 1: Scaffold the Documentation Validator

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/xtask/Cargo.toml`
- Create: `crates/xtask/src/lib.rs`

- [ ] **Step 1: Pin the exact tooling inputs**

Add workspace pins:

```toml
pulldown-cmark = "=0.13.4"
png = "=0.18.1"
unicode-normalization = "=0.1.25"
percent-encoding = "=2.3.2"
```

Expose them from `crates/xtask/Cargo.toml` with `{ workspace = true }`.

- [ ] **Step 2: Create the importable library boundary**

Create `crates/xtask/src/lib.rs`:

```rust
//! Shared, importable validation library for `cargo xtask` commands.
```

Use only `pub mod documentation;` after Task 2 creates that module. At this task boundary, `lib.rs` may be empty except for crate-level documentation so it compiles independently.

- [ ] **Step 3: Verify the scaffold and commit**

```bash
cargo fmt --all -- --check
cargo test -p xtask
git add Cargo.toml Cargo.lock crates/xtask/Cargo.toml crates/xtask/src/lib.rs
git commit -m "refactor(xtask): add validator library boundary"
```

## Task 2: Vendor Willow Artwork With Provenance

**Files:**
- Create: `docs/assets/willow/data-model-{paths,overwrite,prefix-pruning,subspaces,namespaces}.png`
- Create: `docs/assets/willow/meadowcap-{capability-verification,communal-namespace,owned-namespace,capability-ticket}.png`
- Create: `docs/assets/willow/drop-{improvised-carriers,adhoc-transport-chain}.png`
- Create: `docs/assets/willow/confidential-sync-selective-exchange.png`
- Create: `docs/assets/willow/manifest.json`
- Create: `docs/assets/willow/protocols.json`
- Create: `docs/assets/willow/primer.json`
- Create: `docs/assets/willow/coverage.json`
- Create: `docs/assets/willow/ATTRIBUTION.md`
- Create: `docs/assets/willow/LICENSE-EVIDENCE.md`
- Create: `docs/assets/willow/LICENSE-MIT`
- Create: `docs/assets/willow/LICENSE-APACHE`
- Modify: `crates/xtask/src/lib.rs`
- Create: `crates/xtask/src/documentation/mod.rs`
- Create: `crates/xtask/src/documentation/model.rs`
- Create: `crates/xtask/src/documentation/png.rs`
- Create: `crates/xtask/tests/documentation_validation.rs`

- [ ] **Step 1: Write failing manifest and PNG tests**

Tests cover missing assets, wrong hash, wrong magic bytes, APNG, dimensions over 8192, more than 40 megapixels, paths outside `docs/assets/willow`, symlinks, duplicate IDs, unknown protocol IDs, future dates, and noncanonical URLs.
In the same test file, define `FixtureRepo` on `tempfile::TempDir` with
`new`, `write_manifest_asset`, and `validate` helpers so every test owns an
isolated automatically removed repository.

```rust
#[test]
fn rejects_manifest_asset_outside_catalog_root() {
    let repo = FixtureRepo::new();
    repo.write_manifest_asset("../outside.png");
    assert_errors(repo.validate(), &["asset path escapes docs/assets/willow"]);
}
```

- [ ] **Step 2: Run the RED cases**

Run: `cargo test -p xtask --test documentation_validation asset_`

Expected: FAIL because asset validation is absent.

- [ ] **Step 3: Acquire exact upstream PNG bytes**

Fetch these exact content-addressed URLs and save with the matching IDs:

```text
data-model-paths = https://willowprotocol.org/assets/data_model/92890b84daa47f87a25b6f473ce1284b6412a774c4516acbdd1167158329b7bb.png
data-model-overwrite = https://willowprotocol.org/assets/data_model/7e9f5462329d9ddb9f6d9666251c35fe83ff281256b397e7455eff619a1437e8.png
data-model-prefix-pruning = https://willowprotocol.org/assets/data_model/5425b64b48c2a1f6082a633452dbd03639ae028a30504c1a2539429749b08f64.png
data-model-subspaces = https://willowprotocol.org/assets/data_model/1aa5504899909482194d395cdcc0bfdb1cb51f9b09c7d834ca2f7fc538b4d751.png
data-model-namespaces = https://willowprotocol.org/assets/data_model/7a2e8b02247a06101594b16f3994cf851f5a54be08548430a1c7e1eb125c23e9.png
meadowcap-capability-verification = https://willowprotocol.org/assets/meadowcap/4a485c36ef828eba0a1cdf4f48592adfb9a3cebc4b7103602e0579eb5655299f.png
meadowcap-communal-namespace = https://willowprotocol.org/assets/meadowcap/92f3a2f3b3072222e2cfdab3bdc4e2a3a74a45cad96d1d285ad107e089d80387.png
meadowcap-owned-namespace = https://willowprotocol.org/assets/meadowcap/3eee8901364f05f3aab844ffe8d0bca3eb299609f03cd001c2de86106a9a1f03.png
meadowcap-capability-ticket = https://willowprotocol.org/assets/meadowcap/1b2d22be5a6f08bf437a71f6dfe18683ac6016fe9580b34fde3afd0b3d03cb0f.png
drop-improvised-carriers = https://willowprotocol.org/assets/dropformat/7af46ef4eb9414290173ebef72b6b9b173172874789ec9aa632ff88d77c6bfe5.png
drop-adhoc-transport-chain = https://willowprotocol.org/assets/dropformat/02718468ec241a3adc2175ddb3ff04d93e1d1f59deb0b2c840da5fd01fa80246.png
confidential-sync-selective-exchange = https://willowprotocol.org/assets/sync/711342d0ed3a4b8ac990f1eeb67a108798e001e3f101254cf7b684c64b7c473a.png
```

Do not transform the bytes. Record SHA-256, byte length, decoded dimensions, verbatim upstream alt text, content-addressed URL, acquisition date, and reviewer.

- [ ] **Step 4: Record licensing evidence**

`LICENSE-EVIDENCE.md` records Rabble's 2026-07-11 confirmation, the exact twelve-asset scope, `MIT OR Apache-2.0`, confirmation authority, durable reference, and reviewer. If authority to license cannot be recorded, stop before redistribution and report the blocker.

- [ ] **Step 5: Create the typed registries**

Populate `protocols.json` with the eight IDs and official URLs from the design. Populate `primer.json` with the exact Authority → Movement → Convergence copy. Populate `coverage.json` with the initial 36-document list, sorted protocol IDs, exact extra-figure records and captions, plus the dated `COLLABORATION.md` coordination-ledger exemption.

- [ ] **Step 6: Implement bounded PNG and manifest validation**

Add `pub mod documentation;` to `crates/xtask/src/lib.rs`. Implement the initial `documentation::{model,png}` modules with `png::Decoder` limits, sequential decode, cumulative limits, safe ASCII semantic filenames, canonical path containment, and no symlink following. Hashes are drift checks; diagnostics must not call them publisher authentication.

- [ ] **Step 7: Run GREEN tests and commit**

```bash
cargo test -p xtask --test documentation_validation asset_
cargo xtask validate-contracts
git add docs/assets/willow crates/xtask/src/documentation crates/xtask/tests/documentation_validation.rs Cargo.toml Cargo.lock crates/xtask/Cargo.toml
git commit -m "docs: vendor attributed Willow protocol artwork"
```

## Task 3: Build the Canonical Markdown Validator and Synchronizer

**Files:**
- Modify: `crates/xtask/src/documentation/mod.rs`
- Modify: `crates/xtask/src/documentation/model.rs`
- Create: `crates/xtask/src/documentation/markdown.rs`
- Create: `crates/xtask/src/documentation/paths.rs`
- Modify: `crates/xtask/src/documentation/png.rs`
- Modify: `crates/xtask/src/main.rs`
- Modify: `crates/xtask/tests/documentation_validation.rs`

- [ ] **Step 1: Write failing Markdown contract tests**

Cover paired markers, exact primer normalization, semantic step order, exact alt text, skip link, one H1, metadata insertion, four-item boundary, protocol equality, additional protocols, targeted figure adjacency, raw-HTML bypasses, percent/double encoding, README/one-level/two-level relative paths, historical framing, and unrelated non-Willow images.

```rust
#[test]
fn accepts_two_level_relative_willow_asset_path() {
    let repo = FixtureRepo::complete();
    repo.write_covered_doc("docs/superpowers/specs/example.md", "../../assets/willow/data-model-subspaces.png");
    assert!(repo.validate().is_empty());
}
```

- [ ] **Step 2: Run representative RED tests**

```bash
cargo test -p xtask --test documentation_validation accepts_two_level_relative_willow_asset_path -- --exact
cargo test -p xtask --test documentation_validation rejects_undeclared_official_protocol_link -- --exact
```

Expected: FAIL because Markdown normalization and dependency equality are absent.

- [ ] **Step 3: Implement pure validation phases**

Implement, in order: bounded file discovery; strict JSON schema parsing; semantic cross-registry checks; CommonMark/GFM parsing; canonical primer/boundary/figure comparison; URL/path normalization; PNG/catalog inspection; sorted diagnostics. `validate_documentation(root)` performs no writes or network calls.

- [ ] **Step 4: Add an explicit synchronizer command**

Add `cargo xtask sync-willow-docs --check` and `--write`. `--write` inserts or replaces only paired generated primer/source/figure blocks and the fixed skip/content headings; it never rewrites text outside markers. Boundary text and historical context must already exist in `coverage.json` and are emitted from that reviewed data.

- [ ] **Step 5: Run GREEN, REFACTOR, and full xtask tests**

```bash
cargo test -p xtask --test documentation_validation
cargo test -p xtask
cargo clippy -p xtask --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: all PASS.

- [ ] **Step 6: Commit validator infrastructure**

```bash
git add crates/xtask Cargo.toml Cargo.lock
git commit -m "feat(xtask): validate Willow documentation contracts"
```

## Task 4: Migrate the Complete Technical Corpus

**Files:**
- Modify: the initial 36 documents listed in the governing design, any additional Willow-bearing documents found by the repository-wide preflight rescan, and `coverage.json`

- [ ] **Step 1: Prove the committed corpus fails before synchronization**

First rerun the governing design's material-Willow scan across every tracked repository Markdown file outside its explicit generated/vendor/cache exclusions. Add every newly matched document to `coverage.json` or add a dated, owned, justified exemption. Then run: `cargo xtask sync-willow-docs --check`.

Expected: FAIL listing all covered documents missing the versioned primer.

- [ ] **Step 2: Review every tailored boundary before generation**

For each coverage entry, fill nonempty exact text for:

```text
Willow defines
Riot defines
Implemented today
Proposed or gated
```

Historical reports also receive the dated current-context text without modifying their original record. Protocol lists must equal all recognized official Willow spec links in the document.

- [ ] **Step 3: Generate only marked blocks**

Run: `cargo xtask sync-willow-docs --write`

Expected: all currently covered documents updated; no unlisted file changed.

- [ ] **Step 4: Make `willow-architecture.md` the complete canonical explainer**

Expand its body with all twelve figures and direct Data Model, Meadowcap, Willow'25, encoding, Drop Format, WTP, Confidential Sync, and Encrypted Willow links. Preserve the existing Riot-specific architecture and research addenda.

- [ ] **Step 5: Review generated diffs for historical integrity**

Run:

```bash
git diff -- README.md docs/architecture docs/product docs/research docs/decisions docs/superpowers/specs docs/superpowers/plans
cargo xtask sync-willow-docs --check
cargo xtask validate-contracts
```

Expected: only prologue/current-context/targeted-figure additions plus explicit link/status corrections; both commands PASS.

- [ ] **Step 6: Commit the corpus migration**

```bash
git add README.md docs/architecture docs/product docs/research docs/decisions docs/superpowers/specs docs/superpowers/plans docs/assets/willow/coverage.json
git commit -m "docs: explain Riot through Willow's visual model"
```

## Task 5: Add the Visual Story to the Marketing Site

**Files:**
- Modify: `marketing/index.html`
- Modify: `marketing/public/index.html`
- Modify: `marketing/README.md`
- Create: `marketing/assets/willow/{meadowcap-capability-ticket,drop-adhoc-transport-chain,data-model-subspaces}.png`
- Create: `marketing/assets/willow/ATTRIBUTION.md`
- Create: `marketing/public/assets/willow/{meadowcap-capability-ticket,drop-adhoc-transport-chain,data-model-subspaces}.png`
- Create: `marketing/public/assets/willow/ATTRIBUTION.md`
- Extend: `crates/xtask/tests/documentation_validation.rs`

- [ ] **Step 1: Write failing marketing contract tests**

Assert source/deploy HTML equality, exact three asset hashes matching the docs catalog, one `#willow` section before `.techdrop`, direct official links, local images with exact alt text, maturity copy, attribution, and no external image requests.

- [ ] **Step 2: Run the RED test**

Run: `cargo test -p xtask --test documentation_validation marketing_site_has_willow_visual_story -- --exact`

Expected: FAIL because the section does not exist.

- [ ] **Step 3: Add responsive white-card CSS**

Add `.willow-grid`, `.willow-card`, `.willow-card img`, `.willow-source`, and the narrow breakpoint. Cards use `#fff`, `2px solid var(--ink)`, square corners, no gradient, and stack to one column below 760px.

- [ ] **Step 4: Add the audience-facing section**

Insert `<section id="willow">` after `.steps` and before `.techdrop`. Copy uses:

```text
Authority — Human-controlled keys and capabilities say who may read or write.
Movement — Signed information can cross files, USB, messaging, and local wireless.
Convergence — Devices merge what they learned when they meet again.
```

Link each card to the relevant official Willow specification and state that Data Model/Meadowcap are final while Drop Format is a proposal and WTP is a sketch. State clearly which parts Riot implements today.

- [ ] **Step 5: Keep source and deploy trees byte-identical**

Apply the same HTML to both files and copy identical assets to both asset trees. Update `marketing/README.md` to say the Workers deployment is live, document `npx wrangler deploy`, and distinguish it from custom `riot.protest.net`.

- [ ] **Step 6: Run GREEN tests and commit**

```bash
cmp marketing/index.html marketing/public/index.html
cargo test -p xtask --test documentation_validation marketing_
cargo xtask validate-contracts
git add marketing crates/xtask/tests/documentation_validation.rs
git commit -m "feat(marketing): show how Riot uses Willow"
```

## Task 6: Render, Verify, and Iterate

**Files:**
- Create temporary screenshots only under `/tmp/riot-willow-visual-review/`
- Modify source files only if review finds a defect

- [ ] **Step 1: Start a local static server and verify Playwright**

```bash
npx playwright --version
python3 -m http.server 18081 --directory marketing/public
```

- [ ] **Step 2: Capture desktop, mobile, light, and dark screenshots**

```bash
npx playwright screenshot --browser chromium --viewport-size "1440,1000" --wait-for-timeout 1500 http://localhost:18081 /tmp/riot-willow-visual-review/desktop.png
npx playwright screenshot --browser chromium --viewport-size "390,844" --wait-for-timeout 1500 http://localhost:18081 /tmp/riot-willow-visual-review/mobile.png
npx playwright screenshot --browser chromium --color-scheme dark --viewport-size "1440,1000" --wait-for-timeout 1500 http://localhost:18081 /tmp/riot-willow-visual-review/desktop-dark.png
npx playwright screenshot --browser chromium --color-scheme dark --viewport-size "390,844" --wait-for-timeout 1500 http://localhost:18081 /tmp/riot-willow-visual-review/mobile-dark.png
npx playwright screenshot --browser chromium --full-page --viewport-size "1440,1000" --wait-for-timeout 1500 http://localhost:18081/#willow /tmp/riot-willow-visual-review/full-page.png
```

Inspect images for white-card treatment, no overflow, correct order, readable captions, visible attribution, and no broken assets.

- [ ] **Step 3: Review representative Markdown renders**

Render README, canonical architecture, one spec, one plan, and one historical report in GitHub light/dark themes at 320 CSS pixels and 200% zoom. Verify keyboard skip links and VoiceOver order.

- [ ] **Step 4: Fix and recapture until clean**

Re-run only affected screenshots and validation after each fix. Do not accept horizontal scrolling or missing images.

- [ ] **Step 5: Run the five-reader acceptance study**

Within seven days, test the five representative readers and five representative documents defined in the governing design. Record participant categories, per-question results, completion times, expert skip-link times, keyboard/VoiceOver findings, and pass/fail in `docs/decisions/willow-visual-reader-study.md`. At least four readers must answer all five questions correctly within five minutes; every expert must use the skip link within 30 seconds; any critical accessibility failure blocks acceptance.

Add that results file to `coverage.json` as a dated, owned evaluation-artifact exemption, rerun `cargo xtask sync-willow-docs --check` and `cargo xtask validate-contracts`, and commit the study plus updated coverage manifest.

- [ ] **Step 6: Run the full repository gate**

```bash
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo xtask validate-contracts
cargo tarpaulin --fail-under 100
```

Expected: every command passes. The Tarpaulin invocation is the exact command currently stored in `.coverage-thresholds.json`.

## Task 7: Publish the Existing Marketing Worker and Verify Live Bytes

**Files:**
- Modify: `COLLABORATION.md`

- [ ] **Step 1: Confirm account and target without changing state**

```bash
cd marketing
npx wrangler whoami
npx wrangler versions list
npx wrangler deployments list --json
```

Expected: authenticated account owns `riot-protest-net-marketing`. Parse the deployment with 100% traffic from `deployments list --json` and save that version ID as the rollback target; `versions list` alone is not treated as traffic evidence. Stop if account or worker differs.

- [ ] **Step 2: Deploy the verified `marketing/public` tree**

Run: `npx wrangler deploy`

Expected: Wrangler publishes a new version of `riot-protest-net-marketing` and prints its version ID and `https://riot-protest-net-marketing.protestnet.workers.dev` URL.

- [ ] **Step 3: Verify live HTML and assets**

Fetch the live HTML and three asset URLs, compare SHA-256 with the committed deploy tree, verify the `#willow` section and official links, and confirm no private content or write endpoint was introduced.

If verification fails, immediately restore 100% traffic to the saved prior version with `npx wrangler versions deploy <prior-version-id>@100% --yes`, then verify the prior HTML hash before reporting the failed rollout.

- [ ] **Step 4: Record handoff and commit**

Add a `COLLABORATION.md` row with commit, worker version, URL, HTML hash, asset hashes, test results, and custom-domain non-deployment note.

```bash
git add COLLABORATION.md
git commit -m "docs: record Willow marketing deployment"
```

## Final Acceptance

- [ ] Every technical document matched by the final repository-wide material-Willow scan contains one valid full visual primer and tailored boundary or a dated, owned exemption.
- [ ] The canonical architecture uses all relevant official Willow figures and direct specifications.
- [ ] Marketing source and deploy HTML are byte-identical and visually clean at desktop/mobile and light/dark.
- [ ] No doc or site depends on Willow-hosted images at render time.
- [ ] Asset provenance, licensing, hashes, alt text, and protocol maturity are explicit.
- [ ] Workspace tests, strict Clippy, formatting, contract validation, and the configured coverage gate pass.
- [ ] The five-reader study and keyboard/VoiceOver acceptance thresholds pass and are recorded.
- [ ] The Workers deployment is republished and live bytes match the committed `marketing/public` tree.
- [ ] `riot.protest.net` itself is not changed without the separate DNS/TLS and edge-policy approvals.
