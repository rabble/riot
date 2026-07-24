import { randomUUID } from "node:crypto";
import { readFile, readdir, realpath } from "node:fs/promises";
import { dirname, isAbsolute, join, normalize, relative, resolve, sep } from "node:path";

import { canonicalJson } from "./canonical-json.mjs";
import { loadSchemaRegistry, releaseDiagnosticError, validateSource } from "./schema.mjs";

const SOURCES = Object.freeze({
  accessibility: "accessibility",
  accountGates: "account-gates",
  claims: "claims",
  exportCompliance: "export-compliance",
  networkMatrix: "network-matrix",
  policy: "policy",
  privacy: "privacy",
  product: "product",
  reviewInstructions: "review-instructions",
});
const NETWORK_ROWS = ["first-launch", "denied-permission", "granted-permission", "nearby-sync", "followed-site-refresh"];
const REQUIRED_PERMISSIONS = ["camera", "bluetooth", "local-network", "notifications", "android-internet"];
const STORE_PLATFORMS = Object.freeze({
  apple: ["ios", "ipados", "macos"],
  google: ["android"],
});
const URL_EVIDENCE_PATHS = Object.freeze({
  privacy: "marketing/privacy/index.html",
  support: "marketing/support/index.html",
  marketing: "marketing/releases/index.html",
});
const REVIEW_TOPICS = ["first-launch", "demo-content", "no-login", "create", "join", "publish", "restart", "offline", "local-permissions", "nearby-testing", "permission-denial", "invalid-join", "no-peers"];
const DEVICES = ["iphone", "ipad", "mac", "android-phone", "android-tablet"];
const TASKS = ["read", "create", "join", "publish"];
const ACCOUNT_GATES = ["agreements", "tax", "banking", "trader-status", "signing", "hardware", "console"];
const WORKSHEETS = Object.freeze({
  "accessibility.md": ["accessibility"],
  "account-gates.md": ["accountGates"],
  "app-privacy.md": ["privacy", "networkMatrix"],
  "content-rating.md": ["policy", "claims"],
  "data-safety.md": ["privacy", "networkMatrix"],
  "export-compliance.md": ["exportCompliance"],
  "outbound-network.md": ["networkMatrix"],
  "permissions.md": ["privacy", "networkMatrix"],
  "required-reason-apis.md": ["privacy", "networkMatrix"],
  "review-instructions.md": ["reviewInstructions"],
  "ugc-operations.md": ["policy"],
});

function gate(id, state, sourceFile, pointer, observed, expected, recovery) {
  return { id, state, sourceFile, pointer, observed, expected, recovery };
}

