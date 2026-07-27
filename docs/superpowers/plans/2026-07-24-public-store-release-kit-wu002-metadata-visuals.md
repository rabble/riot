# WU-002 Store Metadata, Configuration Validator, and Visual System Plan

> **Status (2026-07-27): APPROVED.** Three-reviewer plan gate passed after
> three rounds: feasibility (B1 renderer font mechanism, B2 schema-ID
> ordering, B3 sharp DI, B4 claim-carrier ordering — all fixed),
> completeness (10 gaps incl. approval-digest self-reference — all fixed),
> scope & alignment (nav/footer/sitemap scope, support-page truthfulness,
> master-plan file-list amendment — all fixed). Master plan WU-002 file
> lists amended to match. Ready for execution.

**Goal:** Generate deterministic, locally validated Apple and Google store
metadata from canonical sources; publish the missing public support and
accessibility pages; and build the cinematic-overlay visual system (icons,
draft compositions for the full six-frame/five-device matrix, and draft
provenance) that WU-005/WU-006 later bind to native candidate captures.

**Architecture:** New pure Node ES modules follow WU-000 conventions:
dependency-injected filesystem/clock/sha256 adapters, closed Ajv 2020-12
schemas registered in the fixed registry, canonical-JSON digests, deep-frozen
outputs, and tri-state gates. `metadata.mjs` validates canonical en-US listing
sources against exact platform field contracts and emits platform-shaped text
files plus digested field manifests. `configuration.mjs` evaluates native
project configuration snapshots (injected as data, never shelling out) and
truthfully reports today's known-incomplete state as `BLOCKED`.
`visual-model.mjs` encodes the 30-cell frame/device matrix and overlay
geometry rules as data; `visual-render.mjs` composes SVG overlays and base
images via an injected Sharp 0.35.3 adapter; `visual-validate.mjs` +
`image-inspector.mjs` mechanically verify dimensions, alpha, EXIF absence,
band geometry, contrast, prohibited tokens, and provenance completeness. All
generated output is byte-reproducible on a single pinned platform and
drift-checked.

**Tech stack:** Node 26 ESM, `node:test`, c8 (100% floor for
`scripts/release/**`), Ajv 8.20.0, Sharp 0.35.3 (sole new dependency,
`@img/sharp-darwin-arm64@0.35.3` + libvips 1.3.2 prebuilt), checked-in fonts
(`apps/ios/Riot/Resources/Fonts/*.ttf`, read-only), existing marketing
contract script.

## Allowed file scope

Metadata half:

- Create `release/source/apple/en-US.json`
- Create `release/source/google/en-US.json`
- Create `release/schemas/apple-metadata.schema.json`
- Create `release/schemas/google-metadata.schema.json`
- Create `scripts/release/metadata.mjs`
- Create `scripts/release/test/metadata.test.mjs`
- Generate `release/generated/apple/en-US/{name,subtitle,promotional-text,description,keywords,release-notes}.txt`
- Generate `release/generated/apple/en-US/manifest.json`
- Generate `release/generated/google/en-US/{title,short-description,full-description,release-notes}.txt`
- Generate `release/generated/google/en-US/manifest.json`
- Modify `release/source/product.json` (support-URL `evidenceState`
  `missing` → `current` only, so the `url.support` gate verifies the new
  page and flips to PASS)
