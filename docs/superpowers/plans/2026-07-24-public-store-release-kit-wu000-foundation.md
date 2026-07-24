# Riot Release Kit WU-000 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the fail-closed canonical record, schema, policy worksheet,
toolchain-manifest, and read-only CLI foundation for Riot’s public early-access
release kit.

**Architecture:** Small Node ES modules receive injected filesystem, clock, and
hash dependencies. Ajv validates closed JSON Schema 2020-12 records; canonical
JSON and SHA-256 bind generated Markdown worksheets to their source evidence.
The first CLI supports only `generate` and read-only `status`; it never imports
store, signing, browser, or network mutation adapters.

**Tech Stack:** Node 26.4.0 ESM, `node:test`, c8 11.0.0, Ajv 8.20.0, JSON
Schema 2020-12, Markdown outputs, npm 11.17.0.

---

**Master plan:** `docs/superpowers/plans/2026-07-24-public-store-release-kit-master-plan.md`

**Approved design:** `docs/superpowers/specs/2026-07-24-public-store-release-kit-design.md`

**Worktree:** `/Users/rabble/.config/superpowers/worktrees/riot/riot-public-store-release-kit`

## Task 1: Release test and dependency harness

**Files:**

- Modify: `package.json`
- Modify: `package-lock.json`
- Create: `scripts/release/test/helpers.mjs`

- [ ] **Step 1: Add the exact test harness helper**

Create `scripts/release/test/helpers.mjs`:

```js
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

export async function tempReleaseRoot() {
  return mkdtemp(join(tmpdir(), "riot-release-test-"));
}

export async function readUtf8(path) {
  return readFile(path, "utf8");
}

export const fixedClock = () => new Date("2026-07-24T00:00:00.000Z");
```

- [ ] **Step 2: Add pinned dependencies and scripts**

Add exact dependencies and scripts:

```json
{
  "scripts": {
    "release:generate": "node scripts/release/cli.mjs generate",
    "release:status": "node scripts/release/cli.mjs status",
    "test:release:unit": "node --test scripts/release/test/*.test.mjs",
    "test:release:coverage": "c8 --100 --all --include='scripts/release/**/*.mjs' --exclude='scripts/release/test/**' node --test scripts/release/test/*.test.mjs"
  },
  "devDependencies": {
    "ajv": "8.20.0"
  }
}
```

Keep existing scripts and dev dependencies unchanged. Run `npm install
--package-lock-only` to update the lockfile mechanically.

- [ ] **Step 3: Verify the harness**

Run:

```bash
npm ci
node -e "import('./scripts/release/test/helpers.mjs').then(async m => console.log(await m.tempReleaseRoot()))"
```

Expected: npm reports zero vulnerabilities and the helper prints one temporary
directory without changing tracked files beyond `package.json` and
`package-lock.json`.

- [ ] **Step 4: Commit**

```bash
git add package.json package-lock.json scripts/release/test/helpers.mjs
git commit -m "test(release): add release tooling harness"
```

## Task 2: Canonical JSON and digested records

**Files:**

- Create: `scripts/release/test/canonical-json.test.mjs`
- Create: `scripts/release/test/records.test.mjs`
- Create: `scripts/release/canonical-json.mjs`
- Create: `scripts/release/records.mjs`

- [ ] **Step 1: Write failing canonical JSON tests**

Cover stable object-key ordering, array-order preservation, terminal newline,
unsupported `undefined`/non-finite numbers, and UTF-8 output:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { canonicalJson } from "../canonical-json.mjs";

test("canonicalJson sorts object keys recursively and preserves array order", () => {
  assert.equal(
    canonicalJson({ z: [{ b: 2, a: 1 }], a: "Riot" }),
    '{"a":"Riot","z":[{"a":1,"b":2}]}\n',
  );
});