export async function loadPolicySources({ sourceDirectory, fs = { readFile, readdir, realpath } }) {
  if (!sourceDirectory || typeof fs?.readFile !== "function" || typeof fs?.realpath !== "function") throw new TypeError("sourceDirectory and fs are required");
  const schemaDirectory = join(dirname(sourceDirectory), "schemas");
  const registry = await loadSchemaRegistry(schemaDirectory, fs);
  const loaded = {};
  const files = {};
  for (const [key, name] of Object.entries(SOURCES)) {
    const sourceFile = join(sourceDirectory, `${name}.json`);
    let value;
    try {
      value = JSON.parse(await fs.readFile(sourceFile, "utf8"));
    } catch (error) {
      throw releaseDiagnosticError(`${sourceFile}: malformed or missing source: ${error.message}`, {
        sourceFile,
        observed: `malformed or missing: ${error.message}`,
        expected: "valid canonical source JSON",
        keyword: "source",
      });
    }
    try {
      loaded[key] = structuredClone(validateSource(registry, name, value));
    } catch (error) {
      error.sourceFile = sourceFile;
      throw error;
    }
    files[key] = sourceFile;
  }
  const repositoryRoot = dirname(dirname(sourceDirectory));
  const repositoryRealPath = await fs.realpath(repositoryRoot);
  const urlEvidence = {};
  for (const [name, value] of Object.entries(loaded.product.urls)) {
    const evidencePath = value.evidencePath;
    const normalized = normalize(evidencePath);
    if (isAbsolute(evidencePath)
      || evidencePath.includes("\\")
      || normalized !== evidencePath
      || evidencePath !== URL_EVIDENCE_PATHS[name]) {
      throw releaseDiagnosticError(`unsafe or mismatched URL evidence path: ${evidencePath}`, {
        sourceFile: files.product,
        pointer: `/urls/${name}/evidencePath`,
        observed: evidencePath,
        expected: `normalized repository-relative ${URL_EVIDENCE_PATHS[name]} page evidence`,
        keyword: "path",
      });
    }
    let state;
    try {
      const targetPath = join(repositoryRoot, evidencePath);
      const targetRealPath = await fs.realpath(targetPath);
      const fromRepository = relative(repositoryRealPath, targetRealPath);
      if (fromRepository === ".." || fromRepository.startsWith(`..${sep}`) || isAbsolute(fromRepository)) {
        throw releaseDiagnosticError(`URL evidence escapes repository: ${evidencePath}`, {
          sourceFile: files.product,
          pointer: `/urls/${name}/evidencePath`,
          observed: targetRealPath,
          expected: "evidence whose real path remains inside the repository",
          keyword: "path",
        });
      }
      const content = await fs.readFile(targetRealPath, "utf8");
      const localState = /<!doctype\s+html|<html(?:\s|>)/i.test(content) ? "current" : "stale";
      state = value.evidenceState === "current" ? localState : value.evidenceState;
    } catch (error) {
      if (error.diagnostics) throw error;
      state = "missing";
    }
    urlEvidence[name] = { state, evidencePath };
  }
  Object.defineProperty(loaded, "_files", { value: files, enumerable: false });
  Object.defineProperty(loaded, "_urlEvidence", { value: urlEvidence, enumerable: false });
  return loaded;
}