- Modify `scripts/release/test/policy.test.mjs` (the three touch points
  pinning the support gate's current `missing` state)

Marketing pages (new pages + site-wide nav/footer/sitemap consistency):

- Create `marketing/support/index.html`
- Create `marketing/accessibility/index.html`
- Create `marketing/public/support/index.html`
- Create `marketing/public/accessibility/index.html`
- Modify `marketing/{index,about,community,guide,open-source,privacy,protocols,releases,why-riot}/index.html`
  — footer/topnav link additions and canonical-fact alignment only
- Modify `marketing/public/{index,about,community,guide,open-source,privacy,protocols,releases,why-riot}/index.html`
  — byte-identical mirror of the same edits
- Modify `marketing/public/sitemap.xml` (add the two routes)
- Modify `marketing/README.md` (routes list)
- Modify `scripts/marketing/protocol-page-contracts.mjs` (add the two routes
  to `allSitePaths` and required-content checks for the new pages; do not
  weaken existing checks)

Visual/configuration half:

- Create `release/source/visuals.json`
- Create `release/schemas/visuals.schema.json`
- Create `scripts/release/configuration.mjs`
- Create `scripts/release/visual-model.mjs`
- Create `scripts/release/visual-render.mjs`
- Create `scripts/release/visual-validate.mjs`
- Create `scripts/release/image-inspector.mjs`
- Create `scripts/release/render-environment.mjs` (fontconfig env setup,
  isolated for child-process coverage — see renderer mechanism)
- Create `scripts/release/test/{configuration,visual-model,visual-render,visual-validate,image-inspector,render-environment}.test.mjs`
- Generate `release/generated/visuals/draft/{iphone,ipad,mac,android-phone,android-tablet}/*.png`
- Generate `release/generated/visual-draft-provenance.json`
- Generate `release/generated/icons/macos/AppIcon.appiconset/**`
- Generate `release/generated/icons/android/{mipmap-mdpi,mipmap-hdpi,mipmap-xhdpi,mipmap-xxhdpi,mipmap-xxxhdpi}/ic_launcher.png`
- Generate `release/generated/icons/android/adaptive/{foreground,background}.png`
- Generate `release/generated/icons/android/play-icon-512.png`
- Generate `release/generated/visuals/google/feature-graphic-1024x500.png`
- Generate `release/generated/visuals/google/play-icon-512.png` (byte copy of
  `icons/android/play-icon-512.png`; single render source, two store-shaped
  output paths per the master plan)

Shared:

- Modify `scripts/release/schema.mjs` (register three new schema IDs in
  `EXPECTED_IDS` only — metadata IDs in Task 1, visuals ID in Task 2; never
  register an ID before its schema file exists in the same change)
- Modify `scripts/release/cli.mjs` (wire metadata/visual generation into
  `generate`; add metadata + configuration gates to `status`)
- Modify `scripts/release/test/cli.test.mjs`
- Modify `package.json` and `package-lock.json` (add `sharp@0.35.3`
  devDependency, nothing else)

Master-plan amendment (done alongside this plan): the WU-002 file list in
`2026-07-24-public-store-release-kit-master-plan.md` is extended with
`package.json`/`package-lock.json`, `scripts/release/schema.mjs`,
`scripts/marketing/protocol-page-contracts.mjs`, the test files above,
`release/source/product.json` + `scripts/release/test/policy.test.mjs`, the
site-wide marketing nav/footer/sitemap files, and
`scripts/release/render-environment.mjs`.

Explicitly out of scope: `release/toolchains.json` and its schema (Sharp
toolchain drift pinning is WU-004), all other WU-000 sources
(`policy.json`, `privacy.json`, `accessibility.json`, `claims.json`,
`review-instructions.json` — the review-contact/instructions and
content-rating worksheets remain WU-000-owned and are not rebuilt here),
native app projects (WU-005/WU-006), candidate/ledger machinery (WU-003),
new committed Playwright tooling (human review uses existing viewers), and
any policy-control remediation.

## Canonical metadata contract

### Field tables (exact limits, Unicode code points counted with `Array.from`)

Apple (`release/source/apple/en-US.json` → `release/generated/apple/en-US/`):

| Field | File | Limit | Required content rules |
| --- | --- | --- | --- |
| `name` | `name.txt` | ≤30 | exact `Riot` |
| `subtitle` | `subtitle.txt` | ≤30 | community/tools positioning, no superlatives |
| `promotionalText` | `promotional-text.txt` | ≤170 | contains the exact positioning sentence (below) |
| `description` | `description.txt` | ≤4000 | paragraph structure pinned below |
| `keywords` | `keywords.txt` | ≤100 | comma-separated, no competitor names, no store terms |
| `releaseNotes` | `release-notes.txt` | ≤4000 | 1.0 early-access notes, exact early-access sentence |

Google (`release/source/google/en-US.json` → `release/generated/google/en-US/`):

| Field | File | Limit | Required content rules |
| --- | --- | --- | --- |
| `title` | `title.txt` | ≤30 | exact `Riot` |
| `shortDescription` | `short-description.txt` | ≤80 | community/tools positioning |
| `fullDescription` | `full-description.txt` | ≤4000 | same factual claims as Apple, Google-shaped |
| `releaseNotes` | `release-notes.txt` | ≤500 | 1.0 early-access notes |

Both sources also carry validated non-emitted fields: `privacyUrl`,
`supportUrl`, `marketingUrl` (exact canonical URLs below), and category
recommendations — Apple `primaryCategory`/`secondaryCategory` from a pinned
App Store allowlist, Google `category` from a pinned Play allowlist. These
validate and appear in the manifest but emit no `*.txt` (stores collect them
in Console UI, not text files). Copyright/seller/developer fields are
explicitly deferred: they require authenticated account confirmation and are
recorded as `HUMAN ACTION` by the existing WU-000 account gates.

### Pinned sentences (verbatim in both platforms where field allows)

- Positioning: `Community-owned news and practical tools designed to stay useful locally when networks are unreliable.`
- Early access: `Version 1.0 is an early-access release.`
- Signature disclaimer: `A valid signature proves source and integrity, not truth.`
- Editorial disclaimer: `Editorial labels are community signals, not independent factual verification.`
- Pricing: `Riot is free, with no in-app purchases.` plus a worldwide
  availability statement.
- Canonical URLs: `https://riot.protest.net/privacy/`,
  `https://riot.protest.net/support/`, `https://riot.protest.net/releases/`.

### Nearby narrowing (1.0-time decision)

Nearby exchange is advertised as `experimental` and **for phones only** in
all 1.0 listing copy. The design permits iPad Nearby advertising after the
named iOS-pair gate; for 1.0 we deliberately narrow to phones until that
gate is recorded. This is a metadata-source choice, revisable later via
metadata-only remediation (new submission-package revision, binary
unchanged). No Mac Nearby claim anywhere (design-fixed).

### Claims whitelist (only these capability claims may appear)

1. `follow-newswire` — Follow community newswires.
2. `publish-signed` — Publish signed updates.
3. `read-signatures-labels` — Read signatures and community editorial labels.
4. `community-tools` — Carry community tools (checklists).
5. `nearby-exchange` — Exchange updates nearby (experimental; phones only).
6. `offline-copy` — Keep a local copy available offline.

Claim-carrier rule: every claim shown in any screenshot headline must also
appear in the platform `description`/`fullDescription` text (color/image
text is never the sole carrier of meaning). `metadata.mjs` enforces this
against the frame headlines declared in `release/source/visuals.json`.

### Prohibited vocabulary (validated mechanically, case-insensitive)

Rankings/superlatives (`best`, `fastest`, `leading`, `number one`, `#1`),
absolute privacy/anonymity claims (`anonymous`, `untraceable`, `fully
private`, `no logs`), price anchors (`$`, `sale`, `discount`), unverifiable
security claims (`military-grade`, `unhackable`, `secure against`), any URL
other than the three canonical URLs, and production/operational identifiers
(`npub1`, `nsec1`, `note1` bech32 prefixes).

### Cross-platform consistency

Apple and Google sources may differ in shape but must share: the positioning
sentence, both disclaimers, the early-access sentence, the pricing sentence,
the three URLs, and the claims whitelist. `metadata.mjs` enforces shared
substrings across both sources.

### Field manifest

Each platform emits `manifest.json`: canonical JSON
`{schemaVersion: 1, platform, locale: "en-US", urls: {privacy, support, marketing}, categories: {...}, fields: {<field>: {file, codePoints, limit, sha256}}}`
where `sha256` digests the exact emitted file bytes (UTF-8, single trailing
LF). The manifest's own canonical bytes are sha256-digested for
submission-package binding (WU-003/WU-009); that top-level digest is printed
by `generate` and re-derived by tests. Schemas: new closed schemas
`apple-metadata.v1` / `google-metadata.v1` registered in `EXPECTED_IDS`
during Task 1.

### Marketing pages

- `marketing/support/index.html`: publishes the public support/report
  channel and escalation path. Hard truthfulness constraints:
  (a) the operator contact is a real, human-confirmed address — the executor
  must ask the user for it before authoring and may not invent one;
  (b) SLA statements (report acknowledgement 24h, imminent-harm decision
  24h, objectionable-content decision 72h) are scoped explicitly to the
  public channel;
  (c) the page discloses that in-app reporting, filtering, and blocking
  controls are not yet shipped (early-access limitation) — it publishes the
  process honestly, never invented controls. The corresponding
  `policy.publicContact` gate stays BLOCKED until the policy workstream
  lands; only `url.support` flips to PASS via the `product.json`
  `evidenceState` change.
- `marketing/accessibility/index.html`: states the accessibility commitment
  and rehearsal scope per `release/source/accessibility.json` (device/task
  matrix), marked as early-access commitments, not audit results.
- Nav/footer/sitemap consistency: the marketing contract enforces that every
  page links every route. Adding `/support/` and `/accessibility/` therefore
  requires footer/topnav link additions on all nine existing pages plus
  mirrors and the two new `sitemap.xml` entries — link additions and
  canonical-fact alignment only, no page redesigns.
- All modified/new pages mirrored byte-identically under
  `marketing/public/<page>/`.
- `scripts/marketing/protocol-page-contracts.mjs` gains the two routes in
  `allSitePaths` and required-content assertions (SLA statements,
  early-access disclosure, accessibility commitment); existing checks
  unchanged.

## Configuration validator contract

`configuration.mjs` exports `evaluateConfiguration(snapshot)` where
`snapshot` is injected data (never live shell-outs): `{ios: {...}, macos: {...}, android: {...}}`.
It emits tri-state gates with the standard 7-field shape:

- `config.ios.version` / `config.macos.version`: PASS iff marketing version
  `1.0` with explicit (non-commit-count) build injection; today `0.1` →
  BLOCKED with recovery pointing at WU-005.
- `config.ios.bundleId` / `config.macos.bundleId`: PASS iff
  `net.protest.riot` (already true → PASS today).
- `config.android.applicationId`: PASS iff `net.protest.riot`; today
  `org.riot.evidence` → BLOCKED, recovery WU-006.
- `config.android.version`: PASS iff versionName `1.0` and explicit positive
  versionCode; today `0.1`/`1` → BLOCKED.
- `config.ios.icon` PASS (1024px present); `config.macos.icon` and
  `config.android.icons` BLOCKED today (no Mac iconset, no Android `res/`
  launcher tree).
- `config.ios.privacyManifest` / `config.macos.privacyManifest`: BLOCKED
  today (absent), recovery WU-005.
- `config.android.signing`: BLOCKED today (no release signing config),
  recovery WU-006; a debug-key or daemon/argument-secret snapshot must also
  fail closed (covered by fixture tests).

Tests drive the validator with fixture snapshots covering every row above
plus the design's failure rows (ad-hoc Mac candidate, Intel claim,
entitlement mismatch, unsigned Android handoff, debug key, daemon password
exposure, wrong IDs/versions). The current-repo snapshot is one explicit
fixture asserting today's exact BLOCKED set — the suite stays green because
BLOCKED is the asserted expectation.

