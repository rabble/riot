# How Mutual Aid and Grassroots Networks Actually Coordinate

Date: 2026-07-10
Method: Deep-research workflow — 5 search angles, 23 sources fetched, 115 claims extracted, top 25 adversarially verified (3 independent verification votes per claim). 24 confirmed, 1 refuted. Findings below are only the survivors.

## Summary

Across disaster response, protest coordination, and movement media, grassroots networks converge on one core pipeline: **intake → verification by trusted roles → dispatch to fulfillment**, with a persistent split between field roles (canvassers, bike messengers, lookouts, medics) and remote coordinators who fuse intelligence and broadcast only verified information. The recurring central data object is a **shared editable ledger** (Occupy Sandy's master spreadsheet, COVID mutual aid's Airtable) surrounded by **paper artifacts** — canvassing forms, index cards, flyers — that serve as both intake instruments and degraded-mode transport. Channel architecture repeatedly resolves into a **2×2 matrix of public/private membership × moderated/unmoderated posting** (formalized by TXTMob in 2004). Networks bootstrap from pre-existing activist channels rather than forming cold. They fail through carrier/platform chokepoints, state repression, tacit knowledge trapped in organizers' heads, and digital exclusion of the most vulnerable.

## Verified Findings

### Disaster response (Occupy Sandy, 2012)

- **Needs intake was form-based and standardized.** Paper canvassing forms — Rockaways Household Canvassing Form (11/16/12), Immediate Needs Canvassing Form, intake form, canvasser tip sheet — collected household-level needs door-to-door, entered into a central database. [3-0] (Superstorm Research Lab document archive)
- **Dispatch was the formalized heart of the operation**, mediated by a single master spreadsheet shared across functional teams (COMMS, KITCHEN) and physically separate hubs. COMMS opened a spreadsheet row per aid request; DISPATCH claimed tasks by color-coding rows; handoff to STAGING used paper index cards (time/location/requester on the front, packing slip on the back). A dated Dispatch Sheet artifact confirms document-mediated assignment. Later the operation partially migrated to Sahana EDEN and CiviCRM, but the shared-editable-ledger pattern held throughout. [3-0] (Urban Omnibus / Greenfield firsthand account; SRL archive)
- **Federated hub topology held together by meeting artifacts**: recurring inter-hub and all-projects meetings with recorded minutes, plus per-hub daily task/outreach flow documents. [3-0]
- **Key failure mode: tacit knowledge.** Coordination knowledge lived in organizers' heads ("nobody can tell you this is how 520 Clinton works"); the firsthand chronicler argued it must be captured in a format robust against disruption, widely transmissible, and user-editable. [3-0]

### Disaster verification (Verificado 19S, Mexico City 2017)

- **Bootstrapped from a pre-existing channel**: grew out of a WhatsApp group of activist friends created years earlier to organize a protest. (Same pattern as Occupy Sandy growing from Occupy Wall Street.) [3-0]
- **Explicit rumor-verification protocol**: information required two sources or direct eyewitness confirmation before publication. Roles split between field verifiers (including bicycle messengers riding to sites to check rumors of building collapses) and remote coordinators. Needs requests were concrete and quantified with delivery logistics ("I need 300 shovels, I need 20 helmets"). [3-0]
- *Refuted [1-2], do not rely on:* the claim that Verificado's core data object was a shared Google Drive spreadsheet with shelter/building tabs. The WhatsApp needs-dispatch testimony survived; the spreadsheet detail did not.

### Protest coordination (TXTMob, 2004 DNC/RNC; street medics; NYC Comms Collective)

