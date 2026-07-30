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
  generateWorksheets as generateWorksheetsImplementation,
  loadPolicySources,
  processIsAlive,
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

function generateWorksheets(options) {
  return generateWorksheetsImplementation({
    ...options,
    repositoryRoot: options.repositoryRoot ?? dirname(options.outputDirectory),
  });
}

test("real policy sources are schema-valid and truthfully not public-ready", async () => {
  const result = evaluatePolicy(await sources());
  assert(result.some(({ id, state }) => id === "product.identity" && state === "PASS"));
  assert(result.some(({ id, state }) => id === "policy.filtering" && state === "BLOCKED"));
  assert(result.some(({ id, state }) => id === "policy.publicContact" && state === "PASS"));
  assert(result.some(({ id, state }) => id === "policy.reportAcknowledgement" && state === "PASS"));
  assert(result.some(({ id, state }) => id === "policy.imminentHarm" && state === "PASS"));
  assert(result.some(({ id, state }) => id === "policy.objectionableContent" && state === "PASS"));
  assert(result.some(({ id, state }) => id === "url.support" && state === "PASS"));
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
  assert.equal(loaded._urlEvidence.support.state, "current");

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

test("URL evidence requires each exact canonical public URL", async () => {
  const root = await mkdtemp(join(tmpdir(), "riot-url-identity-"));
  await realFs.cp(join(repositoryRoot, "release"), join(root, "release"), { recursive: true });
  const productPath = join(root, "release", "source", "product.json");
  const product = JSON.parse(await realFs.readFile(productPath, "utf8"));
  product.urls.privacy.url = "https://example.com/privacy/";
  await realFs.writeFile(productPath, `${JSON.stringify(product)}\n`, "utf8");
  await assert.rejects(
    () => loadPolicySources({ sourceDirectory: join(root, "release", "source"), fs: realFs }),
    (error) => error.sourceFile.endsWith("/product.json")
      && error.diagnostics[0].pointer === "/urls/privacy/url"
      && error.diagnostics[0].expected === "https://riot.protest.net/privacy/",
  );
});

test("policy evaluation rejects missing network rows and contradictory privacy evidence", async () => {
  const loaded = await sources();
  loaded.networkMatrix.rows = loaded.networkMatrix.rows.filter(({ id }) => id !== "nearby-sync");
  loaded.privacy.answers[0].evidenceRowIds = ["not-present"];
  const gates = evaluatePolicy(loaded);
  assert(gates.some(({ id, state }) => id === "inventory.network-rows" && state === "BLOCKED"));
  assert(gates.some(({ id, state }) => id === "network.nearby-sync" && state === "BLOCKED"));
  assert(gates.some(({ id, state }) => id === "privacy.apple" && state === "BLOCKED"));
});

test("duplicate network rows block inventory and cannot override both store privacy gates", async () => {
  const loaded = await sources();
  const safeDuplicate = structuredClone(loaded.networkMatrix.rows[0]);
  loaded.networkMatrix.rows[0].developerOperated = true;
  loaded.networkMatrix.rows.push(safeDuplicate);
  const states = new Map(evaluatePolicy(loaded).map(({ id, state }) => [id, state]));
  assert.equal(states.get("inventory.network-rows"), "BLOCKED");
  assert.equal(states.get("network.first-launch"), "BLOCKED");
  assert.equal(states.get("privacy.apple"), "BLOCKED");
  assert.equal(states.get("privacy.google"), "BLOCKED");
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

test("worksheet generation rejects symlinked ancestors without touching external files", async () => {
  const loaded = await sources();
  const allowedRoot = await mkdtemp(join(tmpdir(), "riot-worksheet-allowed-"));
  const externalRoot = await mkdtemp(join(tmpdir(), "riot-worksheet-external-"));
  const sentinelPath = join(externalRoot, "sentinel.txt");
  await realFs.writeFile(sentinelPath, "external sentinel\n", "utf8");
  await realFs.mkdir(join(allowedRoot, "release"));
  await realFs.symlink(externalRoot, join(allowedRoot, "release", "generated"));
  const beforeNames = (await readdir(externalRoot)).sort();
  const outputDirectory = join(allowedRoot, "release", "generated", "worksheets");

  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      repositoryRoot: allowedRoot,
      outputDirectory,
      fs: realFs,
      sha256,
    }),
    /symlinked worksheet path|escapes repository/,
  );
  assert.deepEqual((await readdir(externalRoot)).sort(), beforeNames);
  assert.equal(await readFile(sentinelPath, "utf8"), "external sentinel\n");
});

test("worksheet path trust rejects escapes, fake roots, files, and physical-path mismatches", async () => {
  const loaded = await sources();
  const allowedRoot = await mkdtemp(join(tmpdir(), "riot-worksheet-path-root-"));
  const outside = await mkdtemp(join(tmpdir(), "riot-worksheet-path-outside-"));
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      repositoryRoot: allowedRoot,
      outputDirectory: join(allowedRoot, "..", "outside-worksheets"),
      fs: realFs,
      sha256,
    }),
    /escapes repository/,
  );
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      repositoryRoot: allowedRoot,
      outputDirectory: allowedRoot,
      fs: realFs,
      sha256,
    }),
    /escapes repository/,
  );
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      repositoryRoot: "/",
      outputDirectory: join(allowedRoot, "worksheets"),
      fs: realFs,
      sha256,
    }),
    /filesystem root/,
  );

  const linkedRoot = `${allowedRoot}-link`;
  await realFs.symlink(allowedRoot, linkedRoot);
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      repositoryRoot: linkedRoot,
      outputDirectory: join(linkedRoot, "worksheets"),
      fs: realFs,
      sha256,
    }),
    /repository root must be a real directory/,
  );

  const fileRoot = join(allowedRoot, "root-file");
  await realFs.writeFile(fileRoot, "not a directory\n");
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      repositoryRoot: fileRoot,
      outputDirectory: join(fileRoot, "worksheets"),
      fs: realFs,
      sha256,
    }),
    /repository root must be a real directory/,
  );

  const fileAncestor = join(allowedRoot, "release-file");
  await realFs.writeFile(fileAncestor, "not a directory\n");
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      repositoryRoot: allowedRoot,
      outputDirectory: join(fileAncestor, "worksheets"),
      fs: realFs,
      sha256,
    }),
    /non-directory ancestor/,
  );

  const releaseDirectory = join(allowedRoot, "release");
  await realFs.mkdir(releaseDirectory);
  const mismatchedFs = {
    ...realFs,
    async realpath(path) {
      if (path === releaseDirectory) return outside;
      return realFs.realpath(path);
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      repositoryRoot: allowedRoot,
      outputDirectory: join(releaseDirectory, "worksheets"),
      fs: mismatchedFs,
      sha256,
    }),
    /escapes repository/,
  );
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
        if (mode === "names" && String(path).includes(".staging")) return [];
        return realFs.readdir(path, options);
      },
      async readFile(path, options) {
        const bytes = await realFs.readFile(path, options);
        return mode === "bytes" && String(path).includes(".staging") ? `${bytes}corrupt` : bytes;
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
      if (String(path).includes(".backup")) {
        backupRemovals += 1;
        if (backupRemovals === 1) throw new Error("injected cleanup refusal");
      }
      return realFs.rm(path, options);
    },
  };
  await assert.doesNotReject(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: cleanupFs, sha256 }),
  );
  assert.match(await readFile(join(outputDirectory, "accessibility.md"), "utf8"), /iphone \/ read: blocked/);
  assert.doesNotReject(() => realFs.access(`${outputDirectory}.backup`));
  await assert.doesNotReject(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 }),
  );
  await assert.rejects(() => realFs.access(`${outputDirectory}.backup`));
});

