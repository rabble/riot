# Riot

Riot is an exploratory native app for offline civic information during internet shutdowns, protests, disasters, and other moments where chat is not enough.

The core idea is to preload the app before a crisis, then let people create, sign, render, exchange, and merge local-first information packets while offline. Packets can behave like small local apps or websites: alerts, resource maps, legal guides, medical checklists, mutual aid boards, and evolving incident pages.

Riot is early planning work. The first technical direction is:

- Willow as the canonical mergeable data model.
- SneakerWeb-style static site rendering for offline browsing.
- Willow Drop Format as the first portable exchange artifact.
- Willow Transfer Protocol later for live local sync.
- Local LLM assistance for drafting, translation, summarization, and formatting from user-provided facts.
- Human signing and provenance for every published update.

## Repository Layout

- `docs/research/` - source-grounded notes from the initial protocol and product research.
- `docs/product/` - product framing, packet concepts, and safety model.
- `docs/architecture/` - proposed app architecture and protocol usage.
- `docs/superpowers/plans/` - implementation plans for future build work.

## Current Status

Planning only. No app code exists yet.