export function evaluatePolicy(sources) {
  const gates = [];
  const file = (key) => sources._files?.[key] ?? `release/source/${SOURCES[key]}.json`;
  const productMatches = sources.product.version === "1.0"
    && sources.product.price === "free"
    && sources.product.availability === "worldwide"
    && sources.product.releaseChannel === "public-early-access"
    && sources.product.appleBundleId === "net.protest.riot"
    && sources.product.androidApplicationId === "net.protest.riot";
  gates.push(gate("product.identity", productMatches ? "PASS" : "BLOCKED", file("product"), "/", JSON.stringify(sources.product), "1.0, free, worldwide, public early access, net.protest.riot", "Correct release/source/product.json."));

  const answerCounts = new Map();
  for (const { store } of sources.privacy.answers) answerCounts.set(store, (answerCounts.get(store) ?? 0) + 1);
  const exactAnswers = sources.privacy.answers.length === 2
    && answerCounts.get("apple") === 1
    && answerCounts.get("google") === 1;
  gates.push(gate("inventory.privacy-answers", exactAnswers ? "PASS" : "BLOCKED", file("privacy"), "/answers", JSON.stringify(Object.fromEntries(answerCounts)), "exactly one Apple and one Google privacy answer", "Restore the exact canonical Apple and Google answer inventory."));

  const accountCounts = new Map();
  for (const { name } of sources.accountGates.gates) accountCounts.set(name, (accountCounts.get(name) ?? 0) + 1);
  const exactAccounts = sources.accountGates.gates.length === ACCOUNT_GATES.length
    && ACCOUNT_GATES.every((name) => accountCounts.get(name) === 1);
  gates.push(gate("inventory.account-gates", exactAccounts ? "PASS" : "BLOCKED", file("accountGates"), "/gates", JSON.stringify(Object.fromEntries(accountCounts)), `exactly one of: ${ACCOUNT_GATES.join(", ")}`, "Restore the exact canonical account/legal gate inventory."));

  for (const [name, value] of Object.entries(sources.product.urls)) {
    const verified = sources._urlEvidence?.[name]?.state ?? "missing";
    gates.push(gate(`url.${name}`, verified === "current" ? "PASS" : "BLOCKED", file("product"), `/urls/${name}/evidencePath`, `${verified}: ${value.evidencePath}`, "current non-empty HTML at the declared repository path", `Publish and verify ${value.evidencePath}.`));
  }

  const rowsById = new Map();
  for (const [index, row] of sources.networkMatrix.rows.entries()) {
    const matches = rowsById.get(row.id) ?? [];
    matches.push({ row, index });
    rowsById.set(row.id, matches);
  }
  const exactNetworkRows = sources.networkMatrix.rows.length === NETWORK_ROWS.length
    && NETWORK_ROWS.every((id) => rowsById.get(id)?.length === 1);
  const observedNetworkRows = Object.fromEntries(
    NETWORK_ROWS.map((id) => [id, rowsById.get(id)?.length ?? 0]),
  );
  gates.push(gate("inventory.network-rows", exactNetworkRows ? "PASS" : "BLOCKED", file("networkMatrix"), "/rows", JSON.stringify(observedNetworkRows), `exactly one of: ${NETWORK_ROWS.join(", ")}`, "Restore the exact canonical outbound-network scenario inventory."));
  for (const row of NETWORK_ROWS) {
    const matches = rowsById.get(row) ?? [];
    const index = matches[0]?.index;
    gates.push(gate(`network.${row}`, matches.length === 1 ? "PASS" : "BLOCKED", file("networkMatrix"), matches.length === 1 ? `/rows/${index}` : "/rows", matches.length, "exactly one canonical evidence row", `Add exactly one ${row} evidence row and remove duplicates or substitutions.`));
  }
  for (const [index, answer] of sources.privacy.answers.entries()) {
    const rows = answer.evidenceRowIds.map((id) => {
      const matches = rowsById.get(id) ?? [];
      return matches.length === 1 ? matches[0].row : undefined;
    });
    const evidenceComplete = exactNetworkRows && rows.every(Boolean);
    const scenarioComplete = answer.evidenceRowIds.length === NETWORK_ROWS.length
      && NETWORK_ROWS.every((id) => answer.evidenceRowIds.includes(id));
    const platformComplete = evidenceComplete && rows.every((row) =>
      STORE_PLATFORMS[answer.store].every((platform) => row.platformCodePaths[platform].length > 0));
    const positionConsistent = Object.values(sources.privacy.position).every(Boolean);
    const networkConsistent = evidenceComplete && rows.every((row) =>
      !row.developerOperated
      && ["none", "peer-local", "destination-controlled"].includes(row.developerRetention)
      && (row.transmittedFields.length === 0 || row.userDirected));
    const semanticallyConsistent = answer.answer !== "no-collection"
      || positionConsistent && networkConsistent;
    gates.push(gate(`privacy.${answer.store}`, evidenceComplete && scenarioComplete && platformComplete && semanticallyConsistent ? "PASS" : "BLOCKED", file("privacy"), `/answers/${index}`, answer.evidenceRowIds.join(","), "all exact scenarios with store-platform code evidence agree with position, fields, destination ownership, retention, and user direction", "Correct the store answer or its canonical platform-specific network evidence."));
  }
  const permissionCounts = new Map();
  for (const { name } of sources.privacy.permissions) permissionCounts.set(name, (permissionCounts.get(name) ?? 0) + 1);
  for (const permission of REQUIRED_PERMISSIONS) {
    const count = permissionCounts.get(permission) ?? 0;
    const index = sources.privacy.permissions.findIndex(({ name }) => name === permission);
    gates.push(gate(`permission.${permission}`, count === 1 ? "PASS" : "BLOCKED", file("privacy"), count === 1 ? `/permissions/${index}` : "/permissions", count, "exactly one canonical permission entry", `Add exactly one ${permission} permission justification and remove substitutions or duplicates.`));
  }
  gates.push(gate("privacy.required-reason-audit", sources.privacy.requiredReasonApis.length > 0 ? "PASS" : "HUMAN ACTION", file("privacy"), "/requiredReasonApis", sources.privacy.requiredReasonApis.length, "completed submitted-dependency API inventory", "Complete the Apple required-reason API audit before archive production."));

  for (const [name, control] of Object.entries(sources.policy.controls)) {
    const requiredHours = name === "reportAcknowledgement" || name === "imminentHarm" ? 24 : name === "objectionableContent" ? 72 : null;
    const complete = control.state === "pass"
      && control.codePaths.length > 0
      && control.operator.length > 0
      && (requiredHours === null || control.maxHours <= requiredHours);
    gates.push(gate(`policy.${name}`, complete ? "PASS" : "BLOCKED", file("policy"), `/controls/${name}`, control.reason, "implemented control with evidence, owner, and required SLA", `Implement and evidence ${name} in a separately approved product workstream.`));
  }

  const reviewIndexes = new Map(sources.reviewInstructions.topics.map(({ id }, index) => [id, index]));
  for (const topic of REVIEW_TOPICS) {
    const index = reviewIndexes.get(topic);
    gates.push(gate(`review.${topic}`, index === undefined ? "BLOCKED" : "PASS", file("reviewInstructions"), index === undefined ? "/topics" : `/topics/${index}`, index === undefined ? "missing" : "present", "present", `Add truthful review instructions for ${topic}.`));
  }

  const accessibility = new Map(sources.accessibility.records.map((record, index) => [`${record.device}:${record.task}`, { record, index }]));
  for (const device of DEVICES) {
    for (const task of TASKS) {
      const match = accessibility.get(`${device}:${task}`);
      const record = match?.record;
      const state = !record || record.state === "blocked" || record.state === "pass" && !record.evidencePath
        ? "BLOCKED"
        : "HUMAN ACTION";
      gates.push(gate(`accessibility.${device}.${task}`, state, file("accessibility"), match ? `/records/${match.index}` : "/records", record?.state ?? "missing", "passing candidate-bound evidence", `Run and record the ${task} accessibility rehearsal on ${device}.`));
    }
  }

  for (const [index, claim] of sources.claims.claims.entries()) {
    const codeEvidenceComplete = claim.platforms.every((platform) => {
      const evidence = claim.evidenceByPlatform?.[platform];
      return evidence?.codePaths.length > 0;
    });
    const candidateEvidenceComplete = claim.platforms.every((platform) => {
      const evidence = claim.evidenceByPlatform?.[platform];
      const candidate = evidence?.candidateJourney;
      return candidate?.candidateId.length > 0
        && evidence.journeyIds.includes(candidate.journeyId)
        && candidate.result === "PASS"
        && /^[0-9a-f]{64}$/.test(candidate.evidenceDigest);
    });
    const state = !codeEvidenceComplete
      ? "BLOCKED"
      : claim.state === "pass"
        ? candidateEvidenceComplete ? "HUMAN ACTION" : "BLOCKED"
        : claim.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
    const journeys = claim.platforms.flatMap((platform) => claim.evidenceByPlatform?.[platform]?.journeyIds ?? []);
    gates.push(gate(`claim.${claim.id}`, state, file("claims"), `/claims/${index}`, claim.state, "platform-specific code and candidate journey evidence for every claimed platform", `Run candidate journeys ${journeys.join(", ")}.`));
  }

  const contentRating = sources.policy.contentRating;
  const contentRatingState = contentRating.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
  gates.push(gate("content-rating", contentRatingState, file("policy"), "/contentRating", `${contentRating.apple}; ${contentRating.google}`, "authenticated store questionnaires confirm the canonical recommendation", "Confirm the content-rating questionnaires in App Store Connect and Google Play Console."));

  const classification = sources.exportCompliance.classification;
  const exportState = classification.state === "blocked"
    || classification.state === "pass" && (!classification.approver || classification.evidence.length === 0)
    ? "BLOCKED"
    : "HUMAN ACTION";
  gates.push(gate("export.classification", exportState, file("exportCompliance"), "/classification", classification.state, "authenticated approved classification", "Obtain legal/export approval and record its evidence."));

  for (const [index, account] of sources.accountGates.gates.entries()) {
    const state = account.state === "blocked" || account.state === "pass" && account.evidence.length === 0
      ? "BLOCKED"
      : "HUMAN ACTION";
    gates.push(gate(`account.${account.name}`, state, file("accountGates"), `/gates/${index}`, account.state, "authenticated current evidence", `Confirm ${account.name} in the authenticated store account.`));
  }
  return gates;
}

