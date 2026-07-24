import assert from "node:assert/strict";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import test from "node:test";

import { renderStatus, runCli, statusExit, summarizeGates } from "../cli.mjs";

const repositoryRoot = dirname(dirname(dirname(dirname(fileURLToPath(import.meta.url)))));
const cli = join(repositoryRoot, "scripts", "release", "cli.mjs");

async function releaseRoot() {
  const root = await mkdtemp(join(tmpdir(), "riot-release-cli-"));
  await cp(join(repositoryRoot, "release"), join(root, "release"), { recursive: true });
  return root;
}

function run(root, args, environment = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [cli, ...args], {
      cwd: root,
      env: { ...process.env, ...environment },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr }));
  });
}

test("status aggregation preserves precedence and exact diagnostics", () => {
  const gates = [
    { id: "a", state: "PASS", sourceFile: "a.json", pointer: "/a", observed: "yes", expected: "yes", recovery: "none" },
    { id: "b", state: "HUMAN ACTION", sourceFile: "b.json", pointer: "/b", observed: "pending", expected: "approved", recovery: "approve" },
    { id: "c", state: "BLOCKED", sourceFile: "c.json", pointer: "/c", observed: "missing", expected: "present", recovery: "add" },
  ];
  assert.equal(summarizeGates(gates), "BLOCKED");
  assert.match(renderStatus(gates), /c\.json \/c.*missing.*present.*add/);
  assert.equal(summarizeGates(gates.slice(0, 2)), "HUMAN ACTION");
  assert.equal(summarizeGates(gates.slice(0, 1)), "PASS");
  assert.throws(() => summarizeGates([{ ...gates[0], state: "UNKNOWN" }]), /unknown gate state/);
});

test("statusExit maps each tri-state summary to its stable process code", () => {
  assert.equal(statusExit("PASS"), 0);
  assert.equal(statusExit("HUMAN ACTION"), 2);
  assert.equal(statusExit("BLOCKED"), 1);
});

test("runCli fails closed on a missing argument array without echoing input", async () => {
  let stdout = "";
  let stderr = "";
  const code = await runCli({
    args: null,
    stdout: { write: (value) => { stdout += value; } },
    stderr: { write: (value) => { stderr += value; } },
  });
  assert.equal(code, 64);
  assert.equal(stdout, "");
  assert.match(stderr, /usage:/);
});

test("status and status --json expose the same ordered truthful gates", async () => {
  const root = await releaseRoot();
  const text = await run(root, ["status"]);
  const json = await run(root, ["status", "--json"]);
  assert.equal(text.code, 1);
  assert.equal(json.code, 1);
  const parsed = JSON.parse(json.stdout);
  assert.equal(parsed.summary, "BLOCKED");
  assert(parsed.gates.some(({ id, state }) => id === "policy.filtering" && state === "BLOCKED"));
  assert(parsed.gates.some(({ id, state }) => id === "toolchain.android-ndk" && state === "PASS"));
  for (const gate of parsed.gates) {
    assert(text.stdout.includes(gate.id));
    assert(text.stdout.includes(gate.sourceFile));
    assert(text.stdout.includes(gate.pointer));
  }
  assert.equal(text.stderr, "");
  assert.equal(json.stderr, "");
});

test("generate is deterministic and creates only the eleven worksheets", async () => {
  const root = await releaseRoot();
  await rm(join(root, "release", "generated"), { recursive: true, force: true });
  const first = await run(root, ["generate"]);
  assert.deepEqual(first, { code: 0, stdout: "generated 11 worksheets\n", stderr: "" });
  const before = await readFile(join(root, "release", "generated", "worksheets", "app-privacy.md"), "utf8");
  const second = await run(root, ["generate"]);
  const after = await readFile(join(root, "release", "generated", "worksheets", "app-privacy.md"), "utf8");
  assert.equal(second.code, 0);
  assert.equal(after, before);
});

