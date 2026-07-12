#!/usr/bin/env node

import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import { once } from "node:events";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const tokensPath = path.join(repoRoot, "fixtures/apps/_shared/tokens.css");
const previewHostPath = path.join(repoRoot, "scripts/apps/miniapp-preview-host.mjs");
const APPS = [
  ["supply-board", "Needs & Offers"],
  ["roll-call", "Events"],
  ["quick-poll", "Decisions"],
];

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
assert.match(tokens, /--focus-ring\s*:/, "tokens must define a dedicated focus-ring color");
assert.match(tokens, /:focus-visible\s*\{[^}]*outline\s*:\s*3px\s+solid\s+var\(--focus-ring\)/s, "keyboard focus must use a visible 3px focus ring");
assert.match(tokens, /@media\s*\(prefers-reduced-motion\s*:\s*reduce\)/, "motion must honor reduced-motion preferences");

const frozenChecklist = path.join(repoRoot, "fixtures/apps/checklist");
const frozenChecklistManifest = JSON.parse(await readFile(path.join(frozenChecklist, "riot-app.json"), "utf8"));
assert.equal(frozenChecklistManifest.name, "Checklist", "the demo Checklist identity stays frozen");
for (const file of ["riot-app.json", "index.html", "style.css", "app.js"]) await access(path.join(frozenChecklist, file));

