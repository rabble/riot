import assert from "node:assert/strict";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { loadSchemaRegistry, validateSource } from "../schema.mjs";

const repositoryRoot = dirname(dirname(dirname(dirname(fileURLToPath(import.meta.url)))));
const schemaDirectory = join(repositoryRoot, "release", "schemas");

const validProduct = {
  schemaVersion: 1,
  name: "riot.protest.net",
  version: "1.0",
  price: "free",
  availability: "worldwide",
  releaseChannel: "public-early-access",
  appleBundleId: "net.protest.riot",
  androidApplicationId: "net.protest.riot",
  urls: {
    privacy: { url: "https://riot.protest.net/privacy/", evidencePath: "marketing/privacy/index.html", evidenceState: "current" },
    support: { url: "https://riot.protest.net/support/", evidencePath: "marketing/support/index.html", evidenceState: "missing" },
    marketing: { url: "https://riot.protest.net/", evidencePath: "marketing/releases/index.html", evidenceState: "current" },
  },
};

test("registry validates a closed source and returns a frozen value", async () => {
  const registry = await loadSchemaRegistry(schemaDirectory);
  const result = validateSource(registry, "product", validProduct);
  assert.deepEqual(result, validProduct);
  assert(Object.isFrozen(result));
});

test("validateSource rejects missing and unknown fields with JSON pointers", async () => {
  const registry = await loadSchemaRegistry(schemaDirectory);
  assert.throws(
    () => validateSource(registry, "product", { schemaVersion: 1, surprise: true }),
    (error) => error.diagnostics.some(({ pointer, keyword }) => pointer === "/surprise" && keyword === "additionalProperties"),
  );
  assert.throws(
    () => validateSource(registry, "product", { schemaVersion: 1 }),
    (error) => error.diagnostics.some(({ pointer, keyword }) => pointer === "/name" && keyword === "required"),
  );
  assert.throws(() => validateSource(registry, "missing", validProduct), /unknown schema/);
});

test("common durable fields reject malformed versions, IDs, digests, and timestamps", async () => {
  const registry = await loadSchemaRegistry(schemaDirectory);
  const valid = {
    schemaVersion: 1,
    operationId: "018f6f2c-89ab-7def-8123-456789abcdef",
    digest: "a".repeat(64),
    createdAt: "2026-07-24T00:00:00.000Z",
  };
  assert.doesNotThrow(() => validateSource(registry, "common", valid));
  for (const [field, value] of [
    ["schemaVersion", 2],
    ["operationId", "short"],
    ["digest", "a".repeat(63)],
    ["digest", "A".repeat(64)],
    ["createdAt", "2026-07-24"],
  ]) {
    assert.throws(() => validateSource(registry, "common", { ...valid, [field]: value }), /validation failed/);
  }
});

test("registry rejects malformed, missing, duplicate, and unexpected fixed schema IDs", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "riot-schema-"));
  await cp(schemaDirectory, temporary, { recursive: true });
  await writeFile(join(temporary, "broken.schema.json"), "{", "utf8");
  await assert.rejects(() => loadSchemaRegistry(temporary), /malformed schema JSON/);

  const productPath = join(temporary, "product.schema.json");
  await rm(join(temporary, "broken.schema.json"));
  const product = JSON.parse(await readFile(productPath, "utf8"));
  delete product.$id;
  await writeFile(productPath, `${JSON.stringify(product)}\n`);
  await assert.rejects(() => loadSchemaRegistry(temporary), /fixed \$id/);

  product.$id = "https://riot.protest.net/release/schemas/privacy.v1.json";
  await writeFile(productPath, `${JSON.stringify(product)}\n`);
  await assert.rejects(() => loadSchemaRegistry(temporary), /fixed \$id|duplicate/);
});

test("toolchain schema requires a non-null authoritative checksum", async () => {
  const registry = await loadSchemaRegistry(schemaDirectory);
  const valid = {
    schemaVersion: 1,
    tools: [{
      name: "node",
      version: "26.4.0",
      versionCommand: ["node", "--version"],
      checksumKind: "normalized-version-output",
      sha256: "a".repeat(64),
      source: "package.json#engines.node",
    }],
  };
  assert.doesNotThrow(() => validateSource(registry, "toolchains", valid));
  assert.throws(
    () => validateSource(registry, "toolchains", {
      ...valid,
      tools: [{ ...valid.tools[0], sha256: null }],
    }),
    /validation failed/,
  );
  assert.throws(
    () => validateSource(registry, "toolchains", {
      ...valid,
      tools: [...valid.tools, { ...valid.tools[0] }],
    }),
    /duplicate tool/,
  );
});
