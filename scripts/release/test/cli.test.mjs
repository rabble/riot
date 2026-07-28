import assert from "node:assert/strict";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import test from "node:test";

import { failureGate, renderStatus, runCli, statusExit, summarizeGates } from "../cli.mjs";

const repositoryRoot = dirname(dirname(dirname(dirname(fileURLToPath(import.meta.url)))));
const cli = join(repositoryRoot, "scripts", "release", "cli.mjs");

async function releaseRoot() {
  const root = await mkdtemp(join(tmpdir(), "riot-release-cli-"));
  await cp(join(repositoryRoot, "release"), join(root, "release"), { recursive: true });
  // Generate re-hashes the shared fixture against its pinned digest.
  await cp(join(repositoryRoot, "fixtures", "release"), join(root, "fixtures", "release"), { recursive: true });
  // Visual generation reads the checked-in fonts and icon master.
  await cp(
    join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts"),
    join(root, "apps", "ios", "Riot", "Resources", "Fonts"),
    { recursive: true },
  );
  await cp(
    join(repositoryRoot, "apps", "ios", "Riot", "Assets.xcassets", "AppIcon.appiconset"),
    join(root, "apps", "ios", "Riot", "Assets.xcassets", "AppIcon.appiconset"),
    { recursive: true },
  );
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
  assert(parsed.gates.some(({ id, state }) => id === "toolchain.android-ndk" && state === "BLOCKED"));
  for (const gate of parsed.gates) {
    assert(text.stdout.includes(gate.id));
    assert(text.stdout.includes(gate.sourceFile));
    assert(text.stdout.includes(gate.pointer));
  }
  assert.equal(text.stderr, "");
  assert.equal(json.stderr, "");
});