test("worksheet generation restores an orphan backup and clears stale swap artifacts", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-recovery-"));
  await generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 });
  const beforeNames = (await readdir(outputDirectory)).sort();
  const before = new Map(await Promise.all(beforeNames.map(async (name) =>
    [name, await readFile(join(outputDirectory, name), "utf8")])));
  const backupDirectory = `${outputDirectory}.backup`;
  const stagingDirectory = `${outputDirectory}.staging`;
  const markerPath = `${outputDirectory}.recovery.json`;
  const lockPath = `${outputDirectory}.lock`;
  await realFs.rename(outputDirectory, backupDirectory);
  await realFs.mkdir(stagingDirectory);
  await realFs.writeFile(join(stagingDirectory, "partial.md"), "partial\n", "utf8");
  await realFs.writeFile(markerPath, `${JSON.stringify({ processId: 4242, phase: "backup-created" })}\n`);
  await realFs.writeFile(lockPath, `${JSON.stringify({ processId: 4242, token: "stale-token" })}\n`);

  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256: () => "short",
      processId: 5252,
      isProcessAlive: () => false,
    }),
    /invalid digest/,
  );
  assert.deepEqual((await readdir(outputDirectory)).sort(), beforeNames);
  for (const [name, bytes] of before) {
    assert.equal(await readFile(join(outputDirectory, name), "utf8"), bytes);
  }
  for (const path of [backupDirectory, stagingDirectory, markerPath, lockPath]) {
    await assert.rejects(() => realFs.access(path), path);
  }
});

