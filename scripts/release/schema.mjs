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

export async function loadSchemaRegistry(directory, fs = { readdir, readFile }) {
  const names = (await fs.readdir(directory)).filter((name) => name.endsWith(".schema.json")).sort();
  const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
  const schemas = new Map();
  const ids = new Set();
  for (const name of names) {
    let schema;
    try {
      schema = JSON.parse(await fs.readFile(join(directory, name), "utf8"));
    } catch (error) {
      throw new Error(`malformed schema JSON in ${name}: ${error.message}`);
    }
    const expected = EXPECTED_IDS[name];
    if (!expected || schema.$id !== expected) {
      throw new Error(`${name} does not use its fixed $id ${expected ?? "(no registered ID)"}`);
    }
    if (ids.has(schema.$id)) throw new Error(`duplicate schema $id: ${schema.$id}`);
    ids.add(schema.$id);
    schemas.set(basename(name, ".schema.json"), schema);
  }
  for (const name of Object.keys(EXPECTED_IDS)) {
    if (!schemas.has(basename(name, ".schema.json"))) throw new Error(`missing schema file: ${name}`);
  }
  for (const [name, schema] of schemas) {
    ajv.addSchema(schema, name);
  }
  return Object.freeze({ ajv, schemas });
}

export function validateSource(registry, name, value) {
  const validator = registry.ajv.getSchema(name);
  if (!validator) throw new Error(`unknown schema: ${name}`);
  if (!validator(value)) {
    const diagnostics = validator.errors
      .map((error) => ({
        pointer: pointerFor(error),
        keyword: error.keyword,
        message: error.message,
      }))
      .sort((left, right) => left.pointer.localeCompare(right.pointer) || left.keyword.localeCompare(right.keyword));
    const error = new Error(`${name} validation failed: ${diagnostics.map(({ pointer, message }) => `${pointer} ${message}`).join("; ")}`);
    error.diagnostics = diagnostics;
    throw error;
  }
  if (name === "toolchains") {
    const toolNames = value.tools.map(({ name: toolName }) => toolName);
    const commands = value.tools.map(({ versionCommand }) => JSON.stringify(versionCommand));
    if (new Set(toolNames).size !== toolNames.length) throw new Error("duplicate tool name");
    if (new Set(commands).size !== commands.length) throw new Error("duplicate tool versionCommand");
  }
  return deepFreeze(structuredClone(value));
}

export const fixedSchemaIds = EXPECTED_IDS;
