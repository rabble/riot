import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import { validateCoverageReport } from "../validate-llvm-coverage.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const validatorPath = path.resolve(here, "../validate-llvm-coverage.mjs");
const webScriptsDirectory = path.resolve(here, "..");
const temporaryDirectory = mkdtempSync(path.join(os.tmpdir(), "riot-llvm-cov-"));
const metricNames = ["lines", "functions", "regions", "branches"];

after(() => rmSync(temporaryDirectory, { force: true, recursive: true }));

function metric(covered = 10_000, count = 10_000, percent = 100) {
  return { count, covered, percent };
}

function report(overrides = {}) {
  return {
    data: [{
      totals: Object.fromEntries(metricNames.map((name) => [
        name,
        overrides[name] ?? metric(),
      ])),
    }],
    type: "llvm.coverage.json.export",
    version: "3.1.0",
  };
}

function runValidator(contents, name = "coverage.json") {
  const file = path.join(temporaryDirectory, name);
  writeFileSync(file, contents);
  return spawnSync(process.execPath, [validatorPath, file], { encoding: "utf8" });
}

function writeExecutable(file, contents) {
  writeFileSync(file, contents, { mode: 0o755 });
}

function createFakeToolRepository() {
  const root = mkdtempSync(path.join(temporaryDirectory, "fake-repo-"));
  const scriptsDirectory = path.join(root, "scripts", "web");
  const binDirectory = path.join(root, "fake-bin");
  const stateDirectory = path.join(root, "fake-state");
  mkdirSync(scriptsDirectory, { recursive: true });
  mkdirSync(binDirectory);
  mkdirSync(stateDirectory);
  writeFileSync(path.join(root, "commands.log"), "");
  copyFileSync(path.join(webScriptsDirectory, "bootstrap.sh"), path.join(scriptsDirectory, "bootstrap.sh"));
  copyFileSync(path.join(webScriptsDirectory, "coverage.sh"), path.join(scriptsDirectory, "coverage.sh"));
  writeFileSync(path.join(root, "package.json"), JSON.stringify({
    engines: { node: "26.4.0", npm: "11.17.0" },
  }));

  const fakeTool = String.raw`#!/bin/sh
set -eu
name=@{0##*/}
log=@{FAKE_LOG:?}
state=@{FAKE_STATE:?}
case "$name:$*" in
  "rustc:+1.95.0 --version") echo "@{FAKE_STABLE_VERSION:-rustc 1.95.0 (test)}" ;;
  "rustc:+nightly-2026-07-01 --version") echo "@{FAKE_NIGHTLY_VERSION:-rustc 1.98.0-nightly (test)}" ;;
  "rustup:component list --toolchain 1.95.0 --installed")
    printf '%s\n' cargo-test clippy-test rustc-test rustfmt-test
    ;;
  "rustup:target list --toolchain 1.95.0 --installed") echo wasm32-unknown-unknown ;;
  "rustup:component list --toolchain nightly-2026-07-01 --installed")
    printf '%s\n' cargo-test llvm-tools-test rustc-test
    ;;
  rustup:*) printf 'rustup %s\n' "$*" >> "$log" ;;
  "cargo:+1.95.0 --version") echo "cargo 1.95.0 (test)" ;;
  "cargo:llvm-cov --version")
    if [ -f "$state/llvm-installed" ]; then echo "cargo-llvm-cov 0.8.7"; else echo "@{FAKE_LLVM_VERSION:-cargo-llvm-cov 0.8.7}"; fi
    ;;
  "cargo:tarpaulin --version") echo "@{FAKE_TARPAULIN_VERSION:-cargo-tarpaulin-tarpaulin 0.37.0}" ;;
  cargo:+1.95.0\ install\ cargo-llvm-cov\ --version\ 0.8.7\ --locked)
    printf 'cargo %s\n' "$*" >> "$log"; : > "$state/llvm-installed"
    ;;
  cargo:+1.95.0\ install*) printf 'cargo %s\n' "$*" >> "$log" ;;
  cargo:*) printf 'cargo %s\n' "$*" >> "$log" ;;
  "wasm-bindgen:--version") echo "@{FAKE_WASM_VERSION:-wasm-bindgen 0.2.126}" ;;
  "node:--version") echo "@{FAKE_NODE_VERSION:-v26.4.0}" ;;
  node:*engines.node*) echo 26.4.0 ;;
  node:*engines.npm*) echo 11.17.0 ;;
  node:*validate-llvm-coverage.mjs*) printf 'node %s\n' "$*" >> "$log" ;;
  "npm:--version") echo "@{FAKE_NPM_VERSION:-11.17.0}" ;;
  npm:*engines.npm*) echo 11.17.0 ;;
  npm:*) printf 'npm %s\n' "$*" >> "$log" ;;
  *) echo "unexpected fake-tool invocation: $name $*" >&2; exit 97 ;;
esac
`.replaceAll("@{", "${");
  for (const command of ["cargo", "node", "npm", "rustc", "rustup", "wasm-bindgen"]) {
    writeExecutable(path.join(binDirectory, command), fakeTool);
  }

  return {
    environment: {
      ...process.env,
      FAKE_LOG: path.join(root, "commands.log"),
      FAKE_STATE: stateDirectory,
      PATH: `${binDirectory}:${process.env.PATH}`,
    },
    root,
  };
}

