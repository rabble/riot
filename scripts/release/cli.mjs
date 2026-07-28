#!/usr/bin/env node

import { createHash } from "node:crypto";
import * as fs from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { evaluateConfiguration, evaluateSnapshotFreshness } from "./configuration.mjs";
import { evaluateMetadata, generateMetadata, loadMetadataSources } from "./metadata.mjs";
import { evaluatePolicy, generateWorksheets, loadPolicySources } from "./policy.mjs";
import { canonicalJson } from "./canonical-json.mjs";
import { loadSchemaRegistry, releaseDiagnosticError, validateSource } from "./schema.mjs";
import { renderDrafts, renderIcons } from "./visual-render.mjs";
import { validateVisuals } from "./visual-validate.mjs";

const STATES = new Set(["PASS", "BLOCKED", "HUMAN ACTION"]);
const REQUIRED_TOOLCHAINS = ["node", "npm", "rustc", "cargo", "gradle", "android-sdk", "android-ndk", "xcode", "swift", "ajv", "c8"];
const USAGE = "usage: node scripts/release/cli.mjs <generate|status [--json]>\n";
const sha256Hex = (bytes) => createHash("sha256").update(bytes).digest("hex");

export function summarizeGates(gates) {
  for (const { state } of gates) {
    if (!STATES.has(state)) throw new TypeError(`unknown gate state: ${state}`);
  }
  if (gates.some(({ state }) => state === "BLOCKED")) return "BLOCKED";
  if (gates.some(({ state }) => state === "HUMAN ACTION")) return "HUMAN ACTION";
  return "PASS";
}

export function renderStatus(gates) {
  const summary = summarizeGates(gates);
  const lines = [`Release status: ${summary}`];
  for (const gate of gates) {
    lines.push(
      `[${gate.state}] ${gate.id} — ${gate.sourceFile} ${gate.pointer} — observed: ${String(gate.observed)} — expected: ${String(gate.expected)} — recovery: ${gate.recovery}`,
    );
  }
  return `${lines.join("\n")}\n`;
}

export function statusExit(summary) {
  if (summary === "PASS") return 0;
  if (summary === "HUMAN ACTION") return 2;
  return 1;
}

async function toolchainGates(root) {
  const sourceFile = join(root, "release", "toolchains.json");
  const schemas = await loadSchemaRegistry(join(root, "release", "schemas"), fs);
  let toolchains;
  try {
    let value;
    try {
      value = JSON.parse(await fs.readFile(sourceFile, "utf8"));
    } catch (error) {
      throw releaseDiagnosticError(`malformed or missing toolchain manifest: ${error.message}`, {
        sourceFile,
        observed: `malformed or missing: ${error.message}`,
        expected: "valid canonical toolchain JSON",
        keyword: "source",
      });
    }
    toolchains = validateSource(schemas, "toolchains", value);
  } catch (error) {
    if (error.diagnostics && !error.sourceFile) error.sourceFile = sourceFile;
    throw error;
  }
  const names = toolchains.tools.map(({ name }) => name);
  const missing = REQUIRED_TOOLCHAINS.filter((name) => !names.includes(name));
  const unexpected = names.filter((name) => !REQUIRED_TOOLCHAINS.includes(name));
  const inventory = {
    id: "inventory.toolchains",
    state: missing.length === 0 && unexpected.length === 0 && names.length === REQUIRED_TOOLCHAINS.length ? "PASS" : "BLOCKED",
    sourceFile,
    pointer: "/tools",
    observed: `missing: ${missing.join(", ") || "none"}; unexpected: ${unexpected.join(", ") || "none"}`,
    expected: `exactly one of: ${REQUIRED_TOOLCHAINS.join(", ")}`,
    recovery: "Restore the exact canonical WU-000 toolchain inventory.",
  };
  const gates = toolchains.tools.map((tool, index) => {
    const pinned = tool.version !== "not-declared" && tool.artifactEvidenceState !== "blocked";
    return {
      id: `toolchain.${tool.name}`,
      state: pinned ? "PASS" : "BLOCKED",
      sourceFile,
      pointer: `/tools/${index}`,
      observed: tool.artifactEvidenceState === "blocked" ? `${tool.version}: ${tool.artifactEvidenceReason}` : tool.version,
      expected: "an exact version and authoritative SHA-256 for every downloaded artifact",
      recovery: pinned
        ? `Verify ${tool.name} against checksum ${tool.sha256}.`
        : tool.artifactEvidenceState === "blocked"
          ? `Record an authoritative SHA-256 for ${tool.downloadUrl} before candidate production.`
          : `Declare and checksum the ${tool.name} version before candidate production.`,
    };
  });
  return [inventory, ...gates];
}

export function failureGate(error, root) {
  const diagnostic = error.diagnostics?.[0];
  if (diagnostic) {
    return {
      id: "foundation.sources",
      state: "BLOCKED",
      sourceFile: error.sourceFile,
      pointer: diagnostic.pointer,
      observed: diagnostic.observed,
      expected: diagnostic.expected,
      recovery: "Correct the named release source or schema file and rerun status.",
    };
  }
  const message = String(error.message).replaceAll(/\s+/g, " ");
  const pathMatch = message.match(/(?:^|\s)(\/[^:]+(?:\.json)?)/);
  return {
    id: "foundation.sources",
    state: "BLOCKED",
    sourceFile: pathMatch?.[1] ?? join(root, "release"),
    pointer: "/",
    observed: "malformed or missing release evidence",
    expected: "schema-valid canonical release evidence",
    recovery: "Correct the named release source or schema file and rerun status.",
  };
}

