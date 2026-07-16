# riot-newswire-dev — a Cloudflare mirror for dev

A hosted **mirror** of a Riot site's reach-layer view, for development. Gives you
a stable public URL to point apps and browsers at while iterating.

**It is not a publisher.** Publishing stays on the p2p network, hidden. This
worker is a dumb host serving the self-contained static dump — the same bytes
could sit on any other mirror, IPFS, or `file://`. It cannot forge content: each
page carries its own CSP in a `<meta>` tag and links `riot://…` for the verified
copy. See `docs/superpowers/specs/2026-07-16-web-viewer-reach-layer-design.md`.

Content is **demo sample data** (`newswire.sample_view()`), flagged "not signed"
in the footer. Real signed E/W content arrives with composite Unit 1/2.

## What it serves

| Path | What |
|---|---|
| `/` | two-column newswire (editorial features + open-wire) |
| `/board/newsprint/site/incident-board/` | incident board, newsprint skin (live vendored filter) |
| `/board/zine/site/incident-board/` | incident board, zine skin |

## Layout

- `build.py` — renders the Python gateway/newswire output into `./dist/` (the
  single source of truth is the Python renderers, not this worker)
- `src/worker.js` — serves `dist/`, adds transport security headers, read-only
- `wrangler.toml` — `run_worker_first` so the worker wraps every asset response

## Run locally

```bash
npm install          # first time (fetches wrangler)
npm run dev          # builds dist/ then wrangler dev → http://localhost:8787
```

## Deploy (needs your Cloudflare login)

```bash
npx wrangler login   # once, in your terminal:  ! npx wrangler login
npm run deploy       # build.py + wrangler deploy → https://riot-newswire-dev.<subdomain>.workers.dev
```

Re-run `npm run build` (or `npm run deploy`) after any change to the Python
renderers to refresh the frozen dump.