test("worksheet generation refuses a live lock without changing the installed set", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-"));
  await generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 });
  const before = await readFile(join(outputDirectory, "accessibility.md"), "utf8");
  const lockPath = `${outputDirectory}.lock`;
  await realFs.writeFile(lockPath, `${JSON.stringify({ processId: 4242, token: "live-token" })}\n`);
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256,
      processId: 5252,
      isProcessAlive: (processId) => processId === 4242,
    }),
    /active worksheet generation lock/,
  );
  assert.equal(await readFile(join(outputDirectory, "accessibility.md"), "utf8"), before);
  assert.doesNotReject(() => realFs.access(lockPath));
});

test("worksheet locks fail closed on ambiguous ownership and filesystem errors", async () => {
  const loaded = await sources();
  for (const owner of [
    "{",
    "{}",
    `${JSON.stringify({ processId: 0, token: "token" })}\n`,
    `${JSON.stringify({ processId: 4242, token: "" })}\n`,
  ]) {
    const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-ambiguous-lock-"));
    const lockPath = `${outputDirectory}.lock`;
    await realFs.writeFile(lockPath, owner);
    await assert.rejects(
      () => generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 }),
      /ambiguous worksheet generation lock/,
    );
    assert.doesNotReject(() => realFs.access(lockPath));
  }

  const deniedDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-open-"));
  const deniedFs = {
    ...realFs,
    async open() {
      const error = new Error("injected lock open refusal");
      error.code = "EACCES";
      throw error;
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory: deniedDirectory,
      fs: deniedFs,
      sha256,
    }),
    /lock open refusal/,
  );

  const writeDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-write-"));
  const writeLockPath = `${writeDirectory}.lock`;
  const writeFs = {
    ...realFs,
    async open(path, flags, mode) {
      const handle = await realFs.open(path, flags, mode);
      return new Proxy(handle, {
        get(target, property) {
          if (property === "writeFile") {
            return async () => {
              throw new Error("injected lock write refusal");
            };
          }
          const value = Reflect.get(target, property, target);
          return typeof value === "function" ? value.bind(target) : value;
        },
      });
    },
    async writeFile(path, bytes, options) {
      if (path === writeLockPath) throw new Error("injected lock write refusal");
      return realFs.writeFile(path, bytes, options);
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory: writeDirectory,
      fs: writeFs,
      sha256,
    }),
    /lock write refusal/,
  );
  await assert.rejects(() => realFs.access(writeLockPath));
});