- **TXTMob formalized the channel matrix**: public/private membership × moderated/unmoderated posting, mapping to four real uses — comms/flashmob broadcast (moderated public), dispatch (moderated private), open forums (unmoderated public), affinity-group channels (unmoderated private). 5,459 registered users, 1,757 messages, 322 groups. [3-0] (Hirsch & Henry, CHI 2005)
- **Medics and legal observers ran moderated private dispatch channels**: designated operators positioned away from the action fused police scanners, calls, SMS, and news, pushing actionable messages to field members (e.g., "1 medic needed for bioterror: meet 1430 copley w/ Jim#"). The NYC Comms Collective layered verification: bike lookouts → secure-location operatives → trusted-source-only broadcast to a 901-member channel, cross-shared with Ruckus and Indymedia channels. Used at the 2004 RNC for tactical blockade decisions and dispelling false rumors. [3-0]
- **Open unmoderated channels had the broadest participation and least accurate information**; users developed bottom-up rumor control — citing sources, signing messages, publicly contesting false texts. Trust came from moderated channels bound to verification roles; open channels needed correction affordances. [3-0]
- **The shift from 2-way radio to SMS was driven by repression and scale**: police had disrupted prior comms by interfering with broadcasts and arresting identifiable comms members; city-wide swarm actions exceeded radio coverage; phones were inconspicuous and carrier SMS hard to jam. [3-0]
- **Centralized carriers were the new chokepoint**: TXTMob's email-to-SMS gateway wouldn't scale past a few hundred to a few thousand users before carriers blocked it — T-Mobile apparently did block TXTMob during the 2004 RNC. [2-1, medium confidence]

### Movement media (Indymedia)