Freshness guard: the checked-in current snapshot embeds the sha256 of every
native file it was derived from (pbxproj/Info.plist/build.gradle.kts/
manifest paths). `status` re-hashes those files and emits a loud
`config.snapshot-freshness` BLOCKED gate on any drift, so out-of-band native
edits (including WU-005/006) can never leave `status` silently stale; the
remediation is regenerating the snapshot, which those work units own.

## Visual system contract

### Matrix

Six frames mapped 1:1 to the WU-001 fixture narrative states (same order and
IDs). Five device classes with pinned pixel geometries:

| Device | Class dims (px) | Orientation |
| --- | --- | --- |
| `iphone` | 1320×2868 | portrait |
| `ipad` | 2752×2064 | landscape |
| `mac` | 2880×1800 | landscape |
| `android-phone` | 1080×2400 | portrait |
| `android-tablet` | 1920×1200 | landscape |

30 cells total. Frame 5 (`nearby-exchange`) selects the `nearby` variant for
`iphone` and `android-phone` only; `ipad`, `mac`, and `android-tablet` use
the `join` fallback variant (Join-by-reference copy) in this draft pass. The
model records the selected variant per cell in provenance.

### Overlay rules (validated mechanically)

- Base image + opaque headline band; band height ≤24% of image height
  (portrait) / ≤28% (landscape); band may bleed to side/bottom edges.
