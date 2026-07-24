import { readFile, readdir } from "node:fs/promises";
import { dirname, join } from "node:path";

import { canonicalJson } from "./canonical-json.mjs";
import { loadSchemaRegistry, validateSource } from "./schema.mjs";

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
const REVIEW_TOPICS = ["first-launch", "demo-content", "no-login", "create", "join", "publish", "restart", "offline", "local-permissions", "nearby-testing", "permission-denial", "invalid-join", "no-peers"];
const DEVICES = ["iphone", "ipad", "mac", "android-phone", "android-tablet"];
const TASKS = ["read", "create", "join", "publish"];
const WORKSHEETS = Object.freeze({
  "accessibility.md": ["accessibility"],
  "account-gates.md": ["accountGates"],
  "app-privacy.md": ["privacy", "networkMatrix"],
  "content-rating.md": ["policy", "claims"],
  "data-safety.md": ["privacy", "networkMatrix"],
  "export-compliance.md": ["exportCompliance"],
  "outbound-network.md": ["networkMatrix"],
  "permissions.md": ["privacy"],
  "required-reason-apis.md": ["privacy"],
  "review-instructions.md": ["reviewInstructions"],
  "ugc-operations.md": ["policy"],
});

function gate(id, state, sourceFile, pointer, observed, expected, recovery) {
  return { id, state, sourceFile, pointer, observed, expected, recovery };
}

export async function loadPolicySources({ sourceDirectory, fs = { readFile, readdir } }) {
  if (!sourceDirectory || typeof fs?.readFile !== "function") throw new TypeError("sourceDirectory and fs are required");
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
      throw new Error(`${sourceFile}: malformed or missing source: ${error.message}`);
    }
    loaded[key] = structuredClone(validateSource(registry, name, value));
    files[key] = sourceFile;
  }
  Object.defineProperty(loaded, "_files", { value: files, enumerable: false });
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

  for (const [name, value] of Object.entries(sources.product.urls)) {
    gates.push(gate(`url.${name}`, value.evidenceState === "current" ? "PASS" : "BLOCKED", file("product"), `/urls/${name}/evidenceState`, value.evidenceState, "current", `Publish and verify ${value.evidencePath}.`));
  }

  const rowIds = new Set(sources.networkMatrix.rows.map(({ id }) => id));
  for (const row of NETWORK_ROWS) {
    gates.push(gate(`network.${row}`, rowIds.has(row) ? "PASS" : "BLOCKED", file("networkMatrix"), "/rows", rowIds.has(row) ? "present" : "missing", "present", `Add the ${row} evidence row.`));
  }
  for (const answer of sources.privacy.answers) {
    const evidenceComplete = answer.evidenceRowIds.every((id) => rowIds.has(id));
    gates.push(gate(`privacy.${answer.store}`, evidenceComplete ? "PASS" : "BLOCKED", file("privacy"), `/answers/${answer.store}`, answer.evidenceRowIds.join(","), "all referenced network evidence rows exist", "Correct the store answer or add the missing network evidence."));
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

  const reviewIds = new Set(sources.reviewInstructions.topics.map(({ id }) => id));
  for (const topic of REVIEW_TOPICS) {
    gates.push(gate(`review.${topic}`, reviewIds.has(topic) ? "PASS" : "BLOCKED", file("reviewInstructions"), "/topics", reviewIds.has(topic) ? "present" : "missing", "present", `Add truthful review instructions for ${topic}.`));
  }

  const accessibility = new Map(sources.accessibility.records.map((record) => [`${record.device}:${record.task}`, record]));
  for (const device of DEVICES) {
    for (const task of TASKS) {
      const record = accessibility.get(`${device}:${task}`);
      const state = !record ? "BLOCKED" : record.state === "pass" && record.evidencePath ? "PASS" : record.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
      gates.push(gate(`accessibility.${device}.${task}`, state, file("accessibility"), "/records", record?.state ?? "missing", "passing candidate-bound evidence", `Run and record the ${task} accessibility rehearsal on ${device}.`));
    }
  }

  for (const claim of sources.claims.claims) {
    const state = claim.state === "pass" ? "PASS" : claim.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
    gates.push(gate(`claim.${claim.id}`, state, file("claims"), `/claims/${claim.id}`, claim.state, "passing code and candidate journey evidence", `Run candidate journeys ${claim.journeyIds.join(", ")}.`));
  }

  const classification = sources.exportCompliance.classification;
  const exportState = classification.state === "pass" && classification.approver && classification.evidence.length > 0
    ? "PASS"
    : classification.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
  gates.push(gate("export.classification", exportState, file("exportCompliance"), "/classification", classification.state, "authenticated approved classification", "Obtain legal/export approval and record its evidence."));

  for (const account of sources.accountGates.gates) {
    const state = account.state === "pass" && account.evidence.length > 0 ? "PASS" : account.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
    gates.push(gate(`account.${account.name}`, state, file("accountGates"), `/gates/${account.name}`, account.state, "authenticated current evidence", `Confirm ${account.name} in the authenticated store account.`));
  }
  return gates;
}

function markdown(name, keys, sources, digest) {
  const gates = evaluatePolicy(sources).filter(({ sourceFile }) =>
    keys.some((key) => sourceFile.endsWith(`${SOURCES[key]}.json`)));
  const body = gates.map(({ id, state, observed, expected, recovery }) =>
    `## ${id}\n\n- State: **${state}**\n- Observed: ${observed}\n- Expected: ${expected}\n- Recovery: ${recovery}\n`).join("\n");
  return `<!-- source-sha256: ${digest} -->\n# ${name.replace(/\\.md$/, "").replaceAll("-", " ")}\n\n${body}`;
}

export async function generateWorksheets({ sources, outputDirectory, fs, sha256 }) {
  if (!fs || typeof fs.mkdir !== "function" || typeof fs.writeFile !== "function" || typeof fs.rename !== "function" || typeof fs.rm !== "function") {
    throw new TypeError("filesystem adapter with mkdir/writeFile/rename/rm is required");
  }
  if (typeof sha256 !== "function") throw new TypeError("sha256 adapter is required");
  await fs.mkdir(outputDirectory, { recursive: true });
  const temporaryPaths = [];
  try {
    let index = 0;
    for (const [name, keys] of Object.entries(WORKSHEETS)) {
      const sourceBytes = canonicalJson(Object.fromEntries(keys.map((key) => [key, sources[key]])));
      const digest = await sha256(sourceBytes);
      if (!/^[0-9a-f]{64}$/.test(digest)) throw new TypeError("sha256 adapter returned an invalid digest");
      const finalPath = join(outputDirectory, name);
      const temporaryPath = `${finalPath}.tmp-${process.pid}-${index}`;
      index += 1;
      temporaryPaths.push(temporaryPath);
      await fs.writeFile(temporaryPath, markdown(name, keys, sources, digest), "utf8");
      await fs.rename(temporaryPath, finalPath);
      temporaryPaths.pop();
    }
  } catch (error) {
    await Promise.all(temporaryPaths.map((path) => fs.rm(path, { force: true }).catch(() => {})));
    throw error;
  }
}
