# Riot marketing site

A dependency-free static site deployed as Cloudflare Workers assets. It has no
backend, analytics, remote fonts, or third-party runtime requests.

## Routes

- `/` — Riot's human-facing product story.
- `/protocols/` — a source-backed field guide comparing Riot and Willow with
  adjacent social, relay, federation, nearby-messaging, and local-first systems.

The protocol page is secondary by design, but intentionally discoverable. The
homepage links to it from the navigation, a prominent comparison panel beneath
“How it works,” “For the technically curious,” and the footer.

The homepage's Lineage section credits @rabble with building Riot using the
Willow libraries, and links the builder history and reported government actions
against Indymedia to their sources. The copy intentionally does not claim that
@rabble implemented Willow or authored the Willow protocol specification.

## Source and deployment mirrors

Edit the source files first:

- `index.html`
- `protocols/index.html`
- `assets/screenshots/` for the real iPhone simulator screens used in the hero

Then update their byte-identical deployment mirrors:

- `public/index.html`
- `public/protocols/index.html`
- `public/assets/screenshots/`

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
`http://localhost:4173/protocols/`.

## Deploy

The Workers assets directory is configured in `wrangler.toml`. From this
directory, deploy with:

```sh
npx wrangler deploy
```

After deployment, fetch both live routes and check their expected headings;
Wrangler success alone does not prove the nested route is being served.

## Scope

This site is separate from `apps/gateway/`, which serves actual Willow `/site/`
content. The marketing site describes Riot and links primary protocol sources;
it does not render, mutate, or fetch a community's Willow data.
