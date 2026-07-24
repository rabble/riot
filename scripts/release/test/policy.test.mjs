import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as realFs from "node:fs/promises";
import { mkdtemp, readdir, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  evaluatePolicy,
  generateWorksheets,
  loadPolicySources,
} from "../policy.mjs";

const repositoryRoot = dirname(dirname(dirname(dirname(fileURLToPath(import.meta.url)))));
const sourceDirectory = join(repositoryRoot, "release", "source");
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const expectedWorksheets = [
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
];

async function sources() {
  return loadPolicySources({ sourceDirectory, fs: realFs });
}

test("real policy sources are schema-valid and truthfully not public-ready", async () => {
  const result = evaluatePolicy(await sources());
  assert(result.some(({ id, state }) => id === "product.identity" && state === "PASS"));
  assert(result.some(({ id, state }) => id === "policy.filtering" && state === "BLOCKED"));
  assert(result.some(({ id, state }) => id === "url.support" && state === "BLOCKED"));
  assert(result.some(({ id, state }) => id === "export.classification" && state === "HUMAN ACTION"));
  assert(result.some(({ id, state }) => id === "account.agreements" && state === "HUMAN ACTION"));
  assert(result.every(({ sourceFile, pointer, observed, expected, recovery }) =>
    sourceFile && pointer && observed !== undefined && expected !== undefined && recovery));
});

test("policy evaluation rejects missing network rows and contradictory privacy evidence", async () => {
  const loaded = await sources();
  loaded.networkMatrix.rows = loaded.networkMatrix.rows.filter(({ id }) => id !== "nearby-sync");
  loaded.privacy.answers[0].evidenceRowIds = ["not-present"];
  const gates = evaluatePolicy(loaded);
  assert(gates.some(({ id, state }) => id === "network.nearby-sync" && state === "BLOCKED"));
  assert(gates.some(({ id, state }) => id === "privacy.apple" && state === "BLOCKED"));
});

test("policy evaluation enforces UGC service levels and complete accessibility evidence", async () => {
  const loaded = await sources();
  loaded.policy.controls.reportAcknowledgement.maxHours = 25;
  loaded.accessibility.records = loaded.accessibility.records.slice(1);
  const gates = evaluatePolicy(loaded);
  assert(gates.some(({ id, state }) => id === "policy.reportAcknowledgement" && state === "BLOCKED"));
  assert(gates.some(({ id, state }) => id.startsWith("accessibility.") && state === "BLOCKED"));
});

test("generateWorksheets emits exactly eleven deterministic digested files", async () => {
  const loaded = await sources();
  const first = await mkdtemp(join(tmpdir(), "riot-worksheets-a-"));
  const second = await mkdtemp(join(tmpdir(), "riot-worksheets-b-"));
  await generateWorksheets({ sources: loaded, outputDirectory: first, fs: realFs, sha256 });
  await generateWorksheets({ sources: loaded, outputDirectory: second, fs: realFs, sha256 });
  assert.deepEqual((await readdir(first)).sort(), expectedWorksheets);
  for (const name of expectedWorksheets) {
    const left = await readFile(join(first, name), "utf8");
    const right = await readFile(join(second, name), "utf8");
    assert.equal(left, right);
    assert.match(left, /^<!-- source-sha256: [0-9a-f]{64} -->\n/);
  }
});

test("generateWorksheets cleans temporary files after an atomic rename failure", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheets-fail-"));
  let renameCalls = 0;
  const failingFs = {
    ...realFs,
    async rename(from, to) {
      renameCalls += 1;
      if (renameCalls === 2) throw new Error("injected rename failure");
      return realFs.rename(from, to);
    },
  };
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: failingFs, sha256 }),
    /injected rename failure/,
  );
  assert((await readdir(outputDirectory)).every((name) => !name.includes(".tmp-")));
});

test("generateWorksheets requires injected filesystem and hash adapters", async () => {
  const loaded = await sources();
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory: "/unused", fs: realFs }),
    /sha256/,
  );
});