function gateSummary(keys, sources) {
  const gates = evaluatePolicy(sources).filter(({ sourceFile }) =>
    keys.some((key) => sourceFile.endsWith(`${SOURCES[key]}.json`)));
  return gates.map(({ id, state, observed, expected, recovery }) =>
    `## ${id}\n\n- State: **${state}**\n- Observed: ${observed}\n- Expected: ${expected}\n- Recovery: ${recovery}\n`).join("\n");
}

function permissionWorksheet(sources) {
  const rows = sources.privacy.permissions.map(({ name, platform, justification, evidencePath }) =>
    `| ${name} | ${platform} | ${justification} | ${evidencePath} |`).join("\n");
  return `## Permission justification inventory\n\n| Permission ID | Platform | Store justification | Code evidence |\n| --- | --- | --- | --- |\n${rows}\n`;
}

function requiredReasonWorksheet(sources) {
  const entries = sources.privacy.requiredReasonApis;
  if (entries.length === 0) {
    return "## Required-reason API inventory\n\nNo approved API reasons are recorded. Audit the submitted Apple archive and all dependencies before production.\n";
  }
  const rows = entries.map(({ api, reason, evidencePath }) => `| ${api} | ${reason} | ${evidencePath} |`).join("\n");
  return `## Required-reason API inventory\n\n| API | Approved reason | Evidence |\n| --- | --- | --- |\n${rows}\n`;
}

