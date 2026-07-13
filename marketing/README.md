# Riot marketing site

A dependency-free static site deployed as Cloudflare Workers assets. It has no
backend, analytics, remote fonts, or runtime asset requests.

## Routes

- `/` — Riot's human-facing product story.
- `/protocols/` — a source-backed field guide comparing Riot and Willow with
  adjacent social, relay, federation, nearby-messaging, and local-first systems.

The protocol page is secondary by design, but intentionally discoverable. The
homepage links to it from the navigation, a prominent comparison panel beneath
“How it works,” “For the technically curious,” and the footer.

## Source and deployment mirrors

Edit the source files first:

- `index.html`
- `protocols/index.html`

Then update their byte-identical deployment mirrors:

- `public/index.html`
- `public/protocols/index.html`

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