test("missing or malformed source fails closed without a stack trace", async () => {
  const root = await releaseRoot();
  const source = join(root, "release", "source", "product.json");
  await writeFile(source, "{", "utf8");
  const result = await run(root, ["status"]);
  assert.equal(result.code, 1);
  assert.match(result.stdout, /BLOCKED/);
  assert.match(result.stdout, /product\.json/);
  assert.doesNotMatch(`${result.stdout}${result.stderr}`, /\n\s+at /);
});

test("schema failures preserve exact source, pointer, observed, and expected diagnostics", async () => {
  const root = await releaseRoot();
  const source = join(root, "release", "source", "product.json");
  const product = JSON.parse(await readFile(source, "utf8"));
  product.surprise = "unsafe";
  await writeFile(source, `${JSON.stringify(product)}\n`, "utf8");
  const result = await run(root, ["status", "--json"]);
  assert.equal(result.code, 1);
  const gate = JSON.parse(result.stdout).gates[0];
  assert(gate.sourceFile.endsWith("/release/source/product.json"));
  assert.equal(gate.pointer, "/surprise");
  assert.equal(gate.observed, "unsafe");
  assert.match(gate.expected, /unknown properties are forbidden/);
});

test("toolchain schema failures preserve their exact source and JSON pointer", async () => {
  const root = await releaseRoot();
  const source = join(root, "release", "toolchains.json");
  const toolchains = JSON.parse(await readFile(source, "utf8"));
  toolchains.tools[0].surprise = "unsafe";
  await writeFile(source, `${JSON.stringify(toolchains)}\n`, "utf8");
  const result = await run(root, ["status", "--json"]);
  assert.equal(result.code, 1);
  const gate = JSON.parse(result.stdout).gates[0];
  assert(gate.sourceFile.endsWith("/release/toolchains.json"));
  assert.equal(gate.pointer, "/tools/0/surprise");
  assert.equal(gate.observed, "unsafe");
});

test("generate reports a fixed redacted failure for malformed source", async () => {
  const root = await releaseRoot();
  await writeFile(join(root, "release", "source", "product.json"), "{", "utf8");
  const result = await run(root, ["generate"]);
  assert.equal(result.code, 1);
  assert.equal(result.stdout, "");
  assert.equal(result.stderr, "release generation failed: correct the canonical source diagnostics with status\n");
});

test("status uses a safe release-root diagnostic when an error names no source path", async () => {
  const root = await releaseRoot();
  await writeFile(join(root, "release", "toolchains.json"), "{", "utf8");
  const result = await run(root, ["status", "--json"]);
  assert.equal(result.code, 1);
  const parsed = JSON.parse(result.stdout);
  assert(parsed.gates[0].sourceFile.endsWith("/release"));
  assert.equal(parsed.gates[0].pointer, "/");
});

test("status blocks a toolchain whose version is explicitly undeclared", async () => {
  const root = await releaseRoot();
  const manifestPath = join(root, "release", "toolchains.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.tools[0].version = "not-declared";
  await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`, "utf8");
  const result = await run(root, ["status", "--json"]);
  const gate = JSON.parse(result.stdout).gates.find(({ id }) => id === "toolchain.node");
  assert.equal(gate.state, "BLOCKED");
  assert.match(gate.recovery, /Declare and checksum/);
});

test("usage rejects unknown options, mutation commands, and credential flags", async () => {
  const root = await releaseRoot();
  for (const args of [
    [],
    ["wat"],
    ["upload"],
    ["status", "--api-key", "SECRET_VALUE"],
    ["generate", "--store"],
  ]) {
    const result = await run(root, args);
    assert.equal(result.code, 64);
    assert.equal(result.stdout, "");
    assert.match(result.stderr, /usage:/);
    assert.doesNotMatch(`${result.stdout}${result.stderr}`, /SECRET_VALUE/);
  }
});

test("status output never includes evidence payloads or environment secrets", async () => {
  const root = await releaseRoot();
  const result = await run(root, ["status", "--json"], { RIOT_TEST_SECRET: "DO_NOT_PRINT_THIS" });
  assert.doesNotMatch(`${result.stdout}${result.stderr}`, /DO_NOT_PRINT_THIS/);
  assert.doesNotMatch(result.stdout, /privateKey|password|token/);
});