- Band color pair: `ink` `#17160f` on `paper` `#eae6da` (default) or inverse;
  pink/blue tokens accent-only.
- Headline ≤3 lines, ≤42 code points; Anton for headlines, Space Mono for
  structural labels, Work Sans for body (variable font: pin the default
  instance weight explicitly in the model).
- Text safe inset ≥5% of image dims on every edge; headline/band contrast
  ≥4.5:1 measured from rendered pixels by `image-inspector.mjs`.
- Cap-height rule enforced at **layout time** from TTF metrics: a small
  pure-JS OS/2-table reader in `visual-model.mjs` extracts `sCapHeight` and
  `unitsPerEm` from each checked-in font (unit-tested against the real
  TTFs), and the model requires `fontSize × sCapHeight/unitsPerEm` ≥56 px on
  phone geometries and ≥72 px on tablet/Mac. No glyph-pixel archaeology.
- 320-px thumbnail mechanical check: after downscaling to 320 px width, the
  rendered headline cap-height must remain ≥14 px and the band must still
  cover its declared inset-corrected region (asserted from the model's
  geometry, not pixels). The legibility judgment itself stays a recorded
  human-review item.
- Per-artifact alpha table: screenshots (30 drafts + future finals) — RGB,
  no alpha; macOS iconset PNGs — opaque; Android legacy launcher PNGs —
  opaque; Android adaptive `foreground.png` — RGBA with transparent margin
  outside the glyph safe zone (required by the platform);
  `background.png` — opaque; Play 512 icon — opaque; feature graphic —
  opaque.