test("acquisition failure never deletes a same-inode successor token", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-acquire-race-"));
  const lockPath = `${outputDirectory}.lock`;
  const quarantineDirectory = `${lockPath}.quarantine`;
  const token = "attempt-token";
  const successor = `${JSON.stringify({ processId: process.pid, token: "successor-token" })}\n`;
  const replacingFs = {
    ...realFs,
    async open(path, flags, mode) {
      const handle = await realFs.open(path, flags, mode);
      if (path !== lockPath || flags !== "wx") return handle;
      return new Proxy(handle, {
        get(target, property) {
          if (property === "writeFile") {
            return async () => {
              await target.writeFile(successor, "utf8");
              throw new Error("injected acquisition write failure");
            };
          }
          const value = Reflect.get(target, property, target);
          return typeof value === "function" ? value.bind(target) : value;
        },
      });
    },
  };

  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: replacingFs,
      sha256,
      createLockToken: () => token,
    }),
    /acquisition write failure/,
  );
  await assert.rejects(() => realFs.access(lockPath));
  const claimedPath = join(quarantineDirectory, token, "lock");
  assert.equal(await readFile(claimedPath, "utf8"), successor);

  const outputBeforeRetry = await readdir(outputDirectory);
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256,
    }),
    /unresolved worksheet lock quarantine/,
  );
  assert.deepEqual(await readdir(outputDirectory), outputBeforeRetry);
  assert.equal(await readFile(claimedPath, "utf8"), successor);
});

test("empty and unparseable acquisition writes become durable ambiguous claims", async () => {
  const loaded = await sources();
  for (const [name, bytes] of [["empty", ""], ["unparseable", "{"]]) {
    const outputDirectory = await mkdtemp(join(tmpdir(), `riot-worksheet-lock-${name}-write-`));
    const lockPath = `${outputDirectory}.lock`;
    const token = `${name}-token`;
    const failingFs = {
      ...realFs,
      async open(path, flags, mode) {
        const handle = await realFs.open(path, flags, mode);
        if (path !== lockPath || flags !== "wx") return handle;
        return new Proxy(handle, {
          get(target, property) {
            if (property === "writeFile") {
              return async () => {
                await target.writeFile(bytes, "utf8");
                throw new Error(`injected ${name} lock write`);
              };
            }
            const value = Reflect.get(target, property, target);
            return typeof value === "function" ? value.bind(target) : value;
          },
        });
      },
    };
    await assert.rejects(
      () => generateWorksheets({
        sources: loaded,
        outputDirectory,
        fs: failingFs,
        sha256,
        createLockToken: () => token,
      }),
      new RegExp(`${name} lock write`),
    );
    const claimedPath = join(`${lockPath}.quarantine`, token, "lock");
    assert.equal(await readFile(claimedPath, "utf8"), bytes);
    await assert.rejects(
      () => generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 }),
      /unresolved worksheet lock quarantine/,
    );
    assert.deepEqual(await readdir(outputDirectory), []);
    assert.equal(await readFile(claimedPath, "utf8"), bytes);
  }

  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-ambiguous-rename-"));
  const lockPath = `${outputDirectory}.lock`;
  const renameRefusingFs = {
    ...realFs,
    async open(path, flags, mode) {
      const handle = await realFs.open(path, flags, mode);
      if (path !== lockPath || flags !== "wx") return handle;
      return new Proxy(handle, {
        get(target, property) {
          if (property === "writeFile") {
            return async () => {
              await target.writeFile("{", "utf8");
              throw new Error("injected ambiguous lock write");
            };
          }
          const value = Reflect.get(target, property, target);
          return typeof value === "function" ? value.bind(target) : value;
        },
      });
    },
    async rename(from, to) {
      if (from === lockPath && String(to).includes(".lock.quarantine")) {
        throw new Error("injected ambiguous quarantine rename refusal");
      }
      return realFs.rename(from, to);
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: renameRefusingFs,
      sha256,
      createLockToken: () => "rename-refused-token",
    }),
    /ambiguous lock write/,
  );
  assert.equal(await readFile(lockPath, "utf8"), "{");
  assert.deepEqual(await readdir(`${lockPath}.quarantine`), []);
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: realFs, sha256 }),
    /ambiguous worksheet generation lock/,
  );
  assert.deepEqual(await readdir(outputDirectory), []);
});