- **Indymedia's 2004 RNC workflow**: an on-site hub verified incoming reports of arrests and police brutality (arriving by phone, email, SMS, open-publishing uploads) with real-time coordination over dedicated IRC channels (#RNCarrests). Video was triaged three ways: immediate publication, legal-defense evidence, documentary use. [3-0] (Costanza-Chock, *Design Justice*)
- **IMC UK: open publishing with post-hoc moderation.** Anyone published directly, unscreened, "so that the flow of information is not held up by bureaucracy"; volunteers moderated after the fact and also curated features. [2-1 — historical practice; IMC UK is effectively archived]
- **Hide, don't delete.** IMC UK's default moderation action was hiding; hidden posts remained publicly inspectable on a "View all posts" page (an approximation of an auditable moderation log). Deletion was reserved for rare cases (pornography, personal details). Editorial disputes lived on a separate meta-channel (imc-uk-moderation list); governance discussion was banned from the newswire itself. [3-0]
- **Radical tech collectives (Autistici/Inventati, RiseUp, May First)** provide movement infrastructure whose most crucial, least-credited function is maintenance/repair and nurturing security practice between groups; Signal emerged from this milieu. [2-1 — the specific RiseUp-personnel link is a single book's uncorroborated assertion]

### Mutual aid organizations (NYC COVID-19 groups)

- **No new platforms — novel configurations of consumer tools**: neighborhood Slack/WhatsApp, Zoom, volunteer-registration and request systems from Google Forms + Airtable, Google Suite, Venmo/Cashapp for funds. Some groups wrote small open-source glue bots. [3-0] (Soden & Owen, CSCW 2021, n=15 groups)
- **Digital-first organizing excluded vulnerable members**; most groups compensated with analog channels — paper flyers, zines, word of mouth, flyers translated into Spanish, Chinese, Arabic. Analog outreach is a required complement, not a fallback. [2-1]
- **Intake-then-dispatch is formalizable and prevents burnout**: the one group that automated a Slack-to-Airtable pipeline with dedicated intake and dispatch teams reported avoiding the volunteer churn most other groups experienced. [3-0]

## Cross-Cutting Patterns

Six patterns each appear independently in at least two contexts:

1. **Intake → verify → dispatch pipeline** with distinct field vs. remote roles.
2. **Shared editable ledger** as the central data object, with paper artifacts as intake instruments and degraded-mode transport.
3. **Channel matrix**: public/private × moderated/unmoderated, with trust concentrated in moderated channels bound to verification protocols.
4. **Bootstrap from pre-existing channels** — networks never form cold.
5. **Transparency-preserving moderation**: hide-not-delete, separate governance channel.
6. **Recurring failure modes**: carrier/platform chokepoints, state repression (jamming, arrests, subpoenas), tacit knowledge loss, digital exclusion.

## Design Implications for Riot

- **Objects to add**: `task` (dispatch ticket with open → claimed → done lifecycle and explicit handoff, the spreadsheet-row/index-card pattern), `verification` (signed attestation on another object: eyewitness / N-sources / method), `moderation_action` (hide-with-reason, publicly inspectable, never a delete). `need`/`offer` gain a claim/fulfillment lifecycle so a space works as the shared ledger.
- **Roles as capability templates**: intake, dispatcher, field verifier, moderator/curator map directly onto Meadowcap path-scoped capabilities.
- **The TXTMob matrix validates the dual-mode architecture**: open space = unmoderated public; its `/features/` = moderated public; publication space = moderated public (writer-gated); group = private, with path-restricted capabilities giving the moderated-private dispatch channel.
- **Paper interop is a requirement, not a nicety**: printable forms and QR round-trips for intake and distribution; flyers/zines as export targets, in multiple languages.
- **Runbooks fight tacit-knowledge loss**: "how this hub works" as first-class, seedable, user-editable packet content (checklists/runbooks), robust against disruption and transmissible.
- **Onboarding assumes existing groups**, not cold start: import-your-crew flows matter more than stranger discovery.
- **Governance meta-channel**: keep moderation disputes off the newswire — a per-space governance path, mirroring imc-uk-moderation.
- **Riot's decentralized sync answers the two documented infrastructure failure modes**: carrier/platform chokepoints (T-Mobile blocking TXTMob; Slack/Airtable/Venmo dependence) and server seizure — no carrier, no canonical server.

## Coverage Holes and Open Questions

No claims survived verification for: Turkey–Syria 2023 earthquake response; hurricane mutual aid beyond Sandy; linksunten.indymedia distribution under the active ban; spokescouncil/affinity-group internal process; and worker coops/timebanks/community fridges (COVID mutual aid is the nearest evidence, so findings skew crisis-mode). Open questions carried forward:

1. How do coops, timebanks, and community fridges coordinate day-to-day?
2. What publishing/distribution workflows do movement media use under an active state ban, and what does that imply for censorship-resistant sync?
3. What fallbacks actually worked with cell networks and power fully down (Turkey–Syria 2023, post-Maria Puerto Rico) — mesh radio, runners, paper — and at what scale?
4. How do private activist groups handle membership vetting and infiltration defense in practice? (Directly relevant to Riot's invite design.)

## Primary Sources

- Superstorm Research Lab volunteer document archive — https://superstormresearchlab.org/resources/volunteer-group-documents/
- Greenfield, "A Diagram of Occupy Sandy," Urban Omnibus (2013) — https://urbanomnibus.net/2013/02/a-diagram-of-occupy-sandy/
- Hirsch & Henry, "TXTmob: Text Messaging for Protest Swarms," CHI 2005 — https://www.researchgate.net/publication/221513633_TXTmob_text_messaging_for_protest_swarms
- Costanza-Chock, *Design Justice*, ch. 3 (MIT Press, open access) — https://designjustice.mitpress.mit.edu/pub/0v6035ye/release/2
- Soden & Owen, "Dilemmas in Mutual Aid," CSCW 2021 — http://robertsoden.io/files/mutual-aid-cscw.pdf
- IMC UK editorial guidelines — https://mob.indymedia.org.uk/en/static/editorial.html
- Verificado 19S participant interviews, Shareable Response podcast — https://www.shareable.net/podcasts/response-podcast-fighting-misinformation-in-the-aftermath-of-the-mexico-city-earthquake/
- Ford Foundation on Verificado 19S — https://www.fordfoundation.org/news-and-stories/stories/verificado19s-help-from-technology-in-the-aftermath-of-mexicos-earthquake/
