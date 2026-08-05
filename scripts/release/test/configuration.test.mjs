import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as realFs from "node:fs/promises";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { tempReleaseRoot } from "./helpers.mjs";
import { evaluateConfiguration, evaluateSnapshotFreshness } from "../configuration.mjs";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

const SNAPSHOT_SOURCE = "release/source/configuration-snapshot.json";

function todaySnapshot() {
  return {
    ios: {
      bundleId: "net.protest.riot",
      marketingVersion: "0.1",
      buildInjection: "commit-count",
      icon: "present",
      privacyManifest: false,
    },
    macos: {
      bundleId: "net.protest.riot",
      marketingVersion: "0.1.0",
      buildInjection: "commit-count",
      icon: "missing",
      privacyManifest: false,
      signing: "missing",
      architecture: "apple-silicon",
    },
    android: {
      applicationId: "org.riot.evidence",
      namespace: "org.riot.evidence",
      versionName: "0.1",
      versionCode: 1,
      icons: "missing",
      signing: "missing",
    },
  };
}

function readySnapshot() {
  return {
    ios: {
      bundleId: "net.protest.riot",
      marketingVersion: "1.0",
      buildInjection: "explicit",
      icon: "present",
      privacyManifest: true,
    },
    macos: {
      bundleId: "net.protest.riot",
      marketingVersion: "1.0",
      buildInjection: "explicit",
      icon: "present",
      privacyManifest: true,
      signing: "distribution",
      architecture: "apple-silicon",
    },
    android: {
      applicationId: "net.protest.riot",
      namespace: "org.riot.evidence",
      versionName: "1.0",
      versionCode: 7,
      icons: "present",
      signing: "release-external",
    },
  };
}

function gate(gates, id) {
  const found = gates.find((candidate) => candidate.id === id);
  assert.ok(found, `expected gate ${id}`);
  for (const field of ["id", "state", "sourceFile", "pointer", "observed", "expected", "recovery"]) {
    assert.notEqual(found[field], undefined, `gate ${id} missing ${field}`);
  }
  return found;
}

test("a fully ready snapshot passes every configuration gate", () => {
  const gates = evaluateConfiguration(readySnapshot());
  const failures = gates.filter((candidate) => candidate.state !== "PASS");
  assert.deepEqual(failures, [], `expected all PASS: ${JSON.stringify(failures)}`);
});

test("today's exact repository state reports its truthful BLOCKED set", () => {
  const gates = evaluateConfiguration(todaySnapshot());
  const expected = {
    "config.ios.version": "BLOCKED",
    "config.ios.bundleId": "PASS",
    "config.ios.icon": "PASS",
    "config.ios.privacyManifest": "BLOCKED",
    "config.macos.version": "BLOCKED",
    "config.macos.bundleId": "PASS",
    "config.macos.icon": "BLOCKED",
    "config.macos.privacyManifest": "BLOCKED",
    "config.macos.signing": "BLOCKED",
    "config.macos.architecture": "PASS",
    "config.android.applicationId": "BLOCKED",
    "config.android.version": "BLOCKED",
    "config.android.icons": "BLOCKED",
    "config.android.signing": "BLOCKED",
  };
  for (const [id, state] of Object.entries(expected)) {
    assert.equal(gate(gates, id).state, state, `${id} should be ${state}`);
    assert.equal(gate(gates, id).sourceFile, SNAPSHOT_SOURCE);
  }
});

test("commit-count build injection fails even at version 1.0", () => {
  const snapshot = readySnapshot();
  snapshot.ios.buildInjection = "commit-count";
  assert.equal(gate(evaluateConfiguration(snapshot), "config.ios.version").state, "BLOCKED");
});

test("wrong bundle and application identifiers fail closed", () => {
  const snapshot = readySnapshot();
  snapshot.ios.bundleId = "com.example.riot";
  snapshot.macos.bundleId = "com.example.riot";
  snapshot.android.applicationId = "org.riot.evidence";
  const gates = evaluateConfiguration(snapshot);
  assert.equal(gate(gates, "config.ios.bundleId").state, "BLOCKED");
  assert.equal(gate(gates, "config.macos.bundleId").state, "BLOCKED");
  assert.equal(gate(gates, "config.android.applicationId").state, "BLOCKED");
});

test("ad-hoc macOS candidate signing fails closed", () => {
  const snapshot = readySnapshot();
  snapshot.macos.signing = "ad-hoc";
  assert.equal(gate(evaluateConfiguration(snapshot), "config.macos.signing").state, "BLOCKED");
});

test("an Intel architecture claim fails closed", () => {
  const snapshot = readySnapshot();
  snapshot.macos.architecture = "intel-claim";
  assert.equal(gate(evaluateConfiguration(snapshot), "config.macos.architecture").state, "BLOCKED");
});

test("debug keys and daemon-delivered secrets fail closed for Android signing", () => {
  for (const signing of ["debug", "daemon-secret", "missing"]) {
    const snapshot = readySnapshot();
    snapshot.android.signing = signing;
    assert.equal(
      gate(evaluateConfiguration(snapshot), "config.android.signing").state,
      "BLOCKED",
      `${signing} must fail`,
    );
  }
});