test("acquisition failure deletes a lock only when its exact owner is still proven", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-exact-cleanup-"));
  const lockPath = `${outputDirectory}.lock`;
  const quarantineDirectory = `${lockPath}.quarantine`;
  let registryCreated = false;
  let registryReads = 0;
  const failingFs = {
    ...realFs,
    async readFile(path, options) {
      const bytes = await realFs.readFile(path, options);
      if (path === lockPath && !registryCreated) {
        registryCreated = true;
        await realFs.mkdir(quarantineDirectory);
      }
      return bytes;
    },
    async readdir(path, options) {
      if (path === quarantineDirectory) {
        registryReads += 1;
        if (registryReads === 1) {
          throw new Error("injected post-acquisition quarantine read refusal");
        }
      }
      return realFs.readdir(path, options);
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: failingFs,
      sha256,
      createLockToken: () => "exact-owner-token",
    }),
    /post-acquisition quarantine read refusal/,
  );
  await assert.rejects(() => realFs.access(lockPath));
  assert.deepEqual(await readdir(quarantineDirectory), []);
  assert.deepEqual(await readdir(outputDirectory), []);
});

test("worksheet locks reject unreadable metadata and stale-lock replacement races", async () => {
  const loaded = await sources();
  const malformedDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-malformed-"));
  const malformedLockPath = `${malformedDirectory}.lock`;
  const malformedFs = {
    ...realFs,
    async readFile(path, options) {
      if (path === malformedLockPath) return "{";
      return realFs.readFile(path, options);
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory: malformedDirectory,
      fs: malformedFs,
      sha256,
    }),
    /worksheet lock ownership changed/,
  );
  await assert.rejects(() => realFs.access(malformedLockPath));

  const staleDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-stale-race-"));
  const staleLockPath = `${staleDirectory}.lock`;
  const displacedPath = `${staleLockPath}.displaced`;
  const staleOwner = `${JSON.stringify({ processId: 4242, token: "stale-token" })}\n`;
  const successor = `${JSON.stringify({ processId: process.pid, token: "successor-token" })}\n`;
  await realFs.writeFile(staleLockPath, staleOwner, "utf8");
  let reads = 0;
  const replacingFs = {
    ...realFs,
    async readFile(path, options) {
      if (path === staleLockPath) {
        reads += 1;
        if (reads === 2) {
          await realFs.rename(staleLockPath, displacedPath);
          await realFs.writeFile(staleLockPath, successor, "utf8");
          return successor;
        }
      }
      return realFs.readFile(path, options);
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory: staleDirectory,
      fs: replacingFs,
      sha256,
      isProcessAlive: () => false,
    }),
    /worksheet lock ownership changed/,
  );
  assert.equal(await readFile(staleLockPath, "utf8"), successor);
  assert.doesNotReject(() => realFs.access(displacedPath));

  const readRaceDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-read-race-"));
  const readRaceLockPath = `${readRaceDirectory}.lock`;
  const readRaceDisplaced = `${readRaceLockPath}.displaced`;
  await realFs.writeFile(readRaceLockPath, staleOwner, "utf8");
  const readRaceFs = {
    ...realFs,
    async open(path, flags, mode) {
      const handle = await realFs.open(path, flags, mode);
      if (path !== readRaceLockPath || flags !== "r") return handle;
      return new Proxy(handle, {
        get(target, property) {
          if (property === "readFile") {
            return async (...args) => {
              const bytes = await target.readFile(...args);
              await realFs.rename(readRaceLockPath, readRaceDisplaced);
              await realFs.writeFile(readRaceLockPath, successor, "utf8");
              return bytes;
            };
          }
          const value = Reflect.get(target, property, target);
          return typeof value === "function" ? value.bind(target) : value;
        },
      });
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory: readRaceDirectory,
      fs: readRaceFs,
      sha256,
      isProcessAlive: () => false,
    }),
    /worksheet lock ownership changed/,
  );
  assert.equal(await readFile(readRaceLockPath, "utf8"), successor);
  assert.doesNotReject(() => realFs.access(readRaceDisplaced));
});

