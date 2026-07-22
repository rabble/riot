import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import { loadLlvmThresholds, run, validateCoverageReport } from "../validate-llvm-coverage.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const validatorPath = path.resolve(here, "../validate-llvm-coverage.mjs");
const webScriptsDirectory = path.resolve(here, "..");
const repositoryRoot = path.resolve(here, "../../..");
const temporaryDirectory = mkdtempSync(path.join(os.tmpdir(), "riot-llvm-cov-"));
const metricNames = ["lines", "functions", "regions", "branches"];

// The floors the committed `.coverage-thresholds.json` (thresholds.llvm) sets,
// which the validator reads when the script runs from the real repo root.
const COVERAGE_CONFIG = JSON.parse(
  readFileSync(path.join(repositoryRoot, ".coverage-thresholds.json"), "utf8"),
);
const FLOORS = loadLlvmThresholds(COVERAGE_CONFIG);

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
  // coverage.sh reads the tarpaulin floor from this file (the source of truth).
  writeFileSync(path.join(root, ".coverage-thresholds.json"), JSON.stringify({
    thresholds: {
      tarpaulin: { lines: 94 },
      llvm: { lines: 95, functions: 95, regions: 92, branches: 83 },
      jsTooling: { lines: 100, branches: 100, functions: 100, statements: 100 },
    },
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
  node:*tarpaulin.lines*) echo 94 ;;
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

test("loadLlvmThresholds returns the four per-metric floors", () => {
  const floors = loadLlvmThresholds({
    thresholds: { llvm: { lines: 95, functions: 95, regions: 92, branches: 83 } },
  });
  assert.deepEqual(floors, { lines: 95, functions: 95, regions: 92, branches: 83 });
});

test("committed coverage floors match an auditable measured baseline", () => {
  const { measured } = COVERAGE_CONFIG.enforcement;
  assert.match(measured.date, /^\d{4}-\d{2}-\d{2}$/);
  assert.match(measured.justification, /workspace.*(?:expanded|growth)/i);

  const measurements = {
    tarpaulin: { lines: measured.tarpaulin.lines },
    llvm: measured.llvm,
  };
  for (const [tool, metrics] of Object.entries(measurements)) {
    for (const [name, measurement] of Object.entries(metrics)) {
      assert.equal(Number.isSafeInteger(measurement.covered), true);
      assert.equal(Number.isSafeInteger(measurement.count), true);
      assert.equal(measurement.covered <= measurement.count, true);
      assert.equal(
        measurement.percent,
        Number(((measurement.covered / measurement.count) * 100).toFixed(2)),
      );
      assert.equal(COVERAGE_CONFIG.thresholds[tool][name], Math.floor(measurement.percent));
    }
  }
});

test("loadLlvmThresholds fails closed on missing or malformed floors", () => {
  assert.throws(() => loadLlvmThresholds(null), /coverage thresholds must be an object/);
  assert.throws(() => loadLlvmThresholds({}), /must define thresholds\.llvm/);
  assert.throws(() => loadLlvmThresholds({ thresholds: {} }), /must define thresholds\.llvm/);
  assert.throws(
    () => loadLlvmThresholds({ thresholds: { llvm: { lines: 95, functions: 95, regions: 92 } } }),
    /thresholds\.llvm\.branches must be a percentage/,
  );
  assert.throws(
    () => loadLlvmThresholds({ thresholds: { llvm: { lines: 101, functions: 95, regions: 92, branches: 83 } } }),
    /thresholds\.llvm\.lines must be a percentage/,
  );
});

test("accepts coverage at or above the committed floors", () => {
  const result = runValidator(JSON.stringify(report()));
  assert.equal(result.status, 0);
  assert.equal(
    result.stdout,
    `LLVM coverage meets the floors (lines>=${FLOORS.lines}%, functions>=${FLOORS.functions}%, `
    + `regions>=${FLOORS.regions}%, branches>=${FLOORS.branches}%).\n`,
  );
  assert.equal(result.stderr, "");
});

for (const metricName of metricNames) {
  test(`rejects ${metricName} coverage below its floor and names the metric`, () => {
    // 80% is below every committed LLVM floor, so any metric at 80% is a deficit.
    const result = runValidator(JSON.stringify(report({
      [metricName]: metric(8_000, 10_000, 80),
    })), `${metricName}.json`);
    assert.equal(result.status, 1);
    assert.match(
      result.stderr,
      new RegExp(`^${metricName}: covered 8000 of 10000 \\(80\\.00%\\); floor is ${FLOORS[metricName]}%\\.`, "m"),
    );
  });
}

test("uses covered/count rather than a forged percentage field", () => {
  const result = runValidator(JSON.stringify(report({
    lines: metric(9_000, 10_000, 100),
  })), "forged-percent.json");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /^lines:/m);
});

test("zero covered of zero count is vacuously full coverage", () => {
  assert.doesNotThrow(() => validateCoverageReport(report({ branches: metric(0, 0, 0) }), FLOORS));
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

test("CLI fails closed when the configured coverage floors cannot be loaded", () => {
  const invalidThresholds = path.join(temporaryDirectory, "invalid-thresholds.json");
  writeFileSync(invalidThresholds, "{}");
  const errors = [];
  const originalConsoleError = console.error;
  console.error = (message) => errors.push(message);
  try {
    assert.equal(run(["unused-report.json"], invalidThresholds), 1);
  } finally {
    console.error = originalConsoleError;
  }
  assert.deepEqual(errors, ["unable to load coverage floors: coverage thresholds must define thresholds.llvm"]);
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
    assert.throws(() => validateCoverageReport(value, FLOORS), new RegExp(message));
  }
});

test("rejects missing and malformed required metrics", () => {
  const absent = report();
  delete absent.data[0].totals.lines;
  assert.throws(() => validateCoverageReport(absent, FLOORS), /lines must be an object/);

  const malformed = report({ lines: [] });
  assert.throws(() => validateCoverageReport(malformed, FLOORS), /lines must be an object/);
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
      () => validateCoverageReport(report({ lines: metric(covered, count) }), FLOORS),
      /lines (?:count|covered|covered cannot exceed count)/,
    );
  }
});

test("validateCoverageReport fails closed on a malformed floor", () => {
  assert.throws(
    () => validateCoverageReport(report(), { lines: 95, functions: 95, regions: 92, branches: 200 }),
    /branches floor must be a percentage/,
  );
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

test("coverage runs the composite gates in fail-fast order at the JSON tarpaulin floor", () => {
  const fixture = createFakeToolRepository();
  const result = runShellScript(fixture.root, "coverage.sh", fixture.environment);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(fixture.environment.FAKE_LOG, "utf8"), [
    "cargo tarpaulin --workspace --all-features --timeout 300 --fail-under 94",
    "cargo +nightly-2026-07-01 llvm-cov clean --workspace",
    "cargo +nightly-2026-07-01 llvm-cov --workspace --all-features --branch --json --output-path target/llvm-cov/riot.json",
    "node scripts/web/validate-llvm-coverage.mjs target/llvm-cov/riot.json",
    "npm run test:web:coverage",
    "",
  ].join("\n"));
});
