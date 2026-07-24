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
  assert(result.some(({ id, state }) => id === "permission.android-internet" && state === "PASS"));
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
      if (key === "platformCodePaths") assertedPaths.push(...Object.values(child).flat());
      if (key === "evidencePath" && typeof child === "string") assertedPaths.push(child);
      visit(child);
    }
  };
  for (const value of Object.values(loaded)) visit(value);
  for (const path of assertedPaths) {
    const productUrl = Object.values(loaded.product.urls).find(({ evidencePath }) => evidencePath === path);
    if (productUrl?.evidenceState === "missing") {
      await assert.rejects(() => realFs.access(join(repositoryRoot, path)), path);
    } else {
      await assert.doesNotReject(() => realFs.access(join(repositoryRoot, path)), path);
    }
  }
});

test("URL evidence is verified from injected filesystem content, not trusted source state", async () => {
  const loaded = await sources();
  assert.equal(loaded._urlEvidence.privacy.state, "current");
  assert.equal(loaded._urlEvidence.marketing.state, "current");
  assert.equal(loaded._urlEvidence.support.state, "missing");

  const root = await mkdtemp(join(tmpdir(), "riot-url-evidence-"));
  await realFs.cp(join(repositoryRoot, "release"), join(root, "release"), { recursive: true });
  await realFs.mkdir(join(root, "marketing", "privacy"), { recursive: true });
  await realFs.mkdir(join(root, "marketing", "releases"), { recursive: true });
  await realFs.writeFile(join(root, "marketing", "privacy", "index.html"), "", "utf8");
  await realFs.writeFile(join(root, "marketing", "releases", "index.html"), "<!doctype html><html></html>", "utf8");
  const checked = await loadPolicySources({
    sourceDirectory: join(root, "release", "source"),
    fs: realFs,
  });
  assert.equal(checked._urlEvidence.privacy.state, "stale");
  assert.equal(checked._urlEvidence.marketing.state, "current");
  assert.equal(checked._urlEvidence.support.state, "missing");

  const productPath = join(root, "release", "source", "product.json");
  await realFs.writeFile(join(root, "marketing", "privacy", "index.html"), "<!doctype html><html></html>", "utf8");
  const product = JSON.parse(await realFs.readFile(productPath, "utf8"));
  product.urls.privacy.evidenceState = "stale";
  await realFs.writeFile(productPath, `${JSON.stringify(product)}\n`, "utf8");
  const explicitlyStale = await loadPolicySources({
    sourceDirectory: join(root, "release", "source"),
    fs: realFs,
  });
  assert.equal(explicitlyStale._urlEvidence.privacy.state, "stale");
  assert.equal(
    evaluatePolicy(explicitlyStale).find(({ id }) => id === "url.privacy").state,
    "BLOCKED",
  );
});

test("URL evidence paths reject absolute, traversal, page-type mismatch, and symlink escape", async () => {
  for (const evidencePath of [
    "/tmp/privacy/index.html",
    "../marketing/privacy/index.html",
    "marketing/privacy/../support/index.html",
    "marketing/support/index.html",
  ]) {
    const root = await mkdtemp(join(tmpdir(), "riot-url-path-"));
    await realFs.cp(join(repositoryRoot, "release"), join(root, "release"), { recursive: true });
    const productPath = join(root, "release", "source", "product.json");
    const product = JSON.parse(await realFs.readFile(productPath, "utf8"));
    product.urls.privacy.evidencePath = evidencePath;
    await realFs.writeFile(productPath, `${JSON.stringify(product)}\n`, "utf8");
    await assert.rejects(
      () => loadPolicySources({ sourceDirectory: join(root, "release", "source"), fs: realFs }),
      (error) => error.sourceFile.endsWith("/product.json")
        && error.diagnostics[0].pointer === "/urls/privacy/evidencePath",
    );
  }

  const root = await mkdtemp(join(tmpdir(), "riot-url-symlink-"));
  await realFs.cp(join(repositoryRoot, "release"), join(root, "release"), { recursive: true });
  const outside = join(await mkdtemp(join(tmpdir(), "riot-url-outside-")), "index.html");
  await realFs.writeFile(outside, "<!doctype html><html></html>", "utf8");
  await realFs.mkdir(join(root, "marketing", "privacy"), { recursive: true });
  await realFs.symlink(outside, join(root, "marketing", "privacy", "index.html"));
  await assert.rejects(
    () => loadPolicySources({ sourceDirectory: join(root, "release", "source"), fs: realFs }),
    (error) => error.diagnostics[0].pointer === "/urls/privacy/evidencePath"
      && /repository/.test(error.diagnostics[0].expected),
  );
});

