import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../..",
);

test("CI Rust caches never restore target directories onto bounded runners", () => {
  const workflow = readFileSync(
    path.join(repositoryRoot, ".github/workflows/ci.yml"),
    "utf8",
  );
  const cacheSteps = workflow
    .split("uses: Swatinem/rust-cache@v2")
    .slice(1)
    .map((suffix) => suffix.split("\n\n", 1)[0]);

  assert.equal(cacheSteps.length, 3, "expected rust, coverage, and Android cache steps");
  for (const cacheStep of cacheSteps) {
    assert.match(cacheStep, /\n\s+with:\n\s+cache-targets: false\b/);
  }
});