- Output: PNG, 8-bit, EXIF/metadata stripped (Sharp pipeline without
  `withMetadata`). Byte-reproducible given the pinned lockfile
  `@img/sharp-libvips-*@1.3.2` **on one platform** (darwin-arm64 is the
  pinned generation platform; cross-platform font rasterization differs, so
  the drift check runs on the pinned platform and provenance records
  `platform: "darwin-arm64"`).
- Prohibited-token scan: `visual-validate.mjs` rejects any production or
  operational identifier/pattern in `visuals.json` strings and provenance —
  the WU-001 prohibited classes (person/name, email/phone,
  location/address/coordinates, notification/device tokens, private data,
  production URLs/hostnames/IPs, `npub1`/`nsec1`/`note1`).

### Renderer mechanism (corrected after feasibility review)

- librsvg resolves fonts through fontconfig only; `@font-face` in SVG is
  unsupported by the prebuilt Sharp/libvips. Therefore
  `render-environment.mjs` generates a hermetic `fonts.conf` in a temp dir
  with `<dir>` entries pointing at `apps/ios/Riot/Resources/Fonts`, and the
  CLI sets `FONTCONFIG_PATH`, `PANGOCAIRO_BACKEND=fontconfig`, and
  `XDG_CACHE_HOME` (fontconfig cache hermeticity) **before** dynamically
  importing `visual-render.mjs`. SVG uses plain `font-family` attributes.