function runShellScript(root, script, environment) {
  return spawnSync("/bin/sh", [path.join(root, "scripts", "web", script)], {
    cwd: temporaryDirectory,
    encoding: "utf8",
    env: environment,
  });
}

test("accepts exact integer equality for all four LLVM totals", () => {
  const result = runValidator(JSON.stringify(report()));
  assert.equal(result.status, 0);
  assert.equal(result.stdout, "LLVM coverage is exactly 100% for lines, functions, regions, and branches.\n");
  assert.equal(result.stderr, "");
});

for (const metricName of metricNames) {
  test(`rejects 99.99 percent ${metricName} coverage and names the metric`, () => {
    const result = runValidator(JSON.stringify(report({
      [metricName]: metric(9_999, 10_000, 99.99),
    })), `${metricName}.json`);
    assert.equal(result.status, 1);
    assert.match(result.stderr, new RegExp(`^${metricName}: covered 9999 of 10000; exact 100% is required\\.`, "m"));
  });
}

test("uses covered/count equality rather than a forged rounded percentage", () => {
  const result = runValidator(JSON.stringify(report({
    lines: metric(9_999, 10_000, 100),
  })), "forged-percent.json");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /^lines:/m);
});

test("zero covered of zero count is exact equality", () => {
  assert.doesNotThrow(() => validateCoverageReport(report({ branches: metric(0, 0, 0) })));
});

test("reports every deficient metric in one invocation", () => {
  const deficient = Object.fromEntries(metricNames.map((name) => [name, metric(0, 1, 0)]));
  const result = runValidator(JSON.stringify(report(deficient)), "all-deficient.json");
  assert.equal(result.status, 1);
  for (const metricName of metricNames) assert.match(result.stderr, new RegExp(`^${metricName}:`, "m"));
});

test("rejects wrong CLI arity, unreadable input, and invalid JSON without stacks", () => {
  for (const args of [[], ["one", "two"]]) {
    const result = spawnSync(process.execPath, [validatorPath, ...args], { encoding: "utf8" });
    assert.equal(result.status, 1);
    assert.equal(result.stderr, "usage: validate-llvm-coverage.mjs <llvm-coverage.json>\n");
  }

  const missing = spawnSync(process.execPath, [validatorPath, path.join(temporaryDirectory, "missing.json")], { encoding: "utf8" });
  assert.equal(missing.status, 1);
  assert.equal(missing.stderr, "unable to read LLVM coverage report\n");

  const invalid = runValidator("{", "invalid.json");
  assert.equal(invalid.status, 1);
  assert.equal(invalid.stderr, "LLVM coverage report is not valid JSON\n");
});

