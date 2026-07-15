// Unit 0C — keeps the hostile-egress fixture's vector enumeration complete and
// honest. The fixture (scripts/apps/fixtures/hostile-egress.html) is the shared
// attacker artifact loaded by the iOS and Android native containment tests. Its
// value is only as good as its coverage: a vector that is declared but never
// fired, or fired but not declared, would silently shrink the attack surface the
// native backstop is proven against. This runs under `node --test` — no browser.

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const fixtureURL = new URL("../fixtures/hostile-egress.html", import.meta.url);
const fixture = await readFile(fileURLToPath(fixtureURL), "utf8");

// Every browser exfiltration/escape vector the plan enumerates for Unit 0C.
// This is the source of truth the fixture must match exactly.
const REQUIRED_VECTORS = [
  "fetch",
  "xhr",
  "websocket",
  "eventsource",
  "sendbeacon",
  "img",
  "script",
  "stylesheet-link",
  "iframe",
  "form-action",
  "window-open",
  "location-assign",
  "location-replace",
  "webrtc",
  "css-url",
  "dns-prefetch",
  "preconnect",
  "favicon",
];

function declaredVectors() {
  const block = fixture.match(/const VECTORS = Object\.freeze\(\[([\s\S]*?)\]\)/);
  assert.ok(block, "fixture must declare a frozen VECTORS list");
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

function firedVectors() {
  return [...fixture.matchAll(/case "([^"]+)":/g)].map((m) => m[1]);
}

test("the fixture carries no Content-Security-Policy", () => {
  // The whole premise of Unit 0C: the attacker controls the page and strips CSP.
  // If this fixture ever grew a CSP, the native tests would be proving the wrong
  // thing (that CSP works), not that the independent backstop works. Match a real
  // declaration (a <meta http-equiv> policy), not the phrase appearing in prose.
  assert.doesNotMatch(
    fixture,
    /http-equiv\s*=\s*["']?content-security-policy/i,
    "hostile fixture must not declare a CSP meta tag — containment must hold without it",
  );
});

test("the declared vector list is exactly the required set", () => {
  const declared = declaredVectors();
  assert.deepEqual(
    [...declared].sort(),
    [...REQUIRED_VECTORS].sort(),
    "fixture VECTORS must match the plan's enumerated vectors exactly",
  );
});

test("every declared vector is actually fired in the runner", () => {
  const declared = new Set(declaredVectors());
  const fired = new Set(firedVectors());
  for (const vector of declared) {
    assert.ok(fired.has(vector), `vector "${vector}" is declared but never fired`);
  }
});

test("no vector is fired that is not declared", () => {
  const declared = new Set(declaredVectors());
  for (const vector of firedVectors()) {
    assert.ok(declared.has(vector), `vector "${vector}" is fired but not declared`);
  }
});

test("the fixture exposes the attacker entry point and vector list", () => {
  assert.match(fixture, /window\.__attemptEgress\s*=/, "must expose __attemptEgress(target)");
  assert.match(fixture, /window\.__RIOT_EGRESS_VECTORS\s*=/, "must expose __RIOT_EGRESS_VECTORS");
});