for (const [app, name] of APPS) {
  const directory = path.join(repoRoot, "fixtures/apps", app);
  for (const file of ["riot-app.json", "index.html", "tokens.css", "style.css", "app.js"]) {
    await access(path.join(directory, file));
  }
  const manifest = JSON.parse(await readFile(path.join(directory, "riot-app.json"), "utf8"));
  assert.equal(manifest.name, name, `${app} must use its approved visible name`);
  const html = await readFile(path.join(directory, "index.html"), "utf8");
  assert.match(html, /href=["']tokens\.css["'][\s\S]*href=["']style\.css["']/, `${app} must load tokens before app styles`);
  const source = await readFile(path.join(directory, "app.js"), "utf8");
  for (const operation of ["riot.watch", "riot.whoami", "riot.profile", "ensureSeeded"]) {
    assert.match(source, new RegExp(operation.replace(".", "\\.")), `${app} must use ${operation}`);
  }
  assert.doesNotMatch(source, /innerHTML\s*=/, `${app} must build untrusted content safely`);
  assert.doesNotMatch(source, /\bfetch\s*\(/, `${app} must not use the network`);
}

function rgb(hex) {
  const value = Number.parseInt(hex.slice(1), 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

function luminance(hex) {
  const channels = rgb(hex).map((channel) => {
    const normalized = channel / 255;
    return normalized <= 0.04045
      ? normalized / 12.92
      : ((normalized + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

function contrast(left, right) {
  const [lighter, darker] = [luminance(left), luminance(right)].sort((a, b) => b - a);
  return (lighter + 0.05) / (darker + 0.05);
}

const darkTokens = tokens.match(/@media\s*\(prefers-color-scheme:\s*dark\)\s*\{([\s\S]*?)\n\}/)?.[1] || "";
const darkPaper = darkTokens.match(/--paper:\s*(#[0-9a-f]{6})/i)?.[1];
const darkSurface = darkTokens.match(/--surface:\s*(#[0-9a-f]{6})/i)?.[1];
const darkAccent = darkTokens.match(/--accent:\s*(#[0-9a-f]{6})/i)?.[1];
const darkFocus = darkTokens.match(/--focus-ring:\s*(#[0-9a-f]{6})/i)?.[1];
assert(darkPaper && darkSurface && darkAccent && darkFocus, "dark mode must define paper, surface, accent, and focus-ring colors");
assert(contrast(darkAccent, darkPaper) >= 3, "dark accent must have at least 3:1 contrast against paper");
assert(contrast(darkAccent, darkSurface) >= 3, "dark accent must have at least 3:1 contrast against surface");
assert(contrast(darkFocus, darkPaper) >= 3, "dark focus ring must have at least 3:1 contrast against paper");

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
  assert.equal(
    page.headers.get("content-security-policy"),
    "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:",
    "preview CSP must match the production app host",
  );
  const html = await page.text();
  assert.match(html, /data-miniapp-preview-bridge/, "preview HTML must inject the mock bridge");
  const injectedScript = html.match(/<script[^>]*data-miniapp-preview-bridge[^>]*>([\s\S]*?)<\/script>/i);
  assert(injectedScript && /\ssrc=/.test(injectedScript[0]) && injectedScript[1].trim() === "", "preview bridge must be an external script");

  const bridgeResponse = await fetch(`${origin}/apps/checklist/__miniapp-preview-bridge.js?state=seeded`);
  assert.equal(bridgeResponse.status, 200, "preview bridge must be served as a same-origin resource");
  assert.match(bridgeResponse.headers.get("content-type") || "", /^text\/javascript\b/);
  const bridgeSource = await bridgeResponse.text();
  for (const operation of ["get", "put", "list", "watch", "whoami", "profile"]) {
    assert.match(bridgeSource, new RegExp(`\\b${operation}\\b`), `mock bridge must implement ${operation}`);
  }

  const context = vm.createContext({ queueMicrotask, window: {} });
  vm.runInContext(bridgeSource, context);
  const evaluate = (source) => vm.runInContext(source, context);
  assert.equal(await evaluate("window.riot.get('items/missing')"), null, "missing get must resolve null");
  assert.equal(await evaluate("window.riot.put('a'.repeat(256), {}).then(() => true)"), true, "a 256-byte component must be accepted");
  assert.equal(await evaluate("window.riot.put(Array(62).fill('a').join('/'), {}).then(() => true)"), true, "62 key segments must be accepted");
  assert.equal(
    await evaluate("window.riot.put([...Array(7).fill('a'.repeat(256)), 'a'.repeat(220)].join('/'), {}).then(() => true)"),
    true,
    "a path totaling 2,048 component bytes must be accepted",
  );
  assert.equal(
    await evaluate("window.riot.put('items/json-roundtrip', { text: 'hello', nested: [1, true, null] }).then(window.riot.get.bind(null, 'items/json-roundtrip')).then(JSON.stringify)"),
    '{"text":"hello","nested":[1,true,null]}',
    "values must round-trip through JSON",
  );
  assert.equal(
    await evaluate("window.riot.put('items/date', new Date('2026-07-12T12:34:56.000Z')).then(window.riot.get.bind(null, 'items/date'))"),
    "2026-07-12T12:34:56.000Z",
    "Date must round-trip through its JSON ISO string",
  );
  assert.equal(
    await evaluate("window.riot.put('items/map', new Map([['ignored', true]])).then(window.riot.get.bind(null, 'items/map')).then(JSON.stringify)"),
    "{}",
    "Map must round-trip through JSON as an empty object",
  );
  assert.equal(
    await evaluate("window.riot.put('items/nan', NaN).then(window.riot.get.bind(null, 'items/nan')).then(JSON.stringify)"),
    "null",
    "NaN must round-trip through JSON as null",
  );
  assert.equal(
    await evaluate("window.riot.put('items/omitted', { kept: 1, omitted: undefined }).then(window.riot.get.bind(null, 'items/omitted')).then(JSON.stringify)"),
    '{"kept":1}',
    "undefined object properties must be omitted like JSON.stringify",
  );
  for (const expression of [
    "window.riot.put('Items/uppercase', {})",
    "window.riot.put('items/under_score', {})",
    "window.riot.put('items/' + 'a'.repeat(257), {})",
    "window.riot.put(Array(63).fill('a').join('/'), {})",
    "window.riot.put(Array(9).fill('a'.repeat(250)).join('/'), {})",
    "window.riot.list('Items')",
    "window.riot.put('items/bigint', 1n)",
    "(() => { const value = {}; value.self = value; return window.riot.put('items/cyclic', value); })()",
    "window.riot.put('items/undefined', undefined)",
  ]) {
    await assert.rejects(evaluate(expression), undefined, `preview bridge must reject: ${expression}`);
  }

  const me = await evaluate("window.riot.whoami()");
  assert.match(me.id, /^[0-9a-f]{64}$/);
  assert.match(me.tag, /^[0-9a-f]{8}$/);
  assert.equal(me.tag, me.id.slice(0, 8));
  const seededIdentities = JSON.parse(await evaluate("window.riot.list('items').then((rows) => JSON.stringify(rows.filter((row) => row.value && row.value.updated_by_id).map((row) => row.value.updated_by_id)))"));
  assert(seededIdentities.length >= 2 && seededIdentities.every((id) => /^[0-9a-f]{64}$/.test(id)), "seed rows must use full valid profile IDs");
  const peer = await evaluate(`window.riot.profile('${"c".repeat(64)}')`);
  assert.equal(peer.displayName, "member");
  assert.equal(peer.tag, "cccccccc");
  await assert.rejects(evaluate("window.riot.profile('not-an-id')"));

  evaluate("window.__watchCalls = []; window.__watchReturn = window.riot.watch('items/', (rows) => window.__watchCalls.push(rows.length))");
  await new Promise(setImmediate);
  assert.equal(evaluate("window.__watchReturn"), undefined, "watch must return undefined");
  assert.equal(evaluate("window.__watchCalls.length"), 1, "watch must make an initial callback");
  await evaluate("window.riot.put('items/watch-update', { ok: true })");
  await new Promise(setImmediate);
  assert.equal(evaluate("window.__watchCalls.length"), 2, "put must trigger a watcher callback");
  evaluate("window.riot.watch('items', async () => { throw new Error('callback failure'); })");
  await new Promise(setImmediate);
  await evaluate("window.riot.put('items/after-callback-failure', { ok: true })");
  await new Promise(setImmediate);

  const errorBridge = await fetch(`${origin}/apps/checklist/__miniapp-preview-bridge.js?state=error`).then((response) => response.text());
  const errorContext = vm.createContext({ queueMicrotask, window: {} });
  vm.runInContext(errorBridge, errorContext);
  await assert.rejects(vm.runInContext("window.riot.put('items/fails', {})", errorContext), /deterministic preview write failure/);

  const stylesheet = await fetch(`${origin}/apps/checklist/style.css`);
  assert.match(stylesheet.headers.get("content-type") || "", /^text\/css\b/);
  const traversal = await fetch(`${origin}/apps/checklist/%252e%252e/%252e%252e/Cargo.toml`);
  assert.equal(traversal.status, 404, "preview host must reject encoded traversal");
  const malformed = await fetch(`${origin}/apps/checklist/%E0%A4%A`);
  assert.equal(malformed.status, 400, "preview host must reject malformed encoding");
  const unsupported = await fetch(`${origin}/apps/checklist/`, { method: "POST" });
  assert.equal(unsupported.status, 405, "preview host must reject unsupported methods");
  const unknownStatePage = await fetch(`${origin}/apps/checklist/?state=unknown`).then((response) => response.text());
  assert.match(unknownStatePage, /__miniapp-preview-bridge\.js\?state=seeded/, "unknown state must fall back to seeded");
} finally {
  server.close();
  await once(server, "close");
}

console.log("miniapp contracts: PASS");
