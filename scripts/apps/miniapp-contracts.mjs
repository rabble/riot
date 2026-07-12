#!/usr/bin/env node

import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import { once } from "node:events";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const tokensPath = path.join(repoRoot, "fixtures/apps/_shared/tokens.css");
const previewHostPath = path.join(repoRoot, "scripts/apps/miniapp-preview-host.mjs");

const missing = [];
for (const [label, file] of [
  ["shared token source", tokensPath],
  ["preview host", previewHostPath],
]) {
  try {
    await access(file);
  } catch {
    missing.push(label);
  }
}

assert.deepEqual(missing, [], `missing miniapp foundation: ${missing.join(", ")}`);

const tokens = await readFile(tokensPath, "utf8");
for (const token of [
  "--paper", "--surface", "--ink", "--muted", "--line", "--accent",
  "--radius", "--shadow", "--space-1", "--font-rounded",
]) {
  assert.match(tokens, new RegExp(`${token}\\s*:`), `shared tokens must define ${token}`);
}
assert.match(tokens, /color-scheme\s*:\s*light\s+dark/, "tokens must opt into light and dark controls");
assert.match(tokens, /box-sizing\s*:\s*border-box/, "tokens must use predictable box sizing");
assert.match(tokens, /body\s*\{[^}]*margin\s*:\s*0/s, "tokens must reset the body margin");
assert.match(tokens, /(?:button|input)[\s\S]*font\s*:\s*inherit/, "form controls must inherit typography");
assert.match(tokens, /min-height\s*:\s*44px\b/, "controls must be at least 44px tall");
assert.match(tokens, /:focus-visible\s*\{[^}]*outline\s*:\s*3px\s+solid\s+var\(--accent\)/s, "keyboard focus must use a visible 3px accent ring");
assert.match(tokens, /@media\s*\(prefers-reduced-motion\s*:\s*reduce\)/, "motion must honor reduced-motion preferences");

const { createPreviewServer } = await import("./miniapp-preview-host.mjs");
const server = createPreviewServer();
server.listen(0, "127.0.0.1");
await once(server, "listening");
try {
  const address = server.address();
  assert(address && typeof address === "object");
  const origin = `http://127.0.0.1:${address.port}`;
  const page = await fetch(`${origin}/apps/checklist/?state=seeded`);
  assert.equal(page.status, 200, "preview host must serve Checklist");
  assert.match(page.headers.get("content-type") || "", /^text\/html\b/);
  const html = await page.text();
  assert.match(html, /data-miniapp-preview-bridge/, "preview HTML must inject the mock bridge");
  for (const operation of ["get", "put", "list", "watch", "whoami", "profile"]) {
    assert.match(html, new RegExp(`\\b${operation}\\b`), `mock bridge must implement ${operation}`);
  }

  const stylesheet = await fetch(`${origin}/apps/checklist/style.css`);
  assert.match(stylesheet.headers.get("content-type") || "", /^text\/css\b/);
  const traversal = await fetch(`${origin}/apps/checklist/%252e%252e/%252e%252e/Cargo.toml`);
  assert.equal(traversal.status, 404, "preview host must reject encoded traversal");
} finally {
  server.close();
  await once(server, "close");
}

console.log("miniapp contracts: PASS");
