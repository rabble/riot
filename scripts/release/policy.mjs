import { readFile, readdir } from "node:fs/promises";
import { dirname, join } from "node:path";

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
  const urlEvidence = {};
  for (const [name, value] of Object.entries(loaded.product.urls)) {
    let state;
    try {
      const content = await fs.readFile(join(repositoryRoot, value.evidencePath), "utf8");
      const localState = /<!doctype\s+html|<html(?:\s|>)/i.test(content) ? "current" : "stale";
      state = value.evidenceState === "current" ? localState : value.evidenceState;
    } catch {
      state = "missing";
    }
    urlEvidence[name] = { state, evidencePath: value.evidencePath };
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

  for (const [name, value] of Object.entries(sources.product.urls)) {
    const verified = sources._urlEvidence?.[name]?.state ?? "missing";
    gates.push(gate(`url.${name}`, verified === "current" ? "PASS" : "BLOCKED", file("product"), `/urls/${name}/evidencePath`, `${verified}: ${value.evidencePath}`, "current non-empty HTML at the declared repository path", `Publish and verify ${value.evidencePath}.`));
  }

  const rowIndexes = new Map(sources.networkMatrix.rows.map(({ id }, index) => [id, index]));
  const rowById = new Map(sources.networkMatrix.rows.map((row) => [row.id, row]));
  for (const row of NETWORK_ROWS) {
    const index = rowIndexes.get(row);
    gates.push(gate(`network.${row}`, index === undefined ? "BLOCKED" : "PASS", file("networkMatrix"), index === undefined ? "/rows" : `/rows/${index}`, index === undefined ? "missing" : "present", "present", `Add the ${row} evidence row.`));
  }
  for (const [index, answer] of sources.privacy.answers.entries()) {
    const rows = answer.evidenceRowIds.map((id) => rowById.get(id));
    const evidenceComplete = rows.every(Boolean);
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
      const state = !record ? "BLOCKED" : record.state === "pass" && record.evidencePath ? "PASS" : record.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
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
        ? candidateEvidenceComplete ? "PASS" : "BLOCKED"
        : claim.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
    const journeys = claim.platforms.flatMap((platform) => claim.evidenceByPlatform?.[platform]?.journeyIds ?? []);
    gates.push(gate(`claim.${claim.id}`, state, file("claims"), `/claims/${index}`, claim.state, "platform-specific code and candidate journey evidence for every claimed platform", `Run candidate journeys ${journeys.join(", ")}.`));
  }

  const contentRating = sources.policy.contentRating;
  const contentRatingState = contentRating.state === "pass" ? "PASS" : contentRating.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
  gates.push(gate("content-rating", contentRatingState, file("policy"), "/contentRating", `${contentRating.apple}; ${contentRating.google}`, "authenticated store questionnaires confirm the canonical recommendation", "Confirm the content-rating questionnaires in App Store Connect and Google Play Console."));

  const classification = sources.exportCompliance.classification;
  const exportState = classification.state === "pass" && classification.approver && classification.evidence.length > 0
    ? "PASS"
    : classification.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
  gates.push(gate("export.classification", exportState, file("exportCompliance"), "/classification", classification.state, "authenticated approved classification", "Obtain legal/export approval and record its evidence."));

  for (const [index, account] of sources.accountGates.gates.entries()) {
    const state = account.state === "pass" && account.evidence.length > 0 ? "PASS" : account.state === "blocked" ? "BLOCKED" : "HUMAN ACTION";
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