- `render-environment.mjs` is the only module that mutates `process.env`;
  it is covered via `node:child_process` spawn tests (env is process-global
  and order-sensitive, so env assertions run in spawned processes).
- **Sharp is a dependency-injected adapter** in `visual-render.mjs` and
  `image-inspector.mjs` (`sharp` passed in, stubbed in tests) so every
  error branch (composite failure, invalid input, save failure) is
  coverable and the c8 `--100` gate holds without contortions.

### Draft bases and draft/final boundary

WU-002 has no native captures yet. `visual-render.mjs` builds deterministic
synthetic base images (solid `paper` field with a centered `ink` fixture
label drawn from the WU-001 fixture state ID — clearly synthetic, never
redrawn UI controls) purely to prove all 30 geometries and overlay math.
Draft PNGs live under `release/generated/visuals/draft/<device>/frame-<1..6>-<state-id>.png`.
`visual-validate.mjs` fails closed if any WU-002 output lands outside
`draft/` on the final visual paths (`visuals/{iphone,ipad,mac,android-phone,android-tablet}/`),
making the draft/final boundary mechanical. WU-005/WU-006 replace bases with
real captures; the overlay stage is shared unchanged.

### Icons

- macOS: full `AppIcon.appiconset` (16/32/128/256/512 pt at 1x/2x = 10 PNGs +
  `Contents.json`) derived from the checked-in iOS 1024px master
  (`apps/ios/Riot/Assets.xcassets/AppIcon.appiconset/AppIcon-1024.png`),
  opaque, no alpha.
- Android: legacy launcher PNGs at mdpi 48 / hdpi 72 / xhdpi 96 / xxhdpi 144
  / xxxhdpi 192; adaptive `foreground.png` 432×432 with glyph inside the
  66%-diameter safe zone (RGBA, transparent margin) and `background.png`
  432×432 opaque `paper`; `play-icon-512.png` full-bleed 512×512.
- Google: `feature-graphic-1024x500.png` branded band composition, no
  screenshot content, no device frames;
  `visuals/google/play-icon-512.png` is a byte copy of the icons output.
- All icon pixels derive from the same 1024 master; generation is
  deterministic.

### Draft provenance and approval record

`release/generated/visual-draft-provenance.json`: canonical JSON with
`schemaVersion: 1`, `platform: "darwin-arm64"`, `locale: "en-US"`,
`appearance: "light"`, `fixtureRevision: "riot-1.0-synthetic-v1"`,
`fixtureSha256` (WU-001 pinned digest), `templateVersion: 1`, one entry per
cell `{device, frame, stateId, claimId, variant, width, height, file, sha256}`
(`claimId` links each frame to the claims whitelist), plus `icons` entries
for every icon file. Exactly 30 cell entries; missing or extra cells fail
validation. The candidate commit is deliberately absent (a tracked file
cannot contain its own commit); WU-005/006 final provenance records the
candidate commit. `visual-validate.mjs` re-derives every digest and geometry
from the actual PNGs.

Human visual review (Task 5) records an approval block inside the
provenance file: `approval: {manifestSha256, reviewer, reviewedAt, nativeSizeResult, thumbnailResult}`
where `manifestSha256` is the sha256 of the canonical provenance bytes
**with the `approval` field absent** (a file cannot contain a digest of
itself; `visual-validate.mjs` re-derives it exactly that way), plus the
human reviewer's name, an RFC 3339 timestamp, and explicit PASS/FAIL for
both the native-size and 320-px thumbnail reviews.

## Task 0: Prerequisites

**Files:** `package.json`, `package-lock.json` only.

1. Verify WU-001 green: focused native fixture suites, `npm run
   test:release:unit`, c8 100% coverage, `npm run release:generate` +
   `git diff --exit-code -- release/generated/worksheets`, expected
   `release:status` exit 1.
2. Add `sharp@0.35.3` to `package.json` devDependencies and run `npm
   install` to update `package-lock.json`; require `node -e
   "import('sharp').then(s=>console.log(s.default.versions.vips))"` to print
   libvips `1.3.2`.
