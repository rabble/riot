# Riot marketing site

A dependency-free static site deployed as Cloudflare Workers assets. It has no
backend, analytics, remote fonts, or third-party runtime requests.

## Routes

- `/` — concise product overview and current app evidence.
- `/why-riot/` — Riot's human-capacity and social-purpose story.
- `/guide/` — task-by-task instructions for the current prototype.
- `/about/` — project lineage, history, and builder.
- `/privacy/` — concise public-publishing, participant-held-data, private-conversation, and website-data boundaries.
- `/open-source/` — licenses, repository layout, and build instructions.
- `/community/` — ways to follow, test, and contribute to the work.
- `/releases/` — current build availability and platform notes.
- `/protocols/` — source-backed protocol comparisons and detailed trust boundaries.

Primary navigation contains every route except Privacy. Privacy remains linked
from relevant boundary copy and every footer. Every footer links all nine
routes, including the page currently being viewed.

## Crawl metadata

`public/sitemap.xml` and `public/robots.txt` live only in the deployment mirror.
The sitemap lists the exact nine editorial routes at the configured Workers
origin. Robots allows all user-agents and points to the sitemap.

## Source and deployment mirrors

Edit these nine editorial source files first:

- `index.html`
- `why-riot/index.html`
- `guide/index.html`
- `about/index.html`
- `privacy/index.html`
- `open-source/index.html`
- `community/index.html`
- `releases/index.html`
- `protocols/index.html`

Then update their byte-identical deployment mirrors:

- `public/index.html`
- `public/why-riot/index.html`
- `public/guide/index.html`
- `public/about/index.html`
- `public/privacy/index.html`
- `public/open-source/index.html`
- `public/community/index.html`
- `public/releases/index.html`
- `public/protocols/index.html`

Real desktop app screens used by the homepage live in `assets/screenshots/` and
their byte-identical deployment copies live in `public/assets/screenshots/`.
The images show current UI; they do not prove physical-radio or field behavior.

`public/sitemap.xml` and `public/robots.txt` have no source copy. Edit those
deployment artifacts in place.

The contract check rejects mirror drift, route/navigation drift, unsafe claims,
remote runtime dependencies, cookie-setting local artifacts, and missing
editorial or accessibility requirements:

```sh
npm run test:marketing
```

Run it from the repository root.

## Local preview

From the repository root:

```sh
python3 -m http.server 4173 --directory marketing/public
```

Then open the nine routes:

- `http://localhost:4173/`
- `http://localhost:4173/why-riot/`
- `http://localhost:4173/guide/`
- `http://localhost:4173/about/`
- `http://localhost:4173/privacy/`
- `http://localhost:4173/open-source/`
- `http://localhost:4173/community/`
- `http://localhost:4173/releases/`
- `http://localhost:4173/protocols/`

The crawl artifacts are at `/sitemap.xml` and `/robots.txt` on the same local
origin.

## Deploy

The Workers assets directory is configured in `wrangler.toml`. From this
directory, deployment uses:

```sh
npx wrangler deploy
```

Deployment is a separate operation. After an approved deployment, fetch every
live route and verify its expected heading and response behavior; Wrangler
success alone does not prove nested routes are being served.

## License

This site and most of the repository are MIT-licensed (see `/LICENSE` at the
repo root). The `crates/riot-anchor` and `crates/riot-anchor-protocol` crates
are exceptions, licensed under AGPL-3.0-or-later as declared in their
`Cargo.toml` files.

## Scope

This site is separate from `apps/gateway/`, which serves actual Willow `/site/`
content. The marketing site describes Riot and links primary protocol sources;
it does not render, mutate, or fetch a community's Willow data.
