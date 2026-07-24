#!/usr/bin/env node

import { createHash } from "node:crypto";
import * as fs from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { evaluatePolicy, generateWorksheets, loadPolicySources } from "./policy.mjs";
import { loadSchemaRegistry, validateSource } from "./schema.mjs";

const STATES = new Set(["PASS", "BLOCKED", "HUMAN ACTION"]);
const USAGE = "usage: node scripts/release/cli.mjs <generate|status [--json]>\n";

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
  const toolchains = validateSource(schemas, "toolchains", JSON.parse(await fs.readFile(sourceFile, "utf8")));
  return toolchains.tools.map((tool, index) => {
    const pinned = tool.version !== "not-declared";
    return {
      id: `toolchain.${tool.name}`,
      state: pinned ? "PASS" : "BLOCKED",
      sourceFile,
      pointer: `/tools/${index}`,
      observed: tool.version,
      expected: "an exact version and non-null authoritative checksum",
      recovery: pinned
        ? `Verify ${tool.name} against checksum ${tool.sha256}.`
        : `Declare and checksum the ${tool.name} version before candidate production.`,
    };
  });
}

function failureGate(error, root) {
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
    const sources = await loadPolicySources({ sourceDirectory: join(root, "release", "source"), fs });
    return [...evaluatePolicy(sources), ...await toolchainGates(root)];
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
      const sources = await loadPolicySources({ sourceDirectory: join(root, "release", "source"), fs });
      await generateWorksheets({
        sources,
        outputDirectory: join(root, "release", "generated", "worksheets"),
        fs,
        sha256: (bytes) => createHash("sha256").update(bytes).digest("hex"),
      });
      stdout.write("generated 11 worksheets\n");
      return 0;
    } catch {
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