test("worksheet lock ownership is bound to the exclusively opened file", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-identity-"));
  const lockPath = `${outputDirectory}.lock`;
  const displacedPath = `${lockPath}.displaced`;
  const successor = `${JSON.stringify({ processId: process.pid, token: "successor-token" })}\n`;
  let replaced = false;
  const replacingFs = {
    ...realFs,
    async open(path, flags, mode) {
      const handle = await realFs.open(path, flags, mode);
      if (path === lockPath && flags === "wx" && !replaced) {
        replaced = true;
        await realFs.rename(lockPath, displacedPath);
        await realFs.writeFile(lockPath, successor, "utf8");
      }
      return handle;
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: replacingFs,
      sha256: () => "short",
    }),
    /worksheet lock ownership changed/,
  );
  assert.equal(await readFile(lockPath, "utf8"), successor);
  assert.doesNotReject(() => realFs.access(displacedPath));
});

test("worksheet cleanup refuses to unlink a successor lock", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-successor-"));
  const lockPath = `${outputDirectory}.lock`;
  const displacedPath = `${lockPath}.displaced`;
  const successor = `${JSON.stringify({ processId: process.pid, token: "successor-token" })}\n`;
  let replaced = false;
  const replacingFs = {
    ...realFs,
    async rename(from, to) {
      await realFs.rename(from, to);
      if (from === `${outputDirectory}.staging` && to === outputDirectory && !replaced) {
        replaced = true;
        await realFs.rename(lockPath, displacedPath);
        await realFs.writeFile(lockPath, successor, "utf8");
      }
    },
  };
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: replacingFs, sha256 }),
    /worksheet lock ownership changed/,
  );
  assert.equal(await readFile(lockPath, "utf8"), successor);
  assert.doesNotReject(() => realFs.access(displacedPath));
});

test("worksheet cleanup atomically preserves a successor installed after validation", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-validation-race-"));
  const lockPath = `${outputDirectory}.lock`;
  const displacedPath = `${lockPath}.displaced`;
  const successor = `${JSON.stringify({ processId: process.pid, token: "successor-token" })}\n`;
  let lockReads = 0;
  const replacingFs = {
    ...realFs,
    async readFile(path, options) {
      const bytes = await realFs.readFile(path, options);
      if (path === lockPath) {
        lockReads += 1;
        if (lockReads === 2) {
          await realFs.rename(lockPath, displacedPath);
          await realFs.writeFile(lockPath, successor, "utf8");
        }
      }
      return bytes;
    },
  };
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: replacingFs, sha256 }),
    /worksheet lock ownership changed/,
  );
  assert.equal(await readFile(lockPath, "utf8"), successor);
  assert.doesNotReject(() => realFs.access(displacedPath));
});

test("stale lock quarantine collisions fail closed without overwriting either artifact", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-quarantine-"));
  const lockPath = `${outputDirectory}.lock`;
  const owner = { processId: 4242, token: "stale-token" };
  const ownerBytes = `${JSON.stringify(owner)}\n`;
  const quarantineDirectory = `${lockPath}.quarantine`;
  const claimDirectory = join(quarantineDirectory, owner.token);
  const quarantineSentinel = join(claimDirectory, "sentinel.txt");
  await realFs.writeFile(lockPath, ownerBytes, "utf8");
  let collided = false;
  const collidingFs = {
    ...realFs,
    async mkdir(path, options) {
      if (path === claimDirectory && !collided) {
        collided = true;
        await realFs.mkdir(claimDirectory, { recursive: true });
        await realFs.writeFile(quarantineSentinel, "orphan claim\n", "utf8");
      }
      return realFs.mkdir(path, options);
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: collidingFs,
      sha256,
      isProcessAlive: () => false,
    }),
    /quarantine collision/,
  );
  assert.equal(await readFile(lockPath, "utf8"), ownerBytes);
  assert.equal(await readFile(quarantineSentinel, "utf8"), "orphan claim\n");
});