test("canonicalJson rejects values JSON cannot bind safely", () => {
  assert.throws(() => canonicalJson({ value: undefined }), /unsupported/);
  assert.throws(() => canonicalJson({ value: Number.NaN }), /finite/);
});
```

- [ ] **Step 2: Run RED**

Run:

```bash
node --test scripts/release/test/canonical-json.test.mjs
```

Expected: FAIL with module-not-found for `canonical-json.mjs`.

- [ ] **Step 3: Implement canonical JSON minimally**

Export:

```js
export function canonicalJson(value) {}
```

Recursively copy arrays and plain objects, sort object keys with code-point
ordering, reject unsupported prototypes/values, call `JSON.stringify`, and add
exactly one newline.

- [ ] **Step 4: Run GREEN**

Run the focused test and expect all cases to pass.

- [ ] **Step 5: Write failing record tests**

Define the record contract:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createRecord, verifyRecord } from "../records.mjs";
import { fixedClock } from "./helpers.mjs";

const sha256 = (bytes) =>
  createHash("sha256").update(bytes).digest("hex");

test("createRecord binds schema, payload, timestamp, and full sha256", async () => {
  const record = await createRecord({
    schema: "riot.release.product.v1",
    payload: { name: "riot.protest.net" },
    clock: fixedClock,
    sha256,
  });
  assert.equal(record.version, 1);
  assert.match(record.digest, /^[0-9a-f]{64}$/);
  assert.equal(record.createdAt, "2026-07-24T00:00:00.000Z");
  await assert.doesNotReject(() => verifyRecord(record, { sha256 }));
});
```

- [ ] **Step 6: Run RED, implement, then run GREEN**

`createRecord` canonicalizes `{version,schema,createdAt,payload}` and stores its
full SHA-256. `verifyRecord` rejects truncated, uppercase, malformed, or
mismatched digests and returns the frozen verified payload.

- [ ] **Step 7: Commit**

```bash
git add scripts/release/canonical-json.mjs scripts/release/records.mjs \
  scripts/release/test/canonical-json.test.mjs scripts/release/test/records.test.mjs
git commit -m "feat(release): add canonical digested records"
```

## Task 3: Closed JSON Schema registry

**Files:**

- Create: `scripts/release/test/schema.test.mjs`
- Create: `scripts/release/schema.mjs`
- Create: `release/schemas/common.schema.json`
- Create: `release/schemas/product.schema.json`
- Create: `release/schemas/privacy.schema.json`
- Create: `release/schemas/policy.schema.json`
- Create: `release/schemas/accessibility.schema.json`
- Create: `release/schemas/claims.schema.json`
- Create: `release/schemas/export-compliance.schema.json`
- Create: `release/schemas/account-gates.schema.json`
- Create: `release/schemas/network-matrix.schema.json`
- Create: `release/schemas/review-instructions.schema.json`
- Create: `release/schemas/toolchains.schema.json`

- [ ] **Step 1: Write failing schema tests**

The tests load a registry from a supplied directory and assert:

```js
test("validateSource rejects unknown and missing fields with JSON pointers", async () => {
  const registry = await loadSchemaRegistry(schemaDir);
  assert.throws(
    () => validateSource(registry, "product", { version: 1, surprise: true }),
    /\/surprise.*unknown/,
  );
  assert.throws(
    () => validateSource(registry, "product", { version: 1 }),
    /\/name.*required/,
  );
});
```

Also cover unknown schema names, malformed schema JSON, duplicate `$id`, wrong
schema version, truncated IDs, and non-RFC3339 timestamps. Keep minimal valid
object factories inline in `schema.test.mjs`; Task 4 exercises the real source
files through the same registry.

- [ ] **Step 2: Run RED**

Expected: module-not-found for `schema.mjs`.

- [ ] **Step 3: Implement the registry**

Use `Ajv2020` from `ajv/dist/2020.js` with:

```js
new Ajv2020({
  allErrors: true,
  strict: true,
  validateFormats: false,
});
```

Every object schema uses `"additionalProperties": false`. Convert Ajv errors
into sorted diagnostics shaped as:

```js
{ pointer: "/field", keyword: "required", message: "must have required property 'field'" }
```

The public API is:

```js
export async function loadSchemaRegistry(directory) {}
export function validateSource(registry, name, value) {}
```

- [ ] **Step 4: Run GREEN**