function networkWorksheet(sources) {
  return sources.networkMatrix.rows.map((row) => `## ${row.id}

- Initiator: ${row.initiator}
- Destination: ${row.destinationClass}
- Transmitted fields: ${row.transmittedFields.length ? row.transmittedFields.join(", ") : "none"}
- Redirect handling: ${row.redirectHandling}
- Retention: ${row.retention}
- Developer operated: ${row.developerOperated}
- Developer retention: ${row.developerRetention}
- User directed: ${row.userDirected}
- iOS code evidence: ${row.platformCodePaths.ios.join(", ")}
- iPadOS code evidence: ${row.platformCodePaths.ipados.join(", ")}
- macOS code evidence: ${row.platformCodePaths.macos.join(", ")}
- Android code evidence: ${row.platformCodePaths.android.join(", ")}
`).join("\n");
}

function reviewWorksheet(sources) {
  return sources.reviewInstructions.topics.map(({ id, instruction, evidencePath }) => `## ${id}

- Instruction: ${instruction}
- Evidence: ${evidencePath}
`).join("\n");
}

function exportWorksheet(sources) {
  const { algorithms, distribution, classification } = sources.exportCompliance;
  return `## Algorithms\n\n${algorithms.map((algorithm) => `- ${algorithm}`).join("\n")}

## Distribution

${distribution}

## Classification

- State: ${classification.state}
- Reason: ${classification.reason}
- Approver: ${classification.approver ?? "not recorded"}
- Evidence: ${classification.evidence.length ? classification.evidence.join(", ") : "not recorded"}
`;
}

function contentRatingWorksheet(sources) {
  const { contentRating } = sources.policy;
  const claims = sources.claims.claims.map(({ text, platforms, state }) =>
    `- ${text} (${platforms.join(", ")}; ${state})`).join("\n");
  return `## Recommended ratings

- Apple: ${contentRating.apple}
- Google Play: ${contentRating.google}
- State: ${contentRating.state}
- Rationale: ${contentRating.rationale}

## Claims considered

${claims}
`;
}

function privacyWorksheet(sources, store) {
  const answer = sources.privacy.answers.find(({ store: candidate }) => candidate === store);
  return `## ${store === "apple" ? "App Privacy" : "Google Play Data safety"} position

- Store answer: ${answer.answer}
- Evidence rows: ${answer.evidenceRowIds.join(", ")}
- No developer account: ${sources.privacy.position.noDeveloperAccount}
- No ads, tracking, or developer analytics: ${sources.privacy.position.noAdsTrackingAnalytics}
- No hidden collection: ${sources.privacy.position.noHiddenCollection}

${networkWorksheet(sources)}`;
}

function accessibilityWorksheet(sources) {
  return sources.accessibility.records.map(({ device, task, state, evidencePath }) =>
    `- ${device} / ${task}: ${state}; evidence: ${evidencePath ?? "not recorded"}`).join("\n");
}

function accountWorksheet(sources) {
  return sources.accountGates.gates.map(({ name, state, reason, evidence }) =>
    `- ${name}: ${state}; ${reason}; evidence: ${evidence.length ? evidence.join(", ") : "not recorded"}`).join("\n");
}

