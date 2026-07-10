# Riot Product Brief

## Thesis

When internet access is degraded, shut down, surveilled, or unreliable, people need more than chat. They need durable local information that can be created in the field, verified by humans, updated, translated, carried, and re-shared.

Riot is a native offline packet runtime for that job.

## Two Modes

Riot has two sides, built as separate subsystems joined only by an explicit bridge (see `docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`):

- **Open newswire:** per-incident open spaces anyone can publish to, and publication spaces where a pseudonymous collective is the publisher. Curation is a reading lens applied by editors, not a gate on publishing. Subscribers' devices are the distribution network, so a banned publication has no server to seize.
- **Private groups:** encrypted, unlinkable spaces for affinity groups, coops, and crews. Joined in person via QR or by portable encrypted invite files.

A public web gateway mirrors newswire content at normal URLs for discovery and onboarding; private groups never touch it.

## What Riot Is

Riot lets people preload and exchange packets. A packet is a small offline app/site with:

- a local UI,
- a structured data schema,
- trusted reference content,
- signing and import policy,
- local LLM authoring instructions,
- and Willow-backed mergeable state.

Packets can be read as local websites, edited through native workflows, exported as portable files, and synchronized locally when possible.

## What Riot Is Not

Riot is not:

- a general social network,
- a chat app,
- an app store replacement,
- an AI authority,
- a crisis prediction system,
- a tool that requires accounts or servers,
- or a browser that casually runs arbitrary web code.

## Why Not Just Preload Everything

Preloading only solves known information. In protests and disasters, the most important information changes in the field:

- routes close,
- police lines move,
- shelters fill,
- clinics relocate,
- water and food supplies shift,
- legal support locations change,
- rumors need correction,
- and multilingual updates need to be produced quickly.

Riot must therefore be able to create new signed updates from inside the already-installed app.

## Packet Examples

### Protest Packet

- know-your-rights pages,
- jail support contacts,
- legal observer check-ins,
- medic locations,
- supply requests,
- route changes,
- police activity updates,
- rumor corrections,
- multilingual public notices.

### Disaster Packet

- shelter locations,
- water and food distribution,
- road closures,
- power and charging spots,
- first-aid checklists,
- reunification boards,
- urgent weather or evacuation alerts.

### Mutual Aid Packet

- needs and offers board,
- volunteer directory,
- delivery routes,
- inventory counts,
- shift schedules,
- local announcements.

### Clinic Packet

- intake forms,
- triage instructions,
- available supplies,
- referral list,
- urgent safety alerts.

## Native App Shape

Riot should have four primary areas:

- **Library:** local packets and sites, with previews, signer status, freshness, and import provenance.
- **Create:** structured field editors for alerts, resources, checklists, reports, translations, and corrections.
- **Exchange:** import/export drops, local peer sync, AirDrop/Files/USB/LAN, and future transport adapters.
- **Read:** a sandboxed local renderer for packet sites and app views.

## Local LLM Role

The LLM is a field editor. It helps with:

- turning rough notes into clear updates,
- translating approved text,
- summarizing imported changes,
- extracting structured objects from user notes,
- generating static pages from packet schemas,
- and formatting content in the packet's house style.

The LLM must not decide what is true. It must not publish directly. Users sign final updates.

## Safety Principles

- No automatic import of arbitrary packet data.
- Preview before ingest.
- Explicit signer and provenance display.
- Trust as a lens, not a gate: open publishing stays frictionless; verification and curation happen at read time. Strict rules (required source, expiry) apply to operational object types, not to publishing in general.
- Required expiry for operational updates.
- Panic wipe.
- No network beacons from rendered packet content.
- No arbitrary native bridge from web content.
- Local-only operation by default.
- Clear separation between verified, trusted, unverified, and blocked content.
