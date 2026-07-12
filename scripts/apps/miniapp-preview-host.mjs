#!/usr/bin/env node

import { realpath, readFile, stat } from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const appsRoot = path.join(repoRoot, "fixtures/apps");
const host = "127.0.0.1";
const port = Number.parseInt(process.env.PORT || "43117", 10);

const mimeTypes = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".png", "image/png"],
  [".svg", "image/svg+xml; charset=utf-8"],
]);

function isWithin(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== "..");
}

function seedRows(app, state) {
  if (app !== "checklist" || state === "empty") return [];
  const rows = [
    ["items/bring-water", { text: "Bring water", done: false, updated_by_id: "profile-alex", updated_at: 1 }],
    ["items/check-radio", { text: "Check the radio", done: true, updated_by_id: "profile-sam", updated_at: 2 }],
  ];
  if (state === "post-action") {
    rows.push(["items/post-action", { text: "Share the route", done: false, updated_by_id: "profile-you", updated_at: 3 }]);
  }
  return rows;
}

function mockBridgeSource(app, state) {
  const initialRows = JSON.stringify(seedRows(app, state));
  const failWrites = state === "error";
  return `
<script data-miniapp-preview-bridge>
(() => {
  "use strict";
  const store = new Map(${initialRows});
  const watchers = [];
  const profiles = new Map([
    ["profile-you", { displayName: "You", tag: "you" }],
    ["profile-alex", { displayName: "Alex Rivera", tag: "alex" }],
    ["profile-sam", { displayName: "Sam Chen", tag: "sam" }],
  ]);
  const normalize = (value) => String(value).replace(/\\/+$/, "");
  const validKey = (key) => key && key.split("/").every((part) => part && part !== "." && part !== "..");
  const clone = (value) => value == null ? null : structuredClone(value);
  const list = async (prefix) => {
    const clean = normalize(prefix);
    return [...store.entries()]
      .filter(([key]) => key === clean || key.startsWith(clean + "/"))
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, value]) => ({ key, value: clone(value) }));
  };
  const notify = () => queueMicrotask(() => {
    watchers.forEach(({ prefix, callback }) => list(prefix).then(callback));
  });
  window.riot = Object.freeze({
    async get(key) {
      const clean = normalize(key);
      if (!validKey(clean)) throw new Error("invalid key");
      return clone(store.get(clean));
    },
    async put(key, value) {
      const clean = normalize(key);
      if (!validKey(clean)) throw new Error("invalid key");
      if (${failWrites}) throw new Error("deterministic preview write failure");
      store.set(clean, clone(value));
      notify();
    },
    list,
    watch(prefix, callback) {
      if (typeof callback !== "function") throw new TypeError("watch callback must be a function");
      const watcher = { prefix: normalize(prefix), callback };
      watchers.push(watcher);
      list(watcher.prefix).then(callback);
      return () => {
        const index = watchers.indexOf(watcher);
        if (index >= 0) watchers.splice(index, 1);
      };
    },
    async whoami() { return { id: "profile-you", ...profiles.get("profile-you") }; },
    async profile(id) { return clone(profiles.get(String(id)) || { displayName: "member", tag: "member" }); },
  });
})();
</script>`;
}

function injectBridge(html, bridge) {
  const foundation = `<link rel="stylesheet" href="/apps/_shared/tokens.css">${bridge}`;
  const head = /<head(?:\s[^>]*)?>/i;
  if (head.test(html)) return html.replace(head, (match) => `${match}${foundation}`);
  return `${foundation}${html}`;
}

async function resolveResource(app, resource) {
  if (!/^(?:_shared|[a-z0-9][a-z0-9-]*)$/.test(app)) return null;
  const appDirectory = path.join(appsRoot, app);
  let realAppDirectory;
  try {
    realAppDirectory = await realpath(appDirectory);
  } catch {
    return null;
  }
  if (!isWithin(await realpath(appsRoot), realAppDirectory)) return null;

  const candidate = path.resolve(realAppDirectory, resource);
  if (!isWithin(realAppDirectory, candidate)) return null;
  try {
    const realCandidate = await realpath(candidate);
    if (!isWithin(realAppDirectory, realCandidate) || !(await stat(realCandidate)).isFile()) return null;
    return realCandidate;
  } catch {
    return null;
  }
}

export function createPreviewServer() {
  return http.createServer(async (request, response) => {
    try {
      const url = new URL(request.url || "/", `http://${host}`);
      let pathname;
      try {
        pathname = decodeURIComponent(url.pathname);
      } catch {
        response.writeHead(400).end("Bad request");
        return;
      }
      const match = pathname.match(/^\/apps\/([^/]+)\/(.*)$/);
      if (!match || request.method !== "GET") {
        response.writeHead(request.method === "GET" ? 404 : 405).end("Not found");
        return;
      }

      const app = match[1];
      const resource = match[2] || "index.html";
      const file = await resolveResource(app, resource);
      const extension = file ? path.extname(file).toLowerCase() : "";
      const mime = mimeTypes.get(extension);
      if (!file || !mime) {
        response.writeHead(404).end("Not found");
        return;
      }

      let body = await readFile(file);
      if (extension === ".html") {
        const requestedState = url.searchParams.get("state") || "seeded";
        const state = ["seeded", "empty", "error", "post-action"].includes(requestedState)
          ? requestedState
          : "seeded";
        body = Buffer.from(injectBridge(body.toString("utf8"), mockBridgeSource(app, state)));
      }
      response.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Length": body.byteLength,
        "Content-Security-Policy": "default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; connect-src 'none'; object-src 'none'; base-uri 'none'",
        "Content-Type": mime,
        "X-Content-Type-Options": "nosniff",
      });
      response.end(body);
    } catch (error) {
      response.writeHead(500).end("Preview host error");
      console.error(error);
    }
  });
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  createPreviewServer().listen(port, host, () => {
    console.log(`miniapp preview host: http://${host}:${port}`);
  });
}
