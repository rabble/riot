# Riot

Riot is an exploratory native app for offline civic information during internet shutdowns, protests, disasters, and other moments where chat is not enough.

The core idea is to preload the app before a crisis, then let people create, sign, render, exchange, and merge local-first information packets while offline. Packets can behave like small local apps or websites: alerts, resource maps, legal guides, medical checklists, mutual aid boards, and evolving incident pages.

Riot has two sides, built as parallel subsystems joined only by an explicit bridge:

- An **open newswire** for emergency publishing and durable movement media: per-incident open spaces anyone can post to, and publication spaces where a pseudonymous collective is the publisher and subscribers' devices are the distribution network (indymedia with no server to raid).
- **Private groups**: encrypted, unlinkable Willow namespaces for affinity groups, coops, and crews, joined in person via QR or by portable encrypted invite files.

The design spec is `docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`. The technical direction:

- Willow as the canonical mergeable data model.
- SneakerWeb-style static site rendering for offline browsing.
- Willow Drop Format as the first portable exchange artifact.
- Willow Transfer Protocol later for live local sync.
- Encrypted Willow techniques on the critical path for private groups.
- A public web gateway (stateless mirror) for discovery and onboarding of newswire content.
- Local LLM assistance for drafting, translation, summarization, and formatting from user-provided facts.
- Human signing and provenance for every published update.

## Repository Layout

- `docs/research/` - source-grounded notes from the initial protocol and product research.
- `docs/product/` - product framing, packet concepts, and safety model.
- `docs/architecture/` - proposed app architecture and protocol usage.
- `docs/superpowers/specs/` - approved design specs.
- `docs/superpowers/plans/` - implementation plans for future build work.

## Current Status

Planning only. No app code exists yet.