test("worksheet quarantine registry rejects symlinks, non-directories, and unresolved entries", async () => {
  const loaded = await sources();
  for (const mode of ["symlink", "file", "unresolved"]) {
    const repositoryRoot = await mkdtemp(join(tmpdir(), `riot-worksheet-registry-${mode}-`));
    const outputDirectory = join(repositoryRoot, "release", "generated");
    const quarantineDirectory = `${outputDirectory}.lock.quarantine`;
    const sentinel = join(await mkdtemp(join(tmpdir(), "riot-worksheet-registry-outside-")), "sentinel");
    await realFs.mkdir(dirname(outputDirectory), { recursive: true });
    await realFs.writeFile(sentinel, "external\n", "utf8");
    if (mode === "symlink") {
      await realFs.symlink(dirname(sentinel), quarantineDirectory);
    } else if (mode === "file") {
      await realFs.writeFile(quarantineDirectory, "not a registry\n", "utf8");
    } else {
      await realFs.mkdir(join(quarantineDirectory, "unknown-claim"), { recursive: true });
    }
    await assert.rejects(
      () => generateWorksheets({
        sources: loaded,
        repositoryRoot,
        outputDirectory,
        fs: realFs,
        sha256,
      }),
      mode === "unresolved"
        ? /unresolved worksheet lock quarantine/
        : /quarantine registry|symlinked worksheet path/,
    );
    assert.equal(await readFile(sentinel, "utf8"), "external\n");
    await assert.rejects(() => realFs.access(`${outputDirectory}.lock`));
    await assert.rejects(() => realFs.access(outputDirectory));
  }
});

test("quarantine claims fail closed on malformed claims and filesystem refusals", async () => {
  const loaded = await sources();
  const malformedDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-claim-malformed-"));
  const malformedLockPath = `${malformedDirectory}.lock`;
  let malformedReads = 0;
  const malformedFs = {
    ...realFs,
    async readFile(path, options) {
      const bytes = await realFs.readFile(path, options);
      if (path === malformedLockPath) {
        malformedReads += 1;
        if (malformedReads === 2) await realFs.writeFile(path, "{", "utf8");
      }
      return bytes;
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory: malformedDirectory,
      fs: malformedFs,
      sha256,
    }),
    /worksheet lock ownership changed/,
  );
  assert.equal(await readFile(malformedLockPath, "utf8"), "{");

  for (const mode of ["mkdir", "claim-mkdir", "rename"]) {
    const outputDirectory = await mkdtemp(join(tmpdir(), `riot-worksheet-claim-${mode}-`));
    const lockPath = `${outputDirectory}.lock`;
    const quarantineDirectory = `${lockPath}.quarantine`;
    const refusingFs = {
      ...realFs,
      async mkdir(path, options) {
        if ((mode === "mkdir" && path === quarantineDirectory)
          || (mode === "claim-mkdir" && dirname(path) === quarantineDirectory)) {
          const error = new Error("injected quarantine mkdir refusal");
          error.code = "EACCES";
          throw error;
        }
        return realFs.mkdir(path, options);
      },
      async rename(from, to) {
        if (mode === "rename" && from === lockPath && String(to).includes(".lock.quarantine")) {
          const error = new Error("injected quarantine rename refusal");
          error.code = "EACCES";
          throw error;
        }
        return realFs.rename(from, to);
      },
    };
    await assert.rejects(
      () => generateWorksheets({ sources: loaded, outputDirectory, fs: refusingFs, sha256 }),
      new RegExp(`quarantine ${mode === "claim-mkdir" ? "mkdir" : mode} refusal`),
    );
    assert.doesNotReject(() => realFs.access(lockPath));
    if (mode === "mkdir") {
      await assert.rejects(() => realFs.access(quarantineDirectory));
    } else {
      assert.deepEqual(await readdir(quarantineDirectory), []);
    }
  }
});

