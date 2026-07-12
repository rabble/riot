#!/usr/bin/env node

import { realpath, readFile, stat } from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const appsRoot = path.join(repoRoot, "fixtures/apps");
const host = "127.0.0.1";
const port = Number.parseInt(process.env.PORT || "43117", 10);
const csp = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:";

const profiles = {
  you: { id: "1111111111111111111111111111111111111111111111111111111111111111", displayName: "You", tag: "11111111" },
  alex: { id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", displayName: "Alex Rivera", tag: "aaaaaaaa" },
  sam: { id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", displayName: "Sam Chen", tag: "bbbbbbbb" },
};

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
  if (state === "empty") return [["meta/seeded", { version: 1 }]];
  const existing = {
    checklist: ["tasks/existing-task", { text: "Existing neighborhood task", created_at: 10, added_by_id: profiles.alex.id, assigned_to_id: "", completed: false }],
    "supply-board": ["items/existing-item", { kind: "need", text: "Existing supply request", created_at: 10, added_by_id: profiles.alex.id, resolved_by_id: "" }],
    "roll-call": ["events/existing-event", { title: "Existing block gathering", starts_at: "2030-07-18T18:00:00.000Z", place: "Library steps", created_by_id: profiles.alex.id }],
    "quick-poll": ["proposals/current", { id: "existing-decision", text: "Existing community decision?", options: ["First option", "Second option"], asked_by_id: profiles.alex.id, at: 10 }],
  }[app];
  if (state === "existing-unmarked") return existing ? [existing] : [];
  if (["delayed-identity", "identity-error", "profile-race", "error"].includes(state)) {
    return existing ? [["meta/seeded", { version: 1, status: "ready" }], existing] : [];
  }
  if (state === "malformed") {
    if (app === "quick-poll") return [["meta/seeded", { version: 1, status: "ready" }], ["proposals/current", null], ["votes/bad/bad", null]];
    const invalidKey = { checklist: "tasks/bad", "supply-board": "items/bad", "roll-call": "events/bad", "quick-poll": "votes/existing-decision/bad" }[app];
    return existing ? [["meta/seeded", { version: 1, status: "ready" }], existing, [invalidKey, null]] : [];
  }
  if (state === "slow-write" && app === "checklist") {
    return [["meta/seeded", { version: 1, status: "ready" }], existing];
  }
  if (app !== "checklist") return [];
  const rows = [
    ["items/bring-water", { text: "Bring water", done: false, updated_by_id: profiles.alex.id, updated_at: 1 }],
    ["items/check-radio", { text: "Check the radio", done: true, updated_by_id: profiles.sam.id, updated_at: 2 }],
  ];
  if (state === "post-action") {
    rows.push(["items/post-action", { text: "Share the route", done: false, updated_by_id: profiles.you.id, updated_at: 3 }]);
  }
  return rows;
}

function mockBridgeSource(app, state) {
  const initialRows = JSON.stringify(seedRows(app, state));
  const serializedProfiles = JSON.stringify(Object.values(profiles));
  const failWrites = state === "error";
  const delayedIdentity = state === "delayed-identity";
  const failedIdentity = state === "identity-error";
  const slowWrites = state === "slow-write";
  const profileRace = state === "profile-race";
  return `
(() => {
  "use strict";
  const store = new Map(${initialRows}.map(([key, value]) => [key, JSON.stringify(value)]));
  const watchers = [];
  const profileRows = ${serializedProfiles};
  const profileByID = new Map(profileRows.map(({ id, displayName, tag }) => [id, { displayName, tag }]));
  const identityPattern = /^[0-9a-f]{64}$/;

  function normalizePrefix(prefix) {
    if (typeof prefix !== "string") throw new TypeError("prefix must be a string");
    return prefix.replace(/\\/+$/, "");
  }

  function validateKey(key) {
    if (typeof key !== "string" || !key) throw new Error("invalid key");
    const segments = key.split("/");
    if (segments.length > 62) throw new Error("invalid key");
    let totalBytes = 36;
    for (const segment of segments) {
      if (!/^[a-z0-9-]+$/.test(segment) || segment.length > 256) throw new Error("invalid key");
      totalBytes += segment.length;
    }
    if (totalBytes > 2048) throw new Error("invalid key");
    return key;
  }

  function serializeJSON(value) {
    const serialized = JSON.stringify(value);
    if (typeof serialized !== "string") throw new TypeError("value must be JSON");
    return serialized;
  }

  const list = async (prefix) => {
    const clean = validateKey(normalizePrefix(prefix));
    return [...store.entries()]
      .filter(([key]) => key === clean || key.startsWith(clean + "/"))
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, value]) => ({ key, value: JSON.parse(value) }));
  };
  const notify = () => queueMicrotask(() => {
    watchers.forEach(({ prefix, callback }) => list(prefix).then(callback).catch(function () {}));
  });
  window.riot = Object.freeze({
    async get(key) {
      const clean = validateKey(key);
      const value = store.get(clean);
      return value === undefined ? null : JSON.parse(value);
    },
    async put(key, value) {
      const clean = validateKey(key);
      const serialized = serializeJSON(value);
      if (${failWrites}) throw new Error("deterministic preview write failure");
      if (${slowWrites}) await new Promise((resolve) => setTimeout(resolve, 150));
      store.set(clean, serialized);
      notify();
    },
    list,
    watch(prefix, callback) {
      if (typeof callback !== "function") throw new TypeError("watch callback must be a function");
      watchers.push({ prefix, callback });
      list(prefix).then(callback).catch(function () {});
    },
    async whoami() {
      if (${failedIdentity}) throw new Error("deterministic identity failure");
      if (${delayedIdentity}) await new Promise((resolve) => setTimeout(resolve, 500));
      return { ...profileRows[0] };
    },
    async profile(id) {
      if (typeof id !== "string" || !identityPattern.test(id)) throw new Error("invalid profile id");
      if (${profileRace}) await new Promise((resolve) => setTimeout(resolve, id === "${profiles.alex.id}" ? 400 : 20));
      return { ...(profileByID.get(id) || { displayName: "member", tag: id.slice(0, 8) }) };
    },
  });
})();
`;
}

function injectBridge(html, app, state) {
  const foundation = `<link rel="stylesheet" href="/apps/_shared/tokens.css"><script data-miniapp-preview-bridge src="/apps/${app}/__miniapp-preview-bridge.js?state=${state}"></script>`;
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
      const requestedState = url.searchParams.get("state") || "seeded";
      const state = ["seeded", "empty", "error", "post-action", "delayed-identity", "identity-error", "existing-unmarked", "malformed", "profile-race", "slow-write"].includes(requestedState)
        ? requestedState
        : "seeded";

      if (resource === "__miniapp-preview-bridge.js") {
        if (!(await resolveResource(app, "index.html"))) {
          response.writeHead(404).end("Not found");
          return;
        }
        const body = Buffer.from(mockBridgeSource(app, state));
        response.writeHead(200, {
          "Cache-Control": "no-store",
          "Content-Length": body.byteLength,
          "Content-Security-Policy": csp,
          "Content-Type": mimeTypes.get(".js"),
          "X-Content-Type-Options": "nosniff",
        });
        response.end(body);
        return;
      }

      const file = await resolveResource(app, resource);
      const extension = file ? path.extname(file).toLowerCase() : "";
      const mime = mimeTypes.get(extension);
      if (!file || !mime) {
        response.writeHead(404).end("Not found");
        return;
      }

      let body = await readFile(file);
      if (extension === ".html") {
        body = Buffer.from(injectBridge(body.toString("utf8"), app, state));
      }
      response.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Length": body.byteLength,
        "Content-Security-Policy": csp,
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
