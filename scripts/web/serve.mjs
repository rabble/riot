#!/usr/bin/env node

import { promises as fs } from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const CONTENT_SECURITY_POLICY = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; worker-src 'self'; manifest-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";
const PERMISSIONS_POLICY = "accelerometer=(), ambient-light-sensor=(), autoplay=(), bluetooth=(), camera=(), display-capture=(), encrypted-media=(), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), publickey-credentials-create=(), publickey-credentials-get=(), serial=(), usb=(), xr-spatial-tracking=()";
const SECURITY_HEADERS = Object.freeze({
  "Content-Security-Policy": CONTENT_SECURITY_POLICY,
  "Permissions-Policy": PERMISSIONS_POLICY,
  "Referrer-Policy": "no-referrer",
  "X-Content-Type-Options": "nosniff",
});
const MIME_TYPES = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".ico", "image/x-icon"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".png", "image/png"],
  [".svg", "image/svg+xml; charset=utf-8"],
  [".wasm", "application/wasm"],
  [".webmanifest", "application/manifest+json; charset=utf-8"],
]);

function send(response, status, body, contentType, headOnly) {
  const bytes = Buffer.isBuffer(body) ? body : Buffer.from(body);
  response.writeHead(status, {
    ...SECURITY_HEADERS,
    "Content-Length": String(bytes.byteLength),
    "Content-Type": contentType,
  });
  response.end(headOnly ? undefined : bytes);
}

function parseRequestPath(target) {
  const queryIndex = target.indexOf("?");
  const rawPath = queryIndex === -1 ? target : target.slice(0, queryIndex);
  if (!rawPath.startsWith("/") || rawPath.startsWith("//")) return { kind: "bad" };
  if (rawPath === "/") return { kind: "valid", segments: ["index.html"] };

  const segments = [];
  for (const encodedSegment of rawPath.slice(1).split("/")) {
    if (encodedSegment.length === 0) return { kind: "unsafe" };
    let segment;
    try {
      segment = decodeURIComponent(encodedSegment);
    } catch {
      return { kind: "bad" };
    }
    if (segment.includes("\0")) return { kind: "bad" };
    if (
      segment === "."
      || segment === ".."
      || segment.includes("/")
      || segment.includes("\\")
      || segment.includes("%")
    ) {
      return { kind: "unsafe" };
    }
    segments.push(segment);
  }
  return { kind: "valid", segments };
}

async function readResource(realRoot, segments) {
  try {
    const candidate = path.join(realRoot, ...segments);
    const realCandidate = await fs.realpath(candidate);
    if (!realCandidate.startsWith(`${realRoot}${path.sep}`)) return null;
    const metadata = await fs.stat(realCandidate);
    if (!metadata.isFile()) return null;
    const contentType = MIME_TYPES.get(path.extname(realCandidate).toLowerCase());
    if (!contentType) return null;
    return { body: await fs.readFile(realCandidate), contentType };
  } catch {
    return null;
  }
}

async function handleRequest(realRoot, request, response) {
  const headOnly = request.method === "HEAD";
  if (request.method !== "GET" && !headOnly) {
    send(response, 405, "Method not allowed\n", "text/plain; charset=utf-8", false);
    return;
  }

  const parsed = parseRequestPath(request.url);
  if (parsed.kind === "bad") {
    send(response, 400, "Bad request\n", "text/plain; charset=utf-8", headOnly);
    return;
  }
  if (parsed.kind === "unsafe") {
    send(response, 404, "Not found\n", "text/plain; charset=utf-8", headOnly);
    return;
  }

  const resource = await readResource(realRoot, parsed.segments);
  if (!resource) {
    send(response, 404, "Not found\n", "text/plain; charset=utf-8", headOnly);
    return;
  }
  send(response, 200, resource.body, resource.contentType, headOnly);
}

export async function startServer({ root, host = "127.0.0.1", port = 0 }) {
  let realRoot;
  let metadata;
  try {
    realRoot = await fs.realpath(root);
    metadata = await fs.stat(realRoot);
  } catch (error) {
    throw new Error("static root is unavailable", { cause: error });
  }
  if (!metadata.isDirectory()) throw new Error("static root is not a directory");

  const server = http.createServer((request, response) => {
    void handleRequest(realRoot, request, response);
  });
  await new Promise((resolve, reject) => {
    const onError = (error) => {
      server.off("listening", onListening);
      reject(error);
    };
    const onListening = () => {
      server.off("error", onError);
      resolve();
    };
    server.once("error", onError);
    server.once("listening", onListening);
    server.listen(port, host);
  });
  return server;
}

const modulePath = fileURLToPath(import.meta.url);
const invokedPath = process.argv.at(1);
const isDirectExecution = invokedPath ? path.resolve(invokedPath) === modulePath : false;
if (isDirectExecution) {
  const root = path.resolve(process.argv.at(2) ?? path.join("target", "web-dist"));
  const server = await startServer({ root });
  const address = server.address();
  console.log(`http://127.0.0.1:${address.port}/`);
  process.once("SIGTERM", () => server.close());
}
