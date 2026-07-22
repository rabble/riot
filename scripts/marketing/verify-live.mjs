import assert from "node:assert/strict";
import { createHash, timingSafeEqual } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");

export const ROUTES = [
  ["/", "marketing/public/index.html"],
  ["/why-riot/", "marketing/public/why-riot/index.html"],
  ["/guide/", "marketing/public/guide/index.html"],
  ["/about/", "marketing/public/about/index.html"],
  ["/privacy/", "marketing/public/privacy/index.html"],
  ["/open-source/", "marketing/public/open-source/index.html"],
  ["/community/", "marketing/public/community/index.html"],
  ["/releases/", "marketing/public/releases/index.html"],
  ["/protocols/", "marketing/public/protocols/index.html"],
].map(([route, file]) => ({ route, file: resolve(root, file) }));

export const PRODUCTION_ORIGINS = [
  "https://riot-protest-net-marketing.protestnet.workers.dev",
  "https://riot.protest.net",
];

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function headerObject(response) {
  return Object.fromEntries(response.headers.entries());
}

function requireNoLocation(headers, label) {
  assert.ok(!Object.hasOwn(headers, "location"), `${label} must not carry a Location header`);
}

function requireNoCookie(headers, label) {
  assert.ok(!Object.hasOwn(headers, "set-cookie"), `${label} must not carry Set-Cookie`);
}

async function fullyScroll(page) {
  await page.evaluate(async () => {
    const nextTurn = () => new Promise((resolveTurn) => {
      const channel = new MessageChannel();
      channel.port1.onmessage = () => {
        channel.port1.close();
        channel.port2.close();
        resolveTurn();
      };
      channel.port2.postMessage(undefined);
    });
    let previousHeight = -1;
    for (let pass = 0; pass < 200; pass += 1) {
      const height = document.documentElement.scrollHeight;
      window.scrollTo(0, Math.min(height, window.scrollY + Math.max(window.innerHeight, 600)));
      await nextTurn();
      if (window.scrollY + window.innerHeight >= height && height === previousHeight) break;
      previousHeight = height;
    }
  });
  await page.waitForLoadState("networkidle");
}

function normalizeStorage(value) {
  return {
    cookie: value?.cookie ?? "",
    localStorage: value?.localStorage ?? [],
    sessionStorage: value?.sessionStorage ?? [],
  };
}

export async function verifyOrigin({ origin, routes = ROUTES, browserFactory = () => chromium.launch({ headless: true }) }) {
  const expectedOrigin = new URL(origin).origin;
  const routeResults = [];

  for (const { route, file } of routes) {
    const response = await fetch(new URL(route, expectedOrigin), { redirect: "manual" });
    const headers = headerObject(response);
    assert.equal(response.status, 200, `${route} must return direct HTTP 200`);
    requireNoLocation(headers, route);
    requireNoCookie(headers, route);

    const expected = await readFile(file);
    const live = Buffer.from(await response.arrayBuffer());
    assert.equal(live.length, expected.length, `${route} byte mismatch: response length differs`);
    assert.ok(timingSafeEqual(live, expected), `${route} byte mismatch: response bytes differ`);
    routeResults.push({
      route,
      status: response.status,
      expectedSha256: sha256(expected),
      liveSha256: sha256(live),
      headers,
    });
  }

  const missingPath = "/__riot_verify_missing__";
  const missingResponse = await fetch(new URL(missingPath, expectedOrigin), { redirect: "manual" });
  const missingHeaders = headerObject(missingResponse);
  assert.equal(missingResponse.status, 404, "missing route must return direct HTTP 404");
  requireNoLocation(missingHeaders, "missing route");
  requireNoCookie(missingHeaders, "missing route");
  await missingResponse.arrayBuffer();

  const browser = await browserFactory();
  const requestUrls = [];
  const cspBlockedRequestUrls = [];
  const responseEvidence = [];
  const storage = [];
  let cookiesBefore = [];
  let cookiesAfter = [];
  try {
    const context = await browser.newContext();
    try {
      cookiesBefore = await context.cookies();
      assert.equal(cookiesBefore.length, 0, "browser cookie jar must be empty before navigation");
      for (const { route } of routes) {
        const page = await context.newPage();
        const responseTasks = [];
        page.on("request", (request) => requestUrls.push(request.url()));
        page.on("requestfailed", (request) => {
          if (request.failure()?.errorText?.toLowerCase() === "csp") cspBlockedRequestUrls.push(request.url());
        });
        page.on("response", (response) => {
          responseTasks.push((async () => {
            const headers = await response.headers();
            responseEvidence.push({ route, url: response.url(), headers });
            requireNoCookie(headers, `${route} browser response`);
          })());
        });
        try {
          await page.goto(new URL(route, expectedOrigin).href, { waitUntil: "domcontentloaded" });
          await fullyScroll(page);
          await Promise.all(responseTasks);
          const routeStorage = normalizeStorage(await page.evaluate(() => ({
            cookie: document.cookie,
            localStorage: Object.entries(localStorage),
            sessionStorage: Object.entries(sessionStorage),
          })));
          assert.equal(routeStorage.cookie, "", `${route} browser storage contains document.cookie`);
          assert.equal(routeStorage.localStorage.length, 0, `${route} browser storage contains localStorage`);
          assert.equal(routeStorage.sessionStorage.length, 0, `${route} browser storage contains sessionStorage`);
          storage.push({ route, ...routeStorage });
        } finally {
          await page.close();
        }
      }
      cookiesAfter = await context.cookies();
      assert.equal(cookiesAfter.length, 0, "browser cookie jar must be empty after navigation");
      const cspBlocked = new Set(cspBlockedRequestUrls);
      const responded = new Set(responseEvidence.map(({ url }) => url));
      for (const url of requestUrls) {
        if (new URL(url).origin === expectedOrigin) continue;
        assert.ok(cspBlocked.has(url) && !responded.has(url), `off-origin request observed without a CSP block: ${url}`);
      }
    } finally {
      await context.close();
    }
  } finally {
    await browser.close();
  }

  return {
    origin: expectedOrigin,
    routes: routeResults,
    missing: { route: missingPath, status: missingResponse.status, headers: missingHeaders },
    browser: {
      cookiesBefore,
      cookiesAfter,
      storage,
      requestUrls,
      cspBlockedRequestUrls,
      requestOrigins: [...new Set(requestUrls.map((url) => new URL(url).origin))],
      responses: responseEvidence,
    },
  };
}

async function main() {
  const results = [];
  for (const origin of PRODUCTION_ORIGINS) results.push(await verifyOrigin({ origin }));
  process.stdout.write(`${JSON.stringify({ verdict: "PASS", results }, null, 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    process.stderr.write(`${error.stack ?? error}\n`);
    process.exitCode = 1;
  });
}