test("policy evaluation rejects missing network rows and contradictory privacy evidence", async () => {
  const loaded = await sources();
  loaded.networkMatrix.rows = loaded.networkMatrix.rows.filter(({ id }) => id !== "nearby-sync");
  loaded.privacy.answers[0].evidenceRowIds = ["not-present"];
  const gates = evaluatePolicy(loaded);
  assert(gates.some(({ id, state }) => id === "network.nearby-sync" && state === "BLOCKED"));
  assert(gates.some(({ id, state }) => id === "privacy.apple" && state === "BLOCKED"));
});

test("permission inventory rejects duplicates and substitution of exact required IDs", async () => {
  const loaded = await sources();
  loaded.privacy.permissions[4] = { ...loaded.privacy.permissions[0] };
  const gates = evaluatePolicy(loaded);
  assert(gates.some(({ id, state }) => id === "permission.camera" && state === "BLOCKED"));
  assert(gates.some(({ id, state }) => id === "permission.android-internet" && state === "BLOCKED"));
});

test("privacy and account inventories require every exact canonical entry once", async () => {
  const privacy = await sources();
  privacy.privacy.answers[1].store = "apple";
  assert.equal(
    evaluatePolicy(privacy).find(({ id }) => id === "inventory.privacy-answers").state,
    "BLOCKED",
  );

  const missingAccount = await sources();
  missingAccount.accountGates.gates.pop();
  assert.equal(
    evaluatePolicy(missingAccount).find(({ id }) => id === "inventory.account-gates").state,
    "BLOCKED",
  );

  const duplicateAccount = await sources();
  duplicateAccount.accountGates.gates[1] = structuredClone(duplicateAccount.accountGates.gates[0]);
  assert.equal(
    evaluatePolicy(duplicateAccount).find(({ id }) => id === "inventory.account-gates").state,
    "BLOCKED",
  );
});

test("no-collection answers require consistent position, fields, destinations, and retention", async () => {
  const loaded = await sources();
  loaded.privacy.position.noHiddenCollection = false;
  loaded.networkMatrix.rows[4].developerOperated = true;
  loaded.networkMatrix.rows[4].developerRetention = "indefinite";
  const gates = evaluatePolicy(loaded);
  assert(gates.some(({ id, state }) => id === "privacy.apple" && state === "BLOCKED"));
  assert(gates.some(({ id, state }) => id === "privacy.google" && state === "BLOCKED"));
});

