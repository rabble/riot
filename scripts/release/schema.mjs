import { readdir, readFile } from "node:fs/promises";
import { basename, join } from "node:path";
import Ajv2020 from "ajv/dist/2020.js";

const BASE = "https://riot.protest.net/release/schemas/";
const EXPECTED_IDS = Object.freeze({
  "accessibility.schema.json": `${BASE}accessibility.v1.json`,
  "account-gates.schema.json": `${BASE}account-gates.v1.json`,
  "claims.schema.json": `${BASE}claims.v1.json`,
  "common.schema.json": `${BASE}common.v1.json`,
  "export-compliance.schema.json": `${BASE}export-compliance.v1.json`,
  "network-matrix.schema.json": `${BASE}network-matrix.v1.json`,
  "policy.schema.json": `${BASE}policy.v1.json`,
  "privacy.schema.json": `${BASE}privacy.v1.json`,
  "product.schema.json": `${BASE}product.v1.json`,
  "review-instructions.schema.json": `${BASE}review-instructions.v1.json`,
  "toolchains.schema.json": `${BASE}toolchains.v1.json`,
});

function deepFreeze(value) {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const child of Object.values(value)) deepFreeze(child);
  }
  return value;
}

function pointerFor(error) {
  if (error.keyword === "required") return `${error.instancePath}/${error.params.missingProperty}`;
  if (error.keyword === "additionalProperties") return `${error.instancePath}/${error.params.additionalProperty}`;
  return error.instancePath || "/";
}

function pointerValue(value, pointer) {
  if (pointer === "/") return value;
  return pointer.slice(1).split("/").reduce((current, token) => {
    const key = token.replaceAll("~1", "/").replaceAll("~0", "~");
    return current?.[key];
  }, value);
}

function diagnosticFor(error, value) {
  const pointer = pointerFor(error);
  if (error.keyword === "required") {
    return {
      pointer,
      keyword: error.keyword,
      observed: "missing",
      expected: `required property ${error.params.missingProperty}`,
      message: error.message,
    };
  }
  if (error.keyword === "additionalProperties") {
    return {
      pointer,
      keyword: error.keyword,
      observed: pointerValue(value, pointer),
      expected: "unknown properties are forbidden",
      message: error.message,
    };
  }
  return {
    pointer,
    keyword: error.keyword,
    observed: pointerValue(value, pointer),
    expected: error.message,
    message: error.message,
  };
}

export function releaseDiagnosticError(message, {
  sourceFile,
  pointer = "/",
  observed,
  expected,
  keyword = "release",
}) {
  const error = new Error(message);
  error.sourceFile = sourceFile;
  error.diagnostics = [{ pointer, keyword, observed, expected, message }];
  return error;
}

export async function loadSchemaRegistry(directory, fs = { readdir, readFile }) {
  let names;
  try {
    names = (await fs.readdir(directory)).filter((name) => name.endsWith(".schema.json")).sort();
  } catch (error) {
    throw releaseDiagnosticError(`schema directory is unreadable: ${error.message}`, {
      sourceFile: directory,
      observed: "missing or unreadable",
      expected: "readable release schema directory",
      keyword: "registry",
    });
  }
  const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
  const schemas = new Map();
  for (const name of names) {
    const sourceFile = join(directory, name);
    let schema;
    try {
      schema = JSON.parse(await fs.readFile(sourceFile, "utf8"));
    } catch (error) {
      throw releaseDiagnosticError(`malformed schema JSON in ${name}: ${error.message}`, {
        sourceFile,
        observed: `malformed JSON: ${error.message}`,
        expected: "valid JSON schema",
        keyword: "registry",
      });
    }
    const expected = EXPECTED_IDS[name];
    if (!expected || schema.$id !== expected) {
      throw releaseDiagnosticError(`${name} does not use its fixed $id ${expected ?? "(no registered ID)"}`, {
        sourceFile,
        pointer: "/$id",
        observed: schema.$id ?? "missing",
        expected: expected ?? "no unregistered schema files",
        keyword: "registry",
      });
    }
    schemas.set(basename(name, ".schema.json"), schema);
  }
  for (const name of Object.keys(EXPECTED_IDS)) {
    if (!schemas.has(basename(name, ".schema.json"))) {
      throw releaseDiagnosticError(`missing schema file: ${name}`, {
        sourceFile: join(directory, name),
        observed: "missing",
        expected: "required fixed schema file",
        keyword: "registry",
      });
    }
  }
  for (const [name, schema] of schemas) {
    try {
      ajv.addSchema(schema, name);
      ajv.getSchema(name);
    } catch (error) {
      throw releaseDiagnosticError(`schema registration failed for ${name}: ${error.message}`, {
        sourceFile: join(directory, `${name}.schema.json`),
        observed: error.message,
        expected: "schema accepted by the fixed strict registry",
        keyword: "registry",
      });
    }
  }
  return Object.freeze({ ajv, schemas });
}

export function validateSource(registry, name, value) {
  const validator = registry.ajv.getSchema(name);
  if (!validator) {
    throw releaseDiagnosticError(`unknown schema: ${name}`, {
      sourceFile: name,
      observed: name,
      expected: "registered fixed schema name",
      keyword: "registry",
    });
  }
  if (!validator(value)) {
    const diagnostics = validator.errors
      .map((validationError) => diagnosticFor(validationError, value))
      .sort((left, right) =>
        `${left.pointer}\u0000${left.keyword}`.localeCompare(`${right.pointer}\u0000${right.keyword}`));
    const error = new Error(`${name} validation failed: ${diagnostics.map(({ pointer, message }) => `${pointer} ${message}`).join("; ")}`);
    error.diagnostics = diagnostics;
    throw error;
  }
  if (name === "toolchains") {
    const names = new Map();
    const commands = new Map();
    for (const [index, tool] of value.tools.entries()) {
      if (names.has(tool.name)) {
        throw releaseDiagnosticError("duplicate tool name", {
          pointer: `/tools/${index}/name`,
          observed: tool.name,
          expected: "unique tool name",
          keyword: "unique",
        });
      }
      names.set(tool.name, index);
      const command = JSON.stringify(tool.versionCommand);
      if (commands.has(command)) {
        throw releaseDiagnosticError("duplicate tool versionCommand", {
          pointer: `/tools/${index}/versionCommand`,
          observed: tool.versionCommand,
          expected: "unique tool versionCommand",
          keyword: "unique",
        });
      }
      commands.set(command, index);
    }
  }
  return deepFreeze(structuredClone(value));
}

export const fixedSchemaIds = EXPECTED_IDS;
