# Riot marketing site

A single self-contained static file (`index.html`) — no build step, no
backend, no external requests (fonts are inlined as base64 `@font-face` data
URIs). Open it directly in a browser or serve the directory with any static
file server.

This is separate from `apps/gateway/`, which serves the actual Willow
`/site/` newswire content per `docs/decisions/riot-protest-net-runbook.md`.
This is copy-only: what Riot is, who it's for, and an honest "not released
yet" status. It does not render or fetch any Willow content.

## What's missing before this can go live at `riot.protest.net`

- **A real contact/updates mechanism.** The closing section currently only
  links out to protest.net — there's no email or signup form because none
  was available to wire up honestly. Add one before publishing, or leave it
  as-is if a bare link is the intended CTA.
- **Deployment itself.** Per `docs/decisions/riot-protest-net-runbook.md`,
  actually serving this at `riot.protest.net` needs a DNS/TLS owner, an
  approved hosting path, and an egress/edge policy review — none of which
  this session has the authority or credentials to set up. This file is
  ready to hand to whoever owns that.

## Editing

Content and styles are in `index.html`. Fonts (Anton, Work Sans, Space Mono)
are embedded inline; to change them, re-generate the base64 `@font-face`
blocks rather than linking an external font CDN (the whole point of inlining
was zero external requests).