test("no-collection requires every exact scenario and store-specific platform evidence", async () => {
  const missingScenario = await sources();
  missingScenario.privacy.answers[0].evidenceRowIds =
    missingScenario.privacy.answers[0].evidenceRowIds.slice(0, -1);
  assert.equal(
    evaluatePolicy(missingScenario).find(({ id }) => id === "privacy.apple").state,
    "BLOCKED",
  );

  const missingApple = await sources();
  missingApple.networkMatrix.rows[0].platformCodePaths.macos = [];
  const appleStates = new Map(evaluatePolicy(missingApple).map(({ id, state }) => [id, state]));
  assert.equal(appleStates.get("privacy.apple"), "BLOCKED");
  assert.equal(appleStates.get("privacy.google"), "PASS");

  const missingAndroid = await sources();
  missingAndroid.networkMatrix.rows[0].platformCodePaths.android = [];
  const androidStates = new Map(evaluatePolicy(missingAndroid).map(({ id, state }) => [id, state]));
  assert.equal(androidStates.get("privacy.apple"), "PASS");
  assert.equal(androidStates.get("privacy.google"), "BLOCKED");
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
  for (const platform of loaded.claims.claims[0].platforms) {
    const evidence = loaded.claims.claims[0].evidenceByPlatform[platform];
    evidence.candidateJourney = {
      candidateId: `candidate-${platform}`,
      journeyId: evidence.journeyIds[0],
      result: "PASS",
      evidenceDigest: "a".repeat(64),
    };
  }
  loaded.claims.claims[1].state = "blocked";
  loaded.policy.contentRating.state = "pass";
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
  assert.equal(states.get("accessibility.iphone.join"), "HUMAN ACTION");
  assert.equal(states.get("claim.read-local-community"), "HUMAN ACTION");
  assert.equal(states.get("claim.create-community"), "BLOCKED");
  assert.equal(states.get("content-rating"), "HUMAN ACTION");
  assert.equal(states.get("export.classification"), "HUMAN ACTION");
  assert.equal(states.get("account.agreements"), "HUMAN ACTION");
  assert.equal(states.get("account.tax"), "BLOCKED");
  assert(gates.every(({ sourceFile }) => sourceFile.startsWith("release/source/")));
});

test("normal gates identify exact array elements and per-platform claim evidence", async () => {
  const loaded = await sources();
  loaded.claims.claims[0].state = "pass";
  delete loaded.claims.claims[0].evidenceByPlatform.android;
  const gates = evaluatePolicy(loaded);
  assert.equal(gates.find(({ id }) => id === "privacy.apple").pointer, "/answers/0");
  assert.equal(gates.find(({ id }) => id === "network.first-launch").pointer, "/rows/0");
  assert.equal(gates.find(({ id }) => id === "accessibility.iphone.read").pointer, "/records/0");
  assert.equal(gates.find(({ id }) => id === "account.agreements").pointer, "/gates/0");
  const claim = gates.find(({ id }) => id === "claim.read-local-community");
  assert.equal(claim.pointer, "/claims/0");
  assert.equal(claim.state, "BLOCKED");
});

test("source-authored evidence cannot self-certify protected human gates", async () => {
  const loaded = await sources();
  const claim = loaded.claims.claims[0];
  claim.state = "pass";
  assert.equal(
    evaluatePolicy(loaded).find(({ id }) => id === "claim.read-local-community").state,
    "BLOCKED",
  );

  for (const platform of claim.platforms) {
    claim.evidenceByPlatform[platform].candidateJourney = {
      candidateId: `riot-1.0-${platform}-candidate`,
      journeyId: `${platform}:first-install-to-first-read`,
      result: "PASS",
      evidenceDigest: "a".repeat(64),
    };
  }
  loaded.accessibility.records[0] = {
    device: "iphone",
    task: "read",
    state: "pass",
    evidencePath: "release/evidence/accessibility.json",
  };
  loaded.policy.contentRating.state = "pass";
  loaded.exportCompliance.classification = {
    state: "pass",
    reason: "Source says approved.",
    evidence: ["release/evidence/export.json"],
    approver: "Source string",
  };
  loaded.accountGates.gates[0] = {
    name: "agreements",
    state: "pass",
    reason: "Source says approved.",
    evidence: ["release/evidence/account.json"],
  };
  const states = new Map(evaluatePolicy(loaded).map(({ id, state }) => [id, state]));
  for (const id of [
    "claim.read-local-community",
    "accessibility.iphone.read",
    "content-rating",
    "export.classification",
    "account.agreements",
  ]) {
    assert.equal(states.get(id), "HUMAN ACTION", id);
  }

  claim.evidenceByPlatform.android.candidateJourney.evidenceDigest = "short";
  assert.equal(
    evaluatePolicy(loaded).find(({ id }) => id === "claim.read-local-community").state,
    "BLOCKED",
  );
});

