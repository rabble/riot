// Riot reach-layer mirror (dev) — DYNAMIC tier.
//
// The live projection is written into KV by the seed (seed.sh: project → push).
// The worker reads KV per request, so the site updates when the store updates —
// no redeploy. This trades the pure-static mirror's max-mirrorability for
// freshness; it is the "origin/dev instance" shape, not the censorship-hard
// copy-anywhere dump (that stays the static-dump path). Static ASSETS are the
// fallback for anything not in KV.
//
// Still a dumb host: pages are self-contained and carry their own CSP in a
// <meta>; the worker only adds transport-side headers and cannot forge content.

const EXTRA_HEADERS = {
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
  "X-Frame-Options": "DENY",
};

function kvKey(pathname) {
  let key = pathname.replace(/^\/+/, "");
  if (key === "" || key.endsWith("/")) key += "index.html";
  return key;
}

function contentType(key) {
  if (key.endsWith(".html")) return "text/html; charset=utf-8";
  if (key.endsWith(".json")) return "application/json; charset=utf-8";
  if (key.endsWith(".svg")) return "image/svg+xml";
  return "text/plain; charset=utf-8";
}

export default {
  async fetch(request, env) {
    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("this mirror is read-only", { status: 405 });
    }

    const key = kvKey(new URL(request.url).pathname);

    // DYNAMIC: serve the live projection from KV if present.
    const live = await env.NEWSWIRE.get(key);
    if (live !== null) {
      const headers = new Headers({ "Content-Type": contentType(key), ...EXTRA_HEADERS });
      headers.set("X-Riot-Source", "kv-live");
      return new Response(live, { status: 200, headers });
    }

    // Fallback: static assets (or 404), with the same headers.
    const res = await env.ASSETS.fetch(request);
    const headers = new Headers(res.headers);
    for (const [name, value] of Object.entries(EXTRA_HEADERS)) {
      headers.set(name, value);
    }
    headers.set("X-Riot-Source", "static");
    return new Response(res.body, { status: res.status, statusText: res.statusText, headers });
  },
};