3. Confirm the four font files and the iOS 1024px icon master exist.
4. Ask the user for the real public support operator contact address;
   record it for Task 2. Do not author the support page without it.

## Task 1: Metadata contract tests RED

**Files:** create `scripts/release/test/metadata.test.mjs`; create the two
metadata schema files and register **only** `apple-metadata` and
`google-metadata` in `EXPECTED_IDS` (their files exist in the same change,
so the closed registry never breaks `generate`/`status` mid-RED).

1. Write tests for: every field at exact limit passes and limit+1 code point
   fails (all emitted fields, both platforms); missing/extra source fields
   fail (closed schemas); URL fields exact; category allowlists; prohibited
   vocabulary table (one case per entry, including mixed case);
   pinned-sentence presence/absence; cross-platform consistency (remove a
   disclaimer from one source → fail); claims whitelist and claim-carrier
   rule against `visuals.json` headlines (a claim outside the list, or a
   headline claim absent from description → fail); manifest digests match
   emitted bytes; generation is deterministic (two runs, byte-equal).
2. Run `node --test scripts/release/test/metadata.test.mjs`; expect failure
   from missing `metadata.mjs` and missing sources. Existing release tests
   stay green throughout.

## Task 2: Metadata sources, generator, and marketing pages

**Files:** the two source JSONs, `release/source/visuals.json` +
`visuals.schema.json` (authored here — the claim-carrier rule reads frame
headlines from `visuals.json`, so it must exist before metadata generation
is wired into `generate`; register the `visuals` schema ID here too, with
its file, per the registry invariant), `metadata.mjs`, `product.json` +
`policy.test.mjs` updates, marketing pages/mirrors/sitemap/README,
contract-script extension, CLI wiring.

1. Author canonical en-US sources honoring the contract above.
2. Implement `metadata.mjs`: `loadMetadataSources`, `evaluateMetadata`
   (gate list, 7-field shape), `generateMetadata` (atomic writes reusing the
   WU-000 staging/swap discipline; deterministic bytes; manifests last).
3. Write the support page (using the user-confirmed operator contact) and
   accessibility page; add footer/topnav links on all pages + mirrors;
   update `sitemap.xml` and `marketing/README.md`; extend
   `protocol-page-contracts.mjs`; run it.
4. Flip `product.json` support `evidenceState` to `current`; update the
   three pinning touch points in `policy.test.mjs` (the `url.support` gate
   outcome, the `_urlEvidence.support.state` assertion, and the
   declared-state branch expectation); `url.support` now PASSes.
5. Wire metadata generation + gates into `cli.mjs` (`generate` prints the
   expanded artifact count; `status` includes `metadata.*` gates).
6. Re-run metadata tests + full release suite; PASS with nonzero coverage.

## Task 3: Configuration validator tests RED, then implement

**Files:** `configuration.mjs`, `test/configuration.test.mjs`, CLI wiring.

1. RED: fixture-snapshot table covering every gate row including today's
   exact current-repo snapshot and the freshness-hash drift case; run,
   expect missing-module failure.
2. Implement `evaluateConfiguration` plus the snapshot freshness check;
   wire `config.*` gates into `status` using the checked-in current
   snapshot.
3. Verify `release:status` still exits 1 (more BLOCKED gates, all truthful)
   and the suite stays green.

## Task 4: Visual model and validator tests RED

**Files:** `visual-model.mjs`, `visual-validate.mjs`, `image-inspector.mjs`,
`render-environment.mjs`, the five test files. (`release/source/visuals.json`,
its schema, and its registry ID were already created in Task 2; this task's
RED tests exercise them against the missing modules.)