test("an unowned atomic claim is preserved when safe restoration is refused", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-claim-restore-"));
  const lockPath = `${outputDirectory}.lock`;
  const displacedPath = `${lockPath}.displaced`;
  const successor = `${JSON.stringify({ processId: process.pid, token: "successor-token" })}\n`;
  let reads = 0;
  const refusingFs = {
    ...realFs,
    async readFile(path, options) {
      const bytes = await realFs.readFile(path, options);
      if (path === lockPath) {
        reads += 1;
        if (reads === 2) {
          await realFs.rename(lockPath, displacedPath);
          await realFs.writeFile(lockPath, successor, "utf8");
        }
      }
      return bytes;
    },
    async link() {
      const error = new Error("injected safe restore refusal");
      error.code = "EEXIST";
      throw error;
    },
  };
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: refusingFs, sha256 }),
    /worksheet lock ownership changed/,
  );
  await assert.rejects(() => realFs.access(lockPath));
  const quarantineDirectory = `${lockPath}.quarantine`;
  const [claimName] = await readdir(quarantineDirectory);
  assert(claimName);
  const claimedPath = join(quarantineDirectory, claimName, "lock");
  assert.equal(
    await readFile(claimedPath, "utf8"),
    successor,
  );
  assert.doesNotReject(() => realFs.access(displacedPath));

  const installedBeforeRetry = await Promise.all(
    expectedWorksheets.map(async (name) => [name, await readFile(join(outputDirectory, name), "utf8")]),
  );
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256: () => "b".repeat(64),
    }),
    /unresolved worksheet lock quarantine/,
  );
  assert.deepEqual(
    await Promise.all(
      expectedWorksheets.map(async (name) => [name, await readFile(join(outputDirectory, name), "utf8")]),
    ),
    installedBeforeRetry,
  );
  assert.equal(
    await readFile(claimedPath, "utf8"),
    successor,
  );
});

test("worksheet recovery fails closed on unreadable swap state", async () => {
  const loaded = await sources();
  const outputDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-readdir-"));
  const failingFs = {
    ...realFs,
    async readdir(path, options) {
      if (path === outputDirectory) {
        const error = new Error("injected swap state refusal");
        error.code = "EACCES";
        throw error;
      }
      return realFs.readdir(path, options);
    },
  };
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory, fs: failingFs, sha256 }),
    /swap state refusal/,
  );
  await assert.rejects(() => realFs.access(`${outputDirectory}.lock`));

  const lstatDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lstat-"));
  const lstatFs = {
    ...realFs,
    async lstat(path, options) {
      if (path === lstatDirectory) {
        const error = new Error("injected lstat refusal");
        error.code = "EACCES";
        throw error;
      }
      return realFs.lstat(path, options);
    },
  };
  await assert.rejects(
    () => generateWorksheets({ sources: loaded, outputDirectory: lstatDirectory, fs: lstatFs, sha256 }),
    /lstat refusal/,
  );
});

test("process liveness checks distinguish current, absent, and invalid process IDs", () => {
  assert.equal(processIsAlive(process.pid), true);
  assert.equal(processIsAlive(2_147_483_647), false);
  assert.throws(() => processIsAlive(Number.NaN));
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
    () => generateWorksheetsImplementation({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256: () => "short",
    }),
    /repositoryRoot/,
  );
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256: () => "short",
    }),
    /invalid digest/,
  );
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256,
      processId: 0,
    }),
    /processId/,
  );
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256,
      isProcessAlive: null,
    }),
    /liveness/,
  );
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256,
      createLockToken: null,
    }),
    /lock token adapters/,
  );
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory,
      fs: realFs,
      sha256,
      createLockToken: () => "",
    }),
    /non-empty string/,
  );

  const closeDirectory = await mkdtemp(join(tmpdir(), "riot-worksheet-lock-close-"));
  const closeFs = {
    ...realFs,
    async open(path, flags, mode) {
      const handle = await realFs.open(path, flags, mode);
      if (flags !== "wx") return handle;
      return new Proxy(handle, {
        get(target, property) {
          if (property === "close") {
            return async () => {
              await target.close();
              throw new Error("injected lock close refusal");
            };
          }
          const value = Reflect.get(target, property, target);
          return typeof value === "function" ? value.bind(target) : value;
        },
      });
    },
  };
  await assert.rejects(
    () => generateWorksheets({
      sources: loaded,
      outputDirectory: closeDirectory,
      fs: closeFs,
      sha256,
    }),
    /lock close refusal/,
  );
  await assert.rejects(() => realFs.access(`${closeDirectory}.lock`));
});
