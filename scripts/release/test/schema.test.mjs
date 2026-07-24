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

const validToolchain = {
  name: "gradle",
  version: "9.1.0",
  versionCommand: ["./gradlew", "--version"],
  checksumKind: "artifact",
  artifactEvidenceState: "current",
  sha256: "a".repeat(64),
  source: "apps/android/gradle/wrapper/gradle-wrapper.properties#distributionUrl",
  downloadUrl: "https://services.gradle.org/distributions/gradle-9.1.0-bin.zip",
  downloadChecksum: { algorithm: "sha256", value: "a".repeat(64) },
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
  assert.throws(
    () => validateSource(registry, "missing", validProduct),
    (error) => /unknown schema/.test(error.message)
      && error.diagnostics[0].pointer === "/"
      && error.diagnostics[0].observed === "missing",
  );
  assert.throws(
    () => validateSource(registry, "product", null),
    (error) => error.diagnostics.some(({ pointer }) => pointer === "/"),
  );
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

  await cp(schemaDirectory, temporary, { recursive: true, force: true });
  await rm(join(temporary, "claims.schema.json"));
  await assert.rejects(() => loadSchemaRegistry(temporary), /missing schema file/);

  await cp(schemaDirectory, temporary, { recursive: true, force: true });
  await writeFile(join(temporary, "unexpected.schema.json"), `${JSON.stringify({
    $schema: "https://json-schema.org/draft/2020-12/schema",
    $id: "https://riot.protest.net/release/schemas/unexpected.v1.json",
    type: "object",
  })}\n`);
  await assert.rejects(() => loadSchemaRegistry(temporary), /no registered ID/);
});

test("toolchain schema requires a non-null authoritative checksum", async () => {
  const registry = await loadSchemaRegistry(schemaDirectory);
  const valid = {
    schemaVersion: 1,
    tools: [validToolchain],
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
      tools: [{ ...validToolchain, downloadUrl: "android-ndk.zip" }],
    }),
    /validation failed/,
  );
  assert.throws(
    () => validateSource(registry, "toolchains", {
      ...valid,
      tools: [{
        ...validToolchain,
        downloadChecksum: { algorithm: "sha1", value: "a".repeat(40) },
      }],
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
  assert.throws(
    () => validateSource(registry, "toolchains", {
      ...valid,
      tools: [
        valid.tools[0],
        { ...valid.tools[0], name: "npm" },
      ],
    }),
    /duplicate tool versionCommand/,
  );
});

test("checked-in toolchain authority covers every WU-000 required tool", async () => {
  const registry = await loadSchemaRegistry(schemaDirectory);
  const manifestPath = join(repositoryRoot, "release", "toolchains.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const validated = validateSource(registry, "toolchains", manifest);
  const names = new Set(validated.tools.map(({ name }) => name));
  for (const name of [
    "node", "npm", "rustc", "cargo", "gradle", "android-sdk", "android-ndk",
    "xcode", "swift", "ajv", "c8",
  ]) {
    assert(names.has(name), `missing toolchain authority for ${name}`);
  }
  assert(validated.tools.every(({ sha256 }) => /^[0-9a-f]{64}$/.test(sha256)));
  const ndk = validated.tools.find(({ name }) => name === "android-ndk");
  assert.equal(ndk.version, "28.2.13676358");
  assert.equal(ndk.source, "scripts/conference/build-native-core.sh:6");
  assert.equal(ndk.downloadUrl, "https://dl.google.com/android/repository/android-ndk-r28c-darwin.zip");
  assert.equal(ndk.artifactEvidenceState, "blocked");
  assert.equal(ndk.publishedChecksum.algorithm, "sha1");
  assert.equal(ndk.publishedChecksum.value, "fc20a6bf15a30fb3428c9b60a7308793a362dc6d");
});