test("export and account evidence never pass on incomplete approvals", async () => {
  const loaded = await sources();
  loaded.policy.contentRating.state = "blocked";
  loaded.exportCompliance.classification.state = "blocked";
  loaded.accountGates.gates[0].state = "pass";
  loaded.accountGates.gates[0].evidence = [];
  const states = new Map(evaluatePolicy(loaded).map(({ id, state }) => [id, state]));
  assert.equal(states.get("export.classification"), "BLOCKED");
  assert.equal(states.get("account.agreements"), "BLOCKED");
  assert.equal(states.get("content-rating"), "BLOCKED");
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
  const permissions = await readFile(join(first, "permissions.md"), "utf8");
  assert.match(permissions, /android-internet[\s\S]*Android uses INTERNET/);
  assert.match(permissions, /apps\/android\/app\/src\/main\/AndroidManifest\.xml/);
  const requiredReason = await readFile(join(first, "required-reason-apis.md"), "utf8");
  assert.match(requiredReason, /Required-reason API inventory/);
  assert.match(requiredReason, /No approved API reasons are recorded/);
  const network = await readFile(join(first, "outbound-network.md"), "utf8");
  assert.match(network, /nearby-sync[\s\S]*user-selected nearby peer[\s\S]*signed community records[\s\S]*Direct local-network transport[\s\S]*receiving peer retains[\s\S]*Transport\/LocalNetworkNearby\.swift/);
  const review = await readFile(join(first, "review-instructions.md"), "utf8");
  assert.match(review, /No developer account or login is required[\s\S]*apps\/ios\/Riot\/RiotApp\.swift/);
  const exportWorksheet = await readFile(join(first, "export-compliance.md"), "utf8");
  assert.match(exportWorksheet, /Ed25519[\s\S]*XChaCha20-Poly1305[\s\S]*Android[\s\S]*worldwide/);
  const contentRating = await readFile(join(first, "content-rating.md"), "utf8");
  assert.match(contentRating, /Apple: 12\+[\s\S]*Google Play: Teen[\s\S]*user-generated content/);
});

test("worksheet digests bind network evidence that affects privacy gate bytes", async () => {
  const loaded = await sources();
  const first = await mkdtemp(join(tmpdir(), "riot-worksheet-digest-a-"));
  const second = await mkdtemp(join(tmpdir(), "riot-worksheet-digest-b-"));
  await generateWorksheets({ sources: loaded, outputDirectory: first, fs: realFs, sha256 });
  loaded.networkMatrix.rows[0].retention = "Changed canonical retention evidence.";
  await generateWorksheets({ sources: loaded, outputDirectory: second, fs: realFs, sha256 });
  for (const name of ["permissions.md", "required-reason-apis.md"]) {
    const before = (await readFile(join(first, name), "utf8")).split("\n")[0];
    const after = (await readFile(join(second, name), "utf8")).split("\n")[0];
    assert.notEqual(after, before, name);
  }
});

test("generateWorksheets swaps the complete set atomically and removes stale managed files", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheets-fail-"));
  await generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 });
  await realFs.writeFile(join(outputDirectory, "stale.md"), "old stale file\n", "utf8");
  const beforeNames = (await readdir(outputDirectory)).sort();
  const before = new Map(await Promise.all(beforeNames.map(async (name) =>
    [name, await readFile(join(outputDirectory, name), "utf8")])));
  loaded.accessibility.records[0].state = "blocked";
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
  assert.deepEqual((await readdir(outputDirectory)).sort(), beforeNames);
  for (const [name, bytes] of before) {
    assert.equal(await readFile(join(outputDirectory, name), "utf8"), bytes);
  }

  await generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 });
  assert.deepEqual((await readdir(outputDirectory)).sort(), expectedWorksheets);
  assert.doesNotMatch(await readFile(join(outputDirectory, "accessibility.md"), "utf8"), /old stale file/);
});