Run `node --test scripts/release/test/schema.test.mjs`; expect all cases to
pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/release/schema.mjs scripts/release/test/schema.test.mjs release/schemas
git commit -m "feat(release): add closed release schemas"
```

## Task 4: Canonical policy sources and explicit worksheets

**Files:**

- Create: `release/source/product.json`
- Create: `release/source/privacy.json`
- Create: `release/source/policy.json`
- Create: `release/source/accessibility.json`
- Create: `release/source/claims.json`
- Create: `release/source/export-compliance.json`
- Create: `release/source/account-gates.json`
- Create: `release/source/network-matrix.json`
- Create: `release/source/review-instructions.json`
- Create: `release/roles.json`
- Create: `scripts/release/test/policy.test.mjs`
- Create: `scripts/release/policy.mjs`
- Generate: `release/generated/worksheets/*.md`

- [ ] **Step 1: Write failing policy tests**

Tests must reject:

- product values other than version `1.0`, price `free`, availability
  `worldwide`, release channel `public-early-access`, Apple bundle ID and
  Android application ID `net.protest.riot`;
- a privacy answer marked `no-collection` without a matching outbound-network
  evidence row;
- missing camera/Bluetooth/local-network/notification/Android-INTERNET
  justification;
- a required-reason API with no manifest reason;
- any UGC safeguard without code-path evidence, operator, or response SLA;
- imminent-harm/illegal response above 24 hours or other objectionable-content
  response above 72 hours;
- an accessibility claim without every common-task/device record;
- a store claim without code-path and candidate-journey IDs;
- resolved Apple export classification without approver/evidence;
- account/legal answers represented as `PASS` without authenticated evidence.

The valid fixture produces exactly these worksheet filenames:

```js
[
  "accessibility.md",
  "account-gates.md",
  "app-privacy.md",
  "content-rating.md",
  "data-safety.md",
  "export-compliance.md",
  "outbound-network.md",
  "permissions.md",
  "required-reason-apis.md",
  "review-instructions.md",
  "ugc-operations.md",
]
```

- [ ] **Step 2: Run RED**

Run:

```bash
node --test scripts/release/test/policy.test.mjs
```

Expected: FAIL because `policy.mjs` and sources do not exist.

- [ ] **Step 3: Populate evidence-backed source records**

Use only claims verified from current code. Unresolved export, agreements,
trader/tax/banking, signed-candidate, hardware, and Console facts use:

```json
{
  "state": "human-action",
  "reason": "Requires authenticated account, legal, signing, hardware, or Console evidence.",
  "evidence": []
}
```

If filtering, in-app content reporting, in-app author reporting, local author
blocking, moderator/tombstone handling, public contact, or response ownership
is absent, record `"state": "blocked"` with the exact missing code path. Do not
edit product code in this work unit.

- [ ] **Step 4: Implement validation and deterministic Markdown generation**

Export:

```js
export function evaluatePolicy(sources) {}
export async function generateWorksheets({ sources, outputDirectory, fs }) {}
```

`evaluatePolicy` returns ordered gates with `PASS`, `BLOCKED`, or
`HUMAN ACTION`, JSON pointer, observed value, expected value, and recovery.
`generateWorksheets` writes through an injected atomic writer and embeds source
digests at the top of each Markdown file.

- [ ] **Step 5: Run GREEN and deterministic-generation check**

```bash
node --test scripts/release/test/policy.test.mjs
```

The policy test generates into two fresh temporary directories and asserts
identical filename lists and bytes. The CLI does not exist until Task 6; repeat
the tracked generated-drift command after Task 6 commits the generated output.

- [ ] **Step 6: Commit**

```bash
git add release/source release/roles.json release/generated/worksheets \
  scripts/release/policy.mjs scripts/release/test/policy.test.mjs
git commit -m "feat(release): add evidence-backed policy worksheets"
```

## Task 5: Checked-in toolchain authority record

**Files:**

- Create: `release/toolchains.json`
- Modify: `release/schemas/toolchains.schema.json`
- Modify: `scripts/release/test/schema.test.mjs`

- [ ] **Step 1: Write failing manifest tests**

Cover the current pinned Rust channel, Node/npm package-manager values, Gradle
distribution checksum, Android SDK/NDK expectations, Xcode/Swift requirements,
Ajv/c8 versions, missing checksum, duplicate tool name, duplicate
`versionCommand`, non-absolute download URLs, and malformed SHA-256.

- [ ] **Step 2: Run RED**

Run `node --test scripts/release/test/schema.test.mjs`.

Expected: FAIL because `release/toolchains.json` is absent.

- [ ] **Step 3: Populate and validate**

Create one entry per required tool:

```json
{
  "version": 1,
  "tools": [
    {
      "name": "node",
      "version": "26.4.0",
      "versionCommand": ["node", "--version"],
      "sha256": null,
      "source": "package.json#engines.node"
    }
  ]
}
```

Downloaded distributions require lowercase 64-character SHA-256; system tools
whose binaries are selected externally use `null` plus an exact checked-in
source pointer. Actual command execution and drift inspection belong to WU-004
and are not implemented here.

- [ ] **Step 4: Run GREEN**

Run the focused schema test and expect all manifest-shape cases to pass.

- [ ] **Step 5: Commit**

```bash
git add release/toolchains.json release/schemas/toolchains.schema.json \
  scripts/release/test/schema.test.mjs
git commit -m "feat(release): pin release toolchains"
```

## Task 6: Read-only generate/status CLI

**Files:**

- Create: `scripts/release/test/cli.test.mjs`
- Create: `scripts/release/cli.mjs`
- Create: `scripts/release/status.mjs`

- [ ] **Step 1: Write failing CLI and status tests**

Use a spawned Node process with a temporary release root. Cover:

- `generate` writes deterministic worksheets and a second run is byte-stable;
- `status` prints every gate and exits `0` only when all pass;
- `status` exits `2` for `HUMAN ACTION`, `1` for `BLOCKED`, and `64` for usage;
- `status --json` emits the same ordered gate objects as text mode;
- mixed results summarize to `BLOCKED`;
- malformed/missing source and schema files fail closed without a stack trace;
- unknown commands/options and store/signing credential flags are rejected;
- stdout contains no secrets or full evidence payloads.

- [ ] **Step 2: Run RED**

Expected: module-not-found for `cli.mjs`.

- [ ] **Step 3: Implement status aggregation**

Export:

```js
export function summarizeGates(gates) {}
export function renderStatus(gates) {}
```

Precedence is `BLOCKED` over `HUMAN ACTION` over `PASS`. Preserve diagnostic
pointer, observed/expected values, and recovery command.

- [ ] **Step 4: Implement the thin CLI**

`cli.mjs` parses only:

```text
generate
status
status --json
```

It imports pure modules, provides real filesystem/clock/hash adapters, catches
expected errors into fixed diagnostics, and contains no store/network/signing
adapter.

- [ ] **Step 5: Run GREEN and full generation**

```bash
node --test scripts/release/test/cli.test.mjs
npm run release:generate
set +e
npm run release:status
release_status=$?
set -e
test "$release_status" -eq 1 -o "$release_status" -eq 2
```

Expected at this stage: generation succeeds; status exits `1` because truthful
UGC/product gaps are blocking, or `2` when only authenticated human decisions
remain. It must never incorrectly return `0`.

- [ ] **Step 6: Commit**

```bash
git add scripts/release/cli.mjs scripts/release/status.mjs \
  scripts/release/test/cli.test.mjs release/generated/worksheets
git commit -m "feat(release): add read-only release status CLI"
```

## Task 7: WU-000 quality gate

**Files:**

- Modify only if coverage exposes an untested branch:
  `scripts/release/**/*.mjs`
- Modify only if tests need a behavior case:
  `scripts/release/test/*.test.mjs`

- [ ] **Step 1: Run the focused unit suite**

```bash
npm run test:release:unit
```

Expected: all tests pass; zero skipped/todo tests.

- [ ] **Step 2: Run the 100 percent release-tool coverage gate**

```bash
npm run test:release:coverage
```

Expected: 100 percent lines, branches, functions, and statements for every
production module under `scripts/release/`.

- [ ] **Step 3: Run existing Node regressions**

```bash
npm run test:web:unit
npm run test:apps:unit
```

Expected: the existing 43 tests and all new release tests pass.

- [ ] **Step 4: Verify generation and scope**

```bash
npm run release:generate
git diff --exit-code -- release/generated
git diff --check
git status --short
```

Expected: only WU-000 declared files differ from its starting commit; no
credential, private fixture, build product, or unredacted evidence is tracked.

- [ ] **Step 5: Commit any test-only closure**

If coverage required no changes, do not create an empty commit. Otherwise:

```bash
git add scripts/release scripts/release/test
git commit -m "test(release): close foundation coverage"
```

## WU-000 completion contract

- Canonical source/schema/toolchain records validate fail closed.
- Every required policy worksheet exists and embeds source digests.
- Public early-access/free/worldwide/product IDs are canonical.
- Missing UGC safeguards block candidate production and name exact recovery.
- Legal/account/export/signing/hardware/Console facts remain `HUMAN ACTION`.
- `generate` is deterministic; `status` and `status --json` agree.
- Release-tool coverage is 100 percent without lowering
  `.coverage-thresholds.json`.
- No store, signing, credential-copy, browser-automation, or production
  mutation code exists.