1. RED tests: exactly 30 cells with pinned dims/orientation; frame-5 variant
   selection (nearby on phones, join elsewhere); band-fraction boundaries
   (24%/28% pass, +1px fail); headline line/length limits; inset ≥5%;
   contrast ≥4.5:1 (synthetic color pairs at threshold ±); TTF-metric
   cap-height minima per class (real-font parse + undersized layout →
   fail); per-artifact alpha table; EXIF absence (metadata-carrying PNG
   fixture → fail); prohibited-token scan (one case per WU-001 class →
   fail); provenance completeness (missing cell, extra cell, wrong digest →
   fail); 320-px thumbnail geometry minima; draft/final path guard;
   fontconfig env setup (spawned-process assertions); Sharp-adapter error
   branches (stubbed); determinism.
2. Run; expect missing-module failures.

## Task 5: Renderer, icons, drafts, provenance, human review

**Files:** `visual-render.mjs` implementation; generate drafts, icons,
feature graphic, provenance (with approval block).

1. Implement the renderer per the mechanism section: hermetic fontconfig,
   injected Sharp, SVG band + text, synthetic bases, EXIF-free PNGs.
2. Generate all 30 drafts, the macOS iconset, Android legacy + adaptive
   icons, Play 512 icon (copied to both output paths), and feature graphic
   from the 1024 master.
3. Emit `release/generated/visual-draft-provenance.json` (approval block
   filled after step 5).
4. Run the full visual suite; PASS. Then `npm run release:generate` twice +
   `git diff --exit-code -- release/generated` (byte-reproducible on this
   platform).
5. Human visual review: open every draft/icon at native size and at 320 px
   thumbnail width; record `manifestSha256`, reviewer, `reviewedAt`, and
   both results in the provenance approval block; regenerate; repeat the
   drift check.

## Task 6: Quality gates and commit

1. `npm run test:release:unit` + c8 `--100` coverage (must include all new
   modules — every line/branch covered).
2. `node scripts/marketing/protocol-page-contracts.mjs`.
3. `release:status --json` exits 1: `url.support` and metadata gates PASS,
   config gates truthfully BLOCKED, no gate reports PASS without evidence.
4. `git diff --check`; stage exactly the declared paths (generated outputs
   included); verify `git diff --cached --name-status` matches scope.
5. Commit:
   `git commit -m "feat(release): generate store metadata and visual system"`.
6. Post-commit: rerun the full release suite, coverage, generation drift
   check, contract script, and clean-worktree check.
7. Independent spec-compliance and code-quality reviews; address findings
   before WU-003 planning begins.

## Acceptance criteria

- All emitted Apple/Google fields validated at exact limits with boundary
  tests; URL and category fields validated; prohibited vocabulary (including
  bech32 prefixes) and pinned-sentence rules enforced mechanically;
  claim-carrier rule enforced against `visuals.json` headlines.
- Generated metadata, manifests, drafts, icons, and provenance are
  byte-reproducible on the pinned darwin-arm64 platform; `git diff
  --exit-code -- release/generated` is clean after regeneration; manifest
  top-level digests re-derivable for WU-003 binding.
- Support page (real user-confirmed operator contact, public-channel SLA
  scope, unshipped-controls disclosure) and accessibility page exist;
  site-wide nav/footer/sitemap consistent; mirrors byte-identical;
  marketing contract script passes; `url.support` gate PASSes;
  `policy.publicContact` remains truthfully BLOCKED.
- Configuration validator reports today's exact BLOCKED set from fixtures
  without turning the suite red; snapshot freshness gate prevents stale
  status; `status` stays exit 1 and truthful.
- Exactly 30 draft cells with pinned geometries; band/inset/contrast/
  cap-height/thumbnail/alpha/EXIF/prohibited-token rules validated; outputs
  confined to `draft/` mechanically; provenance complete with per-cell
  claim IDs and a filled human-review approval block (both review results).
- 100% c8 coverage over all `scripts/release/**/*.mjs` including new
  modules; WU-000/WU-001 regressions green.
- Only declared paths committed; Sharp 0.35.3 is the sole new dependency;
  deferred items recorded: copyright/seller fields (account-confirmed),
  review-instructions/content-rating worksheets (WU-000-owned), Sharp
  toolchain pinning (WU-004).