test("missing privacy manifests fail closed per platform", () => {
  const snapshot = readySnapshot();
  snapshot.ios.privacyManifest = false;
  snapshot.macos.privacyManifest = false;
  const gates = evaluateConfiguration(snapshot);
  assert.equal(gate(gates, "config.ios.privacyManifest").state, "BLOCKED");
  assert.equal(gate(gates, "config.macos.privacyManifest").state, "BLOCKED");
});

test("snapshot freshness re-hashes the pinned native files", async () => {
  const root = await tempReleaseRoot();
  await mkdir(join(root, "apps", "ios"), { recursive: true });
  await mkdir(join(root, "apps", "android"), { recursive: true });
  await writeFile(join(root, "apps", "ios", "project.pbxproj"), "ios-bytes");
  await writeFile(join(root, "apps", "android", "build.gradle.kts"), "android-bytes");
  const snapshot = {
    fileHashes: {
      "apps/ios/project.pbxproj": sha256("ios-bytes"),
      "apps/android/build.gradle.kts": sha256("android-bytes"),
    },
  };
  const fresh = await evaluateSnapshotFreshness({ snapshot, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(fresh.id, "config.snapshot-freshness");
  assert.equal(fresh.state, "PASS");

  await writeFile(join(root, "apps", "android", "build.gradle.kts"), "drifted-bytes");
  const stale = await evaluateSnapshotFreshness({ snapshot, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(stale.state, "BLOCKED");
  assert.match(String(stale.observed), /build\.gradle\.kts/);
});

test("snapshot freshness fails closed when a pinned file disappears", async () => {
  const root = await tempReleaseRoot();
  const snapshot = { fileHashes: { "apps/ios/project.pbxproj": sha256("gone") } };
  const result = await evaluateSnapshotFreshness({ snapshot, repositoryRoot: root, fs: realFs, sha256 });
  assert.equal(result.state, "BLOCKED");
  assert.match(String(result.observed), /project\.pbxproj/);
});

test("the checked-in current snapshot is fresh against this repository", async () => {
  const repositoryRoot = new URL("../../..", import.meta.url).pathname;
  const snapshot = JSON.parse(
    await realFs.readFile(join(repositoryRoot, "release", "source", "configuration-snapshot.json"), "utf8"),
  );
  const result = await evaluateSnapshotFreshness({ snapshot, repositoryRoot, fs: realFs, sha256 });
  assert.equal(result.state, "PASS", `checked-in snapshot drifted: ${JSON.stringify(result)}`);
  const gates = evaluateConfiguration(snapshot);
  // PASS, not BLOCKED. The snapshot this kit shipped with recorded the Android
  // applicationId as `org.riot.evidence` and this test pinned the resulting
  // BLOCKED gate as an outstanding WU-006 task — but `net.protest.riot` had
  // already landed in #137, BEFORE the snapshot was authored in #155. The gate
  // was reporting finished work as undone. Ground truth is
  // apps/android/app/build.gradle.kts:14.
  assert.equal(gate(gates, "config.android.applicationId").state, "PASS");
  assert.equal(gate(gates, "config.ios.bundleId").state, "PASS");
});

test("freshness requires a sha256 dependency", async () => {
  await assert.rejects(
    evaluateSnapshotFreshness({ snapshot: { fileHashes: {} }, repositoryRoot: ".", fs: realFs }),
    /sha256 dependency is required/,
  );
});

test("freshness rejects traversal/absolute fileHashes keys without reading outside the root", async () => {
  const root = await tempReleaseRoot();
  const outside = join(root, "..", `outside-${process.pid}.txt`);
  await realFs.writeFile(outside, "secret");
  try {
    const digest = sha256("secret");
    for (const key of [`../outside-${process.pid}.txt`, "/etc/passwd", "apps\\..\\evil"]) {
      const result = await evaluateSnapshotFreshness({
        snapshot: { fileHashes: { [key]: digest } },
        repositoryRoot: root,
        fs: realFs,
        sha256,
      });
      assert.equal(result.state, "BLOCKED", `key must fail closed: ${key}`);
      assert.match(String(result.observed), new RegExp("unsafe snapshot path"));
    }
  } finally {
    await realFs.rm(outside, { force: true });
  }
});

test("snapshot schema rejects traversal and absolute fileHashes keys", async () => {
  const { loadSchemaRegistry, validateSource } = await import("../schema.mjs");
  const repositoryRoot = new URL("../../..", import.meta.url).pathname;
  const snapshot = JSON.parse(
    await realFs.readFile(join(repositoryRoot, "release", "source", "configuration-snapshot.json"), "utf8"),
  );
  const registry = await loadSchemaRegistry(join(repositoryRoot, "release", "schemas"));
  for (const key of ["../escape", "/abs/path", "a\\b"]) {
    const candidate = structuredClone(snapshot);
    candidate.fileHashes = { [key]: "0".repeat(64) };
    assert.throws(() => validateSource(registry, "configuration-snapshot", candidate), undefined, `schema must reject key: ${key}`);
  }
});