async function collectGates(root) {
  try {
    const sourceDirectory = join(root, "release", "source");
    const registry = await loadSchemaRegistry(join(root, "release", "schemas"), fs);
    const sources = await loadPolicySources({ sourceDirectory, fs });
    const metadataSources = await loadMetadataSources({ sourceDirectory, registry, fs });
    const snapshot = validateSource(
      registry,
      "configuration-snapshot",
      JSON.parse(await fs.readFile(join(sourceDirectory, "configuration-snapshot.json"), "utf8")),
    );
    return [
      ...evaluatePolicy(sources),
      ...evaluateMetadata(metadataSources),
      ...evaluateConfiguration(snapshot),
      await evaluateSnapshotFreshness({ snapshot, repositoryRoot: root, fs, sha256: sha256Hex }),
      ...await validateVisuals({
        visuals: metadataSources.visuals,
        repositoryRoot: root,
        fs,
        sha256: sha256Hex,
      }),
      ...await toolchainGates(root),
    ];
  } catch (error) {
    return [failureGate(error, root)];
  }
}

function validArguments(args) {
  return args.length === 1 && (args[0] === "generate" || args[0] === "status")
    || args.length === 2 && args[0] === "status" && args[1] === "--json";
}

export async function runCli({
  args,
  root = process.cwd(),
  stdout = process.stdout,
  stderr = process.stderr,
} = {}) {
  if (!Array.isArray(args) || !validArguments(args)) {
    stderr.write(USAGE);
    return 64;
  }
  if (args[0] === "generate") {
    try {
      const sha256 = sha256Hex;
      const sourceDirectory = join(root, "release", "source");
      const registry = await loadSchemaRegistry(join(root, "release", "schemas"), fs);
      const sources = await loadPolicySources({ sourceDirectory, fs });
      await generateWorksheets({
        sources,
        repositoryRoot: root,
        outputDirectory: join(root, "release", "generated", "worksheets"),
        fs,
        sha256,
      });
      const metadataSources = await loadMetadataSources({ sourceDirectory, registry, fs });
      const { appleManifestSha256, googleManifestSha256 } = await generateMetadata({
        sources: metadataSources,
        outputDirectory: join(root, "release", "generated"),
        fs,
        sha256,
      });
      // The shared release fixture digest is pinned so provenance stays stable,
      // but it must be recomputed from the actual file at generate time —
      // otherwise a fixture edit silently produces artifacts bound to a stale
      // digest.
      const pinnedFixtureSha256 = "930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469";
      const fixtureSha256 = sha256(await fs.readFile(join(root, "fixtures", "release", "riot-1.0-synthetic.json")));
      if (fixtureSha256 !== pinnedFixtureSha256) {
        const driftError = new Error(
          `release fixture digest drift: fixtures/release/riot-1.0-synthetic.json hashes to ${fixtureSha256}, ` +
          `expected pinned ${pinnedFixtureSha256}. Re-pin deliberately and update every fixture consumer in the same commit.`,
        );
        // Digests are safe to print (no paths or source contents); flag so the
        // generate error handler can surface this instead of the generic
        // redacted failure.
        driftError.exposeToOperator = true;
        throw driftError;
      }
      const { icons } = await renderIcons({
        masterPath: join(root, "apps", "ios", "Riot", "Assets.xcassets", "AppIcon.appiconset", "AppIcon-1024.png"),
        outputDirectory: join(root, "release", "generated"),
        fs,
        sha256,
        visuals: metadataSources.visuals,
        fontsDirectory: join(root, "apps", "ios", "Riot", "Resources", "Fonts"),
      });
      // Every icon file is bound into the draft provenance so validation can
      // re-derive its digest; the approval block survives regeneration only
      // while the unsigned provenance (cells + icons) is unchanged.
      await renderDrafts({
        visuals: metadataSources.visuals,
        fixtureRevision: "riot-1.0-synthetic-v1",
        fixtureSha256,
        fontsDirectory: join(root, "apps", "ios", "Riot", "Resources", "Fonts"),
        outputDirectory: join(root, "release", "generated"),
        fs,
        sha256,
        icons,
      });
      stdout.write(
        `generated 11 worksheets, 12 metadata artifacts (apple ${appleManifestSha256}, google ${googleManifestSha256}), and 52 visual artifacts\n`,
      );
      return 0;
    } catch (error) {
      if (error && error.exposeToOperator === true) {
        stderr.write(`${error.message}\n`);
        return 1;
      }
      stderr.write("release generation failed: correct the canonical source diagnostics with status\n");
      return 1;
    }
  }

  const gates = await collectGates(root);
  const summary = summarizeGates(gates);
  if (args[1] === "--json") {
    stdout.write(`${JSON.stringify({ summary, gates })}\n`);
  } else {
    stdout.write(renderStatus(gates));
  }
  return statusExit(summary);
}

if (fileURLToPath(import.meta.url) === process.argv[1]) {
  const exitCode = await runCli({ args: process.argv.slice(2) });
  process.exitCode = exitCode;
}
