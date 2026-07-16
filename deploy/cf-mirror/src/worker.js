// Riot reach-layer mirror (dev). A dumb host: serves the pre-rendered static
// dump and adds security headers. It cannot forge content — the pages are
// self-contained and carry their own CSP in a <meta> tag, so the mirror's
// authority is zero. This wrapper only tightens transport-side headers.
//
// Deliberately no CSP header here: each page bakes its own per-skin CSP (with a
// style-src hash matching that exact stylesheet) into <head>. A single header
// CSP could not match every page's hash and would wrongly block the page's own
// <style>. The baked meta is authoritative; we add the non-CSP headers only.

const EXTRA_HEADERS = {
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
  "X-Frame-Options": "DENY",
};

export default {
  async fetch(request, env) {
    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("this mirror is read-only", { status: 405 });
    }
    const res = await env.ASSETS.fetch(request);
    const headers = new Headers(res.headers);
    for (const [name, value] of Object.entries(EXTRA_HEADERS)) {
      headers.set(name, value);
    }
    return new Response(res.body, { status: res.status, statusText: res.statusText, headers });
  },
};
