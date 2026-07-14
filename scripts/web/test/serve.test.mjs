import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { promises as fs } from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { startServer } from "../serve.mjs";

const CSP = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; worker-src 'self'; manifest-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";
const PERMISSIONS_POLICY = "accelerometer=(), ambient-light-sensor=(), autoplay=(), bluetooth=(), camera=(), display-capture=(), encrypted-media=(), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), publickey-credentials-create=(), publickey-credentials-get=(), serial=(), usb=(), xr-spatial-tracking=()";
const here = path.dirname(fileURLToPath(import.meta.url));
const serverPath = path.resolve(here, "../serve.mjs");

let temporaryDirectory;
let staticRoot;
let server;
let origin;

function request(requestPath, method = "GET", requestOrigin = origin) {
  const endpoint = new URL(requestOrigin);
  return new Promise((resolve, reject) => {
    const outgoing = http.request({
      hostname: endpoint.hostname,
      port: endpoint.port,
      method,
      path: requestPath,
    }, (response) => {
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => resolve({
        body: Buffer.concat(chunks),
        headers: response.headers,
        status: response.statusCode,
      }));
    });
    outgoing.on("error", reject);
    outgoing.end();
  });
}

function assertSecurityHeaders(response) {
  assert.equal(response.headers["content-security-policy"], CSP);
  assert.equal(response.headers["referrer-policy"], "no-referrer");
  assert.equal(response.headers["x-content-type-options"], "nosniff");
  assert.equal(response.headers["permissions-policy"], PERMISSIONS_POLICY);
  assert.equal(response.headers["access-control-allow-origin"], undefined);
}

function closeServer(runningServer) {
  return new Promise((resolve, reject) => {
    runningServer.close((error) => error ? reject(error) : resolve());
  });
}

async function capture(commandArguments, options = {}) {
  const child = spawn(process.execPath, commandArguments, {
    ...options,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => { stdout += chunk; });
  child.stderr.on("data", (chunk) => { stderr += chunk; });
  const [code, signal] = await once(child, "exit");
  return { code, signal, stderr, stdout };
}

async function runServerProcess(argumentsAfterScript, cwd) {
  const child = spawn(process.execPath, [serverPath, ...argumentsAfterScript], {
    cwd,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => { stdout += chunk; });
  child.stderr.on("data", (chunk) => { stderr += chunk; });
  const exit = once(child, "exit");
  try {
    const printedOrigin = await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error("server URL was not printed")), 5_000);
      child.once("error", (error) => {
        clearTimeout(timeout);
        reject(error);
      });
      child.stdout.on("data", () => {
        const newline = stdout.indexOf("\n");
        if (newline !== -1) {
          clearTimeout(timeout);
          resolve(stdout.slice(0, newline));
        }
      });
    });
    assert.match(printedOrigin, /^http:\/\/127\.0\.0\.1:\d+\/$/);
    const page = await request("/", "GET", printedOrigin);
    assert.equal(page.status, 200);
    return { child, exit, printedOrigin, readOutput: () => ({ stderr, stdout }) };
  } catch (error) {
    child.kill("SIGTERM");
    await exit;
    throw error;
  }
}