function ugcWorksheet(sources) {
  return Object.entries(sources.policy.controls).map(([name, control]) => `## ${name}

- State: ${control.state}
- Reason: ${control.reason}
- Operator: ${control.operator || "not assigned"}
- Maximum response hours: ${control.maxHours ?? "not applicable"}
- Code evidence: ${control.codePaths.length ? control.codePaths.join(", ") : "not implemented"}
`).join("\n");
}

function canonicalWorksheet(name, sources) {
  const renderers = {
    "accessibility.md": accessibilityWorksheet,
    "account-gates.md": accountWorksheet,
    "app-privacy.md": (value) => privacyWorksheet(value, "apple"),
    "content-rating.md": contentRatingWorksheet,
    "data-safety.md": (value) => privacyWorksheet(value, "google"),
    "export-compliance.md": exportWorksheet,
    "outbound-network.md": networkWorksheet,
    "permissions.md": permissionWorksheet,
    "required-reason-apis.md": requiredReasonWorksheet,
    "review-instructions.md": reviewWorksheet,
    "ugc-operations.md": ugcWorksheet,
  };
  return renderers[name](sources);
}

function markdown(name, keys, sources, digest) {
  const title = name.replace(/\.md$/, "").replaceAll("-", " ");
  return `<!-- source-sha256: ${digest} -->\n# ${title}\n\n${canonicalWorksheet(name, sources)}\n# Gate summary\n\n${gateSummary(keys, sources)}`;
}

export function processIsAlive(processId) {
  try {
    process.kill(processId, 0);
    return true;
  } catch (error) {
    if (error.code === "ESRCH") return false;
    throw error;
  }
}

async function directoryExists(fs, path) {
  try {
    await fs.readdir(path);
    return true;
  } catch (error) {
    if (error.code === "ENOENT") return false;
    throw error;
  }
}

async function ignoreFailure(operation) {
  try {
    await operation;
  } catch {
    // Cleanup is best effort only after a complete set is installed or another error is already authoritative.
  }
}

function ambiguousLockError(lockPath) {
  return new Error(`ambiguous worksheet generation lock: ${lockPath}`);
}

function lockOwnershipError(lockPath) {
  return new Error(`worksheet lock ownership changed; refusing to unlink: ${lockPath}`);
}

function sameFileIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

function validLockOwner(owner) {
  return Number.isSafeInteger(owner?.processId)
    && owner.processId > 0
    && typeof owner.token === "string"
    && owner.token.length > 0;
}

