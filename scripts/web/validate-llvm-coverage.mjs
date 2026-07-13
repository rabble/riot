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

export function validateCoverageReport(report) {
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
    if (metric.covered !== metric.count) {
      deficits.push(`${metricName}: covered ${metric.covered} of ${metric.count}; exact 100% is required.`);
    }
  }

  if (deficits.length > 0) throw new Error(deficits.join("\n"));
}

function run(argumentsAfterScript) {
  if (argumentsAfterScript.length !== 1) {
    console.error("usage: validate-llvm-coverage.mjs <llvm-coverage.json>");
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
    validateCoverageReport(report);
  } catch (error) {
    console.error(error.message);
    return 1;
  }

  console.log("LLVM coverage is exactly 100% for lines, functions, regions, and branches.");
  return 0;
}

const isDirectExecution = process.argv[1] !== undefined
  && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isDirectExecution) process.exitCode = run(process.argv.slice(2));