test("generate is deterministic and creates the worksheets, metadata, and visual artifacts", async () => {
  const root = await releaseRoot();
  await rm(join(root, "release", "generated"), { recursive: true, force: true });
  const first = await run(root, ["generate"]);
  assert.equal(first.code, 0);
  assert.equal(first.stderr, "");
  assert.match(
    first.stdout,
    /^generated 11 worksheets, 12 metadata artifacts \(apple [0-9a-f]{64}, google [0-9a-f]{64}\), and 52 visual artifacts\n$/,
  );
  const before = await readFile(join(root, "release", "generated", "worksheets", "app-privacy.md"), "utf8");
  const beforeManifest = await readFile(join(root, "release", "generated", "apple", "en-US", "manifest.json"), "utf8");
  const second = await run(root, ["generate"]);
  const after = await readFile(join(root, "release", "generated", "worksheets", "app-privacy.md"), "utf8");
  const afterManifest = await readFile(join(root, "release", "generated", "apple", "en-US", "manifest.json"), "utf8");
  assert.equal(second.code, 0);
  assert.equal(after, before);
  assert.equal(afterManifest, beforeManifest);
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

test("duplicate tool names and commands preserve exact manifest diagnostics", async () => {
  for (const mutation of ["name", "versionCommand"]) {
    const root = await releaseRoot();
    const source = join(root, "release", "toolchains.json");
    const toolchains = JSON.parse(await readFile(source, "utf8"));
    toolchains.tools[1][mutation] = toolchains.tools[0][mutation];
    await writeFile(source, `${JSON.stringify(toolchains)}\n`, "utf8");
    const result = await run(root, ["status", "--json"]);
    const gate = JSON.parse(result.stdout).gates[0];
    assert(gate.sourceFile.endsWith("/release/toolchains.json"));
    assert.equal(gate.pointer, `/tools/1/${mutation}`);
    assert.notEqual(gate.observed, undefined);
    assert.match(gate.expected, /unique/);
  }
});

test("malformed and missing schema files preserve exact registry diagnostics", async () => {
  const malformedRoot = await releaseRoot();
  const malformed = join(malformedRoot, "release", "schemas", "product.schema.json");
  await writeFile(malformed, "{", "utf8");
  const malformedResult = await run(malformedRoot, ["status", "--json"]);
  const malformedGate = JSON.parse(malformedResult.stdout).gates[0];
  assert(malformedGate.sourceFile.endsWith("/release/schemas/product.schema.json"));
  assert.equal(malformedGate.pointer, "/");
  assert.match(malformedGate.observed, /malformed/);
  assert.match(malformedGate.expected, /valid JSON schema/);

  const missingRoot = await releaseRoot();
  const missing = join(missingRoot, "release", "schemas", "claims.schema.json");
  await rm(missing);
  const missingResult = await run(missingRoot, ["status", "--json"]);
  const missingGate = JSON.parse(missingResult.stdout).gates[0];
  assert(missingGate.sourceFile.endsWith("/release/schemas/claims.schema.json"));
  assert.equal(missingGate.pointer, "/");
  assert.equal(missingGate.observed, "missing");
  assert.match(missingGate.expected, /required fixed schema/);
});

test("unreadable and strict-invalid schema registries preserve exact diagnostics", async () => {
  const unreadableRoot = await releaseRoot();
  await rm(join(unreadableRoot, "release", "schemas"), { recursive: true });
  const unreadableResult = await run(unreadableRoot, ["status", "--json"]);
  const unreadableGate = JSON.parse(unreadableResult.stdout).gates[0];
  assert(unreadableGate.sourceFile.endsWith("/release/schemas"));
  assert.equal(unreadableGate.pointer, "/");
  assert.match(unreadableGate.expected, /readable release schema directory/);

  const invalidRoot = await releaseRoot();
  const invalidPath = join(invalidRoot, "release", "schemas", "product.schema.json");
  const invalid = JSON.parse(await readFile(invalidPath, "utf8"));
  invalid.unknownStrictKeyword = true;
  await writeFile(invalidPath, `${JSON.stringify(invalid)}\n`, "utf8");
  const invalidResult = await run(invalidRoot, ["status", "--json"]);
  const invalidGate = JSON.parse(invalidResult.stdout).gates[0];
  assert(invalidGate.sourceFile.endsWith("/release/schemas/product.schema.json"));
  assert.equal(invalidGate.pointer, "/");
  assert.match(invalidGate.observed, /unknownStrictKeyword/);
  assert.match(invalidGate.expected, /strict registry/);
});

test("truly unknown failures retain a safe diagnostic fallback", () => {
  const rooted = failureGate(new Error("/tmp/example.json: unknown failure"), "/repo");
  assert.equal(rooted.sourceFile, "/tmp/example.json");
  const unrooted = failureGate(new Error("unknown failure"), "/repo");
  assert.equal(unrooted.sourceFile, "/repo/release");
});

test("generate reports a fixed redacted failure for malformed source", async () => {
  const root = await releaseRoot();
  await writeFile(join(root, "release", "source", "product.json"), "{", "utf8");
  const result = await run(root, ["generate"]);
  assert.equal(result.code, 1);
  assert.equal(result.stdout, "");
  assert.equal(result.stderr, "release generation failed: correct the canonical source diagnostics with status\n");
});

test("malformed toolchain JSON identifies its exact manifest diagnostic", async () => {
  const root = await releaseRoot();
  await writeFile(join(root, "release", "toolchains.json"), "{", "utf8");
  const result = await run(root, ["status", "--json"]);
  assert.equal(result.code, 1);
  const parsed = JSON.parse(result.stdout);
  assert(parsed.gates[0].sourceFile.endsWith("/release/toolchains.json"));
  assert.equal(parsed.gates[0].pointer, "/");
  assert.match(parsed.gates[0].observed, /malformed/);
  assert.match(parsed.gates[0].expected, /valid canonical toolchain JSON/);
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

test("status blocks an incomplete canonical toolchain inventory", async () => {
  const root = await releaseRoot();
  const manifestPath = join(root, "release", "toolchains.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.tools = manifest.tools.filter(({ name }) => name !== "c8");
  await writeFile(manifestPath, `${JSON.stringify(manifest)}\n`, "utf8");
  const result = await run(root, ["status", "--json"]);
  const gate = JSON.parse(result.stdout).gates.find(({ id }) => id === "inventory.toolchains");
  assert.equal(gate.state, "BLOCKED");
  assert.match(gate.observed, /missing: c8/);
});

test("status blocks incomplete privacy and account inventories", async () => {
  for (const fixture of [
    {
      file: "privacy.json",
      mutate(value) { value.answers[1].store = "apple"; },
      gateId: "inventory.privacy-answers",
    },
    {
      file: "account-gates.json",
      mutate(value) { value.gates.pop(); },
      gateId: "inventory.account-gates",
    },
  ]) {
    const root = await releaseRoot();
    const sourcePath = join(root, "release", "source", fixture.file);
    const value = JSON.parse(await readFile(sourcePath, "utf8"));
    fixture.mutate(value);
    await writeFile(sourcePath, `${JSON.stringify(value)}\n`, "utf8");
    const result = await run(root, ["status", "--json"]);
    const gate = JSON.parse(result.stdout).gates.find(({ id }) => id === fixture.gateId);
    assert.equal(gate.state, "BLOCKED");
  }
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

test("generate fails closed when the shared fixture drifts from the pinned digest", async () => {
  const root = await releaseRoot();
  const fixturePath = join(root, "fixtures", "release", "riot-1.0-synthetic.json");
  const original = await readFile(fixturePath, "utf8");
  await writeFile(fixturePath, `${original}\n`);
  const result = await run(root, ["generate"]);
  assert.notEqual(result.code, 0, "generate must fail when the fixture digest drifts");
  assert.match(result.stderr + result.stdout, /fixture.*digest|digest.*fixture/i);
});