before(async () => {
  temporaryDirectory = await fs.mkdtemp(path.join(os.tmpdir(), "riot-web-server-"));
  staticRoot = path.join(temporaryDirectory, "public");
  await fs.mkdir(path.join(staticRoot, "directory"), { recursive: true });
  await fs.writeFile(path.join(staticRoot, "index.html"), "<!doctype html><title>Riot</title>\n");
  await fs.writeFile(path.join(staticRoot, "style.css"), "body {}\n");
  await fs.writeFile(path.join(staticRoot, "app.js"), "export {};\n");
  await fs.writeFile(path.join(staticRoot, "module.mjs"), "export {};\n");
  await fs.writeFile(path.join(staticRoot, "UPPER.JS"), "export {};\n");
  await fs.writeFile(path.join(staticRoot, "manifest.webmanifest"), "{}\n");
  await fs.writeFile(path.join(staticRoot, "data.json"), "{}\n");
  await fs.writeFile(path.join(staticRoot, "module.wasm"), Buffer.from([0, 97, 115, 109]));
  await fs.writeFile(path.join(staticRoot, "icon.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n");
  await fs.writeFile(path.join(staticRoot, "icon.png"), Buffer.from([137, 80, 78, 71]));
  await fs.writeFile(path.join(staticRoot, "favicon.ico"), Buffer.from([0, 0, 1, 0]));
  await fs.writeFile(path.join(staticRoot, "unknown.bin"), "do not serve\n");
  await fs.writeFile(path.join(staticRoot, "directory", "index.html"), "do not list me\n");
  await fs.writeFile(path.join(temporaryDirectory, "outside.txt"), "outside secret\n");
  await fs.symlink(path.join(temporaryDirectory, "outside.txt"), path.join(staticRoot, "outside-link.html"));
  await fs.symlink(temporaryDirectory, path.join(staticRoot, "outside-directory"));
  await fs.symlink(path.join(temporaryDirectory, "absent.txt"), path.join(staticRoot, "broken.html"));
  server = await startServer({ root: staticRoot, host: "127.0.0.1", port: 0 });
  const address = server.address();
  assert(address && typeof address === "object");
  origin = `http://127.0.0.1:${address.port}`;
});

after(async () => {
  if (server?.listening) await closeServer(server);
  if (temporaryDirectory) await fs.rm(temporaryDirectory, { recursive: true, force: true });
});

test("serves root bytes with exact security headers and ignores the query", async () => {
  const response = await request("/?cache-bust=1");
  assert.equal(response.status, 200);
  assert.equal(response.body.toString("utf8"), "<!doctype html><title>Riot</title>\n");
  assert.equal(response.headers["content-length"], String(response.body.byteLength));
  assertSecurityHeaders(response);
});

test("serves only the deterministic MIME allowlist", async () => {
  const cases = [
    ["/index.html", "text/html; charset=utf-8"],
    ["/style.css", "text/css; charset=utf-8"],
    ["/app.js", "text/javascript; charset=utf-8"],
    ["/module.mjs", "text/javascript; charset=utf-8"],
    ["/UPPER.JS", "text/javascript; charset=utf-8"],
    ["/manifest.webmanifest", "application/manifest+json; charset=utf-8"],
    ["/data.json", "application/json; charset=utf-8"],
    ["/module.wasm", "application/wasm"],
    ["/icon.svg", "image/svg+xml; charset=utf-8"],
    ["/icon.png", "image/png"],
    ["/favicon.ico", "image/x-icon"],
  ];
  for (const [requestPath, expected] of cases) {
    const response = await request(requestPath);
    assert.equal(response.status, 200, requestPath);
    assert.equal(response.headers["content-type"], expected, requestPath);
  }
});

test("HEAD returns GET metadata without a body", async () => {
  const get = await request("/style.css");
  const head = await request("/style.css", "HEAD");
  assert.equal(head.status, 200);
  assert.equal(head.headers["content-length"], get.headers["content-length"]);
  assert.equal(head.headers["content-type"], get.headers["content-type"]);
  assert.equal(head.body.byteLength, 0);
  assertSecurityHeaders(head);
});

test("uses fixed safe errors for missing, unsupported, and directory paths", async () => {
  for (const requestPath of ["/missing.html", "/unknown.bin", "/directory", "/directory/", "/broken.html"]) {
    const response = await request(requestPath);
    assert.equal(response.status, 404, requestPath);
    assert.equal(response.body.toString("utf8"), "Not found\n", requestPath);
    assert.doesNotMatch(response.body.toString("utf8"), /index\.html|outside|absent/, requestPath);
    assertSecurityHeaders(response);
  }
});

test("rejects methods without enabling CORS", async () => {
  for (const method of ["POST", "OPTIONS"]) {
    const response = await request("/index.html", method);
    assert.equal(response.status, 405);
    assert.equal(response.body.toString("utf8"), "Method not allowed\n");
    assertSecurityHeaders(response);
  }
});

test("rejects raw, encoded, double-encoded, and separator traversal", async () => {
  const attempts = [
    "/../outside.txt",
    "/%2e%2e/outside.txt",
    "/%2E%2E/outside.txt",
    "/%252e%252e/outside.txt",
    "/%2e%2e%2foutside.txt",
    "/..%5coutside.txt",
    "/%2e%2e%5coutside.txt",
    "/./index.html",
  ];
  for (const requestPath of attempts) {
    const response = await request(requestPath);
    assert.equal(response.status, 404, requestPath);
    assert.equal(response.body.toString("utf8"), "Not found\n", requestPath);
    assert.doesNotMatch(response.body.toString("utf8"), /outside secret/, requestPath);
    assertSecurityHeaders(response);
  }
});

test("rejects malformed targets with a fixed bad-request response", async () => {
  for (const requestPath of ["/%E0%A4%A", "/nul%00byte.html", "//index.html", "http://attacker.invalid/index.html"]) {
    const response = await request(requestPath);
    assert.equal(response.status, 400, requestPath);
    assert.equal(response.body.toString("utf8"), "Bad request\n", requestPath);
    assertSecurityHeaders(response);
  }
});

test("realpath containment rejects file and directory symlink escapes", async () => {
  for (const requestPath of ["/outside-link.html", "/outside-directory/outside.txt"]) {
    const response = await request(requestPath);
    assert.equal(response.status, 404, requestPath);
    assert.equal(response.body.toString("utf8"), "Not found\n", requestPath);
    assert.doesNotMatch(response.body.toString("utf8"), /outside secret/, requestPath);
  }
});

test("rejects unusable roots and a listen address already in use", async () => {
  await assert.rejects(
    startServer({ root: path.join(temporaryDirectory, "absent"), host: "127.0.0.1", port: 0 }),
    /static root is unavailable/,
  );
  await assert.rejects(
    startServer({ root: path.join(staticRoot, "index.html"), host: "127.0.0.1", port: 0 }),
    /static root is not a directory/,
  );
  const address = server.address();
  assert(address && typeof address === "object");
  await assert.rejects(
    startServer({ root: staticRoot, host: "127.0.0.1", port: address.port }),
    { code: "EADDRINUSE" },
  );
});

test("module import is silent and has no server side effect", async () => {
  const source = `await import(${JSON.stringify(pathToFileURL(serverPath).href)});`;
  const result = await capture(["--input-type=module", "--eval", source]);
  assert.equal(result.code, 0);
  assert.equal(result.signal, null);
  assert.equal(result.stdout, "");
  assert.equal(result.stderr, "");
});

test("direct execution prints exactly one capturable loopback URL", async () => {
  const running = await runServerProcess([staticRoot], temporaryDirectory);
  try {
    const output = running.readOutput();
    assert.equal(output.stdout, `${running.printedOrigin}\n`);
    assert.equal(output.stderr, "");
  } finally {
    running.child.kill("SIGTERM");
    await running.exit;
  }
});

test("direct execution defaults to target/web-dist", async () => {
  const defaultDirectory = await fs.mkdtemp(path.join(os.tmpdir(), "riot-web-default-"));
  await fs.mkdir(path.join(defaultDirectory, "target", "web-dist"), { recursive: true });
  await fs.writeFile(path.join(defaultDirectory, "target", "web-dist", "index.html"), "default root\n");
  const running = await runServerProcess([], defaultDirectory);
  try {
    const page = await request("/", "GET", running.printedOrigin);
    assert.equal(page.body.toString("utf8"), "default root\n");
    const output = running.readOutput();
    assert.equal(output.stdout, `${running.printedOrigin}\n`);
    assert.equal(output.stderr, "");
  } finally {
    running.child.kill("SIGTERM");
    await running.exit;
    await fs.rm(defaultDirectory, { recursive: true, force: true });
  }
});
