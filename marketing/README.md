# Riot marketing site

A dependency-free static site deployed as Cloudflare Workers assets. It has no
backend, analytics, remote fonts, or third-party runtime requests.

## Routes

- `/` — Riot's human-facing product story.
- `/about/` — why Riot exists: keep publishing through the blackout, in the
  lineage of Indymedia, protest.net, and TXTMob.
- `/privacy/` — what is and is not private today, separated into "this site"
  (no analytics/cookies/accounts) and "the Riot app" (local-first; honest
  boundaries on what is not shipped).
- `/open-source/` — MIT license for the repo (one AGPL-3.0-or-later crate),
  how the repository is organized, and how to build and verify it.
- `/community/` — how to get involved: issues, following the build, contributing.
  Honest about the fact that there is no chat room or mailing list yet.
- `/protocols/` — a source-backed field guide comparing Riot and Willow with
  adjacent social, relay, federation, nearby-messaging, and local-first systems.

Every page's footer links to every other page, so a visitor on any route can
reach all of them. The homepage and protocol page keep their existing top-nav
unchanged; the cross-linking lives in the footer.

The protocol page is secondary by design, but intentionally discoverable. The
homepage links to it from the navigation, a prominent comparison panel beneath
“How it works,” “For the technically curious,” and the footer.

The homepage's Lineage section credits @rabble with building Riot using the
Willow libraries, and links the builder history and reported government actions
against Indymedia to their sources. The copy intentionally does not claim that
@rabble implemented Willow or authored the Willow protocol specification.

## Crawl metadata

`public/sitemap.xml` and `public/robots.txt` live in the deployment mirror
(they are deployment artifacts, not editorial pages, so they have no source
copy). The sitemap lists every route above at the
`https://riot-protest-net-marketing.protestnet.workers.dev` origin; robots
allows all user-agents and points to the sitemap. No route is disallowed — the
entire site is public.

## Source and deployment mirrors

Edit the source files first:

- `index.html`
- `about/index.html`
- `privacy/index.html`
- `open-source/index.html`
- `community/index.html`
- `protocols/index.html`
- `assets/screenshots/` for the real iPhone simulator screens used in the hero

Then update their byte-identical deployment mirrors:

- `public/index.html`
- `public/about/index.html`
- `public/privacy/index.html`
- `public/open-source/index.html`
- `public/community/index.html`
- `public/protocols/index.html`
- `public/assets/screenshots/`

`public/sitemap.xml` and `public/robots.txt` have no source copy — edit them
in place in `public/`.

The hero screenshots demonstrate app UI captured from the iPhone simulator.
They are not evidence of sync over physical-device radios.

The contract check rejects mirror drift and missing editorial or accessibility
requirements:

```sh
node scripts/marketing/protocol-page-contracts.mjs
```

Run it from the repository root.

## Local preview

From the repository root:

```sh
python3 -m http.server 4173 --directory marketing/public
```

Then open `http://localhost:4173/` and
`http://localhost:4173/protocols/`. The other routes (`/about/`, `/privacy/`,
`/open-source/`, `/community/`, `/sitemap.xml`, `/robots.txt`) are served the
same way from `public/`.

## Deploy

The Workers assets directory is configured in `wrangler.toml`. From this
directory, deploy with:

```sh
npx wrangler deploy
```

After deployment, fetch both live routes and check their expected headings;
Wrangler success alone does not prove the nested route is being served.

## License

This site and the rest of the repository are MIT-licensed (see `/LICENSE` at
the repo root). The `crates/riot-anchor-protocol` crate is an exception,
licensed under AGPL-3.0-or-later as declared in its `Cargo.toml`.

## Scope

This site is separate from `apps/gateway/`, which serves actual Willow `/site/`
content. The marketing site describes Riot and links primary protocol sources;
it does not render, mutate, or fetch a community's Willow data.
