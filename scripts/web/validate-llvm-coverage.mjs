#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REQUIRED_METRICS = Object.freeze(["lines", "functions", "regions", "branches"]);

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireSafeNonnegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label} must be a nonnegative safe integer`);
  }
}

function requirePercentFloor(value, label) {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 100) {
    throw new Error(`${label} must be a percentage between 0 and 100`);
  }
}

/**
 * The llvm-cov floors, from the single source of truth `.coverage-thresholds.json`
 * (`thresholds.llvm`). Fails closed: a missing file, malformed JSON, or a missing
 * metric throws rather than silently producing a no-op gate.
 */
export function loadLlvmThresholds(thresholdsJson) {
  if (!isRecord(thresholdsJson)) throw new Error("coverage thresholds must be an object");
  const thresholds = thresholdsJson.thresholds;
  if (!isRecord(thresholds) || !isRecord(thresholds.llvm)) {
    throw new Error("coverage thresholds must define thresholds.llvm");
  }
  const floors = {};
  for (const metricName of REQUIRED_METRICS) {
    const floor = thresholds.llvm[metricName];
    requirePercentFloor(floor, `thresholds.llvm.${metricName}`);
    floors[metricName] = floor;
  }
  return floors;
}

/**
 * A metric passes when its covered percentage is at or above its floor. An empty
 * metric (count 0 — no code of that kind) is vacuously 100% and never a deficit.
 */
export function validateCoverageReport(report, thresholds) {
  if (!isRecord(report)) throw new Error("report must be an object");
  if (!Array.isArray(report.data) || report.data.length !== 1) {
    throw new Error("data must contain exactly one result");
  }

  const [result] = report.data;
  if (!isRecord(result)) throw new Error("data result must be an object");
  if (!isRecord(result.totals)) throw new Error("totals must be an object");

  const deficits = [];
  for (const metricName of REQUIRED_METRICS) {
    const metric = result.totals[metricName];
    if (!isRecord(metric)) throw new Error(`${metricName} must be an object`);
    requireSafeNonnegativeInteger(metric.count, `${metricName} count`);
    requireSafeNonnegativeInteger(metric.covered, `${metricName} covered`);
    if (metric.covered > metric.count) {
      throw new Error(`${metricName} covered cannot exceed count`);
    }
    const floor = thresholds[metricName];
    requirePercentFloor(floor, `${metricName} floor`);
    const percent = metric.count === 0 ? 100 : (metric.covered / metric.count) * 100;
    if (percent < floor) {
      deficits.push(
        `${metricName}: covered ${metric.covered} of ${metric.count} `
        + `(${percent.toFixed(2)}%); floor is ${floor}%.`,
      );
    }
  }

  if (deficits.length > 0) throw new Error(deficits.join("\n"));
}

export function run(
  argumentsAfterScript,
  thresholdsPath = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../..",
    ".coverage-thresholds.json",
  ),
) {
  if (argumentsAfterScript.length !== 1) {
    console.error("usage: validate-llvm-coverage.mjs <llvm-coverage.json>");
    return 1;
  }

  let thresholds;
  try {
    thresholds = loadLlvmThresholds(JSON.parse(readFileSync(thresholdsPath, "utf8")));
  } catch (error) {
    console.error(`unable to load coverage floors: ${error.message}`);
    return 1;
  }

  let contents;
  try {
    contents = readFileSync(argumentsAfterScript[0], "utf8");
  } catch {
    console.error("unable to read LLVM coverage report");
    return 1;
  }

  let report;
  try {
    report = JSON.parse(contents);
  } catch {
    console.error("LLVM coverage report is not valid JSON");
    return 1;
  }

  try {
    validateCoverageReport(report, thresholds);
  } catch (error) {
    console.error(error.message);
    return 1;
  }

  console.log(
    `LLVM coverage meets the floors (lines>=${thresholds.lines}%, `
    + `functions>=${thresholds.functions}%, regions>=${thresholds.regions}%, `
    + `branches>=${thresholds.branches}%).`,
  );
  return 0;
}

const isDirectExecution = process.argv[1] !== undefined
  && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isDirectExecution) process.exitCode = run(process.argv.slice(2));