test("worksheet staging verification and swap errors leave the prior set untouched", async () => {
  const loaded = await sources();
  for (const mode of ["names", "bytes", "swap"]) {
    const outputDirectory = await mkdtemp(join(tmpdir(), `riot-worksheet-${mode}-`));
    await generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 });
    const before = await readFile(join(outputDirectory, "accessibility.md"), "utf8");
    const failingFs = {
      ...realFs,
      async readdir(path, options) {
        if (mode === "names" && String(path).includes(".staging-")) return [];
        return realFs.readdir(path, options);
      },
      async readFile(path, options) {
        const bytes = await realFs.readFile(path, options);
        return mode === "bytes" && String(path).includes(".staging-") ? `${bytes}corrupt` : bytes;
      },
      async rename(from, to) {
        if (mode === "swap" && from === outputDirectory) {
          const error = new Error("injected swap refusal");
          error.code = "EACCES";
          throw error;
        }
        return realFs.rename(from, to);
      },
    };
    await assert.rejects(
      () => generateWorksheets({ sources: loaded, outputDirectory, fs: failingFs, sha256 }),
      /incomplete|verification failed|swap refusal/,
    );
    assert.equal(await readFile(join(outputDirectory, "accessibility.md"), "utf8"), before);
  }
});

test("a post-swap backup cleanup refusal does not invalidate the installed set", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-cleanup-"));
  await generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 });
  loaded.accessibility.records[0].state = "blocked";
  let backupRemovals = 0;
  const cleanupFs = {
    ...realFs,
    async rm(path, options) {
      if (String(path).includes(".backup-")) {
        backupRemovals += 1;
        if (backupRemovals === 2) throw new Error("injected cleanup refusal");
      }
      return realFs.rm(path, options);
    },
  };
  await assert.doesNotReject(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: cleanupFs, sha256 }),
  );
  assert.match(await readFile(join(outputDirectory, "accessibility.md"), "utf8"), /iphone \/ read: blocked/);
});

test("worksheets render recorded inventories, approvals, operators, and evidence", async () => {
  const loaded = await sources();
  loaded.privacy.requiredReasonApis.push({
    api: "file-timestamp",
    reason: "C617.1",
    evidencePath: "release/evidence/privacy-manifest.json",
  });
  loaded.exportCompliance.classification.approver = "Release counsel";
  loaded.exportCompliance.classification.evidence = ["release/evidence/export.json"];
  loaded.accountGates.gates[0].evidence = ["release/evidence/agreements.json"];
  loaded.policy.controls.termsAcceptance.codePaths = ["apps/ios/Riot/TermsView.swift"];
  loaded.policy.controls.termsAcceptance.operator = "Trust operator";
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheets-recorded-"));
  await generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 });
  assert.match(await readFile(join(outputDirectory, "required-reason-apis.md"), "utf8"), /file-timestamp.*C617\.1/);
  assert.match(await readFile(join(outputDirectory, "export-compliance.md"), "utf8"), /release\/evidence\/export\.json/);
  assert.match(await readFile(join(outputDirectory, "account-gates.md"), "utf8"), /release\/evidence\/agreements\.json/);
  assert.match(await readFile(join(outputDirectory, "ugc-operations.md"), "utf8"), /Trust operator.*TermsView\.swift/s);
});

test("generateWorksheets requires injected filesystem and hash adapters", async () => {
  const loaded = await sources();
  for (const fs of [null, {}, { mkdir() {} }, {
    mkdir() {}, writeFile() {}, readFile() {}, readdir() {}, rename() {}, rm: "not-a-function",
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