async function pathStat(fs, path) {
  try {
    return await fs.lstat(path);
  } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

async function lockMatches(fs, lockPath, identity, owner) {
  const current = await pathStat(fs, lockPath);
  if (!current || current.isSymbolicLink() || !sameFileIdentity(current, identity)) return false;
  if (!owner) return true;
  let observed;
  try {
    observed = JSON.parse(await fs.readFile(lockPath, "utf8"));
  } catch {
    return false;
  }
  return observed.processId === owner.processId && observed.token === owner.token;
}

async function removeOwnedLock(fs, lockPath, identity, owner) {
  if (!await lockMatches(fs, lockPath, identity, owner)) return false;
  await fs.rm(lockPath, { force: true });
  return true;
}

async function readExistingLock(fs, lockPath) {
  const handle = await fs.open(lockPath, "r");
  try {
    const identity = await handle.stat();
    let owner;
    try {
      owner = JSON.parse(await handle.readFile("utf8"));
    } catch {
      throw ambiguousLockError(lockPath);
    }
    if (!validLockOwner(owner)) throw ambiguousLockError(lockPath);
    if (!await lockMatches(fs, lockPath, identity, owner)) throw lockOwnershipError(lockPath);
    return { identity, owner };
  } finally {
    await handle.close();
  }
}

async function acquireWorksheetLock(fs, lockPath, processId, isProcessAlive, createLockToken) {
  let handle;
  try {
    handle = await fs.open(lockPath, "wx", 0o600);
  } catch (error) {
    if (error.code !== "EEXIST") throw error;
    const { identity, owner } = await readExistingLock(fs, lockPath);
    if (isProcessAlive(owner.processId)) {
      throw new Error(`active worksheet generation lock owned by process ${owner.processId}`);
    }
    if (!await removeOwnedLock(fs, lockPath, identity, owner)) throw lockOwnershipError(lockPath);
    handle = await fs.open(lockPath, "wx", 0o600);
  }
  const identity = await handle.stat();
  const owner = { processId, token: createLockToken() };
  if (!validLockOwner(owner)) {
    await ignoreFailure(handle.close());
    await ignoreFailure(removeOwnedLock(fs, lockPath, identity));
    throw new TypeError("lock token adapter must return a non-empty string");
  }
  try {
    await handle.writeFile(`${JSON.stringify(owner)}\n`, "utf8");
    if (!await lockMatches(fs, lockPath, identity, owner)) throw lockOwnershipError(lockPath);
    return { handle, identity, owner };
  } catch (error) {
    await ignoreFailure(handle.close());
    await ignoreFailure(removeOwnedLock(fs, lockPath, identity));
    throw error;
  }
}

function isInside(root, target) {
  const fromRoot = relative(root, target);
  return fromRoot === "" || !(fromRoot === ".." || fromRoot.startsWith(`..${sep}`) || isAbsolute(fromRoot));
}

function unsafeWorksheetPath(path, reason) {
  return new Error(`unsafe worksheet path (${reason}): ${path}`);
}

async function validateExistingAncestors(fs, repositoryPath, repositoryRealPath, targetPath) {
  if (!isInside(repositoryPath, targetPath)) {
    throw unsafeWorksheetPath(targetPath, "escapes repository");
  }
  const fromRepository = relative(repositoryPath, targetPath);
  const components = fromRepository === "" ? [] : fromRepository.split(sep);
  let current = repositoryPath;
  for (const [index, component] of components.entries()) {
    current = join(current, component);
    const stat = await pathStat(fs, current);
    if (!stat) break;
    if (stat.isSymbolicLink()) throw unsafeWorksheetPath(current, "symlinked worksheet path");
    if (index < components.length - 1 && !stat.isDirectory()) {
      throw unsafeWorksheetPath(current, "non-directory ancestor");
    }
    const currentRealPath = await fs.realpath(current);
    if (!isInside(repositoryRealPath, currentRealPath)) {
      throw unsafeWorksheetPath(current, "escapes repository");
    }
  }
}

async function validateWorksheetPaths(fs, repositoryRoot, outputDirectory) {
  if (typeof repositoryRoot !== "string" || repositoryRoot.length === 0) {
    throw new TypeError("explicit repositoryRoot is required");
  }
  const repositoryPath = resolve(repositoryRoot);
  if (dirname(repositoryPath) === repositoryPath) {
    throw new TypeError("repositoryRoot must not be the filesystem root");
  }
  const repositoryStat = await fs.lstat(repositoryPath);
  if (repositoryStat.isSymbolicLink() || !repositoryStat.isDirectory()) {
    throw unsafeWorksheetPath(repositoryPath, "repository root must be a real directory");
  }
  const repositoryRealPath = await fs.realpath(repositoryPath);
  const resolvedOutput = resolve(outputDirectory);
  const paths = {
    outputDirectory: resolvedOutput,
    stagingDirectory: `${resolvedOutput}.staging`,
    backupDirectory: `${resolvedOutput}.backup`,
    markerPath: `${resolvedOutput}.recovery.json`,
    lockPath: `${resolvedOutput}.lock`,
  };
  for (const path of Object.values(paths)) {
    await validateExistingAncestors(fs, repositoryPath, repositoryRealPath, path);
  }
  return { repositoryPath, ...paths };
}

async function reconcileWorksheetSwap(fs, {
  outputDirectory,
  stagingDirectory,
  backupDirectory,
  markerPath,
}) {
  const outputExists = await directoryExists(fs, outputDirectory);
  const backupExists = await directoryExists(fs, backupDirectory);
  if (!outputExists && backupExists) {
    await fs.rename(backupDirectory, outputDirectory);
  } else if (outputExists && backupExists) {
    await fs.rm(backupDirectory, { recursive: true, force: true });
  }
  await fs.rm(stagingDirectory, { recursive: true, force: true });
  await fs.rm(markerPath, { force: true });
}

export async function generateWorksheets({
  sources,
  repositoryRoot,
  outputDirectory,
  fs,
  sha256,
  processId = process.pid,
  isProcessAlive = processIsAlive,
  createLockToken = randomUUID,
}) {
  if (!fs
    || typeof fs.mkdir !== "function"
    || typeof fs.writeFile !== "function"
    || typeof fs.readFile !== "function"
    || typeof fs.readdir !== "function"
    || typeof fs.rename !== "function"
    || typeof fs.rm !== "function"
    || typeof fs.open !== "function"
    || typeof fs.lstat !== "function"
    || typeof fs.realpath !== "function") {
    throw new TypeError("filesystem adapter with mkdir/writeFile/readFile/readdir/rename/rm/open/lstat/realpath is required");
  }
  if (typeof sha256 !== "function") throw new TypeError("sha256 adapter is required");
  if (!Number.isSafeInteger(processId)
    || processId <= 0
    || typeof isProcessAlive !== "function"
    || typeof createLockToken !== "function") {
    throw new TypeError("positive processId, process liveness, and lock token adapters are required");
  }
  let {
    outputDirectory: resolvedOutput,
    stagingDirectory,
    backupDirectory,
    markerPath,
    lockPath,
  } = await validateWorksheetPaths(fs, repositoryRoot, outputDirectory);
  outputDirectory = resolvedOutput;
  const parentDirectory = dirname(outputDirectory);
  await fs.mkdir(parentDirectory, { recursive: true });
  await validateWorksheetPaths(fs, repositoryRoot, outputDirectory);
  const lock = await acquireWorksheetLock(
    fs,
    lockPath,
    processId,
    isProcessAlive,
    createLockToken,
  );
  try {
    await reconcileWorksheetSwap(fs, {
      outputDirectory,
      stagingDirectory,
      backupDirectory,
      markerPath,
    });
    await fs.mkdir(stagingDirectory, { recursive: true });
    let existingMoved = false;
    try {
      const expectedBytes = new Map();
      for (const [name, keys] of Object.entries(WORKSHEETS)) {
        const sourceBytes = canonicalJson(Object.fromEntries(keys.map((key) => [key, sources[key]])));
        const digest = await sha256(sourceBytes);
        if (!/^[0-9a-f]{64}$/.test(digest)) throw new TypeError("sha256 adapter returned an invalid digest");
        const bytes = markdown(name, keys, sources, digest);
        expectedBytes.set(name, bytes);
        await fs.writeFile(join(stagingDirectory, name), bytes, "utf8");
      }
      const stagedNames = (await fs.readdir(stagingDirectory)).sort();
      const expectedNames = [...expectedBytes.keys()].sort();
      if (JSON.stringify(stagedNames) !== JSON.stringify(expectedNames)) {
        throw new Error("staged worksheet set is incomplete");
      }
      for (const [name, bytes] of expectedBytes) {
        if (await fs.readFile(join(stagingDirectory, name), "utf8") !== bytes) {
          throw new Error(`staged worksheet verification failed: ${name}`);
        }
      }
      await fs.writeFile(markerPath, `${JSON.stringify({ processId, phase: "prepared" })}\n`, "utf8");
      try {
        await fs.rename(outputDirectory, backupDirectory);
        existingMoved = true;
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
      }
      await fs.writeFile(markerPath, `${JSON.stringify({
        processId,
        phase: existingMoved ? "backup-created" : "no-backup",
      })}\n`, "utf8");
      await fs.rename(stagingDirectory, outputDirectory);
      if (existingMoved) {
        await ignoreFailure(fs.rm(backupDirectory, { recursive: true, force: true }));
      }
      await ignoreFailure(fs.rm(markerPath, { force: true }));
    } catch (error) {
      if (existingMoved) {
        await fs.rename(backupDirectory, outputDirectory);
      }
      await ignoreFailure(fs.rm(stagingDirectory, { recursive: true, force: true }));
      await ignoreFailure(fs.rm(markerPath, { force: true }));
      throw error;
    }
  } finally {
    let closeError;
    try {
      await lock.handle.close();
    } catch (error) {
      closeError = error;
    }
    const removed = await removeOwnedLock(fs, lockPath, lock.identity, lock.owner);
    if (!removed) throw lockOwnershipError(lockPath);
    if (closeError) throw closeError;
  }
}