test("rejects malformed LLVM report containers", () => {
  const malformed = [
    [null, "report must be an object"],
    [{}, "data must contain exactly one result"],
    [{ data: {} }, "data must contain exactly one result"],
    [{ data: [] }, "data must contain exactly one result"],
    [{ data: [{}, {}] }, "data must contain exactly one result"],
    [{ data: [null] }, "data result must be an object"],
    [{ data: [{}] }, "totals must be an object"],
    [{ data: [{ totals: [] }] }, "totals must be an object"],
  ];
  for (const [value, message] of malformed) {
    assert.throws(() => validateCoverageReport(value), new RegExp(message));
  }
});

test("rejects missing and malformed required metrics", () => {
  const absent = report();
  delete absent.data[0].totals.lines;
  assert.throws(() => validateCoverageReport(absent), /lines must be an object/);

  const malformed = report({ lines: [] });
  assert.throws(() => validateCoverageReport(malformed), /lines must be an object/);
});

test("rejects non-safe, negative, and inconsistent counts", () => {
  const invalidPairs = [
    ["100", 100],
    [1.5, 1],
    [-1, 0],
    [Number.MAX_SAFE_INTEGER + 1, 1],
    [100, "100"],
    [100, 1.5],
    [100, -1],
    [100, Number.MAX_SAFE_INTEGER + 1],
    [1, 2],
  ];
  for (const [count, covered] of invalidPairs) {
    assert.throws(
      () => validateCoverageReport(report({ lines: metric(covered, count) })),
      /lines (?:count|covered|covered cannot exceed count)/,
    );
  }
});

test("coverage shell entry points are portable sh syntax", () => {
  for (const script of ["bootstrap.sh", "coverage.sh"]) {
    const result = spawnSync("/bin/sh", ["-n", path.join(webScriptsDirectory, script)], { encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
  }
});

test("bootstrap is idempotent when every exact tool is present", () => {
  const fixture = createFakeToolRepository();
  const first = runShellScript(fixture.root, "bootstrap.sh", fixture.environment);
  assert.equal(first.status, 0, first.stderr);
  const second = runShellScript(fixture.root, "bootstrap.sh", fixture.environment);
  assert.equal(second.status, 0, second.stderr);
  assert.equal(readFileSync(fixture.environment.FAKE_LOG, "utf8"), "");
});

test("bootstrap installs and reverifies an exact mismatched Cargo tool", () => {
  const fixture = createFakeToolRepository();
  const result = runShellScript(fixture.root, "bootstrap.sh", {
    ...fixture.environment,
    FAKE_LLVM_VERSION: "cargo-llvm-cov 0.8.6",
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    readFileSync(fixture.environment.FAKE_LOG, "utf8"),
    "cargo +1.95.0 install cargo-llvm-cov --version 0.8.7 --locked\n",
  );
});

test("coverage rejects version drift before running any gate", () => {
  const fixture = createFakeToolRepository();
  const result = runShellScript(fixture.root, "coverage.sh", {
    ...fixture.environment,
    FAKE_NODE_VERSION: "v26.3.0",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Node 26\.4\.0 is required/);
  assert.equal(readFileSync(fixture.environment.FAKE_LOG, "utf8"), "");
});

test("coverage runs the exact composite gates in fail-fast order", () => {
  const fixture = createFakeToolRepository();
  const result = runShellScript(fixture.root, "coverage.sh", fixture.environment);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(fixture.environment.FAKE_LOG, "utf8"), [
    "cargo tarpaulin --workspace --all-features --fail-under 100",
    "cargo +nightly-2026-07-01 llvm-cov clean --workspace",
    "cargo +nightly-2026-07-01 llvm-cov --workspace --all-features --branch --json --output-path target/llvm-cov/riot.json",
    "node scripts/web/validate-llvm-coverage.mjs target/llvm-cov/riot.json",
    "npm run test:web:coverage",
    "",
  ].join("\n"));
});
