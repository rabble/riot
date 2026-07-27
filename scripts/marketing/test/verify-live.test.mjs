import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { verifyOrigin } from "../verify-live.mjs";

test("deployment CSP blocks edge-injected scripts without breaking static page assets", async () => {
  const headers = await readFile(new URL("../../../marketing/public/_headers", import.meta.url), "utf8");
  assert.match(headers, /^\/\*$/m);
  assert.match(headers, /Content-Security-Policy:[^\n]*default-src 'self'/i);
  assert.match(headers, /script-src 'none'/i);
  assert.match(headers, /style-src 'self' 'unsafe-inline'/i);
  assert.match(headers, /img-src 'self' data:/i);
  assert.match(headers, /connect-src 'self'/i);
  assert.match(headers, /frame-ancestors 'none'/i);
});

async function fixture({ body = "expected\n", routeStatus = 200, routeHeaders = {}, missingHeaders = {} } = {}) {
  const directory = await mkdtemp(join(tmpdir(), "riot-live-verifier-"));
  const file = join(directory, "index.html");
  await writeFile(file, "expected\n");
  const server = createServer((request, response) => {
    if (request.url === "/") {
      response.writeHead(routeStatus, routeHeaders);
      response.end(body);
      return;
    }
    response.writeHead(404, missingHeaders);
    response.end("missing");
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    file,
    origin: `http://127.0.0.1:${port}`,
    routes: [{ route: "/", file }],
    async close() {
      await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
      await rm(directory, { recursive: true, force: true });
    },
  };
}

function fakeBrowserFactory({ cookies = [], storage = { cookie: "", localStorage: [], sessionStorage: [] }, extraRequest, extraRequestFailure } = {}) {
  const observations = { contexts: 0, listenerBeforeGoto: true, gotos: [], scrolls: 0, evaluateSources: [], networkIdleWaits: 0, closed: false };
  const factory = async () => ({
    async newContext() {
      observations.contexts += 1;
      return {
        async cookies() { return cookies; },
        async newPage() {
          const listeners = new Map();
          return {
            on(name, callback) { listeners.set(name, callback); },
            async goto(url) {
              observations.listenerBeforeGoto &&= listeners.has("request") && listeners.has("response");
              observations.gotos.push(url);
              listeners.get("request")?.({ url: () => url });
              if (extraRequest) {
                const request = { url: () => extraRequest, failure: () => extraRequestFailure ? { errorText: extraRequestFailure } : null };
                listeners.get("request")?.(request);
                if (extraRequestFailure) listeners.get("requestfailed")?.(request);
              }
              listeners.get("response")?.({ url: () => url, headers: async () => ({ "content-type": "text/html" }) });
            },
            async evaluate(fn) {
              observations.evaluateSources.push(String(fn));
              if (String(fn).includes("localStorage")) return storage;
              observations.scrolls += 1;
              return undefined;
            },
            async waitForLoadState(state) {
              assert.equal(state, "networkidle");
              observations.networkIdleWaits += 1;
            },
            async close() {},
          };
        },
        async close() {},
      };
    },
    async close() { observations.closed = true; },
  });
  return { factory, observations };
}

async function withFixture(options, run) {
  const site = await fixture(options);
  try { await run(site); } finally { await site.close(); }
}

test("accepts exact direct bytes and fully inspects one fresh browser context", async () => {
  await withFixture({}, async (site) => {
    const browser = fakeBrowserFactory();
    const result = await verifyOrigin({ ...site, browserFactory: browser.factory });
    assert.equal(result.routes[0].status, 200);
    assert.equal(result.routes[0].expectedSha256, result.routes[0].liveSha256);
    assert.equal(result.missing.status, 404);
    assert.equal(browser.observations.contexts, 1);
    assert.equal(browser.observations.listenerBeforeGoto, true);
    assert.deepEqual(browser.observations.gotos, [`${site.origin}/`]);
    assert.equal(browser.observations.scrolls, 1);
    assert.doesNotMatch(browser.observations.evaluateSources[0], /setTimeout/, "full scroll must not impose a fixed delay at every viewport step");
    assert.doesNotMatch(browser.observations.evaluateSources[0], /requestAnimationFrame/, "full scroll must not depend on throttled animation frames in headless mode");
    assert.match(browser.observations.evaluateSources[0], /MessageChannel/, "full scroll must yield event-loop turns for lazy-load observers");
    assert.equal(browser.observations.networkIdleWaits, 1);
    assert.equal(browser.observations.closed, true);
  });
});

test("rejects redirects and Location headers on canonical or missing routes", async () => {
  await withFixture({ routeStatus: 302, routeHeaders: { Location: "/elsewhere" } }, async (site) => {
    await assert.rejects(verifyOrigin({ ...site, browserFactory: fakeBrowserFactory().factory }), /direct HTTP 200/i);
  });
  await withFixture({ routeHeaders: { Location: "/elsewhere" } }, async (site) => {
    await assert.rejects(verifyOrigin({ ...site, browserFactory: fakeBrowserFactory().factory }), /Location/i);
  });
  await withFixture({ missingHeaders: { Location: "/" } }, async (site) => {
    await assert.rejects(verifyOrigin({ ...site, browserFactory: fakeBrowserFactory().factory }), /missing route.*Location/i);
  });
});

test("rejects a live byte mismatch", async () => {
  await withFixture({ body: "different\n" }, async (site) => {
    await assert.rejects(verifyOrigin({ ...site, browserFactory: fakeBrowserFactory().factory }), /byte mismatch/i);
  });
});

test("rejects Set-Cookie from a canonical response", async () => {
  await withFixture({ routeHeaders: { "Set-Cookie": "tracker=yes" } }, async (site) => {
    await assert.rejects(verifyOrigin({ ...site, browserFactory: fakeBrowserFactory().factory }), /Set-Cookie/i);
  });
});

test("rejects browser cookies", async () => {
  await withFixture({}, async (site) => {
    const browser = fakeBrowserFactory({ cookies: [{ name: "tracker", value: "yes" }] });
    await assert.rejects(verifyOrigin({ ...site, browserFactory: browser.factory }), /cookie jar/i);
  });
});

test("rejects browser storage", async () => {
  await withFixture({}, async (site) => {
    const browser = fakeBrowserFactory({ storage: { cookie: "", localStorage: [["key", "value"]], sessionStorage: [] } });
    await assert.rejects(verifyOrigin({ ...site, browserFactory: browser.factory }), /browser storage/i);
  });
});

test("rejects off-origin browser requests", async () => {
  await withFixture({}, async (site) => {
    const browser = fakeBrowserFactory({ extraRequest: "https://tracker.example/pixel" });
    await assert.rejects(verifyOrigin({ ...site, browserFactory: browser.factory }), /off-origin request/i);
  });
});

test("accepts an edge-injected script only when CSP blocks it before any response", async () => {
  await withFixture({}, async (site) => {
    const url = "https://static.cloudflareinsights.com/beacon.min.js/example";
    const browser = fakeBrowserFactory({ extraRequest: url, extraRequestFailure: "csp" });
    const result = await verifyOrigin({ ...site, browserFactory: browser.factory });
    assert.deepEqual(result.browser.cspBlockedRequestUrls, [url]);
    assert.ok(!result.browser.responses.some((response) => response.url === url));
  });
});
