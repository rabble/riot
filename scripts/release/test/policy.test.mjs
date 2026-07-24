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

test("every asserted code and evidence path exists in the repository", async () => {
  const loaded = await sources();
  const assertedPaths = [];
  const visit = (value) => {
    if (Array.isArray(value)) {
      value.forEach(visit);
      return;
    }
    if (!value || typeof value !== "object") return;
    for (const [key, child] of Object.entries(value)) {
      if (key === "codePaths") assertedPaths.push(...child);
      if (key === "evidencePath" && typeof child === "string") assertedPaths.push(child);
      visit(child);
    }
  };
  for (const [key, value] of Object.entries(loaded)) {
    if (key === "product") continue;
    visit(value);
  }
  for (const path of assertedPaths) {
    await assert.doesNotReject(() => realFs.access(join(repositoryRoot, path)), path);
  }
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

test("policy evaluation covers passing, blocked, missing, and human evidence states", async () => {
  const loaded = structuredClone(await sources());
  loaded.product.version = "0.1";
  loaded.privacy.requiredReasonApis.push({
    api: "file-timestamp",
    reason: "C617.1",
    evidencePath: "future/PrivacyInfo.xcprivacy",
  });
  loaded.policy.controls.termsAcceptance = {
    state: "pass",
    codePaths: ["app/TermsView"],
    operator: "Release operator",
    maxHours: null,
    reason: "Implemented and evidenced.",
  };
  loaded.policy.controls.reportAcknowledgement = {
    state: "pass",
    codePaths: ["app/ReportView"],
    operator: "Release operator",
    maxHours: 24,
    reason: "Implemented and evidenced.",
  };
  loaded.reviewInstructions.topics = loaded.reviewInstructions.topics.filter(({ id }) => id !== "no-peers");
  loaded.accessibility.records = loaded.accessibility.records.filter(({ device, task }) => !(device === "iphone" && task === "read"));
  loaded.accessibility.records.find(({ device, task }) => device === "iphone" && task === "create").state = "blocked";
  const joined = loaded.accessibility.records.find(({ device, task }) => device === "iphone" && task === "join");
  joined.state = "pass";
  joined.evidencePath = "evidence/iphone-join.json";
  loaded.claims.claims[0].state = "pass";
  loaded.claims.claims[1].state = "blocked";
  loaded.exportCompliance.classification = {
    state: "pass",
    reason: "Approved.",
    evidence: ["evidence/export.json"],
    approver: "Legal",
  };
  loaded.accountGates.gates[0] = {
    name: "agreements",
    state: "pass",
    reason: "Confirmed.",
    evidence: ["evidence/agreements.json"],
  };
  loaded.accountGates.gates[1].state = "blocked";
  const gates = evaluatePolicy(loaded);
  const states = new Map(gates.map(({ id, state }) => [id, state]));
  assert.equal(states.get("product.identity"), "BLOCKED");
  assert.equal(states.get("privacy.required-reason-audit"), "PASS");
  assert.equal(states.get("policy.termsAcceptance"), "PASS");
  assert.equal(states.get("policy.reportAcknowledgement"), "PASS");
  assert.equal(states.get("review.no-peers"), "BLOCKED");
  assert.equal(states.get("accessibility.iphone.read"), "BLOCKED");
  assert.equal(states.get("accessibility.iphone.create"), "BLOCKED");
  assert.equal(states.get("accessibility.iphone.join"), "PASS");
  assert.equal(states.get("claim.read-local-community"), "PASS");
  assert.equal(states.get("claim.create-community"), "BLOCKED");
  assert.equal(states.get("export.classification"), "PASS");
  assert.equal(states.get("account.agreements"), "PASS");
  assert.equal(states.get("account.tax"), "BLOCKED");
  assert(gates.every(({ sourceFile }) => sourceFile.startsWith("release/source/")));
});

test("export and account evidence never pass on incomplete approvals", async () => {
  const loaded = await sources();
  loaded.exportCompliance.classification.state = "blocked";
  loaded.accountGates.gates[0].state = "pass";
  loaded.accountGates.gates[0].evidence = [];
  const states = new Map(evaluatePolicy(loaded).map(({ id, state }) => [id, state]));
  assert.equal(states.get("export.classification"), "BLOCKED");
  assert.equal(states.get("account.agreements"), "HUMAN ACTION");
});

test("loadPolicySources rejects missing adapters and malformed canonical JSON", async () => {
  await assert.rejects(() => loadPolicySources({ sourceDirectory: "", fs: realFs }), /required/);
  await assert.rejects(() => loadPolicySources({ sourceDirectory, fs: {} }), /required/);
  const root = await mkdtemp(join(tmpdir(), "riot-policy-source-"));
  await realFs.cp(join(repositoryRoot, "release"), join(root, "release"), { recursive: true });
  const directory = join(root, "release", "source");
  await realFs.writeFile(join(directory, "product.json"), "{", "utf8");
  await assert.rejects(
    () => loadPolicySources({ sourceDirectory: directory, fs: realFs }),
    /malformed or missing source/,
  );
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
  for (const fs of [null, {}, { mkdir() {} }, {
    mkdir() {}, writeFile() {}, rename() {}, rm: "not-a-function",
  }]) {
    await assert.rejects(
      () => generateWorksheets({ sources: loaded, outputDirectory: "/unused", fs, sha256 }),
      /filesystem adapter/,
    );
  }
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory: "/unused", fs: realFs }),
    /sha256/,
  );
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheets-hash-"));
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256: () => "short",
    }),
    /invalid digest/,
  );
});
